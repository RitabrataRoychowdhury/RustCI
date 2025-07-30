//! Core domain logic and foundational patterns
//!
//! This module contains the core architectural patterns and domain logic
//! that form the foundation of the RustCI system.

pub mod async_jobs;
pub mod audit;
pub mod caching;
pub mod cluster_coordinator;
pub mod commands;
pub mod concurrency;
pub mod correlation;
pub mod dependency_injection;
pub mod error_manager;
pub mod event_loop;
pub mod event_sourcing;
pub mod events;
pub mod integration_example;
pub mod job_queue;
pub mod logging;
pub mod metrics;
pub mod monitoring;
pub mod node_registry;
pub mod observability;
pub mod profiling;
pub mod projections;
pub mod prometheus_metrics;
pub mod queries;
pub mod resilience;
pub mod runtime_optimization;
pub mod sagas;
pub mod security;
pub mod service_decorators;
pub mod traits;

// Re-export core types for easy access
pub use correlation::CorrelationTracker;
pub use dependency_injection::{ServiceContainer, ServiceFactory, ServiceLifetime, ServiceScope};
pub use resilience::{BulkheadConfig, BulkheadManager, BulkheadStats};
pub use service_decorators::{
    CircuitBreakerConfig, RetryConfig, Service, ServiceContext, ServiceDecorator,
};
// Removed unused imports - these can be imported directly when needed

/// Core result type used throughout the system
pub type CoreResult<T> = crate::error::Result<T>;
