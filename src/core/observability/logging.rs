use crate::{
    config::ObservabilityConfig,
    error::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};
use uuid::Uuid;

/// Initialize structured logging system
pub fn init_structured_logging() -> Result<()> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,rustci=info"));

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_span_events(FmtSpan::NONE)
        .with_timer(fmt::time::ChronoUtc::rfc_3339())
        .compact();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

/// Initialize logging with custom configuration
pub fn init_logging_with_config(config: &ObservabilityConfig) -> Result<()> {
    let level = match config.logging.level.to_lowercase().as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "info" => "info",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };

    let env_filter = EnvFilter::new(level);

    let fmt_layer = match config.logging.format.as_str() {
        "json" => fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_timer(fmt::time::ChronoUtc::rfc_3339())
            .json()
            .boxed(),
        "pretty" => fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_span_events(FmtSpan::CLOSE)
            .with_timer(fmt::time::ChronoUtc::rfc_3339())
            .pretty()
            .boxed(),
        _ => fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_timer(fmt::time::ChronoUtc::rfc_3339())
            .boxed(),
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    info!(
        level = level,
        format = config.logging.format,
        "📝 Structured logging initialized with custom configuration"
    );
    Ok(())
}

/// Log entry structure for structured logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub module: String,
    pub correlation_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub session_id: Option<String>,
    pub request_id: Option<Uuid>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub tags: Vec<String>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

impl LogEntry {
    pub fn new(level: &str, message: String, module: &str) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            message,
            module: module.to_string(),
            correlation_id: None,
            user_id: None,
            session_id: None,
            request_id: None,
            metadata: HashMap::new(),
            tags: Vec::new(),
            duration_ms: None,
            error: None,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_user_context(mut self, user_id: Uuid, session_id: String) -> Self {
        self.user_id = Some(user_id);
        self.session_id = Some(session_id);
        self
    }

    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }
}

/// Correlation tracker for distributed tracing
pub struct CorrelationTracker {
    current_correlation: tokio::sync::RwLock<Option<Uuid>>,
}

impl Default for CorrelationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CorrelationTracker {
    pub fn new() -> Self {
        Self {
            current_correlation: tokio::sync::RwLock::new(None),
        }
    }

    pub async fn start_correlation(&self, correlation_id: Uuid) {
        let mut current = self.current_correlation.write().await;
        *current = Some(correlation_id);
    }

    pub async fn get_current_correlation_id(&self) -> Option<Uuid> {
        *self.current_correlation.read().await
    }

    pub async fn end_correlation(&self) {
        let mut current = self.current_correlation.write().await;
        *current = None;
    }
}

/// Security event logging helper
pub fn log_security_event(
    event_type: &str,
    message: &str,
    user_id: Option<Uuid>,
    metadata: HashMap<String, serde_json::Value>,
) {
    let mut log_data = serde_json::json!({
        "event_type": event_type,
        "message": message,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "category": "security"
    });

    if let Some(uid) = user_id {
        log_data["user_id"] = serde_json::Value::String(uid.to_string());
    }

    for (key, value) in metadata {
        log_data[key] = value;
    }

    warn!(security_event = %log_data, "🔒 Security Event");
}

/// Performance logging helper
pub fn log_performance_event(
    operation: &str,
    duration_ms: u64,
    metadata: HashMap<String, serde_json::Value>,
) {
    let mut log_data = serde_json::json!({
        "operation": operation,
        "duration_ms": duration_ms,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "category": "performance"
    });

    for (key, value) in metadata {
        log_data[key] = value;
    }

    info!(performance_event = %log_data, "⚡ Performance Event");
}

/// Error logging with context
pub fn log_error_with_context(error: &AppError, context: &str, correlation_id: Option<Uuid>) {
    let mut log_data = serde_json::json!({
        "error": error.to_string(),
        "context": context,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "category": "error"
    });

    if let Some(id) = correlation_id {
        log_data["correlation_id"] = serde_json::Value::String(id.to_string());
    }

    error!(error_event = %log_data, "❌ Error Event");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_builder() {
        let entry = LogEntry::new("info", "Test message".to_string(), "test")
            .with_correlation_id(Uuid::new_v4())
            .with_tag("test".to_string())
            .with_duration(100);

        assert_eq!(entry.level, "info");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.module, "test");
        assert!(entry.correlation_id.is_some());
        assert_eq!(entry.tags, vec!["test"]);
        assert_eq!(entry.duration_ms, Some(100));
    }

    #[tokio::test]
    async fn test_correlation_tracker() {
        let tracker = CorrelationTracker::new();
        let correlation_id = Uuid::new_v4();

        tracker.start_correlation(correlation_id).await;

        let current_id = tracker.get_current_correlation_id().await;
        assert_eq!(current_id, Some(correlation_id));

        tracker.end_correlation().await;
        let current_id = tracker.get_current_correlation_id().await;
        assert_eq!(current_id, None);
    }
}
