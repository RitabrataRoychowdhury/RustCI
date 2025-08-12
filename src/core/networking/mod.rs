//! Networking, communication, and security
//!
//! This module contains components for network transport, secure communication,
//! node-to-node messaging, security management, and the Valkyrie Protocol.

pub mod node_communication;
pub mod node_communication_manager;
pub mod secure_transport;
pub mod security;
pub mod transport;
pub mod valkyrie;

// Re-export commonly used types
pub use transport::{Transport, Connection, TransportConfig, Listener};
pub use security::{JwtManager, Permission, SecurityContext, AuditAction, AuditEvent};
pub use node_communication::{
    ProtocolMessage, EnhancedProtocolMessage, PriorityMessageQueue,
    MessageCompressor, MessageIntegrityVerifier, NodeMessage, ControlPlaneMessage,
    NodeId, JobId, LogLevel, StreamType, RealtimeMetrics, ResourceType,
    ResourcePriority, AlertSeverity, QoSPolicy, ClusterState, FlowControlAction
};

// Re-export Valkyrie Protocol components (now from the main valkyrie module)
pub use crate::valkyrie::{
    ValkyrieEngine, ValkyrieConfig, ConnectionHandle, 
    ValkyrieMessage, MessageHeader, MessageType, MessagePriority,
    MessagePayload, RoutingInfo, DestinationType,
    EndpointId, StreamId, CorrelationId,
    LoadBalancingStrategy, BroadcastResult, ValkyrieEvent,
    EngineStats, ProtocolState, Endpoint
};