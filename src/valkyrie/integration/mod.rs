//! Valkyrie Protocol Integration Layer
//!
//! This module provides integration adapters and bridges for connecting
//! the Valkyrie Protocol with external systems, particularly RustCI.

pub mod rustci;
pub mod adapters;
pub mod bridge;

// Re-export integration components
pub use rustci::{RustCIIntegration, RustCIAdapter, RustCIConfig};
pub use adapters::{IntegrationAdapter, AdapterConfig};
pub use bridge::{ProtocolBridge, BridgeConfig};

