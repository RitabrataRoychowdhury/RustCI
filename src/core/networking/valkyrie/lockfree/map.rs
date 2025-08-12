//! Lock-Free Map Implementations
//!
//! This module provides lock-free map implementations including concurrent hash maps
//! and skip lists for high-performance key-value operations.

use super::{LockFreeMetrics, LockFreeConfig, CacheLinePadded, HazardPointer};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::ptr;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::cmp::Ordering as CmpOrdering;

/// Errors that can occur during map operations
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("Key not found")]
    KeyNotFound,
    #[error("Map is full (capacity: {capacity})")]
    Full { capacity: usize },
    #[error("Operation failed after {retries} retries")]
    MaxRetriesExceeded { retries: usize },
    #[error("Memory allocation failed")]
    AllocationFailed,
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
    #[error("Key already exists")]
    KeyExists,
}

/// Statistics for map operations
#[derive(Debug, Clone)]
pub struct MapStats {
    /// Current number of entries in the map
    pub size: usize,
    /// Maximum capacity of the map (0 for unbounded)
    pub capacity: usize,
    /// Total number of insert operations
    pub inserts: usize,
    /// Total number of remove operations
    pub removes: usize,
    /// Total number of lookup operations
    pub lookups: usize,
    /// Number of failed operations
    pub failures: usize,
    /// Average contention level
    pub contention_level: f64,
    /// Load factor (for hash maps)
    pub load_factor: f64,
}

/// Entry in a lock-free map
pub struct MapEntry<K, V> {
    /// The key
    pub key: K,
    /// The value
    pub value: V,
    /// Hash of the key (cached for performance)
    pub hash: u64,
    /// Pointer to next entry (for chaining)
    pub next: AtomicPtr<MapEntry<K, V>>,
    /// Marked for deletion flag
    pub deleted: AtomicUsize,
}

impl<K, V> MapEntry<K, V>
where
    K: Hash,
{
    /// Create a new map entry
    pub fn new(key: K, value: V) -> Self {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        Self {
            key,
            value,
            hash,
            next: AtomicPtr::new(ptr::null_mut()),
            deleted: AtomicUsize::new(0),
        }
    }

    /// Check if this entry is marked for deletion
    pub fn is_deleted(&self) -> bool {
        self.deleted.load(Ordering::Acquire) != 0
    }

    /// Mark this entry for deletion
    pub fn mark_deleted(&self) -> bool {
        self.deleted.compare_exchange(
            0,
            1,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ).is_ok()
    }
}

/// Generic lock-free map trait
pub trait LockFreeMap<K, V>: Send + Sync {
    /// Insert a key-value pair
    fn insert(&self, key: K, value: V) -> Result<Option<V>, MapError>;
    
    /// Remove a key-value pair
    fn remove(&self, key: &K) -> Result<Option<V>, MapError>;
    
    /// Get a value by key
    fn get(&self, key: &K) -> Result<Option<V>, MapError>
    where
        V: Clone;
    
    /// Check if a key exists
    fn contains_key(&self, key: &K) -> bool;
    
    /// Update a value if key exists
    fn update<F>(&self, key: &K, updater: F) -> Result<Option<V>, MapError>
    where
        F: FnOnce(&V) -> V,
        V: Clone;
    
    /// Insert or update a value
    fn upsert<F>(&self, key: K, value: V, updater: F) -> Result<Option<V>, MapError>
    where
        F: FnOnce(&V) -> V,
        V: Clone;
    
    /// Check if map is empty
    fn is_empty(&self) -> bool;
    
    /// Get current size (approximate)
    fn len(&self) -> usize;
    
    /// Get map statistics
    fn stats(&self) -> MapStats;
    
    /// Clear all entries from the map
    fn clear(&self);
}

/// Lock-free concurrent hash map
pub struct ConcurrentHashMap<K, V> 
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Array of hash buckets
    buckets: Vec<CacheLinePadded<AtomicPtr<MapEntry<K, V>>>>,
    /// Number of buckets (must be power of 2)
    bucket_count: usize,
    /// Mask for bucket indexing
    bucket_mask: usize,
    /// Current size
    size: AtomicUsize,
    /// Configuration
    config: LockFreeConfig,
    /// Performance metrics
    metrics: LockFreeMetrics,
    /// Hazard pointers for memory reclamation
    hazard_pointers: Vec<HazardPointer<MapEntry<K, V>>>,
}

impl<K, V> ConcurrentHashMap<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new concurrent hash map
    pub fn new(bucket_count: usize, config: LockFreeConfig) -> Self {
        assert!(bucket_count.is_power_of_two(), "Bucket count must be power of 2");
        
        let buckets = (0..bucket_count)
            .map(|_| CacheLinePadded::new(AtomicPtr::new(ptr::null_mut())))
            .collect();
        
        let num_threads = num_cpus::get() * 2;
        
        Self {
            buckets,
            bucket_count,
            bucket_mask: bucket_count - 1,
            size: AtomicUsize::new(0),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: (0..num_threads).map(|_| HazardPointer::new()).collect(),
        }
    }

    /// Get bucket index for a key
    fn bucket_index(&self, key: &K) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) & self.bucket_mask
    }

    /// Get a hazard pointer for the current thread
    fn get_hazard_pointer(&self) -> &HazardPointer<MapEntry<K, V>> {
        // Use a hash of the thread ID since as_u64() is unstable
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let thread_id = std::thread::current().id();
        let mut hasher = DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let index = (hasher.finish() as usize) % self.hazard_pointers.len();
        &self.hazard_pointers[index]
    }

    /// Find an entry in the bucket chain
    fn find_entry(&self, bucket_head: *mut MapEntry<K, V>, key: &K) -> (*mut MapEntry<K, V>, *mut MapEntry<K, V>) {
        let mut prev = ptr::null_mut();
        let mut current = bucket_head;
        
        while !current.is_null() {
            let entry = unsafe { &*current };
            
            if !entry.is_deleted() && entry.key == *key {
                return (prev, current);
            }
            
            prev = current;
            current = entry.next.load(Ordering::Acquire);
        }
        
        (prev, ptr::null_mut())
    }
}

impl<K, V> LockFreeMap<K, V> for ConcurrentHashMap<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn insert(&self, key: K, value: V) -> Result<Option<V>, MapError> {
        let bucket_index = self.bucket_index(&key);
        let bucket = &self.buckets[bucket_index];
        let new_entry = Box::into_raw(Box::new(MapEntry::new(key.clone(), value)));
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                unsafe { let _ = Box::from_raw(new_entry); }
                self.metrics.record_failure();
                return Err(MapError::MaxRetriesExceeded { retries });
            }

            let head = bucket.load(Ordering::Acquire);
            let hazard = self.get_hazard_pointer();
            hazard.protect(head);

            // Double-check that head hasn't changed
            if bucket.load(Ordering::Acquire) != head {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let (_, existing) = self.find_entry(head, &key);
            
            if !existing.is_null() {
                // Key already exists, update value
                let existing_entry = unsafe { &*existing };
                if !existing_entry.is_deleted() {
                    let old_value = existing_entry.value.clone();
                    unsafe {
                        let entry_mut = &mut *(existing as *mut MapEntry<K, V>);
                        entry_mut.value = (*new_entry).value.clone();
                    }
                    unsafe { let _ = Box::from_raw(new_entry); }
                    hazard.clear();
                    self.metrics.record_success();
                    return Ok(Some(old_value));
                }
            }

            // Insert new entry at head of bucket
            unsafe {
                (*new_entry).next.store(head, Ordering::Relaxed);
            }

            let cas_result = bucket.compare_exchange_weak(
                head,
                new_entry,
                Ordering::Release,
                Ordering::Relaxed,
            );

            self.metrics.record_cas(cas_result.is_ok());

            if cas_result.is_ok() {
                self.size.fetch_add(1, Ordering::Relaxed);
                hazard.clear();
                self.metrics.record_success();
                return Ok(None);
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn remove(&self, key: &K) -> Result<Option<V>, MapError> {
        let bucket_index = self.bucket_index(key);
        let bucket = &self.buckets[bucket_index];
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(MapError::MaxRetriesExceeded { retries });
            }

            let head = bucket.load(Ordering::Acquire);
            let hazard = self.get_hazard_pointer();
            hazard.protect(head);

            // Double-check that head hasn't changed
            if bucket.load(Ordering::Acquire) != head {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let (prev, current) = self.find_entry(head, key);
            
            if current.is_null() {
                hazard.clear();
                self.metrics.record_failure();
                return Err(MapError::KeyNotFound);
            }

            let entry = unsafe { &*current };
            
            // Mark for deletion first
            if !entry.mark_deleted() {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let old_value = entry.value.clone();
            let next = entry.next.load(Ordering::Acquire);

            // Remove from chain
            let cas_result = if prev.is_null() {
                // Removing head of bucket
                bucket.compare_exchange_weak(
                    current,
                    next,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
            } else {
                // Removing from middle of chain
                let prev_entry = unsafe { &*prev };
                prev_entry.next.compare_exchange_weak(
                    current,
                    next,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
            };

            self.metrics.record_cas(cas_result.is_ok());

            if cas_result.is_ok() {
                self.size.fetch_sub(1, Ordering::Relaxed);
                hazard.clear();
                // In a production system, we'd defer the actual deallocation
                unsafe { let _ = Box::from_raw(current); }
                self.metrics.record_success();
                return Ok(Some(old_value));
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn get(&self, key: &K) -> Result<Option<V>, MapError> {
        let bucket_index = self.bucket_index(key);
        let bucket = &self.buckets[bucket_index];
        
        let head = bucket.load(Ordering::Acquire);
        let hazard = self.get_hazard_pointer();
        hazard.protect(head);

        // Double-check that head hasn't changed
        if bucket.load(Ordering::Acquire) != head {
            hazard.clear();
            return self.get(key); // Retry
        }

        let (_, current) = self.find_entry(head, key);
        
        if current.is_null() {
            hazard.clear();
            self.metrics.record_failure();
            return Ok(None);
        }

        let entry = unsafe { &*current };
        if entry.is_deleted() {
            hazard.clear();
            self.metrics.record_failure();
            return Ok(None);
        }

        let value = entry.value.clone();
        hazard.clear();
        self.metrics.record_success();
        Ok(Some(value))
    }

    fn contains_key(&self, key: &K) -> bool {
        self.get(key).unwrap_or(None).is_some()
    }

    fn update<F>(&self, key: &K, updater: F) -> Result<Option<V>, MapError>
    where
        F: FnOnce(&V) -> V,
    {
        let bucket_index = self.bucket_index(key);
        let bucket = &self.buckets[bucket_index];
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(MapError::MaxRetriesExceeded { retries });
            }

            let head = bucket.load(Ordering::Acquire);
            let hazard = self.get_hazard_pointer();
            hazard.protect(head);

            // Double-check that head hasn't changed
            if bucket.load(Ordering::Acquire) != head {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let (_, current) = self.find_entry(head, key);
            
            if current.is_null() {
                hazard.clear();
                self.metrics.record_failure();
                return Err(MapError::KeyNotFound);
            }

            let entry = unsafe { &*current };
            if entry.is_deleted() {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let old_value = entry.value.clone();
            let new_value = updater(&old_value);
            
            // Update the value atomically
            unsafe {
                let entry_mut = &mut *(current as *mut MapEntry<K, V>);
                entry_mut.value = new_value;
            }

            hazard.clear();
            self.metrics.record_success();
            return Ok(Some(old_value));
        }
    }

    fn upsert<F>(&self, key: K, value: V, updater: F) -> Result<Option<V>, MapError>
    where
        F: FnOnce(&V) -> V,
    {
        match self.update(&key, updater) {
            Ok(old_value) => Ok(old_value),
            Err(MapError::KeyNotFound) => {
                self.insert(key, value)?;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn is_empty(&self) -> bool {
        self.size.load(Ordering::Relaxed) == 0
    }

    fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    fn stats(&self) -> MapStats {
        let size = self.len();
        MapStats {
            size,
            capacity: 0, // Unbounded
            inserts: self.metrics.successes.load(Ordering::Relaxed),
            removes: self.metrics.successes.load(Ordering::Relaxed),
            lookups: self.metrics.operations.load(Ordering::Relaxed),
            failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
            load_factor: size as f64 / self.bucket_count as f64,
        }
    }

    fn clear(&self) {
        for bucket in &self.buckets {
            let mut head = bucket.load(Ordering::Acquire);
            
            while !head.is_null() {
                let entry = unsafe { &*head };
                let next = entry.next.load(Ordering::Acquire);
                
                // Try to remove this entry
                if bucket.compare_exchange(
                    head,
                    next,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ).is_ok() {
                    unsafe { let _ = Box::from_raw(head); }
                    self.size.fetch_sub(1, Ordering::Relaxed);
                }
                
                head = bucket.load(Ordering::Acquire);
            }
        }
    }
}

impl<K, V> Drop for ConcurrentHashMap<K, V> 
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn drop(&mut self) {
        self.clear();
    }
}

unsafe impl<K, V> Send for ConcurrentHashMap<K, V> 
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{}

unsafe impl<K, V> Sync for ConcurrentHashMap<K, V> 
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{}

/// Skip list node for ordered map operations
pub struct SkipListNode<K, V> {
    /// The key
    pub key: Option<K>,
    /// The value
    pub value: Option<V>,
    /// Forward pointers (one for each level)
    pub forward: Vec<AtomicPtr<SkipListNode<K, V>>>,
    /// Marked for deletion flag
    pub deleted: AtomicUsize,
}

impl<K, V> SkipListNode<K, V> {
    /// Create a new skip list node
    pub fn new(key: Option<K>, value: Option<V>, level: usize) -> Self {
        Self {
            key,
            value,
            forward: (0..=level).map(|_| AtomicPtr::new(ptr::null_mut())).collect(),
            deleted: AtomicUsize::new(0),
        }
    }

    /// Create a sentinel node (head/tail)
    pub fn sentinel(level: usize) -> Self {
        Self::new(None, None, level)
    }

    /// Check if this node is marked for deletion
    pub fn is_deleted(&self) -> bool {
        self.deleted.load(Ordering::Acquire) != 0
    }

    /// Mark this node for deletion
    pub fn mark_deleted(&self) -> bool {
        self.deleted.compare_exchange(
            0,
            1,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ).is_ok()
    }
}

/// Lock-free skip list map for ordered operations
pub struct SkipListMap<K, V> 
where
    K: Ord + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Head sentinel node
    head: AtomicPtr<SkipListNode<K, V>>,
    /// Maximum level
    max_level: usize,
    /// Current level
    current_level: AtomicUsize,
    /// Current size
    size: AtomicUsize,
    /// Configuration
    config: LockFreeConfig,
    /// Performance metrics
    metrics: LockFreeMetrics,
    /// Hazard pointers for memory reclamation
    hazard_pointers: Vec<HazardPointer<SkipListNode<K, V>>>,
}

impl<K, V> SkipListMap<K, V>
where
    K: Ord + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new skip list map
    pub fn new(max_level: usize, config: LockFreeConfig) -> Self {
        let head = Box::into_raw(Box::new(SkipListNode::sentinel(max_level)));
        let num_threads = num_cpus::get() * 2;
        
        Self {
            head: AtomicPtr::new(head),
            max_level,
            current_level: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: (0..num_threads).map(|_| HazardPointer::new()).collect(),
        }
    }

    /// Generate random level for new node
    fn random_level(&self) -> usize {
        let mut level = 0;
        while level < self.max_level && fastrand::bool() {
            level += 1;
        }
        level
    }

    /// Find predecessors and successors for a key
    fn find(&self, key: &K) -> (Vec<*mut SkipListNode<K, V>>, Vec<*mut SkipListNode<K, V>>) {
        let mut preds = vec![ptr::null_mut(); self.max_level + 1];
        let mut succs = vec![ptr::null_mut(); self.max_level + 1];
        
        let head = self.head.load(Ordering::Acquire);
        let mut pred = head;
        
        for level in (0..=self.current_level.load(Ordering::Relaxed)).rev() {
            let mut curr = unsafe { (*pred).forward[level].load(Ordering::Acquire) };
            
            while !curr.is_null() {
                let curr_node = unsafe { &*curr };
                
                if curr_node.is_deleted() {
                    // Skip deleted nodes
                    curr = curr_node.forward[level].load(Ordering::Acquire);
                    continue;
                }
                
                if let Some(ref curr_key) = curr_node.key {
                    match curr_key.cmp(key) {
                        CmpOrdering::Less => {
                            pred = curr;
                            curr = curr_node.forward[level].load(Ordering::Acquire);
                        }
                        _ => break,
                    }
                } else {
                    break;
                }
            }
            
            preds[level] = pred;
            succs[level] = curr;
        }
        
        (preds, succs)
    }
}

impl<K, V> LockFreeMap<K, V> for SkipListMap<K, V>
where
    K: Ord + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn insert(&self, key: K, value: V) -> Result<Option<V>, MapError> {
        let level = self.random_level();
        let new_node = Box::into_raw(Box::new(SkipListNode::new(Some(key.clone()), Some(value), level)));
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                unsafe { let _ = Box::from_raw(new_node); }
                self.metrics.record_failure();
                return Err(MapError::MaxRetriesExceeded { retries });
            }

            let (preds, succs) = self.find(&key);
            
            // Check if key already exists
            if !succs[0].is_null() {
                let succ_node = unsafe { &*succs[0] };
                if let Some(ref succ_key) = succ_node.key {
                    if succ_key == &key && !succ_node.is_deleted() {
                        let old_value = succ_node.value.clone();
                        unsafe {
                            let node_mut = &mut *(succs[0] as *mut SkipListNode<K, V>);
                            node_mut.value = Some((*new_node).value.as_ref().unwrap().clone());
                        }
                        unsafe { let _ = Box::from_raw(new_node); }
                        self.metrics.record_success();
                        return Ok(old_value);
                    }
                }
            }

            // Link new node
            let mut valid = true;
            for i in 0..=level {
                unsafe {
                    (*new_node).forward[i].store(succs[i], Ordering::Relaxed);
                }
                
                if preds[i].is_null() {
                    valid = false;
                    break;
                }
                
                let pred_node = unsafe { &*preds[i] };
                let cas_result = pred_node.forward[i].compare_exchange_weak(
                    succs[i],
                    new_node,
                    Ordering::Release,
                    Ordering::Relaxed,
                );
                
                self.metrics.record_cas(cas_result.is_ok());
                
                if cas_result.is_err() {
                    valid = false;
                    break;
                }
            }

            if valid {
                // Update current level if necessary
                if level > self.current_level.load(Ordering::Relaxed) {
                    self.current_level.store(level, Ordering::Relaxed);
                }
                
                self.size.fetch_add(1, Ordering::Relaxed);
                self.metrics.record_success();
                return Ok(None);
            }

            // Cleanup partial links on failure
            for i in 0..=level {
                let pred_node = unsafe { &*preds[i] };
                let _ = pred_node.forward[i].compare_exchange(
                    new_node,
                    succs[i],
                    Ordering::Release,
                    Ordering::Relaxed,
                );
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn remove(&self, key: &K) -> Result<Option<V>, MapError> {
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(MapError::MaxRetriesExceeded { retries });
            }

            let (preds, succs) = self.find(key);
            
            if succs[0].is_null() {
                self.metrics.record_failure();
                return Err(MapError::KeyNotFound);
            }

            let node_to_remove = succs[0];
            let node = unsafe { &*node_to_remove };
            
            if let Some(ref node_key) = node.key {
                if node_key != key || node.is_deleted() {
                    retries += 1;
                    self.config.backoff_strategy.backoff_blocking(retries);
                    continue;
                }
            } else {
                self.metrics.record_failure();
                return Err(MapError::KeyNotFound);
            }

            // Mark for deletion
            if !node.mark_deleted() {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let old_value = node.value.clone();
            
            // Remove from all levels
            let mut valid = true;
            for level in 0..node.forward.len() {
                if preds[level].is_null() {
                    valid = false;
                    break;
                }
                
                let pred_node = unsafe { &*preds[level] };
                let next = node.forward[level].load(Ordering::Acquire);
                
                let cas_result = pred_node.forward[level].compare_exchange_weak(
                    node_to_remove,
                    next,
                    Ordering::Release,
                    Ordering::Relaxed,
                );
                
                self.metrics.record_cas(cas_result.is_ok());
                
                if cas_result.is_err() {
                    valid = false;
                    break;
                }
            }

            if valid {
                self.size.fetch_sub(1, Ordering::Relaxed);
                // In a production system, we'd defer the actual deallocation
                unsafe { let _ = Box::from_raw(node_to_remove); }
                self.metrics.record_success();
                return Ok(old_value);
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn get(&self, key: &K) -> Result<Option<V>, MapError> {
        let (_, succs) = self.find(key);
        
        if succs[0].is_null() {
            self.metrics.record_failure();
            return Ok(None);
        }

        let node = unsafe { &*succs[0] };
        
        if let Some(ref node_key) = node.key {
            if node_key == key && !node.is_deleted() {
                let value = node.value.clone();
                self.metrics.record_success();
                return Ok(value);
            }
        }

        self.metrics.record_failure();
        Ok(None)
    }

    fn contains_key(&self, key: &K) -> bool {
        self.get(key).unwrap_or(None).is_some()
    }

    fn update<F>(&self, key: &K, updater: F) -> Result<Option<V>, MapError>
    where
        F: FnOnce(&V) -> V,
    {
        let (_, succs) = self.find(key);
        
        if succs[0].is_null() {
            self.metrics.record_failure();
            return Err(MapError::KeyNotFound);
        }

        let node = unsafe { &*succs[0] };
        
        if let Some(ref node_key) = node.key {
            if node_key == key && !node.is_deleted() {
                if let Some(ref old_value) = node.value {
                    let new_value = updater(old_value);
                    let old_value_clone = old_value.clone();
                    
                    unsafe {
                        let node_mut = &mut *(succs[0] as *mut SkipListNode<K, V>);
                        node_mut.value = Some(new_value);
                    }
                    
                    self.metrics.record_success();
                    return Ok(Some(old_value_clone));
                }
            }
        }

        self.metrics.record_failure();
        Err(MapError::KeyNotFound)
    }

    fn upsert<F>(&self, key: K, value: V, updater: F) -> Result<Option<V>, MapError>
    where
        F: FnOnce(&V) -> V,
    {
        match self.update(&key, updater) {
            Ok(old_value) => Ok(old_value),
            Err(MapError::KeyNotFound) => {
                self.insert(key, value)?;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn is_empty(&self) -> bool {
        self.size.load(Ordering::Relaxed) == 0
    }

    fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    fn stats(&self) -> MapStats {
        MapStats {
            size: self.len(),
            capacity: 0, // Unbounded
            inserts: self.metrics.successes.load(Ordering::Relaxed),
            removes: self.metrics.successes.load(Ordering::Relaxed),
            lookups: self.metrics.operations.load(Ordering::Relaxed),
            failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
            load_factor: 0.0, // Not applicable to skip lists
        }
    }

    fn clear(&self) {
        // Simple implementation: keep removing until empty
        // In production, you'd want a more efficient bulk clear
        while !self.is_empty() {
            let head = self.head.load(Ordering::Acquire);
            let first = unsafe { (*head).forward[0].load(Ordering::Acquire) };
            
            if !first.is_null() {
                let first_node = unsafe { &*first };
                if let Some(ref key) = first_node.key {
                    let _ = self.remove(key);
                }
            }
        }
    }
}

impl<K, V> Drop for SkipListMap<K, V> 
where
    K: Ord + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn drop(&mut self) {
        self.clear();
        
        // Clean up head node
        let head = self.head.load(Ordering::Acquire);
        if !head.is_null() {
            unsafe {
                let _ = Box::from_raw(head);
            }
        }
    }
}

unsafe impl<K, V> Send for SkipListMap<K, V> 
where
    K: Ord + Clone + Send + Sync,
    V: Clone + Send + Sync,
{}

unsafe impl<K, V> Sync for SkipListMap<K, V> 
where
    K: Ord + Clone + Send + Sync,
    V: Clone + Send + Sync,
{}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_hashmap_basic_operations() {
        let map = ConcurrentHashMap::new(16, LockFreeConfig::default());
        
        // Test insert
        assert!(map.insert("key1".to_string(), "value1".to_string()).unwrap().is_none());
        assert!(map.insert("key2".to_string(), "value2".to_string()).unwrap().is_none());
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        
        // Test get
        assert_eq!(map.get(&"key1".to_string()).unwrap(), Some("value1".to_string()));
        assert_eq!(map.get(&"key2".to_string()).unwrap(), Some("value2".to_string()));
        assert_eq!(map.get(&"nonexistent".to_string()).unwrap(), None);
        
        // Test update
        let old_value = map.insert("key1".to_string(), "new_value1".to_string()).unwrap();
        assert_eq!(old_value, Some("value1".to_string()));
        assert_eq!(map.get(&"key1".to_string()).unwrap(), Some("new_value1".to_string()));
        
        // Test remove
        assert_eq!(map.remove(&"key1".to_string()).unwrap(), Some("new_value1".to_string()));
        assert_eq!(map.get(&"key1".to_string()).unwrap(), None);
        assert_eq!(map.len(), 1);
        
        // Test contains_key
        assert!(map.contains_key(&"key2".to_string()));
        assert!(!map.contains_key(&"key1".to_string()));
    }

    #[test]
    fn test_skiplist_basic_operations() {
        let map = SkipListMap::new(16, LockFreeConfig::default());
        
        // Test insert
        assert!(map.insert(1, "value1".to_string()).unwrap().is_none());
        assert!(map.insert(3, "value3".to_string()).unwrap().is_none());
        assert!(map.insert(2, "value2".to_string()).unwrap().is_none());
        assert_eq!(map.len(), 3);
        
        // Test get (should work regardless of insertion order)
        assert_eq!(map.get(&1).unwrap(), Some("value1".to_string()));
        assert_eq!(map.get(&2).unwrap(), Some("value2".to_string()));
        assert_eq!(map.get(&3).unwrap(), Some("value3".to_string()));
        
        // Test remove
        assert_eq!(map.remove(&2).unwrap(), Some("value2".to_string()));
        assert_eq!(map.get(&2).unwrap(), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_concurrent_operations() {
        let map = Arc::new(ConcurrentHashMap::new(64, LockFreeConfig::default()));
        let num_threads = 8;
        let operations_per_thread = 100;
        
        let mut handles = Vec::new();
        
        // Spawn threads that perform mixed operations
        for thread_id in 0..num_threads {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let key = format!("key_{}_{}", thread_id, i);
                    let value = format!("value_{}_{}", thread_id, i);
                    
                    // Insert
                    let _ = map_clone.insert(key.clone(), value.clone());
                    
                    // Get
                    let _ = map_clone.get(&key);
                    
                    // Update
                    let _ = map_clone.update(&key, |v| format!("updated_{}", v));
                    
                    // Remove (some items)
                    if i % 2 == 0 {
                        let _ = map_clone.remove(&key);
                    }
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify final state
        let stats = map.stats();
        assert!(stats.size <= num_threads * operations_per_thread);
        assert!(stats.inserts > 0);
        assert!(stats.lookups > 0);
    }

    #[test]
    fn test_map_stats() {
        let map = ConcurrentHashMap::new(8, LockFreeConfig::default());
        
        let initial_stats = map.stats();
        assert_eq!(initial_stats.size, 0);
        assert_eq!(initial_stats.load_factor, 0.0);
        
        map.insert("key1".to_string(), "value1".to_string()).unwrap();
        map.insert("key2".to_string(), "value2".to_string()).unwrap();
        
        let stats = map.stats();
        assert_eq!(stats.size, 2);
        assert_eq!(stats.load_factor, 2.0 / 8.0);
        assert!(stats.inserts >= 2);
    }

    #[test]
    fn test_upsert_operation() {
        let map = ConcurrentHashMap::new(8, LockFreeConfig::default());
        
        // Upsert on non-existent key (should insert)
        let result = map.upsert("key1".to_string(), "value1".to_string(), |v| format!("updated_{}", v));
        assert!(result.unwrap().is_none());
        assert_eq!(map.get(&"key1".to_string()).unwrap(), Some("value1".to_string()));
        
        // Upsert on existing key (should update)
        let result = map.upsert("key1".to_string(), "new_value".to_string(), |v| format!("updated_{}", v));
        assert_eq!(result.unwrap(), Some("value1".to_string()));
        assert_eq!(map.get(&"key1".to_string()).unwrap(), Some("updated_value1".to_string()));
    }
}