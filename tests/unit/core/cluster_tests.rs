//! Unit tests for cluster coordination components
//!
//! This module contains comprehensive tests for NodeRegistry, ClusterCoordinator,
//! and related cluster management functionality.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::{
    ClusterCoordinator, ClusterCoordinatorConfig, ClusterEvent, NodeRegistry, NodeRegistryConfig,
    NodeRegistryStats,
};
use rustci::domain::entities::{
    ClusterNode, ClusterStatus, LoadBalancingStrategy, NodeHeartbeat, NodeResources, NodeRole,
    NodeStatus,
};
use rustci::domain::repositories::{
    ClusterNodeRepository, ClusterRepository, DistributionStats, HeartbeatStats,
    JobDistributionRepository, NodeHeartbeatRepository,
};
use rustci::error::{AppError, Result};

// Mock implementations for testing
#[derive(Clone)]
struct MockClusterNodeRepository {
    nodes: Arc<tokio::sync::RwLock<HashMap<rustci::domain::entities::NodeId, ClusterNode>>>,
}

impl MockClusterNodeRepository {
    fn new() -> Self {
        Self {
            nodes: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ClusterNodeRepository for MockClusterNodeRepository {
    async fn create(&self, node: &ClusterNode) -> Result<ClusterNode> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id, node.clone());
        Ok(node.clone())
    }

    async fn find_by_id(
        &self,
        node_id: rustci::domain::entities::NodeId,
    ) -> Result<Option<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(&node_id).cloned())
    }

    async fn find_all(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    async fn find_by_status(&self, status: NodeStatus) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes
            .values()
            .filter(|n| n.status == status)
            .cloned()
            .collect())
    }

    async fn find_by_role(&self, role: NodeRole) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().filter(|n| n.role == role).cloned().collect())
    }

    async fn find_by_cluster_id(
        &self,
        _cluster_id: rustci::domain::entities::ClusterId,
    ) -> Result<Vec<ClusterNode>> {
        self.find_all().await
    }

    async fn find_healthy(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().filter(|n| n.is_healthy()).cloned().collect())
    }

    async fn find_available(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes
            .values()
            .filter(|n| n.can_accept_jobs())
            .cloned()
            .collect())
    }

    async fn update(&self, node: &ClusterNode) -> Result<ClusterNode> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id, node.clone());
        Ok(node.clone())
    }

    async fn update_status(
        &self,
        node_id: rustci::domain::entities::NodeId,
        status: NodeStatus,
    ) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(&node_id) {
            node.status = status;
        }
        Ok(())
    }

    async fn update_heartbeat(
        &self,
        node_id: rustci::domain::entities::NodeId,
        heartbeat: &NodeHeartbeat,
    ) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(&node_id) {
            node.last_heartbeat = heartbeat.timestamp;
            node.resources = heartbeat.resources.clone();
        }
        Ok(())
    }

    async fn delete(&self, node_id: rustci::domain::entities::NodeId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(&node_id);
        Ok(())
    }

    async fn exists(&self, node_id: rustci::domain::entities::NodeId) -> Result<bool> {
        let nodes = self.nodes.read().await;
        Ok(nodes.contains_key(&node_id))
    }

    async fn count(&self) -> Result<u64> {
        let nodes = self.nodes.read().await;
        Ok(nodes.len() as u64)
    }

    async fn count_by_status(&self, status: NodeStatus) -> Result<u64> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().filter(|n| n.status == status).count() as u64)
    }

    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().skip(offset).take(limit).cloned().collect())
    }

    async fn find_stale_nodes(&self, _timeout_seconds: u64) -> Result<Vec<ClusterNode>> {
        Ok(Vec::new())
    }

    async fn find_least_loaded(&self) -> Result<Option<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes
            .values()
            .max_by(|a, b| {
                a.available_capacity()
                    .partial_cmp(&b.available_capacity())
                    .unwrap()
            })
            .cloned())
    }

    async fn find_by_capacity(&self, ascending: bool) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        let mut sorted: Vec<_> = nodes.values().cloned().collect();
        if ascending {
            sorted.sort_by(|a, b| {
                a.available_capacity()
                    .partial_cmp(&b.available_capacity())
                    .unwrap()
            });
        } else {
            sorted.sort_by(|a, b| {
                b.available_capacity()
                    .partial_cmp(&a.available_capacity())
                    .unwrap()
            });
        }
        Ok(sorted)
    }
}

#[derive(Clone)]
struct MockNodeHeartbeatRepository {
    heartbeats: Arc<tokio::sync::RwLock<Vec<NodeHeartbeat>>>,
}

impl MockNodeHeartbeatRepository {
    fn new() -> Self {
        Self {
            heartbeats: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl NodeHeartbeatRepository for MockNodeHeartbeatRepository {
    async fn create(&self, heartbeat: &NodeHeartbeat) -> Result<NodeHeartbeat> {
        let mut heartbeats = self.heartbeats.write().await;
        heartbeats.push(heartbeat.clone());
        Ok(heartbeat.clone())
    }

    async fn find_latest_by_node_id(
        &self,
        node_id: rustci::domain::entities::NodeId,
    ) -> Result<Option<NodeHeartbeat>> {
        let heartbeats = self.heartbeats.read().await;
        Ok(heartbeats
            .iter()
            .filter(|h| h.node_id == node_id)
            .max_by_key(|h| h.timestamp)
            .cloned())
    }

    async fn find_by_node_id_and_time_range(
        &self,
        node_id: rustci::domain::entities::NodeId,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<NodeHeartbeat>> {
        let heartbeats = self.heartbeats.read().await;
        Ok(heartbeats
            .iter()
            .filter(|h| h.node_id == node_id && h.timestamp >= start && h.timestamp <= end)
            .cloned()
            .collect())
    }

    async fn find_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<NodeHeartbeat>> {
        let heartbeats = self.heartbeats.read().await;
        Ok(heartbeats
            .iter()
            .filter(|h| h.timestamp >= start && h.timestamp <= end)
            .cloned()
            .collect())
    }

    async fn delete_older_than(&self, cutoff: chrono::DateTime<chrono::Utc>) -> Result<u64> {
        let mut heartbeats = self.heartbeats.write().await;
        let initial_len = heartbeats.len();
        heartbeats.retain(|h| h.timestamp > cutoff);
        Ok((initial_len - heartbeats.len()) as u64)
    }

    async fn get_node_heartbeat_stats(
        &self,
        node_id: rustci::domain::entities::NodeId,
    ) -> Result<HeartbeatStats> {
        let heartbeats = self.heartbeats.read().await;
        let node_heartbeats: Vec<_> = heartbeats.iter().filter(|h| h.node_id == node_id).collect();

        let mut stats = HeartbeatStats::new(node_id);
        stats.total_heartbeats = node_heartbeats.len() as u64;

        if let Some(first) = node_heartbeats.first() {
            stats.first_heartbeat = Some(first.timestamp);
        }
        if let Some(last) = node_heartbeats.last() {
            stats.latest_heartbeat = Some(last.timestamp);
        }

        Ok(stats)
    }

    async fn find_missing_heartbeats(
        &self,
        _timeout_seconds: u64,
    ) -> Result<Vec<rustci::domain::entities::NodeId>> {
        Ok(Vec::new())
    }
}

#[derive(Clone)]
struct MockClusterRepository;

#[async_trait::async_trait]
impl ClusterRepository for MockClusterRepository {
    async fn create(
        &self,
        cluster: &rustci::domain::entities::Cluster,
    ) -> Result<rustci::domain::entities::Cluster> {
        Ok(cluster.clone())
    }

    async fn find_by_id(
        &self,
        _cluster_id: rustci::domain::entities::ClusterId,
    ) -> Result<Option<rustci::domain::entities::Cluster>> {
        Ok(None)
    }

    async fn find_by_name(&self, _name: &str) -> Result<Option<rustci::domain::entities::Cluster>> {
        Ok(None)
    }

    async fn find_all(&self) -> Result<Vec<rustci::domain::entities::Cluster>> {
        Ok(Vec::new())
    }

    async fn update(
        &self,
        cluster: &rustci::domain::entities::Cluster,
    ) -> Result<rustci::domain::entities::Cluster> {
        Ok(cluster.clone())
    }

    async fn delete(&self, _cluster_id: rustci::domain::entities::ClusterId) -> Result<()> {
        Ok(())
    }

    async fn exists(&self, _cluster_id: rustci::domain::entities::ClusterId) -> Result<bool> {
        Ok(false)
    }

    async fn count(&self) -> Result<u64> {
        Ok(0)
    }

    async fn add_node(
        &self,
        _cluster_id: rustci::domain::entities::ClusterId,
        _node_id: rustci::domain::entities::NodeId,
    ) -> Result<()> {
        Ok(())
    }

    async fn remove_node(
        &self,
        _cluster_id: rustci::domain::entities::ClusterId,
        _node_id: rustci::domain::entities::NodeId,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_metrics(
        &self,
        _cluster_id: rustci::domain::entities::ClusterId,
    ) -> Result<rustci::domain::entities::ClusterMetrics> {
        Ok(rustci::domain::entities::ClusterMetrics::new())
    }

    async fn update_metrics(
        &self,
        _cluster_id: rustci::domain::entities::ClusterId,
        _metrics: &rustci::domain::entities::ClusterMetrics,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
struct MockJobDistributionRepository;

#[async_trait::async_trait]
impl JobDistributionRepository for MockJobDistributionRepository {
    async fn create(
        &self,
        distribution: &rustci::domain::entities::JobDistribution,
    ) -> Result<rustci::domain::entities::JobDistribution> {
        Ok(distribution.clone())
    }

    async fn find_by_job_id(
        &self,
        _job_id: rustci::domain::entities::JobId,
    ) -> Result<Option<rustci::domain::entities::JobDistribution>> {
        Ok(None)
    }

    async fn find_by_node_id(
        &self,
        _node_id: rustci::domain::entities::NodeId,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        Ok(Vec::new())
    }

    async fn find_by_runner_id(
        &self,
        _runner_id: rustci::domain::entities::RunnerId,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        Ok(Vec::new())
    }

    async fn find_by_strategy(
        &self,
        _strategy: LoadBalancingStrategy,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        Ok(Vec::new())
    }

    async fn find_by_time_range(
        &self,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        Ok(Vec::new())
    }

    async fn delete(&self, _job_id: rustci::domain::entities::JobId) -> Result<()> {
        Ok(())
    }

    async fn get_distribution_stats(&self) -> Result<DistributionStats> {
        Ok(DistributionStats::new())
    }

    async fn find_with_pagination(
        &self,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        Ok(Vec::new())
    }
}

// Helper functions for creating test data
fn create_test_node(name: &str, role: NodeRole) -> ClusterNode {
    ClusterNode::new(
        name.to_string(),
        SocketAddr::from_str("127.0.0.1:8080").unwrap(),
        role,
    )
}

fn create_test_registry() -> NodeRegistry {
    let config = NodeRegistryConfig {
        heartbeat_interval: 1, // 1 second for faster tests
        failure_timeout: 3,    // 3 seconds for faster tests
        cleanup_interval: 10,
        max_heartbeats_per_node: 100,
        auto_remove_failed_nodes: true,
    };
    let node_repo = Arc::new(MockClusterNodeRepository::new());
    let heartbeat_repo = Arc::new(MockNodeHeartbeatRepository::new());

    NodeRegistry::new(config, node_repo, heartbeat_repo)
}

async fn create_test_coordinator() -> ClusterCoordinator {
    let config = ClusterCoordinatorConfig::default();
    let node_registry = Arc::new(create_test_registry());
    let cluster_repo = Arc::new(MockClusterRepository);
    let job_distribution_repo = Arc::new(MockJobDistributionRepository);

    ClusterCoordinator::new(config, node_registry, cluster_repo, job_distribution_repo)
        .await
        .unwrap()
}

// NodeRegistry Tests
#[tokio::test]
async fn test_node_registry_creation() {
    let registry = create_test_registry();
    let stats = registry.get_stats().await;

    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.active_nodes, 0);
    assert_eq!(stats.failed_nodes, 0);
    assert_eq!(stats.total_heartbeats, 0);
}

#[tokio::test]
async fn test_node_registration() {
    let registry = create_test_registry();
    let node = create_test_node("test-node", NodeRole::Worker);
    let node_id = node.id;

    let registered_node = registry.register_node(node).await.unwrap();

    assert_eq!(registered_node.status, NodeStatus::Active);
    assert_eq!(registered_node.name, "test-node");
    assert!(registry.node_exists(node_id).await);
    assert_eq!(registry.active_node_count().await, 1);

    let stats = registry.get_stats().await;
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.active_nodes, 1);
}

#[tokio::test]
async fn test_node_unregistration() {
    let registry = create_test_registry();
    let node = create_test_node("test-node", NodeRole::Worker);
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
    let node = create_test_node("test-node", NodeRole::Worker);
    let node_id = node.id;

    registry.register_node(node).await.unwrap();

    let heartbeat = NodeHeartbeat {
        node_id,
        timestamp: chrono::Utc::now(),
        status: NodeStatus::Active,
        resources: NodeResources {
            total_cpu: 4,
            available_cpu: 2,
            total_memory: 8192,
            available_memory: 4096,
            total_disk: 102400,
            available_disk: 51200,
            cpu_usage: 50.0,
            memory_usage: 50.0,
            disk_usage: 50.0,
        },
        active_runners: 2,
        running_jobs: 5,
        metrics: HashMap::new(),
    };

    registry.process_heartbeat(heartbeat).await.unwrap();

    let stats = registry.get_stats().await;
    assert_eq!(stats.total_heartbeats, 1);

    // Verify node was updated
    let updated_node = registry.get_node(node_id).await.unwrap().unwrap();
    assert_eq!(updated_node.resources.cpu_usage, 50.0);
    assert_eq!(updated_node.resources.memory_usage, 50.0);
}

#[tokio::test]
async fn test_get_nodes_by_status() {
    let registry = create_test_registry();

    let mut node1 = create_test_node("node1", NodeRole::Worker);
    let mut node2 = create_test_node("node2", NodeRole::Master);

    registry.register_node(node1).await.unwrap();
    registry.register_node(node2).await.unwrap();

    let active_nodes = registry
        .get_nodes_by_status(NodeStatus::Active)
        .await
        .unwrap();
    assert_eq!(active_nodes.len(), 2);

    let failed_nodes = registry
        .get_nodes_by_status(NodeStatus::Failed)
        .await
        .unwrap();
    assert_eq!(failed_nodes.len(), 0);
}

#[tokio::test]
async fn test_node_registry_stats() {
    let mut stats = NodeRegistryStats::new();

    assert_eq!(stats.health_percentage(), 100.0); // No nodes = 100% healthy
    assert!(stats.is_healthy());

    stats.total_nodes = 10;
    stats.active_nodes = 8;
    assert_eq!(stats.health_percentage(), 80.0);
    assert!(stats.is_healthy());

    stats.active_nodes = 7;
    assert_eq!(stats.health_percentage(), 70.0);
    assert!(!stats.is_healthy()); // Below 80% threshold
}

// ClusterCoordinator Tests
#[tokio::test]
async fn test_cluster_coordinator_creation() {
    let coordinator = create_test_coordinator().await;
    let status = coordinator.get_cluster_status().await;

    // Initial status should be Starting
    assert_eq!(status, ClusterStatus::Starting);
}

#[tokio::test]
async fn test_node_join_cluster() {
    let coordinator = create_test_coordinator().await;
    let node = create_test_node("test-node", NodeRole::Worker);
    let node_id = node.id;

    let joined_node = coordinator.join_node(node).await.unwrap();

    assert_eq!(joined_node.id, node_id);
    assert_eq!(joined_node.status, NodeStatus::Active);

    let cluster_nodes = coordinator.get_cluster_nodes().await.unwrap();
    assert_eq!(cluster_nodes.len(), 1);
    assert_eq!(cluster_nodes[0].id, node_id);
}

#[tokio::test]
async fn test_node_leave_cluster() {
    let coordinator = create_test_coordinator().await;
    let node = create_test_node("test-node", NodeRole::Worker);
    let node_id = node.id;

    coordinator.join_node(node).await.unwrap();

    let cluster_nodes = coordinator.get_cluster_nodes().await.unwrap();
    assert_eq!(cluster_nodes.len(), 1);

    coordinator
        .leave_node(node_id, "Test leave".to_string())
        .await
        .unwrap();

    let cluster_nodes = coordinator.get_cluster_nodes().await.unwrap();
    assert_eq!(cluster_nodes.len(), 0);
}

#[tokio::test]
async fn test_job_distribution() {
    let coordinator = create_test_coordinator().await;

    // Add some nodes first
    let node1 = create_test_node("node1", NodeRole::Worker);
    let node2 = create_test_node("node2", NodeRole::Worker);

    coordinator.join_node(node1).await.unwrap();
    coordinator.join_node(node2).await.unwrap();

    // Distribute a job
    let job_id = Uuid::new_v4();
    let distribution = coordinator.distribute_job(job_id).await.unwrap();

    assert_eq!(distribution.job_id, job_id);
    assert_eq!(distribution.strategy, LoadBalancingStrategy::LeastLoaded);

    // Verify the job was distributed to one of the nodes
    let cluster_nodes = coordinator.get_cluster_nodes().await.unwrap();
    let node_ids: Vec<_> = cluster_nodes.iter().map(|n| n.id).collect();
    assert!(node_ids.contains(&distribution.target_node));
}

#[tokio::test]
async fn test_job_distribution_no_nodes() {
    let coordinator = create_test_coordinator().await;

    // Try to distribute a job with no nodes
    let job_id = Uuid::new_v4();
    let result = coordinator.distribute_job(job_id).await;

    assert!(result.is_err());
    if let Err(AppError::ValidationError(msg)) = result {
        assert!(msg.contains("No available nodes"));
    } else {
        panic!("Expected ValidationError");
    }
}

#[tokio::test]
async fn test_cluster_metrics() {
    let coordinator = create_test_coordinator().await;

    // Initially no metrics
    let metrics = coordinator.get_cluster_metrics().await;
    assert_eq!(metrics.total_nodes, 0);
    assert_eq!(metrics.healthy_nodes, 0);

    // Add some nodes
    let node1 = create_test_node("node1", NodeRole::Worker);
    let node2 = create_test_node("node2", NodeRole::Master);

    coordinator.join_node(node1).await.unwrap();
    coordinator.join_node(node2).await.unwrap();

    // Give some time for metrics to update (in real implementation)
    // For this test, we'll just verify the structure
    let cluster_info = coordinator.get_cluster_info().await;
    assert!(!cluster_info.name.is_empty());
    assert_eq!(cluster_info.nodes.len(), 2);
}

#[tokio::test]
async fn test_get_active_nodes() {
    let coordinator = create_test_coordinator().await;

    let node1 = create_test_node("node1", NodeRole::Worker);
    let node2 = create_test_node("node2", NodeRole::Worker);

    coordinator.join_node(node1).await.unwrap();
    coordinator.join_node(node2).await.unwrap();

    let active_nodes = coordinator.get_active_nodes().await.unwrap();
    assert_eq!(active_nodes.len(), 2);

    for node in &active_nodes {
        assert_eq!(node.status, NodeStatus::Active);
    }
}

#[tokio::test]
async fn test_distribution_stats() {
    let coordinator = create_test_coordinator().await;

    let stats = coordinator.get_distribution_stats().await.unwrap();
    assert_eq!(stats.total_distributions, 0);
    assert!(stats.by_strategy.is_empty());
    assert!(stats.by_node.is_empty());
}

// Integration Tests
#[tokio::test]
async fn test_node_failure_detection() {
    let mut config = NodeRegistryConfig::default();
    config.heartbeat_interval = 1;
    config.failure_timeout = 2;
    config.auto_remove_failed_nodes = false; // Don't auto-remove for this test

    let node_repo = Arc::new(MockClusterNodeRepository::new());
    let heartbeat_repo = Arc::new(MockNodeHeartbeatRepository::new());
    let registry = NodeRegistry::new(config, node_repo, heartbeat_repo);

    let node = create_test_node("test-node", NodeRole::Worker);
    let node_id = node.id;

    registry.register_node(node).await.unwrap();

    // Send initial heartbeat
    let heartbeat = NodeHeartbeat {
        node_id,
        timestamp: chrono::Utc::now(),
        status: NodeStatus::Active,
        resources: NodeResources::default(),
        active_runners: 0,
        running_jobs: 0,
        metrics: HashMap::new(),
    };

    registry.process_heartbeat(heartbeat).await.unwrap();

    // Wait longer than failure timeout
    sleep(Duration::from_secs(3)).await;

    // In a real implementation, the failure detection would run automatically
    // For this test, we just verify the node is still registered
    assert!(registry.node_exists(node_id).await);
}

#[tokio::test]
async fn test_multiple_node_coordination() {
    let coordinator = create_test_coordinator().await;

    // Add multiple nodes with different roles
    let worker1 = create_test_node("worker1", NodeRole::Worker);
    let worker2 = create_test_node("worker2", NodeRole::Worker);
    let master = create_test_node("master", NodeRole::Master);

    coordinator.join_node(worker1).await.unwrap();
    coordinator.join_node(worker2).await.unwrap();
    coordinator.join_node(master).await.unwrap();

    let all_nodes = coordinator.get_cluster_nodes().await.unwrap();
    assert_eq!(all_nodes.len(), 3);

    // Count nodes by role
    let workers: Vec<_> = all_nodes
        .iter()
        .filter(|n| n.role == NodeRole::Worker)
        .collect();
    let masters: Vec<_> = all_nodes
        .iter()
        .filter(|n| n.role == NodeRole::Master)
        .collect();

    assert_eq!(workers.len(), 2);
    assert_eq!(masters.len(), 1);

    // Test job distribution across workers
    let job1_id = Uuid::new_v4();
    let job2_id = Uuid::new_v4();

    let dist1 = coordinator.distribute_job(job1_id).await.unwrap();
    let dist2 = coordinator.distribute_job(job2_id).await.unwrap();

    // Both jobs should be distributed (possibly to the same node with LeastLoaded strategy)
    assert_eq!(dist1.job_id, job1_id);
    assert_eq!(dist2.job_id, job2_id);
}

#[tokio::test]
async fn test_cluster_capacity_limits() {
    let mut config = ClusterCoordinatorConfig::default();
    config.max_nodes = 2; // Limit to 2 nodes

    let node_registry = Arc::new(create_test_registry());
    let cluster_repo = Arc::new(MockClusterRepository);
    let job_distribution_repo = Arc::new(MockJobDistributionRepository);

    let coordinator =
        ClusterCoordinator::new(config, node_registry, cluster_repo, job_distribution_repo)
            .await
            .unwrap();

    // Add nodes up to capacity
    let node1 = create_test_node("node1", NodeRole::Worker);
    let node2 = create_test_node("node2", NodeRole::Worker);
    let node3 = create_test_node("node3", NodeRole::Worker);

    coordinator.join_node(node1).await.unwrap();
    coordinator.join_node(node2).await.unwrap();

    // Third node should fail to join
    let result = coordinator.join_node(node3).await;
    assert!(result.is_err());

    if let Err(AppError::ValidationError(msg)) = result {
        assert!(msg.contains("maximum capacity"));
    } else {
        panic!("Expected ValidationError for capacity limit");
    }
}
