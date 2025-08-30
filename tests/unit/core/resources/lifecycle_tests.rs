//! Tests for Resource Lifecycle Management System

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::resources::lifecycle::{
    ResourceLifecycleManager, ResourceMetadata, ResourceType, ResourceState,
    CleanupPriority, RetentionPolicy, ResourceCleanupHandler, DefaultCleanupHandler,
};

/// Test cleanup handler that tracks cleanup calls
struct TestCleanupHandler {
    cleaned_resources: Arc<tokio::sync::Mutex<Vec<Uuid>>>,
}

impl TestCleanupHandler {
    fn new() -> Self {
        Self {
            cleaned_resources: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    async fn get_cleaned_resources(&self) -> Vec<Uuid> {
        self.cleaned_resources.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl ResourceCleanupHandler for TestCleanupHandler {
    async fn cleanup_resource(&self, metadata: &ResourceMetadata) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cleaned = self.cleaned_resources.lock().await;
        cleaned.push(metadata.id);
        Ok(())
    }

    fn can_handle(&self, resource_type: &ResourceType) -> bool {
        matches!(resource_type, ResourceType::TempFile | ResourceType::CacheEntry)
    }

    fn get_handler_name(&self) -> &str {
        "TestCleanupHandler"
    }
}

#[tokio::test]
async fn test_resource_lifecycle_manager_creation() {
    let manager = ResourceLifecycleManager::new();
    let stats = manager.get_resource_stats().await;
    assert!(stats.is_empty());
}

#[tokio::test]
async fn test_resource_registration_and_retrieval() {
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
    
    let original_id = metadata.id;
    let registered_id = manager.register_resource(metadata).await.unwrap();
    
    assert_eq!(original_id, registered_id);
    
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.get(&ResourceType::TempFile), Some(&1));
    
    let temp_files = manager.get_resources_by_type(&ResourceType::TempFile).await;
    assert_eq!(temp_files.len(), 1);
    assert_eq!(temp_files[0].id, original_id);
    assert_eq!(temp_files[0].state, ResourceState::Created);
}

#[tokio::test]
async fn test_resource_state_transitions() {
    let manager = ResourceLifecycleManager::new();
    
    let metadata = ResourceMetadata {
        id: Uuid::new_v4(),
        resource_type: ResourceType::CacheEntry,
        state: ResourceState::Created,
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: Some(512),
        tags: HashMap::new(),
        cleanup_priority: CleanupPriority::High,
    };
    
    let id = manager.register_resource(metadata).await.unwrap();
    
    // Mark as idle
    manager.mark_idle(id).await.unwrap();
    let resources = manager.get_resources_by_type(&ResourceType::CacheEntry).await;
    assert_eq!(resources[0].state, ResourceState::Idle);
    
    // Touch resource (should become active)
    manager.touch_resource(id).await.unwrap();
    let resources = manager.get_resources_by_type(&ResourceType::CacheEntry).await;
    assert_eq!(resources[0].state, ResourceState::Active);
}

#[tokio::test]
async fn test_manual_resource_removal() {
    let manager = ResourceLifecycleManager::new();
    let handler = Arc::new(TestCleanupHandler::new());
    manager.register_cleanup_handler(handler.clone()).await;
    
    let metadata = ResourceMetadata {
        id: Uuid::new_v4(),
        resource_type: ResourceType::TempFile,
        state: ResourceState::Created,
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: Some(2048),
        tags: HashMap::new(),
        cleanup_priority: CleanupPriority::Low,
    };
    
    let id = manager.register_resource(metadata).await.unwrap();
    
    // Verify resource exists
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.get(&ResourceType::TempFile), Some(&1));
    
    // Remove resource
    manager.remove_resource(id).await.unwrap();
    
    // Verify resource is gone
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.get(&ResourceType::TempFile).unwrap_or(&0), &0);
    
    // Verify cleanup handler was called
    let cleaned = handler.get_cleaned_resources().await;
    assert_eq!(cleaned.len(), 1);
    assert_eq!(cleaned[0], id);
}

#[tokio::test]
async fn test_retention_policy_configuration() {
    let manager = ResourceLifecycleManager::new();
    
    let policy = RetentionPolicy {
        resource_type: ResourceType::UserSession,
        max_age: Duration::from_secs(3600),
        max_idle_time: Duration::from_secs(1800),
        max_size_bytes: Some(1024 * 1024),
        max_count: Some(100),
        cleanup_interval: Duration::from_secs(300),
    };
    
    manager.set_retention_policy(policy).await;
    
    // Test that policy is applied (indirectly through cleanup behavior)
    let metadata = ResourceMetadata {
        id: Uuid::new_v4(),
        resource_type: ResourceType::UserSession,
        state: ResourceState::Created,
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: Some(512),
        tags: HashMap::new(),
        cleanup_priority: CleanupPriority::Normal,
    };
    
    let _id = manager.register_resource(metadata).await.unwrap();
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.get(&ResourceType::UserSession), Some(&1));
}

#[tokio::test]
async fn test_automatic_cleanup_process() {
    let manager = ResourceLifecycleManager::new();
    let handler = Arc::new(TestCleanupHandler::new());
    manager.register_cleanup_handler(handler.clone()).await;
    
    // Set aggressive cleanup policy
    let policy = RetentionPolicy {
        resource_type: ResourceType::TempFile,
        max_age: Duration::from_millis(50),
        max_idle_time: Duration::from_millis(25),
        max_size_bytes: Some(1024),
        max_count: Some(10),
        cleanup_interval: Duration::from_millis(25),
    };
    manager.set_retention_policy(policy).await;
    
    // Create resource that will expire quickly
    let metadata = ResourceMetadata {
        id: Uuid::new_v4(),
        resource_type: ResourceType::TempFile,
        state: ResourceState::Created,
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: Some(256),
        tags: HashMap::new(),
        cleanup_priority: CleanupPriority::Normal,
    };
    
    let id = manager.register_resource(metadata).await.unwrap();
    
    // Start cleanup process
    manager.start_cleanup_process().await.unwrap();
    
    // Wait for cleanup to occur
    sleep(Duration::from_millis(100)).await;
    
    // Stop cleanup process
    manager.stop_cleanup_process().await;
    
    // Verify resource was cleaned up
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.get(&ResourceType::TempFile).unwrap_or(&0), &0);
    
    // Verify cleanup handler was called
    let cleaned = handler.get_cleaned_resources().await;
    assert!(cleaned.contains(&id));
}

#[tokio::test]
async fn test_cleanup_statistics() {
    let manager = ResourceLifecycleManager::new();
    let handler = Arc::new(TestCleanupHandler::new());
    manager.register_cleanup_handler(handler.clone()).await;
    
    // Set policy for immediate cleanup
    let policy = RetentionPolicy {
        resource_type: ResourceType::CacheEntry,
        max_age: Duration::from_millis(10),
        max_idle_time: Duration::from_millis(5),
        max_size_bytes: Some(1024),
        max_count: Some(5),
        cleanup_interval: Duration::from_millis(20),
    };
    manager.set_retention_policy(policy).await;
    
    // Create multiple resources
    for i in 0..3 {
        let metadata = ResourceMetadata {
            id: Uuid::new_v4(),
            resource_type: ResourceType::CacheEntry,
            state: ResourceState::Created,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: None,
            size_bytes: Some(100 * (i + 1)),
            tags: HashMap::new(),
            cleanup_priority: CleanupPriority::Normal,
        };
        manager.register_resource(metadata).await.unwrap();
    }
    
    // Start cleanup
    manager.start_cleanup_process().await.unwrap();
    
    // Wait for cleanup
    sleep(Duration::from_millis(50)).await;
    
    manager.stop_cleanup_process().await;
    
    // Check cleanup statistics
    let cleanup_stats = manager.get_cleanup_stats().await;
    assert!(cleanup_stats.total_resources_cleaned > 0);
    assert!(cleanup_stats.bytes_freed > 0);
    assert!(cleanup_stats.cleanup_runs > 0);
    assert!(cleanup_stats.last_cleanup.is_some());
}

#[tokio::test]
async fn test_resource_expiration() {
    let manager = ResourceLifecycleManager::new();
    let handler = Arc::new(DefaultCleanupHandler);
    manager.register_cleanup_handler(handler).await;
    
    // Create resource with immediate expiration
    let expires_at = SystemTime::now() + Duration::from_millis(10);
    let metadata = ResourceMetadata {
        id: Uuid::new_v4(),
        resource_type: ResourceType::ApiToken,
        state: ResourceState::Created,
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: Some(expires_at),
        size_bytes: Some(64),
        tags: HashMap::new(),
        cleanup_priority: CleanupPriority::High,
    };
    
    let _id = manager.register_resource(metadata).await.unwrap();
    
    // Set policy
    let policy = RetentionPolicy {
        resource_type: ResourceType::ApiToken,
        max_age: Duration::from_secs(3600),
        max_idle_time: Duration::from_secs(1800),
        max_size_bytes: Some(1024),
        max_count: Some(100),
        cleanup_interval: Duration::from_millis(20),
    };
    manager.set_retention_policy(policy).await;
    
    // Start cleanup
    manager.start_cleanup_process().await.unwrap();
    
    // Wait for expiration and cleanup
    sleep(Duration::from_millis(50)).await;
    
    manager.stop_cleanup_process().await;
    
    // Resource should be cleaned up due to expiration
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.get(&ResourceType::ApiToken).unwrap_or(&0), &0);
}

#[tokio::test]
async fn test_multiple_resource_types() {
    let manager = ResourceLifecycleManager::new();
    
    // Register different types of resources
    let types = vec![
        ResourceType::TempFile,
        ResourceType::CacheEntry,
        ResourceType::UserSession,
        ResourceType::ApiToken,
        ResourceType::Custom("test".to_string()),
    ];
    
    for resource_type in types.iter() {
        let metadata = ResourceMetadata {
            id: Uuid::new_v4(),
            resource_type: resource_type.clone(),
            state: ResourceState::Created,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: None,
            size_bytes: Some(128),
            tags: HashMap::new(),
            cleanup_priority: CleanupPriority::Normal,
        };
        manager.register_resource(metadata).await.unwrap();
    }
    
    let stats = manager.get_resource_stats().await;
    assert_eq!(stats.len(), 5);
    
    for resource_type in types.iter() {
        assert_eq!(stats.get(resource_type), Some(&1));
    }
}

#[tokio::test]
async fn test_resource_tags_and_metadata() {
    let manager = ResourceLifecycleManager::new();
    
    let mut tags = HashMap::new();
    tags.insert("owner".to_string(), "test_user".to_string());
    tags.insert("environment".to_string(), "test".to_string());
    
    let metadata = ResourceMetadata {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Custom("tagged_resource".to_string()),
        state: ResourceState::Created,
        created_at: SystemTime::now(),
        last_accessed: SystemTime::now(),
        expires_at: None,
        size_bytes: Some(256),
        tags,
        cleanup_priority: CleanupPriority::High,
    };
    
    let id = manager.register_resource(metadata).await.unwrap();
    
    let resources = manager.get_resources_by_type(&ResourceType::Custom("tagged_resource".to_string())).await;
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].tags.get("owner"), Some(&"test_user".to_string()));
    assert_eq!(resources[0].tags.get("environment"), Some(&"test".to_string()));
    assert_eq!(resources[0].cleanup_priority, CleanupPriority::High);
}