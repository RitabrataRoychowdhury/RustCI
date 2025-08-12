//! Valkyrie Protocol - Self-contained distributed communication system
//!
//! The Valkyrie Protocol is RustCI's flagship distributed communication system
//! that enables secure, high-performance, and fault-tolerant communication
//! between control planes and distributed nodes.
//!
//! This module is designed to be self-contained while maintaining seamless
//! integration with RustCI components.

// Core protocol components
pub mod core;
pub mod transport;
pub mod security;
pub mod streaming;
pub mod observability;
pub mod bridge;

// Performance optimization modules
pub mod performance;
pub mod lockfree;

// Integration and API modules
pub mod api;
pub mod integration;

// Configuration and management
pub mod config;
pub mod factory;

// Re-export main components for easy access
pub use core::{
    ValkyrieEngine, ValkyrieMessage, MessageHeader, MessageType, 
    MessagePriority, ConnectionHandle, Listener, ProtocolState,
    ConnectionId, StreamId, EndpointId, CorrelationId, BroadcastResult,
    EngineStats, ValkyrieEvent
};

// Re-export message components
pub use core::message::{
    MessagePayload, RoutingInfo, DestinationType, LoadBalancingStrategy,
    ProtocolInfo, MessageFlags, RoutingHints, CompressionInfo,
    CompressionAlgorithm, ReliabilityLevel, CompressionPreference
};

// Re-export handler components
pub use core::handlers::{MessageHandler, LoggingMessageHandler, EchoMessageHandler};

pub use config::{
    ValkyrieConfig, ValkyrieConfigBuilder, TransportConfig, 
    SecurityConfig, StreamingConfig, ObservabilityConfig,
    ProtocolConfig, ProtocolTimeouts, TransportLayerConfig,
    ConnectionPoolConfig, FlowControlConfig, RoutingConfig,
    AuthenticationConfig, EncryptionConfig, AuthorizationConfig,
    AuditConfig, MetricsConfig, TracingConfig, LoggingConfig,
    AuthMethod, CipherSuite, AuditEvent, LogFormat,
    LoadBalancingStrategy as ConfigLoadBalancingStrategy,
    PerformanceConfig, FeatureFlags
};
pub use factory::{ValkyrieFactory, EngineBuilder};
pub use api::{ValkyrieClient, ClientConfig, ClientMessage};
pub use transport::{
    TransportManager, TransportType, Endpoint, TransportStats,
    TransportSelectionStrategy, ConnectionReuseStrategy
};
pub use security::{SecurityManager};
pub use streaming::{StreamMultiplexer, StreamConfig};
pub use observability::{ObservabilityManager, MetricsCollector};

// Error types
pub use crate::error::{Result, AppError as ValkyrieError};

/// Valkyrie Protocol version
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Protocol magic number (0x56414C4B = "VALK")
pub const PROTOCOL_MAGIC: u32 = 0x56414C4B;