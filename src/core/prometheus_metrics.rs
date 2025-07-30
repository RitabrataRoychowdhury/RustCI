use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Prometheus metrics exporter
pub struct PrometheusMetrics {
    metrics: Arc<RwLock<HashMap<String, MetricValue>>>,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(f64),
    Gauge(f64),
    Histogram {
        sum: f64,
        count: u64,
        buckets: Vec<(f64, u64)>,
    },
}

impl PrometheusMetrics {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Increment a counter metric
    pub async fn increment_counter(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        let entry = metrics
            .entry(name.to_string())
            .or_insert(MetricValue::Counter(0.0));
        if let MetricValue::Counter(ref mut current) = entry {
            *current += value;
        }
    }

    /// Set a gauge metric
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(name.to_string(), MetricValue::Gauge(value));
    }

    /// Record a histogram value
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        let entry = metrics
            .entry(name.to_string())
            .or_insert(MetricValue::Histogram {
                sum: 0.0,
                count: 0,
                buckets: vec![
                    (0.005, 0),
                    (0.01, 0),
                    (0.025, 0),
                    (0.05, 0),
                    (0.1, 0),
                    (0.25, 0),
                    (0.5, 0),
                    (1.0, 0),
                    (2.5, 0),
                    (5.0, 0),
                    (10.0, 0),
                    (f64::INFINITY, 0),
                ],
            });

        if let MetricValue::Histogram {
            ref mut sum,
            ref mut count,
            ref mut buckets,
        } = entry
        {
            *sum += value;
            *count += 1;

            for (bucket_le, bucket_count) in buckets.iter_mut() {
                if value <= *bucket_le {
                    *bucket_count += 1;
                }
            }
        }
    }

    /// Export metrics in Prometheus format
    pub async fn export_metrics(&self) -> String {
        let metrics = self.metrics.read().await;
        let mut output = String::new();

        for (name, value) in metrics.iter() {
            match value {
                MetricValue::Counter(val) => {
                    output.push_str(&format!("# TYPE {} counter\n", name));
                    output.push_str(&format!("{} {}\n", name, val));
                }
                MetricValue::Gauge(val) => {
                    output.push_str(&format!("# TYPE {} gauge\n", name));
                    output.push_str(&format!("{} {}\n", name, val));
                }
                MetricValue::Histogram {
                    sum,
                    count,
                    buckets,
                } => {
                    output.push_str(&format!("# TYPE {} histogram\n", name));

                    for (le, bucket_count) in buckets {
                        if le.is_infinite() {
                            output.push_str(&format!(
                                "{}_bucket{{le=\"+Inf\"}} {}\n",
                                name, bucket_count
                            ));
                        } else {
                            output.push_str(&format!(
                                "{}_bucket{{le=\"{}\"}} {}\n",
                                name, le, bucket_count
                            ));
                        }
                    }

                    output.push_str(&format!("{}_sum {}\n", name, sum));
                    output.push_str(&format!("{}_count {}\n", name, count));
                }
            }
            output.push('\n');
        }

        output
    }

    /// Create router for metrics endpoint
    pub fn create_router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(self)
    }
}

impl Default for PrometheusMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler for /metrics endpoint
async fn metrics_handler(State(metrics): State<Arc<PrometheusMetrics>>) -> impl IntoResponse {
    let metrics_output = metrics.export_metrics().await;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
        .body(metrics_output)
        .unwrap()
}

/// Initialize default RustCI metrics
pub async fn initialize_rustci_metrics(metrics: Arc<PrometheusMetrics>) {
    info!("Initializing RustCI Prometheus metrics");

    // Initialize runner metrics
    metrics.set_gauge("rustci_runners_total", 0.0).await;
    metrics.set_gauge("rustci_runners_active", 0.0).await;
    metrics.set_gauge("rustci_runners_idle", 0.0).await;
    metrics.set_gauge("rustci_runners_failed", 0.0).await;

    // Initialize job metrics
    metrics.set_gauge("rustci_jobs_total", 0.0).await;
    metrics.set_gauge("rustci_jobs_running", 0.0).await;
    metrics.set_gauge("rustci_jobs_queued", 0.0).await;
    metrics.set_gauge("rustci_jobs_completed", 0.0).await;
    metrics.set_gauge("rustci_jobs_failed", 0.0).await;

    // Initialize cluster metrics
    metrics.set_gauge("rustci_cluster_nodes_total", 0.0).await;
    metrics.set_gauge("rustci_cluster_nodes_healthy", 0.0).await;
    metrics
        .set_gauge("rustci_cluster_nodes_unhealthy", 0.0)
        .await;

    // Initialize pipeline metrics
    metrics.set_gauge("rustci_pipelines_total", 0.0).await;
    metrics.set_gauge("rustci_pipelines_active", 0.0).await;

    // Initialize system metrics
    metrics
        .set_gauge("rustci_system_cpu_usage_percent", 0.0)
        .await;
    metrics
        .set_gauge("rustci_system_memory_usage_percent", 0.0)
        .await;
    metrics
        .set_gauge("rustci_system_disk_usage_percent", 0.0)
        .await;

    // Initialize HTTP request metrics
    metrics
        .increment_counter("rustci_http_requests_total", 0.0)
        .await;
    metrics
        .record_histogram("rustci_http_request_duration_seconds", 0.0)
        .await;

    info!("RustCI Prometheus metrics initialized successfully");
}
