//! Plugin integration service for RustCI

use super::{
    manager::{PluginManager, PluginManagerConfig},
    fallback::{FallbackManager, FallbackConfig},
    health::{PluginHealthMonitor, HealthMonitorConfig},
    valkyrie::ValkyriePlugin,
    PluginConfig,
};
use crate::error::{AppError, Result};
use std::sync::Arc;
use tracing::{info, warn};

/// Plugin integration service for coordinating all plugin operations
pub struct PluginIntegrationService {
    plugin_manager: Arc<PluginManager>,
    fallback_manager: Arc<FallbackManager>,
    health_monitor: Arc<PluginHealthMonitor>,
}

impl PluginIntegrationService {
    /// Create a new plugin integration service
    pub fn new(
        plugin_manager_config: PluginManagerConfig,
        fallback_config: FallbackConfig,
        health_monitor_config: HealthMonitorConfig,
    ) -> Self {
        let plugin_manager = Arc::new(PluginManager::new(plugin_manager_config));
        let fallback_manager = Arc::new(FallbackManager::new());
        let health_monitor = Arc::new(PluginHealthMonitor::new(health_monitor_config));
        
        Self {
            plugin_manager,
            fallback_manager,
            health_monitor,
        }
    }
    
    /// Initialize the plugin system
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing plugin integration service");
        
        // Initialize plugin manager
        self.plugin_manager.initialize().await?;
        
        // Initialize health monitoring
        self.health_monitor.start().await?;
        
        // Load built-in plugins
        self.load_builtin_plugins().await?;
        
        info!("Plugin integration service initialized successfully");
        Ok(())
    }
    
    /// Shutdown the plugin system
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down plugin integration service");
        
        // Stop health monitoring
        self.health_monitor.stop().await?;
        
        // Shutdown plugin manager
        self.plugin_manager.shutdown().await?;
        
        info!("Plugin integration service shutdown complete");
        Ok(())
    }
    
    /// Load built-in plugins
    async fn load_builtin_plugins(&self) -> Result<()> {
        info!("Loading built-in plugins");
        
        // Load Valkyrie plugin if enabled
        if self.should_load_valkyrie_plugin().await {
            self.load_valkyrie_plugin().await?;
        }
        
        info!("Built-in plugins loaded successfully");
        Ok(())
    }
    
    /// Check if Valkyrie plugin should be loaded
    async fn should_load_valkyrie_plugin(&self) -> bool {
        // Check configuration or environment variables
        std::env::var("VALKYRIE_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true) // Default to enabled
    }
    
    /// Load the Valkyrie plugin
    async fn load_valkyrie_plugin(&self) -> Result<()> {
        info!("Loading Valkyrie plugin");
        
        let valkyrie_plugin = Box::new(ValkyriePlugin::new());
        
        // Create plugin configuration
        let plugin_config = PluginConfig {
            enabled: true,
            config: {
                let mut config = std::collections::HashMap::new();
                config.insert("valkyrie".to_string(), serde_json::json!({
                    "endpoint": std::env::var("VALKYRIE_ENDPOINT")
                        .unwrap_or_else(|_| "tcp://localhost:8080".to_string()),
                    "connection_timeout_ms": 5000,
                    "request_timeout_ms": 30000,
                    "max_connections": 100,
                    "enable_pooling": true,
                    "health_check_interval": 30,
                    "enable_metrics": true,
                    "enable_http_fallback": true
                }));
                config
            },
            auto_start: true,
            health_check_interval: 30,
        };
        
        // Load the plugin
        self.plugin_manager.load_plugin(valkyrie_plugin, plugin_config).await?;
        
        // Register for health monitoring
        self.health_monitor.register_plugin("valkyrie".to_string()).await;
        
        // Register recovery callback
        let plugin_manager = Arc::clone(&self.plugin_manager);
        self.health_monitor.register_recovery_callback(
            "valkyrie".to_string(),
            move |plugin_id| {
                let manager = Arc::clone(&plugin_manager);
                let plugin_id = plugin_id.to_string();
                tokio::spawn(async move {
                    if let Err(e) = manager.restart_plugin(&plugin_id).await {
                        warn!("Failed to restart plugin {}: {}", plugin_id, e);
                    }
                });
                Ok(())
            }
        ).await;
        
        info!("Valkyrie plugin loaded successfully");
        Ok(())
    }
    
    /// Get the plugin manager
    pub fn plugin_manager(&self) -> Arc<PluginManager> {
        Arc::clone(&self.plugin_manager)
    }
    
    /// Get the fallback manager
    pub fn fallback_manager(&self) -> Arc<FallbackManager> {
        Arc::clone(&self.fallback_manager)
    }
    
    /// Get the health monitor
    pub fn health_monitor(&self) -> Arc<PluginHealthMonitor> {
        Arc::clone(&self.health_monitor)
    }
    
    /// Check if Valkyrie plugin is available
    pub async fn is_valkyrie_available(&self) -> bool {
        if let Ok(health) = self.plugin_manager.get_plugin_health("valkyrie").await {
            health.healthy
        } else {
            false
        }
    }
    
    /// Get Valkyrie plugin instance
    pub async fn get_valkyrie_plugin(&self) -> Option<Arc<ValkyriePlugin>> {
        // This is a simplified version - in reality we'd need proper plugin access
        None
    }
    
    /// Enable or disable a plugin
    pub async fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> Result<()> {
        if enabled {
            self.plugin_manager.start_plugin(plugin_id).await
        } else {
            self.plugin_manager.stop_plugin(plugin_id).await
        }
    }
    
    /// Get plugin system status
    pub async fn get_system_status(&self) -> PluginSystemStatus {
        let system_stats = self.plugin_manager.get_system_stats().await;
        let health_summary = self.health_monitor.get_health_summary().await;
        let fallback_stats = self.fallback_manager.get_all_stats().await;
        
        PluginSystemStatus {
            enabled: self.plugin_manager.is_enabled(),
            total_plugins: system_stats.total_plugins,
            active_plugins: system_stats.active_plugins,
            failed_plugins: system_stats.failed_plugins,
            valkyrie_available: self.is_valkyrie_available().await,
            fallback_active: !fallback_stats.is_empty(),
            health_monitoring_enabled: health_summary.monitoring_enabled,
        }
    }
}

/// Plugin system status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginSystemStatus {
    /// Whether plugin system is enabled
    pub enabled: bool,
    /// Total number of plugins
    pub total_plugins: usize,
    /// Number of active plugins
    pub active_plugins: usize,
    /// Number of failed plugins
    pub failed_plugins: usize,
    /// Whether Valkyrie plugin is available
    pub valkyrie_available: bool,
    /// Whether any fallback is active
    pub fallback_active: bool,
    /// Whether health monitoring is enabled
    pub health_monitoring_enabled: bool,
}