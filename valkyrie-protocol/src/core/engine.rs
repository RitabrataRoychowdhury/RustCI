//! Core Valkyrie Protocol Engine

use crate::{Message, Result, ValkyrieError};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use std::collections::HashMap;

/// Configuration for the Valkyrie engine
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Message processing timeout in milliseconds
    pub message_timeout_ms: u64,
    /// Enable message compression
    pub enable_compression: bool,
    /// Enable message encryption
    pub enable_encryption: bool,
    /// Buffer size for message channels
    pub channel_buffer_size: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_connections: 10000,
            message_timeout_ms: 5000,
            enable_compression: true,
            enable_encryption: true,
            channel_buffer_size: 1000,
        }
    }
}

/// Message handler trait for processing incoming messages
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message and optionally return a response
    async fn handle_message(&self, message: Message) -> Result<Option<Message>>;
}

/// Core Valkyrie Protocol Engine
pub struct ValkyrieEngine {
    config: EngineConfig,
    message_handlers: Arc<RwLock<HashMap<String, Arc<dyn MessageHandler>>>>,
    message_tx: mpsc::UnboundedSender<Message>,
    message_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<Message>>>>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Arc<RwLock<Option<mpsc::Receiver<()>>>>,
}

impl ValkyrieEngine {
    /// Create a new Valkyrie engine with default configuration
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }
    
    /// Create a new Valkyrie engine with custom configuration
    pub fn with_config(config: EngineConfig) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        Self {
            config,
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
            message_tx,
            message_rx: Arc::new(RwLock::new(Some(message_rx))),
            shutdown_tx,
            shutdown_rx: Arc::new(RwLock::new(Some(shutdown_rx))),
        }
    }
    
    /// Register a message handler for a specific endpoint
    pub async fn register_handler<S: Into<String>>(&self, endpoint: S, handler: Arc<dyn MessageHandler>) {
        let endpoint = endpoint.into();
        debug!("Registering message handler for endpoint: {}", endpoint);
        
        let mut handlers = self.message_handlers.write().await;
        handlers.insert(endpoint, handler);
    }
    
    /// Unregister a message handler
    pub async fn unregister_handler<S: AsRef<str>>(&self, endpoint: S) {
        let endpoint = endpoint.as_ref();
        debug!("Unregistering message handler for endpoint: {}", endpoint);
        
        let mut handlers = self.message_handlers.write().await;
        handlers.remove(endpoint);
    }
    
    /// Send a message through the engine
    pub async fn send_message(&self, message: Message) -> Result<()> {
        debug!("Sending message: {} to {}", message.id, message.destination);
        
        if message.is_expired() {
            warn!("Message {} has expired, dropping", message.id);
            return Err(ValkyrieError::timeout(message.ttl_ms.unwrap_or(0)));
        }
        
        self.message_tx.send(message)
            .map_err(|_| ValkyrieError::protocol("Failed to send message to processing queue"))?;
        
        Ok(())
    }
    
    /// Start the engine's message processing loop
    pub async fn start(&self) -> Result<()> {
        info!("Starting Valkyrie engine");
        
        let mut message_rx = {
            let mut rx_guard = self.message_rx.write().await;
            rx_guard.take().ok_or_else(|| ValkyrieError::protocol("Engine already started"))?
        };
        
        let mut shutdown_rx = {
            let mut shutdown_guard = self.shutdown_rx.write().await;
            shutdown_guard.take().ok_or_else(|| ValkyrieError::protocol("Engine already started"))?
        };
        
        let handlers = Arc::clone(&self.message_handlers);
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(message) = message_rx.recv() => {
                        Self::process_message(message, Arc::clone(&handlers)).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Shutting down Valkyrie engine");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop the engine
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Valkyrie engine");
        
        self.shutdown_tx.send(()).await
            .map_err(|_| ValkyrieError::protocol("Failed to send shutdown signal"))?;
        
        Ok(())
    }
    
    /// Process a single message
    async fn process_message(
        message: Message,
        handlers: Arc<RwLock<HashMap<String, Arc<dyn MessageHandler>>>>
    ) {
        let destination = message.destination.clone();
        let message_id = message.id;
        
        debug!("Processing message {} for destination {}", message_id, destination);
        
        let handler = {
            let handlers_guard = handlers.read().await;
            handlers_guard.get(&destination).cloned()
        };
        
        if let Some(handler) = handler {
            match handler.handle_message(message).await {
                Ok(Some(_response)) => {
                    debug!("Handler returned response for message {}", message_id);
                    // TODO: Send response back through appropriate channel
                }
                Ok(None) => {
                    debug!("Handler processed message {} without response", message_id);
                }
                Err(e) => {
                    error!("Handler failed to process message {}: {}", message_id, e);
                }
            }
        } else {
            warn!("No handler registered for destination: {}", destination);
        }
    }
    
    /// Get engine configuration
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }
    
    /// Get engine statistics
    pub async fn stats(&self) -> EngineStats {
        let handlers = self.message_handlers.read().await;
        
        EngineStats {
            registered_handlers: handlers.len(),
            max_connections: self.config.max_connections,
            active_connections: 0, // TODO: Track active connections
        }
    }
}

impl Default for ValkyrieEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    /// Number of registered message handlers
    pub registered_handlers: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
    /// Current active connections
    pub active_connections: usize,
}