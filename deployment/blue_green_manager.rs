use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeploymentEnvironment {
    Blue,
    Green,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeploymentStatus {
    Pending,
    InProgress,
    HealthChecking,
    Ready,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub deployment_id: Uuid,
    pub version: String,
    pub environment: DeploymentEnvironment,
    pub health_check_timeout: Duration,
    pub rollback_timeout: Duration,
    pub traffic_switch_delay: Duration,
    pub health_check_endpoints: Vec<String>,
    pub validation_rules: Vec<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub endpoint: String,
    pub expected_status: u16,
    pub timeout: Duration,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentState {
    pub deployment_id: Uuid,
    pub status: DeploymentStatus,
    pub environment: DeploymentEnvironment,
    pub version: String,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub health_check_results: Vec<HealthCheckResult>,
    pub validation_results: Vec<ValidationResult>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub endpoint: String,
    pub status: bool,
    pub response_time: Duration,
    pub error_message: Option<String>,
    pub checked_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub rule_name: String,
    pub passed: bool,
    pub error_message: Option<String>,
    pub validated_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    pub deployment_id: Uuid,
    pub target_version: String,
    pub reason: String,
    pub automatic: bool,
}

pub trait BlueGreenDeploymentManager: Send + Sync {
    async fn start_deployment(&self, config: DeploymentConfig) -> Result<Uuid>;
    async fn get_deployment_status(&self, deployment_id: Uuid) -> Result<DeploymentState>;
    async fn validate_deployment(&self, deployment_id: Uuid) -> Result<bool>;
    async fn switch_traffic(&self, deployment_id: Uuid) -> Result<()>;
    async fn rollback_deployment(&self, config: RollbackConfig) -> Result<Uuid>;
    async fn cleanup_old_deployment(&self, deployment_id: Uuid) -> Result<()>;
    async fn get_active_environment(&self) -> Result<DeploymentEnvironment>;
    async fn list_deployments(&self) -> Result<Vec<DeploymentState>>;
}

pub struct ProductionBlueGreenManager {
    deployments: Arc<RwLock<HashMap<Uuid, DeploymentState>>>,
    active_environment: Arc<RwLock<DeploymentEnvironment>>,
    health_checker: Arc<dyn HealthChecker>,
    traffic_router: Arc<dyn TrafficRouter>,
}

impl ProductionBlueGreenManager {
    pub fn new(
        health_checker: Arc<dyn HealthChecker>,
        traffic_router: Arc<dyn TrafficRouter>,
    ) -> Self {
        Self {
            deployments: Arc::new(RwLock::new(HashMap::new())),
            active_environment: Arc::new(RwLock::new(DeploymentEnvironment::Blue)),
            health_checker,
            traffic_router,
        }
    }

    async fn perform_health_checks(&self, config: &DeploymentConfig) -> Result<Vec<HealthCheckResult>> {
        let mut results = Vec::new();
        
        for endpoint in &config.health_check_endpoints {
            let result = self.health_checker.check_health(endpoint, config.health_check_timeout).await?;
            results.push(result);
        }
        
        Ok(results)
    }

    async fn perform_validations(&self, config: &DeploymentConfig) -> Result<Vec<ValidationResult>> {
        let mut results = Vec::new();
        
        for rule in &config.validation_rules {
            let result = self.health_checker.validate_rule(rule).await?;
            results.push(result);
        }
        
        Ok(results)
    }

    async fn update_deployment_status(&self, deployment_id: Uuid, status: DeploymentStatus) -> Result<()> {
        let mut deployments = self.deployments.write().await;
        if let Some(deployment) = deployments.get_mut(&deployment_id) {
            deployment.status = status;
            if matches!(status, DeploymentStatus::Ready | DeploymentStatus::Failed | DeploymentStatus::RolledBack) {
                deployment.completed_at = Some(SystemTime::now());
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl BlueGreenDeploymentManager for ProductionBlueGreenManager {
    async fn start_deployment(&self, config: DeploymentConfig) -> Result<Uuid> {
        let deployment_id = config.deployment_id;
        
        let deployment_state = DeploymentState {
            deployment_id,
            status: DeploymentStatus::Pending,
            environment: config.environment.clone(),
            version: config.version.clone(),
            started_at: SystemTime::now(),
            completed_at: None,
            health_check_results: Vec::new(),
            validation_results: Vec::new(),
            error_message: None,
        };

        {
            let mut deployments = self.deployments.write().await;
            deployments.insert(deployment_id, deployment_state);
        }

        // Start deployment process in background
        let manager = Arc::new(self.clone());
        let config_clone = config.clone();
        tokio::spawn(async move {
            if let Err(e) = manager.execute_deployment(config_clone).await {
                let _ = manager.update_deployment_status(deployment_id, DeploymentStatus::Failed).await;
                log::error!("Deployment {} failed: {}", deployment_id, e);
            }
        });

        Ok(deployment_id)
    }

    async fn get_deployment_status(&self, deployment_id: Uuid) -> Result<DeploymentState> {
        let deployments = self.deployments.read().await;
        deployments.get(&deployment_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Deployment {} not found", deployment_id)))
    }

    async fn validate_deployment(&self, deployment_id: Uuid) -> Result<bool> {
        let deployment = self.get_deployment_status(deployment_id).await?;
        
        // Check if all health checks passed
        let health_checks_passed = deployment.health_check_results.iter()
            .all(|result| result.status);
            
        // Check if all validations passed
        let validations_passed = deployment.validation_results.iter()
            .all(|result| result.passed);
            
        Ok(health_checks_passed && validations_passed)
    }

    async fn switch_traffic(&self, deployment_id: Uuid) -> Result<()> {
        let deployment = self.get_deployment_status(deployment_id).await?;
        
        if deployment.status != DeploymentStatus::Ready {
            return Err(AppError::InvalidState(
                format!("Deployment {} is not ready for traffic switch", deployment_id)
            ));
        }

        // Switch traffic to the new environment
        self.traffic_router.switch_traffic(&deployment.environment).await?;
        
        // Update active environment
        {
            let mut active_env = self.active_environment.write().await;
            *active_env = deployment.environment;
        }

        log::info!("Traffic switched to {:?} environment for deployment {}", 
                  deployment.environment, deployment_id);
        
        Ok(())
    }

    async fn rollback_deployment(&self, config: RollbackConfig) -> Result<Uuid> {
        let rollback_id = Uuid::new_v4();
        
        log::warn!("Starting rollback for deployment {} to version {}: {}", 
                  config.deployment_id, config.target_version, config.reason);

        // Get current active environment and switch to the other one
        let current_env = {
            let active_env = self.active_environment.read().await;
            active_env.clone()
        };

        let rollback_env = match current_env {
            DeploymentEnvironment::Blue => DeploymentEnvironment::Green,
            DeploymentEnvironment::Green => DeploymentEnvironment::Blue,
        };

        // Switch traffic back
        self.traffic_router.switch_traffic(&rollback_env).await?;
        
        // Update active environment
        {
            let mut active_env = self.active_environment.write().await;
            *active_env = rollback_env;
        }

        // Mark original deployment as rolled back
        self.update_deployment_status(config.deployment_id, DeploymentStatus::RolledBack).await?;

        log::info!("Rollback completed. Traffic switched to {:?} environment", rollback_env);
        
        Ok(rollback_id)
    }

    async fn cleanup_old_deployment(&self, deployment_id: Uuid) -> Result<()> {
        // Remove deployment from tracking
        {
            let mut deployments = self.deployments.write().await;
            deployments.remove(&deployment_id);
        }

        log::info!("Cleaned up deployment {}", deployment_id);
        Ok(())
    }

    async fn get_active_environment(&self) -> Result<DeploymentEnvironment> {
        let active_env = self.active_environment.read().await;
        Ok(active_env.clone())
    }

    async fn list_deployments(&self) -> Result<Vec<DeploymentState>> {
        let deployments = self.deployments.read().await;
        Ok(deployments.values().cloned().collect())
    }
}

impl ProductionBlueGreenManager {
    async fn execute_deployment(&self, config: DeploymentConfig) -> Result<()> {
        let deployment_id = config.deployment_id;
        
        // Update status to in progress
        self.update_deployment_status(deployment_id, DeploymentStatus::InProgress).await?;

        // Perform health checks
        self.update_deployment_status(deployment_id, DeploymentStatus::HealthChecking).await?;
        
        let health_results = self.perform_health_checks(&config).await?;
        let validation_results = self.perform_validations(&config).await?;

        // Update deployment with results
        {
            let mut deployments = self.deployments.write().await;
            if let Some(deployment) = deployments.get_mut(&deployment_id) {
                deployment.health_check_results = health_results;
                deployment.validation_results = validation_results;
            }
        }

        // Check if deployment is valid
        if self.validate_deployment(deployment_id).await? {
            self.update_deployment_status(deployment_id, DeploymentStatus::Ready).await?;
            log::info!("Deployment {} is ready for traffic switch", deployment_id);
        } else {
            self.update_deployment_status(deployment_id, DeploymentStatus::Failed).await?;
            return Err(AppError::ValidationFailed("Deployment validation failed".to_string()));
        }

        Ok(())
    }
}

impl Clone for ProductionBlueGreenManager {
    fn clone(&self) -> Self {
        Self {
            deployments: Arc::clone(&self.deployments),
            active_environment: Arc::clone(&self.active_environment),
            health_checker: Arc::clone(&self.health_checker),
            traffic_router: Arc::clone(&self.traffic_router),
        }
    }
}

pub trait HealthChecker: Send + Sync {
    async fn check_health(&self, endpoint: &str, timeout: Duration) -> Result<HealthCheckResult>;
    async fn validate_rule(&self, rule: &ValidationRule) -> Result<ValidationResult>;
}

pub trait TrafficRouter: Send + Sync {
    async fn switch_traffic(&self, environment: &DeploymentEnvironment) -> Result<()>;
    async fn get_current_traffic_split(&self) -> Result<HashMap<DeploymentEnvironment, f64>>;
}