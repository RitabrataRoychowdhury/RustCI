//! HTTP/HTTPS Bridge for Valkyrie Protocol
//! 
//! This module provides seamless integration between HTTP/HTTPS and the Valkyrie Protocol,
//! enabling existing systems to communicate with Valkyrie-enabled services without modification.

pub mod http_gateway;
pub mod protocol_negotiation;
pub mod rest_translator;
pub mod websocket_upgrade;
pub mod compatibility_layer;
pub mod performance;
pub mod benchmarks;

pub use http_gateway::*;
pub use protocol_negotiation::*;
pub use rest_translator::*;
pub use websocket_upgrade::*;
pub use compatibility_layer::*;
pub use performance::*;
pub use benchmarks::*;

use crate::core::networking::valkyrie::message::ValkyrieMessage;
use crate::error::ValkyrieError;
use axum::{
    http::{HeaderMap, Method, StatusCode, Uri},
    response::Response,
    body::Body,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Bridge configuration for HTTP/HTTPS integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Enable HTTP to Valkyrie conversion
    pub http_bridge_enabled: bool,
    /// Enable HTTPS to Valkyrie conversion
    pub https_bridge_enabled: bool,
    /// Enable WebSocket upgrade support
    pub websocket_upgrade_enabled: bool,
    /// Enable automatic protocol negotiation
    pub auto_negotiation_enabled: bool,
    /// HTTP listen address
    pub http_listen_addr: String,
    /// HTTPS listen address
    pub https_listen_addr: String,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS private key path
    pub tls_key_path: Option<String>,
    /// Maximum request body size
    pub max_request_size: usize,
    /// Request timeout
    pub request_timeout_ms: u64,
    /// Enable CORS support
    pub cors_enabled: bool,
    /// Allowed origins for CORS
    pub cors_origins: Vec<String>,
    /// Enable performance optimizations for sub-millisecond responses
    pub enable_performance_optimizations: Option<bool>,
    /// Enable zero-copy operations
    pub enable_zero_copy: Option<bool>,
    /// Enable SIMD optimizations
    pub enable_simd: Option<bool>,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            http_bridge_enabled: true,
            https_bridge_enabled: true,
            websocket_upgrade_enabled: true,
            auto_negotiation_enabled: true,
            http_listen_addr: "0.0.0.0:8080".to_string(),
            https_listen_addr: "0.0.0.0:8443".to_string(),
            tls_cert_path: None,
            tls_key_path: None,
            max_request_size: 16 * 1024 * 1024, // 16MB
            request_timeout_ms: 30000, // 30 seconds
            cors_enabled: true,
            cors_origins: vec!["*".to_string()],
            enable_performance_optimizations: Some(true),
            enable_zero_copy: Some(true),
            enable_simd: Some(true),
        }
    }
}

/// HTTP request context for bridge operations
#[derive(Debug, Clone)]
pub struct HttpRequestContext {
    /// Request ID for tracing
    pub request_id: Uuid,
    /// HTTP method
    pub method: Method,
    /// Request URI
    pub uri: Uri,
    /// Request headers
    pub headers: HeaderMap,
    /// Client IP address
    pub client_ip: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Authentication token
    pub auth_token: Option<String>,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// HTTP response context for bridge operations
#[derive(Debug, Clone)]
pub struct HttpResponseContext {
    /// Response status code
    pub status_code: StatusCode,
    /// Response headers
    pub headers: HeaderMap,
    /// Response body size
    pub body_size: usize,
    /// Processing duration
    pub duration_ms: u64,
}

/// Bridge operation result
pub type BridgeResult<T> = Result<T, BridgeError>;

/// Bridge-specific errors
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Protocol negotiation failed: {reason}")]
    ProtocolNegotiationFailed { reason: String },
    
    #[error("HTTP to Valkyrie conversion failed: {details}")]
    HttpConversionFailed { details: String },
    
    #[error("Valkyrie to HTTP conversion failed: {details}")]
    ValkyrieConversionFailed { details: String },
    
    #[error("WebSocket upgrade failed: {reason}")]
    WebSocketUpgradeFailed { reason: String },
    
    #[error("Authentication failed: {method}")]
    AuthenticationFailed { method: String },
    
    #[error("Request too large: {size} bytes exceeds limit of {limit}")]
    RequestTooLarge { size: usize, limit: usize },
    
    #[error("Request timeout after {timeout_ms}ms")]
    RequestTimeout { timeout_ms: u64 },
    
    #[error("Unsupported content type: {content_type}")]
    UnsupportedContentType { content_type: String },
    
    #[error("Invalid request format: {details}")]
    InvalidRequestFormat { details: String },
    
    #[error("Bridge configuration error: {parameter}: {message}")]
    ConfigurationError { parameter: String, message: String },
    
    #[error("Internal bridge error: {message}")]
    InternalError { message: String },
}

impl From<BridgeError> for ValkyrieError {
    fn from(err: BridgeError) -> Self {
        ValkyrieError::InternalError {
            component: "bridge".to_string(),
            message: err.to_string(),
        }
    }
}