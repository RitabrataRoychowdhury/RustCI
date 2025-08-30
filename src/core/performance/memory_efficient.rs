use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// Memory-efficient circular buffer with automatic cleanup
pub struct CircularBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
    total_items: u64,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            total_items: 0,
        }
    }

    pub fn push(&mut self, item: T) -> Option<T> {
        let evicted = if self.buffer.len() >= self.capacity {
            self.buffer.pop_front()
        } else {
            None
        };

        self.buffer.push_back(item);
        self.total_items += 1;
        evicted
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn total_items(&self) -> u64 {
        self.total_items
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Memory-efficient LRU cache with TTL support
pub struct LruTtlCache<K, V> 
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    cache: HashMap<K, CacheEntry<V>>,
    access_order: VecDeque<K>,
    capacity: usize,
    default_ttl: Duration,
    hits: u64,
    misses: u64,
}

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    last_accessed: Instant,
    ttl: Duration,
    access_count: u64,
}

impl<V> CacheEntry<V> {
    fn new(value: V, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            ttl,
            access_count: 1,
        }
    }

    fn is_expired(&self) -> bool {
        self.last_accessed.elapsed() > self.ttl
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

impl<K, V> LruTtlCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(capacity: usize, default_ttl: Duration) -> Self {
        Self {
            cache: HashMap::with_capacity(capacity),
            access_order: VecDeque::with_capacity(capacity),
            capacity,
            default_ttl,
            hits: 0,
            misses: 0,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        self.cleanup_expired();

        let is_expired = if let Some(entry) = self.cache.get(key) {
            entry.is_expired()
        } else {
            return {
                self.misses += 1;
                None
            };
        };

        if is_expired {
            // Entry is expired, remove it
            self.cache.remove(key);
            self.remove_from_order(key);
            self.misses += 1;
            return None;
        }

        // Entry exists and is not expired
        if let Some(entry) = self.cache.get_mut(key) {
            entry.touch();
            let value = entry.value.clone();
            self.move_to_back(key);
            self.hits += 1;
            Some(value)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        self.put_with_ttl(key, value, self.default_ttl)
    }

    pub fn put_with_ttl(&mut self, key: K, value: V, ttl: Duration) -> Option<V> {
        self.cleanup_expired();

        let entry = CacheEntry::new(value, ttl);
        let old_value = if let Some(old_entry) = self.cache.insert(key.clone(), entry) {
            Some(old_entry.value)
        } else {
            None
        };

        if old_value.is_some() {
            self.move_to_back(&key);
        } else {
            self.access_order.push_back(key.clone());
            
            // Evict LRU if over capacity
            if self.cache.len() > self.capacity {
                if let Some(lru_key) = self.access_order.pop_front() {
                    self.cache.remove(&lru_key);
                }
            }
        }

        old_value
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.remove_from_order(key);
        self.cache.remove(key).map(|entry| entry.value)
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            hit_rate: self.hit_rate(),
            size: self.cache.len(),
            capacity: self.capacity,
        }
    }

    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        let mut expired_keys = Vec::new();

        for (key, entry) in &self.cache {
            if entry.is_expired() {
                expired_keys.push(key.clone());
            }
        }

        for key in expired_keys {
            self.cache.remove(&key);
            self.remove_from_order(&key);
        }
    }

    fn move_to_back(&mut self, key: &K) {
        self.remove_from_order(key);
        self.access_order.push_back(key.clone());
    }

    fn remove_from_order(&mut self, key: &K) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            self.access_order.remove(pos);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub size: usize,
    pub capacity: usize,
}

/// Memory-efficient string interning for reducing memory usage
pub struct StringInterner {
    strings: HashMap<String, Arc<str>>,
    reverse_map: HashMap<u64, Arc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            strings: HashMap::new(),
            reverse_map: HashMap::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> Arc<str> {
        if let Some(interned) = self.strings.get(s) {
            Arc::clone(interned)
        } else {
            let interned: Arc<str> = Arc::from(s);
            let hash = self.calculate_hash(&interned);
            
            self.strings.insert(s.to_string(), Arc::clone(&interned));
            self.reverse_map.insert(hash, Arc::clone(&interned));
            
            interned
        }
    }

    pub fn get(&self, s: &str) -> Option<Arc<str>> {
        self.strings.get(s).cloned()
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn memory_usage(&self) -> usize {
        self.strings.iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>()
    }

    fn calculate_hash(&self, s: &Arc<str>) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}

/// Memory pool for reusing allocations
pub struct MemoryPool<T> {
    pool: VecDeque<T>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
    created: u64,
    reused: u64,
}

impl<T> MemoryPool<T> {
    pub fn new<F>(factory: F, max_size: usize) -> Self 
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: VecDeque::with_capacity(max_size),
            factory: Box::new(factory),
            max_size,
            created: 0,
            reused: 0,
        }
    }

    pub fn get(&mut self) -> T {
        if let Some(item) = self.pool.pop_front() {
            self.reused += 1;
            item
        } else {
            self.created += 1;
            (self.factory)()
        }
    }

    pub fn return_item(&mut self, item: T) {
        if self.pool.len() < self.max_size {
            self.pool.push_back(item);
        }
        // If pool is full, item is dropped
    }

    pub fn stats(&self) -> PoolStats {
        PoolStats {
            pool_size: self.pool.len(),
            max_size: self.max_size,
            created: self.created,
            reused: self.reused,
            reuse_rate: if self.created + self.reused == 0 {
                0.0
            } else {
                self.reused as f64 / (self.created + self.reused) as f64
            },
        }
    }

    pub fn clear(&mut self) {
        self.pool.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub pool_size: usize,
    pub max_size: usize,
    pub created: u64,
    pub reused: u64,
    pub reuse_rate: f64,
}

/// Compact bit vector for memory-efficient boolean storage
pub struct BitVector {
    bits: Vec<u64>,
    len: usize,
}

impl BitVector {
    pub fn new(len: usize) -> Self {
        let words_needed = (len + 63) / 64;
        Self {
            bits: vec![0; words_needed],
            len,
        }
    }

    pub fn set(&mut self, index: usize, value: bool) {
        if index >= self.len {
            return;
        }

        let word_index = index / 64;
        let bit_index = index % 64;
        let mask = 1u64 << bit_index;

        if value {
            self.bits[word_index] |= mask;
        } else {
            self.bits[word_index] &= !mask;
        }
    }

    pub fn get(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }

        let word_index = index / 64;
        let bit_index = index % 64;
        let mask = 1u64 << bit_index;

        (self.bits[word_index] & mask) != 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn count_ones(&self) -> usize {
        self.bits.iter().map(|word| word.count_ones() as usize).sum()
    }

    pub fn count_zeros(&self) -> usize {
        self.len - self.count_ones()
    }

    pub fn memory_usage(&self) -> usize {
        self.bits.len() * 8 // 8 bytes per u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_buffer() {
        let mut buffer = CircularBuffer::new(3);
        
        assert_eq!(buffer.push(1), None);
        assert_eq!(buffer.push(2), None);
        assert_eq!(buffer.push(3), None);
        assert_eq!(buffer.push(4), Some(1)); // Evicts 1
        
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.total_items(), 4);
    }

    #[test]
    fn test_lru_ttl_cache() {
        let mut cache = LruTtlCache::new(2, Duration::from_secs(1));
        
        cache.put("key1".to_string(), "value1".to_string());
        cache.put("key2".to_string(), "value2".to_string());
        
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.len(), 2);
        
        // This should evict key2 (LRU)
        cache.put("key3".to_string(), "value3".to_string());
        assert_eq!(cache.get(&"key2".to_string()), None);
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.get(&"key3".to_string()), Some("value3".to_string()));
    }

    #[test]
    fn test_bit_vector() {
        let mut bv = BitVector::new(100);
        
        bv.set(0, true);
        bv.set(50, true);
        bv.set(99, true);
        
        assert!(bv.get(0));
        assert!(bv.get(50));
        assert!(bv.get(99));
        assert!(!bv.get(25));
        
        assert_eq!(bv.count_ones(), 3);
        assert_eq!(bv.count_zeros(), 97);
    }
}