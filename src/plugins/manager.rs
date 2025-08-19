//! Plugin manager for coordinating plugin lifecycle and operations

use super::{
    Plugin, PluginConfig, PluginHealth, PluginMetadata, PluginRegistry, PluginState,
    PluginSystemStats,
};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Plugin manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManagerConfig {
    /// Enable plugin system
    pub enabled: bool,
    /// Plugin directory path
    pub plugin_dir: String,
    /// Health check interval in seconds
    pub health_check_interval: u64,
    /// Maximum plugin startup time in seconds
    pub max_startup_time: u64,
    /// Auto-restart failed plugins
    pub auto_restart: bool,
    /// Maximum restart attempts
    pub max_restart_attempts: u32,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            plugin_dir: "plugins".to_string(),
            health_check_interval: 30,
            max_startup_time: 60,
            auto_restart: true,
            max_restart_attempts: 3,
        }
    }
}

/// Plugin manager for coordinating all plugin operations
pub struct PluginManager {
    config: PluginManagerConfig,
    registry: Arc<PluginRegistry>,
    restart_counts: Arc<RwLock<HashMap<String, u32>>>,
    health_monitor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    start_time: std::time::Instant,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(config: PluginManagerConfig) -> Self {
        Self {
            config,
            registry: Arc::new(PluginRegistry::new()),
            restart_counts: Arc::new(RwLock::new(HashMap::new())),
            health_monitor_handle: Arc::new(Mutex::new(None)),
            start_time: std::time::Instant::now(),
        }
    }

    /// Initialize the plugin manager
    pub async fn initialize(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Plugin system is disabled");
            return Ok();
        }

        info!("Initializing plugin manager");

        // Start health monitoring
        self.start_health_monitoring().await?;

        // Load plugins from configuration
        self.load_configured_plugins().await?;

        info!("Plugin manager initialized successfully");
        Ok(())
    }

    /// Shutdown the plugin manager
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down plugin manager");

        // Stop health monitoring
        {
            let mut handle = self.health_monitor_handle.lock().await;
            if let Some(handle) = handle.take() {
                handle.abort();
            }
        }

        // Stop all plugins
        let plugins = self.registry.list_plugins().await;
        for plugin_metadata in plugins {
            if let Err(e) = self.registry.stop_plugin(&plugin_metadata.id).await {
                warn!("Failed to stop plugin {}: {}", plugin_metadata.id, e);
            }
        }

        info!("Plugin manager shutdown complete");
        Ok(())
    }

    /// Load a plugin dynamically
    pub async fn load_plugin(&self, plugin: Box<dyn Plugin>, config: PluginConfig) -> Result<()> {
        let plugin_id = plugin.metadata().id.clone();
        info!("Loading plugin: {}", plugin_id);

        // Register the plugin
        self.registry.register_plugin(plugin).await?;

        // Update configuration
        self.registry
            .update_plugin_config(&plugin_id, config)
            .await?;

        // Start the plugin if auto-start is enabled
        if config.auto_start {
            self.start_plugin(&plugin_id).await?;
        }

        info!("Plugin {} loaded successfully", plugin_id);
        Ok(())
    }

    /// Unload a plugin dynamically
    pub async fn unload_plugin(&self, plugin_id: &str) -> Result<()> {
        info!("Unloading plugin: {}", plugin_id);

        // Stop the plugin first
        if let Err(e) = self.registry.stop_plugin(plugin_id).await {
            warn!("Failed to stop plugin {} during unload: {}", plugin_id, e);
        }

        // Unregister the plugin
        self.registry.unregister_plugin(plugin_id).await?;

        // Clear restart count
        {
            let mut restart_counts = self.restart_counts.write().await;
            restart_counts.remove(plugin_id);
        }

        info!("Plugin {} unloaded successfully", plugin_id);
        Ok(())
    }

    /// Start a plugin
    pub async fn start_plugin(&self, plugin_id: &str) -> Result<()> {
        info!("Starting plugin: {}", plugin_id);

        // Start with timeout
        let start_future = self.registry.start_plugin(plugin_id);
        let timeout_duration = Duration::from_secs(self.config.max_startup_time);

        match tokio::time::timeout(timeout_duration, start_future).await {
            Ok(result) => {
                result?;
                info!("Plugin {} started successfully", plugin_id);

                // Reset restart count on successful start
                {
                    let mut restart_counts = self.restart_counts.write().await;
                    restart_counts.insert(plugin_id.to_string(), 0);
                }

                Ok(())
            }
            Err(_) => {
                error!(
                    "Plugin {} startup timed out after {}s",
                    plugin_id, self.config.max_startup_time
                );
                Err(AppError::Timeout(format!(
                    "Plugin {} startup timeout",
                    plugin_id
                )))
            }
        }
    }

    /// Stop a plugin
    pub async fn stop_plugin(&self, plugin_id: &str) -> Result<()> {
        info!("Stopping plugin: {}", plugin_id);
        self.registry.stop_plugin(plugin_id).await
    }

    /// Restart a plugin
    pub async fn restart_plugin(&self, plugin_id: &str) -> Result<()> {
        info!("Restarting plugin: {}", plugin_id);

        // Stop first
        if let Err(e) = self.stop_plugin(plugin_id).await {
            warn!("Failed to stop plugin {} during restart: {}", plugin_id, e);
        }

        // Wait a moment
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Start again
        self.start_plugin(plugin_id).await
    }

    /// Get plugin health status
    pub async fn get_plugin_health(&self, plugin_id: &str) -> Result<PluginHealth> {
        self.registry.get_plugin_health(plugin_id).await
    }

    /// Get health status for all plugins
    pub async fn get_all_plugin_health(&self) -> Vec<PluginHealth> {
        self.registry.get_all_plugin_health().await
    }

    /// List all plugins
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.registry.list_plugins().await
    }

    /// Update plugin configuration
    pub async fn update_plugin_config(&self, plugin_id: &str, config: PluginConfig) -> Result<()> {
        self.registry.update_plugin_config(plugin_id, config).await
    }

    /// Get plugin system statistics
    pub async fn get_system_stats(&self) -> PluginSystemStats {
        let plugins = self.registry.list_plugins().await;
        let health_statuses = self.registry.get_all_plugin_health().await;

        let active_plugins = health_statuses.iter().filter(|h| h.healthy).count();
        let failed_plugins = health_statuses.iter().filter(|h| !h.healthy).count();

        PluginSystemStats {
            total_plugins: plugins.len(),
            active_plugins,
            failed_plugins,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            last_health_check: chrono::Utc::now(),
        }
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&self) -> Result<()> {
        let registry = Arc::clone(&self.registry);
        let restart_counts = Arc::clone(&self.restart_counts);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.health_check_interval));

            loop {
                interval.tick().await;

                debug!("Performing plugin health checks");
                let health_statuses = registry.get_all_plugin_health().await;

                for health in health_statuses {
                    if !health.healthy && config.auto_restart {
                        warn!(
                            "Plugin {} is unhealthy: {}",
                            health.plugin_id, health.message
                        );

                        // Check restart count
                        let should_restart = {
                            let mut counts = restart_counts.write().await;
                            let count = counts.entry(health.plugin_id.clone()).or_insert(0);
                            *count += 1;
                            *count <= config.max_restart_attempts
                        };

                        if should_restart {
                            info!(
                                "Attempting to restart unhealthy plugin: {}",
                                health.plugin_id
                            );
                            if let Err(e) = registry.start_plugin(&health.plugin_id).await {
                                error!("Failed to restart plugin {}: {}", health.plugin_id, e);
                            }
                        } else {
                            error!(
                                "Plugin {} has exceeded maximum restart attempts",
                                health.plugin_id
                            );
                        }
                    }
                }
            }
        });

        {
            let mut monitor_handle = self.health_monitor_handle.lock().await;
            *monitor_handle = Some(handle);
        }

        Ok(())
    }

    /// Load plugins from configuration
    async fn load_configured_plugins(&self) -> Result<()> {
        // This would typically load plugins from a configuration file
        // For now, we'll just log that we're ready to load plugins
        info!("Ready to load plugins from configuration");
        Ok(())
    }

    /// Check if plugin system is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get plugin manager configuration
    pub fn config(&self) -> &PluginManagerConfig {
        &self.config
    }
}
