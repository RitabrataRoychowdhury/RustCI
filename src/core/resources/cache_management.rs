//! Cache Management System
//!
//! Provides intelligent cache eviction policies, cache memory monitoring and automatic cleanup,
//! and cache warming and invalidation strategies.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Cache eviction policies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In, First Out
    FIFO,
    /// Time To Live based
    TTL,
    /// Size-based eviction
    SizeBased,
    /// Custom eviction logic
    Custom(String),
}

/// Cache entry metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryMetadata {
    pub key: String,
    pub size_bytes: u64,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub access_count: u64,
    pub ttl: Option<Duration>,
    pub expires_at: Option<SystemTime>,
    pub tags: HashMap<String, String>,
    pub priority: CachePriority,
}

/// Cache entry priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CachePriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Cache entry with data and metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub metadata: CacheEntryMetadata,
    pub data: Vec<u8>,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_size_bytes: u64,
    pub max_entries: usize,
    pub default_ttl: Option<Duration>,
    pub eviction_policy: EvictionPolicy,
    pub cleanup_interval: Duration,
    pub memory_pressure_threshold: f64, // 0.0 to 1.0
    pub enable_compression: bool,
    pub enable_warming: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 100 * 1024 * 1024, // 100MB
            max_entries: 10000,
            default_ttl: Some(Duration::from_secs(3600)), // 1 hour
            eviction_policy: EvictionPolicy::LRU,
            cleanup_interval: Duration::from_secs(60), // 1 minute
            memory_pressure_threshold: 0.8,            // 80%
            enable_compression: false,
            enable_warming: false,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size_bytes: u64,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub warming_count: u64,
    pub compression_ratio: f64,
    pub memory_usage_ratio: f64,
}

impl CacheStats {
    pub fn hit_ratio(&self) -> f64 {
        let total_requests = self.hit_count + self.miss_count;
        if total_requests == 0 {
            0.0
        } else {
            self.hit_count as f64 / total_requests as f64
        }
    }
}

/// Trait for cache warming strategies
#[async_trait::async_trait]
pub trait CacheWarmer: Send + Sync {
    async fn warm_cache(
        &self,
        cache: &CacheManager,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
    fn get_warmer_name(&self) -> &str;
    fn should_warm(&self, stats: &CacheStats) -> bool;
}

/// Trait for cache compression
pub trait CacheCompressor: Send + Sync {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;
    fn get_compressor_name(&self) -> &str;
}

/// Cache invalidation patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidationPattern {
    Exact(String),
    Prefix(String),
    Suffix(String),
    Regex(String),
    Tag(String, String),
    All,
}

/// Cache manager with intelligent eviction and management
pub struct CacheManager {
    config: CacheConfig,
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    access_order: Arc<RwLock<BTreeMap<SystemTime, String>>>, // For LRU
    access_frequency: Arc<RwLock<HashMap<String, u64>>>,     // For LFU
    insertion_order: Arc<RwLock<BTreeMap<SystemTime, String>>>, // For FIFO
    stats: Arc<Mutex<CacheStats>>,
    warmers: Arc<RwLock<Vec<Arc<dyn CacheWarmer>>>>,
    compressor: Option<Arc<dyn CacheCompressor>>,
    is_running: Arc<Mutex<bool>>,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(config: CacheConfig) -> Self {
        let compressor = if config.enable_compression {
            Some(Arc::new(DefaultCompressor) as Arc<dyn CacheCompressor>)
        } else {
            None
        };

        Self {
            config,
            entries: Arc::new(RwLock::new(HashMap::new())),
            access_order: Arc::new(RwLock::new(BTreeMap::new())),
            access_frequency: Arc::new(RwLock::new(HashMap::new())),
            insertion_order: Arc::new(RwLock::new(BTreeMap::new())),
            stats: Arc::new(Mutex::new(CacheStats::default())),
            warmers: Arc::new(RwLock::new(Vec::new())),
            compressor,
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Register a cache warmer
    pub async fn register_warmer(&self, warmer: Arc<dyn CacheWarmer>) {
        let mut warmers = self.warmers.write().await;
        warmers.push(warmer);
    }

    /// Put an entry in the cache
    pub async fn put(
        &self,
        key: String,
        data: Vec<u8>,
        ttl: Option<Duration>,
        priority: Option<CachePriority>,
        tags: Option<HashMap<String, String>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = SystemTime::now();
        let expires_at = ttl.or(self.config.default_ttl).map(|t| now + t);

        // Compress data if enabled
        let final_data = if let Some(compressor) = &self.compressor {
            compressor.compress(&data)?
        } else {
            data
        };

        let metadata = CacheEntryMetadata {
            key: key.clone(),
            size_bytes: final_data.len() as u64,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            ttl,
            expires_at,
            tags: tags.unwrap_or_default(),
            priority: priority.unwrap_or(CachePriority::Normal),
        };

        let entry = CacheEntry {
            metadata,
            data: final_data,
        };

        // Check if we need to evict entries first
        self.ensure_capacity(&key, entry.data.len() as u64).await?;

        // Insert entry
        {
            let mut entries = self.entries.write().await;
            entries.insert(key.clone(), entry);
        }

        // Update tracking structures
        {
            let mut insertion_order = self.insertion_order.write().await;
            insertion_order.insert(now, key.clone());
        }

        {
            let mut access_order = self.access_order.write().await;
            access_order.insert(now, key.clone());
        }

        debug!("Cached entry with key: {}", key);
        Ok(())
    }

    /// Get an entry from the cache
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let now = SystemTime::now();

        let entry = {
            let mut entries = self.entries.write().await;
            if let Some(entry) = entries.get_mut(key) {
                // Check if expired
                if let Some(expires_at) = entry.metadata.expires_at {
                    if now > expires_at {
                        // Entry expired, remove it
                        entries.remove(key);
                        self.update_stats_on_miss().await;
                        return None;
                    }
                }

                // Update access metadata
                entry.metadata.last_accessed = now;
                entry.metadata.access_count += 1;

                Some(entry.clone())
            } else {
                None
            }
        };

        if let Some(entry) = entry {
            // Update access tracking
            {
                let mut access_order = self.access_order.write().await;
                access_order.insert(now, key.to_string());
            }

            {
                let mut access_frequency = self.access_frequency.write().await;
                *access_frequency.entry(key.to_string()).or_insert(0) += 1;
            }

            // Decompress data if needed
            let data = if let Some(compressor) = &self.compressor {
                match compressor.decompress(&entry.data) {
                    Ok(decompressed) => decompressed,
                    Err(e) => {
                        error!("Failed to decompress cache entry {}: {}", key, e);
                        self.update_stats_on_miss().await;
                        return None;
                    }
                }
            } else {
                entry.data
            };

            self.update_stats_on_hit().await;
            debug!("Cache hit for key: {}", key);
            Some(data)
        } else {
            self.update_stats_on_miss().await;
            debug!("Cache miss for key: {}", key);
            None
        }
    }

    /// Remove an entry from the cache
    pub async fn remove(&self, key: &str) -> bool {
        let mut entries = self.entries.write().await;
        if entries.remove(key).is_some() {
            // Clean up tracking structures
            self.cleanup_tracking_for_key(key).await;
            debug!("Removed cache entry: {}", key);
            true
        } else {
            false
        }
    }

    /// Invalidate cache entries based on pattern
    pub async fn invalidate(&self, pattern: InvalidationPattern) -> u64 {
        let keys_to_remove = {
            let entries = self.entries.read().await;
            let mut keys = Vec::new();

            for (key, entry) in entries.iter() {
                let should_remove = match &pattern {
                    InvalidationPattern::Exact(exact_key) => key == exact_key,
                    InvalidationPattern::Prefix(prefix) => key.starts_with(prefix),
                    InvalidationPattern::Suffix(suffix) => key.ends_with(suffix),
                    InvalidationPattern::Regex(regex_pattern) => {
                        // Simple regex matching (in production, use proper regex crate)
                        key.contains(regex_pattern)
                    }
                    InvalidationPattern::Tag(tag_key, tag_value) => {
                        entry.metadata.tags.get(tag_key) == Some(tag_value)
                    }
                    InvalidationPattern::All => true,
                };

                if should_remove {
                    keys.push(key.clone());
                }
            }

            keys
        };

        let removed_count = keys_to_remove.len() as u64;

        {
            let mut entries = self.entries.write().await;
            for key in &keys_to_remove {
                entries.remove(key);
            }
        }

        // Clean up tracking structures
        for key in &keys_to_remove {
            self.cleanup_tracking_for_key(key).await;
        }

        if removed_count > 0 {
            info!("Invalidated {} cache entries", removed_count);
        }

        removed_count
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let mut stats = self.stats.lock().await;

        // Update current metrics
        let entries = self.entries.read().await;
        stats.total_entries = entries.len();
        stats.total_size_bytes = entries.values().map(|e| e.metadata.size_bytes).sum();
        stats.memory_usage_ratio =
            stats.total_size_bytes as f64 / self.config.max_size_bytes as f64;

        stats.clone()
    }

    /// Start automatic cleanup and warming processes
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        // Start cleanup task
        let entries = Arc::clone(&self.entries);
        let access_order = Arc::clone(&self.access_order);
        let access_frequency = Arc::clone(&self.access_frequency);
        let insertion_order = Arc::clone(&self.insertion_order);
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::cleanup_loop(
                entries,
                access_order,
                access_frequency,
                insertion_order,
                stats,
                config,
                is_running,
            )
            .await;
        });

        // Start warming task if enabled
        if self.config.enable_warming {
            let warmers = Arc::clone(&self.warmers);
            let stats = Arc::clone(&self.stats);
            let is_running = Arc::clone(&self.is_running);
            // Note: Warming loop functionality temporarily disabled for Send trait compliance
            // TODO: Implement proper Arc-based cache warming
            tokio::spawn(async move {
                // Self::warming_loop(warmers, stats, is_running).await;
                info!("Cache warming loop started (implementation pending)");
            });
        }

        info!("Started cache management processes");
        Ok(())
    }

    /// Stop automatic processes
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("Stopped cache management processes");
    }

    /// Ensure cache has capacity for new entry
    async fn ensure_capacity(
        &self,
        new_key: &str,
        new_size: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_stats = self.get_stats().await;

        // Check if we need to evict based on size
        if current_stats.total_size_bytes + new_size > self.config.max_size_bytes {
            let bytes_to_free =
                (current_stats.total_size_bytes + new_size) - self.config.max_size_bytes;
            self.evict_by_size(bytes_to_free).await?;
        }

        // Check if we need to evict based on count
        if current_stats.total_entries >= self.config.max_entries {
            let entries_to_free = (current_stats.total_entries + 1) - self.config.max_entries;
            self.evict_by_count(entries_to_free).await?;
        }

        Ok(())
    }

    /// Evict entries to free up specified bytes
    async fn evict_by_size(
        &self,
        bytes_to_free: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let keys_to_evict = self.select_eviction_candidates_by_size(bytes_to_free).await;

        {
            let mut entries = self.entries.write().await;
            for key in &keys_to_evict {
                entries.remove(key);
            }
        }

        for key in &keys_to_evict {
            self.cleanup_tracking_for_key(key).await;
        }

        {
            let mut stats = self.stats.lock().await;
            stats.eviction_count += keys_to_evict.len() as u64;
        }

        debug!(
            "Evicted {} entries to free {} bytes",
            keys_to_evict.len(),
            bytes_to_free
        );
        Ok(())
    }

    /// Evict entries to free up specified count
    async fn evict_by_count(
        &self,
        count_to_free: usize,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let keys_to_evict = self
            .select_eviction_candidates_by_count(count_to_free)
            .await;

        {
            let mut entries = self.entries.write().await;
            for key in &keys_to_evict {
                entries.remove(key);
            }
        }

        for key in &keys_to_evict {
            self.cleanup_tracking_for_key(key).await;
        }

        {
            let mut stats = self.stats.lock().await;
            stats.eviction_count += keys_to_evict.len() as u64;
        }

        debug!("Evicted {} entries", keys_to_evict.len());
        Ok(())
    }

    /// Select candidates for eviction based on size
    async fn select_eviction_candidates_by_size(&self, bytes_to_free: u64) -> Vec<String> {
        let entries = self.entries.read().await;
        let mut candidates: Vec<_> = entries.iter().collect();

        // Sort by eviction policy
        match self.config.eviction_policy {
            EvictionPolicy::LRU => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.last_accessed);
            }
            EvictionPolicy::LFU => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.access_count);
            }
            EvictionPolicy::FIFO => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.created_at);
            }
            EvictionPolicy::TTL => {
                candidates.sort_by_key(|(_, entry)| {
                    entry.metadata.expires_at.unwrap_or(SystemTime::UNIX_EPOCH)
                });
            }
            EvictionPolicy::SizeBased => {
                candidates.sort_by_key(|(_, entry)| std::cmp::Reverse(entry.metadata.size_bytes));
            }
            EvictionPolicy::Custom(_) => {
                // Custom logic would be implemented here
                candidates.sort_by_key(|(_, entry)| entry.metadata.last_accessed);
            }
        }

        let mut selected = Vec::new();
        let mut freed_bytes = 0;

        for (key, entry) in candidates {
            if freed_bytes >= bytes_to_free {
                break;
            }
            selected.push(key.clone());
            freed_bytes += entry.metadata.size_bytes;
        }

        selected
    }

    /// Select candidates for eviction based on count
    async fn select_eviction_candidates_by_count(&self, count_to_free: usize) -> Vec<String> {
        let entries = self.entries.read().await;
        let mut candidates: Vec<_> = entries.iter().collect();

        // Sort by eviction policy (same as size-based)
        match self.config.eviction_policy {
            EvictionPolicy::LRU => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.last_accessed);
            }
            EvictionPolicy::LFU => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.access_count);
            }
            EvictionPolicy::FIFO => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.created_at);
            }
            EvictionPolicy::TTL => {
                candidates.sort_by_key(|(_, entry)| {
                    entry.metadata.expires_at.unwrap_or(SystemTime::UNIX_EPOCH)
                });
            }
            EvictionPolicy::SizeBased => {
                candidates.sort_by_key(|(_, entry)| std::cmp::Reverse(entry.metadata.size_bytes));
            }
            EvictionPolicy::Custom(_) => {
                candidates.sort_by_key(|(_, entry)| entry.metadata.last_accessed);
            }
        }

        candidates
            .into_iter()
            .take(count_to_free)
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Clean up tracking structures for a key
    async fn cleanup_tracking_for_key(&self, key: &str) {
        {
            let mut access_frequency = self.access_frequency.write().await;
            access_frequency.remove(key);
        }

        // Note: Cleaning up time-based maps is more complex and would require
        // reverse lookups or additional indexing in a production implementation
    }

    /// Update statistics on cache hit
    async fn update_stats_on_hit(&self) {
        let mut stats = self.stats.lock().await;
        stats.hit_count += 1;
    }

    /// Update statistics on cache miss
    async fn update_stats_on_miss(&self) {
        let mut stats = self.stats.lock().await;
        stats.miss_count += 1;
    }

    /// Cleanup loop for expired entries
    async fn cleanup_loop(
        entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
        access_order: Arc<RwLock<BTreeMap<SystemTime, String>>>,
        access_frequency: Arc<RwLock<HashMap<String, u64>>>,
        insertion_order: Arc<RwLock<BTreeMap<SystemTime, String>>>,
        stats: Arc<Mutex<CacheStats>>,
        config: CacheConfig,
        is_running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(config.cleanup_interval);

        while {
            let running = is_running.lock().await;
            *running
        } {
            cleanup_interval.tick().await;

            let now = SystemTime::now();
            let mut expired_keys = Vec::new();

            // Find expired entries
            {
                let entries_map = entries.read().await;
                for (key, entry) in entries_map.iter() {
                    if let Some(expires_at) = entry.metadata.expires_at {
                        if now > expires_at {
                            expired_keys.push(key.clone());
                        }
                    }
                }
            }

            // Remove expired entries
            if !expired_keys.is_empty() {
                {
                    let mut entries_map = entries.write().await;
                    for key in &expired_keys {
                        entries_map.remove(key);
                    }
                }

                // Clean up tracking structures
                {
                    let mut access_freq = access_frequency.write().await;
                    for key in &expired_keys {
                        access_freq.remove(key);
                    }
                }

                {
                    let mut stats_guard = stats.lock().await;
                    stats_guard.eviction_count += expired_keys.len() as u64;
                }

                info!("Cleaned up {} expired cache entries", expired_keys.len());
            }
        }
    }

    /// Warming loop for cache warming
    async fn warming_loop(
        warmers: Arc<RwLock<Vec<Arc<dyn CacheWarmer>>>>,
        stats: Arc<Mutex<CacheStats>>,
        is_running: Arc<Mutex<bool>>,
        cache_ptr: *const CacheManager,
    ) {
        let mut warming_interval = interval(Duration::from_secs(300)); // Warm every 5 minutes

        while {
            let running = is_running.lock().await;
            *running
        } {
            warming_interval.tick().await;

            let current_stats = {
                let stats_guard = stats.lock().await;
                stats_guard.clone()
            };

            let warmers_vec = warmers.read().await;
            for warmer in warmers_vec.iter() {
                if warmer.should_warm(&current_stats) {
                    // SAFETY: This is safe because the cache manager lives for the duration
                    // of the warming loop, and we're only reading from it
                    let cache_ref = unsafe { &*cache_ptr };

                    match warmer.warm_cache(cache_ref).await {
                        Ok(warmed_count) => {
                            let mut stats_guard = stats.lock().await;
                            stats_guard.warming_count += warmed_count;
                            info!(
                                "Cache warmer {} warmed {} entries",
                                warmer.get_warmer_name(),
                                warmed_count
                            );
                        }
                        Err(e) => {
                            error!("Cache warmer {} failed: {}", warmer.get_warmer_name(), e);
                        }
                    }
                }
            }
        }
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

/// Default compressor using simple compression
pub struct DefaultCompressor;

impl CacheCompressor for DefaultCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Simple compression simulation (in production, use proper compression library)
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Simple decompression simulation
        Ok(data.to_vec())
    }

    fn get_compressor_name(&self) -> &str {
        "DefaultCompressor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let config = CacheConfig::default();
        let cache = CacheManager::new(config);

        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);
    }

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let cache = CacheManager::default();

        let key = "test_key".to_string();
        let data = b"test data".to_vec();

        cache
            .put(key.clone(), data.clone(), None, None, None)
            .await
            .unwrap();

        let retrieved = cache.get(&key).await;
        assert_eq!(retrieved, Some(data));

        let stats = cache.get_stats().await;
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 0);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = CacheManager::default();

        let result = cache.get("nonexistent_key").await;
        assert_eq!(result, None);

        let stats = cache.get_stats().await;
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 1);
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let cache = CacheManager::default();

        let key = "expiring_key".to_string();
        let data = b"expiring data".to_vec();
        let ttl = Duration::from_millis(50);

        cache
            .put(key.clone(), data.clone(), Some(ttl), None, None)
            .await
            .unwrap();

        // Should be available immediately
        let retrieved = cache.get(&key).await;
        assert_eq!(retrieved, Some(data));

        // Wait for expiration
        sleep(Duration::from_millis(100)).await;

        // Should be expired now
        let retrieved = cache.get(&key).await;
        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_cache_eviction_by_size() {
        let config = CacheConfig {
            max_size_bytes: 100,
            max_entries: 1000,
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };
        let cache = CacheManager::new(config);

        // Fill cache to capacity
        let data1 = vec![0u8; 50];
        let data2 = vec![0u8; 40];

        cache
            .put("key1".to_string(), data1, None, None, None)
            .await
            .unwrap();
        cache
            .put("key2".to_string(), data2, None, None, None)
            .await
            .unwrap();

        // This should trigger eviction
        let data3 = vec![0u8; 30];
        cache
            .put("key3".to_string(), data3, None, None, None)
            .await
            .unwrap();

        // key1 should be evicted (LRU)
        assert_eq!(cache.get("key1").await, None);
        assert!(cache.get("key2").await.is_some());
        assert!(cache.get("key3").await.is_some());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = CacheManager::default();

        // Add multiple entries
        cache
            .put(
                "user:123".to_string(),
                b"user data".to_vec(),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        cache
            .put(
                "user:456".to_string(),
                b"user data 2".to_vec(),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        cache
            .put(
                "session:abc".to_string(),
                b"session data".to_vec(),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Invalidate by prefix
        let invalidated = cache
            .invalidate(InvalidationPattern::Prefix("user:".to_string()))
            .await;
        assert_eq!(invalidated, 2);

        // Check that user entries are gone but session remains
        assert_eq!(cache.get("user:123").await, None);
        assert_eq!(cache.get("user:456").await, None);
        assert!(cache.get("session:abc").await.is_some());
    }
}
