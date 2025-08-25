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

    // Missing variants that are used in the codebase
    #[error("Operation timeout: {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

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

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Security error: {0}")]
    SecurityError(String),

    #[error("Stream error: {stream_id}: {error_type}")]
    StreamError { stream_id: uuid::Uuid, error_type: String },

    #[error("Flow control violation: {stream_id}: {details}")]
    FlowControlViolation { stream_id: uuid::Uuid, details: String },

    #[error("Internal error: {component}: {message}")]
    InternalError { component: String, message: String },

    // Zero-trust security errors
    #[error("Security context not found: {0}")]
    SecurityContextNotFound(String),

    #[error("Security context expired: {0}")]
    SecurityContextExpired(String),

    #[error("Congestion control error: {0}")]
    CongestionControl(String),

    // Valkyrie routing errors
    #[error("Invalid capabilities: {0}")]
    InvalidCapabilities(String),

    #[error("Consensus timeout: {0}")]
    ConsensusTimeout(String),

    #[error("Consensus rejected: {0}")]
    ConsensusRejected(String),

    #[error("Health check error: {0}")]
    HealthCheckError(String),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("No healthy endpoints: {0}")]
    NoHealthyEndpoints(String),

    #[error("Load balancing error: {0}")]
    LoadBalancingError(String),

    // Additional Valkyrie errors
    #[error("Invalid QoS class: {0}")]
    InvalidQoSClass(String),

    #[error("Unsupported adapter: {0}")]
    UnsupportedAdapter(String),

    #[error("No suitable adapter: {0}")]
    NoSuitableAdapter(String),

    #[error("Adapter initialization failed: {0}")]
    AdapterInitializationFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Incompatible version: {0}")]
    IncompatibleVersion(String),

    #[error("Metrics not available: {0}")]
    MetricsNotAvailable(String),

    #[error("Message conversion failed: {0}")]
    MessageConversionFailed(String),

    #[error("Registry full: {0}")]
    RegistryFull(String),

    #[error("Invalid service name: {0}")]
    InvalidServiceName(String),

    #[error("Invalid endpoints: {0}")]
    InvalidEndpoints(String),

    #[error("Invalid condition: {0}")]
    InvalidCondition(String),

    #[error("Queue full: {0}")]
    QueueFull(String),
}

// Valkyrie-specific error type
pub type ValkyrieError = AppError;

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

impl From<crate::valkyrie::config::ConfigValidationError> for AppError {
    fn from(err: crate::valkyrie::config::ConfigValidationError) -> Self {
        AppError::ConfigurationError(format!("Configuration validation error: {}", err))
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            // Authentication & Authorization
            AppError::AuthError(msg) | AppError::AuthenticationError(msg) => {
                (StatusCode::UNAUTHORIZED, msg)
            }
            AppError::AuthorizationError(msg) | AppError::PermissionDenied(msg) => {
                (StatusCode::FORBIDDEN, msg)
            }
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),

            // Validation & Input
            AppError::ValidationError(msg) | AppError::BadRequest(msg) | AppError::InvalidInput(msg) => {
                (StatusCode::BAD_REQUEST, msg)
            }

            // Resource & Lookup
            AppError::NotFound(msg) | AppError::RepositoryNotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::WorkspaceNotFound => (StatusCode::NOT_FOUND, "Workspace not found".to_string()),

            // File handling
            AppError::FileUploadError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::FileSizeExceeded { actual, limit } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "File size {} bytes exceeds limit of {} bytes",
                    actual, limit
                ),
            ),
            AppError::UnsupportedFileType(msg) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, msg),

            // External & IO
            AppError::IoError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ExternalServiceError(msg) | AppError::GitHubApiError(msg) => {
                (StatusCode::BAD_GATEWAY, msg)
            }
            AppError::KubernetesError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),

            // Timeouts & Concurrency
            AppError::TimeoutError(msg) | AppError::Timeout(msg) => {
                (StatusCode::REQUEST_TIMEOUT, msg)
            }
            AppError::ConcurrencyError(msg) => (StatusCode::CONFLICT, msg),
            AppError::RateLimitExceeded { limit, window } => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("Rate limit exceeded: {} requests per {}", limit, window),
            ),
            AppError::RateLimitExceededSimple(msg)
            | AppError::ResourceExhausted(msg)
            | AppError::CongestionControl(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),

            // Security & Encryption
            AppError::EncryptionError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal encryption error".to_string(),
            ),
            AppError::SecurityError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::SecurityContextNotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::SecurityContextExpired(msg) => (StatusCode::UNAUTHORIZED, msg),

            // Internal Failures
            AppError::InternalServerError(msg) | AppError::InternalError { message: msg, .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            AppError::Unimplemented(msg) | AppError::NotImplemented(msg) => {
                (StatusCode::NOT_IMPLEMENTED, msg)
            }
            AppError::ConfigError(msg) | AppError::ConfigurationError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }

            // Serialization & Compression
            AppError::SerializationError(msg) | AppError::CompressionError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }

            // Connector-specific
            AppError::DockerValidationError(msg)
            | AppError::ProjectTypeDetectionFailed(msg)
            | AppError::DockerfileGenerationFailed(msg)
            | AppError::ConnectorConfigError(msg)
            | AppError::InvalidPipelineType(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),

            // Complex/Internal cases (queries, repos, sagas, events, streams)
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
            AppError::EventHandlingError { event_type, handler } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Event handling failed: {} - {}", event_type, handler),
            ),
            AppError::StreamError { stream_id, error_type } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Stream error: {}: {}", stream_id, error_type),
            ),
            AppError::FlowControlViolation { stream_id, details } => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("Flow control violation: {}: {}", stream_id, details),
            ),

            // Default catch-all for any future or unhandled AppError variants
            other => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled error: {}", other),
            ),
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
