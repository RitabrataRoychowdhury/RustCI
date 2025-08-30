use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use crate::core::performance::memory_efficient::{LruTtlCache, CacheStats};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub l1_capacity: usize,
    pub l2_capacity: usize,
    pub l3_capacity: usize,
    pub default_ttl: Duration,
    pub warming_enabled: bool,
    pub warming_threshold: f64,
    pub eviction_policy: EvictionPolicy,
    pub compression_enabled: bool,
    pub metrics_enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_capacity: 1000,
            l2_capacity: 10000,
            l3_capacity: 100000,
            default_ttl: Duration::from_secs(3600), // 1 hour
            warming_enabled: true,
            warming_threshold: 0.8, // Start warming when 80% full
            eviction_policy: EvictionPolicy::LruTtl,
            compression_enabled: false,
            metrics_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    LruTtl,
    Lfu,
    Random,
    Fifo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheTier {
    L1, // In-memory, fastest
    L2, // In-memory, larger
    L3, // Persistent, largest
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheEntry<V> {
    pub value: V,
    pub tier: CacheTier,
    #[serde(skip)]
    pub created_at: Instant,
    #[serde(skip)]
    pub last_accessed: Instant,
    pub access_count: u64,
    pub size_bytes: usize,
    pub compressed: bool,
}

impl<V> CacheEntry<V> {
    pub fn new(value: V, tier: CacheTier, size_bytes: usize) -> Self {
        let now = Instant::now();
        Self {
            value,
            tier,
            created_at: now,
            last_accessed: now,
            access_count: 1,
            size_bytes,
            compressed: false,
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub fn idle_time(&self) -> Duration {
        self.last_accessed.elapsed()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetrics {
    pub l1_stats: CacheStats,
    pub l2_stats: CacheStats,
    pub l3_stats: CacheStats,
    pub total_hits: u64,
    pub total_misses: u64,
    pub total_evictions: u64,
    pub total_size_bytes: usize,
    pub warming_operations: u64,
    pub compression_ratio: f64,
}

impl CacheMetrics {
    pub fn overall_hit_rate(&self) -> f64 {
        let total_requests = self.total_hits + self.total_misses;
        if total_requests == 0 {
            0.0
        } else {
            self.total_hits as f64 / total_requests as f64
        }
    }

    pub fn memory_efficiency(&self) -> f64 {
        if self.total_size_bytes == 0 {
            0.0
        } else {
            let total_entries = self.l1_stats.size + self.l2_stats.size + self.l3_stats.size;
            total_entries as f64 / self.total_size_bytes as f64 * 1000.0 // entries per KB
        }
    }
}

pub trait CacheManager<K, V>: Send + Sync
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn get(&self, key: &K) -> impl std::future::Future<Output = Option<V>> + Send;
    fn put(&self, key: K, value: V) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn put_with_ttl(&self, key: K, value: V, ttl: Duration) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn invalidate(&self, key: &K) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn invalidate_pattern(&self, pattern: &str) -> impl std::future::Future<Output = Result<u64, AppError>> + Send;
    fn warm_cache(&self, keys: Vec<K>) -> impl std::future::Future<Output = Result<u64, AppError>> + Send;
    fn get_metrics(&self) -> impl std::future::Future<Output = CacheMetrics> + Send;
    fn clear(&self) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
}

pub struct MultiTierCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    config: CacheConfig,
    l1_cache: Arc<RwLock<LruTtlCache<K, CacheEntry<V>>>>,
    l2_cache: Arc<RwLock<LruTtlCache<K, CacheEntry<V>>>>,
    l3_cache: Arc<RwLock<LruTtlCache<K, CacheEntry<V>>>>,
    metrics: Arc<RwLock<CacheMetrics>>,
    warmer: Option<Arc<dyn CacheWarmer<K, V>>>,
}

impl<K, V> MultiTierCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(LruTtlCache::new(config.l1_capacity, config.default_ttl))),
            l2_cache: Arc::new(RwLock::new(LruTtlCache::new(config.l2_capacity, config.default_ttl))),
            l3_cache: Arc::new(RwLock::new(LruTtlCache::new(config.l3_capacity, config.default_ttl))),
            metrics: Arc::new(RwLock::new(CacheMetrics {
                l1_stats: CacheStats { hits: 0, misses: 0, hit_rate: 0.0, size: 0, capacity: config.l1_capacity },
                l2_stats: CacheStats { hits: 0, misses: 0, hit_rate: 0.0, size: 0, capacity: config.l2_capacity },
                l3_stats: CacheStats { hits: 0, misses: 0, hit_rate: 0.0, size: 0, capacity: config.l3_capacity },
                total_hits: 0,
                total_misses: 0,
                total_evictions: 0,
                total_size_bytes: 0,
                warming_operations: 0,
                compression_ratio: 1.0,
            })),
            config,
            warmer: None,
        }
    }

    pub fn with_warmer(mut self, warmer: Arc<dyn CacheWarmer<K, V>>) -> Self {
        self.warmer = Some(warmer);
        self
    }

    async fn get_from_tier(&self, key: &K, tier: CacheTier) -> Option<CacheEntry<V>> {
        match tier {
            CacheTier::L1 => {
                let mut cache = self.l1_cache.write().await;
                cache.get(key).map(|entry| {
                    let mut entry = entry.clone();
                    entry.touch();
                    entry
                })
            }
            CacheTier::L2 => {
                let mut cache = self.l2_cache.write().await;
                cache.get(key).map(|entry| {
                    let mut entry = entry.clone();
                    entry.touch();
                    entry
                })
            }
            CacheTier::L3 => {
                let mut cache = self.l3_cache.write().await;
                cache.get(key).map(|entry| {
                    let mut entry = entry.clone();
                    entry.touch();
                    entry
                })
            }
        }
    }

    async fn put_to_tier(&self, key: K, entry: CacheEntry<V>, tier: CacheTier) -> Result<(), AppError> {
        match tier {
            CacheTier::L1 => {
                let mut cache = self.l1_cache.write().await;
                cache.put(key, entry);
            }
            CacheTier::L2 => {
                let mut cache = self.l2_cache.write().await;
                cache.put(key, entry);
            }
            CacheTier::L3 => {
                let mut cache = self.l3_cache.write().await;
                cache.put(key, entry);
            }
        }
        Ok(())
    }

    async fn promote_entry(&self, key: K, entry: CacheEntry<V>) -> Result<(), AppError> {
        // Promote frequently accessed entries to higher tiers
        let target_tier = match entry.tier {
            CacheTier::L3 if entry.access_count > 10 => Some(CacheTier::L2),
            CacheTier::L2 if entry.access_count > 50 => Some(CacheTier::L1),
            _ => None,
        };

        if let Some(tier) = target_tier {
            let mut promoted_entry = entry;
            promoted_entry.tier = tier.clone();
            self.put_to_tier(key, promoted_entry, tier).await?;
        }

        Ok(())
    }

    async fn should_warm_cache(&self) -> bool {
        if !self.config.warming_enabled {
            return false;
        }

        let l1_cache = self.l1_cache.read().await;
        let utilization = l1_cache.len() as f64 / self.config.l1_capacity as f64;
        utilization > self.config.warming_threshold
    }

    async fn update_metrics(&self, hit: bool, tier: Option<CacheTier>) {
        if !self.config.metrics_enabled {
            return;
        }

        let mut metrics = self.metrics.write().await;
        
        if hit {
            metrics.total_hits += 1;
            match tier {
                Some(CacheTier::L1) => metrics.l1_stats.hits += 1,
                Some(CacheTier::L2) => metrics.l2_stats.hits += 1,
                Some(CacheTier::L3) => metrics.l3_stats.hits += 1,
                None => {}
            }
        } else {
            metrics.total_misses += 1;
            metrics.l1_stats.misses += 1; // Miss always starts at L1
        }

        // Update hit rates
        let total_l1 = metrics.l1_stats.hits + metrics.l1_stats.misses;
        if total_l1 > 0 {
            metrics.l1_stats.hit_rate = metrics.l1_stats.hits as f64 / total_l1 as f64;
        }

        let total_l2 = metrics.l2_stats.hits + metrics.l2_stats.misses;
        if total_l2 > 0 {
            metrics.l2_stats.hit_rate = metrics.l2_stats.hits as f64 / total_l2 as f64;
        }

        let total_l3 = metrics.l3_stats.hits + metrics.l3_stats.misses;
        if total_l3 > 0 {
            metrics.l3_stats.hit_rate = metrics.l3_stats.hits as f64 / total_l3 as f64;
        }
    }

    fn estimate_size(&self, _value: &V) -> usize {
        // Simple estimation - in a real implementation, you might use
        // serialization or more sophisticated size calculation
        std::mem::size_of::<V>()
    }
}

impl<K, V> CacheManager<K, V> for MultiTierCacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    async fn get(&self, key: &K) -> Option<V> {
        // Try L1 first
        if let Some(entry) = self.get_from_tier(key, CacheTier::L1).await {
            self.update_metrics(true, Some(CacheTier::L1)).await;
            return Some(entry.value);
        }

        // Try L2
        if let Some(entry) = self.get_from_tier(key, CacheTier::L2).await {
            self.update_metrics(true, Some(CacheTier::L2)).await;
            
            // Promote to L1 if frequently accessed
            if entry.access_count > 5 {
                let promoted_entry = CacheEntry::new(entry.value.clone(), CacheTier::L1, entry.size_bytes);
                let _ = self.put_to_tier(key.clone(), promoted_entry, CacheTier::L1).await;
            }
            
            return Some(entry.value);
        }

        // Try L3
        if let Some(entry) = self.get_from_tier(key, CacheTier::L3).await {
            self.update_metrics(true, Some(CacheTier::L3)).await;
            
            // Promote to L2 if frequently accessed
            if entry.access_count > 2 {
                let promoted_entry = CacheEntry::new(entry.value.clone(), CacheTier::L2, entry.size_bytes);
                let _ = self.put_to_tier(key.clone(), promoted_entry, CacheTier::L2).await;
            }
            
            return Some(entry.value);
        }

        // Cache miss
        self.update_metrics(false, None).await;
        None
    }

    async fn put(&self, key: K, value: V) -> Result<(), AppError> {
        self.put_with_ttl(key, value, self.config.default_ttl).await
    }

    async fn put_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), AppError> {
        let size_bytes = self.estimate_size(&value);
        let entry = CacheEntry::new(value, CacheTier::L1, size_bytes);
        
        // Always start with L1 for new entries
        self.put_to_tier(key, entry, CacheTier::L1).await?;
        
        // Check if we should warm the cache
        if self.should_warm_cache().await {
            if let Some(warmer) = &self.warmer {
                tokio::spawn({
                    let warmer = Arc::clone(warmer);
                    async move {
                        let _ = warmer.warm_popular_keys().await;
                    }
                });
            }
        }
        
        Ok(())
    }

    async fn invalidate(&self, key: &K) -> Result<(), AppError> {
        // Remove from all tiers
        {
            let mut cache = self.l1_cache.write().await;
            cache.remove(key);
        }
        {
            let mut cache = self.l2_cache.write().await;
            cache.remove(key);
        }
        {
            let mut cache = self.l3_cache.write().await;
            cache.remove(key);
        }
        
        Ok(())
    }

    async fn invalidate_pattern(&self, pattern: &str) -> Result<u64, AppError> {
        // This is a simplified implementation
        // In a real system, you'd want pattern matching
        let mut invalidated = 0;
        
        // For now, just clear all caches if pattern is "*"
        if pattern == "*" {
            {
                let mut cache = self.l1_cache.write().await;
                invalidated += cache.len() as u64;
                cache.clear();
            }
            {
                let mut cache = self.l2_cache.write().await;
                invalidated += cache.len() as u64;
                cache.clear();
            }
            {
                let mut cache = self.l3_cache.write().await;
                invalidated += cache.len() as u64;
                cache.clear();
            }
        }
        
        Ok(invalidated)
    }

    async fn warm_cache(&self, keys: Vec<K>) -> Result<u64, AppError> {
        let mut warmed = 0;
        
        if let Some(warmer) = &self.warmer {
            for key in keys {
                if let Some(value) = warmer.load_value(&key).await {
                    self.put(key, value).await?;
                    warmed += 1;
                }
            }
            
            let mut metrics = self.metrics.write().await;
            metrics.warming_operations += warmed;
        }
        
        Ok(warmed)
    }

    async fn get_metrics(&self) -> CacheMetrics {
        let mut metrics = self.metrics.read().await.clone();
        
        // Update current cache sizes
        {
            let l1_cache = self.l1_cache.read().await;
            metrics.l1_stats.size = l1_cache.len();
        }
        {
            let l2_cache = self.l2_cache.read().await;
            metrics.l2_stats.size = l2_cache.len();
        }
        {
            let l3_cache = self.l3_cache.read().await;
            metrics.l3_stats.size = l3_cache.len();
        }
        
        metrics
    }

    async fn clear(&self) -> Result<(), AppError> {
        {
            let mut cache = self.l1_cache.write().await;
            cache.clear();
        }
        {
            let mut cache = self.l2_cache.write().await;
            cache.clear();
        }
        {
            let mut cache = self.l3_cache.write().await;
            cache.clear();
        }
        
        // Reset metrics
        let mut metrics = self.metrics.write().await;
        *metrics = CacheMetrics {
            l1_stats: CacheStats { hits: 0, misses: 0, hit_rate: 0.0, size: 0, capacity: self.config.l1_capacity },
            l2_stats: CacheStats { hits: 0, misses: 0, hit_rate: 0.0, size: 0, capacity: self.config.l2_capacity },
            l3_stats: CacheStats { hits: 0, misses: 0, hit_rate: 0.0, size: 0, capacity: self.config.l3_capacity },
            total_hits: 0,
            total_misses: 0,
            total_evictions: 0,
            total_size_bytes: 0,
            warming_operations: 0,
            compression_ratio: 1.0,
        };
        
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait CacheWarmer<K, V>: Send + Sync
where
    K: Send + Sync,
    V: Send + Sync,
{
    async fn load_value(&self, key: &K) -> Option<V>;
    async fn warm_popular_keys(&self) -> Result<u64, AppError>;
    async fn get_popular_keys(&self) -> Vec<K>;
}

// Example implementation for string-based cache
pub struct StringCacheWarmer {
    popular_keys: Vec<String>,
}

impl StringCacheWarmer {
    pub fn new(popular_keys: Vec<String>) -> Self {
        Self { popular_keys }
    }
}

#[async_trait::async_trait]
impl CacheWarmer<String, String> for StringCacheWarmer {
    async fn load_value(&self, key: &String) -> Option<String> {
        // Simulate loading from a data source
        if self.popular_keys.contains(key) {
            Some(format!("warmed_value_for_{}", key))
        } else {
            None
        }
    }

    async fn warm_popular_keys(&self) -> Result<u64, AppError> {
        // Return the number of keys that would be warmed
        Ok(self.popular_keys.len() as u64)
    }

    async fn get_popular_keys(&self) -> Vec<String> {
        self.popular_keys.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_tier_cache_basic_operations() {
        let config = CacheConfig::default();
        let cache = MultiTierCacheManager::new(config);

        // Test put and get
        cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
        let result = cache.get(&"key1".to_string()).await;
        assert_eq!(result, Some("value1".to_string()));

        // Test miss
        let result = cache.get(&"nonexistent".to_string()).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_cache_promotion() {
        let config = CacheConfig::default();
        let cache = MultiTierCacheManager::new(config);

        // Put in L3 and access multiple times to trigger promotion
        let entry = CacheEntry::new("value1".to_string(), CacheTier::L3, 100);
        cache.put_to_tier("key1".to_string(), entry, CacheTier::L3).await.unwrap();

        // Access multiple times
        for _ in 0..10 {
            cache.get(&"key1".to_string()).await;
        }

        // Should be promoted to L2
        let l2_result = cache.get_from_tier(&"key1".to_string(), CacheTier::L2).await;
        assert!(l2_result.is_some());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let config = CacheConfig::default();
        let cache = MultiTierCacheManager::new(config);

        cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
        assert!(cache.get(&"key1".to_string()).await.is_some());

        cache.invalidate(&"key1".to_string()).await.unwrap();
        assert!(cache.get(&"key1".to_string()).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_metrics() {
        let config = CacheConfig::default();
        let cache = MultiTierCacheManager::new(config);

        cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
        cache.get(&"key1".to_string()).await; // Hit
        cache.get(&"nonexistent".to_string()).await; // Miss

        let metrics = cache.get_metrics().await;
        assert_eq!(metrics.total_hits, 1);
        assert_eq!(metrics.total_misses, 1);
        assert_eq!(metrics.overall_hit_rate(), 0.5);
    }

    #[tokio::test]
    async fn test_cache_warming() {
        let config = CacheConfig::default();
        let warmer = Arc::new(StringCacheWarmer::new(vec!["key1".to_string(), "key2".to_string()]));
        let cache = MultiTierCacheManager::new(config).with_warmer(warmer);

        let warmed = cache.warm_cache(vec!["key1".to_string(), "key2".to_string()]).await.unwrap();
        assert_eq!(warmed, 2);

        // Check that values were cached
        assert!(cache.get(&"key1".to_string()).await.is_some());
        assert!(cache.get(&"key2".to_string()).await.is_some());
    }
}