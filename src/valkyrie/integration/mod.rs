//! Valkyrie Protocol Integration Layer
//!
//! This module provides integration adapters and bridges for connecting
//! the Valkyrie Protocol with external systems, particularly RustCI.

pub mod adapters;
pub mod bridge;
pub mod rustci;

// Re-export integration components
pub use adapters::{AdapterConfig, IntegrationAdapter};
pub use bridge::{BridgeConfig, ProtocolBridge};
pub use rustci::{RustCIAdapter, RustCIConfig, RustCIIntegration};
