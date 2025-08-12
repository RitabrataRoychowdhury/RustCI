//! Cluster domain entities and types
//!
//! This module defines the core cluster entities for multi-node coordination
//! and distributed execution in the RustCI platform.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use utoipa::ToSchema;
use uuid::Uuid;

use super::runner::{JobId, RunnerId};

/// Unique identifier for a cluster node
pub type NodeId = Uuid;

/// Unique identifier for a cluster
pub type ClusterId = Uuid;

/// Cluster node entity representing a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Node name
    pub name: String,
    /// Network address of the node
    pub address: SocketAddr,
    /// Node role in the cluster
    pub role: NodeRole,
    /// Current node status
    pub status: NodeStatus,
    /// List of runners on this node
    pub runners: Vec<RunnerId>,
    /// Node resource information
    pub resources: NodeResources,
    /// Node metadata
    pub metadata: HashMap<String, String>,
    /// Node creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
    /// Node version information
    pub version: String,
}

/// Node roles in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum NodeRole {
    /// Master node that coordinates the cluster
    Master,
    /// Worker node that executes jobs
    Worker,
    /// Hybrid node that can act as both master and worker
    Hybrid,
}

/// Node status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum NodeStatus {
    /// Node is active and healthy
    Active,
    /// Node is starting up
    Starting,
    /// Node is shutting down gracefully
    Stopping,
    /// Node is offline or unreachable
    Offline,
    /// Node is in maintenance mode
    Maintenance,
    /// Node has failed and needs attention
    Failed,
    /// Node is being drained (no new jobs)
    Draining,
}

/// Node resource information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeResources {
    /// Total CPU cores
    pub total_cpu: u32,
    /// Available CPU cores
    pub available_cpu: u32,
    /// Total memory in MB
    pub total_memory: u32,
    /// Available memory in MB
    pub available_memory: u32,
    /// Total disk space in MB
    pub total_disk: u32,
    /// Available disk space in MB
    pub available_disk: u32,
    /// Current CPU usage percentage (0-100)
    pub cpu_usage: f64,
    /// Current memory usage percentage (0-100)
    pub memory_usage: f64,
    /// Current disk usage percentage (0-100)
    pub disk_usage: f64,
}

/// Cluster entity representing the entire cluster
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Cluster {
    /// Unique cluster identifier
    pub id: ClusterId,
    /// Cluster name
    pub name: String,
    /// Cluster configuration
    pub config: ClusterConfig,
    /// Current cluster status
    pub status: ClusterStatus,
    /// List of nodes in the cluster
    pub nodes: Vec<NodeId>,
    /// Cluster metadata
    pub metadata: HashMap<String, String>,
    /// Cluster creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Maximum number of nodes allowed
    pub max_nodes: u32,
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u32,
    /// Node failure timeout in seconds
    pub failure_timeout: u32,
    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// Auto-scaling configuration
    pub auto_scaling: Option<AutoScalingConfig>,
    /// Security configuration
    pub security: SecurityConfig,
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoadBalancingStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// Least loaded node first
    LeastLoaded,
    /// Random selection
    Random,
    /// Weighted distribution based on node capacity
    Weighted,
    /// Affinity-based scheduling
    Affinity,
}

/// Auto-scaling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScalingConfig {
    /// Enable auto-scaling
    pub enabled: bool,
    /// Minimum number of nodes
    pub min_nodes: u32,
    /// Maximum number of nodes
    pub max_nodes: u32,
    /// CPU threshold for scaling up (percentage)
    pub scale_up_cpu_threshold: f64,
    /// CPU threshold for scaling down (percentage)
    pub scale_down_cpu_threshold: f64,
    /// Memory threshold for scaling up (percentage)
    pub scale_up_memory_threshold: f64,
    /// Memory threshold for scaling down (percentage)
    pub scale_down_memory_threshold: f64,
    /// Cooldown period between scaling operations (seconds)
    pub cooldown_period: u32,
}

/// Security configuration for the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable TLS for inter-node communication
    pub tls_enabled: bool,
    /// Certificate authority configuration
    pub ca_config: Option<CertificateConfig>,
    /// Authentication method
    pub auth_method: AuthenticationMethod,
    /// Network policies
    pub network_policies: Vec<NetworkPolicy>,
}

/// Certificate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateConfig {
    /// CA certificate path
    pub ca_cert_path: String,
    /// CA private key path
    pub ca_key_path: String,
    /// Certificate validity period in days
    pub validity_days: u32,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthenticationMethod {
    /// No authentication (development only)
    None,
    /// Token-based authentication
    Token { secret: String },
    /// Certificate-based authentication
    Certificate,
    /// OAuth2-based authentication
    OAuth2 { provider: String },
}

/// Network policy for cluster security
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Policy name
    pub name: String,
    /// Source network/IP ranges
    pub source_ranges: Vec<String>,
    /// Allowed ports
    pub allowed_ports: Vec<u16>,
    /// Protocol (TCP/UDP)
    pub protocol: NetworkProtocol,
}

/// Network protocols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkProtocol {
    TCP,
    UDP,
    Both,
}

/// Overall cluster status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum ClusterStatus {
    /// Cluster is healthy and operational
    Healthy,
    /// Cluster is degraded but functional
    Degraded { reason: String },
    /// Cluster is unhealthy
    Unhealthy { reason: String },
    /// Cluster is starting up
    Starting,
    /// Cluster is shutting down
    Stopping,
    /// Cluster is offline
    Offline,
}

/// Cluster statistics and metrics
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterMetrics {
    /// Total number of nodes
    pub total_nodes: u32,
    /// Number of healthy nodes
    pub healthy_nodes: u32,
    /// Number of failed nodes
    pub failed_nodes: u32,
    /// Total number of runners
    pub total_runners: u32,
    /// Number of active runners
    pub active_runners: u32,
    /// Total number of jobs
    pub total_jobs: u32,
    /// Number of running jobs
    pub running_jobs: u32,
    /// Number of queued jobs
    pub queued_jobs: u32,
    /// Overall cluster load (0-100)
    pub cluster_load: f64,
    /// Average CPU usage across nodes
    pub avg_cpu_usage: f64,
    /// Average memory usage across nodes
    pub avg_memory_usage: f64,
    /// Jobs per second throughput
    pub jobs_per_second: f64,
    /// Average job execution time
    pub avg_job_duration: f64,
}

/// Job distribution information across the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDistribution {
    /// Job ID
    pub job_id: JobId,
    /// Target node for execution
    pub target_node: NodeId,
    /// Target runner
    pub target_runner: RunnerId,
    /// Distribution timestamp
    pub distributed_at: DateTime<Utc>,
    /// Distribution strategy used
    pub strategy: LoadBalancingStrategy,
    /// Distribution metadata
    pub metadata: HashMap<String, String>,
}

/// Node heartbeat information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    /// Node ID
    pub node_id: NodeId,
    /// Heartbeat timestamp
    pub timestamp: DateTime<Utc>,
    /// Node status at heartbeat time
    pub status: NodeStatus,
    /// Resource usage at heartbeat time
    pub resources: NodeResources,
    /// Number of active runners
    pub active_runners: u32,
    /// Number of running jobs
    pub running_jobs: u32,
    /// Additional metrics
    pub metrics: HashMap<String, f64>,
}

impl ClusterNode {
    /// Create a new cluster node
    pub fn new(name: String, address: SocketAddr, role: NodeRole) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            address,
            role,
            status: NodeStatus::Starting,
            runners: Vec::new(),
            resources: NodeResources::default(),
            metadata: HashMap::new(),
            created_at: now,
            last_heartbeat: now,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Update the node's heartbeat
    pub fn update_heartbeat(&mut self, resources: NodeResources) {
        self.last_heartbeat = Utc::now();
        self.resources = resources;
    }

    /// Check if the node is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, NodeStatus::Active)
    }

    /// Check if the node can accept new jobs
    pub fn can_accept_jobs(&self) -> bool {
        matches!(self.status, NodeStatus::Active) && !self.is_draining()
    }

    /// Check if the node is being drained
    pub fn is_draining(&self) -> bool {
        matches!(self.status, NodeStatus::Draining)
    }

    /// Get the node's available capacity
    pub fn available_capacity(&self) -> f64 {
        let cpu_available =
            (self.resources.available_cpu as f64) / (self.resources.total_cpu as f64);
        let memory_available =
            (self.resources.available_memory as f64) / (self.resources.total_memory as f64);
        (cpu_available + memory_available) / 2.0
    }
}

impl Cluster {
    /// Create a new cluster
    pub fn new(name: String, config: ClusterConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            config,
            status: ClusterStatus::Starting,
            nodes: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a node to the cluster
    pub fn add_node(&mut self, node_id: NodeId) {
        if !self.nodes.contains(&node_id) {
            self.nodes.push(node_id);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a node from the cluster
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.nodes.retain(|&id| id != node_id);
        self.updated_at = Utc::now();
    }

    /// Get the number of nodes in the cluster
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the cluster is at capacity
    pub fn is_at_capacity(&self) -> bool {
        self.nodes.len() >= self.config.max_nodes as usize
    }
}

impl Default for NodeResources {
    fn default() -> Self {
        Self {
            total_cpu: 1,
            available_cpu: 1,
            total_memory: 1024, // 1GB
            available_memory: 1024,
            total_disk: 10240, // 10GB
            available_disk: 10240,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
        }
    }
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            max_nodes: 10,
            heartbeat_interval: 30,
            failure_timeout: 120,
            load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
            auto_scaling: None,
            security: SecurityConfig::default(),
        }
    }
}

/// Builder for ClusterConfig using the Builder pattern
pub struct ClusterConfigBuilder {
    max_nodes: u32,
    heartbeat_interval: u32,
    failure_timeout: u32,
    load_balancing_strategy: LoadBalancingStrategy,
    auto_scaling: Option<AutoScalingConfig>,
    security: SecurityConfig,
}

impl ClusterConfigBuilder {
    /// Create a new cluster configuration builder
    pub fn new() -> Self {
        Self {
            max_nodes: 10,
            heartbeat_interval: 30,
            failure_timeout: 120,
            load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
            auto_scaling: None,
            security: SecurityConfig::default(),
        }
    }
    
    /// Set maximum number of nodes
    pub fn max_nodes(mut self, max_nodes: u32) -> Self {
        self.max_nodes = max_nodes;
        self
    }
    
    /// Set heartbeat interval in seconds
    pub fn heartbeat_interval(mut self, interval: u32) -> Self {
        self.heartbeat_interval = interval;
        self
    }
    
    /// Set failure timeout in seconds
    pub fn failure_timeout(mut self, timeout: u32) -> Self {
        self.failure_timeout = timeout;
        self
    }
    
    /// Set load balancing strategy
    pub fn load_balancing_strategy(mut self, strategy: LoadBalancingStrategy) -> Self {
        self.load_balancing_strategy = strategy;
        self
    }
    
    /// Enable auto-scaling with configuration
    pub fn with_auto_scaling(mut self, config: AutoScalingConfig) -> Self {
        self.auto_scaling = Some(config);
        self
    }
    
    /// Set security configuration
    pub fn with_security(mut self, security: SecurityConfig) -> Self {
        self.security = security;
        self
    }
    
    /// Configure for small cluster (development)
    pub fn for_small_cluster(mut self) -> Self {
        self.max_nodes = 3;
        self.heartbeat_interval = 15;
        self.failure_timeout = 60;
        self.load_balancing_strategy = LoadBalancingStrategy::RoundRobin;
        self
    }
    
    /// Configure for large cluster (production)
    pub fn for_large_cluster(mut self) -> Self {
        self.max_nodes = 100;
        self.heartbeat_interval = 60;
        self.failure_timeout = 300;
        self.load_balancing_strategy = LoadBalancingStrategy::LeastLoaded;
        self
    }
    
    /// Enable high availability configuration
    pub fn with_high_availability(mut self) -> Self {
        self.heartbeat_interval = 10;
        self.failure_timeout = 30;
        self.security.tls_enabled = true;
        self
    }
    
    /// Build the final configuration
    pub fn build(self) -> ClusterConfig {
        ClusterConfig {
            max_nodes: self.max_nodes,
            heartbeat_interval: self.heartbeat_interval,
            failure_timeout: self.failure_timeout,
            load_balancing_strategy: self.load_balancing_strategy,
            auto_scaling: self.auto_scaling,
            security: self.security,
        }
    }
}

impl Default for ClusterConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            tls_enabled: false,
            ca_config: None,
            auth_method: AuthenticationMethod::None,
            network_policies: Vec::new(),
        }
    }
}

impl ClusterMetrics {
    /// Create new cluster metrics
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            healthy_nodes: 0,
            failed_nodes: 0,
            total_runners: 0,
            active_runners: 0,
            total_jobs: 0,
            running_jobs: 0,
            queued_jobs: 0,
            cluster_load: 0.0,
            avg_cpu_usage: 0.0,
            avg_memory_usage: 0.0,
            jobs_per_second: 0.0,
            avg_job_duration: 0.0,
        }
    }

    /// Calculate cluster health percentage
    pub fn health_percentage(&self) -> f64 {
        if self.total_nodes == 0 {
            return 0.0;
        }
        (self.healthy_nodes as f64 / self.total_nodes as f64) * 100.0
    }

    /// Check if the cluster is healthy
    pub fn is_healthy(&self) -> bool {
        self.health_percentage() >= 80.0 // 80% of nodes must be healthy
    }
}

impl Default for ClusterMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_cluster_node_creation() {
        let address = SocketAddr::from_str("127.0.0.1:8080").unwrap();
        let node = ClusterNode::new("test-node".to_string(), address, NodeRole::Worker);

        assert_eq!(node.name, "test-node");
        assert_eq!(node.address, address);
        assert_eq!(node.role, NodeRole::Worker);
        assert_eq!(node.status, NodeStatus::Starting);
        assert!(!node.is_healthy());
        assert!(!node.can_accept_jobs());
    }

    #[test]
    fn test_cluster_creation() {
        let config = ClusterConfig::default();
        let cluster = Cluster::new("test-cluster".to_string(), config);

        assert_eq!(cluster.name, "test-cluster");
        assert_eq!(cluster.status, ClusterStatus::Starting);
        assert_eq!(cluster.node_count(), 0);
        assert!(!cluster.is_at_capacity());
    }

    #[test]
    fn test_cluster_node_management() {
        let config = ClusterConfig::default();
        let mut cluster = Cluster::new("test-cluster".to_string(), config);

        let node_id = Uuid::new_v4();
        cluster.add_node(node_id);

        assert_eq!(cluster.node_count(), 1);
        assert!(cluster.nodes.contains(&node_id));

        cluster.remove_node(node_id);
        assert_eq!(cluster.node_count(), 0);
        assert!(!cluster.nodes.contains(&node_id));
    }

    #[test]
    fn test_node_capacity_calculation() {
        let mut node = ClusterNode::new(
            "test-node".to_string(),
            SocketAddr::from_str("127.0.0.1:8080").unwrap(),
            NodeRole::Worker,
        );

        node.resources = NodeResources {
            total_cpu: 4,
            available_cpu: 2,
            total_memory: 8192,
            available_memory: 4096,
            total_disk: 102400,
            available_disk: 51200,
            cpu_usage: 50.0,
            memory_usage: 50.0,
            disk_usage: 50.0,
        };

        let capacity = node.available_capacity();
        assert_eq!(capacity, 0.5); // 50% available
    }

    #[test]
    fn test_cluster_metrics() {
        let mut metrics = ClusterMetrics::new();
        metrics.total_nodes = 10;
        metrics.healthy_nodes = 8;

        assert_eq!(metrics.health_percentage(), 80.0);
        assert!(metrics.is_healthy());

        metrics.healthy_nodes = 7;
        assert_eq!(metrics.health_percentage(), 70.0);
        assert!(!metrics.is_healthy());
    }

    #[test]
    fn test_load_balancing_strategies() {
        assert_eq!(
            LoadBalancingStrategy::RoundRobin,
            LoadBalancingStrategy::RoundRobin
        );
        assert_ne!(
            LoadBalancingStrategy::RoundRobin,
            LoadBalancingStrategy::LeastLoaded
        );
    }

    #[test]
    fn test_node_status_transitions() {
        let mut node = ClusterNode::new(
            "test-node".to_string(),
            SocketAddr::from_str("127.0.0.1:8080").unwrap(),
            NodeRole::Worker,
        );

        assert_eq!(node.status, NodeStatus::Starting);
        assert!(!node.is_healthy());
        assert!(!node.can_accept_jobs());

        node.status = NodeStatus::Active;
        assert!(node.is_healthy());
        assert!(node.can_accept_jobs());

        node.status = NodeStatus::Draining;
        assert!(!node.is_healthy());
        assert!(!node.can_accept_jobs());
        assert!(node.is_draining());
    }
}
