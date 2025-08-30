//! Resource Lifecycle Management System
//! 
//! Provides comprehensive resource lifecycle management with automatic cleanup policies,
//! configurable retention policies, and resource tracking capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, Instant};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

/// Resource types that can be managed by the lifecycle system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    TempFile,
    CacheEntry,
    DatabaseConnection,
    HttpConnection,
    BackgroundJob,
    UserSession,
    ApiToken,
    Custom(String),
}

/// Resource lifecycle states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResourceState {
    Created,
    Active,
    Idle,
    Expired,
    MarkedForCleanup,
    Cleaned,
}

/// Resource metadata for tracking and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    pub id: Uuid,
    pub resource_type: ResourceType,
    pub state: ResourceState,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub size_bytes: Option<u64>,
    pub tags: HashMap<String, String>,
    pub cleanup_priority: CleanupPriority,
}

/// Priority levels for resource cleanup
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CleanupPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Retention policy configuration for different resource types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub resource_type: ResourceType,
    pub max_age: Duration,
    pub max_idle_time: Duration,
    pub max_size_bytes: Option<u64>,
    pub max_count: Option<usize>,
    pub cleanup_interval: Duration,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            resource_type: ResourceType::Custom("default".to_string()),
            max_age: Duration::from_secs(3600), // 1 hour
            max_idle_time: Duration::from_secs(1800), // 30 minutes
            max_size_bytes: Some(100 * 1024 * 1024), // 100MB
            max_count: Some(1000),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Resource cleanup statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupStats {
    pub total_resources_cleaned: u64,
    pub bytes_freed: u64,
    pub cleanup_runs: u64,
    pub last_cleanup: Option<SystemTime>,
    pub cleanup_errors: u64,
}

/// Trait for resource cleanup handlers
#[async_trait::async_trait]
pub trait ResourceCleanupHandler: Send + Sync {
    async fn cleanup_resource(&self, metadata: &ResourceMetadata) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn can_handle(&self, resource_type: &ResourceType) -> bool;
    fn get_handler_name(&self) -> &str;
}

/// Resource lifecycle manager with automatic cleanup capabilities
pub struct ResourceLifecycleManager {
    resources: Arc<RwLock<HashMap<Uuid, ResourceMetadata>>>,
    retention_policies: Arc<RwLock<HashMap<ResourceType, RetentionPolicy>>>,
    cleanup_handlers: Arc<RwLock<Vec<Arc<dyn ResourceCleanupHandler>>>>,
    cleanup_stats: Arc<Mutex<CleanupStats>>,
    is_running: Arc<Mutex<bool>>,
}

impl ResourceLifecycleManager {
    /// Create a new resource lifecycle manager
    pub fn new() -> Self {
        Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            retention_policies: Arc::new(RwLock::new(HashMap::new())),
            cleanup_handlers: Arc::new(RwLock::new(Vec::new())),
            cleanup_stats: Arc::new(Mutex::new(CleanupStats::default())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Register a resource with the lifecycle manager
    pub async fn register_resource(&self, mut metadata: ResourceMetadata) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        metadata.state = ResourceState::Created;
        let id = metadata.id;
        
        let mut resources = self.resources.write().await;
        resources.insert(id, metadata.clone());
        
        debug!("Registered resource {} of type {:?}", id, metadata.resource_type);
        Ok(id)
    }

    /// Update resource access time
    pub async fn touch_resource(&self, id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut resources = self.resources.write().await;
        if let Some(resource) = resources.get_mut(&id) {
            resource.last_accessed = SystemTime::now();
            if resource.state == ResourceState::Idle {
                resource.state = ResourceState::Active;
            }
            debug!("Touched resource {}", id);
        }
        Ok(())
    }

    /// Mark resource as idle
    pub async fn mark_idle(&self, id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut resources = self.resources.write().await;
        if let Some(resource) = resources.get_mut(&id) {
            resource.state = ResourceState::Idle;
            debug!("Marked resource {} as idle", id);
        }
        Ok(())
    }

    /// Manually remove a resource
    pub async fn remove_resource(&self, id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut resources = self.resources.write().await;
        if let Some(mut resource) = resources.remove(&id) {
            resource.state = ResourceState::Cleaned;
            
            // Find and execute cleanup handler
            let handlers = self.cleanup_handlers.read().await;
            for handler in handlers.iter() {
                if handler.can_handle(&resource.resource_type) {
                    if let Err(e) = handler.cleanup_resource(&resource).await {
                        error!("Failed to cleanup resource {}: {}", id, e);
                        return Err(e);
                    }
                    break;
                }
            }
            
            info!("Removed resource {} of type {:?}", id, resource.resource_type);
        }
        Ok(())
    }

    /// Set retention policy for a resource type
    pub async fn set_retention_policy(&self, policy: RetentionPolicy) {
        let mut policies = self.retention_policies.write().await;
        policies.insert(policy.resource_type.clone(), policy);
    }

    /// Register a cleanup handler
    pub async fn register_cleanup_handler(&self, handler: Arc<dyn ResourceCleanupHandler>) {
        let mut handlers = self.cleanup_handlers.write().await;
        handlers.push(handler);
    }

    /// Start the automatic cleanup process
    pub async fn start_cleanup_process(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        let resources = Arc::clone(&self.resources);
        let policies = Arc::clone(&self.retention_policies);
        let handlers = Arc::clone(&self.cleanup_handlers);
        let stats = Arc::clone(&self.cleanup_stats);
        let running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60)); // Check every minute
            
            loop {
                cleanup_interval.tick().await;
                
                let is_running = running.lock().await;
                if !*is_running {
                    break;
                }
                drop(is_running);

                if let Err(e) = Self::run_cleanup_cycle(&resources, &policies, &handlers, &stats).await {
                    error!("Cleanup cycle failed: {}", e);
                }
            }
        });

        info!("Started resource lifecycle cleanup process");
        Ok(())
    }

    /// Stop the automatic cleanup process
    pub async fn stop_cleanup_process(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("Stopped resource lifecycle cleanup process");
    }

    /// Run a single cleanup cycle
    async fn run_cleanup_cycle(
        resources: &Arc<RwLock<HashMap<Uuid, ResourceMetadata>>>,
        policies: &Arc<RwLock<HashMap<ResourceType, RetentionPolicy>>>,
        handlers: &Arc<RwLock<Vec<Arc<dyn ResourceCleanupHandler>>>>,
        stats: &Arc<Mutex<CleanupStats>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = SystemTime::now();
        let policies_map = policies.read().await;
        let handlers_vec = handlers.read().await;
        
        let mut resources_to_cleanup = Vec::new();
        
        // Identify resources that need cleanup
        {
            let resources_map = resources.read().await;
            for (id, resource) in resources_map.iter() {
                if let Some(policy) = policies_map.get(&resource.resource_type) {
                    let should_cleanup = Self::should_cleanup_resource(resource, policy, now);
                    if should_cleanup {
                        resources_to_cleanup.push((*id, resource.clone()));
                    }
                }
            }
        }

        // Perform cleanup
        let mut cleanup_count = 0;
        let mut bytes_freed = 0;
        let mut errors = 0;

        for (id, mut resource) in resources_to_cleanup {
            resource.state = ResourceState::MarkedForCleanup;
            
            // Find appropriate handler
            let mut handled = false;
            for handler in handlers_vec.iter() {
                if handler.can_handle(&resource.resource_type) {
                    match handler.cleanup_resource(&resource).await {
                        Ok(()) => {
                            // Remove from resources map
                            let mut resources_map = resources.write().await;
                            resources_map.remove(&id);
                            
                            cleanup_count += 1;
                            if let Some(size) = resource.size_bytes {
                                bytes_freed += size;
                            }
                            handled = true;
                            debug!("Cleaned up resource {} using handler {}", id, handler.get_handler_name());
                            break;
                        }
                        Err(e) => {
                            error!("Failed to cleanup resource {} with handler {}: {}", id, handler.get_handler_name(), e);
                            errors += 1;
                        }
                    }
                }
            }
            
            if !handled {
                warn!("No handler found for resource type {:?}", resource.resource_type);
            }
        }

        // Update statistics
        {
            let mut stats_guard = stats.lock().await;
            stats_guard.total_resources_cleaned += cleanup_count;
            stats_guard.bytes_freed += bytes_freed;
            stats_guard.cleanup_runs += 1;
            stats_guard.last_cleanup = Some(now);
            stats_guard.cleanup_errors += errors;
        }

        if cleanup_count > 0 {
            info!("Cleanup cycle completed: {} resources cleaned, {} bytes freed, {} errors", 
                  cleanup_count, bytes_freed, errors);
        }

        Ok(())
    }

    /// Check if a resource should be cleaned up based on policy
    fn should_cleanup_resource(resource: &ResourceMetadata, policy: &RetentionPolicy, now: SystemTime) -> bool {
        // Check if expired
        if let Some(expires_at) = resource.expires_at {
            if now > expires_at {
                return true;
            }
        }

        // Check max age
        if let Ok(age) = now.duration_since(resource.created_at) {
            if age > policy.max_age {
                return true;
            }
        }

        // Check idle time
        if resource.state == ResourceState::Idle {
            if let Ok(idle_time) = now.duration_since(resource.last_accessed) {
                if idle_time > policy.max_idle_time {
                    return true;
                }
            }
        }

        false
    }

    /// Get current resource statistics
    pub async fn get_resource_stats(&self) -> HashMap<ResourceType, usize> {
        let resources = self.resources.read().await;
        let mut stats = HashMap::new();
        
        for resource in resources.values() {
            *stats.entry(resource.resource_type.clone()).or_insert(0) += 1;
        }
        
        stats
    }

    /// Get cleanup statistics
    pub async fn get_cleanup_stats(&self) -> CleanupStats {
        let stats = self.cleanup_stats.lock().await;
        stats.clone()
    }

    /// Get all resources of a specific type
    pub async fn get_resources_by_type(&self, resource_type: &ResourceType) -> Vec<ResourceMetadata> {
        let resources = self.resources.read().await;
        resources.values()
            .filter(|r| &r.resource_type == resource_type)
            .cloned()
            .collect()
    }
}

impl Default for ResourceLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Default cleanup handler for basic resource types
pub struct DefaultCleanupHandler;

#[async_trait::async_trait]
impl ResourceCleanupHandler for DefaultCleanupHandler {
    async fn cleanup_resource(&self, metadata: &ResourceMetadata) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Default cleanup for resource {} of type {:?}", metadata.id, metadata.resource_type);
        // Default implementation just logs the cleanup
        Ok(())
    }

    fn can_handle(&self, _resource_type: &ResourceType) -> bool {
        true // Default handler can handle any resource type as fallback
    }

    fn get_handler_name(&self) -> &str {
        "DefaultCleanupHandler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_resource_registration() {
        let manager = ResourceLifecycleManager::new();
        
        let metadata = ResourceMetadata {
            id: Uuid::new_v4(),
            resource_type: ResourceType::TempFile,
            state: ResourceState::Created,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: None,
            size_bytes: Some(1024),
            tags: HashMap::new(),
            cleanup_priority: CleanupPriority::Normal,
        };
        
        let id = manager.register_resource(metadata).await.unwrap();
        let stats = manager.get_resource_stats().await;
        
        assert_eq!(stats.get(&ResourceType::TempFile), Some(&1));
    }

    #[tokio::test]
    async fn test_resource_touch() {
        let manager = ResourceLifecycleManager::new();
        
        let metadata = ResourceMetadata {
            id: Uuid::new_v4(),
            resource_type: ResourceType::TempFile,
            state: ResourceState::Created,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: None,
            size_bytes: Some(1024),
            tags: HashMap::new(),
            cleanup_priority: CleanupPriority::Normal,
        };
        
        let id = manager.register_resource(metadata).await.unwrap();
        manager.mark_idle(id).await.unwrap();
        manager.touch_resource(id).await.unwrap();
        
        // Resource should be active again after touch
        let resources = manager.get_resources_by_type(&ResourceType::TempFile).await;
        assert_eq!(resources[0].state, ResourceState::Active);
    }

    #[tokio::test]
    async fn test_retention_policy() {
        let manager = ResourceLifecycleManager::new();
        
        let policy = RetentionPolicy {
            resource_type: ResourceType::TempFile,
            max_age: Duration::from_millis(100),
            max_idle_time: Duration::from_millis(50),
            max_size_bytes: Some(1024),
            max_count: Some(10),
            cleanup_interval: Duration::from_millis(50),
        };
        
        manager.set_retention_policy(policy).await;
        manager.register_cleanup_handler(Arc::new(DefaultCleanupHandler)).await;
        
        let metadata = ResourceMetadata {
            id: Uuid::new_v4(),
            resource_type: ResourceType::TempFile,
            state: ResourceState::Created,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: None,
            size_bytes: Some(512),
            tags: HashMap::new(),
            cleanup_priority: CleanupPriority::Normal,
        };
        
        let _id = manager.register_resource(metadata).await.unwrap();
        
        // Start cleanup process
        manager.start_cleanup_process().await.unwrap();
        
        // Wait for cleanup to occur
        sleep(Duration::from_millis(200)).await;
        
        let stats = manager.get_resource_stats().await;
        // Resource should be cleaned up due to age
        assert_eq!(stats.get(&ResourceType::TempFile).unwrap_or(&0), &0);
        
        manager.stop_cleanup_process().await;
    }
}