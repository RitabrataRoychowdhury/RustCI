# Valkyrie Protocol External Observability Integration

## Overview

The Valkyrie Protocol provides comprehensive external observability integration, allowing seamless connection with industry-standard monitoring, logging, and tracing systems. This integration extends the built-in self-contained observability with enterprise-grade external tools.

## Supported External Systems

### 📈 Prometheus Integration
- **Metrics Export**: Automatic export of Valkyrie metrics to Prometheus
- **Push Gateway Support**: Batch metric pushing for short-lived processes
- **Custom Metrics**: Support for custom business metrics
- **Alerting**: Integration with Prometheus AlertManager

### 🔍 OpenTelemetry Integration
- **Unified Observability**: Metrics, logs, and traces in one system
- **Vendor Neutral**: Works with any OpenTelemetry-compatible backend
- **Automatic Instrumentation**: Zero-code instrumentation for common operations
- **Custom Spans**: Manual span creation for detailed tracing

### 🕵️ Jaeger Integration
- **Distributed Tracing**: End-to-end request tracing across services
- **Performance Analysis**: Latency analysis and bottleneck identification
- **Service Dependencies**: Visual service dependency mapping
- **Trace Sampling**: Configurable sampling strategies

### 📊 Grafana Integration
- **Dashboard Management**: Automatic dashboard creation and updates
- **Visualization**: Rich visualizations for Valkyrie metrics
- **Alerting**: Grafana-based alerting and notifications
- **Custom Dashboards**: Support for custom dashboard configurations

## Quick Start

### Basic Configuration

```rust
use rustci::core::networking::valkyrie::observability::{
    ObservabilityManager, ObservabilityConfig, ExternalObservabilityConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure external integrations
    let external_config = ExternalObservabilityConfig {
        prometheus_enabled: true,
        prometheus_endpoint: "http://localhost:9090".to_string(),
        prometheus_push_gateway: Some("http://localhost:9091".to_string()),
        
        opentelemetry_enabled: true,
        opentelemetry_endpoint: "http://localhost:4317".to_string(),
        
        jaeger_enabled: true,
        jaeger_endpoint: "http://localhost:14268".to_string(),
        
        grafana_enabled: true,
        grafana_endpoint: "http://localhost:3000".to_string(),
        grafana_api_key: Some("your-api-key".to_string()),
        
        export_interval_seconds: 15,
        batch_size: 1000,
    };

    // Create observability manager with external integrations
    let config = ObservabilityConfig {
        metrics_enabled: true,
        logging_enabled: true,
        health_enabled: true,
        dashboard_enabled: true,
        external_config: Some(external_config),
        ..Default::default()
    };

    let mut manager = ObservabilityManager::new(config);
    manager.start().await?;

    // Your application code here...

    manager.stop().await?;
    Ok(())
}
```

### Environment Configuration

```bash
# External observability configuration
VALKYRIE_PROMETHEUS_ENABLED=true
VALKYRIE_PROMETHEUS_ENDPOINT=http://localhost:9090
VALKYRIE_PROMETHEUS_PUSH_GATEWAY=http://localhost:9091

VALKYRIE_OPENTELEMETRY_ENABLED=true
VALKYRIE_OPENTELEMETRY_ENDPOINT=http://localhost:4317

VALKYRIE_JAEGER_ENABLED=true
VALKYRIE_JAEGER_ENDPOINT=http://localhost:14268

VALKYRIE_GRAFANA_ENABLED=true
VALKYRIE_GRAFANA_ENDPOINT=http://localhost:3000
VALKYRIE_GRAFANA_API_KEY=your-grafana-api-key

VALKYRIE_EXPORT_INTERVAL_SECONDS=15
VALKYRIE_EXPORT_BATCH_SIZE=1000
```

## Prometheus Integration

### Metrics Export

The Valkyrie Protocol automatically exports metrics to Prometheus in the standard exposition format:

```prometheus
# HELP valkyrie_total_metrics Total number of Valkyrie metrics
# TYPE valkyrie_total_metrics gauge
valkyrie_total_metrics 150

# HELP valkyrie_request_duration_ms Request duration in milliseconds
# TYPE valkyrie_request_duration_ms histogram
valkyrie_request_duration_ms_bucket{le="0.1"} 1000
valkyrie_request_duration_ms_bucket{le="0.5"} 1500
valkyrie_request_duration_ms_bucket{le="1.0"} 1800
valkyrie_request_duration_ms_bucket{le="+Inf"} 2000
valkyrie_request_duration_ms_sum 450.5
valkyrie_request_duration_ms_count 2000

# HELP valkyrie_requests_total Total number of requests
# TYPE valkyrie_requests_total counter
valkyrie_requests_total{method="GET",endpoint="/api/v1/jobs"} 1500
valkyrie_requests_total{method="POST",endpoint="/api/v1/jobs"} 500
```

### Push Gateway Configuration

For batch jobs and short-lived processes:

```rust
let external_config = ExternalObservabilityConfig {
    prometheus_enabled: true,
    prometheus_push_gateway: Some("http://localhost:9091".to_string()),
    export_interval_seconds: 30, // Push every 30 seconds
    ..Default::default()
};
```

### Custom Metrics

```rust
// Record custom business metrics
let mut labels = HashMap::new();
labels.insert("service".to_string(), "valkyrie-api".to_string());
labels.insert("version".to_string(), "1.0.0".to_string());

metrics.increment_counter("business_transactions_total", labels.clone()).await?;
metrics.set_gauge("active_users", 1250.0, labels.clone()).await?;
metrics.record_histogram("transaction_amount", vec![99.99, 149.99, 299.99], labels).await?;
```

## OpenTelemetry Integration

### Automatic Instrumentation

The Valkyrie Protocol provides automatic instrumentation for:
- HTTP requests and responses
- Database queries
- Message processing
- Inter-service communication

### Manual Instrumentation

```rust
// Create custom spans
let correlation_id = correlation.start_correlation("user_checkout", None).await?;

let span_id = correlation.start_span(
    correlation_id,
    "payment_processing",
    "payment-service",
    None,
).await?;

// Add span attributes
correlation.add_metadata(
    correlation_id,
    "user_id",
    serde_json::Value::String("user123".to_string()),
).await?;

// Add span events
correlation.add_span_log(
    correlation_id,
    span_id,
    "INFO",
    "Payment validation completed",
    HashMap::new(),
).await?;

// End span
correlation.end_span(correlation_id, span_id, SpanStatus::Ok).await?;
correlation.end_correlation(correlation_id, CorrelationStatus::Completed).await?;
```

### OpenTelemetry Collector Configuration

```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  batch:
    timeout: 1s
    send_batch_size: 1024
  memory_limiter:
    limit_mib: 512

exporters:
  prometheus:
    endpoint: "0.0.0.0:8889"
  jaeger:
    endpoint: jaeger:14250
    tls:
      insecure: true
  logging:
    loglevel: debug

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [jaeger, logging]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [prometheus, logging]
    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [logging]
```

## Jaeger Integration

### Distributed Tracing

Jaeger integration provides comprehensive distributed tracing:

```rust
// Start a distributed trace
let correlation_id = correlation.start_correlation("api_request", None).await?;

// Create nested spans
let auth_span = correlation.start_span(
    correlation_id,
    "authentication",
    "auth-service",
    None,
).await?;

let db_span = correlation.start_span(
    correlation_id,
    "database_query",
    "user-service",
    Some(auth_span),
).await?;

// Add trace context
correlation.add_metadata(
    correlation_id,
    "trace.id",
    serde_json::Value::String(correlation_id.to_string()),
).await?;

// End spans in reverse order
correlation.end_span(correlation_id, db_span, SpanStatus::Ok).await?;
correlation.end_span(correlation_id, auth_span, SpanStatus::Ok).await?;
```

### Trace Sampling

Configure sampling strategies:

```rust
let external_config = ExternalObservabilityConfig {
    jaeger_enabled: true,
    jaeger_endpoint: "http://localhost:14268".to_string(),
    // Additional Jaeger-specific configuration would go here
    ..Default::default()
};
```

## Grafana Integration

### Automatic Dashboard Creation

The Valkyrie Protocol can automatically create and update Grafana dashboards:

```rust
// Update dashboards programmatically
if let Some(external) = manager.external() {
    external.update_dashboards().await?;
}
```

### Dashboard Configuration

Dashboards are created with:
- **Overview Dashboard**: System-wide metrics and health
- **Performance Dashboard**: Latency, throughput, and resource usage
- **Error Dashboard**: Error rates, types, and trends
- **Business Dashboard**: Custom business metrics

### Custom Dashboard Templates

Create custom dashboard templates in `monitoring/grafana/`:

```json
{
  "dashboard": {
    "title": "Valkyrie Custom Dashboard",
    "panels": [
      {
        "title": "Custom Metric",
        "targets": [
          {
            "expr": "valkyrie_custom_metric",
            "legendFormat": "Custom Metric"
          }
        ]
      }
    ]
  }
}
```

## Alerting Configuration

### Prometheus Alerts

Configure alerts in `monitoring/alerts/valkyrie_alerts.yml`:

```yaml
groups:
  - name: valkyrie_alerts
    rules:
      - alert: ValkyrieHighLatency
        expr: histogram_quantile(0.95, rate(valkyrie_request_duration_ms_bucket[5m])) > 1000
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Valkyrie Protocol high latency detected"
          description: "P95 latency is {{ $value }}ms"

      - alert: ValkyrieHighErrorRate
        expr: rate(valkyrie_errors_total[5m]) / rate(valkyrie_requests_total[5m]) > 0.05
        for: 3m
        labels:
          severity: critical
        annotations:
          summary: "Valkyrie Protocol high error rate"
          description: "Error rate is {{ $value | humanizePercentage }}"
```

### Grafana Alerts

Configure Grafana alerts through the API:

```rust
// Example: Create alert rule
let alert_rule = json!({
    "title": "Valkyrie High Memory Usage",
    "condition": "A",
    "data": [{
        "refId": "A",
        "queryType": "",
        "model": {
            "expr": "valkyrie_memory_usage_bytes / valkyrie_memory_limit_bytes > 0.8",
            "interval": "",
            "refId": "A"
        }
    }],
    "intervalSeconds": 60,
    "noDataState": "NoData",
    "execErrState": "Alerting"
});
```

## Deployment

### Docker Compose Setup

```yaml
version: '3.8'
services:
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml
      - ./monitoring/alerts:/etc/prometheus/alerts

  pushgateway:
    image: prom/pushgateway:latest
    ports:
      - "9091:9091"

  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"
      - "14268:14268"
    environment:
      - COLLECTOR_OTLP_ENABLED=true

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - ./monitoring/grafana:/etc/grafana/provisioning

  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    ports:
      - "4317:4317"
      - "4318:4318"
    volumes:
      - ./config/otel-collector-config.yaml:/etc/otel-collector-config.yaml
    command: ["--config=/etc/otel-collector-config.yaml"]

  valkyrie-app:
    build: .
    ports:
      - "8080:8080"
      - "8081:8081"
    environment:
      - VALKYRIE_PROMETHEUS_ENABLED=true
      - VALKYRIE_PROMETHEUS_ENDPOINT=http://prometheus:9090
      - VALKYRIE_PROMETHEUS_PUSH_GATEWAY=http://pushgateway:9091
      - VALKYRIE_OPENTELEMETRY_ENABLED=true
      - VALKYRIE_OPENTELEMETRY_ENDPOINT=http://otel-collector:4317
      - VALKYRIE_JAEGER_ENABLED=true
      - VALKYRIE_JAEGER_ENDPOINT=http://jaeger:14268
      - VALKYRIE_GRAFANA_ENABLED=true
      - VALKYRIE_GRAFANA_ENDPOINT=http://grafana:3000
    depends_on:
      - prometheus
      - jaeger
      - grafana
      - otel-collector
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: valkyrie-app
spec:
  replicas: 3
  selector:
    matchLabels:
      app: valkyrie-app
  template:
    metadata:
      labels:
        app: valkyrie-app
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8081"
        prometheus.io/path: "/valkyrie/metrics"
    spec:
      containers:
      - name: valkyrie-app
        image: valkyrie:latest
        ports:
        - containerPort: 8080
        - containerPort: 8081
        env:
        - name: VALKYRIE_PROMETHEUS_ENABLED
          value: "true"
        - name: VALKYRIE_OPENTELEMETRY_ENABLED
          value: "true"
        - name: VALKYRIE_OPENTELEMETRY_ENDPOINT
          value: "http://otel-collector:4317"
        - name: VALKYRIE_JAEGER_ENABLED
          value: "true"
        - name: VALKYRIE_JAEGER_ENDPOINT
          value: "http://jaeger-collector:14268"
```

## Monitoring Best Practices

### Metric Naming

Follow Prometheus naming conventions:
- Use snake_case: `valkyrie_request_duration_ms`
- Include units: `_seconds`, `_bytes`, `_total`
- Use consistent prefixes: `valkyrie_*`

### Label Management

- Keep cardinality low (< 10 values per label)
- Use meaningful label names
- Avoid high-cardinality labels (user IDs, timestamps)

```rust
// Good
let mut labels = HashMap::new();
labels.insert("method".to_string(), "GET".to_string());
labels.insert("status".to_string(), "200".to_string());
labels.insert("endpoint".to_string(), "/api/v1/jobs".to_string());

// Avoid
labels.insert("user_id".to_string(), "user123456".to_string()); // High cardinality
labels.insert("timestamp".to_string(), "2024-01-15T10:30:00Z".to_string()); // Unique values
```

### Trace Sampling

Configure appropriate sampling rates:
- **Development**: 100% sampling
- **Staging**: 10-50% sampling
- **Production**: 1-10% sampling for high-volume services

### Dashboard Organization

Organize dashboards by:
- **System Level**: Infrastructure metrics
- **Service Level**: Application metrics
- **Business Level**: Business KPIs
- **SLA Level**: Service level objectives

## Troubleshooting

### Common Issues

#### Prometheus Connection Failed
```
Error: Failed to connect to Prometheus at http://localhost:9090
```

**Solution**: Verify Prometheus is running and accessible:
```bash
curl http://localhost:9090/api/v1/status/config
```

#### OpenTelemetry Export Failed
```
Error: OpenTelemetry export failed: connection refused
```

**Solution**: Check OpenTelemetry Collector configuration:
```bash
docker logs otel-collector
```

#### Jaeger Traces Not Appearing
```
Warning: Traces exported but not visible in Jaeger UI
```

**Solution**: Verify Jaeger configuration and sampling:
```bash
curl http://localhost:16686/api/services
```

#### Grafana Dashboard Not Updating
```
Error: Grafana API returned 401 Unauthorized
```

**Solution**: Check API key and permissions:
```bash
curl -H "Authorization: Bearer YOUR_API_KEY" http://localhost:3000/api/dashboards/home
```

### Debug Mode

Enable debug logging for external integrations:

```rust
let external_config = ExternalObservabilityConfig {
    // Enable debug logging
    ..Default::default()
};
```

### Health Checks

Monitor external integration health:

```rust
if let Some(external) = manager.external() {
    let status = external.status().await;
    
    match status.prometheus_status {
        IntegrationStatus::Connected => println!("Prometheus OK"),
        IntegrationStatus::Error(e) => println!("Prometheus Error: {}", e),
        _ => println!("Prometheus Status: {:?}", status.prometheus_status),
    }
}
```

## Performance Considerations

### Export Batching

Configure appropriate batch sizes:
```rust
let external_config = ExternalObservabilityConfig {
    export_interval_seconds: 15,  // Export every 15 seconds
    batch_size: 1000,            // Batch up to 1000 metrics
    ..Default::default()
};
```

### Memory Usage

Monitor memory usage of observability components:
- Metrics retention: Balance between history and memory
- Log retention: Configure appropriate log rotation
- Trace sampling: Reduce sampling in high-volume scenarios

### Network Impact

Minimize network overhead:
- Use compression for large payloads
- Configure appropriate export intervals
- Use local collectors when possible

## Security

### Authentication

Configure secure authentication:
```rust
let external_config = ExternalObservabilityConfig {
    grafana_api_key: Some(std::env::var("GRAFANA_API_KEY")?),
    // Add other authentication configuration
    ..Default::default()
};
```

### TLS Configuration

Enable TLS for external connections:
```rust
// TLS configuration would be added to the external config
// This is a placeholder for future TLS support
```

### Access Control

Implement proper access control:
- Use service accounts for automated access
- Rotate API keys regularly
- Implement least-privilege access

## Examples

Complete examples are available:
- [Basic External Integration](../examples/external_observability_example.rs)
- [Prometheus Integration](../examples/prometheus_integration.rs)
- [OpenTelemetry Integration](../examples/opentelemetry_integration.rs)
- [Jaeger Tracing](../examples/jaeger_tracing.rs)
- [Grafana Dashboards](../examples/grafana_dashboards.rs)

## API Reference

For detailed API documentation, see:
- [External Observability API](./api-reference.md#external-observability)
- [Prometheus Integration API](./api-reference.md#prometheus)
- [OpenTelemetry Integration API](./api-reference.md#opentelemetry)
- [Jaeger Integration API](./api-reference.md#jaeger)
- [Grafana Integration API](./api-reference.md#grafana)