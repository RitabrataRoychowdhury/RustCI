use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during storage operations
#[derive(Debug, Error, Clone)]
pub enum StoreError {
    #[error("Connection failed: {0}")]
    ConnectionError(String),
    
    #[error("Operation timeout after {0:?}")]
    Timeout(Duration),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Backend unavailable: {0}")]
    BackendUnavailable(String),
    
    #[error("Consistency error: {0}")]
    ConsistencyError(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    #[error("Authorization failed: {0}")]
    AuthorizationError(String),
    
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Invalid configuration: {0}")]
    ConfigurationError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl StoreError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StoreError::ConnectionError(_)
                | StoreError::Timeout(_)
                | StoreError::BackendUnavailable(_)
                | StoreError::NetworkError(_)
        )
    }
    
    /// Check if the error indicates a temporary failure
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            StoreError::Timeout(_)
                | StoreError::BackendUnavailable(_)
                | StoreError::NetworkError(_)
        )
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            StoreError::InternalError(_) | StoreError::ConsistencyError(_) => ErrorSeverity::Critical,
            StoreError::BackendUnavailable(_) | StoreError::ConnectionError(_) => ErrorSeverity::High,
            StoreError::Timeout(_) | StoreError::NetworkError(_) => ErrorSeverity::Medium,
            StoreError::AuthenticationError(_) | StoreError::AuthorizationError(_) => ErrorSeverity::High,
            StoreError::KeyNotFound(_) | StoreError::SerializationError(_) => ErrorSeverity::Low,
            StoreError::ConfigurationError(_) => ErrorSeverity::High,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Result type for storage operations
pub type StoreResult<T> = Result<T, StoreError>;