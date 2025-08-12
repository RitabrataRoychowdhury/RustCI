# Valkyrie Protocol Observability Guide

## Overview

The Valkyrie Protocol includes a comprehensive, self-contained observability system that provides built-in metrics collection, structured logging, health monitoring, and an internal dashboard without requiring external dependencies.

## Features

### 🔍 Built-in Metrics Collection
- **Counter Metrics**: Track events and occurrences
- **Gauge Metrics**: Monitor current values and states
- **Histogram Metrics**: Analyze value distributions
- **Summary Metrics**: Aggregate statistics
- **Custom Aggregations**: Sum, average, percentiles (P50, P95, P99)

### 📝 Structured Logging
- **Correlation IDs**: Track requests across components
- **Contextual Data**: Rich structured context with each log entry
- **Log Levels**: Trace, Debug, Info, Warn, Error, Fatal
- **Retention Management**: Automatic cleanup of old log entries
- **Console & JSON Output**: Flexible output formats

### 🏥 Health Monitoring
- **System Health Checks**: Memory, CPU, disk space monitoring
- **Custom Health Checks**: TCP ports, HTTP endpoints, commands
- **Health Status**: Healthy, Degraded, Unhealthy, Unknown states
- **Automatic Recovery**: Circuit breaker patterns and failover
- **Health Summaries**: Comprehensive health reporting

### 📊 Internal Dashboard
- **Real-time Metrics**: Live system performance data
- **Health Status**: Visual health check results
- **System Overview**: Key performance indicators
- **HTML Generation**: Self-contained dashboard without external dependencies
- **Customizable Widgets**: Configurable dashboard components

### 🔗 Correlation Tracking
- **Distributed Tracing**: Track operations across components
- **Span Management**: Detailed operation timing and context
- **Correlation Statistics**: Performance and usage analytics
- **Request Tracing**: End-to-end request tracking

## Quick Start

### Basic Setup

```rust
use rustci::core::networking::valkyrie::observability::{
    ObservabilityManager, ObservabilityConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create observability manager
    let config = ObservabilityConfig::default();
    let mut manager = ObservabilityManager::new(config);
    
    // Start the observability system
    manager.start().await?;
    
    // Use the observability features
    let metrics = manager.metrics();
    let logger = manager.logger();
    let health = manager.health();
    
    // Your application code here...
    
    // Stop the observability system
    manager.stop().await?;
    Ok(())
}
```

### Recording Metrics

```rust
use std::collections::HashMap;

// Counter metrics
let mut labels = HashMap::new();
labels.insert("endpoint".to_string(), "/api/users".to_string());
metrics.increment_counter("http_requests_total", labels).await?;

// Gauge metrics
metrics.set_gauge("memory_usage_bytes", 1024.0 * 1024.0 * 512.0, HashMap::new()).await?;

// Histogram metrics
let response_times = vec![12.5, 23.1, 15.8, 45.2, 8.9];
metrics.record_histogram("response_time_ms", response_times, HashMap::new()).await?;

// Summary metrics
metrics.record_summary("request_size_bytes", 2048.0, 100, HashMap::new()).await?;
```

### Structured Logging

```rust
use std::collections::HashMap;

// Simple logging
logger.info("Application started").await?;
logger.warn("High memory usage detected").await?;
logger.error("Database connection failed").await?;

// Logging with context
let mut context = HashMap::new();
context.insert("user_id".to_string(), serde_json::Value::String("user123".to_string()));
context.insert("operation".to_string(), serde_json::Value::String("user_login".to_string()));
context.insert("duration_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(150)));

logger.log(LogLevel::Info, "User login completed", context).await?;

// Logging with correlation
let correlation_id = manager.new_correlation_id();
manager.log_with_correlation(
    correlation_id,
    LogLevel::Info,
    "Processing user request",
    HashMap::new(),
).await?;
```

### Health Monitoring

```rust
use rustci::core::networking::valkyrie::observability::{
    HealthCheck, HealthCheckType, HealthCheckConfig
};

// Register a custom health check
let health_check = HealthCheck {
    id: "database_connection".to_string(),
    name: "Database Connection".to_string(),
    description: "Checks database connectivity".to_string(),
    check_type: HealthCheckType::TcpPort {
        host: "localhost".to_string(),
        port: 5432,
    },
    config: HealthCheckConfig {
        parameters: HashMap::new(),
        environment: HashMap::new(),
        working_directory: None,
    },
    interval_seconds: 30,
    timeout_seconds: 5,
    failure_threshold: 3,
    success_threshold: 2,
    enabled: true,
    created_at: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs(),
};

health.register_check(health_check).await?;

// Get health status
let summary = health.summary().await;
println!("Overall health: {}", summary.overall_status);
```

### Dashboard Usage

```rust
// Get dashboard data
let dashboard = manager.dashboard();
let data = dashboard.get_data().await;

// Generate HTML dashboard
let html = dashboard.generate_html().await;
std::fs::write("dashboard.html", html)?;

// Access dashboard at http://localhost:8081 (if web server is configured)
```

### Correlation Tracking

```rust
// Start a correlation
let correlation_id = correlation.start_correlation("user_request", None).await?;

// Add metadata
correlation.add_metadata(
    correlation_id,
    "request_id",
    serde_json::Value::String("req_12345".to_string()),
).await?;

// Start a span
let span_id = correlation.start_span(
    correlation_id,
    "database_query",
    "user_service",
    None,
).await?;

// Your operation here...

// End the span
correlation.end_span(correlation_id, span_id, SpanStatus::Ok).await?;

// End the correlation
correlation.end_correlation(correlation_id, CorrelationStatus::Completed).await?;
```

## Configuration

### ObservabilityConfig

```rust
use rustci::core::networking::valkyrie::observability::ObservabilityConfig;

let config = ObservabilityConfig {
    metrics_enabled: true,
    logging_enabled: true,
    health_enabled: true,
    dashboard_enabled: true,
    metrics_retention_seconds: 3600,    // 1 hour
    log_retention_seconds: 86400,       // 24 hours
    health_check_interval_seconds: 30,
    dashboard_refresh_seconds: 5,
};
```

### Environment Variables

```bash
# Enable/disable observability features
VALKYRIE_OBSERVABILITY_ENABLED=true
VALKYRIE_METRICS_ENABLED=true
VALKYRIE_LOGGING_ENABLED=true
VALKYRIE_HEALTH_ENABLED=true
VALKYRIE_DASHBOARD_ENABLED=true

# Configuration
VALKYRIE_METRICS_RETENTION_SECONDS=3600
VALKYRIE_LOG_RETENTION_SECONDS=86400
VALKYRIE_HEALTH_CHECK_INTERVAL=30
VALKYRIE_DASHBOARD_REFRESH_SECONDS=5
VALKYRIE_DASHBOARD_PORT=8081
```

### YAML Configuration

```yaml
# config/valkyrie.yaml
observability:
  metrics_enabled: true
  logging_enabled: true
  health_enabled: true
  dashboard_enabled: true
  
  metrics_retention_seconds: 3600
  log_retention_seconds: 86400
  health_check_interval_seconds: 30
  dashboard_refresh_seconds: 5
  
  logging:
    min_level: "info"
    max_entries: 10000
    console_output: true
    json_output: false
    include_stack_traces: true
  
  health:
    default_checks:
      - name: "system_memory"
        type: "memory"
        max_usage_percent: 90.0
        enabled: true
      
      - name: "system_cpu"
        type: "cpu"
        max_usage_percent: 95.0
        enabled: true
  
  dashboard:
    title: "Valkyrie Protocol Dashboard"
    widgets:
      - id: "system_overview"
        type: "system_overview"
        enabled: true
      
      - id: "health_status"
        type: "health_status"
        enabled: true
```

## Migration from Legacy Systems

### Automatic Migration

Use the provided migration script to automatically migrate from legacy observability systems:

```bash
# Run the migration script
./scripts/migrate-to-valkyrie-observability.sh

# The script will:
# 1. Create backups of existing configuration
# 2. Generate Valkyrie observability configuration
# 3. Create migration tools
# 4. Provide rollback capabilities
```

### Manual Migration

For manual migration, follow these steps:

1. **Install Valkyrie Observability**
   ```rust
   // Add to your Cargo.toml dependencies
   rustci = { version = "1.0", features = ["valkyrie-observability"] }
   ```

2. **Update Configuration**
   ```yaml
   # Add to your config.yaml
   valkyrie:
     observability:
       enabled: true
       # ... other configuration
   ```

3. **Update Code**
   ```rust
   // Replace legacy observability calls
   // Old:
   legacy_metrics.counter("requests").increment();
   
   // New:
   metrics.increment_counter("requests", HashMap::new()).await?;
   ```

### Backward Compatibility

The Valkyrie observability system includes a backward compatibility layer:

```rust
use rustci::core::networking::valkyrie::observability::{
    LegacyObservabilityAdapter, CompatibilityConfig
};

// Create compatibility adapter
let compat_config = CompatibilityConfig::default();
let legacy_adapter = LegacyObservabilityAdapter::new(
    Arc::new(manager),
    compat_config,
);

// Use legacy API (with deprecation warnings)
legacy_adapter.record_counter("requests", 1, None).await?;
legacy_adapter.log_info("Legacy log message").await?;
```

## Best Practices

### Metrics

1. **Use Consistent Naming**: Follow a consistent naming convention (e.g., `component_operation_unit`)
2. **Add Meaningful Labels**: Include relevant context in metric labels
3. **Avoid High Cardinality**: Limit the number of unique label combinations
4. **Use Appropriate Types**: Choose the right metric type for your use case

```rust
// Good
metrics.increment_counter("http_requests_total", labels! {
    "method" => "GET",
    "endpoint" => "/api/users",
    "status" => "200"
}).await?;

// Avoid
metrics.increment_counter("requests", HashMap::new()).await?; // Too generic
```

### Logging

1. **Use Structured Logging**: Always include relevant context
2. **Choose Appropriate Levels**: Use log levels consistently
3. **Include Correlation IDs**: Enable request tracing
4. **Avoid Sensitive Data**: Don't log passwords or tokens

```rust
// Good
let mut context = HashMap::new();
context.insert("user_id".to_string(), serde_json::Value::String(user_id));
context.insert("operation".to_string(), serde_json::Value::String("login"));
logger.log(LogLevel::Info, "User login successful", context).await?;

// Avoid
logger.info("User logged in").await?; // Missing context
```

### Health Checks

1. **Check Critical Dependencies**: Monitor databases, external APIs, etc.
2. **Set Appropriate Thresholds**: Configure realistic failure thresholds
3. **Include Recovery Actions**: Implement automatic recovery where possible
4. **Monitor Resource Usage**: Track memory, CPU, and disk usage

### Correlation Tracking

1. **Use Consistent IDs**: Generate correlation IDs at request boundaries
2. **Propagate Context**: Pass correlation IDs through all operations
3. **Add Meaningful Spans**: Create spans for significant operations
4. **Include Relevant Metadata**: Add context that helps with debugging

## Troubleshooting

### Common Issues

1. **High Memory Usage**
   - Reduce retention periods
   - Limit maximum entries
   - Check for metric cardinality explosion

2. **Performance Impact**
   - Disable unnecessary features
   - Reduce collection frequency
   - Use sampling for high-volume metrics

3. **Missing Data**
   - Check if features are enabled
   - Verify retention settings
   - Ensure proper initialization

### Debug Mode

Enable debug logging to troubleshoot issues:

```rust
let config = ObservabilityConfig {
    logging_enabled: true,
    // ... other config
};

// Set log level to debug
let log_config = LogConfig {
    min_level: LogLevel::Debug,
    console_output: true,
    // ... other config
};
```

### Monitoring the Monitor

Monitor the observability system itself:

```rust
// Check observability status
let status = manager.status().await;
println!("Metrics enabled: {}", status.metrics_enabled);
println!("Metrics count: {}", status.metrics_count);
println!("Log count: {}", status.log_count);
println!("Health status: {}", status.health_status);
```

## API Reference

For detailed API documentation, see:
- [Metrics API](./api-reference.md#metrics)
- [Logging API](./api-reference.md#logging)
- [Health API](./api-reference.md#health)
- [Dashboard API](./api-reference.md#dashboard)
- [Correlation API](./api-reference.md#correlation)

## Examples

Complete examples are available in the `examples/` directory:
- [Basic Observability](../examples/observability_example.rs)
- [Advanced Metrics](../examples/advanced_metrics.rs)
- [Health Monitoring](../examples/health_monitoring.rs)
- [Dashboard Integration](../examples/dashboard_integration.rs)

## Support

For support and questions:
- Check the [FAQ](./faq.md)
- Review [troubleshooting guide](./troubleshooting.md)
- Open an issue on GitHub
- Join the community discussions