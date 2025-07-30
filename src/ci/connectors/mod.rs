//! Connector system for CI/CD pipeline execution
//! 
//! This module provides a modular, extensible architecture for executing
//! CI/CD steps across different platforms and services using the Strategy,
//! Factory, and Facade design patterns.
//!
//! ## Architecture
//! 
//! The connector system is built around three main design patterns:
//! - **Strategy Pattern**: Each connector implements the `Connector` trait
//! - **Factory Pattern**: `ConnectorFactory` creates connector instances
//! - **Facade Pattern**: `ConnectorManager` provides a unified interface
//!
//! ## Usage
//! 
//! ```rust,no_run
//! use crate::ci::connectors::{ConnectorManager, ConnectorType};
//! 
//! let mut manager = ConnectorManager::new();
//! let connector = manager.get_connector(ConnectorType::Docker)?;
//! ```

pub mod traits;
pub mod factory;
pub mod manager;
pub mod docker;
pub mod kubernetes;
pub mod cloud;
pub mod git;

#[cfg(test)]
pub mod tests;

#[cfg(test)]
pub mod integration_tests;

// Re-export core types and traits for easy access
pub use traits::{
    Connector, ConnectorType, ExecutionResult, KubernetesConfig,
    LifecycleHook, LifecycleHookType, MongoOperation, SecurityContext
};
pub use factory::{ConnectorFactory, BuiltInConnectorFactory};
pub use manager::ConnectorManager;

// Re-export connector implementations
pub use docker::DockerConnector;
pub use kubernetes::KubernetesConnector;

// Re-export cloud connector implementations
pub use cloud::{AWSConnector, AzureConnector, GCPConnector};

// Re-export git connector implementations
pub use git::{GitHubConnector, GitLabConnector};

// Convenience type aliases for common use cases
pub type ConnectorResult<T> = crate::error::Result<T>;
pub type ConnectorInstance = std::sync::Arc<dyn Connector>;

/// Version information for the connector system
pub const CONNECTOR_SYSTEM_VERSION: &str = "1.0.0";

/// Default timeout for connector operations (in seconds)
pub const DEFAULT_CONNECTOR_TIMEOUT: u64 = 300;