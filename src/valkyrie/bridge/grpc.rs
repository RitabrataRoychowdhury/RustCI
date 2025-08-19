//! gRPC bridge for Valkyrie Protocol

/// gRPC bridge configuration
pub struct GrpcBridgeConfig {
    /// Bridge port
    pub port: u16,
    /// Enable TLS
    pub enable_tls: bool,
}

/// gRPC bridge for protocol translation
pub struct GrpcBridge {
    config: GrpcBridgeConfig,
}

impl GrpcBridge {
    /// Create a new gRPC bridge
    pub fn new(config: GrpcBridgeConfig) -> Self {
        Self { config }
    }
}
