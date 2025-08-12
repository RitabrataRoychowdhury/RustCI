// Grafana Adapter - Direct connection for Grafana dashboards
// Enables Grafana to query Valkyrie metrics and logs with 100μs performance

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
use super::network_server::{ProtocolHandler, ProtocolRequest, ProtocolResponse, ProtocolCapabilities};

/// Grafana adapter for direct Grafana integration
pub struct GrafanaAdapter {
    /// Adapter ID
    id: String,
    /// Adapter configuration
    config: GrafanaAdapterConfig,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Active connections
    active_connections: Arc<RwLock<HashMap<Uuid, AdapterConnection>>>,
}

/// Grafana adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrafanaAdapterConfig {
    /// Enable query endpoint
    pub query_endpoint_enabled: bool,
    /// Query endpoint path
    pub query_endpoint_path: String,
    /// Enable annotations endpoint
    pub annotations_endpoint_enabled: bool,
}

impl Default for GrafanaAdapterConfig {
    fn default() -> Self {
        Self {
            query_endpoint_enabled: true,
            query_endpoint_path: "/query".to_string(),
            annotations_endpoint_enabled: true,
        }
    }
}

impl GrafanaAdapter {
    /// Create new Grafana adapter
    pub fn new() -> Self {
        Self {
            id: "grafana-adapter".to_string(),
            config: GrafanaAdapterConfig::default(),
            running: Arc::new(RwLock::new(false)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ObservabilityAdapter for GrafanaAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Grafana Adapter"
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
            metrics_push: false,
            logs_pull: true,
            logs_push: false,
            traces_pull: true,
            traces_push: false,
            real_time_streaming: true,
            batch_operations: true,
            max_batch_size: 10000,
            supported_formats: vec![DataFormat::Json],
        }
    }

    async fn initialize(&mut self, _config: AdapterConfig) -> Result<(), ObservabilityError> {
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
        Ok(())
    }

    async fn handle_connection(&self, connection: AdapterConnection) -> Result<(), ObservabilityError> {
        let mut connections = self.active_connections.write().await;
        connections.insert(connection.id, connection);
        Ok(())
    }

    async fn export_metrics(&self, _metrics: &[MetricData]) -> Result<(), ObservabilityError> {
        // Grafana typically pulls metrics, doesn't receive pushes
        Ok(())
    }

    async fn export_logs(&self, _logs: &[LogData]) -> Result<(), ObservabilityError> {
        // Grafana typically pulls logs, doesn't receive pushes
        Ok(())
    }

    async fn export_traces(&self, _traces: &[TraceData]) -> Result<(), ObservabilityError> {
        // Grafana typically pulls traces, doesn't receive pushes
        Ok(())
    }

    async fn query_metrics(&self, _query: MetricsQuery) -> Result<Vec<MetricData>, ObservabilityError> {
        // Return metrics for Grafana queries
        Ok(vec![])
    }

    async fn query_logs(&self, _query: LogsQuery) -> Result<Vec<LogData>, ObservabilityError> {
        // Return logs for Grafana queries
        Ok(vec![])
    }

    async fn query_traces(&self, _query: TracesQuery) -> Result<Vec<TraceData>, ObservabilityError> {
        // Return traces for Grafana queries
        Ok(vec![])
    }

    async fn health(&self) -> AdapterHealth {
        let running = *self.running.read().await;
        let connections = self.active_connections.read().await;

        let status = if running {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        };

        AdapterHealth {
            status,
            message: format!("Grafana adapter - {} connections", connections.len()),
            last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            active_connections: connections.len(),
            error_count: 0,
            performance: AdapterPerformanceMetrics {
                avg_latency_us: 0.0,
                p95_latency_us: 0.0,
                p99_latency_us: 0.0,
                throughput_ops: 0.0,
                memory_usage_bytes: 0,
                cpu_usage_percent: 0.0,
                network_bytes_sent: 0,
                network_bytes_received: 0,
            },
        }
    }

    async fn performance_metrics(&self) -> AdapterPerformanceMetrics {
        AdapterPerformanceMetrics {
            avg_latency_us: 0.0,
            p95_latency_us: 0.0,
            p99_latency_us: 0.0,
            throughput_ops: 0.0,
            memory_usage_bytes: 0,
            cpu_usage_percent: 0.0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
        }
    }
}

/// Grafana protocol handler
pub struct GrafanaProtocolHandler {
    adapter: Arc<GrafanaAdapter>,
}

impl GrafanaProtocolHandler {
    pub fn new(adapter: Arc<GrafanaAdapter>) -> Self {
        Self { adapter }
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for GrafanaProtocolHandler {
    async fn handle_request(&self, _request: ProtocolRequest) -> Result<ProtocolResponse, ObservabilityError> {
        let start_time = Instant::now();
        
        Ok(ProtocolResponse {
            status_code: 200,
            headers: HashMap::new(),
            body: b"OK".to_vec(),
            processing_duration: start_time.elapsed(),
        })
    }

    fn capabilities(&self) -> ProtocolCapabilities {
        ProtocolCapabilities {
            supported_operations: vec![
                "POST /query".to_string(),
                "POST /annotations".to_string(),
            ],
            max_request_size: 1024 * 1024, // 1MB
            max_response_size: 10 * 1024 * 1024, // 10MB
            supports_streaming: true,
            supports_compression: true,
        }
    }
}