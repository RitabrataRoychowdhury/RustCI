//! CI Engine Orchestrator - Main entry point for pipeline execution
//!
//! This orchestrator coordinates between pipeline management, execution strategies,
//! SAGA orchestration, and monitoring systems.

use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::ci::pipeline::{ExecutionStatus, TriggerInfo};
use crate::core::{
    events::EventBus,
    sagas::{SagaContext, SagaOrchestrator},
};
use crate::error::Result;

use super::{
    ExecutionContext, ExecutionCoordinator, ExecutionMonitoring, ExecutionResult,
    PipelineExecutionSagaFactory, PipelineManager,
};

/// Main CI Engine Orchestrator that coordinates all pipeline execution
pub struct CIEngineOrchestrator {
    pipeline_manager: Arc<PipelineManager>,
    execution_coordinator: Arc<ExecutionCoordinator>,
    event_bus: Arc<EventBus>,
    saga_orchestrator: Arc<SagaOrchestrator>,
    saga_factory: Arc<PipelineExecutionSagaFactory>,
    monitoring: Arc<ExecutionMonitoring>,
}

impl CIEngineOrchestrator {
    /// Create a new CI Engine Orchestrator
    pub fn new(
        pipeline_manager: Arc<PipelineManager>,
        execution_coordinator: Arc<ExecutionCoordinator>,
        event_bus: Arc<EventBus>,
        saga_orchestrator: Arc<SagaOrchestrator>,
        saga_factory: Arc<PipelineExecutionSagaFactory>,
        monitoring: Arc<ExecutionMonitoring>,
    ) -> Self {
        Self {
            pipeline_manager,
            execution_coordinator,
            event_bus,
            saga_orchestrator,
            saga_factory,
            monitoring,
        }
    }

    /// Execute a pipeline with SAGA-based rollback support
    pub async fn execute_pipeline(
        &self,
        pipeline_id: Uuid,
        trigger_info: TriggerInfo,
        environment: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Uuid> {
        let correlation_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        info!(
            pipeline_id = %pipeline_id,
            execution_id = %execution_id,
            correlation_id = %correlation_id,
            "Starting pipeline execution with SAGA orchestration"
        );

        // Get pipeline configuration
        let pipeline = self.pipeline_manager.get_pipeline(pipeline_id).await?;

        // Validate pipeline
        self.pipeline_manager.validate_pipeline(&pipeline).await?;

        // Create execution context
        let execution_context = ExecutionContext {
            execution_id,
            pipeline_id,
            correlation_id,
            pipeline: pipeline.clone(),
            trigger_info: trigger_info.clone(),
            environment: environment.unwrap_or_default(),
            started_at: chrono::Utc::now(),
        };

        // Start monitoring
        self.monitoring
            .start_execution_monitoring(execution_id, &execution_context)
            .await?;

        // For now, execute pipeline directly through execution coordinator
        // In a full implementation, we would integrate SAGA steps properly
        let execution_result = self
            .execution_coordinator
            .execute_pipeline(&execution_context)
            .await;

        // Create SAGA context for potential future rollback support
        let _saga_context = SagaContext::new(execution_id, correlation_id);

        // TODO: Integrate SAGA steps properly when SagaOrchestrator provides better API
        // For now, we'll handle the execution result directly
        let saga_result = match execution_result {
            Ok(_result) => {
                info!(
                    execution_id = %execution_id,
                    "Pipeline execution completed successfully through coordinator"
                );
                Ok(crate::core::sagas::SagaExecution {
                    saga_id: execution_id,
                    correlation_id,
                    saga_name: "pipeline-execution".to_string(),
                    status: crate::core::sagas::SagaStatus::Completed,
                    started_at: execution_context.started_at,
                    completed_at: Some(chrono::Utc::now()),
                    current_step: execution_context.pipeline.stages.len(),
                    total_steps: execution_context.pipeline.stages.len(),
                    step_results: Vec::new(),
                    context: SagaContext::new(execution_id, correlation_id),
                    error: None,
                })
            }
            Err(e) => {
                error!(
                    execution_id = %execution_id,
                    error = %e,
                    "Pipeline execution failed through coordinator"
                );
                Ok(crate::core::sagas::SagaExecution {
                    saga_id: execution_id,
                    correlation_id,
                    saga_name: "pipeline-execution".to_string(),
                    status: crate::core::sagas::SagaStatus::Failed,
                    started_at: execution_context.started_at,
                    completed_at: Some(chrono::Utc::now()),
                    current_step: 0,
                    total_steps: execution_context.pipeline.stages.len(),
                    step_results: Vec::new(),
                    context: SagaContext::new(execution_id, correlation_id),
                    error: Some(e.to_string()),
                })
            }
        };

        match saga_result {
            Ok(saga_execution) => {
                info!(
                    execution_id = %execution_id,
                    saga_status = ?saga_execution.status,
                    "Pipeline execution completed through SAGA"
                );

                // Update monitoring with success
                self.monitoring
                    .complete_execution_monitoring(execution_id, ExecutionStatus::Success)
                    .await?;

                Ok(execution_id)
            }
            Err(e) => {
                error!(
                    execution_id = %execution_id,
                    error = %e,
                    "Pipeline execution failed, SAGA compensation may have been triggered"
                );

                // Update monitoring with failure
                self.monitoring
                    .complete_execution_monitoring(execution_id, ExecutionStatus::Failed)
                    .await?;

                Err(e)
            }
        }
    }

    /// Execute pipeline stages using the execution coordinator
    pub async fn execute_pipeline_stages(
        &self,
        execution_context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        debug!(
            execution_id = %execution_context.execution_id,
            pipeline_id = %execution_context.pipeline_id,
            "Executing pipeline stages through coordinator"
        );

        // Delegate to execution coordinator
        self.execution_coordinator
            .execute_pipeline(execution_context)
            .await
    }

    /// Cancel a running pipeline execution
    pub async fn cancel_execution(&self, execution_id: Uuid) -> Result<()> {
        info!(execution_id = %execution_id, "Cancelling pipeline execution");

        // Cancel through execution coordinator
        self.execution_coordinator
            .cancel_execution(execution_id)
            .await?;

        // Update monitoring
        self.monitoring
            .complete_execution_monitoring(execution_id, ExecutionStatus::Cancelled)
            .await?;

        Ok(())
    }

    /// Get execution status and metrics
    pub async fn get_execution_status(&self, execution_id: Uuid) -> Result<ExecutionStatus> {
        self.monitoring.get_execution_status(execution_id).await
    }

    /// Get pipeline execution metrics
    pub async fn get_execution_metrics(
        &self,
        execution_id: Uuid,
    ) -> Result<super::ExecutionMetrics> {
        self.monitoring.get_execution_metrics(execution_id).await
    }

    /// List active executions
    pub async fn list_active_executions(&self) -> Result<Vec<Uuid>> {
        self.monitoring.list_active_executions().await
    }

    /// Get pipeline manager for pipeline operations
    pub fn pipeline_manager(&self) -> &Arc<PipelineManager> {
        &self.pipeline_manager
    }

    /// Get execution coordinator for direct execution operations
    pub fn execution_coordinator(&self) -> &Arc<ExecutionCoordinator> {
        &self.execution_coordinator
    }

    /// Get monitoring system
    pub fn monitoring(&self) -> &Arc<ExecutionMonitoring> {
        &self.monitoring
    }

    /// Create a new pipeline (delegate to pipeline manager)
    pub async fn create_pipeline(&self, pipeline: crate::ci::config::CIPipeline) -> Result<Uuid> {
        self.pipeline_manager.create_pipeline(pipeline).await
    }

    /// Trigger pipeline execution (wrapper around execute_pipeline)
    pub async fn trigger_pipeline(
        &self,
        pipeline_id: Uuid,
        trigger_info: TriggerInfo,
        environment: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Uuid> {
        self.execute_pipeline(pipeline_id, trigger_info, environment)
            .await
    }

    /// Get pipeline execution data (delegate to monitoring)
    pub async fn get_execution(
        &self,
        execution_id: Uuid,
    ) -> Result<crate::ci::pipeline::PipelineExecution> {
        // For now, create a basic PipelineExecution from monitoring data
        let monitoring_data = self
            .monitoring
            .get_execution_monitoring_data(execution_id)
            .await?;

        Ok(crate::ci::pipeline::PipelineExecution {
            mongo_id: None,
            id: monitoring_data.execution_id,
            pipeline_id: monitoring_data.pipeline_id,
            status: monitoring_data.status,
            started_at: Some(monitoring_data.started_at),
            finished_at: monitoring_data.completed_at,
            duration: monitoring_data.duration_ms,
            trigger_info: TriggerInfo {
                trigger_type: "api".to_string(),
                triggered_by: Some("system".to_string()),
                commit_hash: None,
                branch: None,
                repository: None,
                webhook_payload: None,
            },
            stages: Vec::new(), // Would be populated from actual execution data
            environment: std::collections::HashMap::new(),
            logs: Vec::new(),
            artifacts: Vec::new(),
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        })
    }

    /// List all pipelines (delegate to pipeline manager)
    pub async fn list_pipelines(&self) -> Result<Vec<crate::ci::config::CIPipeline>> {
        self.pipeline_manager.list_pipelines().await
    }

    /// List executions for a pipeline or all executions
    pub async fn list_executions(
        &self,
        pipeline_id: Option<Uuid>,
    ) -> Result<Vec<crate::ci::pipeline::PipelineExecution>> {
        // Get execution history from monitoring
        let monitoring_data = self.monitoring.get_execution_history(Some(100)).await?;

        let filtered_data = if let Some(pid) = pipeline_id {
            monitoring_data
                .into_iter()
                .filter(|d| d.pipeline_id == pid)
                .collect()
        } else {
            monitoring_data
        };

        // Convert monitoring data to PipelineExecution
        let executions = filtered_data
            .into_iter()
            .map(|data| crate::ci::pipeline::PipelineExecution {
                mongo_id: None,
                id: data.execution_id,
                pipeline_id: data.pipeline_id,
                status: data.status,
                started_at: Some(data.started_at),
                finished_at: data.completed_at,
                duration: data.duration_ms,
                trigger_info: TriggerInfo {
                    trigger_type: "api".to_string(),
                    triggered_by: Some("system".to_string()),
                    commit_hash: None,
                    branch: None,
                    repository: None,
                    webhook_payload: None,
                },
                stages: Vec::new(),
                environment: std::collections::HashMap::new(),
                artifacts: Vec::new(),
                logs: Vec::new(),
                created_at: Some(chrono::Utc::now()),
                updated_at: Some(chrono::Utc::now()),
            })
            .collect();

        Ok(executions)
    }

    /// Get pipeline configuration (delegate to pipeline manager)
    pub async fn get_pipeline(&self, pipeline_id: Uuid) -> Result<crate::ci::config::CIPipeline> {
        self.pipeline_manager.get_pipeline(pipeline_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{CIPipeline, Stage, Step, StepConfig, StepType};
    use crate::core::{sagas::SagaPersistence, CorrelationTracker};
    use std::collections::HashMap;
    use crate::ci::engine::MetricsCollector;
    use std::env;
    use crate::infrastructure::database::DatabaseManager;

    // Mock implementations for testing
    struct MockSagaPersistence;

    #[async_trait::async_trait]
    impl SagaPersistence for MockSagaPersistence {
        async fn save_execution(
            &self,
            _execution: &crate::core::sagas::SagaExecution,
        ) -> Result<()> {
            Ok(())
        }

        async fn load_execution(
            &self,
            _saga_id: Uuid,
        ) -> Result<Option<crate::core::sagas::SagaExecution>> {
            Ok(None)
        }

        async fn find_by_status(
            &self,
            _status: crate::core::sagas::SagaStatus,
        ) -> Result<Vec<crate::core::sagas::SagaExecution>> {
            Ok(Vec::new())
        }

        async fn find_by_correlation_id(
            &self,
            _correlation_id: Uuid,
        ) -> Result<Vec<crate::core::sagas::SagaExecution>> {
            Ok(Vec::new())
        }

        async fn delete_execution(&self, _saga_id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn get_statistics(&self) -> Result<crate::core::sagas::SagaStatistics> {
            Ok(crate::core::sagas::SagaStatistics {
                total_executions: 0,
                completed_executions: 0,
                failed_executions: 0,
                compensated_executions: 0,
                average_duration_ms: 0.0,
                success_rate: 0.0,
            })
        }
    }

    fn create_test_pipeline() -> CIPipeline {
        CIPipeline {
            mongo_id: None,
            id: Some(Uuid::new_v4()),
            name: "test-pipeline".to_string(),
            description: Some("Test pipeline".to_string()),
            triggers: vec![],                      // No triggers for now
            stages: vec![
                Stage {
                    name: "build".to_string(),
                    condition: None,               // No condition
                    parallel: Some(false),
                    timeout: Some(600),            // 10-minute timeout
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
                            condition: None,       // No step-level condition
                            continue_on_error: Some(false),
                            timeout: Some(300),    // 5 min timeout
                        }
                    ],
                }
            ],
            environment: HashMap::new(),
            timeout: Some(3600),                   // 1 hour pipeline timeout
            retry_count: Some(0),                  // No retries for now
            notifications: None,                   // No notifications
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }    

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker.clone(), None));
        let saga_persistence = Arc::new(MockSagaPersistence);

        let saga_orchestrator = Arc::new(SagaOrchestrator::new(
            "pipeline-execution".to_string(),
            event_bus.clone(),
            correlation_tracker.clone(),
            saga_persistence,
        ));

        // Create mock components
        let mongo_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    
        let mongo_db = env::var("MONGODB_DATABASE").unwrap_or_else(|_| "dqms".into());

        let pipeline_manager = Arc::new(PipelineManager::new(
            DatabaseManager::new(&mongo_uri, &mongo_db) // ✅ borrow
                .await
                .unwrap(),
            event_bus.clone(),
        ));        

        let execution_coordinator =
            Arc::new(ExecutionCoordinator::new(correlation_tracker.clone()));

        let monitoring = Arc::new(ExecutionMonitoring::new(correlation_tracker.clone()));

        // Create mock saga factory
        let workspace_manager = Arc::new(crate::ci::workspace::WorkspaceManager::new(
            std::path::PathBuf::from("/tmp"),
        ));
        let connector_manager = crate::ci::connectors::ConnectorManager::new();
        let executor = Arc::new(crate::ci::executor::PipelineExecutor::new(
            connector_manager,
            std::path::PathBuf::from("/tmp/cache"),
            std::path::PathBuf::from("/tmp/deploy"),
        ));
        let metrics_collector = Arc::new(MetricsCollector::new());

        let saga_factory = Arc::new(PipelineExecutionSagaFactory::new(
            executor,
            workspace_manager,
            monitoring.clone(),
            metrics_collector,
        ));

        let orchestrator = CIEngineOrchestrator::new(
            pipeline_manager,
            execution_coordinator,
            event_bus,
            saga_orchestrator,
            saga_factory,
            monitoring,
        );

        // Test that orchestrator was created successfully
        assert!(
            !orchestrator
                .list_active_executions()
                .await
                .unwrap()
                .is_empty()
                == false
        );
    }
}
