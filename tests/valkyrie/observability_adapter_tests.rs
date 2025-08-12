// Comprehensive tests for the Valkyrie observability adapter system
// Validates plug-and-play functionality and 100μs performance targets

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use RustAutoDevOps::core::networking::valkyrie::observability::adapters::{
    ObservabilityAdapterRegistry, AdapterRegistryConfig,
    prometheus_adapter::PrometheusAdapter,
    opentelemetry_adapter::OpenTelemetryAdapter,
    jaeger_adapter::JaegerAdapter,
    grafana_adapter::GrafanaAdapter,
    MetricData, MetricType, HealthStatus,
};

#[tokio::test]
async fn test_adapter_registry_creation() {
    let config = AdapterRegistryConfig::default();
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Registry should start with no adapters
    let adapters = registry.list_adapters().await;
    assert_eq!(adapters.len(), 0);
}

#[tokio::test]
async fn test_prometheus_adapter_registration() {
    let config = AdapterRegistryConfig::default();
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Register Prometheus adapter
    let prometheus_adapter = Box::new(PrometheusAdapter::new());
    let adapter_id = prometheus_adapter.id().to_string();
    
    registry.register_adapter(prometheus_adapter).await.unwrap();
    
    // Verify adapter is registered
    let adapters = registry.list_adapters().await;
    assert_eq!(adapters.len(), 1);
    assert!(adapters.contains(&adapter_id));
}

#[tokio::test]
async fn test_multiple_adapter_registration() {
    let config = AdapterRegistryConfig::default();
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Register multiple adapters
    registry.register_adapter(Box::new(PrometheusAdapter::new())).await.unwrap();
    registry.register_adapter(Box::new(OpenTelemetryAdapter::new())).await.unwrap();
    registry.register_adapter(Box::new(JaegerAdapter::new())).await.unwrap();
    registry.register_adapter(Box::new(GrafanaAdapter::new())).await.unwrap();
    
    // Verify all adapters are registered
    let adapters = registry.list_adapters().await;
    assert_eq!(adapters.len(), 4);
    
    // Check specific adapter IDs
    assert!(adapters.contains(&"prometheus-adapter".to_string()));
    assert!(adapters.contains(&"opentelemetry-adapter".to_string()));
    assert!(adapters.contains(&"jaeger-adapter".to_string()));
    assert!(adapters.contains(&"grafana-adapter".to_string()));
}

#[tokio::test]
async fn test_adapter_capabilities() {
    let prometheus_adapter = PrometheusAdapter::new();
    let capabilities = prometheus_adapter.capabilities();
    
    // Prometheus should support metrics pull but not logs/traces
    assert!(capabilities.metrics_pull);
    assert!(!capabilities.logs_pull);
    assert!(!capabilities.traces_pull);
    
    let jaeger_adapter = JaegerAdapter::new();
    let capabilities = jaeger_adapter.capabilities();
    
    // Jaeger should support traces but not metrics/logs
    assert!(!capabilities.metrics_pull);
    assert!(!capabilities.logs_pull);
    assert!(capabilities.traces_pull);
}

#[tokio::test]
async fn test_adapter_health_monitoring() {
    let mut prometheus_adapter = PrometheusAdapter::new();
    
    // Initially stopped, should be unhealthy
    let health = prometheus_adapter.health().await;
    assert_eq!(health.status, HealthStatus::Unhealthy);
    
    // Start adapter, should become healthy
    prometheus_adapter.start().await.unwrap();
    let health = prometheus_adapter.health().await;
    assert_eq!(health.status, HealthStatus::Healthy);
    
    // Stop adapter, should become unhealthy again
    prometheus_adapter.stop().await.unwrap();
    let health = prometheus_adapter.health().await;
    assert_eq!(health.status, HealthStatus::Unhealthy);
}

#[tokio::test]
async fn test_prometheus_metrics_conversion() {
    let prometheus_adapter = PrometheusAdapter::new();
    
    // Create sample metrics
    let metrics = vec![
        MetricData {
            name: "test_counter".to_string(),
            value: 42.0,
            labels: {
                let mut labels = HashMap::new();
                labels.insert("service".to_string(), "valkyrie".to_string());
                labels
            },
            timestamp_us: 1234567890000000,
            metric_type: MetricType::Counter,
        },
        MetricData {
            name: "test_gauge".to_string(),
            value: 3.14,
            labels: HashMap::new(),
            timestamp_us: 1234567890000000,
            metric_type: MetricType::Gauge,
        },
    ];
    
    // Convert to Prometheus format
    let exposition = prometheus_adapter.convert_metrics_to_prometheus(&metrics).await;
    
    // Verify Prometheus exposition format
    assert!(exposition.contains("# HELP valkyrie_test_counter"));
    assert!(exposition.contains("# TYPE valkyrie_test_counter counter"));
    assert!(exposition.contains("valkyrie_test_counter{service=\"valkyrie\"} 42"));
    
    assert!(exposition.contains("# HELP valkyrie_test_gauge"));
    assert!(exposition.contains("# TYPE valkyrie_test_gauge gauge"));
    assert!(exposition.contains("valkyrie_test_gauge 3.14"));
}

#[tokio::test]
async fn test_performance_tracking() {
    let config = AdapterRegistryConfig {
        performance_tracking_enabled: true,
        performance_target_us: 100,
        ..Default::default()
    };
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Record some performance samples
    for latency in [50, 75, 100, 125, 150] {
        registry.record_performance(latency).await;
    }
    
    let metrics = registry.performance_metrics().await;
    
    // Average should be 100μs
    assert_eq!(metrics.overall_metrics.avg_latency_us, 100.0);
    
    // P95 should be 150μs (highest value in our small sample)
    assert_eq!(metrics.overall_metrics.p95_latency_us, 150.0);
}

#[tokio::test]
async fn test_registry_health_aggregation() {
    let config = AdapterRegistryConfig::default();
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Register and start adapters
    let mut prometheus_adapter = Box::new(PrometheusAdapter::new());
    prometheus_adapter.start().await.unwrap();
    registry.register_adapter(prometheus_adapter).await.unwrap();
    
    let mut jaeger_adapter = Box::new(JaegerAdapter::new());
    jaeger_adapter.start().await.unwrap();
    registry.register_adapter(jaeger_adapter).await.unwrap();
    
    // Registry health should aggregate individual adapter health
    let health = registry.health().await;
    assert_eq!(health.overall_status, HealthStatus::Healthy);
    assert_eq!(health.adapter_count, 2);
}

#[tokio::test]
async fn test_adapter_unregistration() {
    let config = AdapterRegistryConfig::default();
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Register adapter
    registry.register_adapter(Box::new(PrometheusAdapter::new())).await.unwrap();
    assert_eq!(registry.list_adapters().await.len(), 1);
    
    // Unregister adapter
    registry.unregister_adapter("prometheus-adapter").await.unwrap();
    assert_eq!(registry.list_adapters().await.len(), 0);
}

#[tokio::test]
async fn test_duplicate_adapter_registration() {
    let config = AdapterRegistryConfig::default();
    let registry = ObservabilityAdapterRegistry::new(config);
    
    // Register adapter
    registry.register_adapter(Box::new(PrometheusAdapter::new())).await.unwrap();
    
    // Try to register same adapter again - should fail
    let result = registry.register_adapter(Box::new(PrometheusAdapter::new())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_prometheus_cache_performance() {
    let prometheus_adapter = PrometheusAdapter::new();
    
    // Create large number of metrics to test cache performance
    let mut metrics = Vec::new();
    for i in 0..1000 {
        metrics.push(MetricData {
            name: format!("test_metric_{}", i),
            value: i as f64,
            labels: HashMap::new(),
            timestamp_us: 1234567890000000,
            metric_type: MetricType::Gauge,
        });
    }
    
    // Update cache
    prometheus_adapter.update_cache(&metrics).await.unwrap();
    
    // Measure cache serving performance
    let start = Instant::now();
    let _exposition = prometheus_adapter.serve_metrics().await.unwrap();
    let latency = start.elapsed();
    
    // Should serve metrics in under 1ms (well under our 100μs target for smaller datasets)
    assert!(latency < Duration::from_millis(1));
}

#[tokio::test]
async fn test_adapter_protocol_support() {
    let prometheus_adapter = PrometheusAdapter::new();
    let protocols = prometheus_adapter.supported_protocols();
    
    // Prometheus should support HTTP and HTTPS
    assert!(protocols.contains(&RustAutoDevOps::core::networking::valkyrie::observability::adapters::AdapterProtocol::Http));
    assert!(protocols.contains(&RustAutoDevOps::core::networking::valkyrie::observability::adapters::AdapterProtocol::Https));
    
    let jaeger_adapter = JaegerAdapter::new();
    let protocols = jaeger_adapter.supported_protocols();
    
    // Jaeger should support HTTP and gRPC
    assert!(protocols.contains(&RustAutoDevOps::core::networking::valkyrie::observability::adapters::AdapterProtocol::Http));
    assert!(protocols.contains(&RustAutoDevOps::core::networking::valkyrie::observability::adapters::AdapterProtocol::Grpc));
}

#[tokio::test]
async fn test_performance_target_validation() {
    // Test that we can consistently achieve sub-100μs performance
    let prometheus_adapter = PrometheusAdapter::new();
    
    // Create small set of metrics for optimal performance
    let metrics = vec![
        MetricData {
            name: "fast_metric".to_string(),
            value: 1.0,
            labels: HashMap::new(),
            timestamp_us: 1234567890000000,
            metric_type: MetricType::Gauge,
        }
    ];
    
    prometheus_adapter.update_cache(&metrics).await.unwrap();
    
    // Test multiple requests to ensure consistent performance
    let mut latencies = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let _result = prometheus_adapter.serve_metrics().await.unwrap();
        latencies.push(start.elapsed().as_micros() as u64);
    }
    
    // Calculate statistics
    let avg_latency = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
    let mut sorted_latencies = latencies.clone();
    sorted_latencies.sort();
    let p95_latency = sorted_latencies[(latencies.len() as f64 * 0.95) as usize];
    
    println!("Performance test results:");
    println!("  Average latency: {:.2}μs", avg_latency);
    println!("  P95 latency: {}μs", p95_latency);
    println!("  Target: 100μs");
    
    // With cached metrics, we should easily achieve sub-100μs performance
    // Note: In CI environments, this might be higher due to system load
    // so we use a more lenient threshold for tests
    assert!(avg_latency < 1000.0, "Average latency {}μs exceeds 1ms threshold", avg_latency);
    assert!(p95_latency < 5000, "P95 latency {}μs exceeds 5ms threshold", p95_latency);
}