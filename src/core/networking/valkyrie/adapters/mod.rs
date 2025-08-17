//! Universal Adapter System for Valkyrie Protocol
//! 
//! This module implements the Universal Adapter Factory using enterprise design patterns
//! (Factory, Strategy, Adapter) to provide plug-and-play integration with any infrastructure.

pub mod factory;
pub mod strategy;
pub mod http;
pub mod docker;
pub mod kubernetes;
pub mod redis;
pub mod quic_tcp;

use std::collections::HashMap;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

// Re-export factory components
pub use factory::{
    UniversalAdapterFactory, AdapterBuilder, AdapterRequirements, 
    AdapterInfo
};

// Re-export strategy components  
pub use strategy::{
    AdapterSelectionStrategy, PerformanceBasedStrategy, ReliabilityBasedStrategy,
    LatencyOptimizedStrategy, ThroughputOptimizedStrategy
};

// Re-export specific adapter builders
pub use http::HttpAdapterBuilder;
pub use docker::DockerAdapterBuilder;
pub use kubernetes::KubernetesAdapterBuilder;
pub use redis::RedisAdapterBuilder;
pub use quic_tcp::QuicTcpAdapterBuilder;

/// Universal adapter trait for all infrastructure integrations
#[async_trait]
pub trait UniversalAdapter: Send + Sync {
    /// Send message with QoS parameters
    async fn send(&self, message: AdapterMessage, qos: QoSParams) -> Result<SendResult>;
    
    /// Receive message with timeout
    async fn receive(&self, timeout: Option<Duration>) -> Result<Option<AdapterMessage>>;
    
    /// Get adapter capabilities
    fn capabilities(&self) -> &AdapterCapabilities;
    
    /// Perform health check
    async fn health_check(&self) -> HealthStatus;
    
    /// Get adapter metrics
    async fn metrics(&self) -> AdapterMetrics;
    
    /// Initialize adapter
    async fn initialize(&mut self) -> Result<()>;
    
    /// Shutdown adapter gracefully
    async fn shutdown(&mut self) -> Result<()>;
    
    /// Handle configuration updates
    async fn update_config(&mut self, config: &AdapterConfig) -> Result<()>;
    
    /// Get adapter type identifier
    fn adapter_type(&self) -> AdapterType;
    
    /// Get adapter ID
    fn adapter_id(&self) -> &AdapterId;
}

/// Adapter message format for universal communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterMessage {
    /// Message ID for tracking
    pub id: Uuid,
    /// Message type
    pub message_type: AdapterMessageType,
    /// Message payload
    pub payload: Vec<u8>,
    /// Message metadata
    pub metadata: HashMap<String, String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Priority level
    pub priority: MessagePriority,
    /// Routing information
    pub routing: RoutingInfo,
}

/// QoS parameters for message delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QoSParams {
    /// Maximum latency tolerance
    pub max_latency: Duration,
    /// Reliability requirement (0.0 - 1.0)
    pub reliability: f64,
    /// Bandwidth allocation
    pub bandwidth: Option<u64>,
    /// Priority level
    pub priority: MessagePriority,
    /// Retry policy
    pub retry_policy: RetryPolicy,
}

/// Send operation result
#[derive(Debug, Clone)]
pub struct SendResult {
    /// Operation success
    pub success: bool,
    /// Actual latency
    pub latency: Duration,
    /// Bytes sent
    pub bytes_sent: usize,
    /// Error message if failed
    pub error: Option<String>,
    /// Response data if applicable
    pub response: Option<Vec<u8>>,
}

/// Adapter capabilities for automatic selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    /// Adapter type
    pub adapter_type: AdapterType,
    /// Maximum concurrent connections
    pub max_connections: Option<usize>,
    /// Supported message sizes
    pub max_message_size: usize,
    /// Latency characteristics
    pub latency_profile: LatencyProfile,
    /// Throughput characteristics
    pub throughput_profile: ThroughputProfile,
    /// Reliability features
    pub reliability_features: ReliabilityFeatures,
    /// Security features
    pub security_features: SecurityFeatures,
    /// Transport-specific features
    pub transport_features: TransportFeatures,
    /// Supported operations
    pub supported_operations: Vec<AdapterOperation>,
}

/// Health status of an adapter
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unknown,
}

/// Adapter performance metrics
#[derive(Debug, Clone)]
pub struct AdapterMetrics {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Average latency
    pub avg_latency: Duration,
    /// P95 latency
    pub p95_latency: Duration,
    /// P99 latency
    pub p99_latency: Duration,
    /// Error rate (0.0 - 1.0)
    pub error_rate: f64,
    /// Throughput (messages per second)
    pub throughput: f64,
    /// Connection count
    pub active_connections: usize,
    /// Last health check time
    pub last_health_check: chrono::DateTime<chrono::Utc>,
}

/// Adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Adapter type
    pub adapter_type: AdapterType,
    /// Connection settings
    pub connection: ConnectionConfig,
    /// Security settings
    pub security: SecurityConfig,
    /// Performance settings
    pub performance: PerformanceConfig,
    /// Custom settings
    pub custom: HashMap<String, serde_json::Value>,
}

/// Adapter types supported by the factory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdapterType {
    Http,
    Docker,
    Kubernetes,
    Redis,
    QuicTcp,
    UnixSocket,
    Grpc,
    Mqtt,
    Kafka,
    Custom(u16),
}

/// Unique adapter identifier
pub type AdapterId = Uuid;

/// Message types for adapter communication
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AdapterMessageType {
    Request,
    Response,
    Event,
    Command,
    Data,
    Heartbeat,
    Custom(String),
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MessagePriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

/// Routing information for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingInfo {
    /// Source adapter
    pub source: Option<AdapterId>,
    /// Destination adapter(s)
    pub destination: DestinationType,
    /// Routing hints
    pub hints: HashMap<String, String>,
}

/// Destination types for message routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DestinationType {
    Direct(AdapterId),
    Broadcast,
    LoadBalanced(Vec<AdapterId>),
    Service(String),
}

/// Retry policy for failed operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Initial delay
    pub initial_delay: Duration,
    /// Maximum delay
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Jitter enabled
    pub jitter: bool,
}

/// Latency characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyProfile {
    /// Average latency
    pub avg_latency: Duration,
    /// P95 latency
    pub p95_latency: Duration,
    /// P99 latency
    pub p99_latency: Duration,
    /// Maximum latency
    pub max_latency: Duration,
}

/// Throughput characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputProfile {
    /// Maximum messages per second
    pub max_messages_per_sec: u64,
    /// Maximum bytes per second
    pub max_bytes_per_sec: u64,
    /// Burst capacity
    pub burst_capacity: u64,
}

/// Reliability features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityFeatures {
    /// Supports retry
    pub supports_retry: bool,
    /// Supports circuit breaker
    pub supports_circuit_breaker: bool,
    /// Supports health check
    pub supports_health_check: bool,
    /// Supports failover
    pub supports_failover: bool,
    /// Supports load balancing
    pub supports_load_balancing: bool,
}

/// Security features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFeatures {
    /// Supports TLS
    pub supports_tls: bool,
    /// Supports mutual TLS
    pub supports_mtls: bool,
    /// Supports authentication
    pub supports_auth: bool,
    /// Supports authorization
    pub supports_authz: bool,
    /// Supports encryption
    pub supports_encryption: bool,
}

/// Transport-specific features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportFeatures {
    /// Supports streaming
    pub supports_streaming: bool,
    /// Supports multiplexing
    pub supports_multiplexing: bool,
    /// Supports compression
    pub supports_compression: bool,
    /// Supports keep-alive
    pub supports_keepalive: bool,
    /// Connection pooling
    pub supports_pooling: bool,
}

/// Supported adapter operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdapterOperation {
    Send,
    Receive,
    Stream,
    Batch,
    Transaction,
    Custom(String),
}

/// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Connection timeout
    pub timeout: Duration,
    /// Keep-alive settings
    pub keepalive: Option<Duration>,
    /// Connection pool size
    pub pool_size: Option<usize>,
    /// Maximum connections
    pub max_connections: Option<usize>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable TLS
    pub tls_enabled: bool,
    /// TLS certificate path
    pub cert_path: Option<String>,
    /// TLS key path
    pub key_path: Option<String>,
    /// CA certificate path
    pub ca_path: Option<String>,
    /// Authentication token
    pub auth_token: Option<String>,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Buffer size
    pub buffer_size: usize,
    /// Batch size
    pub batch_size: Option<usize>,
    /// Compression enabled
    pub compression: bool,
    /// Keep-alive enabled
    pub keepalive: bool,
}

impl Default for QoSParams {
    fn default() -> Self {
        Self {
            max_latency: Duration::from_millis(100),
            reliability: 0.99,
            bandwidth: None,
            priority: MessagePriority::Normal,
            retry_policy: RetryPolicy::default(),
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            keepalive: Some(Duration::from_secs(60)),
            pool_size: Some(10),
            max_connections: Some(100),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            tls_enabled: false,
            cert_path: None,
            key_path: None,
            ca_path: None,
            auth_token: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            buffer_size: 8192,
            batch_size: Some(100),
            compression: false,
            keepalive: true,
        }
    }
}

impl std::fmt::Display for AdapterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterType::Http => write!(f, "http"),
            AdapterType::Docker => write!(f, "docker"),
            AdapterType::Kubernetes => write!(f, "kubernetes"),
            AdapterType::Redis => write!(f, "redis"),
            AdapterType::QuicTcp => write!(f, "quic_tcp"),
            AdapterType::UnixSocket => write!(f, "unix_socket"),
            AdapterType::Grpc => write!(f, "grpc"),
            AdapterType::Mqtt => write!(f, "mqtt"),
            AdapterType::Kafka => write!(f, "kafka"),
            AdapterType::Custom(id) => write!(f, "custom_{}", id),
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded { reason } => write!(f, "degraded: {}", reason),
            HealthStatus::Unhealthy { reason } => write!(f, "unhealthy: {}", reason),
            HealthStatus::Unknown => write!(f, "unknown"),
        }
    }
}