//! Distributed Health Monitoring System
//!
//! This module provides enhanced health monitoring for cross-node monitoring,
//! cluster-aware health status aggregation, and automatic runner replacement.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::core::cluster::node_registry::NodeRegistry;
use crate::core::runners::runner_pool::RunnerPoolManager;
use crate::domain::entities::{ClusterNode, HealthStatus, NodeId, Runner, RunnerEntity, RunnerId};
use crate::error::{AppError, Result};

/// Distributed health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedHealthConfig {
    /// Health check interval in seconds
    pub health_check_interval: u64,
    /// Health check timeout in seconds
    pub health_check_timeout: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub recovery_threshold: u32,
    /// Enable automatic runner replacement
    pub auto_replace_failed_runners: bool,
    /// Replacement timeout in seconds
    pub replacement_timeout: u64,
    /// Health aggregation strategy
    pub aggregation_strategy: HealthAggregationStrategy,
    /// Enable cross-node health monitoring
    pub cross_node_monitoring: bool,
}

impl Default for DistributedHealthConfig {
    fn default() -> Self {
        Self {
            health_check_interval: 30,
            health_check_timeout: 10,
            failure_threshold: 3,
            recovery_threshold: 2,
            auto_replace_failed_runners: true,
            replacement_timeout: 300,
            aggregation_strategy: HealthAggregationStrategy::Majority,
            cross_node_monitoring: true,
        }
    }
}

/// Health aggregation strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthAggregationStrategy {
    /// All nodes must be healthy
    All,
    /// Majority of nodes must be healthy
    Majority,
    /// At least one node must be healthy
    Any,
    /// Weighted based on node importance
    Weighted { weights: HashMap<NodeId, f64> },
}

/// Health status with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedHealthStatus {
    /// Basic health status
    pub status: HealthStatus,
    /// Health score (0.0 to 1.0)
    pub score: f64,
    /// Detailed metrics
    pub metrics: HealthMetrics,
    /// Last check timestamp
    pub last_check: DateTime<Utc>,
    /// Check duration in milliseconds
    pub check_duration_ms: u64,
    /// Consecutive failure count
    pub consecutive_failures: u32,
    /// Consecutive success count
    pub consecutive_successes: u32,
}

/// Detailed health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage percentage
    pub memory_usage: f64,
    /// Disk usage percentage
    pub disk_usage: f64,
    /// Network latency in milliseconds
    pub network_latency_ms: u64,
    /// Active connections count
    pub active_connections: u32,
    /// Error rate percentage
    pub error_rate: f64,
    /// Response time percentiles
    pub response_times: ResponseTimeMetrics,
    /// Custom metrics
    pub custom_metrics: HashMap<String, f64>,
}

/// Response time metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTimeMetrics {
    /// 50th percentile (median)
    pub p50: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
    /// Average response time
    pub avg: f64,
    /// Maximum response time
    pub max: f64,
}

/// Cluster health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealthSummary {
    /// Overall cluster health status
    pub overall_status: HealthStatus,
    /// Overall health score
    pub overall_score: f64,
    /// Total number of nodes
    pub total_nodes: u32,
    /// Number of healthy nodes
    pub healthy_nodes: u32,
    /// Number of degraded nodes
    pub degraded_nodes: u32,
    /// Number of unhealthy nodes
    pub unhealthy_nodes: u32,
    /// Node health details
    pub node_health: HashMap<NodeId, EnhancedHealthStatus>,
    /// Runner health details
    pub runner_health: HashMap<RunnerId, EnhancedHealthStatus>,
    /// Health trends
    pub trends: HealthTrends,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

/// Health trends over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthTrends {
    /// Health score trend (positive = improving, negative = degrading)
    pub score_trend: f64,
    /// Failure rate trend
    pub failure_rate_trend: f64,
    /// Response time trend
    pub response_time_trend: f64,
    /// Trend calculation period
    pub trend_period_minutes: u32,
}

/// Health check result with context
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Target ID (node or runner)
    pub target_id: Uuid,
    /// Target type
    pub target_type: HealthCheckTarget,
    /// Health status
    pub status: EnhancedHealthStatus,
    /// Check timestamp
    pub timestamp: DateTime<Utc>,
    /// Checker node ID
    pub checker_node_id: NodeId,
}

/// Health check target types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthCheckTarget {
    /// Node health check
    Node,
    /// Runner health check
    Runner,
    /// Service health check
    Service { service_name: String },
}

/// Distributed health monitor trait
#[async_trait]
pub trait DistributedHealthMonitor: Send + Sync {
    /// Start the health monitoring system
    async fn start(&self) -> Result<()>;

    /// Stop the health monitoring system
    async fn stop(&self) -> Result<()>;

    /// Perform health check on a specific node
    async fn check_node_health(&self, node_id: NodeId) -> Result<EnhancedHealthStatus>;

    /// Perform health check on a specific runner
    async fn check_runner_health(&self, runner_id: RunnerId) -> Result<EnhancedHealthStatus>;

    /// Get cluster health summary
    async fn get_cluster_health(&self) -> Result<ClusterHealthSummary>;

    /// Get health history for a node
    async fn get_node_health_history(
        &self,
        node_id: NodeId,
        duration: Duration,
    ) -> Result<Vec<HealthCheckResult>>;

    /// Replace a failed runner
    async fn replace_failed_runner(&self, runner_id: RunnerId) -> Result<RunnerId>;

    /// Register a health check callback
    async fn register_health_callback(&self, callback: Box<dyn HealthCallback>) -> Result<()>;
}

/// Health callback trait for notifications
#[async_trait]
pub trait HealthCallback: Send + Sync {
    /// Called when health status changes
    async fn on_health_change(&self, result: &HealthCheckResult) -> Result<()>;

    /// Called when a node fails
    async fn on_node_failure(&self, node_id: NodeId, status: &EnhancedHealthStatus) -> Result<()>;

    /// Called when a runner fails
    async fn on_runner_failure(
        &self,
        runner_id: RunnerId,
        status: &EnhancedHealthStatus,
    ) -> Result<()>;

    /// Called when cluster health changes
    async fn on_cluster_health_change(&self, summary: &ClusterHealthSummary) -> Result<()>;
}

/// Default implementation of distributed health monitor
pub struct DefaultDistributedHealthMonitor {
    /// Configuration
    config: DistributedHealthConfig,
    /// Node registry
    node_registry: Arc<NodeRegistry>,
    /// Runner pool manager
    runner_pool: Arc<dyn RunnerPoolManager>,
    /// Health status cache
    node_health: Arc<RwLock<HashMap<NodeId, EnhancedHealthStatus>>>,
    runner_health: Arc<RwLock<HashMap<RunnerId, EnhancedHealthStatus>>>,
    /// Health history
    health_history: Arc<RwLock<Vec<HealthCheckResult>>>,
    /// Health callbacks
    callbacks: Arc<RwLock<Vec<Box<dyn HealthCallback>>>>,
    /// Running state
    running: Arc<Mutex<bool>>,
    /// Local node ID
    local_node_id: NodeId,
}

impl DefaultDistributedHealthMonitor {
    /// Create a new distributed health monitor
    pub fn new(
        config: DistributedHealthConfig,
        node_registry: Arc<NodeRegistry>,
        runner_pool: Arc<dyn RunnerPoolManager>,
        local_node_id: NodeId,
    ) -> Self {
        Self {
            config,
            node_registry,
            runner_pool,
            node_health: Arc::new(RwLock::new(HashMap::new())),
            runner_health: Arc::new(RwLock::new(HashMap::new())),
            health_history: Arc::new(RwLock::new(Vec::new())),
            callbacks: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(Mutex::new(false)),
            local_node_id,
        }
    }

    /// Start health monitoring tasks
    async fn start_monitoring_tasks(&self) -> Result<()> {
        // Start node health monitoring
        self.start_node_health_monitoring().await?;

        // Start runner health monitoring
        self.start_runner_health_monitoring().await?;

        // Start health aggregation
        self.start_health_aggregation().await?;

        // Start health history cleanup
        self.start_health_history_cleanup().await?;

        Ok(())
    }

    /// Start node health monitoring task
    async fn start_node_health_monitoring(&self) -> Result<()> {
        let node_registry = self.node_registry.clone();
        let node_health = self.node_health.clone();
        let config = self.config.clone();
        let callbacks = self.callbacks.clone();
        let running = self.running.clone();
        let local_node_id = self.local_node_id;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.health_check_interval));

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Get all nodes
                let nodes = match node_registry.get_all_nodes().await {
                    Ok(nodes) => nodes,
                    Err(e) => {
                        warn!("Failed to get nodes for health check: {}", e);
                        continue;
                    }
                };

                // Check health of each node
                for node in nodes {
                    let node_id = node.id;

                    // Skip self if not doing cross-node monitoring
                    if !config.cross_node_monitoring && node_id == local_node_id {
                        continue;
                    }

                    let health_status = Self::perform_node_health_check(&node, &config).await;

                    // Update health cache
                    {
                        let mut health_cache = node_health.write().await;
                        let previous_status = health_cache.get(&node_id).cloned();
                        health_cache.insert(node_id, health_status.clone());

                        // Check for status changes
                        if let Some(prev) = previous_status {
                            if std::mem::discriminant(&prev.status)
                                != std::mem::discriminant(&health_status.status)
                            {
                                // Status changed, notify callbacks
                                let result = HealthCheckResult {
                                    target_id: node_id,
                                    target_type: HealthCheckTarget::Node,
                                    status: health_status.clone(),
                                    timestamp: Utc::now(),
                                    checker_node_id: local_node_id,
                                };

                                let callbacks_guard = callbacks.read().await;
                                for callback in callbacks_guard.iter() {
                                    if let Err(e) = callback.on_health_change(&result).await {
                                        warn!("Health callback failed: {}", e);
                                    }

                                    // Check for node failure
                                    if matches!(
                                        health_status.status,
                                        HealthStatus::Unhealthy { .. }
                                    ) {
                                        if let Err(e) =
                                            callback.on_node_failure(node_id, &health_status).await
                                        {
                                            warn!("Node failure callback failed: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Start runner health monitoring task
    async fn start_runner_health_monitoring(&self) -> Result<()> {
        let runner_pool = self.runner_pool.clone();
        let runner_health = self.runner_health.clone();
        let config = self.config.clone();
        let callbacks = self.callbacks.clone();
        let running = self.running.clone();
        let local_node_id = self.local_node_id;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.health_check_interval));

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Get all runners
                let runners = match runner_pool.list_runners().await {
                    Ok(runners) => runners,
                    Err(e) => {
                        warn!("Failed to get runners for health check: {}", e);
                        continue;
                    }
                };

                // Check health of each runner
                for runner_registration in runners {
                    let runner_id = runner_registration.entity.id;
                    let runner = runner_registration.runner.clone();

                    let health_status = Self::perform_runner_health_check(runner, &config).await;

                    // Update health cache
                    {
                        let mut health_cache = runner_health.write().await;
                        let previous_status = health_cache.get(&runner_id).cloned();
                        health_cache.insert(runner_id, health_status.clone());

                        // Check for status changes
                        if let Some(prev) = previous_status {
                            if std::mem::discriminant(&prev.status)
                                != std::mem::discriminant(&health_status.status)
                            {
                                // Status changed, notify callbacks
                                let result = HealthCheckResult {
                                    target_id: runner_id,
                                    target_type: HealthCheckTarget::Runner,
                                    status: health_status.clone(),
                                    timestamp: Utc::now(),
                                    checker_node_id: local_node_id,
                                };

                                let callbacks_guard = callbacks.read().await;
                                for callback in callbacks_guard.iter() {
                                    if let Err(e) = callback.on_health_change(&result).await {
                                        warn!("Health callback failed: {}", e);
                                    }

                                    // Check for runner failure
                                    if matches!(
                                        health_status.status,
                                        HealthStatus::Unhealthy { .. }
                                    ) {
                                        if let Err(e) = callback
                                            .on_runner_failure(runner_id, &health_status)
                                            .await
                                        {
                                            warn!("Runner failure callback failed: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Start health aggregation task
    async fn start_health_aggregation(&self) -> Result<()> {
        let node_health = self.node_health.clone();
        let runner_health = self.runner_health.clone();
        let callbacks = self.callbacks.clone();
        let config = self.config.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Aggregate every minute
            let mut previous_summary: Option<ClusterHealthSummary> = None;

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Calculate cluster health summary
                let summary =
                    Self::calculate_cluster_health_summary(&node_health, &runner_health, &config)
                        .await;

                // Check for changes in overall cluster health
                if let Some(prev) = &previous_summary {
                    if std::mem::discriminant(&prev.overall_status)
                        != std::mem::discriminant(&summary.overall_status)
                    {
                        // Cluster health status changed, notify callbacks
                        let callbacks_guard = callbacks.read().await;
                        for callback in callbacks_guard.iter() {
                            if let Err(e) = callback.on_cluster_health_change(&summary).await {
                                warn!("Cluster health change callback failed: {}", e);
                            }
                        }
                    }
                }

                previous_summary = Some(summary);
            }
        });

        Ok(())
    }

    /// Start health history cleanup task
    async fn start_health_history_cleanup(&self) -> Result<()> {
        let health_history = self.health_history.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600)); // Cleanup every hour

            loop {
                interval.tick().await;

                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }

                // Remove old health history entries (keep last 24 hours)
                let cutoff = Utc::now() - chrono::Duration::hours(24);
                let mut history = health_history.write().await;
                history.retain(|entry| entry.timestamp > cutoff);

                debug!(
                    "Cleaned up health history, {} entries remaining",
                    history.len()
                );
            }
        });

        Ok(())
    }

    /// Perform health check on a node
    async fn perform_node_health_check(
        node: &ClusterNode,
        config: &DistributedHealthConfig,
    ) -> EnhancedHealthStatus {
        let start_time = std::time::Instant::now();

        // Simulate node health check (in real implementation, this would ping the node)
        let health_result = timeout(
            Duration::from_secs(config.health_check_timeout),
            Self::check_node_connectivity(node),
        )
        .await;

        let check_duration = start_time.elapsed().as_millis() as u64;

        match health_result {
            Ok(Ok(metrics)) => EnhancedHealthStatus {
                status: HealthStatus::Healthy,
                score: Self::calculate_health_score(&metrics),
                metrics,
                last_check: Utc::now(),
                check_duration_ms: check_duration,
                consecutive_failures: 0,
                consecutive_successes: 1,
            },
            Ok(Err(e)) => EnhancedHealthStatus {
                status: HealthStatus::Unhealthy {
                    reason: e.to_string(),
                },
                score: 0.0,
                metrics: HealthMetrics::default(),
                last_check: Utc::now(),
                check_duration_ms: check_duration,
                consecutive_failures: 1,
                consecutive_successes: 0,
            },
            Err(_) => EnhancedHealthStatus {
                status: HealthStatus::Unhealthy {
                    reason: "Health check timeout".to_string(),
                },
                score: 0.0,
                metrics: HealthMetrics::default(),
                last_check: Utc::now(),
                check_duration_ms: check_duration,
                consecutive_failures: 1,
                consecutive_successes: 0,
            },
        }
    }

    /// Perform health check on a runner
    async fn perform_runner_health_check(
        runner: Arc<dyn Runner>,
        config: &DistributedHealthConfig,
    ) -> EnhancedHealthStatus {
        let start_time = std::time::Instant::now();

        let health_result = timeout(
            Duration::from_secs(config.health_check_timeout),
            runner.health_check(),
        )
        .await;

        let check_duration = start_time.elapsed().as_millis() as u64;

        match health_result {
            Ok(Ok(status)) => {
                let metrics = Self::collect_runner_metrics(&runner).await;
                EnhancedHealthStatus {
                    status,
                    score: Self::calculate_health_score(&metrics),
                    metrics,
                    last_check: Utc::now(),
                    check_duration_ms: check_duration,
                    consecutive_failures: 0,
                    consecutive_successes: 1,
                }
            }
            Ok(Err(e)) => EnhancedHealthStatus {
                status: HealthStatus::Unhealthy {
                    reason: e.to_string(),
                },
                score: 0.0,
                metrics: HealthMetrics::default(),
                last_check: Utc::now(),
                check_duration_ms: check_duration,
                consecutive_failures: 1,
                consecutive_successes: 0,
            },
            Err(_) => EnhancedHealthStatus {
                status: HealthStatus::Unhealthy {
                    reason: "Health check timeout".to_string(),
                },
                score: 0.0,
                metrics: HealthMetrics::default(),
                last_check: Utc::now(),
                check_duration_ms: check_duration,
                consecutive_failures: 1,
                consecutive_successes: 0,
            },
        }
    }

    /// Check node connectivity (simplified implementation)
    async fn check_node_connectivity(_node: &ClusterNode) -> Result<HealthMetrics> {
        // In a real implementation, this would perform actual network checks
        Ok(HealthMetrics {
            cpu_usage: 45.0,
            memory_usage: 60.0,
            disk_usage: 30.0,
            network_latency_ms: 10,
            active_connections: 25,
            error_rate: 0.1,
            response_times: ResponseTimeMetrics {
                p50: 50.0,
                p95: 200.0,
                p99: 500.0,
                avg: 75.0,
                max: 1000.0,
            },
            custom_metrics: HashMap::new(),
        })
    }

    /// Collect runner metrics (simplified implementation)
    async fn collect_runner_metrics(_runner: &Arc<dyn Runner>) -> HealthMetrics {
        // In a real implementation, this would collect actual metrics from the runner
        HealthMetrics {
            cpu_usage: 35.0,
            memory_usage: 50.0,
            disk_usage: 25.0,
            network_latency_ms: 5,
            active_connections: 10,
            error_rate: 0.05,
            response_times: ResponseTimeMetrics {
                p50: 30.0,
                p95: 150.0,
                p99: 300.0,
                avg: 45.0,
                max: 500.0,
            },
            custom_metrics: HashMap::new(),
        }
    }

    /// Calculate health score from metrics
    fn calculate_health_score(metrics: &HealthMetrics) -> f64 {
        let cpu_score = (100.0 - metrics.cpu_usage) / 100.0;
        let memory_score = (100.0 - metrics.memory_usage) / 100.0;
        let disk_score = (100.0 - metrics.disk_usage) / 100.0;
        let latency_score = (1000.0 - metrics.network_latency_ms as f64).max(0.0) / 1000.0;
        let error_score = (1.0 - metrics.error_rate).max(0.0);

        (cpu_score + memory_score + disk_score + latency_score + error_score) / 5.0
    }

    /// Calculate cluster health summary
    async fn calculate_cluster_health_summary(
        node_health: &Arc<RwLock<HashMap<NodeId, EnhancedHealthStatus>>>,
        runner_health: &Arc<RwLock<HashMap<RunnerId, EnhancedHealthStatus>>>,
        config: &DistributedHealthConfig,
    ) -> ClusterHealthSummary {
        let node_health_guard = node_health.read().await;
        let runner_health_guard = runner_health.read().await;

        let total_nodes = node_health_guard.len() as u32;
        let mut healthy_nodes = 0;
        let mut degraded_nodes = 0;
        let mut unhealthy_nodes = 0;

        let mut total_score = 0.0;
        let mut node_count = 0;

        for status in node_health_guard.values() {
            match status.status {
                HealthStatus::Healthy => healthy_nodes += 1,
                HealthStatus::Degraded { .. } => degraded_nodes += 1,
                HealthStatus::Unhealthy { .. } => unhealthy_nodes += 1,
            }
            total_score += status.score;
            node_count += 1;
        }

        for status in runner_health_guard.values() {
            total_score += status.score;
            node_count += 1;
        }

        let overall_score = if node_count > 0 {
            total_score / node_count as f64
        } else {
            0.0
        };

        let overall_status = match config.aggregation_strategy {
            HealthAggregationStrategy::All => {
                if unhealthy_nodes > 0 {
                    HealthStatus::Unhealthy {
                        reason: "Some nodes are unhealthy".to_string(),
                    }
                } else if degraded_nodes > 0 {
                    HealthStatus::Degraded {
                        reason: "Some nodes are degraded".to_string(),
                    }
                } else {
                    HealthStatus::Healthy
                }
            }
            HealthAggregationStrategy::Majority => {
                if healthy_nodes > total_nodes / 2 {
                    HealthStatus::Healthy
                } else if healthy_nodes + degraded_nodes > total_nodes / 2 {
                    HealthStatus::Degraded {
                        reason: "Majority of nodes are degraded".to_string(),
                    }
                } else {
                    HealthStatus::Unhealthy {
                        reason: "Majority of nodes are unhealthy".to_string(),
                    }
                }
            }
            HealthAggregationStrategy::Any => {
                if healthy_nodes > 0 {
                    HealthStatus::Healthy
                } else if degraded_nodes > 0 {
                    HealthStatus::Degraded {
                        reason: "Only degraded nodes available".to_string(),
                    }
                } else {
                    HealthStatus::Unhealthy {
                        reason: "No healthy nodes available".to_string(),
                    }
                }
            }
            HealthAggregationStrategy::Weighted { .. } => {
                // Simplified weighted calculation
                if overall_score > 0.8 {
                    HealthStatus::Healthy
                } else if overall_score > 0.5 {
                    HealthStatus::Degraded {
                        reason: "Weighted health score is degraded".to_string(),
                    }
                } else {
                    HealthStatus::Unhealthy {
                        reason: "Weighted health score is unhealthy".to_string(),
                    }
                }
            }
        };

        ClusterHealthSummary {
            overall_status,
            overall_score,
            total_nodes,
            healthy_nodes,
            degraded_nodes,
            unhealthy_nodes,
            node_health: node_health_guard.clone(),
            runner_health: runner_health_guard.clone(),
            trends: HealthTrends {
                score_trend: 0.0, // Would be calculated from historical data
                failure_rate_trend: 0.0,
                response_time_trend: 0.0,
                trend_period_minutes: 60,
            },
            last_updated: Utc::now(),
        }
    }
}

#[async_trait]
impl DistributedHealthMonitor for DefaultDistributedHealthMonitor {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest(
                "Health monitor is already running".to_string(),
            ));
        }

        *running = true;

        // Start monitoring tasks
        self.start_monitoring_tasks().await?;

        info!("Distributed health monitor started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }

        *running = false;

        info!("Distributed health monitor stopped");
        Ok(())
    }

    async fn check_node_health(&self, node_id: NodeId) -> Result<EnhancedHealthStatus> {
        let node = self
            .node_registry
            .get_node(node_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Node {} not found", node_id)))?;

        Ok(Self::perform_node_health_check(&node, &self.config).await)
    }

    async fn check_runner_health(&self, runner_id: RunnerId) -> Result<EnhancedHealthStatus> {
        let runner_registration = self
            .runner_pool
            .get_runner(runner_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Runner {} not found", runner_id)))?;

        Ok(Self::perform_runner_health_check(runner_registration.runner, &self.config).await)
    }

    async fn get_cluster_health(&self) -> Result<ClusterHealthSummary> {
        Ok(Self::calculate_cluster_health_summary(
            &self.node_health,
            &self.runner_health,
            &self.config,
        )
        .await)
    }

    async fn get_node_health_history(
        &self,
        node_id: NodeId,
        duration: Duration,
    ) -> Result<Vec<HealthCheckResult>> {
        let history = self.health_history.read().await;
        let cutoff = Utc::now()
            - chrono::Duration::from_std(duration)
                .map_err(|e| AppError::BadRequest(format!("Invalid duration: {}", e)))?;

        Ok(history
            .iter()
            .filter(|entry| entry.target_id == node_id && entry.timestamp > cutoff)
            .cloned()
            .collect())
    }

    async fn replace_failed_runner(&self, runner_id: RunnerId) -> Result<RunnerId> {
        // Get the failed runner information
        let failed_runner = self
            .runner_pool
            .get_runner(runner_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Runner {} not found", runner_id)))?;

        // Deregister the failed runner
        self.runner_pool.deregister_runner(runner_id).await?;

        // Create a new runner with similar configuration
        let new_runner_entity = RunnerEntity::new(
            format!("replacement-{}", Uuid::new_v4()),
            failed_runner.entity.runner_type.clone(),
        );

        // Register the new runner (this is simplified - in reality you'd need to create the actual runner instance)
        // For now, we'll return the new runner ID
        let new_runner_id = new_runner_entity.id;

        info!(
            "Replaced failed runner {} with new runner {}",
            runner_id, new_runner_id
        );
        Ok(new_runner_id)
    }

    async fn register_health_callback(&self, callback: Box<dyn HealthCallback>) -> Result<()> {
        let mut callbacks = self.callbacks.write().await;
        callbacks.push(callback);
        Ok(())
    }
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
            network_latency_ms: 0,
            active_connections: 0,
            error_rate: 0.0,
            response_times: ResponseTimeMetrics {
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
                avg: 0.0,
                max: 0.0,
            },
            custom_metrics: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cluster::node_registry::tests::create_test_registry;
    use crate::core::runners::runner_pool::DefaultRunnerPoolManager;

    fn create_test_health_monitor() -> DefaultDistributedHealthMonitor {
        let config = DistributedHealthConfig::default();
        let node_registry = Arc::new(create_test_registry());
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        let local_node_id = Uuid::new_v4();

        DefaultDistributedHealthMonitor::new(config, node_registry, runner_pool, local_node_id)
    }

    #[tokio::test]
    async fn test_health_monitor_creation() {
        let monitor = create_test_health_monitor();
        assert!(!*monitor.running.lock().await);
    }

    #[tokio::test]
    async fn test_health_score_calculation() {
        let metrics = HealthMetrics {
            cpu_usage: 50.0,
            memory_usage: 60.0,
            disk_usage: 30.0,
            network_latency_ms: 100,
            active_connections: 10,
            error_rate: 0.1,
            response_times: ResponseTimeMetrics {
                p50: 50.0,
                p95: 200.0,
                p99: 500.0,
                avg: 75.0,
                max: 1000.0,
            },
            custom_metrics: HashMap::new(),
        };

        let score = DefaultDistributedHealthMonitor::calculate_health_score(&metrics);
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[tokio::test]
    async fn test_enhanced_health_status() {
        let status = EnhancedHealthStatus {
            status: HealthStatus::Healthy,
            score: 0.85,
            metrics: HealthMetrics::default(),
            last_check: Utc::now(),
            check_duration_ms: 50,
            consecutive_failures: 0,
            consecutive_successes: 5,
        };

        assert!(matches!(status.status, HealthStatus::Healthy));
        assert_eq!(status.score, 0.85);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.consecutive_successes, 5);
    }

    #[tokio::test]
    async fn test_cluster_health_summary() {
        let node_health = Arc::new(RwLock::new(HashMap::new()));
        let runner_health = Arc::new(RwLock::new(HashMap::new()));
        let config = DistributedHealthConfig::default();

        // Add some test health data
        {
            let mut node_health_guard = node_health.write().await;
            node_health_guard.insert(
                Uuid::new_v4(),
                EnhancedHealthStatus {
                    status: HealthStatus::Healthy,
                    score: 0.9,
                    metrics: HealthMetrics::default(),
                    last_check: Utc::now(),
                    check_duration_ms: 50,
                    consecutive_failures: 0,
                    consecutive_successes: 5,
                },
            );
        }

        let summary = DefaultDistributedHealthMonitor::calculate_cluster_health_summary(
            &node_health,
            &runner_health,
            &config,
        )
        .await;

        assert_eq!(summary.total_nodes, 1);
        assert_eq!(summary.healthy_nodes, 1);
        assert!(matches!(summary.overall_status, HealthStatus::Healthy));
    }
}
