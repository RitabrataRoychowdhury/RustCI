//! # Valkyrie SDK
//! 
//! High-level SDK for Valkyrie Protocol with simplified APIs and configuration management.
//! 
//! ## Features
//! 
//! - **Simple APIs**: Easy-to-use client and server builders
//! - **Configuration Management**: YAML/TOML configuration support
//! - **Connection Pooling**: Automatic connection management
//! - **Retry Logic**: Built-in retry and circuit breaker patterns
//! - **Metrics**: Optional metrics collection and reporting
//! 
//! ## Quick Start
//! 
//! ### Client
//! 
//! ```rust
//! use valkyrie_sdk::ClientBuilder;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = ClientBuilder::new()
//!         .endpoint("tcp://localhost:8080")
//!         .timeout_ms(5000)
//!         .build()
//!         .await?;
//!     
//!     let response = client.request("Hello, World!").await?;
//!     println!("Response: {}", response);
//!     
//!     Ok(())
//! }
//! ```
//! 
//! ### Server
//! 
//! ```rust
//! use valkyrie_sdk::{ServerBuilder, MessageHandler};
//! 
//! struct EchoHandler;
//! 
//! #[async_trait::async_trait]
//! impl MessageHandler for EchoHandler {
//!     async fn handle(&self, message: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
//!         Ok(format!("Echo: {}", message))
//!     }
//! }
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let server = ServerBuilder::new()
//!         .bind("0.0.0.0:8080")
//!         .handler("/echo", EchoHandler)
//!         .build()
//!         .await?;
//!     
//!     server.start().await?;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod server;
pub mod config;
pub mod pool;
pub mod retry;
pub mod metrics;

// Re-export core types from valkyrie-protocol
pub use valkyrie_protocol::{
    Message, MessagePriority, MessageType, ValkyrieError, Result,
    PROTOCOL_VERSION, DEFAULT_PORT
};

// Re-export SDK types
pub use client::{Client, ClientBuilder};
pub use server::{Server, ServerBuilder, MessageHandler};
pub use config::Config;
pub use client::ClientConfig;
pub use server::ServerConfig;

/// SDK version
pub const SDK_VERSION: &str = "0.1.0";