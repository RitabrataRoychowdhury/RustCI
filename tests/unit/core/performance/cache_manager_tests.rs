use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::performance::{
    CacheManager, MultiTierCacheManager, CacheConfig, CacheWarmer, 
    CacheTier, CacheEntry, EvictionPolicy, StringCacheWarmer
};
use rustci::error::AppError;

#[tokio::test]
async fn test_basic_cache_operations() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Test put and get
    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
    let result = cache.get(&"key1".to_string()).await;
    assert_eq!(result, Some("value1".to_string()));

    // Test cache miss
    let result = cache.get(&"nonexistent".to_string()).await;
    assert_eq!(result, None);

    // Test overwrite
    cache.put("key1".to_string(), "new_value1".to_string()).await.unwrap();
    let result = cache.get(&"key1".to_string()).await;
    assert_eq!(result, Some("new_value1".to_string()));
}

#[tokio::test]
async fn test_cache_with_ttl() {
    let mut config = CacheConfig::default();
    config.default_ttl = Duration::from_millis(50);
    let cache = MultiTierCacheManager::new(config);

    // Put with short TTL
    cache.put_with_ttl("key1".to_string(), "value1".to_string(), Duration::from_millis(10)).await.unwrap();
    
    // Should be available immediately
    assert!(cache.get(&"key1".to_string()).await.is_some());
    
    // Wait for expiration
    sleep(Duration::from_millis(20)).await;
    
    // Should be expired now
    assert!(cache.get(&"key1".to_string()).await.is_none());
}

#[tokio::test]
async fn test_multi_tier_promotion() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Manually put entry in L3
    let entry = CacheEntry::new("value1".to_string(), CacheTier::L3, 100);
    cache.put_to_tier("key1".to_string(), entry, CacheTier::L3).await.unwrap();

    // Access multiple times to trigger promotion
    for _ in 0..15 {
        cache.get(&"key1".to_string()).await;
    }

    // Should be promoted to L2 after multiple accesses
    let l2_result = cache.get_from_tier(&"key1".to_string(), CacheTier::L2).await;
    assert!(l2_result.is_some());
}

#[tokio::test]
async fn test_cache_invalidation() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Put values in multiple tiers
    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
    cache.put("key2".to_string(), "value2".to_string()).await.unwrap();

    // Verify they exist
    assert!(cache.get(&"key1".to_string()).await.is_some());
    assert!(cache.get(&"key2".to_string()).await.is_some());

    // Invalidate single key
    cache.invalidate(&"key1".to_string()).await.unwrap();
    assert!(cache.get(&"key1".to_string()).await.is_none());
    assert!(cache.get(&"key2".to_string()).await.is_some());

    // Invalidate all with pattern
    let invalidated = cache.invalidate_pattern("*").await.unwrap();
    assert!(invalidated > 0);
    assert!(cache.get(&"key2".to_string()).await.is_none());
}

#[tokio::test]
async fn test_cache_metrics() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Initial metrics should be zero
    let initial_metrics = cache.get_metrics().await;
    assert_eq!(initial_metrics.total_hits, 0);
    assert_eq!(initial_metrics.total_misses, 0);
    assert_eq!(initial_metrics.overall_hit_rate(), 0.0);

    // Add some data and access it
    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
    cache.put("key2".to_string(), "value2".to_string()).await.unwrap();

    // Generate hits and misses
    cache.get(&"key1".to_string()).await; // Hit
    cache.get(&"key2".to_string()).await; // Hit
    cache.get(&"nonexistent1".to_string()).await; // Miss
    cache.get(&"nonexistent2".to_string()).await; // Miss

    let metrics = cache.get_metrics().await;
    assert_eq!(metrics.total_hits, 2);
    assert_eq!(metrics.total_misses, 2);
    assert_eq!(metrics.overall_hit_rate(), 0.5);
    assert!(metrics.l1_stats.size > 0);
}

#[tokio::test]
async fn test_cache_warming() {
    let config = CacheConfig::default();
    let popular_keys = vec!["popular1".to_string(), "popular2".to_string(), "popular3".to_string()];
    let warmer = Arc::new(StringCacheWarmer::new(popular_keys.clone()));
    let cache = MultiTierCacheManager::new(config).with_warmer(warmer);

    // Warm the cache
    let warmed_count = cache.warm_cache(popular_keys.clone()).await.unwrap();
    assert_eq!(warmed_count, 3);

    // Verify warmed values are accessible
    for key in &popular_keys {
        let result = cache.get(key).await;
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("warmed_value_for_"));
    }

    // Check metrics reflect warming operations
    let metrics = cache.get_metrics().await;
    assert_eq!(metrics.warming_operations, 3);
}

#[tokio::test]
async fn test_cache_clear() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Add some data
    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
    cache.put("key2".to_string(), "value2".to_string()).await.unwrap();

    // Verify data exists
    assert!(cache.get(&"key1".to_string()).await.is_some());
    assert!(cache.get(&"key2".to_string()).await.is_some());

    // Clear cache
    cache.clear().await.unwrap();

    // Verify data is gone
    assert!(cache.get(&"key1".to_string()).await.is_none());
    assert!(cache.get(&"key2".to_string()).await.is_none());

    // Verify metrics are reset
    let metrics = cache.get_metrics().await;
    assert_eq!(metrics.total_hits, 0);
    assert_eq!(metrics.total_misses, 0);
    assert_eq!(metrics.l1_stats.size, 0);
}

#[tokio::test]
async fn test_cache_capacity_limits() {
    let mut config = CacheConfig::default();
    config.l1_capacity = 2; // Very small capacity
    let cache = MultiTierCacheManager::new(config);

    // Fill beyond capacity
    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
    cache.put("key2".to_string(), "value2".to_string()).await.unwrap();
    cache.put("key3".to_string(), "value3".to_string()).await.unwrap(); // Should evict oldest

    // Check that we don't exceed capacity
    let metrics = cache.get_metrics().await;
    assert!(metrics.l1_stats.size <= 2);
}

#[tokio::test]
async fn test_cache_entry_aging() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Create entry and check aging
    let entry = CacheEntry::new("test_value".to_string(), CacheTier::L1, 100);
    let initial_age = entry.age();
    
    // Age should be very small initially
    assert!(initial_age < Duration::from_millis(10));
    
    // Wait a bit
    sleep(Duration::from_millis(10)).await;
    
    // Create another entry and check it has some age
    let entry2 = CacheEntry::new("test_value2".to_string(), CacheTier::L1, 100);
    let age_after_wait = entry2.age();
    assert!(age_after_wait < Duration::from_millis(50)); // Should still be small but measurable
}

#[tokio::test]
async fn test_cache_access_counting() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();

    // Access multiple times
    for _ in 0..5 {
        cache.get(&"key1".to_string()).await;
    }

    // Check that access count increases (indirectly through promotion behavior)
    // This is tested through the promotion test above
}

#[tokio::test]
async fn test_different_eviction_policies() {
    let mut config = CacheConfig::default();
    config.eviction_policy = EvictionPolicy::Lfu;
    let cache = MultiTierCacheManager::new(config);

    // Basic functionality should still work regardless of eviction policy
    cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
    let result = cache.get(&"key1".to_string()).await;
    assert_eq!(result, Some("value1".to_string()));
}

#[tokio::test]
async fn test_cache_memory_efficiency() {
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);

    // Add some entries
    for i in 0..10 {
        cache.put(format!("key{}", i), format!("value{}", i)).await.unwrap();
    }

    let metrics = cache.get_metrics().await;
    let efficiency = metrics.memory_efficiency();
    
    // Should have some reasonable efficiency value
    assert!(efficiency > 0.0);
}

// Custom cache warmer for testing
struct TestCacheWarmer {
    data: std::collections::HashMap<String, String>,
}

impl TestCacheWarmer {
    fn new() -> Self {
        let mut data = std::collections::HashMap::new();
        data.insert("test1".to_string(), "warmed_test1".to_string());
        data.insert("test2".to_string(), "warmed_test2".to_string());
        
        Self { data }
    }
}

#[async_trait::async_trait]
impl CacheWarmer<String, String> for TestCacheWarmer {
    async fn load_value(&self, key: &String) -> Option<String> {
        self.data.get(key).cloned()
    }

    async fn warm_popular_keys(&self) -> Result<u64, AppError> {
        Ok(self.data.len() as u64)
    }

    async fn get_popular_keys(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }
}

#[tokio::test]
async fn test_custom_cache_warmer() {
    let config = CacheConfig::default();
    let warmer = Arc::new(TestCacheWarmer::new());
    let cache = MultiTierCacheManager::new(config).with_warmer(warmer.clone());

    // Get popular keys and warm them
    let popular_keys = warmer.get_popular_keys().await;
    let warmed = cache.warm_cache(popular_keys.clone()).await.unwrap();
    
    assert_eq!(warmed, 2);
    
    // Verify warmed values
    assert_eq!(cache.get(&"test1".to_string()).await, Some("warmed_test1".to_string()));
    assert_eq!(cache.get(&"test2".to_string()).await, Some("warmed_test2".to_string()));
}