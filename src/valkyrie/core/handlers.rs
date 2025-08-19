//! Message handlers for the Valkyrie Protocol

use crate::valkyrie::{Result, ValkyrieMessage};
use async_trait::async_trait;

/// Message handler trait for processing incoming messages
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message
    async fn handle_message(&self, message: ValkyrieMessage) -> Result<Option<ValkyrieMessage>>;
}

/// Default message handler that logs messages
pub struct LoggingMessageHandler;

#[async_trait]
impl MessageHandler for LoggingMessageHandler {
    async fn handle_message(&self, message: ValkyrieMessage) -> Result<Option<ValkyrieMessage>> {
        println!("Received message: {:?}", message.header.message_type);
        Ok(None)
    }
}

/// Echo message handler that returns the same message
pub struct EchoMessageHandler;

#[async_trait]
impl MessageHandler for EchoMessageHandler {
    async fn handle_message(&self, message: ValkyrieMessage) -> Result<Option<ValkyrieMessage>> {
        Ok(Some(message))
    }
}
