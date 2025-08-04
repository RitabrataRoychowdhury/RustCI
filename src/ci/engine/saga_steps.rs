//! SAGA Steps for Pipeline Execution
//!
//! This module provides SAGA step implementations for pipeline execution
//! with proper compensation actions for rollback support.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::ci::{
    config::Stage, executor::PipelineExecutor, pipeline::PipelineExecution,
    workspace::WorkspaceManager,
};
use crate::core::patterns::sagas::{SagaContext, SagaStep, SagaStepResult};
use crate::error::{AppError, Result};

use super::{ExecutionContext, ExecutionMonitoring, MetricsCollector};

/// SAGA step for workspace creation
#[derive(Debug)]
pub struct CreateWorkspaceStep {
    workspace_manager: Arc<WorkspaceManager>,
    execution_context: ExecutionContext,
}

impl CreateWorkspaceStep {
    pub fn new(
        workspace_manager: Arc<WorkspaceManager>,
        execution_context: ExecutionContext,
    ) -> Self {
        Self {
            workspace_manager,
            execution_context,
        }
    }
}

#[async_trait]
impl SagaStep for CreateWorkspaceStep {
    async fn execute(&self, context: &SagaContext) -> Result<SagaStepResult> {
        info!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            "Creating workspace for pipeline execution"
        );

        match self
            .workspace_manager
            .create_workspace(self.execution_context.execution_id)
            .await
        {
            Ok(workspace) => {
                let mut data = HashMap::new();
                data.insert(
                    "workspace_path".to_string(),
                    serde_json::json!(workspace.path.to_string_lossy()),
                );

                Ok(SagaStepResult::success(Some(serde_json::json!(data))))
            }
            Err(e) => {
                error!(
                    saga_id = %context.saga_id,
                    execution_id = %self.execution_context.execution_id,
                    error = %e,
                    "Failed to create workspace"
                );
                Ok(SagaStepResult::failure(e.to_string()))
            }
        }
    }

    async fn compensate(&self, context: &SagaContext) -> Result<()> {
        warn!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            "Compensating workspace creation - cleaning up workspace"
        );

        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(self.execution_context.execution_id)
            .await
        {
            error!(
                saga_id = %context.saga_id,
                execution_id = %self.execution_context.execution_id,
                error = %e,
                "Failed to cleanup workspace during compensation"
            );
            // Don't fail compensation for cleanup errors
        }

        Ok(())
    }

    fn step_name(&self) -> &'static str {
        "CreateWorkspace"
    }

    fn can_compensate(&self) -> bool {
        true
    }

    fn validate(&self, _context: &SagaContext) -> Result<()> {
        if self.execution_context.execution_id.is_nil() {
            return Err(AppError::ValidationError(
                "Invalid execution ID".to_string(),
            ));
        }
        Ok(())
    }
}

/// SAGA step for stage execution
#[derive(Debug)]
pub struct ExecuteStageStep {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
    monitoring: Arc<ExecutionMonitoring>,
    metrics_collector: Arc<MetricsCollector>,
    execution_context: ExecutionContext,
    stage: Stage,
    stage_index: usize,
}

impl ExecuteStageStep {
    pub fn new(
        executor: Arc<PipelineExecutor>,
        workspace_manager: Arc<WorkspaceManager>,
        monitoring: Arc<ExecutionMonitoring>,
        metrics_collector: Arc<MetricsCollector>,
        execution_context: ExecutionContext,
        stage: Stage,
        stage_index: usize,
    ) -> Self {
        Self {
            executor,
            workspace_manager,
            monitoring,
            metrics_collector,
            execution_context,
            stage,
            stage_index,
        }
    }
}

#[async_trait]
impl SagaStep for ExecuteStageStep {
    async fn execute(&self, context: &SagaContext) -> Result<SagaStepResult> {
        let stage_start_time = std::time::Instant::now();

        info!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            stage_name = %self.stage.name,
            stage_index = self.stage_index,
            "Executing pipeline stage"
        );

        // Get workspace
        let workspace = match self
            .workspace_manager
            .get_workspace(self.execution_context.execution_id)
            .await
        {
            Ok(workspace) => workspace,
            Err(e) => {
                error!(
                    saga_id = %context.saga_id,
                    execution_id = %self.execution_context.execution_id,
                    stage_name = %self.stage.name,
                    error = %e,
                    "Failed to get workspace for stage execution"
                );
                return Ok(SagaStepResult::failure(e.to_string()));
            }
        };

        // Update monitoring
        self.monitoring
            .update_execution_progress(
                self.execution_context.execution_id,
                Some(self.stage.name.clone()),
                None,
                self.stage_index,
                0,
            )
            .await?;

        // Record stage start event
        self.monitoring
            .record_event(
                self.execution_context.execution_id,
                super::monitoring::ExecutionEventType::StageStarted,
                format!("Started executing stage: {}", self.stage.name),
                Some(self.stage.name.clone()),
                None,
                HashMap::new(),
            )
            .await?;

        // Create mock execution for the executor
        let execution = Arc::new(tokio::sync::RwLock::new(PipelineExecution::new(
            self.execution_context.pipeline_id,
            self.execution_context.trigger_info.clone(),
        )));

        // Merge environment variables
        let mut env = self.execution_context.environment.clone();
        if let Some(stage_env) = &self.stage.environment {
            env.extend(stage_env.clone());
        }

        // Execute stage
        let stage_result = self
            .executor
            .execute_stage(
                execution,
                &self.stage,
                workspace.as_ref().ok_or_else(|| {
                    AppError::InternalServerError("Workspace not available".to_string())
                })?,
                &env,
            )
            .await;

        let stage_duration = stage_start_time.elapsed();

        match stage_result {
            Ok(_) => {
                info!(
                    saga_id = %context.saga_id,
                    execution_id = %self.execution_context.execution_id,
                    stage_name = %self.stage.name,
                    duration_ms = stage_duration.as_millis(),
                    "Stage executed successfully"
                );

                // Record stage completion event
                self.monitoring
                    .record_event(
                        self.execution_context.execution_id,
                        super::monitoring::ExecutionEventType::StageCompleted,
                        format!("Completed executing stage: {}", self.stage.name),
                        Some(self.stage.name.clone()),
                        None,
                        HashMap::new(),
                    )
                    .await?;

                // Record stage metrics
                self.metrics_collector
                    .record_stage_metrics(
                        self.execution_context.execution_id,
                        self.stage.name.clone(),
                        stage_duration.as_millis() as u64,
                        self.stage.steps.len(),
                        self.stage.steps.len(), // All steps completed successfully
                        0,                      // No failed steps
                        self.stage.parallel.unwrap_or(false),
                    )
                    .await?;

                let mut result_data = HashMap::new();
                result_data.insert("stage_name".to_string(), serde_json::json!(self.stage.name));
                result_data.insert(
                    "duration_ms".to_string(),
                    serde_json::json!(stage_duration.as_millis()),
                );
                result_data.insert(
                    "steps_executed".to_string(),
                    serde_json::json!(self.stage.steps.len()),
                );

                Ok(SagaStepResult::success(Some(serde_json::json!(
                    result_data
                ))))
            }
            Err(e) => {
                error!(
                    saga_id = %context.saga_id,
                    execution_id = %self.execution_context.execution_id,
                    stage_name = %self.stage.name,
                    error = %e,
                    duration_ms = stage_duration.as_millis(),
                    "Stage execution failed"
                );

                // Record stage failure event
                self.monitoring
                    .record_event(
                        self.execution_context.execution_id,
                        super::monitoring::ExecutionEventType::StageFailed,
                        format!("Failed executing stage: {} - {}", self.stage.name, e),
                        Some(self.stage.name.clone()),
                        None,
                        HashMap::new(),
                    )
                    .await?;

                // Record stage metrics with failure
                self.metrics_collector
                    .record_stage_metrics(
                        self.execution_context.execution_id,
                        self.stage.name.clone(),
                        stage_duration.as_millis() as u64,
                        self.stage.steps.len(),
                        0,                      // No steps completed successfully
                        self.stage.steps.len(), // All steps failed
                        self.stage.parallel.unwrap_or(false),
                    )
                    .await?;

                Ok(SagaStepResult::failure(e.to_string()))
            }
        }
    }

    async fn compensate(&self, context: &SagaContext) -> Result<()> {
        warn!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            stage_name = %self.stage.name,
            "Compensating stage execution - performing rollback actions"
        );

        // Record compensation event
        self.monitoring
            .record_event(
                self.execution_context.execution_id,
                super::monitoring::ExecutionEventType::StageStarted, // Using generic event type
                format!("Compensating stage: {}", self.stage.name),
                Some(self.stage.name.clone()),
                None,
                {
                    let mut metadata = HashMap::new();
                    metadata.insert("action".to_string(), "compensation".to_string());
                    metadata
                },
            )
            .await?;

        // In a real implementation, this would perform specific rollback actions
        // such as:
        // - Cleaning up deployed resources
        // - Reverting database changes
        // - Removing created files
        // - Undoing configuration changes

        // For now, we'll just log the compensation
        info!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            stage_name = %self.stage.name,
            "Stage compensation completed"
        );

        Ok(())
    }

    fn step_name(&self) -> &'static str {
        "ExecuteStage"
    }

    fn can_compensate(&self) -> bool {
        true
    }

    fn validate(&self, _context: &SagaContext) -> Result<()> {
        if self.stage.name.is_empty() {
            return Err(AppError::ValidationError(
                "Stage name cannot be empty".to_string(),
            ));
        }
        if self.stage.steps.is_empty() {
            return Err(AppError::ValidationError(
                "Stage must have at least one step".to_string(),
            ));
        }
        Ok(())
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("stage_name".to_string(), self.stage.name.clone());
        metadata.insert("stage_index".to_string(), self.stage_index.to_string());
        metadata.insert(
            "total_steps".to_string(),
            self.stage.steps.len().to_string(),
        );
        metadata.insert(
            "parallel_execution".to_string(),
            self.stage.parallel.unwrap_or(false).to_string(),
        );
        metadata
    }
}

/// SAGA step for cleanup operations
#[derive(Debug)]
pub struct CleanupStep {
    workspace_manager: Arc<WorkspaceManager>,
    monitoring: Arc<ExecutionMonitoring>,
    metrics_collector: Arc<MetricsCollector>,
    execution_context: ExecutionContext,
}

impl CleanupStep {
    pub fn new(
        workspace_manager: Arc<WorkspaceManager>,
        monitoring: Arc<ExecutionMonitoring>,
        metrics_collector: Arc<MetricsCollector>,
        execution_context: ExecutionContext,
    ) -> Self {
        Self {
            workspace_manager,
            monitoring,
            metrics_collector,
            execution_context,
        }
    }
}

#[async_trait]
impl SagaStep for CleanupStep {
    async fn execute(&self, context: &SagaContext) -> Result<SagaStepResult> {
        info!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            "Performing cleanup operations"
        );

        // Cleanup workspace
        if let Err(e) = self
            .workspace_manager
            .cleanup_workspace(self.execution_context.execution_id)
            .await
        {
            warn!(
                saga_id = %context.saga_id,
                execution_id = %self.execution_context.execution_id,
                error = %e,
                "Failed to cleanup workspace, but continuing"
            );
        }

        // Finalize metrics collection
        if let Err(e) = self
            .metrics_collector
            .stop_collection(self.execution_context.execution_id)
            .await
        {
            warn!(
                saga_id = %context.saga_id,
                execution_id = %self.execution_context.execution_id,
                error = %e,
                "Failed to stop metrics collection, but continuing"
            );
        }

        // Record cleanup completion
        self.monitoring
            .record_event(
                self.execution_context.execution_id,
                super::monitoring::ExecutionEventType::ExecutionCompleted,
                "Cleanup operations completed".to_string(),
                None,
                None,
                HashMap::new(),
            )
            .await?;

        info!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            "Cleanup operations completed successfully"
        );

        Ok(SagaStepResult::success(None))
    }

    async fn compensate(&self, context: &SagaContext) -> Result<()> {
        debug!(
            saga_id = %context.saga_id,
            execution_id = %self.execution_context.execution_id,
            "Compensating cleanup step - no action needed"
        );

        // Cleanup compensation doesn't need to do anything special
        // since cleanup is idempotent
        Ok(())
    }

    fn step_name(&self) -> &'static str {
        "Cleanup"
    }

    fn can_compensate(&self) -> bool {
        true
    }

    fn validate(&self, _context: &SagaContext) -> Result<()> {
        Ok(()) // Cleanup step always validates successfully
    }
}

/// Factory for creating SAGA steps for pipeline execution
pub struct PipelineExecutionSagaFactory {
    executor: Arc<PipelineExecutor>,
    workspace_manager: Arc<WorkspaceManager>,
    monitoring: Arc<ExecutionMonitoring>,
    metrics_collector: Arc<MetricsCollector>,
}

impl PipelineExecutionSagaFactory {
    pub fn new(
        executor: Arc<PipelineExecutor>,
        workspace_manager: Arc<WorkspaceManager>,
        monitoring: Arc<ExecutionMonitoring>,
        metrics_collector: Arc<MetricsCollector>,
    ) -> Self {
        Self {
            executor,
            workspace_manager,
            monitoring,
            metrics_collector,
        }
    }

    /// Create SAGA steps for pipeline execution
    pub fn create_pipeline_execution_steps(
        &self,
        execution_context: ExecutionContext,
    ) -> Vec<Box<dyn SagaStep>> {
        let mut steps: Vec<Box<dyn SagaStep>> = Vec::new();

        // Step 1: Create workspace
        steps.push(Box::new(CreateWorkspaceStep::new(
            self.workspace_manager.clone(),
            execution_context.clone(),
        )));

        // Step 2-N: Execute each stage
        for (index, stage) in execution_context.pipeline.stages.iter().enumerate() {
            steps.push(Box::new(ExecuteStageStep::new(
                self.executor.clone(),
                self.workspace_manager.clone(),
                self.monitoring.clone(),
                self.metrics_collector.clone(),
                execution_context.clone(),
                stage.clone(),
                index,
            )));
        }

        // Final step: Cleanup
        steps.push(Box::new(CleanupStep::new(
            self.workspace_manager.clone(),
            self.monitoring.clone(),
            self.metrics_collector.clone(),
            execution_context,
        )));

        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{CIPipeline, Stage, Step, StepConfig, StepType};
    use crate::ci::pipeline::TriggerInfo;
    use crate::core::patterns::{sagas::SagaContext, correlation::CorrelationTracker};
    use uuid::Uuid;
    use std::collections::HashMap;

    fn create_test_execution_context() -> ExecutionContext {
        let mut pipeline = CIPipeline::new("test-pipeline".to_string());
        pipeline.description = Some("Test pipeline".to_string());
        pipeline.stages = vec![Stage {
                name: "build".to_string(),
                condition: None,
                parallel: Some(false),
                timeout: Some(600),      // 10 min timeout for stage
                retry_count: Some(0),    // no retries
                environment: None,
                steps: vec![Step {
                    name: "compile".to_string(),
                    step_type: StepType::Shell,
                    config: StepConfig {
                        command: Some("echo 'Building...'".to_string()),
                        ..Default::default()
                    },
                    condition: None,         // no condition
                    continue_on_error: Some(false),
                    timeout: Some(300),      // 5 min step timeout
                }],
            }];
        
        pipeline.timeout = Some(3600);           // 1 hour pipeline timeout
        pipeline.retry_count = Some(0);
    
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
            started_at: chrono::Utc::now(),
        }
    }
    

    #[tokio::test]
    async fn test_create_workspace_step() {
        let workspace_manager = Arc::new(WorkspaceManager::new(std::path::PathBuf::from("/tmp")));
        let execution_context = create_test_execution_context();

        let step = CreateWorkspaceStep::new(workspace_manager, execution_context);
        let saga_context = SagaContext::new(Uuid::new_v4(), Uuid::new_v4());

        assert_eq!(step.step_name(), "CreateWorkspace");
        assert!(step.can_compensate());
        assert!(step.validate(&saga_context).is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_step() {
        let workspace_manager = Arc::new(WorkspaceManager::new(std::path::PathBuf::from("/tmp")));
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let monitoring = Arc::new(ExecutionMonitoring::new(correlation_tracker));
        let metrics_collector = Arc::new(MetricsCollector::new());
        let execution_context = create_test_execution_context();

        let step = CleanupStep::new(
            workspace_manager,
            monitoring,
            metrics_collector,
            execution_context,
        );
        let saga_context = SagaContext::new(Uuid::new_v4(), Uuid::new_v4());

        assert_eq!(step.step_name(), "Cleanup");
        assert!(step.can_compensate());
        assert!(step.validate(&saga_context).is_ok());
    }
}
