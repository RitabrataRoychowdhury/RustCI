//! Plugin loader for dynamic plugin loading and management

use super::{Plugin, PluginConfig, PluginMetadata};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

/// Plugin loader configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLoaderConfig {
    /// Plugin directory path
    pub plugin_dir: PathBuf,
    /// Enable hot reloading
    pub enable_hot_reload: bool,
    /// Plugin file extensions to scan
    pub extensions: Vec<String>,
    /// Maximum plugin load time in seconds
    pub max_load_time: u64,
    /// Enable plugin sandboxing
    pub enable_sandboxing: bool,
}

impl Default for PluginLoaderConfig {
    fn default() -> Self {
        Self {
            plugin_dir: PathBuf::from("plugins"),
            enable_hot_reload: true,
            extensions: vec!["so".to_string(), "dll".to_string(), "dylib".to_string()],
            max_load_time: 30,
            enable_sandboxing: false,
        }
    }
}

/// Plugin discovery information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDiscovery {
    /// Plugin file path
    pub path: PathBuf,
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// File modification time
    pub modified: std::time::SystemTime,
    /// File size in bytes
    pub size: u64,
    /// Plugin checksum
    pub checksum: String,
}

/// Plugin loader for dynamic loading
pub struct PluginLoader {
    config: PluginLoaderConfig,
    discovered_plugins: std::sync::Arc<tokio::sync::RwLock<HashMap<String, PluginDiscovery>>>,
    file_watcher: std::sync::Arc<tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl PluginLoader {
    /// Create a new plugin loader
    pub fn new(config: PluginLoaderConfig) -> Self {
        Self {
            config,
            discovered_plugins: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            file_watcher: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
    
    /// Initialize the plugin loader
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing plugin loader");
        
        // Create plugin directory if it doesn't exist
        if !self.config.plugin_dir.exists() {
            std::fs::create_dir_all(&self.config.plugin_dir)
                .map_err(|e| AppError::IoError(format!("Failed to create plugin directory: {}", e)))?;
            info!("Created plugin directory: {}", self.config.plugin_dir.display());
        }
        
        // Discover existing plugins
        self.discover_plugins().await?;
        
        // Start file watcher if hot reload is enabled
        if self.config.enable_hot_reload {
            self.start_file_watcher().await?;
        }
        
        info!("Plugin loader initialized successfully");
        Ok(())
    }
    
    /// Shutdown the plugin loader
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down plugin loader");
        
        // Stop file watcher
        {
            let mut watcher = self.file_watcher.write().await;
            if let Some(handle) = watcher.take() {
                handle.abort();
            }
        }
        
        info!("Plugin loader shutdown complete");
        Ok(())
    }
    
    /// Discover plugins in the plugin directory
    pub async fn discover_plugins(&self) -> Result<Vec<PluginDiscovery>> {
        debug!("Discovering plugins in: {}", self.config.plugin_dir.display());
        
        let mut discoveries = Vec::new();
        
        if !self.config.plugin_dir.exists() {
            warn!("Plugin directory does not exist: {}", self.config.plugin_dir.display());
            return Ok(discoveries);
        }
        
        let entries = std::fs::read_dir(&self.config.plugin_dir)
            .map_err(|e| AppError::IoError(format!("Failed to read plugin directory: {}", e)))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| AppError::IoError(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
                    if self.config.extensions.contains(&extension.to_string()) {
                        match self.analyze_plugin_file(&path).await {
                            Ok(discovery) => {
                                discoveries.push(discovery.clone());
                                
                                // Update discovered plugins cache
                                {
                                    let mut discovered = self.discovered_plugins.write().await;
                                    discovered.insert(discovery.metadata.id.clone(), discovery);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to analyze plugin file {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }
        
        info!("Discovered {} plugins", discoveries.len());
        Ok(discoveries)
    }
    
    /// Load a plugin from file
    pub async fn load_plugin(&self, plugin_path: &Path) -> Result<Box<dyn Plugin>> {
        info!("Loading plugin from: {}", plugin_path.display());
        
        // For now, we'll return a placeholder since dynamic loading requires unsafe code
        // In a real implementation, this would use libloading or similar
        Err(AppError::NotImplemented("Dynamic plugin loading not yet implemented".to_string()))
    }
    
    /// Load a built-in plugin by name
    pub async fn load_builtin_plugin(&self, plugin_name: &str) -> Result<Box<dyn Plugin>> {
        info!("Loading built-in plugin: {}", plugin_name);
        
        match plugin_name {
            "valkyrie" => {
                let plugin = super::valkyrie::ValkyriePlugin::new();
                Ok(Box::new(plugin))
            }
            _ => Err(AppError::NotFound(format!("Built-in plugin {} not found", plugin_name)))
        }
    }
    
    /// Get discovered plugins
    pub async fn get_discovered_plugins(&self) -> HashMap<String, PluginDiscovery> {
        let discovered = self.discovered_plugins.read().await;
        discovered.clone()
    }
    
    /// Check if a plugin file has been modified
    pub async fn is_plugin_modified(&self, plugin_id: &str) -> Result<bool> {
        let discovered = self.discovered_plugins.read().await;
        
        if let Some(discovery) = discovered.get(plugin_id) {
            let metadata = std::fs::metadata(&discovery.path)
                .map_err(|e| AppError::IoError(format!("Failed to get file metadata: {}", e)))?;
            
            let current_modified = metadata.modified()
                .map_err(|e| AppError::IoError(format!("Failed to get modification time: {}", e)))?;
            
            Ok(current_modified > discovery.modified)
        } else {
            Err(AppError::NotFound(format!("Plugin {} not found in discovered plugins", plugin_id)))
        }
    }
    
    /// Reload a plugin if it has been modified
    pub async fn reload_if_modified(&self, plugin_id: &str) -> Result<Option<Box<dyn Plugin>>> {
        if self.is_plugin_modified(plugin_id).await? {
            info!("Plugin {} has been modified, reloading", plugin_id);
            
            let discovery = {
                let discovered = self.discovered_plugins.read().await;
                discovered.get(plugin_id).cloned()
            };
            
            if let Some(discovery) = discovery {
                // Re-analyze the plugin file
                let updated_discovery = self.analyze_plugin_file(&discovery.path).await?;
                
                // Update cache
                {
                    let mut discovered = self.discovered_plugins.write().await;
                    discovered.insert(plugin_id.to_string(), updated_discovery);
                }
                
                // Load the plugin
                let plugin = self.load_plugin(&discovery.path).await?;
                Ok(Some(plugin))
            } else {
                Err(AppError::NotFound(format!("Plugin {} not found", plugin_id)))
            }
        } else {
            Ok(None)
        }
    }
    
    /// Validate plugin compatibility
    pub fn validate_plugin_compatibility(&self, metadata: &PluginMetadata) -> Result<()> {
        debug!("Validating plugin compatibility: {}", metadata.id);
        
        // Check minimum RustCI version
        // This is a simplified version check
        let current_version = env!("CARGO_PKG_VERSION");
        if metadata.min_rustci_version > current_version {
            return Err(AppError::IncompatibleVersion(
                format!("Plugin {} requires RustCI version {} or higher, current version is {}", 
                    metadata.id, metadata.min_rustci_version, current_version)
            ));
        }
        
        // Check dependencies (simplified)
        for dependency in &metadata.dependencies {
            debug!("Checking dependency: {}", dependency);
            // In a real implementation, we'd check if dependencies are available
        }
        
        debug!("Plugin {} is compatible", metadata.id);
        Ok(())
    }
    
    /// Start file watcher for hot reloading
    async fn start_file_watcher(&self) -> Result<()> {
        info!("Starting file watcher for hot reloading");
        
        let plugin_dir = self.config.plugin_dir.clone();
        let discovered_plugins = std::sync::Arc::clone(&self.discovered_plugins);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                // Check for file changes
                if let Ok(entries) = std::fs::read_dir(&plugin_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            // Check if this is a plugin file that has been modified
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                if let Ok(modified) = metadata.modified() {
                                    // Compare with cached modification time
                                    let discovered = discovered_plugins.read().await;
                                    let needs_reload = discovered.values()
                                        .any(|d| d.path == path && modified > d.modified);
                                    
                                    if needs_reload {
                                        info!("Detected file change: {}", path.display());
                                        // In a real implementation, we'd trigger a reload here
                                    }
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
        
        Ok(())
    }
    
    /// Analyze a plugin file to extract metadata
    async fn analyze_plugin_file(&self, path: &Path) -> Result<PluginDiscovery> {
        debug!("Analyzing plugin file: {}", path.display());
        
        let metadata = std::fs::metadata(path)
            .map_err(|e| AppError::IoError(format!("Failed to get file metadata: {}", e)))?;
        
        let modified = metadata.modified()
            .map_err(|e| AppError::IoError(format!("Failed to get modification time: {}", e)))?;
        
        let size = metadata.len();
        
        // Calculate checksum (simplified)
        let checksum = format!("{:x}", size); // In reality, we'd use a proper hash
        
        // Extract plugin metadata (this is a placeholder)
        // In a real implementation, we'd parse the plugin file to extract metadata
        let plugin_metadata = PluginMetadata {
            id: path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
            name: format!("Plugin from {}", path.display()),
            version: "1.0.0".to_string(),
            description: "Dynamically loaded plugin".to_string(),
            author: "Unknown".to_string(),
            min_rustci_version: "0.1.0".to_string(),
            dependencies: vec![],
        };
        
        Ok(PluginDiscovery {
            path: path.to_path_buf(),
            metadata: plugin_metadata,
            modified,
            size,
            checksum,
        })
    }
    
    /// Get loader statistics
    pub async fn get_stats(&self) -> PluginLoaderStats {
        let discovered = self.discovered_plugins.read().await;
        
        PluginLoaderStats {
            plugin_dir: self.config.plugin_dir.clone(),
            discovered_count: discovered.len(),
            hot_reload_enabled: self.config.enable_hot_reload,
            sandboxing_enabled: self.config.enable_sandboxing,
            supported_extensions: self.config.extensions.clone(),
        }
    }
}

/// Plugin loader statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLoaderStats {
    /// Plugin directory path
    pub plugin_dir: PathBuf,
    /// Number of discovered plugins
    pub discovered_count: usize,
    /// Whether hot reload is enabled
    pub hot_reload_enabled: bool,
    /// Whether sandboxing is enabled
    pub sandboxing_enabled: bool,
    /// Supported file extensions
    pub supported_extensions: Vec<String>,
}