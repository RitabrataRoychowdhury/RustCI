use crate::error::{AppError, Result};
use opentelemetry::{global, Context};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

/// Distributed tracing service for control plane operations
pub struct DistributedTracing {
    active_traces: Arc<RwLock<HashMap<Uuid, TraceContext>>>,
    config: TracingConfig,
}

#[derive(Debug, Clone)]
pub struct TracingConfig {
    pub service_name: String,
    pub jaeger_endpoint: String,
    pub sample_rate: f64,
    pub max_events_per_span: u32,
    pub max_attributes_per_span: u32,
    pub enable_console_exporter: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "rustci-control-plane".to_string(),
            jaeger_endpoint: "http://localhost:14268/api/traces".to_string(),
            sample_rate: 1.0, // Sample all traces in development
            max_events_per_span: 128,
            max_attributes_per_span: 128,
            enable_console_exporter: false,
        }
    }
}

/// Context for a distributed trace
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub correlation_id: Uuid,
    pub operation: String,
    pub component: String,
    pub start_time: std::time::Instant,
    pub attributes: HashMap<String, String>,
    pub events: Vec<TraceEvent>,
}

/// Event within a trace
#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub name: String,
    pub timestamp: std::time::Instant,
    pub attributes: HashMap<String, String>,
    pub level: TraceEventLevel,
}

#[derive(Debug, Clone)]
pub enum TraceEventLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Job lifecycle trace
#[derive(Debug, Clone)]
pub struct JobTrace {
    pub job_id: Uuid,
    pub correlation_id: Uuid,
    pub pipeline_id: Option<String>,
    pub node_id: Option<String>,
    pub stages: Vec<JobStage>,
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
    pub status: JobTraceStatus,
}

#[derive(Debug, Clone)]
pub enum JobTraceStatus {
    Submitted,
    Scheduled,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct JobStage {
    pub name: String,
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
    pub node_id: Option<String>,
    pub status: JobStageStatus,
    pub attributes: HashMap<String, String>,
    pub events: Vec<TraceEvent>,
}

#[derive(Debug, Clone)]
pub enum JobStageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl DistributedTracing {
    /// Initialize distributed tracing
    pub async fn new(config: TracingConfig) -> Result<Self> {
        info!(
            "Distributed tracing initialized with service name: {}",
            config.service_name
        );

        let service = Self {
            active_traces: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        // Initialize OpenTelemetry if enabled
        if !service.config.jaeger_endpoint.is_empty() {
            service.init_opentelemetry().await?;
        }

        Ok(service)
    }

    /// Start a new distributed trace
    pub async fn start_trace(
        &self,
        operation: &str,
        component: &str,
        correlation_id: Option<Uuid>,
    ) -> Uuid {
        let correlation_id = correlation_id.unwrap_or_else(Uuid::new_v4);
        let trace_id = Uuid::new_v4().to_string();

        let trace_context = TraceContext {
            trace_id,
            correlation_id,
            operation: operation.to_string(),
            component: component.to_string(),
            start_time: std::time::Instant::now(),
            attributes: HashMap::new(),
            events: Vec::new(),
        };

        let mut active_traces = self.active_traces.write().await;
        active_traces.insert(correlation_id, trace_context);

        debug!(
            "Started trace for operation '{}' in component '{}' with correlation_id: {}",
            operation, component, correlation_id
        );

        correlation_id
    }

    /// Add attributes to an active trace
    pub async fn add_trace_attributes(
        &self,
        correlation_id: Uuid,
        attributes: HashMap<String, String>,
    ) {
        let mut active_traces = self.active_traces.write().await;
        if let Some(trace_context) = active_traces.get_mut(&correlation_id) {
            trace_context.attributes.extend(attributes.clone());

            // Log attributes with current span
            let current_span = Span::current();
            for (key, value) in attributes {
                current_span.record(key.as_str(), value.as_str());
            }
        }
    }

    /// Add an event to an active trace
    pub async fn add_trace_event(
        &self,
        correlation_id: Uuid,
        event_name: &str,
        level: TraceEventLevel,
        attributes: HashMap<String, String>,
    ) {
        let mut active_traces = self.active_traces.write().await;
        if let Some(trace_context) = active_traces.get_mut(&correlation_id) {
            let event = TraceEvent {
                name: event_name.to_string(),
                timestamp: std::time::Instant::now(),
                attributes: attributes.clone(),
                level: level.clone(),
            };

            trace_context.events.push(event);

            // Log event with current span
            match level {
                TraceEventLevel::Debug => {
                    debug!(
                        correlation_id = %correlation_id,
                        event_name = event_name,
                        attributes = ?attributes,
                        "Trace event"
                    );
                }
                TraceEventLevel::Info => {
                    info!(
                        correlation_id = %correlation_id,
                        event_name = event_name,
                        attributes = ?attributes,
                        "Trace event"
                    );
                }
                TraceEventLevel::Warn => {
                    warn!(
                        correlation_id = %correlation_id,
                        event_name = event_name,
                        attributes = ?attributes,
                        "Trace event"
                    );
                }
                TraceEventLevel::Error => {
                    tracing::error!(
                        correlation_id = %correlation_id,
                        event_name = event_name,
                        attributes = ?attributes,
                        "Trace event"
                    );
                }
            }
        }
    }

    /// End a distributed trace
    pub async fn end_trace(&self, correlation_id: Uuid, success: bool) {
        let mut active_traces = self.active_traces.write().await;
        if let Some(trace_context) = active_traces.remove(&correlation_id) {
            let duration = trace_context.start_time.elapsed();

            if success {
                info!(
                    correlation_id = %correlation_id,
                    operation = trace_context.operation,
                    component = trace_context.component,
                    duration_ms = duration.as_millis(),
                    "Completed trace"
                );
            } else {
                warn!(
                    correlation_id = %correlation_id,
                    operation = trace_context.operation,
                    component = trace_context.component,
                    duration_ms = duration.as_millis(),
                    "Failed trace"
                );
            }
        }
    }

    /// Start job lifecycle tracing
    pub async fn start_job_trace(
        &self,
        job_id: Uuid,
        pipeline_id: Option<String>,
        correlation_id: Option<Uuid>,
    ) -> JobTrace {
        let correlation_id = correlation_id.unwrap_or_else(Uuid::new_v4);

        // Start distributed trace for the job
        self.start_trace("job_execution", "job_scheduler", Some(correlation_id))
            .await;

        let mut attributes = HashMap::new();
        attributes.insert("job_id".to_string(), job_id.to_string());
        if let Some(ref pipeline_id) = pipeline_id {
            attributes.insert("pipeline_id".to_string(), pipeline_id.clone());
        }

        self.add_trace_attributes(correlation_id, attributes).await;

        JobTrace {
            job_id,
            correlation_id,
            pipeline_id,
            node_id: None,
            stages: Vec::new(),
            start_time: std::time::Instant::now(),
            end_time: None,
            status: JobTraceStatus::Submitted,
        }
    }

    /// Update job trace status
    pub async fn update_job_trace_status(&self, job_trace: &mut JobTrace, status: JobTraceStatus) {
        job_trace.status = status.clone();

        let mut attributes = HashMap::new();
        attributes.insert("job_status".to_string(), format!("{:?}", status));

        self.add_trace_attributes(job_trace.correlation_id, attributes)
            .await;

        let event_name = match status {
            JobTraceStatus::Submitted => "job_submitted",
            JobTraceStatus::Scheduled => "job_scheduled",
            JobTraceStatus::Running => "job_started",
            JobTraceStatus::Completed => "job_completed",
            JobTraceStatus::Failed => "job_failed",
            JobTraceStatus::Cancelled => "job_cancelled",
        };

        let level = match status {
            JobTraceStatus::Failed => TraceEventLevel::Error,
            JobTraceStatus::Cancelled => TraceEventLevel::Warn,
            _ => TraceEventLevel::Info,
        };

        self.add_trace_event(job_trace.correlation_id, event_name, level, HashMap::new())
            .await;
    }

    /// Add job stage to trace
    pub async fn add_job_stage(
        &self,
        job_trace: &mut JobTrace,
        stage_name: &str,
        node_id: Option<String>,
    ) -> usize {
        let stage = JobStage {
            name: stage_name.to_string(),
            start_time: std::time::Instant::now(),
            end_time: None,
            node_id: node_id.clone(),
            status: JobStageStatus::Pending,
            attributes: HashMap::new(),
            events: Vec::new(),
        };

        job_trace.stages.push(stage);
        let stage_index = job_trace.stages.len() - 1;

        let mut attributes = HashMap::new();
        attributes.insert("stage_name".to_string(), stage_name.to_string());
        if let Some(node_id) = node_id {
            attributes.insert("node_id".to_string(), node_id);
        }

        self.add_trace_event(
            job_trace.correlation_id,
            "job_stage_added",
            TraceEventLevel::Info,
            attributes,
        )
        .await;

        stage_index
    }

    /// Update job stage status
    pub async fn update_job_stage_status(
        &self,
        job_trace: &mut JobTrace,
        stage_index: usize,
        status: JobStageStatus,
    ) {
        if let Some(stage) = job_trace.stages.get_mut(stage_index) {
            stage.status = status.clone();

            if matches!(
                status,
                JobStageStatus::Completed | JobStageStatus::Failed | JobStageStatus::Skipped
            ) {
                stage.end_time = Some(std::time::Instant::now());
            }

            let mut attributes = HashMap::new();
            attributes.insert("stage_name".to_string(), stage.name.clone());
            attributes.insert("stage_status".to_string(), format!("{:?}", status));

            let event_name = match status {
                JobStageStatus::Pending => "job_stage_pending",
                JobStageStatus::Running => "job_stage_started",
                JobStageStatus::Completed => "job_stage_completed",
                JobStageStatus::Failed => "job_stage_failed",
                JobStageStatus::Skipped => "job_stage_skipped",
            };

            let level = match status {
                JobStageStatus::Failed => TraceEventLevel::Error,
                JobStageStatus::Skipped => TraceEventLevel::Warn,
                _ => TraceEventLevel::Info,
            };

            self.add_trace_event(job_trace.correlation_id, event_name, level, attributes)
                .await;
        }
    }

    /// Complete job trace
    pub async fn complete_job_trace(&self, job_trace: &mut JobTrace, success: bool) {
        job_trace.end_time = Some(std::time::Instant::now());
        job_trace.status = if success {
            JobTraceStatus::Completed
        } else {
            JobTraceStatus::Failed
        };

        let duration = job_trace.start_time.elapsed();

        let mut attributes = HashMap::new();
        attributes.insert(
            "job_duration_ms".to_string(),
            duration.as_millis().to_string(),
        );
        attributes.insert("job_success".to_string(), success.to_string());
        attributes.insert(
            "stages_count".to_string(),
            job_trace.stages.len().to_string(),
        );

        self.add_trace_attributes(job_trace.correlation_id, attributes)
            .await;

        self.end_trace(job_trace.correlation_id, success).await;

        if success {
            info!(
                "Job trace completed successfully: job_id={}, duration={:?}, stages={}",
                job_trace.job_id,
                duration,
                job_trace.stages.len()
            );
        } else {
            warn!(
                "Job trace completed with failure: job_id={}, duration={:?}, stages={}",
                job_trace.job_id,
                duration,
                job_trace.stages.len()
            );
        }
    }

    /// Get active traces count
    pub async fn get_active_traces_count(&self) -> usize {
        let active_traces = self.active_traces.read().await;
        active_traces.len()
    }

    /// Get trace context by correlation ID
    pub async fn get_trace_context(&self, correlation_id: Uuid) -> Option<TraceContext> {
        let active_traces = self.active_traces.read().await;
        active_traces.get(&correlation_id).cloned()
    }

    /// Create a child span for the current trace
    pub fn create_child_span(&self, name: &str) -> tracing::Span {
        let span = tracing::info_span!(
            "control_plane_operation",
            operation = name,
            service = self.config.service_name.as_str()
        );

        // Set OpenTelemetry context
        let cx = Context::current();
        span.set_parent(cx);

        span
    }

    /// Initialize OpenTelemetry tracing
    pub async fn init_opentelemetry(&self) -> Result<()> {
        use opentelemetry_jaeger::new_agent_pipeline;
        // Tracing subscriber imports moved to where they're used

        // Create Jaeger tracer
        let tracer = new_agent_pipeline()
            .with_service_name(&self.config.service_name)
            .with_endpoint(&self.config.jaeger_endpoint)
            .install_simple()
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to initialize Jaeger tracer: {}", e))
            })?;

        // Set global tracer
        global::set_tracer_provider(tracer.provider().unwrap());

        info!(
            "OpenTelemetry tracing initialized with Jaeger endpoint: {}",
            self.config.jaeger_endpoint
        );
        Ok(())
    }

    /// Instrument an async function with tracing
    pub async fn instrument_async<F, T>(&self, operation: &str, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let span = self.create_child_span(operation);
        let _enter = span.enter();

        let start_time = std::time::Instant::now();
        let result = future.await;
        let duration = start_time.elapsed();

        span.record("duration_ms", duration.as_millis());
        result
    }
}

/// Macro for easy trace instrumentation
#[macro_export]
macro_rules! trace_operation {
    ($tracer:expr, $operation:expr, $code:block) => {{
        let correlation_id = $tracer.start_trace($operation, "control_plane", None).await;
        let result = $code;
        $tracer.end_trace(correlation_id, true).await;
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_distributed_tracing_creation() {
        let config = TracingConfig::default();
        let tracing = DistributedTracing::new(config).await;
        assert!(tracing.is_ok());
    }

    #[tokio::test]
    async fn test_trace_lifecycle() {
        let config = TracingConfig::default();
        let tracing = DistributedTracing::new(config).await.unwrap();

        let correlation_id = tracing
            .start_trace("test_operation", "test_component", None)
            .await;

        let mut attributes = HashMap::new();
        attributes.insert("test_key".to_string(), "test_value".to_string());
        tracing
            .add_trace_attributes(correlation_id, attributes)
            .await;

        tracing
            .add_trace_event(
                correlation_id,
                "test_event",
                TraceEventLevel::Info,
                HashMap::new(),
            )
            .await;

        tracing.end_trace(correlation_id, true).await;

        assert_eq!(tracing.get_active_traces_count().await, 0);
    }

    #[tokio::test]
    async fn test_job_trace_lifecycle() {
        let config = TracingConfig::default();
        let tracing = DistributedTracing::new(config).await.unwrap();

        let job_id = Uuid::new_v4();
        let mut job_trace = tracing
            .start_job_trace(job_id, Some("test-pipeline".to_string()), None)
            .await;

        tracing
            .update_job_trace_status(&mut job_trace, JobTraceStatus::Scheduled)
            .await;

        let stage_index = tracing
            .add_job_stage(&mut job_trace, "build", Some("node-1".to_string()))
            .await;

        tracing
            .update_job_stage_status(&mut job_trace, stage_index, JobStageStatus::Running)
            .await;

        sleep(Duration::from_millis(10)).await;

        tracing
            .update_job_stage_status(&mut job_trace, stage_index, JobStageStatus::Completed)
            .await;

        tracing.complete_job_trace(&mut job_trace, true).await;

        assert_eq!(job_trace.stages.len(), 1);
        assert!(matches!(job_trace.status, JobTraceStatus::Completed));
    }

    #[tokio::test]
    async fn test_instrumentation() {
        let config = TracingConfig::default();
        let tracing = DistributedTracing::new(config).await.unwrap();

        let result = tracing
            .instrument_async("test_operation", async {
                sleep(Duration::from_millis(10)).await;
                "test_result"
            })
            .await;

        assert_eq!(result, "test_result");
    }
}
