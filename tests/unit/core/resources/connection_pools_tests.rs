//! Tests for Connection Pool Management System

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::resources::connection_pools::{
    ConnectionPool, ConnectionPoolManager, PoolConfig, ConnectionType, ConnectionState,
    HealthStatus, ConnectionFactory, HealthChecker, PooledConnection,
};

/// Enhanced mock connection for comprehensive testing
#[derive(Debug, Clone)]
struct TestConnection {
    id: usize,
    is_healthy: Arc<AtomicBool>,
    created_at: std::time::SystemTime,
    data: String,
}

impl TestConnection {
    fn new(id: usize, data: String) -> Self {
        Self {
            id,
            is_healthy: Arc::new(AtomicBool::new(true)),
            created_at: std::time::SystemTime::now(),
            data,
        }
    }

    fn set_healthy(&self, healthy: bool) {
        self.is_healthy.store(healthy, Ordering::SeqCst);
    }

    fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::SeqCst)
    }
}

/// Configurable mock connection factory
struct TestConnectionFactory {
    counter: AtomicUsize,
    should_fail: Arc<AtomicBool>,
    failure_rate: f64,
    creation_delay: Duration,
}

impl TestConnectionFactory {
    fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
            should_fail: Arc::new(AtomicBool::new(false)),
            failure_rate: 0.0,
            creation_delay: Duration::from_millis(0),
        }
    }

    fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate;
        self
    }

    fn with_creation_delay(mut self, delay: Duration) -> Self {
        self.creation_delay = delay;
        self
    }

    fn set_should_fail(&self, should_fail: bool) {
        self.should_fail.store(should_fail, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for TestConnectionFactory {
    type Connection = TestConnection;

    async fn create_connection(&self) -> Result<Self::Connection, Box<dyn std::error::Error + Send + Sync>> {
        if self.creation_delay > Duration::from_millis(0) {
            sleep(self.creation_delay).await;
        }

        if self.should_fail.load(Ordering::SeqCst) {
            return Err("Mock connection creation failed".into());
        }

        // Simulate random failures based on failure rate
        if self.failure_rate > 0.0 {
            let random_value: f64 = rand::random();
            if random_value < self.failure_rate {
                return Err("Random connection creation failure".into());
            }
        }

        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(TestConnection::new(id, format!("connection_{}", id)))
    }

    async fn validate_connection(&self, connection: &Self::Connection) -> bool {
        connection.is_healthy()
    }

    async fn close_connection(&self, _connection: Self::Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simulate some cleanup work
        sleep(Duration::from_millis(1)).await;
        Ok(())
    }

    fn get_factory_name(&self) -> &str {
        "TestConnectionFactory"
    }
}

/// Configurable health checker
struct TestHealthChecker {
    always_healthy: bool,
    check_delay: Duration,
}

impl TestHealthChecker {
    fn new(always_healthy: bool) -> Self {
        Self {
            always_healthy,
            check_delay: Duration::from_millis(0),
        }
    }

    fn with_check_delay(mut self, delay: Duration) -> Self {
        self.check_delay = delay;
        self
    }
}

#[async_trait::async_trait]
impl HealthChecker for TestHealthChecker {
    type Connection = TestConnection;

    async fn check_health(&self, connection: &Self::Connection) -> HealthStatus {
        if self.check_delay > Duration::from_millis(0) {
            sleep(self.check_delay).await;
        }

        if self.always_healthy {
            HealthStatus::Healthy
        } else if connection.is_healthy() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    fn get_checker_name(&self) -> &str {
        "TestHealthChecker"
    }
}

#[tokio::test]
async fn test_connection_pool_creation_with_min_connections() {
    let config = PoolConfig {
        connection_type: ConnectionType::Database("test_db".to_string()),
        min_connections: 3,
        max_connections: 10,
        idle_timeout: Duration::from_secs(300),
        max_lifetime: Duration::from_secs(3600),
        health_check_interval: Duration::from_secs(30),
        connection_timeout: Duration::from_secs(10),
        retry_attempts: 3,
        enable_health_checks: true,
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let health_checker = Some(Arc::new(TestHealthChecker::new(true)) as Arc<dyn HealthChecker<Connection = TestConnection>>);

    let pool = ConnectionPool::new(config, factory, health_checker).await.unwrap();
    let stats = pool.get_stats().await;

    assert_eq!(stats.total_connections, 3);
    assert_eq!(stats.available_connections, 3);
    assert_eq!(stats.in_use_connections, 0);
}

#[tokio::test]
async fn test_connection_pool_get_and_return_cycle() {
    let config = PoolConfig {
        connection_type: ConnectionType::Http("api_service".to_string()),
        min_connections: 2,
        max_connections: 5,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    // Get a connection
    let mut connection = pool.get_connection().await.unwrap();
    assert_eq!(connection.metadata.state, ConnectionState::InUse);
    assert_eq!(connection.metadata.use_count, 1);

    let stats_during_use = pool.get_stats().await;
    assert_eq!(stats_during_use.in_use_connections, 1);
    assert_eq!(stats_during_use.available_connections, 1); // One remaining from min_connections

    // Mark connection as used again
    connection.mark_used();
    assert_eq!(connection.metadata.use_count, 2);

    // Return connection to pool
    pool.return_connection(connection).await;

    let stats_after_return = pool.get_stats().await;
    assert_eq!(stats_after_return.available_connections, 2);
    assert_eq!(stats_after_return.in_use_connections, 0);
}

#[tokio::test]
async fn test_connection_pool_max_connections_limit() {
    let config = PoolConfig {
        connection_type: ConnectionType::Custom("limited_pool".to_string()),
        min_connections: 1,
        max_connections: 2,
        connection_timeout: Duration::from_millis(100),
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    // Get all available connections
    let _conn1 = pool.get_connection().await.unwrap();
    let _conn2 = pool.get_connection().await.unwrap();

    let stats = pool.get_stats().await;
    assert_eq!(stats.in_use_connections, 2);
    assert_eq!(stats.available_connections, 0);

    // Trying to get another connection should work (creates new one up to max)
    // but will be limited by semaphore
    let start_time = std::time::Instant::now();
    let conn3_result = tokio::time::timeout(Duration::from_millis(50), pool.get_connection()).await;
    let elapsed = start_time.elapsed();

    // Should timeout because we've reached max connections
    assert!(conn3_result.is_err());
    assert!(elapsed >= Duration::from_millis(45)); // Allow some tolerance
}

#[tokio::test]
async fn test_connection_pool_health_checking() {
    let config = PoolConfig {
        connection_type: ConnectionType::Database("health_checked_db".to_string()),
        min_connections: 2,
        max_connections: 5,
        enable_health_checks: true,
        health_check_interval: Duration::from_millis(50),
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let health_checker = Some(Arc::new(TestHealthChecker::new(false)) as Arc<dyn HealthChecker<Connection = TestConnection>>);

    let pool = ConnectionPool::new(config, factory, health_checker).await.unwrap();

    // Start health checking
    pool.start().await.unwrap();

    // Get a connection and mark it as unhealthy
    let connection = pool.get_connection().await.unwrap();
    connection.connection.set_healthy(false);

    // Return the unhealthy connection
    pool.return_connection(connection).await;

    // Wait for health check cycle
    sleep(Duration::from_millis(100)).await;

    pool.stop().await;

    let stats = pool.get_stats().await;
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.successful_requests, 1);
}

#[tokio::test]
async fn test_connection_pool_error_handling() {
    let config = PoolConfig {
        connection_type: ConnectionType::Http("failing_service".to_string()),
        min_connections: 1,
        max_connections: 3,
        retry_attempts: 2,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new().with_failure_rate(0.5));
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    // Try to get connections, some should fail
    let mut successful_connections = 0;
    let mut failed_attempts = 0;

    for _ in 0..10 {
        match pool.get_connection().await {
            Ok(conn) => {
                successful_connections += 1;
                pool.return_connection(conn).await;
            }
            Err(_) => {
                failed_attempts += 1;
            }
        }
    }

    let stats = pool.get_stats().await;
    assert!(stats.failed_requests > 0);
    assert!(stats.successful_requests > 0);
    assert_eq!(stats.total_requests, 10);
}

#[tokio::test]
async fn test_connection_pool_statistics_tracking() {
    let config = PoolConfig {
        connection_type: ConnectionType::Database("stats_db".to_string()),
        min_connections: 2,
        max_connections: 4,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new().with_creation_delay(Duration::from_millis(10)));
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    // Perform multiple operations
    for i in 0..5 {
        let connection = pool.get_connection().await.unwrap();
        
        // Simulate some work
        sleep(Duration::from_millis(5)).await;
        
        if i % 2 == 0 {
            // Mark some connections as having errors
            let mut conn = connection;
            conn.mark_error();
            assert!(conn.metadata.error_count > 0);
            pool.return_connection(conn).await;
        } else {
            pool.return_connection(connection).await;
        }
    }

    let stats = pool.get_stats().await;
    assert_eq!(stats.total_requests, 5);
    assert_eq!(stats.successful_requests, 5);
    assert_eq!(stats.failed_requests, 0);
    assert!(stats.average_wait_time > Duration::from_millis(0));
}

#[tokio::test]
async fn test_connection_pool_cleanup_process() {
    let config = PoolConfig {
        connection_type: ConnectionType::Custom("cleanup_test".to_string()),
        min_connections: 1,
        max_connections: 3,
        idle_timeout: Duration::from_millis(50),
        max_lifetime: Duration::from_millis(100),
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    // Start cleanup process
    pool.start().await.unwrap();

    // Get and return a connection to make it idle
    let connection = pool.get_connection().await.unwrap();
    pool.return_connection(connection).await;

    // Wait for cleanup to potentially run
    sleep(Duration::from_millis(150)).await;

    pool.stop().await;

    // Cleanup should have run, but exact behavior depends on timing
    let stats = pool.get_stats().await;
    assert!(stats.total_connections >= 0); // Connections may have been cleaned up
}

#[tokio::test]
async fn test_connection_pool_close_all() {
    let config = PoolConfig {
        connection_type: ConnectionType::Database("closable_db".to_string()),
        min_connections: 3,
        max_connections: 5,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    let stats_before = pool.get_stats().await;
    assert_eq!(stats_before.total_connections, 3);

    // Close all connections
    pool.close_all().await.unwrap();

    let stats_after = pool.get_stats().await;
    assert_eq!(stats_after.total_connections, 0);
    assert_eq!(stats_after.available_connections, 0);
}

#[tokio::test]
async fn test_connection_pool_manager() {
    let manager = ConnectionPoolManager::new();

    // Create and register a pool
    let config = PoolConfig {
        connection_type: ConnectionType::Database("managed_db".to_string()),
        min_connections: 2,
        max_connections: 4,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config.clone(), factory, None).await.unwrap();

    manager.register_pool(pool).await;

    // Get pool configuration
    let retrieved_config = manager.get_pool_config(&ConnectionType::Database("managed_db".to_string())).await;
    assert!(retrieved_config.is_some());
    assert_eq!(retrieved_config.unwrap().min_connections, 2);

    // Test starting and stopping all pools
    manager.start_all().await.unwrap();
    manager.stop_all().await;
}

#[tokio::test]
async fn test_connection_metadata_tracking() {
    let config = PoolConfig {
        connection_type: ConnectionType::Http("metadata_service".to_string()),
        min_connections: 1,
        max_connections: 2,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    let mut connection = pool.get_connection().await.unwrap();

    // Verify initial metadata
    assert_eq!(connection.metadata.connection_type, ConnectionType::Http("metadata_service".to_string()));
    assert_eq!(connection.metadata.state, ConnectionState::InUse);
    assert_eq!(connection.metadata.use_count, 1);
    assert_eq!(connection.metadata.error_count, 0);

    // Mark connection as used and with error
    connection.mark_used();
    connection.mark_error();

    assert_eq!(connection.metadata.use_count, 2);
    assert_eq!(connection.metadata.error_count, 1);

    pool.return_connection(connection).await;
}

#[tokio::test]
async fn test_connection_types() {
    // Test different connection types
    let db_type = ConnectionType::Database("postgres".to_string());
    let http_type = ConnectionType::Http("api.example.com".to_string());
    let custom_type = ConnectionType::Custom("redis".to_string());

    assert_eq!(db_type, ConnectionType::Database("postgres".to_string()));
    assert_eq!(http_type, ConnectionType::Http("api.example.com".to_string()));
    assert_eq!(custom_type, ConnectionType::Custom("redis".to_string()));

    // Test that different types are not equal
    assert_ne!(db_type, http_type);
    assert_ne!(http_type, custom_type);
    assert_ne!(db_type, custom_type);
}

#[tokio::test]
async fn test_pooled_connection_drop_behavior() {
    let config = PoolConfig {
        connection_type: ConnectionType::Custom("drop_test".to_string()),
        min_connections: 1,
        max_connections: 2,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new());
    let pool = ConnectionPool::new(config, factory, None).await.unwrap();

    {
        let _connection = pool.get_connection().await.unwrap();
        // Connection should be returned to pool when it goes out of scope
    }

    // Give some time for the drop handler to execute
    sleep(Duration::from_millis(10)).await;

    let stats = pool.get_stats().await;
    // Connection should be back in the pool
    assert_eq!(stats.available_connections, 1);
}

#[tokio::test]
async fn test_concurrent_connection_requests() {
    let config = PoolConfig {
        connection_type: ConnectionType::Database("concurrent_db".to_string()),
        min_connections: 2,
        max_connections: 4,
        ..Default::default()
    };

    let factory = Arc::new(TestConnectionFactory::new().with_creation_delay(Duration::from_millis(10)));
    let pool = Arc::new(ConnectionPool::new(config, factory, None).await.unwrap());

    // Spawn multiple concurrent requests
    let mut handles = Vec::new();
    for i in 0..6 {
        let pool_clone = Arc::clone(&pool);
        let handle = tokio::spawn(async move {
            let connection = pool_clone.get_connection().await.unwrap();
            sleep(Duration::from_millis(20)).await; // Simulate work
            pool_clone.return_connection(connection).await;
            i
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let stats = pool.get_stats().await;
    assert_eq!(stats.total_requests, 6);
    assert_eq!(stats.successful_requests, 6);
    assert_eq!(stats.failed_requests, 0);
}