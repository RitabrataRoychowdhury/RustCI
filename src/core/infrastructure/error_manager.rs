use crate::error::{AppError, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Error context for enriched error handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub correlation_id: Uuid,
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub operation: String,
    pub component: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub stack_trace: Option<String>,
}

impl ErrorContext {
    pub fn new(operation: &str, component: &str) -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            request_id: None,
            user_id: None,
            operation: operation.to_string(),
            component: component.to_string(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
            stack_trace: None,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn with_stack_trace(mut self, stack_trace: String) -> Self {
        self.stack_trace = Some(stack_trace);
        self
    }
}

/// Error response for API endpoints
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_code: String,
    pub correlation_id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: Option<HashMap<String, String>>,
}

/// Error handler trait for pluggable error handling
#[async_trait::async_trait]
pub trait ErrorHandler: Send + Sync {
    async fn handle_error(&self, error: &AppError, context: &ErrorContext)
        -> Result<ErrorResponse>;
    fn can_handle(&self, error: &AppError) -> bool;
    fn priority(&self) -> u8; // Lower number = higher priority
}

/// Error statistics for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct ErrorStats {
    pub total_errors: u64,
    pub errors_by_type: HashMap<String, u64>,
    pub errors_by_component: HashMap<String, u64>,
    pub recent_errors: Vec<ErrorSummary>,
    pub error_rate_per_minute: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorSummary {
    pub error_type: String,
    pub component: String,
    pub operation: String,
    pub count: u64,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

/// Circuit breaker for error handling
#[derive(Debug)]
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    failure_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    state: Arc<RwLock<CircuitBreakerState>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            failure_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
        }
    }

    pub async fn call<F, T>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let state = self.state.read().await.clone();

        match state {
            CircuitBreakerState::Open => {
                let last_failure = *self.last_failure_time.read().await;
                if let Some(last_failure) = last_failure {
                    if last_failure.elapsed() > self.recovery_timeout {
                        let mut state = self.state.write().await;
                        *state = CircuitBreakerState::HalfOpen;
                        debug!("Circuit breaker transitioning to half-open state");
                    } else {
                        return Err(AppError::ServiceUnavailable(
                            "Circuit breaker is open".to_string(),
                        ));
                    }
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Allow one request through
            }
            CircuitBreakerState::Closed => {
                // Normal operation
            }
        }

        match operation.await {
            Ok(result) => {
                if state == CircuitBreakerState::HalfOpen {
                    let mut failure_count = self.failure_count.write().await;
                    *failure_count = 0;
                    let mut state = self.state.write().await;
                    *state = CircuitBreakerState::Closed;
                    debug!("Circuit breaker reset to closed state");
                }
                Ok(result)
            }
            Err(error) => {
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;

                let mut last_failure_time = self.last_failure_time.write().await;
                *last_failure_time = Some(Instant::now());

                if *failure_count >= self.failure_threshold {
                    let mut state = self.state.write().await;
                    *state = CircuitBreakerState::Open;
                    warn!(
                        failure_count = *failure_count,
                        threshold = self.failure_threshold,
                        "Circuit breaker opened due to failures"
                    );
                }

                Err(error)
            }
        }
    }

    pub async fn get_state(&self) -> CircuitBreakerState {
        self.state.read().await.clone()
    }
}

/// Main error manager
pub struct ErrorManager {
    handlers: Vec<Box<dyn ErrorHandler>>,
    error_stats: Arc<RwLock<ErrorStats>>,
    error_history: DashMap<String, ErrorSummary>,
    circuit_breakers: DashMap<String, Arc<CircuitBreaker>>,
    correlation_tracker: Arc<crate::core::patterns::correlation::CorrelationTracker>,
}

impl ErrorManager {
    pub fn new(correlation_tracker: Arc<crate::core::patterns::correlation::CorrelationTracker>) -> Self {
        Self {
            handlers: Vec::new(),
            error_stats: Arc::new(RwLock::new(ErrorStats {
                total_errors: 0,
                errors_by_type: HashMap::new(),
                errors_by_component: HashMap::new(),
                recent_errors: Vec::new(),
                error_rate_per_minute: 0.0,
            })),
            error_history: DashMap::new(),
            circuit_breakers: DashMap::new(),
            correlation_tracker,
        }
    }

    pub fn add_handler(&mut self, handler: Box<dyn ErrorHandler>) {
        let priority = handler.priority();
        self.handlers.push(handler);
        // Sort handlers by priority
        self.handlers.sort_by_key(|h| h.priority());
        debug!("Added error handler with priority {}", priority);
    }

    pub async fn handle_error(&self, error: &AppError, context: &ErrorContext) -> ErrorResponse {
        // Update statistics
        self.update_error_stats(error, context).await;

        // Log the error with context
        self.log_error_with_context(error, context).await;

        // Try to handle with registered handlers
        for handler in &self.handlers {
            if handler.can_handle(error) {
                match handler.handle_error(error, context).await {
                    Ok(response) => return response,
                    Err(handler_error) => {
                        warn!(
                            error = %handler_error,
                            "Error handler failed, trying next handler"
                        );
                    }
                }
            }
        }

        // Default error response
        self.create_default_error_response(error, context)
    }

    async fn update_error_stats(&self, error: &AppError, context: &ErrorContext) {
        let error_type = self.get_error_type(error);
        let key = format!("{}::{}", context.component, context.operation);

        // Update error summary
        let now = chrono::Utc::now();
        self.error_history
            .entry(key.clone())
            .and_modify(|summary| {
                summary.count += 1;
                summary.last_seen = now;
            })
            .or_insert_with(|| ErrorSummary {
                error_type: error_type.clone(),
                component: context.component.clone(),
                operation: context.operation.clone(),
                count: 1,
                first_seen: now,
                last_seen: now,
            });

        // Update global stats
        let mut stats = self.error_stats.write().await;
        stats.total_errors += 1;
        *stats.errors_by_type.entry(error_type).or_insert(0) += 1;
        *stats
            .errors_by_component
            .entry(context.component.clone())
            .or_insert(0) += 1;

        // Update recent errors (keep last 100)
        if stats.recent_errors.len() >= 100 {
            stats.recent_errors.remove(0);
        }
        stats.recent_errors.push(ErrorSummary {
            error_type: self.get_error_type(error),
            component: context.component.clone(),
            operation: context.operation.clone(),
            count: 1,
            first_seen: now,
            last_seen: now,
        });
    }

    async fn log_error_with_context(&self, error: &AppError, context: &ErrorContext) {
        let correlation_id = self
            .correlation_tracker
            .get_current_correlation_id()
            .await
            .unwrap_or(context.correlation_id);

        error!(
            error = %error,
            correlation_id = %correlation_id,
            request_id = ?context.request_id,
            user_id = ?context.user_id,
            operation = context.operation,
            component = context.component,
            metadata = ?context.metadata,
            stack_trace = ?context.stack_trace,
            "Error occurred with context"
        );
    }

    fn create_default_error_response(
        &self,
        error: &AppError,
        context: &ErrorContext,
    ) -> ErrorResponse {
        ErrorResponse {
            error: error.to_string(),
            error_code: self.get_error_code(error),
            correlation_id: context.correlation_id,
            timestamp: chrono::Utc::now(),
            details: Some(context.metadata.clone()),
        }
    }

    fn get_error_type(&self, error: &AppError) -> String {
        match error {
            AppError::DatabaseError(_) => "database_error".to_string(),
            AppError::ValidationError(_) => "validation_error".to_string(),
            AppError::AuthenticationError(_) => "authentication_error".to_string(),
            AppError::AuthorizationError(_) => "authorization_error".to_string(),
            AppError::NotFound(_) => "not_found".to_string(),
            AppError::InternalServerError(_) => "internal_server_error".to_string(),
            AppError::BadRequest(_) => "bad_request".to_string(),
            AppError::ServiceUnavailable(_) => "service_unavailable".to_string(),
            AppError::ConfigError(_) => "config_error".to_string(),
            AppError::ExternalServiceError(_) => "external_service_error".to_string(),
            AppError::TimeoutError(_) => "timeout_error".to_string(),
            AppError::RateLimitExceeded { .. } => "rate_limit_exceeded".to_string(),
            AppError::AuthError(_) => "auth_error".to_string(),
            AppError::Forbidden(_) => "forbidden".to_string(),
            AppError::FileUploadError(_) => "file_upload_error".to_string(),
            AppError::FileSizeExceeded { .. } => "file_size_exceeded".to_string(),
            AppError::UnsupportedFileType(_) => "unsupported_file_type".to_string(),
            AppError::GitHubApiError(_) => "github_api_error".to_string(),
            AppError::DockerValidationError(_) => "docker_validation_error".to_string(),
            AppError::ProjectTypeDetectionFailed(_) => "project_type_detection_failed".to_string(),
            AppError::DockerfileGenerationFailed(_) => "dockerfile_generation_failed".to_string(),
            AppError::EncryptionError(_) => "encryption_error".to_string(),
            AppError::WorkspaceNotFound => "workspace_not_found".to_string(),
            AppError::RepositoryNotFound(_) => "repository_not_found".to_string(),
            AppError::PermissionDenied(_) => "permission_denied".to_string(),
            AppError::KubernetesError(_) => "kubernetes_error".to_string(),
            AppError::ConnectorConfigError(_) => "connector_config_error".to_string(),
            AppError::Unimplemented(_) => "unimplemented".to_string(),
            AppError::CommandExecutionError { .. } => "command_execution_error".to_string(),
            AppError::QueryExecutionError { .. } => "query_execution_error".to_string(),
            AppError::SagaCompensationError { .. } => "saga_compensation_error".to_string(),
            AppError::RepositoryError { .. } => "repository_error".to_string(),
            AppError::DependencyNotFound { .. } => "dependency_not_found".to_string(),
            AppError::EventHandlingError { .. } => "event_handling_error".to_string(),
            AppError::ConfigurationError { .. } => "configuration_error".to_string(),
            AppError::ResourceExhausted(_) => "resource_exhausted".to_string(),
            AppError::ConcurrencyError(_) => "concurrency_error".to_string(),
            AppError::NotImplemented(_) => "not_implemented".to_string(),
            AppError::RateLimitExceededSimple(_) => "rate_limit_exceeded".to_string(),
        }
    }

    fn get_error_code(&self, error: &AppError) -> String {
        match error {
            AppError::DatabaseError(_) => "DB_ERROR".to_string(),
            AppError::ValidationError(_) => "VALIDATION_ERROR".to_string(),
            AppError::AuthenticationError(_) => "AUTH_ERROR".to_string(),
            AppError::AuthorizationError(_) => "AUTHZ_ERROR".to_string(),
            AppError::NotFound(_) => "NOT_FOUND".to_string(),
            AppError::InternalServerError(_) => "INTERNAL_ERROR".to_string(),
            AppError::BadRequest(_) => "BAD_REQUEST".to_string(),
            AppError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE".to_string(),
            AppError::ConfigError(_) => "CONFIG_ERROR".to_string(),
            AppError::ExternalServiceError(_) => "EXTERNAL_SERVICE_ERROR".to_string(),
            AppError::TimeoutError(_) => "TIMEOUT_ERROR".to_string(),
            AppError::RateLimitExceeded { .. } => "RATE_LIMIT_ERROR".to_string(),
            AppError::AuthError(_) => "AUTH_ERROR".to_string(),
            AppError::Forbidden(_) => "FORBIDDEN".to_string(),
            AppError::FileUploadError(_) => "FILE_UPLOAD_ERROR".to_string(),
            AppError::FileSizeExceeded { .. } => "FILE_SIZE_EXCEEDED".to_string(),
            AppError::UnsupportedFileType(_) => "UNSUPPORTED_FILE_TYPE".to_string(),
            AppError::GitHubApiError(_) => "GITHUB_API_ERROR".to_string(),
            AppError::DockerValidationError(_) => "DOCKER_VALIDATION_ERROR".to_string(),
            AppError::ProjectTypeDetectionFailed(_) => "PROJECT_TYPE_DETECTION_FAILED".to_string(),
            AppError::DockerfileGenerationFailed(_) => "DOCKERFILE_GENERATION_FAILED".to_string(),
            AppError::EncryptionError(_) => "ENCRYPTION_ERROR".to_string(),
            AppError::WorkspaceNotFound => "WORKSPACE_NOT_FOUND".to_string(),
            AppError::RepositoryNotFound(_) => "REPOSITORY_NOT_FOUND".to_string(),
            AppError::PermissionDenied(_) => "PERMISSION_DENIED".to_string(),
            AppError::KubernetesError(_) => "KUBERNETES_ERROR".to_string(),
            AppError::ConnectorConfigError(_) => "CONNECTOR_CONFIG_ERROR".to_string(),
            AppError::Unimplemented(_) => "UNIMPLEMENTED".to_string(),
            AppError::CommandExecutionError { .. } => "COMMAND_EXECUTION_ERROR".to_string(),
            AppError::QueryExecutionError { .. } => "QUERY_EXECUTION_ERROR".to_string(),
            AppError::SagaCompensationError { .. } => "SAGA_COMPENSATION_ERROR".to_string(),
            AppError::RepositoryError { .. } => "REPOSITORY_ERROR".to_string(),
            AppError::DependencyNotFound { .. } => "DEPENDENCY_NOT_FOUND".to_string(),
            AppError::EventHandlingError { .. } => "EVENT_HANDLING_ERROR".to_string(),
            AppError::ConfigurationError { .. } => "CONFIGURATION_ERROR".to_string(),
            AppError::ResourceExhausted(_) => "RESOURCE_EXHAUSTED".to_string(),
            AppError::ConcurrencyError(_) => "CONCURRENCY_ERROR".to_string(),
            AppError::NotImplemented(_) => "NOT_IMPLEMENTED".to_string(),
            AppError::RateLimitExceededSimple(_) => "RATE_LIMIT_EXCEEDED".to_string(),
        }
    }

    pub async fn get_error_stats(&self) -> ErrorStats {
        self.error_stats.read().await.clone()
    }

    pub fn get_circuit_breaker(&self, service_name: &str) -> Arc<CircuitBreaker> {
        self.circuit_breakers
            .entry(service_name.to_string())
            .or_insert_with(|| {
                Arc::new(CircuitBreaker::new(
                    5,                       // failure threshold
                    Duration::from_secs(60), // recovery timeout
                ))
            })
            .clone()
    }

    pub async fn get_circuit_breaker_states(&self) -> HashMap<String, CircuitBreakerState> {
        let mut states = HashMap::new();
        for entry in self.circuit_breakers.iter() {
            let state = entry.value().get_state().await;
            states.insert(entry.key().clone(), state);
        }
        states
    }
}

/// Default error handlers
pub struct DatabaseErrorHandler;

#[async_trait::async_trait]
impl ErrorHandler for DatabaseErrorHandler {
    async fn handle_error(
        &self,
        error: &AppError,
        context: &ErrorContext,
    ) -> Result<ErrorResponse> {
        match error {
            AppError::DatabaseError(msg) => Ok(ErrorResponse {
                error: "Database operation failed".to_string(),
                error_code: "DB_ERROR".to_string(),
                correlation_id: context.correlation_id,
                timestamp: chrono::Utc::now(),
                details: Some({
                    let mut details = context.metadata.clone();
                    details.insert("database_error".to_string(), msg.clone());
                    details
                }),
            }),
            _ => Err(AppError::InternalServerError(
                "Cannot handle this error type".to_string(),
            )),
        }
    }

    fn can_handle(&self, error: &AppError) -> bool {
        matches!(error, AppError::DatabaseError(_))
    }

    fn priority(&self) -> u8 {
        1
    }
}

pub struct ValidationErrorHandler;

#[async_trait::async_trait]
impl ErrorHandler for ValidationErrorHandler {
    async fn handle_error(
        &self,
        error: &AppError,
        context: &ErrorContext,
    ) -> Result<ErrorResponse> {
        match error {
            AppError::ValidationError(msg) => Ok(ErrorResponse {
                error: "Validation failed".to_string(),
                error_code: "VALIDATION_ERROR".to_string(),
                correlation_id: context.correlation_id,
                timestamp: chrono::Utc::now(),
                details: Some({
                    let mut details = context.metadata.clone();
                    details.insert("validation_error".to_string(), msg.clone());
                    details
                }),
            }),
            _ => Err(AppError::InternalServerError(
                "Cannot handle this error type".to_string(),
            )),
        }
    }

    fn can_handle(&self, error: &AppError) -> bool {
        matches!(error, AppError::ValidationError(_))
    }

    fn priority(&self) -> u8 {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new(2, Duration::from_millis(100));

        // First failure
        let result = breaker
            .call(async { Err::<(), _>(AppError::InternalServerError("test".to_string())) })
            .await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Closed);

        // Second failure - should open circuit
        let result = breaker
            .call(async { Err::<(), _>(AppError::InternalServerError("test".to_string())) })
            .await;
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitBreakerState::Open);

        // Should fail fast now
        let result = breaker.call(async { Ok::<(), _>(()) }).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AppError::ServiceUnavailable(_)
        ));
    }

    #[tokio::test]
    async fn test_error_context_builder() {
        let context = ErrorContext::new("test_operation", "test_component")
            .with_request_id("req-123".to_string())
            .with_user_id("user-456".to_string())
            .with_metadata("key".to_string(), "value".to_string());

        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.component, "test_component");
        assert_eq!(context.request_id, Some("req-123".to_string()));
        assert_eq!(context.user_id, Some("user-456".to_string()));
        assert_eq!(context.metadata.get("key"), Some(&"value".to_string()));
    }
}
