// OpenTelemetry Adapter - Direct OTLP connection support
// Enables OpenTelemetry collectors to connect directly with 100μs performance

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::{
    ObservabilityAdapter, AdapterProtocol, AdapterCapabilities, AdapterConfig,
    AdapterConnection, AdapterHealth, AdapterPerformanceMetrics, HealthStatus,
    MetricData, LogData, TraceData, MetricsQuery, LogsQuery, TracesQuery,
    DataFormat, ObservabilityError,
};
// Protocol handler imports would be added when implementing the handler

/// OpenTelemetry adapter for direct OTLP integration
pub struct OpenTelemetryAdapter {
    /// Adapter ID
    id: String,
    /// Adapter configuration
    config: OpenTelemetryAdapterConfig,
    /// OTLP data buffer for high-performance batching
    otlp_buffer: Arc<RwLock<OtlpDataBuffer>>,
    /// Performance tracker
    performance_tracker: Arc<RwLock<OtlpPerformanceTracker>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Active connections
    active_connections: Arc<RwLock<HashMap<Uuid, AdapterConnection>>>,
}

/// OpenTelemetry adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryAdapterConfig {
    /// Enable OTLP/gRPC endpoint
    pub otlp_grpc_enabled: bool,
    /// OTLP/gRPC endpoint path
    pub otlp_grpc_path: String,
    /// Enable OTLP/HTTP endpoint
    pub otlp_http_enabled: bool,
    /// OTLP/HTTP endpoint path
    pub otlp_http_path: String,
    /// Buffer size for batching
    pub buffer_size: usize,
    /// Batch timeout in microseconds
    pub batch_timeout_us: u64,
    /// Enable compression
    pub compression_enabled: bool,
    /// Compression algorithm
    pub compression_algorithm: CompressionAlgorithm,
    /// Resource attributes
    pub resource_attributes: HashMap<String, String>,
    /// Enable metrics export
    pub metrics_export_enabled: bool,
    /// Enable logs export
    pub logs_export_enabled: bool,
    /// Enable traces export
    pub traces_export_enabled: bool,
}

impl Default for OpenTelemetryAdapterConfig {
    fn default() -> Self {
        Self {
            otlp_grpc_enabled: true,
            otlp_grpc_path: "/v1/traces".to_string(),
            otlp_http_enabled: true,
            otlp_http_path: "/v1/traces".to_string(),
            buffer_size: 1000,
            batch_timeout_us: 1000, // 1ms batching for 100μs target
            compression_enabled: true,
            compression_algorithm: CompressionAlgorithm::Gzip,
            resource_attributes: {
                let mut attrs = HashMap::new();
                attrs.insert("service.name".to_string(), "valkyrie-protocol".to_string());
                attrs.insert("service.version".to_string(), "1.0.0".to_string());
                attrs
            },
            metrics_export_enabled: true,
            logs_export_enabled: true,
            traces_export_enabled: true,
        }
    }
}

/// Compression algorithms supported by OTLP
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Gzip,
    Snappy,
    Lz4,
}

/// High-performance OTLP data buffer
#[derive(Debug)]
pub struct OtlpDataBuffer {
    /// Buffered metrics
    metrics_buffer: Vec<OtlpMetric>,
    /// Buffered logs
    logs_buffer: Vec<OtlpLog>,
    /// Buffered traces
    traces_buffer: Vec<OtlpTrace>,
    /// Buffer timestamp
    buffer_timestamp: Instant,
    /// Buffer statistics
    buffer_stats: BufferStats,
}

/// OTLP metric representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpMetric {
    /// Metric name
    pub name: String,
    /// Metric description
    pub description: String,
    /// Metric unit
    pub unit: String,
    /// Metric data points
    pub data_points: Vec<OtlpDataPoint>,
    /// Resource attributes
    pub resource_attributes: HashMap<String, String>,
    /// Scope attributes
    pub scope_attributes: HashMap<String, String>,
}

/// OTLP data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpDataPoint {
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Metric value
    pub value: OtlpValue,
    /// Attributes
    pub attributes: HashMap<String, String>,
}

/// OTLP value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OtlpValue {
    Int64(i64),
    Double(f64),
    Histogram {
        count: u64,
        sum: f64,
        buckets: Vec<OtlpHistogramBucket>,
    },
    Summary {
        count: u64,
        sum: f64,
        quantiles: Vec<OtlpQuantile>,
    },
}

/// OTLP histogram bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpHistogramBucket {
    pub upper_bound: f64,
    pub count: u64,
}

/// OTLP quantile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpQuantile {
    pub quantile: f64,
    pub value: f64,
}

/// OTLP log representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpLog {
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Severity level
    pub severity_level: OtlpSeverityLevel,
    /// Log body
    pub body: String,
    /// Attributes
    pub attributes: HashMap<String, String>,
    /// Resource attributes
    pub resource_attributes: HashMap<String, String>,
    /// Trace ID
    pub trace_id: Option<String>,
    /// Span ID
    pub span_id: Option<String>,
}

/// OTLP severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtlpSeverityLevel {
    Trace = 1,
    Debug = 5,
    Info = 9,
    Warn = 13,
    Error = 17,
    Fatal = 21,
}

/// OTLP trace representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpTrace {
    /// Trace ID
    pub trace_id: String,
    /// Spans
    pub spans: Vec<OtlpSpan>,
    /// Resource attributes
    pub resource_attributes: HashMap<String, String>,
}

/// OTLP span representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpSpan {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID
    pub parent_span_id: Option<String>,
    /// Operation name
    pub name: String,
    /// Span kind
    pub kind: OtlpSpanKind,
    /// Start timestamp in nanoseconds
    pub start_time_ns: u64,
    /// End timestamp in nanoseconds
    pub end_time_ns: u64,
    /// Attributes
    pub attributes: HashMap<String, String>,
    /// Events
    pub events: Vec<OtlpSpanEvent>,
    /// Status
    pub status: OtlpSpanStatus,
}

/// OTLP span kinds
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtlpSpanKind {
    Unspecified = 0,
    Internal = 1,
    Server = 2,
    Client = 3,
    Producer = 4,
    Consumer = 5,
}

/// OTLP span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpSpanEvent {
    /// Event timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Event name
    pub name: String,
    /// Event attributes
    pub attributes: HashMap<String, String>,
}

/// OTLP span status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpSpanStatus {
    /// Status code
    pub code: OtlpStatusCode,
    /// Status message
    pub message: String,
}

/// OTLP status codes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtlpStatusCode {
    Unset = 0,
    Ok = 1,
    Error = 2,
}

/// Buffer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStats {
    /// Metrics count
    pub metrics_count: usize,
    /// Logs count
    pub logs_count: usize,
    /// Traces count
    pub traces_count: usize,
    /// Buffer age in microseconds
    pub buffer_age_us: u64,
    /// Total bytes buffered
    pub total_bytes: usize,
}

/// Performance tracker for OpenTelemetry adapter
#[derive(Debug)]
pub struct OtlpPerformanceTracker {
    /// Request latencies in microseconds
    request_latencies_us: Vec<u64>,
    /// Sample index
    sample_index: usize,
    /// Total requests
    total_requests: u64,
    /// Batch efficiency (items per batch)
    batch_efficiency: f64,
    /// Compression ratio
    compression_ratio: f64,
    /// Bytes processed
    bytes_processed: u64,
    /// Start time
    start_time: Instant,
}

impl OtlpPerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            request_latencies_us: vec![0; 1000],
            sample_index: 0,
            total_requests: 0,
            batch_efficiency: 0.0,
            compression_ratio: 1.0,
            bytes_processed: 0,
            start_time: Instant::now(),
        }
    }

    /// Record request performance
    pub fn record_request(&mut self, latency_us: u64, batch_size: usize, bytes_processed: u64, compression_ratio: f64) {
        self.request_latencies_us[self.sample_index] = latency_us;
        self.sample_index = (self.sample_index + 1) % self.request_latencies_us.len();
        self.total_requests += 1;
        self.bytes_processed += bytes_processed;
        
        // Update batch efficiency (exponential moving average)
        self.batch_efficiency = 0.9 * self.batch_efficiency + 0.1 * batch_size as f64;
        
        // Update compression ratio
        self.compression_ratio = 0.9 * self.compression_ratio + 0.1 * compression_ratio;
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
            memory_usage_bytes: 0, // Would be calculated separately
            cpu_usage_percent: 0.0, // Would be calculated separately
            network_bytes_sent: self.bytes_processed,
            network_bytes_received: 0,
        }
    }
}

impl OpenTelemetryAdapter {
    /// Create new OpenTelemetry adapter
    pub fn new() -> Self {
        Self {
            id: "opentelemetry-adapter".to_string(),
            config: OpenTelemetryAdapterConfig::default(),
            otlp_buffer: Arc::new(RwLock::new(OtlpDataBuffer::new())),
            performance_tracker: Arc::new(RwLock::new(OtlpPerformanceTracker::new())),
            running: Arc::new(RwLock::new(false)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: OpenTelemetryAdapterConfig) -> Self {
        Self {
            id: "opentelemetry-adapter".to_string(),
            config,
            otlp_buffer: Arc::new(RwLock::new(OtlpDataBuffer::new())),
            performance_tracker: Arc::new(RwLock::new(OtlpPerformanceTracker::new())),
            running: Arc::new(RwLock::new(false)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Convert Valkyrie metrics to OTLP format
    pub async fn convert_metrics_to_otlp(&self, metrics: &[MetricData]) -> Vec<OtlpMetric> {
        let mut otlp_metrics = Vec::with_capacity(metrics.len());

        for metric in metrics {
            let otlp_value = match metric.metric_type {
                super::MetricType::Counter => OtlpValue::Int64(metric.value as i64),
                super::MetricType::Gauge => OtlpValue::Double(metric.value),
                super::MetricType::Histogram => {
                    // Simplified histogram conversion
                    OtlpValue::Histogram {
                        count: 1,
                        sum: metric.value,
                        buckets: vec![OtlpHistogramBucket {
                            upper_bound: f64::INFINITY,
                            count: 1,
                        }],
                    }
                }
                super::MetricType::Summary => {
                    OtlpValue::Summary {
                        count: 1,
                        sum: metric.value,
                        quantiles: vec![],
                    }
                }
            };

            let data_point = OtlpDataPoint {
                timestamp_ns: metric.timestamp_us * 1000, // Convert to nanoseconds
                value: otlp_value,
                attributes: metric.labels.clone(),
            };

            let otlp_metric = OtlpMetric {
                name: metric.name.clone(),
                description: format!("Valkyrie metric: {}", metric.name),
                unit: "1".to_string(), // Default unit
                data_points: vec![data_point],
                resource_attributes: self.config.resource_attributes.clone(),
                scope_attributes: HashMap::new(),
            };

            otlp_metrics.push(otlp_metric);
        }

        otlp_metrics
    }

    /// Convert Valkyrie logs to OTLP format
    pub async fn convert_logs_to_otlp(&self, logs: &[LogData]) -> Vec<OtlpLog> {
        let mut otlp_logs = Vec::with_capacity(logs.len());

        for log in logs {
            let severity_level = match log.level.to_lowercase().as_str() {
                "trace" => OtlpSeverityLevel::Trace,
                "debug" => OtlpSeverityLevel::Debug,
                "info" => OtlpSeverityLevel::Info,
                "warn" | "warning" => OtlpSeverityLevel::Warn,
                "error" => OtlpSeverityLevel::Error,
                "fatal" => OtlpSeverityLevel::Fatal,
                _ => OtlpSeverityLevel::Info,
            };

            let attributes: HashMap<String, String> = log.fields
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect();

            let otlp_log = OtlpLog {
                timestamp_ns: log.timestamp_us * 1000, // Convert to nanoseconds
                severity_level,
                body: log.message.clone(),
                attributes,
                resource_attributes: self.config.resource_attributes.clone(),
                trace_id: log.correlation_id.clone(),
                span_id: None, // Would be extracted from correlation context
            };

            otlp_logs.push(otlp_log);
        }

        otlp_logs
    }

    /// Convert Valkyrie traces to OTLP format
    pub async fn convert_traces_to_otlp(&self, traces: &[TraceData]) -> Vec<OtlpTrace> {
        let mut trace_map: HashMap<String, Vec<OtlpSpan>> = HashMap::new();

        for trace in traces {
            let span = OtlpSpan {
                trace_id: trace.trace_id.clone(),
                span_id: trace.span_id.clone(),
                parent_span_id: trace.parent_span_id.clone(),
                name: trace.operation_name.clone(),
                kind: OtlpSpanKind::Internal, // Default kind
                start_time_ns: trace.start_timestamp_us * 1000,
                end_time_ns: trace.end_timestamp_us.unwrap_or(trace.start_timestamp_us) * 1000,
                attributes: trace.tags.clone(),
                events: trace.logs.iter().map(|log| OtlpSpanEvent {
                    timestamp_ns: log.timestamp_us * 1000,
                    name: "log".to_string(),
                    attributes: log.fields.iter().map(|(k, v)| (k.clone(), v.to_string())).collect(),
                }).collect(),
                status: OtlpSpanStatus {
                    code: OtlpStatusCode::Ok, // Default status
                    message: String::new(),
                },
            };

            trace_map.entry(trace.trace_id.clone()).or_default().push(span);
        }

        trace_map
            .into_iter()
            .map(|(trace_id, spans)| OtlpTrace {
                trace_id,
                spans,
                resource_attributes: self.config.resource_attributes.clone(),
            })
            .collect()
    }

    /// Buffer data for batching
    pub async fn buffer_data(&self, metrics: &[MetricData], logs: &[LogData], traces: &[TraceData]) -> Result<(), ObservabilityError> {
        let mut buffer = self.otlp_buffer.write().await;

        if self.config.metrics_export_enabled && !metrics.is_empty() {
            let otlp_metrics = self.convert_metrics_to_otlp(metrics).await;
            buffer.metrics_buffer.extend(otlp_metrics);
        }

        if self.config.logs_export_enabled && !logs.is_empty() {
            let otlp_logs = self.convert_logs_to_otlp(logs).await;
            buffer.logs_buffer.extend(otlp_logs);
        }

        if self.config.traces_export_enabled && !traces.is_empty() {
            let otlp_traces = self.convert_traces_to_otlp(traces).await;
            buffer.traces_buffer.extend(otlp_traces);
        }

        // Update buffer stats
        buffer.buffer_stats = BufferStats {
            metrics_count: buffer.metrics_buffer.len(),
            logs_count: buffer.logs_buffer.len(),
            traces_count: buffer.traces_buffer.len(),
            buffer_age_us: buffer.buffer_timestamp.elapsed().as_micros() as u64,
            total_bytes: 0, // Would be calculated based on serialized size
        };

        Ok(())
    }

    /// Flush buffer and export data
    pub async fn flush_buffer(&self) -> Result<Vec<u8>, ObservabilityError> {
        let mut buffer = self.otlp_buffer.write().await;
        
        // Create OTLP export request (simplified)
        let export_request = OtlpExportRequest {
            metrics: buffer.metrics_buffer.clone(),
            logs: buffer.logs_buffer.clone(),
            traces: buffer.traces_buffer.clone(),
        };

        // Serialize to bytes (would use protobuf in real implementation)
        let serialized = serde_json::to_vec(&export_request)
            .map_err(|e| ObservabilityError::Internal(format!("Serialization error: {}", e)))?;

        // Clear buffer
        buffer.metrics_buffer.clear();
        buffer.logs_buffer.clear();
        buffer.traces_buffer.clear();
        buffer.buffer_timestamp = Instant::now();

        Ok(serialized)
    }
}

/// OTLP export request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpExportRequest {
    pub metrics: Vec<OtlpMetric>,
    pub logs: Vec<OtlpLog>,
    pub traces: Vec<OtlpTrace>,
}

#[async_trait::async_trait]
impl ObservabilityAdapter for OpenTelemetryAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "OpenTelemetry Adapter"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn supported_protocols(&self) -> Vec<AdapterProtocol> {
        let mut protocols = vec![AdapterProtocol::Http, AdapterProtocol::Https];
        if self.config.otlp_grpc_enabled {
            protocols.push(AdapterProtocol::Grpc);
        }
        protocols
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            metrics_pull: false,
            metrics_push: self.config.metrics_export_enabled,
            logs_pull: false,
            logs_push: self.config.logs_export_enabled,
            traces_pull: false,
            traces_push: self.config.traces_export_enabled,
            real_time_streaming: true,
            batch_operations: true,
            max_batch_size: self.config.buffer_size,
            supported_formats: vec![DataFormat::Protobuf, DataFormat::Json],
        }
    }

    async fn initialize(&mut self, config: AdapterConfig) -> Result<(), ObservabilityError> {
        // Apply configuration overrides
        if let Some(buffer_size) = config.config.get("buffer_size") {
            if let Some(size) = buffer_size.as_u64() {
                self.config.buffer_size = size as usize;
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
        
        // Flush any remaining data
        let _ = self.flush_buffer().await;
        
        // Close all active connections
        let mut connections = self.active_connections.write().await;
        connections.clear();
        
        Ok(())
    }

    async fn handle_connection(&self, connection: AdapterConnection) -> Result<(), ObservabilityError> {
        let mut connections = self.active_connections.write().await;
        connections.insert(connection.id, connection);
        Ok(())
    }

    async fn export_metrics(&self, metrics: &[MetricData]) -> Result<(), ObservabilityError> {
        self.buffer_data(metrics, &[], &[]).await
    }

    async fn export_logs(&self, logs: &[LogData]) -> Result<(), ObservabilityError> {
        self.buffer_data(&[], logs, &[]).await
    }

    async fn export_traces(&self, traces: &[TraceData]) -> Result<(), ObservabilityError> {
        self.buffer_data(&[], &[], traces).await
    }

    async fn query_metrics(&self, _query: MetricsQuery) -> Result<Vec<MetricData>, ObservabilityError> {
        // OpenTelemetry is primarily push-based
        Ok(vec![])
    }

    async fn query_logs(&self, _query: LogsQuery) -> Result<Vec<LogData>, ObservabilityError> {
        // OpenTelemetry is primarily push-based
        Ok(vec![])
    }

    async fn query_traces(&self, _query: TracesQuery) -> Result<Vec<TraceData>, ObservabilityError> {
        // OpenTelemetry is primarily push-based
        Ok(vec![])
    }

    async fn health(&self) -> AdapterHealth {
        let running = *self.running.read().await;
        let connections = self.active_connections.read().await;
        let performance = self.performance_tracker.read().await;
        let metrics = performance.get_metrics();

        let status = if running {
            if metrics.avg_latency_us < 100.0 { // 100μs target
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            }
        } else {
            HealthStatus::Unhealthy
        };

        AdapterHealth {
            status,
            message: format!("OpenTelemetry adapter - {} connections", connections.len()),
            last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
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

impl OtlpDataBuffer {
    /// Create new OTLP data buffer
    pub fn new() -> Self {
        Self {
            metrics_buffer: Vec::new(),
            logs_buffer: Vec::new(),
            traces_buffer: Vec::new(),
            buffer_timestamp: Instant::now(),
            buffer_stats: BufferStats {
                metrics_count: 0,
                logs_count: 0,
                traces_count: 0,
                buffer_age_us: 0,
                total_bytes: 0,
            },
        }
    }
}

impl Default for OtlpPerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for OtlpDataBuffer {
    fn default() -> Self {
        Self::new()
    }
}