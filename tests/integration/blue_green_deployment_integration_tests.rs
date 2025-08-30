use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use deployment::{
    ProductionBlueGreenManager, DeploymentConfig, DeploymentEnvironment, 
    DeploymentStatus, ProductionHealthChecker, ProductionTrafficRouter, 
    TrafficRoutingConfig, MockLoadBalancerAdapter
};

#[tokio::test]
async fn test_blue_green_deployment_integration() {
    // Set up the blue-green deployment system
    let health_checker = Arc::new(ProductionHealthChecker::new());
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    
    let traffic_config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    
    let traffic_router = Arc::new(ProductionTrafficRouter::new(traffic_config, load_balancer));
    let deployment_manager = ProductionBlueGreenManager::new(health_checker, traffic_router);

    // Create a deployment configuration
    let deployment_config = DeploymentConfig {
        deployment_id: Uuid::new_v4(),
        version: "1.0.0".to_string(),
        environment: DeploymentEnvironment::Green,
        health_check_timeout: Duration::from_secs(30),
        rollback_timeout: Duration::from_secs(300),
        traffic_switch_delay: Duration::from_secs(5),
        health_check_endpoints: vec![
            "http://localhost:8080/health".to_string(),
        ],
        validation_rules: vec![],
    };

    // Start the deployment
    let deployment_id = deployment_manager.start_deployment(deployment_config).await;
    
    // Verify deployment was created
    assert!(deployment_id.is_ok());
    let deployment_id = deployment_id.unwrap();

    // Wait a moment for the deployment to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check deployment status
    let status = deployment_manager.get_deployment_status(deployment_id).await;
    assert!(status.is_ok());
    
    let status = status.unwrap();
    assert_eq!(status.deployment_id, deployment_id);
    assert_eq!(status.environment, DeploymentEnvironment::Green);

    // List deployments
    let deployments = deployment_manager.list_deployments().await;
    assert!(deployments.is_ok());
    
    let deployments = deployments.unwrap();
    assert!(!deployments.is_empty());
    assert!(deployments.iter().any(|d| d.deployment_id == deployment_id));

    // Test cleanup
    let cleanup_result = deployment_manager.cleanup_old_deployment(deployment_id).await;
    assert!(cleanup_result.is_ok());
}

#[tokio::test]
async fn test_traffic_router_integration() {
    let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
    let config = TrafficRoutingConfig {
        blue_upstream: "blue:8080".to_string(),
        green_upstream: "green:8080".to_string(),
        health_check_interval: Duration::from_secs(30),
        failover_threshold: 3,
    };
    
    let router = ProductionTrafficRouter::new(config, load_balancer);

    // Test initial state
    let initial_split = router.get_current_traffic_split().await;
    assert!(initial_split.is_ok());
    
    let split = initial_split.unwrap();
    assert_eq!(split.get(&DeploymentEnvironment::Blue), Some(&100.0));
    assert_eq!(split.get(&DeploymentEnvironment::Green), Some(&0.0));

    // Test traffic switch
    let switch_result = router.switch_traffic(&DeploymentEnvironment::Green).await;
    assert!(switch_result.is_ok());

    let new_split = router.get_current_traffic_split().await.unwrap();
    assert_eq!(new_split.get(&DeploymentEnvironment::Green), Some(&100.0));
    assert_eq!(new_split.get(&DeploymentEnvironment::Blue), Some(&0.0));
}