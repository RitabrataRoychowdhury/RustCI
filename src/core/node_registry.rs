//! Node Registry for cluster coordination
//!
//! This module implements the NodeRegistry for tracking active cluster nodes,
//! managing heartbeats, and handling node failure detection.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

use crate::domain::entities::{ClusterNode, NodeHeartbeat, NodeId, NodeStatus};
use crate::domain::repositories::{ClusterNodeRepository, NodeHeartbeatRepository};
use crate::error::Result;

/// Configuration for the NodeRegistry
#[derive(Debug, Clone)]
pub struct NodeRegistryConfig {
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,
    /// Node failure timeout in seconds
    pub failure_timeout: u64,
    /// Cleanup interval for old heartbeats in seconds
    pub cleanup_interval: u64,
    /// Maximum number of heartbeats to keep per node
    pub max_heartbeats_per_node: usize,
    /// Enable automatic node removal on failure
    pub auto_remove_failed_nodes: bool,
}

impl Default for NodeRegistryConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: 30,
            failure_timeout: 120,
            cleanup_interval: 300, // 5 minutes
            max_heartbeats_per_node: 100,
            auto_remove_failed_nodes: true,
        }
    }
}

/// NodeRegistry manages cluster nodes and their health status
pub struct NodeRegistry {
    /// Configuration
    config: NodeRegistryConfig,
    /// In-memory cache of active nodes
    nodes: Arc<RwLock<HashMap<NodeId, ClusterNode>>>,
    /// Node repository for persistence
    node_repo: Arc<dyn ClusterNodeRepository>,
    /// Heartbeat repository for persistence
    heartbeat_repo: Arc<dyn NodeHeartbeatRepository>,
    /// Failure detection state
    failure_detector: Arc<RwLock<FailureDetector>>,
    /// Registry statistics
    stats: Arc<RwLock<NodeRegistryStats>>,
}

/// Failure detection state and logic
#[derive(Debug)]
struct FailureDetector {
    /// Last heartbeat times for each node
    last_heartbeats: HashMap<NodeId, Instant>,
    /// Failure detection configuration
    config: FailureDetectorConfig,
    /// Suspected failed nodes
    suspected_failures: HashMap<NodeId, Instant>,
}

/// Configuration for failure detection
#[derive(Debug, Clone)]
struct FailureDetectorConfig {
    /// Timeout before considering a node failed
    failure_timeout: Duration,
    /// Grace period before marking as suspected
    suspicion_timeout: Duration,
}

/// Statistics for the NodeRegistry
#[derive(Debug, Clone)]
pub struct NodeRegistryStats {
    /// Total number of registered nodes
    pub total_nodes: u64,
    /// Number of active nodes
    pub active_nodes: u64,
    /// Number of failed nodes
    pub failed_nodes: u64,
    /// Total heartbeats received
    pub total_heartbeats: u64,
    /// Number of node failures detected
    pub failures_detected: u64,
    /// Number of nodes automatically removed
    pub nodes_auto_removed: u64,
    /// Registry uptime
    pub uptime: Duration,
    /// Last cleanup time
    pub last_cleanup: Option<DateTime<Utc>>,
}

impl NodeRegistry {
    /// Create a new NodeRegistry
    pub fn new(
        config: NodeRegistryConfig,
        node_repo: Arc<dyn ClusterNodeRepository>,
        heartbeat_repo: Arc<dyn NodeHeartbeatRepository>,
    ) -> Self {
        let failure_detector_config = FailureDetectorConfig {
            failure_timeout: Duration::from_secs(config.failure_timeout),
            suspicion_timeout: Duration::from_secs(config.failure_timeout / 2),
        };

        Self {
            config,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            node_repo,
            heartbeat_repo,
            failure_detector: Arc::new(RwLock::new(FailureDetector {
                last_heartbeats: HashMap::new(),
                config: failure_detector_config,
                suspected_failures: HashMap::new(),
            })),
            stats: Arc::new(RwLock::new(NodeRegistryStats::new())),
        }
    }

    /// Start the NodeRegistry background tasks
    pub async fn start(&self) -> Result<()> {
        info!("Starting NodeRegistry background tasks");

        // Load existing nodes from repository
        self.load_existing_nodes().await?;

        // Start heartbeat monitoring
        self.start_heartbeat_monitoring().await;

        // Start cleanup task
        self.start_cleanup_task().await;

        info!("NodeRegistry started successfully");
        Ok(())
    }

    /// Register a new node in the cluster
    pub async fn register_node(&self, mut node: ClusterNode) -> Result<ClusterNode> {
        info!("Registering new node: {} ({})", node.name, node.id);

        // Set initial status
        node.status = NodeStatus::Active;
        node.last_heartbeat = Utc::now();

        // Persist to repository
        let saved_node = self.node_repo.create(&node).await?;

        // Add to in-memory cache
        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(saved_node.id, saved_node.clone());
        }

        // Initialize failure detector
        {
            let mut detector = self.failure_detector.write().await;
            detector
                .last_heartbeats
                .insert(saved_node.id, Instant::now());
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_nodes += 1;
            stats.active_nodes += 1;
        }

        info!("Node registered successfully: {}", saved_node.id);
        Ok(saved_node)
    }

    /// Unregister a node from the cluster
    pub async fn unregister_node(&self, node_id: NodeId) -> Result<()> {
        info!("Unregistering node: {}", node_id);

        // Update node status to offline
        if let Err(e) = self
            .node_repo
            .update_status(node_id, NodeStatus::Offline)
            .await
        {
            warn!("Failed to update node status to offline: {}", e);
        }

        // Remove from in-memory cache
        let was_active = {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.remove(&node_id) {
                node.status == NodeStatus::Active
            } else {
                false
            }
        };

        // Remove from failure detector
        {
            let mut detector = self.failure_detector.write().await;
            detector.last_heartbeats.remove(&node_id);
            detector.suspected_failures.remove(&node_id);
        }

        // Update statistics
        if was_active {
            let mut stats = self.stats.write().await;
            stats.active_nodes = stats.active_nodes.saturating_sub(1);
        }

        info!("Node unregistered successfully: {}", node_id);
        Ok(())
    }

    /// Process a heartbeat from a node
    pub async fn process_heartbeat(&self, heartbeat: NodeHeartbeat) -> Result<()> {
        debug!("Processing heartbeat from node: {}", heartbeat.node_id);

        let node_id = heartbeat.node_id;
        let now = Instant::now();

        // Update failure detector
        {
            let mut detector = self.failure_detector.write().await;
            detector.last_heartbeats.insert(node_id, now);
            detector.suspected_failures.remove(&node_id);
        }

        // Update node in cache
        {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(&node_id) {
                node.last_heartbeat = heartbeat.timestamp;
                node.resources = heartbeat.resources.clone();
                node.status = heartbeat.status.clone();
            }
        }

        // Persist heartbeat
        let heartbeat_clone = heartbeat.clone();
        if let Err(e) = self.heartbeat_repo.create(&heartbeat_clone).await {
            warn!("Failed to persist heartbeat for node {}: {}", node_id, e);
        }

        // Update node in repository
        if let Err(e) = self
            .node_repo
            .update_heartbeat(node_id, &heartbeat_clone)
            .await
        {
            warn!("Failed to update node heartbeat in repository: {}", e);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_heartbeats += 1;
        }

        debug!("Heartbeat processed successfully for node: {}", node_id);
        Ok(())
    }

    /// Get all active nodes
    pub async fn get_active_nodes(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes
            .values()
            .filter(|node| node.status == NodeStatus::Active)
            .cloned()
            .collect())
    }

    /// Get all nodes
    pub async fn get_all_nodes(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    /// Get a specific node by ID
    pub async fn get_node(&self, node_id: NodeId) -> Result<Option<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(&node_id).cloned())
    }

    /// Get nodes by status
    pub async fn get_nodes_by_status(&self, status: NodeStatus) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes
            .values()
            .filter(|node| node.status == status)
            .cloned()
            .collect())
    }

    /// Get registry statistics
    pub async fn get_stats(&self) -> NodeRegistryStats {
        self.stats.read().await.clone()
    }

    /// Check if a node exists
    pub async fn node_exists(&self, node_id: NodeId) -> bool {
        let nodes = self.nodes.read().await;
        nodes.contains_key(&node_id)
    }

    /// Get the number of active nodes
    pub async fn active_node_count(&self) -> usize {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|node| node.status == NodeStatus::Active)
            .count()
    }

    /// Load existing nodes from repository
    async fn load_existing_nodes(&self) -> Result<()> {
        info!("Loading existing nodes from repository");

        let existing_nodes = self.node_repo.find_all().await?;
        let mut nodes = self.nodes.write().await;
        let mut detector = self.failure_detector.write().await;
        let mut stats = self.stats.write().await;

        for node in existing_nodes {
            let node_id = node.id;
            let is_active = node.status == NodeStatus::Active;

            nodes.insert(node_id, node);
            detector.last_heartbeats.insert(node_id, Instant::now());

            stats.total_nodes += 1;
            if is_active {
                stats.active_nodes += 1;
            }
        }

        info!("Loaded {} nodes from repository", stats.total_nodes);
        Ok(())
    }

    /// Start heartbeat monitoring task
    async fn start_heartbeat_monitoring(&self) {
        let failure_detector = self.failure_detector.clone();
        let nodes = self.nodes.clone();
        let node_repo = self.node_repo.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.heartbeat_interval));

            loop {
                interval.tick().await;

                if let Err(e) = Self::check_node_failures(
                    &failure_detector,
                    &nodes,
                    &node_repo,
                    &stats,
                    &config,
                )
                .await
                {
                    error!("Error during node failure check: {}", e);
                }
            }
        });
    }

    /// Start cleanup task
    async fn start_cleanup_task(&self) {
        let heartbeat_repo = self.heartbeat_repo.clone();
        let stats = self.stats.clone();
        let cleanup_interval = self.config.cleanup_interval;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(cleanup_interval));

            loop {
                interval.tick().await;

                let cutoff = Utc::now() - chrono::Duration::hours(24); // Keep 24 hours of heartbeats
                match heartbeat_repo.delete_older_than(cutoff).await {
                    Ok(deleted_count) => {
                        debug!("Cleaned up {} old heartbeat records", deleted_count);
                        let mut stats = stats.write().await;
                        stats.last_cleanup = Some(Utc::now());
                    }
                    Err(e) => {
                        error!("Failed to cleanup old heartbeats: {}", e);
                    }
                }
            }
        });
    }

    /// Check for node failures
    async fn check_node_failures(
        failure_detector: &Arc<RwLock<FailureDetector>>,
        nodes: &Arc<RwLock<HashMap<NodeId, ClusterNode>>>,
        node_repo: &Arc<dyn ClusterNodeRepository>,
        stats: &Arc<RwLock<NodeRegistryStats>>,
        config: &NodeRegistryConfig,
    ) -> Result<()> {
        let now = Instant::now();
        let mut failed_nodes = Vec::new();
        let mut suspected_nodes = Vec::new();

        // Check for failures
        {
            let detector = failure_detector.read().await;

            for (&node_id, &last_heartbeat) in &detector.last_heartbeats {
                let time_since_heartbeat = now.duration_since(last_heartbeat);

                if time_since_heartbeat > detector.config.failure_timeout {
                    failed_nodes.push(node_id);
                } else if time_since_heartbeat > detector.config.suspicion_timeout
                    && !detector.suspected_failures.contains_key(&node_id)
                {
                    suspected_nodes.push(node_id);
                }
            }
        }

        // Update suspected failures
        if !suspected_nodes.is_empty() {
            let mut detector = failure_detector.write().await;
            for node_id in &suspected_nodes {
                detector.suspected_failures.insert(*node_id, now);
            }
        }

        // Handle suspected failures
        for node_id in suspected_nodes {
            warn!(
                "Node {} is suspected to have failed (no heartbeat for {:?})",
                node_id,
                failure_detector.read().await.config.suspicion_timeout
            );
        }

        // Handle confirmed failures
        for node_id in failed_nodes {
            error!(
                "Node {} has failed (no heartbeat for {:?})",
                node_id,
                failure_detector.read().await.config.failure_timeout
            );

            // Update node status
            {
                let mut nodes_guard = nodes.write().await;
                if let Some(node) = nodes_guard.get_mut(&node_id) {
                    node.status = NodeStatus::Failed;
                }
            }

            // Update in repository
            if let Err(e) = node_repo.update_status(node_id, NodeStatus::Failed).await {
                error!("Failed to update failed node status in repository: {}", e);
            }

            // Remove from failure detector if auto-removal is enabled
            if config.auto_remove_failed_nodes {
                let mut detector = failure_detector.write().await;
                detector.last_heartbeats.remove(&node_id);
                detector.suspected_failures.remove(&node_id);

                let mut nodes_guard = nodes.write().await;
                nodes_guard.remove(&node_id);

                info!("Automatically removed failed node: {}", node_id);
            }

            // Update statistics
            {
                let mut stats_guard = stats.write().await;
                stats_guard.failures_detected += 1;
                stats_guard.active_nodes = stats_guard.active_nodes.saturating_sub(1);
                stats_guard.failed_nodes += 1;

                if config.auto_remove_failed_nodes {
                    stats_guard.nodes_auto_removed += 1;
                }
            }
        }

        Ok(())
    }
}

impl NodeRegistryStats {
    /// Create new statistics
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            active_nodes: 0,
            failed_nodes: 0,
            total_heartbeats: 0,
            failures_detected: 0,
            nodes_auto_removed: 0,
            uptime: Duration::from_secs(0),
            last_cleanup: None,
        }
    }

    /// Calculate node health percentage
    pub fn health_percentage(&self) -> f64 {
        if self.total_nodes == 0 {
            return 100.0;
        }
        (self.active_nodes as f64 / self.total_nodes as f64) * 100.0
    }

    /// Check if the cluster is healthy
    pub fn is_healthy(&self) -> bool {
        self.health_percentage() >= 80.0
    }
}

impl Default for NodeRegistryStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::domain::entities::{NodeResources, NodeRole};
    use crate::domain::repositories::cluster::HeartbeatStats;
    use std::net::SocketAddr;
    use std::str::FromStr;

    // Mock implementations for testing
    struct MockClusterNodeRepository;
    struct MockNodeHeartbeatRepository;

    #[async_trait::async_trait]
    impl ClusterNodeRepository for MockClusterNodeRepository {
        async fn create(&self, node: &ClusterNode) -> Result<ClusterNode> {
            Ok(node.clone())
        }

        async fn find_by_id(&self, _node_id: NodeId) -> Result<Option<ClusterNode>> {
            Ok(None)
        }

        async fn find_all(&self) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_by_status(&self, _status: NodeStatus) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_by_role(&self, _role: NodeRole) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_by_cluster_id(
            &self,
            _cluster_id: crate::domain::entities::ClusterId,
        ) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_healthy(&self) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_available(&self) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn update(&self, node: &ClusterNode) -> Result<ClusterNode> {
            Ok(node.clone())
        }

        async fn update_status(&self, _node_id: NodeId, _status: NodeStatus) -> Result<()> {
            Ok(())
        }

        async fn update_heartbeat(
            &self,
            _node_id: NodeId,
            _heartbeat: &NodeHeartbeat,
        ) -> Result<()> {
            Ok(())
        }

        async fn delete(&self, _node_id: NodeId) -> Result<()> {
            Ok(())
        }

        async fn exists(&self, _node_id: NodeId) -> Result<bool> {
            Ok(false)
        }

        async fn count(&self) -> Result<u64> {
            Ok(0)
        }

        async fn count_by_status(&self, _status: NodeStatus) -> Result<u64> {
            Ok(0)
        }

        async fn find_with_pagination(
            &self,
            _limit: usize,
            _offset: usize,
        ) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_stale_nodes(&self, _timeout_seconds: u64) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }

        async fn find_least_loaded(&self) -> Result<Option<ClusterNode>> {
            Ok(None)
        }

        async fn find_by_capacity(&self, _ascending: bool) -> Result<Vec<ClusterNode>> {
            Ok(Vec::new())
        }
    }

    #[async_trait::async_trait]
    impl NodeHeartbeatRepository for MockNodeHeartbeatRepository {
        async fn create(&self, heartbeat: &NodeHeartbeat) -> Result<NodeHeartbeat> {
            Ok(heartbeat.clone())
        }

        async fn find_latest_by_node_id(&self, _node_id: NodeId) -> Result<Option<NodeHeartbeat>> {
            Ok(None)
        }

        async fn find_by_node_id_and_time_range(
            &self,
            _node_id: NodeId,
            _start: DateTime<Utc>,
            _end: DateTime<Utc>,
        ) -> Result<Vec<NodeHeartbeat>> {
            Ok(Vec::new())
        }

        async fn find_by_time_range(
            &self,
            _start: DateTime<Utc>,
            _end: DateTime<Utc>,
        ) -> Result<Vec<NodeHeartbeat>> {
            Ok(Vec::new())
        }

        async fn delete_older_than(&self, _cutoff: DateTime<Utc>) -> Result<u64> {
            Ok(0)
        }

        async fn get_node_heartbeat_stats(
            &self,
            _node_id: NodeId,
        ) -> Result<HeartbeatStats> {
            Ok(HeartbeatStats::new(_node_id))
        }

        async fn find_missing_heartbeats(&self, _timeout_seconds: u64) -> Result<Vec<NodeId>> {
            Ok(Vec::new())
        }
    }

    fn create_test_node() -> ClusterNode {
        ClusterNode::new(
            "test-node".to_string(),
            SocketAddr::from_str("127.0.0.1:8080").unwrap(),
            NodeRole::Worker,
        )
    }

    pub fn create_test_registry() -> NodeRegistry {
        let config = NodeRegistryConfig::default();
        let node_repo = Arc::new(MockClusterNodeRepository);
        let heartbeat_repo = Arc::new(MockNodeHeartbeatRepository);

        NodeRegistry::new(config, node_repo, heartbeat_repo)
    }

    #[tokio::test]
    async fn test_node_registry_creation() {
        let registry = create_test_registry();
        let stats = registry.get_stats().await;

        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.active_nodes, 0);
        assert_eq!(stats.failed_nodes, 0);
    }

    #[tokio::test]
    async fn test_node_registration() {
        let registry = create_test_registry();
        let node = create_test_node();
        let node_id = node.id;

        let registered_node = registry.register_node(node).await.unwrap();

        assert_eq!(registered_node.status, NodeStatus::Active);
        assert!(registry.node_exists(node_id).await);
        assert_eq!(registry.active_node_count().await, 1);
    }

    #[tokio::test]
    async fn test_node_unregistration() {
        let registry = create_test_registry();
        let node = create_test_node();
        let node_id = node.id;

        registry.register_node(node).await.unwrap();
        assert!(registry.node_exists(node_id).await);

        registry.unregister_node(node_id).await.unwrap();
        assert!(!registry.node_exists(node_id).await);
        assert_eq!(registry.active_node_count().await, 0);
    }

    #[tokio::test]
    async fn test_heartbeat_processing() {
        let registry = create_test_registry();
        let node = create_test_node();
        let node_id = node.id;

        registry.register_node(node).await.unwrap();

        let heartbeat = NodeHeartbeat {
            node_id,
            timestamp: Utc::now(),
            status: NodeStatus::Active,
            resources: NodeResources::default(),
            active_runners: 2,
            running_jobs: 5,
            metrics: std::collections::HashMap::new(),
        };

        registry.process_heartbeat(heartbeat).await.unwrap();

        let stats = registry.get_stats().await;
        assert_eq!(stats.total_heartbeats, 1);
    }

    #[tokio::test]
    async fn test_get_active_nodes() {
        let registry = create_test_registry();
        let node1 = create_test_node();
        let mut node2 = create_test_node();
        node2.status = NodeStatus::Failed;

        registry.register_node(node1).await.unwrap();
        registry.register_node(node2).await.unwrap();

        let active_nodes = registry.get_active_nodes().await.unwrap();
        assert_eq!(active_nodes.len(), 2); // Both registered as active initially
    }

    #[tokio::test]
    async fn test_stats_health_calculation() {
        let mut stats = NodeRegistryStats::new();
        stats.total_nodes = 10;
        stats.active_nodes = 8;

        assert_eq!(stats.health_percentage(), 80.0);
        assert!(stats.is_healthy());

        stats.active_nodes = 7;
        assert_eq!(stats.health_percentage(), 70.0);
        assert!(!stats.is_healthy());
    }
}
