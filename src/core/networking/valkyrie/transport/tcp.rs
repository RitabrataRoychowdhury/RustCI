use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::core::networking::node_communication::{ProtocolError, ProtocolMessage};
use crate::core::networking::transport::{
    Connection, ConnectionMetadata, Listener, NetworkConditions, Transport, TransportConfig,
    TransportEndpoint, TransportMetrics, TransportType,
};
use crate::core::networking::valkyrie::transport::ConnectionPool;
use crate::core::networking::valkyrie::types::TransportCapabilities;
use crate::error::Result;

/// Enhanced TCP transport implementation for Valkyrie Protocol
pub struct TcpTransport {
    /// Active connections registry
    connections: Arc<RwLock<HashMap<Uuid, Arc<Mutex<TcpConnection>>>>>,
    /// Connection pool for reuse
    connection_pool: Arc<ConnectionPool>,
    /// Transport metrics
    metrics: Arc<RwLock<TransportMetrics>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Transport configuration
    config: Arc<RwLock<TransportConfig>>,
    /// TCP-specific options
    tcp_options: TcpOptions,
}

/// TCP-specific configuration options
#[derive(Debug, Clone)]
pub struct TcpOptions {
    /// Enable TCP_NODELAY (disable Nagle's algorithm)
    pub nodelay: bool,
    /// SO_KEEPALIVE settings
    pub keepalive: Option<TcpKeepalive>,
    /// SO_REUSEADDR setting
    pub reuseaddr: bool,
    /// SO_REUSEPORT setting
    pub reuseport: bool,
    /// TCP receive buffer size
    pub recv_buffer_size: Option<usize>,
    /// TCP send buffer size
    pub send_buffer_size: Option<usize>,
    /// Connection timeout
    pub connect_timeout: Duration,
}

/// TCP keepalive configuration
#[derive(Debug, Clone)]
pub struct TcpKeepalive {
    /// Time before sending keepalive probes
    pub time: Duration,
    /// Interval between keepalive probes
    pub interval: Duration,
    /// Number of probes before declaring connection dead
    pub retries: u32,
}

impl TcpTransport {
    /// Create a new TCP transport
    pub fn new(connection_pool: ConnectionPool) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_pool: Arc::new(connection_pool),
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
            shutdown_tx: None,
            config: Arc::new(RwLock::new(TransportConfig::default())),
            tcp_options: TcpOptions::default(),
        }
    }

    /// Create TCP transport with custom options
    pub fn with_options(connection_pool: ConnectionPool, tcp_options: TcpOptions) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_pool: Arc::new(connection_pool),
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
            shutdown_tx: None,
            config: Arc::new(RwLock::new(TransportConfig::default())),
            tcp_options,
        }
    }

    /// Configure TCP socket with options
    async fn configure_socket(&self, stream: &TcpStream) -> Result<()> {
        // Set TCP_NODELAY
        if self.tcp_options.nodelay {
            stream
                .set_nodelay(true)
                .map_err(|e| ProtocolError::ConnectionError {
                    message: format!("Failed to set TCP_NODELAY: {}", e),
                })?;
        }

        // Configure keepalive
        if let Some(_keepalive) = &self.tcp_options.keepalive {
            // Note: Setting keepalive parameters requires platform-specific code
            // This is a simplified implementation that would need proper socket2 integration
            // For now, we'll skip the actual socket2 configuration
        }

        Ok(())
    }

    /// Update connection metrics
    async fn update_metrics(&self, bytes_sent: u64, bytes_received: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.bytes_sent += bytes_sent;
        metrics.bytes_received += bytes_received;
        if bytes_sent > 0 {
            metrics.messages_sent += 1;
        }
        if bytes_received > 0 {
            metrics.messages_received += 1;
        }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::Tcp
    }

    async fn listen(&self, config: &TransportConfig) -> Result<Box<dyn Listener>> {
        let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port.unwrap_or(8080))
            .parse()
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Invalid bind address: {}", e),
            })?;

        let listener =
            TcpListener::bind(addr)
                .await
                .map_err(|e| ProtocolError::ConnectionError {
                    message: format!("Failed to bind to {}: {}", addr, e),
                })?;

        // Configure listener socket options
        if self.tcp_options.reuseaddr {
            // Note: Socket options configuration would be implemented here
            // For now, we'll skip the actual socket2 configuration
        }

        Ok(Box::new(TcpListenerWrapper::new(listener)))
    }

    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        let addr = format!("{}:{}", endpoint.address, endpoint.port.unwrap_or(8080));

        // Apply connection timeout
        let stream =
            tokio::time::timeout(self.tcp_options.connect_timeout, TcpStream::connect(&addr))
                .await
                .map_err(|_| ProtocolError::ConnectionError {
                    message: format!("Connection timeout to {}", addr),
                })?
                .map_err(|e| ProtocolError::ConnectionError {
                    message: format!("Failed to connect to {}: {}", addr, e),
                })?;

        // Configure the socket
        self.configure_socket(&stream).await?;

        let connection = TcpConnection::new(stream, self.metrics.clone());
        let connection_id = connection.metadata.connection_id;

        // Register connection
        let mut connections = self.connections.write().await;
        connections.insert(connection_id, Arc::new(Mutex::new(connection)));

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.active_connections += 1;
        metrics.total_connections += 1;

        // Return a wrapper that implements the Connection trait
        Ok(Box::new(TcpConnectionWrapper {
            connection_id,
            connections: self.connections.clone(),
            metrics: self.metrics.clone(),
        }))
    }

    async fn shutdown(&self) -> Result<()> {
        // Send shutdown signal
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }

        // Close all active connections
        let mut connections = self.connections.write().await;
        for (_, connection) in connections.drain() {
            let mut conn = connection.lock().await;
            let _ = conn.close().await;
        }

        // Reset metrics
        let mut metrics = self.metrics.write().await;
        metrics.active_connections = 0;

        Ok(())
    }

    async fn get_metrics(&self) -> TransportMetrics {
        self.metrics.read().await.clone()
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            max_message_size: 16 * 1024 * 1024, // 16MB
            compression_algorithms: vec!["gzip".to_string(), "deflate".to_string()],
            encryption_algorithms: vec!["none".to_string()],
            supports_streaming: true,
            supports_multiplexing: false,
            max_connections: 1000,
            security_level: crate::core::networking::valkyrie::types::SecurityLevel::Basic,
            features: std::collections::HashMap::new(),
            supports_compression: true,
            supports_encryption: false, // TLS would be handled by SecureTcp
            supports_flow_control: true,
            encryption_ciphers: vec!["none".to_string()],
            supports_connection_pooling: true,
            supports_failover: true,
        }
    }

    async fn configure(&mut self, config: TransportConfig) -> Result<()> {
        let mut current_config = self.config.write().await;
        *current_config = config;
        Ok(())
    }

    fn supports_endpoint(&self, endpoint: &TransportEndpoint) -> bool {
        matches!(endpoint.transport_type, TransportType::Tcp)
    }

    async fn optimize_for_conditions(&self, conditions: &NetworkConditions) -> TransportConfig {
        let mut config = TransportConfig::default();
        config.transport_type = TransportType::Tcp;

        // Optimize buffer sizes based on bandwidth
        if conditions.bandwidth_mbps > 100.0 {
            config.buffer_sizes.read_buffer_size = 64 * 1024; // 64KB
            config.buffer_sizes.write_buffer_size = 64 * 1024;
        } else if conditions.bandwidth_mbps > 10.0 {
            config.buffer_sizes.read_buffer_size = 32 * 1024; // 32KB
            config.buffer_sizes.write_buffer_size = 32 * 1024;
        }

        // Adjust timeouts based on latency
        if conditions.latency_ms > 100.0 {
            config.timeouts.connect_timeout = Duration::from_secs(60);
            config.timeouts.read_timeout = Duration::from_secs(120);
        }

        config
    }
}

/// TCP connection implementation
pub struct TcpConnection {
    stream: TcpStream,
    metadata: ConnectionMetadata,
    connected: bool,
    metrics: Arc<RwLock<TransportMetrics>>,
}

impl TcpConnection {
    pub fn new(stream: TcpStream, metrics: Arc<RwLock<TransportMetrics>>) -> Self {
        let remote_addr = stream
            .peer_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let local_addr = stream
            .local_addr()
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
            metrics,
        }
    }

    async fn send_internal(&mut self, message: &ProtocolMessage) -> Result<()> {
        let serialized = serde_json::to_vec(message).map_err(ProtocolError::SerializationError)?;

        // Write message length first (4 bytes)
        let length = serialized.len() as u32;
        self.stream
            .write_all(&length.to_be_bytes())
            .await
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to write message length: {}", e),
            })?;

        // Write message data
        self.stream
            .write_all(&serialized)
            .await
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to write message: {}", e),
            })?;

        self.stream
            .flush()
            .await
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to flush stream: {}", e),
            })?;

        // Update metrics
        let bytes_sent = 4 + serialized.len() as u64;
        let mut metrics = self.metrics.write().await;
        metrics.bytes_sent += bytes_sent;
        metrics.messages_sent += 1;

        Ok(())
    }

    async fn receive_internal(&mut self) -> Result<ProtocolMessage> {
        // Read message length (4 bytes)
        let mut length_bytes = [0u8; 4];
        self.stream
            .read_exact(&mut length_bytes)
            .await
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to read message length: {}", e),
            })?;

        let length = u32::from_be_bytes(length_bytes) as usize;

        // Validate message size
        if length > 16 * 1024 * 1024 {
            // 16MB limit
            return Err(ProtocolError::ConnectionError {
                message: format!("Message too large: {} bytes", length),
            }
            .into());
        }

        // Read message data
        let mut buffer = vec![0u8; length];
        self.stream
            .read_exact(&mut buffer)
            .await
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to read message: {}", e),
            })?;

        // Deserialize message
        let message: ProtocolMessage =
            serde_json::from_slice(&buffer).map_err(ProtocolError::SerializationError)?;

        // Update metrics
        let bytes_received = 4 + length as u64;
        let mut metrics = self.metrics.write().await;
        metrics.bytes_received += bytes_received;
        metrics.messages_received += 1;

        Ok(message)
    }

    async fn close_internal(&mut self) -> Result<()> {
        let _ = self.stream.shutdown().await;
        self.connected = false;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        if metrics.active_connections > 0 {
            metrics.active_connections -= 1;
        }

        Ok(())
    }
}

#[async_trait]
impl Connection for TcpConnection {
    async fn send(&mut self, message: &ProtocolMessage) -> Result<()> {
        self.send_internal(message).await
    }

    async fn receive(&mut self) -> Result<ProtocolMessage> {
        self.receive_internal().await
    }

    async fn close(&mut self) -> Result<()> {
        self.close_internal().await
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn metadata(&self) -> ConnectionMetadata {
        self.metadata.clone()
    }
}

/// TCP listener wrapper that implements the Listener trait
pub struct TcpListenerWrapper {
    listener: TcpListener,
}

impl TcpListenerWrapper {
    pub fn new(listener: TcpListener) -> Self {
        Self { listener }
    }
}

#[async_trait]
impl Listener for TcpListenerWrapper {
    async fn accept(&mut self) -> Result<Box<dyn Connection>> {
        let (stream, _addr) =
            self.listener
                .accept()
                .await
                .map_err(|e| ProtocolError::ConnectionError {
                    message: format!("Failed to accept TCP connection: {}", e),
                })?;

        let connection =
            TcpConnection::new(stream, Arc::new(RwLock::new(TransportMetrics::default())));
        Ok(Box::new(connection))
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().map_err(|e| {
            ProtocolError::ConnectionError {
                message: format!("Failed to get local address: {}", e),
            }
            .into()
        })
    }

    async fn close(&mut self) -> Result<()> {
        // TcpListener doesn't have an explicit close method in tokio
        // The listener will be closed when dropped
        Ok(())
    }
}

/// Wrapper for TCP connection that implements the Connection trait
pub struct TcpConnectionWrapper {
    connection_id: Uuid,
    connections: Arc<RwLock<HashMap<Uuid, Arc<Mutex<TcpConnection>>>>>,
    metrics: Arc<RwLock<TransportMetrics>>,
}

#[async_trait]
impl Connection for TcpConnectionWrapper {
    async fn send(&mut self, message: &ProtocolMessage) -> Result<()> {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&self.connection_id) {
            let mut conn = connection.lock().await;
            conn.send_internal(message).await
        } else {
            Err(ProtocolError::ConnectionError {
                message: "Connection not found".to_string(),
            }
            .into())
        }
    }

    async fn receive(&mut self) -> Result<ProtocolMessage> {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&self.connection_id) {
            let mut conn = connection.lock().await;
            conn.receive_internal().await
        } else {
            Err(ProtocolError::ConnectionError {
                message: "Connection not found".to_string(),
            }
            .into())
        }
    }

    async fn close(&mut self) -> Result<()> {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&self.connection_id) {
            let mut conn = connection.lock().await;
            conn.close_internal().await?;
        }

        // Remove from connections registry
        drop(connections);
        let mut connections = self.connections.write().await;
        connections.remove(&self.connection_id);

        Ok(())
    }

    fn is_connected(&self) -> bool {
        // This is a simplified check - in a real implementation,
        // we'd check the actual connection state
        true
    }

    fn metadata(&self) -> ConnectionMetadata {
        // Return a default metadata - in a real implementation,
        // we'd get this from the actual connection
        ConnectionMetadata {
            connection_id: self.connection_id,
            remote_address: "unknown".to_string(),
            local_address: "unknown".to_string(),
            transport_type: TransportType::Tcp,
            established_at: chrono::Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        }
    }
}

impl Default for TcpOptions {
    fn default() -> Self {
        Self {
            nodelay: true,
            keepalive: Some(TcpKeepalive {
                time: Duration::from_secs(7200), // 2 hours
                interval: Duration::from_secs(75),
                retries: 9,
            }),
            reuseaddr: true,
            reuseport: false,
            recv_buffer_size: None,
            send_buffer_size: None,
            connect_timeout: Duration::from_secs(30),
        }
    }
}
