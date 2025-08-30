use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;
use tokio::sync::RwLock;

use deployment::{
    BlueGreenDeploymentManager, ProductionBlueGreenManager, DeploymentConfig,
    DeploymentEnvironment, DeploymentStatus, RollbackConfig, ValidationRule,
    ProductionHealthChecker, HealthChecker, HealthCheckResult, ValidationResult,
    ProductionTrafficRouter, TrafficRouter, TrafficRoutingConfig, TrafficSplit,
    MockLoadBalancerAdapter, ProductionDeploymentAutomation, DeploymentAutomation,
    AutomatedDeploymentConfig, DeploymentPipeline, DeploymentStage, StageType,
    RollbackStrategy, NotificationConfig, MockNotificationService
};
use crate::error::{AppError, Result};

struct MockHealthChecker {
    health_results: Arc<RwLock<HashMap<String, bool>>>,
    validation_results: Arc<RwLock<HashMap<String, bool>>>,
}

impl MockHealthChecker {
    fn new() -> Self {
        Self {
            health_results: Arc::new(RwLock::new(HashMap::new())),
            validation_results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn set_health_result(&self, endpoint: &str, healthy: bool) {
        let mut results = self.health_results.write().await;
        results.insert(endpoint.to_string(), healthy);
    }

    async fn set_validation_result(&self, rule_name: &str, passed: bool) {
        let mut results = self.validation_results.write().await;
        results.insert(rule_name.to_string(), passed);
    }
}

#[async_trait::async_trait]
impl HealthChecker for MockHealthChecker {
    async fn check_health(&self, endpoint: &str, _timeout: Duration) -> Result<HealthCheckResult> {
        let results = self.health_results.read().await;
        let is_healthy = results.get(endpoint).copied().unwrap_or(true);
        
        Ok(HealthCheckResult {
            endpoint: endpoint.to_string(),
            status: is_healthy,
            response_time: Duration::from_millis(100),
            error_message: if is_healthy { None } else { Some("Health check failed".to_string()) },
            checked_at: SystemTime::now(),
        })
    }

    async fn validate_rule(&self, rule: &ValidationRule) -> Result<ValidationResult> {
        let results = self.validation_results.read().await;
        let passed = results.get(&rule.name).copied().unwrap_or(true);
        
        Ok(ValidationResult {
            rule_name: rule.name.clone(),
            passed,
            error_message: if passed { None } else { Some("Validation failed".to_string()) },
            validated_at: SystemTime::now(),
        })
    }
}

#[tokio::test]
async fn test_blue_green_deployment_success() {
    let health_checker = Arc::new(MockHealthChecker::new());
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let traffic_config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
    
    let manager = ProductionBlueGreenManager::new(health_checker.clone(), traffic_router);

    let config = DeploymentConfig {
        deployment_id: Uuid::new_v4(),
        version: "1.0.0".to_string(),
        environment: DeploymentEnvironment::Green,
        health_check_timeout: Duration::from_secs(30),
        rollback_timeout: Duration::from_secs(300),
        traffic_switch_delay: Duration::from_secs(10),
        health_check_endpoints: vec!["http://green:8080/health".to_string()],
        validation_rules: vec![
            ValidationRule {
                name: "api_test".to_string(),
                endpoint: "http://green:8080/api/test".to_string(),
                expected_status: 200,
                timeout: Duration::from_secs(10),
                retry_count: 2,
            }
        ],
    };

    // Set up successful health checks
    health_checker.set_health_result("http://green:8080/health", true).await;
    health_checker.set_validation_result("api_test", true).await;

    let deployment_id = manager.start_deployment(config).await.unwrap();

    // Wait for deployment to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    let status = manager.get_deployment_status(deployment_id).await.unwrap();
    assert_eq!(status.status, DeploymentStatus::Ready);
    assert_eq!(status.environment, DeploymentEnvironment::Green);

    // Validate deployment
    let is_valid = manager.validate_deployment(deployment_id).await.unwrap();
    assert!(is_valid);

    // Switch traffic
    let switch_result = manager.switch_traffic(deployment_id).await;
    assert!(switch_result.is_ok());

    let active_env = manager.get_active_environment().await.unwrap();
    assert_eq!(active_env, DeploymentEnvironment::Green);
}

#[tokio::test]
async fn test_blue_green_deployment_failure() {
    let health_checker = Arc::new(MockHealthChecker::new());
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let traffic_config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
    
    let manager = ProductionBlueGreenManager::new(health_checker.clone(), traffic_router);

    let config = DeploymentConfig {
        deployment_id: Uuid::new_v4(),
        version: "1.0.0".to_string(),
        environment: DeploymentEnvironment::Green,
        health_check_timeout: Duration::from_secs(30),
        rollback_timeout: Duration::from_secs(300),
        traffic_switch_delay: Duration::from_secs(10),
        health_check_endpoints: vec!["http://green:8080/health".to_string()],
        validation_rules: vec![
            ValidationRule {
                name: "api_test".to_string(),
                endpoint: "http://green:8080/api/test".to_string(),
                expected_status: 200,
                timeout: Duration::from_secs(10),
                retry_count: 2,
            }
        ],
    };

    // Set up failing health checks
    health_checker.set_health_result("http://green:8080/health", false).await;
    health_checker.set_validation_result("api_test", false).await;

    let deployment_id = manager.start_deployment(config).await.unwrap();

    // Wait for deployment to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    let status = manager.get_deployment_status(deployment_id).await.unwrap();
    assert_eq!(status.status, DeploymentStatus::Failed);

    // Validate deployment should fail
    let is_valid = manager.validate_deployment(deployment_id).await.unwrap();
    assert!(!is_valid);

    // Traffic switch should fail for failed deployment
    let switch_result = manager.switch_traffic(deployment_id).await;
    assert!(switch_result.is_err());
}

#[tokio::test]
async fn test_deployment_rollback() {
    let health_checker = Arc::new(MockHealthChecker::new());
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let traffic_config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
    
    let manager = ProductionBlueGreenManager::new(health_checker.clone(), traffic_router);

    // First, deploy to green and switch traffic
    let config = DeploymentConfig {
        deployment_id: Uuid::new_v4(),
        version: "1.0.0".to_string(),
        environment: DeploymentEnvironment::Green,
        health_check_timeout: Duration::from_secs(30),
        rollback_timeout: Duration::from_secs(300),
        traffic_switch_delay: Duration::from_secs(10),
        health_check_endpoints: vec!["http://green:8080/health".to_string()],
        validation_rules: vec![],
    };

    health_checker.set_health_result("http://green:8080/health", true).await;
    
    let deployment_id = manager.start_deployment(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    let _ = manager.switch_traffic(deployment_id).await;
    assert_eq!(manager.get_active_environment().await.unwrap(), DeploymentEnvironment::Green);

    // Now rollback
    let rollback_config = RollbackConfig {
        deployment_id,
        target_version: "0.9.0".to_string(),
        reason: "Critical bug found".to_string(),
        automatic: false,
    };

    let rollback_id = manager.rollback_deployment(rollback_config).await.unwrap();
    assert!(rollback_id != deployment_id);

    // Should switch back to blue
    assert_eq!(manager.get_active_environment().await.unwrap(), DeploymentEnvironment::Blue);

    // Original deployment should be marked as rolled back
    let status = manager.get_deployment_status(deployment_id).await.unwrap();
    assert_eq!(status.status, DeploymentStatus::RolledBack);
}

#[tokio::test]
async fn test_traffic_router_gradual_switch() {
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    
    let router = ProductionTrafficRouter::new(config, load_balancer);

    // Test gradual switch to green
    let result = router.gradual_traffic_switch(
        &DeploymentEnvironment::Green,
        4,
        Duration::from_millis(50),
    ).await;
    
    assert!(result.is_ok());

    let split = router.get_current_traffic_split().await.unwrap();
    assert_eq!(split.get(&DeploymentEnvironment::Green), Some(&100.0));
    assert_eq!(split.get(&DeploymentEnvironment::Blue), Some(&0.0));
}

#[tokio::test]
async fn test_traffic_split_validation() {
    let valid_split = TrafficSplit::new(70.0, 30.0);
    assert!(valid_split.is_ok());

    let invalid_split = TrafficSplit::new(70.0, 40.0);
    assert!(invalid_split.is_err());

    let all_blue = TrafficSplit::all_blue();
    assert_eq!(all_blue.blue_percentage, 100.0);
    assert_eq!(all_blue.green_percentage, 0.0);

    let all_green = TrafficSplit::all_green();
    assert_eq!(all_green.blue_percentage, 0.0);
    assert_eq!(all_green.green_percentage, 100.0);
}

#[tokio::test]
async fn test_deployment_automation_pipeline() {
    let health_checker = Arc::new(MockHealthChecker::new());
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let traffic_config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
    let deployment_manager = Arc::new(ProductionBlueGreenManager::new(health_checker.clone(), traffic_router));
    let notification_service = Arc::new(MockNotificationService);
    
    let automation = ProductionDeploymentAutomation::new(deployment_manager, notification_service);

    // Create a deployment pipeline
    let pipeline = DeploymentPipeline {
        pipeline_id: Uuid::new_v4(),
        name: "production-deployment".to_string(),
        stages: vec![
            DeploymentStage {
                name: "pre-deployment".to_string(),
                stage_type: StageType::PreDeployment,
                timeout: Duration::from_secs(60),
                retry_count: 1,
                continue_on_failure: false,
                validation_rules: vec![],
            },
            DeploymentStage {
                name: "deploy".to_string(),
                stage_type: StageType::Deploy,
                timeout: Duration::from_secs(300),
                retry_count: 2,
                continue_on_failure: false,
                validation_rules: vec![],
            },
            DeploymentStage {
                name: "health-check".to_string(),
                stage_type: StageType::HealthCheck,
                timeout: Duration::from_secs(120),
                retry_count: 3,
                continue_on_failure: false,
                validation_rules: vec![],
            },
            DeploymentStage {
                name: "traffic-switch".to_string(),
                stage_type: StageType::TrafficSwitch,
                timeout: Duration::from_secs(60),
                retry_count: 1,
                continue_on_failure: false,
                validation_rules: vec![],
            },
        ],
        rollback_strategy: RollbackStrategy::Automatic,
        notification_config: NotificationConfig {
            webhook_urls: vec!["http://webhook.example.com".to_string()],
            email_addresses: vec!["admin@example.com".to_string()],
            slack_channels: vec!["#deployments".to_string()],
            notify_on_success: true,
            notify_on_failure: true,
            notify_on_rollback: true,
        },
    };

    let pipeline_id = automation.create_pipeline(pipeline).await.unwrap();

    // Create deployment config
    let config = AutomatedDeploymentConfig {
        application_name: "test-app".to_string(),
        version: "2.0.0".to_string(),
        image_tag: "test-app:2.0.0".to_string(),
        target_environment: Some(DeploymentEnvironment::Green),
        auto_promote: true,
        rollback_on_failure: true,
        health_check_timeout: Duration::from_secs(60),
        validation_timeout: Duration::from_secs(120),
        traffic_switch_delay: Duration::from_secs(30),
        custom_validation_rules: vec![],
    };

    // Set up successful health checks
    health_checker.set_health_result("http://test-app:8080/health", true).await;
    health_checker.set_health_result("http://test-app:8080/ready", true).await;

    let execution_id = automation.execute_deployment(config, pipeline_id).await.unwrap();

    // Wait for pipeline execution
    tokio::time::sleep(Duration::from_millis(300)).await;

    let execution = automation.get_execution_status(execution_id).await.unwrap();
    assert_eq!(execution.pipeline_id, pipeline_id);
    
    // Check that stages were executed
    assert!(!execution.stage_results.is_empty());
}

#[tokio::test]
async fn test_deployment_list_and_cleanup() {
    let health_checker = Arc::new(MockHealthChecker::new());
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let traffic_config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
    
    let manager = ProductionBlueGreenManager::new(health_checker.clone(), traffic_router);

    // Create multiple deployments
    let config1 = DeploymentConfig {
        deployment_id: Uuid::new_v4(),
        version: "1.0.0".to_string(),
        environment: DeploymentEnvironment::Green,
        health_check_timeout: Duration::from_secs(30),
        rollback_timeout: Duration::from_secs(300),
        traffic_switch_delay: Duration::from_secs(10),
        health_check_endpoints: vec!["http://green:8080/health".to_string()],
        validation_rules: vec![],
    };

    let config2 = DeploymentConfig {
        deployment_id: Uuid::new_v4(),
        version: "1.1.0".to_string(),
        environment: DeploymentEnvironment::Blue,
        health_check_timeout: Duration::from_secs(30),
        rollback_timeout: Duration::from_secs(300),
        traffic_switch_delay: Duration::from_secs(10),
        health_check_endpoints: vec!["http://blue:8080/health".to_string()],
        validation_rules: vec![],
    };

    health_checker.set_health_result("http://green:8080/health", true).await;
    health_checker.set_health_result("http://blue:8080/health", true).await;

    let deployment_id1 = manager.start_deployment(config1).await.unwrap();
    let deployment_id2 = manager.start_deployment(config2).await.unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    // List all deployments
    let deployments = manager.list_deployments().await.unwrap();
    assert_eq!(deployments.len(), 2);

    // Cleanup one deployment
    let cleanup_result = manager.cleanup_old_deployment(deployment_id1).await;
    assert!(cleanup_result.is_ok());

    // Should have one less deployment
    let deployments_after_cleanup = manager.list_deployments().await.unwrap();
    assert_eq!(deployments_after_cleanup.len(), 1);
    assert_eq!(deployments_after_cleanup[0].deployment_id, deployment_id2);
}

#[tokio::test]
async fn test_health_checker_retry_logic() {
    let health_checker = MockHealthChecker::new();
    
    let rule = ValidationRule {
        name: "flaky_test".to_string(),
        endpoint: "http://flaky:8080/api/test".to_string(),
        expected_status: 200,
        timeout: Duration::from_secs(5),
        retry_count: 3,
    };

    // First set it to fail, then succeed
    health_checker.set_validation_result("flaky_test", false).await;
    
    let result = health_checker.validate_rule(&rule).await.unwrap();
    assert!(!result.passed);
    assert!(result.error_message.is_some());

    // Now set it to succeed
    health_checker.set_validation_result("flaky_test", true).await;
    
    let result = health_checker.validate_rule(&rule).await.unwrap();
    assert!(result.passed);
    assert!(result.error_message.is_none());
}