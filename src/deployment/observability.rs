use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: Uuid,
    pub span_id: Uuid,
    pub parent_span_id: Option<Uuid>,
    pub correlation_id: Uuid,
    pub service_name: String,
    pub operation_name: String,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub duration: Option<Duration>,
    pub status: SpanStatus,
    pub tags: HashMap<String, String>,
    pub logs: Vec<SpanLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredLog {
    pub log_id: Uuid,
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub service: String,
    pub component: String,
    pub message: String,
    pub correlation_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub fields: HashMap<String, serde_json::Value>,
    pub error_details: Option<ErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub error_type: String,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub metric_name: String,
    pub value: f64,
    pub timestamp: SystemTime,
    pub tags: HashMap<String, String>,
    pub metric_type: MetricType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub check_id: Uuid,
    pub service_name: String,
    pub check_name: String,
    pub status: HealthStatus,
    pub timestamp: SystemTime,
    pub response_time: Duration,
    pub details: HashMap<String, serde_json::Value>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_id: Uuid,
    pub alert_name: String,
    pub severity: AlertSeverity,
    pub status: AlertStatus,
    pub triggered_at: SystemTime,
    pub resolved_at: Option<SystemTime>,
    pub service: String,
    pub description: String,
    pub metric_name: String,
    pub threshold: f64,
    pub current_value: f64,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Info = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertStatus {
    Triggered,
    Acknowledged,
    Resolved,
    Suppressed,
}

#[async_trait::async_trait]
pub trait DistributedTracer: Send + Sync {
    async fn start_span(&self, operation_name: &str, parent_context: Option<TraceContext>) -> Result<TraceContext>;
    async fn finish_span(&self, context: &mut TraceContext, status: SpanStatus) -> Result<()>;
    async fn add_span_tag(&self, context: &mut TraceContext, key: &str, value: &str) -> Result<()>;
    async fn add_span_log(&self, context: &mut TraceContext, level: LogLevel, message: &str, fields: HashMap<String, serde_json::Value>) -> Result<()>;
    async fn get_trace(&self, trace_id: Uuid) -> Result<Vec<TraceContext>>;
    async fn search_traces(&self, service: Option<&str>, operation: Option<&str>, duration: Duration) -> Result<Vec<TraceContext>>;
}

#[async_trait::async_trait]
pub trait StructuredLogger: Send + Sync {
    async fn log(&self, log: StructuredLog) -> Result<()>;
    async fn log_with_context(&self, level: LogLevel, message: &str, context: Option<&TraceContext>, fields: HashMap<String, serde_json::Value>) -> Result<()>;
    async fn search_logs(&self, query: LogQuery) -> Result<Vec<StructuredLog>>;
    async fn get_log_statistics(&self, service: &str, duration: Duration) -> Result<LogStatistics>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub service: Option<String>,
    pub level: Option<LogLevel>,
    pub correlation_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
    pub message_contains: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStatistics {
    pub total_logs: u64,
    pub error_count: u64,
    pub warn_count: u64,
    pub info_count: u64,
    pub debug_count: u64,
    pub error_rate: f64,
    pub top_errors: Vec<ErrorSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub error_type: String,
    pub count: u64,
    pub percentage: f64,
    pub last_seen: SystemTime,
}

#[async_trait::async_trait]
pub trait MetricsCollector: Send + Sync {
    async fn record_metric(&self, metric: MetricPoint) -> Result<()>;
    async fn increment_counter(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()>;
    async fn set_gauge(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()>;
    async fn record_histogram(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()>;
    async fn get_metric_values(&self, name: &str, duration: Duration) -> Result<Vec<MetricPoint>>;
    async fn get_service_metrics(&self, service: &str) -> Result<ServiceMetrics>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetrics {
    pub service_name: String,
    pub request_rate: f64,
    pub error_rate: f64,
    pub response_time_p50: f64,
    pub response_time_p95: f64,
    pub response_time_p99: f64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub active_connections: u64,
}

#[async_trait::async_trait]
pub trait HealthMonitor: Send + Sync {
    async fn register_health_check(&self, service: &str, check_name: &str, check_fn: Box<dyn HealthCheckFn>) -> Result<()>;
    async fn run_health_check(&self, service: &str, check_name: &str) -> Result<HealthCheck>;
    async fn get_service_health(&self, service: &str) -> Result<Vec<HealthCheck>>;
    async fn get_overall_health(&self) -> Result<SystemHealth>;
}

#[async_trait::async_trait]
pub trait HealthCheckFn: Send + Sync {
    async fn check(&self) -> Result<(HealthStatus, HashMap<String, serde_json::Value>)>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub service_count: usize,
    pub healthy_services: usize,
    pub degraded_services: usize,
    pub unhealthy_services: usize,
    pub service_health: HashMap<String, HealthStatus>,
}

#[async_trait::async_trait]
pub trait AlertManager: Send + Sync {
    async fn create_alert(&self, alert: Alert) -> Result<Uuid>;
    async fn acknowledge_alert(&self, alert_id: Uuid, acknowledged_by: &str) -> Result<()>;
    async fn resolve_alert(&self, alert_id: Uuid, resolved_by: &str) -> Result<()>;
    async fn get_active_alerts(&self, service: Option<&str>) -> Result<Vec<Alert>>;
    async fn get_alert_history(&self, service: Option<&str>, duration: Duration) -> Result<Vec<Alert>>;
}

pub struct ProductionObservabilitySystem {
    tracer: Arc<dyn DistributedTracer>,
    logger: Arc<dyn StructuredLogger>,
    metrics: Arc<dyn MetricsCollector>,
    health_monitor: Arc<dyn HealthMonitor>,
    alert_manager: Arc<dyn AlertManager>,
}

impl ProductionObservabilitySystem {
    pub fn new(
        tracer: Arc<dyn DistributedTracer>,
        logger: Arc<dyn StructuredLogger>,
        metrics: Arc<dyn MetricsCollector>,
        health_monitor: Arc<dyn HealthMonitor>,
        alert_manager: Arc<dyn AlertManager>,
    ) -> Self {
        Self {
            tracer,
            logger,
            metrics,
            health_monitor,
            alert_manager,
        }
    }

    pub async fn trace_operation<F, T>(&self, operation_name: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let mut span = self.tracer.start_span(operation_name, None).await?;
        
        let start_time = SystemTime::now();
        let result = operation.await;
        let duration = start_time.elapsed().unwrap_or(Duration::ZERO);

        match &result {
            Ok(_) => {
                span.status = SpanStatus::Ok;
                self.tracer.add_span_tag(&mut span, "success", "true").await?;
            }
            Err(e) => {
                span.status = SpanStatus::Error;
                self.tracer.add_span_tag(&mut span, "success", "false").await?;
                self.tracer.add_span_tag(&mut span, "error", &e.to_string()).await?;
            }
        }

        span.duration = Some(duration);
        self.tracer.finish_span(&mut span, span.status.clone()).await?;

        result
    }

    pub async fn log_with_trace(&self, level: LogLevel, message: &str, trace_context: Option<&TraceContext>) -> Result<()> {
        let fields = HashMap::new();
        self.logger.log_with_context(level, message, trace_context, fields).await
    }

    pub async fn record_request_metrics(&self, service: &str, endpoint: &str, method: &str, status_code: u16, duration: Duration) -> Result<()> {
        let mut tags = HashMap::new();
        tags.insert("service".to_string(), service.to_string());
        tags.insert("endpoint".to_string(), endpoint.to_string());
        tags.insert("method".to_string(), method.to_string());
        tags.insert("status_code".to_string(), status_code.to_string());

        // Record request count
        self.metrics.increment_counter("http_requests_total", 1.0, tags.clone()).await?;

        // Record response time
        self.metrics.record_histogram("http_request_duration_seconds", duration.as_secs_f64(), tags.clone()).await?;

        // Record error rate
        if status_code >= 400 {
            self.metrics.increment_counter("http_requests_errors_total", 1.0, tags).await?;
        }

        Ok(())
    }

    pub async fn check_and_alert(&self, service: &str, metric_name: &str, current_value: f64, threshold: f64, severity: AlertSeverity) -> Result<()> {
        if current_value > threshold {
            let alert = Alert {
                alert_id: Uuid::new_v4(),
                alert_name: format!("{} threshold exceeded", metric_name),
                severity,
                status: AlertStatus::Triggered,
                triggered_at: SystemTime::now(),
                resolved_at: None,
                service: service.to_string(),
                description: format!("{} is {} which exceeds threshold of {}", metric_name, current_value, threshold),
                metric_name: metric_name.to_string(),
                threshold,
                current_value,
                tags: HashMap::new(),
            };

            self.alert_manager.create_alert(alert).await?;
        }

        Ok(())
    }

    pub async fn get_service_observability_summary(&self, service: &str) -> Result<ServiceObservabilitySummary> {
        let health_checks = self.health_monitor.get_service_health(service).await?;
        let metrics = self.metrics.get_service_metrics(service).await?;
        let active_alerts = self.alert_manager.get_active_alerts(Some(service)).await?;
        let log_stats = self.logger.get_log_statistics(service, Duration::from_secs(3600)).await?;

        Ok(ServiceObservabilitySummary {
            service_name: service.to_string(),
            health_status: health_checks.first().map(|h| h.status.clone()).unwrap_or(HealthStatus::Unknown),
            metrics,
            active_alerts_count: active_alerts.len(),
            critical_alerts_count: active_alerts.iter().filter(|a| a.severity == AlertSeverity::Critical).count(),
            error_rate: log_stats.error_rate,
            last_updated: SystemTime::now(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceObservabilitySummary {
    pub service_name: String,
    pub health_status: HealthStatus,
    pub metrics: ServiceMetrics,
    pub active_alerts_count: usize,
    pub critical_alerts_count: usize,
    pub error_rate: f64,
    pub last_updated: SystemTime,
}

// Mock implementations for testing
pub struct MockDistributedTracer {
    traces: Arc<RwLock<HashMap<Uuid, Vec<TraceContext>>>>,
}

impl MockDistributedTracer {
    pub fn new() -> Self {
        Self {
            traces: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl DistributedTracer for MockDistributedTracer {
    async fn start_span(&self, operation_name: &str, parent_context: Option<TraceContext>) -> Result<TraceContext> {
        let trace_id = parent_context.as_ref().map(|p| p.trace_id).unwrap_or_else(Uuid::new_v4);
        let parent_span_id = parent_context.map(|p| p.span_id);

        Ok(TraceContext {
            trace_id,
            span_id: Uuid::new_v4(),
            parent_span_id,
            correlation_id: Uuid::new_v4(),
            service_name: "mock-service".to_string(),
            operation_name: operation_name.to_string(),
            started_at: SystemTime::now(),
            completed_at: None,
            duration: None,
            status: SpanStatus::Ok,
            tags: HashMap::new(),
            logs: Vec::new(),
        })
    }

    async fn finish_span(&self, context: &mut TraceContext, status: SpanStatus) -> Result<()> {
        context.completed_at = Some(SystemTime::now());
        context.duration = context.started_at.elapsed().ok();
        context.status = status;

        let mut traces = self.traces.write().await;
        let trace_spans = traces.entry(context.trace_id).or_insert_with(Vec::new);
        trace_spans.push(context.clone());

        Ok(())
    }

    async fn add_span_tag(&self, context: &mut TraceContext, key: &str, value: &str) -> Result<()> {
        context.tags.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn add_span_log(&self, context: &mut TraceContext, level: LogLevel, message: &str, fields: HashMap<String, serde_json::Value>) -> Result<()> {
        let log = SpanLog {
            timestamp: SystemTime::now(),
            level,
            message: message.to_string(),
            fields,
        };
        context.logs.push(log);
        Ok(())
    }

    async fn get_trace(&self, trace_id: Uuid) -> Result<Vec<TraceContext>> {
        let traces = self.traces.read().await;
        Ok(traces.get(&trace_id).cloned().unwrap_or_default())
    }

    async fn search_traces(&self, service: Option<&str>, operation: Option<&str>, _duration: Duration) -> Result<Vec<TraceContext>> {
        let traces = self.traces.read().await;
        let mut results = Vec::new();

        for spans in traces.values() {
            for span in spans {
                let service_match = service.map_or(true, |s| span.service_name == s);
                let operation_match = operation.map_or(true, |o| span.operation_name == o);
                
                if service_match && operation_match {
                    results.push(span.clone());
                }
            }
        }

        Ok(results)
    }
}

impl Default for MockDistributedTracer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockStructuredLogger {
    logs: Arc<RwLock<Vec<StructuredLog>>>,
}

impl MockStructuredLogger {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl StructuredLogger for MockStructuredLogger {
    async fn log(&self, log: StructuredLog) -> Result<()> {
        let mut logs = self.logs.write().await;
        logs.push(log);
        Ok(())
    }

    async fn log_with_context(&self, level: LogLevel, message: &str, context: Option<&TraceContext>, fields: HashMap<String, serde_json::Value>) -> Result<()> {
        let log = StructuredLog {
            log_id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            level,
            service: context.map(|c| c.service_name.clone()).unwrap_or_else(|| "unknown".to_string()),
            component: "mock".to_string(),
            message: message.to_string(),
            correlation_id: context.map(|c| c.correlation_id),
            trace_id: context.map(|c| c.trace_id),
            span_id: context.map(|c| c.span_id),
            fields,
            error_details: None,
        };

        self.log(log).await
    }

    async fn search_logs(&self, query: LogQuery) -> Result<Vec<StructuredLog>> {
        let logs = self.logs.read().await;
        let mut results = Vec::new();

        for log in logs.iter() {
            let service_match = query.service.as_ref().map_or(true, |s| log.service == *s);
            let level_match = query.level.as_ref().map_or(true, |l| std::mem::discriminant(&log.level) == std::mem::discriminant(l));
            let correlation_match = query.correlation_id.map_or(true, |c| log.correlation_id == Some(c));
            let trace_match = query.trace_id.map_or(true, |t| log.trace_id == Some(t));
            let message_match = query.message_contains.as_ref().map_or(true, |m| log.message.contains(m));

            if service_match && level_match && correlation_match && trace_match && message_match {
                results.push(log.clone());
                
                if let Some(limit) = query.limit {
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    async fn get_log_statistics(&self, service: &str, _duration: Duration) -> Result<LogStatistics> {
        let logs = self.logs.read().await;
        let service_logs: Vec<_> = logs.iter().filter(|l| l.service == service).collect();

        let total_logs = service_logs.len() as u64;
        let error_count = service_logs.iter().filter(|l| matches!(l.level, LogLevel::Error)).count() as u64;
        let warn_count = service_logs.iter().filter(|l| matches!(l.level, LogLevel::Warn)).count() as u64;
        let info_count = service_logs.iter().filter(|l| matches!(l.level, LogLevel::Info)).count() as u64;
        let debug_count = service_logs.iter().filter(|l| matches!(l.level, LogLevel::Debug)).count() as u64;

        let error_rate = if total_logs > 0 { error_count as f64 / total_logs as f64 } else { 0.0 };

        Ok(LogStatistics {
            total_logs,
            error_count,
            warn_count,
            info_count,
            debug_count,
            error_rate,
            top_errors: vec![], // Simplified for mock
        })
    }
}

impl Default for MockStructuredLogger {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockMetricsCollector {
    metrics: Arc<RwLock<Vec<MetricPoint>>>,
}

impl MockMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl MetricsCollector for MockMetricsCollector {
    async fn record_metric(&self, metric: MetricPoint) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        metrics.push(metric);
        Ok(())
    }

    async fn increment_counter(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()> {
        let metric = MetricPoint {
            metric_name: name.to_string(),
            value,
            timestamp: SystemTime::now(),
            tags,
            metric_type: MetricType::Counter,
        };
        self.record_metric(metric).await
    }

    async fn set_gauge(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()> {
        let metric = MetricPoint {
            metric_name: name.to_string(),
            value,
            timestamp: SystemTime::now(),
            tags,
            metric_type: MetricType::Gauge,
        };
        self.record_metric(metric).await
    }

    async fn record_histogram(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()> {
        let metric = MetricPoint {
            metric_name: name.to_string(),
            value,
            timestamp: SystemTime::now(),
            tags,
            metric_type: MetricType::Histogram,
        };
        self.record_metric(metric).await
    }

    async fn get_metric_values(&self, name: &str, _duration: Duration) -> Result<Vec<MetricPoint>> {
        let metrics = self.metrics.read().await;
        Ok(metrics.iter().filter(|m| m.metric_name == name).cloned().collect())
    }

    async fn get_service_metrics(&self, service: &str) -> Result<ServiceMetrics> {
        let _metrics = self.metrics.read().await;
        
        // Return mock metrics for the service
        Ok(ServiceMetrics {
            service_name: service.to_string(),
            request_rate: 100.0,
            error_rate: 0.01,
            response_time_p50: 0.1,
            response_time_p95: 0.5,
            response_time_p99: 1.0,
            cpu_usage: 45.0,
            memory_usage: 60.0,
            active_connections: 50,
        })
    }
}

impl Default for MockMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockHealthMonitor {
    health_checks: Arc<RwLock<HashMap<String, Vec<HealthCheck>>>>,
}

impl MockHealthMonitor {
    pub fn new() -> Self {
        Self {
            health_checks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl HealthMonitor for MockHealthMonitor {
    async fn register_health_check(&self, service: &str, check_name: &str, _check_fn: Box<dyn HealthCheckFn>) -> Result<()> {
        // In a real implementation, we would store the check function
        log::info!("Registered health check '{}' for service '{}'", check_name, service);
        Ok(())
    }

    async fn run_health_check(&self, service: &str, check_name: &str) -> Result<HealthCheck> {
        let health_check = HealthCheck {
            check_id: Uuid::new_v4(),
            service_name: service.to_string(),
            check_name: check_name.to_string(),
            status: HealthStatus::Healthy, // Mock always returns healthy
            timestamp: SystemTime::now(),
            response_time: Duration::from_millis(50),
            details: HashMap::new(),
            error_message: None,
        };

        let mut checks = self.health_checks.write().await;
        let service_checks = checks.entry(service.to_string()).or_insert_with(Vec::new);
        service_checks.push(health_check.clone());

        Ok(health_check)
    }

    async fn get_service_health(&self, service: &str) -> Result<Vec<HealthCheck>> {
        let checks = self.health_checks.read().await;
        Ok(checks.get(service).cloned().unwrap_or_default())
    }

    async fn get_overall_health(&self) -> Result<SystemHealth> {
        let checks = self.health_checks.read().await;
        let service_count = checks.len();
        let healthy_services = checks.values().filter(|checks| {
            checks.last().map_or(false, |c| matches!(c.status, HealthStatus::Healthy))
        }).count();

        Ok(SystemHealth {
            overall_status: if healthy_services == service_count { HealthStatus::Healthy } else { HealthStatus::Degraded },
            service_count,
            healthy_services,
            degraded_services: 0,
            unhealthy_services: service_count - healthy_services,
            service_health: HashMap::new(),
        })
    }
}

impl Default for MockHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockAlertManager {
    alerts: Arc<RwLock<HashMap<Uuid, Alert>>>,
}

impl MockAlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl AlertManager for MockAlertManager {
    async fn create_alert(&self, alert: Alert) -> Result<Uuid> {
        let alert_id = alert.alert_id;
        let mut alerts = self.alerts.write().await;
        alerts.insert(alert_id, alert);
        Ok(alert_id)
    }

    async fn acknowledge_alert(&self, alert_id: Uuid, _acknowledged_by: &str) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.get_mut(&alert_id) {
            alert.status = AlertStatus::Acknowledged;
        }
        Ok(())
    }

    async fn resolve_alert(&self, alert_id: Uuid, _resolved_by: &str) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.get_mut(&alert_id) {
            alert.status = AlertStatus::Resolved;
            alert.resolved_at = Some(SystemTime::now());
        }
        Ok(())
    }

    async fn get_active_alerts(&self, service: Option<&str>) -> Result<Vec<Alert>> {
        let alerts = self.alerts.read().await;
        let active_alerts: Vec<Alert> = alerts.values()
            .filter(|alert| matches!(alert.status, AlertStatus::Triggered | AlertStatus::Acknowledged))
            .filter(|alert| service.map_or(true, |s| alert.service == s))
            .cloned()
            .collect();
        Ok(active_alerts)
    }

    async fn get_alert_history(&self, service: Option<&str>, _duration: Duration) -> Result<Vec<Alert>> {
        let alerts = self.alerts.read().await;
        let filtered_alerts: Vec<Alert> = alerts.values()
            .filter(|alert| service.map_or(true, |s| alert.service == s))
            .cloned()
            .collect();
        Ok(filtered_alerts)
    }
}

impl Default for MockAlertManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_distributed_tracing() {
        let tracer = MockDistributedTracer::new();
        
        // Start a root span
        let mut root_span = tracer.start_span("http_request", None).await.unwrap();
        tracer.add_span_tag(&mut root_span, "method", "GET").await.unwrap();
        tracer.add_span_tag(&mut root_span, "url", "/api/users").await.unwrap();

        // Start a child span
        let mut child_span = tracer.start_span("database_query", Some(root_span.clone())).await.unwrap();
        tracer.add_span_tag(&mut child_span, "query", "SELECT * FROM users").await.unwrap();

        // Add logs to child span
        let mut fields = HashMap::new();
        fields.insert("rows_returned".to_string(), serde_json::Value::Number(serde_json::Number::from(42)));
        tracer.add_span_log(&mut child_span, LogLevel::Info, "Query executed successfully", fields).await.unwrap();

        // Finish spans
        tracer.finish_span(&mut child_span, SpanStatus::Ok).await.unwrap();
        tracer.finish_span(&mut root_span, SpanStatus::Ok).await.unwrap();

        // Retrieve trace
        let trace_spans = tracer.get_trace(root_span.trace_id).await.unwrap();
        assert_eq!(trace_spans.len(), 2);
        
        // Search traces
        let search_results = tracer.search_traces(Some("mock-service"), Some("http_request"), Duration::from_secs(3600)).await.unwrap();
        assert!(!search_results.is_empty());
    }

    #[tokio::test]
    async fn test_structured_logging() {
        let logger = MockStructuredLogger::new();
        
        // Log with context
        let trace_context = TraceContext {
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            correlation_id: Uuid::new_v4(),
            service_name: "test-service".to_string(),
            operation_name: "test_operation".to_string(),
            started_at: SystemTime::now(),
            completed_at: None,
            duration: None,
            status: SpanStatus::Ok,
            tags: HashMap::new(),
            logs: Vec::new(),
        };

        let mut fields = HashMap::new();
        fields.insert("user_id".to_string(), serde_json::Value::String("12345".to_string()));
        
        logger.log_with_context(LogLevel::Info, "User logged in", Some(&trace_context), fields).await.unwrap();

        // Search logs
        let query = LogQuery {
            service: Some("test-service".to_string()),
            level: Some(LogLevel::Info),
            correlation_id: Some(trace_context.correlation_id),
            trace_id: None,
            start_time: None,
            end_time: None,
            message_contains: Some("logged in".to_string()),
            limit: Some(10),
        };

        let search_results = logger.search_logs(query).await.unwrap();
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].message, "User logged in");

        // Get statistics
        let stats = logger.get_log_statistics("test-service", Duration::from_secs(3600)).await.unwrap();
        assert_eq!(stats.total_logs, 1);
        assert_eq!(stats.info_count, 1);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let metrics = MockMetricsCollector::new();
        
        let mut tags = HashMap::new();
        tags.insert("service".to_string(), "api".to_string());
        tags.insert("endpoint".to_string(), "/users".to_string());

        // Record different types of metrics
        metrics.increment_counter("http_requests_total", 1.0, tags.clone()).await.unwrap();
        metrics.set_gauge("active_connections", 50.0, tags.clone()).await.unwrap();
        metrics.record_histogram("response_time_seconds", 0.25, tags.clone()).await.unwrap();

        // Get metric values
        let counter_values = metrics.get_metric_values("http_requests_total", Duration::from_secs(3600)).await.unwrap();
        assert_eq!(counter_values.len(), 1);
        assert_eq!(counter_values[0].value, 1.0);

        // Get service metrics
        let service_metrics = metrics.get_service_metrics("api").await.unwrap();
        assert_eq!(service_metrics.service_name, "api");
        assert!(service_metrics.request_rate > 0.0);
    }

    #[tokio::test]
    async fn test_health_monitoring() {
        let health_monitor = MockHealthMonitor::new();
        
        // Register health check (mock implementation doesn't store the function)
        struct MockHealthCheckFn;
        
        #[async_trait::async_trait]
        impl HealthCheckFn for MockHealthCheckFn {
            async fn check(&self) -> Result<(HealthStatus, HashMap<String, serde_json::Value>)> {
                Ok((HealthStatus::Healthy, HashMap::new()))
            }
        }

        health_monitor.register_health_check("api-service", "database", Box::new(MockHealthCheckFn)).await.unwrap();

        // Run health check
        let health_check = health_monitor.run_health_check("api-service", "database").await.unwrap();
        assert_eq!(health_check.service_name, "api-service");
        assert_eq!(health_check.check_name, "database");
        assert!(matches!(health_check.status, HealthStatus::Healthy));

        // Get service health
        let service_health = health_monitor.get_service_health("api-service").await.unwrap();
        assert_eq!(service_health.len(), 1);

        // Get overall health
        let overall_health = health_monitor.get_overall_health().await.unwrap();
        assert!(matches!(overall_health.overall_status, HealthStatus::Healthy));
    }

    #[tokio::test]
    async fn test_alert_management() {
        let alert_manager = MockAlertManager::new();
        
        let alert = Alert {
            alert_id: Uuid::new_v4(),
            alert_name: "High CPU Usage".to_string(),
            severity: AlertSeverity::High,
            status: AlertStatus::Triggered,
            triggered_at: SystemTime::now(),
            resolved_at: None,
            service: "api-service".to_string(),
            description: "CPU usage is above 80%".to_string(),
            metric_name: "cpu_usage".to_string(),
            threshold: 80.0,
            current_value: 85.0,
            tags: HashMap::new(),
        };

        let alert_id = alert_manager.create_alert(alert).await.unwrap();

        // Get active alerts
        let active_alerts = alert_manager.get_active_alerts(Some("api-service")).await.unwrap();
        assert_eq!(active_alerts.len(), 1);
        assert_eq!(active_alerts[0].alert_id, alert_id);

        // Acknowledge alert
        alert_manager.acknowledge_alert(alert_id, "admin").await.unwrap();

        // Resolve alert
        alert_manager.resolve_alert(alert_id, "admin").await.unwrap();

        // Get alert history
        let alert_history = alert_manager.get_alert_history(Some("api-service"), Duration::from_secs(3600)).await.unwrap();
        assert_eq!(alert_history.len(), 1);
    }

    #[tokio::test]
    async fn test_observability_system_integration() {
        let tracer = Arc::new(MockDistributedTracer::new());
        let logger = Arc::new(MockStructuredLogger::new());
        let metrics = Arc::new(MockMetricsCollector::new());
        let health_monitor = Arc::new(MockHealthMonitor::new());
        let alert_manager = Arc::new(MockAlertManager::new());

        let observability = ProductionObservabilitySystem::new(
            tracer, logger, metrics, health_monitor, alert_manager
        );

        // Test traced operation
        let result = observability.trace_operation("test_operation", async {
            Ok::<_, AppError>("success")
        }).await;
        assert!(result.is_ok());

        // Test request metrics recording
        observability.record_request_metrics(
            "api-service",
            "/users",
            "GET",
            200,
            Duration::from_millis(150)
        ).await.unwrap();

        // Test alerting
        observability.check_and_alert(
            "api-service",
            "response_time",
            0.8,
            0.5,
            AlertSeverity::Medium
        ).await.unwrap();

        // Get service observability summary
        let summary = observability.get_service_observability_summary("api-service").await.unwrap();
        assert_eq!(summary.service_name, "api-service");
        assert!(summary.active_alerts_count > 0);
    }
}