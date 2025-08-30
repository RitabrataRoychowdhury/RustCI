//! API Error Handling Middleware
//!
//! This module provides middleware for consistent error handling across all API endpoints.

use crate::api::errors::{create_error_response, ApiError, ApiErrorBuilder, ApiErrorCode, ApiErrorResponse};
use axum::{
    extract::{MatchedPath, Request},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use tracing::{error, warn};
use uuid::Uuid;

/// Middleware for consistent API error handling
pub async fn api_error_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let request_id = Uuid::new_v4();

    // Add request ID to request extensions for handlers to use
    let mut req = req;
    req.extensions_mut().insert(request_id);

    // Execute the request with panic handling
    let response = match catch_unwind(AssertUnwindSafe(|| {
        tokio::runtime::Handle::current().block_on(async {
            next.run(req).await
        })
    })) {
        Ok(response) => response,
        Err(panic_info) => {
            // Handle panics as internal server errors
            error!(
                request_id = %request_id,
                method = %method,
                path = %path,
                panic_info = ?panic_info,
                "Handler panicked"
            );

            let error = ApiErrorBuilder::new(ApiErrorCode::InternalServerError)
                .message("An unexpected error occurred")
                .correlation_id(request_id)
                .build();

            let error_response = create_error_response(
                error,
                Some(request_id),
                Some(path),
                Some(method.to_string()),
            );

            return error_response.into_response();
        }
    };

    // Check if the response is an error and enhance it if needed
    enhance_error_response(response, request_id, &path, &method)
}

/// Enhance error responses with consistent formatting
fn enhance_error_response(
    mut response: Response,
    request_id: Uuid,
    path: &str,
    method: &Method,
) -> Response {
    // Add request ID header to all responses
    response.headers_mut().insert(
        "X-Request-ID",
        request_id.to_string().parse().unwrap(),
    );

    // If it's an error response, check if it needs enhancement
    if response.status().is_client_error() || response.status().is_server_error() {
        // Check if the response body is already in our standard format
        // If not, convert it to our standard format
        match response.status() {
            StatusCode::NOT_FOUND => {
                // Convert generic 404 to our format if it's not already
                if !is_standard_error_format(&response) {
                    let error = ApiErrorBuilder::new(ApiErrorCode::ResourceNotFound)
                        .message("The requested resource was not found")
                        .correlation_id(request_id)
                        .build();

                    let error_response = create_error_response(
                        error,
                        Some(request_id),
                        Some(path.to_string()),
                        Some(method.to_string()),
                    );

                    return error_response.into_response();
                }
            }
            StatusCode::METHOD_NOT_ALLOWED => {
                let error = ApiErrorBuilder::new(ApiErrorCode::InvalidInput)
                    .message(format!("Method {} not allowed for this endpoint", method))
                    .correlation_id(request_id)
                    .build();

                let error_response = create_error_response(
                    error,
                    Some(request_id),
                    Some(path.to_string()),
                    Some(method.to_string()),
                );

                return error_response.into_response();
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                let error = ApiErrorBuilder::new(ApiErrorCode::ValidationFailed)
                    .message("Request validation failed")
                    .correlation_id(request_id)
                    .build();

                let error_response = create_error_response(
                    error,
                    Some(request_id),
                    Some(path.to_string()),
                    Some(method.to_string()),
                );

                return error_response.into_response();
            }
            StatusCode::INTERNAL_SERVER_ERROR => {
                // Log internal server errors
                error!(
                    request_id = %request_id,
                    method = %method,
                    path = %path,
                    status = %response.status(),
                    "Internal server error occurred"
                );

                if !is_standard_error_format(&response) {
                    let error = ApiErrorBuilder::new(ApiErrorCode::InternalServerError)
                        .message("An internal server error occurred")
                        .correlation_id(request_id)
                        .build();

                    let error_response = create_error_response(
                        error,
                        Some(request_id),
                        Some(path.to_string()),
                        Some(method.to_string()),
                    );

                    return error_response.into_response();
                }
            }
            _ => {
                // Log other client/server errors
                if response.status().is_server_error() {
                    error!(
                        request_id = %request_id,
                        method = %method,
                        path = %path,
                        status = %response.status(),
                        "Server error occurred"
                    );
                } else {
                    warn!(
                        request_id = %request_id,
                        method = %method,
                        path = %path,
                        status = %response.status(),
                        "Client error occurred"
                    );
                }
            }
        }
    }

    response
}

/// Check if the response is already in our standard error format
fn is_standard_error_format(response: &Response) -> bool {
    // Check if the response has the content-type header indicating JSON
    response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false)
}

/// Convert application errors to API errors
pub trait IntoApiError {
    fn into_api_error(self) -> ApiError;
}

/// Implementation for common error types
impl IntoApiError for crate::error::AppError {
    fn into_api_error(self) -> ApiError {
        match self {
            crate::error::AppError::ValidationError(message) => {
                let mut field_errors = std::collections::HashMap::new();
                field_errors.insert("field".to_string(), vec![message]);
                crate::api::errors::validation_error(field_errors)
            }
            crate::error::AppError::NotFound(message) => {
                crate::api::errors::resource_not_found_error("resource", &message)
            }
            crate::error::AppError::Forbidden(message) => {
                ApiErrorBuilder::new(ApiErrorCode::Forbidden)
                    .message(message)
                    .build()
            }
            crate::error::AppError::DatabaseError(message) => {
                ApiErrorBuilder::new(ApiErrorCode::DatabaseError)
                    .message("Database operation failed")
                    .build()
            }
            crate::error::AppError::ExternalServiceError(message) => {
                ApiErrorBuilder::new(ApiErrorCode::ExternalServiceError)
                    .message(format!("External service error: {}", message))
                    .build()
            }
            _ => {
                ApiErrorBuilder::new(ApiErrorCode::InternalServerError)
                    .message("An unexpected error occurred")
                    .build()
            }
        }
    }
}

/// Helper function to create a standardized error response
pub fn create_api_error_response<E: IntoApiError>(
    error: E,
    request_id: Option<Uuid>,
    path: Option<String>,
    method: Option<String>,
) -> ApiErrorResponse {
    let api_error = error.into_api_error();
    create_error_response(api_error, request_id, path, method)
}

/// Macro for easy error response creation in handlers
#[macro_export]
macro_rules! api_error {
    ($code:expr) => {
        $crate::api::errors::ApiErrorBuilder::new($code).build()
    };
    ($code:expr, $message:expr) => {
        $crate::api::errors::ApiErrorBuilder::new($code)
            .message($message)
            .build()
    };
    ($code:expr, $message:expr, $details:expr) => {
        $crate::api::errors::ApiErrorBuilder::new($code)
            .message($message)
            .details($details)
            .build()
    };
}

/// Macro for creating error responses in handlers
#[macro_export]
macro_rules! error_response {
    ($code:expr) => {{
        let error = $crate::api_error!($code);
        $crate::api::errors::create_error_response(error, None, None, None)
    }};
    ($code:expr, $message:expr) => {{
        let error = $crate::api_error!($code, $message);
        $crate::api::errors::create_error_response(error, None, None, None)
    }};
    ($code:expr, $message:expr, $details:expr) => {{
        let error = $crate::api_error!($code, $message, $details);
        $crate::api::errors::create_error_response(error, None, None, None)
    }};
}

/// Result type for API handlers
pub type ApiResult<T> = Result<T, ApiErrorResponse>;

/// Helper trait for converting Results to API responses
pub trait IntoApiResult<T> {
    fn into_api_result(self) -> ApiResult<T>;
}

impl<T, E: IntoApiError> IntoApiResult<T> for Result<T, E> {
    fn into_api_result(self) -> ApiResult<T> {
        self.map_err(|e| {
            let api_error = e.into_api_error();
            create_error_response(api_error, None, None, None)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    async fn test_handler() -> Result<Json<serde_json::Value>, ApiErrorResponse> {
        Ok(Json(json!({"message": "success"})))
    }

    async fn error_handler() -> Result<Json<serde_json::Value>, ApiErrorResponse> {
        Err(create_error_response(
            ApiErrorBuilder::new(ApiErrorCode::ValidationFailed)
                .message("Test validation error")
                .build(),
            None,
            None,
            None,
        ))
    }

    async fn panic_handler() -> Result<Json<serde_json::Value>, ApiErrorResponse> {
        panic!("Test panic");
    }

    #[tokio::test]
    async fn test_successful_request() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(axum::middleware::from_fn(api_error_middleware));

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("X-Request-ID"));
    }

    #[tokio::test]
    async fn test_error_response() {
        let app = Router::new()
            .route("/error", get(error_handler))
            .layer(axum::middleware::from_fn(api_error_middleware));

        let request = Request::builder()
            .uri("/error")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response.headers().contains_key("X-Request-ID"));
    }

    #[tokio::test]
    async fn test_not_found_enhancement() {
        let app = Router::new()
            .layer(axum::middleware::from_fn(api_error_middleware));

        let request = Request::builder()
            .uri("/nonexistent")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().contains_key("X-Request-ID"));
    }

    #[test]
    fn test_api_error_macro() {
        let error = api_error!(ApiErrorCode::ValidationFailed);
        assert_eq!(error.code, "VALIDATION_FAILED");

        let error = api_error!(ApiErrorCode::ValidationFailed, "Custom message");
        assert_eq!(error.message, "Custom message");

        let error = api_error!(
            ApiErrorCode::ValidationFailed,
            "Custom message",
            json!({"field": "value"})
        );
        assert!(error.details.is_some());
    }

    #[test]
    fn test_error_response_macro() {
        let response = error_response!(ApiErrorCode::ValidationFailed);
        assert_eq!(response.error.code, "VALIDATION_FAILED");

        let response = error_response!(ApiErrorCode::ValidationFailed, "Custom message");
        assert_eq!(response.error.message, "Custom message");
    }
}