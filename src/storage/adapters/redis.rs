use async_trait::async_trait;
use deadpool_redis::{Config, Pool, Runtime};
use redis::{AsyncCommands, RedisResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::storage::{
    HealthStatus, RedisConfig, StoreAdapter, StoreError, StoreFeature,
    StoreResult, StoreStats,
};

/// Redis storage adapter with connection pooling and retry logic
pub struct RedisAdapter {
    pool: Pool,
    config: RedisConfig,
    stats: Arc<tokio::sync::RwLock<StoreStats>>,
}

impl RedisAdapter {
    /// Create a new Redis adapter
    pub async fn new(config: RedisConfig) -> StoreResult<Self> {
        let mut pool_config = Config::from_url(&config.url);
        if pool_config.is_err() {
            return Err(StoreError::ConfigurationError(format!("Invalid Redis URL: {}", config.url)));
        }
        let pool_config = pool_config.unwrap();

        let pool = pool_config
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| StoreError::ConnectionError(format!("Failed to create Redis pool: {}", e)))?;

        // Test the connection
        let mut conn = pool
            .get()
            .await
            .map_err(|e| StoreError::ConnectionError(format!("Failed to get Redis connection: {}", e)))?;

        // Ping to verify connection
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| StoreError::ConnectionError(format!("Redis ping failed: {}", e)))?;

        info!("Redis adapter initialized successfully");

        Ok(Self {
            pool,
            config,
            stats: Arc::new(tokio::sync::RwLock::new(StoreStats::default())),
        })
    }

    /// Execute a Redis operation with retry logic
    async fn execute_with_retry<F, T>(&self, operation: F) -> StoreResult<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = RedisResult<T>> + Send>> + Send,
        T: Send,
    {
        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            let start_time = Instant::now();
            
            match timeout(Duration::from_millis(self.config.command_timeout_ms), operation()).await {
                Ok(Ok(result)) => {
                    self.record_success(start_time.elapsed()).await;
                    return Ok(result);
                }
                Ok(Err(e)) => {
                    let error_msg = format!("Redis error: {}", e);
                    last_error = Some(e);
                    self.record_error(&error_msg).await;
                }
                Err(_) => {
                    let timeout_error = format!("Redis operation timeout after {:?}", Duration::from_millis(self.config.command_timeout_ms));
                    self.record_error(&timeout_error).await;
                    return Err(StoreError::Timeout(Duration::from_millis(self.config.command_timeout_ms)));
                }
            }

            if attempt < self.config.max_retries {
                let delay = self.calculate_retry_delay(attempt);
                debug!("Redis operation failed, retrying in {:?} (attempt {}/{})", delay, attempt + 1, self.config.max_retries);
                tokio::time::sleep(delay).await;
            }
        }

        Err(StoreError::BackendUnavailable(format!(
            "Redis operation failed after {} retries: {}",
            self.config.max_retries,
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    /// Calculate retry delay with exponential backoff and jitter
    fn calculate_retry_delay(&self, attempt: u32) -> Duration {
        let base_delay = Duration::from_millis(100);
        let delay = base_delay * 2_u32.pow(attempt);
        let max_delay = Duration::from_millis(5000);
        
        // Add jitter to prevent thundering herd
        let jitter = fastrand::f64() * 0.1;
        let jittered_delay = delay.mul_f64(1.0 + jitter);
        
        jittered_delay.min(max_delay)
    }

    /// Record successful operation
    async fn record_success(&self, duration: Duration) {
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.successful_operations += 1;
        
        // Update average latency with exponential moving average
        let latency_ms = duration.as_millis() as f64;
        if stats.average_latency_ms == 0.0 {
            stats.average_latency_ms = latency_ms;
        } else {
            stats.average_latency_ms = stats.average_latency_ms * 0.9 + latency_ms * 0.1;
        }
        
        stats.last_updated = std::time::SystemTime::now();
    }

    /// Record failed operation
    async fn record_error(&self, error: &str) {
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.failed_operations += 1;
        stats.last_updated = std::time::SystemTime::now();
        
        warn!("Redis operation failed: {}", error);
    }

    /// Get a connection from the pool
    async fn get_connection(&self) -> StoreResult<deadpool_redis::Connection> {
        self.pool
            .get()
            .await
            .map_err(|e| StoreError::ConnectionError(format!("Failed to get Redis connection: {}", e)))
    }
}

#[async_trait]
impl StoreAdapter for RedisAdapter {
    async fn get(&self, key: &str) -> StoreResult<Option<Vec<u8>>> {
        self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                conn.get::<_, Option<Vec<u8>>>(key).await
            })
        })
        .await
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> StoreResult<()> {
        self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            let value = value.clone();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                match ttl {
                    Some(ttl) => {
                        conn.set_ex::<_, _, ()>(key, value, ttl.as_secs()).await
                    }
                    None => {
                        conn.set::<_, _, ()>(key, value).await
                    }
                }
            })
        })
        .await
    }

    async fn try_consume(&self, key: &str, amount: u32) -> StoreResult<bool> {
        // Use Lua script for atomic token bucket operation
        let script = r#"
            local key = KEYS[1]
            local amount = tonumber(ARGV[1])
            local capacity = tonumber(ARGV[2])
            local refill_rate = tonumber(ARGV[3])
            local now = tonumber(ARGV[4])
            
            local bucket = redis.call('HMGET', key, 'tokens', 'last_refill')
            local tokens = tonumber(bucket[1]) or capacity
            local last_refill = tonumber(bucket[2]) or now
            
            -- Calculate tokens to add based on time elapsed
            local elapsed = math.max(0, now - last_refill)
            local tokens_to_add = math.floor(elapsed * refill_rate / 1000)
            tokens = math.min(capacity, tokens + tokens_to_add)
            
            if tokens >= amount then
                tokens = tokens - amount
                redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
                redis.call('EXPIRE', key, 3600) -- 1 hour TTL
                return 1
            else
                redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
                redis.call('EXPIRE', key, 3600)
                return 0
            end
        "#;

        let capacity = 100; // Default capacity
        let refill_rate = 10; // Default refill rate (tokens per second)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let result: i32 = self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            let script = script.to_string();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                redis::Script::new(&script)
                    .key(key)
                    .arg(amount)
                    .arg(capacity)
                    .arg(refill_rate)
                    .arg(now)
                    .invoke_async(&mut conn)
                    .await
            })
        })
        .await?;

        Ok(result == 1)
    }

    async fn delete(&self, key: &str) -> StoreResult<bool> {
        let result: i32 = self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                conn.del(key).await
            })
        })
        .await?;

        Ok(result > 0)
    }

    async fn exists(&self, key: &str) -> StoreResult<bool> {
        self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                conn.exists(key).await
            })
        })
        .await
    }

    async fn ttl(&self, key: &str) -> StoreResult<Option<Duration>> {
        let ttl_seconds: i64 = self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                conn.ttl(key).await
            })
        })
        .await?;

        match ttl_seconds {
            -2 => Ok(None), // Key doesn't exist
            -1 => Ok(None), // Key exists but has no TTL
            seconds if seconds > 0 => Ok(Some(Duration::from_secs(seconds as u64))),
            _ => Ok(None),
        }
    }

    async fn increment(&self, key: &str, amount: i64, ttl: Option<Duration>) -> StoreResult<i64> {
        let result = self.execute_with_retry(|| {
            let pool = self.pool.clone();
            let key = key.to_string();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                let result: i64 = conn.incr(key.clone(), amount).await?;
                
                if let Some(ttl) = ttl {
                    let _: () = conn.expire(key, ttl.as_secs() as i64).await?;
                }
                
                Ok(result)
            })
        })
        .await?;

        Ok(result)
    }

    async fn health_check(&self) -> StoreResult<HealthStatus> {
        match timeout(Duration::from_millis(self.config.timeout_ms), async {
            let mut conn = self.get_connection().await?;
            let _: String = redis::cmd("PING")
                .query_async(&mut conn)
                .await
                .map_err(|e| StoreError::ConnectionError(format!("Redis ping failed: {}", e)))?;
            Ok::<(), StoreError>(())
        }).await {
            Ok(Ok(())) => Ok(HealthStatus::Healthy),
            Ok(Err(_)) => Ok(HealthStatus::Unhealthy),
            Err(_) => Ok(HealthStatus::Degraded), // Timeout
        }
    }

    async fn get_stats(&self) -> StoreResult<StoreStats> {
        let mut stats = self.stats.read().await.clone();
        
        // Add Redis-specific stats
        if let Ok(mut conn) = self.get_connection().await {
            if let Ok(info) = redis::cmd("INFO")
                .arg("memory")
                .query_async::<_, String>(&mut conn)
                .await {
                // Parse memory info from Redis INFO command
                for line in info.lines() {
                    if line.starts_with("used_memory:") {
                        if let Some(memory_str) = line.split(':').nth(1) {
                            if let Ok(memory) = memory_str.parse::<u64>() {
                                stats.memory_usage_bytes = memory;
                            }
                        }
                    }
                }
            }
        }
        
        stats.connections_active = self.pool.status().size as u32;
        stats.connections_idle = (self.pool.status().max_size - self.pool.status().size) as u32;
        
        Ok(stats)
    }

    fn backend_name(&self) -> &'static str {
        "redis"
    }

    fn supports_feature(&self, feature: StoreFeature) -> bool {
        match feature {
            StoreFeature::Ttl => true,
            StoreFeature::AtomicOperations => true,
            StoreFeature::Transactions => true,
            StoreFeature::PubSub => true,
            StoreFeature::Scripting => true,
            StoreFeature::Clustering => self.config.cluster_mode,
            StoreFeature::Persistence => true,
            StoreFeature::Replication => true,
            StoreFeature::Compression => false,
            StoreFeature::Encryption => self.config.tls,
        }
    }

    async fn flush_all(&self) -> StoreResult<()> {
        self.execute_with_retry(|| {
            let pool = self.pool.clone();
            Box::pin(async move {
                let mut conn = pool.get().await.map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Pool error", e.to_string())))?;
                redis::cmd("FLUSHALL").query_async(&mut conn).await
            })
        })
        .await
    }
}