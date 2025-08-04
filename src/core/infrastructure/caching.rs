use crate::error::Result;
use dashmap::DashMap;
use moka::future::Cache;
use serde::Serialize;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size: u64,
    pub hit_rate: f64,
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub ttl: Option<Duration>,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 1,
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            self.created_at.elapsed() > ttl
        } else {
            false
        }
    }

    pub fn access(&mut self) -> &T {
        self.last_accessed = Instant::now();
        self.access_count += 1;
        &self.value
    }
}

/// Multi-level cache manager
pub struct CacheManager {
    l1_cache: Cache<String, Arc<dyn std::any::Any + Send + Sync>>,
    l2_cache: Arc<DashMap<String, CacheEntry<Arc<dyn std::any::Any + Send + Sync>>>>,
    stats: Arc<RwLock<HashMap<String, CacheStats>>>,
    metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
}

impl CacheManager {
    pub fn new(
        l1_max_capacity: u64,
        l1_ttl: Duration,
        metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
    ) -> Self {
        let l1_cache = Cache::builder()
            .max_capacity(l1_max_capacity)
            .time_to_live(l1_ttl)
            .build();

        Self {
            l1_cache,
            l2_cache: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
        }
    }

    /// Get value from cache with type safety
    pub async fn get<T>(&self, cache_name: &str, key: &str) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let full_key = format!("{}:{}", cache_name, key);

        // Try L1 cache first
        if let Some(value) = self.l1_cache.get(&full_key).await {
            if let Some(typed_value) = value.downcast_ref::<T>() {
                self.record_hit(cache_name, "l1").await;
                debug!(
                    cache_name = cache_name,
                    key = key,
                    level = "l1",
                    "Cache hit"
                );
                return Some(typed_value.clone());
            }
        }

        // Try L2 cache
        if let Some(mut entry) = self.l2_cache.get_mut(&full_key) {
            if !entry.is_expired() {
                if let Some(typed_value) = entry.value.downcast_ref::<T>() {
                    let result = typed_value.clone();
                    entry.access();

                    // Promote to L1 cache
                    self.l1_cache.insert(full_key, entry.value.clone()).await;

                    self.record_hit(cache_name, "l2").await;
                    debug!(
                        cache_name = cache_name,
                        key = key,
                        level = "l2",
                        "Cache hit"
                    );
                    return Some(result);
                }
            } else {
                // Remove expired entry
                self.l2_cache.remove(&full_key);
                debug!(
                    cache_name = cache_name,
                    key = key,
                    "Removed expired cache entry"
                );
            }
        }

        self.record_miss(cache_name).await;
        debug!(cache_name = cache_name, key = key, "Cache miss");
        None
    }

    /// Set value in cache with type safety
    pub async fn set<T>(&self, cache_name: &str, key: &str, value: T, ttl: Option<Duration>)
    where
        T: Clone + Send + Sync + 'static,
    {
        let full_key = format!("{}:{}", cache_name, key);
        let arc_value: Arc<dyn std::any::Any + Send + Sync> = Arc::new(value);

        // Set in L1 cache
        self.l1_cache
            .insert(full_key.clone(), arc_value.clone())
            .await;

        // Set in L2 cache with metadata
        let entry = CacheEntry::new(arc_value, ttl);
        self.l2_cache.insert(full_key, entry);

        debug!(cache_name = cache_name, key = key, "Set cache value");
    }

    /// Remove value from cache
    pub async fn remove(&self, cache_name: &str, key: &str) {
        let full_key = format!("{}:{}", cache_name, key);

        self.l1_cache.remove(&full_key).await;
        self.l2_cache.remove(&full_key);

        debug!(cache_name = cache_name, key = key, "Removed cache value");
    }

    /// Clear all cache entries for a specific cache
    pub async fn clear_cache(&self, cache_name: &str) {
        let prefix = format!("{}:", cache_name);

        // Clear L1 cache entries with prefix
        self.l1_cache.run_pending_tasks().await;

        // Clear L2 cache entries with prefix
        let keys_to_remove: Vec<String> = self
            .l2_cache
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.l2_cache.remove(&key);
        }

        info!(cache_name = cache_name, "Cleared cache");
    }

    /// Get cache statistics
    pub async fn get_stats(&self, cache_name: &str) -> Option<CacheStats> {
        let stats = self.stats.read().await;
        stats.get(cache_name).cloned()
    }

    /// Get all cache statistics
    pub async fn get_all_stats(&self) -> HashMap<String, CacheStats> {
        self.stats.read().await.clone()
    }

    /// Record cache hit
    async fn record_hit(&self, cache_name: &str, level: &str) {
        let mut stats = self.stats.write().await;
        let cache_stats = stats
            .entry(cache_name.to_string())
            .or_insert_with(|| CacheStats {
                hits: 0,
                misses: 0,
                evictions: 0,
                size: 0,
                hit_rate: 0.0,
            });

        cache_stats.hits += 1;
        cache_stats.hit_rate =
            cache_stats.hits as f64 / (cache_stats.hits + cache_stats.misses) as f64;

        // Record metrics if collector is available
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("cache_name".to_string(), cache_name.to_string());
            labels.insert("level".to_string(), level.to_string());
            metrics.increment_counter("cache_hits_total", labels);
        }
    }

    /// Record cache miss
    async fn record_miss(&self, cache_name: &str) {
        let mut stats = self.stats.write().await;
        let cache_stats = stats
            .entry(cache_name.to_string())
            .or_insert_with(|| CacheStats {
                hits: 0,
                misses: 0,
                evictions: 0,
                size: 0,
                hit_rate: 0.0,
            });

        cache_stats.misses += 1;
        cache_stats.hit_rate =
            cache_stats.hits as f64 / (cache_stats.hits + cache_stats.misses) as f64;

        // Record metrics if collector is available
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("cache_name".to_string(), cache_name.to_string());
            metrics.increment_counter("cache_misses_total", labels);
        }
    }

    /// Clean up expired entries
    pub async fn cleanup_expired(&self) {
        let keys_to_remove: Vec<String> = self
            .l2_cache
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        let expired_count = keys_to_remove.len();

        for key in keys_to_remove {
            self.l2_cache.remove(&key);
        }

        if expired_count > 0 {
            debug!(
                expired_count = expired_count,
                "Cleaned up expired cache entries"
            );
        }
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let cache_manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;
                cache_manager.cleanup_expired().await;
            }
        })
    }
}

impl Clone for CacheManager {
    fn clone(&self) -> Self {
        Self {
            l1_cache: self.l1_cache.clone(),
            l2_cache: Arc::clone(&self.l2_cache),
            stats: Arc::clone(&self.stats),
            metrics_collector: self.metrics_collector.clone(),
        }
    }
}

/// Specialized cache for database query results
pub struct QueryCache {
    cache_manager: CacheManager,
}

impl QueryCache {
    pub fn new(metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>) -> Self {
        Self {
            cache_manager: CacheManager::new(
                1000,                     // L1 max capacity
                Duration::from_secs(300), // L1 TTL (5 minutes)
                metrics_collector,
            ),
        }
    }

    /// Cache a query result
    pub async fn cache_query<T>(&self, query_hash: &str, result: T, ttl: Duration)
    where
        T: Clone + Send + Sync + 'static,
    {
        self.cache_manager
            .set("query", query_hash, result, Some(ttl))
            .await;
    }

    /// Get cached query result
    pub async fn get_cached_query<T>(&self, query_hash: &str) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.cache_manager.get("query", query_hash).await
    }

    /// Invalidate query cache
    pub async fn invalidate_query(&self, query_hash: &str) {
        self.cache_manager.remove("query", query_hash).await;
    }

    /// Clear all query cache
    pub async fn clear_all(&self) {
        self.cache_manager.clear_cache("query").await;
    }
}

/// Specialized cache for user sessions
pub struct SessionCache {
    cache_manager: CacheManager,
}

impl SessionCache {
    pub fn new(metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>) -> Self {
        Self {
            cache_manager: CacheManager::new(
                10000,                     // L1 max capacity
                Duration::from_secs(1800), // L1 TTL (30 minutes)
                metrics_collector,
            ),
        }
    }

    /// Cache user session
    pub async fn cache_session<T>(&self, session_id: &str, session_data: T, ttl: Duration)
    where
        T: Clone + Send + Sync + 'static,
    {
        self.cache_manager
            .set("session", session_id, session_data, Some(ttl))
            .await;
    }

    /// Get cached session
    pub async fn get_session<T>(&self, session_id: &str) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.cache_manager.get("session", session_id).await
    }

    /// Remove session from cache
    pub async fn remove_session(&self, session_id: &str) {
        self.cache_manager.remove("session", session_id).await;
    }

    /// Clear all sessions
    pub async fn clear_all_sessions(&self) {
        self.cache_manager.clear_cache("session").await;
    }
}

/// Cache-aside pattern implementation
pub struct CacheAsideManager<K, V> {
    cache_manager: CacheManager,
    cache_name: String,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> CacheAsideManager<K, V>
where
    K: ToString + Hash + Eq,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(
        cache_name: String,
        metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
    ) -> Self {
        Self {
            cache_manager: CacheManager::new(
                5000,                     // L1 max capacity
                Duration::from_secs(600), // L1 TTL (10 minutes)
                metrics_collector,
            ),
            cache_name,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get or compute value using cache-aside pattern
    pub async fn get_or_compute<F, Fut>(&self, key: K, compute_fn: F, ttl: Duration) -> Result<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V>>,
    {
        let key_str = key.to_string();

        // Try to get from cache first
        if let Some(cached_value) = self
            .cache_manager
            .get::<V>(&self.cache_name, &key_str)
            .await
        {
            return Ok(cached_value);
        }

        // Compute the value
        let computed_value = compute_fn().await?;

        // Store in cache
        self.cache_manager
            .set(
                &self.cache_name,
                &key_str,
                computed_value.clone(),
                Some(ttl),
            )
            .await;

        Ok(computed_value)
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: K) {
        let key_str = key.to_string();
        self.cache_manager.remove(&self.cache_name, &key_str).await;
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Option<CacheStats> {
        self.cache_manager.get_stats(&self.cache_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_manager() {
        let cache_manager = CacheManager::new(100, Duration::from_secs(60), None);

        // Test set and get
        cache_manager
            .set("test", "key1", "value1".to_string(), None)
            .await;
        let result: Option<String> = cache_manager.get("test", "key1").await;
        assert_eq!(result, Some("value1".to_string()));

        // Test miss
        let result: Option<String> = cache_manager.get("test", "nonexistent").await;
        assert_eq!(result, None);

        // Test remove
        cache_manager.remove("test", "key1").await;
        let result: Option<String> = cache_manager.get("test", "key1").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_query_cache() {
        let query_cache = QueryCache::new(None);

        let query_hash = "select_users_123";
        let result = vec!["user1".to_string(), "user2".to_string()];

        query_cache
            .cache_query(query_hash, result.clone(), Duration::from_secs(60))
            .await;

        let cached_result: Option<Vec<String>> = query_cache.get_cached_query(query_hash).await;
        assert_eq!(cached_result, Some(result));
    }

    #[tokio::test]
    async fn test_cache_aside_manager() {
        let cache_aside = CacheAsideManager::<String, String>::new("test".to_string(), None);

        let key = "test_key".to_string();
        let expected_value = "computed_value".to_string();

        let result = cache_aside
            .get_or_compute(
                key.clone(),
                || async { Ok::<String, crate::error::AppError>(expected_value.clone()) },
                Duration::from_secs(60),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_value);

        // Second call should hit cache
        let result2 = cache_aside
            .get_or_compute(
                key,
                || async { Ok::<String, crate::error::AppError>("should_not_be_called".to_string()) },
                Duration::from_secs(60),
            )
            .await;

        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), expected_value);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache_manager = CacheManager::new(100, Duration::from_millis(50), None);

        cache_manager
            .set(
                "test",
                "key1",
                "value1".to_string(),
                Some(Duration::from_millis(100)),
            )
            .await;

        // Should be available immediately
        let result: Option<String> = cache_manager.get("test", "key1").await;
        assert_eq!(result, Some("value1".to_string()));

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired now
        let result: Option<String> = cache_manager.get("test", "key1").await;
        assert_eq!(result, None);
    }
}
