//! High-level client for Valkyrie Protocol

use crate::{Result, ValkyrieError, Message, pool::ConnectionPool, retry::RetryPolicy};
use valkyrie_protocol::{ValkyrieClient, client::ClientConfig as ProtocolClientConfig};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use serde::{Serialize, de::DeserializeOwned};

/// High-level client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Server endpoint
    pub endpoint: String,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Maximum number of connections in pool
    pub max_connections: usize,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Retry policy
    pub retry_policy: RetryPolicy,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "tcp://localhost:8080".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_connections: 10,
            enable_pooling: true,
            retry_policy: RetryPolicy::default(),
            enable_metrics: false,
        }
    }
}

/// High-level Valkyrie client
pub struct Client {
    config: ClientConfig,
    pool: Option<Arc<ConnectionPool>>,
    client: Option<Arc<ValkyrieClient>>,
}

impl Client {
    /// Create a new client with configuration
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let mut client = Self {
            config: config.clone(),
            pool: None,
            client: None,
        };
        
        if config.enable_pooling {
            let pool = ConnectionPool::new(
                config.endpoint.clone(),
                config.max_connections,
                Duration::from_millis(config.connect_timeout_ms),
            ).await?;
            client.pool = Some(Arc::new(pool));
        } else {
            let protocol_config = ProtocolClientConfig {
                connect_timeout_ms: config.connect_timeout_ms,
                request_timeout_ms: config.request_timeout_ms,
                max_concurrent_requests: 100,
                auto_reconnect: true,
                reconnect_interval_ms: 1000,
            };
            
            let valkyrie_client = ValkyrieClient::connect_with_config(&config.endpoint, protocol_config).await?;
            client.client = Some(Arc::new(valkyrie_client));
        }
        
        Ok(client)
    }
    
    /// Send a request and wait for response
    pub async fn request<T, R>(&self, data: T) -> Result<R>
    where
        T: Serialize + Send + Sync,
        R: DeserializeOwned + Send + Sync,
    {
        let payload = serde_json::to_vec(&data)
            .map_err(|e| ValkyrieError::protocol(format!("Serialization failed: {}", e)))?;
        
        let message = Message::request(payload);
        
        let response = if let Some(pool) = &self.pool {
            pool.request(message).await?
        } else if let Some(client) = &self.client {
            client.request(message).await?
        } else {
            return Err(ValkyrieError::protocol("No client or pool available"));
        };
        
        let result: R = serde_json::from_slice(&response.payload)
            .map_err(|e| ValkyrieError::protocol(format!("Deserialization failed: {}", e)))?;
        
        Ok(result)
    }
    
    /// Send a request with string payload
    pub async fn request_string<S: AsRef<str>>(&self, data: S) -> Result<String> {
        let message = Message::request(data.as_ref().as_bytes());
        
        let response = if let Some(pool) = &self.pool {
            pool.request(message).await?
        } else if let Some(client) = &self.client {
            client.request(message).await?
        } else {
            return Err(ValkyrieError::protocol("No client or pool available"));
        };
        
        response.payload_as_string()
            .ok_or_else(|| ValkyrieError::protocol("Invalid UTF-8 in response"))
    }
    
    /// Send a notification (no response expected)
    pub async fn notify<T>(&self, data: T) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let payload = serde_json::to_vec(&data)
            .map_err(|e| ValkyrieError::protocol(format!("Serialization failed: {}", e)))?;
        
        let message = Message::notification(payload);
        
        if let Some(pool) = &self.pool {
            pool.notify(message).await
        } else if let Some(client) = &self.client {
            client.notify(message).await
        } else {
            Err(ValkyrieError::protocol("No client or pool available"))
        }
    }
    
    /// Send a notification with string payload
    pub async fn notify_string<S: AsRef<str>>(&self, data: S) -> Result<()> {
        let message = Message::notification(data.as_ref().as_bytes());
        
        if let Some(pool) = &self.pool {
            pool.notify(message).await
        } else if let Some(client) = &self.client {
            client.notify(message).await
        } else {
            Err(ValkyrieError::protocol("No client or pool available"))
        }
    }
    
    /// Check if client is connected
    pub async fn is_connected(&self) -> bool {
        if let Some(pool) = &self.pool {
            pool.is_healthy().await
        } else if let Some(client) = &self.client {
            client.is_connected().await
        } else {
            false
        }
    }
    
    /// Disconnect the client
    pub async fn disconnect(&self) -> Result<()> {
        if let Some(pool) = &self.pool {
            pool.close().await
        } else if let Some(client) = &self.client {
            client.disconnect().await
        } else {
            Ok(())
        }
    }
    
    /// Get client configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
}

/// Builder for creating Valkyrie clients
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// Create a new client builder
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }
    
    /// Set the server endpoint
    pub fn endpoint<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }
    
    /// Set connection timeout in milliseconds
    pub fn connect_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.config.connect_timeout_ms = timeout_ms;
        self
    }
    
    /// Set request timeout in milliseconds
    pub fn request_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.config.request_timeout_ms = timeout_ms;
        self
    }
    
    /// Set maximum number of connections in pool
    pub fn max_connections(mut self, max: usize) -> Self {
        self.config.max_connections = max;
        self
    }
    
    /// Enable or disable connection pooling
    pub fn enable_pooling(mut self, enable: bool) -> Self {
        self.config.enable_pooling = enable;
        self
    }
    
    /// Set retry policy
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.config.retry_policy = policy;
        self
    }
    
    /// Enable metrics collection
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }
    
    /// Build the client
    pub async fn build(self) -> Result<Client> {
        info!("Building Valkyrie client for endpoint: {}", self.config.endpoint);
        Client::new(self.config).await
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}