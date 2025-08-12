//! Core Valkyrie Protocol components
//!
//! This module contains the fundamental components of the Valkyrie Protocol,
//! including the main engine, message types, and core abstractions.

pub mod engine;
pub mod message;
pub mod connection;
pub mod events;
pub mod handlers;

// Re-export core types
pub use engine::{ValkyrieEngine, ProtocolState, EngineStats, BroadcastResult};
pub use message::{
    ValkyrieMessage, MessageHeader, MessageType, MessagePriority,
    MessagePayload, ProtocolInfo, ProtocolVersion
};
pub use connection::{
    ConnectionHandle, ConnectionId, ConnectionInfo, ConnectionState,
    ConnectionRegistry, Listener, ConnectionStats
};
pub use events::{ValkyrieEvent, EventBus, EventHandler};
pub use handlers::{MessageHandler, LoggingMessageHandler, EchoMessageHandler};

use uuid::Uuid;



/// Stream identifier type  
pub type StreamId = u32;

/// Endpoint identifier type
pub type EndpointId = String;

/// Correlation identifier type
pub type CorrelationId = Uuid;