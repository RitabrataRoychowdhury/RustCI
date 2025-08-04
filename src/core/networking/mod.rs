//! Networking, communication, and security
//!
//! This module contains components for network transport, secure communication,
//! node-to-node messaging, and security management.

pub mod node_communication;
pub mod node_communication_manager;
pub mod secure_transport;
pub mod security;
pub mod transport;

// Re-export commonly used types
pub use transport::{Transport, Connection, TransportConfig};
pub use security::{JwtManager, Permission, SecurityContext, AuditAction, AuditEvent};
pub use node_communication::{ProtocolMessage};