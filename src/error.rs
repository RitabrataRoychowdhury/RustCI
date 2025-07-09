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
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;