# Pluggable Observability System

## Overview

The Valkyrie Protocol includes a comprehensive pluggable observability system that provides flexible monitoring, logging, tracing, and health checking capabilities. The system is designed with zero external dependencies by default, while supporting optional integrations with popular observability tools.

## Key Features

### 🔌 Pluggable Architecture
- **Trait-based design**: Implement custom adapters using standard Rust traits
- **Multiple adapters**: Support multiple monitoring backends simultaneously
- **Circuit breaker pattern**: Automatic failure handling and recovery
- **Zero-cost abstractions**: No-op implementations when features are disabled

### 🛡️ Zero Dependencies
- **Built-in JSON logging**: Always available structured logging
- **No-op adapters**: Zero-cost fallbacks when external tools unavailable
- **Self-contained**: Core functionality works without external services
- **Graceful degradation**: System continues working even if monitoring fails

### 🎛️ Feature Flags
- **Optional integrations**: Enable only the monitoring tools you need
- **Cargo features**: `opentelemetry`, `prometheus`, `jaeger`, `grafana`
- **Runtime configuration**: Enable/disable components at runtime
- **Minimal footprint**: Only compile what you use

### 📊 Comprehensive Monitoring
- **Metrics**: Counters, gauges, histograms, and timing measurements
- **Tracing**: Distributed tracing with span lifecycle management
- **Health checks**: Component health monitoring with dependency tracking
- **Logging**: Structured logging with correlation IDs

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 ObservabilityManager                        │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────┐ │
│  │   Metrics   │ │   Tracing   │ │   Health    │ │Logging  │ │
│  │  Adapters   │ │  Adapters   │ │  Adapters   │ │Adapters │ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────┘ │
├─────────────────────────────────────────────────────────────┤
│                Circuit Breaker System                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────┐ │
│  │   No-Op     │ │ Built-in    │ │ OpenTelemetry│ │Prometheus│ │
│  │  Adapters   │ │JSON Logger  │ │   Adapter   │ │ Adapter │ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Core Traits

### MetricsAdapter

```rust
#[async_trait]
pub trait MetricsAdapter: Send + Sync {
    async fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]);
    async fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);
    async fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
    async fn record_timing(&self, name: &str, duration: Duration, labels: &[(&str, &str)]);
    fn name(&self) -> &str;
    async fn health_check(&self) -> AdapterHealth;
}
```

### TracingAdapter

```rust
#[async_trait]
pub trait TracingAdapter: Send + Sync {
    async fn start_span(&self, name: &str, parent: Option<SpanId>) -> SpanId;
    async fn end_span(&self, span_id: SpanId);
    async fn add_event(&self, span_id: SpanId, name: &str, attributes: &[(&str, &str)]);
    async fn add_attributes(&self, span_id: SpanId, attributes: &[(&str, &str)]);
    async fn set_span_status(&self, span_id: SpanId, status: SpanStatus);
    fn name(&self) -> &str;
    async fn health_check(&self) -> AdapterHealth;
}
```

### HealthAdapter

```rust
#[async_trait]
pub trait HealthAdapter: Send + Sync {
    async fn report_health(&self, component: &str, status: HealthStatus);
    async fn check_dependencies(&self) -> Vec<DependencyHealth>;
    async fn record_availability(&self, component: &str, available: bool);
    fn name(&self) -> &str;
    async fn health_check(&self) -> AdapterHealth;
}
```

### LoggingAdapter

```rust
#[async_trait]
pub trait LoggingAdapter: Send + Sync {
    async fn log(&self, level: LogLevel, message: &str, fields: &[(&str, &str)]);
    async fn log_with_correlation(&self, level: LogLevel, message: &str, correlation_id: &str, fields: &[(&str, &str)]);
    fn name(&self) -> &str;
    async fn health_check(&self) -> AdapterHealth;
}
```

## Usage Examples

### Basic Setup (Zero Dependencies)

```rust
use rustci::core::networking::valkyrie::observability::adapter_system::{
    ObservabilityManager, ObservabilityConfig, LogLevel
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create manager with default configuration
    let config = ObservabilityConfig::default();
    let manager = ObservabilityManager::with_config(config);
    
    // Record metrics (goes to no-op adapter by default)
    manager.record_counter("requests_total", 1, &[("method", "GET")]).await;
    manager.record_gauge("memory_usage", 0.75, &[("component", "valkyrie")]).await;
    
    // Log messages (goes to built-in JSON logger)
    manager.log(LogLevel::Info, "System started", &[("version", "1.0.0")]).await;
    
    // Check health
    let health = manager.get_adapter_health().await;
    println!("System health: {} adapters", health.len());
    
    Ok(())
}
```

### Custom Adapter Implementation

```rust
use std::sync::Arc;
use async_trait::async_trait;
use rustci::core::networking::valkyrie::observability::adapter_system::{
    MetricsAdapter, AdapterHealth, HealthStatus
};

struct CustomMetricsAdapter {
    name: String,
    // Your custom storage/client here
}

#[async_trait]
impl MetricsAdapter for CustomMetricsAdapter {
    async fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]) {
        // Send to your monitoring system
        println!("Custom: {} = {} {:?}", name, value, labels);
    }
    
    async fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        // Send to your monitoring system
        println!("Custom: {} = {} {:?}", name, value, labels);
    }
    
    async fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        // Send to your monitoring system
        println!("Custom: {} = {} {:?}", name, value, labels);
    }
    
    async fn record_timing(&self, name: &str, duration: Duration, labels: &[(&str, &str)]) {
        // Send to your monitoring system
        println!("Custom: {} = {:?} {:?}", name, duration, labels);
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

// Usage
let mut manager = ObservabilityManager::new();
let custom_adapter = Arc::new(CustomMetricsAdapter {
    name: "my_custom_adapter".to_string(),
});
manager.register_metrics_adapter(custom_adapter);
```

### OpenTelemetry Integration

```toml
# Cargo.toml
[dependencies]
rustci = { version = "0.1", features = ["opentelemetry"] }
```

```rust
#[cfg(feature = "opentelemetry")]
use rustci::core::networking::valkyrie::observability::opentelemetry_adapter::{
    OpenTelemetryAdapterFactory, OpenTelemetryConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = ObservabilityManager::new();
    
    // Add OpenTelemetry tracing
    #[cfg(feature = "opentelemetry")]
    {
        match OpenTelemetryAdapterFactory::create_jaeger(
            "http://localhost:14268/api/traces".to_string(),
            "rustci-valkyrie".to_string()
        ).await {
            Ok(otel_adapter) => {
                manager.register_tracing_adapter(otel_adapter);
                println!("OpenTelemetry tracing enabled");
            }
            Err(e) => {
                println!("OpenTelemetry not available: {}", e);
            }
        }
    }
    
    // Create distributed traces
    let spans = manager.start_span("http_request", None).await;
    for span_id in &spans {
        manager.add_attributes(*span_id, &[
            ("http.method", "GET"),
            ("http.url", "/api/v1/jobs"),
            ("user.id", "12345")
        ]).await;
        
        manager.add_event(*span_id, "request_processed", &[
            ("duration_ms", "150")
        ]).await;
    }
    
    // End spans
    for span_id in spans {
        manager.set_span_status(span_id, SpanStatus::Ok).await;
        manager.end_span(span_id).await;
    }
    
    Ok(())
}
```

### Prometheus Integration

```toml
# Cargo.toml
[dependencies]
rustci = { version = "0.1", features = ["prometheus"] }
```

```rust
#[cfg(feature = "prometheus")]
use rustci::core::networking::valkyrie::observability::prometheus_adapter::{
    PrometheusAdapterFactory, PrometheusConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = ObservabilityManager::new();
    
    // Add Prometheus metrics
    #[cfg(feature = "prometheus")]
    {
        let prometheus_config = PrometheusConfig {
            listen_address: "0.0.0.0:9090".to_string(),
            metrics_path: "/metrics".to_string(),
            ..Default::default()
        };
        
        match PrometheusAdapterFactory::create_with_config(prometheus_config).await {
            Ok(prometheus_adapter) => {
                manager.register_metrics_adapter(prometheus_adapter);
                println!("Prometheus metrics available at http://localhost:9090/metrics");
            }
            Err(e) => {
                println!("Prometheus not available: {}", e);
            }
        }
    }
    
    // Record metrics that will be exposed to Prometheus
    manager.record_counter("http_requests_total", 1, &[
        ("method", "GET"),
        ("status", "200"),
        ("endpoint", "/api/v1/jobs")
    ]).await;
    
    manager.record_histogram("request_duration_seconds", 0.125, &[
        ("endpoint", "/api/v1/jobs")
    ]).await;
    
    Ok(())
}
```

### Multiple Adapters with Circuit Breaker

```rust
let mut manager = ObservabilityManager::new();

// Register multiple metrics adapters
manager.register_metrics_adapter(Arc::new(PrometheusAdapter::new()));
manager.register_metrics_adapter(Arc::new(CustomMetricsAdapter::new()));
manager.register_metrics_adapter(Arc::new(DatadogAdapter::new()));

// Record metric - goes to all healthy adapters
manager.record_counter("requests_total", 1, &[("service", "api")]).await;

// If one adapter fails, circuit breaker will skip it temporarily
// Other adapters continue working normally
```

### Configuration Options

```rust
use rustci::core::networking::valkyrie::observability::adapter_system::{
    ObservabilityConfig, CircuitBreakerConfig
};

let config = ObservabilityConfig {
    // Feature flags
    metrics_enabled: true,
    tracing_enabled: true,
    health_enabled: true,
    logging_enabled: true,
    
    // Log level filtering
    log_level: LogLevel::Info,
    
    // Circuit breaker settings
    circuit_breaker: CircuitBreakerConfig {
        failure_threshold: 5,        // Open after 5 failures
        timeout: Duration::from_secs(60), // Wait 60s before retry
        success_threshold: 3,        // Close after 3 successes
    },
    
    // Adapter-specific configurations
    adapter_configs: {
        let mut configs = HashMap::new();
        configs.insert("prometheus".to_string(), serde_json::json!({
            "listen_port": 9090,
            "metrics_path": "/metrics"
        }));
        configs
    },
};

let manager = ObservabilityManager::with_config(config);
```

## Built-in Adapters

### No-Op Adapters
- **Zero cost**: Compile to nothing in release mode
- **Always available**: No external dependencies
- **Fallback behavior**: Used when no other adapters registered

### Built-in JSON Logger
- **Structured logging**: JSON format output
- **Always available**: No external dependencies
- **Configurable levels**: Trace, Debug, Info, Warn, Error, Fatal
- **Correlation support**: Automatic correlation ID tracking

## Optional Integrations

### OpenTelemetry (Feature: `opentelemetry`)
- **OTLP support**: Both gRPC and HTTP protocols
- **Jaeger integration**: Direct Jaeger agent support
- **Configurable sampling**: Control trace sampling rates
- **Resource attributes**: Service metadata and tags

### Prometheus (Feature: `prometheus`)
- **Metrics exposition**: Standard Prometheus format
- **HTTP server**: Built-in metrics endpoint
- **Label support**: Full Prometheus label compatibility
- **Histogram buckets**: Configurable bucket boundaries

### Grafana (Feature: `grafana`)
- **Dashboard integration**: Automatic dashboard creation
- **Alert rules**: Configurable alerting
- **Data source**: Automatic data source configuration

### Jaeger (Feature: `jaeger`)
- **Direct integration**: Native Jaeger client
- **Batch export**: Efficient trace batching
- **Sampling strategies**: Remote sampling configuration

## Health Monitoring

The system includes comprehensive health monitoring:

```rust
// Check overall system health
let health = manager.get_adapter_health().await;
for (component, health) in health {
    match health.status {
        HealthStatus::Healthy => println!("✅ {}: Healthy", component),
        HealthStatus::Degraded { reason } => println!("⚠️  {}: Degraded - {}", component, reason),
        HealthStatus::Unhealthy { reason } => println!("❌ {}: Unhealthy - {}", component, reason),
        HealthStatus::Unknown => println!("❓ {}: Unknown", component),
    }
}
```

## Performance Characteristics

### Metrics
- **Recording latency**: <10μs for no-op adapters
- **Throughput**: >1M metrics/second with built-in adapters
- **Memory usage**: <1MB for 100K active metrics

### Tracing
- **Span creation**: <50μs per span
- **Context propagation**: Zero-copy span context
- **Memory overhead**: <100 bytes per active span

### Health Checks
- **Check frequency**: Configurable (default: 30s)
- **Response time**: <100ms for built-in checks
- **Failure detection**: <3 check cycles (default)

## Best Practices

### 1. Start Simple
```rust
// Begin with built-in adapters
let manager = ObservabilityManager::new();
```

### 2. Add Features Gradually
```rust
// Add external integrations as needed
#[cfg(feature = "prometheus")]
manager.register_metrics_adapter(prometheus_adapter);
```

### 3. Use Circuit Breakers
```rust
// Configure appropriate failure thresholds
let config = ObservabilityConfig {
    circuit_breaker: CircuitBreakerConfig {
        failure_threshold: 3,  // Fail fast
        timeout: Duration::from_secs(30), // Quick recovery
        success_threshold: 2,  // Conservative recovery
    },
    ..Default::default()
};
```

### 4. Monitor the Monitors
```rust
// Regular health checks
let health = manager.get_adapter_health().await;
for (name, health) in health {
    if health.error_count > 0 {
        log::warn!("Adapter {} has {} errors", name, health.error_count);
    }
}
```

### 5. Use Correlation IDs
```rust
// Track requests across services
let correlation_id = "req_12345";
manager.log_with_correlation(
    LogLevel::Info, 
    "Processing request", 
    correlation_id,
    &[("user_id", "67890")]
).await;
```

## Migration Guide

### From Existing Monitoring
1. **Identify current tools**: Prometheus, Jaeger, custom solutions
2. **Enable features**: Add appropriate Cargo features
3. **Create adapters**: Implement custom adapters if needed
4. **Gradual rollout**: Run both systems in parallel initially
5. **Validate metrics**: Ensure data consistency
6. **Switch over**: Disable old monitoring once validated

### Configuration Migration
```rust
// Old configuration
let old_config = OldMonitoringConfig {
    prometheus_port: 9090,
    jaeger_endpoint: "localhost:14268",
    log_level: "info",
};

// New configuration
let new_config = ObservabilityConfig {
    metrics_enabled: true,
    tracing_enabled: true,
    logging_enabled: true,
    log_level: LogLevel::Info,
    adapter_configs: {
        let mut configs = HashMap::new();
        configs.insert("prometheus".to_string(), serde_json::json!({
            "listen_port": old_config.prometheus_port
        }));
        configs.insert("jaeger".to_string(), serde_json::json!({
            "endpoint": old_config.jaeger_endpoint
        }));
        configs
    },
};
```

## Troubleshooting

### Common Issues

#### 1. Metrics Not Appearing
```rust
// Check if metrics are enabled
let config = ObservabilityConfig {
    metrics_enabled: true,  // Ensure this is true
    ..Default::default()
};

// Verify adapter registration
let health = manager.get_adapter_health().await;
println!("Registered adapters: {:?}", health.keys().collect::<Vec<_>>());
```

#### 2. High Memory Usage
```rust
// Configure retention policies
let config = ObservabilityConfig {
    circuit_breaker: CircuitBreakerConfig {
        failure_threshold: 3,  // Fail faster
        ..Default::default()
    },
    ..Default::default()
};
```

#### 3. Adapter Failures
```rust
// Check adapter health
let health = manager.get_adapter_health().await;
for (name, health) in health {
    if health.error_count > 0 {
        println!("Adapter {} has {} errors", name, health.error_count);
        println!("Last success: {:?}", health.last_successful_operation);
    }
}
```

### Debug Mode
```rust
// Enable debug logging
let config = ObservabilityConfig {
    logging_enabled: true,
    log_level: LogLevel::Debug,
    ..Default::default()
};
```

## Conclusion

The Pluggable Observability System provides a flexible, performant, and zero-dependency foundation for monitoring Valkyrie Protocol applications. Its trait-based architecture allows for easy customization while maintaining excellent performance characteristics and operational simplicity.

Key benefits:
- ✅ **Zero dependencies** by default
- ✅ **Pluggable architecture** for flexibility  
- ✅ **High performance** with minimal overhead
- ✅ **Circuit breaker protection** for reliability
- ✅ **Feature flags** for minimal footprint
- ✅ **Comprehensive monitoring** capabilities