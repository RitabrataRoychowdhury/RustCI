use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::core::networking::node_communication::{ProtocolMessage, ProtocolError};
use crate::error::Result;

/// Abstract transport layer for node communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Start listening for incoming connections
    async fn listen(&self, config: &TransportConfig) -> Result<()>;
    
    /// Connect to a remote endpoint
    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>>;
    
    /// Stop the transport and close all connections
    async fn shutdown(&self) -> Result<()>;
    
    /// Get transport-specific metrics
    async fn get_metrics(&self) -> TransportMetrics;
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

/// Supported transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportType {
    Tcp,
    UnixSocket { path: PathBuf },
    WebSocket,
    SecureTcp,
    SecureWebSocket,
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

/// Transport metrics
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
}

/// TCP transport implementation
pub struct TcpTransport {
    connections: Arc<RwLock<HashMap<Uuid, Arc<dyn Connection>>>>,
    listener: Option<TcpListener>,
    metrics: Arc<RwLock<TransportMetrics>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl TcpTransport {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            listener: None,
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
            shutdown_tx: None,
        }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn listen(&self, config: &TransportConfig) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port.unwrap_or(8080))
            .parse()
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Invalid bind address: {}", e) 
            })?;
        
        let _listener = TcpListener::bind(addr).await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to bind to {}: {}", addr, e) 
            })?;
        
        // In a real implementation, we would start the server loop here
        // For now, just store the listener for later use
        
        Ok(())
    }
    
    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        let addr = format!("{}:{}", endpoint.address, endpoint.port.unwrap_or(8080));
        let stream = TcpStream::connect(&addr).await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to connect to {}: {}", addr, e) 
            })?;
        
        let connection = TcpConnection::new(stream);
        Ok(Box::new(connection))
    }
    
    async fn shutdown(&self) -> Result<()> {
        // Send shutdown signal
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
        
        // Close all active connections
        let mut connections = self.connections.write().await;
        for (_, _connection) in connections.drain() {
            // In a real implementation, we'd properly close each connection
        }
        
        Ok(())
    }
    
    async fn get_metrics(&self) -> TransportMetrics {
        self.metrics.read().await.clone()
    }
}

/// TCP connection implementation
pub struct TcpConnection {
    stream: TcpStream,
    metadata: ConnectionMetadata,
    connected: bool,
}

impl TcpConnection {
    pub fn new(stream: TcpStream) -> Self {
        let remote_addr = stream.peer_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        
        let local_addr = stream.local_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        
        Self {
            stream,
            metadata: ConnectionMetadata {
                connection_id: Uuid::new_v4(),
                remote_address: remote_addr,
                local_address: local_addr,
                transport_type: TransportType::Tcp,
                established_at: chrono::Utc::now(),
                bytes_sent: 0,
                bytes_received: 0,
                messages_sent: 0,
                messages_received: 0,
            },
            connected: true,
        }
    }
}

#[async_trait]
impl Connection for TcpConnection {
    async fn send(&mut self, message: &ProtocolMessage) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        
        let serialized = serde_json::to_vec(message)
            .map_err(ProtocolError::SerializationError)?;
        
        // Write message length first (4 bytes)
        let length = serialized.len() as u32;
        self.stream.write_all(&length.to_be_bytes()).await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to write message length: {}", e) 
            })?;
        
        // Write message data
        self.stream.write_all(&serialized).await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to write message: {}", e) 
            })?;
        
        self.stream.flush().await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to flush stream: {}", e) 
            })?;
        
        Ok(())
    }
    
    async fn receive(&mut self) -> Result<ProtocolMessage> {
        use tokio::io::AsyncReadExt;
        
        // Read message length (4 bytes)
        let mut length_bytes = [0u8; 4];
        self.stream.read_exact(&mut length_bytes).await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to read message length: {}", e) 
            })?;
        
        let length = u32::from_be_bytes(length_bytes) as usize;
        
        // Read message data
        let mut buffer = vec![0u8; length];
        self.stream.read_exact(&mut buffer).await
            .map_err(|e| ProtocolError::ConnectionError { 
                message: format!("Failed to read message: {}", e) 
            })?;
        
        // Deserialize message
        let message: ProtocolMessage = serde_json::from_slice(&buffer)
            .map_err(ProtocolError::SerializationError)?;
        
        Ok(message)
    }
    
    async fn close(&mut self) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        
        let _ = self.stream.shutdown().await;
        self.connected = false;
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    fn metadata(&self) -> ConnectionMetadata {
        self.metadata.clone()
    }
}

/// WebSocket transport implementation
pub struct WebSocketTransport {
    connections: Arc<RwLock<HashMap<Uuid, Arc<dyn Connection>>>>,
    metrics: Arc<RwLock<TransportMetrics>>,
}

impl WebSocketTransport {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
        }
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn listen(&self, _config: &TransportConfig) -> Result<()> {
        // WebSocket server implementation would go here
        // This is a simplified placeholder
        Ok(())
    }
    
    async fn connect(&self, _endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        // WebSocket client implementation would go here
        // This is a simplified placeholder
        Err(ProtocolError::ConnectionError { 
            message: "WebSocket transport not fully implemented".to_string() 
        }.into())
    }
    
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    
    async fn get_metrics(&self) -> TransportMetrics {
        self.metrics.read().await.clone()
    }
}

/// Unix socket transport implementation
pub struct UnixSocketTransport {
    connections: Arc<RwLock<HashMap<Uuid, Arc<dyn Connection>>>>,
    metrics: Arc<RwLock<TransportMetrics>>,
}

impl UnixSocketTransport {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
        }
    }
}

#[async_trait]
impl Transport for UnixSocketTransport {
    async fn listen(&self, _config: &TransportConfig) -> Result<()> {
        // Unix socket server implementation would go here
        Ok(())
    }
    
    async fn connect(&self, _endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        // Unix socket client implementation would go here
        Err(ProtocolError::ConnectionError { 
            message: "Unix socket transport not fully implemented".to_string() 
        }.into())
    }
    
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    
    async fn get_metrics(&self) -> TransportMetrics {
        self.metrics.read().await.clone()
    }
}

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