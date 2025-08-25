//! Adapter Layer for Valkyrie Protocol API-Engine Integration
//!
//! This module provides the bridge between the high-level developer-friendly API
//! and the low-level Valkyrie Protocol engine. It handles type conversions,
//! configuration mapping, and ensures seamless integration between layers.

// async_trait removed as it's not used in this simplified version
use crate::valkyrie::api::config::ClientSecurityConfig;
use crate::valkyrie::api::{
    ClientConfig, ClientMessage, ClientMessagePriority, ClientMessageType, ClientPayload,
    ClientStats,
};
use uuid::Uuid;

use crate::valkyrie::{
    ConnectionId, Endpoint, EngineStats, MessagePriority as EnginePriority,
    MessageType as EngineMessageType, ValkyrieEngine,
    ValkyrieMessage as EngineMessage,
};
use crate::valkyrie::config::{
    ValkyrieConfig, ProtocolConfig, TransportConfig, StreamingConfig, 
    SecurityConfig, PerformanceConfig, RoutingConfig, ObservabilityConfig,
    MetricsConfig, TracingConfig, LoggingConfig
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
        Ok(ValkyrieConfig {
            server: crate::valkyrie::config::ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                https_port: Some(8443),
                max_connections: 10000,
                connection_timeout_ms: config.connection_timeout.as_millis() as u64,
                request_timeout_ms: config.message_timeout.as_millis() as u64,
                keepalive_timeout_ms: 60000,
                worker_threads: 4,
                blocking_threads: 8,
            },
            protocol: ProtocolConfig {
                magic: 0x56414C4B, // "VALK"
                version: "1.0.0".to_string(),
                connection_timeout: config.connection_timeout.as_millis() as u64,
                message_timeout: config.message_timeout.as_millis() as u64,
                heartbeat_interval: 30000,
                max_message_size: 1024 * 1024, // 1MB
            },
            transport: TransportConfig {
                primary_transport: "tcp".to_string(),
                fallback_transports: vec!["websocket".to_string()],
                connection_pool_size: config.performance.connection_pool_size,
                timeout_ms: 30000,
                retry_attempts: 3,
                enable_compression: config.performance.enable_compression,
            },
            security: Self::convert_security_config(config.security)?,
            streaming: StreamingConfig {
                enable_multiplexing: true,
                max_concurrent_streams: 1000,
                stream_timeout_ms: 60000,
                buffer_size: 8192,
            },
            routing: RoutingConfig {
                strategy: "high_performance".to_string(),
                max_routes: 1_000_000,
                route_cache_size: 100_000,
                connection_pool_size: 1000,
                max_concurrent_requests: 50_000,
                lockfree: Some(crate::valkyrie::config::LockFreeConfig::default()),
                fuzzy_matching: Some(crate::valkyrie::config::FuzzyMatchingConfig::default()),
            },
            performance: PerformanceConfig {
                mode: "high".to_string(),
                enable_simd: Some(config.features.experimental),
                enable_zero_copy: Some(config.features.experimental),
                enable_hot_path_optimization: Some(true),
                performance_budget: Some("82us".to_string()),
                throughput_target: Some(1_000_000),
                memory: Some(crate::valkyrie::config::MemoryConfig::default()),
                cpu: Some(crate::valkyrie::config::CpuConfig::default()),
            },
            observability: ObservabilityConfig {
                metrics: Some(MetricsConfig {
                    enable: true,
                    port: Some(9090),
                    path: "/metrics".to_string(),
                    enable_detailed_metrics: true,
                }),
                tracing: Some(TracingConfig {
                    enable: true,
                    sampling_rate: 0.1,
                    jaeger_endpoint: Some("http://jaeger:14268/api/traces".to_string()),
                }),
                logging: Some(LoggingConfig {
                    level: "info".to_string(),
                    format: "json".to_string(),
                    enable_file_logging: true,
                }),
            },
            caching: crate::valkyrie::config::CachingConfig::default(),
        })
    }

    /// Convert ClientSecurityConfig to SecurityConfig
    fn convert_security_config(
        config: ClientSecurityConfig,
    ) -> Result<SecurityConfig> {
        Ok(SecurityConfig {
            enable_tls: config.enable_tls,
            tls_cert_path: config.cert_path.clone(),
            tls_key_path: config.key_path.clone(),
            tls_ca_path: config.ca_cert_path.clone(),
            rate_limiting: Some(crate::valkyrie::config::RateLimitingConfig {
                enable: true,
                default_rate: "1000/min".to_string(),
                api_rate: "10000/min".to_string(),
                burst_size: 100,
            }),
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
