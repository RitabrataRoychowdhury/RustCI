// Comprehensive tests for the pluggable observability system
// Task 3.4: Pluggable Observability System

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use tokio::time::sleep;

use rustci::core::networking::valkyrie::observability::adapter_system::{
    ObservabilityManager, MetricsAdapter, TracingAdapter, HealthAdapter, LoggingAdapter,
    SpanId, SpanStatus, HealthStatus, LogLevel, AdapterHealth, DependencyHealth,
    ObservabilityConfig, NoOpMetricsAdapter, NoOpTracingAdapter, NoOpHealthAdapter, BuiltInJsonLogger
};

#[cfg(feature = "opentelemetry")]
use rustci::core::networking::valkyrie::observability::opentelemetry_adapter::{
    OpenTelemetryAdapterFactory, OpenTelemetryConfig
};

/// Mock metrics adapter for testing
struct MockMetricsAdapter {
    name: String,
    recorded_metrics: Arc<tokio::sync::Mutex<Vec<(String, String)>>>, // (name, type)
}

impl MockMetricsAdapter {
    fn new(name: String) -> Self {
        Self {
            name,
            recorded_metrics: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
    
    async fn get_recorded_metrics(&self) -> Vec<(String, String)> {
        self.recorded_metrics.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl MetricsAdapter for MockMetricsAdapter {
    async fn record_counter(&self, name: &str, _value: u64, _labels: &[(&str, &str)]) {
        let mut metrics = self.recorded_metrics.lock().await;
        metrics.push((name.to_string(), "counter".to_string()));
    }
    
    async fn record_gauge(&self, name: &str, _value: f64, _labels: &[(&str, &str)]) {
        let mut metrics = self.recorded_metrics.lock().await;
        metrics.push((name.to_string(), "gauge".to_string()));
    }
    
    async fn record_histogram(&self, name: &str, _value: f64, _labels: &[(&str, &str)]) {
        let mut metrics = self.recorded_metrics.lock().await;
        metrics.push((name.to_string(), "histogram".to_string()));
    }
    
    async fn record_timing(&self, name: &str, _duration: Duration, _labels: &[(&str, &str)]) {
        let mut metrics = self.recorded_metrics.lock().await;
        metrics.push((name.to_string(), "timing".to_string()));
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

/// Mock tracing adapter for testing
struct MockTracingAdapter {
    name: String,
    spans: Arc<tokio::sync::Mutex<HashMap<SpanId, String>>>,
}

impl MockTracingAdapter {
    fn new(name: String) -> Self {
        Self {
            name,
            spans: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
    
    async fn get_active_spans(&self) -> HashMap<SpanId, String> {
        self.spans.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl TracingAdapter for MockTracingAdapter {
    async fn start_span(&self, name: &str, _parent: Option<SpanId>) -> SpanId {
        let span_id = SpanId::new();
        let mut spans = self.spans.lock().await;
        spans.insert(span_id, name.to_string());
        span_id
    }
    
    async fn end_span(&self, span_id: SpanId) {
        let mut spans = self.spans.lock().await;
        spans.remove(&span_id);
    }
    
    async fn add_event(&self, _span_id: SpanId, _name: &str, _attributes: &[(&str, &str)]) {
        // Mock implementation
    }
    
    async fn add_attributes(&self, _span_id: SpanId, _attributes: &[(&str, &str)]) {
        // Mock implementation
    }
    
    async fn set_span_status(&self, _span_id: SpanId, _status: SpanStatus) {
        // Mock implementation
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

#[tokio::test]
async fn test_observability_manager_creation() {
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::with_config(config);
    
    // Should have default adapters registered
    let health = manager.get_adapter_health().await;
    assert!(!health.is_empty(), "Should have default adapters");
    
    // Check that built-in adapters are present
    assert!(health.contains_key("metrics_noop"), "Should have no-op metrics adapter");
    assert!(health.contains_key("tracing_noop"), "Should have no-op tracing adapter");
    assert!(health.contains_key("health_noop"), "Should have no-op health adapter");
    assert!(health.contains_key("logging_builtin_json"), "Should have built-in JSON logger");
}

#[tokio::test]
async fn test_custom_metrics_adapter_registration() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let mock_adapter = Arc::new(MockMetricsAdapter::new("test_metrics".to_string()));
    let mock_adapter_clone = mock_adapter.clone();
    
    manager.register_metrics_adapter(mock_adapter);
    
    // Record some metrics
    manager.record_counter("test_counter", 1, &[("label", "value")]).await;
    manager.record_gauge("test_gauge", 1.0, &[("label", "value")]).await;
    manager.record_histogram("test_histogram", 1.0, &[("label", "value")]).await;
    
    // Verify metrics were recorded
    let recorded = mock_adapter_clone.get_recorded_metrics().await;
    assert_eq!(recorded.len(), 3, "Should have recorded 3 metrics");
    assert!(recorded.contains(&("test_counter".to_string(), "counter".to_string())));
    assert!(recorded.contains(&("test_gauge".to_string(), "gauge".to_string())));
    assert!(recorded.contains(&("test_histogram".to_string(), "histogram".to_string())));
}

#[tokio::test]
async fn test_custom_tracing_adapter_registration() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let mock_adapter = Arc::new(MockTracingAdapter::new("test_tracing".to_string()));
    let mock_adapter_clone = mock_adapter.clone();
    
    manager.register_tracing_adapter(mock_adapter);
    
    // Start some spans
    let spans = manager.start_span("test_span", None).await;
    assert!(!spans.is_empty(), "Should have started spans");
    
    // Verify spans were created
    let active_spans = mock_adapter_clone.get_active_spans().await;
    assert!(!active_spans.is_empty(), "Should have active spans");
    
    // End spans
    for span_id in spans {
        manager.end_span(span_id).await;
    }
    
    // Verify spans were ended
    let active_spans = mock_adapter_clone.get_active_spans().await;
    assert!(active_spans.is_empty(), "Should have no active spans after ending");
}

#[tokio::test]
async fn test_multiple_adapters() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let adapter1 = Arc::new(MockMetricsAdapter::new("adapter1".to_string()));
    let adapter2 = Arc::new(MockMetricsAdapter::new("adapter2".to_string()));
    let adapter1_clone = adapter1.clone();
    let adapter2_clone = adapter2.clone();
    
    manager.register_metrics_adapter(adapter1);
    manager.register_metrics_adapter(adapter2);
    
    // Record a metric - should go to both adapters
    manager.record_counter("multi_test", 1, &[]).await;
    
    // Verify both adapters received the metric
    let recorded1 = adapter1_clone.get_recorded_metrics().await;
    let recorded2 = adapter2_clone.get_recorded_metrics().await;
    
    assert!(recorded1.contains(&("multi_test".to_string(), "counter".to_string())));
    assert!(recorded2.contains(&("multi_test".to_string(), "counter".to_string())));
}

#[tokio::test]
async fn test_feature_flags() {
    let mut config = ObservabilityConfig::default();
    config.metrics_enabled = false;
    config.tracing_enabled = false;
    config.logging_enabled = true;
    
    let mut manager = ObservabilityManager::with_config(config);
    
    let mock_adapter = Arc::new(MockMetricsAdapter::new("disabled_test".to_string()));
    let mock_adapter_clone = mock_adapter.clone();
    
    manager.register_metrics_adapter(mock_adapter);
    
    // Try to record metrics - should be no-op due to disabled flag
    manager.record_counter("disabled_counter", 1, &[]).await;
    
    // Verify no metrics were recorded
    let recorded = mock_adapter_clone.get_recorded_metrics().await;
    assert!(recorded.is_empty(), "Should not record metrics when disabled");
    
    // Logging should still work
    manager.log(LogLevel::Info, "Test message", &[]).await;
}

#[tokio::test]
async fn test_built_in_json_logger() {
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::with_config(config);
    
    // Should work without any external dependencies
    manager.log(LogLevel::Info, "Test info message", &[("key", "value")]).await;
    manager.log(LogLevel::Warn, "Test warning message", &[("component", "test")]).await;
    manager.log(LogLevel::Error, "Test error message", &[("error_code", "500")]).await;
    
    // Get health status
    let health = manager.get_adapter_health().await;
    let logger_health = health.get("logging_builtin_json");
    assert!(logger_health.is_some(), "Should have built-in logger health");
    assert!(matches!(logger_health.unwrap().status, HealthStatus::Healthy));
}

#[tokio::test]
async fn test_no_op_adapters() {
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::new();
    
    // All operations should work without errors (no-op implementations)
    manager.record_counter("noop_counter", 1, &[]).await;
    manager.record_gauge("noop_gauge", 1.0, &[]).await;
    manager.record_histogram("noop_histogram", 1.0, &[]).await;
    
    let spans = manager.start_span("noop_span", None).await;
    for span_id in spans {
        manager.add_event(span_id, "test_event", &[("key", "value")]).await;
        manager.add_attributes(span_id, &[("attr", "value")]).await;
        manager.set_span_status(span_id, SpanStatus::Ok).await;
        manager.end_span(span_id).await;
    }
    
    manager.log(LogLevel::Info, "No-op test", &[]).await;
    
    // Health checks should all be healthy
    let health = manager.get_adapter_health().await;
    for (name, adapter_health) in health {
        assert!(matches!(adapter_health.status, HealthStatus::Healthy), 
            "Adapter {} should be healthy", name);
    }
}

#[tokio::test]
async fn test_adapter_health_monitoring() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let mock_adapter = Arc::new(MockMetricsAdapter::new("health_test".to_string()));
    manager.register_metrics_adapter(mock_adapter);
    
    // Get initial health
    let health = manager.get_adapter_health().await;
    assert!(!health.is_empty(), "Should have adapters");
    
    // All adapters should be healthy initially
    for (name, adapter_health) in health {
        assert!(matches!(adapter_health.status, HealthStatus::Healthy), 
            "Adapter {} should be healthy", name);
        assert!(adapter_health.error_count == 0, "Should have no errors initially");
    }
}

#[cfg(feature = "opentelemetry")]
#[tokio::test]
async fn test_opentelemetry_adapter_integration() {
    // Test OpenTelemetry adapter creation and integration
    match OpenTelemetryAdapterFactory::create_default().await {
        Ok(otel_adapter) => {
            let config = ObservabilityConfig::default();
            let mut manager = ObservabilityManager::with_config(config);
            
            manager.register_tracing_adapter(otel_adapter);
            
            // Test tracing operations
            let spans = manager.start_span("otel_test_span", None).await;
            assert!(!spans.is_empty(), "Should create spans");
            
            for span_id in &spans {
                manager.add_event(*span_id, "test_event", &[("key", "value")]).await;
                manager.add_attributes(*span_id, &[("service", "test")]).await;
                manager.set_span_status(*span_id, SpanStatus::Ok).await;
            }
            
            for span_id in spans {
                manager.end_span(span_id).await;
            }
            
            // Check adapter health
            let health = manager.get_adapter_health().await;
            let otel_health = health.get("tracing_opentelemetry");
            if let Some(health) = otel_health {
                assert!(matches!(health.status, HealthStatus::Healthy));
            }
        }
        Err(_) => {
            // OpenTelemetry not available in test environment, which is expected
            println!("OpenTelemetry adapter not available in test environment");
        }
    }
}

#[cfg(not(feature = "opentelemetry"))]
#[tokio::test]
async fn test_opentelemetry_feature_disabled() {
    use rustci::core::networking::valkyrie::observability::opentelemetry_adapter::OpenTelemetryAdapterFactory;
    
    // Should return error when feature is disabled
    let result = OpenTelemetryAdapterFactory::create_default().await;
    assert!(result.is_err(), "Should fail when OpenTelemetry feature is disabled");
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("OpenTelemetry feature not enabled"), 
        "Error should mention feature not enabled");
}

#[tokio::test]
async fn test_span_lifecycle() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let mock_adapter = Arc::new(MockTracingAdapter::new("span_test".to_string()));
    let mock_adapter_clone = mock_adapter.clone();
    
    manager.register_tracing_adapter(mock_adapter);
    
    // Start parent span
    let parent_spans = manager.start_span("parent_span", None).await;
    assert!(!parent_spans.is_empty(), "Should create parent spans");
    
    let parent_span_id = parent_spans[0];
    
    // Start child span
    let child_spans = manager.start_span("child_span", Some(parent_span_id)).await;
    assert!(!child_spans.is_empty(), "Should create child spans");
    
    let child_span_id = child_spans[0];
    
    // Verify both spans are active
    let active_spans = mock_adapter_clone.get_active_spans().await;
    assert!(active_spans.contains_key(&parent_span_id), "Parent span should be active");
    assert!(active_spans.contains_key(&child_span_id), "Child span should be active");
    
    // End child span first
    manager.end_span(child_span_id).await;
    
    let active_spans = mock_adapter_clone.get_active_spans().await;
    assert!(active_spans.contains_key(&parent_span_id), "Parent span should still be active");
    assert!(!active_spans.contains_key(&child_span_id), "Child span should be ended");
    
    // End parent span
    manager.end_span(parent_span_id).await;
    
    let active_spans = mock_adapter_clone.get_active_spans().await;
    assert!(!active_spans.contains_key(&parent_span_id), "Parent span should be ended");
}

#[tokio::test]
async fn test_concurrent_operations() {
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::with_config(config);
    
    let mock_adapter = Arc::new(MockMetricsAdapter::new("concurrent_test".to_string()));
    let mock_adapter_clone = mock_adapter.clone();
    
    manager.register_metrics_adapter(mock_adapter);
    
    let manager = Arc::new(manager);
    
    // Spawn multiple concurrent tasks
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                manager_clone.record_counter(&format!("concurrent_counter_{}", i), j, &[]).await;
                manager_clone.record_gauge(&format!("concurrent_gauge_{}", i), j as f64, &[]).await;
            }
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all metrics were recorded
    let recorded = mock_adapter_clone.get_recorded_metrics().await;
    assert_eq!(recorded.len(), 200, "Should have recorded 200 metrics (10 tasks * 10 iterations * 2 metric types)");
}

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
}