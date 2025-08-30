use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use RustAutoDevOps::testing::{
    IntegrationTestManager, IntegrationTestManagerInterface, IntegrationTestConfig,
    IntegrationTest, IntegrationTestSuite, IntegrationTestType, MockEndpoint
};

#[tokio::test]
async fn test_integration_test_manager_creation() {
    let manager = IntegrationTestManager::new();
    assert_eq!(manager.config.test_database_name, "rustci_integration_test");
    assert_eq!(manager.config.parallel_test_limit, 4);
    assert!(manager.config.cleanup_after_tests);
}

#[tokio::test]
async fn test_integration_test_manager_with_custom_config() {
    let config = IntegrationTestConfig {
        database_url: "mongodb://custom:27017".to_string(),
        test_database_name: "custom_test_db".to_string(),
        mock_service_port_range: (9000, 9100),
        test_timeout: Duration::from_secs(600),
        cleanup_after_tests: false,
        parallel_test_limit: 8,
        retry_failed_tests: false,
        max_retries: 5,
    };

    let manager = IntegrationTestManager::with_config(config.clone());
    assert_eq!(manager.config.database_url, "mongodb://custom:27017");
    assert_eq!(manager.config.test_database_name, "custom_test_db");
    assert_eq!(manager.config.parallel_test_limit, 8);
    assert!(!manager.config.cleanup_after_tests);
}

#[tokio::test]
async fn test_setup_test_database() {
    let manager = IntegrationTestManager::new();
    let result = manager.setup_test_database().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_setup_mock_services() {
    let manager = IntegrationTestManager::new();
    let result = manager.setup_mock_services().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_and_destroy_test_environment() {
    let manager = IntegrationTestManager::new();
    
    // Create test environment
    let env_id = manager.create_test_environment("test_env").await.unwrap();
    assert!(!env_id.is_nil());
    
    // Verify environment exists
    let environments = manager.test_environments.lock().await;
    assert!(environments.contains_key(&env_id.to_string()));
    drop(environments);
    
    // Destroy test environment
    let result = manager.destroy_test_environment(env_id).await;
    assert!(result.is_ok());
    
    // Verify environment is removed
    let environments = manager.test_environments.lock().await;
    assert!(!environments.contains_key(&env_id.to_string()));
}

#[tokio::test]
async fn test_destroy_nonexistent_test_environment() {
    let manager = IntegrationTestManager::new();
    let fake_id = Uuid::new_v4();
    
    let result = manager.destroy_test_environment(fake_id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Test environment not found"));
}

#[tokio::test]
async fn test_discover_integration_tests() {
    let manager = IntegrationTestManager::new();
    let suites = manager.discover_integration_tests().await.unwrap();
    
    assert!(!suites.is_empty());
    assert!(suites.iter().any(|s| s.name == "API Integration Tests"));
    assert!(suites.iter().any(|s| s.name == "Database Integration Tests"));
    
    // Verify test suite structure
    let api_suite = suites.iter()
        .find(|s| s.name == "API Integration Tests")
        .unwrap();
    
    assert!(!api_suite.tests.is_empty());
    assert!(!api_suite.setup_scripts.is_empty());
    assert!(!api_suite.teardown_scripts.is_empty());
    assert!(api_suite.dependencies.contains(&"test_database".to_string()));
}

#[tokio::test]
async fn test_run_test_suite() {
    let manager = IntegrationTestManager::new();
    
    // Create a simple test suite
    let test_suite = IntegrationTestSuite {
        suite_id: Uuid::new_v4(),
        name: "Simple Test Suite".to_string(),
        tests: vec![
            IntegrationTest {
                test_id: Uuid::new_v4(),
                name: "echo_test".to_string(),
                description: "Simple echo test".to_string(),
                test_type: IntegrationTestType::ApiTest,
                command: "echo".to_string(),
                args: vec!["hello world".to_string()],
                environment_variables: HashMap::new(),
                expected_exit_code: 0,
                timeout: Duration::from_secs(30),
                retry_count: 0,
                dependencies: Vec::new(),
                tags: vec!["simple".to_string()],
            }
        ],
        setup_scripts: vec!["echo 'setup'".to_string()],
        teardown_scripts: vec!["echo 'teardown'".to_string()],
        dependencies: Vec::new(),
    };

    let result = manager.run_test_suite(&test_suite).await.unwrap();
    
    assert_eq!(result.total, 1);
    assert_eq!(result.passed, 1);
    assert_eq!(result.failed, 0);
    assert_eq!(result.skipped, 0);
}

#[tokio::test]
async fn test_run_failing_test_suite() {
    let manager = IntegrationTestManager::new();
    
    // Create a test suite with a failing test
    let test_suite = IntegrationTestSuite {
        suite_id: Uuid::new_v4(),
        name: "Failing Test Suite".to_string(),
        tests: vec![
            IntegrationTest {
                test_id: Uuid::new_v4(),
                name: "failing_test".to_string(),
                description: "Test that should fail".to_string(),
                test_type: IntegrationTestType::ApiTest,
                command: "false".to_string(), // Command that always fails
                args: Vec::new(),
                environment_variables: HashMap::new(),
                expected_exit_code: 0, // Expect success, but command will fail
                timeout: Duration::from_secs(30),
                retry_count: 1,
                dependencies: Vec::new(),
                tags: vec!["failing".to_string()],
            }
        ],
        setup_scripts: Vec::new(),
        teardown_scripts: Vec::new(),
        dependencies: Vec::new(),
    };

    let result = manager.run_test_suite(&test_suite).await.unwrap();
    
    assert_eq!(result.total, 1);
    assert_eq!(result.passed, 0);
    assert_eq!(result.failed, 1);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.failed_test_details.len(), 1);
    
    let failed_detail = &result.failed_test_details[0];
    assert_eq!(failed_detail.test_name, "failing_test");
    assert!(failed_detail.error_message.contains("exit code"));
}

#[tokio::test]
async fn test_cleanup_test_data() {
    let manager = IntegrationTestManager::new();
    
    // Setup some test data
    manager.setup_test_database().await.unwrap();
    manager.setup_mock_services().await.unwrap();
    
    // Verify data exists
    let db_guard = manager.test_database.lock().await;
    assert!(db_guard.is_some());
    drop(db_guard);
    
    let services_guard = manager.mock_services.lock().await;
    assert!(!services_guard.is_empty());
    drop(services_guard);
    
    // Cleanup
    let result = manager.cleanup_test_data().await;
    assert!(result.is_ok());
    
    // Verify cleanup
    let db_guard = manager.test_database.lock().await;
    assert!(db_guard.is_none());
    drop(db_guard);
    
    let services_guard = manager.mock_services.lock().await;
    assert!(services_guard.is_empty());
}

#[tokio::test]
async fn test_cleanup_disabled() {
    let config = IntegrationTestConfig {
        cleanup_after_tests: false,
        ..Default::default()
    };
    let manager = IntegrationTestManager::with_config(config);
    
    // Setup some test data
    manager.setup_test_database().await.unwrap();
    
    // Cleanup should not remove data when disabled
    let result = manager.cleanup_test_data().await;
    assert!(result.is_ok());
    
    // Data should still exist
    let db_guard = manager.test_database.lock().await;
    assert!(db_guard.is_some());
}

#[test]
fn test_integration_test_types() {
    use RustAutoDevOps::testing::IntegrationTestType;
    
    // Test serialization/deserialization
    let test_types = vec![
        IntegrationTestType::ApiTest,
        IntegrationTestType::DatabaseTest,
        IntegrationTestType::ServiceIntegration,
        IntegrationTestType::EndToEnd,
        IntegrationTestType::ContractTest,
        IntegrationTestType::PerformanceIntegration,
    ];
    
    for test_type in test_types {
        let serialized = serde_json::to_string(&test_type).unwrap();
        let deserialized: IntegrationTestType = serde_json::from_str(&serialized).unwrap();
        // Note: We can't directly compare enum variants without implementing PartialEq
        // This test ensures serialization/deserialization works
        assert!(!serialized.is_empty());
    }
}

#[test]
fn test_mock_endpoint_creation() {
    let endpoint = MockEndpoint {
        path: "/api/test".to_string(),
        method: "GET".to_string(),
        response_body: r#"{"status": "ok"}"#.to_string(),
        response_status: 200,
        response_headers: {
            let mut headers = HashMap::new();
            headers.insert("Content-Type".to_string(), "application/json".to_string());
            headers
        },
        delay_ms: Some(100),
    };
    
    assert_eq!(endpoint.path, "/api/test");
    assert_eq!(endpoint.method, "GET");
    assert_eq!(endpoint.response_status, 200);
    assert_eq!(endpoint.delay_ms, Some(100));
    assert!(endpoint.response_headers.contains_key("Content-Type"));
}

#[tokio::test]
async fn test_run_all_integration_tests() {
    let manager = IntegrationTestManager::new();
    
    // This will run the discovered test suites
    let result = manager.run_all_integration_tests().await.unwrap();
    
    // Should have some tests from the discovered suites
    assert!(result.total > 0);
    // Results depend on the actual test execution, but structure should be valid
    assert!(result.passed + result.failed + result.skipped == result.total);
}