use serde::{Deserialize, Serialize};

// Import canonical types from central types module
use super::types::{
    ValkyrieMessage, MessagePriority, TraceContext,
    CompressionPreference, StreamId, CorrelationId, Duration
};

/// Message validation errors
#[derive(Debug, thiserror::Error)]
pub enum MessageValidationError {
    #[error("Invalid protocol magic number: expected {expected:x}, got {actual:x}")]
    InvalidMagic { expected: u32, actual: u32 },
    
    #[error("Unsupported protocol version: {version:?}")]
    UnsupportedVersion { version: crate::core::networking::valkyrie::types::ProtocolVersion },
    
    #[error("Message too large: {size} bytes exceeds limit of {limit}")]
    MessageTooLarge { size: usize, limit: usize },
    
    #[error("Message expired: TTL {ttl:?} exceeded")]
    MessageExpired { ttl: Duration },
    
    #[error("Invalid signature: {reason}")]
    InvalidSignature { reason: String },
    
    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },
    
    #[error("Invalid routing destination")]
    InvalidDestination,
    
    #[error("Compression error: {message}")]
    CompressionError { message: String },
}

/// Message validator
pub struct MessageValidator {
    /// Maximum message size
    pub max_message_size: usize,
    /// Supported protocol versions
    pub supported_versions: Vec<crate::core::networking::valkyrie::types::ProtocolVersion>,
    /// Signature verification enabled
    pub verify_signatures: bool,
}

impl MessageValidator {
    /// Create a new message validator
    pub fn new() -> Self {
        Self {
            max_message_size: 64 * 1024 * 1024, // 64MB default
            supported_versions: vec![crate::core::networking::valkyrie::types::ProtocolVersion { major: 1, minor: 0, patch: 0 }],
            verify_signatures: true,
        }
    }

    /// Validate a message
    pub fn validate(&self, message: &ValkyrieMessage) -> Result<(), MessageValidationError> {
        // Check protocol magic
        if message.header.protocol_info.magic != 0x56414C4B {
            return Err(MessageValidationError::InvalidMagic {
                expected: 0x56414C4B,
                actual: message.header.protocol_info.magic,
            });
        }

        // Check protocol version
        if !self.supported_versions.contains(&message.header.protocol_info.version) {
            return Err(MessageValidationError::UnsupportedVersion {
                version: message.header.protocol_info.version.clone(),
            });
        }

        // Check message size
        let size = message.estimated_size();
        if size > self.max_message_size {
            return Err(MessageValidationError::MessageTooLarge {
                size,
                limit: self.max_message_size,
            });
        }

        // Check TTL
        if message.is_expired() {
            return Err(MessageValidationError::MessageExpired {
                ttl: message.header.ttl.unwrap_or(Duration::ZERO),
            });
        }

        // Validate signature if present and verification is enabled
        if self.verify_signatures && message.signature.is_some() {
            // TODO: Implement signature verification
            // This would involve cryptographic verification of the signature
        }

        Ok(())
    }
}

impl Default for MessageValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ValkyrieMessage {
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

    /// Set stream ID
    pub fn with_stream_id(mut self, stream_id: StreamId) -> Self {
        self.header.stream_id = Some(stream_id);
        self
    }

    /// Set compression preference
    pub fn with_compression(mut self, preference: CompressionPreference) -> Self {
        self.header.routing.hints.compression_preference = preference;
        self
    }

    /// Set reliability level
    pub fn with_reliability(mut self, level: crate::core::networking::valkyrie::types::ReliabilityLevel) -> Self {
        self.header.routing.hints.reliability_level = level;
        self
    }

    /// Mark as requiring acknowledgment
    pub fn requires_ack(mut self) -> Self {
        self.header.flags.requires_ack = true;
        self
    }

    /// Mark as urgent
    pub fn urgent(mut self) -> Self {
        self.header.flags.urgent = true;
        self.header.priority = MessagePriority::High;
        self
    }

    /// Add tracing context
    pub fn with_trace_context(mut self, trace_context: TraceContext) -> Self {
        self.trace_context = Some(trace_context);
        self
    }

    /// Get message size estimate
    pub fn estimated_size(&self) -> usize {
        // Rough estimate based on serialized size
        serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
    }

    /// Check if message has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.header.ttl {
            let elapsed = chrono::Utc::now().signed_duration_since(self.header.timestamp);
            elapsed.to_std().unwrap_or(Duration::ZERO) > ttl
        } else {
            false
        }
    }

    /// Get message age
    pub fn age(&self) -> Duration {
        let elapsed = chrono::Utc::now().signed_duration_since(self.header.timestamp);
        elapsed.to_std().unwrap_or(Duration::ZERO)
    }
}

/// Message processing errors
#[derive(Debug, thiserror::Error)]
pub enum MessageProcessingError {
    #[error("Deserialization error: {reason}")]
    DeserializationError { reason: String },
    
    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },
    
    #[error("Validation error: {reason}")]
    ValidationError { reason: String },
    
    #[error("Compression error: {reason}")]
    CompressionError { reason: String },
    
    #[error("Integrity error: {reason}")]
    IntegrityError { reason: String },
}

/// Compression error types
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },
    
    #[error("Compression failed: {reason}")]
    CompressionFailed { reason: String },
}

/// Message processing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageProcessingMetrics {
    /// Total messages processed
    pub total_processed: u64,
    /// Messages compressed
    pub compressed_messages: u64,
    /// Messages with integrity verification
    pub verified_messages: u64,
    /// Processing errors
    pub processing_errors: u64,
    /// Average processing time
    pub average_processing_time_ms: f64,
    /// Compression ratio statistics
    pub compression_stats: CompressionStats,
}

/// Compression statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    /// Total bytes before compression
    pub total_original_bytes: u64,
    /// Total bytes after compression
    pub total_compressed_bytes: u64,
    /// Average compression ratio
    pub average_compression_ratio: f64,
    /// Best compression ratio achieved
    pub best_compression_ratio: f64,
    /// Worst compression ratio
    pub worst_compression_ratio: f64,
}

impl Default for MessageProcessingMetrics {
    fn default() -> Self {
        Self {
            total_processed: 0,
            compressed_messages: 0,
            verified_messages: 0,
            processing_errors: 0,
            average_processing_time_ms: 0.0,
            compression_stats: CompressionStats::default(),
        }
    }
}

impl Default for CompressionStats {
    fn default() -> Self {
        Self {
            total_original_bytes: 0,
            total_compressed_bytes: 0,
            average_compression_ratio: 1.0,
            best_compression_ratio: 1.0,
            worst_compression_ratio: 1.0,
        }
    }
}