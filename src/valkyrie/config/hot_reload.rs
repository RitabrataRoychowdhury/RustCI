// Hot-Reload Configuration System for Valkyrie High-Performance Routing
// Enables runtime configuration updates without service restart

use crate::valkyrie::config::{ValkyrieConfig, ConfigValidationError};
use crate::valkyrie::config::validation::{ConfigValidator, ValidationReport};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{info, warn, error, debug};

#[derive(Debug, Error)]
pub enum HotReloadError {
    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("File system error: {0}")]
    FileSystemError(String),
    
    #[error("Configuration parsing error: {0}")]
    ParseError(String),
    
    #[error("Hot reload not enabled")]
    NotEnabled,
    
    #[error("Configuration rollback failed: {0}")]
    RollbackFailed(String),
}

pub type HotReloadResult<T> = Result<T, HotReloadError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub timestamp: std::time::SystemTime,
    pub config_path: PathBuf,
    pub change_type: ConfigChangeType,
    pub validation_report: Option<ValidationReport>,
    pub rollback_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChangeType {
    Updated,
    Validated,
    Applied,
    RolledBack,
    ValidationFailed,
}

#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    pub enabled: bool,
    pub watch_paths: Vec<PathBuf>,
    pub validation_timeout: Duration,
    pub apply_delay: Duration,
    pub max_rollback_history: usize,
    pub strict_validation: bool,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            watch_paths: vec![
                PathBuf::from("config/production.toml"),
                PathBuf::from("config/"),
            ],
            validation_timeout: Duration::from_secs(5),
            apply_delay: Duration::from_secs(2),
            max_rollback_history: 10,
            strict_validation: true,
        }
    }
}

pub struct HotReloadManager {
    config: HotReloadConfig,
    current_config: Arc<RwLock<ValkyrieConfig>>,
    config_history: Arc<RwLock<Vec<(Instant, ValkyrieConfig)>>>,
    validator: ConfigValidator,
    change_sender: broadcast::Sender<ConfigChangeEvent>,
    _change_receiver: broadcast::Receiver<ConfigChangeEvent>,
    file_watcher: Option<notify::RecommendedWatcher>,
}

impl HotReloadManager {
    pub fn new(
        initial_config: ValkyrieConfig,
        hot_reload_config: HotReloadConfig,
    ) -> HotReloadResult<Self> {
        let (change_sender, change_receiver) = broadcast::channel(100);
        
        let validator = ConfigValidator::new(hot_reload_config.strict_validation);
        
        let mut manager = Self {
            config: hot_reload_config,
            current_config: Arc::new(RwLock::new(initial_config.clone())),
            config_history: Arc::new(RwLock::new(vec![(Instant::now(), initial_config)])),
            validator,
            change_sender,
            _change_receiver: change_receiver,
            file_watcher: None,
        };

        if manager.config.enabled {
            manager.start_file_watching()?;
        }

        Ok(manager)
    }

    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_sender.subscribe()
    }

    pub fn get_current_config(&self) -> ValkyrieConfig {
        self.current_config.read().unwrap().clone()
    }

    pub fn validate_config_file<P: AsRef<Path>>(&self, path: P) -> HotReloadResult<ValidationReport> {
        let config_content = std::fs::read_to_string(&path)
            .map_err(|e| HotReloadError::FileSystemError(e.to_string()))?;

        let config: ValkyrieConfig = toml::from_str(&config_content)
            .map_err(|e| HotReloadError::ParseError(e.to_string()))?;

        let report = self.validator.validate_config(&config);
        
        debug!(
            "Validated config file {:?}: {} errors, {} warnings",
            path.as_ref(),
            report.errors.len(),
            report.warnings.len()
        );

        Ok(report)
    }

    pub fn apply_config_file<P: AsRef<Path>>(&mut self, path: P) -> HotReloadResult<ValidationReport> {
        if !self.config.enabled {
            return Err(HotReloadError::NotEnabled);
        }

        let config_content = std::fs::read_to_string(&path)
            .map_err(|e| HotReloadError::FileSystemError(e.to_string()))?;

        let new_config: ValkyrieConfig = toml::from_str(&config_content)
            .map_err(|e| HotReloadError::ParseError(e.to_string()))?;

        let validation_report = self.validator.validate_config(&new_config);

        if !validation_report.is_valid && self.config.strict_validation {
            let event = ConfigChangeEvent {
                timestamp: std::time::SystemTime::now(),
                config_path: path.as_ref().to_path_buf(),
                change_type: ConfigChangeType::ValidationFailed,
                validation_report: Some(validation_report.clone()),
                rollback_available: self.has_rollback_available(),
            };
            
            let _ = self.change_sender.send(event);
            
            return Err(HotReloadError::ValidationFailed(format!(
                "Configuration validation failed with {} errors",
                validation_report.errors.len()
            )));
        }

        // Store current config in history before applying new one
        self.store_config_in_history();

        // Apply new configuration
        {
            let mut current = self.current_config.write().unwrap();
            *current = new_config;
        }

        info!(
            "Applied new configuration from {:?} (score: {})",
            path.as_ref(),
            validation_report.performance_score
        );

        let event = ConfigChangeEvent {
            timestamp: std::time::SystemTime::now(),
            config_path: path.as_ref().to_path_buf(),
            change_type: ConfigChangeType::Applied,
            validation_report: Some(validation_report.clone()),
            rollback_available: self.has_rollback_available(),
        };
        
        let _ = self.change_sender.send(event);

        Ok(validation_report)
    }

    pub fn rollback_config(&mut self) -> HotReloadResult<()> {
        if !self.config.enabled {
            return Err(HotReloadError::NotEnabled);
        }

        let previous_config = {
            let mut history = self.config_history.write().unwrap();
            if history.len() < 2 {
                return Err(HotReloadError::RollbackFailed(
                    "No previous configuration available".to_string()
                ));
            }
            
            // Remove current config and get previous one
            history.pop();
            history.last().unwrap().1.clone()
        };

        // Apply previous configuration
        {
            let mut current = self.current_config.write().unwrap();
            *current = previous_config;
        }

        info!("Rolled back to previous configuration");

        let event = ConfigChangeEvent {
            timestamp: std::time::SystemTime::now(),
            config_path: PathBuf::from("rollback"),
            change_type: ConfigChangeType::RolledBack,
            validation_report: None,
            rollback_available: self.has_rollback_available(),
        };
        
        let _ = self.change_sender.send(event);

        Ok(())
    }

    pub fn get_config_history(&self) -> Vec<(Instant, ValidationReport)> {
        let history = self.config_history.read().unwrap();
        history.iter()
            .map(|(timestamp, config)| {
                let report = self.validator.validate_config(config);
                (*timestamp, report)
            })
            .collect()
    }

    pub fn has_rollback_available(&self) -> bool {
        let history = self.config_history.read().unwrap();
        history.len() > 1
    }

    fn start_file_watching(&mut self) -> HotReloadResult<()> {
        let (tx, rx) = channel();
        
        let mut watcher = notify::recommended_watcher(tx)
            .map_err(|e| HotReloadError::FileSystemError(e.to_string()))?;

        for path in &self.config.watch_paths {
            watcher.watch(path, RecursiveMode::Recursive)
                .map_err(|e| HotReloadError::FileSystemError(e.to_string()))?;
            
            info!("Watching configuration path: {:?}", path);
        }

        self.file_watcher = Some(watcher);
        
        // Start background thread to handle file system events
        let change_sender = self.change_sender.clone();
        let apply_delay = self.config.apply_delay;
        let validation_timeout = self.config.validation_timeout;
        
        thread::spawn(move || {
            let mut pending_changes: HashMap<PathBuf, Instant> = HashMap::new();
            
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        if let Ok(event) = event {
                            match event.kind {
                                EventKind::Modify(_) | EventKind::Create(_) => {
                                    for path in event.paths {
                                        if path.extension().map_or(false, |ext| ext == "toml") {
                                            pending_changes.insert(path.clone(), Instant::now());
                                            
                                            let change_event = ConfigChangeEvent {
                                                timestamp: std::time::SystemTime::now(),
                                                config_path: path,
                                                change_type: ConfigChangeType::Updated,
                                                validation_report: None,
                                                rollback_available: true,
                                            };
                                            
                                            let _ = change_sender.send(change_event);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {
                        // Check for pending changes that are ready to apply
                        let now = Instant::now();
                        let ready_changes: Vec<PathBuf> = pending_changes.iter()
                            .filter(|(_, timestamp)| now.duration_since(**timestamp) >= apply_delay)
                            .map(|(path, _)| path.clone())
                            .collect();
                        
                        for path in ready_changes {
                            pending_changes.remove(&path);
                            
                            // Validate configuration in background
                            // Note: In a real implementation, this would need access to the manager
                            // This is a simplified version for demonstration
                            debug!("Configuration file ready for validation: {:?}", path);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    fn store_config_in_history(&self) {
        let current_config = self.current_config.read().unwrap().clone();
        let mut history = self.config_history.write().unwrap();
        
        history.push((Instant::now(), current_config));
        
        // Limit history size
        if history.len() > self.config.max_rollback_history {
            history.remove(0);
        }
    }
}

// Configuration update traits for different components
pub trait ConfigurableComponent {
    type Config;
    
    fn update_config(&mut self, config: Self::Config) -> Result<(), Box<dyn std::error::Error>>;
    fn validate_config(&self, config: &Self::Config) -> ValidationReport;
}

// Hot-reload aware configuration wrapper
pub struct HotReloadableConfig<T> {
    inner: Arc<RwLock<T>>,
    _change_subscription: broadcast::Receiver<ConfigChangeEvent>,
}

impl<T: Clone> HotReloadableConfig<T> {
    pub fn new(initial_config: T, change_receiver: broadcast::Receiver<ConfigChangeEvent>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial_config)),
            _change_subscription: change_receiver,
        }
    }

    pub fn get(&self) -> T {
        self.inner.read().unwrap().clone()
    }

    pub fn update(&self, new_config: T) {
        let mut config = self.inner.write().unwrap();
        *config = new_config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_hot_reload_manager_creation() {
        let config = ValkyrieConfig::default();
        let hot_reload_config = HotReloadConfig {
            enabled: false,
            ..Default::default()
        };
        
        let manager = HotReloadManager::new(config, hot_reload_config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_config_validation() {
        let config = ValkyrieConfig::default();
        let hot_reload_config = HotReloadConfig {
            enabled: false,
            ..Default::default()
        };
        
        let manager = HotReloadManager::new(config, hot_reload_config).unwrap();
        
        // Create temporary config file
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.toml");
        
        let test_config = r#"
[server]
host = "0.0.0.0"
port = 8080

[performance]
mode = "high"
"#;
        
        fs::write(&config_path, test_config).unwrap();
        
        let report = manager.validate_config_file(&config_path);
        assert!(report.is_ok());
    }

    #[test]
    fn test_rollback_functionality() {
        let config = ValkyrieConfig::default();
        let hot_reload_config = HotReloadConfig {
            enabled: true,
            ..Default::default()
        };
        
        let mut manager = HotReloadManager::new(config, hot_reload_config).unwrap();
        
        // Initially should not have rollback available (only one config in history)
        assert!(!manager.has_rollback_available());
        
        // Store a config change
        manager.store_config_in_history();
        
        // Now should have rollback available
        assert!(manager.has_rollback_available());
    }
}