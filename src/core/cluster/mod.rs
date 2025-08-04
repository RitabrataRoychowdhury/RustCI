//! Cluster management and coordination
//!
//! This module contains components for managing cluster nodes, coordination,
//! leadership election, failover, and control plane functionality.

pub mod cluster_coordinator;
pub mod control_plane;
pub mod control_plane_health;
pub mod control_plane_metrics;
pub mod control_plane_observability;
pub mod failover;
pub mod leader_election;
pub mod node_registry;

// Re-export commonly used types
pub use cluster_coordinator::ClusterCoordinator;
pub use node_registry::NodeRegistry;
pub use leader_election::LeaderElectionManager;