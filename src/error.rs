use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use utoipa::ToSchema;

pub mod utils;

#[derive(serde::Serialize, ToSchema)]
pub struct ApiError {
    pub error: String,
    pub status: u16,
    pub timestamp: String,
}

#[derive(thiserror::Error, Debug, Clone)]
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

    // New error types for workspace and Dockerfile features
    #[error("GitHub API error: {0}")]
    GitHubApiError(String),

    #[error("Docker validation error: {0}")]
    DockerValidationError(String),

    #[error("Project type detection failed: {0}")]
    ProjectTypeDetectionFailed(String),

    #[error("Dockerfile generation failed: {0}")]
    DockerfileGenerationFailed(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Workspace not found")]
    WorkspaceNotFound,

    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Authorization error: {0}")]
    AuthorizationError(String),

    // Connector-specific error types
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Kubernetes error: {0}")]
    KubernetesError(String),

    #[error("Connector configuration error: {0}")]
    ConnectorConfigError(String),

    #[error("Feature not implemented: {0}")]
    Unimplemented(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    // Core architecture error types
    #[error("Command execution failed: {command}")]
    CommandExecutionError { command: String },

    #[error("Query execution failed: {query}")]
    QueryExecutionError { query: String },

    #[error("SAGA compensation failed: {saga_id} - {step}")]
    SagaCompensationError { saga_id: uuid::Uuid, step: String },

    #[error("Repository operation failed: {operation} - {entity}")]
    RepositoryError { operation: String, entity: String },

    #[error("Service dependency not found: {service}")]
    DependencyNotFound { service: String },

    #[error("Event handling failed: {event_type} - {handler}")]
    EventHandlingError { event_type: String, handler: String },

    #[error("Configuration error: {key} - {message}")]
    ConfigurationError { key: String, message: String },

    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u32, window: String },

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceededSimple(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Concurrency error: {0}")]
    ConcurrencyError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Operation timed out: {0}")]
    TimeoutError(String),

    #[error("Invalid pipeline type: {0}")]
    InvalidPipelineType(String),
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
                format!(
                    "File size {} bytes exceeds limit of {} bytes",
                    actual, limit
                ),
            ),
            AppError::UnsupportedFileType(msg) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, msg),
            // New error types
            AppError::GitHubApiError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::DockerValidationError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::ProjectTypeDetectionFailed(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::DockerfileGenerationFailed(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::EncryptionError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal encryption error".to_string(),
            ),
            AppError::WorkspaceNotFound => {
                (StatusCode::NOT_FOUND, "Workspace not found".to_string())
            }
            AppError::RepositoryNotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::AuthenticationError(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::AuthorizationError(msg) => (StatusCode::FORBIDDEN, msg),
            // Connector-specific error types
            AppError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::KubernetesError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::ConnectorConfigError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::Unimplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
            AppError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
            // Core architecture error types
            AppError::CommandExecutionError { command } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Command execution failed: {}", command),
            ),
            AppError::QueryExecutionError { query } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Query execution failed: {}", query),
            ),
            AppError::SagaCompensationError { saga_id, step } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("SAGA compensation failed: {} - {}", saga_id, step),
            ),
            AppError::RepositoryError { operation, entity } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Repository operation failed: {} - {}", operation, entity),
            ),
            AppError::DependencyNotFound { service } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Service dependency not found: {}", service),
            ),
            AppError::EventHandlingError {
                event_type,
                handler,
            } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Event handling failed: {} - {}", event_type, handler),
            ),
            AppError::ConfigurationError { key, message } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Configuration error: {} - {}", key, message),
            ),
            AppError::RateLimitExceeded { limit, window } => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("Rate limit exceeded: {} requests per {}", limit, window),
            ),
            AppError::RateLimitExceededSimple(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
            AppError::ConfigError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ResourceExhausted(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
            AppError::ConcurrencyError(msg) => (StatusCode::CONFLICT, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::TimeoutError(msg) => (StatusCode::REQUEST_TIMEOUT, msg),
            AppError::InvalidPipelineType(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
