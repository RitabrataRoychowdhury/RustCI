use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, Event, Subscriber};
use tracing_subscriber::{
    fmt::{format::Writer, FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};
use uuid::Uuid;

/// Structured logging service for control plane operations
pub struct StructuredLogging {
    correlation_tracker: Arc<CorrelationTracker>,
    log_buffer: Arc<RwLock<Vec<LogEntry>>>,
    config: LoggingConfig,
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub service_name: String,
    pub environment: String,
    pub log_level: String,
    pub enable_json_format: bool,
    pub enable_console_output: bool,
    pub enable_file_output: bool,
    pub file_path: Option<String>,
    pub max_buffer_size: usize,
    pub enable_correlation_tracking: bool,
    pub enable_performance_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            service_name: "rustci-control-plane".to_string(),
            environment: std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()),
            log_level: "info".to_string(),
            enable_json_format: true,
            enable_console_output: true,
            enable_file_output: false,
            file_path: None,
            max_buffer_size: 10000,
            enable_correlation_tracking: true,
            enable_performance_logging: true,
        }
    }
}

/// Correlation tracking for distributed operations
pub struct CorrelationTracker {
    active_correlations: Arc<RwLock<HashMap<Uuid, CorrelationContext>>>,
}

#[derive(Debug, Clone)]
pub struct CorrelationContext {
    pub correlation_id: Uuid,
    pub operation: String,
    pub component: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub parent_correlation_id: Option<Uuid>,
    pub start_time: SystemTime,
    pub attributes: HashMap<String, String>,
    pub breadcrumbs: Vec<String>,
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
    pub service: String,
    pub environment: String,
    pub correlation_id: Option<String>,
    pub operation: Option<String>,
    pub component: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub duration_ms: Option<u64>,
    pub attributes: HashMap<String, serde_json::Value>,
    pub error: Option<ErrorInfo>,
    pub performance: Option<PerformanceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub error_type: String,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceInfo {
    pub operation_duration_ms: u64,
    pub memory_usage_bytes: Option<u64>,
    pub cpu_usage_percent: Option<f64>,
    pub db_query_count: Option<u32>,
    pub db_query_duration_ms: Option<u64>,
    pub cache_hits: Option<u32>,
    pub cache_misses: Option<u32>,
}

impl CorrelationTracker {
    pub fn new() -> Self {
        Self {
            active_correlations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a new correlation context
    pub async fn start_correlation(
        &self,
        operation: String,
        component: String,
        parent_correlation_id: Option<Uuid>,
    ) -> Uuid {
        let correlation_id = Uuid::new_v4();
        
        let context = CorrelationContext {
            correlation_id,
            operation: operation.clone(),
            component: component.clone(),
            user_id: None,
            session_id: None,
            request_id: None,
            parent_correlation_id,
            start_time: SystemTime::now(),
            attributes: HashMap::new(),
            breadcrumbs: vec![format!("Started {} in {}", operation, component)],
        };

        let mut correlations = self.active_correlations.write().await;
        correlations.insert(correlation_id, context);

        debug!(
            correlation_id = %correlation_id,
            operation = operation,
            component = component,
            parent_correlation_id = ?parent_correlation_id,
            "Started correlation context"
        );

        correlation_id
    }

    /// Update correlation context
    pub async fn update_correlation(
        &self,
        correlation_id: Uuid,
        user_id: Option<String>,
        session_id: Option<String>,
        request_id: Option<String>,
        attributes: HashMap<String, String>,
    ) {
        let mut correlations = self.active_correlations.write().await;
        if let Some(context) = correlations.get_mut(&correlation_id) {
            if let Some(user_id) = user_id {
                context.user_id = Some(user_id);
            }
            if let Some(session_id) = session_id {
                context.session_id = Some(session_id);
            }
            if let Some(request_id) = request_id {
                context.request_id = Some(request_id);
            }
            context.attributes.extend(attributes);
        }
    }

    /// Add breadcrumb to correlation context
    pub async fn add_breadcrumb(&self, correlation_id: Uuid, breadcrumb: String) {
        let mut correlations = self.active_correlations.write().await;
        if let Some(context) = correlations.get_mut(&correlation_id) {
            context.breadcrumbs.push(breadcrumb);
            
            // Keep only last 50 breadcrumbs to prevent memory issues
            if context.breadcrumbs.len() > 50 {
                context.breadcrumbs.remove(0);
            }
        }
    }

    /// Get correlation context
    pub async fn get_correlation(&self, correlation_id: Uuid) -> Option<CorrelationContext> {
        let correlations = self.active_correlations.read().await;
        correlations.get(&correlation_id).cloned()
    }

    /// End correlation context
    pub async fn end_correlation(&self, correlation_id: Uuid) -> Option<CorrelationContext> {
        let mut correlations = self.active_correlations.write().await;
        let context = correlations.remove(&correlation_id);

        if let Some(ref context) = context {
            let duration = context.start_time.elapsed().unwrap_or_default();
            debug!(
                correlation_id = %correlation_id,
                operation = context.operation,
                component = context.component,
                duration_ms = duration.as_millis(),
                breadcrumbs_count = context.breadcrumbs.len(),
                "Ended correlation context"
            );
        }

        context
    }

    /// Get active correlations count
    pub async fn get_active_count(&self) -> usize {
        let correlations = self.active_correlations.read().await;
        correlations.len()
    }
}

impl Default for CorrelationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl StructuredLogging {
    /// Initialize structured logging
    pub fn new(config: LoggingConfig) -> Result<Self> {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        
        // Initialize tracing subscriber
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

        let mut layers = Vec::new();

        // Console output layer
        if config.enable_console_output {
            if config.enable_json_format {
                let console_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .event_format(StructuredFormatter::new(
                        config.service_name.clone(),
                        config.environment.clone(),
                        Arc::clone(&correlation_tracker),
                    ));
                layers.push(console_layer.boxed());
            } else {
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true);
                layers.push(console_layer.boxed());
            }
        }

        // File output layer
        if config.enable_file_output {
            if let Some(ref file_path) = config.file_path {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)
                    .map_err(|e| AppError::InternalServerError(format!("Failed to open log file: {}", e)))?;

                let file_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(Arc::new(file))
                    .with_current_span(true)
                    .with_span_list(true)
                    .event_format(StructuredFormatter::new(
                        config.service_name.clone(),
                        config.environment.clone(),
                        Arc::clone(&correlation_tracker),
                    ));
                layers.push(file_layer.boxed());
            }
        }

        // Initialize subscriber
        tracing_subscriber::registry()
            .with(env_filter)
            .with(layers)
            .init();

        info!(
            service = config.service_name,
            environment = config.environment,
            "Structured logging initialized"
        );

        Ok(Self {
            correlation_tracker,
            log_buffer: Arc::new(RwLock::new(Vec::new())),
            config,
        })
    }

    /// Get correlation tracker
    pub fn correlation_tracker(&self) -> Arc<CorrelationTracker> {
        Arc::clone(&self.correlation_tracker)
    }

    /// Log with correlation context
    pub async fn log_with_correlation(
        &self,
        level: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        operation: Option<String>,
        component: Option<String>,
        attributes: HashMap<String, serde_json::Value>,
        error: Option<ErrorInfo>,
        performance: Option<PerformanceInfo>,
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let (correlation_context, duration_ms) = if let Some(correlation_id) = correlation_id {
            let context = self.correlation_tracker.get_correlation(correlation_id).await;
            let duration = context.as_ref().map(|c| 
                c.start_time.elapsed().unwrap_or_default().as_millis() as u64
            );
            (context, duration)
        } else {
            (None, None)
        };

        let log_entry = LogEntry {
            timestamp,
            level: level.to_string(),
            message: message.to_string(),
            service: self.config.service_name.clone(),
            environment: self.config.environment.clone(),
            correlation_id: correlation_id.map(|id| id.to_string()),
            operation: operation.or_else(|| correlation_context.as_ref().map(|c| c.operation.clone())),
            component: component.or_else(|| correlation_context.as_ref().map(|c| c.component.clone())),
            user_id: correlation_context.as_ref().and_then(|c| c.user_id.clone()),
            session_id: correlation_context.as_ref().and_then(|c| c.session_id.clone()),
            request_id: correlation_context.as_ref().and_then(|c| c.request_id.clone()),
            duration_ms,
            attributes,
            error,
            performance,
        };

        // Add to buffer
        let mut buffer = self.log_buffer.write().await;
        buffer.push(log_entry.clone());

        // Trim buffer if it exceeds max size
        if buffer.len() > self.config.max_buffer_size {
            buffer.remove(0);
        }

        // Log using tracing macros with structured data
        match level {
            "error" => error!(
                correlation_id = ?correlation_id,
                operation = ?log_entry.operation,
                component = ?log_entry.component,
                user_id = ?log_entry.user_id,
                duration_ms = ?log_entry.duration_ms,
                attributes = ?log_entry.attributes,
                error = ?log_entry.error,
                "{}", message
            ),
            "warn" => warn!(
                correlation_id = ?correlation_id,
                operation = ?log_entry.operation,
                component = ?log_entry.component,
                user_id = ?log_entry.user_id,
                duration_ms = ?log_entry.duration_ms,
                attributes = ?log_entry.attributes,
                "{}", message
            ),
            "debug" => debug!(
                correlation_id = ?correlation_id,
                operation = ?log_entry.operation,
                component = ?log_entry.component,
                user_id = ?log_entry.user_id,
                duration_ms = ?log_entry.duration_ms,
                attributes = ?log_entry.attributes,
                "{}", message
            ),
            _ => info!(
                correlation_id = ?correlation_id,
                operation = ?log_entry.operation,
                component = ?log_entry.component,
                user_id = ?log_entry.user_id,
                duration_ms = ?log_entry.duration_ms,
                attributes = ?log_entry.attributes,
                "{}", message
            ),
        }
    }

    /// Log job lifecycle event
    pub async fn log_job_event(
        &self,
        job_id: Uuid,
        event_type: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        node_id: Option<String>,
        pipeline_id: Option<String>,
        stage: Option<String>,
        duration_ms: Option<u64>,
        error: Option<String>,
    ) {
        let mut attributes = HashMap::new();
        attributes.insert("job_id".to_string(), serde_json::Value::String(job_id.to_string()));
        attributes.insert("event_type".to_string(), serde_json::Value::String(event_type.to_string()));

        if let Some(node_id) = node_id {
            attributes.insert("node_id".to_string(), serde_json::Value::String(node_id));
        }
        if let Some(pipeline_id) = pipeline_id {
            attributes.insert("pipeline_id".to_string(), serde_json::Value::String(pipeline_id));
        }
        if let Some(stage) = stage {
            attributes.insert("stage".to_string(), serde_json::Value::String(stage));
        }

        let error_info = error.map(|e| ErrorInfo {
            error_type: "JobExecutionError".to_string(),
            error_message: e,
            stack_trace: None,
            error_code: None,
        });

        let performance_info = duration_ms.map(|duration| PerformanceInfo {
            operation_duration_ms: duration,
            memory_usage_bytes: None,
            cpu_usage_percent: None,
            db_query_count: None,
            db_query_duration_ms: None,
            cache_hits: None,
            cache_misses: None,
        });

        let level = if error_info.is_some() { "error" } else { "info" };

        self.log_with_correlation(
            level,
            message,
            correlation_id,
            Some("job_execution".to_string()),
            Some("job_scheduler".to_string()),
            attributes,
            error_info,
            performance_info,
        ).await;
    }

    /// Log node event
    pub async fn log_node_event(
        &self,
        node_id: String,
        event_type: &str,
        message: &str,
        correlation_id: Option<Uuid>,
        health_status: Option<String>,
        resource_usage: Option<HashMap<String, f64>>,
    ) {
        let mut attributes = HashMap::new();
        attributes.insert("node_id".to_string(), serde_json::Value::String(node_id));
        attributes.insert("event_type".to_string(), serde_json::Value::String(event_type.to_string()));

        if let Some(health_status) = health_status {
            attributes.insert("health_status".to_string(), serde_json::Value::String(health_status));
        }

        if let Some(resource_usage) = resource_usage {
            for (key, value) in resource_usage {
                attributes.insert(
                    format!("resource_{}", key),
                    serde_json::Value::Number(serde_json::Number::from_f64(value).unwrap_or_else(|| serde_json::Number::from(0))),
                );
            }
        }

        self.log_with_correlation(
            "info",
            message,
            correlation_id,
            Some("node_management".to_string()),
            Some("node_registry".to_string()),
            attributes,
            None,
            None,
        ).await;
    }

    /// Get recent log entries
    pub async fn get_recent_logs(&self, limit: usize) -> Vec<LogEntry> {
        let buffer = self.log_buffer.read().await;
        let start_index = if buffer.len() > limit {
            buffer.len() - limit
        } else {
            0
        };
        buffer[start_index..].to_vec()
    }

    /// Search logs by correlation ID
    pub async fn search_logs_by_correlation(&self, correlation_id: Uuid) -> Vec<LogEntry> {
        let buffer = self.log_buffer.read().await;
        buffer
            .iter()
            .filter(|entry| {
                entry.correlation_id.as_ref()
                    .map(|id| id == &correlation_id.to_string())
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Get logging statistics
    pub async fn get_statistics(&self) -> LoggingStatistics {
        let buffer = self.log_buffer.read().await;
        let active_correlations = self.correlation_tracker.get_active_count().await;

        let mut level_counts = HashMap::new();
        let mut component_counts = HashMap::new();
        let mut error_count = 0;

        for entry in buffer.iter() {
            *level_counts.entry(entry.level.clone()).or_insert(0) += 1;
            if let Some(ref component) = entry.component {
                *component_counts.entry(component.clone()).or_insert(0) += 1;
            }
            if entry.error.is_some() {
                error_count += 1;
            }
        }

        LoggingStatistics {
            total_entries: buffer.len(),
            active_correlations,
            level_counts,
            component_counts,
            error_count,
            buffer_size: buffer.len(),
            max_buffer_size: self.config.max_buffer_size,
        }
    }
}

/// Custom formatter for structured logging
pub struct StructuredFormatter {
    service_name: String,
    environment: String,
    correlation_tracker: Arc<CorrelationTracker>,
}

impl StructuredFormatter {
    pub fn new(
        service_name: String,
        environment: String,
        correlation_tracker: Arc<CorrelationTracker>,
    ) -> Self {
        Self {
            service_name,
            environment,
            correlation_tracker,
        }
    }
}

impl<S, N> FormatEvent<S, N> for StructuredFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();
        
        let mut json_event = serde_json::Map::new();
        json_event.insert("timestamp".to_string(), 
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()));
        json_event.insert("level".to_string(), 
            serde_json::Value::String(metadata.level().to_string()));
        json_event.insert("service".to_string(), 
            serde_json::Value::String(self.service_name.clone()));
        json_event.insert("environment".to_string(), 
            serde_json::Value::String(self.environment.clone()));
        json_event.insert("target".to_string(), 
            serde_json::Value::String(metadata.target().to_string()));

        // Add span information
        if let Some(span) = ctx.lookup_current() {
            json_event.insert("span_name".to_string(), 
                serde_json::Value::String(span.name().to_string()));
        }

        let json_value = serde_json::Value::Object(json_event);
        writeln!(writer, "{}", json_value)
    }
}

/// Logging statistics
#[derive(Debug, Clone, Serialize)]
pub struct LoggingStatistics {
    pub total_entries: usize,
    pub active_correlations: usize,
    pub level_counts: HashMap<String, usize>,
    pub component_counts: HashMap<String, usize>,
    pub error_count: usize,
    pub buffer_size: usize,
    pub max_buffer_size: usize,
}

/// Convenience macros for structured logging
#[macro_export]
macro_rules! log_with_correlation {
    ($logger:expr, $level:expr, $correlation_id:expr, $message:expr) => {
        $logger.log_with_correlation(
            $level,
            $message,
            Some($correlation_id),
            None,
            None,
            std::collections::HashMap::new(),
            None,
            None,
        ).await;
    };
    
    ($logger:expr, $level:expr, $correlation_id:expr, $message:expr, $attributes:expr) => {
        $logger.log_with_correlation(
            $level,
            $message,
            Some($correlation_id),
            None,
            None,
            $attributes,
            None,
            None,
        ).await;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_correlation_tracker() {
        let tracker = CorrelationTracker::new();
        
        let correlation_id = tracker.start_correlation(
            "test_operation".to_string(),
            "test_component".to_string(),
            None,
        ).await;

        tracker.add_breadcrumb(correlation_id, "Step 1 completed".to_string()).await;
        tracker.add_breadcrumb(correlation_id, "Step 2 completed".to_string()).await;

        let context = tracker.get_correlation(correlation_id).await;
        assert!(context.is_some());
        assert_eq!(context.unwrap().breadcrumbs.len(), 3); // Including initial breadcrumb

        let ended_context = tracker.end_correlation(correlation_id).await;
        assert!(ended_context.is_some());
        assert_eq!(tracker.get_active_count().await, 0);
    }

    #[tokio::test]
    async fn test_structured_logging() {
        let config = LoggingConfig::default();
        let logger = StructuredLogging::new(config).unwrap();

        let correlation_id = logger.correlation_tracker()
            .start_correlation("test".to_string(), "test".to_string(), None).await;

        let mut attributes = HashMap::new();
        attributes.insert("test_key".to_string(), serde_json::Value::String("test_value".to_string()));

        logger.log_with_correlation(
            "info",
            "Test message",
            Some(correlation_id),
            None,
            None,
            attributes,
            None,
            None,
        ).await;

        let logs = logger.search_logs_by_correlation(correlation_id).await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "Test message");
    }

    #[tokio::test]
    async fn test_job_logging() {
        let config = LoggingConfig::default();
        let logger = StructuredLogging::new(config).unwrap();

        let job_id = Uuid::new_v4();
        let correlation_id = logger.correlation_tracker()
            .start_correlation("job_execution".to_string(), "scheduler".to_string(), None).await;

        logger.log_job_event(
            job_id,
            "job_started",
            "Job execution started",
            Some(correlation_id),
            Some("node-1".to_string()),
            Some("pipeline-1".to_string()),
            Some("build".to_string()),
            None,
            None,
        ).await;

        let logs = logger.get_recent_logs(10).await;
        assert!(!logs.is_empty());
        
        let job_log = logs.iter().find(|log| 
            log.attributes.get("job_id").map(|v| v.as_str()) == Some(Some(&job_id.to_string()))
        );
        assert!(job_log.is_some());
    }
}