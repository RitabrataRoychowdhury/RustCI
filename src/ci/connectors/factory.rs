//! Connector factory implementations
//!
//! This module provides the factory pattern implementation for creating
//! connector instances. It includes the built-in factory and supports
//! dynamic registration of custom factories.

use super::traits::{Connector, ConnectorType};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Factory trait for creating connectors
#[async_trait]
pub trait ConnectorFactory: Send + Sync {
    /// Create a connector instance for the given type
    fn create_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>>;

    /// Check if this factory supports the given connector type
    fn supports_type(&self, connector_type: &ConnectorType) -> bool;

    /// Get the name of this factory
    fn name(&self) -> &str;
}

/// Built-in connector factory that provides default implementations
pub struct BuiltInConnectorFactory;

impl BuiltInConnectorFactory {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BuiltInConnectorFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConnectorFactory for BuiltInConnectorFactory {
    fn create_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>> {
        debug!("🏭 Creating connector for type: {}", connector_type);

        match connector_type {
            ConnectorType::Docker => {
                info!("📦 Creating Docker connector");
                Ok(Arc::new(super::DockerConnector::new()))
            }
            ConnectorType::Kubernetes => {
                info!("☸️ Creating Kubernetes connector");
                Ok(Arc::new(super::KubernetesConnector::new()))
            }
            ConnectorType::AWS => {
                info!("☁️ Creating AWS connector");
                Ok(Arc::new(super::AWSConnector::new()))
            }
            ConnectorType::Azure => {
                info!("🔷 Creating Azure connector");
                Ok(Arc::new(super::AzureConnector::new()))
            }
            ConnectorType::GCP => {
                info!("🌐 Creating GCP connector");
                Ok(Arc::new(super::GCPConnector::new()))
            }
            ConnectorType::GitHub => {
                info!("🐙 Creating GitHub connector");
                Ok(Arc::new(super::GitHubConnector::new()))
            }
            ConnectorType::GitLab => {
                info!("🦊 Creating GitLab connector");
                Ok(Arc::new(super::GitLabConnector::new()))
            }
            ConnectorType::Custom(name) => {
                warn!(
                    "🔧 Custom connector requested but not implemented: {}",
                    name
                );
                Err(AppError::Unimplemented(format!(
                    "Custom connector: {}",
                    name
                )))
            }
        }
    }

    fn supports_type(&self, connector_type: &ConnectorType) -> bool {
        matches!(
            connector_type,
            ConnectorType::Docker
                | ConnectorType::Kubernetes
                | ConnectorType::AWS
                | ConnectorType::Azure
                | ConnectorType::GCP
                | ConnectorType::GitHub
                | ConnectorType::GitLab
        )
    }

    fn name(&self) -> &str {
        "built-in"
    }
}
