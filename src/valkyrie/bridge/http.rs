//! HTTP bridge for Valkyrie Protocol

/// HTTP bridge configuration
pub struct HttpBridgeConfig {
    /// Bridge port
    pub port: u16,
    /// Enable HTTPS
    pub enable_https: bool,
}

/// HTTP bridge for protocol translation
pub struct HttpBridge {
    config: HttpBridgeConfig,
}

impl HttpBridge {
    /// Create a new HTTP bridge
    pub fn new(config: HttpBridgeConfig) -> Self {
        Self { config }
    }
}
