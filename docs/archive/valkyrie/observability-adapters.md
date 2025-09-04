# Valkyrie Protocol Observability Adapters

## Overview

The Valkyrie Protocol observability adapter system provides **plug-and-play network-level integration** for external observability systems, competing with Jenkins' pluggability while targeting **100 microsecond performance**.

External systems like Prometheus, OpenTelemetry, Jaeger, and Grafana can connect directly to Valkyrie through dedicated adapters, enabling seamless integration without code changes.

## Key Features

- 🔌 **Plug-and-Play**: Register adapters dynamically without restarts
- ⚡ **100μs Performance**: Sub-millisecond response times for metrics serving
- 🌐 **Network-Level**: Direct connections from external systems
- 📊 **Multi-Protocol**: HTTP, HTTPS, gRPC, TCP, WebSocket support
- 🎯 **Jenkins-Level Pluggability**: Extensible architecture for custom adapters
- 📈 **Real-Time Monitoring**: Performance tracking and health monitoring

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 External Systems                            │
├─────────────────┬─────────────────┬─────────────────────────┤
│   Prometheus    │  OpenTelemetry  │    Jaeger    │ Grafana │
│   (HTTP/HTTPS)  │     (gRPC)      │   (HTTP)     │ (HTTP)  │
└─────────┬───────┴─────────┬───────┴─────────┬────┴─────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│            Valkyrie Observability Network Server           │
│                    (Port 8082)                             │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌────────┐ │
│  │ Prometheus  │ │OpenTelemetry│ │   Jaeger    │ │Grafana │ │
│  │   Adapter   │ │   Adapter   │ │   Adapter   │ │Adapter │ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └────────┘ │
├─────────────────────────────────────────────────────────────┤
│              Valkyrie Core Observability                   │
│         (Metrics, Logs, Traces, Health)                    │
└─────────────────────────────────────────────────────────────┘
```

## Built-in Adapters

### Prometheus Adapter

**Purpose**: Direct metrics scraping for Prometheus servers
**Protocols**: HTTP, HTTPS
**Endpoints**: `/metrics`, `/metrics/job/{job}` (push gateway)

```rust
use RustAutoDevOps::core::networking::valkyrie::observability::adapters::prometheus_adapter::PrometheusAdapter;

// Create and configure Prometheus adapter
let mut prometheus_adapter = PrometheusAdapter::new();
prometheus_adapter.initialize(config).await?;

// Register with adapter registry
registry.register_adapter(Box::new(prometheus_adapter)).await?;
```

**Features**:
- Sub-millisecond metrics serving with intelligent caching
- Prometheus exposition format conversion
- Push gateway support for batch metrics
- Automatic label management and prefixing

### OpenTelemetry Adapter

**Purpose**: OTLP protocol support for OpenTelemetry collectors
**Protocols**: HTTP, gRPC
**Endpoints**: `/v1/traces`, `/v1/metrics`, `/v1/logs`

```rust
use RustAutoDevOps::core::networking::valkyrie::observability::adapters::opentelemetry_adapter::OpenTelemetryAdapter;

let otel_adapter = OpenTelemetryAdapter::new();
registry.register_adapter(Box::new(otel_adapter)).await?;
```

**Features**:
- OTLP protocol compliance
- Batch and streaming export
- Trace correlation and sampling
- Resource attribute management

### Jaeger Adapter

**Purpose**: Distributed tracing integration with Jaeger
**Protocols**: HTTP, gRPC
**Endpoints**: `/api/traces`, `/api/spans`

```rust
use RustAutoDevOps::core::networking::valkyrie::observability::adapters::jaeger_adapter::JaegerAdapter;

let jaeger_adapter = JaegerAdapter::new();
registry.register_adapter(Box::new(jaeger_adapter)).await?;
```

**Features**:
- Jaeger trace format support
- Span relationship management
- Service topology discovery
- Performance analytics

### Grafana Adapter

**Purpose**: Dashboard queries and data source integration
**Protocols**: HTTP, HTTPS
**Endpoints**: `/query`, `/annotations`, `/search`

```rust
use RustAutoDevOps::core::networking::valkyrie::observability::adapters::grafana_adapter::GrafanaAdapter;

let grafana_adapter = GrafanaAdapter::new();
registry.register_adapter(Box::new(grafana_adapter)).await?;
```

**Features**:
- Grafana data source API
- Time series queries
- Annotation support
- Dashboard templating

## Usage Examples

### Basic Setup

```rust
use RustAutoDevOps::core::networking::valkyrie::observability::adapters::{
    ObservabilityAdapterRegistry, AdapterRegistryConfig,
    prometheus_adapter::PrometheusAdapter,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create high-performance adapter registry
    let config = AdapterRegistryConfig {
        network_server_enabled: true,
        bind_address: "0.0.0.0".to_string(),
        port: 8082,
        max_connections: 10000,
        connection_timeout_us: 100, // 100μs target
        performance_tracking_enabled: true,
        performance_target_us: 100,
    };
    
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Register Prometheus adapter
    let prometheus_adapter = Box::new(PrometheusAdapter::new());
    registry.register_adapter(prometheus_adapter).await?;
    
    // Start network server
    registry.start().await?;
    
    println!("Observability adapters ready!");
    println!("Prometheus: curl http://localhost:8082/metrics");
    
    Ok(())
}
```

### Custom Adapter Development

```rust
use async_trait::async_trait;
use RustAutoDevOps::core::networking::valkyrie::observability::adapters::{
    ObservabilityAdapter, AdapterProtocol, AdapterCapabilities,
    MetricData, LogData, TraceData, ObservabilityError,
};

pub struct CustomAdapter {
    id: String,
    // Custom adapter state
}

#[async_trait]
impl ObservabilityAdapter for CustomAdapter {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "Custom Adapter" }
    fn version(&self) -> &str { "1.0.0" }
    
    fn supported_protocols(&self) -> Vec<AdapterProtocol> {
        vec![AdapterProtocol::Http, AdapterProtocol::Custom("my-protocol".to_string())]
    }
    
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            metrics_pull: true,
            metrics_push: true,
            logs_pull: true,
            logs_push: true,
            traces_pull: false,
            traces_push: false,
            real_time_streaming: true,
            batch_operations: true,
            max_batch_size: 1000,
            supported_formats: vec![DataFormat::Json, DataFormat::Custom("my-format".to_string())],
        }
    }
    
    async fn export_metrics(&self, metrics: &[MetricData]) -> Result<(), ObservabilityError> {
        // Custom metrics export logic
        println!("Exporting {} metrics to custom system", metrics.len());
        Ok(())
    }
    
    // Implement other required methods...
}
```

## Performance Optimization

### Caching Strategy

The adapter system uses intelligent caching to achieve 100μs performance targets:

```rust
// Prometheus adapter with optimized caching
let config = PrometheusAdapterConfig {
    cache_refresh_interval_us: 1000, // 1ms cache refresh
    max_cache_size: 10000,
    compression_enabled: true,
    ..Default::default()
};

let prometheus_adapter = PrometheusAdapter::with_config(config);
```

### Memory Pool Optimization

```rust
// Configure memory pools for zero-allocation serving
let config = AdapterRegistryConfig {
    max_connections: 10000,
    connection_timeout_us: 100,
    performance_target_us: 100,
    ..Default::default()
};
```

### SIMD Operations

The adapter system uses SIMD operations for high-performance data processing:

- Vectorized metric aggregation
- Parallel label processing
- Optimized serialization

## Monitoring and Health

### Health Monitoring

```rust
// Check overall registry health
let health = registry.health().await;
println!("Status: {:?}", health.overall_status);
println!("Adapters: {}", health.adapter_count);

// Check individual adapter health
for (adapter_id, adapter_health) in health.adapter_healths {
    println!("{}: {:?} ({} connections)", 
        adapter_id, 
        adapter_health.status,
        adapter_health.active_connections
    );
}
```

### Performance Metrics

```rust
// Get performance metrics
let metrics = registry.performance_metrics().await;
println!("Average latency: {:.2}μs", metrics.overall_metrics.avg_latency_us);
println!("P95 latency: {:.2}μs", metrics.overall_metrics.p95_latency_us);
println!("P99 latency: {:.2}μs", metrics.overall_metrics.p99_latency_us);
println!("Throughput: {:.2} ops/sec", metrics.overall_metrics.throughput_ops);
```

## Configuration

### Registry Configuration

```yaml
# config/valkyrie.yaml
observability:
  adapters:
    network_server_enabled: true
    bind_address: "0.0.0.0"
    port: 8082
    max_connections: 10000
    connection_timeout_us: 100
    performance_tracking_enabled: true
    performance_target_us: 100
```

### Adapter-Specific Configuration

```yaml
# Prometheus adapter configuration
prometheus:
  metrics_endpoint_enabled: true
  metrics_endpoint_path: "/metrics"
  push_gateway_enabled: true
  cache_refresh_interval_us: 1000
  max_cache_size: 10000
  compression_enabled: true
  metric_prefix: "valkyrie_"
  default_labels:
    service: "valkyrie"
    version: "1.0.0"

# OpenTelemetry adapter configuration
opentelemetry:
  otlp_endpoint_enabled: true
  traces_endpoint: "/v1/traces"
  metrics_endpoint: "/v1/metrics"
  logs_endpoint: "/v1/logs"
  batch_timeout_ms: 1000
  max_batch_size: 512
```

## External System Integration

### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'valkyrie'
    static_configs:
      - targets: ['localhost:8082']
    metrics_path: '/metrics'
    scrape_interval: 15s
    scrape_timeout: 5s
```

### OpenTelemetry Collector Configuration

```yaml
# otel-collector.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: localhost:8082
      http:
        endpoint: localhost:8082

exporters:
  jaeger:
    endpoint: jaeger:14250

service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [jaeger]
```

### Grafana Data Source

```json
{
  "name": "Valkyrie",
  "type": "prometheus",
  "url": "http://localhost:8082",
  "access": "proxy",
  "isDefault": true
}
```

## Best Practices

### Performance Optimization

1. **Use Caching**: Enable caching for frequently accessed metrics
2. **Batch Operations**: Group multiple operations for better throughput
3. **Connection Pooling**: Reuse connections to minimize overhead
4. **Compression**: Enable compression for large payloads

### Security

1. **TLS Encryption**: Use HTTPS for production deployments
2. **Authentication**: Implement API key or token-based auth
3. **Rate Limiting**: Protect against abuse with rate limiting
4. **IP Whitelisting**: Restrict access to known systems

### Monitoring

1. **Health Checks**: Monitor adapter health continuously
2. **Performance Metrics**: Track latency and throughput
3. **Error Rates**: Monitor and alert on error conditions
4. **Resource Usage**: Track memory and CPU consumption

## Troubleshooting

### Common Issues

**High Latency**:
- Check cache configuration
- Verify network connectivity
- Monitor system resources

**Connection Failures**:
- Verify port availability
- Check firewall settings
- Validate adapter configuration

**Memory Issues**:
- Adjust cache sizes
- Monitor connection counts
- Check for memory leaks

### Debug Mode

```rust
// Enable debug logging
let config = AdapterRegistryConfig {
    debug_enabled: true,
    log_level: "debug",
    ..Default::default()
};
```

## Roadmap

- **Custom Protocol Support**: Plugin system for custom protocols
- **Auto-Discovery**: Automatic adapter discovery and registration
- **Load Balancing**: Built-in load balancing for high availability
- **Clustering**: Multi-node adapter registry clustering
- **Machine Learning**: AI-powered performance optimization

## Contributing

To contribute a new adapter:

1. Implement the `ObservabilityAdapter` trait
2. Add protocol handler if needed
3. Include comprehensive tests
4. Update documentation
5. Submit pull request

See the [Custom Adapter Development](#custom-adapter-development) section for details.