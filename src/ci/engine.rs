use crate::ci::{
    config::CIPipeline,
    pipeline::{PipelineExecution, ExecutionStatus, TriggerInfo, LogLevel},
    executor::PipelineExecutor,
    workspace::WorkspaceManager,
};
use crate::error::{AppError, Result};
use crate::database::DatabaseManager;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, error, debug, warn};
use uuid::Uuid;
use std::collections::HashMap;

#[allow(dead_code)] // Will be used when CI engine is fully implemented
#[derive(Clone)]
pub struct CIEngine {
    #[allow(dead_code)] // Will be used for database operations
    pub db: Arc<DatabaseManager>,
    pub workspace_manager: Arc<WorkspaceManager>,
    pub executor: Arc<PipelineExecutor>,
    pub running_executions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<PipelineExecution>>>>>,
    pub event_sender: mpsc::UnboundedSender<CIEvent>,
}

#[allow(dead_code)] // Will be used for event handling
#[derive(Debug, Clone)]
pub enum CIEvent {
    #[allow(dead_code)] // Will be used for pipeline start events
    PipelineStarted { execution_id: Uuid, pipeline_id: Uuid },
    #[allow(dead_code)] // Will be used for pipeline finish events
    PipelineFinished { execution_id: Uuid, status: ExecutionStatus },
    #[allow(dead_code)] // Will be used for stage start events
    StageStarted { execution_id: Uuid, stage_name: String },
    #[allow(dead_code)] // Will be used for stage finish events
    StageFinished { execution_id: Uuid, stage_name: String, status: ExecutionStatus },
    #[allow(dead_code)] // Will be used for step start events
    StepStarted { execution_id: Uuid, stage_name: String, step_name: String },
    #[allow(dead_code)] // Will be used for step finish events
    StepFinished { execution_id: Uuid, stage_name: String, step_name: String, status: ExecutionStatus },
    #[allow(dead_code)] // Will be used for log events
    LogMessage { execution_id: Uuid, level: LogLevel, message: String },
}

impl CIEngine {
    #[allow(dead_code)] // Will be used when CI engine is initialized in AppState
    pub fn new(
        db: Arc<DatabaseManager>,
        workspace_manager: Arc<WorkspaceManager>,
        executor: Arc<PipelineExecutor>,
    ) -> Self {
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let running_executions = Arc::new(RwLock::new(HashMap::new()));

        // Spawn event handler
        let running_executions_clone = running_executions.clone();
        tokio::spawn(async move {
            while let Some(event) = event_receiver.recv().await {
                Self::handle_event(event, running_executions_clone.clone()).await;
            }
        });

        Self {
            db,
            workspace_manager,
            executor,
            running_executions,
            event_sender,
        }
    }

    pub async fn create_pipeline(&self, pipeline: CIPipeline) -> Result<Uuid> {
        info!("🔄 Creating new pipeline: {}", pipeline.name);
        
        // Validate pipeline configuration
        pipeline.validate().map_err(|e| AppError::ValidationError(e))?;

        // Store pipeline in database
        let pipeline_id = self.store_pipeline(&pipeline).await?;
        
        info!("✅ Pipeline created successfully: {} (ID: {})", pipeline.name, pipeline_id);
        Ok(pipeline_id)
    }

    pub async fn get_pipeline(&self, pipeline_id: Uuid) -> Result<CIPipeline> {
        self.get_pipeline_from_db(pipeline_id).await
    }

    pub async fn trigger_pipeline(
        &self,
        pipeline_id: Uuid,
        trigger_info: TriggerInfo,
        environment: Option<HashMap<String, String>>,
    ) -> Result<Uuid> {
        info!("🚀 Triggering pipeline: {}", pipeline_id);

        // Get pipeline configuration
        let pipeline = self.get_pipeline(pipeline_id).await?;
        
        // Create execution record
        let mut execution = PipelineExecution::new(pipeline_id, trigger_info);
        execution.initialize_stages(&pipeline);
        
        if let Some(env) = environment {
            execution.environment = env;
        }

        // Store execution in database
        let execution_id = self.store_execution(&execution).await?;
        execution.id = execution_id;

        // Add to running executions
        let execution_arc = Arc::new(RwLock::new(execution));
        self.running_executions.write().await.insert(execution_id, execution_arc.clone());

        // Send start event
        let _ = self.event_sender.send(CIEvent::PipelineStarted {
            execution_id,
            pipeline_id,
        });

        // Execute pipeline asynchronously
        let engine_clone = self.clone();
        let pipeline_clone = pipeline.clone();
        tokio::spawn(async move {
            if let Err(e) = engine_clone.execute_pipeline_async(execution_arc, pipeline_clone).await {
                error!("❌ Pipeline execution failed: {}", e);
            }
        });

        info!("✅ Pipeline triggered successfully: {} (Execution ID: {})", pipeline_id, execution_id);
        Ok(execution_id)
    }

    async fn execute_pipeline_async(
        &self,
        execution: Arc<RwLock<PipelineExecution>>,
        pipeline: CIPipeline,
    ) -> Result<()> {
        let execution_id = {
            let exec = execution.read().await;
            exec.id
        };

        info!("🔄 Starting pipeline execution: {}", execution_id);

        // Start execution
        {
            let mut exec = execution.write().await;
            exec.start();
        }

        // Create workspace
        let workspace = self.workspace_manager.create_workspace(execution_id).await?;
        
        let mut overall_success = true;

        // Execute stages sequentially
        for stage in &pipeline.stages {
            info!("🔄 Executing stage: {}", stage.name);
            
            // Send stage start event
            let _ = self.event_sender.send(CIEvent::StageStarted {
                execution_id,
                stage_name: stage.name.clone(),
            });

            // Update stage status
            {
                let mut exec = execution.write().await;
                if let Some(stage_exec) = exec.get_stage_mut(&stage.name) {
                    stage_exec.start();
                }
            }

            // Execute stage
            let stage_result = self.executor.execute_stage(
                execution.clone(),
                stage,
                &workspace,
                &pipeline.environment,
            ).await;

            let stage_status = match stage_result {
                Ok(_) => {
                    info!("✅ Stage completed successfully: {}", stage.name);
                    ExecutionStatus::Success
                }
                Err(e) => {
                    error!("❌ Stage failed: {} - {}", stage.name, e);
                    overall_success = false;
                    ExecutionStatus::Failed
                }
            };

            // Update stage status
            {
                let mut exec = execution.write().await;
                if let Some(stage_exec) = exec.get_stage_mut(&stage.name) {
                    stage_exec.finish(stage_status.clone());
                }
            }

            // Send stage finish event
            let _ = self.event_sender.send(CIEvent::StageFinished {
                execution_id,
                stage_name: stage.name.clone(),
                status: stage_status.clone(),
            });

            // Stop execution if stage failed and no continue_on_error
            if !overall_success {
                break;
            }
        }

        // Finish execution
        let final_status = if overall_success {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::Failed
        };

        {
            let mut exec = execution.write().await;
            exec.finish(final_status.clone());
        }

        // Update execution in database
        {
            let exec = execution.read().await;
            self.update_execution(&exec).await?;
        }

        // Clean up workspace
        if let Err(e) = self.workspace_manager.cleanup_workspace(execution_id).await {
            warn!("⚠️ Failed to cleanup workspace: {}", e);
        }

        // Remove from running executions
        self.running_executions.write().await.remove(&execution_id);

        // Send finish event
        let _ = self.event_sender.send(CIEvent::PipelineFinished {
            execution_id,
            status: final_status,
        });

        info!("✅ Pipeline execution completed: {}", execution_id);
        Ok(())
    }

    pub async fn get_execution(&self, execution_id: Uuid) -> Result<PipelineExecution> {
        // First check running executions
        if let Some(execution) = self.running_executions.read().await.get(&execution_id) {
            return Ok(execution.read().await.clone());
        }

        // Otherwise get from database
        self.get_execution_from_db(execution_id).await
    }

    pub async fn cancel_execution(&self, execution_id: Uuid) -> Result<()> {
        info!("🛑 Cancelling pipeline execution: {}", execution_id);

        if let Some(execution) = self.running_executions.read().await.get(&execution_id) {
            let mut exec = execution.write().await;
            exec.finish(ExecutionStatus::Cancelled);
            
            // Update in database
            self.update_execution(&exec).await?;
            
            info!("✅ Pipeline execution cancelled: {}", execution_id);
            Ok(())
        } else {
            Err(AppError::NotFound("Execution not found or not running".to_string()))
        }
    }

    pub async fn list_pipelines(&self) -> Result<Vec<CIPipeline>> {
        self.get_all_pipelines().await
    }

    pub async fn list_executions(&self, pipeline_id: Option<Uuid>) -> Result<Vec<PipelineExecution>> {
        self.get_executions(pipeline_id).await
    }

    #[allow(dead_code)] // Will be used for event processing
    async fn handle_event(event: CIEvent, _running_executions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<PipelineExecution>>>>>) {
        match event {
            CIEvent::PipelineStarted { execution_id, pipeline_id } => {
                info!("📡 Event: Pipeline started - Execution: {}, Pipeline: {}", execution_id, pipeline_id);
            }
            CIEvent::PipelineFinished { execution_id, status } => {
                info!("📡 Event: Pipeline finished - Execution: {}, Status: {:?}", execution_id, status);
            }
            CIEvent::StageStarted { execution_id, stage_name } => {
                debug!("📡 Event: Stage started - Execution: {}, Stage: {}", execution_id, stage_name);
            }
            CIEvent::StageFinished { execution_id, stage_name, status } => {
                debug!("📡 Event: Stage finished - Execution: {}, Stage: {}, Status: {:?}", execution_id, stage_name, status);
            }
            CIEvent::StepStarted { execution_id, stage_name, step_name } => {
                debug!("📡 Event: Step started - Execution: {}, Stage: {}, Step: {}", execution_id, stage_name, step_name);
            }
            CIEvent::StepFinished { execution_id, stage_name, step_name, status } => {
                debug!("📡 Event: Step finished - Execution: {}, Stage: {}, Step: {}, Status: {:?}", execution_id, stage_name, step_name, status);
            }
            CIEvent::LogMessage { execution_id, level, message } => {
                debug!("📡 Event: Log - Execution: {}, Level: {:?}, Message: {}", execution_id, level, message);
            }
        }
    }

    // Database operations (to be implemented)
    async fn store_pipeline(&self, pipeline: &CIPipeline) -> Result<Uuid> {
        // TODO: Implement MongoDB storage for pipelines
        Ok(pipeline.id.unwrap_or_else(|| Uuid::new_v4()))
    }

    async fn get_pipeline_from_db(&self, _pipeline_id: Uuid) -> Result<CIPipeline> {
        // TODO: Implement MongoDB retrieval for pipelines
        Err(AppError::NotFound("Pipeline not found".to_string()))
    }

    async fn store_execution(&self, execution: &PipelineExecution) -> Result<Uuid> {
        // TODO: Implement MongoDB storage for executions
        Ok(execution.id)
    }

    async fn update_execution(&self, _execution: &PipelineExecution) -> Result<()> {
        // TODO: Implement MongoDB update for executions
        Ok(())
    }

    async fn get_execution_from_db(&self, _execution_id: Uuid) -> Result<PipelineExecution> {
        // TODO: Implement MongoDB retrieval for executions
        Err(AppError::NotFound("Execution not found".to_string()))
    }

    async fn get_all_pipelines(&self) -> Result<Vec<CIPipeline>> {
        // TODO: Implement MongoDB retrieval for all pipelines
        Ok(Vec::new())
    }

    async fn get_executions(&self, _pipeline_id: Option<Uuid>) -> Result<Vec<PipelineExecution>> {
        // TODO: Implement MongoDB retrieval for executions
        Ok(Vec::new())
    }
}