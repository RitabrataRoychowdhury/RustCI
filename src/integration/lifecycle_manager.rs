//! Lifecycle Manager
//!
//! Manages the lifecycle of all system components including initialization,
//! startup, shutdown, and health monitoring.

use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{Duration, timeout};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

/// Lifecycle manager for coordinating component lifecycles
pub struct LifecycleManager {
    components: Arc<RwLock<HashMap<String, ComponentLifecycle>>>,
    initialization_semaphore: Arc<Semaphore>,
    shutdown_semaphore: Arc<Semaphore>,
}

/// Lifecycle information for a component
#[derive(Debug, Clone)]
pub struct ComponentLifecycle {
    pub name: String,
    pub state: LifecycleState,
    pub initialization_order: u32,
    pub shutdown_order: u32,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub health_check_interval: Duration,
    pub last_health_check: Option<DateTime<Utc>>,
    pub initialization_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub restart_count: u32,
    pub max_restarts: u32,
}

/// Lifecycle state of a component
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleState {
    NotInitialized,
    Initializing,
    Running,
    Degraded,
    Failed,
    ShuttingDown,
    Stopped,
    Restarting,
}

/// Lifecycle event
#[derive(Debug, Clone)]
pub struct LifecycleEvent {
    pub component_name: String,
    pub event_type: LifecycleEventType,
    pub timestamp: DateTime<Utc>,
    pub details: Option<String>,
}

/// Type of lifecycle event
#[derive(Debug, Clone)]
pub enum LifecycleEventType {
    InitializationStarted,
    InitializationCompleted,
    InitializationFailed,
    HealthCheckPassed,
    HealthCheckFailed,
    ShutdownStarted,
    ShutdownCompleted,
    RestartInitiated,
    DependencyFailed,
}

impl LifecycleManager {
    /// Create a new lifecycle manager
    pub fn new() -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
            initialization_semaphore: Arc::new(Semaphore::new(10)), // Max 10 concurrent initializations
            shutdown_semaphore: Arc::new(Semaphore::new(10)), // Max 10 concurrent shutdowns
        }
    }   
 
    /// Register a component for lifecycle management
    pub async fn register_component(
        &self,
        name: String,
        dependencies: Vec<String>,
        initialization_order: u32,
        shutdown_order: u32,
        health_check_interval: Duration,
        initialization_timeout: Duration,
        shutdown_timeout: Duration,
        max_restarts: u32,
    ) -> Result<()> {
        let lifecycle = ComponentLifecycle {
            name: name.clone(),
            state: LifecycleState::NotInitialized,
            initialization_order,
            shutdown_order,
            dependencies,
            dependents: Vec::new(),
            health_check_interval,
            last_health_check: None,
            initialization_timeout,
            shutdown_timeout,
            restart_count: 0,
            max_restarts,
        };
        
        let mut components = self.components.write().await;
        components.insert(name, lifecycle);
        
        Ok(())
    }
    
    /// Initialize a component
    pub async fn initialize_component<F, Fut>(&self, name: &str, initializer: F) -> Result<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let _permit = self.initialization_semaphore.acquire().await
            .map_err(|_| AppError::LifecycleManagement("Failed to acquire initialization permit".to_string()))?;
        
        // Update state to initializing
        self.update_component_state(name, LifecycleState::Initializing).await?;
        
        let timeout_duration = {
            let components = self.components.read().await;
            components.get(name)
                .map(|c| c.initialization_timeout)
                .unwrap_or(Duration::from_secs(60))
        };
        
        // Run initialization with timeout
        let result = timeout(timeout_duration, initializer()).await;
        
        match result {
            Ok(Ok(())) => {
                self.update_component_state(name, LifecycleState::Running).await?;
                self.emit_lifecycle_event(name, LifecycleEventType::InitializationCompleted, None).await;
                info!("✅ Component '{}' initialized successfully", name);
                Ok(())
            }
            Ok(Err(e)) => {
                self.update_component_state(name, LifecycleState::Failed).await?;
                self.emit_lifecycle_event(name, LifecycleEventType::InitializationFailed, Some(e.to_string())).await;
                error!("❌ Component '{}' initialization failed: {}", name, e);
                Err(e)
            }
            Err(_) => {
                let error = AppError::LifecycleManagement(format!("Component '{}' initialization timeout", name));
                self.update_component_state(name, LifecycleState::Failed).await?;
                self.emit_lifecycle_event(name, LifecycleEventType::InitializationFailed, Some("Timeout".to_string())).await;
                error!("⏰ Component '{}' initialization timeout", name);
                Err(error)
            }
        }
    }
    
    /// Shutdown a component
    pub async fn shutdown_component<F, Fut>(&self, name: &str, shutdown_fn: F) -> Result<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let _permit = self.shutdown_semaphore.acquire().await
            .map_err(|_| AppError::LifecycleManagement("Failed to acquire shutdown permit".to_string()))?;
        
        // Update state to shutting down
        self.update_component_state(name, LifecycleState::ShuttingDown).await?;
        self.emit_lifecycle_event(name, LifecycleEventType::ShutdownStarted, None).await;
        
        let timeout_duration = {
            let components = self.components.read().await;
            components.get(name)
                .map(|c| c.shutdown_timeout)
                .unwrap_or(Duration::from_secs(30))
        };
        
        // Run shutdown with timeout
        let result = timeout(timeout_duration, shutdown_fn()).await;
        
        match result {
            Ok(Ok(())) => {
                self.update_component_state(name, LifecycleState::Stopped).await?;
                self.emit_lifecycle_event(name, LifecycleEventType::ShutdownCompleted, None).await;
                info!("✅ Component '{}' shutdown successfully", name);
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("⚠️ Component '{}' shutdown failed: {}", name, e);
                self.update_component_state(name, LifecycleState::Failed).await?;
                // Don't return error for shutdown failures - log and continue
                Ok(())
            }
            Err(_) => {
                warn!("⏰ Component '{}' shutdown timeout - forcing stop", name);
                self.update_component_state(name, LifecycleState::Stopped).await?;
                // Don't return error for shutdown timeout - log and continue
                Ok(())
            }
        }
    }
    
    /// Perform health check on a component
    pub async fn health_check_component<F, Fut>(&self, name: &str, health_check: F) -> Result<bool>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<bool>> + Send,
    {
        let result = timeout(Duration::from_secs(10), health_check()).await;
        
        let is_healthy = match result {
            Ok(Ok(healthy)) => healthy,
            Ok(Err(_)) => false,
            Err(_) => false, // Timeout
        };
        
        // Update last health check time
        {
            let mut components = self.components.write().await;
            if let Some(component) = components.get_mut(name) {
                component.last_health_check = Some(Utc::now());
                
                // Update state based on health check result
                if is_healthy {
                    if component.state == LifecycleState::Degraded {
                        component.state = LifecycleState::Running;
                    }
                } else if component.state == LifecycleState::Running {
                    component.state = LifecycleState::Degraded;
                }
            }
        }
        
        let event_type = if is_healthy {
            LifecycleEventType::HealthCheckPassed
        } else {
            LifecycleEventType::HealthCheckFailed
        };
        
        self.emit_lifecycle_event(name, event_type, None).await;
        
        Ok(is_healthy)
    }
    
    /// Restart a component
    pub async fn restart_component<F, Fut, S, SFut>(
        &self,
        name: &str,
        shutdown_fn: S,
        initializer: F,
    ) -> Result<()>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<()>> + Send,
        S: FnOnce() -> SFut + Send,
        SFut: std::future::Future<Output = Result<()>> + Send,
    {
        // Check restart count
        {
            let mut components = self.components.write().await;
            if let Some(component) = components.get_mut(name) {
                if component.restart_count >= component.max_restarts {
                    return Err(AppError::LifecycleManagement(
                        format!("Component '{}' has exceeded maximum restart count", name)
                    ));
                }
                component.restart_count += 1;
                component.state = LifecycleState::Restarting;
            }
        }
        
        self.emit_lifecycle_event(name, LifecycleEventType::RestartInitiated, None).await;
        info!("🔄 Restarting component '{}'", name);
        
        // Shutdown first
        if let Err(e) = self.shutdown_component(name, shutdown_fn).await {
            warn!("⚠️ Shutdown failed during restart of '{}': {}", name, e);
        }
        
        // Wait a bit before restarting
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Initialize again
        self.initialize_component(name, initializer).await
    }
    
    /// Update component state
    async fn update_component_state(&self, name: &str, state: LifecycleState) -> Result<()> {
        let mut components = self.components.write().await;
        if let Some(component) = components.get_mut(name) {
            component.state = state;
            Ok(())
        } else {
            Err(AppError::LifecycleManagement(
                format!("Component '{}' not found", name)
            ))
        }
    }
    
    /// Emit a lifecycle event
    async fn emit_lifecycle_event(&self, component_name: &str, event_type: LifecycleEventType, details: Option<String>) {
        let event = LifecycleEvent {
            component_name: component_name.to_string(),
            event_type,
            timestamp: Utc::now(),
            details,
        };
        
        // Log the event
        match event.event_type {
            LifecycleEventType::InitializationStarted => debug!("🚀 {} initialization started", component_name),
            LifecycleEventType::InitializationCompleted => info!("✅ {} initialization completed", component_name),
            LifecycleEventType::InitializationFailed => error!("❌ {} initialization failed", component_name),
            LifecycleEventType::HealthCheckPassed => debug!("💚 {} health check passed", component_name),
            LifecycleEventType::HealthCheckFailed => warn!("💔 {} health check failed", component_name),
            LifecycleEventType::ShutdownStarted => info!("🛑 {} shutdown started", component_name),
            LifecycleEventType::ShutdownCompleted => info!("✅ {} shutdown completed", component_name),
            LifecycleEventType::RestartInitiated => info!("🔄 {} restart initiated", component_name),
            LifecycleEventType::DependencyFailed => error!("💥 {} dependency failed", component_name),
        }
    }
    
    /// Get component state
    pub async fn get_component_state(&self, name: &str) -> Option<LifecycleState> {
        let components = self.components.read().await;
        components.get(name).map(|c| c.state.clone())
    }
    
    /// Get all component states
    pub async fn get_all_component_states(&self) -> HashMap<String, LifecycleState> {
        let components = self.components.read().await;
        components.iter()
            .map(|(name, lifecycle)| (name.clone(), lifecycle.state.clone()))
            .collect()
    }
    
    /// Get components in initialization order
    pub async fn get_initialization_order(&self) -> Vec<String> {
        let components = self.components.read().await;
        let mut ordered: Vec<_> = components.values().collect();
        ordered.sort_by_key(|c| c.initialization_order);
        ordered.iter().map(|c| c.name.clone()).collect()
    }
    
    /// Get components in shutdown order
    pub async fn get_shutdown_order(&self) -> Vec<String> {
        let components = self.components.read().await;
        let mut ordered: Vec<_> = components.values().collect();
        ordered.sort_by_key(|c| c.shutdown_order);
        ordered.iter().map(|c| c.name.clone()).collect()
    }
    
    /// Get lifecycle statistics
    pub async fn get_statistics(&self) -> LifecycleStatistics {
        let components = self.components.read().await;
        
        let total_components = components.len();
        let running_components = components.values().filter(|c| c.state == LifecycleState::Running).count();
        let failed_components = components.values().filter(|c| c.state == LifecycleState::Failed).count();
        let degraded_components = components.values().filter(|c| c.state == LifecycleState::Degraded).count();
        let total_restarts = components.values().map(|c| c.restart_count).sum();
        
        LifecycleStatistics {
            total_components,
            running_components,
            failed_components,
            degraded_components,
            total_restarts,
        }
    }
}

/// Lifecycle statistics
#[derive(Debug, Clone)]
pub struct LifecycleStatistics {
    pub total_components: usize,
    pub running_components: usize,
    pub failed_components: usize,
    pub degraded_components: usize,
    pub total_restarts: u32,
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}