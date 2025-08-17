//! Configuration management for Valkyrie SDK

use crate::{Result, ValkyrieError, client::ClientConfig, server::ServerConfig, retry::RetryPolicy};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Client configuration
    pub client: Option<ClientConfig>,
    /// Server configuration
    pub server: Option<ServerConfig>,
    /// Global settings
    pub global: GlobalConfig,
}

/// Global configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Log level
    pub log_level: String,
    /// Enable metrics
    pub enable_metrics: bool,
    /// Metrics endpoint
    pub metrics_endpoint: Option<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            enable_metrics: false,
            metrics_endpoint: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client: None,
            server: None,
            global: GlobalConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| ValkyrieError::configuration(format!("Failed to read config file: {}", e)))?;
        
        match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(&content)
                    .map_err(|e| ValkyrieError::configuration(format!("Failed to parse YAML config: {}", e)))
            }
            Some("toml") => {
                toml::from_str(&content)
                    .map_err(|e| ValkyrieError::configuration(format!("Failed to parse TOML config: {}", e)))
            }
            Some("json") => {
                serde_json::from_str(&content)
                    .map_err(|e| ValkyrieError::configuration(format!("Failed to parse JSON config: {}", e)))
            }
            _ => Err(ValkyrieError::configuration("Unsupported config file format. Use .yaml, .toml, or .json"))
        }
    }
    
    /// Save configuration to a file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        
        let content = match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::to_string(self)
                    .map_err(|e| ValkyrieError::configuration(format!("Failed to serialize to YAML: {}", e)))?
            }
            Some("toml") => {
                toml::to_string(self)
                    .map_err(|e| ValkyrieError::configuration(format!("Failed to serialize to TOML: {}", e)))?
            }
            Some("json") => {
                serde_json::to_string_pretty(self)
                    .map_err(|e| ValkyrieError::configuration(format!("Failed to serialize to JSON: {}", e)))?
            }
            _ => return Err(ValkyrieError::configuration("Unsupported config file format. Use .yaml, .toml, or .json"))
        };
        
        std::fs::write(path, content)
            .map_err(|e| ValkyrieError::configuration(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }
    
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();
        
        // Global settings
        if let Ok(log_level) = std::env::var("VALKYRIE_LOG_LEVEL") {
            config.global.log_level = log_level;
        }
        
        if let Ok(enable_metrics) = std::env::var("VALKYRIE_ENABLE_METRICS") {
            config.global.enable_metrics = enable_metrics.parse().unwrap_or(false);
        }
        
        if let Ok(metrics_endpoint) = std::env::var("VALKYRIE_METRICS_ENDPOINT") {
            config.global.metrics_endpoint = Some(metrics_endpoint);
        }
        
        // Client settings
        if let Ok(endpoint) = std::env::var("VALKYRIE_CLIENT_ENDPOINT") {
            let mut client_config = ClientConfig::default();
            client_config.endpoint = endpoint;
            
            if let Ok(timeout) = std::env::var("VALKYRIE_CLIENT_TIMEOUT_MS") {
                if let Ok(timeout_ms) = timeout.parse() {
                    client_config.request_timeout_ms = timeout_ms;
                }
            }
            
            config.client = Some(client_config);
        }
        
        // Server settings
        if let Ok(bind_address) = std::env::var("VALKYRIE_SERVER_BIND") {
            let mut server_config = ServerConfig::default();
            server_config.bind_address = bind_address;
            
            if let Ok(port) = std::env::var("VALKYRIE_SERVER_PORT") {
                if let Ok(port_num) = port.parse() {
                    server_config.port = port_num;
                }
            }
            
            config.server = Some(server_config);
        }
        
        Ok(config)
    }
    
    /// Merge with another configuration (other takes precedence)
    pub fn merge(mut self, other: Config) -> Self {
        if other.client.is_some() {
            self.client = other.client;
        }
        
        if other.server.is_some() {
            self.server = other.server;
        }
        
        // Merge global settings
        if !other.global.log_level.is_empty() {
            self.global.log_level = other.global.log_level;
        }
        
        self.global.enable_metrics = other.global.enable_metrics;
        
        if other.global.metrics_endpoint.is_some() {
            self.global.metrics_endpoint = other.global.metrics_endpoint;
        }
        
        self
    }
}

// Make ClientConfig and ServerConfig serializable
impl Serialize for ClientConfig {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ClientConfig", 7)?;
        state.serialize_field("endpoint", &self.endpoint)?;
        state.serialize_field("connect_timeout_ms", &self.connect_timeout_ms)?;
        state.serialize_field("request_timeout_ms", &self.request_timeout_ms)?;
        state.serialize_field("max_connections", &self.max_connections)?;
        state.serialize_field("enable_pooling", &self.enable_pooling)?;
        state.serialize_field("retry_policy", &self.retry_policy)?;
        state.serialize_field("enable_metrics", &self.enable_metrics)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ClientConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ClientConfigHelper {
            endpoint: Option<String>,
            connect_timeout_ms: Option<u64>,
            request_timeout_ms: Option<u64>,
            max_connections: Option<usize>,
            enable_pooling: Option<bool>,
            retry_policy: Option<RetryPolicy>,
            enable_metrics: Option<bool>,
        }
        
        let helper = ClientConfigHelper::deserialize(deserializer)?;
        let default = ClientConfig::default();
        
        Ok(ClientConfig {
            endpoint: helper.endpoint.unwrap_or(default.endpoint),
            connect_timeout_ms: helper.connect_timeout_ms.unwrap_or(default.connect_timeout_ms),
            request_timeout_ms: helper.request_timeout_ms.unwrap_or(default.request_timeout_ms),
            max_connections: helper.max_connections.unwrap_or(default.max_connections),
            enable_pooling: helper.enable_pooling.unwrap_or(default.enable_pooling),
            retry_policy: helper.retry_policy.unwrap_or(default.retry_policy),
            enable_metrics: helper.enable_metrics.unwrap_or(default.enable_metrics),
        })
    }
}

impl Serialize for ServerConfig {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ServerConfig", 8)?;
        state.serialize_field("bind_address", &self.bind_address)?;
        state.serialize_field("port", &self.port)?;
        state.serialize_field("max_connections", &self.max_connections)?;
        state.serialize_field("connection_timeout_ms", &self.connection_timeout_ms)?;
        state.serialize_field("enable_tls", &self.enable_tls)?;
        state.serialize_field("tls_cert_path", &self.tls_cert_path)?;
        state.serialize_field("tls_key_path", &self.tls_key_path)?;
        state.serialize_field("enable_metrics", &self.enable_metrics)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ServerConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ServerConfigHelper {
            bind_address: Option<String>,
            port: Option<u16>,
            max_connections: Option<usize>,
            connection_timeout_ms: Option<u64>,
            enable_tls: Option<bool>,
            tls_cert_path: Option<String>,
            tls_key_path: Option<String>,
            enable_metrics: Option<bool>,
        }
        
        let helper = ServerConfigHelper::deserialize(deserializer)?;
        let default = ServerConfig::default();
        
        Ok(ServerConfig {
            bind_address: helper.bind_address.unwrap_or(default.bind_address),
            port: helper.port.unwrap_or(default.port),
            max_connections: helper.max_connections.unwrap_or(default.max_connections),
            connection_timeout_ms: helper.connection_timeout_ms.unwrap_or(default.connection_timeout_ms),
            enable_tls: helper.enable_tls.unwrap_or(default.enable_tls),
            tls_cert_path: helper.tls_cert_path.or(default.tls_cert_path),
            tls_key_path: helper.tls_key_path.or(default.tls_key_path),
            enable_metrics: helper.enable_metrics.unwrap_or(default.enable_metrics),
        })
    }
}