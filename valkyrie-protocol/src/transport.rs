//! Transport layer for Valkyrie Protocol

pub mod tcp;
pub mod quic;
pub mod unix_socket;
pub mod websocket;

use crate::{Message, Result};
use async_trait::async_trait;

/// Transport capabilities
#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Supports encryption
    pub supports_encryption: bool,
    /// Supports compression
    pub supports_compression: bool,
    /// Supports multiplexing
    pub supports_multiplexing: bool,
    /// Connection-oriented transport
    pub connection_oriented: bool,
}

/// Generic transport trait
#[async_trait]
pub trait Transport: Send + Sync {
    /// Get transport name
    fn name(&self) -> &str;
    
    /// Get transport capabilities
    fn capabilities(&self) -> &TransportCapabilities;
    
    /// Send a message
    async fn send(&self, message: Message) -> Result<()>;
    
    /// Receive a message (blocking)
    async fn recv(&self) -> Result<Message>;
    
    /// Try to receive a message (non-blocking)
    async fn try_recv(&self) -> Result<Option<Message>>;
    
    /// Close the transport
    async fn close(&self) -> Result<()>;
    
    /// Check if transport is connected
    fn is_connected(&self) -> bool;
}

/// Transport factory for creating transport instances
pub struct TransportFactory;

impl TransportFactory {
    /// Create a transport from a URL
    pub async fn create_transport(url: &str) -> Result<Box<dyn Transport>> {
        let url = url::Url::parse(url)
            .map_err(|e| crate::ValkyrieError::configuration(format!("Invalid URL: {}", e)))?;
        
        match url.scheme() {
            "tcp" => {
                let transport = tcp::TcpTransport::connect(&url).await?;
                Ok(Box::new(transport))
            }
            "quic" => {
                let transport = quic::QuicTransport::connect(&url).await?;
                Ok(Box::new(transport))
            }
            "unix" => {
                let transport = unix_socket::UnixSocketTransport::connect(&url).await?;
                Ok(Box::new(transport))
            }
            "ws" | "wss" => {
                let transport = websocket::WebSocketTransport::connect(&url).await?;
                Ok(Box::new(transport))
            }
            scheme => Err(crate::ValkyrieError::configuration(format!("Unsupported transport scheme: {}", scheme)))
        }
    }
}