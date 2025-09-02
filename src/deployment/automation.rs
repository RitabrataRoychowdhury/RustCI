use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::deployment::blue_green_manager::{
    BlueGreenDeploymentManager, DeploymentConfig, DeploymentEnvironment, 
    DeploymentStatus, RollbackConfig, ValidationRule
};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedDeploymentConfig {
    pub application_name: String,
    pub version: String,
    pub image_tag: String,
    pub target_environment: Option<DeploymentEnvironment>,
    pub auto_promote: bool,
    pub rollback_on_failure: bool,
    pub health_check_timeout: Duration,
    pub validation_timeout: Duration,
    pub traffic_switch_delay: Duration,
    pub custom_validation_rules: Vec<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentPipeline {
    pub pipeline_id: Uuid,
    pub name: String,
    pub stages: Vec<DeploymentStage>,
    pub rollback_strategy: RollbackStrategy,
    pub notification_config: NotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentStage {
    pub name: String,
    pub stage_type: StageType,
    pub timeout: Duration,
    pub retry_count: u32,
    pub continue_on_failure: bool,
    pub validation_rules: Vec<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageType {
    PreDeployment,
    Deploy,
    HealthCheck,
    Validation,
    TrafficSwitch,
    PostDeployment,
    Cleanup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackStrategy {
    Automatic,
    Manual,
    Conditional { conditions: Vec<RollbackCondition> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackCondition {
    pub metric: String,
    pub threshold: f64,
    pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub webhook_urls: Vec<String>,
    pub email_addresses: Vec<String>,
    pub slack_channels: Vec<String>,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notify_on_rollback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentExecution {
    pub execution_id: Uuid,
    pub pipeline_id: Uuid,
    pub deployment_id: Uuid,
    pub status: ExecutionStatus,
    pub current_stage: Option<String>,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub stage_results: HashMap<String, StageResult>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    RolledBack,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage_name: String,
    pub status: StageStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub duration: Option<Duration>,
    pub error_message: Option<String>,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    Retrying,
}

#[async_trait::async_trait]
pub trait DeploymentAutomation: Send + Sync {
    async fn create_pipeline(&self, pipeline: DeploymentPipeline) -> Result<Uuid>;
    async fn execute_deployment(&self, config: AutomatedDeploymentConfig, pipeline_id: Uuid) -> Result<Uuid>;
    async fn get_execution_status(&self, execution_id: Uuid) -> Result<DeploymentExecution>;
    async fn cancel_deployment(&self, execution_id: Uuid) -> Result<()>;
    async fn trigger_rollback(&self, execution_id: Uuid, reason: String) -> Result<Uuid>;
    async fn list_executions(&self, pipeline_id: Option<Uuid>) -> Result<Vec<DeploymentExecution>>;
}

pub struct ProductionDeploymentAutomation {
    pipelines: Arc<RwLock<HashMap<Uuid, DeploymentPipeline>>>,
    executions: Arc<RwLock<HashMap<Uuid, DeploymentExecution>>>,
    deployment_manager: Arc<dyn BlueGreenDeploymentManager>,
    notification_service: Arc<dyn NotificationService>,
}

impl ProductionDeploymentAutomation {
    pub fn new(
        deployment_manager: Arc<dyn BlueGreenDeploymentManager>,
        notification_service: Arc<dyn NotificationService>,
    ) -> Self {
        Self {
            pipelines: Arc::new(RwLock::new(HashMap::new())),
            executions: Arc::new(RwLock::new(HashMap::new())),
            deployment_manager,
            notification_service,
        }
    }

    async fn execute_stage(
        &self,
        execution_id: Uuid,
        deployment_id: Uuid,
        stage: &DeploymentStage,
    ) -> Result<StageResult> {
        let started_at = SystemTime::now();
        let mut retry_count = 0;

        loop {
            let stage_result = match stage.stage_type {
                StageType::PreDeployment => self.execute_pre_deployment_stage(stage).await,
                StageType::Deploy => self.execute_deploy_stage(deployment_id, stage).await,
                StageType::HealthCheck => self.execute_health_check_stage(deployment_id, stage).await,
                StageType::Validation => self.execute_validation_stage(deployment_id, stage).await,
                StageType::TrafficSwitch => self.execute_traffic_switch_stage(deployment_id, stage).await,
                StageType::PostDeployment => self.execute_post_deployment_stage(stage).await,
                StageType::Cleanup => self.execute_cleanup_stage(deployment_id, stage).await,
            };

            match stage_result {
                Ok(_) => {
                    return Ok(StageResult {
                        stage_name: stage.name.clone(),
                        status: StageStatus::Completed,
                        started_at,
                        completed_at: Some(SystemTime::now()),
                        duration: started_at.elapsed().ok(),
                        error_message: None,
                        retry_count,
                    });
                }
                Err(e) => {
                    retry_count += 1;
                    
                    if retry_count > stage.retry_count {
                        return Ok(StageResult {
                            stage_name: stage.name.clone(),
                            status: if stage.continue_on_failure { StageStatus::Skipped } else { StageStatus::Failed },
                            started_at,
                            completed_at: Some(SystemTime::now()),
                            duration: started_at.elapsed().ok(),
                            error_message: Some(e.to_string()),
                            retry_count,
                        });
                    }

                    // Wait before retry
                    tokio::time::sleep(Duration::from_secs(2_u64.pow(retry_count - 1))).await;
                }
            }
        }
    }

    async fn execute_pre_deployment_stage(&self, _stage: &DeploymentStage) -> Result<()> {
        // Pre-deployment checks (e.g., resource availability, dependencies)
        log::info!("Executing pre-deployment stage");
        Ok(())
    }

    async fn execute_deploy_stage(&self, deployment_id: Uuid, _stage: &DeploymentStage) -> Result<()> {
        // The actual deployment is handled by the BlueGreenDeploymentManager
        let status = self.deployment_manager.get_deployment_status(deployment_id).await?;
        
        if matches!(status.status, DeploymentStatus::Failed) {
            return Err(AppError::DeploymentFailed("Deployment failed".to_string()));
        }
        
        Ok(())
    }

    async fn execute_health_check_stage(&self, deployment_id: Uuid, _stage: &DeploymentStage) -> Result<()> {
        let is_valid = self.deployment_manager.validate_deployment(deployment_id).await?;
        
        if !is_valid {
            return Err(AppError::ValidationFailed("Health checks failed".to_string()));
        }
        
        Ok(())
    }

    async fn execute_validation_stage(&self, deployment_id: Uuid, _stage: &DeploymentStage) -> Result<()> {
        let is_valid = self.deployment_manager.validate_deployment(deployment_id).await?;
        
        if !is_valid {
            return Err(AppError::ValidationFailed("Validation failed".to_string()));
        }
        
        Ok(())
    }

    async fn execute_traffic_switch_stage(&self, deployment_id: Uuid, _stage: &DeploymentStage) -> Result<()> {
        self.deployment_manager.switch_traffic(deployment_id).await
    }

    async fn execute_post_deployment_stage(&self, _stage: &DeploymentStage) -> Result<()> {
        // Post-deployment tasks (e.g., cache warming, monitoring setup)
        log::info!("Executing post-deployment stage");
        Ok(())
    }

    async fn execute_cleanup_stage(&self, deployment_id: Uuid, _stage: &DeploymentStage) -> Result<()> {
        // Cleanup old deployments
        self.deployment_manager.cleanup_old_deployment(deployment_id).await
    }

    async fn should_rollback(&self, execution: &DeploymentExecution, pipeline: &DeploymentPipeline) -> bool {
        match &pipeline.rollback_strategy {
            RollbackStrategy::Automatic => true,
            RollbackStrategy::Manual => false,
            RollbackStrategy::Conditional { conditions } => {
                // Check rollback conditions
                for condition in conditions {
                    // This would integrate with monitoring systems to check metrics
                    // For now, we'll use a simple heuristic
                    if execution.stage_results.values().any(|r| matches!(r.status, StageStatus::Failed)) {
                        return true;
                    }
                }
                false
            }
        }
    }

    async fn update_execution_status(&self, execution_id: Uuid, status: ExecutionStatus) -> Result<()> {
        let mut executions = self.executions.write().await;
        if let Some(execution) = executions.get_mut(&execution_id) {
            execution.status = status;
            if matches!(status, ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::RolledBack) {
                execution.completed_at = Some(SystemTime::now());
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl DeploymentAutomation for ProductionDeploymentAutomation {
    async fn create_pipeline(&self, pipeline: DeploymentPipeline) -> Result<Uuid> {
        let pipeline_id = pipeline.pipeline_id;
        
        {
            let mut pipelines = self.pipelines.write().await;
            pipelines.insert(pipeline_id, pipeline);
        }

        log::info!("Created deployment pipeline {}", pipeline_id);
        Ok(pipeline_id)
    }

    async fn execute_deployment(&self, config: AutomatedDeploymentConfig, pipeline_id: Uuid) -> Result<Uuid> {
        let execution_id = Uuid::new_v4();
        
        // Get pipeline
        let pipeline = {
            let pipelines = self.pipelines.read().await;
            pipelines.get(&pipeline_id)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("Pipeline {} not found", pipeline_id)))?
        };

        // Determine target environment
        let target_env = config.target_environment.unwrap_or_else(|| {
            // Auto-select environment based on current active environment
            DeploymentEnvironment::Green // Simplified logic
        });

        // Create deployment config
        let deployment_config = DeploymentConfig {
            deployment_id: Uuid::new_v4(),
            version: config.version.clone(),
            environment: target_env,
            health_check_timeout: config.health_check_timeout,
            rollback_timeout: Duration::from_secs(300),
            traffic_switch_delay: config.traffic_switch_delay,
            health_check_endpoints: vec![
                format!("http://{}:8080/health", config.application_name),
                format!("http://{}:8080/ready", config.application_name),
            ],
            validation_rules: config.custom_validation_rules,
        };

        // Start deployment
        let deployment_id = self.deployment_manager.start_deployment(deployment_config).await?;

        // Create execution record
        let execution = DeploymentExecution {
            execution_id,
            pipeline_id,
            deployment_id,
            status: ExecutionStatus::Pending,
            current_stage: None,
            started_at: SystemTime::now(),
            completed_at: None,
            stage_results: HashMap::new(),
            error_message: None,
        };

        {
            let mut executions = self.executions.write().await;
            executions.insert(execution_id, execution);
        }

        // Execute pipeline stages in background
        let automation = Arc::new(self.clone());
        let pipeline_clone = pipeline.clone();
        tokio::spawn(async move {
            if let Err(e) = automation.execute_pipeline_stages(execution_id, deployment_id, pipeline_clone).await {
                let _ = automation.update_execution_status(execution_id, ExecutionStatus::Failed).await;
                log::error!("Pipeline execution {} failed: {}", execution_id, e);
            }
        });

        Ok(execution_id)
    }

    async fn get_execution_status(&self, execution_id: Uuid) -> Result<DeploymentExecution> {
        let executions = self.executions.read().await;
        executions.get(&execution_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Execution {} not found", execution_id)))
    }

    async fn cancel_deployment(&self, execution_id: Uuid) -> Result<()> {
        self.update_execution_status(execution_id, ExecutionStatus::Cancelled).await?;
        log::info!("Cancelled deployment execution {}", execution_id);
        Ok(())
    }

    async fn trigger_rollback(&self, execution_id: Uuid, reason: String) -> Result<Uuid> {
        let execution = self.get_execution_status(execution_id).await?;
        
        let rollback_config = RollbackConfig {
            deployment_id: execution.deployment_id,
            target_version: "previous".to_string(), // This would be determined from deployment history
            reason,
            automatic: false,
        };

        let rollback_id = self.deployment_manager.rollback_deployment(rollback_config).await?;
        self.update_execution_status(execution_id, ExecutionStatus::RolledBack).await?;

        log::info!("Triggered rollback {} for execution {}", rollback_id, execution_id);
        Ok(rollback_id)
    }

    async fn list_executions(&self, pipeline_id: Option<Uuid>) -> Result<Vec<DeploymentExecution>> {
        let executions = self.executions.read().await;
        
        let filtered_executions: Vec<DeploymentExecution> = executions.values()
            .filter(|execution| {
                pipeline_id.map_or(true, |pid| execution.pipeline_id == pid)
            })
            .cloned()
            .collect();

        Ok(filtered_executions)
    }
}

impl ProductionDeploymentAutomation {
    async fn execute_pipeline_stages(
        &self,
        execution_id: Uuid,
        deployment_id: Uuid,
        pipeline: DeploymentPipeline,
    ) -> Result<()> {
        self.update_execution_status(execution_id, ExecutionStatus::Running).await?;

        let mut stage_results = HashMap::new();
        let mut failed_stage = None;

        for stage in &pipeline.stages {
            // Update current stage
            {
                let mut executions = self.executions.write().await;
                if let Some(execution) = executions.get_mut(&execution_id) {
                    execution.current_stage = Some(stage.name.clone());
                }
            }

            let stage_result = self.execute_stage(execution_id, deployment_id, stage).await?;
            
            if matches!(stage_result.status, StageStatus::Failed) && !stage.continue_on_failure {
                failed_stage = Some(stage.name.clone());
                stage_results.insert(stage.name.clone(), stage_result);
                break;
            }

            stage_results.insert(stage.name.clone(), stage_result);
        }

        // Update execution with stage results
        {
            let mut executions = self.executions.write().await;
            if let Some(execution) = executions.get_mut(&execution_id) {
                execution.stage_results = stage_results;
                execution.current_stage = None;
            }
        }

        if let Some(failed_stage_name) = failed_stage {
            let execution = self.get_execution_status(execution_id).await?;
            
            if self.should_rollback(&execution, &pipeline).await {
                let rollback_config = RollbackConfig {
                    deployment_id,
                    target_version: "previous".to_string(),
                    reason: format!("Stage '{}' failed", failed_stage_name),
                    automatic: true,
                };
                
                let _ = self.deployment_manager.rollback_deployment(rollback_config).await;
                self.update_execution_status(execution_id, ExecutionStatus::RolledBack).await?;
            } else {
                self.update_execution_status(execution_id, ExecutionStatus::Failed).await?;
            }
        } else {
            self.update_execution_status(execution_id, ExecutionStatus::Completed).await?;
        }

        Ok(())
    }
}

impl Clone for ProductionDeploymentAutomation {
    fn clone(&self) -> Self {
        Self {
            pipelines: Arc::clone(&self.pipelines),
            executions: Arc::clone(&self.executions),
            deployment_manager: Arc::clone(&self.deployment_manager),
            notification_service: Arc::clone(&self.notification_service),
        }
    }
}

#[async_trait::async_trait]
pub trait NotificationService: Send + Sync {
    async fn send_notification(&self, message: &str, config: &NotificationConfig) -> Result<()>;
}

pub struct MockNotificationService;

#[async_trait::async_trait]
impl NotificationService for MockNotificationService {
    async fn send_notification(&self, message: &str, _config: &NotificationConfig) -> Result<()> {
        log::info!("Notification: {}", message);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::deployment::blue_green_manager::ProductionBlueGreenManager;
    use crate::deployment::health_checker::ProductionHealthChecker;
    use crate::deployment::traffic_router::{ProductionTrafficRouter, MockLoadBalancerAdapter, TrafficRoutingConfig};

    #[tokio::test]
    async fn test_deployment_automation() {
        let health_checker = Arc::new(ProductionHealthChecker::new());
        let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
        let traffic_config = TrafficRoutingConfig {
            blue_upstream: "blue:8080".to_string(),
            green_upstream: "green:8080".to_string(),
            health_check_interval: Duration::from_secs(30),
            failover_threshold: 3,
        };
        let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
        let deployment_manager = Arc::new(ProductionBlueGreenManager::new(health_checker, traffic_router));
        let notification_service = Arc::new(MockNotificationService);
        
        let automation = ProductionDeploymentAutomation::new(deployment_manager, notification_service);

        // Create a simple pipeline
        let pipeline = DeploymentPipeline {
            pipeline_id: Uuid::new_v4(),
            name: "test-pipeline".to_string(),
            stages: vec![
                DeploymentStage {
                    name: "deploy".to_string(),
                    stage_type: StageType::Deploy,
                    timeout: Duration::from_secs(300),
                    retry_count: 2,
                    continue_on_failure: false,
                    validation_rules: vec![],
                },
            ],
            rollback_strategy: RollbackStrategy::Manual,
            notification_config: NotificationConfig {
                webhook_urls: vec![],
                email_addresses: vec![],
                slack_channels: vec![],
                notify_on_success: true,
                notify_on_failure: true,
                notify_on_rollback: true,
            },
        };

        let pipeline_id = automation.create_pipeline(pipeline).await.unwrap();

        let config = AutomatedDeploymentConfig {
            application_name: "test-app".to_string(),
            version: "1.0.0".to_string(),
            image_tag: "test-app:1.0.0".to_string(),
            target_environment: Some(DeploymentEnvironment::Green),
            auto_promote: true,
            rollback_on_failure: true,
            health_check_timeout: Duration::from_secs(60),
            validation_timeout: Duration::from_secs(120),
            traffic_switch_delay: Duration::from_secs(30),
            custom_validation_rules: vec![],
        };

        let execution_id = automation.execute_deployment(config, pipeline_id).await.unwrap();
        
        // Wait a bit for background processing
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let execution = automation.get_execution_status(execution_id).await.unwrap();
        assert_eq!(execution.pipeline_id, pipeline_id);
    }
}