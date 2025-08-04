//! Job Scheduler Factory for creating different types of schedulers
//!
//! This module provides a factory for creating job schedulers with different
//! capabilities, including distributed scheduling for cluster environments.

use std::sync::Arc;

use crate::core::cluster::cluster_coordinator::ClusterCoordinator;
use crate::core::jobs::distributed_job_scheduler::{
    DistributedJobScheduler, DistributedJobSchedulerConfig,
};
use crate::core::patterns::events::EventBus;
use crate::core::jobs::job_scheduler::{DefaultJobScheduler, JobScheduler, JobSchedulerConfig};
use crate::core::cluster::node_registry::NodeRegistry;
use crate::core::runners::runner_pool::RunnerPoolManager;
use crate::error::Result;

/// Job scheduler factory for creating different scheduler types
pub struct JobSchedulerFactory;

/// Job scheduler type configuration
#[derive(Debug, Clone)]
pub enum JobSchedulerType {
    /// Standard single-node scheduler
    Standard {
        config: JobSchedulerConfig,
    },
    /// Distributed cluster-aware scheduler
    Distributed {
        config: DistributedJobSchedulerConfig,
    },
}

impl JobSchedulerFactory {
    /// Create a new job scheduler based on the specified type
    pub async fn create_scheduler(
        scheduler_type: JobSchedulerType,
        runner_pool: Arc<dyn RunnerPoolManager>,
        cluster_coordinator: Option<Arc<ClusterCoordinator>>,
        node_registry: Option<Arc<NodeRegistry>>,
        event_bus: Option<Arc<EventBus>>,
    ) -> Result<Arc<dyn JobScheduler>> {
        match scheduler_type {
            JobSchedulerType::Standard { config } => {
                let scheduler = DefaultJobScheduler::new(runner_pool, config);
                Ok(Arc::new(scheduler))
            }
            JobSchedulerType::Distributed { config } => {
                // Create base scheduler
                let base_scheduler = Arc::new(DefaultJobScheduler::new(
                    runner_pool,
                    config.base_config.clone(),
                ));

                // Ensure required dependencies are provided
                let cluster_coordinator = cluster_coordinator.ok_or_else(|| {
                    crate::error::AppError::ValidationError(
                        "ClusterCoordinator required for distributed scheduler".to_string(),
                    )
                })?;

                let node_registry = node_registry.ok_or_else(|| {
                    crate::error::AppError::ValidationError(
                        "NodeRegistry required for distributed scheduler".to_string(),
                    )
                })?;

                let event_bus = event_bus.ok_or_else(|| {
                    crate::error::AppError::ValidationError(
                        "EventBus required for distributed scheduler".to_string(),
                    )
                })?;

                // Create distributed scheduler
                let distributed_scheduler = DistributedJobScheduler::new(
                    config,
                    base_scheduler,
                    cluster_coordinator,
                    node_registry,
                    event_bus,
                );

                Ok(Arc::new(distributed_scheduler))
            }
        }
    }

    /// Create a standard job scheduler with default configuration
    pub async fn create_standard_scheduler(
        runner_pool: Arc<dyn RunnerPoolManager>,
    ) -> Result<Arc<dyn JobScheduler>> {
        Self::create_scheduler(
            JobSchedulerType::Standard {
                config: JobSchedulerConfig::default(),
            },
            runner_pool,
            None,
            None,
            None,
        )
        .await
    }

    /// Create a distributed job scheduler with cluster dependencies
    pub async fn create_distributed_scheduler(
        runner_pool: Arc<dyn RunnerPoolManager>,
        cluster_coordinator: Arc<ClusterCoordinator>,
        node_registry: Arc<NodeRegistry>,
        event_bus: Arc<EventBus>,
        config: Option<DistributedJobSchedulerConfig>,
    ) -> Result<Arc<dyn JobScheduler>> {
        let config = config.unwrap_or_default();

        Self::create_scheduler(
            JobSchedulerType::Distributed { config },
            runner_pool,
            Some(cluster_coordinator),
            Some(node_registry),
            Some(event_bus),
        )
        .await
    }

    /// Detect the appropriate scheduler type based on available dependencies
    pub fn detect_scheduler_type(
        has_cluster_coordinator: bool,
        has_node_registry: bool,
        has_event_bus: bool,
    ) -> JobSchedulerType {
        if has_cluster_coordinator && has_node_registry && has_event_bus {
            JobSchedulerType::Distributed {
                config: DistributedJobSchedulerConfig::default(),
            }
        } else {
            JobSchedulerType::Standard {
                config: JobSchedulerConfig::default(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runners::runner_pool::{DefaultRunnerPoolManager, RunnerPoolConfig};

    #[tokio::test]
    async fn test_create_standard_scheduler() {
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        
        let scheduler = JobSchedulerFactory::create_standard_scheduler(runner_pool)
            .await
            .unwrap();

        // Verify scheduler was created
        assert!(scheduler.get_stats().await.is_ok());
    }

    #[test]
    fn test_scheduler_type_detection() {
        // Test detection with all dependencies
        let scheduler_type = JobSchedulerFactory::detect_scheduler_type(true, true, true);
        assert!(matches!(scheduler_type, JobSchedulerType::Distributed { .. }));

        // Test detection without dependencies
        let scheduler_type = JobSchedulerFactory::detect_scheduler_type(false, false, false);
        assert!(matches!(scheduler_type, JobSchedulerType::Standard { .. }));

        // Test detection with partial dependencies
        let scheduler_type = JobSchedulerFactory::detect_scheduler_type(true, false, false);
        assert!(matches!(scheduler_type, JobSchedulerType::Standard { .. }));
    }
}