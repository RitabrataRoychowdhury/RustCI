//! # Valkyrie Protocol
//! 
//! High-performance message protocol for distributed systems with sub-100μs latency.
//! 
//! ## Features
//! 
//! - **Ultra-low latency**: <100μs message processing
//! - **High throughput**: 1M+ messages/second
//! - **Zero-copy operations**: Minimal memory allocations
//! - **Pluggable transports**: TCP, QUIC, Unix sockets, WebSockets
//! - **Built-in security**: ChaCha20-Poly1305, AES-256-GCM encryption
//! - **QoS support**: Priority-based message routing
//! 
//! ## Quick Start
//! 
//! ```rust
//! use valkyrie_protocol::{ValkyrieClient, ValkyrieServer, Message};
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client
//!     let client = ValkyrieClient::connect("tcp://localhost:8080").await?;
//!     
//!     // Send a message
//!     let response = client.send(Message::new("Hello, Valkyrie!")).await?;
//!     
//!     println!("Response: {}", response);
//!     Ok(())
//! }
//! ```

pub mod core;
pub mod transport;
pub mod security;
pub mod message;
pub mod client;
pub mod server;
pub mod error;

// Re-export main types for convenience
pub use client::ValkyrieClient;
pub use server::ValkyrieServer;
pub use message::{Message, MessagePriority, MessageType};
pub use error::{ValkyrieError, Result};
pub use core::engine::ValkyrieEngine;

/// Protocol version
pub const PROTOCOL_VERSION: &str = "2.0.0";

/// Default port for Valkyrie protocol
pub const DEFAULT_PORT: u16 = 8080;