//! Valkyrie Protocol Configuration Management
//!
//! This module provides comprehensive configuration management for Valkyrie Protocol
//! with support for environment-specific overrides, validation, and hot reloading.

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info};

/// Main Valkyrie configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieConfig {
    /// Global settings
    pub global: GlobalConfig,
    /// Server configuration
    pub server: Option<ServerConfig>,
    /// Client configuration
    pub client: Option<ClientConfig>,
    /// Transport configuration
    pub transport: Option<TransportConfig>,
    /// Security configuration
    pub security: Option<SecurityConfig>,
    /// Plugin system configuration
    pub plugins: Option<PluginConfig>,
    /// Fallback system configuration
    pub fallback: Option<FallbackConfig>,
    /// Observability configuration
    pub observability: Option<ObservabilityConfig>,
    /// Performance tuning configuration
    pub performance: Option<PerformanceConfig>,
    /// Environment-specific overrides
    pub overrides: Option<HashMap<String, serde_json::Value>>,
}

/// Global configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Environment (development, staging, production)
    pub environment: String,
    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics endpoint for external systems
    pub metrics_endpoint: Option<String>,
    /// Configuration hot-reload interval in seconds
    pub config_reload_interval: Option<u64>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address
    pub bind_address: String,
    /// Port number
    pub port: u16,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
    /// Enable TLS
    pub enable_tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS private key path
    pub tls_key_path: Option<String>,
    /// Enable graceful shutdown
    pub enable_graceful_shutdown: Option<bool>,
    /// Graceful shutdown timeout in seconds
    pub graceful_shutdown_timeout: Option<u64>,
}

/// Client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Default server endpoint
    pub endpoint: String,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Maximum connections in pool
    pub max_connections: usize,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Enable automatic reconnection
    pub auto_reconnect: Option<bool>,
    /// Reconnection interval in milliseconds
    pub reconnect_interval_ms: Option<u64>,
    /// Retry policy configuration
    pub retry_policy: Option<RetryPolicyConfig>,
}

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicyConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Enable jitter to avoid thundering herd
    pub enable_jitter: bool,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Available transports
    pub enabled_transports: Vec<String>,
    /// Default transport
    pub default_transport: String,
    /// TCP transport settings
    pub tcp: Option<TcpConfig>,
    /// QUIC transport settings
    pub quic: Option<QuicConfig>,
    /// Unix socket settings
    pub unix: Option<UnixConfig>,
    /// WebSocket settings
    pub websocket: Option<WebSocketConfig>,
}

/// TCP transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    /// TCP keep-alive settings
    pub keep_alive: Option<bool>,
    /// Keep-alive interval in seconds
    pub keep_alive_interval: Option<u64>,
    /// TCP no-delay (Nagle's algorithm)
    pub no_delay: Option<bool>,
    /// Socket buffer sizes
    pub send_buffer_size: Option<usize>,
    pub recv_buffer_size: Option<usize>,
}

/// QUIC transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicConfig {
    /// Maximum concurrent streams
    pub max_concurrent_streams: Option<usize>,
    /// Connection idle timeout in seconds
    pub idle_timeout: Option<u64>,
    /// Enable 0-RTT
    pub enable_0rtt: Option<bool>,
}

/// Unix socket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnixConfig {
    /// Socket file path
    pub socket_path: String,
    /// Socket permissions
    pub socket_permissions: Option<u32>,
}

/// WebSocket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Maximum message size in bytes
    pub max_message_size: Option<usize>,
    /// Enable compression
    pub enable_compression: Option<bool>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable encryption
    pub enable_encryption: Option<bool>,
    /// Encryption algorithm
    pub encryption_algorithm: Option<String>,
    /// Enable authentication
    pub enable_authentication: Option<bool>,
    /// Authentication method
    pub authentication_method: Option<String>,
    /// JWT settings
    pub jwt: Option<JwtConfig>,
    /// API Key settings
    pub api_key: Option<ApiKeyConfig>,
    /// mTLS settings
    pub mtls: Option<MtlsConfig>,
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// JWT secret key
    pub secret_key: String,
    /// Token expiration time in seconds
    pub expiration_time: Option<u64>,
    /// Issuer
    pub issuer: Option<String>,
    /// Audience
    pub audience: Option<String>,
}

/// API Key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// API key header name
    pub header_name: String,
    /// Valid API keys
    pub valid_keys: Vec<String>,
}

/// mTLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// CA certificate path
    pub ca_cert_path: String,
    /// Client certificate verification
    pub verify_client_cert: Option<bool>,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Enable plugin system
    pub enabled: Option<bool>,
    /// Plugin directory
    pub plugin_dir: Option<String>,
    /// Enable hot reloading
    pub enable_hot_reload: Option<bool>,
    /// Health check interval in seconds
    pub health_check_interval: Option<u64>,
    /// Maximum plugin startup time in seconds
    pub max_startup_time: Option<u64>,
    /// Auto-restart failed plugins
    pub auto_restart: Option<bool>,
    /// Maximum restart attempts
    pub max_restart_attempts: Option<u32>,
    /// Valkyrie plugin configuration
    pub valkyrie: Option<ValkyriePluginConfig>,
}

/// Valkyrie plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyriePluginConfig {
    /// Enable Valkyrie plugin
    pub enabled: Option<bool>,
    /// Valkyrie server endpoint
    pub endpoint: Option<String>,
    /// Connection settings
    pub connection_timeout_ms: Option<u64>,
    pub request_timeout_ms: Option<u64>,
    pub max_connections: Option<usize>,
    pub enable_pooling: Option<bool>,
    /// Health monitoring
    pub health_check_interval: Option<u64>,
    pub enable_metrics: Option<bool>,
    /// Fallback settings
    pub enable_http_fallback: Option<bool>,
}

/// Fallback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Enable fallback system
    pub enabled: Option<bool>,
    /// Fallback detection timeout in milliseconds
    pub detection_timeout_ms: Option<u64>,
    /// Maximum fallback attempts
    pub max_attempts: Option<u32>,
    /// Retry interval in milliseconds
    pub retry_interval_ms: Option<u64>,
    /// HTTP fallback configuration
    pub http: Option<HttpFallbackConfig>,
}

/// HTTP fallback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpFallbackConfig {
    /// Base URL for HTTP API
    pub base_url: String,
    /// Request timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Maximum retries
    pub max_retries: Option<u32>,
    /// Enable compression
    pub enable_compression: Option<bool>,
}

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable observability
    pub enabled: Option<bool>,
    /// Metrics configuration
    pub metrics: Option<MetricsConfig>,
    /// Tracing configuration
    pub tracing: Option<TracingConfig>,
    /// Logging configuration
    pub logging: Option<LoggingConfig>,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: Option<bool>,
    /// Metrics collection interval in seconds
    pub collection_interval: Option<u64>,
    /// Prometheus endpoint
    pub prometheus_endpoint: Option<String>,
    /// Enable custom metrics
    pub enable_custom_metrics: Option<bool>,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: Option<bool>,
    /// Tracing backend
    pub backend: Option<String>,
    /// Jaeger settings
    pub jaeger: Option<JaegerConfig>,
    /// Sampling rate
    pub sampling_rate: Option<f64>,
}

/// Jaeger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JaegerConfig {
    /// Jaeger agent endpoint
    pub agent_endpoint: Option<String>,
    /// Service name
    pub service_name: Option<String>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log format
    pub format: Option<String>,
    /// Enable structured logging
    pub structured: Option<bool>,
    /// Log file path
    pub file_path: Option<String>,
    /// Log rotation
    pub rotation: Option<LogRotationConfig>,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Enable log rotation
    pub enabled: Option<bool>,
    /// Maximum file size in MB
    pub max_size_mb: Option<u64>,
    /// Maximum number of files to keep
    pub max_files: Option<u32>,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Thread pool configuration
    pub thread_pool: Option<ThreadPoolConfig>,
    /// Memory configuration
    pub memory: Option<MemoryConfig>,
    /// Network tuning
    pub network: Option<NetworkConfig>,
}

/// Thread pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadPoolConfig {
    /// Core thread count
    pub core_threads: Option<usize>,
    /// Maximum thread count
    pub max_threads: Option<usize>,
    /// Thread keep-alive time in seconds
    pub keep_alive_time: Option<u64>,
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Enable memory pooling
    pub enable_pooling: Option<bool>,
    /// Buffer pool size
    pub buffer_pool_size: Option<usize>,
    /// Maximum buffer size in bytes
    pub max_buffer_size: Option<usize>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable TCP fast open
    pub tcp_fast_open: Option<bool>,
    /// Socket reuse settings
    pub reuse_address: Option<bool>,
    pub reuse_port: Option<bool>,
    /// Network buffer sizes
    pub send_buffer_size: Option<usize>,
    pub recv_buffer_size: Option<usize>,
}

impl ValkyrieConfig {
    /// Load configuration from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading Valkyrie configuration from: {}", path.display());

        let content = std::fs::read_to_string(path).map_err(|e| {
            AppError::ConfigurationError(format!("Failed to read config file: {}", e))
        })?;

        let config: Self = match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => serde_yaml::from_str(&content).map_err(|e| {
                AppError::ConfigurationError(format!("Failed to parse YAML config: {}", e))
            })?,
            Some("json") => serde_json::from_str(&content).map_err(|e| {
                AppError::ConfigurationError(format!("Failed to parse JSON config: {}", e))
            })?,
            _ => {
                return Err(AppError::ConfigurationError(
                    "Unsupported config file format. Use .yaml or .json".to_string(),
                ));
            }
        };

        debug!("Configuration loaded successfully");
        Ok(config)
    }

    /// Save configuration to a file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        info!("Saving Valkyrie configuration to: {}", path.display());

        let content = match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => serde_yaml::to_string(self).map_err(|e| {
                AppError::ConfigurationError(format!("Failed to serialize to YAML: {}", e))
            })?,
            Some("json") => serde_json::to_string_pretty(self).map_err(|e| {
                AppError::ConfigurationError(format!("Failed to serialize to JSON: {}", e))
            })?,
            _ => {
                return Err(AppError::ConfigurationError(
                    "Unsupported config file format. Use .yaml or .json".to_string(),
                ));
            }
        };

        std::fs::write(path, content).map_err(|e| {
            AppError::ConfigurationError(format!("Failed to write config file: {}", e))
        })?;

        debug!("Configuration saved successfully");
        Ok(())
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        info!("Loading Valkyrie configuration from environment variables");

        let environment =
            std::env::var("VALKYRIE_ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        let log_level = std::env::var("VALKYRIE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        let global = GlobalConfig {
            environment,
            log_level,
            enable_metrics: std::env::var("VALKYRIE_ENABLE_METRICS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
            metrics_endpoint: std::env::var("VALKYRIE_METRICS_ENDPOINT").ok(),
            config_reload_interval: std::env::var("VALKYRIE_CONFIG_RELOAD_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok()),
        };

        let server = if std::env::var("VALKYRIE_SERVER_ENABLED").unwrap_or_default() == "true" {
            Some(ServerConfig {
                bind_address: std::env::var("VALKYRIE_SERVER_BIND")
                    .unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: std::env::var("VALKYRIE_SERVER_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(8080),
                max_connections: std::env::var("VALKYRIE_SERVER_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10000),
                connection_timeout_ms: std::env::var("VALKYRIE_SERVER_CONNECTION_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30000),
                enable_tls: std::env::var("VALKYRIE_SERVER_TLS")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false),
                tls_cert_path: std::env::var("VALKYRIE_SERVER_TLS_CERT").ok(),
                tls_key_path: std::env::var("VALKYRIE_SERVER_TLS_KEY").ok(),
                enable_graceful_shutdown: Some(true),
                graceful_shutdown_timeout: Some(30),
            })
        } else {
            None
        };

        let client = if std::env::var("VALKYRIE_CLIENT_ENABLED").unwrap_or_default() == "true" {
            Some(ClientConfig {
                endpoint: std::env::var("VALKYRIE_CLIENT_ENDPOINT")
                    .unwrap_or_else(|_| "tcp://localhost:8080".to_string()),
                connect_timeout_ms: std::env::var("VALKYRIE_CLIENT_CONNECT_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5000),
                request_timeout_ms: std::env::var("VALKYRIE_CLIENT_REQUEST_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30000),
                max_connections: std::env::var("VALKYRIE_CLIENT_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10),
                enable_pooling: std::env::var("VALKYRIE_CLIENT_POOLING")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(true),
                auto_reconnect: Some(true),
                reconnect_interval_ms: Some(1000),
                retry_policy: None,
            })
        } else {
            None
        };

        Ok(ValkyrieConfig {
            global,
            server,
            client,
            transport: None,
            security: None,
            plugins: None,
            fallback: None,
            observability: None,
            performance: None,
            overrides: None,
        })
    }

    /// Apply environment-specific overrides
    pub fn apply_environment_overrides(&mut self) -> Result<()> {
        let environment = self.global.environment.clone();
        debug!("Applying environment overrides for: {}", environment);

        if let Some(overrides) = self.overrides.clone() {
            if let Some(env_overrides) = overrides.get(&environment) {
                info!("Applying {} environment overrides", environment);
                self.merge_overrides(env_overrides)?;
            }
        }

        Ok(())
    }

    /// Merge configuration overrides
    fn merge_overrides(&mut self, overrides: &serde_json::Value) -> Result<()> {
        // This is a simplified merge implementation
        // In a production system, you'd want a more sophisticated merge strategy
        if let serde_json::Value::Object(override_map) = overrides {
            for (key, value) in override_map {
                match key.as_str() {
                    "global" => {
                        if let Ok(global_overrides) =
                            serde_json::from_value::<GlobalConfig>(value.clone())
                        {
                            self.merge_global_config(global_overrides);
                        }
                    }
                    "server" => {
                        if let Ok(server_overrides) =
                            serde_json::from_value::<ServerConfig>(value.clone())
                        {
                            self.server = Some(server_overrides);
                        }
                    }
                    "client" => {
                        if let Ok(client_overrides) =
                            serde_json::from_value::<ClientConfig>(value.clone())
                        {
                            self.client = Some(client_overrides);
                        }
                    }
                    _ => {
                        debug!("Unknown override key: {}", key);
                    }
                }
            }
        }

        Ok(())
    }

    /// Merge global configuration
    fn merge_global_config(&mut self, overrides: GlobalConfig) {
        if !overrides.environment.is_empty() {
            self.global.environment = overrides.environment;
        }
        if !overrides.log_level.is_empty() {
            self.global.log_level = overrides.log_level;
        }
        self.global.enable_metrics = overrides.enable_metrics;
        if overrides.metrics_endpoint.is_some() {
            self.global.metrics_endpoint = overrides.metrics_endpoint;
        }
        if overrides.config_reload_interval.is_some() {
            self.global.config_reload_interval = overrides.config_reload_interval;
        }
    }

    /// Expand environment variables in configuration
    pub fn expand_env_vars(&mut self) -> Result<()> {
        debug!("Expanding environment variables in configuration");

        // This is a simplified implementation
        // In a real system, you'd recursively traverse all string fields
        if let Some(ref mut server) = self.server {
            if let Some(ref mut cert_path) = server.tls_cert_path {
                *cert_path = expand_env_var(cert_path);
            }
            if let Some(ref mut key_path) = server.tls_key_path {
                *key_path = expand_env_var(key_path);
            }
        }

        if let Some(ref mut client) = self.client {
            client.endpoint = expand_env_var(&client.endpoint);
        }

        if let Some(ref mut security) = self.security {
            if let Some(ref mut jwt) = security.jwt {
                jwt.secret_key = expand_env_var(&jwt.secret_key);
            }
        }

        Ok(())
    }

    /// Get configuration as Duration
    pub fn get_duration(&self, path: &str) -> Option<Duration> {
        // Helper method to convert millisecond values to Duration
        match path {
            "server.connection_timeout_ms" => self
                .server
                .as_ref()
                .map(|s| Duration::from_millis(s.connection_timeout_ms)),
            "client.connect_timeout_ms" => self
                .client
                .as_ref()
                .map(|c| Duration::from_millis(c.connect_timeout_ms)),
            "client.request_timeout_ms" => self
                .client
                .as_ref()
                .map(|c| Duration::from_millis(c.request_timeout_ms)),
            _ => None,
        }
    }
}

impl Default for ValkyrieConfig {
    fn default() -> Self {
        Self {
            global: GlobalConfig {
                environment: "development".to_string(),
                log_level: "info".to_string(),
                enable_metrics: false,
                metrics_endpoint: None,
                config_reload_interval: Some(30),
            },
            server: None,
            client: None,
            transport: None,
            security: None,
            plugins: None,
            fallback: None,
            observability: None,
            performance: None,
            overrides: None,
        }
    }
}

/// Expand environment variables in a string
fn expand_env_var(input: &str) -> String {
    if input.starts_with("${") && input.ends_with('}') {
        let var_spec = &input[2..input.len() - 1];

        // Handle default values: ${VAR_NAME:default_value}
        if let Some(colon_pos) = var_spec.find(':') {
            let var_name = &var_spec[..colon_pos];
            let default_value = &var_spec[colon_pos + 1..];
            std::env::var(var_name).unwrap_or_else(|_| default_value.to_string())
        } else {
            std::env::var(var_spec).unwrap_or_else(|_| input.to_string())
        }
    } else {
        input.to_string()
    }
}
