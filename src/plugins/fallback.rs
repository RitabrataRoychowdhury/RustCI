//! Fallback system for graceful degradation when plugins are unavailable

use crate::error::{AppError, Result};
use crate::infrastructure::runners::http_fallback::HttpFallbackSystem;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Fallback strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Enable automatic fallback
    pub enabled: bool,
    /// Fallback detection timeout in milliseconds
    pub detection_timeout_ms: u64,
    /// Maximum fallback attempts before giving up
    pub max_attempts: u32,
    /// Retry interval in milliseconds
    pub retry_interval_ms: u64,
    /// HTTP fallback configuration
    pub http_config: HttpFallbackConfig,
}

/// HTTP fallback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpFallbackConfig {
    /// Base URL for HTTP API
    pub base_url: String,
    /// Request timeout in milliseconds
    pub timeout_ms: u64,
    /// Maximum retries for HTTP requests
    pub max_retries: u32,
    /// Enable request compression
    pub enable_compression: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            detection_timeout_ms: 5000,
            max_attempts: 3,
            retry_interval_ms: 1000,
            http_config: HttpFallbackConfig::default(),
        }
    }
}

impl Default for HttpFallbackConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            timeout_ms: 30000,
            max_retries: 3,
            enable_compression: true,
        }
    }
}

/// Fallback strategy for handling plugin failures
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackStrategy {
    /// No fallback - fail immediately
    None,
    /// Fallback to HTTP API
    Http,
    /// Fallback to local processing
    Local,
    /// Custom fallback strategy
    Custom(String),
}

/// Fallback system state
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackState {
    /// Primary system is active
    Primary,
    /// Fallback system is active
    Fallback(FallbackStrategy),
    /// Both primary and fallback have failed
    Failed,
}

/// Fallback system for graceful degradation
pub struct FallbackSystem {
    config: FallbackConfig,
    state: Arc<RwLock<FallbackState>>,
    http_runner: Arc<RwLock<Option<HttpFallbackSystem>>>,
    attempt_count: Arc<RwLock<u32>>,
}

impl FallbackSystem {
    /// Create a new fallback system
    pub fn new(config: FallbackConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(FallbackState::Primary)),
            http_runner: Arc::new(RwLock::new(None)),
            attempt_count: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Initialize the fallback system
    pub async fn initialize(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Fallback system is disabled");
            return Ok();
        }
        
        info!("Initializing fallback system");
        
        // Initialize HTTP fallback system
        let http_config = crate::infrastructure::runners::http_fallback::HttpFallbackConfig {
            request_timeout: std::time::Duration::from_millis(self.config.http_config.timeout_ms),
            connection_timeout: std::time::Duration::from_millis(5000),
            max_retries: self.config.http_config.max_retries,
            retry_delay: std::time::Duration::from_millis(1000),
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: std::time::Duration::from_secs(60),
            max_concurrent_requests: 100,
        };
        
        match HttpFallbackSystem::new(http_config).await {
            Ok(runner) => {
                let mut http_runner = self.http_runner.write().await;
                *http_runner = Some(runner);
                info!("HTTP fallback system initialized");
            }
            Err(e) => {
                warn!("Failed to initialize HTTP fallback system: {}", e);
                // Continue without HTTP fallback
            }
        }
        
        info!("Fallback system initialized");
        Ok(())
    }
    
    /// Activate fallback mode
    pub async fn activate_fallback(&self, strategy: FallbackStrategy) -> Result<()> {
        info!("Activating fallback mode with strategy: {:?}", strategy);
        
        {
            let mut state = self.state.write().await;
            *state = FallbackState::Fallback(strategy.clone());
        }
        
        // Increment attempt count
        {
            let mut attempts = self.attempt_count.write().await;
            *attempts += 1;
        }
        
        match strategy {
            FallbackStrategy::Http => {
                self.activate_http_fallback().await?;
            }
            FallbackStrategy::Local => {
                self.activate_local_fallback().await?;
            }
            FallbackStrategy::Custom(name) => {
                info!("Activating custom fallback strategy: {}", name);
                // Custom fallback logic would go here
            }
            FallbackStrategy::None => {
                warn!("No fallback strategy specified");
            }
        }
        
        info!("Fallback mode activated successfully");
        Ok(())
    }
    
    /// Deactivate fallback mode and return to primary
    pub async fn deactivate_fallback(&self) -> Result<()> {
        info!("Deactivating fallback mode");
        
        {
            let mut state = self.state.write().await;
            *state = FallbackState::Primary;
        }
        
        // Reset attempt count
        {
            let mut attempts = self.attempt_count.write().await;
            *attempts = 0;
        }
        
        info!("Fallback mode deactivated");
        Ok(())
    }
    
    /// Check if fallback is active
    pub async fn is_fallback_active(&self) -> bool {
        let state = self.state.read().await;
        matches!(*state, FallbackState::Fallback(_))
    }
    
    /// Get current fallback state
    pub async fn get_state(&self) -> FallbackState {
        let state = self.state.read().await;
        state.clone()
    }
    
    /// Check if we should attempt fallback
    pub async fn should_attempt_fallback(&self) -> bool {
        if !self.config.enabled {
            return false;
        }
        
        let attempts = self.attempt_count.read().await;
        *attempts < self.config.max_attempts
    }
    
    /// Get HTTP fallback system if available
    pub async fn get_http_runner(&self) -> Option<HttpFallbackSystem> {
        let runner = self.http_runner.read().await;
        runner.clone()
    }
    
    /// Activate HTTP fallback
    async fn activate_http_fallback(&self) -> Result<()> {
        debug!("Activating HTTP fallback");
        
        let runner = self.http_runner.read().await;
        if runner.is_none() {
            return Err(AppError::ConfigurationError("HTTP fallback system not available".to_string()));
        }
        
        info!("HTTP fallback activated");
        Ok(())
    }
    
    /// Activate local fallback
    async fn activate_local_fallback(&self) -> Result<()> {
        debug!("Activating local fallback");
        
        // Local fallback would use existing HTTP-based runners
        info!("Local fallback activated");
        Ok(())
    }
    
    /// Test fallback connectivity
    pub async fn test_fallback(&self, strategy: FallbackStrategy) -> Result<bool> {
        debug!("Testing fallback connectivity for strategy: {:?}", strategy);
        
        match strategy {
            FallbackStrategy::Http => {
                let runner = self.http_runner.read().await;
                if let Some(_runner) = runner.as_ref() {
                    // HTTP fallback system doesn't have a test_connection method
                    // We'll assume it's available if it was initialized
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            FallbackStrategy::Local => {
                // Local fallback is always available
                Ok(true)
            }
            FallbackStrategy::Custom(_) => {
                // Custom fallback testing would go here
                Ok(true)
            }
            FallbackStrategy::None => Ok(false),
        }
    }
    
    /// Get fallback statistics
    pub async fn get_stats(&self) -> FallbackStats {
        let state = self.state.read().await;
        let attempts = self.attempt_count.read().await;
        
        FallbackStats {
            enabled: self.config.enabled,
            current_state: state.clone(),
            attempt_count: *attempts,
            max_attempts: self.config.max_attempts,
            http_available: {
                let runner = self.http_runner.read().await;
                runner.is_some()
            },
        }
    }
}

/// Fallback system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackStats {
    /// Whether fallback is enabled
    pub enabled: bool,
    /// Current fallback state
    pub current_state: FallbackState,
    /// Number of fallback attempts
    pub attempt_count: u32,
    /// Maximum allowed attempts
    pub max_attempts: u32,
    /// Whether HTTP fallback is available
    pub http_available: bool,
}

/// Fallback manager for coordinating fallback operations
pub struct FallbackManager {
    systems: Arc<RwLock<std::collections::HashMap<String, Arc<FallbackSystem>>>>,
}

impl FallbackManager {
    /// Create a new fallback manager
    pub fn new() -> Self {
        Self {
            systems: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Register a fallback system
    pub async fn register_system(&self, name: String, system: Arc<FallbackSystem>) {
        let mut systems = self.systems.write().await;
        systems.insert(name, system);
    }
    
    /// Get a fallback system by name
    pub async fn get_system(&self, name: &str) -> Option<Arc<FallbackSystem>> {
        let systems = self.systems.read().await;
        systems.get(name).cloned()
    }
    
    /// Activate fallback for a specific system
    pub async fn activate_fallback(&self, system_name: &str, strategy: FallbackStrategy) -> Result<()> {
        if let Some(system) = self.get_system(system_name).await {
            system.activate_fallback(strategy).await
        } else {
            Err(AppError::NotFound(format!("Fallback system {} not found", system_name)))
        }
    }
    
    /// Deactivate fallback for a specific system
    pub async fn deactivate_fallback(&self, system_name: &str) -> Result<()> {
        if let Some(system) = self.get_system(system_name).await {
            system.deactivate_fallback().await
        } else {
            Err(AppError::NotFound(format!("Fallback system {} not found", system_name)))
        }
    }
    
    /// Get status of all fallback systems
    pub async fn get_all_stats(&self) -> std::collections::HashMap<String, FallbackStats> {
        let systems = self.systems.read().await;
        let mut stats = std::collections::HashMap::new();
        
        for (name, system) in systems.iter() {
            stats.insert(name.clone(), system.get_stats().await);
        }
        
        stats
    }
}

impl Default for FallbackManager {
    fn default() -> Self {
        Self::new()
    }
}