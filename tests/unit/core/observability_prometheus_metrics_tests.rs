use rustci::core::observability::prometheus_metrics::{
    PrometheusMetrics, MetricsConfig, BusinessMetrics, PerformanceMetrics,
    SystemHealthMetrics, CustomMetricCollector, MetricValue, MetricMetadata,
    SystemResourceCollector, DatabaseMetricsCollector,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[tokio::test]
async fn test_prometheus_metrics_initialization() {
    let config = MetricsConfig {
        service_name: "test-service".to_string(),
        service_version: "1.0.0".to_string(),
        environment: "test".to_string(),
        ..MetricsConfig::default()
    };

    let metrics = PrometheusMetrics::with_config(config);
    let summary = metrics.get_metrics_summary().await;
    
    assert_eq!(summary.service_name, "test-service");
    assert_eq!(summary.service_version, "1.0.0");
    assert_eq!(summary.environment, "test");
}

#[tokio::test]
async fn test_counter_metrics() {
    let metrics = PrometheusMetrics::new();

    // Test basic counter
    metrics.increment_counter("test_counter", 1.0).await;
    metrics.increment_counter("test_counter", 2.0).await;

    // Test counter with labels
    let mut labels = HashMap::new();
    labels.insert("method".to_string(), "GET".to_string());
    labels.insert("status".to_string(), "200".to_string());
    
    metrics.increment_counter_with_labels("http_requests_total", 1.0, labels.clone()).await;
    metrics.increment_counter_with_labels("http_requests_total", 1.0, labels).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("test_counter"));
    assert!(exported.contains("http_requests_total"));
    assert!(exported.contains("method=\"GET\""));
}

#[tokio::test]
async fn test_gauge_metrics() {
    let metrics = PrometheusMetrics::new();

    // Test basic gauge
    metrics.set_gauge("cpu_usage", 75.5).await;
    metrics.set_gauge("memory_usage", 60.2).await;

    // Test gauge with labels
    let mut labels = HashMap::new();
    labels.insert("node".to_string(), "node-1".to_string());
    
    metrics.set_gauge_with_labels("node_cpu_usage", 80.0, labels).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("cpu_usage 75.5"));
    assert!(exported.contains("memory_usage 60.2"));
    assert!(exported.contains("node_cpu_usage"));
    assert!(exported.contains("node=\"node-1\""));
}

#[tokio::test]
async fn test_histogram_metrics() {
    let metrics = PrometheusMetrics::new();

    // Record multiple values
    metrics.record_histogram("request_duration", 0.1).await;
    metrics.record_histogram("request_duration", 0.5).await;
    metrics.record_histogram("request_duration", 1.2).await;
    metrics.record_histogram("request_duration", 0.05).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("request_duration_bucket"));
    assert!(exported.contains("request_duration_sum"));
    assert!(exported.contains("request_duration_count"));
    assert!(exported.contains("le=\"0.1\""));
    assert!(exported.contains("le=\"+Inf\""));
}

#[tokio::test]
async fn test_summary_metrics() {
    let metrics = PrometheusMetrics::new();

    // Record multiple values
    metrics.record_summary("response_size", 1024.0).await;
    metrics.record_summary("response_size", 2048.0).await;
    metrics.record_summary("response_size", 512.0).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("response_size{quantile="));
    assert!(exported.contains("response_size_sum"));
    assert!(exported.contains("response_size_count"));
}

#[tokio::test]
async fn test_business_metrics() {
    let metrics = PrometheusMetrics::new();

    let business_metrics = BusinessMetrics {
        pipeline_success_rate: 0.95,
        average_build_time: Duration::from_secs(120),
        active_users: 150,
        resource_utilization: 0.75,
        error_rate: 0.02,
        throughput_per_minute: 25.5,
    };

    metrics.record_business_metrics(business_metrics).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("rustci_pipeline_success_rate 0.95"));
    assert!(exported.contains("rustci_average_build_time_seconds 120"));
    assert!(exported.contains("rustci_active_users 150"));
    assert!(exported.contains("rustci_resource_utilization 0.75"));
    assert!(exported.contains("rustci_error_rate 0.02"));
    assert!(exported.contains("rustci_throughput_per_minute 25.5"));
}

#[tokio::test]
async fn test_performance_metrics() {
    let metrics = PrometheusMetrics::new();

    let performance_metrics = PerformanceMetrics {
        cpu_usage_percent: 65.5,
        memory_usage_percent: 80.2,
        disk_usage_percent: 45.0,
        network_io_bytes_per_sec: 1024000.0,
        database_connection_pool_usage: 0.6,
        cache_hit_rate: 0.85,
    };

    metrics.record_performance_metrics(performance_metrics).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("rustci_cpu_usage_percent 65.5"));
    assert!(exported.contains("rustci_memory_usage_percent 80.2"));
    assert!(exported.contains("rustci_disk_usage_percent 45"));
    assert!(exported.contains("rustci_network_io_bytes_per_sec 1024000"));
    assert!(exported.contains("rustci_database_connection_pool_usage 0.6"));
    assert!(exported.contains("rustci_cache_hit_rate 0.85"));
}

#[tokio::test]
async fn test_system_health_metrics() {
    let metrics = PrometheusMetrics::new();

    let health_metrics = SystemHealthMetrics {
        uptime_seconds: 3600,
        healthy_nodes: 5,
        total_nodes: 6,
        active_connections: 25,
        queue_depth: 10,
        last_health_check: SystemTime::now(),
    };

    metrics.record_system_health_metrics(health_metrics).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("rustci_uptime_seconds 3600"));
    assert!(exported.contains("rustci_healthy_nodes 5"));
    assert!(exported.contains("rustci_total_nodes 6"));
    assert!(exported.contains("rustci_active_connections 25"));
    assert!(exported.contains("rustci_queue_depth 10"));
    assert!(exported.contains("rustci_last_health_check_timestamp"));
}

#[tokio::test]
async fn test_job_metrics() {
    let metrics = PrometheusMetrics::new();

    let job_id = Uuid::new_v4();
    let duration = Duration::from_secs(45);

    // Record successful job
    metrics.record_job_metrics(job_id, "build", duration, true).await;

    // Record failed job
    let failed_job_id = Uuid::new_v4();
    metrics.record_job_metrics(failed_job_id, "test", Duration::from_secs(30), false).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("rustci_jobs_total"));
    assert!(exported.contains("rustci_job_duration_seconds"));
    assert!(exported.contains("rustci_jobs_successful_total"));
    assert!(exported.contains("rustci_jobs_failed_total"));
    assert!(exported.contains("stage=\"build\""));
    assert!(exported.contains("stage=\"test\""));
    assert!(exported.contains("success=\"true\""));
    assert!(exported.contains("success=\"false\""));
}

#[tokio::test]
async fn test_api_request_metrics() {
    let metrics = PrometheusMetrics::new();

    // Record various API requests
    metrics.record_api_request("GET", "/api/jobs", 200, Duration::from_millis(150)).await;
    metrics.record_api_request("POST", "/api/jobs", 201, Duration::from_millis(300)).await;
    metrics.record_api_request("GET", "/api/jobs/123", 404, Duration::from_millis(50)).await;
    metrics.record_api_request("DELETE", "/api/jobs/456", 500, Duration::from_millis(1000)).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("rustci_http_requests_total"));
    assert!(exported.contains("rustci_http_request_duration_seconds"));
    assert!(exported.contains("rustci_http_responses_by_category"));
    assert!(exported.contains("method=\"GET\""));
    assert!(exported.contains("method=\"POST\""));
    assert!(exported.contains("status_code=\"200\""));
    assert!(exported.contains("status_code=\"404\""));
    assert!(exported.contains("status_category=\"2xx\""));
    assert!(exported.contains("status_category=\"4xx\""));
    assert!(exported.contains("status_category=\"5xx\""));
}

#[tokio::test]
async fn test_custom_collectors() {
    let metrics = PrometheusMetrics::new();

    // Add system resource collector
    metrics.add_custom_collector(Box::new(SystemResourceCollector::new())).await;
    
    // Add database metrics collector
    metrics.add_custom_collector(Box::new(DatabaseMetricsCollector::new())).await;

    // Collect custom metrics
    metrics.collect_custom_metrics().await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("rustci_system_cpu_cores"));
    assert!(exported.contains("rustci_system_memory_total_bytes"));
    assert!(exported.contains("rustci_db_connections_active"));
    assert!(exported.contains("rustci_db_queries_total"));
    assert!(exported.contains("rustci_db_query_duration_seconds"));
}

#[tokio::test]
async fn test_metrics_summary() {
    let metrics = PrometheusMetrics::new();

    // Add various metrics
    metrics.increment_counter("test_counter", 1.0).await;
    metrics.set_gauge("test_gauge", 50.0).await;
    metrics.record_histogram("test_histogram", 0.5).await;
    metrics.record_summary("test_summary", 100.0).await;

    let summary = metrics.get_metrics_summary().await;
    assert!(summary.total_metrics >= 4);
    assert!(summary.counter_count >= 1);
    assert!(summary.gauge_count >= 1);
    assert!(summary.histogram_count >= 1);
    assert!(summary.summary_count >= 1);
}

#[tokio::test]
async fn test_metric_labels_and_grouping() {
    let metrics = PrometheusMetrics::new();

    // Create metrics with different label combinations
    let mut labels1 = HashMap::new();
    labels1.insert("service".to_string(), "api".to_string());
    labels1.insert("version".to_string(), "v1".to_string());

    let mut labels2 = HashMap::new();
    labels2.insert("service".to_string(), "worker".to_string());
    labels2.insert("version".to_string(), "v1".to_string());

    metrics.increment_counter_with_labels("requests_total", 10.0, labels1).await;
    metrics.increment_counter_with_labels("requests_total", 5.0, labels2).await;

    let exported = metrics.export_metrics().await;
    assert!(exported.contains("requests_total{service=\"api\",version=\"v1\"}"));
    assert!(exported.contains("requests_total{service=\"worker\",version=\"v1\"}"));
}

#[tokio::test]
async fn test_metrics_reset() {
    let metrics = PrometheusMetrics::new();

    // Add some metrics
    metrics.increment_counter("test_counter", 5.0).await;
    metrics.set_gauge("test_gauge", 100.0).await;

    let summary_before = metrics.get_metrics_summary().await;
    assert!(summary_before.total_metrics > 0);

    // Reset metrics
    metrics.reset_metrics().await;

    let summary_after = metrics.get_metrics_summary().await;
    assert_eq!(summary_after.total_metrics, 0);
    assert_eq!(summary_after.metadata_count, 0);
}

#[tokio::test]
async fn test_prometheus_export_format() {
    let metrics = PrometheusMetrics::new();

    // Add a counter with help text
    metrics.increment_counter("test_requests_total", 42.0).await;
    
    // Add a gauge
    metrics.set_gauge("test_temperature_celsius", 23.5).await;

    let exported = metrics.export_metrics().await;
    
    // Check service info is included
    assert!(exported.contains("rustci_info{"));
    assert!(exported.contains("service=\"rustci\""));
    
    // Check metrics are properly formatted
    assert!(exported.contains("test_requests_total 42"));
    assert!(exported.contains("test_temperature_celsius 23.5"));
    
    // Check that lines end with newlines
    for line in exported.lines() {
        if !line.is_empty() && !line.starts_with('#') {
            assert!(line.contains(' '), "Metric line should contain space: {}", line);
        }
    }
}

// Mock custom collector for testing
struct MockCustomCollector {
    name: String,
}

impl MockCustomCollector {
    fn new(name: String) -> Self {
        Self { name }
    }
}

impl CustomMetricCollector for MockCustomCollector {
    fn collect_metrics(&self) -> HashMap<String, MetricValue> {
        let mut metrics = HashMap::new();
        metrics.insert("mock_metric".to_string(), MetricValue::Gauge(42.0));
        metrics.insert("mock_counter".to_string(), MetricValue::Counter(100.0));
        metrics
    }

    fn get_metric_metadata(&self) -> Vec<MetricMetadata> {
        vec![
            MetricMetadata {
                name: "mock_metric".to_string(),
                help: "A mock gauge metric".to_string(),
                metric_type: "gauge".to_string(),
                labels: HashMap::new(),
                created_at: SystemTime::now(),
                last_updated: SystemTime::now(),
            }
        ]
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[tokio::test]
async fn test_mock_custom_collector() {
    let metrics = PrometheusMetrics::new();
    
    let mock_collector = MockCustomCollector::new("test_collector".to_string());
    metrics.add_custom_collector(Box::new(mock_collector)).await;
    
    metrics.collect_custom_metrics().await;
    
    let exported = metrics.export_metrics().await;
    assert!(exported.contains("mock_metric 42"));
    assert!(exported.contains("mock_counter 100"));
}