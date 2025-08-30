use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub health_check_interval: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 50,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300), // 5 minutes
            max_lifetime: Duration::from_secs(3600), // 1 hour
            health_check_interval: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PooledConnection<T> {
    pub connection: T,
    pub id: Uuid,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: u64,
}

impl<T> PooledConnection<T> {
    pub fn new(connection: T) -> Self {
        let now = Instant::now();
        Self {
            connection,
            id: Uuid::new_v4(),
            created_at: now,
            last_used: now,
            use_count: 0,
        }
    }

    pub fn is_expired(&self, config: &ConnectionPoolConfig) -> bool {
        let now = Instant::now();
        now.duration_since(self.created_at) > config.max_lifetime
            || now.duration_since(self.last_used) > config.idle_timeout
    }

    pub fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.use_count += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub total_connections: u32,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub pending_requests: u32,
    pub total_created: u64,
    pub total_destroyed: u64,
    pub average_wait_time: Duration,
    pub peak_connections: u32,
}

pub trait ConnectionFactory<T>: Send + Sync {
    fn create_connection(&self) -> impl std::future::Future<Output = Result<T, AppError>> + Send;
    fn validate_connection(&self, connection: &T) -> impl std::future::Future<Output = bool> + Send;
    fn destroy_connection(&self, connection: T) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
}

pub struct ConnectionPool<T, F> 
where
    T: Send + 'static,
    F: ConnectionFactory<T> + 'static,
{
    config: ConnectionPoolConfig,
    factory: Arc<F>,
    available: Arc<Mutex<VecDeque<PooledConnection<T>>>>,
    semaphore: Arc<Semaphore>,
    stats: Arc<Mutex<PoolStats>>,
}

impl<T, F> ConnectionPool<T, F>
where
    T: Send + 'static,
    F: ConnectionFactory<T>,
{
    pub async fn new(config: ConnectionPoolConfig, factory: F) -> Result<Arc<Self>, AppError> {
        let pool = Arc::new(Self {
            semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
            available: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(PoolStats {
                total_connections: 0,
                active_connections: 0,
                idle_connections: 0,
                pending_requests: 0,
                total_created: 0,
                total_destroyed: 0,
                average_wait_time: Duration::from_millis(0),
                peak_connections: 0,
            })),
            config,
            factory: Arc::new(factory),
        });

        // Pre-populate with minimum connections
        pool.ensure_min_connections().await?;
        
        // Start background maintenance task
        pool.clone().start_maintenance_task();

        Ok(pool)
    }

    pub async fn get_connection(self: &Arc<Self>) -> Result<ConnectionGuard<T, F>, AppError> {
        let start_time = Instant::now();
        
        // Update pending requests
        {
            let mut stats = self.stats.lock().await;
            stats.pending_requests += 1;
        }

        // Acquire semaphore permit
        let permit = timeout(self.config.connection_timeout, self.semaphore.clone().acquire_owned())
            .await
            .map_err(|_| AppError::ConnectionTimeout("Connection acquisition timeout".to_string()))?
            .map_err(|e| AppError::ConnectionPool(format!("Failed to acquire connection permit: {}", e)))?;

        // Try to get an existing connection
        let connection = {
            let mut available = self.available.lock().await;
            
            // Remove expired connections
            while let Some(conn) = available.front() {
                if conn.is_expired(&self.config) {
                    let expired = available.pop_front().unwrap();
                    tokio::spawn({
                        let factory = Arc::clone(&self.factory);
                        async move {
                            let _ = factory.destroy_connection(expired.connection).await;
                        }
                    });
                } else {
                    break;
                }
            }
            
            available.pop_front()
        };

        let pooled_connection = match connection {
            Some(mut conn) => {
                // Validate existing connection
                if self.factory.validate_connection(&conn.connection).await {
                    conn.mark_used();
                    conn
                } else {
                    // Connection is invalid, create a new one
                    let _ = self.factory.destroy_connection(conn.connection).await;
                    let new_conn = self.factory.create_connection().await?;
                    PooledConnection::new(new_conn)
                }
            }
            None => {
                // Create new connection
                let new_conn = self.factory.create_connection().await?;
                let mut stats = self.stats.lock().await;
                stats.total_created += 1;
                stats.total_connections += 1;
                stats.peak_connections = stats.peak_connections.max(stats.total_connections);
                PooledConnection::new(new_conn)
            }
        };

        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.pending_requests = stats.pending_requests.saturating_sub(1);
            stats.active_connections += 1;
            stats.idle_connections = stats.idle_connections.saturating_sub(1);
            
            let wait_time = start_time.elapsed();
            stats.average_wait_time = Duration::from_millis(
                (stats.average_wait_time.as_millis() as u64 + wait_time.as_millis() as u64) / 2
            );
        }

        Ok(ConnectionGuard {
            connection: Some(pooled_connection),
            pool: Arc::clone(self),
            permit: Some(permit),
        })
    }

    pub async fn get_stats(&self) -> PoolStats {
        self.stats.lock().await.clone()
    }

    async fn return_connection(&self, mut connection: PooledConnection<T>) {
        // Validate connection before returning to pool
        if self.factory.validate_connection(&connection.connection).await 
            && !connection.is_expired(&self.config) {
            
            connection.last_used = Instant::now();
            
            let mut available = self.available.lock().await;
            available.push_back(connection);
            
            let mut stats = self.stats.lock().await;
            stats.active_connections = stats.active_connections.saturating_sub(1);
            stats.idle_connections += 1;
        } else {
            // Connection is invalid or expired, destroy it
            let _ = self.factory.destroy_connection(connection.connection).await;
            
            let mut stats = self.stats.lock().await;
            stats.active_connections = stats.active_connections.saturating_sub(1);
            stats.total_connections = stats.total_connections.saturating_sub(1);
            stats.total_destroyed += 1;
        }
    }

    async fn ensure_min_connections(&self) -> Result<(), AppError> {
        let current_count = {
            let available = self.available.lock().await;
            available.len() as u32
        };

        if current_count < self.config.min_connections {
            let needed = self.config.min_connections - current_count;
            
            for _ in 0..needed {
                let connection = self.factory.create_connection().await?;
                let pooled = PooledConnection::new(connection);
                
                let mut available = self.available.lock().await;
                available.push_back(pooled);
                
                let mut stats = self.stats.lock().await;
                stats.total_connections += 1;
                stats.idle_connections += 1;
                stats.total_created += 1;
            }
        }

        Ok(())
    }

    async fn cleanup_expired_connections(&self) {
        let mut available = self.available.lock().await;
        let mut expired_connections = Vec::new();
        
        // Collect expired connections
        while let Some(conn) = available.front() {
            if conn.is_expired(&self.config) {
                expired_connections.push(available.pop_front().unwrap());
            } else {
                break;
            }
        }
        
        drop(available);

        // Destroy expired connections
        for conn in expired_connections {
            let _ = self.factory.destroy_connection(conn.connection).await;
            
            let mut stats = self.stats.lock().await;
            stats.total_connections = stats.total_connections.saturating_sub(1);
            stats.idle_connections = stats.idle_connections.saturating_sub(1);
            stats.total_destroyed += 1;
        }
    }

    fn start_maintenance_task(self: Arc<Self>) {
        let pool_weak = Arc::downgrade(&self);
        let interval = self.config.health_check_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                if let Some(pool) = pool_weak.upgrade() {
                    pool.cleanup_expired_connections().await;
                    let _ = pool.ensure_min_connections().await;
                } else {
                    break;
                }
            }
        });
    }
}

pub struct ConnectionGuard<T, F>
where
    T: Send + 'static,
    F: ConnectionFactory<T> + 'static,
{
    connection: Option<PooledConnection<T>>,
    pool: Arc<ConnectionPool<T, F>>,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl<T, F> ConnectionGuard<T, F>
where
    T: Send + 'static,
    F: ConnectionFactory<T> + 'static,
{
    pub fn connection(&self) -> &T {
        &self.connection.as_ref().unwrap().connection
    }

    pub fn connection_mut(&mut self) -> &mut T {
        &mut self.connection.as_mut().unwrap().connection
    }
}

impl<T, F> Drop for ConnectionGuard<T, F>
where
    T: Send + 'static,
    F: ConnectionFactory<T> + 'static,
{
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let pool = Arc::clone(&self.pool);
            tokio::spawn(async move {
                pool.return_connection(connection).await;
            });
        }
    }
}

// Example HTTP connection factory
#[derive(Debug)]
pub struct HttpConnection {
    pub client: reqwest::Client,
    pub base_url: String,
}

pub struct HttpConnectionFactory {
    pub base_url: String,
    pub timeout: Duration,
}

impl ConnectionFactory<HttpConnection> for HttpConnectionFactory {
    fn create_connection(&self) -> impl std::future::Future<Output = Result<HttpConnection, AppError>> + Send {
        let base_url = self.base_url.clone();
        let timeout = self.timeout;
        
        async move {
            let client = reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| AppError::ConnectionCreation(format!("Failed to create HTTP client: {}", e)))?;

            Ok(HttpConnection {
                client,
                base_url,
            })
        }
    }

    fn validate_connection(&self, connection: &HttpConnection) -> impl std::future::Future<Output = bool> + Send {
        let base_url = connection.base_url.clone();
        let client = connection.client.clone();
        
        async move {
            // Simple health check - try to make a HEAD request
            client
                .head(&base_url)
                .send()
                .await
                .map(|resp| resp.status().is_success())
                .unwrap_or(false)
        }
    }

    fn destroy_connection(&self, _connection: HttpConnection) -> impl std::future::Future<Output = Result<(), AppError>> + Send {
        async move {
            // HTTP connections don't need explicit cleanup
            Ok(())
        }
    }
}