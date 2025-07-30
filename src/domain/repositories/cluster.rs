//! Cluster repository interfaces and implementations
//!
//! This module defines the repository interfaces for managing cluster nodes
//! and cluster-wide operations in the RustCI platform.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::entities::{
    Cluster, ClusterId, ClusterMetrics, ClusterNode, JobDistribution, LoadBalancingStrategy,
    NodeHeartbeat, NodeId, NodeRole, NodeStatus,
};
use crate::error::Result;

/// Repository interface for managing cluster nodes
#[async_trait]
pub trait ClusterNodeRepository: Send + Sync {
    /// Create a new cluster node
    async fn create(&self, node: &ClusterNode) -> Result<ClusterNode>;

    /// Find a node by its ID
    async fn find_by_id(&self, node_id: NodeId) -> Result<Option<ClusterNode>>;

    /// Find all nodes
    async fn find_all(&self) -> Result<Vec<ClusterNode>>;

    /// Find nodes by status
    async fn find_by_status(&self, status: NodeStatus) -> Result<Vec<ClusterNode>>;

    /// Find nodes by role
    async fn find_by_role(&self, role: NodeRole) -> Result<Vec<ClusterNode>>;

    /// Find nodes by cluster ID
    async fn find_by_cluster_id(&self, cluster_id: ClusterId) -> Result<Vec<ClusterNode>>;

    /// Find healthy nodes
    async fn find_healthy(&self) -> Result<Vec<ClusterNode>>;

    /// Find available nodes (can accept jobs)
    async fn find_available(&self) -> Result<Vec<ClusterNode>>;

    /// Update a node
    async fn update(&self, node: &ClusterNode) -> Result<ClusterNode>;

    /// Update node status
    async fn update_status(&self, node_id: NodeId, status: NodeStatus) -> Result<()>;

    /// Update node heartbeat
    async fn update_heartbeat(&self, node_id: NodeId, heartbeat: &NodeHeartbeat) -> Result<()>;

    /// Delete a node
    async fn delete(&self, node_id: NodeId) -> Result<()>;

    /// Check if a node exists
    async fn exists(&self, node_id: NodeId) -> Result<bool>;

    /// Count total nodes
    async fn count(&self) -> Result<u64>;

    /// Count nodes by status
    async fn count_by_status(&self, status: NodeStatus) -> Result<u64>;

    /// Find nodes with pagination
    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<ClusterNode>>;

    /// Find nodes that haven't sent heartbeat within timeout
    async fn find_stale_nodes(&self, timeout_seconds: u64) -> Result<Vec<ClusterNode>>;

    /// Get node with least load
    async fn find_least_loaded(&self) -> Result<Option<ClusterNode>>;

    /// Get nodes ordered by available capacity
    async fn find_by_capacity(&self, ascending: bool) -> Result<Vec<ClusterNode>>;
}

/// Repository interface for managing clusters
#[async_trait]
pub trait ClusterRepository: Send + Sync {
    /// Create a new cluster
    async fn create(&self, cluster: &Cluster) -> Result<Cluster>;

    /// Find a cluster by its ID
    async fn find_by_id(&self, cluster_id: ClusterId) -> Result<Option<Cluster>>;

    /// Find a cluster by name
    async fn find_by_name(&self, name: &str) -> Result<Option<Cluster>>;

    /// Find all clusters
    async fn find_all(&self) -> Result<Vec<Cluster>>;

    /// Update a cluster
    async fn update(&self, cluster: &Cluster) -> Result<Cluster>;

    /// Delete a cluster
    async fn delete(&self, cluster_id: ClusterId) -> Result<()>;

    /// Check if a cluster exists
    async fn exists(&self, cluster_id: ClusterId) -> Result<bool>;

    /// Count total clusters
    async fn count(&self) -> Result<u64>;

    /// Add node to cluster
    async fn add_node(&self, cluster_id: ClusterId, node_id: NodeId) -> Result<()>;

    /// Remove node from cluster
    async fn remove_node(&self, cluster_id: ClusterId, node_id: NodeId) -> Result<()>;

    /// Get cluster metrics
    async fn get_metrics(&self, cluster_id: ClusterId) -> Result<ClusterMetrics>;

    /// Update cluster metrics
    async fn update_metrics(&self, cluster_id: ClusterId, metrics: &ClusterMetrics) -> Result<()>;
}

/// Repository interface for managing job distribution
#[async_trait]
pub trait JobDistributionRepository: Send + Sync {
    /// Record a job distribution
    async fn create(&self, distribution: &JobDistribution) -> Result<JobDistribution>;

    /// Find distribution by job ID
    async fn find_by_job_id(
        &self,
        job_id: crate::domain::entities::JobId,
    ) -> Result<Option<JobDistribution>>;

    /// Find distributions by node ID
    async fn find_by_node_id(&self, node_id: NodeId) -> Result<Vec<JobDistribution>>;

    /// Find distributions by runner ID
    async fn find_by_runner_id(
        &self,
        runner_id: crate::domain::entities::RunnerId,
    ) -> Result<Vec<JobDistribution>>;

    /// Find distributions by strategy
    async fn find_by_strategy(
        &self,
        strategy: LoadBalancingStrategy,
    ) -> Result<Vec<JobDistribution>>;

    /// Find distributions within time range
    async fn find_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<JobDistribution>>;

    /// Delete distribution record
    async fn delete(&self, job_id: crate::domain::entities::JobId) -> Result<()>;

    /// Get distribution statistics
    async fn get_distribution_stats(&self) -> Result<DistributionStats>;

    /// Find distributions with pagination
    async fn find_with_pagination(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobDistribution>>;
}

/// Repository interface for managing node heartbeats
#[async_trait]
pub trait NodeHeartbeatRepository: Send + Sync {
    /// Record a node heartbeat
    async fn create(&self, heartbeat: &NodeHeartbeat) -> Result<NodeHeartbeat>;

    /// Find latest heartbeat for a node
    async fn find_latest_by_node_id(&self, node_id: NodeId) -> Result<Option<NodeHeartbeat>>;

    /// Find heartbeats by node ID within time range
    async fn find_by_node_id_and_time_range(
        &self,
        node_id: NodeId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<NodeHeartbeat>>;

    /// Find all heartbeats within time range
    async fn find_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<NodeHeartbeat>>;

    /// Delete old heartbeats (cleanup)
    async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64>;

    /// Get heartbeat statistics for a node
    async fn get_node_heartbeat_stats(&self, node_id: NodeId) -> Result<HeartbeatStats>;

    /// Find nodes with missing heartbeats
    async fn find_missing_heartbeats(&self, timeout_seconds: u64) -> Result<Vec<NodeId>>;
}

/// Distribution statistics
#[derive(Debug, Clone)]
pub struct DistributionStats {
    /// Total number of job distributions
    pub total_distributions: u64,
    /// Distributions by strategy
    pub by_strategy: std::collections::HashMap<String, u64>,
    /// Distributions by node
    pub by_node: std::collections::HashMap<NodeId, u64>,
    /// Average distribution time
    pub avg_distribution_time: f64,
    /// Most used strategy
    pub most_used_strategy: Option<LoadBalancingStrategy>,
}

/// Heartbeat statistics for a node
#[derive(Debug, Clone)]
pub struct HeartbeatStats {
    /// Node ID
    pub node_id: NodeId,
    /// Total heartbeats recorded
    pub total_heartbeats: u64,
    /// First heartbeat timestamp
    pub first_heartbeat: Option<DateTime<Utc>>,
    /// Latest heartbeat timestamp
    pub latest_heartbeat: Option<DateTime<Utc>>,
    /// Average heartbeat interval in seconds
    pub avg_interval: f64,
    /// Missed heartbeats count
    pub missed_heartbeats: u64,
    /// Uptime percentage
    pub uptime_percentage: f64,
}

/// Query parameters for finding cluster nodes
#[derive(Debug, Clone, Default)]
pub struct ClusterNodeQuery {
    /// Filter by status
    pub status: Option<NodeStatus>,
    /// Filter by role
    pub role: Option<NodeRole>,
    /// Filter by cluster ID
    pub cluster_id: Option<ClusterId>,
    /// Filter by health status
    pub healthy_only: bool,
    /// Filter by availability
    pub available_only: bool,
    /// Pagination
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Sorting
    pub sort_by: Option<NodeSortField>,
    pub sort_order: Option<SortOrder>,
}

/// Fields to sort nodes by
#[derive(Debug, Clone)]
pub enum NodeSortField {
    Name,
    Status,
    CreatedAt,
    LastHeartbeat,
    AvailableCapacity,
}

/// Sort order
#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}


impl DistributionStats {
    /// Create new empty distribution statistics
    pub fn new() -> Self {
        Self {
            total_distributions: 0,
            by_strategy: std::collections::HashMap::new(),
            by_node: std::collections::HashMap::new(),
            avg_distribution_time: 0.0,
            most_used_strategy: None,
        }
    }
}

impl Default for DistributionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl HeartbeatStats {
    /// Create new heartbeat statistics
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            total_heartbeats: 0,
            first_heartbeat: None,
            latest_heartbeat: None,
            avg_interval: 0.0,
            missed_heartbeats: 0,
            uptime_percentage: 0.0,
        }
    }

    /// Calculate uptime percentage
    pub fn calculate_uptime(&mut self, expected_heartbeats: u64) {
        if expected_heartbeats > 0 {
            let successful_heartbeats = expected_heartbeats - self.missed_heartbeats;
            self.uptime_percentage =
                (successful_heartbeats as f64 / expected_heartbeats as f64) * 100.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_distribution_stats_creation() {
        let stats = DistributionStats::new();
        assert_eq!(stats.total_distributions, 0);
        assert!(stats.by_strategy.is_empty());
        assert!(stats.most_used_strategy.is_none());
    }

    #[test]
    fn test_heartbeat_stats_creation() {
        let node_id = Uuid::new_v4();
        let stats = HeartbeatStats::new(node_id);
        assert_eq!(stats.node_id, node_id);
        assert_eq!(stats.total_heartbeats, 0);
        assert_eq!(stats.uptime_percentage, 0.0);
    }

    #[test]
    fn test_heartbeat_stats_uptime_calculation() {
        let node_id = Uuid::new_v4();
        let mut stats = HeartbeatStats::new(node_id);
        stats.missed_heartbeats = 2;
        stats.calculate_uptime(10);
        assert_eq!(stats.uptime_percentage, 80.0);
    }

    #[test]
    fn test_cluster_node_query_default() {
        let query = ClusterNodeQuery::default();
        assert!(query.status.is_none());
        assert!(query.role.is_none());
        assert!(!query.healthy_only);
        assert!(!query.available_only);
    }
}
