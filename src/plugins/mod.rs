//! Plugin system for RustCI
//! 
//! This module provides a pluggable architecture that allows components like Valkyrie
//! to be dynamically loaded and unloaded without requiring system restarts.

pub mod manager;
pub mod loader;
pub mod valkyrie;
pub mod fallback;
pub mod health;
pub mod integration;

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Plugin state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin is not loaded
    Unloaded,
    /// Plugin is loading
    Loading,
    /// Plugin is loaded and active
    Active,
    /// Plugin is unloading
    Unloading,
    /// Plugin failed to load or encountered an error
    Failed(String),
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// Minimum RustCI version required
    pub min_rustci_version: String,
    /// Plugin dependencies
    pub dependencies: Vec<String>,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Plugin-specific configuration
    pub config: HashMap<String, serde_json::Value>,
    /// Auto-start on system startup
    pub auto_start: bool,
    /// Health check interval in seconds
    pub health_check_interval: u64,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            config: HashMap::new(),
            auto_start: true,
            health_check_interval: 30,
        }
    }
}

/// Plugin health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    /// Plugin ID
    pub plugin_id: String,
    /// Health status
    pub healthy: bool,
    /// Last health check timestamp
    pub last_check: chrono::DateTime<chrono::Utc>,
    /// Health check message
    pub message: String,
    /// Response time in milliseconds
    pub response_time_ms: u64,
}

/// Plugin trait that all plugins must implement
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;
    
    /// Initialize the plugin with configuration
    async fn initialize(&mut self, config: PluginConfig) -> Result<()>;
    
    /// Start the plugin
    async fn start(&mut self) -> Result<()>;
    
    /// Stop the plugin
    async fn stop(&mut self) -> Result<()>;
    
    /// Get current plugin state
    fn state(&self) -> PluginState;
    
    /// Perform health check
    async fn health_check(&self) -> Result<PluginHealth>;
    
    /// Handle configuration update
    async fn update_config(&mut self, config: PluginConfig) -> Result<()>;
    
    /// Get plugin statistics
    async fn stats(&self) -> Result<serde_json::Value>;
}

/// Plugin registry for managing loaded plugins
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, Box<dyn Plugin>>>>,
    configs: Arc<RwLock<HashMap<String, PluginConfig>>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a plugin
    pub async fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<()> {
        let plugin_id = plugin.metadata().id.clone();
        tracing::info!("Registering plugin: {}", plugin_id);
        
        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin_id, plugin);
        
        Ok(())
    }
    
    /// Unregister a plugin
    pub async fn unregister_plugin(&self, plugin_id: &str) -> Result<()> {
        tracing::info!("Unregistering plugin: {}", plugin_id);
        
        let mut plugins = self.plugins.write().await;
        if let Some(mut plugin) = plugins.remove(plugin_id) {
            // Stop the plugin before removing
            if let Err(e) = plugin.stop().await {
                tracing::warn!("Failed to stop plugin {}: {}", plugin_id, e);
            }
        }
        
        // Remove configuration
        let mut configs = self.configs.write().await;
        configs.remove(plugin_id);
        
        Ok(())
    }
    
    /// Get a plugin by ID
    pub async fn get_plugin(&self, plugin_id: &str) -> Option<Box<dyn Plugin>> {
        let plugins = self.plugins.read().await;
        // Note: This is a simplified version - in reality we'd need to handle borrowing differently
        None // Placeholder for now
    }
    
    /// List all registered plugins
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values()
            .map(|plugin| plugin.metadata().clone())
            .collect()
    }
    
    /// Start a plugin
    pub async fn start_plugin(&self, plugin_id: &str) -> Result<()> {
        tracing::info!("Starting plugin: {}", plugin_id);
        
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.start().await?;
        } else {
            return Err(AppError::NotFound(format!("Plugin {} not found", plugin_id)));
        }
        
        Ok(())
    }
    
    /// Stop a plugin
    pub async fn stop_plugin(&self, plugin_id: &str) -> Result<()> {
        tracing::info!("Stopping plugin: {}", plugin_id);
        
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.stop().await?;
        } else {
            return Err(AppError::NotFound(format!("Plugin {} not found", plugin_id)));
        }
        
        Ok(())
    }
    
    /// Update plugin configuration
    pub async fn update_plugin_config(&self, plugin_id: &str, config: PluginConfig) -> Result<()> {
        tracing::info!("Updating configuration for plugin: {}", plugin_id);
        
        // Store configuration
        {
            let mut configs = self.configs.write().await;
            configs.insert(plugin_id.to_string(), config.clone());
        }
        
        // Update plugin
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.update_config(config).await?;
        } else {
            return Err(AppError::NotFound(format!("Plugin {} not found", plugin_id)));
        }
        
        Ok(())
    }
    
    /// Get plugin health status
    pub async fn get_plugin_health(&self, plugin_id: &str) -> Result<PluginHealth> {
        let plugins = self.plugins.read().await;
        if let Some(plugin) = plugins.get(plugin_id) {
            plugin.health_check().await
        } else {
            Err(AppError::NotFound(format!("Plugin {} not found", plugin_id)))
        }
    }
    
    /// Get health status for all plugins
    pub async fn get_all_plugin_health(&self) -> Vec<PluginHealth> {
        let plugins = self.plugins.read().await;
        let mut health_statuses = Vec::new();
        
        for plugin in plugins.values() {
            match plugin.health_check().await {
                Ok(health) => health_statuses.push(health),
                Err(e) => {
                    health_statuses.push(PluginHealth {
                        plugin_id: plugin.metadata().id.clone(),
                        healthy: false,
                        last_check: chrono::Utc::now(),
                        message: format!("Health check failed: {}", e),
                        response_time_ms: 0,
                    });
                }
            }
        }
        
        health_statuses
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSystemStats {
    /// Total number of registered plugins
    pub total_plugins: usize,
    /// Number of active plugins
    pub active_plugins: usize,
    /// Number of failed plugins
    pub failed_plugins: usize,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Last health check timestamp
    pub last_health_check: chrono::DateTime<chrono::Utc>,
}