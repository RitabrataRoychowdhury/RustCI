use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::core::networking::node_communication::{ProtocolError, ProtocolMessage};
use crate::core::networking::transport::{
    Connection, ConnectionMetadata, Listener, NetworkConditions, Transport, TransportConfig,
    TransportEndpoint, TransportMetrics, TransportType,
};
use crate::core::networking::valkyrie::transport::{CompressionConfig, ConnectionPool};
use crate::core::networking::valkyrie::types::TransportCapabilities;
use crate::error::Result;

/// WebSocket transport implementation for browser compatibility
pub struct WebSocketTransport {
    /// Active connections registry
    connections: Arc<RwLock<HashMap<Uuid, Arc<Mutex<WebSocketConnection>>>>>,
    /// Connection pool for reuse
    connection_pool: Arc<ConnectionPool>,
    /// Transport metrics
    metrics: Arc<RwLock<TransportMetrics>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Transport configuration
    config: Arc<RwLock<TransportConfig>>,
    /// WebSocket-specific configuration
    websocket_config: WebSocketConfig,
}

/// WebSocket-specific configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum message size
    pub max_message_size: usize,
    /// Maximum frame size
    pub max_frame_size: usize,
    /// Enable compression
    pub compression: CompressionConfig,
    /// Ping interval for keepalive
    pub ping_interval: Duration,
    /// Pong timeout
    pub pong_timeout: Duration,
    /// Subprotocols to negotiate
    pub subprotocols: Vec<String>,
    /// Custom headers for handshake
    pub headers: HashMap<String, String>,
    /// Enable automatic ping/pong
    pub auto_ping_pong: bool,
}

impl WebSocketTransport {
    /// Create a new WebSocket transport
    pub fn new(connection_pool: ConnectionPool, websocket_config: WebSocketConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_pool: Arc::new(connection_pool),
            metrics: Arc::new(RwLock::new(TransportMetrics::default())),
            shutdown_tx: None,
            config: Arc::new(RwLock::new(TransportConfig::default())),
            websocket_config,
        }
    }

    /// Create WebSocket server configuration
    fn create_server_config(&self) -> WebSocketServerConfig {
        WebSocketServerConfig {
            max_message_size: self.websocket_config.max_message_size,
            max_frame_size: self.websocket_config.max_frame_size,
            compression: self.websocket_config.compression.clone(),
            subprotocols: self.websocket_config.subprotocols.clone(),
            auto_ping_pong: self.websocket_config.auto_ping_pong,
        }
    }

    /// Create WebSocket client configuration
    fn create_client_config(&self) -> WebSocketClientConfig {
        WebSocketClientConfig {
            max_message_size: self.websocket_config.max_message_size,
            max_frame_size: self.websocket_config.max_frame_size,
            compression: self.websocket_config.compression.clone(),
            headers: self.websocket_config.headers.clone(),
            subprotocols: self.websocket_config.subprotocols.clone(),
            ping_interval: self.websocket_config.ping_interval,
            pong_timeout: self.websocket_config.pong_timeout,
        }
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
impl Transport for WebSocketTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::WebSocket
    }

    async fn listen(&self, config: &TransportConfig) -> Result<Box<dyn Listener>> {
        let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port.unwrap_or(8080))
            .parse()
            .map_err(|e| ProtocolError::ConnectionError {
                message: format!("Invalid bind address: {}", e),
            })?;

        // Create WebSocket listener (simplified implementation)
        let listener = WebSocketListener::new(addr, self.create_server_config())?;

        Ok(Box::new(listener))
    }

    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        let url = if endpoint.tls_enabled {
            format!(
                "wss://{}:{}",
                endpoint.address,
                endpoint.port.unwrap_or(443)
            )
        } else {
            format!("ws://{}:{}", endpoint.address, endpoint.port.unwrap_or(80))
        };

        // Create WebSocket connection (simplified implementation)
        let connection = WebSocketConnection::connect(&url, self.create_client_config()).await?;
        let connection_id = connection.metadata().connection_id;

        // Register connection
        let mut connections = self.connections.write().await;
        connections.insert(connection_id, Arc::new(Mutex::new(connection)));

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.active_connections += 1;
        metrics.total_connections += 1;

        // Return a wrapper that implements the Connection trait
        Ok(Box::new(WebSocketConnectionWrapper {
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
            max_message_size: self.websocket_config.max_message_size,
            compression_algorithms: vec!["deflate".to_string()],
            encryption_algorithms: vec!["aes-256-gcm".to_string()],
            supports_streaming: true,
            supports_multiplexing: false, // WebSocket doesn't natively support multiplexing
            max_connections: 1000,
            security_level: crate::core::networking::valkyrie::types::SecurityLevel::Enhanced,
            features: std::collections::HashMap::new(),
            supports_compression: self.websocket_config.compression.enabled,
            supports_encryption: true, // WSS support
            supports_flow_control: false,
            encryption_ciphers: vec!["aes-256-gcm".to_string()],
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
        matches!(
            endpoint.transport_type,
            TransportType::WebSocket | TransportType::SecureWebSocket
        )
    }

    async fn optimize_for_conditions(&self, conditions: &NetworkConditions) -> TransportConfig {
        let mut config = TransportConfig::default();
        config.transport_type = if conditions.is_mobile || conditions.is_metered {
            TransportType::WebSocket // Use unencrypted for mobile/metered to save bandwidth
        } else {
            TransportType::SecureWebSocket
        };

        // Optimize for mobile networks
        if conditions.is_mobile {
            config.timeouts.connect_timeout = Duration::from_secs(60);
            config.timeouts.read_timeout = Duration::from_secs(120);
            config.buffer_sizes.max_message_size = 512 * 1024; // 512KB for mobile
        }

        config
    }
}

/// WebSocket server configuration
#[derive(Debug, Clone)]
pub struct WebSocketServerConfig {
    pub max_message_size: usize,
    pub max_frame_size: usize,
    pub compression: CompressionConfig,
    pub subprotocols: Vec<String>,
    pub auto_ping_pong: bool,
}

/// WebSocket client configuration
#[derive(Debug, Clone)]
pub struct WebSocketClientConfig {
    pub max_message_size: usize,
    pub max_frame_size: usize,
    pub compression: CompressionConfig,
    pub headers: HashMap<String, String>,
    pub subprotocols: Vec<String>,
    pub ping_interval: Duration,
    pub pong_timeout: Duration,
}

/// WebSocket listener implementation
pub struct WebSocketListener {
    addr: SocketAddr,
    config: WebSocketServerConfig,
}

impl WebSocketListener {
    pub fn new(addr: SocketAddr, config: WebSocketServerConfig) -> Result<Self> {
        Ok(Self { addr, config })
    }
}

#[async_trait]
impl Listener for WebSocketListener {
    async fn accept(&mut self) -> Result<Box<dyn Connection>> {
        // Simplified implementation - in reality, this would use a WebSocket library
        // like tokio-tungstenite to accept incoming WebSocket connections
        Err(ProtocolError::ConnectionError {
            message: "WebSocket listener not fully implemented".to_string(),
        }
        .into())
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.addr)
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// WebSocket connection implementation
pub struct WebSocketConnection {
    connection_id: Uuid,
    url: String,
    metadata: ConnectionMetadata,
    connected: bool,
    config: WebSocketClientConfig,
    last_ping: Option<std::time::Instant>,
    last_pong: Option<std::time::Instant>,
}

impl WebSocketConnection {
    pub async fn connect(url: &str, config: WebSocketClientConfig) -> Result<Self> {
        // Simplified implementation - in reality, this would establish a WebSocket connection
        let connection_id = Uuid::new_v4();

        Ok(Self {
            connection_id,
            url: url.to_string(),
            metadata: ConnectionMetadata {
                connection_id,
                remote_address: url.to_string(),
                local_address: "unknown".to_string(),
                transport_type: TransportType::WebSocket,
                established_at: chrono::Utc::now(),
                bytes_sent: 0,
                bytes_received: 0,
                messages_sent: 0,
                messages_received: 0,
            },
            connected: true,
            config,
            last_ping: None,
            last_pong: None,
        })
    }

    pub fn metadata(&self) -> ConnectionMetadata {
        self.metadata.clone()
    }

    async fn send_internal(&mut self, message: &ProtocolMessage) -> Result<()> {
        // In a real implementation, this would send the message over WebSocket
        let serialized = serde_json::to_vec(message).map_err(ProtocolError::SerializationError)?;

        // Check message size
        if serialized.len() > self.config.max_message_size {
            return Err(ProtocolError::ConnectionError {
                message: format!("Message too large: {} bytes", serialized.len()),
            }
            .into());
        }

        // Simulate sending over WebSocket
        // In reality, this would use a WebSocket library to send the message

        // Update metadata
        self.metadata.bytes_sent += serialized.len() as u64;
        self.metadata.messages_sent += 1;

        Ok(())
    }

    async fn receive_internal(&mut self) -> Result<ProtocolMessage> {
        // In a real implementation, this would receive from WebSocket
        // For now, we'll return an error indicating incomplete implementation
        Err(ProtocolError::ConnectionError {
            message: "WebSocket receive not fully implemented".to_string(),
        }
        .into())
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    /// Send ping frame for keepalive
    async fn send_ping(&mut self) -> Result<()> {
        // In a real implementation, this would send a WebSocket ping frame
        self.last_ping = Some(std::time::Instant::now());
        Ok(())
    }

    /// Handle pong frame
    async fn handle_pong(&mut self) -> Result<()> {
        self.last_pong = Some(std::time::Instant::now());
        Ok(())
    }

    /// Check if connection is healthy based on ping/pong
    fn is_healthy(&self) -> bool {
        if let (Some(ping_time), Some(pong_time)) = (self.last_ping, self.last_pong) {
            pong_time >= ping_time
        } else {
            true // No ping/pong yet, assume healthy
        }
    }
}

/// Wrapper for WebSocket connection that implements the Connection trait
pub struct WebSocketConnectionWrapper {
    connection_id: Uuid,
    connections: Arc<RwLock<HashMap<Uuid, Arc<Mutex<WebSocketConnection>>>>>,
    metrics: Arc<RwLock<TransportMetrics>>,
}

#[async_trait]
impl Connection for WebSocketConnectionWrapper {
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
            remote_address: "unknown".to_string(),
            local_address: "unknown".to_string(),
            transport_type: TransportType::WebSocket,
            established_at: chrono::Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        }
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_message_size: 16 * 1024 * 1024, // 16MB
            max_frame_size: 64 * 1024,          // 64KB
            compression: CompressionConfig::default(),
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(10),
            subprotocols: vec!["valkyrie".to_string()],
            headers: HashMap::new(),
            auto_ping_pong: true,
        }
    }
}
