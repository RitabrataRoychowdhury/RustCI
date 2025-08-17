//! Connection management for Valkyrie Protocol

use crate::{Message, Result, ValkyrieError};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};
use std::collections::HashMap;


/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection is active and ready
    Connected,
    /// Connection is being closed
    Disconnecting,
    /// Connection is closed
    Disconnected,
    /// Connection failed
    Failed(String),
}

/// Connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Unique connection identifier
    pub id: String,
    /// Remote endpoint address
    pub remote_address: String,
    /// Connection state
    pub state: ConnectionState,
    /// Connection established timestamp
    pub established_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
}

/// Generic connection trait
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    /// Get connection ID
    fn id(&self) -> &str;
    
    /// Get connection info
    fn info(&self) -> &ConnectionInfo;
    
    /// Send a message
    async fn send(&self, message: Message) -> Result<()>;
    
    /// Receive a message (non-blocking)
    async fn try_recv(&self) -> Result<Option<Message>>;
    
    /// Close the connection
    async fn close(&self) -> Result<()>;
    
    /// Check if connection is active
    fn is_active(&self) -> bool {
        matches!(self.info().state, ConnectionState::Connected)
    }
}

/// Connection manager for handling multiple connections
pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<String, Arc<dyn Connection>>>>,
    max_connections: usize,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(max_connections: usize) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections,
        }
    }
    
    /// Add a new connection
    pub async fn add_connection(&self, connection: Arc<dyn Connection>) -> Result<()> {
        let connection_id = connection.id().to_string();
        debug!("Adding connection: {}", connection_id);
        
        let mut connections = self.connections.write().await;
        
        if connections.len() >= self.max_connections {
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
        if let Some(connection) = connections.remove(connection_id) {
            if let Err(e) = connection.close().await {
                error!("Failed to close connection {}: {}", connection_id, e);
            }
        }
        
        Ok(())
    }
    
    /// Get a connection by ID
    pub async fn get_connection<S: AsRef<str>>(&self, connection_id: S) -> Option<Arc<dyn Connection>> {
        let connections = self.connections.read().await;
        connections.get(connection_id.as_ref()).cloned()
    }
    
    /// Get all active connections
    pub async fn get_active_connections(&self) -> Vec<Arc<dyn Connection>> {
        let connections = self.connections.read().await;
        connections.values()
            .filter(|conn| conn.is_active())
            .cloned()
            .collect()
    }
    
    /// Broadcast a message to all active connections
    pub async fn broadcast(&self, message: Message) -> Result<Vec<String>> {
        debug!("Broadcasting message {} to all connections", message.id);
        
        let connections = self.get_active_connections().await;
        let mut errors = Vec::new();
        
        for connection in connections {
            if let Err(e) = connection.send(message.clone()).await {
                errors.push(format!("Connection {}: {}", connection.id(), e));
            }
        }
        
        if !errors.is_empty() {
            error!("Broadcast errors: {:?}", errors);
        }
        
        Ok(errors)
    }
    
    /// Send a message to a specific connection
    pub async fn send_to_connection<S: AsRef<str>>(&self, connection_id: S, message: Message) -> Result<()> {
        let connection_id = connection_id.as_ref();
        
        if let Some(connection) = self.get_connection(connection_id).await {
            connection.send(message).await
        } else {
            Err(ValkyrieError::connection(format!("Connection {} not found", connection_id)))
        }
    }
    
    /// Get connection statistics
    pub async fn stats(&self) -> ConnectionManagerStats {
        let connections = self.connections.read().await;
        let active_count = connections.values()
            .filter(|conn| conn.is_active())
            .count();
        
        ConnectionManagerStats {
            total_connections: connections.len(),
            active_connections: active_count,
            max_connections: self.max_connections,
        }
    }
    
    /// Clean up inactive connections
    pub async fn cleanup_inactive(&self) -> Result<usize> {
        debug!("Cleaning up inactive connections");
        
        let inactive_ids: Vec<String> = {
            let connections = self.connections.read().await;
            connections.iter()
                .filter(|(_, conn)| !conn.is_active())
                .map(|(id, _)| id.clone())
                .collect()
        };
        
        let count = inactive_ids.len();
        for id in inactive_ids {
            self.remove_connection(&id).await?;
        }
        
        debug!("Cleaned up {} inactive connections", count);
        Ok(count)
    }
}

/// Connection manager statistics
#[derive(Debug, Clone)]
pub struct ConnectionManagerStats {
    /// Total number of connections
    pub total_connections: usize,
    /// Number of active connections
    pub active_connections: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
}