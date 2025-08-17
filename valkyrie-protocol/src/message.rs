//! Message types and serialization for Valkyrie Protocol

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Message priority levels for QoS routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessagePriority {
    /// Critical system messages (highest priority)
    Critical = 0,
    /// High priority messages
    High = 1,
    /// Normal priority messages (default)
    Normal = 2,
    /// Low priority messages
    Low = 3,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Message types for different use cases
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    /// Request message expecting a response
    Request,
    /// Response to a request
    Response,
    /// One-way notification
    Notification,
    /// Heartbeat/keepalive message
    Heartbeat,
    /// System control message
    Control,
    /// Data transfer message
    Data,
    /// Custom message type
    Custom(String),
}

impl Default for MessageType {
    fn default() -> Self {
        Self::Request
    }
}

/// Core message structure for Valkyrie Protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: Uuid,
    
    /// Message type
    pub message_type: MessageType,
    
    /// Message priority for QoS routing
    pub priority: MessagePriority,
    
    /// Source endpoint identifier
    pub source: String,
    
    /// Destination endpoint identifier
    pub destination: String,
    
    /// Message payload
    pub payload: Vec<u8>,
    
    /// Message headers for metadata
    pub headers: HashMap<String, String>,
    
    /// Timestamp when message was created
    pub timestamp: DateTime<Utc>,
    
    /// Optional correlation ID for request/response matching
    pub correlation_id: Option<Uuid>,
    
    /// Time-to-live in milliseconds
    pub ttl_ms: Option<u64>,
}

impl Message {
    /// Create a new message with default values
    pub fn new<T: Into<Vec<u8>>>(payload: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type: MessageType::default(),
            priority: MessagePriority::default(),
            source: String::new(),
            destination: String::new(),
            payload: payload.into(),
            headers: HashMap::new(),
            timestamp: Utc::now(),
            correlation_id: None,
            ttl_ms: None,
        }
    }
    
    /// Create a request message
    pub fn request<T: Into<Vec<u8>>>(payload: T) -> Self {
        let mut msg = Self::new(payload);
        msg.message_type = MessageType::Request;
        msg.correlation_id = Some(Uuid::new_v4());
        msg
    }
    
    /// Create a response message
    pub fn response<T: Into<Vec<u8>>>(payload: T, correlation_id: Uuid) -> Self {
        let mut msg = Self::new(payload);
        msg.message_type = MessageType::Response;
        msg.correlation_id = Some(correlation_id);
        msg
    }
    
    /// Create a notification message
    pub fn notification<T: Into<Vec<u8>>>(payload: T) -> Self {
        let mut msg = Self::new(payload);
        msg.message_type = MessageType::Notification;
        msg
    }
    
    /// Set message priority
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }
    
    /// Set source endpoint
    pub fn with_source<S: Into<String>>(mut self, source: S) -> Self {
        self.source = source.into();
        self
    }
    
    /// Set destination endpoint
    pub fn with_destination<S: Into<String>>(mut self, destination: S) -> Self {
        self.destination = destination.into();
        self
    }
    
    /// Add a header
    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
    
    /// Set TTL in milliseconds
    pub fn with_ttl_ms(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = Some(ttl_ms);
        self
    }
    
    /// Check if message has expired based on TTL
    pub fn is_expired(&self) -> bool {
        if let Some(ttl_ms) = self.ttl_ms {
            let elapsed = Utc::now().signed_duration_since(self.timestamp);
            elapsed.num_milliseconds() as u64 > ttl_ms
        } else {
            false
        }
    }
    
    /// Get payload as string (if valid UTF-8)
    pub fn payload_as_string(&self) -> Option<String> {
        String::from_utf8(self.payload.clone()).ok()
    }
    
    /// Get payload as JSON value
    pub fn payload_as_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.payload)
    }
    
    /// Set payload from JSON value
    pub fn with_json_payload<T: Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> {
        self.payload = serde_json::to_vec(value)?;
        Ok(self)
    }
    
    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }
    
    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}