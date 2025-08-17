//! Service Registry & Discovery System
//! 
//! Provides intelligent service discovery with health monitoring, load balancing,
//! and automatic failover for distributed systems.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use dashmap::DashMap;
use tracing::{debug, info, warn, error};

use crate::core::networking::valkyrie::adapters::*;
use crate::core::networking::valkyrie::streaming::QoSClass;
use crate::error::{Result, ValkyrieError};

pub mod discovery;
pub mod health;
pub mod load_balancer;
pub mod consensus;

// Re-export key types
pub use discovery::{ServiceDiscovery, DiscoveryStrategy, DiscoveryQuery, QueryType};
pub use health::{HealthMonitor, HealthStatus, HealthCheckConfig, HealthCheckType, CircuitBreaker, CircuitBreakerState};
pub use load_balancer::{LoadBalancer, EndpointState, ResourceUtilization};
// Import LoadBalancingStrategy from types
use crate::core::networking::valkyrie::types::LoadBalancingStrategy;
pub use consensus::{ConsensusManager, ConsensusProtocol, NodeInfo, NodeRole, NodeStatus, ClusterConfig, ClusterState, LogEntry, LogEntryType};

/// Service registry with intelligent discovery and health monitoring
pub struct ServiceRegistry {
    /// Registered services
    services: Arc<DashMap<ServiceId, ServiceEntry>>,
    /// Service discovery engine
    discovery: Arc<ServiceDiscovery>,
    /// Health monitoring system
    health_monitor: Arc<HealthMonitor>,
    /// Load balancer
    load_balancer: Arc<LoadBalancer>,
    /// Consensus manager for distributed coordination
    consensus: Arc<ConsensusManager>,
    /// Registry configuration
    config: RegistryConfig,
    /// Performance metrics
    metrics: Arc<RwLock<RegistryMetrics>>,
    /// Event handlers
    event_handlers: Arc<RwLock<Vec<Arc<dyn ServiceEventHandler>>>>,
}

/// Service entry in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Service ID
    pub service_id: ServiceId,
    /// Service metadata
    pub metadata: ServiceMetadata,
    /// Service endpoints
    pub endpoints: Vec<ServiceEndpoint>,
    /// Health status
    pub health_status: HealthStatus,
    /// Load balancing weight
    pub weight: u32,
    /// Service capabilities
    pub capabilities: ServiceCapabilities,
    /// Registration timestamp
    #[serde(skip)]
    pub registered_at: Instant,
    /// Last heartbeat
    #[serde(skip)]
    pub last_heartbeat: Instant,
    /// Service tags for discovery
    pub tags: HashMap<String, String>,
    /// Service version
    pub version: String,
}

/// Service metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Service name
    pub name: String,
    /// Service type
    pub service_type: ServiceType,
    /// Service description
    pub description: String,
    /// Service owner/team
    pub owner: String,
    /// Environment (dev, staging, prod)
    pub environment: String,
    /// Deployment region
    pub region: String,
    /// Service dependencies
    pub dependencies: Vec<ServiceDependency>,
}

/// Service endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    /// Endpoint ID
    pub endpoint_id: EndpointId,
    /// Adapter ID for this endpoint
    pub adapter_id: AdapterId,
    /// Endpoint address
    pub address: String,
    /// Port number
    pub port: u16,
    /// Protocol (HTTP, gRPC, TCP, etc.)
    pub protocol: String,
    /// Endpoint health status
    pub health_status: HealthStatus,
    /// Endpoint capabilities
    pub capabilities: EndpointCapabilities,
    /// Load balancing weight
    pub weight: u32,
    /// Last health check
    #[serde(skip)]
    pub last_health_check: Instant,
    /// Endpoint latency
    pub latency: Option<Duration>,
}

/// Service capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCapabilities {
    /// Supported QoS classes
    pub qos_classes: Vec<QoSClass>,
    /// Maximum concurrent connections
    pub max_connections: Option<u32>,
    /// Supported message types
    pub message_types: Vec<AdapterMessageType>,
    /// Performance characteristics
    pub performance: PerformanceProfile,
    /// Security features
    pub security_features: Vec<String>,
    /// Custom capabilities
    pub custom: HashMap<String, serde_json::Value>,
}

/// Endpoint capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointCapabilities {
    /// Maximum requests per second
    pub max_rps: Option<u32>,
    /// Average latency
    pub avg_latency: Duration,
    /// Reliability score (0.0 - 1.0)
    pub reliability: f64,
    /// Supported protocols
    pub protocols: Vec<String>,
    /// Load balancing algorithms supported
    pub load_balancing: Vec<String>,
}

/// Performance profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    /// Average latency
    pub avg_latency: Duration,
    /// P95 latency
    pub p95_latency: Duration,
    /// P99 latency
    pub p99_latency: Duration,
    /// Maximum throughput (requests/sec)
    pub max_throughput: u32,
    /// CPU usage (0.0 - 1.0)
    pub cpu_usage: f64,
    /// Memory usage (bytes)
    pub memory_usage: u64,
}

/// Service dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDependency {
    /// Dependency service name
    pub service_name: String,
    /// Dependency type
    pub dependency_type: DependencyType,
    /// Is this dependency critical
    pub critical: bool,
    /// Timeout for dependency calls
    pub timeout: Duration,
    /// Retry policy
    pub retry_policy: RetryPolicy,
}

/// Service types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceType {
    /// Web API service
    WebApi,
    /// Background job processor
    JobProcessor,
    /// Database service
    Database,
    /// Message queue
    MessageQueue,
    /// Cache service
    Cache,
    /// Authentication service
    Auth,
    /// Monitoring service
    Monitoring,
    /// Custom service type
    Custom(String),
}

/// Dependency types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyType {
    /// Synchronous dependency
    Synchronous,
    /// Asynchronous dependency
    Asynchronous,
    /// Optional dependency
    Optional,
    /// Circuit breaker protected
    CircuitBreaker,
}

/// Retry policy
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff strategy
    pub backoff_strategy: BackoffStrategy,
}

/// Backoff strategies
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// Fixed delay
    Fixed,
    /// Linear backoff
    Linear,
    /// Exponential backoff
    Exponential,
    /// Jittered exponential backoff
    JitteredExponential,
}

/// Registry configuration
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// Health check interval
    pub health_check_interval: Duration,
    /// Service TTL (time to live)
    pub service_ttl: Duration,
    /// Discovery refresh interval
    pub discovery_refresh_interval: Duration,
    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// Enable consensus for distributed registry
    pub enable_consensus: bool,
    /// Consensus protocol
    pub consensus_protocol: ConsensusProtocol,
    /// Maximum services per registry
    pub max_services: usize,
    /// Enable service mesh integration
    pub enable_service_mesh: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
}

/// Registry performance metrics
#[derive(Debug, Clone)]
pub struct RegistryMetrics {
    /// Total registered services
    pub total_services: usize,
    /// Healthy services count
    pub healthy_services: usize,
    /// Discovery requests
    pub discovery_requests: u64,
    /// Average discovery latency
    pub avg_discovery_latency: Duration,
    /// Health check failures
    pub health_check_failures: u64,
    /// Load balancing decisions
    pub load_balancing_decisions: u64,
    /// Service registrations
    pub service_registrations: u64,
    /// Service deregistrations
    pub service_deregistrations: u64,
    /// Consensus operations
    pub consensus_operations: u64,
}

/// Service events
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    /// Service registered
    ServiceRegistered {
        service_id: ServiceId,
        metadata: ServiceMetadata,
    },
    /// Service deregistered
    ServiceDeregistered {
        service_id: ServiceId,
        reason: String,
    },
    /// Service health changed
    HealthChanged {
        service_id: ServiceId,
        old_status: HealthStatus,
        new_status: HealthStatus,
    },
    /// Service discovered
    ServiceDiscovered {
        service_id: ServiceId,
        discovery_method: String,
    },
    /// Load balancing decision made
    LoadBalancingDecision {
        service_name: String,
        selected_endpoint: EndpointId,
        strategy: LoadBalancingStrategy,
    },
    /// Consensus event
    ConsensusEvent {
        event_type: String,
        details: HashMap<String, String>,
    },
}

/// Service event handler trait
#[async_trait]
pub trait ServiceEventHandler: Send + Sync {
    /// Handle service event
    async fn handle_event(&self, event: ServiceEvent) -> Result<()>;
}

/// Service ID type
pub type ServiceId = uuid::Uuid;

/// Endpoint ID type
pub type EndpointId = uuid::Uuid;

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(30),
            service_ttl: Duration::from_secs(300), // 5 minutes
            discovery_refresh_interval: Duration::from_secs(60),
            load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
            enable_consensus: true,
            consensus_protocol: ConsensusProtocol::Raft,
            max_services: 10000,
            enable_service_mesh: false,
            metrics_interval: Duration::from_secs(10),
        }
    }
}

impl ServiceRegistry {
    /// Create new service registry
    pub fn new() -> Self {
        Self::with_config(RegistryConfig::default())
    }

    /// Create service registry with custom configuration
    pub fn with_config(config: RegistryConfig) -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            discovery: Arc::new(ServiceDiscovery::new()),
            health_monitor: Arc::new(HealthMonitor::new(config.health_check_interval)),
            load_balancer: Arc::new(LoadBalancer::new(config.load_balancing_strategy.clone())),
            consensus: Arc::new(ConsensusManager::new(config.consensus_protocol.clone())),
            metrics: Arc::new(RwLock::new(RegistryMetrics::default())),
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Register a service
    pub async fn register_service(
        &self,
        metadata: ServiceMetadata,
        endpoints: Vec<ServiceEndpoint>,
        capabilities: ServiceCapabilities,
        tags: HashMap<String, String>,
    ) -> Result<ServiceId> {
        let service_id = uuid::Uuid::new_v4();
        
        // Validate service registration
        self.validate_service_registration(&metadata, &endpoints, &capabilities).await?;

        let service_entry = ServiceEntry {
            service_id,
            metadata: metadata.clone(),
            endpoints,
            health_status: HealthStatus::Unknown,
            weight: 100, // Default weight
            capabilities,
            registered_at: Instant::now(),
            last_heartbeat: Instant::now(),
            tags,
            version: "1.0.0".to_string(), // Default version
        };

        // Store service entry
        self.services.insert(service_id, service_entry.clone());

        // Start health monitoring
        self.health_monitor.start_monitoring(service_id, &service_entry.endpoints).await?;

        // Register with discovery system
        self.discovery.register_service(service_id, &service_entry).await?;

        // Update consensus if enabled
        if self.config.enable_consensus {
            self.consensus.register_service(service_id, &service_entry).await?;
        }

        // Emit event
        self.emit_event(ServiceEvent::ServiceRegistered {
            service_id,
            metadata,
        }).await;

        // Update metrics
        self.update_registration_metrics().await;

        info!("Registered service: {} ({})", service_entry.metadata.name, service_id);
        Ok(service_id)
    }

    /// Deregister a service
    pub async fn deregister_service(&self, service_id: ServiceId, reason: String) -> Result<()> {
        if let Some((_, service_entry)) = self.services.remove(&service_id) {
            // Stop health monitoring
            self.health_monitor.stop_monitoring(service_id).await?;

            // Deregister from discovery
            self.discovery.deregister_service(service_id).await?;

            // Update consensus if enabled
            if self.config.enable_consensus {
                self.consensus.deregister_service(service_id).await?;
            }

            // Emit event
            self.emit_event(ServiceEvent::ServiceDeregistered {
                service_id,
                reason: reason.clone(),
            }).await;

            // Update metrics
            self.update_deregistration_metrics().await;

            info!("Deregistered service: {} ({}), reason: {}", 
                  service_entry.metadata.name, service_id, reason);
        }

        Ok(())
    }

    /// Discover services by name
    pub async fn discover_services(&self, service_name: &str) -> Result<Vec<ServiceEntry>> {
        let start_time = Instant::now();
        
        let services = self.discovery.discover_by_name(service_name).await?;
        
        // Update metrics
        self.update_discovery_metrics(start_time.elapsed()).await;

        // Emit discovery events
        for service in &services {
            self.emit_event(ServiceEvent::ServiceDiscovered {
                service_id: service.service_id,
                discovery_method: "name".to_string(),
            }).await;
        }

        Ok(services)
    }

    /// Discover services by tags
    pub async fn discover_by_tags(&self, tags: &HashMap<String, String>) -> Result<Vec<ServiceEntry>> {
        let start_time = Instant::now();
        
        let services = self.discovery.discover_by_tags(tags).await?;
        
        // Update metrics
        self.update_discovery_metrics(start_time.elapsed()).await;

        // Emit discovery events
        for service in &services {
            self.emit_event(ServiceEvent::ServiceDiscovered {
                service_id: service.service_id,
                discovery_method: "tags".to_string(),
            }).await;
        }

        Ok(services)
    }

    /// Get best endpoint for service using load balancing
    pub async fn get_best_endpoint(
        &self,
        service_name: &str,
        qos_requirements: Option<QoSClass>,
    ) -> Result<ServiceEndpoint> {
        let services = self.discover_services(service_name).await?;
        
        if services.is_empty() {
            return Err(ValkyrieError::ServiceNotFound(format!(
                "No services found with name: {}", service_name
            )));
        }

        // Filter services by QoS requirements
        let filtered_services = if let Some(qos_class) = qos_requirements {
            services.into_iter()
                .filter(|s| s.capabilities.qos_classes.contains(&qos_class))
                .collect()
        } else {
            services
        };

        if filtered_services.is_empty() {
            return Err(ValkyrieError::ServiceNotFound(format!(
                "No services found matching QoS requirements for: {}", service_name
            )));
        }

        // Use load balancer to select best endpoint
        let selected_endpoint = self.load_balancer.select_endpoint(&filtered_services).await?;

        // Emit load balancing event
        self.emit_event(ServiceEvent::LoadBalancingDecision {
            service_name: service_name.to_string(),
            selected_endpoint: selected_endpoint.endpoint_id,
            strategy: self.config.load_balancing_strategy.clone(),
        }).await;

        // Update metrics
        self.update_load_balancing_metrics().await;

        Ok(selected_endpoint)
    }

    /// Update service heartbeat
    pub async fn heartbeat(&self, service_id: ServiceId) -> Result<()> {
        if let Some(mut service_entry) = self.services.get_mut(&service_id) {
            service_entry.last_heartbeat = Instant::now();
            
            // Update health status if it was unhealthy
            if matches!(service_entry.health_status, HealthStatus::Unhealthy { .. }) {
                let old_status = service_entry.health_status.clone();
                service_entry.health_status = HealthStatus::Healthy;
                
                self.emit_event(ServiceEvent::HealthChanged {
                    service_id,
                    old_status,
                    new_status: HealthStatus::Healthy,
                }).await;
            }
            
            debug!("Received heartbeat from service: {}", service_id);
        }

        Ok(())
    }

    /// Get service by ID
    pub async fn get_service(&self, service_id: ServiceId) -> Option<ServiceEntry> {
        self.services.get(&service_id).map(|entry| entry.clone())
    }

    /// List all services
    pub async fn list_services(&self) -> Vec<ServiceEntry> {
        self.services.iter().map(|entry| entry.clone()).collect()
    }

    /// List services by type
    pub async fn list_services_by_type(&self, service_type: ServiceType) -> Vec<ServiceEntry> {
        self.services.iter()
            .filter(|entry| entry.metadata.service_type == service_type)
            .map(|entry| entry.clone())
            .collect()
    }

    /// Add event handler
    pub async fn add_event_handler(&self, handler: Arc<dyn ServiceEventHandler>) {
        let mut handlers = self.event_handlers.write().await;
        handlers.push(handler);
    }

    /// Emit service event
    async fn emit_event(&self, event: ServiceEvent) {
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.handle_event(event.clone()).await {
                error!("Event handler error: {}", e);
            }
        }
    }

    /// Validate service registration
    async fn validate_service_registration(
        &self,
        metadata: &ServiceMetadata,
        endpoints: &[ServiceEndpoint],
        capabilities: &ServiceCapabilities,
    ) -> Result<()> {
        // Check if we've reached the maximum number of services
        if self.services.len() >= self.config.max_services {
            return Err(ValkyrieError::RegistryFull(format!(
                "Registry has reached maximum capacity: {}", self.config.max_services
            )));
        }

        // Validate service name
        if metadata.name.is_empty() {
            return Err(ValkyrieError::InvalidServiceName("Service name cannot be empty".to_string()));
        }

        // Validate endpoints
        if endpoints.is_empty() {
            return Err(ValkyrieError::InvalidEndpoints("Service must have at least one endpoint".to_string()));
        }

        // Validate endpoint addresses
        for endpoint in endpoints {
            if endpoint.address.is_empty() {
                return Err(ValkyrieError::InvalidEndpoints("Endpoint address cannot be empty".to_string()));
            }
            if endpoint.port == 0 {
                return Err(ValkyrieError::InvalidEndpoints("Endpoint port must be greater than 0".to_string()));
            }
        }

        // Validate capabilities
        if capabilities.qos_classes.is_empty() {
            return Err(ValkyrieError::InvalidCapabilities("Service must support at least one QoS class".to_string()));
        }

        Ok(())
    }

    /// Update registration metrics
    async fn update_registration_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.service_registrations += 1;
        metrics.total_services = self.services.len();
    }

    /// Update deregistration metrics
    async fn update_deregistration_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.service_deregistrations += 1;
        metrics.total_services = self.services.len();
    }

    /// Update discovery metrics
    async fn update_discovery_metrics(&self, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.discovery_requests += 1;
        
        // Update average discovery latency
        let total_latency = metrics.avg_discovery_latency.as_nanos() * (metrics.discovery_requests - 1) as u128
            + latency.as_nanos();
        metrics.avg_discovery_latency = Duration::from_nanos((total_latency / metrics.discovery_requests as u128) as u64);
    }

    /// Update load balancing metrics
    async fn update_load_balancing_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.load_balancing_decisions += 1;
    }

    /// Get registry metrics
    pub async fn metrics(&self) -> RegistryMetrics {
        let mut metrics = self.metrics.read().await.clone();
        
        // Update real-time metrics
        metrics.total_services = self.services.len();
        metrics.healthy_services = self.services.iter()
            .filter(|entry| matches!(entry.health_status, HealthStatus::Healthy))
            .count();
        
        metrics
    }

    /// Perform cleanup of expired services
    pub async fn cleanup_expired_services(&self) -> Result<()> {
        let now = Instant::now();
        let mut expired_services = Vec::new();

        // Find expired services
        for entry in self.services.iter() {
            if now.duration_since(entry.last_heartbeat) > self.config.service_ttl {
                expired_services.push(entry.service_id);
            }
        }

        // Remove expired services
        for service_id in expired_services {
            self.deregister_service(service_id, "TTL expired".to_string()).await?;
        }

        Ok(())
    }

    /// Start background tasks
    pub async fn start_background_tasks(&self) -> Result<()> {
        // Start health monitoring
        self.health_monitor.start().await?;

        // Start discovery refresh
        self.discovery.start_refresh_task(self.config.discovery_refresh_interval).await?;

        // Start consensus if enabled
        if self.config.enable_consensus {
            self.consensus.start().await?;
        }

        // Start cleanup task
        let registry = Arc::new(self.clone());
        let cleanup_interval = self.config.service_ttl / 2; // Cleanup twice as often as TTL
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                if let Err(e) = registry.cleanup_expired_services().await {
                    error!("Cleanup task error: {}", e);
                }
            }
        });

        info!("Started service registry background tasks");
        Ok(())
    }
}

impl Clone for ServiceRegistry {
    fn clone(&self) -> Self {
        Self {
            services: Arc::clone(&self.services),
            discovery: Arc::clone(&self.discovery),
            health_monitor: Arc::clone(&self.health_monitor),
            load_balancer: Arc::clone(&self.load_balancer),
            consensus: Arc::clone(&self.consensus),
            config: self.config.clone(),
            metrics: Arc::clone(&self.metrics),
            event_handlers: Arc::clone(&self.event_handlers),
        }
    }
}

impl Default for RegistryMetrics {
    fn default() -> Self {
        Self {
            total_services: 0,
            healthy_services: 0,
            discovery_requests: 0,
            avg_discovery_latency: Duration::from_nanos(0),
            health_check_failures: 0,
            load_balancing_decisions: 0,
            service_registrations: 0,
            service_deregistrations: 0,
            consensus_operations: 0,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Exponential,
        }
    }
}

impl Default for PerformanceProfile {
    fn default() -> Self {
        Self {
            avg_latency: Duration::from_millis(50),
            p95_latency: Duration::from_millis(100),
            p99_latency: Duration::from_millis(200),
            max_throughput: 1000,
            cpu_usage: 0.5,
            memory_usage: 100 * 1024 * 1024, // 100MB
        }
    }
}

impl Default for EndpointCapabilities {
    fn default() -> Self {
        Self {
            max_rps: Some(1000),
            avg_latency: Duration::from_millis(50),
            reliability: 0.99,
            protocols: vec!["http".to_string()],
            load_balancing: vec!["round_robin".to_string()],
        }
    }
}

impl std::fmt::Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceType::WebApi => write!(f, "WebApi"),
            ServiceType::JobProcessor => write!(f, "JobProcessor"),
            ServiceType::Database => write!(f, "Database"),
            ServiceType::MessageQueue => write!(f, "MessageQueue"),
            ServiceType::Cache => write!(f, "Cache"),
            ServiceType::Auth => write!(f, "Auth"),
            ServiceType::Monitoring => write!(f, "Monitoring"),
            ServiceType::Custom(name) => write!(f, "Custom({})", name),
        }
    }
}