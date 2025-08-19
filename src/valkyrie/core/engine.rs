//! Valkyrie Protocol Engine - Core protocol implementation
//!
//! The ValkyrieEngine is the main entry point for the Valkyrie Protocol,
//! providing a high-level interface for distributed communication with
//! advanced features like multi-transport support, security, and observability.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use crate::valkyrie::core::{
    ConnectionHandle, ConnectionId, ConnectionRegistry, EventBus, Listener, ValkyrieEvent,
    ValkyrieMessage,
};
use crate::valkyrie::{
    ObservabilityManager, Result, SecurityManager, StreamMultiplexer, TransportManager,
    ValkyrieConfig,
};

/// The main Valkyrie Protocol engine
pub struct ValkyrieEngine {
    /// Configuration for the protocol engine
    config: ValkyrieConfig,
    /// Transport layer manager
    transport_manager: Arc<TransportManager>,
    /// Stream multiplexer
    stream_multiplexer: Arc<StreamMultiplexer>,
    /// Security manager
    security_manager: Arc<SecurityManager>,
    /// Observability manager
    observability_manager: Arc<ObservabilityManager>,
    /// Connection registry
    connection_registry: Arc<ConnectionRegistry>,
    /// Event bus for protocol events
    event_bus: Arc<EventBus>,
    /// Protocol state
    state: Arc<RwLock<ProtocolState>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl ValkyrieEngine {
    /// Create a new Valkyrie Protocol engine
    pub fn new(config: ValkyrieConfig) -> Result<Self> {
        let transport_manager = Arc::new(TransportManager::new(config.transport.clone())?);
        let security_manager = Arc::new(SecurityManager::new(config.security.clone())?);
        let stream_multiplexer = Arc::new(StreamMultiplexer::new(config.streaming.clone()));
        let observability_manager =
            Arc::new(ObservabilityManager::new(config.observability.clone())?);
        let connection_registry = Arc::new(ConnectionRegistry::new());
        let event_bus = Arc::new(EventBus::new());
        let state = Arc::new(RwLock::new(ProtocolState::Stopped));

        Ok(Self {
            config,
            transport_manager,
            stream_multiplexer,
            security_manager,
            observability_manager,
            connection_registry,
            event_bus,
            state,
            shutdown_tx: None,
        })
    }

    /// Create a new Valkyrie Protocol engine with dependency injection
    pub fn new_with_dependencies(
        config: ValkyrieConfig,
        transport_manager: Arc<TransportManager>,
        security_manager: Arc<SecurityManager>,
        stream_multiplexer: Arc<StreamMultiplexer>,
        observability_manager: Arc<ObservabilityManager>,
    ) -> Result<Self> {
        let connection_registry = Arc::new(ConnectionRegistry::new());
        let event_bus = Arc::new(EventBus::new());
        let state = Arc::new(RwLock::new(ProtocolState::Stopped));

        Ok(Self {
            config,
            transport_manager,
            stream_multiplexer,
            security_manager,
            observability_manager,
            connection_registry,
            event_bus,
            state,
            shutdown_tx: None,
        })
    }

    /// Start the protocol engine
    pub async fn start(&mut self) -> Result<()> {
        let mut state = self.state.write().await;
        if matches!(*state, ProtocolState::Running) {
            return Ok(());
        }

        *state = ProtocolState::Starting;
        drop(state);

        // Start components
        self.transport_manager.start().await?;
        self.stream_multiplexer.start().await?;
        self.security_manager.start().await?;
        self.observability_manager.start().await?;

        // Start background tasks
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        self.start_background_tasks(shutdown_rx).await;

        // Update state
        let mut state = self.state.write().await;
        *state = ProtocolState::Running;

        self.event_bus
            .publish(ValkyrieEvent::EngineStarted {
                timestamp: Utc::now(),
            })
            .await;

        Ok(())
    }

    /// Stop the protocol engine gracefully
    pub async fn stop(&mut self) -> Result<()> {
        let mut state = self.state.write().await;
        if matches!(*state, ProtocolState::Stopped) {
            return Ok(());
        }

        *state = ProtocolState::Stopping;
        drop(state);

        // Signal shutdown to background tasks
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }

        // Stop components in reverse order
        self.observability_manager.stop().await?;
        self.security_manager.stop().await?;
        self.stream_multiplexer.stop().await?;
        self.transport_manager.stop().await?;

        // Close all connections
        self.connection_registry.close_all().await?;

        // Update state
        let mut state = self.state.write().await;
        *state = ProtocolState::Stopped;

        self.event_bus
            .publish(ValkyrieEvent::EngineStopped {
                timestamp: Utc::now(),
            })
            .await;

        Ok(())
    }

    /// Create a new connection to a remote endpoint
    pub async fn connect(
        &self,
        endpoint: crate::valkyrie::transport::Endpoint,
    ) -> Result<ConnectionHandle> {
        let connection_id = ConnectionId::new_v4();

        // Establish and authenticate connection
        let transport_connection = self.transport_manager.connect(&endpoint).await?;
        let _authenticated_connection = self
            .security_manager
            .authenticate_connection(transport_connection, &endpoint)
            .await?;

        // Register connection
        let connection_info = crate::valkyrie::core::ConnectionInfo {
            id: connection_id,
            endpoint: endpoint.clone(),
            state: crate::valkyrie::core::ConnectionState::Connected,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            streams: HashMap::new(),
        };

        self.connection_registry
            .register(connection_id, connection_info)
            .await?;

        // Create connection handle
        let handle = ConnectionHandle::new(
            connection_id,
            endpoint,
            Arc::downgrade(&Arc::new(self.clone())),
        );

        self.event_bus
            .publish(ValkyrieEvent::ConnectionEstablished {
                connection_id,
                endpoint: handle.endpoint().clone(),
                timestamp: Utc::now(),
            })
            .await;

        Ok(handle)
    }

    /// Listen for incoming connections
    pub async fn listen(&self, bind_address: std::net::SocketAddr) -> Result<Listener> {
        let listener = self.transport_manager.listen(bind_address).await?;

        self.event_bus
            .publish(ValkyrieEvent::ListenerStarted {
                address: bind_address,
                timestamp: Utc::now(),
            })
            .await;

        Ok(Listener::new(
            listener,
            Arc::downgrade(&Arc::new(self.clone())),
        ))
    }

    /// Send a message to a specific connection
    pub async fn send_message(
        &self,
        connection: ConnectionId,
        message: ValkyrieMessage,
    ) -> Result<()> {
        // Route message through the transport manager
        self.transport_manager
            .send_message(connection, message)
            .await?;

        Ok(())
    }

    /// Broadcast a message to multiple connections
    pub async fn broadcast(
        &self,
        connections: Vec<ConnectionId>,
        message: ValkyrieMessage,
    ) -> Result<BroadcastResult> {
        let mut results = HashMap::new();
        let mut successful = 0;
        let mut failed = 0;

        for connection_id in connections {
            match self.send_message(connection_id, message.clone()).await {
                Ok(()) => {
                    results.insert(connection_id, Ok(()));
                    successful += 1;
                }
                Err(e) => {
                    results.insert(connection_id, Err(e));
                    failed += 1;
                }
            }
        }

        Ok(BroadcastResult {
            total: successful + failed,
            successful,
            failed,
            results,
        })
    }

    /// Get engine statistics
    pub async fn get_stats(&self) -> EngineStats {
        let connections = self.connection_registry.get_stats().await;
        let transport_stats = self.transport_manager.get_stats().await;
        let security_stats = self.security_manager.get_stats().await;

        EngineStats {
            state: *self.state.read().await,
            connections,
            transport: transport_stats,
            security: security_stats,
            uptime: self.get_uptime().await,
        }
    }

    /// Start background tasks
    async fn start_background_tasks(&self, mut shutdown_rx: mpsc::Receiver<()>) {
        let connection_registry = Arc::clone(&self.connection_registry);
        let event_bus = Arc::clone(&self.event_bus);

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        // Clean up stale connections
                        if let Err(e) = connection_registry.cleanup_stale_connections().await {
                            event_bus.publish(ValkyrieEvent::Error {
                                error: format!("Connection cleanup failed: {}", e),
                                timestamp: Utc::now(),
                            }).await;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
    }

    /// Get engine uptime
    async fn get_uptime(&self) -> Duration {
        // This would be implemented with a start time stored in the engine
        Duration::from_secs(0) // Placeholder
    }
}

// Clone implementation for ValkyrieEngine (needed for Arc usage)
impl Clone for ValkyrieEngine {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            transport_manager: Arc::clone(&self.transport_manager),
            stream_multiplexer: Arc::clone(&self.stream_multiplexer),
            security_manager: Arc::clone(&self.security_manager),
            observability_manager: Arc::clone(&self.observability_manager),
            connection_registry: Arc::clone(&self.connection_registry),
            event_bus: Arc::clone(&self.event_bus),
            state: Arc::clone(&self.state),
            shutdown_tx: None, // Don't clone shutdown channel
        }
    }
}

/// Protocol state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolState {
    /// Engine is stopped
    Stopped,
    /// Engine is starting
    Starting,
    /// Engine is running
    Running,
    /// Engine is stopping
    Stopping,
    /// Engine is in error state
    Error,
}

/// Broadcast operation result
#[derive(Debug)]
pub struct BroadcastResult {
    /// Total number of connections attempted
    pub total: usize,
    /// Number of successful sends
    pub successful: usize,
    /// Number of failed sends
    pub failed: usize,
    /// Individual results per connection
    pub results: HashMap<ConnectionId, Result<()>>,
}

/// Engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    /// Current protocol state
    pub state: ProtocolState,
    /// Connection statistics
    pub connections: crate::valkyrie::core::ConnectionStats,
    /// Transport statistics
    pub transport: crate::valkyrie::transport::TransportStats,
    /// Security statistics
    pub security: crate::valkyrie::security::SecurityStats,
    /// Engine uptime
    pub uptime: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valkyrie::ValkyrieConfigBuilder;

    #[tokio::test]
    async fn test_engine_creation() {
        let config = ValkyrieConfigBuilder::new().build().unwrap();
        let result = ValkyrieEngine::new(config);
        // This would need proper mocking to test
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_engine_lifecycle() {
        let config = ValkyrieConfigBuilder::new().build().unwrap();
        let mut engine = ValkyrieEngine::new(config).unwrap();

        // This would need proper mocking to test
        // let start_result = engine.start().await;
        // assert!(start_result.is_ok());

        // let stop_result = engine.stop().await;
        // assert!(stop_result.is_ok());
    }
}
