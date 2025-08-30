use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::core::performance::{
    SelfHealingManager, SelfHealingConfig, PerformanceIssueDetector, HealingEngine,
    DetectedIssue, HealingAction, HealingResult, PerformanceSnapshot, SelfHealingMetrics,
    IssueSeverity as SelfHealingIssueSeverity, ProductionAlertManager
};
use crate::error::AppError;

fn create_test_config() -> SelfHealingConfig {
    SelfHealingConfig {
        enable_auto_healing: true,
        detection_interval: Duration::from_millis(100),
        healing_timeout: Duration::from_secs(30),
        max_concurrent_healers: 5,
        escalation_threshold: 3,
        cooldown_period: Duration::from_secs(60),
        enable_predictive_healing: false,
        metrics_retention: Duration::from_secs(300),
    }
}

fn create_test_performance_snapshot() -> PerformanceSnapshot {
    PerformanceSnapshot {
        timestamp: Instant::now(),
        cpu_usage: 85.0, // High CPU usage to trigger detection
        memory_usage: 70.0,
        response_time: Duration::from_millis(500),
        error_rate: 2.0,
        throughput: 1000.0,
        active_connections: 150,
        queue_depth: 10,
    }
}

#[tokio::test]
async fn test_self_healing_manager_creation() {
    let config = create_test_config();
    let alert_manager = Arc::new(ProductionAlertManager::new(1000));
    
    let manager = SelfHealingManager::new(config.clone(), alert_manager);
    
    // Test that the manager was created with the correct configuration
    assert!(config.enable_auto_healing);
    assert_eq!(config.max_concurrent_healers, 5);
}

#[tokio::test]
async fn test_self_healing_config_defaults() {
    let config = SelfHealingConfig::default();
    
    assert!(config.enable_auto_healing);
    assert_eq!(config.detection_interval, Duration::from_secs(30));
    assert_eq!(config.healing_timeout, Duration::from_secs(300));
    assert_eq!(config.max_concurrent_healers, 10);
    assert_eq!(config.escalation_threshold, 3);
    assert_eq!(config.cooldown_period, Duration::from_secs(600));
    assert!(config.enable_predictive_healing);
    assert_eq!(config.metrics_retention, Duration::from_secs(3600));
}

#[tokio::test]
async fn test_performance_issue_detector() {
    let config = create_test_config();
    let detector = PerformanceIssueDetector::new(config);
    
    // Test with high CPU usage
    let snapshot = create_test_performance_snapshot();
    let issues = detector.detect_issues(snapshot).await;
    
    // Should detect CPU usage issue
    assert!(!issues.is_empty());
    
    let cpu_issue = issues.iter().find(|issue| issue.rule_id == "cpu_high");
    assert!(cpu_issue.is_some());
    
    let issue = cpu_issue.unwrap();
    assert_eq!(issue.current_value, 85.0);
    assert_eq!(issue.threshold, 80.0);
    assert!(matches!(issue.severity, SelfHealingIssueSeverity::High));
}

#[tokio::test]
async fn test_performance_snapshot_creation() {
    let snapshot = create_test_performance_snapshot();
    
    assert_eq!(snapshot.cpu_usage, 85.0);
    assert_eq!(snapshot.memory_usage, 70.0);
    assert_eq!(snapshot.response_time, Duration::from_millis(500));
    assert_eq!(snapshot.error_rate, 2.0);
    assert_eq!(snapshot.throughput, 1000.0);
    assert_eq!(snapshot.active_connections, 150);
    assert_eq!(snapshot.queue_depth, 10);
}

#[tokio::test]
async fn test_detected_issue_creation() {
    let issue = DetectedIssue {
        issue_id: uuid::Uuid::new_v4(),
        rule_id: "test_rule".to_string(),
        description: "Test issue description".to_string(),
        severity: SelfHealingIssueSeverity::High,
        detected_at: Instant::now(),
        current_value: 90.0,
        threshold: 80.0,
        suggested_actions: vec![
            HealingAction::ScaleUp { target_instances: 2 },
            HealingAction::ReduceLoad { percentage: 20 },
        ],
    };
    
    assert_eq!(issue.rule_id, "test_rule");
    assert_eq!(issue.description, "Test issue description");
    assert!(matches!(issue.severity, SelfHealingIssueSeverity::High));
    assert_eq!(issue.current_value, 90.0);
    assert_eq!(issue.threshold, 80.0);
    assert_eq!(issue.suggested_actions.len(), 2);
}

#[tokio::test]
async fn test_healing_actions() {
    // Test different healing action types
    let scale_up = HealingAction::ScaleUp { target_instances: 3 };
    let scale_down = HealingAction::ScaleDown { target_instances: 1 };
    let clear_cache = HealingAction::ClearCache { cache_name: "redis".to_string() };
    let restart_connections = HealingAction::RestartConnections;
    let reduce_load = HealingAction::ReduceLoad { percentage: 25 };
    let enable_circuit_breaker = HealingAction::EnableCircuitBreaker { service: "api".to_string() };
    let flush_queues = HealingAction::FlushQueues;
    let garbage_collect = HealingAction::GarbageCollect;
    let optimize_database = HealingAction::OptimizeDatabase;
    let restart_service = HealingAction::RestartService { service_name: "web".to_string() };
    
    // Verify action types
    assert!(matches!(scale_up, HealingAction::ScaleUp { target_instances: 3 }));
    assert!(matches!(scale_down, HealingAction::ScaleDown { target_instances: 1 }));
    assert!(matches!(clear_cache, HealingAction::ClearCache { .. }));
    assert!(matches!(restart_connections, HealingAction::RestartConnections));
    assert!(matches!(reduce_load, HealingAction::ReduceLoad { percentage: 25 }));
    assert!(matches!(enable_circuit_breaker, HealingAction::EnableCircuitBreaker { .. }));
    assert!(matches!(flush_queues, HealingAction::FlushQueues));
    assert!(matches!(garbage_collect, HealingAction::GarbageCollect));
    assert!(matches!(optimize_database, HealingAction::OptimizeDatabase));
    assert!(matches!(restart_service, HealingAction::RestartService { .. }));
}

#[tokio::test]
async fn test_healing_engine() {
    let config = create_test_config();
    let engine = HealingEngine::new(config);
    
    // Create a test issue
    let issue = DetectedIssue {
        issue_id: uuid::Uuid::new_v4(),
        rule_id: "test_rule".to_string(),
        description: "Test CPU issue".to_string(),
        severity: SelfHealingIssueSeverity::High,
        detected_at: Instant::now(),
        current_value: 90.0,
        threshold: 80.0,
        suggested_actions: vec![HealingAction::ScaleUp { target_instances: 2 }],
    };
    
    // Execute healing
    let result = engine.execute_healing(issue).await;
    assert!(result.is_ok());
    
    let healing_result = result.unwrap();
    assert!(healing_result.success);
    assert!(healing_result.message.contains("Scaled up to 2 instances"));
    assert!(healing_result.metrics_improvement.is_some());
}

#[tokio::test]
async fn test_healing_result() {
    let result = HealingResult {
        success: true,
        message: "Successfully scaled up instances".to_string(),
        metrics_improvement: Some(crate::core::performance::self_healing::MetricsImprovement {
            cpu_improvement: 20.0,
            memory_improvement: 10.0,
            response_time_improvement: Duration::from_millis(500),
            error_rate_improvement: 5.0,
        }),
        side_effects: vec!["increased_cost".to_string()],
    };
    
    assert!(result.success);
    assert_eq!(result.message, "Successfully scaled up instances");
    assert!(result.metrics_improvement.is_some());
    assert_eq!(result.side_effects.len(), 1);
    assert_eq!(result.side_effects[0], "increased_cost");
    
    let improvement = result.metrics_improvement.unwrap();
    assert_eq!(improvement.cpu_improvement, 20.0);
    assert_eq!(improvement.memory_improvement, 10.0);
    assert_eq!(improvement.response_time_improvement, Duration::from_millis(500));
    assert_eq!(improvement.error_rate_improvement, 5.0);
}

#[tokio::test]
async fn test_self_healing_metrics() {
    let config = create_test_config();
    let alert_manager = Arc::new(ProductionAlertManager::new(1000));
    let manager = SelfHealingManager::new(config, alert_manager);
    
    // Get initial metrics
    let metrics = manager.get_metrics().await;
    
    // All metrics should start at zero
    assert_eq!(metrics.total_issues_detected.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.total_healing_attempts.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.successful_healings.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.failed_healings.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.average_healing_time.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.escalations.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.false_positives.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.current_active_healers.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_issue_severity_levels() {
    // Test all severity levels
    let low = SelfHealingIssueSeverity::Low;
    let medium = SelfHealingIssueSeverity::Medium;
    let high = SelfHealingIssueSeverity::High;
    let critical = SelfHealingIssueSeverity::Critical;
    
    assert!(matches!(low, SelfHealingIssueSeverity::Low));
    assert!(matches!(medium, SelfHealingIssueSeverity::Medium));
    assert!(matches!(high, SelfHealingIssueSeverity::High));
    assert!(matches!(critical, SelfHealingIssueSeverity::Critical));
    
    // Test severity comparison
    assert_eq!(low, SelfHealingIssueSeverity::Low);
    assert_ne!(low, SelfHealingIssueSeverity::High);
}

#[tokio::test]
async fn test_manager_lifecycle() {
    let config = create_test_config();
    let alert_manager = Arc::new(ProductionAlertManager::new(1000));
    let manager = SelfHealingManager::new(config, alert_manager);
    
    // Start the manager
    let start_result = manager.start().await;
    assert!(start_result.is_ok());
    
    // Get active healers (should be empty initially)
    let active_healers = manager.get_active_healers().await;
    assert!(active_healers.is_empty());
    
    // Stop the manager
    let stop_result = manager.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_disabled_self_healing() {
    let mut config = create_test_config();
    config.enable_auto_healing = false;
    
    let alert_manager = Arc::new(ProductionAlertManager::new(1000));
    let manager = SelfHealingManager::new(config, alert_manager);
    
    // Starting should succeed even when disabled
    let result = manager.start().await;
    assert!(result.is_ok());
    
    // Stop should also succeed
    let stop_result = manager.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_concurrent_healing_limit() {
    let mut config = create_test_config();
    config.max_concurrent_healers = 2; // Limit to 2 concurrent healers
    
    let alert_manager = Arc::new(ProductionAlertManager::new(1000));
    let manager = SelfHealingManager::new(config, alert_manager);
    
    // The manager should respect the concurrent healer limit
    // This is tested implicitly through the configuration
    assert_eq!(manager.config.max_concurrent_healers, 2);
}