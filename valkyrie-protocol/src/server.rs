//! Valkyrie Protocol Server

use crate::{Message, Result, ValkyrieError, core::engine::{ValkyrieEngine, MessageHandler}};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use std::collections::HashMap;

/// Configuration for Valkyrie server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_address: String,
    /// Server port
    pub port: u16,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
    /// Enable TLS
    pub enable_tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS private key path
    pub tls_key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 10000,
            connection_timeout_ms: 30000,
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

/// Valkyrie Protocol Server
pub struct ValkyrieServer {
    config: ServerConfig,
    engine: Arc<ValkyrieEngine>,
    running: Arc<RwLock<bool>>,
    connections: Arc<RwLock<HashMap<String, Arc<dyn Connection>>>>,
}

/// Connection trait for managing client connections
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    /// Get connection ID
    fn id(&self) -> &str;
    
    /// Send a message to the client
    async fn send_message(&self, message: Message) -> Result<()>;
    
    /// Close the connection
    async fn close(&self) -> Result<()>;
    
    /// Check if connection is active
    fn is_active(&self) -> bool;
}

impl ValkyrieServer {
    /// Create a new server with default configuration
    pub fn new() -> Self {
        Self::with_config(ServerConfig::default())
    }
    
    /// Create a new server with custom configuration
    pub fn with_config(config: ServerConfig) -> Self {
        let engine = Arc::new(ValkyrieEngine::new());
        
        Self {
            config,
            engine,
            running: Arc::new(RwLock::new(false)),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a message handler for a specific endpoint
    pub async fn register_handler<S: Into<String>>(&self, endpoint: S, handler: Arc<dyn MessageHandler>) {
        self.engine.register_handler(endpoint, handler).await;
    }
    
    /// Unregister a message handler
    pub async fn unregister_handler<S: AsRef<str>>(&self, endpoint: S) {
        self.engine.unregister_handler(endpoint).await;
    }
    
    /// Start the server
    pub async fn start(&self) -> Result<()> {
        info!("Starting Valkyrie server on {}:{}", self.config.bind_address, self.config.port);
        
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(ValkyrieError::protocol("Server already running"));
            }
            *running = true;
        }
        
        // Start the engine
        self.engine.start().await?;
        
        // TODO: Start actual network listeners based on transport types
        self.start_listeners().await?;
        
        info!("Valkyrie server started successfully");
        Ok(())
    }
    
    /// Stop the server
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Valkyrie server");
        
        {
            let mut running = self.running.write().await;
            if !*running {
                return Ok(());
            }
            *running = false;
        }
        
        // Close all connections
        {
            let connections = self.connections.read().await;
            for connection in connections.values() {
                if let Err(e) = connection.close().await {
                    error!("Failed to close connection {}: {}", connection.id(), e);
                }
            }
        }
        
        // Stop the engine
        self.engine.stop().await?;
        
        info!("Valkyrie server stopped");
        Ok(())
    }
    
    /// Check if server is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
    
    /// Get server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
    
    /// Get server statistics
    pub async fn stats(&self) -> ServerStats {
        let connections = self.connections.read().await;
        let engine_stats = self.engine.stats().await;
        
        ServerStats {
            active_connections: connections.len(),
            max_connections: self.config.max_connections,
            registered_handlers: engine_stats.registered_handlers,
            is_running: self.is_running().await,
        }
    }
    
    /// Broadcast a message to all connected clients
    pub async fn broadcast(&self, message: Message) -> Result<()> {
        debug!("Broadcasting message {} to all connections", message.id);
        
        let connections = self.connections.read().await;
        let mut errors = Vec::new();
        
        for connection in connections.values() {
            if let Err(e) = connection.send_message(message.clone()).await {
                errors.push(format!("Connection {}: {}", connection.id(), e));
            }
        }
        
        if !errors.is_empty() {
            error!("Broadcast errors: {:?}", errors);
        }
        
        Ok(())
    }
    
    /// Send a message to a specific client
    pub async fn send_to_client<S: AsRef<str>>(&self, client_id: S, message: Message) -> Result<()> {
        let client_id = client_id.as_ref();
        debug!("Sending message {} to client {}", message.id, client_id);
        
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(client_id) {
            connection.send_message(message).await
        } else {
            Err(ValkyrieError::connection(format!("Client {} not found", client_id)))
        }
    }
    
    /// Add a new connection
    pub async fn add_connection(&self, connection: Arc<dyn Connection>) -> Result<()> {
        let connection_id = connection.id().to_string();
        debug!("Adding connection: {}", connection_id);
        
        let mut connections = self.connections.write().await;
        if connections.len() >= self.config.max_connections {
            return Err(ValkyrieError::connection("Maximum connections reached"));
        }
        
        connections.insert(connection_id, connection);
        Ok(())
    }
    
    /// Remove a connection
    pub async fn remove_connection<S: AsRef<str>>(&self, connection_id: S) -> Result<()> {
        let connection_id = connection_id.as_ref();
        debug!("Removing connection: {}", connection_id);
        
        let mut connections = self.connections.write().await;
        connections.remove(connection_id);
        Ok(())
    }
    
    /// Start network listeners
    async fn start_listeners(&self) -> Result<()> {
        // TODO: Implement actual network listeners based on configuration
        // This would include TCP, QUIC, Unix socket, WebSocket listeners
        debug!("Starting network listeners");
        Ok(())
    }
}

impl Default for ValkyrieServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    /// Number of active connections
    pub active_connections: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
    /// Number of registered message handlers
    pub registered_handlers: usize,
    /// Whether server is running
    pub is_running: bool,
}