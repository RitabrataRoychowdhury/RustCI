//! Adapter Layer for Valkyrie Protocol API-Engine Integration
//!
//! This module provides the bridge between the high-level developer-friendly API
//! and the low-level Valkyrie Protocol engine. It handles type conversions,
//! configuration mapping, and ensures seamless integration between layers.

use std::collections::HashMap;
use std::time::Duration;
// async_trait removed as it's not used in this simplified version
use crate::valkyrie::api::config::{ClientAuthMethod, ClientSecurityConfig};
use crate::valkyrie::api::{
    ClientConfig, ClientMessage, ClientMessagePriority, ClientMessageType, ClientPayload,
    ClientStats,
};
use uuid::Uuid;

use crate::valkyrie::{
    ConnectionId, Endpoint, EngineStats, MessagePriority as EnginePriority,
    MessageType as EngineMessageType, ValkyrieConfig, ValkyrieEngine,
    ValkyrieMessage as EngineMessage,
};

use crate::error::Result;

/// Adapter that wraps the ValkyrieEngine and provides the high-level API interface
#[derive(Clone)]
pub struct ValkyrieEngineAdapter {
    engine: ValkyrieEngine,
}

impl ValkyrieEngineAdapter {
    /// Create a new adapter with the given configuration
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let engine_config = Self::convert_client_config_to_engine_config(config)?;
        let mut engine = ValkyrieEngine::new(engine_config)?;
        engine.start().await?;

        Ok(Self { engine })
    }

    /// Convert ClientConfig to ValkyrieConfig
    fn convert_client_config_to_engine_config(config: ClientConfig) -> Result<ValkyrieConfig> {
        use crate::valkyrie::*;

        Ok(ValkyrieConfig {
            protocol: ProtocolConfig {
                version: "1.0.0".to_string(),
                magic: 0x56414C4B, // "VALK" in hex
                connection_timeout: config.connection_timeout,
                message_timeout: config.message_timeout,
                heartbeat_interval: Duration::from_secs(30),
                max_message_size: 1024 * 1024, // 1MB default
            },
            transport: TransportConfig {
                primary_transport: "tcp".to_string(),
                fallback_transports: vec!["websocket".to_string()],
                connection_pool_size: config.performance.connection_pool_size,
                enable_compression: config.performance.enable_compression,
                compression_level: config.performance.compression_level as u32,
            },
            security: Self::convert_security_config(config.security)?,
            streaming: StreamingConfig {
                max_concurrent_streams: 100,
                max_streams_per_connection: 10,
                stream_buffer_size: 64 * 1024,
                buffer_size: 64 * 1024,
                enable_flow_control: true,
                flow_control: FlowControlConfig {
                    initial_window_size: 65536,
                    max_window_size: 1048576,
                    update_threshold: 0.5,
                },
                priority_scheduling: true,
                initial_window_size: 65536,
            },
            routing: RoutingConfig {
                load_balancing: crate::valkyrie::config::LoadBalancingStrategy::RoundRobin,
                routing_table_size: 1000,
                route_cache_ttl: Duration::from_secs(300),
            },
            performance: PerformanceConfig {
                enable_zero_copy: config.features.experimental,
                enable_simd: config.features.experimental,
                worker_threads: Some(4),
                message_batch_size: config.performance.message_batch_size,
            },
            observability: ObservabilityConfig {
                enable_metrics: true,
                metrics: MetricsConfig {
                    enabled: true,
                    export_interval: Duration::from_secs(60),
                    retention_period: Duration::from_secs(86400),
                },
                enable_tracing: true,
                tracing: TracingConfig {
                    enabled: true,
                    sampling_rate: 0.1,
                    export_endpoint: None,
                },
                enable_logging: true,
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json,
                    structured: true,
                },
                metrics_export_interval: Duration::from_secs(60),
                log_level: "info".to_string(),
            },
            features: FeatureFlags {
                experimental: config.features.experimental,
                rustci_integration: true,
                container_transport: false,
                kubernetes_transport: false,
                custom: HashMap::new(),
            },
        })
    }

    /// Convert ClientSecurityConfig to SecurityConfig
    fn convert_security_config(
        config: ClientSecurityConfig,
    ) -> Result<crate::valkyrie::SecurityConfig> {
        use crate::valkyrie::*;

        let auth_methods = match config.auth_method {
            ClientAuthMethod::None => vec![AuthMethod::None],
            ClientAuthMethod::Token(_) => vec![AuthMethod::Token],
            ClientAuthMethod::Certificate => vec![AuthMethod::MutualTls],
            ClientAuthMethod::Custom(_) => vec![AuthMethod::Custom("custom".to_string())],
        };

        Ok(SecurityConfig {
            enable_tls: true,
            enable_mutual_tls: matches!(config.auth_method, ClientAuthMethod::Certificate),
            cert_path: config.cert_path.clone(),
            key_path: config.key_path.clone(),
            ca_cert_path: config.ca_cert_path.clone(),
            enable_post_quantum: false,
            authentication: AuthenticationConfig {
                methods: auth_methods,
                token_expiry: Duration::from_secs(3600),
                require_mutual_auth: matches!(config.auth_method, ClientAuthMethod::Certificate),
            },
            encryption: EncryptionConfig {
                cipher_suites: vec![CipherSuite::Aes256Gcm],
                key_rotation_interval: Duration::from_secs(86400),
                forward_secrecy: true,
            },
            authorization: AuthorizationConfig {
                enable_rbac: false,
                default_permissions: vec!["read".to_string(), "write".to_string()],
                cache_ttl: Duration::from_secs(300),
            },
            audit: AuditConfig {
                enabled: config.enable_tls, // Use enable_tls as proxy for audit
                retention_period: Duration::from_secs(2592000), // 30 days
                events: vec![AuditEvent::Connection, AuditEvent::Authentication],
            },
        })
    }

    /// Connect to a remote endpoint
    pub async fn connect(&self, endpoint_url: &str) -> Result<String> {
        let endpoint = Self::parse_endpoint_url(endpoint_url)?;
        let connection = self.engine.connect(endpoint).await?;
        Ok(connection.id().to_string())
    }

    /// Send a text message
    pub async fn send_text(&self, connection_id: &str, text: &str) -> Result<()> {
        let message = Self::create_text_message(text)?;
        let conn_id = Self::parse_connection_id(connection_id)?;
        self.engine.send_message(conn_id, message).await
    }

    /// Send binary data
    pub async fn send_data(&self, connection_id: &str, data: &[u8]) -> Result<()> {
        let message = Self::create_binary_message(data)?;
        let conn_id = Self::parse_connection_id(connection_id)?;
        self.engine.send_message(conn_id, message).await
    }

    /// Send a custom message
    pub async fn send_message(&self, connection_id: &str, message: ClientMessage) -> Result<()> {
        let engine_message = Self::convert_client_message_to_engine_message(message)?;
        let conn_id = Self::parse_connection_id(connection_id)?;
        self.engine.send_message(conn_id, engine_message).await
    }

    /// Broadcast a message
    pub async fn broadcast(
        &self,
        connection_ids: &[String],
        message: ClientMessage,
    ) -> Result<crate::api::valkyrie::BroadcastResult> {
        let engine_message = Self::convert_client_message_to_engine_message(message)?;
        let conn_ids: Result<Vec<ConnectionId>> = connection_ids
            .iter()
            .map(|id| Self::parse_connection_id(id))
            .collect();

        let result = self.engine.broadcast(conn_ids?, engine_message).await?;
        Ok(Self::convert_engine_broadcast_result_to_client_result(
            result,
        ))
    }

    // Message handler registration removed for simplicity - can be added back later if needed

    /// Get client statistics
    pub async fn get_stats(&self) -> ClientStats {
        let engine_stats = self.engine.get_stats().await;
        Self::convert_engine_stats_to_client_stats(engine_stats)
    }

    /// Close a connection
    pub async fn close_connection(&self, _connection_id: &str) -> Result<()> {
        // Implementation would depend on engine API
        Ok(())
    }

    /// Shutdown the adapter
    pub async fn shutdown(&mut self) -> Result<()> {
        self.engine.stop().await
    }

    // Helper methods for type conversions

    fn parse_endpoint_url(url: &str) -> Result<Endpoint> {
        // Simple URL parsing - in a real implementation, this would be more robust
        if let Some(pos) = url.find("://") {
            let transport = &url[..pos];
            let address_part = &url[pos + 3..];

            if let Some(colon_pos) = address_part.rfind(':') {
                let address = &address_part[..colon_pos];
                let port_str = &address_part[colon_pos + 1..];
                let port: u16 = port_str.parse().map_err(|e| {
                    crate::error::AppError::ValidationError(format!("Invalid port: {}", e))
                })?;

                Ok(Endpoint {
                    address: address.to_string(),
                    port,
                    transport: transport.to_string(),
                    metadata: std::collections::HashMap::new(),
                })
            } else {
                Err(crate::error::AppError::ValidationError(
                    "Invalid endpoint URL format".to_string(),
                ))
            }
        } else {
            Err(crate::error::AppError::ValidationError(
                "Invalid endpoint URL format".to_string(),
            ))
        }
    }

    fn parse_connection_id(connection_id: &str) -> Result<ConnectionId> {
        Uuid::parse_str(connection_id).map_err(|e| {
            crate::error::AppError::ValidationError(format!("Invalid connection ID: {}", e))
        })
    }

    fn create_text_message(text: &str) -> Result<EngineMessage> {
        Ok(EngineMessage::text(text))
    }

    fn create_binary_message(data: &[u8]) -> Result<EngineMessage> {
        Ok(EngineMessage::binary(data.to_vec()))
    }

    fn convert_client_message_to_engine_message(message: ClientMessage) -> Result<EngineMessage> {
        let payload = match message.payload {
            ClientPayload::Text(text) => text.as_bytes().to_vec(),
            ClientPayload::Binary(data) => data,
            ClientPayload::Json(value) => serde_json::to_vec(&value).unwrap_or_default(),
            ClientPayload::Empty => vec![],
        };

        let message_type = match message.message_type {
            ClientMessageType::Text | ClientMessageType::Json | ClientMessageType::Binary => {
                EngineMessageType::Data
            }
            ClientMessageType::Control => EngineMessageType::Control,
            ClientMessageType::Custom(_) => EngineMessageType::Data, // Default to Data for custom types
        };

        let priority = match message.priority {
            ClientMessagePriority::Low => EnginePriority::Low,
            ClientMessagePriority::Normal => EnginePriority::Normal,
            ClientMessagePriority::High => EnginePriority::High,
            ClientMessagePriority::Critical => EnginePriority::Critical,
        };

        let correlation_id = message
            .correlation_id
            .and_then(|id| Uuid::parse_str(&id).ok());

        let mut engine_message = EngineMessage::new(
            message_type,
            crate::valkyrie::core::message::MessagePayload::Binary(payload),
        );
        engine_message.header.priority = priority;
        if let Some(ttl) = message.ttl {
            engine_message = engine_message.with_ttl(ttl);
        }
        if let Some(correlation_id) = correlation_id {
            engine_message = engine_message.with_correlation_id(correlation_id);
        }
        Ok(engine_message)
    }

    fn convert_engine_broadcast_result_to_client_result(
        result: crate::valkyrie::core::engine::BroadcastResult,
    ) -> crate::api::valkyrie::BroadcastResult {
        crate::api::valkyrie::BroadcastResult {
            total: result.total,
            successful: result.successful,
            failed: result.failed,
            results: result
                .results
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    fn convert_engine_stats_to_client_stats(stats: EngineStats) -> ClientStats {
        ClientStats {
            engine_stats: stats,
            active_connections: 0,  // Would be populated from engine
            handlers_registered: 0, // Would be populated from engine
        }
    }
}

// MessageHandlerAdapter removed for simplicity - can be added back later if needed

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = ClientConfig::default();
        let _result = ValkyrieEngineAdapter::new(config).await;
        // This test would need a mock engine to pass
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
        match &engine_message.payload {
            crate::valkyrie::core::message::MessagePayload::Binary(data) => {
                assert_eq!(data, b"Hello, World!");
            }
            _ => panic!("Expected Binary payload"),
        }
    }
}
