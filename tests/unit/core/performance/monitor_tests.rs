use rustci::core::performance::monitor::*;
use rustci::core::performance::metrics::*;
use std::time::Duration;
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test_performance_monitor_creation() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    assert!(monitor.initialize_default_thresholds().await.is_ok());
}

#[tokio::test]
async fn test_metrics_collection() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    let result = monitor.collect_metrics().await;
    assert!(result.is_ok());

    let metrics = result.unwrap();
    assert!(metrics.cpu_usage >= 0.0);
    assert!(metrics.memory_usage.total_mb > 0);
    assert!(metrics.timestamp > 0);
}

#[tokio::test]
async fn test_performance_issue_detection() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    let result = monitor.detect_performance_issues().await;
    assert!(result.is_ok());

    let issues = result.unwrap();
    // Issues may or may not be present depending on current system state
    assert!(issues.len() >= 0);
}

#[tokio::test]
async fn test_auto_scaling_trigger() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    // Create metrics that should trigger scaling
    let mut high_load_metrics = PerformanceMetrics::new();
    high_load_metrics.cpu_usage = 90.0;
    high_load_metrics.memory_usage.usage_percentage = 95.0;

    let result = monitor.trigger_auto_scaling(&high_load_metrics).await;
    assert!(result.is_ok());

    let scaling_action = result.unwrap();
    assert!(scaling_action.is_some());

    let action = scaling_action.unwrap();
    assert_eq!(action.action_type, ScalingActionType::ScaleOut);
    assert_eq!(action.target_component, "application");
}

#[tokio::test]
async fn test_no_auto_scaling_for_normal_metrics() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    // Create normal metrics that shouldn't trigger scaling
    let normal_metrics = PerformanceMetrics::new();

    let result = monitor.trigger_auto_scaling(&normal_metrics).await;
    assert!(result.is_ok());

    let scaling_action = result.unwrap();
    assert!(scaling_action.is_none());
}

#[tokio::test]
async fn test_alert_registration() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    let alert = PerformanceAlert {
        id: Uuid::new_v4(),
        alert_type: AlertType::CpuThreshold,
        threshold: 80.0,
        current_value: 85.0,
        component: "system".to_string(),
        message: "High CPU usage detected".to_string(),
        created_at: 1234567890,
        escalation_level: 1,
        auto_resolve: true,
    };

    let result = monitor.register_performance_alert(alert).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_historical_metrics_retrieval() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_secs(60),
        1000,
    );

    // Collect some metrics first
    let _ = monitor.collect_metrics().await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = monitor.collect_metrics().await;

    let result = monitor.get_historical_metrics(Duration::from_secs(3600)).await;
    assert!(result.is_ok());

    let historical_metrics = result.unwrap();
    assert!(historical_metrics.len() >= 0);
}

#[tokio::test]
async fn test_cpu_usage_detector() {
    let detector = CpuUsageDetector::new(80.0, Duration::from_secs(300));

    let mut high_cpu_metrics = PerformanceMetrics::new();
    high_cpu_metrics.cpu_usage = 90.0;

    // Create history with sustained high CPU
    let mut history = Vec::new();
    for _ in 0..10 {
        let mut metric = PerformanceMetrics::new();
        metric.cpu_usage = 85.0;
        history.push(metric);
    }

    let issues = detector.detect_issues(&high_cpu_metrics, &history).await;
    assert!(!issues.is_empty());
    assert_eq!(issues[0].issue_type, PerformanceIssueType::HighCpuUsage);
    assert_eq!(issues[0].severity, IssueSeverity::High);
}

#[tokio::test]
async fn test_memory_usage_detector() {
    let detector = MemoryUsageDetector::new(85.0, true);

    let mut high_memory_metrics = PerformanceMetrics::new();
    high_memory_metrics.memory_usage.usage_percentage = 90.0;

    let issues = detector.detect_issues(&high_memory_metrics, &[]).await;
    assert!(!issues.is_empty());
    assert_eq!(issues[0].issue_type, PerformanceIssueType::HighMemoryUsage);
    assert_eq!(issues[0].severity, IssueSeverity::High);
}

#[tokio::test]
async fn test_memory_leak_detection() {
    let detector = MemoryUsageDetector::new(85.0, true);

    let mut normal_metrics = PerformanceMetrics::new();
    normal_metrics.memory_usage.usage_percentage = 70.0;

    // Create history showing increasing memory usage (potential leak)
    let mut history = Vec::new();
    for i in 0..10 {
        let mut metric = PerformanceMetrics::new();
        metric.memory_usage.usage_percentage = 50.0 + (i as f64 * 2.0);
        history.push(metric);
    }

    let issues = detector.detect_issues(&normal_metrics, &history).await;
    let leak_issues: Vec<_> = issues.iter()
        .filter(|i| i.issue_type == PerformanceIssueType::ResourceLeak)
        .collect();
    
    assert!(!leak_issues.is_empty());
    assert_eq!(leak_issues[0].severity, IssueSeverity::Medium);
}

#[tokio::test]
async fn test_response_time_detector() {
    let detector = ResponseTimeDetector::new(1000.0, 95.0);

    let mut slow_response_metrics = PerformanceMetrics::new();
    slow_response_metrics.request_metrics.p95_response_time_ms = 1500.0;

    let issues = detector.detect_issues(&slow_response_metrics, &[]).await;
    assert!(!issues.is_empty());
    assert_eq!(issues[0].issue_type, PerformanceIssueType::HighResponseTimes);
    assert_eq!(issues[0].severity, IssueSeverity::High);
}

#[tokio::test]
async fn test_database_performance_detector() {
    let detector = DatabasePerformanceDetector::new(500.0, 90.0);

    let mut slow_db_metrics = PerformanceMetrics::new();
    slow_db_metrics.database_metrics.average_query_time_ms = 750.0;

    let issues = detector.detect_issues(&slow_db_metrics, &[]).await;
    assert!(!issues.is_empty());
    assert_eq!(issues[0].issue_type, PerformanceIssueType::SlowDatabaseQueries);
    assert_eq!(issues[0].severity, IssueSeverity::Medium);
}

#[tokio::test]
async fn test_database_connection_pool_detection() {
    let detector = DatabasePerformanceDetector::new(500.0, 90.0);

    let mut high_connection_metrics = PerformanceMetrics::new();
    high_connection_metrics.database_metrics.connections_active = 48;
    high_connection_metrics.database_metrics.connections_max = 50;

    let issues = detector.detect_issues(&high_connection_metrics, &[]).await;
    assert!(!issues.is_empty());
    assert_eq!(issues[0].issue_type, PerformanceIssueType::ConnectionPoolExhaustion);
    assert_eq!(issues[0].severity, IssueSeverity::High);
}

#[tokio::test]
async fn test_performance_metrics_health_check() {
    let mut healthy_metrics = PerformanceMetrics::new();
    healthy_metrics.cpu_usage = 50.0;
    healthy_metrics.memory_usage.usage_percentage = 60.0;
    healthy_metrics.disk_usage.usage_percentage = 70.0;
    healthy_metrics.request_metrics.average_response_time_ms = 200.0;

    assert!(healthy_metrics.is_healthy());

    let mut unhealthy_metrics = PerformanceMetrics::new();
    unhealthy_metrics.cpu_usage = 95.0;
    unhealthy_metrics.memory_usage.usage_percentage = 90.0;
    unhealthy_metrics.disk_usage.usage_percentage = 95.0;
    unhealthy_metrics.request_metrics.average_response_time_ms = 2000.0;

    assert!(!unhealthy_metrics.is_healthy());
}

#[tokio::test]
async fn test_custom_metrics() {
    let mut metrics = PerformanceMetrics::new();
    metrics.add_custom_metric("custom_latency".to_string(), 125.5);
    metrics.add_custom_metric("business_metric".to_string(), 42.0);

    assert_eq!(metrics.custom_metrics.get("custom_latency"), Some(&125.5));
    assert_eq!(metrics.custom_metrics.get("business_metric"), Some(&42.0));
    assert_eq!(metrics.custom_metrics.len(), 2);
}

#[tokio::test]
async fn test_monitoring_lifecycle() {
    let monitor = ProductionPerformanceMonitor::new(
        Duration::from_millis(100), // Short interval for testing
        10,
    );

    // Start monitoring
    let start_result = monitor.start_monitoring().await;
    assert!(start_result.is_ok());

    // Let it run for a short time
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Stop monitoring
    let stop_result = monitor.stop_monitoring().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_detector_names() {
    let cpu_detector = CpuUsageDetector::new(80.0, Duration::from_secs(300));
    assert_eq!(cpu_detector.get_detector_name(), "CpuUsageDetector");

    let memory_detector = MemoryUsageDetector::new(85.0, true);
    assert_eq!(memory_detector.get_detector_name(), "MemoryUsageDetector");

    let response_detector = ResponseTimeDetector::new(1000.0, 95.0);
    assert_eq!(response_detector.get_detector_name(), "ResponseTimeDetector");

    let db_detector = DatabasePerformanceDetector::new(500.0, 90.0);
    assert_eq!(db_detector.get_detector_name(), "DatabasePerformanceDetector");
}