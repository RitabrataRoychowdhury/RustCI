//! Error types for Valkyrie Protocol

use thiserror::Error;

/// Result type alias for Valkyrie operations
pub type Result<T> = std::result::Result<T, ValkyrieError>;

/// Main error type for Valkyrie Protocol
#[derive(Error, Debug)]
pub enum ValkyrieError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Transport error: {0}")]
    Transport(String),
    
    #[error("Security error: {0}")]
    Security(String),
    
    #[error("Timeout error: operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl ValkyrieError {
    /// Create a new connection error
    pub fn connection<S: Into<String>>(msg: S) -> Self {
        Self::Connection(msg.into())
    }
    
    /// Create a new protocol error
    pub fn protocol<S: Into<String>>(msg: S) -> Self {
        Self::Protocol(msg.into())
    }
    
    /// Create a new transport error
    pub fn transport<S: Into<String>>(msg: S) -> Self {
        Self::Transport(msg.into())
    }
    
    /// Create a new security error
    pub fn security<S: Into<String>>(msg: S) -> Self {
        Self::Security(msg.into())
    }
    
    /// Create a new timeout error
    pub fn timeout(timeout_ms: u64) -> Self {
        Self::Timeout { timeout_ms }
    }
    
    /// Create a new configuration error
    pub fn configuration<S: Into<String>>(msg: S) -> Self {
        Self::Configuration(msg.into())
    }
    
    /// Create a new invalid message error
    pub fn invalid_message<S: Into<String>>(msg: S) -> Self {
        Self::InvalidMessage(msg.into())
    }
    
    /// Create a new service unavailable error
    pub fn service_unavailable<S: Into<String>>(msg: S) -> Self {
        Self::ServiceUnavailable(msg.into())
    }
}