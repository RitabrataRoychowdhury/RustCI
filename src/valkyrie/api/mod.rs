//! Valkyrie Protocol External API
//!
//! This module provides the main external API for the Valkyrie Protocol,
//! designed to be developer-friendly and easy to integrate with external systems.

pub mod adapters;
pub mod client;
pub mod config;
pub mod message;
pub mod stats;

// Re-export main API components
pub use adapters::ValkyrieEngineAdapter;
pub use client::{ClientBuilder, ValkyrieClient};
pub use config::{ClientConfig, ClientConfigBuilder};
pub use message::{
    BroadcastResult, ClientMessage, ClientMessagePriority, ClientMessageType, ClientPayload,
};
pub use stats::ClientStats;
