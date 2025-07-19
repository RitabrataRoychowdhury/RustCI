//! Connector manager implementation
//! 
//! This module provides the facade pattern implementation for managing
//! connectors. It handles factory registration, connector caching, and
//! provides a unified interface for connector operations.

use crate::error::{AppError, Result};
use super::traits::{Connector, ConnectorType, ExecutionResult};
use super::factory::{ConnectorFactory, BuiltInConnectorFactory};
use crate::ci::{config::{Step, StepType}, workspace::Workspace};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Connector manager that provides a unified facade for all connector operations
pub struct ConnectorManager {
    factories: HashMap<String, Arc<dyn ConnectorFactory>>,
    connector_cache: HashMap<ConnectorType, Arc<dyn Connector>>,
}

impl ConnectorManager {
    /// Create a new connector manager with default built-in factory
    pub fn new() -> Self {
        let mut manager = Self {
            factories: HashMap::new(),
            connector_cache: HashMap::new(),
        };

        // Register the built-in factory
        manager.register_factory(Arc::new(BuiltInConnectorFactory::new()));
        
        info!("🎯 ConnectorManager initialized with built-in factory");
        manager
    }

    /// Register a connector factory
    pub fn register_factory(&mut self, factory: Arc<dyn ConnectorFactory>) {
        let name = factory.name().to_string();
        info!("🏭 Registering connector factory: {}", name);
        self.factories.insert(name, factory);
    }

    /// Get a connector for the specified type
    pub fn get_connector(&mut self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>> {
        // Check cache first
        if let Some(connector) = self.connector_cache.get(&connector_type) {
            debug!("♻️ Using cached connector for type: {}", connector_type);
            return Ok(Arc::clone(connector));
        }

        // Find a factory that supports this connector type
        for factory in self.factories.values() {
            if factory.supports_type(&connector_type) {
                debug!("🔍 Found factory {} for connector type: {}", factory.name(), connector_type);
                let connector = factory.create_connector(connector_type.clone())?;
                
                // Cache the connector for future use
                self.connector_cache.insert(connector_type.clone(), Arc::clone(&connector));
                
                return Ok(connector);
            }
        }

        error!("❌ No factory found for connector type: {}", connector_type);
        Err(AppError::NotFound(format!("No factory found for connector type: {}", connector_type)))
    }

    /// Execute a step using the appropriate connector
    pub async fn execute_step(
        &mut self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        let connector_type = self.determine_connector_type(step)?;
        debug!("🎯 Determined connector type: {} for step: {}", connector_type, step.name);

        let connector = self.get_connector(connector_type)?;
        
        // Validate configuration
        connector.validate_config(step)?;
        
        // Execute pre-hook
        connector.pre_execute(step).await?;
        
        // Execute the step
        info!("🚀 Executing step '{}' with connector '{}'", step.name, connector.name());
        let result = connector.execute_step(step, workspace, env).await;
        
        match &result {
            Ok(exec_result) => {
                info!("✅ Step '{}' completed with exit code: {}", step.name, exec_result.exit_code);
                // Execute post-hook
                if let Err(e) = connector.post_execute(step, exec_result).await {
                    warn!("⚠️ Post-execution hook failed for step '{}': {}", step.name, e);
                }
            },
            Err(e) => {
                error!("❌ Step '{}' failed: {}", step.name, e);
            }
        }
        
        result
    }

    /// Determine the connector type based on the step configuration
    pub fn determine_connector_type(&self, step: &Step) -> Result<ConnectorType> {
        let connector_type = match step.step_type {
            StepType::Docker => ConnectorType::Docker,
            StepType::Kubernetes => ConnectorType::Kubernetes,
            StepType::AWS => ConnectorType::AWS,
            StepType::Azure => ConnectorType::Azure,
            StepType::GCP => ConnectorType::GCP,
            StepType::GitHub => ConnectorType::GitHub,
            StepType::GitLab => ConnectorType::GitLab,
            StepType::Shell => {
                // For shell steps, we default to Docker for containerized execution
                debug!("🐚 Shell step detected, defaulting to Docker connector");
                ConnectorType::Docker
            },
            StepType::Custom => {
                if let Some(plugin_name) = &step.config.plugin_name {
                    ConnectorType::Custom(plugin_name.clone())
                } else {
                    return Err(AppError::ValidationError(
                        "Custom step type requires plugin_name in config".to_string()
                    ));
                }
            }
        };

        Ok(connector_type)
    }

    /// List all available connector types
    pub fn list_available_connectors(&self) -> Vec<ConnectorType> {
        vec![
            ConnectorType::Docker,
            ConnectorType::Kubernetes,
            ConnectorType::AWS,
            ConnectorType::Azure,
            ConnectorType::GCP,
            ConnectorType::GitHub,
            ConnectorType::GitLab,
        ]
    }

    /// Get statistics about connector usage
    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("factories_registered".to_string(), self.factories.len());
        stats.insert("connectors_cached".to_string(), self.connector_cache.len());
        stats
    }

    /// Clear the connector cache
    pub fn clear_cache(&mut self) {
        debug!("🧹 Clearing connector cache");
        self.connector_cache.clear();
    }

    /// Get the number of registered factories
    pub fn factory_count(&self) -> usize {
        self.factories.len()
    }

    /// Check if a factory with the given name is registered
    pub fn has_factory(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }
}

impl Default for ConnectorManager {
    fn default() -> Self {
        Self::new()
    }
}