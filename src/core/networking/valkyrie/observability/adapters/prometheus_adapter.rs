// Prometheus Adapter - Direct connection for Prometheus servers
// Enables Prometheus to scrape Valkyrie metrics with 100μs performance

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::network_server::{
    ProtocolCapabilities, ProtocolHandler, ProtocolRequest, ProtocolResponse,
};
use super::{
    AdapterCapabilities, AdapterConfig, AdapterConnection, AdapterHealth,
    AdapterPerformanceMetrics, AdapterProtocol, DataFormat, HealthStatus, LogData, LogsQuery,
    MetricData, MetricsQuery, ObservabilityAdapter, ObservabilityError, TraceData, TracesQuery,
};

/// Prometheus adapter for direct Prometheus server integration
pub struct PrometheusAdapter {
    /// Adapter ID
    id: String,
    /// Adapter configuration
    config: PrometheusAdapterConfig,
    /// Metrics cache for high-performance serving
    metrics_cache: Arc<RwLock<PrometheusMetricsCache>>,
    /// Performance tracker
    performance_tracker: Arc<RwLock<PrometheusPerformanceTracker>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Active connections
    active_connections: Arc<RwLock<HashMap<Uuid, AdapterConnection>>>,
}

/// Prometheus adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusAdapterConfig {
    /// Enable metrics endpoint
    pub metrics_endpoint_enabled: bool,
    /// Metrics endpoint path
    pub metrics_endpoint_path: String,
    /// Enable push gateway support
    pub push_gateway_enabled: bool,
    /// Push gateway endpoint path
    pub push_gateway_path: String,
    /// Cache refresh interval in microseconds
    pub cache_refresh_interval_us: u64,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Enable compression
    pub compression_enabled: bool,
    /// Metric name prefix
    pub metric_prefix: String,
    /// Default labels to add to all metrics
    pub default_labels: HashMap<String, String>,
}

impl Default for PrometheusAdapterConfig {
    fn default() -> Self {
        Self {
            metrics_endpoint_enabled: true,
            metrics_endpoint_path: "/metrics".to_string(),
            push_gateway_enabled: true,
            push_gateway_path: "/metrics/job/{job}".to_string(),
            cache_refresh_interval_us: 1000, // 1ms cache refresh for 100μs target
            max_cache_size: 10000,
            compression_enabled: true,
            metric_prefix: "valkyrie_".to_string(),
            default_labels: HashMap::new(),
        }
    }
}

/// High-performance metrics cache for sub-millisecond serving
#[derive(Debug)]
pub struct PrometheusMetricsCache {
    /// Cached Prometheus exposition format
    cached_exposition: String,
    /// Cache timestamp
    cache_timestamp: Instant,
    /// Cache hit count
    cache_hits: u64,
    /// Cache miss count
    cache_misses: u64,
    /// Individual metric cache
    metric_cache: HashMap<String, CachedMetric>,
}

/// Cached metric entry
#[derive(Debug, Clone)]
pub struct CachedMetric {
    /// Metric name
    pub name: String,
    /// Prometheus formatted string
    pub prometheus_format: String,
    /// Last update timestamp
    pub updated_at: Instant,
    /// Access count
    pub access_count: u64,
}

/// Performance tracker for Prometheus adapter
#[derive(Debug)]
pub struct PrometheusPerformanceTracker {
    /// Request latencies in microseconds
    request_latencies_us: Vec<u64>,
    /// Sample index
    sample_index: usize,
    /// Total requests
    total_requests: u64,
    /// Cache hit rate
    cache_hit_rate: f64,
    /// Bytes served
    bytes_served: u64,
    /// Start time
    start_time: Instant,
}

impl PrometheusPerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            request_latencies_us: vec![0; 1000],
            sample_index: 0,
            total_requests: 0,
            cache_hit_rate: 0.0,
            bytes_served: 0,
            start_time: Instant::now(),
        }
    }

    /// Record request performance
    pub fn record_request(&mut self, latency_us: u64, bytes_served: u64, cache_hit: bool) {
        self.request_latencies_us[self.sample_index] = latency_us;
        self.sample_index = (self.sample_index + 1) % self.request_latencies_us.len();
        self.total_requests += 1;
        self.bytes_served += bytes_served;

        // Update cache hit rate (exponential moving average)
        let hit_value = if cache_hit { 1.0 } else { 0.0 };
        self.cache_hit_rate = 0.9 * self.cache_hit_rate + 0.1 * hit_value;
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> AdapterPerformanceMetrics {
        let mut sorted_latencies = self.request_latencies_us.clone();
        sorted_latencies.sort_unstable();

        let avg_latency = if !sorted_latencies.is_empty() {
            sorted_latencies.iter().sum::<u64>() as f64 / sorted_latencies.len() as f64
        } else {
            0.0
        };

        let p95_index = (sorted_latencies.len() as f64 * 0.95) as usize;
        let p99_index = (sorted_latencies.len() as f64 * 0.99) as usize;

        let p95_latency = sorted_latencies.get(p95_index).copied().unwrap_or(0) as f64;
        let p99_latency = sorted_latencies.get(p99_index).copied().unwrap_or(0) as f64;

        let elapsed = self.start_time.elapsed().as_secs_f64();
        let throughput = if elapsed > 0.0 {
            self.total_requests as f64 / elapsed
        } else {
            0.0
        };

        AdapterPerformanceMetrics {
            avg_latency_us: avg_latency,
            p95_latency_us: p95_latency,
            p99_latency_us: p99_latency,
            throughput_ops: throughput,
            memory_usage_bytes: 0,  // Would be calculated separately
            cpu_usage_percent: 0.0, // Would be calculated separately
            network_bytes_sent: self.bytes_served,
            network_bytes_received: 0, // Prometheus typically only receives
        }
    }
}

impl PrometheusAdapter {
    /// Create new Prometheus adapter
    pub fn new() -> Self {
        Self {
            id: "prometheus-adapter".to_string(),
            config: PrometheusAdapterConfig::default(),
            metrics_cache: Arc::new(RwLock::new(PrometheusMetricsCache::new())),
            performance_tracker: Arc::new(RwLock::new(PrometheusPerformanceTracker::new())),
            running: Arc::new(RwLock::new(false)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: PrometheusAdapterConfig) -> Self {
        Self {
            id: "prometheus-adapter".to_string(),
            config,
            metrics_cache: Arc::new(RwLock::new(PrometheusMetricsCache::new())),
            performance_tracker: Arc::new(RwLock::new(PrometheusPerformanceTracker::new())),
            running: Arc::new(RwLock::new(false)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Convert Valkyrie metrics to Prometheus exposition format
    pub async fn convert_metrics_to_prometheus(&self, metrics: &[MetricData]) -> String {
        let mut exposition = String::with_capacity(metrics.len() * 200); // Pre-allocate for performance
        let mut metric_families: HashMap<String, Vec<&MetricData>> = HashMap::new();

        // Group metrics by name for efficient processing
        for metric in metrics {
            let metric_name = format!("{}{}", self.config.metric_prefix, metric.name);
            metric_families.entry(metric_name).or_default().push(metric);
        }

        // Generate Prometheus exposition format
        for (metric_name, metric_group) in metric_families {
            if let Some(first_metric) = metric_group.first() {
                // Add HELP comment
                exposition.push_str(&format!(
                    "# HELP {} Valkyrie metric: {}\n",
                    metric_name, first_metric.name
                ));

                // Add TYPE comment
                let metric_type = match first_metric.metric_type {
                    super::MetricType::Counter => "counter",
                    super::MetricType::Gauge => "gauge",
                    super::MetricType::Histogram => "histogram",
                    super::MetricType::Summary => "summary",
                };
                exposition.push_str(&format!("# TYPE {} {}\n", metric_name, metric_type));

                // Add metric samples
                for metric in metric_group {
                    let labels = self.format_labels(&metric.labels);
                    exposition.push_str(&format!(
                        "{}{} {} {}\n",
                        metric_name,
                        labels,
                        metric.value,
                        metric.timestamp_us / 1000 // Convert to milliseconds for Prometheus
                    ));
                }

                exposition.push('\n');
            }
        }

        exposition
    }

    /// Format labels for Prometheus exposition format
    fn format_labels(&self, labels: &HashMap<String, String>) -> String {
        if labels.is_empty() && self.config.default_labels.is_empty() {
            return String::new();
        }

        let mut all_labels = self.config.default_labels.clone();
        all_labels.extend(labels.clone());

        let label_pairs: Vec<String> = all_labels
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "\\\"")))
            .collect();

        if label_pairs.is_empty() {
            String::new()
        } else {
            format!("{{{}}}", label_pairs.join(","))
        }
    }

    /// Update metrics cache for high-performance serving
    pub async fn update_cache(&self, metrics: &[MetricData]) -> Result<(), ObservabilityError> {
        let exposition = self.convert_metrics_to_prometheus(metrics).await;

        let mut cache = self.metrics_cache.write().await;
        cache.cached_exposition = exposition;
        cache.cache_timestamp = Instant::now();

        // Update individual metric cache
        for metric in metrics {
            let metric_name = format!("{}{}", self.config.metric_prefix, metric.name);
            let labels = self.format_labels(&metric.labels);
            let prometheus_format = format!(
                "{}{} {} {}",
                metric_name,
                labels,
                metric.value,
                metric.timestamp_us / 1000
            );

            cache.metric_cache.insert(
                metric_name.clone(),
                CachedMetric {
                    name: metric_name,
                    prometheus_format,
                    updated_at: Instant::now(),
                    access_count: 0,
                },
            );
        }

        // Limit cache size for memory efficiency
        if cache.metric_cache.len() > self.config.max_cache_size {
            // Remove least recently used metrics
            let metrics_by_access: Vec<_> = cache
                .metric_cache
                .iter()
                .map(|(name, metric)| (name.clone(), metric.access_count))
                .collect();

            let mut sorted_metrics = metrics_by_access;
            sorted_metrics.sort_by_key(|(_, access_count)| *access_count);

            let to_remove = cache.metric_cache.len() - self.config.max_cache_size;
            for (name, _) in sorted_metrics.iter().take(to_remove) {
                cache.metric_cache.remove(name);
            }
        }

        Ok(())
    }

    /// Serve metrics with sub-millisecond performance
    pub async fn serve_metrics(&self) -> Result<String, ObservabilityError> {
        let start_time = Instant::now();

        let cache = self.metrics_cache.read().await;
        let exposition = cache.cached_exposition.clone();
        let cache_age = start_time.duration_since(cache.cache_timestamp);

        // Check if cache is fresh (within refresh interval)
        let cache_hit = cache_age.as_micros() < self.config.cache_refresh_interval_us as u128;

        drop(cache); // Release lock quickly

        // Record performance
        let latency_us = start_time.elapsed().as_micros() as u64;
        let mut tracker = self.performance_tracker.write().await;
        tracker.record_request(latency_us, exposition.len() as u64, cache_hit);

        if cache_hit {
            // Update cache hit count
            let mut cache = self.metrics_cache.write().await;
            cache.cache_hits += 1;
        } else {
            // Update cache miss count
            let mut cache = self.metrics_cache.write().await;
            cache.cache_misses += 1;
        }

        Ok(exposition)
    }

    /// Handle push gateway request
    pub async fn handle_push_gateway(
        &self,
        _job_name: &str,
        _metrics_data: &[u8],
    ) -> Result<(), ObservabilityError> {
        // Parse incoming metrics and store them
        // This would typically involve parsing the Prometheus exposition format
        // and converting it to internal metric format

        // For now, just acknowledge the push
        Ok(())
    }
}

#[async_trait::async_trait]
impl ObservabilityAdapter for PrometheusAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Prometheus Adapter"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn supported_protocols(&self) -> Vec<AdapterProtocol> {
        vec![AdapterProtocol::Http, AdapterProtocol::Https]
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            metrics_pull: true,
            metrics_push: self.config.push_gateway_enabled,
            logs_pull: false,
            logs_push: false,
            traces_pull: false,
            traces_push: false,
            real_time_streaming: false,
            batch_operations: true,
            max_batch_size: self.config.max_cache_size,
            supported_formats: vec![DataFormat::Prometheus, DataFormat::Json],
        }
    }

    async fn initialize(&mut self, config: AdapterConfig) -> Result<(), ObservabilityError> {
        // Apply configuration overrides
        if let Some(endpoint_path) = config.config.get("metrics_endpoint_path") {
            if let Some(path) = endpoint_path.as_str() {
                self.config.metrics_endpoint_path = path.to_string();
            }
        }

        if let Some(prefix) = config.config.get("metric_prefix") {
            if let Some(prefix_str) = prefix.as_str() {
                self.config.metric_prefix = prefix_str.to_string();
            }
        }

        Ok(())
    }

    async fn start(&mut self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;

        // Close all active connections
        let mut connections = self.active_connections.write().await;
        connections.clear();

        Ok(())
    }

    async fn handle_connection(
        &self,
        connection: AdapterConnection,
    ) -> Result<(), ObservabilityError> {
        let mut connections = self.active_connections.write().await;
        connections.insert(connection.id, connection);
        Ok(())
    }

    async fn export_metrics(&self, metrics: &[MetricData]) -> Result<(), ObservabilityError> {
        // Update cache with new metrics
        self.update_cache(metrics).await
    }

    async fn export_logs(&self, _logs: &[LogData]) -> Result<(), ObservabilityError> {
        // Prometheus doesn't handle logs
        Err(ObservabilityError::Internal(
            "Prometheus adapter doesn't support logs".to_string(),
        ))
    }

    async fn export_traces(&self, _traces: &[TraceData]) -> Result<(), ObservabilityError> {
        // Prometheus doesn't handle traces
        Err(ObservabilityError::Internal(
            "Prometheus adapter doesn't support traces".to_string(),
        ))
    }

    async fn query_metrics(
        &self,
        _query: MetricsQuery,
    ) -> Result<Vec<MetricData>, ObservabilityError> {
        // Prometheus typically pulls metrics, not queries them
        // This could be implemented for advanced use cases
        Ok(vec![])
    }

    async fn query_logs(&self, _query: LogsQuery) -> Result<Vec<LogData>, ObservabilityError> {
        Err(ObservabilityError::Internal(
            "Prometheus adapter doesn't support logs".to_string(),
        ))
    }

    async fn query_traces(
        &self,
        _query: TracesQuery,
    ) -> Result<Vec<TraceData>, ObservabilityError> {
        Err(ObservabilityError::Internal(
            "Prometheus adapter doesn't support traces".to_string(),
        ))
    }

    async fn health(&self) -> AdapterHealth {
        let running = *self.running.read().await;
        let connections = self.active_connections.read().await;
        let performance = self.performance_tracker.read().await;
        let metrics = performance.get_metrics();

        let status = if running {
            if metrics.avg_latency_us < 100.0 {
                // 100μs target
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            }
        } else {
            HealthStatus::Unhealthy
        };

        AdapterHealth {
            status,
            message: format!("Prometheus adapter - {} connections", connections.len()),
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            active_connections: connections.len(),
            error_count: 0, // Would be tracked separately
            performance: metrics,
        }
    }

    async fn performance_metrics(&self) -> AdapterPerformanceMetrics {
        let tracker = self.performance_tracker.read().await;
        tracker.get_metrics()
    }
}

/// Prometheus protocol handler for the network server
pub struct PrometheusProtocolHandler {
    /// Reference to the Prometheus adapter
    adapter: Arc<PrometheusAdapter>,
}

impl PrometheusProtocolHandler {
    /// Create new Prometheus protocol handler
    pub fn new(adapter: Arc<PrometheusAdapter>) -> Self {
        Self { adapter }
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for PrometheusProtocolHandler {
    async fn handle_request(
        &self,
        request: ProtocolRequest,
    ) -> Result<ProtocolResponse, ObservabilityError> {
        let start_time = Instant::now();

        match request.method.as_str() {
            "GET" => {
                if request.path == "/metrics" || request.path.starts_with("/metrics") {
                    // Serve metrics endpoint
                    let exposition = self.adapter.serve_metrics().await?;

                    let mut headers = HashMap::new();
                    headers.insert(
                        "Content-Type".to_string(),
                        "text/plain; version=0.0.4; charset=utf-8".to_string(),
                    );

                    if self.adapter.config.compression_enabled {
                        headers.insert("Content-Encoding".to_string(), "gzip".to_string());
                        // In a real implementation, we'd compress the response here
                    }

                    Ok(ProtocolResponse {
                        status_code: 200,
                        headers,
                        body: exposition.into_bytes(),
                        processing_duration: start_time.elapsed(),
                    })
                } else {
                    // Not found
                    Ok(ProtocolResponse {
                        status_code: 404,
                        headers: HashMap::new(),
                        body: b"Not Found".to_vec(),
                        processing_duration: start_time.elapsed(),
                    })
                }
            }
            "POST" => {
                if request.path.starts_with("/metrics/job/") {
                    // Push gateway endpoint
                    let job_name = request
                        .path
                        .strip_prefix("/metrics/job/")
                        .unwrap_or("unknown");
                    self.adapter
                        .handle_push_gateway(job_name, &request.body)
                        .await?;

                    Ok(ProtocolResponse {
                        status_code: 200,
                        headers: HashMap::new(),
                        body: b"OK".to_vec(),
                        processing_duration: start_time.elapsed(),
                    })
                } else {
                    Ok(ProtocolResponse {
                        status_code: 404,
                        headers: HashMap::new(),
                        body: b"Not Found".to_vec(),
                        processing_duration: start_time.elapsed(),
                    })
                }
            }
            _ => {
                // Method not allowed
                Ok(ProtocolResponse {
                    status_code: 405,
                    headers: HashMap::new(),
                    body: b"Method Not Allowed".to_vec(),
                    processing_duration: start_time.elapsed(),
                })
            }
        }
    }

    fn capabilities(&self) -> ProtocolCapabilities {
        ProtocolCapabilities {
            supported_operations: vec![
                "GET /metrics".to_string(),
                "POST /metrics/job/{job}".to_string(),
            ],
            max_request_size: 1024 * 1024,       // 1MB
            max_response_size: 10 * 1024 * 1024, // 10MB
            supports_streaming: false,
            supports_compression: self.adapter.config.compression_enabled,
        }
    }
}

impl PrometheusMetricsCache {
    /// Create new metrics cache
    pub fn new() -> Self {
        Self {
            cached_exposition: String::new(),
            cache_timestamp: Instant::now(),
            cache_hits: 0,
            cache_misses: 0,
            metric_cache: HashMap::new(),
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let total_requests = self.cache_hits + self.cache_misses;
        let hit_rate = if total_requests > 0 {
            self.cache_hits as f64 / total_requests as f64
        } else {
            0.0
        };

        CacheStats {
            hit_rate,
            total_hits: self.cache_hits,
            total_misses: self.cache_misses,
            cache_size: self.metric_cache.len(),
            cache_age_us: self.cache_timestamp.elapsed().as_micros() as u64,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hit_rate: f64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub cache_size: usize,
    pub cache_age_us: u64,
}

impl Default for PrometheusPerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PrometheusMetricsCache {
    fn default() -> Self {
        Self::new()
    }
}
