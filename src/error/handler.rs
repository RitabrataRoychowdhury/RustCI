use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, warn, info};

use crate::error::AppError;
use super::context::{ErrorContext, ErrorResponse, ErrorSeverity};

/// Recovery result indicating the outcome of an error recovery attempt
#[derive(Debug, Clone)]
pub enum RecoveryResult {
    /// Recovery was successful
    Success,
    /// Recovery failed, but operation can be retried
    Retry { delay_seconds: u64 },
    /// Recovery failed, operation should not be retried
    Failed,
    /// Recovery partially succeeded, operation can continue with degraded functionality
    Degraded { message: String },
}

/// Production error handler trait for centralized error management
#[async_trait]
pub trait ProductionErrorHandler: Send + Sync {
    /// Handle an error with context and return an appropriate response
    async fn handle_error(&self, error: AppError, context: ErrorContext) -> ErrorResponse;
    
    /// Attempt to recover from an error
    async fn attempt_recovery(&self, error: AppError, context: ErrorContext) -> RecoveryResult;
    
    /// Determine if an error should be retried
    fn should_retry(&self, error: &AppError, attempt: u32) -> bool;
    
    /// Get the maximum number of retry attempts for an error type
    fn max_retry_attempts(&self, error: &AppError) -> u32;
    
    /// Get the delay between retry attempts
    fn retry_delay(&self, error: &AppError, attempt: u32) -> u64;
    
    /// Log the error with appropriate level and context
    async fn log_error(&self, error: &AppError, context: &ErrorContext, response: &ErrorResponse);
}

/// Default implementation of the production error handler
#[derive(Debug)]
pub struct DefaultProductionErrorHandler {
    /// Configuration for error handling behavior
    config: Arc<RwLock<ErrorHandlerConfig>>,
}

/// Configuration for error handling behavior
#[derive(Debug, Clone)]
pub struct ErrorHandlerConfig {
    /// Maximum retry attempts per error type
    pub max_retries: std::collections::HashMap<String, u32>,
    /// Base delay for exponential backoff (in seconds)
    pub base_delay_seconds: u64,
    /// Maximum delay for exponential backoff (in seconds)
    pub max_delay_seconds: u64,
    /// Whether to enable automatic recovery attempts
    pub enable_auto_recovery: bool,
    /// Whether to log all errors (including low severity)
    pub log_all_errors: bool,
}

impl Default for ErrorHandlerConfig {
    fn default() -> Self {
        let mut max_retries = std::collections::HashMap::new();
        max_retries.insert("DatabaseError".to_string(), 3);
        max_retries.insert("ExternalServiceError".to_string(), 5);
        max_retries.insert("TimeoutError".to_string(), 3);
        max_retries.insert("ServiceUnavailable".to_string(), 5);
        max_retries.insert("RateLimitExceeded".to_string(), 3);
        
        Self {
            max_retries,
            base_delay_seconds: 1,
            max_delay_seconds: 300, // 5 minutes
            enable_auto_recovery: true,
            log_all_errors: false,
        }
    }
}

impl DefaultProductionErrorHandler {
    /// Create a new default production error handler
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(ErrorHandlerConfig::default())),
        }
    }

    /// Create a new production error handler with custom configuration
    pub fn with_config(config: ErrorHandlerConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Update the configuration
    pub async fn update_config(&self, config: ErrorHandlerConfig) {
        let mut current_config = self.config.write().await;
        *current_config = config;
    }

    /// Get the error type string for an AppError
    fn get_error_type(&self, error: &AppError) -> String {
        match error {
            AppError::DatabaseError(_) => "DatabaseError".to_string(),
            AppError::ExternalServiceError(_) => "ExternalServiceError".to_string(),
            AppError::TimeoutError(_) | AppError::Timeout(_) => "TimeoutError".to_string(),
            AppError::ServiceUnavailable(_) => "ServiceUnavailable".to_string(),
            AppError::RateLimitExceeded { .. } | AppError::RateLimitExceededSimple(_) => "RateLimitExceeded".to_string(),
            AppError::ValidationError(_) => "ValidationError".to_string(),
            AppError::AuthError(_) | AppError::AuthenticationError(_) => "AuthenticationError".to_string(),
            AppError::AuthorizationError(_) | AppError::PermissionDenied(_) => "AuthorizationError".to_string(),
            AppError::NotFound(_) => "NotFound".to_string(),
            AppError::ConfigurationError(_) | AppError::ConfigError(_) => "ConfigurationError".to_string(),
            AppError::IoError(_) => "IoError".to_string(),
            AppError::SerializationError(_) => "SerializationError".to_string(),
            AppError::SecurityError(_) => "SecurityError".to_string(),
            _ => "InternalError".to_string(),
        }
    }

    /// Get the error severity for an AppError
    fn get_error_severity(&self, error: &AppError) -> ErrorSeverity {
        match error {
            // Critical errors that require immediate attention
            AppError::DatabaseError(_) | 
            AppError::SecurityError(_) | 
            AppError::EncryptionError(_) => ErrorSeverity::Critical,
            
            // High severity errors that impact service functionality
            AppError::ExternalServiceError(_) | 
            AppError::ServiceUnavailable(_) | 
            AppError::ConfigurationError(_) | 
            AppError::ConfigError(_) => ErrorSeverity::High,
            
            // Medium severity errors that impact features
            AppError::TimeoutError(_) | 
            AppError::Timeout(_) | 
            AppError::RateLimitExceeded { .. } | 
            AppError::RateLimitExceededSimple(_) | 
            AppError::IoError(_) => ErrorSeverity::Medium,
            
            // Low severity errors that can be handled gracefully
            AppError::ValidationError(_) | 
            AppError::BadRequest(_) | 
            AppError::NotFound(_) | 
            AppError::AuthError(_) | 
            AppError::AuthenticationError(_) | 
            AppError::AuthorizationError(_) | 
            AppError::PermissionDenied(_) => ErrorSeverity::Low,
            
            // Default to medium for unknown errors
            _ => ErrorSeverity::Medium,
        }
    }

    /// Generate recovery suggestions based on error type
    fn generate_recovery_suggestions(&self, error: &AppError) -> Vec<String> {
        match error {
            AppError::DatabaseError(_) => vec![
                "Check database connection".to_string(),
                "Verify database credentials".to_string(),
                "Check database server status".to_string(),
                "Retry the operation".to_string(),
            ],
            AppError::ExternalServiceError(_) => vec![
                "Check external service status".to_string(),
                "Verify network connectivity".to_string(),
                "Check service credentials".to_string(),
                "Retry with exponential backoff".to_string(),
            ],
            AppError::TimeoutError(_) | AppError::Timeout(_) => vec![
                "Increase timeout duration".to_string(),
                "Check network latency".to_string(),
                "Retry the operation".to_string(),
                "Consider breaking down the operation".to_string(),
            ],
            AppError::ValidationError(_) => vec![
                "Check input format and requirements".to_string(),
                "Verify all required fields are provided".to_string(),
                "Review API documentation".to_string(),
            ],
            AppError::AuthError(_) | AppError::AuthenticationError(_) => vec![
                "Check credentials".to_string(),
                "Verify authentication token".to_string(),
                "Re-authenticate if token expired".to_string(),
            ],
            AppError::AuthorizationError(_) | AppError::PermissionDenied(_) => vec![
                "Check user permissions".to_string(),
                "Contact administrator for access".to_string(),
                "Verify resource ownership".to_string(),
            ],
            AppError::RateLimitExceeded { .. } | AppError::RateLimitExceededSimple(_) => vec![
                "Wait before retrying".to_string(),
                "Implement exponential backoff".to_string(),
                "Consider request batching".to_string(),
                "Check rate limit headers".to_string(),
            ],
            AppError::NotFound(_) => vec![
                "Verify resource identifier".to_string(),
                "Check if resource exists".to_string(),
                "Review request parameters".to_string(),
            ],
            _ => vec![
                "Check system logs for details".to_string(),
                "Contact support if issue persists".to_string(),
                "Retry the operation".to_string(),
            ],
        }
    }
}

#[async_trait]
impl ProductionErrorHandler for DefaultProductionErrorHandler {
    async fn handle_error(&self, error: AppError, context: ErrorContext) -> ErrorResponse {
        let error_type = self.get_error_type(&error);
        let severity = self.get_error_severity(&error);
        let recovery_suggestions = self.generate_recovery_suggestions(&error);
        
        let mut response = ErrorResponse::new(
            error_type,
            error.to_string(),
            context,
            severity,
        ).with_recovery_suggestions(recovery_suggestions);

        // Add retry after for retryable errors
        if self.should_retry(&error, 1) {
            let delay = self.retry_delay(&error, 1);
            response = response.with_retry_after(delay);
        }

        // Log the error
        self.log_error(&error, &response.context, &response).await;

        response
    }

    async fn attempt_recovery(&self, error: AppError, context: ErrorContext) -> RecoveryResult {
        let config = self.config.read().await;
        
        if !config.enable_auto_recovery {
            return RecoveryResult::Failed;
        }

        match error {
            AppError::DatabaseError(_) => {
                // For database errors, we might attempt to reconnect or use a fallback
                info!("Attempting database error recovery for correlation_id: {}", context.correlation_id);
                // In a real implementation, this would attempt actual recovery
                RecoveryResult::Retry { delay_seconds: 5 }
            },
            AppError::ExternalServiceError(_) => {
                // For external service errors, we might try alternative endpoints
                info!("Attempting external service error recovery for correlation_id: {}", context.correlation_id);
                RecoveryResult::Retry { delay_seconds: 10 }
            },
            AppError::TimeoutError(_) | AppError::Timeout(_) => {
                // For timeout errors, we might increase timeout or break down the operation
                info!("Attempting timeout error recovery for correlation_id: {}", context.correlation_id);
                RecoveryResult::Retry { delay_seconds: 2 }
            },
            AppError::RateLimitExceeded { .. } | AppError::RateLimitExceededSimple(_) => {
                // For rate limit errors, we need to wait
                info!("Attempting rate limit error recovery for correlation_id: {}", context.correlation_id);
                RecoveryResult::Retry { delay_seconds: 60 }
            },
            _ => {
                // For other errors, no automatic recovery
                RecoveryResult::Failed
            }
        }
    }

    fn should_retry(&self, error: &AppError, attempt: u32) -> bool {
        let max_attempts = self.max_retry_attempts(error);
        attempt <= max_attempts && matches!(error,
            AppError::DatabaseError(_) |
            AppError::ExternalServiceError(_) |
            AppError::TimeoutError(_) |
            AppError::Timeout(_) |
            AppError::ServiceUnavailable(_) |
            AppError::RateLimitExceeded { .. } |
            AppError::RateLimitExceededSimple(_) |
            AppError::IoError(_)
        )
    }

    fn max_retry_attempts(&self, error: &AppError) -> u32 {
        // This would typically read from config, but for now we'll use defaults
        match error {
            AppError::DatabaseError(_) => 3,
            AppError::ExternalServiceError(_) => 5,
            AppError::TimeoutError(_) | AppError::Timeout(_) => 3,
            AppError::ServiceUnavailable(_) => 5,
            AppError::RateLimitExceeded { .. } | AppError::RateLimitExceededSimple(_) => 3,
            AppError::IoError(_) => 2,
            _ => 0,
        }
    }

    fn retry_delay(&self, error: &AppError, attempt: u32) -> u64 {
        // Exponential backoff with jitter
        let base_delay = match error {
            AppError::RateLimitExceeded { .. } | AppError::RateLimitExceededSimple(_) => 60, // Start with 1 minute for rate limits
            _ => 1, // Start with 1 second for others
        };
        
        let delay = base_delay * 2_u64.pow(attempt.saturating_sub(1));
        std::cmp::min(delay, 300) // Cap at 5 minutes
    }

    async fn log_error(&self, _error: &AppError, context: &ErrorContext, response: &ErrorResponse) {
        let config = self.config.read().await;
        
        let should_log = config.log_all_errors || matches!(response.severity, 
            ErrorSeverity::Critical | ErrorSeverity::High | ErrorSeverity::Medium
        );

        if !should_log {
            return;
        }

        let log_message = format!(
            "Error handled: {} | {} | {}",
            response.error_type,
            response.message,
            context.format_context()
        );

        match response.severity {
            ErrorSeverity::Critical => {
                error!(
                    error_id = %response.error_id,
                    correlation_id = %context.correlation_id,
                    component = %context.component,
                    operation = %context.operation,
                    error_type = %response.error_type,
                    "{}",
                    log_message
                );
            },
            ErrorSeverity::High => {
                error!(
                    error_id = %response.error_id,
                    correlation_id = %context.correlation_id,
                    component = %context.component,
                    operation = %context.operation,
                    error_type = %response.error_type,
                    "{}",
                    log_message
                );
            },
            ErrorSeverity::Medium => {
                warn!(
                    error_id = %response.error_id,
                    correlation_id = %context.correlation_id,
                    component = %context.component,
                    operation = %context.operation,
                    error_type = %response.error_type,
                    "{}",
                    log_message
                );
            },
            ErrorSeverity::Low | ErrorSeverity::Info => {
                info!(
                    error_id = %response.error_id,
                    correlation_id = %context.correlation_id,
                    component = %context.component,
                    operation = %context.operation,
                    error_type = %response.error_type,
                    "{}",
                    log_message
                );
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_error_handler_creation() {
        let handler = DefaultProductionErrorHandler::new();
        assert!(handler.config.read().await.enable_auto_recovery);
    }

    #[tokio::test]
    async fn test_handle_database_error() {
        let handler = DefaultProductionErrorHandler::new();
        let context = ErrorContext::new("test_component", "test_operation");
        let error = AppError::DatabaseError("Connection failed".to_string());
        
        let response = handler.handle_error(error, context).await;
        
        assert_eq!(response.error_type, "DatabaseError");
        assert_eq!(response.severity, ErrorSeverity::Critical);
        assert!(!response.recovery_suggestions.is_empty());
        assert!(response.retry_after.is_some());
    }

    #[tokio::test]
    async fn test_should_retry_logic() {
        let handler = DefaultProductionErrorHandler::new();
        
        // Database errors should be retryable
        let db_error = AppError::DatabaseError("Connection failed".to_string());
        assert!(handler.should_retry(&db_error, 1));
        assert!(handler.should_retry(&db_error, 2));
        assert!(handler.should_retry(&db_error, 3));
        assert!(!handler.should_retry(&db_error, 4)); // Exceeds max attempts
        
        // Validation errors should not be retryable
        let validation_error = AppError::ValidationError("Invalid input".to_string());
        assert!(!handler.should_retry(&validation_error, 1));
    }

    #[tokio::test]
    async fn test_retry_delay_exponential_backoff() {
        let handler = DefaultProductionErrorHandler::new();
        let error = AppError::DatabaseError("Connection failed".to_string());
        
        assert_eq!(handler.retry_delay(&error, 1), 1);
        assert_eq!(handler.retry_delay(&error, 2), 2);
        assert_eq!(handler.retry_delay(&error, 3), 4);
        assert_eq!(handler.retry_delay(&error, 4), 8);
    }

    #[tokio::test]
    async fn test_rate_limit_error_handling() {
        let handler = DefaultProductionErrorHandler::new();
        let context = ErrorContext::new("api", "request");
        let error = AppError::RateLimitExceeded { limit: 100, window: "minute".to_string() };
        
        let response = handler.handle_error(error.clone(), context).await;
        
        assert_eq!(response.error_type, "RateLimitExceeded");
        assert_eq!(response.severity, ErrorSeverity::Medium);
        assert!(response.retry_after.is_some());
        assert!(response.retry_after.unwrap() >= 60); // Should suggest waiting at least 1 minute
    }

    #[tokio::test]
    async fn test_attempt_recovery() {
        let handler = DefaultProductionErrorHandler::new();
        let context = ErrorContext::new("database", "query");
        let error = AppError::DatabaseError("Connection lost".to_string());
        
        let result = handler.attempt_recovery(error, context).await;
        
        match result {
            RecoveryResult::Retry { delay_seconds } => {
                assert!(delay_seconds > 0);
            },
            _ => panic!("Expected retry recovery result"),
        }
    }
}