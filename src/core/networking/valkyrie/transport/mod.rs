use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::networking::transport::{
    NetworkConditions, Transport, TransportEndpoint, TransportType,
};
use crate::error::Result;

// Import canonical types from central types module
use super::types::{CompressionAlgorithm, Duration};

// Transport implementations
pub mod quic;
pub mod selector;
pub mod tcp;
pub mod unix_socket;
pub mod websocket;

// Re-export transport implementations
pub use quic::QuicTransport;
pub use selector::{SelectionState, TransportHealth, TransportSelector};
pub use tcp::TcpTransport;
pub use unix_socket::UnixSocketTransport;
pub use websocket::WebSocketTransport;

// TransportCapabilities, CompressionAlgorithm, and EncryptionCipher definitions
// removed - now using canonical types from types module

/// Transport selection strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportSelectionStrategy {
    /// Always use the first available transport
    FirstAvailable,
    /// Select based on latency requirements
    LatencyOptimized,
    /// Select based on throughput requirements
    ThroughputOptimized,
    /// Select based on reliability requirements
    ReliabilityOptimized,
    /// Use round-robin selection
    RoundRobin,
    /// Use weighted selection
    Weighted(HashMap<TransportType, f64>),
    /// Custom selection logic
    Custom(String),
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPool {
    /// Maximum connections per endpoint
    pub max_connections_per_endpoint: usize,
    /// Minimum idle connections to maintain
    pub min_idle_connections: usize,
    /// Maximum idle time before closing connection
    pub max_idle_time: Duration,
    /// Connection validation interval
    pub validation_interval: Duration,
    /// Enable connection health checks
    pub enable_health_checks: bool,
}

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// Enable automatic failover
    pub enabled: bool,
    /// Maximum failover attempts
    pub max_attempts: u32,
    /// Failover timeout
    pub timeout: Duration,
    /// Backoff strategy
    pub backoff_strategy: BackoffStrategy,
    /// Health check configuration
    pub health_check: HealthCheckConfig,
}

/// Backoff strategy for failover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between attempts
    Fixed(Duration),
    /// Exponential backoff
    Exponential {
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
    },
    /// Linear backoff
    Linear {
        initial_delay: Duration,
        increment: Duration,
    },
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Enable health checks
    pub enabled: bool,
    /// Health check interval
    pub interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
}

// Use the canonical LoadBalancingStrategy from types
pub use crate::core::networking::valkyrie::types::LoadBalancingStrategy;

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable compression
    pub enabled: bool,
    /// Compression algorithm
    pub algorithm: CompressionAlgorithm,
    /// Compression level (0-9)
    pub level: u8,
    /// Minimum message size to compress
    pub min_size: usize,
    /// Dictionary for compression
    pub dictionary: Option<Vec<u8>>,
}

/// Transport manager for handling multiple transports
pub struct TransportManager {
    /// Available transports
    transports: HashMap<TransportType, Arc<dyn Transport>>,
    /// Transport selection strategy
    selection_strategy: TransportSelectionStrategy,
    /// Connection pool
    connection_pool: Arc<ConnectionPool>,
    /// Failover configuration
    failover_config: FailoverConfig,
    /// Transport metrics
    metrics: Arc<
        tokio::sync::RwLock<
            HashMap<TransportType, crate::core::networking::transport::TransportMetrics>,
        >,
    >,
}

impl TransportManager {
    /// Create a new transport manager
    pub fn new(
        selection_strategy: TransportSelectionStrategy,
        connection_pool: ConnectionPool,
        failover_config: FailoverConfig,
    ) -> Self {
        Self {
            transports: HashMap::new(),
            selection_strategy,
            connection_pool: Arc::new(connection_pool),
            failover_config,
            metrics: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Register a transport
    pub fn register_transport(&mut self, transport: Arc<dyn Transport>) {
        let transport_type = transport.transport_type();
        self.transports.insert(transport_type, transport);
    }

    /// Select optimal transport for endpoint
    pub async fn select_transport(
        &self,
        endpoint: &TransportEndpoint,
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        match &self.selection_strategy {
            TransportSelectionStrategy::FirstAvailable => self.select_first_available(endpoint),
            TransportSelectionStrategy::LatencyOptimized => {
                self.select_latency_optimized(endpoint, conditions).await
            }
            TransportSelectionStrategy::ThroughputOptimized => {
                self.select_throughput_optimized(endpoint, conditions).await
            }
            TransportSelectionStrategy::ReliabilityOptimized => {
                self.select_reliability_optimized(endpoint, conditions)
                    .await
            }
            TransportSelectionStrategy::RoundRobin => self.select_round_robin(endpoint),
            TransportSelectionStrategy::Weighted(weights) => {
                self.select_weighted(endpoint, weights)
            }
            TransportSelectionStrategy::Custom(_) => {
                // Custom selection logic would be implemented here
                self.select_first_available(endpoint)
            }
        }
    }

    /// Get all available transports
    pub fn get_available_transports(&self) -> Vec<TransportType> {
        self.transports.keys().cloned().collect()
    }

    /// Get transport by type
    pub fn get_transport(&self, transport_type: &TransportType) -> Option<Arc<dyn Transport>> {
        self.transports.get(transport_type).cloned()
    }

    /// Update transport metrics
    pub async fn update_metrics(
        &self,
        transport_type: TransportType,
        metrics: crate::core::networking::transport::TransportMetrics,
    ) {
        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(transport_type, metrics);
    }

    /// Get aggregated metrics
    pub async fn get_aggregated_metrics(
        &self,
    ) -> HashMap<TransportType, crate::core::networking::transport::TransportMetrics> {
        self.metrics.read().await.clone()
    }

    // Private helper methods
    fn select_first_available(&self, endpoint: &TransportEndpoint) -> Result<Arc<dyn Transport>> {
        for transport in self.transports.values() {
            if transport.supports_endpoint(endpoint) {
                return Ok(transport.clone());
            }
        }
        Err(
            crate::core::networking::node_communication::ProtocolError::ConnectionError {
                message: "No suitable transport found".to_string(),
            }
            .into(),
        )
    }

    async fn select_latency_optimized(
        &self,
        endpoint: &TransportEndpoint,
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        // For low latency, prefer QUIC or TCP
        let preferred_types = if conditions.latency_ms < 10.0 {
            vec![TransportType::Quic, TransportType::Tcp]
        } else {
            vec![TransportType::Tcp, TransportType::Quic]
        };

        for transport_type in preferred_types {
            if let Some(transport) = self.transports.get(&transport_type) {
                if transport.supports_endpoint(endpoint) {
                    return Ok(transport.clone());
                }
            }
        }

        self.select_first_available(endpoint)
    }

    async fn select_throughput_optimized(
        &self,
        endpoint: &TransportEndpoint,
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        // For high throughput, prefer transports with multiplexing
        let preferred_types = if conditions.bandwidth_mbps > 100.0 {
            vec![TransportType::Quic, TransportType::SecureTcp]
        } else {
            vec![TransportType::Tcp, TransportType::Quic]
        };

        for transport_type in preferred_types {
            if let Some(transport) = self.transports.get(&transport_type) {
                if transport.supports_endpoint(endpoint) {
                    return Ok(transport.clone());
                }
            }
        }

        self.select_first_available(endpoint)
    }

    async fn select_reliability_optimized(
        &self,
        endpoint: &TransportEndpoint,
        conditions: &NetworkConditions,
    ) -> Result<Arc<dyn Transport>> {
        // For reliability, prefer TCP-based transports
        let preferred_types = if conditions.packet_loss_rate > 0.01 {
            vec![TransportType::SecureTcp, TransportType::Tcp]
        } else {
            vec![TransportType::Quic, TransportType::SecureTcp]
        };

        for transport_type in preferred_types {
            if let Some(transport) = self.transports.get(&transport_type) {
                if transport.supports_endpoint(endpoint) {
                    return Ok(transport.clone());
                }
            }
        }

        self.select_first_available(endpoint)
    }

    fn select_round_robin(&self, endpoint: &TransportEndpoint) -> Result<Arc<dyn Transport>> {
        // Simple round-robin implementation
        // In a real implementation, this would maintain state
        self.select_first_available(endpoint)
    }

    fn select_weighted(
        &self,
        endpoint: &TransportEndpoint,
        _weights: &HashMap<TransportType, f64>,
    ) -> Result<Arc<dyn Transport>> {
        // Weighted selection implementation
        // In a real implementation, this would use the weights
        self.select_first_available(endpoint)
    }
}

// Default implementation moved to types.rs to avoid conflicts

impl Default for ConnectionPool {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 10,
            min_idle_connections: 2,
            max_idle_time: Duration::from_secs(300), // 5 minutes
            validation_interval: Duration::from_secs(30),
            enable_health_checks: true,
        }
    }
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            timeout: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Exponential {
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(30),
                multiplier: 2.0,
            },
            health_check: HealthCheckConfig::default(),
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            success_threshold: 2,
        }
    }
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            algorithm: CompressionAlgorithm::None,
            level: 6,
            min_size: 1024, // 1KB
            dictionary: None,
        }
    }
}
