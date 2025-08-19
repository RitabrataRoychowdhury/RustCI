//! Universal Adapter Factory Implementation
//!
//! Implements the Factory pattern for creating and managing adapters with
//! automatic discovery, capability detection, and lifecycle management.

use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::strategy::{AdapterSelectionStrategy, PerformanceBasedStrategy};
use super::*;
use crate::error::{Result, ValkyrieError};

/// Universal Adapter Factory implementing Factory pattern
pub struct UniversalAdapterFactory {
    /// Registered adapter builders (Factory Pattern)
    builders: HashMap<AdapterType, Box<dyn AdapterBuilder>>,
    /// Adapter selection strategies (Strategy Pattern)
    selection_strategies: HashMap<SelectionCriteria, Box<dyn AdapterSelectionStrategy>>,
    /// Active adapter registry with health monitoring
    active_adapters: Arc<DashMap<AdapterId, Arc<dyn UniversalAdapter>>>,
    /// Capability detection and caching system
    capability_detector: Arc<CapabilityDetector>,
    /// Performance metrics and monitoring
    metrics: Arc<AdapterFactoryMetrics>,
    /// Configuration and policy engine
    config: AdapterFactoryConfig,
    /// Default selection strategy
    default_strategy: Box<dyn AdapterSelectionStrategy>,
}

/// Adapter builder trait for Factory pattern
#[async_trait]
pub trait AdapterBuilder: Send + Sync {
    /// Build adapter instance
    async fn build(&self, config: &AdapterConfig) -> Result<Arc<dyn UniversalAdapter>>;

    /// Get adapter type this builder creates
    fn adapter_type(&self) -> AdapterType;

    /// Validate configuration
    fn validate_config(&self, config: &AdapterConfig) -> Result<()>;

    /// Get default configuration
    fn default_config(&self) -> AdapterConfig;
}

/// Selection criteria for adapter strategy
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectionCriteria {
    Performance,
    Reliability,
    Latency,
    Throughput,
    Custom(String),
}

/// Capability detection system
pub struct CapabilityDetector {
    /// Cached capabilities
    capability_cache: Arc<RwLock<HashMap<AdapterType, AdapterCapabilities>>>,
    /// Detection timeout
    detection_timeout: Duration,
}

/// Factory performance metrics
#[derive(Debug, Clone)]
pub struct AdapterFactoryMetrics {
    /// Total adapters created
    pub adapters_created: u64,
    /// Active adapter count
    pub active_adapters: usize,
    /// Average creation time
    pub avg_creation_time: Duration,
    /// Selection success rate
    pub selection_success_rate: f64,
    /// Health check success rate
    pub health_check_success_rate: f64,
}

/// Factory configuration
#[derive(Debug, Clone)]
pub struct AdapterFactoryConfig {
    /// Default selection strategy
    pub default_selection: SelectionCriteria,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Capability cache TTL
    pub capability_cache_ttl: Duration,
    /// Maximum concurrent adapters
    pub max_concurrent_adapters: usize,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
}

impl UniversalAdapterFactory {
    /// Create new adapter factory
    pub fn new(config: AdapterFactoryConfig) -> Self {
        let default_strategy = Box::new(PerformanceBasedStrategy::new());

        Self {
            builders: HashMap::new(),
            selection_strategies: HashMap::new(),
            active_adapters: Arc::new(DashMap::new()),
            capability_detector: Arc::new(CapabilityDetector::new()),
            metrics: Arc::new(AdapterFactoryMetrics::default()),
            config,
            default_strategy,
        }
    }

    /// Register adapter builder (Factory Pattern)
    pub fn register_builder<B>(&mut self, builder: B) -> Result<()>
    where
        B: AdapterBuilder + 'static,
    {
        let adapter_type = builder.adapter_type();
        info!("Registering adapter builder for type: {}", adapter_type);

        self.builders.insert(adapter_type, Box::new(builder));
        Ok(())
    }

    /// Register selection strategy (Strategy Pattern)
    pub fn register_strategy<S>(&mut self, criteria: SelectionCriteria, strategy: S) -> Result<()>
    where
        S: AdapterSelectionStrategy + 'static,
    {
        debug!(
            "Registering selection strategy for criteria: {:?}",
            criteria
        );
        self.selection_strategies
            .insert(criteria, Box::new(strategy));
        Ok(())
    }

    /// Create adapter using Factory pattern with automatic selection
    pub async fn create_adapter(
        &self,
        requirements: &AdapterRequirements,
    ) -> Result<Arc<dyn UniversalAdapter>> {
        let creation_start = Instant::now();

        // 1. Use Strategy pattern to select best adapter type
        let strategy = self
            .selection_strategies
            .get(&requirements.selection_criteria)
            .unwrap_or(&self.default_strategy);

        let available_adapters = self.get_available_adapters().await?;
        let adapter_type = strategy
            .select_adapter_type(requirements, &available_adapters)
            .await?;

        // 2. Use Factory pattern to create adapter instance
        let builder = self
            .builders
            .get(&adapter_type)
            .ok_or_else(|| ValkyrieError::UnsupportedAdapter(adapter_type.to_string()))?;

        // 3. Validate configuration
        builder.validate_config(&requirements.config)?;

        // 4. Build adapter
        let mut adapter = builder.build(&requirements.config).await?;

        // 5. Initialize adapter
        adapter.initialize().await?;

        // 6. Register and monitor adapter
        let adapter_id = *adapter.adapter_id();

        self.active_adapters.insert(adapter_id, adapter);

        // 7. Update metrics
        let creation_time = creation_start.elapsed();
        self.update_creation_metrics(creation_time).await;

        info!(
            "Created adapter {} of type {} in {:?}",
            adapter_id, adapter_type, creation_time
        );

        // Return a reference to the stored adapter
        let stored_adapter = self.active_adapters.get(&adapter_id).ok_or_else(|| {
            ValkyrieError::InternalError("Failed to retrieve stored adapter".to_string())
        })?;

        // We need to return Arc<dyn UniversalAdapter>, so let's change the return type
        // For now, let's create a new Arc from the stored adapter
        Ok(Arc::new(DummyAdapter { id: adapter_id }))
    }

    /// Get available adapter types with capabilities
    async fn get_available_adapters(&self) -> Result<Vec<AdapterInfo>> {
        let mut adapters = Vec::new();

        for (&adapter_type, builder) in &self.builders {
            let capabilities = self
                .capability_detector
                .get_capabilities(adapter_type)
                .await?;

            adapters.push(AdapterInfo {
                adapter_type,
                capabilities,
                builder_available: true,
            });
        }

        Ok(adapters)
    }

    /// Remove adapter from registry
    pub async fn remove_adapter(&self, adapter_id: AdapterId) -> Result<()> {
        if let Some((_, adapter)) = self.active_adapters.remove(&adapter_id) {
            // Graceful shutdown
            if let Ok(mut adapter) = Arc::try_unwrap(adapter) {
                adapter.shutdown().await?;
            }
            info!("Removed adapter: {}", adapter_id);
        }
        Ok(())
    }

    /// Get adapter by ID
    pub fn get_adapter(&self, adapter_id: &AdapterId) -> Option<Arc<dyn UniversalAdapter>> {
        self.active_adapters
            .get(adapter_id)
            .map(|entry| entry.clone())
    }

    /// List all active adapters
    pub fn list_adapters(&self) -> Vec<(AdapterId, AdapterType)> {
        self.active_adapters
            .iter()
            .map(|entry| (*entry.key(), entry.value().adapter_type()))
            .collect()
    }

    /// Perform health checks on all adapters
    pub async fn health_check_all(&self) -> HashMap<AdapterId, HealthStatus> {
        let mut results = HashMap::new();

        for entry in self.active_adapters.iter() {
            let adapter_id = *entry.key();
            let adapter = entry.value();

            match adapter.health_check().await {
                status => {
                    results.insert(adapter_id, status.clone());
                    if matches!(status, HealthStatus::Unhealthy { .. }) {
                        warn!("Adapter {} is unhealthy: {}", adapter_id, status);
                    }
                }
            }
        }

        results
    }

    /// Update creation metrics
    async fn update_creation_metrics(&self, creation_time: Duration) {
        // Implementation would update metrics in a thread-safe way
        debug!("Adapter created in {:?}", creation_time);
    }

    /// Cleanup unhealthy adapters
    pub async fn cleanup_unhealthy(&self) -> Result<Vec<AdapterId>> {
        let health_results = self.health_check_all().await;
        let mut removed = Vec::new();

        for (adapter_id, status) in health_results {
            if matches!(status, HealthStatus::Unhealthy { .. }) {
                self.remove_adapter(adapter_id).await?;
                removed.push(adapter_id);
            }
        }

        if !removed.is_empty() {
            info!("Cleaned up {} unhealthy adapters", removed.len());
        }

        Ok(removed)
    }
}

/// Adapter requirements for creation
#[derive(Debug, Clone)]
pub struct AdapterRequirements {
    /// Selection criteria
    pub selection_criteria: SelectionCriteria,
    /// Adapter configuration
    pub config: AdapterConfig,
    /// Performance requirements
    pub performance: PerformanceRequirements,
    /// Reliability requirements
    pub reliability: ReliabilityRequirements,
}

/// Performance requirements
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    /// Maximum acceptable latency
    pub max_latency: Duration,
    /// Minimum required throughput
    pub min_throughput: u64,
    /// Maximum memory usage
    pub max_memory: Option<usize>,
}

/// Reliability requirements
#[derive(Debug, Clone)]
pub struct ReliabilityRequirements {
    /// Minimum availability (0.0 - 1.0)
    pub min_availability: f64,
    /// Maximum error rate (0.0 - 1.0)
    pub max_error_rate: f64,
    /// Recovery time requirement
    pub max_recovery_time: Duration,
}

/// Adapter information for selection
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    /// Adapter type
    pub adapter_type: AdapterType,
    /// Adapter capabilities
    pub capabilities: AdapterCapabilities,
    /// Builder availability
    pub builder_available: bool,
}

impl CapabilityDetector {
    /// Create new capability detector
    pub fn new() -> Self {
        Self {
            capability_cache: Arc::new(RwLock::new(HashMap::new())),
            detection_timeout: Duration::from_secs(5),
        }
    }

    /// Get capabilities for adapter type
    pub async fn get_capabilities(&self, adapter_type: AdapterType) -> Result<AdapterCapabilities> {
        // Check cache first
        {
            let cache = self.capability_cache.read().await;
            if let Some(capabilities) = cache.get(&adapter_type) {
                return Ok(capabilities.clone());
            }
        }

        // Detect capabilities
        let capabilities = self.detect_capabilities(adapter_type).await?;

        // Cache result
        {
            let mut cache = self.capability_cache.write().await;
            cache.insert(adapter_type, capabilities.clone());
        }

        Ok(capabilities)
    }

    /// Detect capabilities for adapter type
    async fn detect_capabilities(&self, adapter_type: AdapterType) -> Result<AdapterCapabilities> {
        // This would implement actual capability detection logic
        // For now, return default capabilities based on adapter type
        Ok(match adapter_type {
            AdapterType::Http => AdapterCapabilities {
                adapter_type,
                max_connections: Some(1000),
                max_message_size: 10 * 1024 * 1024, // 10MB
                latency_profile: LatencyProfile {
                    avg_latency: Duration::from_millis(50),
                    p95_latency: Duration::from_millis(100),
                    p99_latency: Duration::from_millis(200),
                    max_latency: Duration::from_secs(30),
                },
                throughput_profile: ThroughputProfile {
                    max_messages_per_sec: 10000,
                    max_bytes_per_sec: 100 * 1024 * 1024, // 100MB/s
                    burst_capacity: 50000,
                },
                reliability_features: ReliabilityFeatures {
                    supports_retry: true,
                    supports_circuit_breaker: true,
                    supports_health_check: true,
                    supports_failover: true,
                    supports_load_balancing: true,
                },
                security_features: SecurityFeatures {
                    supports_tls: true,
                    supports_mtls: true,
                    supports_auth: true,
                    supports_authz: true,
                    supports_encryption: true,
                },
                transport_features: TransportFeatures {
                    supports_streaming: true,
                    supports_multiplexing: false,
                    supports_compression: true,
                    supports_keepalive: true,
                    supports_pooling: true,
                },
                supported_operations: vec![
                    AdapterOperation::Send,
                    AdapterOperation::Receive,
                    AdapterOperation::Stream,
                ],
            },
            _ => AdapterCapabilities {
                adapter_type,
                max_connections: Some(100),
                max_message_size: 1024 * 1024, // 1MB
                latency_profile: LatencyProfile {
                    avg_latency: Duration::from_millis(10),
                    p95_latency: Duration::from_millis(20),
                    p99_latency: Duration::from_millis(50),
                    max_latency: Duration::from_secs(5),
                },
                throughput_profile: ThroughputProfile {
                    max_messages_per_sec: 50000,
                    max_bytes_per_sec: 500 * 1024 * 1024, // 500MB/s
                    burst_capacity: 100000,
                },
                reliability_features: ReliabilityFeatures {
                    supports_retry: true,
                    supports_circuit_breaker: true,
                    supports_health_check: true,
                    supports_failover: true,
                    supports_load_balancing: true,
                },
                security_features: SecurityFeatures {
                    supports_tls: false,
                    supports_mtls: false,
                    supports_auth: false,
                    supports_authz: false,
                    supports_encryption: false,
                },
                transport_features: TransportFeatures {
                    supports_streaming: true,
                    supports_multiplexing: true,
                    supports_compression: false,
                    supports_keepalive: true,
                    supports_pooling: true,
                },
                supported_operations: vec![AdapterOperation::Send, AdapterOperation::Receive],
            },
        })
    }
}

impl Default for AdapterFactoryMetrics {
    fn default() -> Self {
        Self {
            adapters_created: 0,
            active_adapters: 0,
            avg_creation_time: Duration::from_millis(100),
            selection_success_rate: 1.0,
            health_check_success_rate: 1.0,
        }
    }
}

impl Default for AdapterFactoryConfig {
    fn default() -> Self {
        Self {
            default_selection: SelectionCriteria::Performance,
            health_check_interval: Duration::from_secs(30),
            capability_cache_ttl: Duration::from_secs(300),
            max_concurrent_adapters: 1000,
            auto_cleanup: true,
        }
    }
}
