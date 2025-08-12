//! Valkyrie Protocol Configuration
//!
//! This module provides configuration management specifically for the Valkyrie Protocol,
//! integrating with the main application configuration system.

use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::core::networking::valkyrie::engine::{
    ValkyrieConfig, TransportSelectionStrategy, ConnectionReuseStrategy,
    AuthMethod, CipherSuite, LoadBalancingStrategy, LogFormat,
};
use crate::error::{AppError, Result};

/// Valkyrie Protocol configuration manager
#[derive(Debug, Clone)]
pub struct ValkyrieConfigManager {
    /// Current configuration
    config: ValkyrieConfig,
    /// Configuration source (file path or "environment")
    source: String,
}

impl ValkyrieConfigManager {
    /// Create a new configuration manager with default settings
    pub fn new() -> Self {
        Self {
            config: ValkyrieConfig::default(),
            source: "default".to_string(),
        }
    }

    /// Load configuration from environment variables with comprehensive validation
    pub fn load_from_env(&mut self) -> Result<()> {
        let mut config = ValkyrieConfig::default();

        // Protocol configuration
        if let Ok(version) = std::env::var("VALKYRIE_PROTOCOL_VERSION") {
            if !self.is_valid_version(&version) {
                return Err(AppError::ConfigError(format!("Invalid protocol version format: {}", version)));
            }
            config.protocol.version = version;
        }
        
        if let Ok(bind_address) = std::env::var("VALKYRIE_BIND_ADDRESS") {
            if !self.is_valid_bind_address(&bind_address) {
                return Err(AppError::ConfigError(format!("Invalid bind address: {}", bind_address)));
            }
            // Store bind address in config (assuming it's added to the config struct)
        }
        
        if let Ok(max_message_size) = std::env::var("VALKYRIE_MAX_MESSAGE_SIZE") {
            let size: usize = max_message_size.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid max message size: {}", e)))?;
            if size == 0 || size > 1_073_741_824 { // 1GB limit
                return Err(AppError::ConfigError("Max message size must be between 1 and 1GB".to_string()));
            }
            // Store in config
        }
        
        if let Ok(compat_mode) = std::env::var("VALKYRIE_COMPATIBILITY_MODE") {
            config.protocol.compatibility_mode = compat_mode.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid compatibility mode: {}", e)))?;
        }

        // Bridge configuration
        if let Ok(bridge_enabled) = std::env::var("VALKYRIE_BRIDGE_ENABLED") {
            let _enabled: bool = bridge_enabled.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid bridge enabled setting: {}", e)))?;
            // Store in config
        }
        
        if let Ok(http_port) = std::env::var("VALKYRIE_HTTP_PORT") {
            let port: u16 = http_port.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid HTTP port: {}", e)))?;
            if port < 1024 && port != 80 {
                return Err(AppError::ConfigError("HTTP port should be >= 1024 or 80".to_string()));
            }
            // Store in config
        }
        
        if let Ok(https_port) = std::env::var("VALKYRIE_HTTPS_PORT") {
            let port: u16 = https_port.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid HTTPS port: {}", e)))?;
            if port < 1024 && port != 443 {
                return Err(AppError::ConfigError("HTTPS port should be >= 1024 or 443".to_string()));
            }
            // Store in config
        }

        // Security configuration enhancements
        if let Ok(token_secret) = std::env::var("VALKYRIE_TOKEN_SECRET") {
            if token_secret.len() < 32 {
                return Err(AppError::ConfigError("Token secret must be at least 32 characters".to_string()));
            }
            // Store in config
        }
        
        if let Ok(cert_file) = std::env::var("VALKYRIE_CERT_FILE") {
            if !std::path::Path::new(&cert_file).exists() {
                return Err(AppError::ConfigError(format!("Certificate file not found: {}", cert_file)));
            }
            // Store in config
        }
        
        if let Ok(key_file) = std::env::var("VALKYRIE_KEY_FILE") {
            if !std::path::Path::new(&key_file).exists() {
                return Err(AppError::ConfigError(format!("Key file not found: {}", key_file)));
            }
            // Store in config
        }

        // Performance configuration with validation
        if let Ok(max_connections) = std::env::var("VALKYRIE_MAX_CONNECTIONS") {
            let max: u32 = max_connections.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid max connections: {}", e)))?;
            if max == 0 || max > 1_000_000 {
                return Err(AppError::ConfigError("Max connections must be between 1 and 1,000,000".to_string()));
            }
            // Store in config
        }
        
        if let Ok(worker_threads) = std::env::var("VALKYRIE_WORKER_THREADS") {
            if worker_threads != "auto" {
                let threads: usize = worker_threads.parse()
                    .map_err(|e| AppError::ConfigError(format!("Invalid worker threads (use 'auto' or number): {}", e)))?;
                if threads == 0 || threads > 1024 {
                    return Err(AppError::ConfigError("Worker threads must be between 1 and 1024".to_string()));
                }
                config.performance.worker_threads = Some(threads);
            }
        }

        // Observability configuration
        if let Ok(log_level) = std::env::var("VALKYRIE_LOG_LEVEL") {
            if !self.is_valid_log_level(&log_level) {
                return Err(AppError::ConfigError(format!("Invalid log level: {} (use trace, debug, info, warn, error)", log_level)));
            }
            config.observability.logging.level = log_level.to_lowercase();
        }
        
        if let Ok(metrics_endpoint) = std::env::var("VALKYRIE_METRICS_ENDPOINT") {
            if !metrics_endpoint.starts_with('/') {
                return Err(AppError::ConfigError("Metrics endpoint must start with '/'".to_string()));
            }
            // Store in config
        }
        
        if let Ok(jaeger_endpoint) = std::env::var("VALKYRIE_JAEGER_ENDPOINT") {
            if !self.is_valid_url(&jaeger_endpoint) {
                return Err(AppError::ConfigError(format!("Invalid Jaeger endpoint URL: {}", jaeger_endpoint)));
            }
            config.observability.tracing.export_endpoint = Some(jaeger_endpoint);
        }

        // Cluster configuration
        if let Ok(node_id) = std::env::var("VALKYRIE_NODE_ID") {
            if node_id.is_empty() || node_id.len() > 64 {
                return Err(AppError::ConfigError("Node ID must be between 1 and 64 characters".to_string()));
            }
            // Store in config
        }
        
        if let Ok(cluster_nodes) = std::env::var("VALKYRIE_CLUSTER_NODES") {
            let nodes: Vec<&str> = cluster_nodes.split(',').collect();
            for node in nodes {
                if !self.is_valid_endpoint(node.trim()) {
                    return Err(AppError::ConfigError(format!("Invalid cluster node endpoint: {}", node)));
                }
            }
            // Store in config
        }

        // Timeout configuration
        if let Ok(connect_timeout) = std::env::var("VALKYRIE_CONNECT_TIMEOUT_SECS") {
            let secs: u64 = connect_timeout.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid connect timeout: {}", e)))?;
            config.protocol.timeouts.connect_timeout = Duration::from_secs(secs);
        }
        if let Ok(send_timeout) = std::env::var("VALKYRIE_SEND_TIMEOUT_SECS") {
            let secs: u64 = send_timeout.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid send timeout: {}", e)))?;
            config.protocol.timeouts.send_timeout = Duration::from_secs(secs);
        }
        if let Ok(heartbeat_interval) = std::env::var("VALKYRIE_HEARTBEAT_INTERVAL_SECS") {
            let secs: u64 = heartbeat_interval.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid heartbeat interval: {}", e)))?;
            config.protocol.timeouts.heartbeat_interval = Duration::from_secs(secs);
        }
        if let Ok(idle_timeout) = std::env::var("VALKYRIE_IDLE_TIMEOUT_SECS") {
            let secs: u64 = idle_timeout.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid idle timeout: {}", e)))?;
            config.protocol.timeouts.idle_timeout = Duration::from_secs(secs);
        }

        // Transport configuration
        if let Ok(strategy) = std::env::var("VALKYRIE_TRANSPORT_SELECTION_STRATEGY") {
            config.transport.selection_strategy = match strategy.to_lowercase().as_str() {
                "primary" => TransportSelectionStrategy::Primary,
                "roundrobin" | "round_robin" => TransportSelectionStrategy::RoundRobin,
                "latency" | "latency_based" => TransportSelectionStrategy::LatencyBased,
                "bandwidth" | "bandwidth_based" => TransportSelectionStrategy::BandwidthBased,
                custom => TransportSelectionStrategy::Custom(custom.to_string()),
            };
        }

        if let Ok(max_conn) = std::env::var("VALKYRIE_MAX_CONNECTIONS_PER_ENDPOINT") {
            let max: u32 = max_conn.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid max connections: {}", e)))?;
            config.transport.connection_pooling.max_connections_per_endpoint = max;
        }

        if let Ok(reuse_strategy) = std::env::var("VALKYRIE_CONNECTION_REUSE_STRATEGY") {
            config.transport.connection_pooling.reuse_strategy = match reuse_strategy.to_lowercase().as_str() {
                "none" => ConnectionReuseStrategy::None,
                "per_endpoint" | "perendpoint" => ConnectionReuseStrategy::PerEndpoint,
                "global" => ConnectionReuseStrategy::Global,
                _ => return Err(AppError::ConfigError(format!("Invalid connection reuse strategy: {}", reuse_strategy))),
            };
        }

        // Security configuration
        if let Ok(auth_methods) = std::env::var("VALKYRIE_AUTH_METHODS") {
            let methods: Result<Vec<AuthMethod>> = auth_methods
                .split(',')
                .map(|method| match method.trim().to_lowercase().as_str() {
                    "none" => Ok(AuthMethod::None),
                    "token" => Ok(AuthMethod::Token),
                    "mtls" | "mutual_tls" => Ok(AuthMethod::MutualTls),
                    custom => Ok(AuthMethod::Custom(custom.to_string())),
                })
                .collect();
            config.security.authentication.methods = methods?;
        }

        if let Ok(token_expiry) = std::env::var("VALKYRIE_TOKEN_EXPIRY_SECS") {
            let secs: u64 = token_expiry.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid token expiry: {}", e)))?;
            config.security.authentication.token_expiry = Duration::from_secs(secs);
        }

        if let Ok(mutual_auth) = std::env::var("VALKYRIE_REQUIRE_MUTUAL_AUTH") {
            config.security.authentication.require_mutual_auth = mutual_auth.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid mutual auth setting: {}", e)))?;
        }

        if let Ok(cipher_suites) = std::env::var("VALKYRIE_CIPHER_SUITES") {
            let suites: Result<Vec<CipherSuite>> = cipher_suites
                .split(',')
                .map(|suite| match suite.trim().to_lowercase().as_str() {
                    "aes256gcm" | "aes_256_gcm" => Ok(CipherSuite::Aes256Gcm),
                    "chacha20poly1305" | "chacha20_poly1305" => Ok(CipherSuite::ChaCha20Poly1305),
                    "postquantum" | "post_quantum" => Ok(CipherSuite::PostQuantum),
                    _ => Err(AppError::ConfigError(format!("Invalid cipher suite: {}", suite))),
                })
                .collect();
            config.security.encryption.cipher_suites = suites?;
        }

        if let Ok(key_rotation) = std::env::var("VALKYRIE_KEY_ROTATION_INTERVAL_SECS") {
            let secs: u64 = key_rotation.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid key rotation interval: {}", e)))?;
            config.security.encryption.key_rotation_interval = Duration::from_secs(secs);
        }

        if let Ok(forward_secrecy) = std::env::var("VALKYRIE_FORWARD_SECRECY") {
            config.security.encryption.forward_secrecy = forward_secrecy.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid forward secrecy setting: {}", e)))?;
        }

        // Streaming configuration
        if let Ok(max_streams) = std::env::var("VALKYRIE_MAX_STREAMS_PER_CONNECTION") {
            let max: u32 = max_streams.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid max streams: {}", e)))?;
            config.streaming.max_streams_per_connection = max;
        }

        if let Ok(buffer_size) = std::env::var("VALKYRIE_STREAM_BUFFER_SIZE") {
            let size: usize = buffer_size.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid buffer size: {}", e)))?;
            config.streaming.buffer_size = size;
        }

        if let Ok(initial_window) = std::env::var("VALKYRIE_INITIAL_WINDOW_SIZE") {
            let size: u32 = initial_window.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid initial window size: {}", e)))?;
            config.streaming.flow_control.initial_window_size = size;
        }

        if let Ok(max_window) = std::env::var("VALKYRIE_MAX_WINDOW_SIZE") {
            let size: u32 = max_window.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid max window size: {}", e)))?;
            config.streaming.flow_control.max_window_size = size;
        }

        if let Ok(priority_scheduling) = std::env::var("VALKYRIE_PRIORITY_SCHEDULING") {
            config.streaming.priority_scheduling = priority_scheduling.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid priority scheduling setting: {}", e)))?;
        }

        // Routing configuration
        if let Ok(load_balancing) = std::env::var("VALKYRIE_LOAD_BALANCING_STRATEGY") {
            config.routing.load_balancing = match load_balancing.to_lowercase().as_str() {
                "roundrobin" | "round_robin" => LoadBalancingStrategy::RoundRobin,
                "leastconnections" | "least_connections" => LoadBalancingStrategy::LeastConnections,
                "weightedroundrobin" | "weighted_round_robin" => LoadBalancingStrategy::WeightedRoundRobin,
                "consistenthashing" | "consistent_hashing" => LoadBalancingStrategy::ConsistentHashing,
                _ => return Err(AppError::ConfigError(format!("Invalid load balancing strategy: {}", load_balancing))),
            };
        }

        if let Ok(routing_table_size) = std::env::var("VALKYRIE_ROUTING_TABLE_SIZE") {
            let size: usize = routing_table_size.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid routing table size: {}", e)))?;
            config.routing.routing_table_size = size;
        }

        if let Ok(route_cache_ttl) = std::env::var("VALKYRIE_ROUTE_CACHE_TTL_SECS") {
            let secs: u64 = route_cache_ttl.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid route cache TTL: {}", e)))?;
            config.routing.route_cache_ttl = Duration::from_secs(secs);
        }

        // Performance configuration
        if let Ok(worker_threads) = std::env::var("VALKYRIE_WORKER_THREADS") {
            let threads: usize = worker_threads.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid worker threads: {}", e)))?;
            config.performance.worker_threads = Some(threads);
        }

        if let Ok(batch_size) = std::env::var("VALKYRIE_MESSAGE_BATCH_SIZE") {
            let size: usize = batch_size.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid message batch size: {}", e)))?;
            config.performance.message_batch_size = size;
        }

        if let Ok(zero_copy) = std::env::var("VALKYRIE_ZERO_COPY") {
            config.performance.zero_copy = zero_copy.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid zero copy setting: {}", e)))?;
        }

        if let Ok(simd) = std::env::var("VALKYRIE_SIMD") {
            config.performance.simd = simd.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid SIMD setting: {}", e)))?;
        }

        // Observability configuration
        if let Ok(metrics_enabled) = std::env::var("VALKYRIE_METRICS_ENABLED") {
            config.observability.metrics.enabled = metrics_enabled.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid metrics enabled setting: {}", e)))?;
        }

        if let Ok(export_interval) = std::env::var("VALKYRIE_METRICS_EXPORT_INTERVAL_SECS") {
            let secs: u64 = export_interval.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid metrics export interval: {}", e)))?;
            config.observability.metrics.export_interval = Duration::from_secs(secs);
        }

        if let Ok(tracing_enabled) = std::env::var("VALKYRIE_TRACING_ENABLED") {
            config.observability.tracing.enabled = tracing_enabled.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid tracing enabled setting: {}", e)))?;
        }

        if let Ok(sampling_rate) = std::env::var("VALKYRIE_TRACING_SAMPLING_RATE") {
            let rate: f64 = sampling_rate.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid sampling rate: {}", e)))?;
            if rate < 0.0 || rate > 1.0 {
                return Err(AppError::ConfigError("Sampling rate must be between 0.0 and 1.0".to_string()));
            }
            config.observability.tracing.sampling_rate = rate;
        }

        if let Ok(export_endpoint) = std::env::var("VALKYRIE_TRACING_EXPORT_ENDPOINT") {
            config.observability.tracing.export_endpoint = Some(export_endpoint);
        }

        if let Ok(log_level) = std::env::var("VALKYRIE_LOG_LEVEL") {
            config.observability.logging.level = log_level;
        }

        if let Ok(log_format) = std::env::var("VALKYRIE_LOG_FORMAT") {
            config.observability.logging.format = match log_format.to_lowercase().as_str() {
                "text" => LogFormat::Text,
                "json" => LogFormat::Json,
                custom => LogFormat::Custom(custom.to_string()),
            };
        }

        if let Ok(structured_logging) = std::env::var("VALKYRIE_STRUCTURED_LOGGING") {
            config.observability.logging.structured = structured_logging.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid structured logging setting: {}", e)))?;
        }

        // Feature flags
        if let Ok(experimental) = std::env::var("VALKYRIE_EXPERIMENTAL_FEATURES") {
            config.features.experimental = experimental.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid experimental features setting: {}", e)))?;
        }

        if let Ok(post_quantum) = std::env::var("VALKYRIE_POST_QUANTUM_CRYPTO") {
            config.features.post_quantum_crypto = post_quantum.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid post-quantum crypto setting: {}", e)))?;
        }

        if let Ok(ml_optimizations) = std::env::var("VALKYRIE_ML_OPTIMIZATIONS") {
            config.features.ml_optimizations = ml_optimizations.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid ML optimizations setting: {}", e)))?;
        }

        // Custom feature flags (format: KEY1=value1,KEY2=value2)
        if let Ok(custom_flags) = std::env::var("VALKYRIE_CUSTOM_FEATURES") {
            for flag in custom_flags.split(',') {
                if let Some((key, value)) = flag.split_once('=') {
                    let bool_value = value.trim().parse()
                        .map_err(|e| AppError::ConfigError(format!("Invalid custom feature flag value for {}: {}", key, e)))?;
                    config.features.custom.insert(key.trim().to_string(), bool_value);
                }
            }
        }

        self.config = config;
        self.source = "environment".to_string();

        Ok(())
    }

    /// Load configuration from a YAML file
    pub async fn load_from_file(&mut self, path: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| AppError::ConfigError(format!("Failed to read Valkyrie config file {}: {}", path, e)))?;

        let config: ValkyrieConfig = serde_yaml::from_str(&content)
            .map_err(|e| AppError::ConfigError(format!("Invalid Valkyrie YAML config: {}", e)))?;

        self.validate_config(&config)?;
        self.config = config;
        self.source = path.to_string();

        Ok(())
    }

    /// Load configuration from JSON
    pub fn load_from_json(&mut self, json: &str) -> Result<()> {
        let config: ValkyrieConfig = serde_json::from_str(json)
            .map_err(|e| AppError::ConfigError(format!("Invalid Valkyrie JSON config: {}", e)))?;

        self.validate_config(&config)?;
        self.config = config;
        self.source = "json".to_string();

        Ok(())
    }

    /// Validate the configuration
    pub fn validate_config(&self, config: &ValkyrieConfig) -> Result<()> {
        // Validate protocol version
        if config.protocol.version.is_empty() {
            return Err(AppError::ConfigError("Protocol version cannot be empty".to_string()));
        }

        // Validate timeouts
        if config.protocol.timeouts.connect_timeout.as_secs() == 0 {
            return Err(AppError::ConfigError("Connect timeout must be greater than 0".to_string()));
        }
        if config.protocol.timeouts.send_timeout.as_secs() == 0 {
            return Err(AppError::ConfigError("Send timeout must be greater than 0".to_string()));
        }
        if config.protocol.timeouts.heartbeat_interval.as_secs() == 0 {
            return Err(AppError::ConfigError("Heartbeat interval must be greater than 0".to_string()));
        }

        // Validate transport configuration
        if config.transport.connection_pooling.max_connections_per_endpoint == 0 {
            return Err(AppError::ConfigError("Max connections per endpoint must be greater than 0".to_string()));
        }

        // Validate security configuration
        if config.security.authentication.methods.is_empty() {
            return Err(AppError::ConfigError("At least one authentication method must be specified".to_string()));
        }
        if config.security.encryption.cipher_suites.is_empty() {
            return Err(AppError::ConfigError("At least one cipher suite must be specified".to_string()));
        }

        // Validate streaming configuration
        if config.streaming.max_streams_per_connection == 0 {
            return Err(AppError::ConfigError("Max streams per connection must be greater than 0".to_string()));
        }
        if config.streaming.buffer_size == 0 {
            return Err(AppError::ConfigError("Stream buffer size must be greater than 0".to_string()));
        }
        if config.streaming.flow_control.initial_window_size == 0 {
            return Err(AppError::ConfigError("Initial window size must be greater than 0".to_string()));
        }
        if config.streaming.flow_control.max_window_size < config.streaming.flow_control.initial_window_size {
            return Err(AppError::ConfigError("Max window size must be greater than or equal to initial window size".to_string()));
        }
        if config.streaming.flow_control.update_threshold <= 0.0 || config.streaming.flow_control.update_threshold > 1.0 {
            return Err(AppError::ConfigError("Update threshold must be between 0.0 and 1.0".to_string()));
        }

        // Validate routing configuration
        if config.routing.routing_table_size == 0 {
            return Err(AppError::ConfigError("Routing table size must be greater than 0".to_string()));
        }

        // Validate performance configuration
        if let Some(threads) = config.performance.worker_threads {
            if threads == 0 {
                return Err(AppError::ConfigError("Worker threads must be greater than 0".to_string()));
            }
        }
        if config.performance.message_batch_size == 0 {
            return Err(AppError::ConfigError("Message batch size must be greater than 0".to_string()));
        }

        // Validate observability configuration
        if config.observability.tracing.sampling_rate < 0.0 || config.observability.tracing.sampling_rate > 1.0 {
            return Err(AppError::ConfigError("Tracing sampling rate must be between 0.0 and 1.0".to_string()));
        }

        // Validate log level
        match config.observability.logging.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(AppError::ConfigError(format!("Invalid log level: {}", config.observability.logging.level))),
        }

        Ok(())
    }

    /// Get the current configuration
    pub fn get_config(&self) -> &ValkyrieConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn get_config_mut(&mut self) -> &mut ValkyrieConfig {
        &mut self.config
    }

    /// Get the configuration source
    pub fn get_source(&self) -> &str {
        &self.source
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: ValkyrieConfig) -> Result<()> {
        self.validate_config(&config)?;
        self.config = config;
        self.source = "updated".to_string();
        Ok(())
    }

    /// Export configuration to YAML
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(&self.config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize config to YAML: {}", e)))
    }

    /// Export configuration to JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize config to JSON: {}", e)))
    }

    /// Create a configuration template with comments
    pub fn create_template() -> String {
        r#"# Valkyrie Protocol Configuration Template
# This file contains all available configuration options for the Valkyrie Protocol

# Protocol-level configuration
protocol:
  version: "1.0.0"                    # Protocol version
  supported_messages:                 # Supported message types
    - "heartbeat"
    - "data"
    - "control"
  extensions: []                      # Protocol extensions
  compatibility_mode: true            # Enable backward compatibility
  timeouts:
    connect_timeout: "30s"            # Connection establishment timeout
    send_timeout: "10s"               # Message send timeout
    heartbeat_interval: "30s"         # Heartbeat interval
    idle_timeout: "300s"              # Connection idle timeout

# Transport layer configuration
transport:
  primary:                            # Primary transport configuration
    transport_type: "Tcp"
    bind_address: "0.0.0.0"
    port: 8080
  fallbacks: []                       # Fallback transports
  selection_strategy: "Primary"       # Transport selection strategy
  connection_pooling:
    max_connections_per_endpoint: 10  # Max connections per endpoint
    idle_timeout: "300s"              # Connection idle timeout
    reuse_strategy: "PerEndpoint"     # Connection reuse strategy

# Security configuration
security:
  authentication:
    methods: ["Token"]                # Authentication methods
    token_expiry: "3600s"             # Token expiration time
    require_mutual_auth: false        # Require mutual authentication
  encryption:
    cipher_suites: ["Aes256Gcm"]      # Supported cipher suites
    key_rotation_interval: "86400s"   # Key rotation interval
    forward_secrecy: true             # Enable forward secrecy
  authorization:
    enable_rbac: true                 # Enable role-based access control
    default_permissions: ["read"]     # Default permissions
    cache_ttl: "300s"                 # Permission cache TTL
  audit:
    enabled: true                     # Enable audit logging
    retention_period: "2592000s"      # Audit log retention (30 days)
    events: ["Connection", "Authentication"]  # Events to audit

# Stream multiplexing configuration
streaming:
  max_streams_per_connection: 100     # Max streams per connection
  buffer_size: 8192                   # Stream buffer size
  flow_control:
    initial_window_size: 65536        # Initial flow control window size
    max_window_size: 1048576          # Maximum window size
    update_threshold: 0.5             # Window update threshold
  priority_scheduling: true           # Enable priority scheduling

# Routing configuration
routing:
  load_balancing: "RoundRobin"        # Load balancing strategy
  routing_table_size: 1000            # Routing table size
  route_cache_ttl: "300s"             # Route cache TTL

# Performance configuration
performance:
  worker_threads: null                # Worker thread count (null = auto)
  message_batch_size: 100             # Message batch size
  zero_copy: true                     # Enable zero-copy optimizations
  simd: true                          # Enable SIMD optimizations

# Observability configuration
observability:
  metrics:
    enabled: true                     # Enable metrics collection
    export_interval: "60s"            # Metrics export interval
    retention_period: "86400s"        # Metrics retention period
  tracing:
    enabled: true                     # Enable distributed tracing
    sampling_rate: 0.1                # Trace sampling rate
    export_endpoint: null             # Trace export endpoint
  logging:
    level: "info"                     # Log level
    format: "Json"                    # Log format
    structured: true                  # Enable structured logging

# Feature flags
features:
  experimental: false                 # Enable experimental features
  post_quantum_crypto: false          # Enable post-quantum cryptography
  ml_optimizations: false             # Enable ML optimizations
  custom: {}                          # Custom feature flags
"#.to_string()
    }

    /// Validate protocol version format
    fn is_valid_version(&self, version: &str) -> bool {
        // Version should be in format X.Y.Z or X.Y
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return false;
        }
        
        parts.iter().all(|part| part.parse::<u32>().is_ok())
    }
    
    /// Validate bind address format
    fn is_valid_bind_address(&self, address: &str) -> bool {
        // Should be in format IP:PORT or HOST:PORT
        if let Some((host, port)) = address.rsplit_once(':') {
            if let Ok(port_num) = port.parse::<u16>() {
                return port_num > 0 && (host == "0.0.0.0" || host.parse::<std::net::IpAddr>().is_ok() || self.is_valid_hostname(host));
            }
        }
        false
    }
    
    /// Validate hostname format
    fn is_valid_hostname(&self, hostname: &str) -> bool {
        !hostname.is_empty() && 
        hostname.len() <= 253 &&
        hostname.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') &&
        !hostname.starts_with('-') &&
        !hostname.ends_with('-')
    }
    
    /// Validate log level
    fn is_valid_log_level(&self, level: &str) -> bool {
        matches!(level.to_lowercase().as_str(), "trace" | "debug" | "info" | "warn" | "error")
    }
    
    /// Validate URL format
    fn is_valid_url(&self, url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }
    
    /// Validate endpoint format (host:port)
    fn is_valid_endpoint(&self, endpoint: &str) -> bool {
        self.is_valid_bind_address(endpoint)
    }
    
    /// Load configuration with environment variable substitution
    pub fn load_with_env_substitution(&mut self, yaml_content: &str) -> Result<()> {
        let substituted = self.substitute_env_vars(yaml_content)?;
        let config: ValkyrieConfig = serde_yaml::from_str(&substituted)
            .map_err(|e| AppError::ConfigError(format!("Invalid Valkyrie YAML config: {}", e)))?;
        
        self.validate_config(&config)?;
        self.config = config;
        self.source = "yaml_with_env_substitution".to_string();
        
        Ok(())
    }
    
    /// Substitute environment variables in configuration
    fn substitute_env_vars(&self, content: &str) -> Result<String> {
        let mut result = content.to_string();
        
        // Find all ${VAR} or ${VAR:-default} patterns
        let re = regex::Regex::new(r"\$\{([^}]+)\}")
            .map_err(|e| AppError::ConfigError(format!("Regex error: {}", e)))?;
        
        for cap in re.captures_iter(content) {
            let full_match = &cap[0];
            let var_expr = &cap[1];
            
            let (var_name, default_value) = if let Some((name, default)) = var_expr.split_once(":-") {
                (name, Some(default))
            } else {
                (var_expr, None)
            };
            
            let value = match std::env::var(var_name) {
                Ok(val) => val,
                Err(_) => {
                    if let Some(default) = default_value {
                        default.to_string()
                    } else {
                        return Err(AppError::ConfigError(format!("Environment variable {} not found and no default provided", var_name)));
                    }
                }
            };
            
            result = result.replace(full_match, &value);
        }
        
        Ok(result)
    }
    
    /// Validate configuration with detailed error reporting
    pub fn validate_config_detailed(&self, config: &ValkyrieConfig) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        
        // Check for potential performance issues
        if let Some(threads) = config.performance.worker_threads {
            let cpu_count = num_cpus::get();
            if threads > cpu_count * 2 {
                warnings.push(format!("Worker threads ({}) exceeds 2x CPU count ({}), may cause context switching overhead", threads, cpu_count));
            }
        }
        
        // Check security settings
        if config.security.authentication.methods.contains(&AuthMethod::None) && config.security.authentication.methods.len() > 1 {
            warnings.push("Authentication includes 'None' method alongside others, which may be insecure".to_string());
        }
        
        // Check observability settings
        if config.observability.tracing.enabled && config.observability.tracing.sampling_rate == 1.0 {
            warnings.push("Tracing sampling rate is 100%, which may impact performance in production".to_string());
        }
        
        // Check streaming configuration
        if config.streaming.max_streams_per_connection > 10000 {
            warnings.push("Very high max streams per connection may cause memory issues".to_string());
        }
        
        // Validate the config first
        self.validate_config(config)?;
        
        Ok(warnings)
    }
    
    /// Get environment variables that affect Valkyrie configuration
    pub fn get_env_vars() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            // (Variable Name, Description, Default/Example)
            ("VALKYRIE_PROTOCOL_VERSION", "Protocol version", "2.0"),
            ("VALKYRIE_BIND_ADDRESS", "Bind address for Valkyrie protocol", "0.0.0.0:8080"),
            ("VALKYRIE_MAX_MESSAGE_SIZE", "Maximum message size in bytes", "67108864"),
            ("VALKYRIE_TOKEN_SECRET", "Secret key for token authentication", "your-secret-key"),
            ("VALKYRIE_BRIDGE_ENABLED", "Enable HTTP/HTTPS bridge", "true"),
            ("VALKYRIE_HTTP_PORT", "HTTP bridge port", "8081"),
            ("VALKYRIE_HTTPS_PORT", "HTTPS bridge port", "8443"),
            ("VALKYRIE_CERT_FILE", "TLS certificate file path", "/etc/valkyrie/certs/server.crt"),
            ("VALKYRIE_KEY_FILE", "TLS private key file path", "/etc/valkyrie/certs/server.key"),
            ("VALKYRIE_MAX_CONNECTIONS", "Maximum concurrent connections", "100000"),
            ("VALKYRIE_WORKER_THREADS", "Number of worker threads", "auto"),
            ("VALKYRIE_LOG_LEVEL", "Logging level", "info"),
            ("VALKYRIE_METRICS_ENDPOINT", "Metrics endpoint path", "/metrics"),
            ("VALKYRIE_JAEGER_ENDPOINT", "Jaeger tracing endpoint", "http://localhost:14268/api/traces"),
            ("VALKYRIE_NODE_ID", "Cluster node identifier", "node-1"),
            ("VALKYRIE_CLUSTER_NODES", "Comma-separated cluster nodes", "node1:8080,node2:8080"),
            ("VALKYRIE_CONNECT_TIMEOUT_SECS", "Connection timeout in seconds", "30"),
            ("VALKYRIE_SEND_TIMEOUT_SECS", "Send timeout in seconds", "10"),
            ("VALKYRIE_HEARTBEAT_INTERVAL_SECS", "Heartbeat interval in seconds", "30"),
            ("VALKYRIE_IDLE_TIMEOUT_SECS", "Idle timeout in seconds", "300"),
            ("VALKYRIE_TRANSPORT_SELECTION_STRATEGY", "Transport selection strategy", "primary"),
            ("VALKYRIE_MAX_CONNECTIONS_PER_ENDPOINT", "Max connections per endpoint", "10"),
            ("VALKYRIE_CONNECTION_REUSE_STRATEGY", "Connection reuse strategy", "per_endpoint"),
            ("VALKYRIE_AUTH_METHODS", "Comma-separated auth methods", "token"),
            ("VALKYRIE_TOKEN_EXPIRY_SECS", "Token expiry in seconds", "3600"),
            ("VALKYRIE_REQUIRE_MUTUAL_AUTH", "Require mutual authentication", "false"),
            ("VALKYRIE_CIPHER_SUITES", "Comma-separated cipher suites", "aes256gcm"),
            ("VALKYRIE_KEY_ROTATION_INTERVAL_SECS", "Key rotation interval", "86400"),
            ("VALKYRIE_FORWARD_SECRECY", "Enable forward secrecy", "true"),
            ("VALKYRIE_MAX_STREAMS_PER_CONNECTION", "Max streams per connection", "100"),
            ("VALKYRIE_STREAM_BUFFER_SIZE", "Stream buffer size", "8192"),
            ("VALKYRIE_INITIAL_WINDOW_SIZE", "Initial flow control window", "65536"),
            ("VALKYRIE_MAX_WINDOW_SIZE", "Maximum flow control window", "1048576"),
            ("VALKYRIE_PRIORITY_SCHEDULING", "Enable priority scheduling", "true"),
            ("VALKYRIE_LOAD_BALANCING_STRATEGY", "Load balancing strategy", "round_robin"),
            ("VALKYRIE_ROUTING_TABLE_SIZE", "Routing table size", "1000"),
            ("VALKYRIE_ROUTE_CACHE_TTL_SECS", "Route cache TTL", "300"),
            ("VALKYRIE_MESSAGE_BATCH_SIZE", "Message batch size", "100"),
            ("VALKYRIE_ZERO_COPY", "Enable zero-copy optimizations", "true"),
            ("VALKYRIE_SIMD", "Enable SIMD optimizations", "true"),
            ("VALKYRIE_METRICS_ENABLED", "Enable metrics collection", "true"),
            ("VALKYRIE_METRICS_EXPORT_INTERVAL_SECS", "Metrics export interval", "60"),
            ("VALKYRIE_TRACING_ENABLED", "Enable distributed tracing", "true"),
            ("VALKYRIE_TRACING_SAMPLING_RATE", "Tracing sampling rate", "0.1"),
            ("VALKYRIE_TRACING_EXPORT_ENDPOINT", "Tracing export endpoint", "http://localhost:14268/api/traces"),
            ("VALKYRIE_LOG_FORMAT", "Log format", "json"),
            ("VALKYRIE_STRUCTURED_LOGGING", "Enable structured logging", "true"),
            ("VALKYRIE_EXPERIMENTAL_FEATURES", "Enable experimental features", "false"),
            ("VALKYRIE_POST_QUANTUM_CRYPTO", "Enable post-quantum crypto", "false"),
            ("VALKYRIE_ML_OPTIMIZATIONS", "Enable ML optimizations", "false"),
            ("VALKYRIE_CUSTOM_FEATURES", "Custom feature flags", "feature1=true,feature2=false"),
        ]
    }

    /// Get configuration summary for debugging
    pub fn get_summary(&self) -> ConfigSummary {
        ConfigSummary {
            source: self.source.clone(),
            protocol_version: self.config.protocol.version.clone(),
            transport_strategy: format!("{:?}", self.config.transport.selection_strategy),
            auth_methods: self.config.security.authentication.methods.len(),
            cipher_suites: self.config.security.encryption.cipher_suites.len(),
            max_streams: self.config.streaming.max_streams_per_connection,
            load_balancing: format!("{:?}", self.config.routing.load_balancing),
            metrics_enabled: self.config.observability.metrics.enabled,
            tracing_enabled: self.config.observability.tracing.enabled,
            experimental_features: self.config.features.experimental,
        }
    }
}

impl Default for ValkyrieConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration summary for debugging and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    pub source: String,
    pub protocol_version: String,
    pub transport_strategy: String,
    pub auth_methods: usize,
    pub cipher_suites: usize,
    pub max_streams: u32,
    pub load_balancing: String,
    pub metrics_enabled: bool,
    pub tracing_enabled: bool,
    pub experimental_features: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let manager = ValkyrieConfigManager::new();
        let config = manager.get_config();
        
        assert_eq!(config.protocol.version, "1.0.0");
        assert!(config.security.authentication.methods.contains(&AuthMethod::Token));
        assert!(config.observability.metrics.enabled);
    }

    #[test]
    fn test_config_validation() {
        let manager = ValkyrieConfigManager::new();
        let config = manager.get_config();
        
        // Valid config should pass
        assert!(manager.validate_config(config).is_ok());
        
        // Invalid config should fail
        let mut invalid_config = config.clone();
        invalid_config.protocol.version = "".to_string();
        assert!(manager.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_env_config_loading() {
        // Set test environment variables
        env::set_var("VALKYRIE_PROTOCOL_VERSION", "2.0.0");
        env::set_var("VALKYRIE_METRICS_ENABLED", "false");
        env::set_var("VALKYRIE_MAX_STREAMS_PER_CONNECTION", "200");
        
        let mut manager = ValkyrieConfigManager::new();
        assert!(manager.load_from_env().is_ok());
        
        let config = manager.get_config();
        assert_eq!(config.protocol.version, "2.0.0");
        assert!(!config.observability.metrics.enabled);
        assert_eq!(config.streaming.max_streams_per_connection, 200);
        
        // Clean up
        env::remove_var("VALKYRIE_PROTOCOL_VERSION");
        env::remove_var("VALKYRIE_METRICS_ENABLED");
        env::remove_var("VALKYRIE_MAX_STREAMS_PER_CONNECTION");
    }

    #[test]
    fn test_json_serialization() {
        let manager = ValkyrieConfigManager::new();
        let json = manager.to_json().unwrap();
        
        let mut new_manager = ValkyrieConfigManager::new();
        assert!(new_manager.load_from_json(&json).is_ok());
        
        // Configs should be equivalent
        assert_eq!(manager.get_config().protocol.version, new_manager.get_config().protocol.version);
    }

    #[test]
    fn test_config_template() {
        let template = ValkyrieConfigManager::create_template();
        assert!(template.contains("Valkyrie Protocol Configuration Template"));
        assert!(template.contains("protocol:"));
        assert!(template.contains("security:"));
        assert!(template.contains("observability:"));
    }

    #[test]
    fn test_config_summary() {
        let manager = ValkyrieConfigManager::new();
        let summary = manager.get_summary();
        
        assert_eq!(summary.protocol_version, "1.0.0");
        assert!(summary.metrics_enabled);
        assert!(!summary.experimental_features);
    }
}