//! Valkyrie Protocol - Central Type Definitions
//!
//! This module contains the canonical type definitions for the Valkyrie Protocol.
//! All other modules should import types from here to ensure consistency and
//! eliminate type conflicts throughout the codebase.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Type Aliases
// ============================================================================

/// Connection identifier for the Valkyrie Protocol
pub type ConnectionId = Uuid;

/// Stream identifier for multiplexed connections
pub type StreamId = u32;

/// Endpoint identifier for routing
pub type EndpointId = String;

/// Correlation identifier for request-response matching
pub type CorrelationId = Uuid;

/// Standardized Duration type (using std::time::Duration)
pub type Duration = std::time::Duration;

// ============================================================================
// Core Protocol Types
// ============================================================================

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
    // Core protocol messages
    Hello = 0x00,
    HelloAck = 0x01,
    Ping = 0x02,
    Pong = 0x03,
    Error = 0x04,
    Close = 0x05,
    
    // Job execution messages
    JobExecution = 0x20,
    
    // Basic message types for compatibility
    Heartbeat = 0x06,
    Data = 0x07,
    Control = 0x08,
    
    // Authentication and security
    AuthChallenge = 0x10,
    AuthResponse = 0x11,
    AuthSuccess = 0x12,
    AuthFailure = 0x13,
    KeyExchange = 0x14,
    CertificateRequest = 0x15,
    
    // Job management (enhanced)
    JobRequest = 0x20,
    JobAccept = 0x21,
    JobReject = 0x22,
    JobStart = 0x23,
    JobProgress = 0x24,
    JobComplete = 0x25,
    JobFailed = 0x26,
    JobCancel = 0x27,
    JobMigrate = 0x28,
    JobPause = 0x29,
    JobResume = 0x2A,
    
    // Streaming and data transfer
    StreamOpen = 0x30,
    StreamData = 0x31,
    StreamClose = 0x32,
    StreamAck = 0x33,
    StreamReset = 0x34,
    FileTransferStart = 0x35,
    FileTransferChunk = 0x36,
    FileTransferComplete = 0x37,
    
    // Observability and monitoring
    MetricsReport = 0x40,
    LogEntry = 0x41,
    TraceSpan = 0x42,
    HealthCheck = 0x43,
    StatusUpdate = 0x44,
    AlertNotification = 0x45,
    
    // Cluster management
    NodeJoin = 0x50,
    NodeLeave = 0x51,
    NodeUpdate = 0x52,
    ClusterState = 0x53,
    LeaderElection = 0x54,
    ConsensusProposal = 0x55,
    ConsensusVote = 0x56,
    
    // Service discovery and routing
    ServiceRegister = 0x60,
    ServiceDeregister = 0x61,
    ServiceQuery = 0x62,
    ServiceResponse = 0x63,
    RouteUpdate = 0x64,
    LoadBalanceUpdate = 0x65,
    
    // Flow control and QoS
    FlowControlUpdate = 0x70,
    BackpressureSignal = 0x71,
    QoSUpdate = 0x72,
    PriorityUpdate = 0x73,
    
    // Custom and extensible
    Custom(u16) = 0xFF00,
}

/// Message flags for control and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFlags {
    /// Message requires acknowledgment
    pub requires_ack: bool,
    /// Message is compressed
    pub compressed: bool,
    /// Message is encrypted
    pub encrypted: bool,
    /// Message is fragmented
    pub fragmented: bool,
    /// Message is urgent
    pub urgent: bool,
    /// Message is a retransmission
    pub retransmission: bool,
    /// Message contains binary data
    pub binary_data: bool,
    /// Custom flags
    pub custom_flags: u16,
}

/// Enhanced message payload with advanced features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Binary payload
    Binary(Vec<u8>),
    /// JSON payload
    Json(serde_json::Value),
    /// Text payload
    Text(String),
    /// Structured payload
    Structured(StructuredPayload),
    /// Stream data
    Stream(StreamPayload),
    /// File transfer data
    FileTransfer(FileTransferPayload),
    /// Empty payload
    Empty,
}

impl MessagePayload {
    /// Get the approximate size of the payload in bytes
    pub fn len(&self) -> usize {
        match self {
            MessagePayload::Binary(data) => data.len(),
            MessagePayload::Json(value) => {
                serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)
            },
            MessagePayload::Text(text) => text.len(),
            MessagePayload::Structured(payload) => {
                serde_json::to_string(payload).map(|s| s.len()).unwrap_or(0)
            },
            MessagePayload::Stream(payload) => payload.data.as_ref().map(|d| d.len()).unwrap_or(0),
            MessagePayload::FileTransfer(payload) => payload.chunk_data.as_ref().map(|d| d.len()).unwrap_or(0),
            MessagePayload::Empty => 0,
        }
    }

    /// Check if the payload is empty
    pub fn is_empty(&self) -> bool {
        match self {
            MessagePayload::Binary(data) => data.is_empty(),
            MessagePayload::Json(_) => false,
            MessagePayload::Text(text) => text.is_empty(),
            MessagePayload::Structured(_) => false,
            MessagePayload::Stream(payload) => payload.data.as_ref().map(|d| d.is_empty()).unwrap_or(true),
            MessagePayload::FileTransfer(payload) => payload.chunk_data.as_ref().map(|d| d.is_empty()).unwrap_or(true),
            MessagePayload::Empty => true,
        }
    }
}

/// Structured payload for typed messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredPayload {
    /// Payload type
    pub payload_type: String,
    /// Payload version
    pub version: String,
    /// Payload data
    pub data: serde_json::Value,
    /// Schema reference
    pub schema: Option<String>,
}

/// Stream payload for streaming data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamPayload {
    /// Stream identifier
    pub stream_id: StreamId,
    /// Stream operation
    pub operation: StreamOperation,
    /// Stream data
    pub data: Option<Vec<u8>>,
    /// Stream metadata
    pub metadata: HashMap<String, String>,
}

/// Stream operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamOperation {
    /// Open stream
    Open,
    /// Send data
    Data,
    /// Close stream
    Close,
    /// Reset stream
    Reset,
    /// Acknowledge data
    Ack,
    /// Flow control update
    FlowControl,
}

/// File transfer payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferPayload {
    /// Transfer ID
    pub transfer_id: Uuid,
    /// File information
    pub file_info: FileInfo,
    /// Transfer operation
    pub operation: FileTransferOperation,
    /// Chunk data
    pub chunk_data: Option<Vec<u8>>,
    /// Chunk metadata
    pub chunk_metadata: Option<ChunkMetadata>,
}

/// File information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// File name
    pub name: String,
    /// File path
    pub path: String,
    /// File size
    pub size: u64,
    /// File checksum
    pub checksum: String,
    /// MIME type
    pub mime_type: Option<String>,
    /// File permissions
    pub permissions: Option<u32>,
    /// Last modified time
    pub modified_time: Option<DateTime<Utc>>,
}

/// File transfer operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTransferOperation {
    /// Start transfer
    Start,
    /// Send chunk
    Chunk,
    /// Complete transfer
    Complete,
    /// Abort transfer
    Abort,
    /// Request retransmission
    Retransmit,
}

/// Chunk metadata for file transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Chunk number
    pub chunk_number: u32,
    /// Total chunks
    pub total_chunks: u32,
    /// Chunk size
    pub chunk_size: u32,
    /// Chunk offset
    pub offset: u64,
    /// Chunk checksum
    pub checksum: String,
}

/// Intelligent routing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingInfo {
    /// Source endpoint
    pub source: EndpointId,
    /// Destination endpoint(s)
    pub destination: DestinationType,
    /// Routing hints for optimization
    pub hints: RoutingHints,
    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
    /// Routing path (for debugging/tracing)
    pub path: Vec<EndpointId>,
}

/// Destination types for flexible routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DestinationType {
    /// Single destination
    Unicast(EndpointId),
    /// Multiple specific destinations
    Multicast(Vec<EndpointId>),
    /// Broadcast to all connected endpoints
    Broadcast,
    /// Route based on service discovery
    Service(ServiceSelector),
    /// Route based on geographic location
    Geographic(GeographicSelector),
    /// Route based on custom criteria
    Custom(CustomSelector),
}

/// Service selector for service-based routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSelector {
    /// Service name
    pub service: String,
    /// Service version constraint
    pub version: Option<String>,
    /// Service tags/labels
    pub tags: HashMap<String, String>,
    /// Preferred instance
    pub preferred_instance: Option<String>,
}

/// Geographic selector for location-based routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicSelector {
    /// Region preference
    pub region: Option<String>,
    /// Zone preference
    pub zone: Option<String>,
    /// Datacenter preference
    pub datacenter: Option<String>,
    /// Maximum latency tolerance
    pub max_latency_ms: Option<u32>,
}

/// Custom selector for application-specific routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSelector {
    /// Selector type
    pub selector_type: String,
    /// Selector criteria
    pub criteria: HashMap<String, serde_json::Value>,
}

/// Routing hints for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingHints {
    /// Preferred transport protocol
    pub preferred_transport: Option<String>,
    /// Latency sensitivity
    pub latency_sensitive: bool,
    /// Bandwidth requirements
    pub bandwidth_requirements: Option<u64>,
    /// Reliability requirements
    pub reliability_level: ReliabilityLevel,
    /// Caching hints
    pub cacheable: bool,
    /// Compression preference
    pub compression_preference: CompressionPreference,
}

/// Reliability level requirements
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReliabilityLevel {
    /// Best effort delivery
    BestEffort,
    /// At least once delivery
    AtLeastOnce,
    /// At most once delivery
    AtMostOnce,
    /// Exactly once delivery
    ExactlyOnce,
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LoadBalancingStrategy {
    /// Round robin
    RoundRobin,
    /// Least connections
    LeastConnections,
    /// Weighted round robin
    WeightedRoundRobin,
    /// Weighted least connections
    WeightedLeastConnections,
    /// Random selection
    Random,
    /// Weighted random
    WeightedRandom,
    /// Consistent hashing
    ConsistentHashing,
    /// Latency-based
    LatencyBased,
    /// Resource-based
    ResourceBased,
    /// Response time based
    ResponseTime,
    /// Adaptive (ML-based)
    Adaptive,
    /// Custom strategy
    Custom(String),
}

/// Compression preference
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CompressionPreference {
    /// No compression preferred
    None,
    /// Fast compression (LZ4)
    Fast,
    /// Balanced compression (Zstd)
    Balanced,
    /// High compression ratio (Brotli)
    HighRatio,
    /// Automatic selection
    Auto,
}

/// Supported compression algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// Gzip compression
    Gzip,
    /// LZ4 compression (fast)
    Lz4,
    /// Zstd compression (balanced)
    Zstd,
    /// Brotli compression (high ratio)
    Brotli,
    /// Custom compression
    Custom(String),
}

/// Encryption cipher types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionCipher {
    /// AES-256-GCM
    Aes256Gcm,
    /// ChaCha20-Poly1305
    ChaCha20Poly1305,
    /// Custom cipher
    Custom(String),
}

/// Compression information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionInfo {
    /// Compression algorithm used
    pub algorithm: CompressionAlgorithm,
    /// Original size before compression
    pub original_size: Option<u64>,
    /// Compressed size
    pub compressed_size: Option<u64>,
    /// Compression level
    pub level: Option<u8>,
}

/// Message signature for integrity verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSignature {
    /// Signature algorithm
    pub algorithm: SignatureAlgorithm,
    /// Digital signature
    pub signature: Vec<u8>,
    /// Public key identifier
    pub key_id: String,
    /// Signature timestamp
    pub timestamp: DateTime<Utc>,
}

/// Supported signature algorithms
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
    /// Custom algorithm
    Custom(String),
}

/// Distributed tracing context
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
    /// Baggage items
    pub baggage: HashMap<String, String>,
}

impl Default for TraceContext {
    fn default() -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            span_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: None,
            flags: 0,
            baggage: HashMap::new(),
        }
    }
}

// ============================================================================
// Message System Types
// ============================================================================

/// Enhanced Valkyrie message with advanced framing and metadata
/// This is the canonical ValkyrieMessage type used throughout the system
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

/// Advanced message header with extended capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Message ID
    pub id: Uuid,
    /// Source endpoint
    pub source: EndpointId,
    /// Destination endpoint
    pub destination: Option<EndpointId>,
    /// Protocol magic number and version
    pub protocol_info: ProtocolInfo,
    /// Message type and subtype
    pub message_type: MessageType,
    /// Stream identifier for multiplexing
    pub stream_id: Option<StreamId>,
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
    /// Sequence number
    pub sequence_number: u64,
    /// Acknowledgment number
    pub ack_number: Option<u64>,
}

// MessagePayload definition moved to message.rs - it's more comprehensive there
// We'll re-export it from message.rs to maintain the central types pattern

// MessageSignature definition moved to message.rs - it's more comprehensive there

// TraceContext definition moved to message.rs - it's more comprehensive there

// ProtocolInfo definition moved to message.rs - it's more comprehensive there

// MessageType definition moved to message.rs - it's more comprehensive there
// We'll re-export it from message.rs to maintain the central types pattern

// MessageFlags definition moved to message.rs - it's more comprehensive there

/// Message priority levels for QoS
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MessagePriority {
    /// Low priority
    Low,
    /// Normal priority
    Normal,
    /// High priority
    High,
    /// Critical priority (0-99)
    Critical(u8),
    /// System priority (100-999)
    System(u16),
    /// Job execution priority (1000-9999)
    JobExecution(u16),
    /// Data transfer priority (10000-99999)
    DataTransfer(u32),
    /// Logs and metrics priority (100000+)
    LogsMetrics(u32),
}

// RoutingInfo definition moved to message.rs - it's more comprehensive there

// ============================================================================
// Transport System Types
// ============================================================================

/// Transport capabilities for feature negotiation
/// This is the canonical TransportCapabilities type used throughout the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransportCapabilities {
    /// Maximum message size supported
    pub max_message_size: usize,
    /// Supported compression algorithms
    pub compression_algorithms: Vec<String>,
    /// Supported encryption algorithms
    pub encryption_algorithms: Vec<String>,
    /// Supports streaming
    pub supports_streaming: bool,
    /// Supports multiplexing
    pub supports_multiplexing: bool,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Security level
    pub security_level: SecurityLevel,
    /// Transport-specific features
    pub features: HashMap<String, serde_json::Value>,
    /// Supports compression
    pub supports_compression: bool,
    /// Supports encryption
    pub supports_encryption: bool,
    /// Supports flow control
    pub supports_flow_control: bool,
    /// Encryption ciphers
    pub encryption_ciphers: Vec<String>,
    /// Supports connection pooling
    pub supports_connection_pooling: bool,
    /// Supports failover
    pub supports_failover: bool,
}

/// Security levels for transport
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// No security
    None,
    /// Basic security (TLS)
    Basic,
    /// Enhanced security (mTLS)
    Enhanced,
    /// Maximum security (post-quantum)
    Maximum,
}

// ============================================================================
// Backward Compatibility Type Aliases
// ============================================================================

/// Alias for backward compatibility with engine module
pub type EngineMessage = ValkyrieMessage;

/// Alias for backward compatibility with client API
pub type ClientMessage = ValkyrieMessage;

/// Alias for backward compatibility with message module
pub type Message = ValkyrieMessage;

// ============================================================================
// Type Conversion Traits
// ============================================================================

// Type conversion implementations moved to message.rs where MessagePayload is defined

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during type conversions
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Invalid payload format: {0}")]
    InvalidPayload(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

// ============================================================================
// Default Implementations
// ============================================================================

// MessageFlags Default implementation moved to message.rs

impl Default for MessagePriority {
    fn default() -> Self {
        MessagePriority::Normal
    }
}

impl Default for ProtocolInfo {
    fn default() -> Self {
        Self {
            magic: 0x56414C4B, // "VALK"
            version: ProtocolVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            extensions: vec![],
        }
    }
}

impl Default for MessageFlags {
    fn default() -> Self {
        Self {
            requires_ack: false,
            compressed: false,
            encrypted: false,
            fragmented: false,
            urgent: false,
            retransmission: false,
            binary_data: false,
            custom_flags: 0,
        }
    }
}

impl Default for RoutingInfo {
    fn default() -> Self {
        Self {
            source: "unknown".to_string(),
            destination: DestinationType::Broadcast,
            hints: RoutingHints {
                preferred_transport: None,
                latency_sensitive: false,
                bandwidth_requirements: None,
                reliability_level: ReliabilityLevel::BestEffort,
                cacheable: false,
                compression_preference: CompressionPreference::Auto,
            },
            load_balancing: LoadBalancingStrategy::RoundRobin,
            path: vec![],
        }
    }
}

impl Default for CompressionInfo {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::None,
            original_size: None,
            compressed_size: None,
            level: None,
        }
    }
}

// ProtocolInfo Default implementation moved to message.rs

// RoutingInfo Default implementation moved to message.rs

impl Default for TransportCapabilities {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024, // 1MB
            compression_algorithms: vec!["gzip".to_string(), "lz4".to_string()],
            encryption_algorithms: vec!["aes-256-gcm".to_string()],
            supports_streaming: true,
            supports_multiplexing: true,
            max_connections: 1000,
            security_level: SecurityLevel::Enhanced,
            features: HashMap::new(),
            supports_compression: true,
            supports_encryption: true,
            supports_flow_control: true,
            encryption_ciphers: vec!["aes-256-gcm".to_string()],
            supports_connection_pooling: true,
            supports_failover: true,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

impl ValkyrieMessage {
    /// Create a new ValkyrieMessage with minimal required fields
    pub fn new(message_type: MessageType, payload: MessagePayload) -> Self {
        Self {
            header: MessageHeader {
                id: Uuid::new_v4(),
                source: "unknown".to_string(),
                destination: None,
                protocol_info: ProtocolInfo::default(),
                message_type,
                stream_id: None,
                flags: MessageFlags::default(),
                priority: MessagePriority::default(),
                timestamp: Utc::now(),
                ttl: None,
                correlation_id: None,
                routing: RoutingInfo::default(),
                compression: CompressionInfo::default(),
                sequence_number: 0,
                ack_number: None,
            },
            payload,
            signature: None,
            trace_context: None,
        }
    }

    /// Create a heartbeat message
    pub fn heartbeat() -> Self {
        Self::new(MessageType::Ping, MessagePayload::Empty)
    }

    /// Create a data message
    pub fn data(payload: Vec<u8>) -> Self {
        Self::new(MessageType::StreamData, MessagePayload::Binary(payload))
    }
}

impl MessagePriority {
    /// Get the numeric priority value for comparison
    pub fn value(&self) -> u32 {
        match self {
            MessagePriority::Low => 100000,
            MessagePriority::Normal => 50000,
            MessagePriority::High => 10000,
            MessagePriority::Critical(p) => *p as u32,
            MessagePriority::System(p) => *p as u32,
            MessagePriority::JobExecution(p) => *p as u32,
            MessagePriority::DataTransfer(p) => *p,
            MessagePriority::LogsMetrics(p) => *p,
        }
    }
}