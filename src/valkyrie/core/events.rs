//! Valkyrie Protocol Event System
//!
//! This module provides event handling and publishing capabilities
//! for the Valkyrie Protocol.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use chrono::{DateTime, Utc};
use async_trait::async_trait;

use crate::valkyrie::core::{ConnectionId};
use crate::valkyrie::transport::Endpoint;
use crate::valkyrie::Result;

/// Valkyrie Protocol events
#[derive(Debug, Clone)]
pub enum ValkyrieEvent {
    /// Engine started
    EngineStarted {
        timestamp: DateTime<Utc>,
    },
    /// Engine stopped
    EngineStopped {
        timestamp: DateTime<Utc>,
    },
    /// Connection established
    ConnectionEstablished {
        connection_id: ConnectionId,
        endpoint: Endpoint,
        timestamp: DateTime<Utc>,
    },
    /// Connection closed
    ConnectionClosed {
        connection_id: ConnectionId,
        timestamp: DateTime<Utc>,
    },
    /// Listener started
    ListenerStarted {
        address: std::net::SocketAddr,
        timestamp: DateTime<Utc>,
    },
    /// Listener stopped
    ListenerStopped {
        address: std::net::SocketAddr,
        timestamp: DateTime<Utc>,
    },
    /// Message sent
    MessageSent {
        connection_id: ConnectionId,
        message_id: uuid::Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Message received
    MessageReceived {
        connection_id: ConnectionId,
        message_id: uuid::Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Error occurred
    Error {
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Custom event
    Custom {
        event_type: String,
        data: HashMap<String, String>,
        timestamp: DateTime<Utc>,
    },
}

/// Event handler trait
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: &ValkyrieEvent) -> Result<()>;
}

/// Event bus for publishing and subscribing to events
pub struct EventBus {
    /// Event handlers
    handlers: Arc<RwLock<Vec<Arc<dyn EventHandler>>>>,
    /// Event channel
    event_tx: mpsc::UnboundedSender<ValkyrieEvent>,
    /// Event receiver (for internal processing)
    _event_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<ValkyrieEvent>>>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            _event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }
    
    /// Subscribe to events with a handler
    pub async fn subscribe(&self, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }
    
    /// Unsubscribe a handler (simplified - removes all instances)
    pub async fn unsubscribe(&self, _handler: Arc<dyn EventHandler>) {
        // In a real implementation, this would remove the specific handler
        // For now, this is a placeholder
    }
    
    /// Publish an event
    pub async fn publish(&self, event: ValkyrieEvent) {
        // Send to internal channel for processing
        if let Err(_) = self.event_tx.send(event.clone()) {
            // Channel is closed, ignore
            return;
        }
        
        // Notify all handlers
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.handle_event(&event).await {
                // Log error but continue with other handlers
                eprintln!("Event handler error: {}", e);
            }
        }
    }
    
    /// Start the event processing loop
    pub async fn start(&self) -> Result<()> {
        // In a real implementation, this would start a background task
        // to process events from the channel
        Ok(())
    }
    
    /// Stop the event bus
    pub async fn stop(&self) -> Result<()> {
        // In a real implementation, this would stop the background task
        Ok(())
    }
}

/// Default event handler that logs events
pub struct LoggingEventHandler;

#[async_trait]
impl EventHandler for LoggingEventHandler {
    async fn handle_event(&self, event: &ValkyrieEvent) -> Result<()> {
        match event {
            ValkyrieEvent::EngineStarted { timestamp } => {
                println!("Engine started at {}", timestamp);
            }
            ValkyrieEvent::EngineStopped { timestamp } => {
                println!("Engine stopped at {}", timestamp);
            }
            ValkyrieEvent::ConnectionEstablished { connection_id, endpoint, timestamp } => {
                println!("Connection {} established to {}:{} at {}", 
                    connection_id, endpoint.address, endpoint.port, timestamp);
            }
            ValkyrieEvent::ConnectionClosed { connection_id, timestamp } => {
                println!("Connection {} closed at {}", connection_id, timestamp);
            }
            ValkyrieEvent::Error { error, timestamp } => {
                eprintln!("Error at {}: {}", timestamp, error);
            }
            _ => {
                // Handle other events
            }
        }
        Ok(())
    }
}

/// Metrics event handler that collects statistics
pub struct MetricsEventHandler {
    /// Event counters
    counters: Arc<RwLock<HashMap<String, u64>>>,
}

impl MetricsEventHandler {
    /// Create a new metrics event handler
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get event counters
    pub async fn get_counters(&self) -> HashMap<String, u64> {
        self.counters.read().await.clone()
    }
    
    /// Increment a counter
    async fn increment_counter(&self, name: &str) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += 1;
    }
}

#[async_trait]
impl EventHandler for MetricsEventHandler {
    async fn handle_event(&self, event: &ValkyrieEvent) -> Result<()> {
        match event {
            ValkyrieEvent::EngineStarted { .. } => {
                self.increment_counter("engine_started").await;
            }
            ValkyrieEvent::EngineStopped { .. } => {
                self.increment_counter("engine_stopped").await;
            }
            ValkyrieEvent::ConnectionEstablished { .. } => {
                self.increment_counter("connections_established").await;
            }
            ValkyrieEvent::ConnectionClosed { .. } => {
                self.increment_counter("connections_closed").await;
            }
            ValkyrieEvent::MessageSent { .. } => {
                self.increment_counter("messages_sent").await;
            }
            ValkyrieEvent::MessageReceived { .. } => {
                self.increment_counter("messages_received").await;
            }
            ValkyrieEvent::Error { .. } => {
                self.increment_counter("errors").await;
            }
            _ => {
                self.increment_counter("other_events").await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    struct TestEventHandler {
        call_count: Arc<AtomicUsize>,
    }
    
    impl TestEventHandler {
        fn new() -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
        
        fn get_call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }
    
    #[async_trait]
    impl EventHandler for TestEventHandler {
        async fn handle_event(&self, _event: &ValkyrieEvent) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_event_bus() {
        let event_bus = EventBus::new();
        let handler = Arc::new(TestEventHandler::new());
        
        event_bus.subscribe(handler.clone()).await;
        
        let event = ValkyrieEvent::EngineStarted {
            timestamp: Utc::now(),
        };
        
        event_bus.publish(event).await;
        
        // Give some time for async processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        assert_eq!(handler.get_call_count(), 1);
    }
    
    #[tokio::test]
    async fn test_metrics_handler() {
        let handler = MetricsEventHandler::new();
        
        let event = ValkyrieEvent::ConnectionEstablished {
            connection_id: uuid::Uuid::new_v4(),
            endpoint: Endpoint::tcp("localhost", 8080),
            timestamp: Utc::now(),
        };
        
        handler.handle_event(&event).await.unwrap();
        
        let counters = handler.get_counters().await;
        assert_eq!(counters.get("connections_established"), Some(&1));
    }
}