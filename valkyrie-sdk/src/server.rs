//! High-level server for Valkyrie Protocol

use crate::{Result, ValkyrieError, Message};
use valkyrie_protocol::{ValkyrieServer, server::ServerConfig as ProtocolServerConfig, core::engine::MessageHandler as ProtocolMessageHandler};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, info, error};
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

/// High-level message handler trait
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle a message with string payload
    async fn handle(&self, message: String) -> Result<String>;
}

/// JSON message handler trait
#[async_trait]
pub trait JsonMessageHandler<T, R>: Send + Sync
where
    T: DeserializeOwned + Send + Sync,
    R: Serialize + Send + Sync,
{
    /// Handle a message with JSON payload
    async fn handle_json(&self, request: T) -> Result<R>;
}

/// High-level server configuration
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
    /// Enable metrics collection
    pub enable_metrics: bool,
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
            enable_metrics: false,
        }
    }
}

/// High-level Valkyrie server
pub struct Server {
    config: ServerConfig,
    server: Arc<ValkyrieServer>,
    handlers: Arc<tokio::sync::RwLock<HashMap<String, Arc<dyn MessageHandler>>>>,
}

impl Server {
    /// Create a new server with configuration
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let protocol_config = ProtocolServerConfig {
            bind_address: config.bind_address.clone(),
            port: config.port,
            max_connections: config.max_connections,
            connection_timeout_ms: config.connection_timeout_ms,
            enable_tls: config.enable_tls,
            tls_cert_path: config.tls_cert_path.clone(),
            tls_key_path: config.tls_key_path.clone(),
        };
        
        let server = Arc::new(ValkyrieServer::with_config(protocol_config));
        
        Ok(Self {
            config,
            server,
            handlers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }
    
    /// Register a message handler for an endpoint
    pub async fn register_handler<S: Into<String>>(&self, endpoint: S, handler: Arc<dyn MessageHandler>) -> Result<()> {
        let endpoint = endpoint.into();
        info!("Registering handler for endpoint: {}", endpoint);
        
        // Store the handler
        {
            let mut handlers = self.handlers.write().await;
            handlers.insert(endpoint.clone(), handler);
        }
        
        // Create a protocol handler wrapper
        let handlers_ref = Arc::clone(&self.handlers);
        let protocol_handler = Arc::new(ProtocolHandlerWrapper {
            endpoint: endpoint.clone(),
            handlers: handlers_ref,
        });
        
        self.server.register_handler(endpoint, protocol_handler).await;
        Ok(())
    }
    
    /// Register a JSON message handler
    pub async fn register_json_handler<S, T, R, H>(&self, endpoint: S, handler: H) -> Result<()>
    where
        S: Into<String>,
        T: DeserializeOwned + Send + Sync + 'static,
        R: Serialize + Send + Sync + 'static,
        H: JsonMessageHandler<T, R> + 'static,
    {
        let json_handler = Arc::new(JsonHandlerWrapper {
            handler: Arc::new(handler),
            _phantom: std::marker::PhantomData,
        });
        
        self.register_handler(endpoint, json_handler).await
    }
    
    /// Start the server
    pub async fn start(&self) -> Result<()> {
        info!("Starting Valkyrie server on {}:{}", self.config.bind_address, self.config.port);
        self.server.start().await
    }
    
    /// Stop the server
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Valkyrie server");
        self.server.stop().await
    }
    
    /// Check if server is running
    pub async fn is_running(&self) -> bool {
        self.server.is_running().await
    }
    
    /// Get server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
    
    /// Get server statistics
    pub async fn stats(&self) -> ServerStats {
        let protocol_stats = self.server.stats().await;
        let handlers = self.handlers.read().await;
        
        ServerStats {
            active_connections: protocol_stats.active_connections,
            max_connections: protocol_stats.max_connections,
            registered_handlers: handlers.len(),
            is_running: protocol_stats.is_running,
        }
    }
}

/// Wrapper to adapt high-level MessageHandler to protocol MessageHandler
struct ProtocolHandlerWrapper {
    endpoint: String,
    handlers: Arc<tokio::sync::RwLock<HashMap<String, Arc<dyn MessageHandler>>>>,
}

#[async_trait]
impl ProtocolMessageHandler for ProtocolHandlerWrapper {
    async fn handle_message(&self, message: Message) -> Result<Option<Message>> {
        debug!("Handling message {} for endpoint {}", message.id, self.endpoint);
        
        let handler = {
            let handlers = self.handlers.read().await;
            handlers.get(&self.endpoint).cloned()
        };
        
        if let Some(handler) = handler {
            // Convert payload to string
            let input = message.payload_as_string()
                .unwrap_or_else(|| String::from_utf8_lossy(&message.payload).to_string());
            
            match handler.handle(input).await {
                Ok(response) => {
                    if message.message_type == crate::MessageType::Request {
                        // Create response message
                        let response_msg = Message::response(
                            response.into_bytes(),
                            message.correlation_id.unwrap_or(message.id)
                        );
                        Ok(Some(response_msg))
                    } else {
                        Ok(None)
                    }
                }
                Err(e) => {
                    error!("Handler error for endpoint {}: {}", self.endpoint, e);
                    if message.message_type == crate::MessageType::Request {
                        let error_msg = Message::response(
                            format!("Error: {}", e).into_bytes(),
                            message.correlation_id.unwrap_or(message.id)
                        );
                        Ok(Some(error_msg))
                    } else {
                        Err(e)
                    }
                }
            }
        } else {
            error!("No handler found for endpoint: {}", self.endpoint);
            Err(ValkyrieError::protocol(format!("No handler for endpoint: {}", self.endpoint)))
        }
    }
}

/// Wrapper for JSON message handlers
struct JsonHandlerWrapper<T, R, H>
where
    T: DeserializeOwned + Send + Sync,
    R: Serialize + Send + Sync,
    H: JsonMessageHandler<T, R>,
{
    handler: Arc<H>,
    _phantom: std::marker::PhantomData<(T, R)>,
}

#[async_trait]
impl<T, R, H> MessageHandler for JsonHandlerWrapper<T, R, H>
where
    T: DeserializeOwned + Send + Sync,
    R: Serialize + Send + Sync,
    H: JsonMessageHandler<T, R>,
{
    async fn handle(&self, message: String) -> Result<String> {
        let request: T = serde_json::from_str(&message)
            .map_err(|e| ValkyrieError::protocol(format!("Deserialization failed: {}", e)))?;
        
        let response = self.handler.handle_json(request).await?;
        
        serde_json::to_string(&response)
            .map_err(|e| ValkyrieError::protocol(format!("Serialization failed: {}", e)))
    }
}

/// Builder for creating Valkyrie servers
pub struct ServerBuilder {
    config: ServerConfig,
    handlers: Vec<(String, Arc<dyn MessageHandler>)>,
}

impl ServerBuilder {
    /// Create a new server builder
    pub fn new() -> Self {
        Self {
            config: ServerConfig::default(),
            handlers: Vec::new(),
        }
    }
    
    /// Set bind address and port
    pub fn bind<S: AsRef<str>>(mut self, address: S) -> Self {
        let address = address.as_ref();
        if let Some((host, port)) = address.rsplit_once(':') {
            self.config.bind_address = host.to_string();
            if let Ok(port_num) = port.parse::<u16>() {
                self.config.port = port_num;
            }
        } else {
            self.config.bind_address = address.to_string();
        }
        self
    }
    
    /// Set bind address
    pub fn bind_address<S: Into<String>>(mut self, address: S) -> Self {
        self.config.bind_address = address.into();
        self
    }
    
    /// Set port
    pub fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }
    
    /// Set maximum connections
    pub fn max_connections(mut self, max: usize) -> Self {
        self.config.max_connections = max;
        self
    }
    
    /// Enable TLS with certificate and key files
    pub fn tls<S: Into<String>>(mut self, cert_path: S, key_path: S) -> Self {
        self.config.enable_tls = true;
        self.config.tls_cert_path = Some(cert_path.into());
        self.config.tls_key_path = Some(key_path.into());
        self
    }
    
    /// Add a message handler
    pub fn handler<S: Into<String>, H: MessageHandler + 'static>(mut self, endpoint: S, handler: H) -> Self {
        self.handlers.push((endpoint.into(), Arc::new(handler)));
        self
    }
    
    /// Enable metrics collection
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }
    
    /// Build the server
    pub async fn build(self) -> Result<Server> {
        info!("Building Valkyrie server on {}:{}", self.config.bind_address, self.config.port);
        
        let server = Server::new(self.config).await?;
        
        // Register all handlers
        for (endpoint, handler) in self.handlers {
            server.register_handler(endpoint, handler).await?;
        }
        
        Ok(server)
    }
}

impl Default for ServerBuilder {
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
    /// Number of registered handlers
    pub registered_handlers: usize,
    /// Whether server is running
    pub is_running: bool,
}