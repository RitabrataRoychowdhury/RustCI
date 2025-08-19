//! Plugin health monitoring and management

use super::{PluginHealth, PluginState};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Enable health monitoring
    pub enabled: bool,
    /// Health check interval in seconds
    pub check_interval: u64,
    /// Health check timeout in milliseconds
    pub check_timeout_ms: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes to mark healthy again
    pub recovery_threshold: u32,
    /// Enable automatic recovery attempts
    pub auto_recovery: bool,
    /// Maximum recovery attempts
    pub max_recovery_attempts: u32,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval: 30,
            check_timeout_ms: 5000,
            failure_threshold: 3,
            recovery_threshold: 2,
            auto_recovery: true,
            max_recovery_attempts: 5,
        }
    }
}

/// Plugin health tracking information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealthTracker {
    /// Plugin ID
    pub plugin_id: String,
    /// Current health status
    pub current_health: PluginHealth,
    /// Consecutive failure count
    pub consecutive_failures: u32,
    /// Consecutive success count
    pub consecutive_successes: u32,
    /// Total health checks performed
    pub total_checks: u64,
    /// Total failures
    pub total_failures: u64,
    /// Recovery attempt count
    pub recovery_attempts: u32,
    /// Last recovery attempt timestamp
    pub last_recovery_attempt: Option<chrono::DateTime<chrono::Utc>>,
    /// Health history (last 100 checks)
    pub health_history: Vec<PluginHealthHistoryEntry>,
}

/// Health history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealthHistoryEntry {
    /// Timestamp of the health check
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether the check was successful
    pub healthy: bool,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Health check message
    pub message: String,
}

impl PluginHealthTracker {
    /// Create a new health tracker
    pub fn new(plugin_id: String) -> Self {
        Self {
            plugin_id: plugin_id.clone(),
            current_health: PluginHealth {
                plugin_id,
                healthy: false,
                last_check: chrono::Utc::now(),
                message: "Not yet checked".to_string(),
                response_time_ms: 0,
            },
            consecutive_failures: 0,
            consecutive_successes: 0,
            total_checks: 0,
            total_failures: 0,
            recovery_attempts: 0,
            last_recovery_attempt: None,
            health_history: Vec::new(),
        }
    }

    /// Update health status
    pub fn update_health(&mut self, health: PluginHealth) {
        self.current_health = health.clone();
        self.total_checks += 1;

        if health.healthy {
            self.consecutive_successes += 1;
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            self.consecutive_successes = 0;
            self.total_failures += 1;
        }

        // Add to history
        let history_entry = PluginHealthHistoryEntry {
            timestamp: health.last_check,
            healthy: health.healthy,
            response_time_ms: health.response_time_ms,
            message: health.message,
        };

        self.health_history.push(history_entry);

        // Keep only last 100 entries
        if self.health_history.len() > 100 {
            self.health_history.remove(0);
        }
    }

    /// Check if plugin should be considered unhealthy
    pub fn is_unhealthy(&self, threshold: u32) -> bool {
        self.consecutive_failures >= threshold
    }

    /// Check if plugin has recovered
    pub fn has_recovered(&self, threshold: u32) -> bool {
        self.consecutive_successes >= threshold
    }

    /// Record recovery attempt
    pub fn record_recovery_attempt(&mut self) {
        self.recovery_attempts += 1;
        self.last_recovery_attempt = Some(chrono::Utc::now());
    }

    /// Reset recovery attempts
    pub fn reset_recovery_attempts(&mut self) {
        self.recovery_attempts = 0;
        self.last_recovery_attempt = None;
    }

    /// Get health statistics
    pub fn get_stats(&self) -> PluginHealthStats {
        let success_rate = if self.total_checks > 0 {
            ((self.total_checks - self.total_failures) as f64 / self.total_checks as f64) * 100.0
        } else {
            0.0
        };

        let avg_response_time = if !self.health_history.is_empty() {
            self.health_history
                .iter()
                .map(|h| h.response_time_ms)
                .sum::<u64>() as f64
                / self.health_history.len() as f64
        } else {
            0.0
        };

        PluginHealthStats {
            plugin_id: self.plugin_id.clone(),
            current_healthy: self.current_health.healthy,
            success_rate,
            total_checks: self.total_checks,
            total_failures: self.total_failures,
            consecutive_failures: self.consecutive_failures,
            consecutive_successes: self.consecutive_successes,
            recovery_attempts: self.recovery_attempts,
            avg_response_time_ms: avg_response_time,
            last_check: self.current_health.last_check,
        }
    }
}

/// Plugin health statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealthStats {
    /// Plugin ID
    pub plugin_id: String,
    /// Current health status
    pub current_healthy: bool,
    /// Success rate percentage
    pub success_rate: f64,
    /// Total health checks performed
    pub total_checks: u64,
    /// Total failures
    pub total_failures: u64,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Consecutive successes
    pub consecutive_successes: u32,
    /// Recovery attempts
    pub recovery_attempts: u32,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Last health check timestamp
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Health monitor for tracking plugin health
pub struct PluginHealthMonitor {
    config: HealthMonitorConfig,
    trackers: Arc<RwLock<HashMap<String, PluginHealthTracker>>>,
    monitor_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    recovery_callbacks: Arc<RwLock<HashMap<String, Box<dyn Fn(&str) -> Result<()> + Send + Sync>>>>,
}

impl PluginHealthMonitor {
    /// Create a new health monitor
    pub fn new(config: HealthMonitorConfig) -> Self {
        Self {
            config,
            trackers: Arc::new(RwLock::new(HashMap::new())),
            monitor_handle: Arc::new(RwLock::new(None)),
            recovery_callbacks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start health monitoring
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Plugin health monitoring is disabled");
            return Ok();
        }

        info!("Starting plugin health monitoring");

        let trackers = Arc::clone(&self.trackers);
        let config = self.config.clone();
        let recovery_callbacks = Arc::clone(&self.recovery_callbacks);

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.check_interval));

            loop {
                interval.tick().await;

                debug!("Performing plugin health checks");

                let plugin_ids: Vec<String> = {
                    let trackers_guard = trackers.read().await;
                    trackers_guard.keys().cloned().collect()
                };

                for plugin_id in plugin_ids {
                    // Perform health check with timeout
                    let health_result = tokio::time::timeout(
                        Duration::from_millis(config.check_timeout_ms),
                        Self::perform_health_check(&plugin_id),
                    )
                    .await;

                    let health = match health_result {
                        Ok(Ok(health)) => health,
                        Ok(Err(e)) => {
                            warn!("Health check failed for plugin {}: {}", plugin_id, e);
                            PluginHealth {
                                plugin_id: plugin_id.clone(),
                                healthy: false,
                                last_check: chrono::Utc::now(),
                                message: format!("Health check error: {}", e),
                                response_time_ms: config.check_timeout_ms,
                            }
                        }
                        Err(_) => {
                            warn!("Health check timed out for plugin {}", plugin_id);
                            PluginHealth {
                                plugin_id: plugin_id.clone(),
                                healthy: false,
                                last_check: chrono::Utc::now(),
                                message: "Health check timeout".to_string(),
                                response_time_ms: config.check_timeout_ms,
                            }
                        }
                    };

                    // Update tracker
                    {
                        let mut trackers_guard = trackers.write().await;
                        if let Some(tracker) = trackers_guard.get_mut(&plugin_id) {
                            let was_healthy = tracker.current_health.healthy;
                            tracker.update_health(health);

                            // Check for state changes
                            if !was_healthy && tracker.has_recovered(config.recovery_threshold) {
                                info!("Plugin {} has recovered", plugin_id);
                                tracker.reset_recovery_attempts();
                            } else if was_healthy && tracker.is_unhealthy(config.failure_threshold)
                            {
                                warn!("Plugin {} is now unhealthy", plugin_id);

                                // Attempt recovery if enabled
                                if config.auto_recovery
                                    && tracker.recovery_attempts < config.max_recovery_attempts
                                {
                                    info!("Attempting recovery for plugin {}", plugin_id);
                                    tracker.record_recovery_attempt();

                                    // Call recovery callback
                                    let callbacks = recovery_callbacks.read().await;
                                    if let Some(callback) = callbacks.get(&plugin_id) {
                                        if let Err(e) = callback(&plugin_id) {
                                            error!(
                                                "Recovery callback failed for plugin {}: {}",
                                                plugin_id, e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        {
            let mut monitor_handle = self.monitor_handle.write().await;
            *monitor_handle = Some(handle);
        }

        info!("Plugin health monitoring started");
        Ok(())
    }

    /// Stop health monitoring
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping plugin health monitoring");

        {
            let mut monitor_handle = self.monitor_handle.write().await;
            if let Some(handle) = monitor_handle.take() {
                handle.abort();
            }
        }

        info!("Plugin health monitoring stopped");
        Ok(())
    }

    /// Register a plugin for health monitoring
    pub async fn register_plugin(&self, plugin_id: String) {
        info!("Registering plugin for health monitoring: {}", plugin_id);

        let tracker = PluginHealthTracker::new(plugin_id.clone());

        {
            let mut trackers = self.trackers.write().await;
            trackers.insert(plugin_id, tracker);
        }
    }

    /// Unregister a plugin from health monitoring
    pub async fn unregister_plugin(&self, plugin_id: &str) {
        info!("Unregistering plugin from health monitoring: {}", plugin_id);

        {
            let mut trackers = self.trackers.write().await;
            trackers.remove(plugin_id);
        }

        {
            let mut callbacks = self.recovery_callbacks.write().await;
            callbacks.remove(plugin_id);
        }
    }

    /// Register a recovery callback for a plugin
    pub async fn register_recovery_callback<F>(&self, plugin_id: String, callback: F)
    where
        F: Fn(&str) -> Result<()> + Send + Sync + 'static,
    {
        let mut callbacks = self.recovery_callbacks.write().await;
        callbacks.insert(plugin_id, Box::new(callback));
    }

    /// Get health status for a plugin
    pub async fn get_plugin_health(&self, plugin_id: &str) -> Option<PluginHealth> {
        let trackers = self.trackers.read().await;
        trackers.get(plugin_id).map(|t| t.current_health.clone())
    }

    /// Get health statistics for a plugin
    pub async fn get_plugin_stats(&self, plugin_id: &str) -> Option<PluginHealthStats> {
        let trackers = self.trackers.read().await;
        trackers.get(plugin_id).map(|t| t.get_stats())
    }

    /// Get health statistics for all plugins
    pub async fn get_all_stats(&self) -> HashMap<String, PluginHealthStats> {
        let trackers = self.trackers.read().await;
        trackers
            .iter()
            .map(|(id, tracker)| (id.clone(), tracker.get_stats()))
            .collect()
    }

    /// Get overall health summary
    pub async fn get_health_summary(&self) -> HealthSummary {
        let trackers = self.trackers.read().await;

        let total_plugins = trackers.len();
        let healthy_plugins = trackers
            .values()
            .filter(|t| t.current_health.healthy)
            .count();
        let unhealthy_plugins = total_plugins - healthy_plugins;

        let avg_response_time = if !trackers.is_empty() {
            trackers
                .values()
                .map(|t| t.current_health.response_time_ms)
                .sum::<u64>() as f64
                / trackers.len() as f64
        } else {
            0.0
        };

        HealthSummary {
            total_plugins,
            healthy_plugins,
            unhealthy_plugins,
            avg_response_time_ms: avg_response_time,
            monitoring_enabled: self.config.enabled,
            last_check: chrono::Utc::now(),
        }
    }

    /// Perform health check for a plugin (placeholder)
    async fn perform_health_check(plugin_id: &str) -> Result<PluginHealth> {
        // This is a placeholder implementation
        // In reality, this would call the plugin's health_check method
        debug!("Performing health check for plugin: {}", plugin_id);

        // Simulate health check
        tokio::time::sleep(Duration::from_millis(10)).await;

        Ok(PluginHealth {
            plugin_id: plugin_id.to_string(),
            healthy: true,
            last_check: chrono::Utc::now(),
            message: "Health check passed".to_string(),
            response_time_ms: 10,
        })
    }
}

/// Overall health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    /// Total number of monitored plugins
    pub total_plugins: usize,
    /// Number of healthy plugins
    pub healthy_plugins: usize,
    /// Number of unhealthy plugins
    pub unhealthy_plugins: usize,
    /// Average response time across all plugins
    pub avg_response_time_ms: f64,
    /// Whether monitoring is enabled
    pub monitoring_enabled: bool,
    /// Last check timestamp
    pub last_check: chrono::DateTime<chrono::Utc>,
}
