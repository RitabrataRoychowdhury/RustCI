use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use crate::storage::{HealthStatus, NodeInfo, StoreStats};

/// Valkyrie storage protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageMessage {
    /// Request messages
    Request(StorageRequest),
    /// Response messages
    Response(StorageResponse),
    /// Error messages
    Error(StorageError),
}

/// Storage request types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRequest {
    /// Get a value by key
    Get {
        key: String,
        request_id: Uuid,
    },
    /// Set a value with optional TTL
    Set {
        key: String,
        value: Vec<u8>,
        ttl: Option<Duration>,
        request_id: Uuid,
    },
    /// Try to consume tokens from a rate limiter
    TryConsume {
        key: String,
        amount: u32,
        request_id: Uuid,
    },
    /// Delete a key
    Delete {
        key: String,
        request_id: Uuid,
    },
    /// Check if a key exists
    Exists {
        key: String,
        request_id: Uuid,
    },
    /// Get TTL for a key
    Ttl {
        key: String,
        request_id: Uuid,
    },
    /// Increment a counter
    Increment {
        key: String,
        amount: i64,
        ttl: Option<Duration>,
        request_id: Uuid,
    },
    /// Health check
    HealthCheck {
        request_id: Uuid,
    },
    /// Get statistics
    GetStats {
        request_id: Uuid,
    },
    /// Owner redirect request
    OwnerRedirect {
        key: String,
        request_id: Uuid,
    },
}

/// Storage response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageResponse {
    /// Get response
    Get {
        value: Option<Vec<u8>>,
        request_id: Uuid,
    },
    /// Set response
    Set {
        success: bool,
        request_id: Uuid,
    },
    /// TryConsume response
    TryConsume {
        consumed: bool,
        request_id: Uuid,
    },
    /// Delete response
    Delete {
        deleted: bool,
        request_id: Uuid,
    },
    /// Exists response
    Exists {
        exists: bool,
        request_id: Uuid,
    },
    /// TTL response
    Ttl {
        ttl: Option<Duration>,
        request_id: Uuid,
    },
    /// Increment response
    Increment {
        new_value: i64,
        request_id: Uuid,
    },
    /// Health check response
    HealthCheck {
        status: HealthStatus,
        request_id: Uuid,
    },
    /// Statistics response
    GetStats {
        stats: StoreStats,
        request_id: Uuid,
    },
    /// Owner redirect response
    OwnerRedirect {
        owner_node: NodeInfo,
        request_id: Uuid,
    },
}

/// Storage error types for network protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageError {
    pub error_type: String,
    pub message: String,
    pub request_id: Uuid,
    pub retryable: bool,
}

impl From<crate::storage::StoreError> for StorageError {
    fn from(error: crate::storage::StoreError) -> Self {
        Self {
            error_type: match error {
                crate::storage::StoreError::ConnectionError(_) => "connection_error".to_string(),
                crate::storage::StoreError::Timeout(_) => "timeout".to_string(),
                crate::storage::StoreError::SerializationError(_) => "serialization_error".to_string(),
                crate::storage::StoreError::BackendUnavailable(_) => "backend_unavailable".to_string(),
                crate::storage::StoreError::ConsistencyError(_) => "consistency_error".to_string(),
                crate::storage::StoreError::AuthenticationError(_) => "authentication_error".to_string(),
                crate::storage::StoreError::AuthorizationError(_) => "authorization_error".to_string(),
                crate::storage::StoreError::KeyNotFound(_) => "key_not_found".to_string(),
                crate::storage::StoreError::ConfigurationError(_) => "configuration_error".to_string(),
                crate::storage::StoreError::NetworkError(_) => "network_error".to_string(),
                crate::storage::StoreError::InternalError(_) => "internal_error".to_string(),
            },
            message: error.to_string(),
            request_id: Uuid::new_v4(), // Will be overridden by caller
            retryable: error.is_retryable(),
        }
    }
}

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Request timeout
    pub request_timeout: Duration,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// Enable compression
    pub enable_compression: bool,
    /// Compression threshold (bytes)
    pub compression_threshold: usize,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024, // 1MB
            request_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(60),
            max_concurrent_requests: 1000,
            enable_compression: true,
            compression_threshold: 1024, // 1KB
        }
    }
}

/// Message serialization and deserialization
impl StorageMessage {
    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        bincode::serialize(self).map_err(Into::into)
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    /// Get request ID from message
    pub fn request_id(&self) -> Option<Uuid> {
        match self {
            StorageMessage::Request(req) => Some(req.request_id()),
            StorageMessage::Response(resp) => Some(resp.request_id()),
            StorageMessage::Error(err) => Some(err.request_id),
        }
    }
}

impl StorageRequest {
    /// Get request ID
    pub fn request_id(&self) -> Uuid {
        match self {
            StorageRequest::Get { request_id, .. } => *request_id,
            StorageRequest::Set { request_id, .. } => *request_id,
            StorageRequest::TryConsume { request_id, .. } => *request_id,
            StorageRequest::Delete { request_id, .. } => *request_id,
            StorageRequest::Exists { request_id, .. } => *request_id,
            StorageRequest::Ttl { request_id, .. } => *request_id,
            StorageRequest::Increment { request_id, .. } => *request_id,
            StorageRequest::HealthCheck { request_id } => *request_id,
            StorageRequest::GetStats { request_id } => *request_id,
            StorageRequest::OwnerRedirect { request_id, .. } => *request_id,
        }
    }

    /// Get the key being operated on (if applicable)
    pub fn key(&self) -> Option<&str> {
        match self {
            StorageRequest::Get { key, .. } => Some(key),
            StorageRequest::Set { key, .. } => Some(key),
            StorageRequest::TryConsume { key, .. } => Some(key),
            StorageRequest::Delete { key, .. } => Some(key),
            StorageRequest::Exists { key, .. } => Some(key),
            StorageRequest::Ttl { key, .. } => Some(key),
            StorageRequest::Increment { key, .. } => Some(key),
            StorageRequest::OwnerRedirect { key, .. } => Some(key),
            StorageRequest::HealthCheck { .. } => None,
            StorageRequest::GetStats { .. } => None,
        }
    }
}

impl StorageResponse {
    /// Get request ID
    pub fn request_id(&self) -> Uuid {
        match self {
            StorageResponse::Get { request_id, .. } => *request_id,
            StorageResponse::Set { request_id, .. } => *request_id,
            StorageResponse::TryConsume { request_id, .. } => *request_id,
            StorageResponse::Delete { request_id, .. } => *request_id,
            StorageResponse::Exists { request_id, .. } => *request_id,
            StorageResponse::Ttl { request_id, .. } => *request_id,
            StorageResponse::Increment { request_id, .. } => *request_id,
            StorageResponse::HealthCheck { request_id, .. } => *request_id,
            StorageResponse::GetStats { request_id, .. } => *request_id,
            StorageResponse::OwnerRedirect { request_id, .. } => *request_id,
        }
    }
}

/// Routing context for consistent hashing
#[derive(Debug, Clone)]
pub struct RoutingContext {
    pub key: String,
    pub operation: String,
    pub client_id: Option<String>,
    pub request_id: Uuid,
}

impl RoutingContext {
    pub fn new(key: String, operation: String) -> Self {
        Self {
            key,
            operation,
            client_id: None,
            request_id: Uuid::new_v4(),
        }
    }

    pub fn with_client_id(mut self, client_id: String) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }
}