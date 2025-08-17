//! Connection pooling for Valkyrie clients

use crate::{Result, ValkyrieError, Message};
use valkyrie_protocol::{ValkyrieClient, client::ClientConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Semaphore, RwLock};
use tracing::{debug, warn, error};
use std::collections::VecDeque;

/// Connection pool for managing multiple client connections
pub struct ConnectionPool {
    endpoint: String,
    max_connections: usize,
    connect_timeout: Duration,
    connections: Arc<RwLock<VecDeque<Arc<ValkyrieClient>>>>,
    semaphore: Arc<Semaphore>,
    healthy: Arc<RwLock<bool>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub async fn new(endpoint: String, max_connections: usize, connect_timeout: Duration) -> Result<Self> {
        let pool = Self {
            endpoint,
            max_connections,
            connect_timeout,
            connections: Arc::new(RwLock::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_connections)),
            healthy: Arc::new(RwLock::new(true)),
        };
        
        // Pre-populate with one connection to test connectivity
        pool.create_connection().await?;
        
        Ok(pool)
    }
    
    /// Get a connection from the pool
    async fn get_connection(&self) -> Result<Arc<ValkyrieClient>> {
        // Acquire semaphore permit
        let _permit = self.semaphore.acquire().await
            .map_err(|_| ValkyrieError::connection("Failed to acquire connection permit"))?;
        
        // Try to get an existing connection
        {
            let mut connections = self.connections.write().await;
            if let Some(client) = connections.pop_front() {
                if client.is_connected().await {
                    return Ok(client);
                }
                // Connection is dead, continue to create a new one
            }
        }
        
        // Create a new connection
        self.create_connection().await
    }
    
    /// Return a connection to the pool
    async fn return_connection(&self, client: Arc<ValkyrieClient>) {
        if client.is_connected().await {
            let mut connections = self.connections.write().await;
            if connections.len() < self.max_connections {
                connections.push_back(client);
            }
            // If pool is full, just drop the connection
        }
    }
    
    /// Create a new connection
    async fn create_connection(&self) -> Result<Arc<ValkyrieClient>> {
        debug!("Creating new connection to {}", self.endpoint);
        
        let config = ClientConfig {
            connect_timeout_ms: self.connect_timeout.as_millis() as u64,
            request_timeout_ms: 30000,
            max_concurrent_requests: 100,
            auto_reconnect: true,
            reconnect_interval_ms: 1000,
        };
        
        match ValkyrieClient::connect_with_config(&self.endpoint, config).await {
            Ok(client) => {
                let client = Arc::new(client);
                
                // Mark pool as healthy
                {
                    let mut healthy = self.healthy.write().await;
                    *healthy = true;
                }
                
                Ok(client)
            }
            Err(e) => {
                error!("Failed to create connection to {}: {}", self.endpoint, e);
                
                // Mark pool as unhealthy
                {
                    let mut healthy = self.healthy.write().await;
                    *healthy = false;
                }
                
                Err(e)
            }
        }
    }
    
    /// Send a request using a pooled connection
    pub async fn request(&self, message: Message) -> Result<Message> {
        let client = self.get_connection().await?;
        
        match client.request(message).await {
            Ok(response) => {
                self.return_connection(client).await;
                Ok(response)
            }
            Err(e) => {
                // Don't return a failed connection to the pool
                warn!("Request failed, not returning connection to pool: {}", e);
                Err(e)
            }
        }
    }
    
    /// Send a notification using a pooled connection
    pub async fn notify(&self, message: Message) -> Result<()> {
        let client = self.get_connection().await?;
        
        match client.notify(message).await {
            Ok(()) => {
                self.return_connection(client).await;
                Ok(())
            }
            Err(e) => {
                // Don't return a failed connection to the pool
                warn!("Notification failed, not returning connection to pool: {}", e);
                Err(e)
            }
        }
    }
    
    /// Check if the pool is healthy
    pub async fn is_healthy(&self) -> bool {
        *self.healthy.read().await
    }
    
    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let connections = self.connections.read().await;
        let available_permits = self.semaphore.available_permits();
        
        PoolStats {
            max_connections: self.max_connections,
            available_connections: connections.len(),
            active_connections: self.max_connections - available_permits,
            is_healthy: self.is_healthy().await,
        }
    }
    
    /// Close all connections in the pool
    pub async fn close(&self) -> Result<()> {
        debug!("Closing connection pool for {}", self.endpoint);
        
        let mut connections = self.connections.write().await;
        
        while let Some(client) = connections.pop_front() {
            if let Err(e) = client.disconnect().await {
                warn!("Failed to disconnect client: {}", e);
            }
        }
        
        // Mark as unhealthy
        {
            let mut healthy = self.healthy.write().await;
            *healthy = false;
        }
        
        Ok(())
    }
    
    /// Perform health check on all connections
    pub async fn health_check(&self) -> Result<()> {
        debug!("Performing health check on connection pool");
        
        let mut connections = self.connections.write().await;
        let mut healthy_connections = VecDeque::new();
        
        while let Some(client) = connections.pop_front() {
            if client.is_connected().await {
                healthy_connections.push_back(client);
            } else {
                debug!("Removing unhealthy connection from pool");
            }
        }
        
        *connections = healthy_connections;
        
        // Update pool health status
        let is_healthy = !connections.is_empty() || self.create_connection().await.is_ok();
        {
            let mut healthy = self.healthy.write().await;
            *healthy = is_healthy;
        }
        
        Ok(())
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Number of available connections in pool
    pub available_connections: usize,
    /// Number of active connections in use
    pub active_connections: usize,
    /// Whether the pool is healthy
    pub is_healthy: bool,
}