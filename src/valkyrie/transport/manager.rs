//! Transport Manager for Valkyrie Protocol
//!
//! This module provides the transport manager that handles multiple
//! transport types and connection management.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::valkyrie::core::{ConnectionId, ValkyrieMessage};
use crate::valkyrie::transport::{Connection, Endpoint, Listener, Transport};
use crate::valkyrie::Result;
use crate::valkyrie::config::TransportConfig;

/// Transport manager that handles multiple transport types
pub struct TransportManager {
    /// Configuration
    config: TransportConfig,
    /// Available transports
    transports: Arc<RwLock<HashMap<String, Arc<dyn Transport>>>>,
    /// Active connections
    connections: Arc<RwLock<HashMap<ConnectionId, Box<dyn Connection>>>>,
}

impl TransportManager {
    /// Create a new transport manager
    pub fn new(config: TransportConfig) -> Result<Self> {
        Ok(Self {
            config,
            transports: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start the transport manager
    pub async fn start(&self) -> Result<()> {
        // Initialize transports based on configuration
        self.initialize_transports().await?;
        Ok(())
    }

    /// Stop the transport manager
    pub async fn stop(&self) -> Result<()> {
        // Close all connections
        let mut connections = self.connections.write().await;
        for (_, mut connection) in connections.drain() {
            let _ = connection.close().await;
        }
        Ok(())
    }

    /// Connect to a remote endpoint
    pub async fn connect(&self, endpoint: &Endpoint) -> Result<Box<dyn Connection>> {
        let transports = self.transports.read().await;

        if let Some(transport) = transports.get(&endpoint.transport) {
            let connection = transport.connect(endpoint).await?;
            Ok(connection)
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                format!("Transport {} not available", endpoint.transport),
            ))
        }
    }

    /// Listen for incoming connections
    pub async fn listen(&self, bind_address: std::net::SocketAddr) -> Result<Box<dyn Listener>> {
        // Use primary transport for listening
        let transports = self.transports.read().await;

        if let Some(transport) = transports.get(&self.config.primary_transport) {
            let listener = transport.listen(bind_address).await?;
            Ok(listener)
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                "Primary transport not available".to_string(),
            ))
        }
    }

    /// Send a message through a connection
    pub async fn send_message(
        &self,
        connection_id: ConnectionId,
        message: ValkyrieMessage,
    ) -> Result<()> {
        let mut connections = self.connections.write().await;

        if let Some(connection) = connections.get_mut(&connection_id) {
            // Serialize message and send
            let data = self.serialize_message(&message)?;
            connection.send(&data).await?;
            Ok(())
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                "Connection not found".to_string(),
            ))
        }
    }

    /// Get transport statistics
    pub async fn get_stats(&self) -> TransportStats {
        let connections = self.connections.read().await;

        TransportStats {
            active_connections: connections.len(),
            total_bytes_sent: 0,     // Placeholder
            total_bytes_received: 0, // Placeholder
            connection_errors: 0,    // Placeholder
        }
    }

    /// Initialize available transports
    async fn initialize_transports(&self) -> Result<()> {
        let _transports = self.transports.write().await;

        // Initialize TCP transport
        if self.config.primary_transport == "tcp"
            || self.config.fallback_transports.contains(&"tcp".to_string())
        {
            // transports.insert("tcp".to_string(), Arc::new(TcpTransport::new()));
        }

        // Initialize WebSocket transport
        if self.config.primary_transport == "websocket"
            || self
                .config
                .fallback_transports
                .contains(&"websocket".to_string())
        {
            // transports.insert("websocket".to_string(), Arc::new(WebSocketTransport::new()));
        }

        // Initialize Unix socket transport
        if self.config.primary_transport == "unix"
            || self
                .config
                .fallback_transports
                .contains(&"unix".to_string())
        {
            // transports.insert("unix".to_string(), Arc::new(UnixSocketTransport::new()));
        }

        Ok(())
    }

    /// Serialize a message for transport
    fn serialize_message(&self, message: &ValkyrieMessage) -> Result<Vec<u8>> {
        // Placeholder implementation
        serde_json::to_vec(message).map_err(|e| {
            crate::valkyrie::ValkyrieError::InternalServerError(format!(
                "Message serialization failed: {}",
                e
            ))
        })
    }
}

/// Transport statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransportStats {
    /// Number of active connections
    pub active_connections: usize,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Number of connection errors
    pub connection_errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valkyrie::TransportConfig;

    #[tokio::test]
    async fn test_transport_manager_creation() {
        let config = TransportConfig {
            primary_transport: "tcp".to_string(),
            fallback_transports: vec!["websocket".to_string()],
            connection_pool_size: 10,
            enable_compression: false,
            compression_level: 6,
        };

        let result = TransportManager::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transport_manager_lifecycle() {
        let config = TransportConfig {
            primary_transport: "tcp".to_string(),
            fallback_transports: vec![],
            connection_pool_size: 10,
            enable_compression: false,
            compression_level: 6,
        };

        let manager = TransportManager::new(config).unwrap();

        // This would need proper transport implementations to test
        // let start_result = manager.start().await;
        // assert!(start_result.is_ok());

        // let stop_result = manager.stop().await;
        // assert!(stop_result.is_ok());
    }
}
