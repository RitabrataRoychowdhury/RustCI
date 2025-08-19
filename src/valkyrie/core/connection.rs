//! Valkyrie Protocol Connection Management
//!
//! This module provides connection management components including
//! connection handles, registries, and listeners.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::valkyrie::core::{StreamId, ValkyrieMessage};
use crate::valkyrie::transport::Endpoint;
use crate::valkyrie::{Result, ValkyrieEngine};

/// Connection identifier type
pub type ConnectionId = Uuid;

/// Connection handle for client usage
#[derive(Debug)]
pub struct ConnectionHandle {
    /// Connection ID
    id: ConnectionId,
    /// Remote endpoint
    endpoint: Endpoint,
    /// Weak reference to engine
    engine: Weak<ValkyrieEngine>,
}

impl ConnectionHandle {
    /// Create a new connection handle
    pub fn new(id: ConnectionId, endpoint: Endpoint, engine: Weak<ValkyrieEngine>) -> Self {
        Self {
            id,
            endpoint,
            engine,
        }
    }

    /// Get connection ID
    pub fn id(&self) -> ConnectionId {
        self.id
    }

    /// Get endpoint
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Send a message through this connection
    pub async fn send(&self, message: ValkyrieMessage) -> Result<()> {
        if let Some(engine) = self.engine.upgrade() {
            engine.send_message(self.id, message).await
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                "Engine has been dropped".to_string(),
            ))
        }
    }

    /// Close this connection
    pub async fn close(&self) -> Result<()> {
        if let Some(_engine) = self.engine.upgrade() {
            // This would need to be implemented in the engine
            // engine.connection_registry.close_connection(self.id).await
            Ok(())
        } else {
            Ok(()) // Engine already dropped, connection is effectively closed
        }
    }
}

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Connection ID
    pub id: ConnectionId,
    /// Remote endpoint
    pub endpoint: Endpoint,
    /// Connection state
    pub state: ConnectionState,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Active streams
    pub streams: HashMap<StreamId, StreamInfo>,
}

/// Connection state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection is active
    Connected,
    /// Connection is being closed
    Closing,
    /// Connection is closed
    Closed,
    /// Connection is in error state
    Error(String),
}

/// Stream information
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Stream ID
    pub id: StreamId,
    /// Stream state
    pub state: StreamState,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
}

/// Stream state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is active
    Active,
    /// Stream is paused
    Paused,
    /// Stream is closed
    Closed,
}

/// Connection registry for managing active connections
pub struct ConnectionRegistry {
    /// Active connections
    connections: Arc<RwLock<HashMap<ConnectionId, ConnectionInfo>>>,
}

impl ConnectionRegistry {
    /// Create a new connection registry
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new connection
    pub async fn register(&self, id: ConnectionId, info: ConnectionInfo) -> Result<()> {
        let mut connections = self.connections.write().await;
        connections.insert(id, info);
        Ok(())
    }

    /// Unregister a connection
    pub async fn unregister(&self, id: ConnectionId) -> Result<()> {
        let mut connections = self.connections.write().await;
        connections.remove(&id);
        Ok(())
    }

    /// Get connection information
    pub async fn get(&self, id: ConnectionId) -> Option<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.get(&id).cloned()
    }

    /// Close all connections
    pub async fn close_all(&self) -> Result<()> {
        let mut connections = self.connections.write().await;
        connections.clear();
        Ok(())
    }

    /// Clean up stale connections
    pub async fn cleanup_stale_connections(&self) -> Result<()> {
        let mut connections = self.connections.write().await;
        let now = Utc::now();
        let stale_timeout = chrono::Duration::minutes(5);

        connections.retain(|_, info| now.signed_duration_since(info.last_activity) < stale_timeout);

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let total_connections = connections.len();
        let active_connections = connections
            .values()
            .filter(|info| info.state == ConnectionState::Connected)
            .count();

        ConnectionStats {
            total_connections,
            active_connections,
            connecting_connections: connections
                .values()
                .filter(|info| info.state == ConnectionState::Connecting)
                .count(),
            closing_connections: connections
                .values()
                .filter(|info| info.state == ConnectionState::Closing)
                .count(),
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// Total number of connections
    pub total_connections: usize,
    /// Number of active connections
    pub active_connections: usize,
    /// Number of connecting connections
    pub connecting_connections: usize,
    /// Number of closing connections
    pub closing_connections: usize,
}

/// Listener for incoming connections
pub struct Listener {
    /// Inner transport listener
    inner: Box<dyn crate::valkyrie::transport::Listener>,
    /// Weak reference to engine
    engine: Weak<ValkyrieEngine>,
}

impl Listener {
    /// Create a new listener
    pub fn new(
        inner: Box<dyn crate::valkyrie::transport::Listener>,
        engine: Weak<ValkyrieEngine>,
    ) -> Self {
        Self { inner, engine }
    }

    /// Accept incoming connections
    pub async fn accept(&mut self) -> Result<ConnectionHandle> {
        let _connection = self.inner.accept().await?;

        if let Some(_engine) = self.engine.upgrade() {
            // Process the incoming connection through the engine
            let connection_id = ConnectionId::new_v4();

            // This would involve authentication, registration, etc.
            // For now, create a basic handle
            let handle = ConnectionHandle::new(
                connection_id,
                Endpoint::tcp("unknown", 0),
                self.engine.clone(),
            );

            Ok(handle)
        } else {
            Err(crate::valkyrie::ValkyrieError::InternalServerError(
                "Engine has been dropped".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_registry() {
        let registry = ConnectionRegistry::new();
        let connection_id = ConnectionId::new_v4();

        let info = ConnectionInfo {
            id: connection_id,
            endpoint: Endpoint::tcp("localhost", 8080),
            state: ConnectionState::Connected,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            streams: HashMap::new(),
        };

        registry
            .register(connection_id, info.clone())
            .await
            .unwrap();

        let retrieved = registry.get(connection_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, connection_id);

        let stats = registry.get_stats().await;
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.active_connections, 1);
    }

    #[test]
    fn test_connection_state() {
        let state = ConnectionState::Connected;
        assert_eq!(state, ConnectionState::Connected);

        let error_state = ConnectionState::Error("Test error".to_string());
        assert!(matches!(error_state, ConnectionState::Error(_)));
    }
}
