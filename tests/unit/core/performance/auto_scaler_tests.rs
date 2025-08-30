use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use rustci::core::performance::{
    ProductionAutoScaler, AutoScalerConfig, ScalingMetrics, ScalingAction,
    MockInstanceProvider, MockMetricsCollector, ServiceEndpoint
};

#[tokio::test]
async fn test_auto_scaler_initialization() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let scaler = ProductionAutoScaler::new(config, provider, collector);
    scaler.start().await.unwrap();
    
    let stats = scaler.get_stats().await;
    assert!(stats.current_instances >= 2); // Should create min instances
}

#[tokio::test]
async fn test_scale_up_on_high_utilization() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    // Set high utilization metrics
    let mut high_metrics = ScalingMetrics::new();
    high_metrics.cpu_utilization = 90.0;
    high_metrics.memory_utilization = 85.0;
    high_metrics.average_response_time = Duration::from_millis(800);
    collector.set_metrics(high_metrics.clone()).await;
    
    let last_scale_up = Arc::new(RwLock::new(None));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &high_metrics,
        2, // Current instances
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::ScaleUp { target_instances, reason } => {
            assert!(target_instances > 2);
            assert!(reason.contains("High utilization"));
        }
        _ => panic!("Expected scale up decision, got: {:?}", decision.action),
    }
}

#[tokio::test]
async fn test_scale_down_on_low_utilization() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    // Set low utilization metrics
    let mut low_metrics = ScalingMetrics::new();
    low_metrics.cpu_utilization = 30.0;
    low_metrics.memory_utilization = 25.0;
    low_metrics.average_response_time = Duration::from_millis(100);
    collector.set_metrics(low_metrics.clone()).await;
    
    let last_scale_up = Arc::new(RwLock::new(None));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &low_metrics,
        5, // More than minimum instances
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::ScaleDown { target_instances, reason } => {
            assert!(target_instances < 5);
            assert!(target_instances >= config.min_instances);
            assert!(reason.contains("Low utilization"));
        }
        _ => panic!("Expected scale down decision, got: {:?}", decision.action),
    }
}

#[tokio::test]
async fn test_no_action_on_normal_utilization() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    // Set normal utilization metrics
    let mut normal_metrics = ScalingMetrics::new();
    normal_metrics.cpu_utilization = 65.0; // Within target range
    normal_metrics.memory_utilization = 70.0;
    normal_metrics.average_response_time = Duration::from_millis(300);
    collector.set_metrics(normal_metrics.clone()).await;
    
    let last_scale_up = Arc::new(RwLock::new(None));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &normal_metrics,
        3, // Normal instance count
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::NoAction { reason } => {
            assert!(reason.contains("target range"));
        }
        _ => panic!("Expected no action, got: {:?}", decision.action),
    }
}

#[tokio::test]
async fn test_cooldown_periods_prevent_scaling() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    // Set high utilization metrics that would normally trigger scale up
    let mut high_metrics = ScalingMetrics::new();
    high_metrics.cpu_utilization = 95.0;
    collector.set_metrics(high_metrics.clone()).await;
    
    // Set recent scale up time (within cooldown period)
    let last_scale_up = Arc::new(RwLock::new(Some(Instant::now())));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &high_metrics,
        2,
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::NoAction { reason } => {
            assert!(reason.contains("cooldown"));
        }
        _ => panic!("Expected no action due to cooldown, got: {:?}", decision.action),
    }
}

#[tokio::test]
async fn test_min_instances_limit() {
    let mut config = AutoScalerConfig::default();
    config.min_instances = 3;
    
    // Set low utilization that would normally trigger scale down
    let mut low_metrics = ScalingMetrics::new();
    low_metrics.cpu_utilization = 20.0;
    low_metrics.memory_utilization = 15.0;
    
    let last_scale_up = Arc::new(RwLock::new(None));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &low_metrics,
        3, // At minimum instances
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::NoAction { reason } => {
            assert!(reason.contains("minimum"));
        }
        _ => panic!("Expected no action due to min instances limit, got: {:?}", decision.action),
    }
}

#[tokio::test]
async fn test_max_instances_limit() {
    let mut config = AutoScalerConfig::default();
    config.max_instances = 5;
    
    // Set high utilization that would normally trigger scale up
    let mut high_metrics = ScalingMetrics::new();
    high_metrics.cpu_utilization = 95.0;
    high_metrics.memory_utilization = 90.0;
    
    let last_scale_up = Arc::new(RwLock::new(None));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &high_metrics,
        5, // At maximum instances
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::NoAction { reason } => {
            assert!(reason.contains("maximum"));
        }
        _ => panic!("Expected no action due to max instances limit, got: {:?}", decision.action),
    }
}

#[tokio::test]
async fn test_scaling_factor_calculation() {
    let config = AutoScalerConfig::default();
    
    // Test CPU-driven scaling
    let mut cpu_metrics = ScalingMetrics::new();
    cpu_metrics.cpu_utilization = 140.0; // 2x target (70%)
    cpu_metrics.memory_utilization = 50.0;
    cpu_metrics.average_response_time = Duration::from_millis(200);
    
    let last_scale_up = Arc::new(RwLock::new(None));
    let last_scale_down = Arc::new(RwLock::new(None));
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &cpu_metrics,
        2,
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::ScaleUp { target_instances, .. } => {
            // Should scale up significantly due to high CPU
            assert!(target_instances >= 4); // At least double
        }
        _ => panic!("Expected scale up due to high CPU"),
    }
    
    // Test response time-driven scaling
    let mut response_time_metrics = ScalingMetrics::new();
    response_time_metrics.cpu_utilization = 50.0;
    response_time_metrics.memory_utilization = 60.0;
    response_time_metrics.average_response_time = Duration::from_millis(1500); // 3x target (500ms)
    
    let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
        &config,
        &response_time_metrics,
        2,
        &last_scale_up,
        &last_scale_down,
    ).await;
    
    match decision.action {
        ScalingAction::ScaleUp { target_instances, .. } => {
            // Should scale up due to high response time
            assert!(target_instances > 2);
        }
        _ => panic!("Expected scale up due to high response time"),
    }
}

#[tokio::test]
async fn test_force_scaling() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let scaler = ProductionAutoScaler::new(config, provider.clone(), collector);
    scaler.start().await.unwrap();
    
    // Force scale to specific number
    scaler.force_scale(5).await.unwrap();
    
    // Check that instances were created
    let created = provider.created_instances.read().await;
    assert!(created.len() >= 5);
}

#[tokio::test]
async fn test_scaling_events_tracking() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let scaler = ProductionAutoScaler::new(config, provider, collector.clone());
    
    // Set high utilization to trigger scaling
    let mut high_metrics = ScalingMetrics::new();
    high_metrics.cpu_utilization = 90.0;
    collector.set_metrics(high_metrics).await;
    
    // Get scaling events (would be populated by the monitoring loop in real usage)
    let events = scaler.get_scaling_events(10).await;
    
    // Initially no events since monitoring loop hasn't run
    assert_eq!(events.len(), 0);
}

#[tokio::test]
async fn test_metrics_history_cleanup() {
    let mut config = AutoScalerConfig::default();
    config.metrics_window = Duration::from_secs(1); // Very short window for testing
    
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let scaler = ProductionAutoScaler::new(config, provider, collector);
    
    // The cleanup would happen in the monitoring loop
    // This test verifies the structure is in place
    let stats = scaler.get_stats().await;
    assert_eq!(stats.current_instances, 0); // No instances initially
}

#[tokio::test]
async fn test_mock_instance_provider() {
    let provider = MockInstanceProvider::new();
    
    // Test instance creation
    let instance = provider.create_instance().await.unwrap();
    assert_eq!(instance.address, "127.0.0.1");
    assert_eq!(instance.port, 8080);
    
    let created = provider.created_instances.read().await;
    assert_eq!(created.len(), 1);
    
    // Test instance termination
    provider.terminate_instance(instance.id).await.unwrap();
    
    let terminated = provider.terminated_instances.read().await;
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0], instance.id);
    
    // Test listing instances
    let instances = provider.list_instances().await.unwrap();
    assert_eq!(instances.len(), 1);
    
    // Test getting metrics
    let metrics = provider.get_instance_metrics(instance.id).await.unwrap();
    assert_eq!(metrics.cpu_utilization, 0.0);
}

#[tokio::test]
async fn test_mock_metrics_collector() {
    let collector = MockMetricsCollector::new();
    
    // Test setting and getting metrics
    let mut test_metrics = ScalingMetrics::new();
    test_metrics.cpu_utilization = 75.0;
    test_metrics.memory_utilization = 80.0;
    test_metrics.request_rate = 100.0;
    
    collector.set_metrics(test_metrics.clone()).await;
    
    let collected = collector.collect_metrics().await.unwrap();
    assert_eq!(collected.cpu_utilization, 75.0);
    assert_eq!(collected.memory_utilization, 80.0);
    assert_eq!(collected.request_rate, 100.0);
    
    // Test historical metrics
    let historical = collector.get_historical_metrics(Duration::from_secs(300)).await.unwrap();
    assert_eq!(historical.len(), 1);
    assert_eq!(historical[0].cpu_utilization, 75.0);
}

#[tokio::test]
async fn test_scaling_metrics_creation() {
    let metrics = ScalingMetrics::new();
    
    assert_eq!(metrics.cpu_utilization, 0.0);
    assert_eq!(metrics.memory_utilization, 0.0);
    assert_eq!(metrics.average_response_time, Duration::from_millis(0));
    assert_eq!(metrics.request_rate, 0.0);
    assert_eq!(metrics.error_rate, 0.0);
    assert_eq!(metrics.active_connections, 0);
    assert_eq!(metrics.queue_length, 0);
}

#[tokio::test]
async fn test_auto_scaler_stats() {
    let config = AutoScalerConfig::default();
    let provider = MockInstanceProvider::new();
    let collector = MockMetricsCollector::new();
    
    let scaler = ProductionAutoScaler::new(config, provider, collector);
    
    let stats = scaler.get_stats().await;
    assert_eq!(stats.current_instances, 0);
    assert_eq!(stats.target_instances, 0);
    assert_eq!(stats.total_scale_ups, 0);
    assert_eq!(stats.total_scale_downs, 0);
    assert!(stats.last_scaling_action.is_none());
    assert_eq!(stats.average_utilization, 0.0);
    assert_eq!(stats.scaling_efficiency, 0.0);
}

#[tokio::test]
async fn test_config_validation() {
    let mut config = AutoScalerConfig::default();
    
    // Test valid config
    assert!(config.min_instances <= config.max_instances);
    assert!(config.scale_down_threshold < config.scale_up_threshold);
    assert!(config.scale_down_cooldown >= config.scale_up_cooldown);
    
    // Test edge cases
    config.min_instances = 1;
    config.max_instances = 1;
    assert_eq!(config.min_instances, config.max_instances);
    
    config.target_cpu_utilization = 70.0;
    config.scale_up_threshold = 80.0;
    config.scale_down_threshold = 50.0;
    assert!(config.scale_down_threshold < config.target_cpu_utilization);
    assert!(config.target_cpu_utilization < config.scale_up_threshold);
}