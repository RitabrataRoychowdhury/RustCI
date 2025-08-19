//! Valkyrie Protocol Transport Layer
//!
//! This module provides transport layer abstractions and implementations
//! for the Valkyrie Protocol, supporting multiple transport types.

pub mod endpoint;
pub mod manager;
pub mod tcp;
pub mod unix_socket;
pub mod websocket;

// Re-export main transport components
pub use endpoint::{Endpoint, TransportType};
pub use manager::{TransportManager, TransportStats};

// Additional transport configuration types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TransportSelectionStrategy {
    Primary,
    RoundRobin,
    LatencyBased,
    BandwidthBased,
    Custom(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConnectionReuseStrategy {
    None,
    PerEndpoint,
    Global,
}

use crate::valkyrie::Result;

/// Transport layer trait for different transport implementations
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Connect to a remote endpoint
    async fn connect(&self, endpoint: &Endpoint) -> Result<Box<dyn Connection>>;

    /// Listen for incoming connections
    async fn listen(&self, bind_address: std::net::SocketAddr) -> Result<Box<dyn Listener>>;

    /// Get transport capabilities
    fn capabilities(&self) -> TransportCapabilities;
}

/// Connection trait for transport connections
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    /// Send data over the connection
    async fn send(&mut self, data: &[u8]) -> Result<()>;

    /// Receive data from the connection
    async fn receive(&mut self) -> Result<Vec<u8>>;

    /// Close the connection
    async fn close(&mut self) -> Result<()>;

    /// Get connection metadata
    fn metadata(&self) -> ConnectionMetadata;
}

/// Listener trait for accepting incoming connections
#[async_trait::async_trait]
pub trait Listener: Send + Sync {
    /// Accept an incoming connection
    async fn accept(&mut self) -> Result<Box<dyn Connection>>;

    /// Close the listener
    async fn close(&mut self) -> Result<()>;
}

/// Transport capabilities
#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    /// Supports encryption
    pub encryption: bool,
    /// Supports compression
    pub compression: bool,
    /// Supports multiplexing
    pub multiplexing: bool,
    /// Maximum message size
    pub max_message_size: usize,
}

/// Connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionMetadata {
    /// Remote address
    pub remote_address: String,
    /// Local address
    pub local_address: String,
    /// Transport type
    pub transport_type: TransportType,
    /// Connection established time
    pub established_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_capabilities() {
        let caps = TransportCapabilities {
            encryption: true,
            compression: false,
            multiplexing: true,
            max_message_size: 1024 * 1024,
        };

        assert!(caps.encryption);
        assert!(!caps.compression);
        assert!(caps.multiplexing);
        assert_eq!(caps.max_message_size, 1024 * 1024);
    }
}
