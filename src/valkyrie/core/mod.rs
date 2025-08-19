//! Core Valkyrie Protocol components
//!
//! This module contains the fundamental components of the Valkyrie Protocol,
//! including the main engine, message types, and core abstractions.

pub mod connection;
pub mod engine;
pub mod events;
pub mod handlers;
pub mod message;

// Re-export core types
pub use connection::{
    ConnectionHandle, ConnectionId, ConnectionInfo, ConnectionRegistry, ConnectionState,
    ConnectionStats, Listener,
};
pub use engine::{BroadcastResult, EngineStats, ProtocolState, ValkyrieEngine};
pub use events::{EventBus, EventHandler, ValkyrieEvent};
pub use handlers::{EchoMessageHandler, LoggingMessageHandler, MessageHandler};
pub use message::{
    MessageHeader, MessagePayload, MessagePriority, MessageType, ProtocolInfo, ProtocolVersion,
    ValkyrieMessage,
};

use uuid::Uuid;

/// Stream identifier type  
pub type StreamId = u32;

/// Endpoint identifier type
pub type EndpointId = String;

/// Correlation identifier type
pub type CorrelationId = Uuid;
