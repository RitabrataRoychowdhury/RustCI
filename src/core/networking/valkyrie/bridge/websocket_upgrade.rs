//! WebSocket Upgrade Handler for Valkyrie Protocol
//! 
//! Handles WebSocket upgrade requests and provides bidirectional communication
//! between WebSocket clients and Valkyrie Protocol services.

use super::{BridgeError, BridgeResult};
use crate::core::networking::valkyrie::{
    engine::ValkyrieEngine,
    message::{ValkyrieMessage, MessageType, MessagePayload},
};
use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade, Message as WsMessage},
        State, ConnectInfo,
    },
    http::HeaderMap,
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// WebSocket upgrade handler
pub struct WebSocketUpgradeHandler {
    /// Valkyrie engine instance
    valkyrie_engine: Arc<ValkyrieEngine>,
    /// Active WebSocket connections
    connections: Arc<RwLock<HashMap<Uuid, WebSocketConnection>>>,
    /// WebSocket configuration
    config: WebSocketConfig,
    /// Connection metrics
    metrics: Arc<WebSocketMetrics>,
}

/// WebSocket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    /// Ping interval in seconds
    pub ping_interval_seconds: u64,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Enable message compression
    pub enable_compression: bool,
    /// Enable automatic reconnection
    pub enable_auto_reconnect: bool,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_seconds: u64,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_message_size: 16 * 1024 * 1024, // 16MB
            connection_timeout_seconds: 300, // 5 minutes
            ping_interval_seconds: 30,
            max_connections: 10000,
            enable_compression: true,
            enable_auto_reconnect: true,
            heartbeat_interval_seconds: 60,
        }
    }
}

/// WebSocket connection information
#[derive(Debug, Clone)]
pub struct WebSocketConnection {
    /// Connection ID
    pub id: Uuid,
    /// Client address
    pub client_addr: SocketAddr,
    /// Connection start time
    pub start_time: Instant,
    /// Last activity time
    pub last_activity: Instant,
    /// Connection state
    pub state: ConnectionState,
    /// Message sender channel
    pub sender: mpsc::UnboundedSender<WsMessage>,
    /// Connection metadata
    pub metadata: HashMap<String, String>,
    /// Authentication status
    pub authenticated: bool,
    /// User ID if authenticated
    pub user_id: Option<String>,
}

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection is active and ready
    Connected,
    /// Connection is being closed
    Closing,
    /// Connection is closed
    Closed,
    /// Connection encountered an error
    Error(String),
}

/// WebSocket metrics
#[derive(Debug, Default)]
pub struct WebSocketMetrics {
    /// Total connections established
    pub total_connections: Arc<std::sync::atomic::AtomicU64>,
    /// Currently active connections
    pub active_connections: Arc<std::sync::atomic::AtomicU64>,
    /// Total messages sent
    pub messages_sent: Arc<std::sync::atomic::AtomicU64>,
    /// Total messages received
    pub messages_received: Arc<std::sync::atomic::AtomicU64>,
    /// Connection errors
    pub connection_errors: Arc<std::sync::atomic::AtomicU64>,
    /// Average message processing time
    pub avg_processing_time_ms: Arc<std::sync::atomic::AtomicU64>,
}

/// WebSocket message types for Valkyrie integration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ValkyrieWebSocketMessage {
    /// Authentication message
    Auth {
        token: String,
        user_id: Option<String>,
    },
    /// Subscribe to message types
    Subscribe {
        message_types: Vec<String>,
        filters: Option<HashMap<String, String>>,
    },
    /// Unsubscribe from message types
    Unsubscribe {
        message_types: Vec<String>,
    },
    /// Send a Valkyrie message
    Send {
        message: serde_json::Value,
        target: Option<String>,
    },
    /// Receive a Valkyrie message
    Receive {
        message: serde_json::Value,
        source: String,
        timestamp: String,
    },
    /// Ping message for keepalive
    Ping {
        timestamp: String,
    },
    /// Pong response
    Pong {
        timestamp: String,
    },
    /// Error message
    Error {
        code: u16,
        message: String,
        details: Option<HashMap<String, String>>,
    },
    /// Status update
    Status {
        connection_id: String,
        state: String,
        metadata: HashMap<String, String>,
    },
}

impl WebSocketUpgradeHandler {
    /// Create a new WebSocket upgrade handler
    pub fn new(valkyrie_engine: Arc<ValkyrieEngine>, config: WebSocketConfig) -> Self {
        Self {
            valkyrie_engine,
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(WebSocketMetrics::default()),
        }
    }

    /// Handle WebSocket upgrade request
    pub async fn handle_upgrade(
        &self,
        ws: WebSocketUpgrade,
        headers: HeaderMap,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ) -> BridgeResult<Response> {
        info!("WebSocket upgrade request from {}", addr);

        // Check connection limits
        let active_connections = self.metrics.active_connections.load(std::sync::atomic::Ordering::Relaxed);
        if active_connections >= self.config.max_connections as u64 {
            warn!("WebSocket connection limit reached: {}", active_connections);
            return Err(BridgeError::WebSocketUpgradeFailed {
                reason: "Connection limit reached".to_string(),
            });
        }

        // Validate upgrade headers
        self.validate_upgrade_headers(&headers)?;

        let handler = self.clone();
        let response = ws.on_upgrade(move |socket| {
            handler.handle_socket(socket, addr)
        });

        Ok(response)
    }

    /// Validate WebSocket upgrade headers
    fn validate_upgrade_headers(&self, headers: &HeaderMap) -> BridgeResult<()> {
        // Check for required WebSocket headers
        if !headers.contains_key("sec-websocket-key") {
            return Err(BridgeError::WebSocketUpgradeFailed {
                reason: "Missing Sec-WebSocket-Key header".to_string(),
            });
        }

        if let Some(version) = headers.get("sec-websocket-version") {
            if version != "13" {
                return Err(BridgeError::WebSocketUpgradeFailed {
                    reason: format!("Unsupported WebSocket version: {:?}", version),
                });
            }
        }

        Ok(())
    }

    /// Handle WebSocket connection
    async fn handle_socket(self, socket: WebSocket, addr: SocketAddr) {
        let connection_id = Uuid::new_v4();
        info!("New WebSocket connection: {} from {}", connection_id, addr);

        self.metrics.total_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Create message channel for this connection
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

        // Create connection record
        let connection = WebSocketConnection {
            id: connection_id,
            client_addr: addr,
            start_time: Instant::now(),
            last_activity: Instant::now(),
            state: ConnectionState::Connecting,
            sender: tx.clone(),
            metadata: HashMap::new(),
            authenticated: false,
            user_id: None,
        };

        // Store connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id, connection);
        }

        // Split socket into sender and receiver
        let (mut ws_sender, mut ws_receiver) = socket.split();

        // Spawn task to handle outgoing messages
        let connections_clone = self.connections.clone();
        let outgoing_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = ws_sender.send(message).await {
                    error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        });

        // Handle incoming messages
        let handler = self.clone();
        let incoming_task = tokio::spawn(async move {
            while let Some(message) = ws_receiver.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) = handler.handle_message(connection_id, msg).await {
                            error!("Error handling WebSocket message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        });

        // Start heartbeat task
        let heartbeat_handler = self.clone();
        let heartbeat_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(heartbeat_handler.config.heartbeat_interval_seconds)
            );

            loop {
                interval.tick().await;
                if let Err(e) = heartbeat_handler.send_heartbeat(connection_id).await {
                    debug!("Heartbeat failed for connection {}: {}", connection_id, e);
                    break;
                }
            }
        });

        // Wait for tasks to complete
        tokio::select! {
            _ = incoming_task => {
                debug!("Incoming message task completed for connection {}", connection_id);
            }
            _ = outgoing_task => {
                debug!("Outgoing message task completed for connection {}", connection_id);
            }
            _ = heartbeat_task => {
                debug!("Heartbeat task completed for connection {}", connection_id);
            }
        }

        // Clean up connection
        self.cleanup_connection(connection_id).await;
    }

    /// Handle incoming WebSocket message
    async fn handle_message(
        &self,
        connection_id: Uuid,
        message: WsMessage,
    ) -> BridgeResult<()> {
        self.metrics.messages_received.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let start_time = Instant::now();

        // Update last activity
        self.update_connection_activity(connection_id).await;

        match message {
            WsMessage::Text(text) => {
                debug!("Received text message from {}: {}", connection_id, text);
                self.handle_text_message(connection_id, text).await?;
            }
            WsMessage::Binary(data) => {
                debug!("Received binary message from {}: {} bytes", connection_id, data.len());
                self.handle_binary_message(connection_id, data).await?;
            }
            WsMessage::Ping(data) => {
                debug!("Received ping from {}", connection_id);
                self.send_message(connection_id, WsMessage::Pong(data)).await?;
            }
            WsMessage::Pong(_) => {
                debug!("Received pong from {}", connection_id);
                // Update connection activity (already done above)
            }
            WsMessage::Close(close_frame) => {
                info!("Received close message from {}: {:?}", connection_id, close_frame);
                self.close_connection(connection_id).await?;
            }
        }

        let processing_time = start_time.elapsed();
        self.metrics.avg_processing_time_ms.store(
            processing_time.as_millis() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

        Ok(())
    }

    /// Handle text message (JSON)
    async fn handle_text_message(
        &self,
        connection_id: Uuid,
        text: String,
    ) -> BridgeResult<()> {
        // Parse as Valkyrie WebSocket message
        let ws_message: ValkyrieWebSocketMessage = serde_json::from_str(&text)
            .map_err(|e| BridgeError::InvalidRequestFormat {
                details: format!("Invalid JSON message: {}", e),
            })?;

        match ws_message {
            ValkyrieWebSocketMessage::Auth { token, user_id } => {
                self.handle_auth(connection_id, token, user_id).await?;
            }
            ValkyrieWebSocketMessage::Subscribe { message_types, filters } => {
                self.handle_subscribe(connection_id, message_types, filters).await?;
            }
            ValkyrieWebSocketMessage::Unsubscribe { message_types } => {
                self.handle_unsubscribe(connection_id, message_types).await?;
            }
            ValkyrieWebSocketMessage::Send { message, target } => {
                self.handle_send_message(connection_id, message, target).await?;
            }
            ValkyrieWebSocketMessage::Ping { timestamp } => {
                let pong = ValkyrieWebSocketMessage::Pong { timestamp };
                self.send_valkyrie_message(connection_id, pong).await?;
            }
            _ => {
                warn!("Unexpected message type from client: {:?}", ws_message);
            }
        }

        Ok(())
    }

    /// Handle binary message
    async fn handle_binary_message(
        &self,
        connection_id: Uuid,
        data: Vec<u8>,
    ) -> BridgeResult<()> {
        // For now, treat binary messages as Valkyrie protocol messages
        // In a full implementation, this would deserialize the binary data
        debug!("Handling binary message of {} bytes from {}", data.len(), connection_id);
        
        // TODO: Implement binary message handling
        // This would involve deserializing the binary data as a ValkyrieMessage
        // and processing it through the Valkyrie engine
        
        Ok(())
    }

    /// Handle authentication
    async fn handle_auth(
        &self,
        connection_id: Uuid,
        token: String,
        user_id: Option<String>,
    ) -> BridgeResult<()> {
        debug!("Handling authentication for connection {}", connection_id);

        // TODO: Implement actual authentication logic
        // For now, accept all authentication attempts
        let authenticated = true;

        // Update connection authentication status
        {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.authenticated = authenticated;
                connection.user_id = user_id.clone();
            }
        }

        // Send authentication result
        let auth_result = if authenticated {
            ValkyrieWebSocketMessage::Status {
                connection_id: connection_id.to_string(),
                state: "authenticated".to_string(),
                metadata: {
                    let mut meta = HashMap::new();
                    if let Some(uid) = user_id {
                        meta.insert("user_id".to_string(), uid);
                    }
                    meta
                },
            }
        } else {
            ValkyrieWebSocketMessage::Error {
                code: 401,
                message: "Authentication failed".to_string(),
                details: None,
            }
        };

        self.send_valkyrie_message(connection_id, auth_result).await?;
        Ok(())
    }

    /// Handle subscription to message types
    async fn handle_subscribe(
        &self,
        connection_id: Uuid,
        message_types: Vec<String>,
        filters: Option<HashMap<String, String>>,
    ) -> BridgeResult<()> {
        debug!("Handling subscription for connection {}: {:?}", connection_id, message_types);

        // TODO: Implement subscription logic
        // This would involve registering the connection to receive specific message types
        // from the Valkyrie engine

        let status = ValkyrieWebSocketMessage::Status {
            connection_id: connection_id.to_string(),
            state: "subscribed".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("message_types".to_string(), message_types.join(","));
                if let Some(f) = filters {
                    meta.insert("filters".to_string(), serde_json::to_string(&f).unwrap_or_default());
                }
                meta
            },
        };

        self.send_valkyrie_message(connection_id, status).await?;
        Ok(())
    }

    /// Handle unsubscription from message types
    async fn handle_unsubscribe(
        &self,
        connection_id: Uuid,
        message_types: Vec<String>,
    ) -> BridgeResult<()> {
        debug!("Handling unsubscription for connection {}: {:?}", connection_id, message_types);

        // TODO: Implement unsubscription logic

        let status = ValkyrieWebSocketMessage::Status {
            connection_id: connection_id.to_string(),
            state: "unsubscribed".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("message_types".to_string(), message_types.join(","));
                meta
            },
        };

        self.send_valkyrie_message(connection_id, status).await?;
        Ok(())
    }

    /// Handle sending a message through Valkyrie
    async fn handle_send_message(
        &self,
        connection_id: Uuid,
        message: serde_json::Value,
        target: Option<String>,
    ) -> BridgeResult<()> {
        debug!("Handling send message from connection {}", connection_id);

        // TODO: Convert JSON message to ValkyrieMessage and send through engine
        // For now, echo the message back
        let echo = ValkyrieWebSocketMessage::Receive {
            message,
            source: connection_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        self.send_valkyrie_message(connection_id, echo).await?;
        Ok(())
    }

    /// Send a Valkyrie WebSocket message to a connection
    async fn send_valkyrie_message(
        &self,
        connection_id: Uuid,
        message: ValkyrieWebSocketMessage,
    ) -> BridgeResult<()> {
        let json = serde_json::to_string(&message)
            .map_err(|e| BridgeError::InternalError {
                message: format!("Failed to serialize message: {}", e),
            })?;

        self.send_message(connection_id, WsMessage::Text(json)).await
    }

    /// Send a WebSocket message to a connection
    async fn send_message(
        &self,
        connection_id: Uuid,
        message: WsMessage,
    ) -> BridgeResult<()> {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&connection_id) {
            if let Err(e) = connection.sender.send(message) {
                error!("Failed to send message to connection {}: {}", connection_id, e);
                return Err(BridgeError::InternalError {
                    message: format!("Failed to send message: {}", e),
                });
            }
            self.metrics.messages_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            return Err(BridgeError::InternalError {
                message: format!("Connection not found: {}", connection_id),
            });
        }

        Ok(())
    }

    /// Send heartbeat to a connection
    async fn send_heartbeat(&self, connection_id: Uuid) -> BridgeResult<()> {
        let heartbeat = ValkyrieWebSocketMessage::Ping {
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        self.send_valkyrie_message(connection_id, heartbeat).await
    }

    /// Update connection activity timestamp
    async fn update_connection_activity(&self, connection_id: Uuid) {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(&connection_id) {
            connection.last_activity = Instant::now();
        }
    }

    /// Close a connection
    async fn close_connection(&self, connection_id: Uuid) -> BridgeResult<()> {
        debug!("Closing connection {}", connection_id);

        {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.state = ConnectionState::Closing;
            }
        }

        // Send close message
        self.send_message(connection_id, WsMessage::Close(None)).await?;
        
        Ok(())
    }

    /// Clean up a connection
    async fn cleanup_connection(&self, connection_id: Uuid) {
        info!("Cleaning up connection {}", connection_id);

        {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id);
        }

        self.metrics.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get connection metrics
    pub async fn get_metrics(&self) -> HashMap<String, u64> {
        let mut metrics = HashMap::new();
        metrics.insert("total_connections".to_string(), 
                      self.metrics.total_connections.load(std::sync::atomic::Ordering::Relaxed));
        metrics.insert("active_connections".to_string(), 
                      self.metrics.active_connections.load(std::sync::atomic::Ordering::Relaxed));
        metrics.insert("messages_sent".to_string(), 
                      self.metrics.messages_sent.load(std::sync::atomic::Ordering::Relaxed));
        metrics.insert("messages_received".to_string(), 
                      self.metrics.messages_received.load(std::sync::atomic::Ordering::Relaxed));
        metrics.insert("connection_errors".to_string(), 
                      self.metrics.connection_errors.load(std::sync::atomic::Ordering::Relaxed));
        metrics.insert("avg_processing_time_ms".to_string(), 
                      self.metrics.avg_processing_time_ms.load(std::sync::atomic::Ordering::Relaxed));
        metrics
    }

    /// Clean up expired connections
    pub async fn cleanup_expired_connections(&self) {
        let mut connections = self.connections.write().await;
        let now = Instant::now();
        let timeout = Duration::from_secs(self.config.connection_timeout_seconds);

        connections.retain(|connection_id, connection| {
            let expired = now.duration_since(connection.last_activity) > timeout;
            if expired {
                info!("Removing expired connection: {}", connection_id);
                self.metrics.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            !expired
        });
    }
}

impl Clone for WebSocketUpgradeHandler {
    fn clone(&self) -> Self {
        Self {
            valkyrie_engine: self.valkyrie_engine.clone(),
            connections: self.connections.clone(),
            config: self.config.clone(),
            metrics: self.metrics.clone(),
        }
    }
}