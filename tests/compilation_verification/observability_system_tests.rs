// Comprehensive tests for the pluggable observability system
// Verifies that all observability components compile and work correctly

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;

// Import the observability system components
use RustAutoDevOps::core::networking::valkyrie::observability::adapter_system::{
    ObservabilityManager, MetricsAdapter, TracingAdapter, HealthAdapter, LoggingAdapter,
    SpanId, SpanStatus, HealthStatus, LogLevel, AdapterHealth, DependencyHealth,
    ObservabilityConfig, NoOpMetricsAdapter, NoOpTracingAdapter, NoOpHealthAdapter, BuiltInJsonLogger
};

/// Test that the observability manager can be created and used
#[tokio::test]
async fn test_observability_manager_creation_and_basic_usage() {
    // Create manager with default configuration
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::with_config(config);
    
    // Test basic operations
    manager.record_counter("test_counter", 1, &[("label", "value")]).await;
    manager.record_gauge("test_gauge", 1.0, &[("label", "value")]).await;
    
    // Test logging
    manager.log(LogLevel::Info, "Test message", &[("field", "value")]).await;
    
    // Test health checks
    let health = manager.get_adapter_health().await;
    assert!(!health.is_empty(), "Should have default adapters");
    
    // Verify all default adapters are healthy
    for (name, adapter_health) in health {
        assert!(matches!(adapter_health.status, HealthStatus::Healthy), 
            "Adapter {} should be healthy", name);
    }
}

/// Test custom adapter registration and usage
#[tokio::test]
async fn test_custom_adapter_registration() {
    struct TestMetricsAdapter {
        name: String,
        recorded_metrics: Arc<tokio::sync::Mutex<Vec<String>>>,
    }
    
    impl TestMetricsAdapter {
        fn new(name: String) -> Self {
            Self {
                name,
                recorded_metrics: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            }
        }
        
        async fn get_recorded_metrics(&self) -> Vec<String> {
            self.recorded_metrics.lock().await.clone()
        }
    }
    
    #[async_trait::async_trait]
    impl MetricsAdapter for TestMetricsAdapter {
        async fn record_counter(&self, name: &str, _value: u64, _labels: &[(&str, &str)]) {
            let mut metrics = self.recorded_metrics.lock().await;
            metrics.push(format!("counter:{}", name));
        }
        
        async fn record_gauge(&self, name: &str, _value: f64, _labels: &[(&str, &str)]) {
            let mut metrics = self.recorded_metrics.lock().await;
            metrics.push(format!("gauge:{}", name));
        }
        
        async fn record_histogram(&self, name: &str, _value: f64, _labels: &[(&str, &str)]) {
            let mut metrics = self.recorded_metrics.lock().await;
            metrics.push(format!("histogram:{}", name));
        }
        
        async fn record_timing(&self, name: &str, _duration: Duration, _labels: &[(&str, &str)]) {
            let mut metrics = self.recorded_metrics.lock().await;
            metrics.push(format!("timing:{}", name));
        }
        
        fn name(&self) -> &str {
            &self.name
        }
        
        async fn health_check(&self) -> AdapterHealth {
            AdapterHealth {
                status: HealthStatus::Healthy,
                last_successful_operation: Some(SystemTime::now()),
                error_count: 0,
                total_operations: 1,
            }
        }
    }
    
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let test_adapter = Arc::new(TestMetricsAdapter::new("test_adapter".to_string()));
    let test_adapter_clone = test_adapter.clone();
    
    manager.register_metrics_adapter(test_adapter);
    
    // Record some metrics
    manager.record_counter("test_counter", 1, &[]).await;
    manager.record_gauge("test_gauge", 1.0, &[]).await;
    manager.record_histogram("test_histogram", 1.0, &[]).await;
    
    // Verify metrics were recorded
    let recorded = test_adapter_clone.get_recorded_metrics().await;
    assert_eq!(recorded.len(), 3);
    assert!(recorded.contains(&"counter:test_counter".to_string()));
    assert!(recorded.contains(&"gauge:test_gauge".to_string()));
    assert!(recorded.contains(&"histogram:test_histogram".to_string()));
}

/// Test that feature flags work correctly
#[tokio::test]
async fn test_feature_flags() {
    let mut config = ObservabilityConfig::default();
    config.metrics_enabled = false;
    config.logging_enabled = true;
    
    let manager = ObservabilityManager::with_config(config);
    
    // Metrics should be disabled, so this should be a no-op
    manager.record_counter("disabled_counter", 1, &[]).await;
    
    // Logging should still work
    manager.log(LogLevel::Info, "Test message", &[]).await;
    
    // Health checks should still work
    let health = manager.get_adapter_health().await;
    assert!(!health.is_empty());
}

/// Test built-in JSON logger
#[tokio::test]
async fn test_builtin_json_logger() {
    let logger = BuiltInJsonLogger::new();
    
    // Test different log levels
    logger.log(LogLevel::Trace, "Trace message", &[("key", "value")]).await;
    logger.log(LogLevel::Debug, "Debug message", &[("key", "value")]).await;
    logger.log(LogLevel::Info, "Info message", &[("key", "value")]).await;
    logger.log(LogLevel::Warn, "Warn message", &[("key", "value")]).await;
    logger.log(LogLevel::Error, "Error message", &[("key", "value")]).await;
    
    // Test correlation logging
    logger.log_with_correlation(LogLevel::Info, "Correlated message", "test-correlation-id", &[]).await;
    
    // Test health check
    let health = logger.health_check().await;
    assert!(matches!(health.status, HealthStatus::Healthy));
}

/// Test no-op adapters
#[tokio::test]
async fn test_noop_adapters() {
    let metrics_adapter = NoOpMetricsAdapter;
    let tracing_adapter = NoOpTracingAdapter;
    let health_adapter = NoOpHealthAdapter;
    
    // All operations should work without errors
    metrics_adapter.record_counter("test", 1, &[]).await;
    metrics_adapter.record_gauge("test", 1.0, &[]).await;
    metrics_adapter.record_histogram("test", 1.0, &[]).await;
    metrics_adapter.record_timing("test", Duration::from_millis(100), &[]).await;
    
    let span_id = tracing_adapter.start_span("test_span", None).await;
    tracing_adapter.add_event(span_id, "test_event", &[("key", "value")]).await;
    tracing_adapter.add_attributes(span_id, &[("attr", "value")]).await;
    tracing_adapter.set_span_status(span_id, SpanStatus::Ok).await;
    tracing_adapter.end_span(span_id).await;
    
    health_adapter.report_health("test_component", HealthStatus::Healthy).await;
    let deps = health_adapter.check_dependencies().await;
    assert!(deps.is_empty());
    health_adapter.record_availability("test_component", true).await;
    
    // All health checks should be healthy
    let metrics_health = metrics_adapter.health_check().await;
    let tracing_health = tracing_adapter.health_check().await;
    let health_health = health_adapter.health_check().await;
    
    assert!(matches!(metrics_health.status, HealthStatus::Healthy));
    assert!(matches!(tracing_health.status, HealthStatus::Healthy));
    assert!(matches!(health_health.status, HealthStatus::Healthy));
}

/// Test concurrent operations
#[tokio::test]
async fn test_concurrent_operations() {
    let config = ObservabilityConfig::default();
    let manager = Arc::new(ObservabilityManager::with_config(config));
    
    let mut handles = Vec::new();
    
    // Spawn multiple concurrent tasks
    for i in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                manager_clone.record_counter(&format!("concurrent_counter_{}", i), j, &[]).await;
                manager_clone.log(LogLevel::Info, &format!("Concurrent log {} {}", i, j), &[]).await;
            }
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // System should still be healthy
    let health = manager.get_adapter_health().await;
    for (name, adapter_health) in health {
        assert!(matches!(adapter_health.status, HealthStatus::Healthy), 
            "Adapter {} should be healthy after concurrent operations", name);
    }
}

/// Test span lifecycle
#[tokio::test]
async fn test_span_lifecycle() {
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::with_config(config);
    
    // Start parent span
    let parent_spans = manager.start_span("parent_span", None).await;
    assert!(!parent_spans.is_empty());
    
    let parent_span_id = parent_spans[0];
    
    // Start child span
    let child_spans = manager.start_span("child_span", Some(parent_span_id)).await;
    assert!(!child_spans.is_empty());
    
    let child_span_id = child_spans[0];
    
    // Add events and attributes
    manager.add_event(child_span_id, "processing_started", &[("step", "1")]).await;
    manager.add_attributes(child_span_id, &[("user_id", "12345")]).await;
    
    // Set status and end spans
    manager.set_span_status(child_span_id, SpanStatus::Ok).await;
    manager.end_span(child_span_id).await;
    
    manager.set_span_status(parent_span_id, SpanStatus::Ok).await;
    manager.end_span(parent_span_id).await;
}

/// Test configuration validation
#[tokio::test]
async fn test_configuration_validation() {
    // Test default configuration
    let default_config = ObservabilityConfig::default();
    assert!(default_config.metrics_enabled);
    assert!(default_config.tracing_enabled);
    assert!(default_config.health_enabled);
    assert!(default_config.logging_enabled);
    
    // Test custom configuration
    let mut custom_config = ObservabilityConfig::default();
    custom_config.metrics_enabled = false;
    custom_config.tracing_enabled = true;
    custom_config.health_enabled = false;
    custom_config.logging_enabled = true;
    
    let manager = ObservabilityManager::with_config(custom_config);
    
    // Operations should respect configuration flags
    manager.record_counter("config_test", 1, &[]).await; // Should be no-op
    manager.log(LogLevel::Info, "Config test", &[]).await; // Should work
    
    let health = manager.get_adapter_health().await;
    assert!(!health.is_empty());
}