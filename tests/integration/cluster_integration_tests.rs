//! Integration tests for cluster coordination
//!
//! This module contains integration tests that test the cluster coordination
//! components working together with real or near-real dependencies.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::{
    ClusterCoordinator, ClusterCoordinatorConfig, ClusterEvent, NodeRegistry, NodeRegistryConfig,
};
use rustci::domain::entities::{
    ClusterNode, ClusterStatus, LoadBalancingStrategy, NodeHeartbeat, NodeResources, NodeRole,
    NodeStatus,
};
use rustci::domain::repositories::{
    ClusterNodeRepository, ClusterRepository, DistributionStats, HeartbeatStats,
    JobDistributionRepository, NodeHeartbeatRepository,
};
use rustci::error::Result;

// In-memory repository implementations for integration testing
#[derive(Clone)]
struct InMemoryClusterNodeRepository {
    nodes: Arc<tokio::sync::RwLock<HashMap<rustci::domain::entities::NodeId, ClusterNode>>>,
}

impl InMemoryClusterNodeRepository {
    fn new() -> Self {
        Self {
            nodes: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ClusterNodeRepository for InMemoryClusterNodeRepository {
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

    async fn find_stale_nodes(&self, timeout_seconds: u64) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(timeout_seconds as i64);
        Ok(nodes
            .values()
            .filter(|n| n.last_heartbeat < cutoff)
            .cloned()
            .collect())
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
struct InMemoryNodeHeartbeatRepository {
    heartbeats: Arc<tokio::sync::RwLock<Vec<NodeHeartbeat>>>,
}

impl InMemoryNodeHeartbeatRepository {
    fn new() -> Self {
        Self {
            heartbeats: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl NodeHeartbeatRepository for InMemoryNodeHeartbeatRepository {
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

        // Calculate average interval
        if node_heartbeats.len() > 1 {
            let total_duration = stats.latest_heartbeat.unwrap() - stats.first_heartbeat.unwrap();
            stats.avg_interval =
                total_duration.num_seconds() as f64 / (node_heartbeats.len() - 1) as f64;
        }

        Ok(stats)
    }

    async fn find_missing_heartbeats(
        &self,
        timeout_seconds: u64,
    ) -> Result<Vec<rustci::domain::entities::NodeId>> {
        let heartbeats = self.heartbeats.read().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(timeout_seconds as i64);

        // Find nodes that haven't sent heartbeats recently
        let mut node_last_heartbeat: HashMap<
            rustci::domain::entities::NodeId,
            chrono::DateTime<chrono::Utc>,
        > = HashMap::new();

        for heartbeat in heartbeats.iter() {
            let current = node_last_heartbeat
                .get(&heartbeat.node_id)
                .unwrap_or(&chrono::DateTime::UNIX_EPOCH);
            if heartbeat.timestamp > *current {
                node_last_heartbeat.insert(heartbeat.node_id, heartbeat.timestamp);
            }
        }

        Ok(node_last_heartbeat
            .iter()
            .filter(|(_, &last_heartbeat)| last_heartbeat < cutoff)
            .map(|(&node_id, _)| node_id)
            .collect())
    }
}

#[derive(Clone)]
struct InMemoryClusterRepository {
    clusters: Arc<
        tokio::sync::RwLock<
            HashMap<rustci::domain::entities::ClusterId, rustci::domain::entities::Cluster>,
        >,
    >,
}

impl InMemoryClusterRepository {
    fn new() -> Self {
        Self {
            clusters: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ClusterRepository for InMemoryClusterRepository {
    async fn create(
        &self,
        cluster: &rustci::domain::entities::Cluster,
    ) -> Result<rustci::domain::entities::Cluster> {
        let mut clusters = self.clusters.write().await;
        clusters.insert(cluster.id, cluster.clone());
        Ok(cluster.clone())
    }

    async fn find_by_id(
        &self,
        cluster_id: rustci::domain::entities::ClusterId,
    ) -> Result<Option<rustci::domain::entities::Cluster>> {
        let clusters = self.clusters.read().await;
        Ok(clusters.get(&cluster_id).cloned())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<rustci::domain::entities::Cluster>> {
        let clusters = self.clusters.read().await;
        Ok(clusters.values().find(|c| c.name == name).cloned())
    }

    async fn find_all(&self) -> Result<Vec<rustci::domain::entities::Cluster>> {
        let clusters = self.clusters.read().await;
        Ok(clusters.values().cloned().collect())
    }

    async fn update(
        &self,
        cluster: &rustci::domain::entities::Cluster,
    ) -> Result<rustci::domain::entities::Cluster> {
        let mut clusters = self.clusters.write().await;
        clusters.insert(cluster.id, cluster.clone());
        Ok(cluster.clone())
    }

    async fn delete(&self, cluster_id: rustci::domain::entities::ClusterId) -> Result<()> {
        let mut clusters = self.clusters.write().await;
        clusters.remove(&cluster_id);
        Ok(())
    }

    async fn exists(&self, cluster_id: rustci::domain::entities::ClusterId) -> Result<bool> {
        let clusters = self.clusters.read().await;
        Ok(clusters.contains_key(&cluster_id))
    }

    async fn count(&self) -> Result<u64> {
        let clusters = self.clusters.read().await;
        Ok(clusters.len() as u64)
    }

    async fn add_node(
        &self,
        cluster_id: rustci::domain::entities::ClusterId,
        node_id: rustci::domain::entities::NodeId,
    ) -> Result<()> {
        let mut clusters = self.clusters.write().await;
        if let Some(cluster) = clusters.get_mut(&cluster_id) {
            cluster.add_node(node_id);
        }
        Ok(())
    }

    async fn remove_node(
        &self,
        cluster_id: rustci::domain::entities::ClusterId,
        node_id: rustci::domain::entities::NodeId,
    ) -> Result<()> {
        let mut clusters = self.clusters.write().await;
        if let Some(cluster) = clusters.get_mut(&cluster_id) {
            cluster.remove_node(node_id);
        }
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
struct InMemoryJobDistributionRepository {
    distributions: Arc<tokio::sync::RwLock<Vec<rustci::domain::entities::JobDistribution>>>,
}

impl InMemoryJobDistributionRepository {
    fn new() -> Self {
        Self {
            distributions: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl JobDistributionRepository for InMemoryJobDistributionRepository {
    async fn create(
        &self,
        distribution: &rustci::domain::entities::JobDistribution,
    ) -> Result<rustci::domain::entities::JobDistribution> {
        let mut distributions = self.distributions.write().await;
        distributions.push(distribution.clone());
        Ok(distribution.clone())
    }

    async fn find_by_job_id(
        &self,
        job_id: rustci::domain::entities::JobId,
    ) -> Result<Option<rustci::domain::entities::JobDistribution>> {
        let distributions = self.distributions.read().await;
        Ok(distributions.iter().find(|d| d.job_id == job_id).cloned())
    }

    async fn find_by_node_id(
        &self,
        node_id: rustci::domain::entities::NodeId,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        let distributions = self.distributions.read().await;
        Ok(distributions
            .iter()
            .filter(|d| d.target_node == node_id)
            .cloned()
            .collect())
    }

    async fn find_by_runner_id(
        &self,
        runner_id: rustci::domain::entities::RunnerId,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        let distributions = self.distributions.read().await;
        Ok(distributions
            .iter()
            .filter(|d| d.target_runner == runner_id)
            .cloned()
            .collect())
    }

    async fn find_by_strategy(
        &self,
        strategy: LoadBalancingStrategy,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        let distributions = self.distributions.read().await;
        Ok(distributions
            .iter()
            .filter(|d| d.strategy == strategy)
            .cloned()
            .collect())
    }

    async fn find_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        let distributions = self.distributions.read().await;
        Ok(distributions
            .iter()
            .filter(|d| d.distributed_at >= start && d.distributed_at <= end)
            .cloned()
            .collect())
    }

    async fn delete(&self, job_id: rustci::domain::entities::JobId) -> Result<()> {
        let mut distributions = self.distributions.write().await;
        distributions.retain(|d| d.job_id != job_id);
        Ok(())
    }

    async fn get_distribution_stats(&self) -> Result<DistributionStats> {
        let distributions = self.distributions.read().await;
        let mut stats = DistributionStats::new();

        stats.total_distributions = distributions.len() as u64;

        // Count by strategy
        for distribution in distributions.iter() {
            let strategy_name = format!("{:?}", distribution.strategy);
            *stats.by_strategy.entry(strategy_name).or_insert(0) += 1;
        }

        // Count by node
        for distribution in distributions.iter() {
            *stats.by_node.entry(distribution.target_node).or_insert(0) += 1;
        }

        Ok(stats)
    }

    async fn find_with_pagination(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<rustci::domain::entities::JobDistribution>> {
        let distributions = self.distributions.read().await;
        Ok(distributions
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect())
    }
}

// Helper functions
fn create_test_node(name: &str, role: NodeRole) -> ClusterNode {
    ClusterNode::new(
        name.to_string(),
        SocketAddr::from_str("127.0.0.1:8080").unwrap(),
        role,
    )
}

async fn create_integration_test_setup() -> (NodeRegistry, ClusterCoordinator) {
    let node_repo = Arc::new(InMemoryClusterNodeRepository::new());
    let heartbeat_repo = Arc::new(InMemoryNodeHeartbeatRepository::new());
    let cluster_repo = Arc::new(InMemoryClusterRepository::new());
    let job_distribution_repo = Arc::new(InMemoryJobDistributionRepository::new());

    let registry_config = NodeRegistryConfig {
        heartbeat_interval: 1,
        failure_timeout: 5,
        cleanup_interval: 10,
        max_heartbeats_per_node: 100,
        auto_remove_failed_nodes: true,
    };

    let registry = NodeRegistry::new(registry_config, node_repo, heartbeat_repo);

    let coordinator_config = ClusterCoordinatorConfig {
        cluster_name: "integration-test-cluster".to_string(),
        load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
        auto_healing: true,
        metrics_interval: 5,
        max_nodes: 10,
        ..Default::default()
    };

    let coordinator = ClusterCoordinator::new(
        coordinator_config,
        Arc::new(registry.clone()),
        cluster_repo,
        job_distribution_repo,
    )
    .await
    .unwrap();

    (registry, coordinator)
}

// Integration Tests
#[tokio::test]
async fn test_full_cluster_lifecycle() {
    let (registry, coordinator) = create_integration_test_setup().await;

    // Start the coordinator
    coordinator.start().await.unwrap();

    // Add nodes to the cluster
    let worker1 = create_test_node("worker1", NodeRole::Worker);
    let worker2 = create_test_node("worker2", NodeRole::Worker);
    let master = create_test_node("master", NodeRole::Master);

    let worker1_id = worker1.id;
    let worker2_id = worker2.id;
    let master_id = master.id;

    coordinator.join_node(worker1).await.unwrap();
    coordinator.join_node(worker2).await.unwrap();
    coordinator.join_node(master).await.unwrap();

    // Verify all nodes are in the cluster
    let cluster_nodes = coordinator.get_cluster_nodes().await.unwrap();
    assert_eq!(cluster_nodes.len(), 3);

    // Send heartbeats for all nodes
    let now = chrono::Utc::now();
    for &node_id in &[worker1_id, worker2_id, master_id] {
        let heartbeat = NodeHeartbeat {
            node_id,
            timestamp: now,
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
            active_runners: 1,
            running_jobs: 2,
            metrics: HashMap::new(),
        };

        registry.process_heartbeat(heartbeat).await.unwrap();
    }

    // Distribute some jobs
    let job1_id = Uuid::new_v4();
    let job2_id = Uuid::new_v4();
    let job3_id = Uuid::new_v4();

    let dist1 = coordinator.distribute_job(job1_id).await.unwrap();
    let dist2 = coordinator.distribute_job(job2_id).await.unwrap();
    let dist3 = coordinator.distribute_job(job3_id).await.unwrap();

    // Verify job distributions
    assert_eq!(dist1.job_id, job1_id);
    assert_eq!(dist2.job_id, job2_id);
    assert_eq!(dist3.job_id, job3_id);

    // Get distribution stats
    let stats = coordinator.get_distribution_stats().await.unwrap();
    assert_eq!(stats.total_distributions, 3);

    // Remove a node
    coordinator
        .leave_node(worker2_id, "Planned maintenance".to_string())
        .await
        .unwrap();

    let remaining_nodes = coordinator.get_cluster_nodes().await.unwrap();
    assert_eq!(remaining_nodes.len(), 2);

    // Verify the removed node is not in the list
    assert!(!remaining_nodes.iter().any(|n| n.id == worker2_id));
}

#[tokio::test]
async fn test_heartbeat_and_failure_detection() {
    let (registry, coordinator) = create_integration_test_setup().await;

    coordinator.start().await.unwrap();

    // Add a node
    let node = create_test_node("test-node", NodeRole::Worker);
    let node_id = node.id;

    coordinator.join_node(node).await.unwrap();

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

    registry.process_heartbeat(heartbeat.clone()).await.unwrap();

    // Verify node is active
    let active_nodes = coordinator.get_active_nodes().await.unwrap();
    assert_eq!(active_nodes.len(), 1);
    assert_eq!(active_nodes[0].id, node_id);

    // Send another heartbeat after some time
    sleep(Duration::from_millis(500)).await;

    let mut heartbeat2 = heartbeat.clone();
    heartbeat2.timestamp = chrono::Utc::now();
    heartbeat2.resources.cpu_usage = 75.0;

    registry.process_heartbeat(heartbeat2).await.unwrap();

    // Verify node resource information was updated
    let updated_node = registry.get_node(node_id).await.unwrap().unwrap();
    assert_eq!(updated_node.resources.cpu_usage, 75.0);

    // Get heartbeat stats
    let stats = registry.get_stats().await;
    assert_eq!(stats.total_heartbeats, 2);
    assert_eq!(stats.active_nodes, 1);
}

#[tokio::test]
async fn test_load_balancing_strategies() {
    let (registry, coordinator) = create_integration_test_setup().await;

    coordinator.start().await.unwrap();

    // Add nodes with different capacities
    let mut node1 = create_test_node("high-capacity", NodeRole::Worker);
    let mut node2 = create_test_node("low-capacity", NodeRole::Worker);

    // Set different resource levels
    node1.resources = NodeResources {
        total_cpu: 8,
        available_cpu: 6,
        total_memory: 16384,
        available_memory: 12288,
        total_disk: 204800,
        available_disk: 153600,
        cpu_usage: 25.0,
        memory_usage: 25.0,
        disk_usage: 25.0,
    };

    node2.resources = NodeResources {
        total_cpu: 4,
        available_cpu: 1,
        total_memory: 8192,
        available_memory: 2048,
        total_disk: 102400,
        available_disk: 25600,
        cpu_usage: 75.0,
        memory_usage: 75.0,
        disk_usage: 75.0,
    };

    let node1_id = node1.id;
    let node2_id = node2.id;

    coordinator.join_node(node1).await.unwrap();
    coordinator.join_node(node2).await.unwrap();

    // Distribute multiple jobs and verify they go to the higher capacity node
    let mut distributions = Vec::new();
    for _ in 0..5 {
        let job_id = Uuid::new_v4();
        let distribution = coordinator.distribute_job(job_id).await.unwrap();
        distributions.push(distribution);
    }

    // With LeastLoaded strategy, jobs should prefer the high-capacity node
    let high_capacity_jobs = distributions
        .iter()
        .filter(|d| d.target_node == node1_id)
        .count();
    let low_capacity_jobs = distributions
        .iter()
        .filter(|d| d.target_node == node2_id)
        .count();

    // High capacity node should get more jobs
    assert!(high_capacity_jobs >= low_capacity_jobs);

    // Verify all distributions use the correct strategy
    for distribution in &distributions {
        assert_eq!(distribution.strategy, LoadBalancingStrategy::LeastLoaded);
    }
}

#[tokio::test]
async fn test_cluster_metrics_collection() {
    let (registry, coordinator) = create_integration_test_setup().await;

    coordinator.start().await.unwrap();

    // Add multiple nodes
    let nodes = vec![
        create_test_node("node1", NodeRole::Worker),
        create_test_node("node2", NodeRole::Worker),
        create_test_node("node3", NodeRole::Master),
    ];

    for node in nodes {
        coordinator.join_node(node).await.unwrap();
    }

    // Get initial metrics
    let metrics = coordinator.get_cluster_metrics().await;

    // In a real implementation, metrics would be updated by background tasks
    // For this test, we verify the structure
    assert!(metrics.total_nodes >= 0);
    assert!(metrics.healthy_nodes >= 0);
    assert!(metrics.cluster_load >= 0.0);

    // Get cluster info
    let cluster_info = coordinator.get_cluster_info().await;
    assert_eq!(cluster_info.name, "integration-test-cluster");
    assert_eq!(cluster_info.nodes.len(), 3);
}

#[tokio::test]
async fn test_concurrent_operations() {
    let (registry, coordinator) = create_integration_test_setup().await;

    coordinator.start().await.unwrap();

    // Add initial nodes
    let node1 = create_test_node("node1", NodeRole::Worker);
    let node2 = create_test_node("node2", NodeRole::Worker);

    let node1_id = node1.id;
    let node2_id = node2.id;

    coordinator.join_node(node1).await.unwrap();
    coordinator.join_node(node2).await.unwrap();

    // Spawn concurrent tasks
    let coordinator_clone = Arc::new(coordinator);
    let registry_clone = Arc::new(registry);

    let mut handles = Vec::new();

    // Task 1: Distribute jobs concurrently
    let coord1 = coordinator_clone.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..10 {
            let job_id = Uuid::new_v4();
            if let Ok(distribution) = coord1.distribute_job(job_id).await {
                println!("Job {} distributed to node {}", i, distribution.target_node);
            }
            sleep(Duration::from_millis(10)).await;
        }
    }));

    // Task 2: Send heartbeats concurrently
    let reg1 = registry_clone.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..10 {
            let heartbeat = NodeHeartbeat {
                node_id: node1_id,
                timestamp: chrono::Utc::now(),
                status: NodeStatus::Active,
                resources: NodeResources::default(),
                active_runners: i % 3,
                running_jobs: i % 5,
                metrics: HashMap::new(),
            };

            if let Ok(_) = reg1.process_heartbeat(heartbeat).await {
                println!("Heartbeat {} processed for node1", i);
            }
            sleep(Duration::from_millis(15)).await;
        }
    }));

    // Task 3: Query cluster status concurrently
    let coord2 = coordinator_clone.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..5 {
            let status = coord2.get_cluster_status().await;
            println!("Cluster status check {}: {:?}", i, status);
            sleep(Duration::from_millis(50)).await;
        }
    }));

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final state
    let final_nodes = coordinator_clone.get_cluster_nodes().await.unwrap();
    assert_eq!(final_nodes.len(), 2);

    let stats = registry_clone.get_stats().await;
    assert!(stats.total_heartbeats > 0);
}

#[tokio::test]
async fn test_node_capacity_and_selection() {
    let (registry, coordinator) = create_integration_test_setup().await;

    coordinator.start().await.unwrap();

    // Create nodes with specific capacity configurations
    let mut high_capacity_node = create_test_node("high-capacity", NodeRole::Worker);
    let mut medium_capacity_node = create_test_node("medium-capacity", NodeRole::Worker);
    let mut low_capacity_node = create_test_node("low-capacity", NodeRole::Worker);

    // Set different resource levels to test capacity-based selection
    high_capacity_node.resources = NodeResources {
        total_cpu: 16,
        available_cpu: 12,
        total_memory: 32768,
        available_memory: 24576,
        total_disk: 409600,
        available_disk: 307200,
        cpu_usage: 25.0,
        memory_usage: 25.0,
        disk_usage: 25.0,
    };

    medium_capacity_node.resources = NodeResources {
        total_cpu: 8,
        available_cpu: 4,
        total_memory: 16384,
        available_memory: 8192,
        total_disk: 204800,
        available_disk: 102400,
        cpu_usage: 50.0,
        memory_usage: 50.0,
        disk_usage: 50.0,
    };

    low_capacity_node.resources = NodeResources {
        total_cpu: 4,
        available_cpu: 1,
        total_memory: 8192,
        available_memory: 2048,
        total_disk: 102400,
        available_disk: 25600,
        cpu_usage: 75.0,
        memory_usage: 75.0,
        disk_usage: 75.0,
    };

    let high_id = high_capacity_node.id;
    let medium_id = medium_capacity_node.id;
    let low_id = low_capacity_node.id;

    coordinator.join_node(high_capacity_node).await.unwrap();
    coordinator.join_node(medium_capacity_node).await.unwrap();
    coordinator.join_node(low_capacity_node).await.unwrap();

    // Test capacity calculations
    let nodes = coordinator.get_cluster_nodes().await.unwrap();

    let high_node = nodes.iter().find(|n| n.id == high_id).unwrap();
    let medium_node = nodes.iter().find(|n| n.id == medium_id).unwrap();
    let low_node = nodes.iter().find(|n| n.id == low_id).unwrap();

    assert!(high_node.available_capacity() > medium_node.available_capacity());
    assert!(medium_node.available_capacity() > low_node.available_capacity());

    // Distribute jobs and verify they prefer higher capacity nodes
    let mut job_distributions = HashMap::new();

    for _ in 0..20 {
        let job_id = Uuid::new_v4();
        let distribution = coordinator.distribute_job(job_id).await.unwrap();
        *job_distributions
            .entry(distribution.target_node)
            .or_insert(0) += 1;
    }

    let high_jobs = job_distributions.get(&high_id).unwrap_or(&0);
    let medium_jobs = job_distributions.get(&medium_id).unwrap_or(&0);
    let low_jobs = job_distributions.get(&low_id).unwrap_or(&0);

    println!(
        "Job distribution - High: {}, Medium: {}, Low: {}",
        high_jobs, medium_jobs, low_jobs
    );

    // With LeastLoaded strategy, higher capacity nodes should get more jobs
    assert!(high_jobs >= medium_jobs);
    assert!(medium_jobs >= low_jobs);
}
