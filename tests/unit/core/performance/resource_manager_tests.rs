use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::performance::{
    ResourceManager, ProductionResourceManager, ResourceRequirements, 
    ResourceQuota, ResourcePriority, OptimizationReport
};

#[tokio::test]
async fn test_resource_allocation_and_release() {
    let quota = ResourceQuota {
        max_cpu_cores: 10,
        max_memory_mb: 1000,
        max_disk_gb: 100,
        max_network_bandwidth: 1000,
        max_concurrent_allocations: 5,
    };

    let manager = ProductionResourceManager::new(quota);

    let requirements = ResourceRequirements {
        cpu_cores: 2,
        memory_mb: 200,
        disk_gb: 10,
        network_bandwidth: 100,
        priority: ResourcePriority::Medium,
        timeout: Some(Duration::from_secs(60)),
    };

    // Test allocation
    let allocation = manager.allocate_resources(requirements.clone()).await.unwrap();
    assert_eq!(allocation.cpu_cores, 2);
    assert_eq!(allocation.memory_mb, 200);
    assert_eq!(allocation.disk_gb, 10);
    assert_eq!(allocation.network_bandwidth, 100);
    assert_eq!(allocation.priority, ResourcePriority::Medium);

    // Test resource usage
    let usage = manager.get_resource_usage().await.unwrap();
    assert_eq!(usage.allocated_cpu_cores, 2);
    assert_eq!(usage.allocated_memory_mb, 200);
    assert_eq!(usage.allocated_disk_gb, 10);
    assert_eq!(usage.allocated_network_bandwidth, 100);
    assert_eq!(usage.active_allocations, 1);

    // Test release
    manager.release_resources(allocation).await.unwrap();

    let usage_after_release = manager.get_resource_usage().await.unwrap();
    assert_eq!(usage_after_release.allocated_cpu_cores, 0);
    assert_eq!(usage_after_release.allocated_memory_mb, 0);
    assert_eq!(usage_after_release.allocated_disk_gb, 0);
    assert_eq!(usage_after_release.allocated_network_bandwidth, 0);
    assert_eq!(usage_after_release.active_allocations, 0);
}

#[tokio::test]
async fn test_resource_quota_enforcement() {
    let quota = ResourceQuota {
        max_cpu_cores: 4,
        max_memory_mb: 400,
        max_disk_gb: 40,
        max_network_bandwidth: 400,
        max_concurrent_allocations: 2,
    };

    let manager = ProductionResourceManager::new(quota);

    let requirements = ResourceRequirements {
        cpu_cores: 3,
        memory_mb: 300,
        disk_gb: 30,
        network_bandwidth: 300,
        priority: ResourcePriority::High,
        timeout: Some(Duration::from_secs(60)),
    };

    // First allocation should succeed
    let allocation1 = manager.allocate_resources(requirements.clone()).await.unwrap();

    // Second allocation should fail due to resource limits
    let result = manager.allocate_resources(requirements.clone()).await;
    assert!(result.is_err());

    // Release first allocation
    manager.release_resources(allocation1).await.unwrap();

    // Now second allocation should succeed
    let allocation2 = manager.allocate_resources(requirements).await.unwrap();
    manager.release_resources(allocation2).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_allocations_limit() {
    let quota = ResourceQuota {
        max_cpu_cores: 10,
        max_memory_mb: 1000,
        max_disk_gb: 100,
        max_network_bandwidth: 1000,
        max_concurrent_allocations: 2,
    };

    let manager = ProductionResourceManager::new(quota);

    let requirements = ResourceRequirements {
        cpu_cores: 1,
        memory_mb: 100,
        disk_gb: 10,
        network_bandwidth: 100,
        priority: ResourcePriority::Medium,
        timeout: Some(Duration::from_secs(60)),
    };

    // Allocate up to the limit
    let allocation1 = manager.allocate_resources(requirements.clone()).await.unwrap();
    let allocation2 = manager.allocate_resources(requirements.clone()).await.unwrap();

    // Third allocation should fail due to concurrent allocation limit
    let result = manager.allocate_resources(requirements.clone()).await;
    assert!(result.is_err());

    // Release one allocation
    manager.release_resources(allocation1).await.unwrap();

    // Now third allocation should succeed
    let allocation3 = manager.allocate_resources(requirements).await.unwrap();
    
    manager.release_resources(allocation2).await.unwrap();
    manager.release_resources(allocation3).await.unwrap();
}

#[tokio::test]
async fn test_resource_expiration() {
    let quota = ResourceQuota {
        max_cpu_cores: 10,
        max_memory_mb: 1000,
        max_disk_gb: 100,
        max_network_bandwidth: 1000,
        max_concurrent_allocations: 5,
    };

    let manager = ProductionResourceManager::new(quota);

    let requirements = ResourceRequirements {
        cpu_cores: 2,
        memory_mb: 200,
        disk_gb: 20,
        network_bandwidth: 200,
        priority: ResourcePriority::Medium,
        timeout: Some(Duration::from_millis(100)), // Very short timeout
    };

    // Allocate resources with short timeout
    let _allocation = manager.allocate_resources(requirements.clone()).await.unwrap();

    // Wait for expiration
    sleep(Duration::from_millis(150)).await;

    // Check that expired resources are cleaned up
    let usage = manager.get_resource_usage().await.unwrap();
    assert_eq!(usage.active_allocations, 0);

    // Should be able to allocate again after cleanup
    let allocation2 = manager.allocate_resources(requirements).await.unwrap();
    manager.release_resources(allocation2).await.unwrap();
}

#[tokio::test]
async fn test_optimization_recommendations() {
    let quota = ResourceQuota {
        max_cpu_cores: 10,
        max_memory_mb: 1000,
        max_disk_gb: 100,
        max_network_bandwidth: 1000,
        max_concurrent_allocations: 5,
    };

    let manager = ProductionResourceManager::new(quota);

    // Test low utilization scenario
    let low_requirements = ResourceRequirements {
        cpu_cores: 1,
        memory_mb: 100,
        disk_gb: 10,
        network_bandwidth: 100,
        priority: ResourcePriority::Low,
        timeout: None,
    };

    let allocation = manager.allocate_resources(low_requirements).await.unwrap();
    let report = manager.optimize_resource_usage().await.unwrap();
    
    // Should recommend scaling down due to low utilization
    assert!(!report.recommendations.is_empty());
    assert!(report.efficiency_score < 90.0);

    manager.release_resources(allocation).await.unwrap();

    // Test high utilization scenario
    let high_requirements = ResourceRequirements {
        cpu_cores: 9,
        memory_mb: 900,
        disk_gb: 90,
        network_bandwidth: 900,
        priority: ResourcePriority::High,
        timeout: None,
    };

    let allocation = manager.allocate_resources(high_requirements).await.unwrap();
    let report = manager.optimize_resource_usage().await.unwrap();
    
    // Should recommend scaling up due to high utilization
    assert!(!report.recommendations.is_empty());

    manager.release_resources(allocation).await.unwrap();
}

#[tokio::test]
async fn test_available_resources() {
    let quota = ResourceQuota {
        max_cpu_cores: 10,
        max_memory_mb: 1000,
        max_disk_gb: 100,
        max_network_bandwidth: 1000,
        max_concurrent_allocations: 5,
    };

    let manager = ProductionResourceManager::new(quota);

    // Initially, all resources should be available
    let available = manager.get_available_resources().await.unwrap();
    assert_eq!(available.cpu_cores, 10);
    assert_eq!(available.memory_mb, 1000);
    assert_eq!(available.disk_gb, 100);
    assert_eq!(available.network_bandwidth, 1000);

    // Allocate some resources
    let requirements = ResourceRequirements {
        cpu_cores: 3,
        memory_mb: 300,
        disk_gb: 30,
        network_bandwidth: 300,
        priority: ResourcePriority::Medium,
        timeout: None,
    };

    let allocation = manager.allocate_resources(requirements).await.unwrap();

    // Check remaining available resources
    let available_after = manager.get_available_resources().await.unwrap();
    assert_eq!(available_after.cpu_cores, 7);
    assert_eq!(available_after.memory_mb, 700);
    assert_eq!(available_after.disk_gb, 70);
    assert_eq!(available_after.network_bandwidth, 700);

    manager.release_resources(allocation).await.unwrap();
}

#[tokio::test]
async fn test_quota_updates() {
    let initial_quota = ResourceQuota {
        max_cpu_cores: 5,
        max_memory_mb: 500,
        max_disk_gb: 50,
        max_network_bandwidth: 500,
        max_concurrent_allocations: 3,
    };

    let manager = ProductionResourceManager::new(initial_quota);

    // Update quota
    let new_quota = ResourceQuota {
        max_cpu_cores: 10,
        max_memory_mb: 1000,
        max_disk_gb: 100,
        max_network_bandwidth: 1000,
        max_concurrent_allocations: 5,
    };

    manager.set_resource_quota(new_quota).await.unwrap();

    // Verify new quota is in effect
    let available = manager.get_available_resources().await.unwrap();
    assert_eq!(available.cpu_cores, 10);
    assert_eq!(available.memory_mb, 1000);
    assert_eq!(available.disk_gb, 100);
    assert_eq!(available.network_bandwidth, 1000);
}

#[tokio::test]
async fn test_priority_handling() {
    let quota = ResourceQuota {
        max_cpu_cores: 4,
        max_memory_mb: 400,
        max_disk_gb: 40,
        max_network_bandwidth: 400,
        max_concurrent_allocations: 5,
    };

    let manager = ProductionResourceManager::new(quota);

    let high_priority_req = ResourceRequirements {
        cpu_cores: 2,
        memory_mb: 200,
        disk_gb: 20,
        network_bandwidth: 200,
        priority: ResourcePriority::Critical,
        timeout: None,
    };

    let low_priority_req = ResourceRequirements {
        cpu_cores: 2,
        memory_mb: 200,
        disk_gb: 20,
        network_bandwidth: 200,
        priority: ResourcePriority::Low,
        timeout: None,
    };

    // Allocate high priority first
    let high_allocation = manager.allocate_resources(high_priority_req).await.unwrap();
    assert_eq!(high_allocation.priority, ResourcePriority::Critical);

    // Allocate low priority
    let low_allocation = manager.allocate_resources(low_priority_req).await.unwrap();
    assert_eq!(low_allocation.priority, ResourcePriority::Low);

    manager.release_resources(high_allocation).await.unwrap();
    manager.release_resources(low_allocation).await.unwrap();
}