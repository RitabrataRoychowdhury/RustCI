use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::networking::node_communication::ProtocolMessage;
use crate::error::Result;

// Forward declarations for types that will be defined in valkyrie module
pub use crate::core::networking::valkyrie::types::{
    TransportCapabilities
};

/// Enhanced abstract transport layer for Valkyrie Protocol
#[async_trait]
pub trait Transport: Send + Sync {
    /// Get transport type identifier
    fn transport_type(&self) -> TransportType;
    
    /// Start listening for incoming connections
    async fn listen(&self, config: &TransportConfig) -> Result<Box<dyn Listener>>;
    
    /// Connect to a remote endpoint
    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>>;
    
    /// Stop the transport and close all connections
    async fn shutdown(&self) -> Result<()>;
    
    /// Get transport-specific metrics
    async fn get_metrics(&self) -> TransportMetrics;
    
    /// Get transport capabilities
    fn capabilities(&self) -> TransportCapabilities;
    
    /// Configure transport parameters
    async fn configure(&mut self, config: TransportConfig) -> Result<()>;
    
    /// Check if transport supports the given endpoint
    fn supports_endpoint(&self, endpoint: &TransportEndpoint) -> bool;
    
    /// Get optimal configuration for given network conditions
    async fn optimize_for_conditions(&self, conditions: &NetworkConditions) -> TransportConfig;
}

/// Connection abstraction for different transport types
#[async_trait]
pub trait Connection: Send + Sync {
    /// Send a message over the connection
    async fn send(&mut self, message: &ProtocolMessage) -> Result<()>;
    
    /// Receive a message from the connection
    async fn receive(&mut self) -> Result<ProtocolMessage>;
    
    /// Close the connection gracefully
    async fn close(&mut self) -> Result<()>;
    
    /// Check if the connection is still alive
    fn is_connected(&self) -> bool;
    
    /// Get connection metadata
    fn metadata(&self) -> ConnectionMetadata;
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub transport_type: TransportType,
    pub bind_address: String,
    pub port: Option<u16>,
    pub tls_config: Option<TlsConfig>,
    pub authentication: AuthenticationConfig,
    pub timeouts: TimeoutConfig,
    pub buffer_sizes: BufferConfig,
}

/// Enhanced transport types for Valkyrie Protocol
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    /// Standard TCP transport
    Tcp,
    /// TCP with TLS encryption
    SecureTcp,
    /// QUIC transport for modern networking
    Quic,
    /// WebSocket transport for browser compatibility
    WebSocket,
    /// Secure WebSocket with TLS
    SecureWebSocket,
    /// Unix domain socket transport
    UnixSocket { path: PathBuf },
    /// Shared memory transport for local communication
    SharedMemory { segment_name: String },
    /// UDP transport for low-latency scenarios
    Udp,
    /// RDMA transport for high-performance networking
    Rdma { device: String },
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub ca_cert_path: Option<PathBuf>,
    pub verify_client: bool,
    pub cipher_suites: Vec<String>,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    pub method: AuthMethod,
    pub jwt_secret: Option<String>,
    pub token_expiry: Duration,
    pub require_mutual_auth: bool,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    None,
    JwtToken,
    MutualTls,
    ApiKey,
}

/// Timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub keepalive_interval: Duration,
}

/// Buffer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferConfig {
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
    pub max_message_size: usize,
}

/// Transport endpoint specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportEndpoint {
    pub transport_type: TransportType,
    pub address: String,
    pub port: Option<u16>,
    pub path: Option<PathBuf>,
    pub tls_enabled: bool,
}

/// Connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionMetadata {
    pub connection_id: Uuid,
    pub remote_address: String,
    pub local_address: String,
    pub transport_type: TransportType,
    pub established_at: chrono::DateTime<chrono::Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// Transport metrics with enhanced observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportMetrics {
    pub active_connections: u32,
    pub total_connections: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connection_errors: u64,
    pub authentication_failures: u64,
    pub average_latency_ms: f64,
    pub throughput_mbps: f64,
    pub packet_loss_rate: f64,
    pub connection_success_rate: f64,
}

/// Network conditions for transport optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConditions {
    pub latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub packet_loss_rate: f64,
    pub jitter_ms: f64,
    pub is_mobile: bool,
    pub is_metered: bool,
    pub congestion_level: CongestionLevel,
}

/// Network congestion levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CongestionLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Listener trait for accepting incoming connections
#[async_trait]
pub trait Listener: Send + Sync {
    /// Accept an incoming connection
    async fn accept(&mut self) -> Result<Box<dyn Connection>>;
    
    /// Get the local address the listener is bound to
    fn local_addr(&self) -> Result<SocketAddr>;
    
    /// Close the listener
    async fn close(&mut self) -> Result<()>;
}

// TCP transport implementation moved to valkyrie::transport::tcp

// TCP connection implementation moved to valkyrie::transport::tcp

// WebSocket transport implementation moved to valkyrie::transport::websocket

// Unix socket transport implementation moved to valkyrie::transport::unix_socket

impl Default for TransportMetrics {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_connections: 0,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            connection_errors: 0,
            authentication_failures: 0,
            average_latency_ms: 0.0,
            throughput_mbps: 0.0,
            packet_loss_rate: 0.0,
            connection_success_rate: 1.0,
        }
    }
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self {
            latency_ms: 50.0,
            bandwidth_mbps: 100.0,
            packet_loss_rate: 0.0,
            jitter_ms: 5.0,
            is_mobile: false,
            is_metered: false,
            congestion_level: CongestionLevel::Low,
        }
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(60),
            write_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(30),
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: 8192,
            write_buffer_size: 8192,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::Tcp,
            bind_address: "0.0.0.0".to_string(),
            port: Some(8080),
            tls_config: None,
            authentication: AuthenticationConfig {
                method: AuthMethod::None,
                jwt_secret: None,
                token_expiry: Duration::from_secs(3600),
                require_mutual_auth: false,
            },
            timeouts: TimeoutConfig::default(),
            buffer_sizes: BufferConfig::default(),
        }
    }
}