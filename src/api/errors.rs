//! Consistent API Error Handling System
//!
//! This module provides standardized error response formats, actionable error codes,
//! and recovery suggestions for all API endpoints.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use tracing::{error, warn};
use uuid::Uuid;

/// Standardized API error response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiError,
    pub request_id: Option<Uuid>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub path: Option<String>,
    pub method: Option<String>,
}

/// Core API error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub recovery_suggestions: Vec<RecoverySuggestion>,
    pub documentation_url: Option<String>,
    pub correlation_id: Option<Uuid>,
}

/// Recovery suggestion for error resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySuggestion {
    pub action: String,
    pub description: String,
    pub example: Option<String>,
    pub priority: SuggestionPriority,
}

/// Priority level for recovery suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionPriority {
    High,
    Medium,
    Low,
}

/// Standard API error codes
#[derive(Debug, Clone, PartialEq)]
pub enum ApiErrorCode {
    // Authentication & Authorization
    Unauthorized,
    Forbidden,
    TokenExpired,
    TokenInvalid,
    MfaRequired,
    
    // Validation
    ValidationFailed,
    InvalidInput,
    MissingRequiredField,
    InvalidFormat,
    ValueOutOfRange,
    
    // Resource Management
    ResourceNotFound,
    ResourceAlreadyExists,
    ResourceConflict,
    ResourceLocked,
    
    // Rate Limiting
    RateLimitExceeded,
    QuotaExceeded,
    TooManyRequests,
    
    // System Errors
    InternalServerError,
    ServiceUnavailable,
    DatabaseError,
    ExternalServiceError,
    TimeoutError,
    
    // API Versioning
    UnsupportedApiVersion,
    DeprecatedEndpoint,
    
    // Pipeline Specific
    PipelineNotFound,
    PipelineExecutionFailed,
    InvalidPipelineConfig,
    ExecutionNotFound,
    ExecutionAlreadyCancelled,
    
    // File Operations
    FileUploadFailed,
    FileNotFound,
    FileSizeTooLarge,
    UnsupportedFileType,
    
    // Custom/Unknown
    Unknown,
}

impl fmt::Display for ApiErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            ApiErrorCode::Unauthorized => "UNAUTHORIZED",
            ApiErrorCode::Forbidden => "FORBIDDEN",
            ApiErrorCode::TokenExpired => "TOKEN_EXPIRED",
            ApiErrorCode::TokenInvalid => "TOKEN_INVALID",
            ApiErrorCode::MfaRequired => "MFA_REQUIRED",
            
            ApiErrorCode::ValidationFailed => "VALIDATION_FAILED",
            ApiErrorCode::InvalidInput => "INVALID_INPUT",
            ApiErrorCode::MissingRequiredField => "MISSING_REQUIRED_FIELD",
            ApiErrorCode::InvalidFormat => "INVALID_FORMAT",
            ApiErrorCode::ValueOutOfRange => "VALUE_OUT_OF_RANGE",
            
            ApiErrorCode::ResourceNotFound => "RESOURCE_NOT_FOUND",
            ApiErrorCode::ResourceAlreadyExists => "RESOURCE_ALREADY_EXISTS",
            ApiErrorCode::ResourceConflict => "RESOURCE_CONFLICT",
            ApiErrorCode::ResourceLocked => "RESOURCE_LOCKED",
            
            ApiErrorCode::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            ApiErrorCode::QuotaExceeded => "QUOTA_EXCEEDED",
            ApiErrorCode::TooManyRequests => "TOO_MANY_REQUESTS",
            
            ApiErrorCode::InternalServerError => "INTERNAL_SERVER_ERROR",
            ApiErrorCode::ServiceUnavailable => "SERVICE_UNAVAILABLE",
            ApiErrorCode::DatabaseError => "DATABASE_ERROR",
            ApiErrorCode::ExternalServiceError => "EXTERNAL_SERVICE_ERROR",
            ApiErrorCode::TimeoutError => "TIMEOUT_ERROR",
            
            ApiErrorCode::UnsupportedApiVersion => "UNSUPPORTED_API_VERSION",
            ApiErrorCode::DeprecatedEndpoint => "DEPRECATED_ENDPOINT",
            
            ApiErrorCode::PipelineNotFound => "PIPELINE_NOT_FOUND",
            ApiErrorCode::PipelineExecutionFailed => "PIPELINE_EXECUTION_FAILED",
            ApiErrorCode::InvalidPipelineConfig => "INVALID_PIPELINE_CONFIG",
            ApiErrorCode::ExecutionNotFound => "EXECUTION_NOT_FOUND",
            ApiErrorCode::ExecutionAlreadyCancelled => "EXECUTION_ALREADY_CANCELLED",
            
            ApiErrorCode::FileUploadFailed => "FILE_UPLOAD_FAILED",
            ApiErrorCode::FileNotFound => "FILE_NOT_FOUND",
            ApiErrorCode::FileSizeTooLarge => "FILE_SIZE_TOO_LARGE",
            ApiErrorCode::UnsupportedFileType => "UNSUPPORTED_FILE_TYPE",
            
            ApiErrorCode::Unknown => "UNKNOWN_ERROR",
        };
        write!(f, "{}", code)
    }
}

impl ApiErrorCode {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiErrorCode::Unauthorized | ApiErrorCode::TokenExpired | ApiErrorCode::TokenInvalid => {
                StatusCode::UNAUTHORIZED
            }
            ApiErrorCode::Forbidden | ApiErrorCode::MfaRequired => StatusCode::FORBIDDEN,
            
            ApiErrorCode::ValidationFailed
            | ApiErrorCode::InvalidInput
            | ApiErrorCode::MissingRequiredField
            | ApiErrorCode::InvalidFormat
            | ApiErrorCode::ValueOutOfRange
            | ApiErrorCode::UnsupportedApiVersion
            | ApiErrorCode::InvalidPipelineConfig
            | ApiErrorCode::FileSizeTooLarge
            | ApiErrorCode::UnsupportedFileType => StatusCode::BAD_REQUEST,
            
            ApiErrorCode::ResourceNotFound
            | ApiErrorCode::PipelineNotFound
            | ApiErrorCode::ExecutionNotFound
            | ApiErrorCode::FileNotFound => StatusCode::NOT_FOUND,
            
            ApiErrorCode::ResourceAlreadyExists => StatusCode::CONFLICT,
            ApiErrorCode::ResourceConflict | ApiErrorCode::ExecutionAlreadyCancelled => {
                StatusCode::CONFLICT
            }
            ApiErrorCode::ResourceLocked => StatusCode::LOCKED,
            
            ApiErrorCode::RateLimitExceeded | ApiErrorCode::TooManyRequests => {
                StatusCode::TOO_MANY_REQUESTS
            }
            ApiErrorCode::QuotaExceeded => StatusCode::PAYMENT_REQUIRED,
            
            ApiErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::TimeoutError => StatusCode::REQUEST_TIMEOUT,
            
            ApiErrorCode::DeprecatedEndpoint => StatusCode::GONE,
            
            ApiErrorCode::InternalServerError
            | ApiErrorCode::DatabaseError
            | ApiErrorCode::ExternalServiceError
            | ApiErrorCode::PipelineExecutionFailed
            | ApiErrorCode::FileUploadFailed
            | ApiErrorCode::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get default recovery suggestions for this error type
    pub fn default_recovery_suggestions(&self) -> Vec<RecoverySuggestion> {
        match self {
            ApiErrorCode::Unauthorized => vec![
                RecoverySuggestion {
                    action: "authenticate".to_string(),
                    description: "Provide valid authentication credentials".to_string(),
                    example: Some("Include 'Authorization: Bearer <token>' header".to_string()),
                    priority: SuggestionPriority::High,
                },
                RecoverySuggestion {
                    action: "refresh_token".to_string(),
                    description: "Refresh your authentication token if expired".to_string(),
                    example: Some("POST /auth/refresh with refresh token".to_string()),
                    priority: SuggestionPriority::High,
                },
            ],
            
            ApiErrorCode::ValidationFailed => vec![
                RecoverySuggestion {
                    action: "check_input".to_string(),
                    description: "Review and correct the input data".to_string(),
                    example: Some("Ensure all required fields are provided and properly formatted".to_string()),
                    priority: SuggestionPriority::High,
                },
                RecoverySuggestion {
                    action: "consult_docs".to_string(),
                    description: "Check API documentation for correct format".to_string(),
                    example: Some("Visit /docs/api for field specifications".to_string()),
                    priority: SuggestionPriority::Medium,
                },
            ],
            
            ApiErrorCode::ResourceNotFound => vec![
                RecoverySuggestion {
                    action: "verify_id".to_string(),
                    description: "Verify the resource ID is correct".to_string(),
                    example: Some("Check that the UUID format is valid".to_string()),
                    priority: SuggestionPriority::High,
                },
                RecoverySuggestion {
                    action: "list_resources".to_string(),
                    description: "List available resources to find the correct ID".to_string(),
                    example: Some("GET /api/v2/pipelines to see available pipelines".to_string()),
                    priority: SuggestionPriority::Medium,
                },
            ],
            
            ApiErrorCode::RateLimitExceeded => vec![
                RecoverySuggestion {
                    action: "wait_and_retry".to_string(),
                    description: "Wait for the rate limit window to reset".to_string(),
                    example: Some("Check 'Retry-After' header for wait time".to_string()),
                    priority: SuggestionPriority::High,
                },
                RecoverySuggestion {
                    action: "reduce_frequency".to_string(),
                    description: "Reduce the frequency of API calls".to_string(),
                    example: Some("Implement exponential backoff or batching".to_string()),
                    priority: SuggestionPriority::Medium,
                },
            ],
            
            ApiErrorCode::InternalServerError => vec![
                RecoverySuggestion {
                    action: "retry_later".to_string(),
                    description: "Retry the request after a short delay".to_string(),
                    example: Some("Wait 30 seconds and try again".to_string()),
                    priority: SuggestionPriority::High,
                },
                RecoverySuggestion {
                    action: "contact_support".to_string(),
                    description: "Contact support if the issue persists".to_string(),
                    example: Some("Include the request ID in your support request".to_string()),
                    priority: SuggestionPriority::Medium,
                },
            ],
            
            _ => vec![
                RecoverySuggestion {
                    action: "check_documentation".to_string(),
                    description: "Consult the API documentation for guidance".to_string(),
                    example: Some("Visit /docs/api for detailed information".to_string()),
                    priority: SuggestionPriority::Medium,
                },
            ],
        }
    }

    /// Get documentation URL for this error type
    pub fn documentation_url(&self) -> Option<String> {
        match self {
            ApiErrorCode::Unauthorized | ApiErrorCode::Forbidden => {
                Some("/docs/authentication".to_string())
            }
            ApiErrorCode::ValidationFailed | ApiErrorCode::InvalidInput => {
                Some("/docs/validation".to_string())
            }
            ApiErrorCode::RateLimitExceeded => Some("/docs/rate-limiting".to_string()),
            ApiErrorCode::UnsupportedApiVersion => Some("/docs/versioning".to_string()),
            ApiErrorCode::PipelineNotFound | ApiErrorCode::PipelineExecutionFailed => {
                Some("/docs/pipelines".to_string())
            }
            _ => Some("/docs/errors".to_string()),
        }
    }
}

/// Builder for creating API errors
pub struct ApiErrorBuilder {
    code: ApiErrorCode,
    message: Option<String>,
    details: Option<serde_json::Value>,
    recovery_suggestions: Vec<RecoverySuggestion>,
    documentation_url: Option<String>,
    correlation_id: Option<Uuid>,
}

impl ApiErrorBuilder {
    /// Create a new error builder
    pub fn new(code: ApiErrorCode) -> Self {
        Self {
            code,
            message: None,
            details: None,
            recovery_suggestions: Vec::new(),
            documentation_url: None,
            correlation_id: None,
        }
    }

    /// Set custom error message
    pub fn message<S: Into<String>>(mut self, message: S) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Add error details
    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Add a recovery suggestion
    pub fn suggestion(mut self, suggestion: RecoverySuggestion) -> Self {
        self.recovery_suggestions.push(suggestion);
        self
    }

    /// Add multiple recovery suggestions
    pub fn suggestions(mut self, suggestions: Vec<RecoverySuggestion>) -> Self {
        self.recovery_suggestions.extend(suggestions);
        self
    }

    /// Set documentation URL
    pub fn documentation_url<S: Into<String>>(mut self, url: S) -> Self {
        self.documentation_url = Some(url.into());
        self
    }

    /// Set correlation ID
    pub fn correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Build the API error
    pub fn build(self) -> ApiError {
        let message = self.message.unwrap_or_else(|| {
            match self.code {
                ApiErrorCode::Unauthorized => "Authentication required".to_string(),
                ApiErrorCode::Forbidden => "Access denied".to_string(),
                ApiErrorCode::ValidationFailed => "Input validation failed".to_string(),
                ApiErrorCode::ResourceNotFound => "Resource not found".to_string(),
                ApiErrorCode::InternalServerError => "Internal server error occurred".to_string(),
                _ => "An error occurred".to_string(),
            }
        });

        let mut recovery_suggestions = self.recovery_suggestions;
        if recovery_suggestions.is_empty() {
            recovery_suggestions = self.code.default_recovery_suggestions();
        }

        let documentation_url = self.documentation_url
            .or_else(|| self.code.documentation_url());

        ApiError {
            code: self.code.to_string(),
            message,
            details: self.details,
            recovery_suggestions,
            documentation_url,
            correlation_id: self.correlation_id,
        }
    }
}

/// Create an API error response
pub fn create_error_response(
    error: ApiError,
    request_id: Option<Uuid>,
    path: Option<String>,
    method: Option<String>,
) -> ApiErrorResponse {
    ApiErrorResponse {
        error,
        request_id,
        timestamp: chrono::Utc::now(),
        path,
        method,
    }
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorDetails {
    pub field_errors: HashMap<String, Vec<String>>,
    pub global_errors: Vec<String>,
}

/// Create a validation error with field-specific details
pub fn validation_error(field_errors: HashMap<String, Vec<String>>) -> ApiError {
    let details = ValidationErrorDetails {
        field_errors: field_errors.clone(),
        global_errors: Vec::new(),
    };

    let mut suggestions = vec![
        RecoverySuggestion {
            action: "fix_validation_errors".to_string(),
            description: "Correct the validation errors in the specified fields".to_string(),
            example: None,
            priority: SuggestionPriority::High,
        },
    ];

    // Add field-specific suggestions
    for (field, errors) in &field_errors {
        suggestions.push(RecoverySuggestion {
            action: format!("fix_field_{}", field),
            description: format!("Fix validation errors in field '{}': {}", field, errors.join(", ")),
            example: None,
            priority: SuggestionPriority::High,
        });
    }

    ApiErrorBuilder::new(ApiErrorCode::ValidationFailed)
        .message("Input validation failed")
        .details(serde_json::to_value(details).unwrap())
        .suggestions(suggestions)
        .build()
}

/// Create a resource not found error
pub fn resource_not_found_error(resource_type: &str, resource_id: &str) -> ApiError {
    ApiErrorBuilder::new(ApiErrorCode::ResourceNotFound)
        .message(format!("{} with ID '{}' not found", resource_type, resource_id))
        .details(serde_json::json!({
            "resource_type": resource_type,
            "resource_id": resource_id
        }))
        .build()
}

/// Create a rate limit exceeded error
pub fn rate_limit_error(limit: u32, window_seconds: u32, retry_after: u32) -> ApiError {
    ApiErrorBuilder::new(ApiErrorCode::RateLimitExceeded)
        .message(format!("Rate limit of {} requests per {} seconds exceeded", limit, window_seconds))
        .details(serde_json::json!({
            "limit": limit,
            "window_seconds": window_seconds,
            "retry_after_seconds": retry_after
        }))
        .suggestion(RecoverySuggestion {
            action: "wait_and_retry".to_string(),
            description: format!("Wait {} seconds before retrying", retry_after),
            example: Some(format!("Retry after {} seconds", retry_after)),
            priority: SuggestionPriority::High,
        })
        .build()
}

/// Implementation of IntoResponse for ApiErrorResponse
impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        let status_code = ApiErrorCode::from_str(&self.error.code)
            .map(|code| code.status_code())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        // Log the error
        match status_code {
            StatusCode::INTERNAL_SERVER_ERROR => {
                error!(
                    error_code = %self.error.code,
                    message = %self.error.message,
                    request_id = ?self.request_id,
                    correlation_id = ?self.error.correlation_id,
                    "API error occurred"
                );
            }
            _ => {
                warn!(
                    error_code = %self.error.code,
                    message = %self.error.message,
                    request_id = ?self.request_id,
                    "API client error"
                );
            }
        }

        (status_code, Json(self)).into_response()
    }
}

impl ApiErrorCode {
    /// Parse error code from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "UNAUTHORIZED" => Ok(ApiErrorCode::Unauthorized),
            "FORBIDDEN" => Ok(ApiErrorCode::Forbidden),
            "TOKEN_EXPIRED" => Ok(ApiErrorCode::TokenExpired),
            "TOKEN_INVALID" => Ok(ApiErrorCode::TokenInvalid),
            "MFA_REQUIRED" => Ok(ApiErrorCode::MfaRequired),
            
            "VALIDATION_FAILED" => Ok(ApiErrorCode::ValidationFailed),
            "INVALID_INPUT" => Ok(ApiErrorCode::InvalidInput),
            "MISSING_REQUIRED_FIELD" => Ok(ApiErrorCode::MissingRequiredField),
            "INVALID_FORMAT" => Ok(ApiErrorCode::InvalidFormat),
            "VALUE_OUT_OF_RANGE" => Ok(ApiErrorCode::ValueOutOfRange),
            
            "RESOURCE_NOT_FOUND" => Ok(ApiErrorCode::ResourceNotFound),
            "RESOURCE_ALREADY_EXISTS" => Ok(ApiErrorCode::ResourceAlreadyExists),
            "RESOURCE_CONFLICT" => Ok(ApiErrorCode::ResourceConflict),
            "RESOURCE_LOCKED" => Ok(ApiErrorCode::ResourceLocked),
            
            "RATE_LIMIT_EXCEEDED" => Ok(ApiErrorCode::RateLimitExceeded),
            "QUOTA_EXCEEDED" => Ok(ApiErrorCode::QuotaExceeded),
            "TOO_MANY_REQUESTS" => Ok(ApiErrorCode::TooManyRequests),
            
            "INTERNAL_SERVER_ERROR" => Ok(ApiErrorCode::InternalServerError),
            "SERVICE_UNAVAILABLE" => Ok(ApiErrorCode::ServiceUnavailable),
            "DATABASE_ERROR" => Ok(ApiErrorCode::DatabaseError),
            "EXTERNAL_SERVICE_ERROR" => Ok(ApiErrorCode::ExternalServiceError),
            "TIMEOUT_ERROR" => Ok(ApiErrorCode::TimeoutError),
            
            "UNSUPPORTED_API_VERSION" => Ok(ApiErrorCode::UnsupportedApiVersion),
            "DEPRECATED_ENDPOINT" => Ok(ApiErrorCode::DeprecatedEndpoint),
            
            "PIPELINE_NOT_FOUND" => Ok(ApiErrorCode::PipelineNotFound),
            "PIPELINE_EXECUTION_FAILED" => Ok(ApiErrorCode::PipelineExecutionFailed),
            "INVALID_PIPELINE_CONFIG" => Ok(ApiErrorCode::InvalidPipelineConfig),
            "EXECUTION_NOT_FOUND" => Ok(ApiErrorCode::ExecutionNotFound),
            "EXECUTION_ALREADY_CANCELLED" => Ok(ApiErrorCode::ExecutionAlreadyCancelled),
            
            "FILE_UPLOAD_FAILED" => Ok(ApiErrorCode::FileUploadFailed),
            "FILE_NOT_FOUND" => Ok(ApiErrorCode::FileNotFound),
            "FILE_SIZE_TOO_LARGE" => Ok(ApiErrorCode::FileSizeTooLarge),
            "UNSUPPORTED_FILE_TYPE" => Ok(ApiErrorCode::UnsupportedFileType),
            
            _ => Err(format!("Unknown error code: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_status_mapping() {
        assert_eq!(ApiErrorCode::Unauthorized.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(ApiErrorCode::ValidationFailed.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(ApiErrorCode::ResourceNotFound.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(ApiErrorCode::InternalServerError.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_builder() {
        let error = ApiErrorBuilder::new(ApiErrorCode::ValidationFailed)
            .message("Custom validation message")
            .details(serde_json::json!({"field": "value"}))
            .build();

        assert_eq!(error.code, "VALIDATION_FAILED");
        assert_eq!(error.message, "Custom validation message");
        assert!(error.details.is_some());
    }

    #[test]
    fn test_validation_error_creation() {
        let mut field_errors = HashMap::new();
        field_errors.insert("email".to_string(), vec!["Invalid email format".to_string()]);
        field_errors.insert("age".to_string(), vec!["Must be between 18 and 120".to_string()]);

        let error = validation_error(field_errors);
        assert_eq!(error.code, "VALIDATION_FAILED");
        assert!(error.details.is_some());
        assert!(!error.recovery_suggestions.is_empty());
    }

    #[test]
    fn test_resource_not_found_error() {
        let error = resource_not_found_error("Pipeline", "123e4567-e89b-12d3-a456-426614174000");
        assert_eq!(error.code, "RESOURCE_NOT_FOUND");
        assert!(error.message.contains("Pipeline"));
        assert!(error.message.contains("123e4567-e89b-12d3-a456-426614174000"));
    }

    #[test]
    fn test_rate_limit_error() {
        let error = rate_limit_error(100, 3600, 300);
        assert_eq!(error.code, "RATE_LIMIT_EXCEEDED");
        assert!(error.message.contains("100 requests per 3600 seconds"));
        assert!(!error.recovery_suggestions.is_empty());
    }

    #[test]
    fn test_error_code_parsing() {
        assert_eq!(ApiErrorCode::from_str("UNAUTHORIZED").unwrap(), ApiErrorCode::Unauthorized);
        assert_eq!(ApiErrorCode::from_str("VALIDATION_FAILED").unwrap(), ApiErrorCode::ValidationFailed);
        assert!(ApiErrorCode::from_str("INVALID_CODE").is_err());
    }
}