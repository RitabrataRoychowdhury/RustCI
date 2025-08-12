// External observability integrations (Prometheus, OpenTelemetry, Jaeger)

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

use super::{ObservabilityError, MetricsCollector, StructuredLogger, CorrelationId};

/// External observability integration manager
pub struct ExternalObservabilityIntegration {
    /// Prometheus integration
    prometheus: Option<Arc<PrometheusIntegration>>,
    /// OpenTelemetry integration
    opentelemetry: Option<Arc<OpenTelemetryIntegration>>,
    /// Jaeger tracing integration
    jaeger: Option<Arc<JaegerIntegration>>,
    /// Grafana dashboard integration
    grafana: Option<Arc<GrafanaIntegration>>,
    /// Configuration
    config: ExternalObservabilityConfig,
}

/// Configuration for external observability integrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalObservabilityConfig {
    /// Enable Prometheus metrics export
    pub prometheus_enabled: bool,
    /// Prometheus endpoint configuration
    pub prometheus_endpoint: String,
    /// Prometheus push gateway URL
    pub prometheus_push_gateway: Option<String>,
    /// Enable OpenTelemetry tracing
    pub opentelemetry_enabled: bool,
    /// OpenTelemetry collector endpoint
    pub opentelemetry_endpoint: String,
    /// Enable Jaeger tracing
    pub jaeger_enabled: bool,
    /// Jaeger agent endpoint
    pub jaeger_endpoint: String,
    /// Enable Grafana dashboard integration
    pub grafana_enabled: bool,
    /// Grafana API endpoint
    pub grafana_endpoint: String,
    /// Grafana API key
    pub grafana_api_key: Option<String>,
    /// Metrics export interval in seconds
    pub export_interval_seconds: u64,
    /// Batch size for metric exports
    pub batch_size: usize,
}

impl Default for ExternalObservabilityConfig {
    fn default() -> Self {
        Self {
            prometheus_enabled: false,
            prometheus_endpoint: "http://localhost:9090".to_string(),
            prometheus_push_gateway: None,
            opentelemetry_enabled: false,
            opentelemetry_endpoint: "http://localhost:4317".to_string(),
            jaeger_enabled: false,
            jaeger_endpoint: "http://localhost:14268".to_string(),
            grafana_enabled: false,
            grafana_endpoint: "http://localhost:3000".to_string(),
            grafana_api_key: None,
            export_interval_seconds: 15,
            batch_size: 1000,
        }
    }
}

impl ExternalObservabilityIntegration {
    /// Create a new external observability integration
    pub fn new(config: ExternalObservabilityConfig) -> Self {
        let prometheus = if config.prometheus_enabled {
            Some(Arc::new(PrometheusIntegration::new(&config)))
        } else {
            None
        };

        let opentelemetry = if config.opentelemetry_enabled {
            Some(Arc::new(OpenTelemetryIntegration::new(&config)))
        } else {
            None
        };

        let jaeger = if config.jaeger_enabled {
            Some(Arc::new(JaegerIntegration::new(&config)))
        } else {
            None
        };

        let grafana = if config.grafana_enabled {
            Some(Arc::new(GrafanaIntegration::new(&config)))
        } else {
            None
        };

        Self {
            prometheus,
            opentelemetry,
            jaeger,
            grafana,
            config,
        }
    }

    /// Start external integrations
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        if let Some(ref prometheus) = self.prometheus {
            prometheus.start().await?;
        }

        if let Some(ref opentelemetry) = self.opentelemetry {
            opentelemetry.start().await?;
        }

        if let Some(ref jaeger) = self.jaeger {
            jaeger.start().await?;
        }

        if let Some(ref grafana) = self.grafana {
            grafana.start().await?;
        }

        Ok(())
    }

    /// Stop external integrations
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        if let Some(ref grafana) = self.grafana {
            grafana.stop().await?;
        }

        if let Some(ref jaeger) = self.jaeger {
            jaeger.stop().await?;
        }

        if let Some(ref opentelemetry) = self.opentelemetry {
            opentelemetry.stop().await?;
        }

        if let Some(ref prometheus) = self.prometheus {
            prometheus.stop().await?;
        }

        Ok(())
    }

    /// Export metrics to external systems
    pub async fn export_metrics(&self, metrics_collector: &MetricsCollector) -> Result<(), ObservabilityError> {
        if let Some(ref prometheus) = self.prometheus {
            prometheus.export_metrics(metrics_collector).await?;
        }

        if let Some(ref opentelemetry) = self.opentelemetry {
            opentelemetry.export_metrics(metrics_collector).await?;
        }

        Ok(())
    }

    /// Export traces to external systems
    pub async fn export_traces(&self, correlation_id: CorrelationId, spans: &[TraceSpan]) -> Result<(), ObservabilityError> {
        if let Some(ref jaeger) = self.jaeger {
            jaeger.export_traces(correlation_id, spans).await?;
        }

        if let Some(ref opentelemetry) = self.opentelemetry {
            opentelemetry.export_traces(correlation_id, spans).await?;
        }

        Ok(())
    }

    /// Export logs to external systems
    pub async fn export_logs(&self, logger: &StructuredLogger) -> Result<(), ObservabilityError> {
        if let Some(ref opentelemetry) = self.opentelemetry {
            opentelemetry.export_logs(logger).await?;
        }

        Ok(())
    }

    /// Update Grafana dashboards
    pub async fn update_dashboards(&self) -> Result<(), ObservabilityError> {
        if let Some(ref grafana) = self.grafana {
            grafana.update_dashboards().await?;
        }

        Ok(())
    }

    /// Get integration status
    pub async fn status(&self) -> ExternalObservabilityStatus {
        ExternalObservabilityStatus {
            prometheus_enabled: self.config.prometheus_enabled,
            prometheus_status: if let Some(ref prometheus) = self.prometheus {
                prometheus.status().await
            } else {
                IntegrationStatus::Disabled
            },
            opentelemetry_enabled: self.config.opentelemetry_enabled,
            opentelemetry_status: if let Some(ref opentelemetry) = self.opentelemetry {
                opentelemetry.status().await
            } else {
                IntegrationStatus::Disabled
            },
            jaeger_enabled: self.config.jaeger_enabled,
            jaeger_status: if let Some(ref jaeger) = self.jaeger {
                jaeger.status().await
            } else {
                IntegrationStatus::Disabled
            },
            grafana_enabled: self.config.grafana_enabled,
            grafana_status: if let Some(ref grafana) = self.grafana {
                grafana.status().await
            } else {
                IntegrationStatus::Disabled
            },
        }
    }
}

/// Prometheus integration
pub struct PrometheusIntegration {
    /// Prometheus endpoint
    endpoint: String,
    /// Push gateway URL
    push_gateway: Option<String>,
    /// Export interval
    export_interval_seconds: u64,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Export task handle
    export_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl PrometheusIntegration {
    /// Create new Prometheus integration
    pub fn new(config: &ExternalObservabilityConfig) -> Self {
        Self {
            endpoint: config.prometheus_endpoint.clone(),
            push_gateway: config.prometheus_push_gateway.clone(),
            export_interval_seconds: config.export_interval_seconds,
            running: Arc::new(RwLock::new(false)),
            export_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Start Prometheus integration
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        // Start background export task if push gateway is configured
        if let Some(ref push_gateway) = self.push_gateway {
            let push_gateway = push_gateway.clone();
            let running_flag = self.running.clone();
            let export_interval = self.export_interval_seconds;

            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(export_interval));

                while *running_flag.read().await {
                    interval.tick().await;
                    
                    // Export metrics to push gateway
                    if let Err(e) = Self::push_metrics_to_gateway(&push_gateway).await {
                        eprintln!("Failed to push metrics to Prometheus gateway: {}", e);
                    }
                }
            });

            *self.export_handle.write().await = Some(handle);
        }

        Ok(())
    }

    /// Stop Prometheus integration
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);

        if let Some(handle) = self.export_handle.write().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Export metrics to Prometheus
    pub async fn export_metrics(&self, metrics_collector: &MetricsCollector) -> Result<(), ObservabilityError> {
        let summary = metrics_collector.summary().await;
        
        // Convert Valkyrie metrics to Prometheus format
        let prometheus_metrics = self.convert_to_prometheus_format(&summary).await?;
        
        // Send to Prometheus (implementation depends on whether using push gateway or pull model)
        if let Some(ref push_gateway) = self.push_gateway {
            self.push_to_gateway(push_gateway, &prometheus_metrics).await?;
        }

        Ok(())
    }

    /// Get integration status
    pub async fn status(&self) -> IntegrationStatus {
        if *self.running.read().await {
            // Test connectivity to Prometheus
            if self.test_connectivity().await {
                IntegrationStatus::Connected
            } else {
                IntegrationStatus::Error("Cannot connect to Prometheus".to_string())
            }
        } else {
            IntegrationStatus::Stopped
        }
    }

    /// Convert metrics to Prometheus format
    async fn convert_to_prometheus_format(&self, summary: &super::metrics::MetricsSummary) -> Result<String, ObservabilityError> {
        let mut prometheus_output = String::new();
        
        // Add Valkyrie-specific metrics
        prometheus_output.push_str(&format!(
            "# HELP valkyrie_total_metrics Total number of Valkyrie metrics\n\
             # TYPE valkyrie_total_metrics gauge\n\
             valkyrie_total_metrics {}\n\n",
            summary.total_metrics
        ));

        prometheus_output.push_str(&format!(
            "# HELP valkyrie_total_data_points Total number of Valkyrie data points\n\
             # TYPE valkyrie_total_data_points gauge\n\
             valkyrie_total_data_points {}\n\n",
            summary.total_data_points
        ));

        prometheus_output.push_str(&format!(
            "# HELP valkyrie_metrics_retention_seconds Metrics retention period in seconds\n\
             # TYPE valkyrie_metrics_retention_seconds gauge\n\
             valkyrie_metrics_retention_seconds {}\n\n",
            summary.retention_seconds
        ));

        Ok(prometheus_output)
    }

    /// Push metrics to Prometheus push gateway
    async fn push_to_gateway(&self, gateway_url: &str, metrics: &str) -> Result<(), ObservabilityError> {
        // Simplified implementation - in practice, use prometheus client library
        let client = reqwest::Client::new();
        let url = format!("{}/metrics/job/valkyrie", gateway_url);
        
        let response = client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(metrics.to_string())
            .send()
            .await
            .map_err(|e| ObservabilityError::Internal(format!("Failed to push to Prometheus: {}", e)))?;

        if !response.status().is_success() {
            return Err(ObservabilityError::Internal(format!(
                "Prometheus push gateway returned error: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Push metrics to gateway (static method for background task)
    async fn push_metrics_to_gateway(_gateway_url: &str) -> Result<(), ObservabilityError> {
        // Placeholder implementation
        Ok(())
    }

    /// Test connectivity to Prometheus
    async fn test_connectivity(&self) -> bool {
        // Simplified connectivity test
        if let Ok(client) = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            if let Ok(response) = client.get(&format!("{}/api/v1/status/config", self.endpoint)).send().await {
                response.status().is_success()
            } else {
                false
            }
        } else {
            false
        }
    }
}

/// OpenTelemetry integration
pub struct OpenTelemetryIntegration {
    /// OpenTelemetry collector endpoint
    endpoint: String,
    /// Export interval
    export_interval_seconds: u64,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl OpenTelemetryIntegration {
    /// Create new OpenTelemetry integration
    pub fn new(config: &ExternalObservabilityConfig) -> Self {
        Self {
            endpoint: config.opentelemetry_endpoint.clone(),
            export_interval_seconds: config.export_interval_seconds,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start OpenTelemetry integration
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = true;
        Ok(())
    }

    /// Stop OpenTelemetry integration
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    /// Export metrics to OpenTelemetry
    pub async fn export_metrics(&self, _metrics_collector: &MetricsCollector) -> Result<(), ObservabilityError> {
        // Convert Valkyrie metrics to OpenTelemetry format and export
        // This would use the OpenTelemetry SDK in a real implementation
        Ok(())
    }

    /// Export traces to OpenTelemetry
    pub async fn export_traces(&self, _correlation_id: CorrelationId, _spans: &[TraceSpan]) -> Result<(), ObservabilityError> {
        // Convert Valkyrie traces to OpenTelemetry format and export
        // This would use the OpenTelemetry SDK in a real implementation
        Ok(())
    }

    /// Export logs to OpenTelemetry
    pub async fn export_logs(&self, _logger: &StructuredLogger) -> Result<(), ObservabilityError> {
        // Convert Valkyrie logs to OpenTelemetry format and export
        // This would use the OpenTelemetry SDK in a real implementation
        Ok(())
    }

    /// Get integration status
    pub async fn status(&self) -> IntegrationStatus {
        if *self.running.read().await {
            IntegrationStatus::Connected
        } else {
            IntegrationStatus::Stopped
        }
    }
}

/// Jaeger tracing integration
pub struct JaegerIntegration {
    /// Jaeger agent endpoint
    endpoint: String,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl JaegerIntegration {
    /// Create new Jaeger integration
    pub fn new(config: &ExternalObservabilityConfig) -> Self {
        Self {
            endpoint: config.jaeger_endpoint.clone(),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start Jaeger integration
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = true;
        Ok(())
    }

    /// Stop Jaeger integration
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    /// Export traces to Jaeger
    pub async fn export_traces(&self, _correlation_id: CorrelationId, _spans: &[TraceSpan]) -> Result<(), ObservabilityError> {
        // Convert Valkyrie traces to Jaeger format and export
        // This would use the Jaeger client library in a real implementation
        Ok(())
    }

    /// Get integration status
    pub async fn status(&self) -> IntegrationStatus {
        if *self.running.read().await {
            IntegrationStatus::Connected
        } else {
            IntegrationStatus::Stopped
        }
    }
}

/// Grafana dashboard integration
pub struct GrafanaIntegration {
    /// Grafana API endpoint
    endpoint: String,
    /// API key
    api_key: Option<String>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl GrafanaIntegration {
    /// Create new Grafana integration
    pub fn new(config: &ExternalObservabilityConfig) -> Self {
        Self {
            endpoint: config.grafana_endpoint.clone(),
            api_key: config.grafana_api_key.clone(),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start Grafana integration
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = true;
        Ok(())
    }

    /// Stop Grafana integration
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    /// Update Grafana dashboards
    pub async fn update_dashboards(&self) -> Result<(), ObservabilityError> {
        // Create or update Valkyrie-specific dashboards in Grafana
        // This would use the Grafana API in a real implementation
        Ok(())
    }

    /// Get integration status
    pub async fn status(&self) -> IntegrationStatus {
        if *self.running.read().await {
            IntegrationStatus::Connected
        } else {
            IntegrationStatus::Stopped
        }
    }
}

/// Integration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrationStatus {
    Disabled,
    Stopped,
    Starting,
    Connected,
    Error(String),
}

/// External observability status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalObservabilityStatus {
    pub prometheus_enabled: bool,
    pub prometheus_status: IntegrationStatus,
    pub opentelemetry_enabled: bool,
    pub opentelemetry_status: IntegrationStatus,
    pub jaeger_enabled: bool,
    pub jaeger_status: IntegrationStatus,
    pub grafana_enabled: bool,
    pub grafana_status: IntegrationStatus,
}

/// Trace span for external export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub duration_us: Option<u64>,
    pub tags: HashMap<String, String>,
    pub logs: Vec<SpanLog>,
}

/// Span log for external export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
}