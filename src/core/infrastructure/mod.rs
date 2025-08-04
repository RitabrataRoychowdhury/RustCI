//! Core infrastructure services and utilities
//!
//! This module contains foundational infrastructure components like
//! caching, concurrency, dependency injection, and service decorators.

pub mod caching;
pub mod concurrency;
pub mod dependency_injection;
pub mod error_manager;
pub mod event_loop;
pub mod integration_example;
pub mod resilience;
pub mod runtime_optimization;
pub mod service_decorators;
pub mod state_snapshot;
pub mod traits;

// Re-export commonly used types
pub use dependency_injection::{ServiceContainer, ServiceFactory};
pub use service_decorators::{Service, ServiceDecorator};
pub use resilience::{BulkheadManager, BulkheadConfig};