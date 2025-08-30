//! Tests for API error handling system

use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use tower::ServiceExt;
use uuid::Uuid;

use RustAutoDevOps::api::{
    error_middleware::{api_error_middleware, ApiResult, IntoApiError},
    errors::{
        create_error_response, resource_not_found_error, validation_error, ApiError,
        ApiErrorBuilder, ApiErrorCode, ApiErrorResponse, RecoverySuggestion, SuggestionPriority,
    },
};

#[tokio::test]
async fn test_api_error_builder() {
    let error = ApiErrorBuilder::new(ApiErrorCode::ValidationFailed)
        .message("Custom validation message")
        .details(json!({"field": "email", "value": "invalid"}))
        .suggestion(RecoverySuggestion {
            action: "fix_email".to_string(),
            description: "Provide a valid email address".to_string(),
            example: Some("user@example.com".to_string()),
            priority: SuggestionPriority::High,
        })
        .correlation_id(Uuid::new_v4())
        .build();

    assert_eq!(error.code, "VALIDATION_FAILED");
    assert_eq!(error.message, "Custom validation message");
    assert!(error.details.is_some());
    assert_eq!(error.recovery_suggestions.len(), 1);
    assert!(error.correlation_id.is_some());
}

#[tokio::test]
async fn test_error_code_status_mapping() {
    assert_eq!(ApiErrorCode::Unauthorized.status_code(), StatusCode::UNAUTHORIZED);
    assert_eq!(ApiErrorCode::Forbidden.status_code(), StatusCode::FORBIDDEN);
    assert_eq!(ApiErrorCode::ValidationFailed.status_code(), StatusCode::BAD_REQUEST);
    assert_eq!(ApiErrorCode::ResourceNotFound.status_code(), StatusCode::NOT_FOUND);
    assert_eq!(ApiErrorCode::ResourceConflict.status_code(), StatusCode::CONFLICT);
    assert_eq!(ApiErrorCode::RateLimitExceeded.status_code(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(ApiErrorCode::InternalServerError.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_validation_error_creation() {
    let mut field_errors = HashMap::new();
    field_errors.insert("email".to_string(), vec!["Invalid email format".to_string()]);
    field_errors.insert("age".to_string(), vec![
        "Must be a number".to_string(),
        "Must be between 18 and 120".to_string(),
    ]);

    let error = validation_error(field_errors);

    assert_eq!(error.code, "VALIDATION_FAILED");
    assert_eq!(error.message, "Input validation failed");
    assert!(error.details.is_some());
    
    // Should have recovery suggestions for each field plus general ones
    assert!(error.recovery_suggestions.len() >= 3);
    
    // Check that field-specific suggestions are included
    let field_suggestions: Vec<_> = error.recovery_suggestions
        .iter()
        .filter(|s| s.action.starts_with("fix_field_"))
        .collect();
    assert_eq!(field_suggestions.len(), 2); // One for each field
}

#[tokio::test]
async fn test_resource_not_found_error() {
    let error = resource_not_found_error("Pipeline", "123e4567-e89b-12d3-a456-426614174000");

    assert_eq!(error.code, "RESOURCE_NOT_FOUND");
    assert!(error.message.contains("Pipeline"));
    assert!(error.message.contains("123e4567-e89b-12d3-a456-426614174000"));
    assert!(error.details.is_some());
    
    if let Some(details) = error.details {
        assert_eq!(details["resource_type"], "Pipeline");
        assert_eq!(details["resource_id"], "123e4567-e89b-12d3-a456-426614174000");
    }
}

#[tokio::test]
async fn test_default_recovery_suggestions() {
    let unauthorized_suggestions = ApiErrorCode::Unauthorized.default_recovery_suggestions();
    assert!(!unauthorized_suggestions.is_empty());
    assert!(unauthorized_suggestions.iter().any(|s| s.action == "authenticate"));

    let validation_suggestions = ApiErrorCode::ValidationFailed.default_recovery_suggestions();
    assert!(!validation_suggestions.is_empty());
    assert!(validation_suggestions.iter().any(|s| s.action == "check_input"));

    let not_found_suggestions = ApiErrorCode::ResourceNotFound.default_recovery_suggestions();
    assert!(!not_found_suggestions.is_empty());
    assert!(not_found_suggestions.iter().any(|s| s.action == "verify_id"));
}

#[tokio::test]
async fn test_documentation_urls() {
    assert_eq!(
        ApiErrorCode::Unauthorized.documentation_url(),
        Some("/docs/authentication".to_string())
    );
    assert_eq!(
        ApiErrorCode::ValidationFailed.documentation_url(),
        Some("/docs/validation".to_string())
    );
    assert_eq!(
        ApiErrorCode::RateLimitExceeded.documentation_url(),
        Some("/docs/rate-limiting".to_string())
    );
}

#[tokio::test]
async fn test_error_response_creation() {
    let error = ApiErrorBuilder::new(ApiErrorCode::ValidationFailed)
        .message("Test error")
        .build();

    let request_id = Uuid::new_v4();
    let response = create_error_response(
        error,
        Some(request_id),
        Some("/api/v2/test".to_string()),
        Some("POST".to_string()),
    );

    assert_eq!(response.error.code, "VALIDATION_FAILED");
    assert_eq!(response.request_id, Some(request_id));
    assert_eq!(response.path, Some("/api/v2/test".to_string()));
    assert_eq!(response.method, Some("POST".to_string()));
    assert!(response.timestamp <= chrono::Utc::now());
}

#[tokio::test]
async fn test_error_code_parsing() {
    assert_eq!(
        ApiErrorCode::from_str("UNAUTHORIZED").unwrap(),
        ApiErrorCode::Unauthorized
    );
    assert_eq!(
        ApiErrorCode::from_str("VALIDATION_FAILED").unwrap(),
        ApiErrorCode::ValidationFailed
    );
    assert_eq!(
        ApiErrorCode::from_str("RESOURCE_NOT_FOUND").unwrap(),
        ApiErrorCode::ResourceNotFound
    );
    
    assert!(ApiErrorCode::from_str("INVALID_CODE").is_err());
    assert!(ApiErrorCode::from_str("").is_err());
}

// Test handlers for middleware testing
async fn success_handler() -> Result<Json<Value>, ApiErrorResponse> {
    Ok(Json(json!({"message": "success"})))
}

async fn validation_error_handler() -> Result<Json<Value>, ApiErrorResponse> {
    let mut field_errors = HashMap::new();
    field_errors.insert("email".to_string(), vec!["Invalid format".to_string()]);
    
    let error = validation_error(field_errors);
    let response = create_error_response(error, None, None, None);
    Err(response)
}

async fn not_found_handler() -> Result<Json<Value>, ApiErrorResponse> {
    let error = resource_not_found_error("Pipeline", "nonexistent-id");
    let response = create_error_response(error, None, None, None);
    Err(response)
}

async fn panic_handler() -> Result<Json<Value>, ApiErrorResponse> {
    panic!("Test panic for error handling");
}

#[tokio::test]
async fn test_error_middleware_success() {
    let app = Router::new()
        .route("/success", get(success_handler))
        .layer(axum::middleware::from_fn(api_error_middleware));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/success")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("X-Request-ID"));
}

#[tokio::test]
async fn test_error_middleware_validation_error() {
    let app = Router::new()
        .route("/validation-error", get(validation_error_handler))
        .layer(axum::middleware::from_fn(api_error_middleware));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/validation-error")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().contains_key("X-Request-ID"));
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error_response: ApiErrorResponse = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(error_response.error.code, "VALIDATION_FAILED");
    assert!(error_response.request_id.is_some());
}

#[tokio::test]
async fn test_error_middleware_not_found() {
    let app = Router::new()
        .route("/not-found", get(not_found_handler))
        .layer(axum::middleware::from_fn(api_error_middleware));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/not-found")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error_response: ApiErrorResponse = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(error_response.error.code, "RESOURCE_NOT_FOUND");
}

#[tokio::test]
async fn test_error_middleware_404_enhancement() {
    let app = Router::new()
        .layer(axum::middleware::from_fn(api_error_middleware));

    let request = Request::builder()
        .method(Method::GET)
        .uri("/nonexistent-endpoint")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(response.headers().contains_key("X-Request-ID"));
}

#[tokio::test]
async fn test_error_middleware_method_not_allowed() {
    let app = Router::new()
        .route("/test", get(success_handler))
        .layer(axum::middleware::from_fn(api_error_middleware));

    let request = Request::builder()
        .method(Method::POST) // GET endpoint called with POST
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(response.headers().contains_key("X-Request-ID"));
}

#[tokio::test]
async fn test_api_error_macros() {
    // Test api_error! macro
    let error1 = crate::api_error!(ApiErrorCode::ValidationFailed);
    assert_eq!(error1.code, "VALIDATION_FAILED");

    let error2 = crate::api_error!(ApiErrorCode::ValidationFailed, "Custom message");
    assert_eq!(error2.message, "Custom message");

    let error3 = crate::api_error!(
        ApiErrorCode::ValidationFailed,
        "Custom message",
        json!({"field": "email"})
    );
    assert!(error3.details.is_some());

    // Test error_response! macro
    let response1 = crate::error_response!(ApiErrorCode::ValidationFailed);
    assert_eq!(response1.error.code, "VALIDATION_FAILED");

    let response2 = crate::error_response!(ApiErrorCode::ValidationFailed, "Custom message");
    assert_eq!(response2.error.message, "Custom message");
}

#[tokio::test]
async fn test_suggestion_priorities() {
    let high_priority = RecoverySuggestion {
        action: "critical_fix".to_string(),
        description: "Fix this immediately".to_string(),
        example: None,
        priority: SuggestionPriority::High,
    };

    let medium_priority = RecoverySuggestion {
        action: "moderate_fix".to_string(),
        description: "Fix this soon".to_string(),
        example: None,
        priority: SuggestionPriority::Medium,
    };

    let low_priority = RecoverySuggestion {
        action: "minor_fix".to_string(),
        description: "Fix this when convenient".to_string(),
        example: None,
        priority: SuggestionPriority::Low,
    };

    // Test serialization
    let high_json = serde_json::to_value(&high_priority).unwrap();
    assert_eq!(high_json["priority"], "High");

    let medium_json = serde_json::to_value(&medium_priority).unwrap();
    assert_eq!(medium_json["priority"], "Medium");

    let low_json = serde_json::to_value(&low_priority).unwrap();
    assert_eq!(low_json["priority"], "Low");
}

#[tokio::test]
async fn test_error_correlation_ids() {
    let correlation_id = Uuid::new_v4();
    
    let error = ApiErrorBuilder::new(ApiErrorCode::InternalServerError)
        .correlation_id(correlation_id)
        .build();

    assert_eq!(error.correlation_id, Some(correlation_id));

    let response = create_error_response(
        error,
        Some(Uuid::new_v4()),
        None,
        None,
    );

    assert_eq!(response.error.correlation_id, Some(correlation_id));
}

#[tokio::test]
async fn test_pipeline_specific_errors() {
    let pipeline_not_found = ApiErrorBuilder::new(ApiErrorCode::PipelineNotFound)
        .message("Pipeline with ID 'test-pipeline' not found")
        .build();

    assert_eq!(pipeline_not_found.code, "PIPELINE_NOT_FOUND");
    assert_eq!(pipeline_not_found.status_code(), StatusCode::NOT_FOUND);

    let execution_failed = ApiErrorBuilder::new(ApiErrorCode::PipelineExecutionFailed)
        .message("Pipeline execution failed due to configuration error")
        .build();

    assert_eq!(execution_failed.code, "PIPELINE_EXECUTION_FAILED");
    assert_eq!(execution_failed.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_rate_limit_error_with_headers() {
    use RustAutoDevOps::api::errors::rate_limit_error;
    
    let error = rate_limit_error(100, 3600, 300);
    
    assert_eq!(error.code, "RATE_LIMIT_EXCEEDED");
    assert!(error.message.contains("100 requests per 3600 seconds"));
    
    if let Some(details) = error.details {
        assert_eq!(details["limit"], 100);
        assert_eq!(details["window_seconds"], 3600);
        assert_eq!(details["retry_after_seconds"], 300);
    }
    
    // Should have a specific retry suggestion
    let retry_suggestion = error.recovery_suggestions
        .iter()
        .find(|s| s.action == "wait_and_retry");
    assert!(retry_suggestion.is_some());
}