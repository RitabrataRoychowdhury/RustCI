//! Valkyrie Protocol Message Types
//!
//! This module defines the core message types and structures used
//! throughout the Valkyrie Protocol.

use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::valkyrie::core::{StreamId, EndpointId, CorrelationId};

/// Enhanced Valkyrie message with advanced framing and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieMessage {
    /// Message header with protocol metadata
    pub header: MessageHeader,
    /// Message payload
    pub payload: MessagePayload,
    /// Message signature for integrity verification
    pub signature: Option<MessageSignature>,
    /// Tracing context for distributed tracing
    pub trace_context: Option<TraceContext>,
}

impl ValkyrieMessage {
    /// Create a new message with the given type and payload
    pub fn new(message_type: MessageType, payload: MessagePayload) -> Self {
        Self {
            header: MessageHeader::new(message_type),
            payload,
            signature: None,
            trace_context: None,
        }
    }
    
    /// Create a text message
    pub fn text(content: &str) -> Self {
        Self::new(MessageType::Data, MessagePayload::Text(content.to_string()))
    }
    
    /// Create a binary message
    pub fn binary(data: Vec<u8>) -> Self {
        Self::new(MessageType::Data, MessagePayload::Binary(data))
    }
    
    /// Create a JSON message
    pub fn json<T: serde::Serialize>(data: &T) -> crate::valkyrie::Result<Self> {
        let json_value = serde_json::to_value(data)
            .map_err(|e| crate::valkyrie::ValkyrieError::InternalServerError(format!("JSON serialization failed: {}", e)))?;
        Ok(Self::new(MessageType::Data, MessagePayload::Json(json_value)))
    }
    
    /// Set message priority
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.header.priority = priority;
        self
    }
    
    /// Set message TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.header.ttl = Some(ttl);
        self
    }
    
    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: CorrelationId) -> Self {
        self.header.correlation_id = Some(correlation_id);
        self
    }
}

/// Advanced message header with extended capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Message ID
    pub id: Uuid,
    /// Protocol magic number and version
    pub protocol_info: ProtocolInfo,
    /// Message type and subtype
    pub message_type: MessageType,
    /// Stream identifier for multiplexing
    pub stream_id: StreamId,
    /// Message flags and control bits
    pub flags: MessageFlags,
    /// Message priority for QoS
    pub priority: MessagePriority,
    /// Message timestamp
    pub timestamp: DateTime<Utc>,
    /// Message TTL for expiration
    pub ttl: Option<Duration>,
    /// Correlation ID for request-response matching
    pub correlation_id: Option<CorrelationId>,
    /// Routing information
    pub routing: RoutingInfo,
    /// Compression information
    pub compression: CompressionInfo,
    /// Message sequence number
    pub sequence_number: u64,
    /// Acknowledgment number
    pub ack_number: Option<u64>,
}

impl MessageHeader {
    /// Create a new message header
    pub fn new(message_type: MessageType) -> Self {
        Self {
            id: Uuid::new_v4(),
            protocol_info: ProtocolInfo::default(),
            message_type,
            stream_id: 0,
            flags: MessageFlags::default(),
            priority: MessagePriority::Normal,
            timestamp: Utc::now(),
            ttl: None,
            correlation_id: None,
            routing: RoutingInfo::default(),
            compression: CompressionInfo::default(),
            sequence_number: 0,
            ack_number: None,
        }
    }
}

/// Protocol version and magic number information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolInfo {
    /// Protocol magic number (0x56414C4B = "VALK")
    pub magic: u32,
    /// Protocol version
    pub version: ProtocolVersion,
    /// Protocol extensions enabled
    pub extensions: Vec<ProtocolExtension>,
}

impl Default for ProtocolInfo {
    fn default() -> Self {
        Self {
            magic: crate::valkyrie::PROTOCOL_MAGIC,
            version: ProtocolVersion::current(),
            extensions: Vec::new(),
        }
    }
}

/// Protocol version information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version
    pub major: u16,
    /// Minor version
    pub minor: u16,
    /// Patch version
    pub patch: u16,
}

impl ProtocolVersion {
    /// Get the current protocol version
    pub fn current() -> Self {
        Self {
            major: 1,
            minor: 0,
            patch: 0,
        }
    }
}

/// Protocol extensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolExtension {
    /// Compression support
    Compression,
    /// Encryption support
    Encryption,
    /// Stream multiplexing
    Multiplexing,
    /// Priority queuing
    PriorityQueuing,
    /// Flow control
    FlowControl,
    /// Custom extension
    Custom(String),
}

/// Extended message type system for Valkyrie Protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    /// Heartbeat message
    Heartbeat = 0x0001,
    /// Data message
    Data = 0x0002,
    /// Control message
    Control = 0x0003,
    /// Error message
    Error = 0x0004,
    /// Stream control message
    StreamControl = 0x0005,
    /// Authentication message
    Authentication = 0x0006,
    /// Custom message type
    Custom(u16),
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MessagePriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority
    Critical = 3,
}

/// Message payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Empty payload
    Empty,
    /// Text payload
    Text(String),
    /// Binary payload
    Binary(Vec<u8>),
    /// JSON payload
    Json(serde_json::Value),
    /// Structured payload with metadata
    Structured(StructuredPayload),
    /// Stream payload for large data
    Stream(StreamPayload),
    /// File transfer payload
    FileTransfer(FileTransferPayload),
}

/// Structured payload with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredPayload {
    /// Content type
    pub content_type: String,
    /// Content encoding
    pub encoding: Option<String>,
    /// Payload metadata
    pub metadata: HashMap<String, String>,
    /// Actual payload data
    pub data: Vec<u8>,
}

/// Stream payload for large data transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamPayload {
    /// Stream identifier
    pub stream_id: StreamId,
    /// Chunk sequence number
    pub chunk_sequence: u64,
    /// Total chunks expected
    pub total_chunks: Option<u64>,
    /// Chunk data
    pub data: Vec<u8>,
    /// Stream operation type
    pub operation: StreamOperation,
}

/// Stream operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamOperation {
    /// Start stream
    Start,
    /// Continue stream
    Continue,
    /// End stream
    End,
    /// Abort stream
    Abort,
}

/// File transfer payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferPayload {
    /// File name
    pub filename: String,
    /// File size
    pub file_size: u64,
    /// File chunk offset
    pub offset: u64,
    /// Chunk data
    pub data: Vec<u8>,
    /// File transfer operation
    pub operation: FileTransferOperation,
    /// File metadata
    pub metadata: HashMap<String, String>,
}

/// File transfer operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTransferOperation {
    /// Start file transfer
    Start,
    /// Continue file transfer
    Continue,
    /// Complete file transfer
    Complete,
    /// Cancel file transfer
    Cancel,
}

/// Message flags for control and metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageFlags {
    /// Requires acknowledgment
    pub requires_ack: bool,
    /// Is acknowledgment message
    pub is_ack: bool,
    /// Is compressed
    pub compressed: bool,
    /// Is encrypted
    pub encrypted: bool,
    /// Is fragmented
    pub fragmented: bool,
    /// Is last fragment
    pub last_fragment: bool,
}

/// Routing information for message delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingInfo {
    /// Source endpoint
    pub source: EndpointId,
    /// Destination type and target
    pub destination: DestinationType,
    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
    /// Routing path
    pub path: Vec<EndpointId>,
    /// Routing hints
    pub hints: RoutingHints,
}

impl Default for RoutingInfo {
    fn default() -> Self {
        Self {
            source: "unknown".to_string(),
            destination: DestinationType::Broadcast,
            load_balancing: LoadBalancingStrategy::RoundRobin,
            path: Vec::new(),
            hints: RoutingHints::default(),
        }
    }
}

/// Destination types for message routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DestinationType {
    /// Unicast to specific endpoint
    Unicast(EndpointId),
    /// Multicast to group
    Multicast(Vec<EndpointId>),
    /// Broadcast to all
    Broadcast,
    /// Service-based routing
    Service(ServiceSelector),
    /// Geographic routing
    Geographic(GeographicSelector),
    /// Custom routing
    Custom(CustomSelector),
}

/// Service selector for service-based routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSelector {
    /// Service name
    pub service_name: String,
    /// Service version
    pub version: Option<String>,
    /// Service tags
    pub tags: Vec<String>,
}

/// Geographic selector for location-based routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicSelector {
    /// Region
    pub region: String,
    /// Zone
    pub zone: Option<String>,
    /// Data center
    pub datacenter: Option<String>,
}

/// Custom selector for application-specific routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSelector {
    /// Selector type
    pub selector_type: String,
    /// Selector criteria
    pub criteria: HashMap<String, String>,
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Round-robin
    RoundRobin,
    /// Least connections
    LeastConnections,
    /// Weighted round-robin
    WeightedRoundRobin,
    /// Consistent hashing
    ConsistentHashing,
    /// Random selection
    Random,
}

/// Routing hints for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingHints {
    /// Latency sensitive
    pub latency_sensitive: bool,
    /// Bandwidth requirements
    pub bandwidth_requirements: Option<u64>,
    /// Reliability level
    pub reliability_level: ReliabilityLevel,
    /// Cacheable
    pub cacheable: bool,
    /// Compression preference
    pub compression_preference: CompressionPreference,
}

impl Default for RoutingHints {
    fn default() -> Self {
        Self {
            latency_sensitive: false,
            bandwidth_requirements: None,
            reliability_level: ReliabilityLevel::BestEffort,
            cacheable: false,
            compression_preference: CompressionPreference::None,
        }
    }
}

/// Reliability levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReliabilityLevel {
    /// Best effort delivery
    BestEffort,
    /// At least once delivery
    AtLeastOnce,
    /// Exactly once delivery
    ExactlyOnce,
}

/// Compression preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionPreference {
    /// No compression
    None,
    /// Fast compression
    Fast,
    /// Balanced compression
    Balanced,
    /// Maximum compression
    Maximum,
}

/// Compression information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionInfo {
    /// Compression algorithm
    pub algorithm: CompressionAlgorithm,
    /// Compression ratio achieved
    pub ratio: f32,
    /// Compression level used
    pub level: u8,
    /// Dictionary ID (if applicable)
    pub dictionary_id: Option<u32>,
}

impl Default for CompressionInfo {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::None,
            ratio: 1.0,
            level: 0,
            dictionary_id: None,
        }
    }
}

/// Compression algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// Gzip compression
    Gzip,
    /// LZ4 compression
    Lz4,
    /// Zstd compression
    Zstd,
    /// Brotli compression
    Brotli,
}

/// Message signature for integrity verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSignature {
    /// Signature algorithm
    pub algorithm: SignatureAlgorithm,
    /// Signature data
    pub signature: Vec<u8>,
    /// Signer identity
    pub signer: Option<String>,
}

/// Signature algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    /// HMAC-SHA256
    HmacSha256,
    /// RSA-SHA256
    RsaSha256,
    /// ECDSA-SHA256
    EcdsaSha256,
    /// Ed25519
    Ed25519,
}

/// Tracing context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID
    pub parent_span_id: Option<String>,
    /// Trace flags
    pub flags: u8,
    /// Baggage
    pub baggage: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let message = ValkyrieMessage::text("Hello, World!");
        assert!(matches!(message.payload, MessagePayload::Text(_)));
        assert_eq!(message.header.message_type, MessageType::Data);
    }
    
    #[test]
    fn test_message_with_priority() {
        let message = ValkyrieMessage::text("Important message")
            .with_priority(MessagePriority::High);
        assert_eq!(message.header.priority, MessagePriority::High);
    }
    
    #[test]
    fn test_json_message() {
        let data = serde_json::json!({"key": "value"});
        let result = ValkyrieMessage::json(&data);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_protocol_version() {
        let version = ProtocolVersion::current();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }
}