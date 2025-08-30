//! Tests for Resource Quota and Throttling System

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::resources::quota_throttling::{
    ResourceQuotaManager, ResourceQuota, ResourceType, AllocationRequest, AllocationPriority,
    ThrottlingConfig, BackoffStrategy, ResourceMonitor, AlertHandler, ResourceAlert,
    AlertType, AlertSeverity, LoggingAlertHandler,
};

/// Test resource monitor for testing monitoring functionality
struct TestResourceMonitor {
    name: String,
    usage_values: HashMap<ResourceType, u64>,
}

impl TestResourceMonitor {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            usage_values: HashMap::new(),
        }
    }

    fn set_usage(&mut self, resource_type: ResourceType, usage: u64) {
        self.usage_values.insert(resource_type, usage);
    }
}

#[async_trait::async_trait]
impl ResourceMonitor for TestResourceMonitor {
    async fn get_current_usage(&self, resource_type: &ResourceType) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.usage_values.get(resource_type).copied().unwrap_or(0))
    }

    fn get_monitor_name(&self) -> &str {
        &self.name
    }
}

/// Test alert handler for testing alert functionality
struct TestAlertHandler {
    name: String,
    handled_alerts: Arc<tokio::sync::Mutex<Vec<ResourceAlert>>>,
}

impl TestAlertHandler {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            handled_alerts: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    async fn get_handled_alerts(&self) -> Vec<ResourceAlert> {
        let alerts = self.handled_alerts.lock().await;
        alerts.clone()
    }
}

#[async_trait::async_trait]
impl AlertHandler for TestAlertHandler {
    async fn handle_alert(&self, alert: &ResourceAlert) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut alerts = self.handled_alerts.lock().await;
        alerts.push(alert.clone());
        Ok(())
    }

    fn can_handle(&self, alert_type: &AlertType) -> bool {
        matches!(alert_type, AlertType::WarningThreshold | AlertType::CriticalThreshold)
    }

    fn get_handler_name(&self) -> &str {
        &self.name
    }
}

#[tokio::test]
async fn test_resource_quota_manager_creation() {
    let manager = ResourceQuotaManager::new();
    let stats = manager.get_stats().await;

    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_allocations, 0);
    assert_eq!(stats.failed_allocations, 0);
    assert_eq!(stats.throttled_requests, 0);
    assert_eq!(stats.active_allocations, 0);
    assert_eq!(stats.alerts_triggered, 0);
}

#[tokio::test]
async fn test_quota_configuration_and_basic_allocation() {
    let manager = ResourceQuotaManager::new();

    // Configure quota for memory
    let quota = ResourceQuota {
        resource_type: ResourceType::Memory,
        max_allocation: 2048,
        warning_threshold: 1600,
        critical_threshold: 1900,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(200),
        recovery_rate: Some(50),
    };
    manager.set_quota(quota).await;

    // Request allocation within limits
    let request = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Memory,
        amount: 1024,
        priority: AllocationPriority::Normal,
        requester_id: "test_user_1".to_string(),
        timeout: Some(Duration::from_secs(300)),
        tags: {
            let mut tags = HashMap::new();
            tags.insert("application".to_string(), "test_app".to_string());
            tags
        },
    };

    let result = manager.request_allocation(request).await;

    assert!(result.success);
    assert_eq!(result.allocated_amount, 1024);
    assert!(result.allocation_id.is_some());
    assert!(result.error_message.is_none());
    assert!(result.wait_time >= Duration::from_nanos(0));

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.successful_allocations, 1);
    assert_eq!(stats.failed_allocations, 0);
    assert_eq!(stats.active_allocations, 1);
}

#[tokio::test]
async fn test_quota_exceeded_rejection() {
    let manager = ResourceQuotaManager::new();

    // Configure small quota
    let quota = ResourceQuota {
        resource_type: ResourceType::CPU,
        max_allocation: 500,
        warning_threshold: 400,
        critical_threshold: 450,
        time_window: Duration::from_secs(30),
        burst_allowance: Some(50),
        recovery_rate: Some(25),
    };
    manager.set_quota(quota).await;

    // Request allocation that exceeds quota
    let request = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::CPU,
        amount: 600,
        priority: AllocationPriority::High,
        requester_id: "greedy_user".to_string(),
        timeout: None,
        tags: HashMap::new(),
    };

    let result = manager.request_allocation(request).await;

    assert!(!result.success);
    assert_eq!(result.allocated_amount, 0);
    assert!(result.allocation_id.is_none());
    assert!(result.error_message.is_some());
    assert!(result.error_message.unwrap().contains("Quota exceeded"));

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.successful_allocations, 0);
    assert_eq!(stats.failed_allocations, 1);
    assert_eq!(stats.active_allocations, 0);
}

#[tokio::test]
async fn test_multiple_allocations_and_quota_enforcement() {
    let manager = ResourceQuotaManager::new();

    let quota = ResourceQuota {
        resource_type: ResourceType::Disk,
        max_allocation: 1000,
        warning_threshold: 800,
        critical_threshold: 900,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(100),
        recovery_rate: Some(20),
    };
    manager.set_quota(quota).await;

    // Make multiple allocations
    let mut allocation_ids = Vec::new();

    // First allocation (should succeed)
    let request1 = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Disk,
        amount: 400,
        priority: AllocationPriority::Normal,
        requester_id: "user1".to_string(),
        timeout: None,
        tags: HashMap::new(),
    };
    let result1 = manager.request_allocation(request1).await;
    assert!(result1.success);
    allocation_ids.push(result1.allocation_id.unwrap());

    // Second allocation (should succeed)
    let request2 = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Disk,
        amount: 300,
        priority: AllocationPriority::Normal,
        requester_id: "user2".to_string(),
        timeout: None,
        tags: HashMap::new(),
    };
    let result2 = manager.request_allocation(request2).await;
    assert!(result2.success);
    allocation_ids.push(result2.allocation_id.unwrap());

    // Third allocation (should fail - would exceed quota)
    let request3 = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Disk,
        amount: 400,
        priority: AllocationPriority::Normal,
        requester_id: "user3".to_string(),
        timeout: None,
        tags: HashMap::new(),
    };
    let result3 = manager.request_allocation(request3).await;
    assert!(!result3.success);

    let stats = manager.get_stats().await;
    assert_eq!(stats.successful_allocations, 2);
    assert_eq!(stats.failed_allocations, 1);
    assert_eq!(stats.active_allocations, 2);

    // Release one allocation
    manager.release_allocation(allocation_ids[0]).await.unwrap();

    let stats_after_release = manager.get_stats().await;
    assert_eq!(stats_after_release.active_allocations, 1);

    // Now the third allocation should succeed
    let result3_retry = manager.request_allocation(request3).await;
    assert!(result3_retry.success);
}

#[tokio::test]
async fn test_allocation_release_and_cleanup() {
    let manager = ResourceQuotaManager::new();

    let quota = ResourceQuota {
        resource_type: ResourceType::Network,
        max_allocation: 800,
        warning_threshold: 600,
        critical_threshold: 700,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(80),
        recovery_rate: Some(40),
    };
    manager.set_quota(quota).await;

    // Allocate resource
    let request = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Network,
        amount: 250,
        priority: AllocationPriority::High,
        requester_id: "network_user".to_string(),
        timeout: Some(Duration::from_secs(120)),
        tags: HashMap::new(),
    };

    let result = manager.request_allocation(request).await;
    assert!(result.success);
    let allocation_id = result.allocation_id.unwrap();

    // Verify allocation exists
    let active_allocations = manager.get_active_allocations().await;
    assert_eq!(active_allocations.len(), 1);
    assert_eq!(active_allocations[0].id, allocation_id);
    assert_eq!(active_allocations[0].amount, 250);

    // Release allocation
    let release_result = manager.release_allocation(allocation_id).await;
    assert!(release_result.is_ok());

    // Verify allocation is gone
    let active_allocations_after = manager.get_active_allocations().await;
    assert_eq!(active_allocations_after.len(), 0);

    let stats = manager.get_stats().await;
    assert_eq!(stats.active_allocations, 0);

    // Try to release non-existent allocation
    let fake_id = Uuid::new_v4();
    let fake_release_result = manager.release_allocation(fake_id).await;
    assert!(fake_release_result.is_err());
}

#[tokio::test]
async fn test_throttling_configuration_and_enforcement() {
    let manager = ResourceQuotaManager::new();

    // Configure throttling
    let throttling_config = ThrottlingConfig {
        resource_type: ResourceType::Requests,
        rate_limit: 5,
        time_window: Duration::from_secs(1),
        burst_size: 3,
        queue_size: 10,
        backoff_strategy: BackoffStrategy::Fixed { delay: Duration::from_millis(100) },
    };
    manager.set_throttling_config(throttling_config).await;

    // Make rapid requests to trigger throttling
    let mut successful_requests = 0;
    let mut throttled_requests = 0;

    for i in 0..8 {
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Requests,
            amount: 1,
            priority: AllocationPriority::Normal,
            requester_id: format!("rapid_user_{}", i),
            timeout: None,
            tags: HashMap::new(),
        };

        let result = manager.request_allocation(request).await;
        if result.success {
            successful_requests += 1;
        } else if result.error_message.as_ref().map_or(false, |msg| msg.contains("throttled")) {
            throttled_requests += 1;
        }
    }

    // Some requests should be throttled due to burst limit
    assert!(throttled_requests > 0);
    assert!(successful_requests > 0);

    let stats = manager.get_stats().await;
    assert!(stats.throttled_requests > 0);
}

#[tokio::test]
async fn test_resource_usage_tracking() {
    let manager = ResourceQuotaManager::new();

    let quota = ResourceQuota {
        resource_type: ResourceType::Files,
        max_allocation: 1000,
        warning_threshold: 750,
        critical_threshold: 900,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(100),
        recovery_rate: Some(50),
    };
    manager.set_quota(quota).await;

    // Make several allocations
    let mut allocation_ids = Vec::new();
    let amounts = vec![200, 150, 300];

    for (i, amount) in amounts.iter().enumerate() {
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Files,
            amount: *amount,
            priority: AllocationPriority::Normal,
            requester_id: format!("file_user_{}", i),
            timeout: None,
            tags: HashMap::new(),
        };

        let result = manager.request_allocation(request).await;
        assert!(result.success);
        allocation_ids.push(result.allocation_id.unwrap());
    }

    // Check resource usage
    let usage = manager.get_resource_usage(&ResourceType::Files).await;
    assert!(usage.is_some());

    let usage = usage.unwrap();
    assert_eq!(usage.current_usage, 650); // 200 + 150 + 300
    assert_eq!(usage.allocation_count, 3);
    assert_eq!(usage.peak_usage, 650);

    // Release one allocation
    manager.release_allocation(allocation_ids[1]).await.unwrap();

    let usage_after_release = manager.get_resource_usage(&ResourceType::Files).await.unwrap();
    assert_eq!(usage_after_release.current_usage, 500); // 650 - 150
    assert_eq!(usage_after_release.allocation_count, 2);
}

#[tokio::test]
async fn test_allocation_priority_handling() {
    let manager = ResourceQuotaManager::new();

    let quota = ResourceQuota {
        resource_type: ResourceType::Custom("priority_test".to_string()),
        max_allocation: 100,
        warning_threshold: 80,
        critical_threshold: 90,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(10),
        recovery_rate: Some(5),
    };
    manager.set_quota(quota).await;

    // Test different priority levels
    let priorities = vec![
        AllocationPriority::Low,
        AllocationPriority::Normal,
        AllocationPriority::High,
        AllocationPriority::Critical,
    ];

    for (i, priority) in priorities.iter().enumerate() {
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Custom("priority_test".to_string()),
            amount: 20,
            priority: priority.clone(),
            requester_id: format!("priority_user_{}", i),
            timeout: None,
            tags: HashMap::new(),
        };

        let result = manager.request_allocation(request).await;
        assert!(result.success);

        // Verify allocation has correct priority
        let allocations = manager.get_active_allocations().await;
        let allocation = allocations.iter().find(|a| a.requester_id == format!("priority_user_{}", i)).unwrap();
        assert_eq!(allocation.priority, *priority);
    }
}

#[tokio::test]
async fn test_allocations_by_requester() {
    let manager = ResourceQuotaManager::new();

    let quota = ResourceQuota {
        resource_type: ResourceType::Connections,
        max_allocation: 500,
        warning_threshold: 400,
        critical_threshold: 450,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(50),
        recovery_rate: Some(25),
    };
    manager.set_quota(quota).await;

    // Create allocations for different users
    let user1_amounts = vec![50, 75];
    let user2_amounts = vec![100, 25, 60];

    for amount in &user1_amounts {
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Connections,
            amount: *amount,
            priority: AllocationPriority::Normal,
            requester_id: "user1".to_string(),
            timeout: None,
            tags: HashMap::new(),
        };
        manager.request_allocation(request).await;
    }

    for amount in &user2_amounts {
        let request = AllocationRequest {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Connections,
            amount: *amount,
            priority: AllocationPriority::Normal,
            requester_id: "user2".to_string(),
            timeout: None,
            tags: HashMap::new(),
        };
        manager.request_allocation(request).await;
    }

    // Check allocations by requester
    let user1_allocations = manager.get_allocations_by_requester("user1").await;
    let user2_allocations = manager.get_allocations_by_requester("user2").await;
    let user3_allocations = manager.get_allocations_by_requester("user3").await;

    assert_eq!(user1_allocations.len(), 2);
    assert_eq!(user2_allocations.len(), 3);
    assert_eq!(user3_allocations.len(), 0);

    let user1_total: u64 = user1_allocations.iter().map(|a| a.amount).sum();
    let user2_total: u64 = user2_allocations.iter().map(|a| a.amount).sum();

    assert_eq!(user1_total, 125); // 50 + 75
    assert_eq!(user2_total, 185); // 100 + 25 + 60
}

#[tokio::test]
async fn test_resource_monitor_registration() {
    let manager = ResourceQuotaManager::new();

    let mut monitor = TestResourceMonitor::new("test_monitor");
    monitor.set_usage(ResourceType::Memory, 512);
    monitor.set_usage(ResourceType::CPU, 256);

    manager.register_monitor(Arc::new(monitor)).await;

    // Start monitoring (in a real test, we would verify monitoring behavior)
    manager.start().await.unwrap();
    sleep(Duration::from_millis(50)).await;
    manager.stop().await;
}

#[tokio::test]
async fn test_alert_handler_registration_and_triggering() {
    let manager = ResourceQuotaManager::new();

    let alert_handler = Arc::new(TestAlertHandler::new("test_handler"));
    manager.register_alert_handler(alert_handler.clone()).await;

    // Also register the logging handler
    manager.register_alert_handler(Arc::new(LoggingAlertHandler)).await;

    // Configure quota with low thresholds to trigger alerts
    let quota = ResourceQuota {
        resource_type: ResourceType::Memory,
        max_allocation: 100,
        warning_threshold: 60,
        critical_threshold: 80,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(10),
        recovery_rate: Some(5),
    };
    manager.set_quota(quota).await;

    // Make allocation that should trigger warning
    let request = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Memory,
        amount: 70, // Above warning threshold
        priority: AllocationPriority::Normal,
        requester_id: "alert_user".to_string(),
        timeout: None,
        tags: HashMap::new(),
    };

    let result = manager.request_allocation(request).await;
    assert!(result.success);

    // Give some time for alert processing
    sleep(Duration::from_millis(10)).await;

    let handled_alerts = alert_handler.get_handled_alerts().await;
    assert!(!handled_alerts.is_empty());

    let warning_alert = handled_alerts.iter().find(|a| a.alert_type == AlertType::WarningThreshold);
    assert!(warning_alert.is_some());

    let stats = manager.get_stats().await;
    assert!(stats.alerts_triggered > 0);
}

#[tokio::test]
async fn test_backoff_strategies() {
    // Test different backoff strategies
    let linear_backoff = BackoffStrategy::Linear { increment: Duration::from_millis(50) };
    let exponential_backoff = BackoffStrategy::Exponential { base: 2.0, max_delay: Duration::from_secs(5) };
    let fixed_backoff = BackoffStrategy::Fixed { delay: Duration::from_millis(100) };
    let custom_backoff = BackoffStrategy::Custom("custom_strategy".to_string());

    // Verify they can be created and cloned
    let _linear_clone = linear_backoff.clone();
    let _exponential_clone = exponential_backoff.clone();
    let _fixed_clone = fixed_backoff.clone();
    let _custom_clone = custom_backoff.clone();
}

#[tokio::test]
async fn test_resource_types() {
    // Test different resource types
    let memory = ResourceType::Memory;
    let cpu = ResourceType::CPU;
    let disk = ResourceType::Disk;
    let network = ResourceType::Network;
    let connections = ResourceType::Connections;
    let requests = ResourceType::Requests;
    let files = ResourceType::Files;
    let custom = ResourceType::Custom("custom_resource".to_string());

    // Test equality
    assert_eq!(memory, ResourceType::Memory);
    assert_eq!(custom, ResourceType::Custom("custom_resource".to_string()));
    assert_ne!(memory, cpu);
    assert_ne!(custom, ResourceType::Custom("other_resource".to_string()));

    // Test that they can be used as HashMap keys
    let mut resource_map = HashMap::new();
    resource_map.insert(memory, 100);
    resource_map.insert(cpu, 200);
    resource_map.insert(custom, 300);

    assert_eq!(resource_map.get(&ResourceType::Memory), Some(&100));
    assert_eq!(resource_map.get(&ResourceType::CPU), Some(&200));
    assert_eq!(resource_map.get(&ResourceType::Custom("custom_resource".to_string())), Some(&300));
}

#[tokio::test]
async fn test_automatic_cleanup_of_expired_allocations() {
    let manager = ResourceQuotaManager::new();

    let quota = ResourceQuota {
        resource_type: ResourceType::Custom("cleanup_test".to_string()),
        max_allocation: 1000,
        warning_threshold: 800,
        critical_threshold: 900,
        time_window: Duration::from_secs(60),
        burst_allowance: Some(100),
        recovery_rate: Some(50),
    };
    manager.set_quota(quota).await;

    // Create allocation with short timeout
    let request = AllocationRequest {
        id: Uuid::new_v4(),
        resource_type: ResourceType::Custom("cleanup_test".to_string()),
        amount: 200,
        priority: AllocationPriority::Normal,
        requester_id: "cleanup_user".to_string(),
        timeout: Some(Duration::from_millis(50)), // Very short timeout
        tags: HashMap::new(),
    };

    let result = manager.request_allocation(request).await;
    assert!(result.success);

    // Start cleanup process
    manager.start().await.unwrap();

    // Wait for allocation to expire and cleanup to run
    sleep(Duration::from_millis(150)).await;

    manager.stop().await;

    // Allocation should be cleaned up
    let stats = manager.get_stats().await;
    assert_eq!(stats.active_allocations, 0);
}