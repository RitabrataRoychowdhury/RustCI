//! Execution Coordinator - Manages pipeline execution strategies
//!
//! This coordinator selects and manages execution strategies for different
//! pipeline configurations and handles resource management.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::patterns::correlation::CorrelationTracker;
use crate::error::{AppError, Result};

use super::{ExecutionContext, ExecutionResult, ExecutionStrategyFactory, ExecutionStrategyType};

/// Resource manager for tracking execution resources
#[derive(Debug, Clone)]
pub struct ResourceManager {
    max_concurrent_executions: usize,
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionContext>>>,
    resource_limits: ResourceLimits,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_mb: usize,
    pub max_cpu_cores: usize,
    pub max_disk_gb: usize,
    pub max_execution_time_minutes: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096,
            max_cpu_cores: 4,
            max_disk_gb: 10,
            max_execution_time_minutes: 60,
        }
    }
}

impl ResourceManager {
    pub fn new(max_concurrent_executions: usize) -> Self {
        Self {
            max_concurrent_executions,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            resource_limits: ResourceLimits::default(),
        }
    }

    pub async fn can_start_execution(&self) -> bool {
        let active = self.active_executions.read().await;
        active.len() < self.max_concurrent_executions
    }

    pub async fn register_execution(
        &self,
        execution_id: Uuid,
        context: ExecutionContext,
    ) -> Result<()> {
        let mut active = self.active_executions.write().await;

        if active.len() >= self.max_concurrent_executions {
            return Err(AppError::InternalServerError(
                "Maximum concurrent executions reached".to_string(),
            ));
        }

        active.insert(execution_id, context);
        debug!(execution_id = %execution_id, "Registered execution with resource manager");
        Ok(())
    }

    pub async fn unregister_execution(&self, execution_id: Uuid) {
        let mut active = self.active_executions.write().await;
        active.remove(&execution_id);
        debug!(execution_id = %execution_id, "Unregistered execution from resource manager");
    }

    pub async fn get_active_executions(&self) -> Vec<Uuid> {
        let active = self.active_executions.read().await;
        active.keys().cloned().collect()
    }

    pub async fn get_resource_usage(&self) -> ResourceUsage {
        let active = self.active_executions.read().await;
        ResourceUsage {
            active_executions: active.len(),
            max_executions: self.max_concurrent_executions,
            memory_usage_mb: 0, // Would be calculated from actual resource monitoring
            cpu_usage_percent: 0.0,
            disk_usage_gb: 0,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResourceUsage {
    pub active_executions: usize,
    pub max_executions: usize,
    pub memory_usage_mb: usize,
    pub cpu_usage_percent: f64,
    pub disk_usage_gb: usize,
}

/// Execution Coordinator manages pipeline execution strategies and resources
pub struct ExecutionCoordinator {
    strategy_factory: Arc<ExecutionStrategyFactory>,
    resource_manager: Arc<ResourceManager>,
    correlation_tracker: Arc<CorrelationTracker>,
    active_executions: Arc<RwLock<HashMap<Uuid, tokio::task::JoinHandle<Result<ExecutionResult>>>>>,
}

impl ExecutionCoordinator {
    /// Create a new execution coordinator with strategies
    pub fn new(correlation_tracker: Arc<CorrelationTracker>) -> Self {
        Self {
            strategy_factory: Arc::new(ExecutionStrategyFactory::new()),
            resource_manager: Arc::new(ResourceManager::new(10)), // Default max 10 concurrent
            correlation_tracker,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new execution coordinator with initialized strategies
    pub fn with_strategies(
        correlation_tracker: Arc<CorrelationTracker>,
        executor: Arc<crate::ci::executor::PipelineExecutor>,
        workspace_manager: Arc<crate::ci::workspace::WorkspaceManager>,
    ) -> Self {
        // Use the multi-tier strategy factory that includes all pipeline types
        let strategy_factory = ExecutionStrategyFactory::with_multi_tier_strategies(
            executor,
            workspace_manager,
        );
        
        Self {
            strategy_factory: Arc::new(strategy_factory),
            resource_manager: Arc::new(ResourceManager::new(10)),
            correlation_tracker,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom resource limits
    pub fn with_resource_limits(
        correlation_tracker: Arc<CorrelationTracker>,
        max_concurrent: usize,
        resource_limits: ResourceLimits,
    ) -> Self {
        let mut resource_manager = ResourceManager::new(max_concurrent);
        resource_manager.resource_limits = resource_limits;

        Self {
            strategy_factory: Arc::new(ExecutionStrategyFactory::new()),
            resource_manager: Arc::new(resource_manager),
            correlation_tracker,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a pipeline using the appropriate strategy
    pub async fn execute_pipeline(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let execution_id = context.execution_id;

        info!(
            execution_id = %execution_id,
            pipeline_id = %context.pipeline_id,
            correlation_id = %context.correlation_id,
            "Starting pipeline execution coordination"
        );

        // Set correlation context
        self.correlation_tracker
            .set_correlation_id(context.correlation_id)
            .await;

        // Check resource availability
        if !self.resource_manager.can_start_execution().await {
            return Err(AppError::InternalServerError(
                "Cannot start execution: resource limits exceeded".to_string(),
            ));
        }

        // Register execution with resource manager
        self.resource_manager
            .register_execution(execution_id, context.clone())
            .await?;

        // Get pipeline type and try to use specific strategy
        let pipeline_type = context.pipeline.get_pipeline_type();
        
        debug!(
            execution_id = %execution_id,
            pipeline_type = ?pipeline_type,
            "Detected pipeline type"
        );

        // Try to get strategy for specific pipeline type first
        let strategy = match self.strategy_factory.create_strategy_for_pipeline_type(pipeline_type) {
            Ok(strategy) => {
                info!(
                    execution_id = %execution_id,
                    strategy_name = strategy.strategy_name(),
                    "Using pipeline type-specific strategy"
                );
                strategy
            }
            Err(_) => {
                // Fallback to traditional strategy selection
                let strategy_type = self.determine_execution_strategy(context);
                
                debug!(
                    execution_id = %execution_id,
                    strategy_type = ?strategy_type,
                    "Falling back to traditional strategy selection"
                );
                
                self.strategy_factory.create_strategy(strategy_type)?
            }
        };

        // Execute pipeline with selected strategy
        let execution_result = strategy.execute(context).await;

        // Unregister from resource manager
        self.resource_manager
            .unregister_execution(execution_id)
            .await;

        match execution_result {
            Ok(result) => {
                info!(
                    execution_id = %execution_id,
                    duration_ms = result.duration_ms,
                    "Pipeline execution completed successfully"
                );
                Ok(result)
            }
            Err(e) => {
                error!(
                    execution_id = %execution_id,
                    error = %e,
                    "Pipeline execution failed"
                );
                Err(e)
            }
        }
    }

    /// Execute pipeline asynchronously
    pub async fn execute_pipeline_async(&self, context: ExecutionContext) -> Result<Uuid> {
        let execution_id = context.execution_id;

        // Check resource availability
        if !self.resource_manager.can_start_execution().await {
            return Err(AppError::InternalServerError(
                "Cannot start execution: resource limits exceeded".to_string(),
            ));
        }

        // Clone necessary data for the async task
        let coordinator = self.clone();

        // Spawn async execution task
        let handle = tokio::spawn(async move { coordinator.execute_pipeline(&context).await });

        // Store the handle for potential cancellation
        {
            let mut active = self.active_executions.write().await;
            active.insert(execution_id, handle);
        }

        info!(execution_id = %execution_id, "Started asynchronous pipeline execution");
        Ok(execution_id)
    }

    /// Cancel a running execution
    pub async fn cancel_execution(&self, execution_id: Uuid) -> Result<()> {
        info!(execution_id = %execution_id, "Cancelling pipeline execution");

        // Get and remove the execution handle
        let handle = {
            let mut active = self.active_executions.write().await;
            active.remove(&execution_id)
        };

        if let Some(handle) = handle {
            // Cancel the task
            handle.abort();

            // Unregister from resource manager
            self.resource_manager
                .unregister_execution(execution_id)
                .await;

            info!(execution_id = %execution_id, "Pipeline execution cancelled");
            Ok(())
        } else {
            warn!(execution_id = %execution_id, "Execution not found for cancellation");
            Err(AppError::NotFound("Execution not found".to_string()))
        }
    }

    /// Get active executions
    pub async fn get_active_executions(&self) -> Vec<Uuid> {
        self.resource_manager.get_active_executions().await
    }

    /// Get resource usage statistics
    pub async fn get_resource_usage(&self) -> ResourceUsage {
        self.resource_manager.get_resource_usage().await
    }

    /// Determine the best execution strategy for a pipeline
    fn determine_execution_strategy(&self, context: &ExecutionContext) -> ExecutionStrategyType {
        // First, try to use the pipeline type to determine strategy
        let pipeline_type = context.pipeline.get_pipeline_type();
        
        // Map pipeline types to execution strategy types
        match pipeline_type {
            crate::ci::config::PipelineType::Minimal => {
                // Check if we have a Minimal strategy registered
                if self.strategy_factory.available_strategies().iter()
                    .any(|s| matches!(s, ExecutionStrategyType::Sequential)) {
                    // For now, use Sequential as Minimal isn't in ExecutionStrategyType enum
                    ExecutionStrategyType::Sequential
                } else {
                    ExecutionStrategyType::Sequential
                }
            }
            crate::ci::config::PipelineType::Simple => {
                // Use Sequential for simple linear execution
                ExecutionStrategyType::Sequential
            }
            crate::ci::config::PipelineType::Standard => {
                // Analyze if parallel execution would be beneficial
                let has_parallel_stages = context
                    .pipeline
                    .stages
                    .iter()
                    .any(|stage| stage.parallel.unwrap_or(false));

                if has_parallel_stages {
                    ExecutionStrategyType::Parallel
                } else {
                    ExecutionStrategyType::Sequential
                }
            }
            crate::ci::config::PipelineType::Advanced => {
                // Advanced pipelines can benefit from parallel execution
                // Check if we have matrix configurations or multiple jobs
                let has_matrix = context.pipeline.matrix.is_some() || 
                    context.pipeline.jobs.as_ref().map_or(false, |jobs| {
                        jobs.values().any(|job| match job {
                            crate::ci::config::PipelineJob::Detailed { matrix, .. } => matrix.is_some(),
                            _ => false,
                        })
                    });

                if has_matrix {
                    ExecutionStrategyType::Parallel
                } else {
                    // Fallback to analyzing stage structure
                    let has_parallel_stages = context
                        .pipeline
                        .stages
                        .iter()
                        .any(|stage| stage.parallel.unwrap_or(false));

                    if has_parallel_stages {
                        ExecutionStrategyType::Parallel
                    } else {
                        ExecutionStrategyType::Sequential
                    }
                }
            }
        }
    }

    /// Get strategy factory for testing or advanced usage
    pub fn strategy_factory(&self) -> &Arc<ExecutionStrategyFactory> {
        &self.strategy_factory
    }

    /// Get resource manager
    pub fn resource_manager(&self) -> &Arc<ResourceManager> {
        &self.resource_manager
    }
}

// Implement Clone for ExecutionCoordinator to support async spawning
impl Clone for ExecutionCoordinator {
    fn clone(&self) -> Self {
        Self {
            strategy_factory: self.strategy_factory.clone(),
            resource_manager: self.resource_manager.clone(),
            correlation_tracker: self.correlation_tracker.clone(),
            active_executions: self.active_executions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{CIPipeline, Stage, Step, StepConfig, StepType};
    use crate::ci::pipeline::TriggerInfo;
    use std::collections::HashMap;
    use uuid::Uuid;                  // ✅ Needed for `Uuid::new_v4()`
    use chrono::Utc;                 // ✅ Needed for `Utc::now()`

    fn create_test_context() -> ExecutionContext {
        // ✅ Build the test pipeline
        let mut pipeline = CIPipeline::new("test-pipeline".to_string());
        pipeline.description = Some("Test pipeline".to_string());
        pipeline.stages = vec![
                Stage {
                    name: "build".to_string(),
                    condition: None,
                    steps: vec![
                        Step {
                            name: "compile".to_string(),
                            step_type: StepType::Shell,
                            config: StepConfig {
                                command: Some("echo 'Building...'".to_string()),
                                ..Default::default()
                            },
                            condition: None,
                            continue_on_error: Some(false),
                            timeout: None,
                        }
                    ],
                    parallel: Some(false),
                    timeout: Some(600),    // 10-minute stage timeout
                    retry_count: Some(0),
                    environment: None,
                }
            ];
        
        pipeline.timeout = Some(3600);           // 1-hour pipeline timeout
        pipeline.retry_count = Some(0);
    
        // ✅ Build the ExecutionContext
        ExecutionContext {
            execution_id: Uuid::new_v4(),
            pipeline_id: pipeline.id.expect("Pipeline should have an ID"),
            correlation_id: Uuid::new_v4(),
            pipeline,
            trigger_info: TriggerInfo {
                trigger_type: "manual".to_string(),
                triggered_by: Some("test".to_string()),
                commit_hash: None,
                branch: None,
                repository: None,
                webhook_payload: None,
            },
            environment: HashMap::new(),
            started_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_resource_manager() {
        let manager = ResourceManager::new(2);

        assert!(manager.can_start_execution().await);

        let context1 = create_test_context();
        let context2 = create_test_context();
        let context3 = create_test_context();

        // Register two executions (should succeed)
        assert!(manager
            .register_execution(context1.execution_id, context1)
            .await
            .is_ok());
        assert!(manager
            .register_execution(context2.execution_id, context2)
            .await
            .is_ok());

        // Third execution should fail (exceeds limit)
        assert!(manager
            .register_execution(context3.execution_id, context3)
            .await
            .is_err());

        // Should not be able to start more executions
        assert!(!manager.can_start_execution().await);
    }

    #[tokio::test]
    async fn test_execution_coordinator_creation() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let coordinator = ExecutionCoordinator::new(correlation_tracker);

        let active = coordinator.get_active_executions().await;
        assert!(active.is_empty());

        let usage = coordinator.get_resource_usage().await;
        assert_eq!(usage.active_executions, 0);
    }

    #[tokio::test]
    async fn test_strategy_determination() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let coordinator = ExecutionCoordinator::new(correlation_tracker);

        let context = create_test_context();
        let strategy_type = coordinator.determine_execution_strategy(&context);

        // Should select sequential for simple pipeline
        assert_eq!(strategy_type, ExecutionStrategyType::Sequential);
    }
}
