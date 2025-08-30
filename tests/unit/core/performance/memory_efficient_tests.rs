use std::time::Duration;
use tokio::time::sleep;

use rustci::core::performance::{
    CircularBuffer, LruTtlCache, StringInterner, MemoryPool, BitVector
};

#[test]
fn test_circular_buffer_basic_operations() {
    let mut buffer = CircularBuffer::new(3);
    
    // Test empty buffer
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
    assert_eq!(buffer.capacity(), 3);
    assert_eq!(buffer.total_items(), 0);

    // Test adding items within capacity
    assert_eq!(buffer.push(1), None);
    assert_eq!(buffer.push(2), None);
    assert_eq!(buffer.push(3), None);
    
    assert_eq!(buffer.len(), 3);
    assert!(!buffer.is_empty());
    assert_eq!(buffer.total_items(), 3);

    // Test eviction when over capacity
    assert_eq!(buffer.push(4), Some(1)); // Should evict 1
    assert_eq!(buffer.push(5), Some(2)); // Should evict 2
    
    assert_eq!(buffer.len(), 3);
    assert_eq!(buffer.total_items(), 5);

    // Test iteration
    let items: Vec<_> = buffer.iter().cloned().collect();
    assert_eq!(items, vec![3, 4, 5]);

    // Test clear
    buffer.clear();
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
}

#[test]
fn test_lru_ttl_cache_basic_operations() {
    let mut cache = LruTtlCache::new(3, Duration::from_secs(60));
    
    // Test empty cache
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    assert_eq!(cache.hit_rate(), 0.0);

    // Test putting and getting
    cache.put("key1".to_string(), "value1".to_string());
    cache.put("key2".to_string(), "value2".to_string());
    cache.put("key3".to_string(), "value3".to_string());
    
    assert_eq!(cache.len(), 3);
    assert!(!cache.is_empty());

    assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
    assert_eq!(cache.get(&"key2".to_string()), Some("value2".to_string()));
    assert_eq!(cache.get(&"key3".to_string()), Some("value3".to_string()));
    assert_eq!(cache.get(&"nonexistent".to_string()), None);

    // Test LRU eviction
    cache.put("key4".to_string(), "value4".to_string()); // Should evict key1 (LRU)
    assert_eq!(cache.get(&"key1".to_string()), None);
    assert_eq!(cache.get(&"key4".to_string()), Some("value4".to_string()));

    // Test hit rate calculation
    let stats = cache.stats();
    assert!(stats.hit_rate > 0.0);
    assert_eq!(stats.size, 3);
    assert_eq!(stats.capacity, 3);
}

#[tokio::test]
async fn test_lru_ttl_cache_expiration() {
    let mut cache = LruTtlCache::new(5, Duration::from_millis(50));
    
    // Add item with short TTL
    cache.put_with_ttl("key1".to_string(), "value1".to_string(), Duration::from_millis(10));
    cache.put("key2".to_string(), "value2".to_string()); // Uses default TTL
    
    // Should be able to get immediately
    assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
    assert_eq!(cache.get(&"key2".to_string()), Some("value2".to_string()));
    
    // Wait for key1 to expire
    sleep(Duration::from_millis(20)).await;
    
    // key1 should be expired, key2 should still be valid
    assert_eq!(cache.get(&"key1".to_string()), None);
    assert_eq!(cache.get(&"key2".to_string()), Some("value2".to_string()));
}

#[test]
fn test_lru_ttl_cache_update_existing() {
    let mut cache = LruTtlCache::new(3, Duration::from_secs(60));
    
    cache.put("key1".to_string(), "value1".to_string());
    assert_eq!(cache.len(), 1);
    
    // Update existing key
    let old_value = cache.put("key1".to_string(), "new_value1".to_string());
    assert_eq!(old_value, Some("value1".to_string()));
    assert_eq!(cache.len(), 1); // Size shouldn't change
    assert_eq!(cache.get(&"key1".to_string()), Some("new_value1".to_string()));
}

#[test]
fn test_string_interner() {
    let mut interner = StringInterner::new();
    
    // Test interning
    let str1 = interner.intern("hello");
    let str2 = interner.intern("world");
    let str3 = interner.intern("hello"); // Same string
    
    assert_eq!(str1.as_ref(), "hello");
    assert_eq!(str2.as_ref(), "world");
    assert_eq!(str3.as_ref(), "hello");
    
    // str1 and str3 should be the same Arc
    assert!(Arc::ptr_eq(&str1, &str3));
    
    assert_eq!(interner.len(), 2); // Only 2 unique strings
    
    // Test getting existing string
    let str4 = interner.get("hello").unwrap();
    assert!(Arc::ptr_eq(&str1, &str4));
    
    // Test getting non-existent string
    assert!(interner.get("nonexistent").is_none());
    
    // Test memory usage calculation
    let memory_usage = interner.memory_usage();
    assert!(memory_usage > 0);
}

#[test]
fn test_memory_pool() {
    let mut pool = MemoryPool::new(|| Vec::<i32>::with_capacity(100), 5);
    
    // Test getting from empty pool (should create new)
    let mut vec1 = pool.get();
    vec1.push(1);
    vec1.push(2);
    
    let stats = pool.stats();
    assert_eq!(stats.created, 1);
    assert_eq!(stats.reused, 0);
    assert_eq!(stats.pool_size, 0);
    
    // Return to pool
    vec1.clear(); // Clear but keep capacity
    pool.return_item(vec1);
    
    let stats_after_return = pool.stats();
    assert_eq!(stats_after_return.pool_size, 1);
    
    // Get from pool (should reuse)
    let vec2 = pool.get();
    assert_eq!(vec2.capacity(), 100); // Should have original capacity
    
    let stats_after_reuse = pool.stats();
    assert_eq!(stats_after_reuse.created, 1);
    assert_eq!(stats_after_reuse.reused, 1);
    assert_eq!(stats_after_reuse.reuse_rate, 0.5);
    
    pool.return_item(vec2);
    
    // Test pool size limit
    for i in 0..10 {
        let vec = pool.get();
        pool.return_item(vec);
    }
    
    let final_stats = pool.stats();
    assert!(final_stats.pool_size <= 5); // Should not exceed max_size
}

#[test]
fn test_bit_vector() {
    let mut bv = BitVector::new(100);
    
    // Test initial state
    assert_eq!(bv.len(), 100);
    assert_eq!(bv.count_ones(), 0);
    assert_eq!(bv.count_zeros(), 100);
    
    // Test setting bits
    bv.set(0, true);
    bv.set(50, true);
    bv.set(99, true);
    
    assert!(bv.get(0));
    assert!(bv.get(50));
    assert!(bv.get(99));
    assert!(!bv.get(25));
    assert!(!bv.get(75));
    
    assert_eq!(bv.count_ones(), 3);
    assert_eq!(bv.count_zeros(), 97);
    
    // Test unsetting bits
    bv.set(50, false);
    assert!(!bv.get(50));
    assert_eq!(bv.count_ones(), 2);
    assert_eq!(bv.count_zeros(), 98);
    
    // Test out of bounds (should not panic)
    bv.set(200, true); // Should be ignored
    assert!(!bv.get(200)); // Should return false
    
    // Test memory usage
    let memory_usage = bv.memory_usage();
    assert!(memory_usage > 0);
    assert_eq!(memory_usage, 16); // 2 u64 words * 8 bytes = 16 bytes for 100 bits
}

#[test]
fn test_bit_vector_edge_cases() {
    // Test with size that's not a multiple of 64
    let mut bv = BitVector::new(65);
    
    bv.set(0, true);
    bv.set(63, true);
    bv.set(64, true);
    
    assert!(bv.get(0));
    assert!(bv.get(63));
    assert!(bv.get(64));
    assert_eq!(bv.count_ones(), 3);
    
    // Test with very small size
    let mut small_bv = BitVector::new(1);
    small_bv.set(0, true);
    assert!(small_bv.get(0));
    assert_eq!(small_bv.count_ones(), 1);
    assert_eq!(small_bv.count_zeros(), 0);
}

#[test]
fn test_memory_pool_clear() {
    let mut pool = MemoryPool::new(|| String::new(), 3);
    
    // Add some items to pool
    for i in 0..3 {
        let item = pool.get();
        pool.return_item(item);
    }
    
    assert_eq!(pool.stats().pool_size, 3);
    
    // Clear pool
    pool.clear();
    assert_eq!(pool.stats().pool_size, 0);
    
    // Should still be able to get new items
    let item = pool.get();
    assert_eq!(pool.stats().created, 4); // 3 initial + 1 after clear
}

#[test]
fn test_lru_cache_remove() {
    let mut cache = LruTtlCache::new(3, Duration::from_secs(60));
    
    cache.put("key1".to_string(), "value1".to_string());
    cache.put("key2".to_string(), "value2".to_string());
    
    assert_eq!(cache.len(), 2);
    
    // Remove existing key
    let removed = cache.remove(&"key1".to_string());
    assert_eq!(removed, Some("value1".to_string()));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&"key1".to_string()), None);
    
    // Remove non-existent key
    let not_found = cache.remove(&"nonexistent".to_string());
    assert_eq!(not_found, None);
    assert_eq!(cache.len(), 1);
}