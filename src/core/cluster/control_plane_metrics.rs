use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Control plane specific metrics collector
pub struct ControlPlaneMetrics {
    start_time: Instant,
    job_metrics: Arc<RwLock<JobMetrics>>,
    node_metrics: Arc<RwLock<NodeMetrics>>,
    cluster_metrics: Arc<RwLock<ClusterMetrics>>,
}

#[derive(Debug, Default)]
struct JobMetrics {
    jobs_scheduled_total: u64,
    jobs_completed_total: u64,
    jobs_failed_total: u64,
    jobs_running: u64,
    jobs_queued: u64,
    job_queue_length: u64,
    job_execution_times: Vec<Duration>,
}

#[derive(Debug, Default)]
struct NodeMetrics {
    nodes_total: u64,
    nodes_healthy: u64,
    nodes_unhealthy: u64,
    nodes_joining: u64,
    nodes_leaving: u64,
    node_heartbeat_failures: u64,
    node_resource_usage: HashMap<String, NodeResourceUsage>,
}

#[derive(Debug, Default, Clone)]
struct NodeResourceUsage {
    cpu_usage_percent: f64,
    memory_usage_bytes: u64,
    memory_total_bytes: u64,
    disk_usage_bytes: u64,
    disk_total_bytes: u64,
    network_rx_bytes: u64,
    network_tx_bytes: u64,
}

#[derive(Debug, Default)]
struct ClusterMetrics {
    cluster_size: u64,
    cluster_capacity: u64,
    cluster_utilization_percent: f64,
    leader_elections_total: u64,
    network_partitions_total: u64,
    failovers_total: u64,
}

impl ControlPlaneMetrics {
    /// Create new control plane metrics collector
    pub fn new() -> Result<Self> {
        Ok(Self {
            start_time: Instant::now(),
            job_metrics: Arc::new(RwLock::new(JobMetrics::default())),
            node_metrics: Arc::new(RwLock::new(NodeMetrics::default())),
            cluster_metrics: Arc::new(RwLock::new(ClusterMetrics::default())),
        })
    }



    /// Record job scheduled
    pub async fn record_job_scheduled(&self, job_id: &Uuid, node_id: &str) {
        let mut metrics = self.job_metrics.write().await;
        metrics.jobs_scheduled_total += 1;
        metrics.jobs_queued += 1;

        debug!("Job {} scheduled to node {}", job_id, node_id);
    }

    /// Record job started
    pub async fn record_job_started(&self, job_id: &Uuid, node_id: &str) {
        let mut metrics = self.job_metrics.write().await;
        metrics.jobs_running += 1;
        metrics.jobs_queued = metrics.jobs_queued.saturating_sub(1);

        debug!("Job {} started on node {}", job_id, node_id);
    }

    /// Record job completed
    pub async fn record_job_completed(&self, job_id: &Uuid, node_id: &str, duration: Duration) {
        let mut metrics = self.job_metrics.write().await;
        metrics.jobs_completed_total += 1;
        metrics.jobs_running = metrics.jobs_running.saturating_sub(1);
        metrics.job_execution_times.push(duration);

        info!("Job {} completed on node {} in {:?}", job_id, node_id, duration);
    }

    /// Record job failed
    pub async fn record_job_failed(&self, job_id: &Uuid, node_id: &str, error: &str) {
        let mut metrics = self.job_metrics.write().await;
        metrics.jobs_failed_total += 1;
        metrics.jobs_running = metrics.jobs_running.saturating_sub(1);

        warn!("Job {} failed on node {}: {}", job_id, node_id, error);
    }

    /// Record job scheduling duration
    pub fn record_job_scheduling_duration(&self, duration: Duration) {
        debug!("Job scheduling took {:?}", duration);
    }

    /// Update job queue length
    pub async fn update_job_queue_length(&self, length: u64) {
        let mut metrics = self.job_metrics.write().await;
        metrics.job_queue_length = length;
        debug!("Job queue length updated to {}", length);
    }

    /// Record node joined
    pub async fn record_node_joined(&self, node_id: &str) {
        let mut metrics = self.node_metrics.write().await;
        metrics.nodes_total += 1;
        metrics.nodes_healthy += 1;

        info!("Node {} joined the cluster", node_id);
    }

    /// Record node left
    pub async fn record_node_left(&self, node_id: &str, reason: &str) {
        let mut metrics = self.node_metrics.write().await;
        metrics.nodes_total = metrics.nodes_total.saturating_sub(1);
        metrics.nodes_healthy = metrics.nodes_healthy.saturating_sub(1);

        info!("Node {} left the cluster: {}", node_id, reason);
    }

    /// Record node health change
    pub async fn record_node_health_change(&self, node_id: &str, is_healthy: bool) {
        let mut metrics = self.node_metrics.write().await;
        
        if is_healthy {
            metrics.nodes_healthy += 1;
            metrics.nodes_unhealthy = metrics.nodes_unhealthy.saturating_sub(1);
        } else {
            metrics.nodes_unhealthy += 1;
            metrics.nodes_healthy = metrics.nodes_healthy.saturating_sub(1);
        }

        if is_healthy {
            info!("Node {} recovered", node_id);
        } else {
            warn!("Node {} became unhealthy", node_id);
        }
    }

    /// Record node heartbeat failure
    pub async fn record_node_heartbeat_failure(&self, node_id: &str) {
        let mut metrics = self.node_metrics.write().await;
        metrics.node_heartbeat_failures += 1;

        warn!("Node {} heartbeat failure", node_id);
    }

    /// Update node resource usage
    pub async fn update_node_resource_usage(&self, node_id: &str, usage: NodeResourceUsage) {
        let mut metrics = self.node_metrics.write().await;
        metrics.node_resource_usage.insert(node_id.to_string(), usage.clone());

        debug!("Updated resource usage for node {}: CPU: {:.1}%, Memory: {} bytes", 
               node_id, usage.cpu_usage_percent, usage.memory_usage_bytes);
    }

    /// Record leader election
    pub async fn record_leader_election(&self, new_leader: &str) {
        let mut metrics = self.cluster_metrics.write().await;
        metrics.leader_elections_total += 1;

        info!("Leader election completed, new leader: {}", new_leader);
    }

    /// Record network partition
    pub async fn record_network_partition(&self, affected_nodes: &[String]) {
        let mut metrics = self.cluster_metrics.write().await;
        metrics.network_partitions_total += 1;

        warn!("Network partition detected affecting {} nodes: {:?}", 
              affected_nodes.len(), affected_nodes);
    }

    /// Record failover
    pub async fn record_failover(&self, from_node: &str, to_node: &str) {
        let mut metrics = self.cluster_metrics.write().await;
        metrics.failovers_total += 1;

        info!("Failover completed from {} to {}", from_node, to_node);
    }

    /// Update cluster metrics
    pub async fn update_cluster_metrics(&self, size: u64, capacity: u64, utilization: f64) {
        let mut metrics = self.cluster_metrics.write().await;
        metrics.cluster_size = size;
        metrics.cluster_capacity = capacity;
        metrics.cluster_utilization_percent = utilization;

        debug!("Cluster metrics updated: size={}, capacity={}, utilization={:.1}%", 
               size, capacity, utilization);
    }

    /// Record API request
    pub fn record_api_request(&self, method: &str, path: &str, status_code: u16, duration: Duration) {
        debug!("API request: {} {} -> {} in {:?}", method, path, status_code, duration);
    }

    /// Update system metrics
    pub fn update_system_metrics(&self, memory_usage: u64, cpu_usage: f64) {
        let uptime = self.start_time.elapsed().as_secs();
        debug!("System metrics: uptime={}s, memory={}MB, cpu={:.1}%", 
               uptime, memory_usage / 1024 / 1024, cpu_usage);
    }

    /// Get metrics snapshot for debugging
    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let job_metrics = self.job_metrics.read().await;
        let node_metrics = self.node_metrics.read().await;
        let cluster_metrics = self.cluster_metrics.read().await;

        MetricsSnapshot {
            uptime_seconds: self.start_time.elapsed().as_secs(),
            jobs_scheduled_total: job_metrics.jobs_scheduled_total,
            jobs_completed_total: job_metrics.jobs_completed_total,
            jobs_failed_total: job_metrics.jobs_failed_total,
            jobs_running: job_metrics.jobs_running,
            jobs_queued: job_metrics.jobs_queued,
            job_queue_length: job_metrics.job_queue_length,
            nodes_total: node_metrics.nodes_total,
            nodes_healthy: node_metrics.nodes_healthy,
            nodes_unhealthy: node_metrics.nodes_unhealthy,
            cluster_size: cluster_metrics.cluster_size,
            cluster_capacity: cluster_metrics.cluster_capacity,
            cluster_utilization_percent: cluster_metrics.cluster_utilization_percent,
            leader_elections_total: cluster_metrics.leader_elections_total,
            network_partitions_total: cluster_metrics.network_partitions_total,
            failovers_total: cluster_metrics.failovers_total,
        }
    }

    /// Get Prometheus metrics as string
    pub fn get_prometheus_metrics(&self) -> String {
        // For now, return a simple metrics format
        // In a real implementation, this would format all metrics in Prometheus format
        format!(
            "# HELP rustci_control_plane_uptime_seconds Control plane uptime in seconds\n\
             # TYPE rustci_control_plane_uptime_seconds gauge\n\
             rustci_control_plane_uptime_seconds {}\n",
            self.start_time.elapsed().as_secs()
        )
    }
}

impl Default for ControlPlaneMetrics {
    fn default() -> Self {
        Self::new().expect("Failed to create default ControlPlaneMetrics")
    }
}

/// Snapshot of current metrics for debugging and monitoring
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub uptime_seconds: u64,
    pub jobs_scheduled_total: u64,
    pub jobs_completed_total: u64,
    pub jobs_failed_total: u64,
    pub jobs_running: u64,
    pub jobs_queued: u64,
    pub job_queue_length: u64,
    pub nodes_total: u64,
    pub nodes_healthy: u64,
    pub nodes_unhealthy: u64,
    pub cluster_size: u64,
    pub cluster_capacity: u64,
    pub cluster_utilization_percent: f64,
    pub leader_elections_total: u64,
    pub network_partitions_total: u64,
    pub failovers_total: u64,
}

/// Node resource usage information
impl NodeResourceUsage {
    pub fn new(
        cpu_usage_percent: f64,
        memory_usage_bytes: u64,
        memory_total_bytes: u64,
        disk_usage_bytes: u64,
        disk_total_bytes: u64,
        network_rx_bytes: u64,
        network_tx_bytes: u64,
    ) -> Self {
        Self {
            cpu_usage_percent,
            memory_usage_bytes,
            memory_total_bytes,
            disk_usage_bytes,
            disk_total_bytes,
            network_rx_bytes,
            network_tx_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_control_plane_metrics_creation() {
        let metrics = ControlPlaneMetrics::new();
        assert!(metrics.is_ok());
    }

    #[tokio::test]
    async fn test_job_metrics() {
        let metrics = ControlPlaneMetrics::new().unwrap();
        let job_id = Uuid::new_v4();
        let node_id = "test-node-1";

        metrics.record_job_scheduled(&job_id, node_id).await;
        metrics.record_job_started(&job_id, node_id).await;
        
        sleep(Duration::from_millis(100)).await;
        
        metrics.record_job_completed(&job_id, node_id, Duration::from_millis(100)).await;

        let snapshot = metrics.get_metrics_snapshot().await;
        assert_eq!(snapshot.jobs_scheduled_total, 1);
        assert_eq!(snapshot.jobs_completed_total, 1);
        assert_eq!(snapshot.jobs_running, 0);
    }

    #[tokio::test]
    async fn test_node_metrics() {
        let metrics = ControlPlaneMetrics::new().unwrap();
        let node_id = "test-node-1";

        metrics.record_node_joined(node_id).await;
        
        let usage = NodeResourceUsage::new(50.0, 1024*1024*1024, 2*1024*1024*1024, 
                                          10*1024*1024*1024, 100*1024*1024*1024, 
                                          1000, 2000);
        metrics.update_node_resource_usage(node_id, usage).await;

        let snapshot = metrics.get_metrics_snapshot().await;
        assert_eq!(snapshot.nodes_total, 1);
        assert_eq!(snapshot.nodes_healthy, 1);
    }

    #[tokio::test]
    async fn test_cluster_metrics() {
        let metrics = ControlPlaneMetrics::new().unwrap();

        metrics.update_cluster_metrics(5, 10, 75.0).await;
        metrics.record_leader_election("leader-node").await;

        let snapshot = metrics.get_metrics_snapshot().await;
        assert_eq!(snapshot.cluster_size, 5);
        assert_eq!(snapshot.cluster_capacity, 10);
        assert_eq!(snapshot.cluster_utilization_percent, 75.0);
        assert_eq!(snapshot.leader_elections_total, 1);
    }
}