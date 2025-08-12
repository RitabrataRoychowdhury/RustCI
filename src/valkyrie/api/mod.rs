//! Valkyrie Protocol External API
//!
//! This module provides the main external API for the Valkyrie Protocol,
//! designed to be developer-friendly and easy to integrate with external systems.

pub mod client;
pub mod message;
pub mod config;
pub mod adapters;
pub mod stats;

// Re-export main API components
pub use client::{ValkyrieClient, ClientBuilder};
pub use message::{ClientMessage, ClientMessageType, ClientPayload, ClientMessagePriority, BroadcastResult};
pub use config::{ClientConfig, ClientConfigBuilder};
pub use adapters::ValkyrieEngineAdapter;
pub use stats::ClientStats;

