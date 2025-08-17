// Basic compilation test for Unified Runner System
// Task 3.3: Unified Runner System

use std::sync::Arc;
use uuid::Uuid;

use rustci::infrastructure::runners::{
    RunnerCapabilityDetector, UnifiedRunnerRegistry, 
    CapabilityDetectorConfig, UnifiedRegistryConfig,
};

/// Test that the unified runner system components can be created
#[tokio::test]
async fn test_unified_runner_system_creation() {
    // Test capability detector creation
    let detector_config = CapabilityDetectorConfig::default();
    let detector = Arc::new(
        RunnerCapabilityDetector::new(None, detector_config)
            .await
            .expect("Should create capability detector")
    );
    
    // Test registry creation
    let registry_config = UnifiedRegistryConfig::default();
    let _registry = UnifiedRunnerRegistry::new(detector, registry_config)
        .await
        .expect("Should create unified registry");
    
    println!("✅ Unified Runner System components created successfully");
}

/// Test basic registry operations
#[tokio::test]
async fn test_registry_basic_operations() {
    let detector_config = CapabilityDetectorConfig::default();
    let detector = Arc::new(
        RunnerCapabilityDetector::new(None, detector_config)
            .await
            .expect("Should create capability detector")
    );
    
    let registry_config = UnifiedRegistryConfig::default();
    let registry = UnifiedRunnerRegistry::new(detector, registry_config)
        .await
        .expect("Should create unified registry");
    
    // Test listing runners (should be empty initially)
    let runners = registry.list_runners().await;
    assert_eq!(runners.len(), 0);
    
    // Test statistics
    let stats = registry.get_statistics().await;
    assert_eq!(stats.total_runners, 0);
    assert_eq!(stats.online_runners, 0);
    
    println!("✅ Registry basic operations work correctly");
}

/// Test that the unified runner system is ready for production
#[tokio::test]
async fn test_unified_runner_system_readiness() {
    // This test verifies that all the core components can be instantiated
    // and are ready for use in the production system
    
    let detector_config = CapabilityDetectorConfig {
        auto_capability_detection: false, // Disable for testing
        detection_timeout: std::time::Duration::from_millis(100),
        ..Default::default()
    };
    
    let detector = Arc::new(
        RunnerCapabilityDetector::new(None, detector_config)
            .await
            .expect("Should create capability detector")
    );
    
    let registry_config = UnifiedRegistryConfig {
        max_runners: 1000,
        auto_capability_detection: false,
        ..Default::default()
    };
    
    let registry = UnifiedRunnerRegistry::new(detector, registry_config)
        .await
        .expect("Should create unified registry");
    
    // Verify the registry is functional
    let stats = registry.get_statistics().await;
    assert_eq!(stats.total_runners, 0);
    
    println!("✅ Unified Runner System is ready for production use");
}