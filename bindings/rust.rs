//! Rust Language Bindings for Valkyrie Protocol
//!
//! This module provides the native Rust API for the Valkyrie Protocol.
//! Since the protocol is implemented in Rust, this is the primary interface.

pub use crate::api::valkyrie::*;

/// Re-export commonly used types for convenience
pub mod prelude {
    pub use crate::api::valkyrie::{
        ClientConfig, ClientMessage, ClientMessageHandler, ClientMessagePriority,
        ClientMessageType, ClientPayload, ClientStats, ValkyrieClient, ValkyrieListener,
    };
    pub use crate::core::networking::valkyrie::{
        ConnectionId, Endpoint, MessagePriority, MessageType, ValkyrieConfig, ValkyrieEngine,
        ValkyrieMessage,
    };
}

/// Rust-specific utilities and helpers
pub mod utils {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Thread-safe client wrapper for multi-threaded applications
    pub type SharedClient = Arc<RwLock<ValkyrieClient>>;

    /// Create a shared client instance
    pub async fn create_shared_client(config: ClientConfig) -> crate::error::Result<SharedClient> {
        let client = ValkyrieClient::new(config).await?;
        Ok(Arc::new(RwLock::new(client)))
    }

    /// Convenience macro for creating messages
    #[macro_export]
    macro_rules! valkyrie_message {
        (text: $content:expr) => {
            ClientMessage::text($content)
        };
        (binary: $data:expr) => {
            ClientMessage::binary($data)
        };
        (json: $data:expr) => {
            ClientMessage::json(&$data)
        };
    }

    /// Convenience macro for message handlers
    #[macro_export]
    macro_rules! message_handler {
        ($handler:expr) => {
            Box::new($handler) as Box<dyn ClientMessageHandler>
        };
    }
}

/// Async utilities for Rust applications
pub mod async_utils {
    use super::*;
    use futures::stream::{Stream, StreamExt};
    use std::pin::Pin;
    use tokio::sync::mpsc;

    /// Message stream for handling incoming messages
    pub type MessageStream = Pin<Box<dyn Stream<Item = (String, ClientMessage)> + Send>>;

    /// Create a message stream from a client
    pub fn create_message_stream(
        client: Arc<ValkyrieClient>,
    ) -> (MessageStream, mpsc::Sender<(String, ClientMessage)>) {
        let (tx, mut rx) = mpsc::channel(100);

        let stream = async_stream::stream! {
            while let Some(message) = rx.recv().await {
                yield message;
            }
        };

        (Box::pin(stream), tx)
    }

    /// Batch message sender for high-throughput scenarios
    pub struct BatchSender {
        client: Arc<ValkyrieClient>,
        batch_size: usize,
        batch: Vec<(String, ClientMessage)>,
    }

    impl BatchSender {
        pub fn new(client: Arc<ValkyrieClient>, batch_size: usize) -> Self {
            Self {
                client,
                batch_size,
                batch: Vec::with_capacity(batch_size),
            }
        }

        pub async fn send(
            &mut self,
            connection_id: String,
            message: ClientMessage,
        ) -> crate::error::Result<()> {
            self.batch.push((connection_id, message));

            if self.batch.len() >= self.batch_size {
                self.flush().await?;
            }

            Ok(())
        }

        pub async fn flush(&mut self) -> crate::error::Result<()> {
            for (connection_id, message) in self.batch.drain(..) {
                self.client.send_message(&connection_id, message).await?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::valkyrie::*;

    #[tokio::test]
    async fn test_rust_bindings() {
        let config = ClientConfig::default();
        let client = ValkyrieClient::new(config).await.unwrap();

        // Test that we can use the client through Rust bindings
        let stats = client.get_stats().await;
        assert_eq!(stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_shared_client() {
        let config = ClientConfig::default();
        let shared_client = utils::create_shared_client(config).await.unwrap();

        // Test that we can access the shared client
        let stats = shared_client.read().await.get_stats().await;
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn test_message_macro() {
        let message = valkyrie_message!(text: "Hello, World!");
        assert!(matches!(message.message_type, ClientMessageType::Text));
    }
}
