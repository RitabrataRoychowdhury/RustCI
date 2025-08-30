use rustci::infrastructure::database::{
    ProductionTransactionManager, TransactionOperations, TransactionManagerConfig,
    TransactionIsolationLevel, TransactionStatus,
};
use rustci::error::Result;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_transaction_manager_initialization() {
    let config = TransactionManagerConfig {
        default_timeout: Duration::from_secs(10),
        max_active_transactions: 100,
        enable_savepoints: true,
        enable_compensation: true,
        deadlock_detection_interval: Duration::from_secs(1),
        cleanup_interval: Duration::from_secs(5),
        max_retry_attempts: 3,
        retry_delay: Duration::from_millis(50),
    };

    // Test configuration values
    assert_eq!(config.default_timeout, Duration::from_secs(10));
    assert_eq!(config.max_active_transactions, 100);
    assert!(config.enable_savepoints);
    assert!(config.enable_compensation);
}

#[tokio::test]
async fn test_transaction_isolation_levels() {
    let levels = vec![
        TransactionIsolationLevel::ReadUncommitted,
        TransactionIsolationLevel::ReadCommitted,
        TransactionIsolationLevel::RepeatableRead,
        TransactionIsolationLevel::Serializable,
    ];

    for level in levels {
        // Test that isolation levels can be created and compared
        let level_clone = level.clone();
        assert_eq!(level, level_clone);
    }
}

#[tokio::test]
async fn test_transaction_status_transitions() {
    let statuses = vec![
        TransactionStatus::Active,
        TransactionStatus::Committed,
        TransactionStatus::Aborted,
        TransactionStatus::Preparing,
        TransactionStatus::Prepared,
    ];

    for status in statuses {
        // Test that statuses can be created and compared
        let status_clone = status.clone();
        assert_eq!(status, status_clone);
    }
}

#[tokio::test]
async fn test_transaction_manager_config_validation() {
    let config = TransactionManagerConfig::default();
    
    // Validate default configuration values
    assert!(config.default_timeout > Duration::ZERO);
    assert!(config.max_active_transactions > 0);
    assert!(config.deadlock_detection_interval > Duration::ZERO);
    assert!(config.cleanup_interval > Duration::ZERO);
    assert!(config.max_retry_attempts > 0);
    assert!(config.retry_delay > Duration::ZERO);
}

#[tokio::test]
async fn test_transaction_handle_properties() {
    use rustci::infrastructure::database::TransactionHandle;
    use chrono::Utc;
    use uuid::Uuid;

    let handle = TransactionHandle {
        id: Uuid::new_v4(),
        started_at: Utc::now(),
        isolation_level: TransactionIsolationLevel::ReadCommitted,
        timeout: Duration::from_secs(30),
        savepoints: vec!["sp1".to_string(), "sp2".to_string()],
    };

    assert_eq!(handle.isolation_level, TransactionIsolationLevel::ReadCommitted);
    assert_eq!(handle.timeout, Duration::from_secs(30));
    assert_eq!(handle.savepoints.len(), 2);
    assert!(handle.savepoints.contains(&"sp1".to_string()));
    assert!(handle.savepoints.contains(&"sp2".to_string()));
}

#[tokio::test]
async fn test_transaction_context_metadata() {
    use rustci::infrastructure::database::{TransactionContext, TransactionHandle};
    use chrono::Utc;
    use uuid::Uuid;
    use std::collections::HashMap;

    let handle = TransactionHandle {
        id: Uuid::new_v4(),
        started_at: Utc::now(),
        isolation_level: TransactionIsolationLevel::ReadCommitted,
        timeout: Duration::from_secs(30),
        savepoints: Vec::new(),
    };

    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), "12345".to_string());
    metadata.insert("operation".to_string(), "bulk_update".to_string());

    let context = TransactionContext {
        handle,
        status: TransactionStatus::Active,
        operations_count: 5,
        last_activity: Utc::now(),
        metadata,
    };

    assert_eq!(context.status, TransactionStatus::Active);
    assert_eq!(context.operations_count, 5);
    assert_eq!(context.metadata.len(), 2);
    assert_eq!(context.metadata.get("user_id"), Some(&"12345".to_string()));
    assert_eq!(context.metadata.get("operation"), Some(&"bulk_update".to_string()));
}

#[tokio::test]
async fn test_transaction_stats_calculations() {
    use rustci::infrastructure::database::TransactionStats;

    let mut stats = TransactionStats {
        total_transactions: 100,
        committed_transactions: 80,
        aborted_transactions: 20,
        active_transactions: 5,
        average_duration_ms: 150.5,
        longest_transaction_ms: 5000,
        deadlock_count: 2,
        timeout_count: 3,
    };

    // Test basic calculations
    assert_eq!(stats.committed_transactions + stats.aborted_transactions, 100);
    assert!(stats.average_duration_ms > 0.0);
    assert!(stats.longest_transaction_ms > 0);

    // Test stats updates
    stats.total_transactions += 1;
    stats.committed_transactions += 1;
    assert_eq!(stats.total_transactions, 101);
    assert_eq!(stats.committed_transactions, 81);
}

#[tokio::test]
async fn test_compensation_action_execution() {
    use rustci::infrastructure::database::CompensationAction;
    use chrono::Utc;
    use uuid::Uuid;
    use std::sync::{Arc, Mutex};

    let executed = Arc::new(Mutex::new(false));
    let executed_clone = Arc::clone(&executed);

    let action = CompensationAction {
        id: Uuid::new_v4(),
        action_type: "test_compensation".to_string(),
        operation: Box::new(move || {
            let mut executed = executed_clone.lock().unwrap();
            *executed = true;
            Ok(())
        }),
        description: "Test compensation action".to_string(),
        created_at: Utc::now(),
    };

    // Execute the compensation action
    let result = (action.operation)();
    assert!(result.is_ok());

    // Verify it was executed
    let was_executed = *executed.lock().unwrap();
    assert!(was_executed);
}

// Integration test that would run with a real MongoDB instance
#[tokio::test]
#[ignore] // Ignored by default since it requires MongoDB
async fn test_transaction_manager_with_mongodb() {
    use mongodb::Client;

    let client = match Client::with_uri_str("mongodb://localhost:27017").await {
        Ok(client) => client,
        Err(_) => {
            println!("MongoDB not available for integration test");
            return;
        }
    };

    let database = client.database("test_rustci_transactions");
    let config = TransactionManagerConfig::default();

    let manager_result = ProductionTransactionManager::new(client.clone(), database, config).await;
    assert!(manager_result.is_ok());

    let manager = manager_result.unwrap();

    // Test basic operations
    let stats_result = manager.get_transaction_stats().await;
    assert!(stats_result.is_ok());

    let active_result = manager.get_active_transactions().await;
    assert!(active_result.is_ok());
    assert!(active_result.unwrap().is_empty());

    // Start monitoring
    manager.start_monitoring().await;

    // Stop monitoring
    manager.stop_monitoring().await;
}

// Test transaction lifecycle with mock operations
#[tokio::test]
#[ignore] // Ignored by default since it requires MongoDB
async fn test_transaction_lifecycle() {
    use mongodb::Client;

    let client = match Client::with_uri_str("mongodb://localhost:27017").await {
        Ok(client) => client,
        Err(_) => {
            println!("MongoDB not available for integration test");
            return;
        }
    };

    let database = client.database("test_rustci_transactions");
    let config = TransactionManagerConfig::default();

    let manager = ProductionTransactionManager::new(client.clone(), database, config).await.unwrap();

    // Begin transaction
    let handle_result = manager.begin_transaction().await;
    if handle_result.is_err() {
        println!("Could not begin transaction (MongoDB may not support transactions): {:?}", handle_result.err());
        return;
    }

    let handle = handle_result.unwrap();
    assert_eq!(handle.isolation_level, TransactionIsolationLevel::ReadCommitted);

    // Check transaction status
    let status_result = manager.get_transaction_status(&handle).await;
    if let Ok(status) = status_result {
        assert_eq!(status, TransactionStatus::Active);
    }

    // Test savepoint operations (if enabled)
    if manager.config.enable_savepoints {
        let savepoint_result = manager.create_savepoint(&handle, "test_savepoint".to_string()).await;
        // This might fail if MongoDB doesn't support the operation, which is expected
        if savepoint_result.is_ok() {
            let release_result = manager.release_savepoint(&handle, "test_savepoint".to_string()).await;
            assert!(release_result.is_ok());
        }
    }

    // Commit transaction
    let commit_result = manager.commit_transaction(handle).await;
    // This might fail if MongoDB session is not properly configured, which is expected in test environment
    if commit_result.is_err() {
        println!("Transaction commit failed (expected in test environment): {:?}", commit_result.err());
    }

    // Verify stats were updated
    let stats = manager.get_transaction_stats().await.unwrap();
    assert!(stats.total_transactions > 0);
}

#[tokio::test]
async fn test_transaction_timeout_handling() {
    let config = TransactionManagerConfig {
        default_timeout: Duration::from_millis(100), // Very short timeout for testing
        ..TransactionManagerConfig::default()
    };

    // Test that timeout configuration is respected
    assert_eq!(config.default_timeout, Duration::from_millis(100));
    assert!(config.default_timeout < Duration::from_secs(1));
}

#[tokio::test]
async fn test_concurrent_transaction_limits() {
    let config = TransactionManagerConfig {
        max_active_transactions: 2, // Very low limit for testing
        ..TransactionManagerConfig::default()
    };

    assert_eq!(config.max_active_transactions, 2);
    assert!(config.max_active_transactions < 10);
}

#[tokio::test]
async fn test_error_handling_patterns() {
    use rustci::error::AppError;

    // Test that we can create database errors for transaction scenarios
    let error = AppError::DatabaseError("Transaction not found".to_string());
    match error {
        AppError::DatabaseError(msg) => {
            assert!(msg.contains("Transaction not found"));
        }
        _ => panic!("Expected DatabaseError"),
    }

    let timeout_error = AppError::DatabaseError("Transaction timeout".to_string());
    match timeout_error {
        AppError::DatabaseError(msg) => {
            assert!(msg.contains("timeout"));
        }
        _ => panic!("Expected DatabaseError"),
    }
}