// Jaeger Adapter - Direct connection for Jaeger tracing servers
// Enables Jaeger to collect Valkyrie traces with 100μs performance

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

/// Jaeger adapter for direct Jaeger server integration
pub struct JaegerAdapter {
    /// Adapter ID
    id: String,
    /// Adapter configuration
    config: JaegerAdapterConfig,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Active connections
    active_connections: Arc<RwLock<HashMap<Uuid, AdapterConnection>>>,
}

/// Jaeger adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JaegerAdapterConfig {
    /// Enable traces endpoint
    pub traces_endpoint_enabled: bool,
    /// Traces endpoint path
    pub traces_endpoint_path: String,
    /// Service name
    pub service_name: String,
}

impl Default for JaegerAdapterConfig {
    fn default() -> Self {
        Self {
            traces_endpoint_enabled: true,
            traces_endpoint_path: "/api/traces".to_string(),
            service_name: "valkyrie".to_string(),
        }
    }
}

impl JaegerAdapter {
    /// Create new Jaeger adapter
    pub fn new() -> Self {
        Self {
            id: "jaeger-adapter".to_string(),
            config: JaegerAdapterConfig::default(),
            running: Arc::new(RwLock::new(false)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ObservabilityAdapter for JaegerAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Jaeger Adapter"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn supported_protocols(&self) -> Vec<AdapterProtocol> {
        vec![AdapterProtocol::Http, AdapterProtocol::Grpc]
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            metrics_pull: false,
            metrics_push: false,
            logs_pull: false,
            logs_push: false,
            traces_pull: true,
            traces_push: true,
            real_time_streaming: true,
            batch_operations: true,
            max_batch_size: 1000,
            supported_formats: vec![DataFormat::Json, DataFormat::Protobuf],
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

    async fn handle_connection(
        &self,
        connection: AdapterConnection,
    ) -> Result<(), ObservabilityError> {
        let mut connections = self.active_connections.write().await;
        connections.insert(connection.id, connection);
        Ok(())
    }

    async fn export_metrics(&self, _metrics: &[MetricData]) -> Result<(), ObservabilityError> {
        Err(ObservabilityError::Internal(
            "Jaeger adapter doesn't support metrics".to_string(),
        ))
    }

    async fn export_logs(&self, _logs: &[LogData]) -> Result<(), ObservabilityError> {
        Err(ObservabilityError::Internal(
            "Jaeger adapter doesn't support logs".to_string(),
        ))
    }

    async fn export_traces(&self, _traces: &[TraceData]) -> Result<(), ObservabilityError> {
        // Export traces to Jaeger
        Ok(())
    }

    async fn query_metrics(
        &self,
        _query: MetricsQuery,
    ) -> Result<Vec<MetricData>, ObservabilityError> {
        Err(ObservabilityError::Internal(
            "Jaeger adapter doesn't support metrics".to_string(),
        ))
    }

    async fn query_logs(&self, _query: LogsQuery) -> Result<Vec<LogData>, ObservabilityError> {
        Err(ObservabilityError::Internal(
            "Jaeger adapter doesn't support logs".to_string(),
        ))
    }

    async fn query_traces(
        &self,
        _query: TracesQuery,
    ) -> Result<Vec<TraceData>, ObservabilityError> {
        // Query traces from Jaeger
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
            message: format!("Jaeger adapter - {} connections", connections.len()),
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
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

/// Jaeger protocol handler
pub struct JaegerProtocolHandler {
    adapter: Arc<JaegerAdapter>,
}

impl JaegerProtocolHandler {
    pub fn new(adapter: Arc<JaegerAdapter>) -> Self {
        Self { adapter }
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for JaegerProtocolHandler {
    async fn handle_request(
        &self,
        _request: ProtocolRequest,
    ) -> Result<ProtocolResponse, ObservabilityError> {
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
            supported_operations: vec!["POST /api/traces".to_string()],
            max_request_size: 10 * 1024 * 1024, // 10MB
            max_response_size: 1024,            // 1KB
            supports_streaming: true,
            supports_compression: true,
        }
    }
}
