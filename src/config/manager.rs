//! Configuration manager with hot reloading and environment support

use super::valkyrie_config::ValkyrieConfig;
use super::validation::{ConfigValidator, ValidationResult};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, watch};
use tracing::{debug, error, info, warn};

/// Configuration manager for handling Valkyrie configuration
pub struct ValkyrieConfigManager {
    /// Current configuration
    config: Arc<RwLock<ValkyrieConfig>>,
    /// Configuration file path
    config_path: PathBuf,
    /// Configuration validator
    validator: ConfigValidator,
    /// File watcher for hot reloading
    file_watcher: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Configuration change notifier
    change_notifier: watch::Sender<ValkyrieConfig>,
    /// Last modification time
    last_modified: Arc<RwLock<Option<SystemTime>>>,
    /// Hot reload enabled
    hot_reload_enabled: bool,
}

impl ValkyrieConfigManager {
    /// Create a new configuration manager
    pub async fn new<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        info!("Creating Valkyrie configuration manager for: {}", config_path.display());
        
        // Load initial configuration
        let config = if config_path.exists() {
            ValkyrieConfig::from_file(&config_path)?
        } else {
            warn!("Configuration file not found, using defaults: {}", config_path.display());
            ValkyrieConfig::default()
        };
        
        // Create validator and load schema
        let mut validator = ConfigValidator::new();
        let schema_path = config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("schemas/valkyrie.json");
        
        if schema_path.exists() {
            validator.load_schema(&schema_path)?;
        }
        
        // Create change notifier
        let (change_notifier, _) = watch::channel(config.clone());
        
        // Get file modification time
        let last_modified = if config_path.exists() {
            std::fs::metadata(&config_path)
                .ok()
                .and_then(|m| m.modified().ok())
        } else {
            None
        };
        
        let manager = Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            validator,
            file_watcher: Arc::new(RwLock::new(None)),
            change_notifier,
            last_modified: Arc::new(RwLock::new(last_modified)),
            hot_reload_enabled: false,
        };
        
        Ok(manager)
    }
    
    /// Enable hot reloading
    pub async fn enable_hot_reload(&mut self, check_interval: Duration) -> Result<()> {
        info!("Enabling configuration hot reloading (interval: {:?})", check_interval);
        
        let config_path = self.config_path.clone();
        let config = Arc::clone(&self.config);
        let last_modified = Arc::clone(&self.last_modified);
        let change_notifier = self.change_notifier.clone();
        let mut validator = ConfigValidator::new();
        
        // Load schema for validation
        let schema_path = config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("schemas/valkyrie.json");
        
        if schema_path.exists() {
            validator.load_schema(&schema_path)?;
        }
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            
            loop {
                interval.tick().await;
                
                // Check if file has been modified
                if let Ok(metadata) = std::fs::metadata(&config_path) {
                    if let Ok(modified) = metadata.modified() {
                        let should_reload = {
                            let last_mod = last_modified.read().await;
                            last_mod.map_or(true, |last| modified > last)
                        };
                        
                        if should_reload {
                            info!("Configuration file changed, reloading...");
                            
                            match ValkyrieConfig::from_file(&config_path) {
                                Ok(mut new_config) => {
                                    // Apply environment overrides
                                    if let Err(e) = new_config.apply_environment_overrides() {
                                        warn!("Failed to apply environment overrides: {}", e);
                                    }
                                    
                                    // Expand environment variables
                                    if let Err(e) = new_config.expand_env_vars() {
                                        warn!("Failed to expand environment variables: {}", e);
                                    }
                                    
                                    // Validate new configuration
                                    match validator.validate(&new_config) {
                                        Ok(validation_result) => {
                                            if validation_result.valid {
                                                // Update configuration
                                                {
                                                    let mut config_guard = config.write().await;
                                                    *config_guard = new_config.clone();
                                                }
                                                
                                                // Update last modified time
                                                {
                                                    let mut last_mod = last_modified.write().await;
                                                    *last_mod = Some(modified);
                                                }
                                                
                                                // Notify subscribers
                                                if let Err(e) = change_notifier.send(new_config) {
                                                    warn!("Failed to notify configuration change: {}", e);
                                                }
                                                
                                                info!("Configuration reloaded successfully");
                                            } else {
                                                error!("New configuration is invalid, keeping current configuration");
                                                for issue in validation_result.issues {
                                                    error!("Validation issue: {} - {}", issue.path, issue.message);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Configuration validation failed: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to reload configuration: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });
        
        {
            let mut watcher = self.file_watcher.write().await;
            *watcher = Some(handle);
        }
        
        self.hot_reload_enabled = true;
        info!("Hot reloading enabled");
        Ok(())
    }
    
    /// Disable hot reloading
    pub async fn disable_hot_reload(&mut self) {
        info!("Disabling configuration hot reloading");
        
        {
            let mut watcher = self.file_watcher.write().await;
            if let Some(handle) = watcher.take() {
                handle.abort();
            }
        }
        
        self.hot_reload_enabled = false;
        info!("Hot reloading disabled");
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> ValkyrieConfig {
        let config = self.config.read().await;
        config.clone()
    }
    
    /// Update configuration
    pub async fn update_config(&self, new_config: ValkyrieConfig) -> Result<()> {
        info!("Updating configuration");
        
        // Validate new configuration
        let validation_result = self.validator.validate(&new_config)?;
        if !validation_result.valid {
            let error_messages: Vec<String> = validation_result.issues
                .iter()
                .filter(|i| i.severity == super::validation::ValidationSeverity::Error)
                .map(|i| format!("{}: {}", i.path, i.message))
                .collect();
            
            return Err(AppError::ConfigurationError(
                format!("Configuration validation failed: {}", error_messages.join(", "))
            ));
        }
        
        // Update configuration
        {
            let mut config = self.config.write().await;
            *config = new_config.clone();
        }
        
        // Notify subscribers
        if let Err(e) = self.change_notifier.send(new_config) {
            warn!("Failed to notify configuration change: {}", e);
        }
        
        info!("Configuration updated successfully");
        Ok(())
    }
    
    /// Save current configuration to file
    pub async fn save_config(&self) -> Result<()> {
        info!("Saving configuration to file: {}", self.config_path.display());
        
        let config = self.get_config().await;
        config.to_file(&self.config_path)?;
        
        // Update last modified time
        if let Ok(metadata) = std::fs::metadata(&self.config_path) {
            if let Ok(modified) = metadata.modified() {
                let mut last_mod = self.last_modified.write().await;
                *last_mod = Some(modified);
            }
        }
        
        info!("Configuration saved successfully");
        Ok(())
    }
    
    /// Reload configuration from file
    pub async fn reload_config(&self) -> Result<()> {
        info!("Reloading configuration from file");
        
        let mut new_config = ValkyrieConfig::from_file(&self.config_path)?;
        
        // Apply environment overrides
        new_config.apply_environment_overrides()?;
        
        // Expand environment variables
        new_config.expand_env_vars()?;
        
        // Update configuration
        self.update_config(new_config).await?;
        
        info!("Configuration reloaded successfully");
        Ok(())
    }
    
    /// Validate current configuration
    pub async fn validate_config(&self) -> Result<ValidationResult> {
        debug!("Validating current configuration");
        
        let config = self.get_config().await;
        self.validator.validate(&config)
    }
    
    /// Subscribe to configuration changes
    pub fn subscribe_to_changes(&self) -> watch::Receiver<ValkyrieConfig> {
        self.change_notifier.subscribe()
    }
    
    /// Get configuration file path
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
    
    /// Check if hot reload is enabled
    pub fn is_hot_reload_enabled(&self) -> bool {
        self.hot_reload_enabled
    }
    
    /// Get configuration manager statistics
    pub async fn get_stats(&self) -> ConfigManagerStats {
        let config = self.get_config().await;
        let last_modified = self.last_modified.read().await;
        
        ConfigManagerStats {
            config_path: self.config_path.clone(),
            environment: config.global.environment,
            hot_reload_enabled: self.hot_reload_enabled,
            last_modified: *last_modified,
            subscriber_count: self.change_notifier.receiver_count(),
        }
    }
}

/// Configuration manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManagerStats {
    /// Configuration file path
    pub config_path: PathBuf,
    /// Current environment
    pub environment: String,
    /// Whether hot reload is enabled
    pub hot_reload_enabled: bool,
    /// Last file modification time
    pub last_modified: Option<SystemTime>,
    /// Number of change subscribers
    pub subscriber_count: usize,
}

/// Configuration testing utilities
pub struct ConfigTester;

impl ConfigTester {
    /// Test configuration file
    pub fn test_config_file<P: AsRef<Path>>(config_path: P) -> Result<ValidationResult> {
        let config_path = config_path.as_ref();
        info!("Testing configuration file: {}", config_path.display());
        
        // Load configuration
        let config = ValkyrieConfig::from_file(config_path)?;
        
        // Create validator
        let mut validator = ConfigValidator::new();
        
        // Load schema if available
        let schema_path = config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("schemas/valkyrie.json");
        
        if schema_path.exists() {
            validator.load_schema(&schema_path)?;
        }
        
        // Validate configuration
        validator.validate(&config)
    }
    
    /// Test configuration with environment overrides
    pub fn test_config_with_env<P: AsRef<Path>>(config_path: P, environment: &str) -> Result<ValidationResult> {
        let config_path = config_path.as_ref();
        info!("Testing configuration with environment: {}", environment);
        
        // Load configuration
        let mut config = ValkyrieConfig::from_file(config_path)?;
        
        // Override environment
        config.global.environment = environment.to_string();
        
        // Apply environment overrides
        config.apply_environment_overrides()?;
        
        // Expand environment variables
        config.expand_env_vars()?;
        
        // Create validator
        let mut validator = ConfigValidator::new();
        
        // Load schema if available
        let schema_path = config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("schemas/valkyrie.json");
        
        if schema_path.exists() {
            validator.load_schema(&schema_path)?;
        }
        
        // Validate configuration
        validator.validate(&config)
    }
    
    /// Generate sample configurations for all environments
    pub fn generate_sample_configs<P: AsRef<Path>>(output_dir: P) -> Result<()> {
        let output_dir = output_dir.as_ref();
        info!("Generating sample configurations in: {}", output_dir.display());
        
        // Create output directory
        std::fs::create_dir_all(output_dir)
            .map_err(|e| AppError::ConfigurationError(format!("Failed to create output directory: {}", e)))?;
        
        let environments = ["development", "staging", "production"];
        
        for env in &environments {
            let mut config = ValkyrieConfig::default();
            config.global.environment = env.to_string();
            
            // Apply environment-specific defaults
            match *env {
                "development" => {
                    config.global.log_level = "debug".to_string();
                    config.global.enable_metrics = false;
                }
                "staging" => {
                    config.global.log_level = "info".to_string();
                    config.global.enable_metrics = true;
                }
                "production" => {
                    config.global.log_level = "warn".to_string();
                    config.global.enable_metrics = true;
                }
                _ => {}
            }
            
            let output_path = output_dir.join(format!("valkyrie-{}.yaml", env));
            config.to_file(&output_path)?;
            info!("Generated sample configuration: {}", output_path.display());
        }
        
        Ok(())
    }
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Previous configuration
    pub previous: ValkyrieConfig,
    /// New configuration
    pub current: ValkyrieConfig,
    /// Timestamp of change
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Change source
    pub source: ConfigChangeSource,
}

/// Configuration change source
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChangeSource {
    /// File system change (hot reload)
    FileSystem,
    /// API update
    Api,
    /// Environment variable change
    Environment,
    /// Manual update
    Manual,
}

/// Configuration watcher for monitoring changes
pub struct ConfigWatcher {
    /// Configuration manager
    manager: Arc<ValkyrieConfigManager>,
    /// Change receiver
    change_receiver: watch::Receiver<ValkyrieConfig>,
    /// Event handlers
    event_handlers: Vec<Box<dyn Fn(ConfigChangeEvent) -> Result<()> + Send + Sync>>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher
    pub fn new(manager: Arc<ValkyrieConfigManager>) -> Self {
        let change_receiver = manager.subscribe_to_changes();
        
        Self {
            manager,
            change_receiver,
            event_handlers: Vec::new(),
        }
    }
    
    /// Add an event handler for configuration changes
    pub fn add_event_handler<F>(&mut self, handler: F)
    where
        F: Fn(ConfigChangeEvent) -> Result<()> + Send + Sync + 'static,
    {
        self.event_handlers.push(Box::new(handler));
    }
    
    /// Start watching for configuration changes
    pub async fn start_watching(&mut self) -> Result<()> {
        info!("Starting configuration watcher");
        
        let mut previous_config = self.manager.get_config().await;
        
        loop {
            // Wait for configuration change
            if self.change_receiver.changed().await.is_err() {
                warn!("Configuration change channel closed");
                break;
            }
            
            let current_config = self.change_receiver.borrow().clone();
            
            // Create change event
            let event = ConfigChangeEvent {
                previous: previous_config.clone(),
                current: current_config.clone(),
                timestamp: chrono::Utc::now(),
                source: ConfigChangeSource::FileSystem, // Simplified
            };
            
            // Notify event handlers
            for handler in &self.event_handlers {
                if let Err(e) = handler(event.clone()) {
                    error!("Configuration change handler failed: {}", e);
                }
            }
            
            previous_config = current_config;
        }
        
        Ok(())
    }
}

/// Environment-specific configuration loader
pub struct EnvironmentConfigLoader;

impl EnvironmentConfigLoader {
    /// Load configuration for a specific environment
    pub fn load_for_environment<P: AsRef<Path>>(
        base_config_path: P,
        environment: &str,
    ) -> Result<ValkyrieConfig> {
        let base_config_path = base_config_path.as_ref();
        info!("Loading configuration for environment: {}", environment);
        
        // Load base configuration
        let mut config = ValkyrieConfig::from_file(base_config_path)?;
        
        // Override environment
        config.global.environment = environment.to_string();
        
        // Look for environment-specific configuration file
        let env_config_path = base_config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("valkyrie-{}.yaml", environment));
        
        if env_config_path.exists() {
            info!("Loading environment-specific configuration: {}", env_config_path.display());
            let env_config = ValkyrieConfig::from_file(&env_config_path)?;
            config = Self::merge_configs(config, env_config)?;
        }
        
        // Apply environment overrides
        config.apply_environment_overrides()?;
        
        // Expand environment variables
        config.expand_env_vars()?;
        
        Ok(config)
    }
    
    /// Merge two configurations (second takes precedence)
    fn merge_configs(base: ValkyrieConfig, override_config: ValkyrieConfig) -> Result<ValkyrieConfig> {
        // This is a simplified merge - in a real implementation you'd do deep merging
        let mut merged = base;
        
        // Override non-None values
        if override_config.server.is_some() {
            merged.server = override_config.server;
        }
        
        if override_config.client.is_some() {
            merged.client = override_config.client;
        }
        
        if override_config.transport.is_some() {
            merged.transport = override_config.transport;
        }
        
        if override_config.security.is_some() {
            merged.security = override_config.security;
        }
        
        if override_config.plugins.is_some() {
            merged.plugins = override_config.plugins;
        }
        
        if override_config.fallback.is_some() {
            merged.fallback = override_config.fallback;
        }
        
        if override_config.observability.is_some() {
            merged.observability = override_config.observability;
        }
        
        if override_config.performance.is_some() {
            merged.performance = override_config.performance;
        }
        
        Ok(merged)
    }
}