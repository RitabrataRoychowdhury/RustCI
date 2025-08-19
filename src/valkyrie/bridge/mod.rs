//! Valkyrie Protocol Bridge Layer
//!
//! This module provides bridge components for integrating the Valkyrie Protocol
//! with other protocols and systems.

pub mod grpc;
pub mod http;
pub mod websocket;

// Re-export bridge components
pub use grpc::{GrpcBridge, GrpcBridgeConfig};
pub use http::{HttpBridge, HttpBridgeConfig};
pub use websocket::{WebSocketBridge, WebSocketBridgeConfig};
