//! Unix Socket Transport Implementation for Valkyrie Protocol

use crate::valkyrie::transport::{
    Connection, Endpoint, Listener, Transport, TransportCapabilities,
};
use crate::valkyrie::Result;
use async_trait::async_trait;

/// Unix socket transport implementation
pub struct UnixSocketTransport;

impl UnixSocketTransport {
    /// Create a new Unix socket transport
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Transport for UnixSocketTransport {
    async fn connect(&self, _endpoint: &Endpoint) -> Result<Box<dyn Connection>> {
        // Placeholder implementation - Unix socket connect not yet implemented
        Err(crate::valkyrie::ValkyrieError::NotImplemented(
            "Unix socket connect not yet implemented".to_string()
        ))
    }

    async fn listen(&self, _bind_address: std::net::SocketAddr) -> Result<Box<dyn Listener>> {
        // Placeholder implementation - Unix socket listen not yet implemented
        Err(crate::valkyrie::ValkyrieError::NotImplemented(
            "Unix socket listen not yet implemented".to_string()
        ))
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            encryption: false,
            compression: false,
            multiplexing: false,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}
