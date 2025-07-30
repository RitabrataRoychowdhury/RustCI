//! Cluster Coordinator for managing cluster membership and coordination
//!
//! This module implements the ClusterCoordinator that manages cluster membership,
//! coordinates distributed operations, and provides APIs for cluster management.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::node_registry::{NodeRegistry, NodeRegistryConfig};
use crate::domain::entities::{
    Cluster, ClusterId, ClusterMetrics, ClusterNode, ClusterStatus, JobDistribution,
    LoadBalancingStrategy, NodeId,
};
use crate::domain::repositories::{ClusterRepository, JobDistributionRepository};
use crate::error::{AppError, Result};

/// Configuration for the ClusterCoordinator
#[derive(Debug, Clone)]
pub struct ClusterCoordinatorConfig {
    /// Cluster ID
    pub cluster_id: ClusterId,
    /// Cluster name
    pub cluster_name: String,
    /// Node registry configuration
    pub node_registry_config: NodeRegistryConfig,
    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// Enable automatic cluster healing
    pub auto_healing: bool,
    /// Metrics collection interval in seconds
    pub metrics_interval: u64,
    /// Maximum nodes allowed in cluster
    pub max_nodes: u32,
}

impl Default for ClusterCoordinatorConfig {
    fn default() -> Self {
        Self {
            cluster_id: Uuid::new_v4(),
            cluster_name: "default-cluster".to_string(),
            node_registry_config: NodeRegistryConfig::default(),
            load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
            auto_healing: true,
            metrics_interval: 60,
            max_nodes: 100,
        }
    }
}

/// Events that can occur in the cluster
#[derive(Debug, Clone)]
pub enum ClusterEvent {
    /// Node joined the cluster
    NodeJoined { node_id: NodeId, node: ClusterNode },
    /// Node left the cluster
    NodeLeft { node_id: NodeId, reason: String },
    /// Node failed
    NodeFailed { node_id: NodeId, reason: String },
    /// Cluster status changed
    StatusChanged {
        old_status: ClusterStatus,
        new_status: ClusterStatus,
    },
    /// Job distributed to node
    JobDistributed { distribution: JobDistribution },
    /// Metrics updated
    MetricsUpdated { metrics: ClusterMetrics },
}

/// ClusterCoordinator manages cluster membership and coordination
pub struct ClusterCoordinator {
    /// Configuration
    config: ClusterCoordinatorConfig,
    /// Current cluster state
    cluster: Arc<RwLock<Cluster>>,
    /// Node registry for managing nodes
    node_registry: Arc<NodeRegistry>,
    /// Cluster repository
    cluster_repo: Arc<dyn ClusterRepository>,
    /// Job distribution repository
    job_distribution_repo: Arc<dyn JobDistributionRepository>,
    /// Event channel for cluster events
    event_sender: mpsc::UnboundedSender<ClusterEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<ClusterEvent>>>>,
    /// Current cluster metrics
    metrics: Arc<RwLock<ClusterMetrics>>,
    /// Load balancer state
    load_balancer: Arc<RwLock<LoadBalancerState>>,
}

/// Load balancer state for job distribution
#[derive(Debug)]
struct LoadBalancerState {
    /// Current strategy
    strategy: LoadBalancingStrategy,
    /// Round-robin counter
    round_robin_counter: usize,
    /// Node load tracking
    node_loads: HashMap<NodeId, f64>,
    /// Job distribution history
    distribution_history: Vec<JobDistribution>,
}

impl ClusterCoordinator {
    /// Create a new ClusterCoordinator
    pub async fn new(
        config: ClusterCoordinatorConfig,
        node_registry: Arc<NodeRegistry>,
        cluster_repo: Arc<dyn ClusterRepository>,
        job_distribution_repo: Arc<dyn JobDistributionRepository>,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // Load or create cluster
        let cluster = match cluster_repo.find_by_id(config.cluster_id).await? {
            Some(existing_cluster) => existing_cluster,
            None => {
                let new_cluster = Cluster::new(
                    config.cluster_name.clone(),
                    crate::domain::entities::ClusterConfig {
                        max_nodes: config.max_nodes,
                        heartbeat_interval: config.node_registry_config.heartbeat_interval as u32,
                        failure_timeout: config.node_registry_config.failure_timeout as u32,
                        load_balancing_strategy: config.load_balancing_strategy.clone(),
                        auto_scaling: None,
                        security: crate::domain::entities::SecurityConfig::default(),
                    },
                );
                cluster_repo.create(&new_cluster).await?
            }
        };

        let coordinator = Self {
            config,
            cluster: Arc::new(RwLock::new(cluster)),
            node_registry,
            cluster_repo,
            job_distribution_repo,
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            metrics: Arc::new(RwLock::new(ClusterMetrics::new())),
            load_balancer: Arc::new(RwLock::new(LoadBalancerState::new(
                LoadBalancingStrategy::LeastLoaded,
            ))),
        };

        Ok(coordinator)
    }

    /// Start the cluster coordinator
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting ClusterCoordinator for cluster: {}",
            self.config.cluster_name
        );

        // Start node registry
        self.node_registry.start().await?;

        // Start metrics collection
        self.start_metrics_collection().await;

        // Start event processing
        self.start_event_processing().await;

        // Update cluster status
        self.update_cluster_status(ClusterStatus::Healthy).await?;

        info!("ClusterCoordinator started successfully");
        Ok(())
    }

    /// Stop the cluster coordinator
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping ClusterCoordinator");

        // Update cluster status
        self.update_cluster_status(ClusterStatus::Stopping).await?;

        // Additional cleanup can be added here

        info!("ClusterCoordinator stopped");
        Ok(())
    }

    /// Join a node to the cluster
    pub async fn join_node(&self, node: ClusterNode) -> Result<ClusterNode> {
        info!("Node {} requesting to join cluster", node.id);

        // Check if cluster is at capacity
        {
            let cluster = self.cluster.read().await;
            if cluster.is_at_capacity() {
                return Err(AppError::ValidationError(format!(
                    "Cluster is at maximum capacity of {} nodes",
                    cluster.config.max_nodes
                )));
            }
        }

        // Register node with registry
        let registered_node = self.node_registry.register_node(node.clone()).await?;

        // Add node to cluster
        {
            let mut cluster = self.cluster.write().await;
            cluster.add_node(registered_node.id);
        }

        // Update cluster in repository
        {
            let cluster = self.cluster.read().await;
            self.cluster_repo.update(&cluster).await?;
        }

        // Send event
        let _ = self.event_sender.send(ClusterEvent::NodeJoined {
            node_id: registered_node.id,
            node: registered_node.clone(),
        });

        info!("Node {} successfully joined cluster", registered_node.id);
        Ok(registered_node)
    }

    /// Remove a node from the cluster
    pub async fn leave_node(&self, node_id: NodeId, reason: String) -> Result<()> {
        info!("Node {} leaving cluster: {}", node_id, reason);

        // Unregister node from registry
        self.node_registry.unregister_node(node_id).await?;

        // Remove node from cluster
        {
            let mut cluster = self.cluster.write().await;
            cluster.remove_node(node_id);
        }

        // Update cluster in repository
        {
            let cluster = self.cluster.read().await;
            self.cluster_repo.update(&cluster).await?;
        }

        // Send event
        let _ = self
            .event_sender
            .send(ClusterEvent::NodeLeft { node_id, reason });

        info!("Node {} successfully left cluster", node_id);
        Ok(())
    }

    /// Get cluster status
    pub async fn get_cluster_status(&self) -> ClusterStatus {
        let cluster = self.cluster.read().await;
        cluster.status.clone()
    }

    /// Get cluster information
    pub async fn get_cluster_info(&self) -> Cluster {
        let cluster = self.cluster.read().await;
        cluster.clone()
    }

    /// Get cluster metrics
    pub async fn get_cluster_metrics(&self) -> ClusterMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get all nodes in the cluster
    pub async fn get_cluster_nodes(&self) -> Result<Vec<ClusterNode>> {
        self.node_registry.get_all_nodes().await
    }

    /// Get active nodes in the cluster
    pub async fn get_active_nodes(&self) -> Result<Vec<ClusterNode>> {
        self.node_registry.get_active_nodes().await
    }

    /// Distribute a job to an appropriate node
    pub async fn distribute_job(
        &self,
        job_id: crate::domain::entities::JobId,
    ) -> Result<JobDistribution> {
        debug!("Distributing job: {}", job_id);

        // Get available nodes
        let available_nodes = self.node_registry.get_active_nodes().await?;
        if available_nodes.is_empty() {
            return Err(AppError::ValidationError(
                "No available nodes for job distribution".to_string(),
            ));
        }

        // Select node based on load balancing strategy
        let selected_node = self.select_node_for_job(&available_nodes).await?;

        // Create distribution record
        let distribution = JobDistribution {
            job_id,
            target_node: selected_node.id,
            target_runner: selected_node
                .runners
                .first()
                .copied()
                .unwrap_or_else(Uuid::new_v4),
            distributed_at: Utc::now(),
            strategy: self.config.load_balancing_strategy.clone(),
            metadata: HashMap::new(),
        };

        // Persist distribution
        let saved_distribution = self.job_distribution_repo.create(&distribution).await?;

        // Update load balancer state
        self.update_load_balancer_state(&saved_distribution).await;

        // Send event
        let _ = self.event_sender.send(ClusterEvent::JobDistributed {
            distribution: saved_distribution.clone(),
        });

        debug!("Job {} distributed to node {}", job_id, selected_node.id);
        Ok(saved_distribution)
    }

    /// Get job distribution statistics
    pub async fn get_distribution_stats(
        &self,
    ) -> Result<crate::domain::repositories::DistributionStats> {
        self.job_distribution_repo.get_distribution_stats().await
    }

    /// Subscribe to cluster events
    pub async fn subscribe_to_events(&self) -> Option<mpsc::UnboundedReceiver<ClusterEvent>> {
        let mut receiver_guard = self.event_receiver.write().await;
        receiver_guard.take()
    }

    /// Update cluster status
    async fn update_cluster_status(&self, new_status: ClusterStatus) -> Result<()> {
        let old_status = {
            let mut cluster = self.cluster.write().await;
            let old_status = cluster.status.clone();
            cluster.status = new_status.clone();
            cluster.updated_at = Utc::now();
            old_status
        };

        // Update in repository
        {
            let cluster = self.cluster.read().await;
            self.cluster_repo.update(&cluster).await?;
        }

        // Send event if status changed
        if old_status != new_status {
            let _ = self.event_sender.send(ClusterEvent::StatusChanged {
                old_status,
                new_status,
            });
        }

        Ok(())
    }

    /// Select a node for job distribution based on load balancing strategy
    async fn select_node_for_job(&self, available_nodes: &[ClusterNode]) -> Result<ClusterNode> {
        let load_balancer = self.load_balancer.read().await;

        match load_balancer.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let index = load_balancer.round_robin_counter % available_nodes.len();
                Ok(available_nodes[index].clone())
            }
            LoadBalancingStrategy::LeastLoaded => {
                // Find node with highest available capacity
                available_nodes
                    .iter()
                    .max_by(|a, b| {
                        a.available_capacity()
                            .partial_cmp(&b.available_capacity())
                            .unwrap()
                    })
                    .cloned()
                    .ok_or_else(|| AppError::ValidationError("No suitable node found".to_string()))
            }
            LoadBalancingStrategy::Random => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let index = rng.gen_range(0..available_nodes.len());
                Ok(available_nodes[index].clone())
            }
            LoadBalancingStrategy::Weighted => {
                // Weighted selection based on available capacity
                let total_capacity: f64 =
                    available_nodes.iter().map(|n| n.available_capacity()).sum();
                if total_capacity == 0.0 {
                    return Ok(available_nodes[0].clone());
                }

                use rand::Rng;
                let mut rng = rand::thread_rng();
                let mut random_value = rng.gen::<f64>() * total_capacity;

                for node in available_nodes {
                    random_value -= node.available_capacity();
                    if random_value <= 0.0 {
                        return Ok(node.clone());
                    }
                }

                Ok(available_nodes[available_nodes.len() - 1].clone())
            }
            LoadBalancingStrategy::Affinity => {
                // For now, use least loaded as fallback
                available_nodes
                    .iter()
                    .max_by(|a, b| {
                        a.available_capacity()
                            .partial_cmp(&b.available_capacity())
                            .unwrap()
                    })
                    .cloned()
                    .ok_or_else(|| AppError::ValidationError("No suitable node found".to_string()))
            }
        }
    }

    /// Update load balancer state after job distribution
    async fn update_load_balancer_state(&self, distribution: &JobDistribution) {
        let mut load_balancer = self.load_balancer.write().await;

        // Update round-robin counter
        load_balancer.round_robin_counter = load_balancer.round_robin_counter.wrapping_add(1);

        // Update node load tracking
        let current_load = *load_balancer
            .node_loads
            .get(&distribution.target_node)
            .unwrap_or(&0.0);
        load_balancer
            .node_loads
            .insert(distribution.target_node, current_load + 1.0);

        // Add to distribution history (keep last 1000 entries)
        load_balancer
            .distribution_history
            .push(distribution.clone());
        if load_balancer.distribution_history.len() > 1000 {
            load_balancer.distribution_history.remove(0);
        }
    }

    /// Start metrics collection task
    async fn start_metrics_collection(&self) {
        let node_registry = self.node_registry.clone();
        let cluster = self.cluster.clone();
        let metrics = self.metrics.clone();
        let event_sender = self.event_sender.clone();
        let interval_secs = self.config.metrics_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                if let Err(e) =
                    Self::collect_metrics(&node_registry, &cluster, &metrics, &event_sender).await
                {
                    error!("Failed to collect cluster metrics: {}", e);
                }
            }
        });
    }

    /// Collect cluster metrics
    async fn collect_metrics(
        node_registry: &Arc<NodeRegistry>,
        _cluster: &Arc<RwLock<Cluster>>,
        metrics: &Arc<RwLock<ClusterMetrics>>,
        event_sender: &mpsc::UnboundedSender<ClusterEvent>,
    ) -> Result<()> {
        let registry_stats = node_registry.get_stats().await;
        let _all_nodes = node_registry.get_all_nodes().await?;
        let active_nodes = node_registry.get_active_nodes().await?;

        let mut new_metrics = ClusterMetrics::new();
        new_metrics.total_nodes = registry_stats.total_nodes as u32;
        new_metrics.healthy_nodes = registry_stats.active_nodes as u32;
        new_metrics.failed_nodes = registry_stats.failed_nodes as u32;

        // Calculate resource utilization
        if !active_nodes.is_empty() {
            let total_cpu: f64 = active_nodes.iter().map(|n| n.resources.cpu_usage).sum();
            let total_memory: f64 = active_nodes.iter().map(|n| n.resources.memory_usage).sum();

            new_metrics.avg_cpu_usage = total_cpu / active_nodes.len() as f64;
            new_metrics.avg_memory_usage = total_memory / active_nodes.len() as f64;
        }

        // Calculate cluster load
        new_metrics.cluster_load = if new_metrics.total_nodes > 0 {
            (new_metrics.healthy_nodes as f64 / new_metrics.total_nodes as f64) * 100.0
        } else {
            0.0
        };

        // Update metrics
        {
            let mut metrics_guard = metrics.write().await;
            *metrics_guard = new_metrics.clone();
        }

        // Send event
        let _ = event_sender.send(ClusterEvent::MetricsUpdated {
            metrics: new_metrics,
        });

        Ok(())
    }

    /// Start event processing task
    async fn start_event_processing(&self) {
        let event_receiver = self.event_receiver.clone();

        tokio::spawn(async move {
            let receiver = {
                let mut guard = event_receiver.write().await;
                guard.take()
            };

            if let Some(mut rx) = receiver {
                while let Some(event) = rx.recv().await {
                    Self::process_cluster_event(event).await;
                }
            }
        });
    }

    /// Process cluster events
    async fn process_cluster_event(event: ClusterEvent) {
        match event {
            ClusterEvent::NodeJoined { node_id, node } => {
                info!("Cluster event: Node {} ({}) joined", node_id, node.name);
            }
            ClusterEvent::NodeLeft { node_id, reason } => {
                info!("Cluster event: Node {} left ({})", node_id, reason);
            }
            ClusterEvent::NodeFailed { node_id, reason } => {
                warn!("Cluster event: Node {} failed ({})", node_id, reason);
            }
            ClusterEvent::StatusChanged {
                old_status,
                new_status,
            } => {
                info!(
                    "Cluster event: Status changed from {:?} to {:?}",
                    old_status, new_status
                );
            }
            ClusterEvent::JobDistributed { distribution } => {
                debug!(
                    "Cluster event: Job {} distributed to node {}",
                    distribution.job_id, distribution.target_node
                );
            }
            ClusterEvent::MetricsUpdated { metrics } => {
                debug!(
                    "Cluster event: Metrics updated - {} nodes, {:.1}% health",
                    metrics.total_nodes,
                    metrics.health_percentage()
                );
            }
        }
    }
}

impl LoadBalancerState {
    fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            round_robin_counter: 0,
            node_loads: HashMap::new(),
            distribution_history: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::NodeRole;
    use std::net::SocketAddr;
    use std::str::FromStr;

    // Mock implementations for testing
    struct MockClusterRepository;
    struct MockJobDistributionRepository;

    #[async_trait::async_trait]
    impl ClusterRepository for MockClusterRepository {
        async fn create(&self, cluster: &Cluster) -> Result<Cluster> {
            Ok(cluster.clone())
        }

        async fn find_by_id(&self, _cluster_id: ClusterId) -> Result<Option<Cluster>> {
            Ok(None)
        }

        async fn find_by_name(&self, _name: &str) -> Result<Option<Cluster>> {
            Ok(None)
        }

        async fn find_all(&self) -> Result<Vec<Cluster>> {
            Ok(Vec::new())
        }

        async fn update(&self, cluster: &Cluster) -> Result<Cluster> {
            Ok(cluster.clone())
        }

        async fn delete(&self, _cluster_id: ClusterId) -> Result<()> {
            Ok(())
        }

        async fn exists(&self, _cluster_id: ClusterId) -> Result<bool> {
            Ok(false)
        }

        async fn count(&self) -> Result<u64> {
            Ok(0)
        }

        async fn add_node(&self, _cluster_id: ClusterId, _node_id: NodeId) -> Result<()> {
            Ok(())
        }

        async fn remove_node(&self, _cluster_id: ClusterId, _node_id: NodeId) -> Result<()> {
            Ok(())
        }

        async fn get_metrics(&self, _cluster_id: ClusterId) -> Result<ClusterMetrics> {
            Ok(ClusterMetrics::new())
        }

        async fn update_metrics(
            &self,
            _cluster_id: ClusterId,
            _metrics: &ClusterMetrics,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl JobDistributionRepository for MockJobDistributionRepository {
        async fn create(&self, distribution: &JobDistribution) -> Result<JobDistribution> {
            Ok(distribution.clone())
        }

        async fn find_by_job_id(
            &self,
            _job_id: crate::domain::entities::JobId,
        ) -> Result<Option<JobDistribution>> {
            Ok(None)
        }

        async fn find_by_node_id(&self, _node_id: NodeId) -> Result<Vec<JobDistribution>> {
            Ok(Vec::new())
        }

        async fn find_by_runner_id(
            &self,
            _runner_id: crate::domain::entities::RunnerId,
        ) -> Result<Vec<JobDistribution>> {
            Ok(Vec::new())
        }

        async fn find_by_strategy(
            &self,
            _strategy: LoadBalancingStrategy,
        ) -> Result<Vec<JobDistribution>> {
            Ok(Vec::new())
        }

        async fn find_by_time_range(
            &self,
            _start: DateTime<Utc>,
            _end: DateTime<Utc>,
        ) -> Result<Vec<JobDistribution>> {
            Ok(Vec::new())
        }

        async fn delete(&self, _job_id: crate::domain::entities::JobId) -> Result<()> {
            Ok(())
        }

        async fn get_distribution_stats(
            &self,
        ) -> Result<crate::domain::repositories::DistributionStats> {
            Ok(crate::domain::repositories::DistributionStats::new())
        }

        async fn find_with_pagination(
            &self,
            _limit: usize,
            _offset: usize,
        ) -> Result<Vec<JobDistribution>> {
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

    #[tokio::test]
    async fn test_cluster_coordinator_creation() {
        let config = ClusterCoordinatorConfig::default();
        let node_registry = Arc::new(crate::core::node_registry::tests::create_test_registry());
        let cluster_repo = Arc::new(MockClusterRepository);
        let job_distribution_repo = Arc::new(MockJobDistributionRepository);

        let coordinator =
            ClusterCoordinator::new(config, node_registry, cluster_repo, job_distribution_repo)
                .await
                .unwrap();

        let status = coordinator.get_cluster_status().await;
        assert_eq!(status, ClusterStatus::Starting);
    }

    #[tokio::test]
    async fn test_node_join_and_leave() {
        let config = ClusterCoordinatorConfig::default();
        let node_registry = Arc::new(crate::core::node_registry::tests::create_test_registry());
        let cluster_repo = Arc::new(MockClusterRepository);
        let job_distribution_repo = Arc::new(MockJobDistributionRepository);

        let coordinator =
            ClusterCoordinator::new(config, node_registry, cluster_repo, job_distribution_repo)
                .await
                .unwrap();

        let node = create_test_node();
        let node_id = node.id;

        // Join node
        let joined_node = coordinator.join_node(node).await.unwrap();
        assert_eq!(joined_node.id, node_id);

        // Leave node
        coordinator
            .leave_node(node_id, "Test leave".to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_job_distribution() {
        let config = ClusterCoordinatorConfig::default();
        let node_registry = Arc::new(crate::core::node_registry::tests::create_test_registry());
        let cluster_repo = Arc::new(MockClusterRepository);
        let job_distribution_repo = Arc::new(MockJobDistributionRepository);

        let coordinator = ClusterCoordinator::new(
            config,
            node_registry.clone(),
            cluster_repo,
            job_distribution_repo,
        )
        .await
        .unwrap();

        // Add a node first
        let node = create_test_node();
        node_registry.register_node(node).await.unwrap();

        // Distribute a job
        let job_id = Uuid::new_v4();
        let distribution = coordinator.distribute_job(job_id).await.unwrap();

        assert_eq!(distribution.job_id, job_id);
        assert_eq!(distribution.strategy, LoadBalancingStrategy::LeastLoaded);
    }
}
