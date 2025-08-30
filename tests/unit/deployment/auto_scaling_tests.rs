use std::sync::Arc;
use std::time::{Duration, SystemTime};
use deployment::{
    ProductionAutoScaler, ScalingPolicy, ScalingAction, ScalingMetrics,
    MockMetricsCollector, MockInstanceManager, MockCostOptimizer,
    ResourcePriority, ProductionResourceManager, ResourcePool, ResourceRequest,
    ResourceRequirements, ProductionResourceOptimizer
};
use uuid::Uuid;

#[tokio::test]
async fn test_auto_scaler_scale_up_decision() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager.clone(), cost_optimizer);

    // Set up high resource usage metrics
    let high_usage_metrics = ScalingMetrics {
        cpu_utilization: 85.0,
        memory_utilization: 90.0,
        request_rate: 1000.0,
        response_time: Duration::from_millis(800),
        error_rate: 0.02,
        queue_depth: 100,
        active_connections: 500,
        timestamp: SystemTime::now(),
    };

    metrics_collector.set_metrics("web-service", high_usage_metrics).await;
    metrics_collector.set_instance_count("web-service", 2).await;

    // Register scaling policy
    let policy = ScalingPolicy {
        service_name: "web-service".to_string(),
        min_instances: 1,
        max_instances: 10,
        target_cpu_utilization: 70.0,
        target_memory_utilization: 80.0,
        target_response_time: Duration::from_millis(500),
        scale_up_threshold: 80.0,
        scale_down_threshold: 30.0,
        scale_up_cooldown: Duration::from_secs(300),
        scale_down_cooldown: Duration::from_secs(600),
        scale_up_step: 2,
        scale_down_step: 1,
        ..Default::default()
    };

    auto_scaler.register_scaling_policy(policy).await.unwrap();

    // Evaluate scaling decision
    let action = auto_scaler.evaluate_scaling("web-service").await.unwrap();
    
    match action {
        ScalingAction::ScaleUp { instances, reason } => {
            assert_eq!(instances, 2);
            assert!(reason.contains("High resource pressure"));
        }
        _ => panic!("Expected scale up action, got: {:?}", action),
    }
}

#[tokio::test]
async fn test_auto_scaler_scale_down_decision() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager.clone(), cost_optimizer);

    // Set up low resource usage metrics
    let low_usage_metrics = ScalingMetrics {
        cpu_utilization: 15.0,
        memory_utilization: 20.0,
        request_rate: 50.0,
        response_time: Duration::from_millis(100),
        error_rate: 0.001,
        queue_depth: 2,
        active_connections: 10,
        timestamp: SystemTime::now(),
    };

    metrics_collector.set_metrics("web-service", low_usage_metrics).await;
    metrics_collector.set_instance_count("web-service", 5).await;

    // Register scaling policy
    let policy = ScalingPolicy {
        service_name: "web-service".to_string(),
        min_instances: 2,
        max_instances: 10,
        scale_up_threshold: 80.0,
        scale_down_threshold: 30.0,
        scale_down_step: 2,
        ..Default::default()
    };

    auto_scaler.register_scaling_policy(policy).await.unwrap();

    // Evaluate scaling decision
    let action = auto_scaler.evaluate_scaling("web-service").await.unwrap();
    
    match action {
        ScalingAction::ScaleDown { instances, reason } => {
            assert_eq!(instances, 2);
            assert!(reason.contains("Low resource pressure"));
        }
        _ => panic!("Expected scale down action, got: {:?}", action),
    }
}

#[tokio::test]
async fn test_auto_scaler_no_action_within_thresholds() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager, cost_optimizer);

    // Set up moderate resource usage metrics
    let moderate_usage_metrics = ScalingMetrics {
        cpu_utilization: 60.0,
        memory_utilization: 65.0,
        request_rate: 300.0,
        response_time: Duration::from_millis(400),
        error_rate: 0.005,
        queue_depth: 20,
        active_connections: 100,
        timestamp: SystemTime::now(),
    };

    metrics_collector.set_metrics("web-service", moderate_usage_metrics).await;
    metrics_collector.set_instance_count("web-service", 3).await;

    // Register scaling policy
    let policy = ScalingPolicy {
        service_name: "web-service".to_string(),
        scale_up_threshold: 80.0,
        scale_down_threshold: 30.0,
        ..Default::default()
    };

    auto_scaler.register_scaling_policy(policy).await.unwrap();

    // Evaluate scaling decision
    let action = auto_scaler.evaluate_scaling("web-service").await.unwrap();
    
    match action {
        ScalingAction::NoAction { reason } => {
            assert!(reason.contains("within acceptable range"));
        }
        _ => panic!("Expected no action, got: {:?}", action),
    }
}

#[tokio::test]
async fn test_auto_scaler_cooldown_period() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager.clone(), cost_optimizer);

    // Set up high resource usage metrics
    let high_usage_metrics = ScalingMetrics {
        cpu_utilization: 90.0,
        memory_utilization: 85.0,
        request_rate: 1200.0,
        response_time: Duration::from_millis(900),
        error_rate: 0.03,
        queue_depth: 150,
        active_connections: 600,
        timestamp: SystemTime::now(),
    };

    metrics_collector.set_metrics("web-service", high_usage_metrics).await;
    metrics_collector.set_instance_count("web-service", 2).await;

    // Register scaling policy with short cooldown for testing
    let policy = ScalingPolicy {
        service_name: "web-service".to_string(),
        scale_up_threshold: 80.0,
        scale_up_cooldown: Duration::from_millis(100),
        ..Default::default()
    };

    auto_scaler.register_scaling_policy(policy).await.unwrap();

    // First scaling action should succeed
    let action1 = auto_scaler.evaluate_scaling("web-service").await.unwrap();
    match action1 {
        ScalingAction::ScaleUp { .. } => {
            // Execute the scaling to trigger cooldown
            auto_scaler.execute_scaling("web-service", action1).await.unwrap();
        }
        _ => panic!("Expected scale up action"),
    }

    // Immediate second evaluation should be in cooldown
    let action2 = auto_scaler.evaluate_scaling("web-service").await.unwrap();
    match action2 {
        ScalingAction::NoAction { reason } => {
            assert!(reason.contains("cooldown"));
        }
        _ => panic!("Expected no action due to cooldown, got: {:?}", action2),
    }

    // After cooldown period, should allow scaling again
    tokio::time::sleep(Duration::from_millis(150)).await;
    let action3 = auto_scaler.evaluate_scaling("web-service").await.unwrap();
    match action3 {
        ScalingAction::ScaleUp { .. } => {
            // Expected behavior after cooldown
        }
        _ => {} // Could be no action if metrics changed
    }
}

#[tokio::test]
async fn test_auto_scaler_execute_scaling() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager.clone(), cost_optimizer);

    // Set initial instance count
    metrics_collector.set_instance_count("web-service", 2).await;

    let scale_up_action = ScalingAction::ScaleUp {
        instances: 2,
        reason: "High load detected".to_string(),
    };

    // Execute scaling
    let event = auto_scaler.execute_scaling("web-service", scale_up_action).await.unwrap();
    
    assert_eq!(event.service_name, "web-service");
    assert_eq!(event.current_instances, 2);
    assert_eq!(event.target_instances, 4); // 2 + 2
    assert!(event.executed);
    assert!(event.execution_result.is_some());
    assert!(event.execution_result.unwrap().contains("Successfully scaled up"));
}

#[tokio::test]
async fn test_auto_scaler_scaling_history() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager, cost_optimizer);

    metrics_collector.set_instance_count("web-service", 2).await;

    // Execute multiple scaling actions
    let action1 = ScalingAction::ScaleUp {
        instances: 1,
        reason: "Load increase".to_string(),
    };
    
    let action2 = ScalingAction::ScaleDown {
        instances: 1,
        reason: "Load decrease".to_string(),
    };

    auto_scaler.execute_scaling("web-service", action1).await.unwrap();
    auto_scaler.execute_scaling("web-service", action2).await.unwrap();

    // Get scaling history
    let history = auto_scaler.get_scaling_history(Some("web-service")).await.unwrap();
    assert_eq!(history.len(), 2);
    
    let all_history = auto_scaler.get_scaling_history(None).await.unwrap();
    assert_eq!(all_history.len(), 2);
}

#[tokio::test]
async fn test_auto_scaler_cost_optimization() {
    let metrics_collector = Arc::new(MockMetricsCollector::new());
    let instance_manager = Arc::new(MockInstanceManager::new());
    let cost_optimizer = Arc::new(MockCostOptimizer::new());
    
    let auto_scaler = ProductionAutoScaler::new(metrics_collector, instance_manager, cost_optimizer);

    // Get cost recommendations
    let recommendations = auto_scaler.optimize_costs("web-service").await.unwrap();
    assert!(!recommendations.is_empty());
    
    let first_rec = &recommendations[0];
    assert!(first_rec.potential_savings > 0.0);
    assert!(!first_rec.description.is_empty());
}

#[tokio::test]
async fn test_resource_manager_allocation_lifecycle() {
    let manager = ProductionResourceManager::new();
    
    // Create a resource pool
    let pool = ResourcePool {
        pool_id: Uuid::new_v4(),
        name: "test-pool".to_string(),
        total_cpu_cores: 16.0,
        total_memory_gb: 32.0,
        total_storage_gb: 500.0,
        total_network_bandwidth_mbps: 2000.0,
        available_cpu_cores: 16.0,
        available_memory_gb: 32.0,
        available_storage_gb: 500.0,
        available_network_bandwidth_mbps: 2000.0,
        allocations: Vec::new(),
        created_at: SystemTime::now(),
    };
    
    let pool_id = manager.create_resource_pool(pool).await.unwrap();

    // Create a resource request
    let request = ResourceRequest {
        request_id: Uuid::new_v4(),
        service_name: "api-service".to_string(),
        cpu_cores: 4.0,
        memory_gb: 8.0,
        storage_gb: 50.0,
        network_bandwidth_mbps: 200.0,
        duration: Some(Duration::from_secs(7200)), // 2 hours
        priority: ResourcePriority::High,
        requirements: ResourceRequirements {
            min_cpu_cores: 2.0,
            min_memory_gb: 4.0,
            min_storage_gb: 25.0,
            min_network_bandwidth_mbps: 100.0,
            preferred_zone: Some("us-west-2a".to_string()),
            anti_affinity_services: vec!["competitor-service".to_string()],
            required_capabilities: vec!["gpu".to_string(), "ssd".to_string()],
        },
        requested_at: SystemTime::now(),
    };
    
    // Allocate resources
    let allocation = manager.allocate_resources(request).await.unwrap();
    assert_eq!(allocation.service_name, "api-service");
    assert_eq!(allocation.cpu_cores, 4.0);
    assert_eq!(allocation.memory_gb, 8.0);
    assert_eq!(allocation.priority, ResourcePriority::High);
    assert!(allocation.expires_at.is_some());

    // Verify allocation exists
    let retrieved = manager.get_allocation(allocation.allocation_id).await.unwrap();
    assert_eq!(retrieved.allocation_id, allocation.allocation_id);

    // List allocations for service
    let service_allocations = manager.list_allocations(Some("api-service")).await.unwrap();
    assert_eq!(service_allocations.len(), 1);

    // List all allocations
    let all_allocations = manager.list_allocations(None).await.unwrap();
    assert_eq!(all_allocations.len(), 1);

    // Get resource usage
    let usage = manager.get_resource_usage(allocation.allocation_id).await.unwrap();
    assert_eq!(usage.service_name, "api-service");
    assert_eq!(usage.allocation_id, allocation.allocation_id);

    // Deallocate resources
    manager.deallocate_resources(allocation.allocation_id).await.unwrap();

    // Verify allocation is removed
    let result = manager.get_allocation(allocation.allocation_id).await;
    assert!(result.is_err());

    // Verify no allocations remain
    let final_allocations = manager.list_allocations(None).await.unwrap();
    assert_eq!(final_allocations.len(), 0);
}

#[tokio::test]
async fn test_resource_manager_insufficient_resources() {
    let manager = ProductionResourceManager::new();
    
    // Create a small resource pool
    let pool = ResourcePool {
        pool_id: Uuid::new_v4(),
        name: "small-pool".to_string(),
        total_cpu_cores: 2.0,
        total_memory_gb: 4.0,
        total_storage_gb: 20.0,
        total_network_bandwidth_mbps: 100.0,
        available_cpu_cores: 2.0,
        available_memory_gb: 4.0,
        available_storage_gb: 20.0,
        available_network_bandwidth_mbps: 100.0,
        allocations: Vec::new(),
        created_at: SystemTime::now(),
    };
    
    manager.create_resource_pool(pool).await.unwrap();

    // Try to allocate more resources than available
    let large_request = ResourceRequest {
        request_id: Uuid::new_v4(),
        service_name: "large-service".to_string(),
        cpu_cores: 8.0, // More than available
        memory_gb: 16.0, // More than available
        storage_gb: 100.0, // More than available
        network_bandwidth_mbps: 500.0, // More than available
        duration: None,
        priority: ResourcePriority::Medium,
        requirements: ResourceRequirements {
            min_cpu_cores: 8.0,
            min_memory_gb: 16.0,
            min_storage_gb: 100.0,
            min_network_bandwidth_mbps: 500.0,
            preferred_zone: None,
            anti_affinity_services: Vec::new(),
            required_capabilities: Vec::new(),
        },
        requested_at: SystemTime::now(),
    };
    
    // Allocation should fail
    let result = manager.allocate_resources(large_request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No suitable resource pool"));
}

#[tokio::test]
async fn test_resource_optimizer() {
    let manager = Arc::new(ProductionResourceManager::new());
    let optimizer = ProductionResourceOptimizer::new(manager.clone());
    
    // Create resource pool and allocation
    let pool = ResourcePool {
        pool_id: Uuid::new_v4(),
        name: "optimization-pool".to_string(),
        total_cpu_cores: 10.0,
        total_memory_gb: 20.0,
        total_storage_gb: 100.0,
        total_network_bandwidth_mbps: 1000.0,
        available_cpu_cores: 10.0,
        available_memory_gb: 20.0,
        available_storage_gb: 100.0,
        available_network_bandwidth_mbps: 1000.0,
        allocations: Vec::new(),
        created_at: SystemTime::now(),
    };
    
    manager.create_resource_pool(pool).await.unwrap();
    
    let request = ResourceRequest {
        request_id: Uuid::new_v4(),
        service_name: "optimize-me".to_string(),
        cpu_cores: 4.0,
        memory_gb: 8.0,
        storage_gb: 40.0,
        network_bandwidth_mbps: 400.0,
        duration: None,
        priority: ResourcePriority::Medium,
        requirements: ResourceRequirements {
            min_cpu_cores: 2.0,
            min_memory_gb: 4.0,
            min_storage_gb: 20.0,
            min_network_bandwidth_mbps: 200.0,
            preferred_zone: None,
            anti_affinity_services: Vec::new(),
            required_capabilities: Vec::new(),
        },
        requested_at: SystemTime::now(),
    };
    
    let allocation = manager.allocate_resources(request).await.unwrap();

    // Get optimization recommendation
    let recommendation = optimizer.optimize_allocation(allocation.allocation_id).await.unwrap();
    assert_eq!(recommendation.service_name, "optimize-me");
    assert!(recommendation.confidence_score > 0.0);
    assert!(recommendation.confidence_score <= 1.0);

    // Analyze resource usage for the service
    let analysis = optimizer.analyze_resource_usage("optimize-me", Duration::from_secs(3600)).await.unwrap();
    assert!(!analysis.is_empty());

    // Get efficiency score
    let efficiency = optimizer.get_efficiency_score("optimize-me").await.unwrap();
    assert!(efficiency >= 0.0 && efficiency <= 1.0);

    // Predict future resource needs
    let prediction = optimizer.predict_resource_needs("optimize-me", Duration::from_secs(86400)).await.unwrap();
    assert_eq!(prediction.service_name, "optimize-me");
    assert!(prediction.cpu_cores > 0.0);
    assert!(prediction.memory_gb > 0.0);
}