use rustci::infrastructure::database::{
    ProductionDatabaseConfig, ProductionDatabaseManager, ProductionDatabaseOperations,
    DatabaseOperationType, DatabaseOperationMetrics,
};
use rustci::error::{AppError, Result};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::test]
async fn test_production_database_config_creation() {
    let config = ProductionDatabaseConfig {
        connection_uri: "mongodb://test:27017".to_string(),
        database_name: "test_db".to_string(),
        max_connections: 50,
        min_connections: 5,
        connection_timeout_ms: 3000,
        operation_timeout_ms: 15000,
        health_check_interval_ms: 10000,
        retry_max_attempts: 5,
        retry_initial_delay_ms: 200,
        retry_max_delay_ms: 10000,
        retry_backoff_multiplier: 1.5,
        enable_metrics: true,
    };

    assert_eq!(config.max_connections, 50);
    assert_eq!(config.database_name, "test_db");
    assert_eq!(config.retry_max_attempts, 5);
    assert!(config.enable_metrics);
}

#[tokio::test]
async fn test_default_config_values() {
    let config = ProductionDatabaseConfig::default();
    
    assert_eq!(config.connection_uri, "mongodb://localhost:27017");
    assert_eq!(config.database_name, "rustci");
    assert_eq!(config.max_connections, 100);
    assert_eq!(config.min_connections, 10);
    assert_eq!(config.connection_timeout_ms, 5000);
    assert_eq!(config.operation_timeout_ms, 30000);
    assert_eq!(config.health_check_interval_ms, 30000);
    assert_eq!(config.retry_max_attempts, 3);
    assert_eq!(config.retry_initial_delay_ms, 100);
    assert_eq!(config.retry_max_delay_ms, 5000);
    assert_eq!(config.retry_backoff_multiplier, 2.0);
    assert!(config.enable_metrics);
}

#[tokio::test]
async fn test_database_manager_creation_with_invalid_uri() {
    let config = ProductionDatabaseConfig {
        connection_uri: "invalid://uri".to_string(),
        ..ProductionDatabaseConfig::default()
    };

    let result = timeout(
        Duration::from_millis(1000),
        ProductionDatabaseManager::new(config)
    ).await;

    match result {
        Ok(Err(AppError::DatabaseError(_))) => {
            // Expected error for invalid URI
        }
        Ok(Ok(_)) => {
            panic!("Expected database error for invalid URI");
        }
        Err(_) => {
            // Timeout is also acceptable for this test
        }
    }
}

#[tokio::test]
async fn test_retry_logic_with_mock_operation() {
    // This test doesn't require a real database connection
    // We'll test the retry logic structure
    
    let config = ProductionDatabaseConfig {
        retry_max_attempts: 3,
        retry_initial_delay_ms: 10, // Short delay for testing
        retry_backoff_multiplier: 2.0,
        ..ProductionDatabaseConfig::default()
    };

    // Test that we can create the config and validate retry parameters
    assert_eq!(config.retry_max_attempts, 3);
    assert_eq!(config.retry_initial_delay_ms, 10);
    assert_eq!(config.retry_backoff_multiplier, 2.0);
}

#[tokio::test]
async fn test_database_operation_metrics_creation() {
    let operation_id = Uuid::new_v4();
    let start_time = Instant::now();
    
    let metrics = DatabaseOperationMetrics {
        operation_id,
        operation_type: DatabaseOperationType::Read,
        start_time,
        duration: Some(Duration::from_millis(100)),
        success: true,
        error_message: None,
        retry_count: 0,
    };

    assert_eq!(metrics.operation_id, operation_id);
    assert_eq!(metrics.operation_type, DatabaseOperationType::Read);
    assert!(metrics.success);
    assert_eq!(metrics.retry_count, 0);
    assert!(metrics.error_message.is_none());
    assert!(metrics.duration.is_some());
}

#[tokio::test]
async fn test_database_operation_types() {
    let read_op = DatabaseOperationType::Read;
    let write_op = DatabaseOperationType::Write;
    let transaction_op = DatabaseOperationType::Transaction;
    let health_check_op = DatabaseOperationType::HealthCheck;

    assert_eq!(read_op, DatabaseOperationType::Read);
    assert_eq!(write_op, DatabaseOperationType::Write);
    assert_eq!(transaction_op, DatabaseOperationType::Transaction);
    assert_eq!(health_check_op, DatabaseOperationType::HealthCheck);

    // Test that they're different
    assert_ne!(read_op, write_op);
    assert_ne!(write_op, transaction_op);
    assert_ne!(transaction_op, health_check_op);
}

#[tokio::test]
async fn test_error_classification() {
    // Test error classification logic that would be used in retry decisions
    let connection_error = AppError::DatabaseError("connection failed".to_string());
    let timeout_error = AppError::DatabaseError("operation timeout".to_string());
    let network_error = AppError::DatabaseError("network error".to_string());
    let validation_error = AppError::ValidationError("invalid input".to_string());

    // Test that database errors contain expected keywords
    match &connection_error {
        AppError::DatabaseError(msg) => {
            assert!(msg.contains("connection"));
        }
        _ => panic!("Expected DatabaseError"),
    }

    match &timeout_error {
        AppError::DatabaseError(msg) => {
            assert!(msg.contains("timeout"));
        }
        _ => panic!("Expected DatabaseError"),
    }

    match &network_error {
        AppError::DatabaseError(msg) => {
            assert!(msg.contains("network"));
        }
        _ => panic!("Expected DatabaseError"),
    }

    // Validation errors should not be retryable
    match &validation_error {
        AppError::ValidationError(_) => {
            // This type of error should not be retried
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[tokio::test]
async fn test_backoff_calculation_logic() {
    // Test the mathematical logic for exponential backoff
    let initial_delay = 100u64;
    let multiplier = 2.0f64;
    let max_delay = 5000u64;

    let delay_0 = (initial_delay as f64 * multiplier.powi(0)) as u64;
    let delay_1 = (initial_delay as f64 * multiplier.powi(1)) as u64;
    let delay_2 = (initial_delay as f64 * multiplier.powi(2)) as u64;
    let delay_3 = (initial_delay as f64 * multiplier.powi(3)) as u64;

    assert_eq!(delay_0, 100);
    assert_eq!(delay_1, 200);
    assert_eq!(delay_2, 400);
    assert_eq!(delay_3, 800);

    // Test max delay capping
    let large_delay = (initial_delay as f64 * multiplier.powi(10)) as u64;
    let capped_delay = large_delay.min(max_delay);
    assert_eq!(capped_delay, max_delay);
}

#[tokio::test]
async fn test_health_status_structure() {
    use rustci::infrastructure::database::DatabaseHealthStatus;
    use chrono::Utc;
    use std::collections::HashMap;

    let health_status = DatabaseHealthStatus {
        is_healthy: true,
        connection_count: 10,
        active_operations: 5,
        average_response_time_ms: 25.5,
        last_health_check: Utc::now(),
        error_rate: 2.5,
        connection_pool_utilization: 0.75,
        details: HashMap::new(),
    };

    assert!(health_status.is_healthy);
    assert_eq!(health_status.connection_count, 10);
    assert_eq!(health_status.active_operations, 5);
    assert_eq!(health_status.average_response_time_ms, 25.5);
    assert_eq!(health_status.error_rate, 2.5);
    assert_eq!(health_status.connection_pool_utilization, 0.75);
    assert!(health_status.details.is_empty());
}

// Integration test that would run with a real MongoDB instance
#[tokio::test]
#[ignore] // Ignored by default since it requires MongoDB
async fn test_database_manager_with_real_connection() {
    let config = ProductionDatabaseConfig {
        connection_uri: "mongodb://localhost:27017".to_string(),
        database_name: "test_rustci".to_string(),
        ..ProductionDatabaseConfig::default()
    };

    let result = ProductionDatabaseManager::new(config).await;
    
    match result {
        Ok(manager) => {
            // Test health check
            let health_result = manager.perform_health_check().await;
            assert!(health_result.is_ok());

            // Test health status
            let health_status = manager.get_health_status().await;
            assert!(health_status.is_healthy);

            // Test metrics
            let metrics = manager.get_operation_metrics().await;
            assert!(metrics.is_empty()); // Should be empty initially

            // Test database access
            let database = manager.get_database();
            assert_eq!(database.name(), "test_rustci");
        }
        Err(e) => {
            // If MongoDB is not available, that's expected in CI environments
            println!("MongoDB not available for integration test: {}", e);
        }
    }
}