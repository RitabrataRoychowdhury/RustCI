//! Valkyrie Protocol Client

use crate::{Message, Result, ValkyrieError, MessageType};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};
use uuid::Uuid;
use std::collections::HashMap;
use std::time::Duration;

/// Configuration for Valkyrie client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Maximum number of concurrent requests
    pub max_concurrent_requests: usize,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Reconnection interval in milliseconds
    pub reconnect_interval_ms: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_concurrent_requests: 1000,
            auto_reconnect: true,
            reconnect_interval_ms: 1000,
        }
    }
}

/// Valkyrie Protocol Client
pub struct ValkyrieClient {
    config: ClientConfig,
    endpoint: String,
    client_id: String,
    connected: Arc<RwLock<bool>>,
    pending_requests: Arc<RwLock<HashMap<Uuid, mpsc::Sender<Message>>>>,
}

impl ValkyrieClient {
    /// Create a new client with default configuration
    pub async fn connect<S: Into<String>>(endpoint: S) -> Result<Self> {
        Self::connect_with_config(endpoint, ClientConfig::default()).await
    }
    
    /// Create a new client with custom configuration
    pub async fn connect_with_config<S: Into<String>>(endpoint: S, config: ClientConfig) -> Result<Self> {
        let endpoint = endpoint.into();
        let client_id = format!("valkyrie-client-{}", Uuid::new_v4());
        
        info!("Connecting Valkyrie client {} to {}", client_id, endpoint);
        
        let client = Self {
            config,
            endpoint,
            client_id,
            connected: Arc::new(RwLock::new(false)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        };
        
        client.establish_connection().await?;
        
        Ok(client)
    }
    
    /// Send a request message and wait for response
    pub async fn request(&self, mut message: Message) -> Result<Message> {
        self.ensure_connected().await?;
        
        // Set up request message
        message.message_type = MessageType::Request;
        message.source = self.client_id.clone();
        let correlation_id = message.correlation_id.unwrap_or_else(|| Uuid::new_v4());
        message.correlation_id = Some(correlation_id);
        
        debug!("Sending request {} with correlation ID {}", message.id, correlation_id);
        
        // Create response channel
        let (response_tx, mut response_rx) = mpsc::channel(1);
        
        // Register pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(correlation_id, response_tx);
        }
        
        // Send the message
        self.send_internal(message).await?;
        
        // Wait for response with timeout
        let timeout = Duration::from_millis(self.config.request_timeout_ms);
        
        match tokio::time::timeout(timeout, response_rx.recv()).await {
            Ok(Some(response)) => {
                debug!("Received response for correlation ID {}", correlation_id);
                
                // Clean up pending request
                {
                    let mut pending = self.pending_requests.write().await;
                    pending.remove(&correlation_id);
                }
                
                Ok(response)
            }
            Ok(None) => {
                error!("Response channel closed for correlation ID {}", correlation_id);
                Err(ValkyrieError::protocol("Response channel closed"))
            }
            Err(_) => {
                error!("Request timeout for correlation ID {}", correlation_id);
                
                // Clean up pending request
                {
                    let mut pending = self.pending_requests.write().await;
                    pending.remove(&correlation_id);
                }
                
                Err(ValkyrieError::timeout(self.config.request_timeout_ms))
            }
        }
    }
    
    /// Send a notification message (no response expected)
    pub async fn notify(&self, mut message: Message) -> Result<()> {
        self.ensure_connected().await?;
        
        message.message_type = MessageType::Notification;
        message.source = self.client_id.clone();
        
        debug!("Sending notification {}", message.id);
        
        self.send_internal(message).await
    }
    
    /// Send a message without waiting for response
    pub async fn send(&self, mut message: Message) -> Result<()> {
        self.ensure_connected().await?;
        
        message.source = self.client_id.clone();
        
        debug!("Sending message {}", message.id);
        
        self.send_internal(message).await
    }
    
    /// Check if client is connected
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    /// Disconnect the client
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting Valkyrie client {}", self.client_id);
        
        {
            let mut connected = self.connected.write().await;
            *connected = false;
        }
        
        // Cancel all pending requests
        {
            let mut pending = self.pending_requests.write().await;
            pending.clear();
        }
        
        Ok(())
    }
    
    /// Get client configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
    
    /// Get client ID
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    
    /// Get endpoint
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    
    /// Establish connection to the server
    async fn establish_connection(&self) -> Result<()> {
        debug!("Establishing connection to {}", self.endpoint);
        
        // TODO: Implement actual connection logic based on transport type
        // For now, just mark as connected
        {
            let mut connected = self.connected.write().await;
            *connected = true;
        }
        
        info!("Connected to {}", self.endpoint);
        Ok(())
    }
    
    /// Ensure client is connected
    async fn ensure_connected(&self) -> Result<()> {
        if !self.is_connected().await {
            if self.config.auto_reconnect {
                self.establish_connection().await?;
            } else {
                return Err(ValkyrieError::connection("Client not connected"));
            }
        }
        Ok(())
    }
    
    /// Internal message sending implementation
    async fn send_internal(&self, message: Message) -> Result<()> {
        // TODO: Implement actual message sending based on transport
        debug!("Sending message {} to {}", message.id, message.destination);
        Ok(())
    }
    
    /// Handle incoming response message
    async fn handle_response(&self, message: Message) -> Result<()> {
        if let Some(correlation_id) = message.correlation_id {
            debug!("Handling response for correlation ID {}", correlation_id);
            
            let sender = {
                let pending = self.pending_requests.read().await;
                pending.get(&correlation_id).cloned()
            };
            
            if let Some(sender) = sender {
                if let Err(_) = sender.send(message).await {
                    error!("Failed to send response to waiting request");
                }
            } else {
                debug!("No pending request found for correlation ID {}", correlation_id);
            }
        }
        
        Ok(())
    }
}