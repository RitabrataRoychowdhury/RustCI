// Pluggable Observability System Example
// Task 3.4: Demonstrates the flexible observability integration

use std::sync::Arc;
use std::time::Duration;
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

#[cfg(feature = "prometheus")]
use rustci::core::networking::valkyrie::observability::prometheus_adapter::{
    PrometheusAdapterFactory, PrometheusConfig
};

/// Custom metrics adapter example
struct CustomMetricsAdapter {
    name: String,
}

impl CustomMetricsAdapter {
    fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl MetricsAdapter for CustomMetricsAdapter {
    async fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]) {
        println!("[{}] Counter: {} = {} {:?}", self.name, name, value, labels);
    }
    
    async fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        println!("[{}] Gauge: {} = {} {:?}", self.name, name, value, labels);
    }
    
    async fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        println!("[{}] Histogram: {} = {} {:?}", self.name, name, value, labels);
    }
    
    async fn record_timing(&self, name: &str, duration: Duration, labels: &[(&str, &str)]) {
        println!("[{}] Timing: {} = {:?} {:?}", self.name, name, duration, labels);
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: HealthStatus::Healthy,
            last_successful_operation: Some(std::time::SystemTime::now()),
            error_count: 0,
            total_operations: 100,
        }
    }
}

/// Custom health adapter example
struct CustomHealthAdapter {
    name: String,
}

impl CustomHealthAdapter {
    fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl HealthAdapter for CustomHealthAdapter {
    async fn report_health(&self, component: &str, status: HealthStatus) {
        println!("[{}] Health Report: {} = {:?}", self.name, component, status);
    }
    
    async fn check_dependencies(&self) -> Vec<DependencyHealth> {
        vec![
            DependencyHealth {
                name: "database".to_string(),
                status: HealthStatus::Healthy,
                last_check: std::time::SystemTime::now(),
                response_time: Some(Duration::from_millis(50)),
                error_count: 0,
            },
            DependencyHealth {
                name: "redis".to_string(),
                status: HealthStatus::Healthy,
                last_check: std::time::SystemTime::now(),
                response_time: Some(Duration::from_millis(10)),
                error_count: 0,
            }
        ]
    }
    
    async fn record_availability(&self, component: &str, available: bool) {
        println!("[{}] Availability: {} = {}", self.name, component, available);
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: HealthStatus::Healthy,
            last_successful_operation: Some(std::time::SystemTime::now()),
            error_count: 0,
            total_operations: 50,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Pluggable Observability System Example");
    println!("==========================================");
    
    // Example 1: Minimal setup with built-in adapters only
    println!("\n📊 Example 1: Minimal Setup (Built-in adapters only)");
    {
        let config = ObservabilityConfig::default();
        let manager = ObservabilityManager::with_config(config);
        
        // Record some metrics
        manager.record_counter("requests_total", 100, &[("method", "GET"), ("status", "200")]).await;
        manager.record_gauge("memory_usage", 0.75, &[("component", "valkyrie")]).await;
        
        // Log some messages
        manager.log(LogLevel::Info, "System started", &[("version", "1.0.0")]).await;
        manager.log(LogLevel::Warn, "High memory usage detected", &[("usage", "75%")]).await;
        
        // Check health
        let health = manager.get_adapter_health().await;
        println!("Adapter Health: {} adapters", health.len());
        for (name, health) in health {
            println!("  - {}: {:?}", name, health.status);
        }
    }
    
    // Example 2: Custom adapters
    println!("\n🔧 Example 2: Custom Adapters");
    {
        let config = ObservabilityConfig::default();
        let mut manager = ObservabilityManager::with_config(config);
        
        // Register custom adapters
        manager.register_metrics_adapter(Arc::new(CustomMetricsAdapter::new("custom_metrics".to_string())));
        manager.register_health_adapter(Arc::new(CustomHealthAdapter::new("custom_health".to_string())));
        
        // Use the system
        manager.record_counter("custom_counter", 42, &[("source", "example")]).await;
        
        let health = manager.get_adapter_health().await;
        println!("Custom Adapter Health: {} adapters", health.len());
    }
    
    // Example 3: OpenTelemetry integration (if feature enabled)
    #[cfg(feature = "opentelemetry")]
    {
        println!("\n🌐 Example 3: OpenTelemetry Integration");
        
        let config = ObservabilityConfig::default();
        let mut manager = ObservabilityManager::with_config(config);
        
        // Try to create OpenTelemetry adapter
        match OpenTelemetryAdapterFactory::create_default().await {
            Ok(otel_adapter) => {
                manager.register_tracing_adapter(otel_adapter);
                
                // Create some spans
                let spans = manager.start_span("example_operation", None).await;
                println!("Started {} spans", spans.len());
                
                // Add events and attributes
                for span_id in &spans {
                    manager.add_event(*span_id, "processing_started", &[("step", "1")]).await;
                    manager.add_attributes(*span_id, &[("user_id", "12345"), ("operation", "example")]).await;
                }
                
                // Simulate some work
                sleep(Duration::from_millis(100)).await;
                
                // End spans
                for span_id in spans {
                    manager.set_span_status(span_id, SpanStatus::Ok).await;
                    manager.end_span(span_id).await;
                }
                
                println!("OpenTelemetry tracing completed");
            }
            Err(e) => {
                println!("OpenTelemetry not available: {}", e);
            }
        }
    }
    
    #[cfg(not(feature = "opentelemetry"))]
    {
        println!("\n🌐 Example 3: OpenTelemetry Integration (Feature Disabled)");
        println!("OpenTelemetry feature not enabled. Enable with --features opentelemetry");
    }
    
    // Example 4: Prometheus integration (if feature enabled)
    #[cfg(feature = "prometheus")]
    {
        println!("\n📈 Example 4: Prometheus Integration");
        
        let config = ObservabilityConfig::default();
        let mut manager = ObservabilityManager::with_config(config);
        
        // Try to create Prometheus adapter
        match PrometheusAdapterFactory::create_default().await {
            Ok(prometheus_adapter) => {
                manager.register_metrics_adapter(prometheus_adapter);
                
                // Record metrics that will be exposed to Prometheus
                manager.record_counter("http_requests_total", 1000, &[("method", "GET"), ("status", "200")]).await;
                manager.record_gauge("memory_usage_bytes", 1024.0 * 1024.0 * 512.0, &[("component", "valkyrie")]).await;
                manager.record_histogram("request_duration_seconds", 0.125, &[("endpoint", "/api/v1/jobs")]).await;
                
                println!("Prometheus metrics recorded");
            }
            Err(e) => {
                println!("Prometheus not available: {}", e);
            }
        }
    }
    
    #[cfg(not(feature = "prometheus"))]
    {
        println!("\n📈 Example 4: Prometheus Integration (Feature Disabled)");
        println!("Prometheus feature not enabled. Enable with --features prometheus");
    }
    
    // Example 5: Multiple adapters with circuit breaker
    println!("\n⚡ Example 5: Multiple Adapters with Circuit Breaker");
    {
        let config = ObservabilityConfig::default();
        let mut manager = ObservabilityManager::with_config(config);
        
        // Register multiple adapters
        manager.register_metrics_adapter(Arc::new(CustomMetricsAdapter::new("adapter_1".to_string())));
        manager.register_metrics_adapter(Arc::new(CustomMetricsAdapter::new("adapter_2".to_string())));
        manager.register_metrics_adapter(Arc::new(CustomMetricsAdapter::new("adapter_3".to_string())));
        
        // Record metrics - should go to all adapters
        manager.record_counter("multi_adapter_test", 1, &[("test", "circuit_breaker")]).await;
        
        println!("Metrics sent to multiple adapters");
    }
    
    // Example 6: Health monitoring
    println!("\n🏥 Example 6: Health Monitoring");
    {
        let config = ObservabilityConfig::default();
        let mut manager = ObservabilityManager::with_config(config);
        
        // Register health adapter
        manager.register_health_adapter(Arc::new(CustomHealthAdapter::new("health_monitor".to_string())));
        
        // Get comprehensive health status
        let health = manager.get_adapter_health().await;
        println!("System Health Summary:");
        for (component, health) in health {
            println!("  - {}: {:?} (errors: {}, operations: {})", 
                component, health.status, health.error_count, health.total_operations);
        }
    }
    
    // Example 7: Feature flags demonstration
    println!("\n🚩 Example 7: Feature Flags");
    {
        let mut config = ObservabilityConfig::default();
        
        // Disable specific features
        config.metrics_enabled = false;
        config.tracing_enabled = true;
        config.health_enabled = true;
        config.logging_enabled = true;
        
        let manager = ObservabilityManager::with_config(config);
        
        // These operations will be no-ops due to disabled metrics
        manager.record_counter("disabled_metric", 1, &[]).await;
        manager.record_gauge("disabled_gauge", 1.0, &[]).await;
        
        // These will still work
        manager.log(LogLevel::Info, "Logging still works", &[]).await;
        
        println!("Feature flags demonstration completed");
    }
    
    println!("\n✅ All examples completed successfully!");
    println!("\nKey Features Demonstrated:");
    println!("  ✓ Zero-dependency built-in adapters");
    println!("  ✓ Custom adapter implementation");
    println!("  ✓ Feature-gated external integrations");
    println!("  ✓ Circuit breaker pattern");
    println!("  ✓ Multiple adapter support");
    println!("  ✓ Comprehensive health monitoring");
    println!("  ✓ Flexible configuration");
    
    Ok(())
}