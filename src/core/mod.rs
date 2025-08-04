//! Core domain logic and foundational patterns
//!
//! This module contains the core architectural patterns and domain logic
//! that form the foundation of the RustCI system. It's organized into
//! logical subdirectories for better maintainability and separation of concerns.

// Cluster management and coordination
pub mod cluster;

// Job scheduling, execution, and management
pub mod jobs;

// Runner lifecycle, discovery, and management
pub mod runners;

// Networking, communication, and security
pub mod networking;

// Observability, monitoring, and diagnostics
pub mod observability;

// Architectural patterns and design patterns
pub mod patterns;

// Core infrastructure services and utilities
pub mod infrastructure;

// Re-export commonly used types for easy access
pub use cluster::{ClusterCoordinator, NodeRegistry, LeaderElectionManager};
pub use jobs::{DefaultJobScheduler, JobQueue, InMemoryJobQueue, DistributedJobScheduler};
pub use runners::{RunnerPoolManager, DefaultRunnerPoolManager, RunnerDiscoveryService, DefaultRunnerDiscoveryService};
pub use networking::{Transport, Connection, TransportConfig};
pub use observability::{MetricsCollector};
pub use patterns::{CommandHandler, EventBus, EventHandler};
pub use infrastructure::{ServiceContainer, ServiceFactory, Service, ServiceDecorator};

// Re-export specific commonly used items
pub use patterns::correlation::CorrelationTracker;
pub use networking::security::{JwtManager, Permission, SecurityContext, AuditAction, AuditEvent};

/// Core result type used throughout the system
pub type CoreResult<T> = crate::error::Result<T>;
