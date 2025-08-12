//! Type Conversion Utilities for Valkyrie Protocol API
//!
//! This module provides comprehensive type conversion functions between
//! the high-level client API types and the low-level protocol types.

use std::collections::HashMap;
use uuid::Uuid;

use crate::valkyrie::api::{
    ClientMessage, ClientMessageType, ClientMessagePriority, ClientPayload
};

use crate::valkyrie::{
    ValkyrieMessage as EngineMessage, MessageType as EngineMessageType,
    MessagePriority as EnginePriority,
    MessagePayload as CorePayload
};

use crate::error::Result;

/// Convert ClientMessage to EngineMessage (low-level protocol message)
pub fn client_message_to_engine_message(message: ClientMessage) -> Result<EngineMessage> {
    let engine_payload = match message.payload {
        ClientPayload::Text(text) => CorePayload::Text(text),
        ClientPayload::Binary(data) => CorePayload::Binary(data),
        ClientPayload::Json(value) => CorePayload::Json(value),
        ClientPayload::Empty => CorePayload::Empty,
    };

    let message_type = match message.message_type {
        ClientMessageType::Text | ClientMessageType::Json | ClientMessageType::Binary => EngineMessageType::Data,
        ClientMessageType::Control => EngineMessageType::Control,
        ClientMessageType::Custom(_) => EngineMessageType::Custom(0x1000),
    };
    
    let mut engine_message = EngineMessage::new(message_type, engine_payload);
    
    // Set priority
    engine_message.header.priority = match message.priority {
        ClientMessagePriority::Low => EnginePriority::Low,
        ClientMessagePriority::Normal => EnginePriority::Normal,
        ClientMessagePriority::High => EnginePriority::High,
        ClientMessagePriority::Critical => EnginePriority::Critical,
    };
    
    // Set correlation ID if provided
    if let Some(correlation_id_str) = message.correlation_id {
        if let Ok(correlation_id) = Uuid::parse_str(&correlation_id_str) {
            engine_message.header.correlation_id = Some(correlation_id);
        }
    }
    
    // Set TTL if provided
    if let Some(ttl) = message.ttl {
        engine_message.header.ttl = Some(ttl);
    }

    Ok(engine_message)
}

/// Convert EngineMessage to ClientMessage
pub fn engine_message_to_client_message(message: EngineMessage) -> Result<ClientMessage> {
    let payload = match message.payload {
        CorePayload::Text(text) => ClientPayload::Text(text),
        CorePayload::Binary(data) => ClientPayload::Binary(data),
        CorePayload::Json(value) => ClientPayload::Json(value),
        CorePayload::Empty => ClientPayload::Empty,
        _ => ClientPayload::Binary(vec![]), // Fallback for other types
    };

    let message_type = match message.header.message_type {
        EngineMessageType::Data => ClientMessageType::Text, // Default to text
        EngineMessageType::Control => ClientMessageType::Control,
        EngineMessageType::Custom(_) => ClientMessageType::Custom("custom".to_string()),
        _ => ClientMessageType::Text,
    };

    let priority = match message.header.priority {
        EnginePriority::Low => ClientMessagePriority::Low,
        EnginePriority::Normal => ClientMessagePriority::Normal,
        EnginePriority::High => ClientMessagePriority::High,
        EnginePriority::Critical => ClientMessagePriority::Critical,
    };

    Ok(ClientMessage {
        message_type,
        payload,
        priority,
        correlation_id: message.header.correlation_id.map(|id| id.to_string()),
        ttl: message.header.ttl,
        metadata: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_client_to_engine_message_conversion() {
        let client_message = ClientMessage::text("Hello, World!");
        let result = client_message_to_engine_message(client_message);
        assert!(result.is_ok());
        
        let engine_message = result.unwrap();
        assert_eq!(engine_message.header.message_type, EngineMessageType::Data);
        assert!(matches!(engine_message.payload, CorePayload::Text(_)));
    }
    
    #[test]
    fn test_engine_to_client_message_conversion() {
        let engine_message = EngineMessage::text("Hello, World!");
        let result = engine_message_to_client_message(engine_message);
        assert!(result.is_ok());
        
        let client_message = result.unwrap();
        assert_eq!(client_message.message_type, ClientMessageType::Text);
        assert!(matches!(client_message.payload, ClientPayload::Text(_)));
    }
}