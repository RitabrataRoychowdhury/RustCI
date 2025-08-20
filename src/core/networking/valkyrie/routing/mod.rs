// Advanced Message Routing & Load Balancing System
// Task 3.1.1.1: Routing Algorithm Framework

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod adaptive;
pub mod algorithms;
pub mod cache;
pub mod config;
pub mod load_balancer;
pub mod metrics;
pub mod qos;
pub mod topology;

#[cfg(test)]
pub mod tests;

// Core Types
pub type NodeId = Uuid;
pub type RouteId = Uuid;
pub type LinkId = Uuid;
pub type EndpointId = Uuid;
pub type ServiceId = Uuid;
pub type RegionId = String;
pub type ZoneId = String;

/// Routing context containing all information needed for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingContext {
    pub message_id: Uuid,
    pub source: NodeId,
    pub destination: NodeId,
    pub qos_requirements: QoSRequirements,
    pub security_context: SecurityContext,
    pub routing_hints: RoutingHints,
    pub deadline: Option<std::time::SystemTime>,
    pub created_at: std::time::SystemTime,
}

/// Quality of Service requirements for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QoSRequirements {
    pub max_latency: Option<Duration>,
    pub min_bandwidth: Option<u64>,
    pub reliability_threshold: Option<f64>,
    pub priority: MessagePriority,
    pub sla_class: Option<SLAClass>,
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessagePriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

/// SLA classification for messages
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SLAClass {
    Platinum,
    Gold,
    Silver,
    Bronze,
    BestEffort,
}

/// Security context for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub security_level: SecurityLevel,
    pub allowed_regions: Vec<RegionId>,
    pub encryption_required: bool,
    pub audit_required: bool,
}

/// Security levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecurityLevel {
    Public,
    Internal,
    Confidential,
    Secret,
    TopSecret,
}

/// Routing hints to influence path selection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingHints {
    pub preferred_regions: Vec<RegionId>,
    pub avoid_nodes: Vec<NodeId>,
    pub preferred_transports: Vec<TransportType>,
    pub cost_weight: Option<f64>,
    pub latency_weight: Option<f64>,
    pub reliability_weight: Option<f64>,
}

/// Transport types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    TCP,
    QUIC,
    WebSocket,
    UnixSocket,
    HTTP,
    HTTPS,
}

/// A calculated route between two nodes
#[derive(Debug, Clone)]
pub struct Route {
    pub id: RouteId,
    pub path: Vec<NodeId>,
    pub hops: Vec<Hop>,
    pub total_cost: f64,
    pub estimated_latency: Duration,
    pub reliability_score: f64,
    pub bandwidth_available: u64,
    pub created_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub metadata: RouteMetadata,
}

/// Individual hop in a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hop {
    pub from: NodeId,
    pub to: NodeId,
    pub transport: TransportType,
    pub cost: f64,
    pub latency: Duration,
    pub bandwidth: u64,
    pub reliability: f64,
    pub security_level: SecurityLevel,
}

/// Additional route metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteMetadata {
    pub algorithm_used: String,
    pub calculation_time: Duration,
    pub cache_hit: bool,
    pub alternative_routes: u32,
    pub confidence_score: f64,
}

/// Available routing algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoutingAlgorithm {
    ShortestPath,
    LoadAware,
    LatencyOptimized,
    CostOptimized,
    Adaptive,
    Custom(u32), // Custom algorithm ID
}

impl Default for RoutingAlgorithm {
    fn default() -> Self {
        Self::LoadAware
    }
}

/// Core trait for routing strategies
#[async_trait::async_trait]
pub trait RoutingStrategy: Send + Sync {
    /// Calculate the optimal route for given context
    async fn calculate_route(
        &self,
        source: &NodeId,
        destination: &NodeId,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError>;

    /// Get the algorithm type this strategy implements
    fn get_algorithm_type(&self) -> RoutingAlgorithm;

    /// Get the priority of this strategy (higher = more preferred)
    fn get_priority(&self) -> u8;

    /// Check if this strategy can handle the given context
    fn can_handle(&self, context: &RoutingContext) -> bool;

    /// Get performance characteristics of this strategy
    fn get_performance_profile(&self) -> PerformanceProfile;
}

/// Performance characteristics of a routing strategy
#[derive(Debug, Clone)]
pub struct PerformanceProfile {
    pub avg_calculation_time: Duration,
    pub memory_usage: u64,
    pub cpu_intensity: f64,      // 0.0 to 1.0
    pub scalability_factor: f64, // How well it scales with topology size
}

/// Network topology representation
#[derive(Debug, Clone)]
pub struct NetworkTopology {
    pub nodes: HashMap<NodeId, NetworkNode>,
    pub links: HashMap<LinkId, NetworkLink>,
    pub regions: HashMap<RegionId, Region>,
    pub last_updated: SystemTime,
    pub version: u64,
}

/// Network node information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkNode {
    pub id: NodeId,
    pub address: std::net::SocketAddr,
    pub region: RegionId,
    pub zone: ZoneId,
    pub capabilities: NodeCapabilities,
    pub metrics: NodeMetrics,
    pub status: NodeStatus,
    pub security_level: SecurityLevel,
}

/// Node capabilities
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub supported_transports: Vec<TransportType>,
    pub max_connections: u32,
    pub max_bandwidth: u64,
    pub encryption_support: Vec<String>,
    pub features: Vec<String>,
}

/// Real-time node metrics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub network_utilization: f64,
    pub connection_count: u32,
    pub message_rate: u64,
    pub error_rate: f64,
    pub last_updated: SystemTime,
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            network_utilization: 0.0,
            connection_count: 0,
            message_rate: 0,
            error_rate: 0.0,
            last_updated: SystemTime::now(),
        }
    }
}

/// Node operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Degraded,
    Maintenance,
    Offline,
    Unknown,
}

/// Network link between nodes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkLink {
    pub id: LinkId,
    pub from: NodeId,
    pub to: NodeId,
    pub transport: TransportType,
    pub latency: Duration,
    pub bandwidth: u64,
    pub reliability: f64,
    pub cost: f64,
    pub utilization: f64,
    pub status: LinkStatus,
    pub last_updated: SystemTime,
}

/// Link operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkStatus {
    Active,
    Congested,
    Degraded,
    Failed,
    Maintenance,
}

/// Geographic region information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub id: RegionId,
    pub name: String,
    pub coordinates: Option<GeoCoordinates>,
    pub nodes: std::collections::HashSet<NodeId>,
    pub inter_region_links: HashMap<RegionId, Vec<LinkId>>,
    pub regulatory_constraints: RegulatoryConstraints,
}

/// Geographic coordinates
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GeoCoordinates {
    pub latitude: f64,
    pub longitude: f64,
}

/// Regulatory constraints for a region
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegulatoryConstraints {
    pub data_residency_required: bool,
    pub encryption_required: bool,
    pub audit_logging_required: bool,
    pub allowed_countries: Vec<String>,
    pub restricted_data_types: Vec<String>,
}

/// Routing errors
#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error("No route found from {src} to {dst}")]
    NoRouteFound { src: String, dst: String },

    #[error("Topology not available")]
    TopologyUnavailable,

    #[error("Invalid routing context: {reason}")]
    InvalidContext { reason: String },

    #[error("QoS requirements cannot be satisfied: {requirements:?}")]
    QoSNotSatisfiable { requirements: QoSRequirements },

    #[error("Security constraints violated: {constraint}")]
    SecurityViolation { constraint: String },

    #[error("Algorithm error: {algorithm:?} - {error}")]
    AlgorithmError {
        algorithm: RoutingAlgorithm,
        error: String,
    },

    #[error("Timeout calculating route after {duration:?}")]
    CalculationTimeout { duration: Duration },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Algorithm manager for routing strategies
pub struct AlgorithmManager {
    strategies: HashMap<RoutingAlgorithm, Arc<dyn RoutingStrategy>>,
    default_algorithm: RoutingAlgorithm,
    algorithm_selector: Arc<dyn AlgorithmSelector>,
    performance_tracker: Arc<PerformanceTracker>,
}

/// Trait for selecting the best algorithm for a given context
#[async_trait::async_trait]
pub trait AlgorithmSelector: Send + Sync {
    async fn select_algorithm(
        &self,
        context: &RoutingContext,
        available_algorithms: &[RoutingAlgorithm],
        performance_history: &PerformanceHistory,
    ) -> RoutingAlgorithm;
}

/// Performance tracking for algorithms
pub struct PerformanceTracker {
    history: Arc<RwLock<HashMap<RoutingAlgorithm, PerformanceHistory>>>,
}

/// Historical performance data for an algorithm
#[derive(Debug, Clone)]
pub struct PerformanceHistory {
    pub total_calculations: u64,
    pub successful_calculations: u64,
    pub avg_calculation_time: Duration,
    pub avg_route_quality: f64,
    pub last_updated: SystemTime,
}

impl Default for PerformanceHistory {
    fn default() -> Self {
        Self {
            total_calculations: 0,
            successful_calculations: 0,
            avg_calculation_time: Duration::ZERO,
            avg_route_quality: 0.0,
            last_updated: SystemTime::now(),
        }
    }
}

impl AlgorithmManager {
    /// Create a new algorithm manager
    pub fn new(
        default_algorithm: RoutingAlgorithm,
        algorithm_selector: Arc<dyn AlgorithmSelector>,
    ) -> Self {
        Self {
            strategies: HashMap::new(),
            default_algorithm,
            algorithm_selector,
            performance_tracker: Arc::new(PerformanceTracker::new()),
        }
    }

    /// Register a routing strategy
    pub fn register_strategy(
        &mut self,
        algorithm: RoutingAlgorithm,
        strategy: Arc<dyn RoutingStrategy>,
    ) {
        self.strategies.insert(algorithm, strategy);
    }

    /// Calculate route using the best available algorithm
    pub async fn calculate_route(
        &self,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        let start_time = SystemTime::now();

        // Select the best algorithm for this context
        let available_algorithms: Vec<_> = self.strategies.keys().copied().collect();
        let performance_history_map = self.performance_tracker.get_history().await;

        // Pick one entry, for example the first available algorithm
        let fallback = PerformanceHistory::default();

        let perf = available_algorithms
            .first()
            .and_then(|alg| performance_history_map.get(alg))
            .unwrap_or(&fallback);

        let selected_algorithm = self
            .algorithm_selector
            .select_algorithm(context, &available_algorithms, perf)
            .await;

        // Get the strategy for the selected algorithm
        let strategy = self.strategies.get(&selected_algorithm).ok_or_else(|| {
            RoutingError::AlgorithmError {
                algorithm: selected_algorithm,
                error: "Strategy not found".to_string(),
            }
        })?;

        // Calculate the route
        let mut route = strategy
            .calculate_route(&context.source, &context.destination, context, topology)
            .await?;

        // Update route metadata
        route.metadata.algorithm_used = format!("{:?}", selected_algorithm);
        route.metadata.calculation_time = start_time.elapsed().unwrap_or_default();

        // Track performance
        self.performance_tracker
            .record_calculation(
                selected_algorithm,
                start_time.elapsed().unwrap_or_default(),
                &route,
            )
            .await;

        Ok(route)
    }

    /// Get available algorithms
    pub fn get_available_algorithms(&self) -> Vec<RoutingAlgorithm> {
        self.strategies.keys().copied().collect()
    }

    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> HashMap<RoutingAlgorithm, PerformanceHistory> {
        self.performance_tracker.get_history().await
    }
}

impl PerformanceTracker {
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn record_calculation(
        &self,
        algorithm: RoutingAlgorithm,
        duration: Duration,
        route: &Route,
    ) {
        let mut history = self.history.write().await;
        let entry = history.entry(algorithm).or_default();

        entry.total_calculations += 1;
        entry.successful_calculations += 1;

        // Update running average
        let alpha = 0.1; // Exponential moving average factor
        let new_time = duration.as_nanos() as f64;
        let current_avg = entry.avg_calculation_time.as_nanos() as f64;
        let updated_avg = current_avg * (1.0 - alpha) + new_time * alpha;
        entry.avg_calculation_time = Duration::from_nanos(updated_avg as u64);

        // Update route quality (simplified metric)
        let quality = 1.0 / (1.0 + route.total_cost);
        entry.avg_route_quality = entry.avg_route_quality * (1.0 - alpha) + quality * alpha;

        entry.last_updated = SystemTime::now();
    }

    pub async fn get_history(&self) -> HashMap<RoutingAlgorithm, PerformanceHistory> {
        self.history.read().await.clone()
    }
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl Default for SecurityLevel {
    fn default() -> Self {
        Self::Internal
    }
}

impl Default for NodeStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl Default for LinkStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl NetworkTopology {
    /// Create a new empty topology
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            links: HashMap::new(),
            regions: HashMap::new(),
            last_updated: SystemTime::now(),
            version: 1,
        }
    }

    /// Get outgoing links from a node
    pub fn get_outgoing_links(&self, node_id: &NodeId) -> Vec<&NetworkLink> {
        self.links
            .values()
            .filter(|link| link.from == *node_id && link.status == LinkStatus::Active)
            .collect()
    }

    /// Get incoming links to a node
    pub fn get_incoming_links(&self, node_id: &NodeId) -> Vec<&NetworkLink> {
        self.links
            .values()
            .filter(|link| link.to == *node_id && link.status == LinkStatus::Active)
            .collect()
    }

    /// Check if two nodes are directly connected
    pub fn are_connected(&self, from: &NodeId, to: &NodeId) -> bool {
        self.links.values().any(|link| {
            (link.from == *from && link.to == *to) || (link.from == *to && link.to == *from)
        })
    }

    /// Get all active nodes
    pub fn get_active_nodes(&self) -> Vec<&NetworkNode> {
        self.nodes
            .values()
            .filter(|node| node.status == NodeStatus::Active)
            .collect()
    }

    /// Update topology version
    pub fn update_version(&mut self) {
        self.version += 1;
        self.last_updated = SystemTime::now();
    }
}

impl RoutingContext {
    /// Create a new routing context
    pub fn new(source: NodeId, destination: NodeId, qos_requirements: QoSRequirements) -> Self {
        Self {
            message_id: Uuid::new_v4(),
            source,
            destination,
            qos_requirements,
            security_context: SecurityContext::default(),
            routing_hints: RoutingHints::default(),
            deadline: None,
            created_at: SystemTime::now(),
        }
    }

    /// Check if the context has expired
    pub fn is_expired(&self) -> bool {
        if let Some(deadline) = self.deadline {
            SystemTime::now() > deadline
        } else {
            false
        }
    }

    /// Get remaining time until deadline
    pub fn remaining_time(&self) -> Option<Duration> {
        self.deadline.map(|deadline| {
            deadline
                .duration_since(SystemTime::now())
                .unwrap_or(Duration::ZERO)
        })
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self {
            user_id: None,
            tenant_id: None,
            security_level: SecurityLevel::Internal,
            allowed_regions: Vec::new(),
            encryption_required: false,
            audit_required: false,
        }
    }
}

impl Default for QoSRequirements {
    fn default() -> Self {
        Self {
            max_latency: None,
            min_bandwidth: None,
            reliability_threshold: None,
            priority: MessagePriority::Normal,
            sla_class: None,
        }
    }
}

// Re-export commonly used types
pub use algorithms::*;
pub use cache::*;
pub use config::*;
pub use load_balancer::*;
pub use qos::*;
pub use topology::*;

// Additional imports needed
use sha2;
