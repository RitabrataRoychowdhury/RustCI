use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Correlation tracker for distributed tracing
#[derive(Debug)]
pub struct CorrelationTracker {
    current_correlation: Arc<RwLock<Option<Uuid>>>,
    correlation_context: Arc<RwLock<HashMap<String, String>>>,
}

impl CorrelationTracker {
    pub fn new() -> Self {
        Self {
            current_correlation: Arc::new(RwLock::new(None)),
            correlation_context: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_correlation(&self, correlation_id: Uuid) {
        let mut current = self.current_correlation.write().await;
        *current = Some(correlation_id);

        debug!(correlation_id = %correlation_id, "Started correlation tracking");
    }

    pub async fn start_correlation_with_context(
        &self,
        correlation_id: Uuid,
        context: HashMap<String, String>,
    ) {
        let mut current = self.current_correlation.write().await;
        *current = Some(correlation_id);

        let mut ctx = self.correlation_context.write().await;
        *ctx = context;

        debug!(
            correlation_id = %correlation_id,
            context = ?ctx,
            "Started correlation tracking with context"
        );
    }

    pub async fn get_current_correlation_id(&self) -> Option<Uuid> {
        *self.current_correlation.read().await
    }

    pub async fn get_correlation_context(&self) -> HashMap<String, String> {
        self.correlation_context.read().await.clone()
    }

    pub async fn add_context(&self, key: String, value: String) {
        let key_clone = key.clone();
        let value_clone = value.clone();

        let mut context = self.correlation_context.write().await;
        context.insert(key, value);

        debug!(key = %key_clone, value = %value_clone, "Added correlation context");
    }

    pub async fn set_correlation_id(&self, correlation_id: uuid::Uuid) {
        self.start_correlation(correlation_id).await;
    }

    pub async fn remove_context(&self, key: &str) {
        let mut context = self.correlation_context.write().await;
        context.remove(key);

        debug!(key = %key, "Removed correlation context");
    }

    pub async fn end_correlation(&self) {
        let correlation_id = {
            let mut current = self.current_correlation.write().await;
            let id = *current;
            *current = None;
            id
        };

        let mut context = self.correlation_context.write().await;
        context.clear();

        if let Some(id) = correlation_id {
            debug!(correlation_id = %id, "Ended correlation tracking");
        }
    }

    pub async fn with_correlation<F, T>(&self, correlation_id: Uuid, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.start_correlation(correlation_id).await;
        let result = future.await;
        self.end_correlation().await;
        result
    }

    pub async fn with_correlation_and_context<F, T>(
        &self,
        correlation_id: Uuid,
        context: HashMap<String, String>,
        future: F,
    ) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.start_correlation_with_context(correlation_id, context)
            .await;
        let result = future.await;
        self.end_correlation().await;
        result
    }
}

impl Default for CorrelationTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Correlation middleware for HTTP requests
pub struct CorrelationMiddleware {
    tracker: Arc<CorrelationTracker>,
}

impl CorrelationMiddleware {
    pub fn new(tracker: Arc<CorrelationTracker>) -> Self {
        Self { tracker }
    }

    pub async fn extract_correlation_id(&self, headers: &axum::http::HeaderMap) -> Uuid {
        // Try to extract correlation ID from headers
        if let Some(header_value) = headers.get("x-correlation-id") {
            if let Ok(header_str) = header_value.to_str() {
                if let Ok(correlation_id) = Uuid::parse_str(header_str) {
                    return correlation_id;
                }
            }
        }

        // Generate new correlation ID if not found
        Uuid::new_v4()
    }

    pub async fn extract_context(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> HashMap<String, String> {
        let mut context = HashMap::new();

        // Extract common context headers
        if let Some(user_agent) = headers.get("user-agent") {
            if let Ok(ua_str) = user_agent.to_str() {
                context.insert("user_agent".to_string(), ua_str.to_string());
            }
        }

        if let Some(forwarded_for) = headers.get("x-forwarded-for") {
            if let Ok(ip_str) = forwarded_for.to_str() {
                context.insert("client_ip".to_string(), ip_str.to_string());
            }
        }

        if let Some(request_id) = headers.get("x-request-id") {
            if let Ok(req_id_str) = request_id.to_str() {
                context.insert("request_id".to_string(), req_id_str.to_string());
            }
        }

        context
    }
}

/// Span context for distributed tracing
#[derive(Debug, Clone)]
pub struct SpanContext {
    pub trace_id: Uuid,
    pub span_id: Uuid,
    pub parent_span_id: Option<Uuid>,
    pub operation_name: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub tags: HashMap<String, String>,
    pub logs: Vec<SpanLog>,
}

#[derive(Debug, Clone)]
pub struct SpanLog {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    pub fields: HashMap<String, String>,
}

impl SpanContext {
    pub fn new(operation_name: String) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            operation_name,
            start_time: chrono::Utc::now(),
            tags: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn child_span(&self, operation_name: String) -> Self {
        Self {
            trace_id: self.trace_id,
            span_id: Uuid::new_v4(),
            parent_span_id: Some(self.span_id),
            operation_name,
            start_time: chrono::Utc::now(),
            tags: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn add_tag(&mut self, key: String, value: String) {
        self.tags.insert(key, value);
    }

    pub fn add_log(&mut self, level: String, message: String, fields: HashMap<String, String>) {
        self.logs.push(SpanLog {
            timestamp: chrono::Utc::now(),
            level,
            message,
            fields,
        });
    }

    pub fn duration(&self) -> chrono::Duration {
        chrono::Utc::now() - self.start_time
    }
}

/// Distributed tracing manager
pub struct TracingManager {
    active_spans: Arc<RwLock<HashMap<Uuid, SpanContext>>>,
    correlation_tracker: Arc<CorrelationTracker>,
}

impl TracingManager {
    pub fn new(correlation_tracker: Arc<CorrelationTracker>) -> Self {
        Self {
            active_spans: Arc::new(RwLock::new(HashMap::new())),
            correlation_tracker,
        }
    }

    pub async fn start_span(&self, operation_name: String) -> Uuid {
        let span = SpanContext::new(operation_name.clone());
        let span_id = span.span_id;

        let mut spans = self.active_spans.write().await;
        spans.insert(span_id, span);

        info!(
            span_id = %span_id,
            operation = %operation_name,
            "Started span"
        );

        span_id
    }

    pub async fn start_child_span(
        &self,
        parent_span_id: Uuid,
        operation_name: String,
    ) -> Option<Uuid> {
        let parent_span = {
            let spans = self.active_spans.read().await;
            spans.get(&parent_span_id).cloned()
        };

        if let Some(parent) = parent_span {
            let child_span = parent.child_span(operation_name.clone());
            let child_span_id = child_span.span_id;

            let mut spans = self.active_spans.write().await;
            spans.insert(child_span_id, child_span);

            info!(
                span_id = %child_span_id,
                parent_span_id = %parent_span_id,
                operation = %operation_name,
                "Started child span"
            );

            Some(child_span_id)
        } else {
            warn!(
                parent_span_id = %parent_span_id,
                "Parent span not found, cannot create child span"
            );
            None
        }
    }

    pub async fn add_span_tag(&self, span_id: Uuid, key: String, value: String) {
        let mut spans = self.active_spans.write().await;
        if let Some(span) = spans.get_mut(&span_id) {
            span.add_tag(key.clone(), value.clone());
            debug!(
                span_id = %span_id,
                key = %key,
                value = %value,
                "Added span tag"
            );
        }
    }

    pub async fn add_span_log(
        &self,
        span_id: Uuid,
        level: String,
        message: String,
        fields: HashMap<String, String>,
    ) {
        let mut spans = self.active_spans.write().await;
        if let Some(span) = spans.get_mut(&span_id) {
            span.add_log(level.clone(), message.clone(), fields.clone());
            debug!(
                span_id = %span_id,
                level = %level,
                message = %message,
                "Added span log"
            );
        }
    }

    pub async fn finish_span(&self, span_id: Uuid) {
        let span = {
            let mut spans = self.active_spans.write().await;
            spans.remove(&span_id)
        };

        if let Some(span) = span {
            let duration = span.duration();
            info!(
                span_id = %span_id,
                operation = %span.operation_name,
                duration_ms = duration.num_milliseconds(),
                "Finished span"
            );

            // Here you would typically export the span to a tracing backend
            // like Jaeger, Zipkin, or OpenTelemetry collector
        } else {
            warn!(span_id = %span_id, "Attempted to finish non-existent span");
        }
    }

    pub async fn get_active_spans(&self) -> Vec<SpanContext> {
        let spans = self.active_spans.read().await;
        spans.values().cloned().collect()
    }

    pub async fn with_span<F, T>(&self, operation_name: String, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let span_id = self.start_span(operation_name).await;
        let result = future.await;
        self.finish_span(span_id).await;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_correlation_tracker() {
        let tracker = CorrelationTracker::new();
        let correlation_id = Uuid::new_v4();

        // Test basic correlation tracking
        tracker.start_correlation(correlation_id).await;

        let current_id = tracker.get_current_correlation_id().await;
        assert_eq!(current_id, Some(correlation_id));

        tracker.end_correlation().await;
        let current_id = tracker.get_current_correlation_id().await;
        assert_eq!(current_id, None);
    }

    #[tokio::test]
    async fn test_correlation_with_context() {
        let tracker = CorrelationTracker::new();
        let correlation_id = Uuid::new_v4();
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), "123".to_string());

        tracker
            .start_correlation_with_context(correlation_id, context.clone())
            .await;

        let current_context = tracker.get_correlation_context().await;
        assert_eq!(current_context.get("user_id"), Some(&"123".to_string()));

        tracker
            .add_context("request_id".to_string(), "req-456".to_string())
            .await;
        let updated_context = tracker.get_correlation_context().await;
        assert_eq!(
            updated_context.get("request_id"),
            Some(&"req-456".to_string())
        );

        tracker.end_correlation().await;
        let final_context = tracker.get_correlation_context().await;
        assert!(final_context.is_empty());
    }

    #[tokio::test]
    async fn test_span_context() {
        let mut span = SpanContext::new("test_operation".to_string());
        assert_eq!(span.operation_name, "test_operation");

        span.add_tag("component".to_string(), "test".to_string());
        assert_eq!(span.tags.get("component"), Some(&"test".to_string()));

        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "value".to_string());
        span.add_log("info".to_string(), "Test log".to_string(), fields);
        assert_eq!(span.logs.len(), 1);
        assert_eq!(span.logs[0].message, "Test log");
    }

    #[tokio::test]
    async fn test_tracing_manager() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let tracing_manager = TracingManager::new(correlation_tracker);

        let span_id = tracing_manager
            .start_span("test_operation".to_string())
            .await;

        tracing_manager
            .add_span_tag(span_id, "component".to_string(), "test".to_string())
            .await;

        let active_spans = tracing_manager.get_active_spans().await;
        assert_eq!(active_spans.len(), 1);
        assert_eq!(active_spans[0].operation_name, "test_operation");

        tracing_manager.finish_span(span_id).await;

        let active_spans = tracing_manager.get_active_spans().await;
        assert_eq!(active_spans.len(), 0);
    }
}
