//! Execution Strategies - Strategy pattern implementation for pipeline execution
//!
//! This module provides different execution strategies for pipelines,
//! including sequential, parallel, and hybrid execution approaches.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::ci::{
    config::{CIPipeline, Stage},
    executor::PipelineExecutor,
    pipeline::{ExecutionStatus, TriggerInfo},
    template_engine::PipelineTemplateEngine,
    workspace::{Workspace, WorkspaceContext, WorkspaceManager},
};
use crate::error::{AppError, Result};

/// Execution context passed to strategies
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub execution_id: Uuid,
    pub pipeline_id: Uuid,
    pub correlation_id: Uuid,
    pub pipeline: CIPipeline,
    pub trigger_info: TriggerInfo,
    pub environment: HashMap<String, String>,
    pub started_at: DateTime<Utc>,
}

/// Result of pipeline execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub execution_id: Uuid,
    pub status: ExecutionStatus,
    pub stages_executed: usize,
    pub steps_executed: usize,
    pub duration_ms: u64,
    pub artifacts: Vec<String>,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

/// Types of execution strategies
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecutionStrategyType {
    Sequential,
    Parallel,
    Hybrid,
    Conditional,
}

/// Core trait for execution strategies
#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    /// Execute the pipeline with this strategy
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult>;

    /// Get the strategy type
    fn strategy_type(&self) -> ExecutionStrategyType;

    /// Check if this strategy supports rollback
    fn supports_rollback(&self) -> bool;

    /// Get strategy name for logging
    fn strategy_name(&self) -> &'static str;

    /// Validate if this strategy can handle the given context
    fn can_handle(&self, context: &ExecutionContext) -> bool;

    /// Get estimated execution time (for scheduling)
    fn estimate_execution_time(&self, context: &ExecutionContext) -> std::time::Duration {
        // Default estimation based on number of steps
        let total_steps: usize = context
            .pipeline
            .stages
            .iter()
            .map(|stage| stage.steps.len())
            .sum();
        std::time::Duration::from_secs((total_steps * 30) as u64) // 30 seconds per step estimate
    }
}

/// Sequential execution strategy - executes stages and steps one by one
pub struct SequentialExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
}

impl SequentialExecutionStrategy {
    pub fn new(executor: Arc<PipelineExecutor>, workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self {
            executor,
            workspace_manager,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for SequentialExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "sequential",
            "Starting sequential pipeline execution"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        // Use the original pipeline - workspace context injection handles path replacement
        let processed_pipeline = context.pipeline.clone();
        // Note: Template engine disabled to prevent double path processing
        // let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
        // template_engine.process_pipeline(&mut processed_pipeline)?;

        info!(
            execution_id = %context.execution_id,
            workspace_path = %workspace_context.workspace_path.display(),
            source_dir = %workspace_context.source_directory.display(),
            "Enhanced workspace created with path injection"
        );

        // Create a traditional workspace for compatibility
        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut stages_executed = 0;
        let mut steps_executed = 0;
        let mut logs = Vec::new();
        let mut artifacts = Vec::new();

        // Execute stages sequentially using processed pipeline
        for stage in &processed_pipeline.stages {
            debug!(
                execution_id = %context.execution_id,
                stage_name = %stage.name,
                "Executing stage sequentially"
            );

            // Execute stage with workspace context
            match self
                .execute_stage_sequential(context, stage, &workspace, &workspace_context)
                .await
            {
                Ok(stage_result) => {
                    stages_executed += 1;
                    steps_executed += stage.steps.len();
                    logs.extend(stage_result.logs);
                    artifacts.extend(stage_result.artifacts);
                }
                Err(e) => {
                    error!(
                        execution_id = %context.execution_id,
                        stage_name = %stage.name,
                        error = %e,
                        "Stage execution failed in sequential strategy"
                    );

                    // Cleanup workspace
                    if let Err(cleanup_err) = self
                        .workspace_manager
                        .cleanup_workspace(context.execution_id)
                        .await
                    {
                        warn!(
                            execution_id = %context.execution_id,
                            error = %cleanup_err,
                            "Failed to cleanup workspace after stage failure"
                        );
                    }

                    return Ok(ExecutionResult {
                        execution_id: context.execution_id,
                        status: ExecutionStatus::Failed,
                        stages_executed,
                        steps_executed,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        artifacts,
                        logs,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(
                execution_id = %context.execution_id,
                error = %e,
                "Failed to cleanup workspace after successful execution"
            );
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            stages_executed = stages_executed,
            steps_executed = steps_executed,
            duration_ms = duration.as_millis(),
            "Sequential pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed,
            steps_executed,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Sequential
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Sequential"
    }

    fn can_handle(&self, _context: &ExecutionContext) -> bool {
        true // Sequential strategy can handle any pipeline
    }
}

impl SequentialExecutionStrategy {
    async fn execute_stage_sequential(
        &self,
        context: &ExecutionContext,
        stage: &Stage,
        workspace: &Workspace,
        workspace_context: &WorkspaceContext,
    ) -> Result<StageExecutionResult> {
        let mut logs = Vec::new();
        let artifacts = Vec::new();

        // Merge environment variables with workspace context
        let mut env = context.environment.clone();
        if let Some(stage_env) = &stage.environment {
            env.extend(stage_env.clone());
        }
        // Add workspace environment variables
        env.extend(workspace_context.get_environment_variables());

        // Execute steps sequentially
        for step in &stage.steps {
            debug!(
                execution_id = %context.execution_id,
                stage_name = %stage.name,
                step_name = %step.name,
                "Executing step sequentially"
            );

            // Create a mock execution for the executor
            let execution = Arc::new(tokio::sync::RwLock::new(
                crate::ci::pipeline::PipelineExecution::new(
                    context.pipeline_id,
                    context.trigger_info.clone(),
                ),
            ));

            match self
                .executor
                .execute_step(execution, &stage.name, step, workspace, &env)
                .await
            {
                Ok(_) => {
                    logs.push(format!("Step '{}' completed successfully", step.name));
                }
                Err(e) => {
                    if step.continue_on_error.unwrap_or(false) {
                        warn!(
                            execution_id = %context.execution_id,
                            step_name = %step.name,
                            error = %e,
                            "Step failed but continuing due to continue_on_error flag"
                        );
                        logs.push(format!("Step '{}' failed but continuing: {}", step.name, e));
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(StageExecutionResult { logs, artifacts })
    }
}

/// Parallel execution strategy - executes stages in parallel where possible
pub struct ParallelExecutionStrategy {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
    max_parallel_stages: usize,
}

impl ParallelExecutionStrategy {
    pub fn new(
        executor: Arc<PipelineExecutor>,
        workspace_manager: Arc<WorkspaceManager>,
        max_parallel_stages: usize,
    ) -> Self {
        Self {
            executor,
            workspace_manager,
            max_parallel_stages,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for ParallelExecutionStrategy {
    async fn execute(&self, context: &ExecutionContext) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        info!(
            execution_id = %context.execution_id,
            strategy = "parallel",
            max_parallel_stages = self.max_parallel_stages,
            "Starting parallel pipeline execution"
        );

        // Create workspace with enhanced context
        let workspace_context = self
            .workspace_manager
            .create_workspace_with_context(context.execution_id)
            .await?;

        // Use the original pipeline - workspace context injection handles path replacement
        let processed_pipeline = context.pipeline.clone();
        // Note: Template engine disabled to prevent double path processing
        // let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
        // template_engine.process_pipeline(&mut processed_pipeline)?;

        info!(
            execution_id = %context.execution_id,
            workspace_path = %workspace_context.workspace_path.display(),
            source_dir = %workspace_context.source_directory.display(),
            "Enhanced workspace created with path injection for parallel execution"
        );

        // Create a traditional workspace for compatibility
        let workspace = Workspace {
            id: uuid::Uuid::new_v4(),
            path: workspace_context.workspace_path.clone(),
            execution_id: context.execution_id,
        };

        let mut stages_executed = 0;
        let mut steps_executed = 0;
        let mut logs = Vec::new();
        let mut artifacts = Vec::new();

        // Group stages that can be executed in parallel using processed pipeline
        let stage_groups = self.group_stages_for_parallel_execution(&processed_pipeline.stages);

        // Execute stage groups
        for (group_index, stage_group) in stage_groups.iter().enumerate() {
            debug!(
                execution_id = %context.execution_id,
                group_index = group_index,
                stages_in_group = stage_group.len(),
                "Executing stage group in parallel"
            );

            // Execute stages in this group in parallel
            let mut handles = Vec::new();

            for stage in stage_group {
                let context_clone = context.clone();
                let stage_clone = stage.clone();
                let workspace_clone = workspace.clone();
                let executor_clone = self.executor.clone();

                let workspace_context_clone = workspace_context.clone();
                let handle = tokio::spawn(async move {
                    Self::execute_stage_parallel(
                        executor_clone,
                        &context_clone,
                        &stage_clone,
                        &workspace_clone,
                        &workspace_context_clone,
                    )
                    .await
                });

                handles.push((stage.name.clone(), handle));
            }

            // Wait for all stages in the group to complete
            let mut group_success = true;
            for (stage_name, handle) in handles {
                match handle.await {
                    Ok(Ok(stage_result)) => {
                        debug!(
                            execution_id = %context.execution_id,
                            stage_name = %stage_name,
                            "Parallel stage execution completed successfully"
                        );
                        stages_executed += 1;
                        steps_executed += stage_result.steps_count;
                        logs.extend(stage_result.logs);
                        artifacts.extend(stage_result.artifacts);
                    }
                    Ok(Err(e)) => {
                        error!(
                            execution_id = %context.execution_id,
                            stage_name = %stage_name,
                            error = %e,
                            "Parallel stage execution failed"
                        );
                        group_success = false;
                        logs.push(format!("Stage '{}' failed: {}", stage_name, e));
                    }
                    Err(e) => {
                        error!(
                            execution_id = %context.execution_id,
                            stage_name = %stage_name,
                            error = %e,
                            "Parallel stage execution panicked"
                        );
                        group_success = false;
                        logs.push(format!("Stage '{}' panicked: {}", stage_name, e));
                    }
                }
            }

            // If any stage in the group failed, stop execution
            if !group_success {
                // Cleanup workspace
                if let Err(e) = self
                    .workspace_manager
                    .cleanup_workspace(context.execution_id)
                    .await
                {
                    warn!(
                        execution_id = %context.execution_id,
                        error = %e,
                        "Failed to cleanup workspace after parallel stage failure"
                    );
                }

                return Ok(ExecutionResult {
                    execution_id: context.execution_id,
                    status: ExecutionStatus::Failed,
                    stages_executed,
                    steps_executed,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    artifacts,
                    logs,
                    error: Some("One or more parallel stages failed".to_string()),
                });
            }
        }

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(context.execution_id)
            .await
        {
            warn!(
                execution_id = %context.execution_id,
                error = %e,
                "Failed to cleanup workspace after successful parallel execution"
            );
        }

        let duration = start_time.elapsed();

        info!(
            execution_id = %context.execution_id,
            stages_executed = stages_executed,
            steps_executed = steps_executed,
            duration_ms = duration.as_millis(),
            "Parallel pipeline execution completed successfully"
        );

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            status: ExecutionStatus::Success,
            stages_executed,
            steps_executed,
            duration_ms: duration.as_millis() as u64,
            artifacts,
            logs,
            error: None,
        })
    }

    fn strategy_type(&self) -> ExecutionStrategyType {
        ExecutionStrategyType::Parallel
    }

    fn supports_rollback(&self) -> bool {
        true
    }

    fn strategy_name(&self) -> &'static str {
        "Parallel"
    }

    fn can_handle(&self, context: &ExecutionContext) -> bool {
        // Check if pipeline has stages that can benefit from parallel execution
        context.pipeline.stages.len() > 1
            || context
                .pipeline
                .stages
                .iter()
                .any(|stage| stage.parallel.unwrap_or(false))
    }

    fn estimate_execution_time(&self, context: &ExecutionContext) -> std::time::Duration {
        // For parallel execution, estimate based on the longest stage group
        let stage_groups = self.group_stages_for_parallel_execution(&context.pipeline.stages);
        let max_group_steps = stage_groups
            .iter()
            .map(|group| group.iter().map(|stage| stage.steps.len()).sum::<usize>())
            .max()
            .unwrap_or(0);

        std::time::Duration::from_secs((max_group_steps * 30) as u64)
    }
}

impl ParallelExecutionStrategy {
    /// Group stages for parallel execution based on dependencies
    fn group_stages_for_parallel_execution(&self, stages: &[Stage]) -> Vec<Vec<Stage>> {
        // Simple implementation: each stage is its own group
        // In a more sophisticated implementation, you would analyze dependencies
        stages.iter().map(|stage| vec![stage.clone()]).collect()
    }

    async fn execute_stage_parallel(
        executor: Arc<PipelineExecutor>,
        context: &ExecutionContext,
        stage: &Stage,
        workspace: &Workspace,
        workspace_context: &WorkspaceContext,
    ) -> Result<ParallelStageResult> {
        let mut logs = Vec::new();
        let artifacts = Vec::new();
        let mut steps_count = 0;

        // Merge environment variables with workspace context
        let mut env = context.environment.clone();
        if let Some(stage_env) = &stage.environment {
            env.extend(stage_env.clone());
        }
        // Add workspace environment variables
        env.extend(workspace_context.get_environment_variables());

        // Check if stage should execute steps in parallel
        if stage.parallel.unwrap_or(false) {
            // Execute steps in parallel
            let mut handles = Vec::new();

            for step in &stage.steps {
                let execution = Arc::new(tokio::sync::RwLock::new(
                    crate::ci::pipeline::PipelineExecution::new(
                        context.pipeline_id,
                        context.trigger_info.clone(),
                    ),
                ));

                let executor_clone = executor.clone();
                let stage_name = stage.name.clone();
                let step_clone = step.clone();
                let workspace_clone = workspace.clone();
                let env_clone = env.clone();

                let handle = tokio::spawn(async move {
                    executor_clone
                        .execute_step(
                            execution,
                            &stage_name,
                            &step_clone,
                            &workspace_clone,
                            &env_clone,
                        )
                        .await
                });

                handles.push((step.name.clone(), handle));
            }

            // Wait for all steps to complete
            for (step_name, handle) in handles {
                match handle.await {
                    Ok(Ok(_)) => {
                        logs.push(format!(
                            "Parallel step '{}' completed successfully",
                            step_name
                        ));
                        steps_count += 1;
                    }
                    Ok(Err(e)) => {
                        return Err(AppError::InternalServerError(format!(
                            "Parallel step '{}' failed: {}",
                            step_name, e
                        )));
                    }
                    Err(e) => {
                        return Err(AppError::InternalServerError(format!(
                            "Parallel step '{}' panicked: {}",
                            step_name, e
                        )));
                    }
                }
            }
        } else {
            // Execute steps sequentially within the stage
            for step in &stage.steps {
                let execution = Arc::new(tokio::sync::RwLock::new(
                    crate::ci::pipeline::PipelineExecution::new(
                        context.pipeline_id,
                        context.trigger_info.clone(),
                    ),
                ));

                match executor
                    .execute_step(execution, &stage.name, step, workspace, &env)
                    .await
                {
                    Ok(_) => {
                        logs.push(format!("Step '{}' completed successfully", step.name));
                        steps_count += 1;
                    }
                    Err(e) => {
                        if step.continue_on_error.unwrap_or(false) {
                            logs.push(format!("Step '{}' failed but continuing: {}", step.name, e));
                            steps_count += 1;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
        }

        Ok(ParallelStageResult {
            logs,
            artifacts,
            steps_count,
        })
    }
}

/// Factory for creating execution strategies
pub struct ExecutionStrategyFactory {
    strategies: HashMap<ExecutionStrategyType, Box<dyn ExecutionStrategy>>,
}

impl ExecutionStrategyFactory {
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    /// Register a strategy
    pub fn register_strategy(&mut self, strategy: Box<dyn ExecutionStrategy>) {
        let strategy_type = strategy.strategy_type();
        self.strategies.insert(strategy_type, strategy);
    }

    /// Create a strategy instance
    pub fn create_strategy(
        &self,
        strategy_type: ExecutionStrategyType,
    ) -> Result<&dyn ExecutionStrategy> {
        self.strategies
            .get(&strategy_type)
            .map(|s| s.as_ref())
            .ok_or_else(|| {
                AppError::InternalServerError(format!("Strategy not found: {:?}", strategy_type))
            })
    }

    /// Get available strategies
    pub fn available_strategies(&self) -> Vec<ExecutionStrategyType> {
        self.strategies.keys().cloned().collect()
    }

    /// Find best strategy for context
    pub fn find_best_strategy(&self, context: &ExecutionContext) -> Option<ExecutionStrategyType> {
        // Find the first strategy that can handle the context
        for (strategy_type, strategy) in &self.strategies {
            if strategy.can_handle(context) {
                return Some(strategy_type.clone());
            }
        }
        None
    }
}

impl Default for ExecutionStrategyFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper structs for execution results
#[derive(Debug)]
struct StageExecutionResult {
    logs: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Debug)]
struct ParallelStageResult {
    logs: Vec<String>,
    artifacts: Vec<String>,
    steps_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{Step, StepType, StepConfig, Stage};
    use std::collections::HashMap;

    fn create_test_context() -> ExecutionContext {
        let pipeline = CIPipeline {
            mongo_id: None,
            id: Some(Uuid::new_v4()),
            name: "test-pipeline".to_string(),
            description: Some("Test pipeline".to_string()),
            triggers: vec![],
            stages: vec![
                Stage {
                    name: "build".to_string(),
                    condition: None,
                    parallel: Some(false),
                    timeout: Some(600),     // 10-minute stage timeout
                    retry_count: Some(0),
                    environment: None,
                    steps: vec![
                        Step {
                            name: "compile".to_string(),
                            step_type: StepType::Shell,
                            config: StepConfig {
                                command: Some("echo 'Building...'".to_string()),
                                ..Default::default()
                            },
                            condition: None,         // ✅ now filled
                            continue_on_error: Some(false),
                            timeout: Some(300),      // ✅ e.g. 5-minute step timeout
                        }
                    ],
                }
            ],
            environment: HashMap::new(),
            timeout: Some(3600),            // 1-hour pipeline timeout
            retry_count: Some(0),
            notifications: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        };
    
        ExecutionContext {
            execution_id: Uuid::new_v4(),
            pipeline_id: pipeline.id.unwrap(),
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

    #[test]
    fn test_execution_strategy_types() {
        assert_eq!(
            ExecutionStrategyType::Sequential,
            ExecutionStrategyType::Sequential
        );
        assert_ne!(
            ExecutionStrategyType::Sequential,
            ExecutionStrategyType::Parallel
        );
    }

    #[test]
    fn test_execution_context_creation() {
        let context = create_test_context();
        assert!(!context.execution_id.is_nil());
        assert!(!context.pipeline_id.is_nil());
        assert!(!context.correlation_id.is_nil());
        assert_eq!(context.pipeline.name, "test-pipeline");
    }

    #[test]
    fn test_execution_strategy_factory() {
        let factory = ExecutionStrategyFactory::new();
        assert!(factory.available_strategies().is_empty());

        let context = create_test_context();
        assert!(factory.find_best_strategy(&context).is_none());
    }
}
