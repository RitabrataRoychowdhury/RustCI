use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::performance::{
    ConnectionPool, ConnectionPoolConfig, ConnectionFactory
};
use rustci::error::AppError;

#[derive(Debug, Clone)]
struct TestConnection {
    id: u32,
    is_valid: Arc<AtomicU32>, // 0 = invalid, 1 = valid
}

impl TestConnection {
    fn new(id: u32) -> Self {
        Self {
            id,
            is_valid: Arc::new(AtomicU32::new(1)),
        }
    }

    fn invalidate(&self) {
        self.is_valid.store(0, Ordering::Relaxed);
    }

    fn is_valid(&self) -> bool {
        self.is_valid.load(Ordering::Relaxed) == 1
    }
}

struct TestConnectionFactory {
    counter: Arc<AtomicU32>,
    should_fail: Arc<AtomicU32>,
}

impl TestConnectionFactory {
    fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU32::new(0)),
            should_fail: Arc::new(AtomicU32::new(0)),
        }
    }

    fn set_should_fail(&self, should_fail: bool) {
        self.should_fail.store(if should_fail { 1 } else { 0 }, Ordering::Relaxed);
    }

    fn get_created_count(&self) -> u32 {
        self.counter.load(Ordering::Relaxed)
    }
}

impl ConnectionFactory<TestConnection> for TestConnectionFactory {
    fn create_connection(&self) -> impl std::future::Future<Output = Result<TestConnection, AppError>> + Send {
        let should_fail = self.should_fail.load(Ordering::Relaxed);
        let counter = Arc::clone(&self.counter);
        
        async move {
            if should_fail == 1 {
                return Err(AppError::ConnectionCreation("Simulated failure".to_string()));
            }

            let id = counter.fetch_add(1, Ordering::Relaxed);
            Ok(TestConnection::new(id))
        }
    }

    fn validate_connection(&self, connection: &TestConnection) -> impl std::future::Future<Output = bool> + Send {
        let is_valid = connection.is_valid();
        async move { is_valid }
    }

    fn destroy_connection(&self, _connection: TestConnection) -> impl std::future::Future<Output = Result<(), AppError>> + Send {
        async move { Ok(()) }
    }
}

#[tokio::test]
async fn test_connection_pool_basic_operations() {
    let config = ConnectionPoolConfig {
        min_connections: 2,
        max_connections: 5,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    // Test getting a connection
    let guard = pool.get_connection().await.unwrap();
    assert!(guard.connection().is_valid());

    // Test stats
    let stats = pool.get_stats().await;
    assert_eq!(stats.active_connections, 1);
    assert!(stats.total_connections >= 2); // At least min_connections

    drop(guard); // Return connection to pool

    // Give some time for the connection to be returned
    sleep(Duration::from_millis(10)).await;

    let stats_after = pool.get_stats().await;
    assert_eq!(stats_after.active_connections, 0);
}

#[tokio::test]
async fn test_connection_pool_min_connections() {
    let config = ConnectionPoolConfig {
        min_connections: 3,
        max_connections: 10,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    let stats = pool.get_stats().await;
    assert!(stats.total_connections >= 3);
    assert_eq!(stats.idle_connections, 3);
}

#[tokio::test]
async fn test_connection_pool_max_connections() {
    let config = ConnectionPoolConfig {
        min_connections: 1,
        max_connections: 2,
        connection_timeout: Duration::from_millis(100),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    // Get maximum connections
    let guard1 = pool.get_connection().await.unwrap();
    let guard2 = pool.get_connection().await.unwrap();

    // Third connection should timeout
    let result = pool.get_connection().await;
    assert!(result.is_err());

    drop(guard1);
    drop(guard2);
}

#[tokio::test]
async fn test_connection_validation() {
    let config = ConnectionPoolConfig {
        min_connections: 1,
        max_connections: 5,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    // Get a connection and invalidate it
    {
        let guard = pool.get_connection().await.unwrap();
        guard.connection().invalidate();
    } // Connection returned to pool

    // Give time for connection to be returned
    sleep(Duration::from_millis(10)).await;

    // Next get should create a new connection since the old one is invalid
    let guard = pool.get_connection().await.unwrap();
    assert!(guard.connection().is_valid());
}

#[tokio::test]
async fn test_connection_lifetime_expiration() {
    let config = ConnectionPoolConfig {
        min_connections: 1,
        max_connections: 5,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_millis(50), // Very short idle timeout
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_millis(25),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    let initial_created = factory.get_created_count();

    // Get and return a connection
    {
        let _guard = pool.get_connection().await.unwrap();
    }

    // Wait for idle timeout and cleanup
    sleep(Duration::from_millis(100)).await;

    // Get another connection - should create a new one due to expiration
    {
        let _guard = pool.get_connection().await.unwrap();
    }

    // Should have created more connections due to expiration
    assert!(factory.get_created_count() > initial_created);
}

#[tokio::test]
async fn test_connection_creation_failure() {
    let config = ConnectionPoolConfig {
        min_connections: 0, // Start with no connections
        max_connections: 5,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    factory.set_should_fail(true);

    // Pool creation should succeed even if we can't create min connections
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    // Getting a connection should fail
    let result = pool.get_connection().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_connection_access() {
    let config = ConnectionPoolConfig {
        min_connections: 2,
        max_connections: 5,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    let mut handles = Vec::new();

    // Spawn multiple tasks to get connections concurrently
    for i in 0..10 {
        let pool_clone = Arc::clone(&pool);
        let handle = tokio::spawn(async move {
            let guard = pool_clone.get_connection().await.unwrap();
            sleep(Duration::from_millis(10)).await; // Hold connection briefly
            guard.connection().id // Return connection ID
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // All tasks should have succeeded
    assert_eq!(results.len(), 10);
}

#[tokio::test]
async fn test_pool_stats() {
    let config = ConnectionPoolConfig {
        min_connections: 2,
        max_connections: 5,
        connection_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        max_lifetime: Duration::from_secs(60),
        health_check_interval: Duration::from_secs(5),
    };

    let factory = TestConnectionFactory::new();
    let pool = ConnectionPool::new(config, factory).await.unwrap();

    let initial_stats = pool.get_stats().await;
    assert!(initial_stats.total_created >= 2);
    assert_eq!(initial_stats.active_connections, 0);
    assert!(initial_stats.idle_connections >= 2);

    // Get some connections
    let guard1 = pool.get_connection().await.unwrap();
    let guard2 = pool.get_connection().await.unwrap();

    let active_stats = pool.get_stats().await;
    assert_eq!(active_stats.active_connections, 2);

    drop(guard1);
    drop(guard2);

    // Give time for connections to be returned
    sleep(Duration::from_millis(10)).await;

    let final_stats = pool.get_stats().await;
    assert_eq!(final_stats.active_connections, 0);
}