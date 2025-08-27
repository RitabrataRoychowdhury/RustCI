//! Valkyrie plugin implementation for RustCI

use super::{Plugin, PluginConfig, PluginHealth, PluginMetadata, PluginState};
use crate::error::{AppError, Result};
use crate::infrastructure::runners::valkyrie_adapter::ValkyrieRunnerAdapter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Valkyrie plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyriePluginConfig {
    /// Valkyrie server endpoint
    pub endpoint: String,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Health check interval in seconds
    pub health_check_interval: u64,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Fallback to HTTP when Valkyrie unavailable
    pub enable_http_fallback: bool,
}

impl Default for ValkyriePluginConfig {
    fn default() -> Self {
        Self {
            endpoint: "tcp://localhost:8080".to_string(),
            connection_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_connections: 100,
            enable_pooling: true,
            health_check_interval: 30,
            enable_metrics: true,
            enable_http_fallback: true,
        }
    }
}

/// Valkyrie plugin for RustCI
pub struct ValkyriePlugin {
    metadata: PluginMetadata,
    state: Arc<RwLock<PluginState>>,
    config: Arc<RwLock<Option<ValkyriePluginConfig>>>,
    adapter: Arc<RwLock<Option<ValkyrieRunnerAdapter>>>,
    health_monitor_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl ValkyriePlugin {
    /// Create a new Valkyrie plugin
    pub fn new() -> Self {
        let metadata = PluginMetadata {
            id: "valkyrie".to_string(),
            name: "Valkyrie Protocol Plugin".to_string(),
            version: "2.0.0".to_string(),
            description: "High-performance message protocol plugin for RustCI".to_string(),
            author: "RustCI Team".to_string(),
            min_rustci_version: "0.1.0".to_string(),
            dependencies: vec![],
        };

        Self {
            metadata,
            state: Arc::new(RwLock::new(PluginState::Unloaded)),
            config: Arc::new(RwLock::new(None)),
            adapter: Arc::new(RwLock::new(None)),
            health_monitor_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Extract Valkyrie configuration from plugin config
    fn extract_valkyrie_config(plugin_config: &PluginConfig) -> Result<ValkyriePluginConfig> {
        if let Some(valkyrie_config) = plugin_config.config.get("valkyrie") {
            serde_json::from_value(valkyrie_config.clone()).map_err(|e| {
                AppError::ConfigurationError(format!("Invalid Valkyrie config: {}", e))
            })
        } else {
            Ok(ValkyriePluginConfig::default())
        }
    }

    /// Start health monitoring
    async fn start_health_monitoring(&self, interval_secs: u64) -> Result<()> {
        let adapter = Arc::clone(&self.adapter);
        let state = Arc::clone(&self.state);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                // Check adapter health
                let is_healthy = {
                    let adapter_guard = adapter.read().await;
                    if let Some(_adapter) = adapter_guard.as_ref() {
                        // Implement basic health check for ValkyrieRunnerAdapter
                        // In a full implementation, this would:
                        // - Check adapter connectivity
                        // - Verify resource availability
                        // - Test message routing capabilities
                        // - Check for any error conditions
                        true
                    } else {
                        false
                    }
                };

                // Update state based on health
                {
                    let mut state_guard = state.write().await;
                    if !is_healthy && *state_guard == PluginState::Active {
                        warn!("Valkyrie plugin health check failed");
                        *state_guard = PluginState::Failed("Health check failed".to_string());
                    }
                }
            }
        });

        {
            let mut monitor_handle = self.health_monitor_handle.write().await;
            *monitor_handle = Some(handle);
        }

        Ok(())
    }

    /// Stop health monitoring
    async fn stop_health_monitoring(&self) {
        let mut monitor_handle = self.health_monitor_handle.write().await;
        if let Some(handle) = monitor_handle.take() {
            handle.abort();
        }
    }

    /// Get the Valkyrie adapter
    pub async fn get_adapter(&self) -> Option<ValkyrieRunnerAdapter> {
        let adapter_guard = self.adapter.read().await;
        adapter_guard.as_ref().cloned()
    }

    /// Check if Valkyrie is available
    pub async fn is_available(&self) -> bool {
        let state_guard = self.state.read().await;
        *state_guard == PluginState::Active
    }
}

#[async_trait::async_trait]
impl Plugin for ValkyriePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, plugin_config: PluginConfig) -> Result<()> {
        info!("Initializing Valkyrie plugin");

        {
            let mut state = self.state.write().await;
            *state = PluginState::Loading;
        }

        // Extract Valkyrie-specific configuration
        let valkyrie_config = Self::extract_valkyrie_config(&plugin_config)?;

        // Store configuration
        {
            let mut config = self.config.write().await;
            *config = Some(valkyrie_config.clone());
        }

        // Create Valkyrie adapter
        let adapter_config =
            crate::infrastructure::runners::valkyrie_adapter::ValkyrieAdapterConfig {
                max_concurrent_jobs: valkyrie_config.max_connections as u32,
                dispatch_timeout: std::time::Duration::from_millis(
                    valkyrie_config.request_timeout_ms,
                ),
                queue_capacity: 10000,
                enable_http_fallback: valkyrie_config.enable_http_fallback,
                fallback_timeout: std::time::Duration::from_millis(
                    valkyrie_config.connection_timeout_ms,
                ),
                enable_intelligent_routing: true,
                health_check_interval: std::time::Duration::from_secs(30),
                metrics_enabled: valkyrie_config.enable_metrics,
                metrics_interval: std::time::Duration::from_secs(60),
            };

        // Create a default ValkyrieEngine for now
        let valkyrie_config = crate::core::networking::valkyrie::engine::ValkyrieConfig::default();
        let valkyrie_engine =
            match crate::core::networking::valkyrie::engine::ValkyrieEngine::new(valkyrie_config) {
                Ok(engine) => Arc::new(engine),
                Err(e) => {
                    error!("Failed to create ValkyrieEngine: {}", e);
                    return Err(AppError::InternalError {
                        component: "valkyrie_plugin".to_string(),
                        message: format!("Failed to create ValkyrieEngine: {}", e),
                    });
                }
            };

        match ValkyrieRunnerAdapter::new(valkyrie_engine, adapter_config).await {
            Ok(adapter) => {
                {
                    let mut adapter_guard = self.adapter.write().await;
                    *adapter_guard = Some(adapter);
                }

                info!("Valkyrie plugin initialized successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to initialize Valkyrie adapter: {}", e);
                {
                    let mut state = self.state.write().await;
                    *state = PluginState::Failed(format!("Initialization failed: {}", e));
                }
                Err(AppError::InternalError {
                    component: "valkyrie_plugin".to_string(),
                    message: format!("Failed to create ValkyrieRunnerAdapter: {}", e),
                })
            }
        }
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting Valkyrie plugin");

        // Check if we have an adapter
        let has_adapter = {
            let adapter_guard = self.adapter.read().await;
            adapter_guard.is_some()
        };

        if !has_adapter {
            let error_msg = "Cannot start plugin: adapter not initialized";
            error!("{}", error_msg);
            {
                let mut state = self.state.write().await;
                *state = PluginState::Failed(error_msg.to_string());
            }
            return Err(AppError::InvalidState(error_msg.to_string()));
        }

        // Start health monitoring
        let health_check_interval = {
            let config_guard = self.config.read().await;
            config_guard
                .as_ref()
                .map(|c| c.health_check_interval)
                .unwrap_or(30)
        };

        self.start_health_monitoring(health_check_interval).await?;

        // Update state
        {
            let mut state = self.state.write().await;
            *state = PluginState::Active;
        }

        info!("Valkyrie plugin started successfully");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Valkyrie plugin");

        {
            let mut state = self.state.write().await;
            *state = PluginState::Unloading;
        }

        // Stop health monitoring
        self.stop_health_monitoring().await;

        // Clean up adapter
        {
            let mut adapter_guard = self.adapter.write().await;
            *adapter_guard = None;
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = PluginState::Unloaded;
        }

        info!("Valkyrie plugin stopped successfully");
        Ok(())
    }

    fn state(&self) -> PluginState {
        // This is a synchronous method, so we can't use async read
        // In a real implementation, we'd need to handle this differently
        PluginState::Active // Placeholder
    }

    async fn health_check(&self) -> Result<PluginHealth> {
        debug!("Performing Valkyrie plugin health check");

        let start_time = std::time::Instant::now();

        let (healthy, message) = {
            let adapter_guard = self.adapter.read().await;
            if let Some(adapter) = adapter_guard.as_ref() {
                let is_healthy = true; // TODO: Implement proper health check
                if is_healthy {
                    (true, "Valkyrie adapter is healthy".to_string())
                } else {
                    (false, "Valkyrie adapter is unhealthy".to_string())
                }
            } else {
                (false, "Valkyrie adapter not initialized".to_string())
            }
        };

        let response_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(PluginHealth {
            plugin_id: self.metadata.id.clone(),
            healthy,
            last_check: chrono::Utc::now(),
            message,
            response_time_ms,
        })
    }

    async fn update_config(&mut self, plugin_config: PluginConfig) -> Result<()> {
        info!("Updating Valkyrie plugin configuration");

        let valkyrie_config = Self::extract_valkyrie_config(&plugin_config)?;

        // Store new configuration
        {
            let mut config = self.config.write().await;
            *config = Some(valkyrie_config.clone());
        }

        // If plugin is active, we might need to restart the adapter
        let current_state = {
            let state_guard = self.state.read().await;
            state_guard.clone()
        };

        if current_state == PluginState::Active {
            info!("Restarting Valkyrie adapter with new configuration");

            // Stop current adapter
            {
                let mut adapter_guard = self.adapter.write().await;
                *adapter_guard = None;
            }

            // Create new adapter with updated config
            let adapter_config =
                crate::infrastructure::runners::valkyrie_adapter::ValkyrieAdapterConfig {
                    max_concurrent_jobs: valkyrie_config.max_connections as u32,
                    dispatch_timeout: std::time::Duration::from_millis(
                        valkyrie_config.request_timeout_ms,
                    ),
                    queue_capacity: 10000,
                    enable_http_fallback: valkyrie_config.enable_http_fallback,
                    fallback_timeout: std::time::Duration::from_millis(
                        valkyrie_config.connection_timeout_ms,
                    ),
                    enable_intelligent_routing: true,
                    health_check_interval: std::time::Duration::from_secs(30),
                    metrics_enabled: valkyrie_config.enable_metrics,
                    metrics_interval: std::time::Duration::from_secs(60),
                };

            // Create a default ValkyrieEngine for now
            let valkyrie_config_engine =
                crate::core::networking::valkyrie::engine::ValkyrieConfig::default();
            let valkyrie_engine =
                match crate::core::networking::valkyrie::engine::ValkyrieEngine::new(
                    valkyrie_config_engine,
                ) {
                    Ok(engine) => Arc::new(engine),
                    Err(e) => {
                        error!("Failed to create ValkyrieEngine: {}", e);
                        return Err(AppError::InternalError {
                            component: "valkyrie_plugin".to_string(),
                            message: format!("Failed to create ValkyrieEngine: {}", e),
                        });
                    }
                };

            match ValkyrieRunnerAdapter::new(valkyrie_engine, adapter_config).await {
                Ok(adapter) => {
                    let mut adapter_guard = self.adapter.write().await;
                    *adapter_guard = Some(adapter);
                    info!("Valkyrie adapter restarted with new configuration");
                }
                Err(e) => {
                    error!("Failed to restart Valkyrie adapter: {}", e);
                    {
                        let mut state = self.state.write().await;
                        *state = PluginState::Failed(format!("Config update failed: {}", e));
                    }
                    return Err(AppError::InternalError {
                        component: "valkyrie_plugin".to_string(),
                        message: format!("Failed to restart ValkyrieRunnerAdapter: {}", e),
                    });
                }
            }
        }

        info!("Valkyrie plugin configuration updated successfully");
        Ok(())
    }

    async fn stats(&self) -> Result<serde_json::Value> {
        let adapter_guard = self.adapter.read().await;
        if let Some(adapter) = adapter_guard.as_ref() {
            // Get adapter statistics
            let endpoint = {
                let config_guard = self.config.read().await;
                config_guard
                    .as_ref()
                    .map(|c| c.endpoint.clone())
                    .unwrap_or_default()
            };

            let stats = serde_json::json!({
                "plugin_id": self.metadata.id,
                "plugin_name": self.metadata.name,
                "plugin_version": self.metadata.version,
                "state": self.state().clone(),
                "adapter_healthy": true, // TODO: Implement proper health check
                "endpoint": endpoint,
                "timestamp": chrono::Utc::now()
            });
            Ok(stats)
        } else {
            Ok(serde_json::json!({
                "plugin_id": self.metadata.id,
                "plugin_name": self.metadata.name,
                "plugin_version": self.metadata.version,
                "state": self.state().clone(),
                "adapter_healthy": false,
                "error": "Adapter not initialized",
                "timestamp": chrono::Utc::now()
            }))
        }
    }
}

impl Default for ValkyriePlugin {
    fn default() -> Self {
        Self::new()
    }
}
