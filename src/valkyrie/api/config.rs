//! Client configuration for the Valkyrie Protocol API

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Client configuration for the Valkyrie Protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Server endpoint to connect to
    pub server_endpoint: String,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Message timeout
    pub message_timeout: Duration,
    /// Security configuration
    pub security: ClientSecurityConfig,
    /// Performance configuration
    pub performance: ClientPerformanceConfig,
    /// Feature flags
    pub features: ClientFeatureFlags,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_endpoint: "tcp://localhost:8080".to_string(),
            connection_timeout: Duration::from_secs(30),
            message_timeout: Duration::from_secs(10),
            security: ClientSecurityConfig::default(),
            performance: ClientPerformanceConfig::default(),
            features: ClientFeatureFlags::default(),
        }
    }
}

/// Client security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSecurityConfig {
    /// Enable TLS
    pub enable_tls: bool,
    /// Certificate path
    pub cert_path: Option<String>,
    /// Private key path
    pub key_path: Option<String>,
    /// CA certificate path
    pub ca_cert_path: Option<String>,
    /// Authentication method
    pub auth_method: ClientAuthMethod,
}

impl Default for ClientSecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: true,
            cert_path: None,
            key_path: None,
            ca_cert_path: None,
            auth_method: ClientAuthMethod::None,
        }
    }
}

/// Client authentication methods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientAuthMethod {
    /// No authentication
    None,
    /// Token-based authentication
    Token(String),
    /// Certificate-based authentication
    Certificate,
    /// Custom authentication
    Custom(String),
}

/// Client performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPerformanceConfig {
    /// Enable compression
    pub enable_compression: bool,
    /// Compression level
    pub compression_level: u8,
    /// Connection pool size
    pub connection_pool_size: usize,
    /// Message batch size
    pub message_batch_size: usize,
}

impl Default for ClientPerformanceConfig {
    fn default() -> Self {
        Self {
            enable_compression: false,
            compression_level: 6,
            connection_pool_size: 5,
            message_batch_size: 100,
        }
    }
}

/// Client feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientFeatureFlags {
    /// Enable experimental features
    pub experimental: bool,
    /// Enable metrics collection
    pub metrics: bool,
    /// Enable tracing
    pub tracing: bool,
    /// Custom features
    pub custom: std::collections::HashMap<String, bool>,
}

impl Default for ClientFeatureFlags {
    fn default() -> Self {
        Self {
            experimental: false,
            metrics: true,
            tracing: false,
            custom: std::collections::HashMap::new(),
        }
    }
}

/// Builder for client configuration
pub struct ClientConfigBuilder {
    config: ClientConfig,
}

impl ClientConfigBuilder {
    /// Create a new client config builder
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }
    
    /// Set server endpoint
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.config.server_endpoint = endpoint.to_string();
        self
    }
    
    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.config.connection_timeout = timeout;
        self
    }
    
    /// Set message timeout
    pub fn with_message_timeout(mut self, timeout: Duration) -> Self {
        self.config.message_timeout = timeout;
        self
    }
    
    /// Enable TLS
    pub fn with_tls(mut self) -> Self {
        self.config.security.enable_tls = true;
        self
    }
    
    /// Set certificate paths
    pub fn with_certificates(mut self, cert_path: &str, key_path: &str, ca_cert_path: Option<&str>) -> Self {
        self.config.security.cert_path = Some(cert_path.to_string());
        self.config.security.key_path = Some(key_path.to_string());
        self.config.security.ca_cert_path = ca_cert_path.map(|s| s.to_string());
        self
    }
    
    /// Set authentication method
    pub fn with_auth_method(mut self, auth_method: ClientAuthMethod) -> Self {
        self.config.security.auth_method = auth_method;
        self
    }
    
    /// Enable compression
    pub fn with_compression(mut self, level: u8) -> Self {
        self.config.performance.enable_compression = true;
        self.config.performance.compression_level = level;
        self
    }
    
    /// Set connection pool size
    pub fn with_connection_pool_size(mut self, size: usize) -> Self {
        self.config.performance.connection_pool_size = size;
        self
    }
    
    /// Enable experimental features
    pub fn with_experimental_features(mut self) -> Self {
        self.config.features.experimental = true;
        self
    }
    
    /// Enable metrics
    pub fn with_metrics(mut self) -> Self {
        self.config.features.metrics = true;
        self
    }
    
    /// Enable tracing
    pub fn with_tracing(mut self) -> Self {
        self.config.features.tracing = true;
        self
    }
    
    /// Set custom feature
    pub fn with_custom_feature(mut self, name: &str, enabled: bool) -> Self {
        self.config.features.custom.insert(name.to_string(), enabled);
        self
    }
    
    /// Build the configuration
    pub fn build(self) -> ClientConfig {
        self.config
    }
}

impl Default for ClientConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.server_endpoint, "tcp://localhost:8080");
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
        assert!(config.security.enable_tls);
    }
    
    #[test]
    fn test_client_config_builder() {
        let config = ClientConfigBuilder::new()
            .with_endpoint("tcp://example.com:9090")
            .with_connection_timeout(Duration::from_secs(60))
            .with_tls()
            .with_compression(9)
            .build();
        
        assert_eq!(config.server_endpoint, "tcp://example.com:9090");
        assert_eq!(config.connection_timeout, Duration::from_secs(60));
        assert!(config.security.enable_tls);
        assert!(config.performance.enable_compression);
        assert_eq!(config.performance.compression_level, 9);
    }
    
    #[test]
    fn test_auth_method() {
        let token_auth = ClientAuthMethod::Token("test-token".to_string());
        assert!(matches!(token_auth, ClientAuthMethod::Token(_)));
        
        let cert_auth = ClientAuthMethod::Certificate;
        assert_eq!(cert_auth, ClientAuthMethod::Certificate);
    }
}