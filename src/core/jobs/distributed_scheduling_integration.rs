//! Distributed Scheduling Integration Module
//!
//! This module provides a comprehensive integration layer that combines
//! the distributed job scheduler, failure recovery, and cluster coordination
//! into a unified distributed scheduling system.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use crate::core::cluster::cluster_coordinator::ClusterCoordinator;
use crate::core::jobs::distributed_job_scheduler::{
    DistributedJobScheduler, DistributedJobSchedulerConfig, ReschedulingReason, JobPlacement, DistributedJobSchedulerStats,
};
use crate::core::patterns::events::EventBus;
use crate::core::jobs::job_failure_recovery::{JobFailureRecovery, FailureRecoveryConfig, JobFailure, FailurePattern, FailureRecoveryStats};
use crate::core::jobs::job_scheduler::JobScheduler;

use crate::core::cluster::node_registry::{NodeRegistry, NodeRegistryStats};
use crate::core::runners::runner_pool::RunnerPoolManager;
use crate::domain::entities::{Job, JobId, JobStatus};
use crate::error::Result;

/// Configuration for the distributed scheduling system
#[derive(Debug, Clone)]
pub struct DistributedSchedulingConfig {
    /// Distributed job scheduler configuration
    pub scheduler_config: DistributedJobSchedulerConfig,
    /// Failure recovery configuration
    pub failure_recovery_config: FailureRecoveryConfig,
    /// Enable comprehensive monitoring
    pub enable_monitoring: bool,
    /// Enable performance optimization
    pub enable_optimization: bool,
}

impl Default for DistributedSchedulingConfig {
    fn default() -> Self {
        Self {
            scheduler_config: DistributedJobSchedulerConfig::default(),
            failure_recovery_config: FailureRecoveryConfig::default(),
            enable_monitoring: true,
            enable_optimization: true,
        }
    }
}

/// Comprehensive distributed scheduling system
pub struct DistributedSchedulingSystem {
    /// Configuration
    config: DistributedSchedulingConfig,
    /// Distributed job scheduler
    scheduler: Arc<DistributedJobScheduler>,
    /// Failure recovery system
    failure_recovery: Arc<JobFailureRecovery>,
    /// Cluster coordinator
    cluster_coordinator: Arc<ClusterCoordinator>,
    /// Node registry
    node_registry: Arc<NodeRegistry>,
    /// Event bus
    event_bus: Arc<EventBus>,
    /// System state
    state: Arc<RwLock<SystemState>>,
}

/// System state tracking
#[derive(Debug, Clone)]
struct SystemState {
    /// System is running
    is_running: bool,
    /// System start time
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Total jobs processed
    total_jobs_processed: u64,
    /// System health status
    health_status: SystemHealthStatus,
}

/// System health status
#[derive(Debug, Clone, PartialEq)]
enum SystemHealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

impl DistributedSchedulingSystem {
    /// Create a new distributed scheduling system
    pub async fn new(
        config: DistributedSchedulingConfig,
        runner_pool: Arc<dyn RunnerPoolManager>,
        cluster_coordinator: Arc<ClusterCoordinator>,
        node_registry: Arc<NodeRegistry>,
        event_bus: Arc<EventBus>,
    ) -> Result<Self> {
        info!("Creating distributed scheduling system");

        // Create distributed job scheduler directly
        let base_scheduler = Arc::new(crate::core::jobs::job_scheduler::DefaultJobScheduler::new(
            runner_pool,
            config.scheduler_config.base_config.clone(),
        ));

        let distributed_scheduler = Arc::new(DistributedJobScheduler::new(
            config.scheduler_config.clone(),
            base_scheduler,
            cluster_coordinator.clone(),
            node_registry.clone(),
            event_bus.clone(),
        ));

        // Create failure recovery system
        let failure_recovery = Arc::new(JobFailureRecovery::new(
            config.failure_recovery_config.clone(),
            distributed_scheduler.clone(),
            event_bus.clone(),
        ));

        let system = Self {
            config,
            scheduler: distributed_scheduler.clone(),
            failure_recovery,
            cluster_coordinator,
            node_registry,
            event_bus,
            state: Arc::new(RwLock::new(SystemState {
                is_running: false,
                started_at: None,
                total_jobs_processed: 0,
                health_status: SystemHealthStatus::Healthy,
            })),
        };

        info!("Distributed scheduling system created successfully");
        Ok(system)
    }

    /// Start the distributed scheduling system
    pub async fn start(&self) -> Result<()> {
        info!("Starting distributed scheduling system");

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = true;
            state.started_at = Some(chrono::Utc::now());
            state.health_status = SystemHealthStatus::Healthy;
        }

        // Start cluster coordinator
        self.cluster_coordinator.start().await?;

        // Start node registry
        self.node_registry.start().await?;

        // Start distributed scheduler
        self.scheduler.start().await?;

        // Start failure recovery system
        self.failure_recovery.start().await?;

        // Start monitoring if enabled
        if self.config.enable_monitoring {
            self.start_monitoring_tasks().await;
        }

        // Start optimization if enabled
        if self.config.enable_optimization {
            self.start_optimization_tasks().await;
        }

        info!("Distributed scheduling system started successfully");
        Ok(())
    }

    /// Stop the distributed scheduling system
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping distributed scheduling system");

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = false;
        }

        // Stop components in reverse order
        if let Err(e) = self.scheduler.stop().await {
            error!("Error stopping scheduler: {}", e);
        }

        if let Err(e) = self.cluster_coordinator.stop().await {
            error!("Error stopping cluster coordinator: {}", e);
        }

        info!("Distributed scheduling system stopped");
        Ok(())
    }

    /// Schedule a job using the distributed system
    pub async fn schedule_job(&self, job: Job) -> Result<()> {
        // Check system health
        self.check_system_health().await?;

        // Schedule job
        let result = self.scheduler.schedule_job(job.clone()).await;

        // Update job counter
        if result.is_ok() {
            let mut state = self.state.write().await;
            state.total_jobs_processed += 1;
        }

        result
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: JobId) -> Result<Option<JobStatus>> {
        self.scheduler.get_job_status(job_id).await
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: JobId) -> Result<bool> {
        self.scheduler.cancel_job(job_id).await
    }

    /// Get comprehensive system statistics
    pub async fn get_system_stats(&self) -> Result<SystemStats> {
        let scheduler_stats = self.scheduler.get_distributed_stats().await;
        let failure_recovery_stats = self.failure_recovery.get_stats().await;
        let cluster_metrics = self.cluster_coordinator.get_cluster_metrics().await;
        let node_registry_stats = self.node_registry.get_stats().await;

        let state = self.state.read().await;

        Ok(SystemStats {
            system_state: state.clone(),
            scheduler_stats,
            failure_recovery_stats,
            cluster_metrics,
            node_registry_stats,
        })
    }

    /// Get system health status
    pub async fn get_health_status(&self) -> SystemHealthStatus {
        let state = self.state.read().await;
        state.health_status.clone()
    }

    /// Check and update system health
    async fn check_system_health(&self) -> Result<()> {
        let cluster_status = self.cluster_coordinator.get_cluster_status().await;
        let node_count = self.node_registry.active_node_count().await;

        let health_status = if node_count == 0 {
            SystemHealthStatus::Unhealthy {
                reason: "No active nodes available".to_string(),
            }
        } else if matches!(cluster_status, crate::domain::entities::ClusterStatus::Degraded { .. }) {
            SystemHealthStatus::Degraded {
                reason: "Cluster is in degraded state".to_string(),
            }
        } else if matches!(cluster_status, crate::domain::entities::ClusterStatus::Unhealthy { .. }) {
            SystemHealthStatus::Unhealthy {
                reason: "Cluster is unhealthy".to_string(),
            }
        } else {
            SystemHealthStatus::Healthy
        };

        // Update state
        {
            let mut state = self.state.write().await;
            state.health_status = health_status.clone();
        }

        // Return error if system is unhealthy
        match health_status {
            SystemHealthStatus::Unhealthy { reason } => {
                Err(crate::error::AppError::ValidationError(format!(
                    "System is unhealthy: {}",
                    reason
                )))
            }
            _ => Ok(()),
        }
    }

    /// Start monitoring background tasks
    async fn start_monitoring_tasks(&self) {
        let state = self.state.clone();
        let cluster_coordinator = self.cluster_coordinator.clone();
        let node_registry = self.node_registry.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Check system health
                let cluster_status = cluster_coordinator.get_cluster_status().await;
                let node_count = node_registry.active_node_count().await;

                let health_status = if node_count == 0 {
                    SystemHealthStatus::Unhealthy {
                        reason: "No active nodes available".to_string(),
                    }
                } else if matches!(cluster_status, crate::domain::entities::ClusterStatus::Degraded { .. }) {
                    SystemHealthStatus::Degraded {
                        reason: "Cluster is in degraded state".to_string(),
                    }
                } else if matches!(cluster_status, crate::domain::entities::ClusterStatus::Unhealthy { .. }) {
                    SystemHealthStatus::Unhealthy {
                        reason: "Cluster is unhealthy".to_string(),
                    }
                } else {
                    SystemHealthStatus::Healthy
                };

                // Update state
                {
                    let mut state_guard = state.write().await;
                    state_guard.health_status = health_status;
                }
            }
        });
    }

    /// Start optimization background tasks
    async fn start_optimization_tasks(&self) {
        let scheduler = self.scheduler.clone();
        let cluster_coordinator = self.cluster_coordinator.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                // Perform optimization tasks
                if let Err(e) = Self::perform_optimization(&scheduler, &cluster_coordinator).await {
                    error!("Optimization task failed: {}", e);
                }
            }
        });
    }

    /// Perform system optimization
    async fn perform_optimization(
        _scheduler: &Arc<DistributedJobScheduler>,
        _cluster_coordinator: &Arc<ClusterCoordinator>,
    ) -> Result<()> {
        // This would implement various optimization strategies:
        // - Load balancing adjustments
        // - Resource allocation optimization
        // - Job priority adjustments
        // - Node utilization optimization

        // For now, this is a placeholder
        info!("Performing system optimization");
        Ok(())
    }

    /// Get job placement information
    pub async fn get_job_placement(&self, job_id: JobId) -> Option<JobPlacement> {
        self.scheduler.get_job_placement(job_id).await
    }

    /// Get failure recovery history
    pub async fn get_failure_history(&self) -> Vec<JobFailure> {
        self.failure_recovery.get_failure_history(Some(50)).await
    }

    /// Get detected failure patterns
    pub async fn get_failure_patterns(&self) -> Vec<FailurePattern> {
        self.failure_recovery.get_detected_patterns().await
    }

    /// Manually trigger job rescheduling
    pub async fn reschedule_job(
        &self,
        job_id: JobId,
        reason: ReschedulingReason,
    ) -> Result<JobPlacement> {
        self.scheduler.reschedule_job(job_id, reason).await
    }
}

/// Comprehensive system statistics
#[derive(Debug, Clone)]
pub struct SystemStats {
    /// System state information
    pub system_state: SystemState,
    /// Distributed scheduler statistics
    pub scheduler_stats: DistributedJobSchedulerStats,
    /// Failure recovery statistics
    pub failure_recovery_stats: FailureRecoveryStats,
    /// Cluster metrics
    pub cluster_metrics: crate::domain::entities::ClusterMetrics,
    /// Node registry statistics
    pub node_registry_stats: NodeRegistryStats,
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runners::runner_pool::{DefaultRunnerPoolManager, RunnerPoolConfig};

    #[test]
    fn test_system_health_status() {
        assert_eq!(SystemHealthStatus::Healthy, SystemHealthStatus::Healthy);
        assert_ne!(
            SystemHealthStatus::Healthy,
            SystemHealthStatus::Degraded {
                reason: "test".to_string()
            }
        );
    }

    #[test]
    fn test_distributed_scheduling_config() {
        let config = DistributedSchedulingConfig::default();
        assert!(config.enable_monitoring);
        assert!(config.enable_optimization);
    }

    #[tokio::test]
    async fn test_system_state() {
        let state = SystemState {
            is_running: false,
            started_at: None,
            total_jobs_processed: 0,
            health_status: SystemHealthStatus::Healthy,
        };

        assert!(!state.is_running);
        assert_eq!(state.total_jobs_processed, 0);
        assert_eq!(state.health_status, SystemHealthStatus::Healthy);
    }
}