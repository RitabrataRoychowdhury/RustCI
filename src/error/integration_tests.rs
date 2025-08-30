use crate::error::{
    AppError, ErrorContext, DefaultProductionErrorHandler, ProductionErrorHandler,
    RetryManager, RetryPolicy, CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Integration tests for the enhanced error handling system
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_error_handler_with_retry_manager_integration() {
        let error_handler = DefaultProductionErrorHandler::new();
        let retry_manager = RetryManager::new();
        let context = ErrorContext::new("integration_test", "test_operation");
        
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        // Test successful retry after failures
        let result = retry_manager.execute_with_retry(
            || {
                let count = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count < 2 {
                        Err(AppError::DatabaseError("Connection failed".to_string()))
                    } else {
                        Ok("success".to_string())
                    }
                }
            },
            context.clone(),
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
        
        // Test error handling for the failed attempts
        let error = AppError::DatabaseError("Test error".to_string());
        let error_response = error_handler.handle_error(error, context).await;
        
        assert_eq!(error_response.error_type, "DatabaseError");
        assert!(!error_response.recovery_suggestions.is_empty());
        assert!(error_response.retry_after.is_some());
    }

    #[tokio::test]
    async fn test_circuit_breaker_with_error_handler_integration() {
        let error_handler = DefaultProductionErrorHandler::new();
        let circuit_breaker = CircuitBreaker::with_config(
            "integration_test",
            CircuitBreakerConfig {
                failure_threshold: 2,
                minimum_requests: 2,
                failure_rate_threshold: 0.5,
                timeout_ms: 100, // Short timeout for testing
                half_open_max_calls: 1,
                time_window_ms: 1000,
            }
        );
        
        let context = ErrorContext::new("integration_test", "circuit_breaker_test");
        
        // First failure
        let result1 = circuit_breaker.execute(
            || async { Err::<String, AppError>(AppError::ExternalServiceError("Service down".to_string())) },
            context.clone(),
        ).await;
        assert!(result1.is_err());
        assert_eq!(circuit_breaker.get_state().await, CircuitBreakerState::Closed);
        
        // Second failure - should open the circuit
        let result2 = circuit_breaker.execute(
            || async { Err::<String, AppError>(AppError::ExternalServiceError("Service down".to_string())) },
            context.clone(),
        ).await;
        assert!(result2.is_err());
        assert_eq!(circuit_breaker.get_state().await, CircuitBreakerState::Open);
        
        // Third request should be rejected immediately
        let result3 = circuit_breaker.execute(
            || async { Ok::<String, AppError>("success".to_string()) },
            context.clone(),
        ).await;
        assert!(result3.is_err());
        assert!(result3.unwrap_err().to_string().contains("Circuit breaker"));
        
        // Handle the circuit breaker error
        let cb_error = AppError::ServiceUnavailable("Circuit breaker open".to_string());
        let error_response = error_handler.handle_error(cb_error, context).await;
        
        assert_eq!(error_response.error_type, "ServiceUnavailable");
        assert!(!error_response.recovery_suggestions.is_empty());
    }

    #[tokio::test]
    async fn test_error_context_propagation() {
        let parent_context = ErrorContext::new("parent_service", "parent_operation")
            .with_request_id("req-123")
            .with_user_id(uuid::Uuid::new_v4());
        
        let child_context = parent_context.create_child_context("child_service", "child_operation");
        
        // Verify context propagation
        assert_eq!(child_context.request_id, parent_context.request_id);
        assert_eq!(child_context.user_id, parent_context.user_id);
        assert_eq!(child_context.parent_correlation_id, Some(parent_context.correlation_id));
        assert_ne!(child_context.correlation_id, parent_context.correlation_id);
        
        // Test error handling with child context
        let error_handler = DefaultProductionErrorHandler::new();
        let error = AppError::ValidationError("Invalid input".to_string());
        let error_response = error_handler.handle_error(error, child_context).await;
        
        assert_eq!(error_response.context.parent_correlation_id, Some(parent_context.correlation_id));
    }

    #[tokio::test]
    async fn test_retry_with_circuit_breaker_combination() {
        let retry_manager = RetryManager::new();
        let circuit_breaker = CircuitBreaker::with_config(
            "combined_test",
            CircuitBreakerConfig {
                failure_threshold: 3,
                minimum_requests: 3,
                failure_rate_threshold: 1.0, // 100% failure rate to open
                timeout_ms: 50,
                half_open_max_calls: 1,
                time_window_ms: 1000,
            }
        );
        
        let context = ErrorContext::new("combined_test", "retry_with_circuit_breaker");
        let failure_count = Arc::new(AtomicU32::new(0));
        let failure_count_clone = failure_count.clone();
        
        // Test that circuit breaker prevents excessive retries
        let context_clone = context.clone();
        let result = retry_manager.execute_with_retry(
            || {
                let cb = circuit_breaker.clone();
                let ctx = context.clone();
                let count_clone = failure_count_clone.clone();
                
                async move {
                    cb.execute(
                        || {
                            count_clone.fetch_add(1, Ordering::SeqCst);
                            async { Err::<String, AppError>(AppError::ExternalServiceError("Always fails".to_string())) }
                        },
                        ctx,
                    ).await
                }
            },
            context_clone,
        ).await;
        
        assert!(result.is_err());
        
        // Circuit breaker should have opened and prevented some retry attempts
        let final_count = failure_count.load(Ordering::SeqCst);
        assert!(final_count > 0);
        
        // Verify circuit breaker is open
        assert_eq!(circuit_breaker.get_state().await, CircuitBreakerState::Open);
    }

    #[tokio::test]
    async fn test_error_recovery_strategies() {
        let error_handler = DefaultProductionErrorHandler::new();
        let context = ErrorContext::new("recovery_test", "test_strategies");
        
        // Test different error types and their recovery strategies
        let database_error = AppError::DatabaseError("Connection lost".to_string());
        let recovery_result = error_handler.attempt_recovery(database_error, context.clone()).await;
        
        match recovery_result {
            crate::error::RecoveryResult::Retry { delay_seconds } => {
                assert!(delay_seconds > 0);
            }
            _ => panic!("Expected retry recovery for database error"),
        }
        
        // Test non-recoverable error
        let validation_error = AppError::ValidationError("Invalid format".to_string());
        let recovery_result = error_handler.attempt_recovery(validation_error, context).await;
        
        match recovery_result {
            crate::error::RecoveryResult::Failed => {
                // Expected for validation errors
            }
            _ => panic!("Expected failed recovery for validation error"),
        }
    }

    #[tokio::test]
    async fn test_error_severity_classification() {
        let error_handler = DefaultProductionErrorHandler::new();
        let context = ErrorContext::new("severity_test", "classification");
        
        // Test critical error
        let security_error = AppError::SecurityError("Authentication bypass detected".to_string());
        let response = error_handler.handle_error(security_error, context.clone()).await;
        assert_eq!(response.severity, crate::error::ErrorSeverity::Critical);
        
        // Test high severity error
        let service_error = AppError::ExternalServiceError("Payment service down".to_string());
        let response = error_handler.handle_error(service_error, context.clone()).await;
        assert_eq!(response.severity, crate::error::ErrorSeverity::High);
        
        // Test low severity error
        let validation_error = AppError::ValidationError("Missing field".to_string());
        let response = error_handler.handle_error(validation_error, context).await;
        assert_eq!(response.severity, crate::error::ErrorSeverity::Low);
    }

    #[tokio::test]
    async fn test_comprehensive_error_flow() {
        // This test simulates a complete error handling flow
        let error_handler = DefaultProductionErrorHandler::new();
        let retry_manager = RetryManager::new();
        let circuit_breaker = CircuitBreaker::new("comprehensive_test");
        
        let context = ErrorContext::new("comprehensive_test", "full_flow")
            .with_request_id("req-comprehensive-123")
            .with_context("user_action", serde_json::json!("create_resource"));
        
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        // Simulate a service that fails a few times then succeeds
        let result = retry_manager.execute_with_retry(
            || {
                let cb = circuit_breaker.clone();
                let ctx = context.clone();
                let count_clone = attempt_count_clone.clone();
                
                async move {
                    cb.execute(
                        || {
                            let count = count_clone.fetch_add(1, Ordering::SeqCst);
                            async move {
                                if count < 2 {
                                    Err(AppError::ExternalServiceError("Temporary service unavailable".to_string()))
                                } else {
                                    Ok("Resource created successfully".to_string())
                                }
                            }
                        },
                        ctx,
                    ).await
                }
            },
            context.clone(),
        ).await;
        
        // Should succeed after retries
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Resource created successfully");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
        
        // Circuit breaker should still be closed
        assert_eq!(circuit_breaker.get_state().await, CircuitBreakerState::Closed);
        
        // Test error handling for a failed scenario
        let final_error = AppError::ExternalServiceError("Service permanently down".to_string());
        let error_response = error_handler.handle_error(final_error, context).await;
        
        // Verify comprehensive error response
        assert_eq!(error_response.error_type, "ExternalServiceError");
        assert_eq!(error_response.severity, crate::error::ErrorSeverity::High);
        assert!(!error_response.recovery_suggestions.is_empty());
        assert!(error_response.retry_after.is_some());
        assert!(error_response.context.additional_context.contains_key("user_action"));
    }
}