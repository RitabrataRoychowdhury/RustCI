//! Valkyrie Protocol Bridge Layer
//!
//! This module provides bridge components for integrating the Valkyrie Protocol
//! with other protocols and systems.

pub mod http;
pub mod websocket;
pub mod grpc;

// Re-export bridge components
pub use http::{HttpBridge, HttpBridgeConfig};
pub use websocket::{WebSocketBridge, WebSocketBridgeConfig};
pub use grpc::{GrpcBridge, GrpcBridgeConfig};