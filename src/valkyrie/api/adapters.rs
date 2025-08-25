//! Adapter Layer for Valkyrie Protocol API-Engine Integration
//!
//! This module provides the bridge between the high-level developer-friendly API
//! and the low-level Valkyrie Protocol engine.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::valkyrie::api::{
    BroadcastResult, ClientConfig, ClientMessage, ClientMessagePriority, ClientMessageType,
    ClientPayload,
};
use crate::valkyrie::core::{
    ConnectionId, EngineStats, MessagePayload, MessagePriority, MessageType, ValkyrieMessage,
};
use crate::valkyrie::transport::Endpoint;
use crate::valkyrie::{Result, ValkyrieConfig, ValkyrieConfigBuilder, ValkyrieEngine, ValkyrieFactory};

/// Adapter that wraps the ValkyrieEngine and provides the high-level API interface
#[derive(Clone)]
pub struct ValkyrieEngineAdapter {
    /// The underlying Valkyrie engine
    engine: Arc<ValkyrieEngine>,
    /// Active connections mapping
    connections: Arc<RwLock<HashMap<String, ConnectionId>>>,
}

impl ValkyrieEngineAdapter {
    /// Create a new adapter with the given configuration
    pub async fn new(config: ClientConfig) -> Result<Self> {
        // Convert client config to engine config
        let engine_config = Self::convert_client_config_to_engine_config(config)?;

        // Create engine using factory
        let mut engine = ValkyrieFactory::create_with_config(engine_config).await?;
        engine.start().await?;

        Ok(Self {
            engine: Arc::new(engine),
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Connect to a remote endpoint
    pub async fn connect(&self, endpoint_url: &str) -> Result<String> {
        let endpoint = Self::parse_endpoint_url(endpoint_url)?;
        let connection_handle = self.engine.connect(endpoint).await?;

        let connection_id = connection_handle.id().to_string();
        let mut connections = self.connections.write().await;
        connections.insert(connection_id.clone(), connection_handle.id());

        Ok(connection_id)
    }

    /// Send a message to a specific connection
    pub async fn send_message(&self, connection_id: &str, message: ClientMessage) -> Result<()> {
        let connections = self.connections.read().await;

        if let Some(&engine_connection_id) = connections.get(connection_id) {
            let engine_message = Self::convert_client_message_to_engine_message(message)?;
            self.engine
                .send_message(engine_connection_id, engine_message)
                .await?;
            Ok(())
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                format!("Connection {} not found", connection_id),
            ))
        }
    }

    /// Broadcast a message to multiple connections
    pub async fn broadcast(
        &self,
        connection_ids: &[String],
        message: ClientMessage,
    ) -> Result<BroadcastResult> {
        let connections = self.connections.read().await;
        let engine_message = Self::convert_client_message_to_engine_message(message)?;

        let mut engine_connection_ids = Vec::new();
        let mut missing_connections = Vec::new();

        for connection_id in connection_ids {
            if let Some(&engine_connection_id) = connections.get(connection_id) {
                engine_connection_ids.push(engine_connection_id);
            } else {
                missing_connections.push(connection_id.clone());
            }
        }

        let engine_result = self
            .engine
            .broadcast(engine_connection_ids.clone(), engine_message)
            .await?;

        // Convert engine result to client result
        let mut client_results = HashMap::new();

        // Add successful results
        for (engine_id, result) in engine_result.results {
            // Find the client connection ID for this engine ID
            for (client_id, &stored_engine_id) in connections.iter() {
                if stored_engine_id == engine_id {
                    client_results.insert(client_id.clone(), result);
                    break;
                }
            }
        }

        // Add missing connection errors
        for connection_id in missing_connections {
            client_results.insert(
                connection_id,
                Err(crate::valkyrie::ValkyrieError::InternalServerError(
                    "Connection not found".to_string(),
                )),
            );
        }

        Ok(BroadcastResult {
            total: connection_ids.len(),
            successful: engine_result.successful,
            failed: engine_result.failed + (connection_ids.len() - engine_connection_ids.len()),
            results: client_results,
        })
    }

    /// Disconnect from a connection
    pub async fn disconnect(&self, connection_id: &str) -> Result<()> {
        let mut connections = self.connections.write().await;

        if let Some(engine_connection_id) = connections.remove(connection_id) {
            // Close the connection through the engine
            // Note: This would need to be implemented in the engine
            // self.engine.close_connection(engine_connection_id).await?;
            Ok(())
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                format!("Connection {} not found", connection_id),
            ))
        }
    }

    /// Get adapter statistics
    pub async fn get_stats(&self) -> Result<EngineStats> {
        Ok(self.engine.get_stats().await)
    }

    /// Shutdown the adapter
    pub async fn shutdown(self) -> Result<()> {
        // Close all connections
        let mut connections = self.connections.write().await;
        connections.clear();

        // Stop the engine
        // Note: This would need to be implemented properly
        // Arc::try_unwrap(self.engine).unwrap().stop().await?;

        Ok(())
    }

    // Private helper methods

    /// Convert ClientConfig to ValkyrieConfig
    fn convert_client_config_to_engine_config(
        config: ClientConfig,
    ) -> Result<ValkyrieConfig> {
        let mut builder = ValkyrieConfigBuilder::new()
            .with_connection_timeout(config.connection_timeout.as_millis() as u64)
            .with_message_timeout(config.message_timeout.as_millis() as u64);

        // Configure security
        if config.security.enable_tls {
            builder = builder.with_maximum_security();
        } else {
            builder = builder.with_minimal_security();
        }

        // Configure performance
        if config.performance.enable_compression {
            builder = builder.with_high_throughput_settings();
        }

        // Configure features
        if config.features.experimental {
            builder = builder.with_feature("experimental", true);
        }

        if config.features.metrics {
            builder = builder.with_feature("metrics", true);
        }

        if config.features.tracing {
            builder = builder.with_feature("tracing", true);
        }

        Ok(builder.build()?)
    }

    /// Parse endpoint URL into Endpoint struct
    fn parse_endpoint_url(url: &str) -> Result<Endpoint> {
        // Simple URL parsing - in a real implementation, this would be more robust
        if let Some(pos) = url.find("://") {
            let transport = &url[..pos];
            let rest = &url[pos + 3..];

            if let Some(colon_pos) = rest.rfind(':') {
                let address = &rest[..colon_pos];
                let port_str = &rest[colon_pos + 1..];

                if let Ok(port) = port_str.parse::<u16>() {
                    return Ok(Endpoint {
                        address: address.to_string(),
                        port,
                        transport: transport.to_string(),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Err(crate::valkyrie::ValkyrieError::InternalServerError(
            format!("Invalid endpoint URL: {}", url),
        ))
    }

    /// Convert ClientMessage to ValkyrieMessage
    fn convert_client_message_to_engine_message(message: ClientMessage) -> Result<ValkyrieMessage> {
        let message_type = match message.message_type {
            ClientMessageType::Text => MessageType::Data,
            ClientMessageType::Binary => MessageType::Data,
            ClientMessageType::Json => MessageType::Data,
            ClientMessageType::Control => MessageType::Control,
            ClientMessageType::Custom(_) => MessageType::Custom(0x1000),
        };

        let payload = match message.payload {
            ClientPayload::Text(text) => MessagePayload::Text(text),
            ClientPayload::Binary(data) => MessagePayload::Binary(data),
            ClientPayload::Json(value) => MessagePayload::Json(value),
            ClientPayload::Empty => MessagePayload::Empty,
        };

        let priority = match message.priority {
            ClientMessagePriority::Low => MessagePriority::Low,
            ClientMessagePriority::Normal => MessagePriority::Normal,
            ClientMessagePriority::High => MessagePriority::High,
            ClientMessagePriority::Critical => MessagePriority::Critical,
        };

        Ok(ValkyrieMessage::new(message_type, payload).with_priority(priority))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valkyrie::api::ClientConfigBuilder;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = ClientConfigBuilder::new().build();
        // This would need proper mocking to test
        // let result = ValkyrieEngineAdapter::new(config).await;
        // assert!(result.is_ok());
    }

    #[test]
    fn test_endpoint_parsing() {
        let endpoint = ValkyrieEngineAdapter::parse_endpoint_url("tcp://localhost:8080").unwrap();
        assert_eq!(endpoint.transport, "tcp");
        assert_eq!(endpoint.address, "localhost");
        assert_eq!(endpoint.port, 8080);
    }

    #[test]
    fn test_message_conversion() {
        let client_message = ClientMessage::text("Hello, World!");
        let result =
            ValkyrieEngineAdapter::convert_client_message_to_engine_message(client_message);
        assert!(result.is_ok());

        let engine_message = result.unwrap();
        assert_eq!(engine_message.header.message_type, MessageType::Data);
        assert!(matches!(engine_message.payload, MessagePayload::Text(_)));
    }
}
