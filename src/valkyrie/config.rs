//! Valkyrie Protocol Configuration Management
//!
//! This module provides configuration management for the Valkyrie Protocol
//! using the Builder pattern for flexible and type-safe configuration.

use crate::valkyrie::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main Valkyrie Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieConfig {
    /// Protocol-level configuration
    pub protocol: ProtocolConfig,
    /// Transport layer configuration
    pub transport: TransportConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Streaming configuration
    pub streaming: StreamingConfig,
    /// Routing configuration
    pub routing: RoutingConfig,
    /// Observability configuration
    pub observability: ObservabilityConfig,
    /// Performance configuration
    pub performance: PerformanceConfig,
    /// Feature flags
    pub features: FeatureFlags,
}

/// Routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
    /// Routing table size
    pub routing_table_size: usize,
    /// Route cache TTL
    pub route_cache_ttl: Duration,
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Round-robin
    RoundRobin,
    /// Least connections
    LeastConnections,
    /// Weighted round-robin
    WeightedRoundRobin,
    /// Consistent hashing
    ConsistentHashing,
}

/// Protocol-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Protocol version
    pub version: String,
    /// Protocol magic number
    pub magic: u32,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Message timeout
    pub message_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Maximum message size
    pub max_message_size: usize,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Primary transport type
    pub primary_transport: String,
    /// Fallback transports
    pub fallback_transports: Vec<String>,
    /// Connection pool size
    pub connection_pool_size: usize,
    /// Enable compression
    pub enable_compression: bool,
    /// Compression level
    pub compression_level: u32,
}

/// Protocol timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolTimeouts {
    /// Connection establishment timeout
    pub connect_timeout: Duration,
    /// Message send timeout
    pub send_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Connection idle timeout
    pub idle_timeout: Duration,
}

/// Transport layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportLayerConfig {
    /// Primary transport configuration
    pub primary: TransportConfig,
    /// Fallback transports
    pub fallbacks: Vec<TransportConfig>,
    /// Transport selection strategy
    pub selection_strategy: crate::valkyrie::transport::TransportSelectionStrategy,
    /// Connection pooling settings
    pub connection_pooling: ConnectionPoolConfig,
}

/// Connection pooling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// Maximum connections per endpoint
    pub max_connections_per_endpoint: u32,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Connection reuse strategy
    pub reuse_strategy: crate::valkyrie::transport::ConnectionReuseStrategy,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable TLS
    pub enable_tls: bool,
    /// Enable mutual TLS
    pub enable_mutual_tls: bool,
    /// Certificate path
    pub cert_path: Option<String>,
    /// Private key path
    pub key_path: Option<String>,
    /// CA certificate path
    pub ca_cert_path: Option<String>,
    /// Enable post-quantum cryptography
    pub enable_post_quantum: bool,
    /// Authentication configuration
    pub authentication: AuthenticationConfig,
    /// Encryption configuration
    pub encryption: EncryptionConfig,
    /// Authorization configuration
    pub authorization: AuthorizationConfig,
    /// Audit configuration
    pub audit: AuditConfig,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    /// Enabled authentication methods
    pub methods: Vec<AuthMethod>,
    /// Token expiration time
    pub token_expiry: Duration,
    /// Require mutual authentication
    pub require_mutual_auth: bool,
}

/// Authentication methods
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// No authentication
    None,
    /// Token-based authentication
    Token,
    /// Mutual TLS
    MutualTls,
    /// Custom authentication
    Custom(String),
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Enabled cipher suites
    pub cipher_suites: Vec<CipherSuite>,
    /// Key rotation interval
    pub key_rotation_interval: Duration,
    /// Enable forward secrecy
    pub forward_secrecy: bool,
}

/// Supported cipher suites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CipherSuite {
    /// AES-256-GCM
    Aes256Gcm,
    /// ChaCha20-Poly1305
    ChaCha20Poly1305,
    /// Post-quantum cipher
    PostQuantum,
}

/// Authorization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationConfig {
    /// Enable role-based access control
    pub enable_rbac: bool,
    /// Default permissions
    pub default_permissions: Vec<String>,
    /// Permission cache TTL
    pub cache_ttl: Duration,
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable audit logging
    pub enabled: bool,
    /// Audit log retention period
    pub retention_period: Duration,
    /// Events to audit
    pub events: Vec<AuditEvent>,
}

/// Audit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    /// Connection events
    Connection,
    /// Authentication events
    Authentication,
    /// Message events
    Message,
    /// Security events
    Security,
}

/// Streaming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Maximum concurrent streams
    pub max_concurrent_streams: u32,
    /// Maximum streams per connection
    pub max_streams_per_connection: u32,
    /// Stream buffer size
    pub stream_buffer_size: usize,
    /// Buffer size (alias for compatibility)
    pub buffer_size: usize,
    /// Enable flow control
    pub enable_flow_control: bool,
    /// Flow control configuration
    pub flow_control: FlowControlConfig,
    /// Enable priority scheduling
    pub priority_scheduling: bool,
    /// Initial window size
    pub initial_window_size: u32,
}

/// Flow control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlConfig {
    /// Initial window size
    pub initial_window_size: u32,
    /// Maximum window size
    pub max_window_size: u32,
    /// Window update threshold
    pub update_threshold: f64,
}

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable metrics
    pub enable_metrics: bool,
    /// Metrics configuration
    pub metrics: MetricsConfig,
    /// Enable tracing
    pub enable_tracing: bool,
    /// Tracing configuration
    pub tracing: TracingConfig,
    /// Enable logging
    pub enable_logging: bool,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Metrics export interval
    pub metrics_export_interval: Duration,
    /// Log level
    pub log_level: String,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Metrics export interval
    pub export_interval: Duration,
    /// Metrics retention period
    pub retention_period: Duration,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: bool,
    /// Sampling rate
    pub sampling_rate: f64,
    /// Trace export endpoint
    pub export_endpoint: Option<String>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    /// Log format
    pub format: LogFormat,
    /// Enable structured logging
    pub structured: bool,
}

/// Log formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    /// Plain text
    Text,
    /// JSON format
    Json,
    /// Custom format
    Custom(String),
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable zero-copy optimizations
    pub enable_zero_copy: bool,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Worker thread count
    pub worker_threads: Option<usize>,
    /// Message batch size
    pub message_batch_size: usize,
}

/// Feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable experimental features
    pub experimental: bool,
    /// Enable RustCI integration
    pub rustci_integration: bool,
    /// Enable container transport
    pub container_transport: bool,
    /// Enable Kubernetes transport
    pub kubernetes_transport: bool,
    /// Custom feature flags
    pub custom: HashMap<String, bool>,
}

/// Builder for Valkyrie configuration using the Builder pattern
pub struct ValkyrieConfigBuilder {
    protocol: ProtocolConfig,
    transport: TransportConfig,
    security: SecurityConfig,
    streaming: StreamingConfig,
    routing: RoutingConfig,
    observability: ObservabilityConfig,
    performance: PerformanceConfig,
    features: FeatureFlags,
}

impl ValkyrieConfigBuilder {
    /// Create a new configuration builder with defaults
    pub fn new() -> Self {
        Self {
            protocol: ProtocolConfig {
                version: crate::valkyrie::PROTOCOL_VERSION.to_string(),
                magic: crate::valkyrie::PROTOCOL_MAGIC,
                connection_timeout: Duration::from_secs(30),
                message_timeout: Duration::from_secs(10),
                heartbeat_interval: Duration::from_secs(30),
                max_message_size: 1024 * 1024, // 1MB
            },
            transport: TransportConfig {
                primary_transport: "tcp".to_string(),
                fallback_transports: vec!["websocket".to_string()],
                connection_pool_size: 10,
                enable_compression: false,
                compression_level: 6,
            },
            security: SecurityConfig {
                enable_tls: true,
                enable_mutual_tls: false,
                cert_path: None,
                key_path: None,
                ca_cert_path: None,
                enable_post_quantum: false,
                authentication: AuthenticationConfig {
                    methods: vec![AuthMethod::Token],
                    token_expiry: Duration::from_secs(3600),
                    require_mutual_auth: false,
                },
                encryption: EncryptionConfig {
                    cipher_suites: vec![CipherSuite::Aes256Gcm],
                    key_rotation_interval: Duration::from_secs(86400),
                    forward_secrecy: true,
                },
                authorization: AuthorizationConfig {
                    enable_rbac: false,
                    default_permissions: vec![],
                    cache_ttl: Duration::from_secs(300),
                },
                audit: AuditConfig {
                    enabled: false,
                    retention_period: Duration::from_secs(86400 * 7),
                    events: vec![],
                },
            },
            streaming: StreamingConfig {
                max_concurrent_streams: 100,
                max_streams_per_connection: 10,
                stream_buffer_size: 64 * 1024, // 64KB
                buffer_size: 64 * 1024,        // 64KB (alias)
                enable_flow_control: true,
                flow_control: FlowControlConfig {
                    initial_window_size: 65536,
                    max_window_size: 1024 * 1024,
                    update_threshold: 0.5,
                },
                priority_scheduling: true,
                initial_window_size: 65536,
            },
            routing: RoutingConfig {
                load_balancing: LoadBalancingStrategy::RoundRobin,
                routing_table_size: 1000,
                route_cache_ttl: Duration::from_secs(300),
            },
            observability: ObservabilityConfig {
                enable_metrics: true,
                metrics: MetricsConfig {
                    enabled: true,
                    export_interval: Duration::from_secs(60),
                    retention_period: Duration::from_secs(3600),
                },
                enable_tracing: true,
                tracing: TracingConfig {
                    enabled: true,
                    sampling_rate: 1.0,
                    export_endpoint: None,
                },
                enable_logging: true,
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Text,
                    structured: false,
                },
                metrics_export_interval: Duration::from_secs(60),
                log_level: "info".to_string(),
            },
            performance: PerformanceConfig {
                enable_zero_copy: false,
                enable_simd: false,
                worker_threads: None,
                message_batch_size: 100,
            },
            features: FeatureFlags {
                experimental: false,
                rustci_integration: false,
                container_transport: false,
                kubernetes_transport: false,
                custom: HashMap::new(),
            },
        }
    }

    /// Configure for RustCI integration
    pub fn with_rustci_defaults(mut self) -> Self {
        self.features.rustci_integration = true;
        self.features.container_transport = true;
        self.transport.primary_transport = "tcp".to_string();
        self.transport.fallback_transports = vec!["websocket".to_string(), "unix".to_string()];
        self.performance.worker_threads = Some(4);
        self
    }

    /// Enable CI-specific optimizations
    pub fn enable_ci_optimizations(mut self) -> Self {
        self.performance.message_batch_size = 500;
        self.streaming.max_concurrent_streams = 200;
        self.transport.connection_pool_size = 20;
        self
    }

    /// Enable container transport
    pub fn enable_container_transport(mut self) -> Self {
        self.features.container_transport = true;
        if !self
            .transport
            .fallback_transports
            .contains(&"unix".to_string())
        {
            self.transport.fallback_transports.push("unix".to_string());
        }
        self
    }

    /// Enable Kubernetes transport
    pub fn enable_kubernetes_transport(mut self) -> Self {
        self.features.kubernetes_transport = true;
        self
    }

    /// Configure for high throughput
    pub fn with_high_throughput_settings(mut self) -> Self {
        self.performance.enable_zero_copy = true;
        self.performance.enable_simd = true;
        self.performance.message_batch_size = 1000;
        self.streaming.stream_buffer_size = 256 * 1024; // 256KB
        self.transport.enable_compression = true;
        self
    }

    /// Configure with default transport
    pub fn with_default_transport(self) -> Self {
        // Already configured in new()
        self
    }

    /// Configure with default security
    pub fn with_default_security(self) -> Self {
        // Already configured in new()
        self
    }

    /// Configure with default streaming
    pub fn with_default_streaming(self) -> Self {
        // Already configured in new()
        self
    }

    /// Configure with default observability
    pub fn with_default_observability(self) -> Self {
        // Already configured in new()
        self
    }

    /// Enable zero-copy optimizations
    pub fn enable_zero_copy(mut self) -> Self {
        self.performance.enable_zero_copy = true;
        self
    }

    /// Enable SIMD optimizations
    pub fn enable_simd_optimizations(mut self) -> Self {
        self.performance.enable_simd = true;
        self
    }

    /// Configure with minimal security (for testing)
    pub fn with_minimal_security(mut self) -> Self {
        self.security.enable_tls = false;
        self.security.enable_mutual_tls = false;
        self
    }

    /// Configure with maximum security
    pub fn with_maximum_security(mut self) -> Self {
        self.security.enable_tls = true;
        self.security.enable_mutual_tls = true;
        self.security.enable_post_quantum = true;
        self
    }

    /// Enable post-quantum cryptography
    pub fn enable_post_quantum_crypto(mut self) -> Self {
        self.security.enable_post_quantum = true;
        self
    }

    /// Enable mutual TLS
    pub fn enable_mutual_tls(mut self) -> Self {
        self.security.enable_mutual_tls = true;
        self
    }

    /// Enable audit logging
    pub fn enable_audit_logging(mut self) -> Self {
        self.observability.log_level = "debug".to_string();
        self
    }

    /// Set custom feature flag
    pub fn with_feature(mut self, name: &str, enabled: bool) -> Self {
        self.features.custom.insert(name.to_string(), enabled);
        self
    }

    /// Set worker thread count
    pub fn with_worker_threads(mut self, count: usize) -> Self {
        self.performance.worker_threads = Some(count);
        self
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.protocol.connection_timeout = timeout;
        self
    }

    /// Set message timeout
    pub fn with_message_timeout(mut self, timeout: Duration) -> Self {
        self.protocol.message_timeout = timeout;
        self
    }

    /// Build the final configuration
    pub fn build(self) -> Result<ValkyrieConfig> {
        Ok(ValkyrieConfig {
            protocol: self.protocol,
            transport: self.transport,
            security: self.security,
            streaming: self.streaming,
            routing: self.routing,
            observability: self.observability,
            performance: self.performance,
            features: self.features,
        })
    }
}

impl Default for ValkyrieConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder_default() {
        let config = ValkyrieConfigBuilder::new().build().unwrap();
        assert_eq!(config.protocol.version, crate::valkyrie::PROTOCOL_VERSION);
        assert_eq!(config.protocol.magic, crate::valkyrie::PROTOCOL_MAGIC);
    }

    #[test]
    fn test_config_builder_rustci() {
        let config = ValkyrieConfigBuilder::new()
            .with_rustci_defaults()
            .build()
            .unwrap();
        assert!(config.features.rustci_integration);
        assert!(config.features.container_transport);
    }

    #[test]
    fn test_config_builder_high_performance() {
        let config = ValkyrieConfigBuilder::new()
            .with_high_throughput_settings()
            .build()
            .unwrap();
        assert!(config.performance.enable_zero_copy);
        assert!(config.performance.enable_simd);
        assert_eq!(config.performance.message_batch_size, 1000);
    }
}
