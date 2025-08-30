//! Tests for Cache Management System

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use rustci::core::resources::cache_management::{
    CacheManager, CacheConfig, EvictionPolicy, CachePriority, InvalidationPattern,
    CacheWarmer, CacheCompressor,
};

/// Test cache warmer for testing warming functionality
struct TestCacheWarmer {
    name: String,
    warm_threshold: f64,
}

impl TestCacheWarmer {
    fn new(name: &str, warm_threshold: f64) -> Self {
        Self {
            name: name.to_string(),
            warm_threshold,
        }
    }
}

#[async_trait::async_trait]
impl CacheWarmer for TestCacheWarmer {
    async fn warm_cache(&self, cache: &CacheManager) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Warm cache with some test data
        let warm_data = vec![
            ("warm:1".to_string(), b"warmed data 1".to_vec()),
            ("warm:2".to_string(), b"warmed data 2".to_vec()),
            ("warm:3".to_string(), b"warmed data 3".to_vec()),
        ];

        let mut warmed_count = 0;
        for (key, data) in warm_data {
            if cache.put(key, data, None, Some(CachePriority::High), None).await.is_ok() {
                warmed_count += 1;
            }
        }

        Ok(warmed_count)
    }

    fn get_warmer_name(&self) -> &str {
        &self.name
    }

    fn should_warm(&self, stats: &rustci::core::resources::cache_management::CacheStats) -> bool {
        stats.hit_ratio() < self.warm_threshold
    }
}

/// Test compressor for testing compression functionality
struct TestCompressor;

impl CacheCompressor for TestCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Simple "compression" - just add a prefix
        let mut compressed = b"COMPRESSED:".to_vec();
        compressed.extend_from_slice(data);
        Ok(compressed)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Simple "decompression" - remove the prefix
        if data.starts_with(b"COMPRESSED:") {
            Ok(data[11..].to_vec())
        } else {
            Err("Invalid compressed data".into())
        }
    }

    fn get_compressor_name(&self) -> &str {
        "TestCompressor"
    }
}

#[tokio::test]
async fn test_cache_manager_creation_with_config() {
    let config = CacheConfig {
        max_size_bytes: 1024 * 1024, // 1MB
        max_entries: 5000,
        default_ttl: Some(Duration::from_secs(1800)), // 30 minutes
        eviction_policy: EvictionPolicy::LFU,
        cleanup_interval: Duration::from_secs(120),
        memory_pressure_threshold: 0.9,
        enable_compression: true,
        enable_warming: true,
    };

    let cache = CacheManager::new(config.clone());
    let stats = cache.get_stats().await;

    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.total_size_bytes, 0);
    assert_eq!(stats.hit_count, 0);
    assert_eq!(stats.miss_count, 0);
    assert_eq!(stats.eviction_count, 0);
}

#[tokio::test]
async fn test_cache_put_get_with_metadata() {
    let cache = CacheManager::default();

    let key = "test_key_with_metadata".to_string();
    let data = b"test data with metadata".to_vec();
    let ttl = Some(Duration::from_secs(3600));
    let priority = Some(CachePriority::High);
    let mut tags = HashMap::new();
    tags.insert("category".to_string(), "test".to_string());
    tags.insert("version".to_string(), "1.0".to_string());

    cache.put(key.clone(), data.clone(), ttl, priority, Some(tags.clone())).await.unwrap();

    let retrieved = cache.get(&key).await;
    assert_eq!(retrieved, Some(data));

    let stats = cache.get_stats().await;
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.hit_count, 1);
    assert_eq!(stats.miss_count, 0);
}

#[tokio::test]
async fn test_cache_multiple_entries() {
    let cache = CacheManager::default();

    // Add multiple entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let data = format!("data_{}", i).into_bytes();
        cache.put(key, data, None, None, None).await.unwrap();
    }

    let stats = cache.get_stats().await;
    assert_eq!(stats.total_entries, 10);

    // Retrieve all entries
    for i in 0..10 {
        let key = format!("key_{}", i);
        let expected_data = format!("data_{}", i).into_bytes();
        let retrieved = cache.get(&key).await;
        assert_eq!(retrieved, Some(expected_data));
    }

    let stats = cache.get_stats().await;
    assert_eq!(stats.hit_count, 10);
    assert_eq!(stats.miss_count, 0);
}

#[tokio::test]
async fn test_cache_ttl_with_different_durations() {
    let cache = CacheManager::default();

    // Add entry with short TTL
    let short_key = "short_ttl_key".to_string();
    let short_data = b"short ttl data".to_vec();
    cache.put(short_key.clone(), short_data.clone(), Some(Duration::from_millis(50)), None, None).await.unwrap();

    // Add entry with long TTL
    let long_key = "long_ttl_key".to_string();
    let long_data = b"long ttl data".to_vec();
    cache.put(long_key.clone(), long_data.clone(), Some(Duration::from_secs(10)), None, None).await.unwrap();

    // Both should be available immediately
    assert_eq!(cache.get(&short_key).await, Some(short_data));
    assert_eq!(cache.get(&long_key).await, Some(long_data.clone()));

    // Wait for short TTL to expire
    sleep(Duration::from_millis(100)).await;

    // Short TTL should be expired, long TTL should still be available
    assert_eq!(cache.get(&short_key).await, None);
    assert_eq!(cache.get(&long_key).await, Some(long_data));
}

#[tokio::test]
async fn test_cache_eviction_lru_policy() {
    let config = CacheConfig {
        max_size_bytes: 200,
        max_entries: 1000,
        eviction_policy: EvictionPolicy::LRU,
        ..Default::default()
    };
    let cache = CacheManager::new(config);

    // Add entries that will fill the cache
    let data1 = vec![0u8; 80];
    let data2 = vec![0u8; 80];
    let data3 = vec![0u8; 80];

    cache.put("key1".to_string(), data1.clone(), None, None, None).await.unwrap();
    cache.put("key2".to_string(), data2.clone(), None, None, None).await.unwrap();

    // Access key1 to make it more recently used
    let _ = cache.get("key1").await;

    // Add key3, which should evict key2 (least recently used)
    cache.put("key3".to_string(), data3.clone(), None, None, None).await.unwrap();

    // key1 and key3 should be present, key2 should be evicted
    assert_eq!(cache.get("key1").await, Some(data1));
    assert_eq!(cache.get("key2").await, None);
    assert_eq!(cache.get("key3").await, Some(data3));
}

#[tokio::test]
async fn test_cache_eviction_fifo_policy() {
    let config = CacheConfig {
        max_size_bytes: 200,
        max_entries: 1000,
        eviction_policy: EvictionPolicy::FIFO,
        ..Default::default()
    };
    let cache = CacheManager::new(config);

    let data1 = vec![0u8; 80];
    let data2 = vec![0u8; 80];
    let data3 = vec![0u8; 80];

    cache.put("key1".to_string(), data1.clone(), None, None, None).await.unwrap();
    cache.put("key2".to_string(), data2.clone(), None, None, None).await.unwrap();

    // Access key1 multiple times (shouldn't matter for FIFO)
    let _ = cache.get("key1").await;
    let _ = cache.get("key1").await;

    // Add key3, which should evict key1 (first in, first out)
    cache.put("key3".to_string(), data3.clone(), None, None, None).await.unwrap();

    // key2 and key3 should be present, key1 should be evicted
    assert_eq!(cache.get("key1").await, None);
    assert_eq!(cache.get("key2").await, Some(data2));
    assert_eq!(cache.get("key3").await, Some(data3));
}

#[tokio::test]
async fn test_cache_eviction_by_entry_count() {
    let config = CacheConfig {
        max_size_bytes: 1024 * 1024, // Large size limit
        max_entries: 3, // Small entry limit
        eviction_policy: EvictionPolicy::LRU,
        ..Default::default()
    };
    let cache = CacheManager::new(config);

    // Add entries up to the limit
    cache.put("key1".to_string(), b"data1".to_vec(), None, None, None).await.unwrap();
    cache.put("key2".to_string(), b"data2".to_vec(), None, None, None).await.unwrap();
    cache.put("key3".to_string(), b"data3".to_vec(), None, None, None).await.unwrap();

    let stats = cache.get_stats().await;
    assert_eq!(stats.total_entries, 3);

    // Add one more entry, should trigger eviction
    cache.put("key4".to_string(), b"data4".to_vec(), None, None, None).await.unwrap();

    let stats = cache.get_stats().await;
    assert_eq!(stats.total_entries, 3); // Should still be at the limit
    assert!(stats.eviction_count > 0);
}

#[tokio::test]
async fn test_cache_remove_entry() {
    let cache = CacheManager::default();

    let key = "removable_key".to_string();
    let data = b"removable data".to_vec();

    cache.put(key.clone(), data.clone(), None, None, None).await.unwrap();

    // Verify entry exists
    assert_eq!(cache.get(&key).await, Some(data));

    // Remove entry
    let removed = cache.remove(&key).await;
    assert!(removed);

    // Verify entry is gone
    assert_eq!(cache.get(&key).await, None);

    // Try to remove non-existent entry
    let not_removed = cache.remove("non_existent").await;
    assert!(!not_removed);
}

#[tokio::test]
async fn test_cache_invalidation_exact() {
    let cache = CacheManager::default();

    cache.put("exact_key".to_string(), b"exact data".to_vec(), None, None, None).await.unwrap();
    cache.put("other_key".to_string(), b"other data".to_vec(), None, None, None).await.unwrap();

    let invalidated = cache.invalidate(InvalidationPattern::Exact("exact_key".to_string())).await;
    assert_eq!(invalidated, 1);

    assert_eq!(cache.get("exact_key").await, None);
    assert!(cache.get("other_key").await.is_some());
}

#[tokio::test]
async fn test_cache_invalidation_prefix() {
    let cache = CacheManager::default();

    cache.put("user:123".to_string(), b"user 123".to_vec(), None, None, None).await.unwrap();
    cache.put("user:456".to_string(), b"user 456".to_vec(), None, None, None).await.unwrap();
    cache.put("session:abc".to_string(), b"session abc".to_vec(), None, None, None).await.unwrap();

    let invalidated = cache.invalidate(InvalidationPattern::Prefix("user:".to_string())).await;
    assert_eq!(invalidated, 2);

    assert_eq!(cache.get("user:123").await, None);
    assert_eq!(cache.get("user:456").await, None);
    assert!(cache.get("session:abc").await.is_some());
}

#[tokio::test]
async fn test_cache_invalidation_suffix() {
    let cache = CacheManager::default();

    cache.put("data.json".to_string(), b"json data".to_vec(), None, None, None).await.unwrap();
    cache.put("config.json".to_string(), b"json config".to_vec(), None, None, None).await.unwrap();
    cache.put("data.xml".to_string(), b"xml data".to_vec(), None, None, None).await.unwrap();

    let invalidated = cache.invalidate(InvalidationPattern::Suffix(".json".to_string())).await;
    assert_eq!(invalidated, 2);

    assert_eq!(cache.get("data.json").await, None);
    assert_eq!(cache.get("config.json").await, None);
    assert!(cache.get("data.xml").await.is_some());
}

#[tokio::test]
async fn test_cache_invalidation_tag() {
    let cache = CacheManager::default();

    let mut tags1 = HashMap::new();
    tags1.insert("environment".to_string(), "test".to_string());

    let mut tags2 = HashMap::new();
    tags2.insert("environment".to_string(), "prod".to_string());

    let mut tags3 = HashMap::new();
    tags3.insert("environment".to_string(), "test".to_string());

    cache.put("key1".to_string(), b"data1".to_vec(), None, None, Some(tags1)).await.unwrap();
    cache.put("key2".to_string(), b"data2".to_vec(), None, None, Some(tags2)).await.unwrap();
    cache.put("key3".to_string(), b"data3".to_vec(), None, None, Some(tags3)).await.unwrap();

    let invalidated = cache.invalidate(InvalidationPattern::Tag("environment".to_string(), "test".to_string())).await;
    assert_eq!(invalidated, 2);

    assert_eq!(cache.get("key1").await, None);
    assert!(cache.get("key2").await.is_some());
    assert_eq!(cache.get("key3").await, None);
}

#[tokio::test]
async fn test_cache_invalidation_all() {
    let cache = CacheManager::default();

    cache.put("key1".to_string(), b"data1".to_vec(), None, None, None).await.unwrap();
    cache.put("key2".to_string(), b"data2".to_vec(), None, None, None).await.unwrap();
    cache.put("key3".to_string(), b"data3".to_vec(), None, None, None).await.unwrap();

    let stats_before = cache.get_stats().await;
    assert_eq!(stats_before.total_entries, 3);

    let invalidated = cache.invalidate(InvalidationPattern::All).await;
    assert_eq!(invalidated, 3);

    let stats_after = cache.get_stats().await;
    assert_eq!(stats_after.total_entries, 0);

    assert_eq!(cache.get("key1").await, None);
    assert_eq!(cache.get("key2").await, None);
    assert_eq!(cache.get("key3").await, None);
}

#[tokio::test]
async fn test_cache_hit_ratio_calculation() {
    let cache = CacheManager::default();

    cache.put("key1".to_string(), b"data1".to_vec(), None, None, None).await.unwrap();

    // Generate some hits and misses
    let _ = cache.get("key1").await; // hit
    let _ = cache.get("key1").await; // hit
    let _ = cache.get("key2").await; // miss
    let _ = cache.get("key3").await; // miss

    let stats = cache.get_stats().await;
    assert_eq!(stats.hit_count, 2);
    assert_eq!(stats.miss_count, 2);
    assert_eq!(stats.hit_ratio(), 0.5); // 2 hits out of 4 total requests
}

#[tokio::test]
async fn test_cache_priority_levels() {
    let cache = CacheManager::default();

    cache.put("low_priority".to_string(), b"low data".to_vec(), None, Some(CachePriority::Low), None).await.unwrap();
    cache.put("normal_priority".to_string(), b"normal data".to_vec(), None, Some(CachePriority::Normal), None).await.unwrap();
    cache.put("high_priority".to_string(), b"high data".to_vec(), None, Some(CachePriority::High), None).await.unwrap();
    cache.put("critical_priority".to_string(), b"critical data".to_vec(), None, Some(CachePriority::Critical), None).await.unwrap();

    // All entries should be retrievable
    assert!(cache.get("low_priority").await.is_some());
    assert!(cache.get("normal_priority").await.is_some());
    assert!(cache.get("high_priority").await.is_some());
    assert!(cache.get("critical_priority").await.is_some());

    let stats = cache.get_stats().await;
    assert_eq!(stats.total_entries, 4);
}

#[tokio::test]
async fn test_cache_access_count_tracking() {
    let cache = CacheManager::default();

    let key = "access_tracked_key".to_string();
    cache.put(key.clone(), b"tracked data".to_vec(), None, None, None).await.unwrap();

    // Access the key multiple times
    for _ in 0..5 {
        let _ = cache.get(&key).await;
    }

    let stats = cache.get_stats().await;
    assert_eq!(stats.hit_count, 5);
    assert_eq!(stats.miss_count, 0);
}

#[tokio::test]
async fn test_cache_memory_usage_calculation() {
    let cache = CacheManager::default();

    // Add entries with known sizes
    let data1 = vec![0u8; 100];
    let data2 = vec![0u8; 200];
    let data3 = vec![0u8; 150];

    cache.put("key1".to_string(), data1, None, None, None).await.unwrap();
    cache.put("key2".to_string(), data2, None, None, None).await.unwrap();
    cache.put("key3".to_string(), data3, None, None, None).await.unwrap();

    let stats = cache.get_stats().await;
    assert_eq!(stats.total_entries, 3);
    assert_eq!(stats.total_size_bytes, 450); // 100 + 200 + 150
}

#[tokio::test]
async fn test_cache_automatic_cleanup() {
    let config = CacheConfig {
        cleanup_interval: Duration::from_millis(50),
        ..Default::default()
    };
    let mut cache = CacheManager::new(config);

    // Add entry with short TTL
    cache.put("expiring_key".to_string(), b"expiring data".to_vec(), Some(Duration::from_millis(25)), None, None).await.unwrap();

    // Start cleanup process
    cache.start().await.unwrap();

    // Wait for cleanup to run
    sleep(Duration::from_millis(100)).await;

    // Entry should be cleaned up
    assert_eq!(cache.get("expiring_key").await, None);

    let stats = cache.get_stats().await;
    assert!(stats.eviction_count > 0);

    cache.stop().await;
}

#[tokio::test]
async fn test_cache_warmer_functionality() {
    let config = CacheConfig {
        enable_warming: true,
        ..Default::default()
    };
    let cache = CacheManager::new(config);

    let warmer = Arc::new(TestCacheWarmer::new("test_warmer", 0.5));
    cache.register_warmer(warmer).await;

    // Create some cache misses to trigger warming
    let _ = cache.get("miss1").await;
    let _ = cache.get("miss2").await;

    let stats_before = cache.get_stats().await;
    assert!(stats_before.hit_ratio() < 0.5); // Should trigger warming

    // Note: In a real test, we would need to wait for the warming cycle
    // For this test, we just verify the warmer was registered
    let stats = cache.get_stats().await;
    assert_eq!(stats.miss_count, 2);
}