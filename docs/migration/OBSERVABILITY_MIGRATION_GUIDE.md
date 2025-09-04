# RustCI Protocol Observability Migration Guide

## Overview

This guide helps you migrate from legacy observability systems to the new RustCI Protocol observability system. The migration provides enhanced features, better performance, and self-contained operation without external dependencies.

## Why Migrate?

### Benefits of RustCI Observability

✅ **Self-Contained**: No external dependencies required  
✅ **High Performance**: Sub-millisecond metric recording  
✅ **Built-in Dashboard**: Internal HTML dashboard generation  
✅ **Correlation Tracking**: Distributed tracing capabilities  
✅ **Backward Compatible**: Legacy API support with deprecation warnings  
✅ **Structured Logging**: Rich contextual logging with correlation IDs  
✅ **Health Monitoring**: Comprehensive system health checks  
✅ **Easy Configuration**: YAML-based configuration with sensible defaults  

### Migration Timeline

- **Phase 1**: Install and configure RustCI observability alongside existing systems
- **Phase 2**: Gradually migrate metrics, logging, and health checks
- **Phase 3**: Switch to RustCI dashboard and disable legacy systems
- **Phase 4**: Remove legacy observability dependencies

## Pre-Migration Checklist

Before starting the migration, ensure you have:

- [ ] Current system backup
- [ ] Inventory of existing metrics, logs, and health checks
- [ ] Understanding of current observability dependencies
- [ ] Test environment for validation
- [ ] Rollback plan prepared

## Automated Migration

### Using the Migration Script

The easiest way to migrate is using the provided automated migration script:

```bash
# Make the script executable
chmod +x scripts/migrate-to-rustci-observability.sh

# Run the migration
./scripts/migrate-to-rustci-observability.sh
```

The script will:
1. Create a backup of your current configuration
2. Generate RustCI observability configuration
3. Update environment variables
4. Create migration and rollback tools
5. Validate the new configuration

### Migration Script Output

```
🛡️ RustCI Protocol Observability Migration Tool
=================================================

[2024-01-15 10:30:00] Starting RustCI Protocol observability migration
[2024-01-15 10:30:01] Creating backup directory: ./rustci-migration-backup-20240115_103000
[2024-01-15 10:30:01] Backed up config.yaml
[2024-01-15 10:30:02] Created RustCI observability configuration: config/rustci.yaml
[2024-01-15 10:30:02] Updated .env with RustCI observability variables
[2024-01-15 10:30:03] Created data migration tool: tools/rustci-data-migrator.sh
[2024-01-15 10:30:03] Created rollback tool: tools/rustci-rollback.sh
[2024-01-15 10:30:04] Configuration file syntax validated
[2024-01-15 10:30:04] Migration validation completed
[2024-01-15 10:30:04] Migration completed successfully

🎉 RustCI Protocol Observability Migration Complete!
=====================================================

📁 Backup created in: ./rustci-migration-backup-20240115_103000
⚙️  Configuration file: config/rustci.yaml
📊 Migration log: rustci-observability-migration.log

🔧 Migration tools created:
   • tools/rustci-data-migrator.sh - Migrate existing data
   • tools/rustci-rollback.sh - Rollback migration

📋 Next steps:
   1. Review the configuration in config/rustci.yaml
   2. Run: tools/rustci-data-migrator.sh (if you have existing data)
   3. Restart RustCI to enable RustCI observability
   4. Access dashboard at: http://localhost:8081

🔄 To rollback: tools/rustci-rollback.sh ./rustci-migration-backup-20240115_103000
```

## Manual Migration

### Step 1: Install RustCI Observability

Add RustCI observability to your project:

```toml
# Cargo.toml
[dependencies]
rustci = { version = "1.0", features = ["rustci-observability"] }
```

### Step 2: Configuration Migration

#### Legacy Configuration (Before)

```yaml
# config.yaml (legacy)
observability:
  metrics:
    enabled: true
    backend: "prometheus"
    port: 9090
  
  logging:
    level: "info"
    format: "json"
    output: "stdout"
  
  health:
    enabled: true
    port: 8080
    checks:
      - name: "database"
        type: "tcp"
        target: "localhost:5432"
```

#### RustCI Configuration (After)

```yaml
# config/rustci.yaml (new)
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
    json_output: true
    include_stack_traces: true
  
  health:
    default_checks:
      - name: "database_connection"
        type: "tcp_port"
        host: "localhost"
        port: 5432
        interval_seconds: 30
        enabled: true
      
      - name: "system_memory"
        type: "memory"
        max_usage_percent: 90.0
        enabled: true
      
      - name: "system_cpu"
        type: "cpu"
        max_usage_percent: 95.0
        enabled: true
  
  dashboard:
    title: "RustCI Dashboard"
    widgets:
      - id: "system_overview"
        type: "system_overview"
        enabled: true
      
      - id: "health_status"
        type: "health_status"
        enabled: true
      
      - id: "performance_metrics"
        type: "line_chart"
        metrics: ["latency_ms", "throughput_rps"]
        enabled: true
```

### Step 3: Code Migration

#### Metrics Migration

**Legacy Code (Before):**
```rust
// Legacy metrics
use prometheus::{Counter, Gauge, Histogram, Registry};

let counter = Counter::new("requests_total", "Total requests")?;
let gauge = Gauge::new("memory_usage", "Memory usage")?;
let histogram = Histogram::new("request_duration", "Request duration")?;

counter.inc();
gauge.set(1024.0);
histogram.observe(0.5);
```

**RustCI Code (After):**
```rust
// RustCI metrics
use rustci::core::networking::rustci::observability::{
    ObservabilityManager, ObservabilityConfig, MetricValue
};
use std::collections::HashMap;

let config = ObservabilityConfig::default();
let mut manager = ObservabilityManager::new(config);
manager.start().await?;

let metrics = manager.metrics();
let mut labels = HashMap::new();
labels.insert("endpoint".to_string(), "/api/users".to_string());

// Counter
metrics.increment_counter("requests_total", labels.clone()).await?;

// Gauge
metrics.set_gauge("memory_usage_bytes", 1024.0, labels.clone()).await?;

// Histogram
metrics.record_histogram("request_duration_seconds", vec![0.5], labels).await?;
```

#### Logging Migration

**Legacy Code (Before):**
```rust
// Legacy logging
use log::{info, warn, error};

info!("Application started");
warn!("High memory usage: {}MB", memory_mb);
error!("Database connection failed: {}", error);
```

**RustCI Code (After):**
```rust
// RustCI logging
use rustci::core::networking::rustci::observability::LogLevel;
use std::collections::HashMap;

let logger = manager.logger();

// Simple logging
logger.info("Application started").await?;

// Structured logging
let mut context = HashMap::new();
context.insert("memory_mb".to_string(), serde_json::Value::Number(serde_json::Number::from(512)));
logger.log(LogLevel::Warn, "High memory usage detected", context).await?;

// Error logging with context
let mut error_context = HashMap::new();
error_context.insert("error".to_string(), serde_json::Value::String(error.to_string()));
error_context.insert("component".to_string(), serde_json::Value::String("database".to_string()));
logger.log(LogLevel::Error, "Database connection failed", error_context).await?;
```

#### Health Checks Migration

**Legacy Code (Before):**
```rust
// Legacy health checks
use actix_web_httpauth::middleware::HttpAuthentication;

async fn health_check() -> impl Responder {
    // Check database connection
    if database_is_healthy().await {
        HttpResponse::Ok().json(json!({"status": "healthy"}))
    } else {
        HttpResponse::ServiceUnavailable().json(json!({"status": "unhealthy"}))
    }
}
```

**RustCI Code (After):**
```rust
// RustCI health checks
use rustci::core::networking::rustci::observability::{
    HealthCheck, HealthCheckType, HealthCheckConfig
};

let health = manager.health();

// Register database health check
let db_check = HealthCheck {
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

health.register_check(db_check).await?;

// Get health status
let summary = health.summary().await;
println!("Overall health: {}", summary.overall_status);
```

### Step 4: Environment Variables

Update your environment variables:

```bash
# .env (add these variables)

# Enable RustCI observability
RUSTCI_OBSERVABILITY_ENABLED=true
RUSTCI_METRICS_ENABLED=true
RUSTCI_LOGGING_ENABLED=true
RUSTCI_HEALTH_ENABLED=true
RUSTCI_DASHBOARD_ENABLED=true

# Configuration
RUSTCI_METRICS_RETENTION_SECONDS=3600
RUSTCI_LOG_RETENTION_SECONDS=86400
RUSTCI_HEALTH_CHECK_INTERVAL=30
RUSTCI_DASHBOARD_REFRESH_SECONDS=5
RUSTCI_DASHBOARD_PORT=8081

# Legacy system (disable gradually)
LEGACY_METRICS_ENABLED=false
LEGACY_LOGGING_ENABLED=false
PROMETHEUS_ENABLED=false
```

## Backward Compatibility

### Using the Compatibility Layer

For gradual migration, use the backward compatibility layer:

```rust
use rustci::core::networking::rustci::observability::{
    LegacyObservabilityAdapter, CompatibilityConfig
};

// Create compatibility adapter
let compat_config = CompatibilityConfig {
    legacy_api_enabled: true,
    auto_metric_conversion: true,
    legacy_log_format: true,
    deprecation_warnings: true,
    compatibility_version: "1.0.0".to_string(),
};

let legacy_adapter = LegacyObservabilityAdapter::new(
    Arc::new(manager),
    compat_config,
);

// Use legacy API (with deprecation warnings)
legacy_adapter.record_counter("requests", 1, None).await?;
legacy_adapter.log_info("Legacy log message").await?;
legacy_adapter.record_gauge("memory_usage", 1024.0, None).await?;
```

### Protocol Version Negotiation

Handle different protocol versions:

```rust
use rustci::core::networking::rustci::observability::ProtocolVersionNegotiator;

let negotiator = ProtocolVersionNegotiator::new();

// Client requests specific versions
let client_versions = vec!["1.0.0".to_string(), "0.9.0".to_string()];
if let Some(version) = negotiator.negotiate_version(&client_versions) {
    println!("Using protocol version: {}", version);
} else {
    println!("No compatible version found");
}
```

### Feature Detection

Detect available features for graceful degradation:

```rust
use rustci::core::networking::rustci::observability::FeatureDetector;

let detector = FeatureDetector::new();

if detector.is_feature_available("structured_logging") {
    // Use structured logging
    logger.log(LogLevel::Info, "Message", context).await?;
} else {
    // Fall back to simple logging
    logger.info("Message").await?;
}
```

## Data Migration

### Migrating Existing Data

Use the data migration tool to convert existing observability data:

```bash
# Run data migration
./tools/rustci-data-migrator.sh

# The tool will:
# 1. Convert existing metrics to RustCI format
# 2. Transform logs to structured format
# 3. Migrate health check configurations
# 4. Preserve historical data where possible
```

### Custom Data Migration

For custom data migration needs:

```rust
// Example: Migrate Prometheus metrics to RustCI
use std::fs::File;
use std::io::{BufRead, BufReader};

async fn migrate_prometheus_data(
    prometheus_file: &str,
    rustci_metrics: &MetricsCollector,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(prometheus_file)?;
    let reader = BufReader::new(file);
    
    for line in reader.lines() {
        let line = line?;
        if let Some((metric_name, value, labels)) = parse_prometheus_line(&line) {
            rustci_metrics.record(
                &metric_name,
                MetricValue::Gauge(value),
                labels,
            ).await?;
        }
    }
    
    Ok(())
}

fn parse_prometheus_line(line: &str) -> Option<(String, f64, HashMap<String, String>)> {
    // Parse Prometheus exposition format
    // Implementation depends on your specific format
    None // Placeholder
}
```

## Testing the Migration

### Validation Steps

1. **Start Both Systems**: Run legacy and RustCI observability in parallel
2. **Compare Metrics**: Verify metrics are being recorded correctly
3. **Test Logging**: Ensure logs are structured and searchable
4. **Health Checks**: Confirm health checks are working
5. **Dashboard**: Access the new dashboard and verify data

### Test Script

```bash
#!/bin/bash
# test-migration.sh

echo "🧪 Testing RustCI Observability Migration"

# Test metrics endpoint
echo "Testing metrics..."
curl -s http://localhost:8081/metrics | grep -q "rustci_" && echo "✅ Metrics OK" || echo "❌ Metrics Failed"

# Test health endpoint
echo "Testing health..."
curl -s http://localhost:8081/health | grep -q "healthy" && echo "✅ Health OK" || echo "❌ Health Failed"

# Test dashboard
echo "Testing dashboard..."
curl -s http://localhost:8081/dashboard | grep -q "RustCI" && echo "✅ Dashboard OK" || echo "❌ Dashboard Failed"

# Test logging
echo "Testing logging..."
tail -n 10 /var/log/rustci/rustci.log | grep -q "correlation_id" && echo "✅ Logging OK" || echo "❌ Logging Failed"

echo "🎉 Migration test complete!"
```

## Rollback Procedure

### Automatic Rollback

If you need to rollback the migration:

```bash
# Use the generated rollback script
./tools/rustci-rollback.sh ./rustci-migration-backup-20240115_103000

# This will:
# 1. Restore original configuration files
# 2. Revert environment variables
# 3. Disable RustCI observability
# 4. Re-enable legacy systems
```

### Manual Rollback

For manual rollback:

1. **Stop RustCI Observability**
   ```rust
   manager.stop().await?;
   ```

2. **Restore Configuration**
   ```bash
   cp backup/config.yaml config.yaml
   rm config/rustci.yaml
   ```

3. **Update Environment**
   ```bash
   # .env
   RUSTCI_OBSERVABILITY_ENABLED=false
   LEGACY_METRICS_ENABLED=true
   PROMETHEUS_ENABLED=true
   ```

4. **Restart Application**
   ```bash
   systemctl restart rustci
   ```

## Troubleshooting

### Common Issues

#### High Memory Usage
```rust
// Reduce retention periods
let config = ObservabilityConfig {
    metrics_retention_seconds: 1800,  // 30 minutes instead of 1 hour
    log_retention_seconds: 43200,     // 12 hours instead of 24 hours
    // ...
};
```

#### Missing Metrics
```rust
// Check if metrics are enabled
let status = manager.status().await;
if !status.metrics_enabled {
    println!("Metrics are disabled!");
}

// Verify metric recording
let summary = metrics.summary().await;
println!("Total metrics: {}", summary.total_metrics);
```

#### Dashboard Not Loading
```bash
# Check if dashboard is enabled
curl http://localhost:8081/health

# Check logs for errors
tail -f /var/log/rustci/rustci.log | grep -i error
```

### Debug Mode

Enable debug logging for troubleshooting:

```rust
let log_config = LogConfig {
    min_level: LogLevel::Debug,
    console_output: true,
    json_output: true,
    include_stack_traces: true,
    // ...
};
```

## Performance Considerations

### Optimization Tips

1. **Metric Cardinality**: Limit unique label combinations
2. **Retention Periods**: Set appropriate retention based on needs
3. **Sampling**: Use sampling for high-volume metrics
4. **Batch Operations**: Group related operations together

### Performance Monitoring

```rust
// Monitor observability system performance
let status = manager.status().await;
println!("Metrics count: {}", status.metrics_count);
println!("Log count: {}", status.log_count);

// Check correlation statistics
let correlation_stats = correlation.get_statistics().await;
println!("Average duration: {:.2}s", correlation_stats.average_duration_seconds);
```

## Post-Migration

### Cleanup

After successful migration:

1. **Remove Legacy Dependencies**
   ```toml
   # Remove from Cargo.toml
   # prometheus = "0.13"
   # log = "0.4"
   # env_logger = "0.10"
   ```

2. **Clean Up Configuration**
   ```bash
   rm config/prometheus.yml
   rm config/logging.conf
   ```

3. **Update Documentation**
   - Update deployment guides
   - Update monitoring runbooks
   - Update troubleshooting guides

### Monitoring

Set up monitoring for the new observability system:

```rust
// Monitor the monitor
let health_summary = health.summary().await;
if health_summary.overall_status != HealthStatus::Healthy {
    // Alert on observability system issues
    alert_manager.send_alert("Observability system unhealthy").await?;
}
```

## Support

### Getting Help

- 📖 [Architecture Documentation](../architecture/README.md)
- 🔧 [API Reference](../api/README.md)
- 🐛 [Deployment Guide](../deployment/guide.md)
- 💬 [Community Discussions](https://github.com/rustci/discussions)
- 🎫 [Issue Tracker](https://github.com/rustci/issues)

### Migration Support

For migration assistance:
- Review the [User Documentation](../user/README.md)
- Check [Development Documentation](../development/README.md)
- Join the [migration support channel](https://discord.gg/rustci-migration)

---

**Next Steps**: After completing the migration, see the [Architecture Documentation](../architecture/README.md) for advanced usage patterns and best practices.