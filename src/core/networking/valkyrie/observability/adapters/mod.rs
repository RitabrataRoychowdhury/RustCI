// Valkyrie Protocol Observability Adapters - Plug-and-Play Network-Level Integration
// Competing with Jenkins' pluggability at the network level with 100μs performance

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::ObservabilityError;

pub mod prometheus_adapter;
pub mod opentelemetry_adapter;
pub mod jaeger_adapter;
pub mod grafana_adapter;
pub mod custom_adapter;
pub mod network_server;

/// Plug-and-play observability adapter trait
/// External systems implement this to connect directly to Valkyrie
#[async_trait::async_trait]
pub trait ObservabilityAdapter: Send + Sync {
    /// Adapter unique identifier
    fn id(&self) -> &str;
    
    /// Adapter name for display
    fn name(&self) -> &str;
    
    /// Adapter version
    fn version(&self) -> &str;
    
    /// Supported protocols (HTTP, gRPC, TCP, WebSocket, etc.)
    fn supported_protocols(&self) -> Vec<AdapterProtocol>;
    
    /// Adapter capabilities
    fn capabilities(&self) -> AdapterCapabilities;
    
    /// Initialize the adapter
    async fn initialize(&mut self, config: AdapterConfig) -> Result<(), ObservabilityError>;
    
    /// Start the adapter (begin accepting connections)
    async fn start(&mut self) -> Result<(), ObservabilityError>;
    
    /// Stop the adapter
    async fn stop(&mut self) -> Result<(), ObservabilityError>;
    
    /// Handle incoming connection from external system
    async fn handle_connection(&self, connection: AdapterConnection) -> Result<(), ObservabilityError>;
    
    /// Export metrics (push model)
    async fn export_metrics(&self, metrics: &[MetricData]) -> Result<(), ObservabilityError>;
    
    /// Export logs (push model)
    async fn export_logs(&self, logs: &[LogData]) -> Result<(), ObservabilityError>;
    
    /// Export traces (push model)
    async fn export_traces(&self, traces: &[TraceData]) -> Result<(), ObservabilityError>;
    
    /// Handle metrics query (pull model)
    async fn query_metrics(&self, query: MetricsQuery) -> Result<Vec<MetricData>, ObservabilityError>;
    
    /// Handle logs query (pull model)
    async fn query_logs(&self, query: LogsQuery) -> Result<Vec<LogData>, ObservabilityError>;
    
    /// Handle traces query (pull model)
    async fn query_traces(&self, query: TracesQuery) -> Result<Vec<TraceData>, ObservabilityError>;
    
    /// Get adapter health status
    async fn health(&self) -> AdapterHealth;
    
    /// Get adapter performance metrics
    async fn performance_metrics(&self) -> AdapterPerformanceMetrics;
}

/// Observability adapter registry - manages all plug-and-play adapters
pub struct ObservabilityAdapterRegistry {
    /// Registered adapters
    adapters: Arc<RwLock<HashMap<String, Box<dyn ObservabilityAdapter>>>>,
    /// Network server for handling connections
    network_server: Arc<network_server::ObservabilityNetworkServer>,
    /// Performance tracker
    performance_tracker: Arc<RwLock<PerformanceTracker>>,
    /// Configuration
    config: AdapterRegistryConfig,
}

/// Adapter registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterRegistryConfig {
    /// Enable network server
    pub network_server_enabled: bool,
    /// Network server bind address
    pub bind_address: String,
    /// Network server port
    pub port: u16,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection timeout in microseconds
    pub connection_timeout_us: u64,
    /// Enable performance tracking
    pub performance_tracking_enabled: bool,
    /// Performance target in microseconds
    pub performance_target_us: u64,
}

impl Default for AdapterRegistryConfig {
    fn default() -> Self {
        Self {
            network_server_enabled: true,
            bind_address: "0.0.0.0".to_string(),
            port: 8082, // Dedicated port for observability adapters
            max_connections: 10000,
            connection_timeout_us: 100, // 100 microseconds target
            performance_tracking_enabled: true,
            performance_target_us: 100, // 100 microseconds target
        }
    }
}

/// Supported adapter protocols
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdapterProtocol {
    Http,
    Https,
    Grpc,
    Tcp,
    Udp,
    WebSocket,
    UnixSocket,
    Custom(String),
}

/// Adapter capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    /// Supports metrics pull
    pub metrics_pull: bool,
    /// Supports metrics push
    pub metrics_push: bool,
    /// Supports logs pull
    pub logs_pull: bool,
    /// Supports logs push
    pub logs_push: bool,
    /// Supports traces pull
    pub traces_pull: bool,
    /// Supports traces push
    pub traces_push: bool,
    /// Supports real-time streaming
    pub real_time_streaming: bool,
    /// Supports batch operations
    pub batch_operations: bool,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Supported data formats
    pub supported_formats: Vec<DataFormat>,
}

/// Supported data formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataFormat {
    Json,
    Protobuf,
    MessagePack,
    Avro,
    Prometheus,
    OpenTelemetry,
    Custom(String),
}

/// Adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Adapter-specific configuration
    pub config: HashMap<String, serde_json::Value>,
    /// Performance requirements
    pub performance_requirements: PerformanceRequirements,
    /// Security settings
    pub security_settings: SecuritySettings,
}

/// Performance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRequirements {
    /// Maximum latency in microseconds
    pub max_latency_us: u64,
    /// Minimum throughput (operations per second)
    pub min_throughput_ops: u64,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            max_latency_us: 100, // 100 microseconds
            min_throughput_ops: 100000, // 100k ops/sec
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            max_cpu_percent: 10.0, // 10% CPU
        }
    }
}

/// Security settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    /// Enable TLS
    pub tls_enabled: bool,
    /// Require authentication
    pub auth_required: bool,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Allowed IP addresses
    pub allowed_ips: Vec<String>,
    /// Rate limiting
    pub rate_limit_per_second: Option<u64>,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            tls_enabled: false,
            auth_required: false,
            api_key: None,
            allowed_ips: vec!["127.0.0.1".to_string()],
            rate_limit_per_second: Some(10000),
        }
    }
}

/// Adapter connection information
#[derive(Debug, Clone)]
pub struct AdapterConnection {
    /// Connection ID
    pub id: Uuid,
    /// Remote address
    pub remote_addr: String,
    /// Protocol used
    pub protocol: AdapterProtocol,
    /// Connection timestamp
    pub connected_at: Instant,
    /// Connection metadata
    pub metadata: HashMap<String, String>,
}

/// Adapter health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealth {
    /// Overall health status
    pub status: HealthStatus,
    /// Health message
    pub message: String,
    /// Last health check timestamp
    pub last_check: u64,
    /// Active connections count
    pub active_connections: usize,
    /// Error count in last minute
    pub error_count: u64,
    /// Performance metrics
    pub performance: AdapterPerformanceMetrics,
}

/// Health status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Adapter performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterPerformanceMetrics {
    /// Average latency in microseconds
    pub avg_latency_us: f64,
    /// P95 latency in microseconds
    pub p95_latency_us: f64,
    /// P99 latency in microseconds
    pub p99_latency_us: f64,
    /// Throughput (operations per second)
    pub throughput_ops: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Network bytes sent
    pub network_bytes_sent: u64,
    /// Network bytes received
    pub network_bytes_received: u64,
}

/// Performance tracker for monitoring adapter performance
#[derive(Debug)]
pub struct PerformanceTracker {
    /// Latency samples (ring buffer)
    latency_samples: Vec<u64>,
    /// Sample index
    sample_index: usize,
    /// Total operations
    total_operations: u64,
    /// Start time
    start_time: Instant,
    /// Memory usage samples
    memory_samples: Vec<u64>,
    /// CPU usage samples
    cpu_samples: Vec<f64>,
}

impl PerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            latency_samples: vec![0; 1000], // Keep last 1000 samples
            sample_index: 0,
            total_operations: 0,
            start_time: Instant::now(),
            memory_samples: vec![0; 100],
            cpu_samples: vec![0.0; 100],
        }
    }

    /// Record operation latency
    pub fn record_latency(&mut self, latency_us: u64) {
        self.latency_samples[self.sample_index] = latency_us;
        self.sample_index = (self.sample_index + 1) % self.latency_samples.len();
        self.total_operations += 1;
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> AdapterPerformanceMetrics {
        let mut sorted_latencies = self.latency_samples.clone();
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
            self.total_operations as f64 / elapsed
        } else {
            0.0
        };

        AdapterPerformanceMetrics {
            avg_latency_us: avg_latency,
            p95_latency_us: p95_latency,
            p99_latency_us: p99_latency,
            throughput_ops: throughput,
            memory_usage_bytes: self.memory_samples.last().copied().unwrap_or(0),
            cpu_usage_percent: self.cpu_samples.last().copied().unwrap_or(0.0),
            network_bytes_sent: 0, // Would be tracked separately
            network_bytes_received: 0, // Would be tracked separately
        }
    }
}

/// Standardized metric data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricData {
    /// Metric name
    pub name: String,
    /// Metric value
    pub value: f64,
    /// Metric labels
    pub labels: HashMap<String, String>,
    /// Timestamp in microseconds since epoch
    pub timestamp_us: u64,
    /// Metric type
    pub metric_type: MetricType,
}

/// Metric type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// Standardized log data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogData {
    /// Log level
    pub level: String,
    /// Log message
    pub message: String,
    /// Timestamp in microseconds since epoch
    pub timestamp_us: u64,
    /// Log fields
    pub fields: HashMap<String, serde_json::Value>,
    /// Correlation ID
    pub correlation_id: Option<String>,
}

/// Standardized trace data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceData {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID
    pub parent_span_id: Option<String>,
    /// Operation name
    pub operation_name: String,
    /// Start timestamp in microseconds since epoch
    pub start_timestamp_us: u64,
    /// End timestamp in microseconds since epoch
    pub end_timestamp_us: Option<u64>,
    /// Duration in microseconds
    pub duration_us: Option<u64>,
    /// Span tags
    pub tags: HashMap<String, String>,
    /// Span logs
    pub logs: Vec<SpanLogData>,
}

/// Span log data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLogData {
    /// Timestamp in microseconds since epoch
    pub timestamp_us: u64,
    /// Log fields
    pub fields: HashMap<String, serde_json::Value>,
}

/// Metrics query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQuery {
    /// Metric name pattern
    pub name_pattern: Option<String>,
    /// Label filters
    pub label_filters: HashMap<String, String>,
    /// Start time in microseconds since epoch
    pub start_time_us: Option<u64>,
    /// End time in microseconds since epoch
    pub end_time_us: Option<u64>,
    /// Maximum number of results
    pub limit: Option<usize>,
}

/// Logs query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsQuery {
    /// Log level filter
    pub level_filter: Option<String>,
    /// Message pattern
    pub message_pattern: Option<String>,
    /// Field filters
    pub field_filters: HashMap<String, serde_json::Value>,
    /// Start time in microseconds since epoch
    pub start_time_us: Option<u64>,
    /// End time in microseconds since epoch
    pub end_time_us: Option<u64>,
    /// Maximum number of results
    pub limit: Option<usize>,
}

/// Traces query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracesQuery {
    /// Trace ID
    pub trace_id: Option<String>,
    /// Service name
    pub service_name: Option<String>,
    /// Operation name pattern
    pub operation_pattern: Option<String>,
    /// Tag filters
    pub tag_filters: HashMap<String, String>,
    /// Start time in microseconds since epoch
    pub start_time_us: Option<u64>,
    /// End time in microseconds since epoch
    pub end_time_us: Option<u64>,
    /// Maximum number of results
    pub limit: Option<usize>,
}

impl ObservabilityAdapterRegistry {
    /// Create new adapter registry
    pub fn new(config: AdapterRegistryConfig) -> Self {
        let network_server = Arc::new(network_server::ObservabilityNetworkServer::new(
            config.bind_address.clone(),
            config.port,
            config.max_connections,
        ));

        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            network_server,
            performance_tracker: Arc::new(RwLock::new(PerformanceTracker::new())),
            config,
        }
    }

    /// Register a new adapter
    pub async fn register_adapter(&self, adapter: Box<dyn ObservabilityAdapter>) -> Result<(), ObservabilityError> {
        let adapter_id = adapter.id().to_string();
        let mut adapters = self.adapters.write().await;
        
        if adapters.contains_key(&adapter_id) {
            return Err(ObservabilityError::Internal(format!(
                "Adapter with ID '{}' is already registered", adapter_id
            )));
        }

        adapters.insert(adapter_id, adapter);
        Ok(())
    }

    /// Unregister an adapter
    pub async fn unregister_adapter(&self, adapter_id: &str) -> Result<(), ObservabilityError> {
        let mut adapters = self.adapters.write().await;
        
        if let Some(mut adapter) = adapters.remove(adapter_id) {
            adapter.stop().await?;
        }

        Ok(())
    }

    /// Start all adapters and network server
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        // Start network server
        if self.config.network_server_enabled {
            self.network_server.start().await?;
        }

        // Start all registered adapters
        let mut adapters = self.adapters.write().await;
        for (_, adapter) in adapters.iter_mut() {
            adapter.start().await?;
        }

        Ok(())
    }

    /// Stop all adapters and network server
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        // Stop all registered adapters
        let mut adapters = self.adapters.write().await;
        for (_, adapter) in adapters.iter_mut() {
            adapter.stop().await?;
        }

        // Stop network server
        if self.config.network_server_enabled {
            self.network_server.stop().await?;
        }

        Ok(())
    }

    /// Get adapter by ID
    pub async fn get_adapter(&self, _adapter_id: &str) -> Option<Box<dyn ObservabilityAdapter>> {
        let _adapters = self.adapters.read().await;
        // Note: This is a simplified version - in practice, we'd need to handle the trait object differently
        None // Placeholder
    }

    /// List all registered adapters
    pub async fn list_adapters(&self) -> Vec<String> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Get registry health status
    pub async fn health(&self) -> RegistryHealth {
        let adapters = self.adapters.read().await;
        let mut adapter_healths = HashMap::new();

        for (id, adapter) in adapters.iter() {
            adapter_healths.insert(id.clone(), adapter.health().await);
        }

        let overall_status = if adapter_healths.values().all(|h| h.status == HealthStatus::Healthy) {
            HealthStatus::Healthy
        } else if adapter_healths.values().any(|h| h.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        RegistryHealth {
            overall_status,
            adapter_count: adapter_healths.len(),
            adapter_healths,
            network_server_status: if self.config.network_server_enabled {
                self.network_server.health().await
            } else {
                HealthStatus::Unknown
            },
        }
    }

    /// Get performance metrics for all adapters
    pub async fn performance_metrics(&self) -> RegistryPerformanceMetrics {
        let adapters = self.adapters.read().await;
        let mut adapter_metrics = HashMap::new();

        for (id, adapter) in adapters.iter() {
            adapter_metrics.insert(id.clone(), adapter.performance_metrics().await);
        }

        let tracker = self.performance_tracker.read().await;
        let overall_metrics = tracker.get_metrics();

        RegistryPerformanceMetrics {
            overall_metrics,
            adapter_metrics,
            network_server_metrics: if self.config.network_server_enabled {
                Some(self.network_server.performance_metrics().await)
            } else {
                None
            },
        }
    }

    /// Record operation performance
    pub async fn record_performance(&self, latency_us: u64) {
        if self.config.performance_tracking_enabled {
            let mut tracker = self.performance_tracker.write().await;
            tracker.record_latency(latency_us);
        }
    }
}

/// Registry health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryHealth {
    /// Overall registry status
    pub overall_status: HealthStatus,
    /// Number of registered adapters
    pub adapter_count: usize,
    /// Individual adapter health statuses
    pub adapter_healths: HashMap<String, AdapterHealth>,
    /// Network server status
    pub network_server_status: HealthStatus,
}

/// Registry performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPerformanceMetrics {
    /// Overall registry performance
    pub overall_metrics: AdapterPerformanceMetrics,
    /// Individual adapter performance
    pub adapter_metrics: HashMap<String, AdapterPerformanceMetrics>,
    /// Network server performance
    pub network_server_metrics: Option<AdapterPerformanceMetrics>,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}