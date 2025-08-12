// Valkyrie Protocol Observability Tests

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::networking::valkyrie::observability::{
    ObservabilityManager, ObservabilityConfig, MetricValue, LogLevel, HealthStatus,
    HealthCheck, HealthCheckType, HealthCheckConfig, CorrelationStatus,
};

#[tokio::test]
async fn test_observability_manager_lifecycle() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);

    // Start the observability manager
    assert!(manager.start().await.is_ok());

    // Check initial status
    let status = manager.status().await;
    assert!(status.metrics_enabled);
    assert!(status.logging_enabled);
    assert!(status.health_enabled);
    assert!(status.dashboard_enabled);

    // Stop the observability manager
    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_metrics_collection() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    let metrics = manager.metrics();

    // Test counter metric
    let labels = HashMap::new();
    assert!(metrics.increment_counter("test_counter", labels.clone()).await.is_ok());
    assert!(metrics.increment_counter("test_counter", labels.clone()).await.is_ok());

    // Test gauge metric
    assert!(metrics.set_gauge("test_gauge", 42.5, labels.clone()).await.is_ok());

    // Test histogram metric
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    assert!(metrics.record_histogram("test_histogram", values, labels.clone()).await.is_ok());

    // Test summary metric
    assert!(metrics.record_summary("test_summary", 100.0, 10, labels).await.is_ok());

    // Verify metrics were recorded
    let summary = metrics.summary().await;
    assert!(summary.total_metrics > 0);
    assert!(summary.total_data_points > 0);

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_structured_logging() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    let logger = manager.logger();

    // Test different log levels
    assert!(logger.info("Test info message").await.is_ok());
    assert!(logger.warn("Test warning message").await.is_ok());
    assert!(logger.error("Test error message").await.is_ok());

    // Test logging with context
    let mut context = HashMap::new();
    context.insert("user_id".to_string(), serde_json::Value::String("user123".to_string()));
    context.insert("operation".to_string(), serde_json::Value::String("test_op".to_string()));
    
    assert!(logger.log(LogLevel::Info, "Test with context", context).await.is_ok());

    // Verify logs were recorded
    let stats = logger.statistics().await;
    assert!(stats.total_entries > 0);
    assert!(stats.level_counts.contains_key(&LogLevel::Info));

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_health_monitoring() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    let health = manager.health();

    // Wait a moment for default health checks to run
    sleep(Duration::from_millis(100)).await;

    // Check overall health status
    let status = health.overall_status().await;
    assert!(matches!(status, HealthStatus::Healthy | HealthStatus::Unknown));

    // Get health summary
    let summary = health.summary().await;
    assert!(summary.total_checks > 0);

    // Register a custom health check
    let custom_check = HealthCheck {
        id: "custom_test_check".to_string(),
        name: "Custom Test Check".to_string(),
        description: "A test health check".to_string(),
        check_type: HealthCheckType::Memory { max_usage_percent: 90.0 },
        config: HealthCheckConfig {
            parameters: HashMap::new(),
            environment: HashMap::new(),
            working_directory: None,
        },
        interval_seconds: 10,
        timeout_seconds: 5,
        failure_threshold: 3,
        success_threshold: 2,
        enabled: true,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    assert!(health.register_check(custom_check).await.is_ok());

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_correlation_tracking() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    let correlation = manager.correlation();

    // Start a correlation
    let correlation_id = correlation.start_correlation("test_operation", None).await.unwrap();

    // Add metadata
    assert!(correlation.add_metadata(
        correlation_id,
        "test_key",
        serde_json::Value::String("test_value".to_string())
    ).await.is_ok());

    // Start a span
    let span_id = correlation.start_span(
        correlation_id,
        "test_span",
        "test_component",
        None
    ).await.unwrap();

    // Add span log
    let mut fields = HashMap::new();
    fields.insert("field1".to_string(), serde_json::Value::String("value1".to_string()));
    assert!(correlation.add_span_log(
        correlation_id,
        span_id,
        "INFO",
        "Test span log",
        fields
    ).await.is_ok());

    // End the span
    assert!(correlation.end_span(
        correlation_id,
        span_id,
        rustci::core::networking::valkyrie::observability::correlation::SpanStatus::Ok
    ).await.is_ok());

    // End the correlation
    assert!(correlation.end_correlation(correlation_id, CorrelationStatus::Completed).await.is_ok());

    // Check statistics
    let stats = correlation.get_statistics().await;
    assert!(stats.total_created > 0);
    assert!(stats.completed_count > 0);

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_dashboard_functionality() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    let dashboard = manager.dashboard();

    // Get dashboard data
    let data = dashboard.get_data().await;
    assert!(!data.metadata.title.is_empty());
    assert!(data.last_updated > 0);

    // Generate HTML dashboard
    let html = dashboard.generate_html().await;
    assert!(html.contains("Valkyrie Protocol Dashboard"));
    assert!(html.contains("System Overview"));

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_observability_with_correlation() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    // Create a correlation ID
    let correlation_id = manager.new_correlation_id();

    // Record metric with correlation
    let mut labels = HashMap::new();
    labels.insert("operation".to_string(), "test".to_string());
    
    assert!(manager.record_metric_with_correlation(
        correlation_id,
        "test_metric",
        MetricValue::Counter(1),
        labels
    ).await.is_ok());

    // Log with correlation
    let mut context = HashMap::new();
    context.insert("test_field".to_string(), serde_json::Value::String("test_value".to_string()));
    
    assert!(manager.log_with_correlation(
        correlation_id,
        LogLevel::Info,
        "Test message with correlation",
        context
    ).await.is_ok());

    assert!(manager.stop().await.is_ok());
}

#[tokio::test]
async fn test_observability_configuration() {
    // Test with custom configuration
    let config = ObservabilityConfig {
        metrics_enabled: true,
        logging_enabled: false,
        health_enabled: true,
        dashboard_enabled: false,
        metrics_retention_seconds: 1800,
        log_retention_seconds: 43200,
        health_check_interval_seconds: 60,
        dashboard_refresh_seconds: 10,
    };

    let mut manager = ObservabilityManager::new(config);
    assert!(manager.start().await.is_ok());

    let status = manager.status().await;
    assert!(status.metrics_enabled);
    assert!(!status.logging_enabled);
    assert!(status.health_enabled);
    assert!(!status.dashboard_enabled);

    assert!(manager.stop().await.is_ok());
}