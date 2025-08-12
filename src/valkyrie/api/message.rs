//! Client-facing message types for the Valkyrie Protocol API

use serde::{Deserialize, Serialize};
use crate::valkyrie::Result;

/// Client message for external API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    /// Message type
    pub message_type: ClientMessageType,
    /// Message payload
    pub payload: ClientPayload,
    /// Message priority
    pub priority: ClientMessagePriority,
    /// Correlation ID for request-response matching
    pub correlation_id: Option<String>,
    /// Time to live
    pub ttl: Option<std::time::Duration>,
    /// Message metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl ClientMessage {
    /// Create a text message
    pub fn text(content: &str) -> Self {
        Self {
            message_type: ClientMessageType::Text,
            payload: ClientPayload::Text(content.to_string()),
            priority: ClientMessagePriority::Normal,
            correlation_id: None,
            ttl: None,
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Create a binary message
    pub fn binary(data: Vec<u8>) -> Self {
        Self {
            message_type: ClientMessageType::Binary,
            payload: ClientPayload::Binary(data),
            priority: ClientMessagePriority::Normal,
            correlation_id: None,
            ttl: None,
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Create a JSON message
    pub fn json<T: serde::Serialize>(data: &T) -> Result<Self> {
        let json_value = serde_json::to_value(data)
            .map_err(|e| crate::valkyrie::ValkyrieError::InternalServerError(format!("JSON serialization failed: {}", e)))?;
        
        Ok(Self {
            message_type: ClientMessageType::Json,
            payload: ClientPayload::Json(json_value),
            priority: ClientMessagePriority::Normal,
            correlation_id: None,
            ttl: None,
            metadata: std::collections::HashMap::new(),
        })
    }
    
    /// Set message priority
    pub fn with_priority(mut self, priority: ClientMessagePriority) -> Self {
        self.priority = priority;
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Client message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMessageType {
    /// Text message
    Text,
    /// Binary message
    Binary,
    /// JSON message
    Json,
    /// Control message
    Control,
    /// Custom message type
    Custom(String),
}

/// Client message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientPayload {
    /// Text payload
    Text(String),
    /// Binary payload
    Binary(Vec<u8>),
    /// JSON payload
    Json(serde_json::Value),
    /// Empty payload
    Empty,
}

/// Client message priority
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ClientMessagePriority {
    /// Low priority
    Low,
    /// Normal priority
    Normal,
    /// High priority
    High,
    /// Critical priority
    Critical,
}

/// Broadcast result for client API
#[derive(Debug, Clone)]
pub struct BroadcastResult {
    /// Total number of connections attempted
    pub total: usize,
    /// Number of successful sends
    pub successful: usize,
    /// Number of failed sends
    pub failed: usize,
    /// Individual results per connection
    pub results: std::collections::HashMap<String, Result<()>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_client_message_creation() {
        let message = ClientMessage::text("Hello, World!");
        assert_eq!(message.message_type, ClientMessageType::Text);
        assert!(matches!(message.payload, ClientPayload::Text(_)));
        assert_eq!(message.priority, ClientMessagePriority::Normal);
    }
    
    #[test]
    fn test_client_message_with_priority() {
        let message = ClientMessage::text("Important message")
            .with_priority(ClientMessagePriority::High);
        assert_eq!(message.priority, ClientMessagePriority::High);
    }
    
    #[test]
    fn test_client_message_with_metadata() {
        let message = ClientMessage::text("Message with metadata")
            .with_metadata("source", "test");
        assert_eq!(message.metadata.get("source"), Some(&"test".to_string()));
    }
    
    #[test]
    fn test_json_message() {
        let data = serde_json::json!({"key": "value"});
        let result = ClientMessage::json(&data);
        assert!(result.is_ok());
        
        let message = result.unwrap();
        assert_eq!(message.message_type, ClientMessageType::Json);
    }
}