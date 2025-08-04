use crate::error::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;
use sysinfo::{SystemExt, CpuExt};

use super::{
    control_plane_health::{ControlPlaneHealth, ControlPlaneHealthResponse, HealthConfig},
    control_plane_metrics::{ControlPlaneMetrics, MetricsSnapshot},
};
use crate::core::observability::{
    distributed_tracing::TracingConfig,
    structured_logging::LoggingConfig,
};

use crate::core::observability::{
    distributed_tracing::{DistributedTracing, JobTrace, JobTraceStatus},
    structured_logging::{StructuredLogging, LoggingStatistics},
};

/// Comprehensive observability service for the control plane
pub struct ControlPlaneObservability {
    health_monitor: Arc<ControlPlaneHealth>,
    metrics_collector: Arc<ControlPlaneMetrics>,
    distributed_tracing: Arc<DistributedTracing>,
    structured_logging: Arc<StructuredLogging>,
    active_job_traces: Arc<RwLock<HashMap<Uuid, JobTrace>>>,
    config: ObservabilityConfig,
}

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub service_name: String,
    pub environment: String,
    pub enable_health_checks: bool,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub enable_structured_logging: bool,
    pub health_config: HealthConfig,
    pub tracing_config: TracingConfig,
    pub logging_config: LoggingConfig,
    pub monitoring_interval_seconds: u64,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "rustci-control-plane".to_string(),
            environment: std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()),
            enable_health_checks: true,
            enable_metrics: true,
            enable_tracing: true,
            enable_structured_logging: true,
            health_config: HealthConfig::default(),
            tracing_config: TracingConfig::default(),
            logging_config: LoggingConfig::default(),
            monitoring_interval_seconds: 30,
        }
    }
}

/// Comprehensive observability status
#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityStatus {
    pub timestamp: u64,
    pub service_name: String,
    pub environment: String,
    pub health: ControlPlaneHealthResponse,
    pub metrics: MetricsSnapshot,
    pub logging_stats: LoggingStatistics,
    pub active_traces: usize,
    pub active_job_traces: usize,
}

/// Job execution observability context
#[derive(Debug, Clone)]
pub struct JobObservabilityContext {
    pub job_id: Uuid,
    pub correlation_id: Uuid,
    pub job_trace: JobTrace,
    pub start_time: std::time::Instant,
}

impl ControlPlaneObservability {
    /// Initialize comprehensive observability
    pub async fn new(config: ObservabilityConfig) -> Result<Self> {
        info!("Initializing control plane observability");

        // Initialize health monitoring
        let health_monitor = if config.enable_health_checks {
            Arc::new(ControlPlaneHealth::new(config.health_config.clone()))
        } else {
            Arc::new(ControlPlaneHealth::new(HealthConfig::default()))
        };

        // Initialize metrics collection
        let metrics_collector = Arc::new(ControlPlaneMetrics::new()?);

        // Initialize distributed tracing
        let distributed_tracing = if config.enable_tracing {
            Arc::new(DistributedTracing::new(config.tracing_config.clone()).await?)
        } else {
            Arc::new(DistributedTracing::new(TracingConfig::default()).await?)
        };

        // Initialize structured logging
        let structured_logging = if config.enable_structured_logging {
            Arc::new(StructuredLogging::new(config.logging_config.clone())?)
        } else {
            Arc::new(StructuredLogging::new(LoggingConfig::default())?)
        };

        let service = Self {
            health_monitor,
            metrics_collector,
            distributed_tracing,
            structured_logging,
            active_job_traces: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        // Start background monitoring
        service.start_background_monitoring().await?;

        info!("Control plane observability initialized successfully");
        Ok(service)
    }

    /// Start background monitoring tasks
    async fn start_background_monitoring(&self) -> Result<()> {
        // Start health monitoring
        if self.config.enable_health_checks {
            self.health_monitor.start_monitoring().await?;
        }

        // Start periodic system metrics collection
        if self.config.enable_metrics {
            let metrics_collector = Arc::clone(&self.metrics_collector);
            let interval = Duration::from_secs(self.config.monitoring_interval_seconds);

            tokio::spawn(async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    // Collect system metrics
                    let mut system = {
                        use sysinfo::SystemExt;
                        sysinfo::System::new_all()
                    };
                    system.refresh_all();
                    
                    let memory_usage = system.used_memory();
                    let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
                    
                    metrics_collector.update_system_metrics(memory_usage, cpu_usage);
                }
            });
        }

        info!("Background monitoring tasks started");
        Ok(())
    }

    /// Start job observability context
    pub async fn start_job_observability(
        &self,
        job_id: Uuid,
        pipeline_id: Option<String>,
        node_id: Option<String>,
    ) -> Result<JobObservabilityContext> {
        // Start distributed trace
        let job_trace = self.distributed_tracing
            .start_job_trace(job_id, pipeline_id.clone(), None)
            .await;

        let correlation_id = job_trace.correlation_id;

        // Record metrics
        if let Some(ref node_id) = node_id {
            self.metrics_collector.record_job_scheduled(&job_id, node_id).await;
        }

        // Log job start
        self.structured_logging.log_job_event(
            job_id,
            "job_submitted",
            "Job submitted for execution",
            Some(correlation_id),
            node_id,
            pipeline_id,
            None,
            None,
            None,
        ).await;

        let context = JobObservabilityContext {
            job_id,
            correlation_id,
            job_trace,
            start_time: std::time::Instant::now(),
        };

        // Store active job trace
        let mut active_traces = self.active_job_traces.write().await;
        active_traces.insert(job_id, context.job_trace.clone());

        debug!("Started job observability context for job {}", job_id);
        Ok(context)
    }

    /// Update job status in observability context
    pub async fn update_job_status(
        &self,
        context: &mut JobObservabilityContext,
        status: JobTraceStatus,
        node_id: Option<String>,
        error_message: Option<String>,
    ) -> Result<()> {
        // Update distributed trace
        self.distributed_tracing
            .update_job_trace_status(&mut context.job_trace, status.clone())
            .await;

        // Record metrics based on status
        match status {
            JobTraceStatus::Scheduled => {
                if let Some(ref node_id) = node_id {
                    self.metrics_collector.record_job_started(&context.job_id, node_id).await;
                }
            }
            JobTraceStatus::Running => {
                if let Some(ref node_id) = node_id {
                    self.metrics_collector.record_job_started(&context.job_id, node_id).await;
                }
            }
            JobTraceStatus::Completed => {
                let duration = context.start_time.elapsed();
                if let Some(ref node_id) = node_id {
                    self.metrics_collector
                        .record_job_completed(&context.job_id, node_id, duration)
                        .await;
                }
            }
            JobTraceStatus::Failed => {
                if let Some(ref node_id) = node_id {
                    let error_msg = error_message.as_deref().unwrap_or("Unknown error");
                    self.metrics_collector
                        .record_job_failed(&context.job_id, node_id, error_msg)
                        .await;
                }
            }
            _ => {}
        }

        // Log status change
        let event_type = match status {
            JobTraceStatus::Submitted => "job_submitted",
            JobTraceStatus::Scheduled => "job_scheduled",
            JobTraceStatus::Running => "job_started",
            JobTraceStatus::Completed => "job_completed",
            JobTraceStatus::Failed => "job_failed",
            JobTraceStatus::Cancelled => "job_cancelled",
        };

        let message = match status {
            JobTraceStatus::Completed => "Job completed successfully",
            JobTraceStatus::Failed => "Job execution failed",
            JobTraceStatus::Cancelled => "Job was cancelled",
            _ => "Job status updated",
        };

        self.structured_logging.log_job_event(
            context.job_id,
            event_type,
            message,
            Some(context.correlation_id),
            node_id,
            context.job_trace.pipeline_id.clone(),
            None,
            Some(context.start_time.elapsed().as_millis() as u64),
            error_message,
        ).await;

        // Update active traces
        let mut active_traces = self.active_job_traces.write().await;
        active_traces.insert(context.job_id, context.job_trace.clone());

        debug!("Updated job status to {:?} for job {}", status, context.job_id);
        Ok(())
    }

    /// Complete job observability context
    pub async fn complete_job_observability(
        &self,
        mut context: JobObservabilityContext,
        success: bool,
        final_node_id: Option<String>,
    ) -> Result<()> {
        // Complete distributed trace
        self.distributed_tracing
            .complete_job_trace(&mut context.job_trace, success)
            .await;

        // Final metrics recording
        let duration = context.start_time.elapsed();
        if let Some(ref node_id) = final_node_id {
            if success {
                self.metrics_collector
                    .record_job_completed(&context.job_id, node_id, duration)
                    .await;
            } else {
                self.metrics_collector
                    .record_job_failed(&context.job_id, node_id, "Job execution failed")
                    .await;
            }
        }

        // Final log entry
        let message = if success {
            "Job execution completed successfully"
        } else {
            "Job execution completed with failure"
        };

        self.structured_logging.log_job_event(
            context.job_id,
            if success { "job_completed" } else { "job_failed" },
            message,
            Some(context.correlation_id),
            final_node_id,
            context.job_trace.pipeline_id.clone(),
            None,
            Some(duration.as_millis() as u64),
            if success { None } else { Some("Job execution failed".to_string()) },
        ).await;

        // Remove from active traces
        let mut active_traces = self.active_job_traces.write().await;
        active_traces.remove(&context.job_id);

        info!(
            "Completed job observability for job {} (success: {}, duration: {:?})",
            context.job_id, success, duration
        );
        Ok(())
    }

    /// Record node event
    pub async fn record_node_event(
        &self,
        node_id: String,
        event_type: &str,
        message: &str,
        health_status: Option<String>,
        resource_usage: Option<HashMap<String, f64>>,
    ) -> Result<()> {
        // Start correlation for node event
        let correlation_id = self.distributed_tracing
            .start_trace("node_event", "node_registry", None)
            .await;

        // Record metrics based on event type
        match event_type {
            "node_joined" => {
                self.metrics_collector.record_node_joined(&node_id).await;
            }
            "node_left" => {
                self.metrics_collector.record_node_left(&node_id, message).await;
            }
            "node_health_changed" => {
                if let Some(ref status) = health_status {
                    let is_healthy = status == "healthy";
                    self.metrics_collector
                        .record_node_health_change(&node_id, is_healthy)
                        .await;
                }
            }
            "node_heartbeat_failure" => {
                self.metrics_collector.record_node_heartbeat_failure(&node_id).await;
            }
            _ => {}
        }

        // Log node event
        self.structured_logging.log_node_event(
            node_id,
            event_type,
            message,
            Some(correlation_id),
            health_status,
            resource_usage,
        ).await;

        // End correlation
        self.distributed_tracing.end_trace(correlation_id, true).await;

        Ok(())
    }

    /// Get comprehensive observability status
    pub async fn get_observability_status(&self) -> ObservabilityStatus {
        let health = self.health_monitor.check_health().await;
        let metrics = self.metrics_collector.get_metrics_snapshot().await;
        let logging_stats = self.structured_logging.get_statistics().await;
        let active_traces = self.distributed_tracing.get_active_traces_count().await;
        let active_job_traces = self.active_job_traces.read().await.len();

        ObservabilityStatus {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            service_name: self.config.service_name.clone(),
            environment: self.config.environment.clone(),
            health,
            metrics,
            logging_stats,
            active_traces,
            active_job_traces,
        }
    }

    /// Create HTTP router for observability endpoints
    pub fn create_router(self: Arc<Self>) -> Router {
        Router::new()
            // Health endpoints
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .route("/health/deep", get(deep_health_handler))
            
            // Metrics endpoints
            .route("/metrics", get(metrics_handler))
            .route("/metrics/prometheus", get(prometheus_metrics_handler))
            .route("/metrics/snapshot", get(metrics_snapshot_handler))
            
            // Tracing endpoints
            .route("/traces/active", get(active_traces_handler))
            .route("/traces/job/:job_id", get(job_trace_handler))
            
            // Logging endpoints
            .route("/logs/recent", get(recent_logs_handler))
            .route("/logs/search", get(search_logs_handler))
            .route("/logs/correlation/:correlation_id", get(correlation_logs_handler))
            .route("/logs/stats", get(logging_stats_handler))
            
            // Comprehensive status
            .route("/status", get(observability_status_handler))
            .route("/status/summary", get(status_summary_handler))
            
            .with_state(self)
    }

    /// Get health monitor
    pub fn health_monitor(&self) -> Arc<ControlPlaneHealth> {
        Arc::clone(&self.health_monitor)
    }

    /// Get metrics collector
    pub fn metrics_collector(&self) -> Arc<ControlPlaneMetrics> {
        Arc::clone(&self.metrics_collector)
    }

    /// Get distributed tracing
    pub fn distributed_tracing(&self) -> Arc<DistributedTracing> {
        Arc::clone(&self.distributed_tracing)
    }

    /// Get structured logging
    pub fn structured_logging(&self) -> Arc<StructuredLogging> {
        Arc::clone(&self.structured_logging)
    }
}

// HTTP handlers
async fn health_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let health_response = observability.health_monitor.check_health().await;
    
    let status_code = match health_response.status {
        super::control_plane_health::HealthStatus::Healthy => StatusCode::OK,
        super::control_plane_health::HealthStatus::Degraded => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(health_response))
}

async fn liveness_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({
        "status": "alive",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    })))
}

async fn readiness_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let health_response = observability.health_monitor.check_health().await;
    
    let is_ready = matches!(
        health_response.status,
        super::control_plane_health::HealthStatus::Healthy | 
        super::control_plane_health::HealthStatus::Degraded
    );

    let status_code = if is_ready { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };

    (status_code, Json(serde_json::json!({
        "status": if is_ready { "ready" } else { "not_ready" },
        "health_status": health_response.status,
        "timestamp": health_response.timestamp
    })))
}

async fn deep_health_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let status = observability.get_observability_status().await;
    (StatusCode::OK, Json(status))
}

async fn metrics_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let snapshot = observability.metrics_collector.get_metrics_snapshot().await;
    (StatusCode::OK, Json(snapshot))
}

async fn prometheus_metrics_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let metrics = observability.metrics_collector.get_prometheus_metrics();
    (StatusCode::OK, metrics)
}

async fn metrics_snapshot_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let snapshot = observability.metrics_collector.get_metrics_snapshot().await;
    (StatusCode::OK, Json(snapshot))
}

async fn active_traces_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let active_count = observability.distributed_tracing.get_active_traces_count().await;
    let active_job_traces = observability.active_job_traces.read().await.len();
    
    (StatusCode::OK, Json(serde_json::json!({
        "active_traces": active_count,
        "active_job_traces": active_job_traces
    })))
}

async fn job_trace_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let job_id = match Uuid::parse_str(&job_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Invalid job ID format"
        }))),
    };

    let active_traces = observability.active_job_traces.read().await;
    if let Some(_job_trace) = active_traces.get(&job_id) {
        (StatusCode::OK, Json(serde_json::json!({
            "job_id": job_id.to_string(),
            "status": "found",
            "message": "Job trace found but serialization not implemented yet"
        })))
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Job trace not found"
        })))
    }
}

#[derive(Deserialize)]
struct LogsQuery {
    limit: Option<usize>,
}

async fn recent_logs_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100);
    let logs = observability.structured_logging.get_recent_logs(limit).await;
    (StatusCode::OK, Json(serde_json::to_value(logs).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<usize>,
}

async fn search_logs_handler(
    State(_observability): State<Arc<ControlPlaneObservability>>,
    Query(_query): Query<SearchQuery>,
) -> impl IntoResponse {
    // TODO: Implement log search functionality
    (StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({
        "error": "Log search not yet implemented"
    })))
}

async fn correlation_logs_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
    Path(correlation_id): Path<String>,
) -> impl IntoResponse {
    let correlation_id = match Uuid::parse_str(&correlation_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Invalid correlation ID format"
        }))),
    };

    let logs = observability.structured_logging
        .search_logs_by_correlation(correlation_id).await;
    (StatusCode::OK, Json(serde_json::to_value(logs).unwrap_or_default()))
}

async fn logging_stats_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let stats = observability.structured_logging.get_statistics().await;
    (StatusCode::OK, Json(stats))
}

async fn observability_status_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let status = observability.get_observability_status().await;
    (StatusCode::OK, Json(status))
}

async fn status_summary_handler(
    State(observability): State<Arc<ControlPlaneObservability>>,
) -> impl IntoResponse {
    let status = observability.get_observability_status().await;
    
    let summary = serde_json::json!({
        "service": status.service_name,
        "environment": status.environment,
        "health_status": status.health.status,
        "uptime_seconds": status.health.uptime_seconds,
        "active_traces": status.active_traces,
        "active_job_traces": status.active_job_traces,
        "total_jobs_scheduled": status.metrics.jobs_scheduled_total,
        "total_jobs_completed": status.metrics.jobs_completed_total,
        "total_jobs_failed": status.metrics.jobs_failed_total,
        "jobs_running": status.metrics.jobs_running,
        "nodes_total": status.metrics.nodes_total,
        "nodes_healthy": status.metrics.nodes_healthy,
        "cluster_size": status.metrics.cluster_size,
        "cluster_utilization_percent": status.metrics.cluster_utilization_percent,
        "log_entries": status.logging_stats.total_entries,
        "error_count": status.logging_stats.error_count
    });

    (StatusCode::OK, Json(summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability_creation() {
        let config = ObservabilityConfig::default();
        let observability = ControlPlaneObservability::new(config).await;
        assert!(observability.is_ok());
    }

    #[tokio::test]
    async fn test_job_observability_lifecycle() {
        let config = ObservabilityConfig::default();
        let observability = ControlPlaneObservability::new(config).await.unwrap();

        let job_id = Uuid::new_v4();
        let mut context = observability
            .start_job_observability(job_id, Some("test-pipeline".to_string()), Some("node-1".to_string()))
            .await
            .unwrap();

        observability
            .update_job_status(&mut context, JobTraceStatus::Running, Some("node-1".to_string()), None)
            .await
            .unwrap();

        observability
            .complete_job_observability(context, true, Some("node-1".to_string()))
            .await
            .unwrap();

        let status = observability.get_observability_status().await;
        assert_eq!(status.active_job_traces, 0);
    }

    #[tokio::test]
    async fn test_node_event_recording() {
        let config = ObservabilityConfig::default();
        let observability = ControlPlaneObservability::new(config).await.unwrap();

        let mut resource_usage = HashMap::new();
        resource_usage.insert("cpu_percent".to_string(), 75.0);
        resource_usage.insert("memory_percent".to_string(), 60.0);

        observability
            .record_node_event(
                "test-node-1".to_string(),
                "node_joined",
                "Node joined the cluster",
                Some("healthy".to_string()),
                Some(resource_usage),
            )
            .await
            .unwrap();

        let status = observability.get_observability_status().await;
        assert!(status.metrics.nodes_total >= 1);
    }
}