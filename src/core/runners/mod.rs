//! Runner lifecycle, discovery, and management
//!
//! This module contains components for runner pool management, discovery,
//! migration, scaling, health monitoring, and coordinated shutdown.

pub mod coordinated_shutdown;
pub mod distributed_health_monitoring;
pub mod dynamic_runner_scaling;
pub mod load_balancer;
pub mod runner_discovery;
pub mod runner_migration;
pub mod runner_pool;

// Re-export commonly used types
pub use distributed_health_monitoring::{
    DefaultDistributedHealthMonitor, DistributedHealthMonitor,
};
pub use runner_discovery::{DefaultRunnerDiscoveryService, RunnerDiscoveryService};
pub use runner_pool::{DefaultRunnerPoolManager, RunnerPoolManager};
