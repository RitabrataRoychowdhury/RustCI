//! Transport endpoint definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transport endpoint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// Endpoint address
    pub address: String,
    /// Port number
    pub port: u16,
    /// Transport type
    pub transport: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Endpoint {
    /// Create a new TCP endpoint
    pub fn tcp(address: &str, port: u16) -> Self {
        Self {
            address: address.to_string(),
            port,
            transport: "tcp".to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new WebSocket endpoint
    pub fn websocket(address: &str, port: u16) -> Self {
        Self {
            address: address.to_string(),
            port,
            transport: "websocket".to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new Unix socket endpoint
    pub fn unix_socket(path: &str) -> Self {
        Self {
            address: path.to_string(),
            port: 0,
            transport: "unix".to_string(),
            metadata: HashMap::new(),
        }
    }
}

/// Transport type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportType {
    /// TCP transport
    Tcp,
    /// WebSocket transport
    WebSocket,
    /// Unix socket transport
    UnixSocket,
    /// QUIC transport
    Quic,
    /// Custom transport
    Custom(String),
}

impl From<&str> for TransportType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "tcp" => TransportType::Tcp,
            "websocket" | "ws" => TransportType::WebSocket,
            "unix" | "unix_socket" => TransportType::UnixSocket,
            "quic" => TransportType::Quic,
            custom => TransportType::Custom(custom.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_creation() {
        let endpoint = Endpoint::tcp("localhost", 8080);
        assert_eq!(endpoint.address, "localhost");
        assert_eq!(endpoint.port, 8080);
        assert_eq!(endpoint.transport, "tcp");
    }

    #[test]
    fn test_transport_type_conversion() {
        assert_eq!(TransportType::from("tcp"), TransportType::Tcp);
        assert_eq!(TransportType::from("websocket"), TransportType::WebSocket);
        assert_eq!(TransportType::from("unix"), TransportType::UnixSocket);
    }
}
