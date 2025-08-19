//! WebSocket bridge for Valkyrie Protocol

/// WebSocket bridge configuration
pub struct WebSocketBridgeConfig {
    /// Bridge port
    pub port: u16,
    /// Enable WSS
    pub enable_wss: bool,
}

/// WebSocket bridge for protocol translation
pub struct WebSocketBridge {
    config: WebSocketBridgeConfig,
}

impl WebSocketBridge {
    /// Create a new WebSocket bridge
    pub fn new(config: WebSocketBridgeConfig) -> Self {
        Self { config }
    }
}
