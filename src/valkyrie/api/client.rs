//! High-level Valkyrie Protocol client for external applications

use std::collections::HashMap;
use crate::valkyrie::{
    ConnectionHandle, Result
};
use crate::valkyrie::api::{ClientConfig, ClientMessage, ValkyrieEngineAdapter};

/// High-level Valkyrie Protocol client for external applications
/// 
/// This is the main entry point for applications wanting to use the Valkyrie Protocol.
/// It provides a simplified, developer-friendly interface that abstracts away
/// the complexity of the underlying protocol implementation.
pub struct ValkyrieClient {
    /// Internal adapter that bridges to the engine
    adapter: ValkyrieEngineAdapter,
    /// Client configuration
    config: ClientConfig,
    /// Active connections
    connections: HashMap<String, ConnectionHandle>,
}

impl ValkyrieClient {
    /// Create a new Valkyrie Protocol client
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let adapter = ValkyrieEngineAdapter::new(config.clone()).await?;
        
        Ok(Self {
            adapter,
            config,
            connections: HashMap::new(),
        })
    }
    
    /// Connect to a remote endpoint
    pub async fn connect(&mut self, endpoint_url: &str) -> Result<String> {
        let connection_id = self.adapter.connect(endpoint_url).await?;
        Ok(connection_id)
    }
    
    /// Send a text message
    pub async fn send_text(&self, connection_id: &str, text: &str) -> Result<()> {
        let message = ClientMessage::text(text);
        self.adapter.send_message(connection_id, message).await
    }
    
    /// Send a binary message
    pub async fn send_binary(&self, connection_id: &str, data: Vec<u8>) -> Result<()> {
        let message = ClientMessage::binary(data);
        self.adapter.send_message(connection_id, message).await
    }
    
    /// Send a JSON message
    pub async fn send_json<T: serde::Serialize>(&self, connection_id: &str, data: &T) -> Result<()> {
        let message = ClientMessage::json(data)?;
        self.adapter.send_message(connection_id, message).await
    }
    
    /// Broadcast a message to multiple connections
    pub async fn broadcast(&self, connection_ids: &[String], message: ClientMessage) -> Result<crate::valkyrie::api::BroadcastResult> {
        self.adapter.broadcast(connection_ids, message).await
    }
    
    /// Close a connection
    pub async fn disconnect(&mut self, connection_id: &str) -> Result<()> {
        self.adapter.disconnect(connection_id).await?;
        self.connections.remove(connection_id);
        Ok(())
    }
    
    /// Get client statistics
    pub async fn get_stats(&self) -> Result<ClientStats> {
        let engine_stats = self.adapter.get_stats().await?;
        Ok(ClientStats {
            active_connections: self.connections.len(),
            engine_stats,
        })
    }
    
    /// Shutdown the client gracefully
    pub async fn shutdown(self) -> Result<()> {
        self.adapter.shutdown().await
    }
}

/// Builder pattern for creating Valkyrie clients
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// Create a new client builder
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }
    
    /// Set the server endpoint
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.config.server_endpoint = endpoint.to_string();
        self
    }
    
    /// Enable TLS
    pub fn with_tls(mut self) -> Self {
        self.config.security.enable_tls = true;
        self
    }
    
    /// Set connection timeout
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.connection_timeout = timeout;
        self
    }
    
    /// Build the client
    pub async fn build(self) -> Result<ValkyrieClient> {
        ValkyrieClient::new(self.config).await
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Client statistics
#[derive(Debug, Clone)]
pub struct ClientStats {
    /// Number of active connections
    pub active_connections: usize,
    /// Engine statistics
    pub engine_stats: crate::valkyrie::core::EngineStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_client_builder() {
        let builder = ClientBuilder::new()
            .with_endpoint("tcp://localhost:8080")
            .with_tls();
        
        // This would need proper mocking to test
        // let result = builder.build().await;
        // assert!(result.is_ok());
    }
}