//! Error types for the automated testing infrastructure

use thiserror::Error;

/// Main error type for deployment testing operations
#[derive(Debug, Error)]
pub enum DeploymentTestError {
    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),

    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Pipeline upload failed: {message}")]
    PipelineUpload { message: String },

    #[error("Pipeline execution failed: {message}")]
    PipelineExecution { message: String },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Timeout exceeded after {seconds}s: {operation}")]
    Timeout { seconds: u64, operation: String },

    #[error("Retry limit exceeded: {attempts} attempts failed for {operation}")]
    RetryLimitExceeded { attempts: u32, operation: String },

    #[error("Environment setup failed: {message}")]
    EnvironmentSetup { message: String },

    #[error("HTTP status error: {status} - {message}")]
    HttpStatus { status: u16, message: String },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for deployment testing operations
pub type DeploymentTestResult<T> = Result<T, DeploymentTestError>;

impl DeploymentTestError {
    /// Create a new pipeline upload error
    pub fn pipeline_upload<S: Into<String>>(message: S) -> Self {
        Self::PipelineUpload {
            message: message.into(),
        }
    }

    /// Create a new pipeline execution error
    pub fn pipeline_execution<S: Into<String>>(message: S) -> Self {
        Self::PipelineExecution {
            message: message.into(),
        }
    }

    /// Create a new validation error
    pub fn validation_failed<S: Into<String>>(message: S) -> Self {
        Self::ValidationFailed {
            message: message.into(),
        }
    }

    /// Create a new timeout error
    pub fn timeout<S: Into<String>>(seconds: u64, operation: S) -> Self {
        Self::Timeout {
            seconds,
            operation: operation.into(),
        }
    }

    /// Create a new retry limit exceeded error
    pub fn retry_limit_exceeded<S: Into<String>>(attempts: u32, operation: S) -> Self {
        Self::RetryLimitExceeded {
            attempts,
            operation: operation.into(),
        }
    }

    /// Create a new environment setup error
    pub fn environment_setup<S: Into<String>>(message: S) -> Self {
        Self::EnvironmentSetup {
            message: message.into(),
        }
    }

    /// Create a new HTTP status error
    pub fn http_status<S: Into<String>>(status: u16, message: S) -> Self {
        Self::HttpStatus {
            status,
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }
}
