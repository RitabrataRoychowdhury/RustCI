use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    AuthError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("External service error: {0}")]
    ExternalServiceError(String),
    
    #[error("Internal server error: {0}")]
    InternalServerError(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Forbidden: {0}")]
    Forbidden(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("File upload error: {0}")]
    FileUploadError(String),
    
    #[error("File size exceeds limit: {actual} bytes (max: {limit} bytes)")]
    FileSizeExceeded { actual: usize, limit: usize },
    
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),
}

// Add From implementations for common error types
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::InternalServerError(format!("IO error: {}", err))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::InternalServerError(format!("JSON error: {}", err))
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(err: serde_yaml::Error) -> Self {
        AppError::ValidationError(format!("YAML error: {}", err))
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::AuthError(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ExternalServiceError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::FileUploadError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::FileSizeExceeded { actual, limit } => (
                StatusCode::PAYLOAD_TOO_LARGE, 
                format!("File size {} bytes exceeds limit of {} bytes", actual, limit)
            ),
            AppError::UnsupportedFileType(msg) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, msg),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;