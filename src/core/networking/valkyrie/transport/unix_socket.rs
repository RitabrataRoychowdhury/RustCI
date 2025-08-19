use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
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

/// Unix socket transport implementation for local communication
pub struct UnixSocketTransport {
    /// Active connections registry
    connections: Arc<RwLock<HashMap<Uuid, Arc<Mutex<UnixSocketConnection>>>>>,
    /// Connection pool for reuse
    connection_pool: Arc<ConnectionPool>,
    /// Transport metrics
    metrics: Arc<RwLock<TransportMetrics>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Transport configuration
    config: Arc<RwLock<TransportConfig>>,
    /// Unix socket-specific configuration
    unix_config: UnixSocketConfig,
}

/// Unix socket-specific configuration
#[derive(Debug, Clone)]
pub struct UnixSocketConfig {
    /// Socket file permissions (octal)
    pub socket_permissions: u32,
    /// Enable credential passing
    pub enable_credentials: bool,
    /// Buffer size for socket operations
    pub buffer_size: usize,
    /// Maximum message size
    pub max_message_size: usize,
    /// Socket timeout
    pub socket_timeout: Duration,
    /// Cleanup socket file on shutdown
    pub cleanup_on_shutdown: bool,
}

impl UnixSocketTransport {
    /// Create a new Unix socket transport
    pub fn new(connection_pool: ConnectionPool, unix_config: UnixSocketConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_pool: Arc::new(connection_pool),
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
            shutdown_tx: None,
            config: Arc::new(RwLock::new(TransportConfig::default())),
            unix_config,
        }
    }

    /// Set socket permissions
    fn set_socket_permissions(&self, path: &PathBuf) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(self.unix_config.socket_permissions);
            std::fs::set_permissions(path, permissions).map_err(|e| {
                ProtocolError::ConnectionError {
                    message: format!("Failed to set socket permissions: {}", e),
                }
            })?;
        }
        Ok(())
    }

    /// Cleanup socket file
    fn cleanup_socket(&self, path: &PathBuf) -> Result<()> {
        if self.unix_config.cleanup_on_shutdown && path.exists() {
            std::fs::remove_file(path).map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to cleanup socket file: {}", e),
            })?;
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
impl Transport for UnixSocketTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::UnixSocket {
            path: PathBuf::from("/tmp/valkyrie.sock"), // Default path
        }
    }

    async fn listen(&self, config: &TransportConfig) -> Result<Box<dyn Listener>> {
        let socket_path = match &config.transport_type {
            TransportType::UnixSocket { path } => path.clone(),
            _ => {
                return Err(ProtocolError::ConnectionError {
                    message: "Invalid transport type for Unix socket".to_string(),
                }
                .into())
            }
        };

        // Remove existing socket file if it exists
        if socket_path.exists() {
            std::fs::remove_file(&socket_path).map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to remove existing socket file: {}", e),
            })?;
        }

        // Create Unix listener
        let listener =
            UnixListener::bind(&socket_path).map_err(|e| ProtocolError::ConnectionError {
                message: format!("Failed to bind Unix socket to {:?}: {}", socket_path, e),
            })?;

        // Set socket permissions
        self.set_socket_permissions(&socket_path)?;

        Ok(Box::new(UnixSocketListener::new(
            listener,
            socket_path,
            self.unix_config.clone(),
        )))
    }

    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        let socket_path = match &endpoint.transport_type {
            TransportType::UnixSocket { path } => path.clone(),
            _ => {
                return Err(ProtocolError::ConnectionError {
                    message: "Invalid transport type for Unix socket".to_string(),
                }
                .into())
            }
        };

        // Connect to Unix socket
        let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
            ProtocolError::ConnectionError {
                message: format!("Failed to connect to Unix socket {:?}: {}", socket_path, e),
            }
        })?;

        let connection = UnixSocketConnection::new(stream, socket_path, self.metrics.clone());
        let connection_id = connection.metadata().connection_id;

        // Register connection
        let mut connections = self.connections.write().await;
        connections.insert(connection_id, Arc::new(Mutex::new(connection)));

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.active_connections += 1;
        metrics.total_connections += 1;

        // Return a wrapper that implements the Connection trait
        Ok(Box::new(UnixSocketConnectionWrapper {
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
            max_message_size: self.unix_config.max_message_size,
            compression_algorithms: vec!["gzip".to_string(), "lz4".to_string()],
            encryption_algorithms: vec!["none".to_string()],
            supports_streaming: true,
            supports_multiplexing: false,
            max_connections: 1000,
            security_level: crate::core::networking::valkyrie::types::SecurityLevel::Basic,
            features: std::collections::HashMap::new(),
            supports_compression: true,
            supports_encryption: false, // Unix sockets rely on filesystem permissions
            supports_flow_control: true,
            encryption_ciphers: vec!["none".to_string()],
            supports_connection_pooling: true,
            supports_failover: false, // Local communication doesn't need failover
        }
    }

    async fn configure(&mut self, config: TransportConfig) -> Result<()> {
        let mut current_config = self.config.write().await;
        *current_config = config;
        Ok(())
    }

    fn supports_endpoint(&self, endpoint: &TransportEndpoint) -> bool {
        matches!(endpoint.transport_type, TransportType::UnixSocket { .. })
    }

    async fn optimize_for_conditions(&self, _conditions: &NetworkConditions) -> TransportConfig {
        let mut config = TransportConfig::default();
        config.transport_type = TransportType::UnixSocket {
            path: PathBuf::from("/tmp/valkyrie.sock"),
        };

        // Unix sockets are local, so network conditions don't apply much
        // Optimize for high throughput local communication
        config.buffer_sizes.read_buffer_size = 128 * 1024; // 128KB
        config.buffer_sizes.write_buffer_size = 128 * 1024;
        config.buffer_sizes.max_message_size = 64 * 1024 * 1024; // 64MB

        // Very short timeouts for local communication
        config.timeouts.connect_timeout = Duration::from_secs(5);
        config.timeouts.read_timeout = Duration::from_secs(30);
        config.timeouts.write_timeout = Duration::from_secs(30);

        config
    }
}

/// Unix socket listener implementation
pub struct UnixSocketListener {
    listener: UnixListener,
    socket_path: PathBuf,
    config: UnixSocketConfig,
}

impl UnixSocketListener {
    pub fn new(listener: UnixListener, socket_path: PathBuf, config: UnixSocketConfig) -> Self {
        Self {
            listener,
            socket_path,
            config,
        }
    }
}

#[async_trait]
impl Listener for UnixSocketListener {
    async fn accept(&mut self) -> Result<Box<dyn Connection>> {
        let (stream, _addr) =
            self.listener
                .accept()
                .await
                .map_err(|e| ProtocolError::ConnectionError {
                    message: format!("Failed to accept Unix socket connection: {}", e),
                })?;

        let connection = UnixSocketConnection::new(
            stream,
            self.socket_path.clone(),
            Arc::new(RwLock::new(TransportMetrics::default())),
        );

        Ok(Box::new(connection))
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        // Unix sockets don't have network addresses, return a placeholder
        Ok("127.0.0.1:0".parse().unwrap())
    }

    async fn close(&mut self) -> Result<()> {
        // Cleanup socket file if configured to do so
        if self.config.cleanup_on_shutdown && self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                ProtocolError::ConnectionError {
                    message: format!("Failed to cleanup socket file: {}", e),
                }
            })?;
        }
        Ok(())
    }
}

/// Unix socket connection implementation
pub struct UnixSocketConnection {
    stream: UnixStream,
    socket_path: PathBuf,
    metadata: ConnectionMetadata,
    connected: bool,
    metrics: Arc<RwLock<TransportMetrics>>,
}

impl UnixSocketConnection {
    pub fn new(
        stream: UnixStream,
        socket_path: PathBuf,
        metrics: Arc<RwLock<TransportMetrics>>,
    ) -> Self {
        let connection_id = Uuid::new_v4();

        Self {
            stream,
            socket_path: socket_path.clone(),
            metadata: ConnectionMetadata {
                connection_id,
                remote_address: socket_path.to_string_lossy().to_string(),
                local_address: "unix_socket".to_string(),
                transport_type: TransportType::UnixSocket { path: socket_path },
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

    pub fn metadata(&self) -> ConnectionMetadata {
        self.metadata.clone()
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
        if length > 64 * 1024 * 1024 {
            // 64MB limit for Unix sockets
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

    async fn close(&mut self) -> Result<()> {
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
impl Connection for UnixSocketConnection {
    async fn send(&mut self, message: &ProtocolMessage) -> Result<()> {
        self.send_internal(message).await
    }

    async fn receive(&mut self) -> Result<ProtocolMessage> {
        self.receive_internal().await
    }

    async fn close(&mut self) -> Result<()> {
        self.close().await
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn metadata(&self) -> ConnectionMetadata {
        self.metadata.clone()
    }
}

/// Wrapper for Unix socket connection that implements the Connection trait
pub struct UnixSocketConnectionWrapper {
    connection_id: Uuid,
    connections: Arc<RwLock<HashMap<Uuid, Arc<Mutex<UnixSocketConnection>>>>>,
    metrics: Arc<RwLock<TransportMetrics>>,
}

#[async_trait]
impl Connection for UnixSocketConnectionWrapper {
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
            conn.close().await?;
        }

        // Remove from connections registry
        drop(connections);
        let mut connections = self.connections.write().await;
        connections.remove(&self.connection_id);

        Ok(())
    }

    fn is_connected(&self) -> bool {
        true // Simplified implementation
    }

    fn metadata(&self) -> ConnectionMetadata {
        ConnectionMetadata {
            connection_id: self.connection_id,
            remote_address: "unix_socket".to_string(),
            local_address: "unix_socket".to_string(),
            transport_type: TransportType::UnixSocket {
                path: PathBuf::from("/tmp/valkyrie.sock"),
            },
            established_at: chrono::Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        }
    }
}

impl Default for UnixSocketConfig {
    fn default() -> Self {
        Self {
            socket_permissions: 0o660, // rw-rw----
            enable_credentials: false,
            buffer_size: 64 * 1024,             // 64KB
            max_message_size: 64 * 1024 * 1024, // 64MB
            socket_timeout: Duration::from_secs(30),
            cleanup_on_shutdown: true,
        }
    }
}
