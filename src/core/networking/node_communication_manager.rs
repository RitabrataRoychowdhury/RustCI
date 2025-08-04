use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::interval;
use uuid::Uuid;

use crate::core::networking::node_communication::{
    NodeId, ProtocolMessage, MessagePayload, NodeMessage, ControlPlaneMessage,
    NodeInfo, ProtocolError
};
use crate::core::networking::transport::{Transport, Connection, TransportConfig, TransportEndpoint, TransportType, AuthenticationConfig, AuthMethod, TimeoutConfig, BufferConfig};
use crate::core::networking::secure_transport::{AuthenticationManager, SecurityAuditor, SecurityEventType, SecuritySeverity};
use crate::error::Result;

/// Node communication manager handles all node-to-control-plane communication
pub struct NodeCommunicationManager {
    /// Transport layer for communication
    transport: Arc<dyn Transport>,
    /// Authentication manager
    auth_manager: Arc<AuthenticationManager>,
    /// Active connections to nodes
    connections: Arc<RwLock<HashMap<NodeId, Arc<Mutex<Box<dyn Connection>>>>>>,
    /// Message handlers
    message_handlers: Arc<RwLock<HashMap<String, Box<dyn MessageHandler>>>>,
    /// Event channels
    event_tx: mpsc::UnboundedSender<CommunicationEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<CommunicationEvent>>>,
    /// Configuration
    config: CommunicationConfig,
    /// Security auditor
    auditor: Arc<SecurityAuditor>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl NodeCommunicationManager {
    pub fn new(
        transport: Arc<dyn Transport>,
        auth_manager: Arc<AuthenticationManager>,
        config: CommunicationConfig,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let auditor = Arc::new(SecurityAuditor::new(10000));
        
        Self {
            transport,
            auth_manager,
            connections: Arc::new(RwLock::new(HashMap::new())),
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            config,
            auditor,
            shutdown_tx: None,
        }
    }
    
    /// Start the communication manager
    pub async fn start(&mut self) -> Result<()> {
        // Start listening for incoming connections
        self.transport.listen(&self.config.transport_config).await?;
        
        // Start background tasks
        let (shutdown_tx, _shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start event processing loop
        let (_event_shutdown_tx, event_shutdown_rx) = mpsc::channel(1);
        self.start_event_loop(event_shutdown_rx).await;
        
        // Start heartbeat monitoring
        let (_heartbeat_shutdown_tx, heartbeat_shutdown_rx) = mpsc::channel(1);
        self.start_heartbeat_monitor(heartbeat_shutdown_rx).await;
        
        // Start connection cleanup
        let (_cleanup_shutdown_tx, cleanup_shutdown_rx) = mpsc::channel(1);
        self.start_connection_cleanup(cleanup_shutdown_rx).await;
        
        Ok(())
    }
    
    /// Stop the communication manager
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
        
        // Close all connections
        let mut connections = self.connections.write().await;
        for (node_id, connection) in connections.drain() {
            let mut conn = connection.lock().await;
            let _ = conn.close().await;
            
            self.auditor.log_event(
                SecurityEventType::ConnectionClosed,
                Some(node_id),
                "control-plane".to_string(),
                HashMap::new(),
                SecuritySeverity::Info,
            ).await;
        }
        
        // Shutdown transport
        self.transport.shutdown().await?;
        
        Ok(())
    }
    
    /// Register a message handler
    pub async fn register_handler<H>(&self, message_type: String, handler: H)
    where
        H: MessageHandler + 'static,
    {
        self.message_handlers.write().await.insert(message_type, Box::new(handler));
    }
    
    /// Send a message to a specific node
    pub async fn send_to_node(&self, node_id: NodeId, message: ControlPlaneMessage) -> Result<()> {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&node_id) {
            let mut conn = connection.lock().await;
            
            let protocol_message = ProtocolMessage::new(
                Uuid::new_v4(), // Control plane ID
                MessagePayload::ControlPlaneMessage(message),
            ).with_destination(node_id);
            
            conn.send(&protocol_message).await?;
            
            self.event_tx.send(CommunicationEvent::MessageSent {
                node_id,
                message_id: protocol_message.id,
            }).map_err(|_| ProtocolError::ConnectionError {
                message: "Failed to send event".to_string(),
            })?;
        } else {
            return Err(ProtocolError::NodeNotRegistered { node_id }.into());
        }
        
        Ok(())
    }
    
    /// Broadcast a message to all connected nodes
    pub async fn broadcast(&self, message: ControlPlaneMessage) -> Result<()> {
        let connections = self.connections.read().await;
        let mut failed_nodes = Vec::new();
        
        for (node_id, connection) in connections.iter() {
            let mut conn = connection.lock().await;
            
            let protocol_message = ProtocolMessage::new(
                Uuid::new_v4(), // Control plane ID
                MessagePayload::ControlPlaneMessage(message.clone()),
            ).with_destination(*node_id);
            
            if let Err(e) = conn.send(&protocol_message).await {
                failed_nodes.push((*node_id, e));
            } else {
                let _ = self.event_tx.send(CommunicationEvent::MessageSent {
                    node_id: *node_id,
                    message_id: protocol_message.id,
                });
            }
        }
        
        if !failed_nodes.is_empty() {
            // Log failed broadcasts but don't fail the entire operation
            for (node_id, error) in failed_nodes {
                self.auditor.log_event(
                    SecurityEventType::SuspiciousActivity,
                    Some(node_id),
                    "control-plane".to_string(),
                    [("error".to_string(), error.to_string())].into(),
                    SecuritySeverity::Warning,
                ).await;
            }
        }
        
        Ok(())
    }
    
    /// Connect to a remote node
    pub async fn connect_to_node(&self, endpoint: TransportEndpoint) -> Result<NodeId> {
        let connection = self.transport.connect(&endpoint).await?;
        
        // For outbound connections, we need to wait for the node to register
        // This is a simplified implementation
        let node_id = Uuid::new_v4(); // In practice, this would come from the registration
        
        self.connections.write().await.insert(node_id, Arc::new(Mutex::new(connection)));
        
        self.auditor.log_event(
            SecurityEventType::ConnectionEstablished,
            Some(node_id),
            endpoint.address.clone(),
            HashMap::new(),
            SecuritySeverity::Info,
        ).await;
        
        Ok(node_id)
    }
    
    /// Get communication statistics
    pub async fn get_stats(&self) -> CommunicationStats {
        let connections = self.connections.read().await;
        let transport_metrics = self.transport.get_metrics().await;
        
        CommunicationStats {
            active_connections: connections.len() as u32,
            total_messages_sent: transport_metrics.messages_sent,
            total_messages_received: transport_metrics.messages_received,
            total_bytes_sent: transport_metrics.bytes_sent,
            total_bytes_received: transport_metrics.bytes_received,
            connection_errors: transport_metrics.connection_errors,
            authentication_failures: transport_metrics.authentication_failures,
        }
    }
    
    /// Start the event processing loop
    async fn start_event_loop(&self, mut shutdown_rx: mpsc::Receiver<()>) {
        let event_rx = self.event_rx.clone();
        let handlers = self.message_handlers.clone();
        let connections = self.connections.clone();
        let auditor = self.auditor.clone();
        
        tokio::spawn(async move {
            let mut event_rx = event_rx.lock().await;
            
            loop {
                tokio::select! {
                    event = event_rx.recv() => {
                        if let Some(event) = event {
                            match event {
                                CommunicationEvent::MessageReceived { node_id, message } => {
                                    // Handle incoming message
                                    let handlers = handlers.read().await;
                                    
                                    match &message.message {
                                        MessagePayload::NodeMessage(node_msg) => {
                                            let message_type = match node_msg {
                                                NodeMessage::RegisterNode { .. } => "register_node",
                                                NodeMessage::Heartbeat { .. } => "heartbeat",
                                                NodeMessage::JobResult { .. } => "job_result",
                                                NodeMessage::ShutdownNotice { .. } => "shutdown_notice",
                                                NodeMessage::JobStatusUpdate { .. } => "job_status_update",
                                            };
                                            
                                            if let Some(handler) = handlers.get(message_type) {
                                                if let Err(e) = handler.handle_message(node_id, node_msg.clone()).await {
                                                    auditor.log_event(
                                                        SecurityEventType::SuspiciousActivity,
                                                        Some(node_id),
                                                        "unknown".to_string(),
                                                        [("error".to_string(), e.to_string())].into(),
                                                        SecuritySeverity::Error,
                                                    ).await;
                                                }
                                            }
                                        }
                                        _ => {
                                            // Unexpected message type from node
                                        }
                                    }
                                }
                                CommunicationEvent::NodeDisconnected { node_id } => {
                                    // Remove disconnected node
                                    connections.write().await.remove(&node_id);
                                    
                                    auditor.log_event(
                                        SecurityEventType::ConnectionClosed,
                                        Some(node_id),
                                        "unknown".to_string(),
                                        HashMap::new(),
                                        SecuritySeverity::Info,
                                    ).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
    }
    
    /// Start heartbeat monitoring
    async fn start_heartbeat_monitor(&self, mut shutdown_rx: mpsc::Receiver<()>) {
        let connections = self.connections.clone();
        let event_tx = self.event_tx.clone();
        let _heartbeat_timeout = self.config.heartbeat_timeout;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check for nodes that haven't sent heartbeats
                        let connections_guard = connections.read().await;
                        let mut disconnected_nodes = Vec::new();
                        
                        for (node_id, connection) in connections_guard.iter() {
                            let conn = connection.lock().await;
                            if !conn.is_connected() {
                                disconnected_nodes.push(*node_id);
                            }
                        }
                        
                        drop(connections_guard);
                        
                        // Remove disconnected nodes
                        for node_id in disconnected_nodes {
                            connections.write().await.remove(&node_id);
                            let _ = event_tx.send(CommunicationEvent::NodeDisconnected { node_id });
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
    }
    
    /// Start connection cleanup task
    async fn start_connection_cleanup(&self, mut shutdown_rx: mpsc::Receiver<()>) {
        let auth_manager = self.auth_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Clean up expired tokens
                        let _ = auth_manager.cleanup_expired_tokens().await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
    }
}

/// Message handler trait for processing incoming messages
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle_message(&self, node_id: NodeId, message: NodeMessage) -> Result<()>;
}

/// Communication events
#[derive(Debug, Clone)]
pub enum CommunicationEvent {
    MessageSent {
        node_id: NodeId,
        message_id: Uuid,
    },
    MessageReceived {
        node_id: NodeId,
        message: Box<ProtocolMessage>,
    },
    NodeConnected {
        node_id: NodeId,
        node_info: NodeInfo,
    },
    NodeDisconnected {
        node_id: NodeId,
    },
    AuthenticationFailed {
        source_ip: String,
        reason: String,
    },
}

/// Communication configuration
#[derive(Debug, Clone)]
pub struct CommunicationConfig {
    pub transport_config: TransportConfig,
    pub heartbeat_timeout: Duration,
    pub message_timeout: Duration,
    pub max_connections: u32,
    pub enable_compression: bool,
    pub enable_encryption: bool,
}

impl Default for CommunicationConfig {
    fn default() -> Self {
        Self {
            transport_config: TransportConfig {
                transport_type: TransportType::Tcp,
                bind_address: "0.0.0.0".to_string(),
                port: Some(8080),
                tls_config: None,
                authentication: AuthenticationConfig {
                    method: AuthMethod::JwtToken,
                    jwt_secret: Some("default-secret".to_string()),
                    token_expiry: Duration::from_secs(3600),
                    require_mutual_auth: false,
                },
                timeouts: TimeoutConfig::default(),
                buffer_sizes: BufferConfig::default(),
            },
            heartbeat_timeout: Duration::from_secs(60),
            message_timeout: Duration::from_secs(30),
            max_connections: 1000,
            enable_compression: true,
            enable_encryption: true,
        }
    }
}

/// Communication statistics
#[derive(Debug, Clone)]
pub struct CommunicationStats {
    pub active_connections: u32,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub connection_errors: u64,
    pub authentication_failures: u64,
}

/// Example message handlers
pub struct RegistrationHandler {
    auth_manager: Arc<AuthenticationManager>,
    event_tx: mpsc::UnboundedSender<CommunicationEvent>,
}

impl RegistrationHandler {
    pub fn new(
        auth_manager: Arc<AuthenticationManager>,
        event_tx: mpsc::UnboundedSender<CommunicationEvent>,
    ) -> Self {
        Self {
            auth_manager,
            event_tx,
        }
    }
}

#[async_trait]
impl MessageHandler for RegistrationHandler {
    async fn handle_message(&self, node_id: NodeId, message: NodeMessage) -> Result<()> {
        if let NodeMessage::RegisterNode { node_info, capabilities, auth_token } = message {
            // Verify the authentication token
            let claims = self.auth_manager.verify_token(&auth_token).await?;
            
            // Send registration event
            self.event_tx.send(CommunicationEvent::NodeConnected {
                node_id: claims.node_id,
                node_info,
            }).map_err(|_| ProtocolError::ConnectionError {
                message: "Failed to send registration event".to_string(),
            })?;
            
            Ok(())
        } else {
            Err(ProtocolError::InvalidMessageFormat {
                details: "Expected RegisterNode message".to_string(),
            }.into())
        }
    }
}

pub struct HeartbeatHandler {
    event_tx: mpsc::UnboundedSender<CommunicationEvent>,
}

impl HeartbeatHandler {
    pub fn new(event_tx: mpsc::UnboundedSender<CommunicationEvent>) -> Self {
        Self { event_tx }
    }
}

#[async_trait]
impl MessageHandler for HeartbeatHandler {
    async fn handle_message(&self, node_id: NodeId, message: NodeMessage) -> Result<()> {
        if let NodeMessage::Heartbeat { status, resources, metrics, .. } = message {
            // Process heartbeat - update node status, resources, metrics
            // This would typically update a node registry or database
            
            Ok(())
        } else {
            Err(ProtocolError::InvalidMessageFormat {
                details: "Expected Heartbeat message".to_string(),
            }.into())
        }
    }
}

pub struct JobResultHandler {
    event_tx: mpsc::UnboundedSender<CommunicationEvent>,
}

impl JobResultHandler {
    pub fn new(event_tx: mpsc::UnboundedSender<CommunicationEvent>) -> Self {
        Self { event_tx }
    }
}

#[async_trait]
impl MessageHandler for JobResultHandler {
    async fn handle_message(&self, node_id: NodeId, message: NodeMessage) -> Result<()> {
        if let NodeMessage::JobResult { job_id, result, metrics, .. } = message {
            // Process job result - update job status, store results, etc.
            // This would typically update a job database or trigger further processing
            
            Ok(())
        } else {
            Err(ProtocolError::InvalidMessageFormat {
                details: "Expected JobResult message".to_string(),
            }.into())
        }
    }
}