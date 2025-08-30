use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Enhanced error context with correlation IDs and detailed context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Unique correlation ID for tracking errors across request boundaries
    pub correlation_id: Uuid,
    /// Optional request ID for HTTP requests
    pub request_id: Option<String>,
    /// Optional user ID associated with the error
    pub user_id: Option<Uuid>,
    /// Component where the error occurred
    pub component: String,
    /// Operation that was being performed when the error occurred
    pub operation: String,
    /// Timestamp when the error occurred
    pub timestamp: DateTime<Utc>,
    /// Additional context information as key-value pairs
    pub additional_context: HashMap<String, serde_json::Value>,
    /// Stack trace information if available
    pub stack_trace: Option<String>,
    /// Parent correlation ID for nested operations
    pub parent_correlation_id: Option<Uuid>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(component: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            request_id: None,
            user_id: None,
            component: component.into(),
            operation: operation.into(),
            timestamp: Utc::now(),
            additional_context: HashMap::new(),
            stack_trace: None,
            parent_correlation_id: None,
        }
    }

    /// Create a new error context with correlation ID
    pub fn with_correlation_id(
        correlation_id: Uuid,
        component: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id,
            request_id: None,
            user_id: None,
            component: component.into(),
            operation: operation.into(),
            timestamp: Utc::now(),
            additional_context: HashMap::new(),
            stack_trace: None,
            parent_correlation_id: None,
        }
    }

    /// Set the request ID
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Set the user ID
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Add additional context information
    pub fn with_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.additional_context.insert(key.into(), value);
        self
    }

    /// Add multiple context entries
    pub fn with_contexts(mut self, contexts: HashMap<String, serde_json::Value>) -> Self {
        self.additional_context.extend(contexts);
        self
    }

    /// Set the stack trace
    pub fn with_stack_trace(mut self, stack_trace: impl Into<String>) -> Self {
        self.stack_trace = Some(stack_trace.into());
        self
    }

    /// Set the parent correlation ID for nested operations
    pub fn with_parent_correlation_id(mut self, parent_correlation_id: Uuid) -> Self {
        self.parent_correlation_id = Some(parent_correlation_id);
        self
    }

    /// Create a child context for nested operations
    pub fn create_child_context(&self, component: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            request_id: self.request_id.clone(),
            user_id: self.user_id,
            component: component.into(),
            operation: operation.into(),
            timestamp: Utc::now(),
            additional_context: HashMap::new(),
            stack_trace: None,
            parent_correlation_id: Some(self.correlation_id),
        }
    }

    /// Get a formatted string representation of the context
    pub fn format_context(&self) -> String {
        format!(
            "correlation_id={}, component={}, operation={}, timestamp={}{}{}",
            self.correlation_id,
            self.component,
            self.operation,
            self.timestamp.to_rfc3339(),
            self.request_id.as_ref().map(|id| format!(", request_id={}", id)).unwrap_or_default(),
            self.user_id.map(|id| format!(", user_id={}", id)).unwrap_or_default()
        )
    }
}

/// Enhanced error response with recovery suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Unique error ID for tracking
    pub error_id: Uuid,
    /// Error type classification
    pub error_type: String,
    /// Human-readable error message
    pub message: String,
    /// Additional error details
    pub details: Option<serde_json::Value>,
    /// Suggested recovery actions
    pub recovery_suggestions: Vec<String>,
    /// Retry after duration in seconds
    pub retry_after: Option<u64>,
    /// Error context information
    pub context: ErrorContext,
    /// Error severity level
    pub severity: ErrorSeverity,
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// System-threatening errors requiring immediate attention
    Critical,
    /// Service-impacting errors requiring prompt resolution
    High,
    /// Feature-impacting errors that should be addressed
    Medium,
    /// Minor issues that can be handled gracefully
    Low,
    /// Informational errors for monitoring purposes
    Info,
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(
        error_type: impl Into<String>,
        message: impl Into<String>,
        context: ErrorContext,
        severity: ErrorSeverity,
    ) -> Self {
        Self {
            error_id: Uuid::new_v4(),
            error_type: error_type.into(),
            message: message.into(),
            details: None,
            recovery_suggestions: Vec::new(),
            retry_after: None,
            context,
            severity,
        }
    }

    /// Add error details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Add recovery suggestions
    pub fn with_recovery_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.recovery_suggestions = suggestions;
        self
    }

    /// Add a single recovery suggestion
    pub fn with_recovery_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.recovery_suggestions.push(suggestion.into());
        self
    }

    /// Set retry after duration
    pub fn with_retry_after(mut self, retry_after_seconds: u64) -> Self {
        self.retry_after = Some(retry_after_seconds);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_error_context_creation() {
        let context = ErrorContext::new("test_component", "test_operation");
        
        assert_eq!(context.component, "test_component");
        assert_eq!(context.operation, "test_operation");
        assert!(context.request_id.is_none());
        assert!(context.user_id.is_none());
        assert!(context.additional_context.is_empty());
    }

    #[test]
    fn test_error_context_with_request_id() {
        let context = ErrorContext::new("test_component", "test_operation")
            .with_request_id("req-123");
        
        assert_eq!(context.request_id, Some("req-123".to_string()));
    }

    #[test]
    fn test_error_context_with_user_id() {
        let user_id = Uuid::new_v4();
        let context = ErrorContext::new("test_component", "test_operation")
            .with_user_id(user_id);
        
        assert_eq!(context.user_id, Some(user_id));
    }

    #[test]
    fn test_error_context_with_context() {
        let context = ErrorContext::new("test_component", "test_operation")
            .with_context("key1", json!("value1"))
            .with_context("key2", json!(42));
        
        assert_eq!(context.additional_context.get("key1"), Some(&json!("value1")));
        assert_eq!(context.additional_context.get("key2"), Some(&json!(42)));
    }

    #[test]
    fn test_child_context_creation() {
        let parent_context = ErrorContext::new("parent_component", "parent_operation")
            .with_request_id("req-123")
            .with_user_id(Uuid::new_v4());
        
        let child_context = parent_context.create_child_context("child_component", "child_operation");
        
        assert_eq!(child_context.component, "child_component");
        assert_eq!(child_context.operation, "child_operation");
        assert_eq!(child_context.request_id, parent_context.request_id);
        assert_eq!(child_context.user_id, parent_context.user_id);
        assert_eq!(child_context.parent_correlation_id, Some(parent_context.correlation_id));
        assert_ne!(child_context.correlation_id, parent_context.correlation_id);
    }

    #[test]
    fn test_error_response_creation() {
        let context = ErrorContext::new("test_component", "test_operation");
        let response = ErrorResponse::new("ValidationError", "Test error message", context.clone(), ErrorSeverity::Medium);
        
        assert_eq!(response.error_type, "ValidationError");
        assert_eq!(response.message, "Test error message");
        assert_eq!(response.severity, ErrorSeverity::Medium);
        assert_eq!(response.context.correlation_id, context.correlation_id);
    }

    #[test]
    fn test_error_response_with_recovery_suggestions() {
        let context = ErrorContext::new("test_component", "test_operation");
        let response = ErrorResponse::new("ValidationError", "Test error message", context, ErrorSeverity::Medium)
            .with_recovery_suggestion("Check input format")
            .with_recovery_suggestion("Retry with valid data");
        
        assert_eq!(response.recovery_suggestions.len(), 2);
        assert_eq!(response.recovery_suggestions[0], "Check input format");
        assert_eq!(response.recovery_suggestions[1], "Retry with valid data");
    }

    #[test]
    fn test_format_context() {
        let context = ErrorContext::new("test_component", "test_operation")
            .with_request_id("req-123");
        
        let formatted = context.format_context();
        assert!(formatted.contains("test_component"));
        assert!(formatted.contains("test_operation"));
        assert!(formatted.contains("req-123"));
    }
}