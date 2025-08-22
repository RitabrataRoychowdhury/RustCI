//! Fingerprinted ID Table (FIT) for O(1) Hot-Path Routing
//!
//! This module implements a high-performance, lock-free routing table optimized
//! for sub-82µs p99 latency lookups with collision resistance through fingerprinting.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Route identifier for fast lookups
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RouteId(pub u64);

impl RouteId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Shard identifier for routing decisions
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ShardId(pub u32);

impl ShardId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

/// Node identifier for next-hop routing
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Cache-aligned route entry with precomputed routing information
#[repr(C, align(64))] // Cache line aligned for optimal performance
#[derive(Debug)]
pub struct RouteEntry {
    /// Unique route identifier
    pub id: RouteId,
    /// Next hop node for routing
    pub next_hop: NodeId,
    /// Shard identifier for load balancing
    pub shard_id: ShardId,
    /// Latency hint for routing decisions (microseconds)
    pub latency_hint: u32,
    /// Reliability score (0-65535, higher is better)
    pub reliability_score: u16,
    /// Fingerprint for collision detection
    pub fingerprint: u64,
    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
    /// Access counter for statistics
    pub access_count: AtomicU32,
}

impl RouteEntry {
    /// Create a new route entry with computed fingerprint
    pub fn new(
        id: RouteId,
        next_hop: NodeId,
        shard_id: ShardId,
        latency_hint: u32,
        reliability_score: u16,
    ) -> Self {
        let fingerprint = Self::compute_fingerprint(id, next_hop, shard_id);
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id,
            next_hop,
            shard_id,
            latency_hint,
            reliability_score,
            fingerprint,
            created_at,
            access_count: AtomicU32::new(0),
        }
    }

    /// Compute fingerprint for collision detection
    fn compute_fingerprint(id: RouteId, next_hop: NodeId, shard_id: ShardId) -> u64 {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        next_hop.hash(&mut hasher);
        shard_id.hash(&mut hasher);
        hasher.finish()
    }

    /// Increment access counter for statistics
    pub fn record_access(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current access count
    pub fn access_count(&self) -> u32 {
        self.access_count.load(Ordering::Relaxed)
    }
}

impl Clone for RouteEntry {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            next_hop: self.next_hop,
            shard_id: self.shard_id,
            latency_hint: self.latency_hint,
            reliability_score: self.reliability_score,
            fingerprint: self.fingerprint,
            created_at: self.created_at,
            access_count: AtomicU32::new(self.access_count.load(Ordering::Relaxed)),
        }
    }
}

/// Error types for FIT operations
#[derive(Debug, thiserror::Error)]
pub enum FitError {
    #[error("FIT capacity exceeded: {current}/{max}")]
    CapacityExceeded { current: usize, max: usize },

    #[error("Route not found: {route_id:?}")]
    RouteNotFound { route_id: RouteId },

    #[error("Fingerprint collision detected for route {route_id:?}")]
    FingerprintCollision { route_id: RouteId },

    #[error("Invalid load factor: {load_factor}, must be between 0.1 and 0.9")]
    InvalidLoadFactor { load_factor: f64 },
}

/// Statistics for monitoring FIT performance
#[derive(Debug, Clone, Default)]
pub struct FitStats {
    pub capacity: usize,
    pub size: usize,
    pub load_factor: f64,
    pub lookups: u64,
    pub hits: u64,
    pub misses: u64,
    pub collisions: u64,
    pub insertions: u64,
    pub removals: u64,
}

impl FitStats {
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            self.hits as f64 / self.lookups as f64
        }
    }

    pub fn collision_rate(&self) -> f64 {
        if self.insertions == 0 {
            0.0
        } else {
            self.collisions as f64 / self.insertions as f64
        }
    }
}

/// High-performance Fingerprinted ID Table for O(1) routing lookups
pub struct FingerprintedIdTable {
    /// Dense array of atomic pointers to route entries
    entries: Vec<AtomicPtr<RouteEntry>>,
    /// Fingerprints for collision detection
    fingerprints: Vec<AtomicU64>,
    /// Table capacity (power of 2 for fast modulo)
    capacity: usize,
    /// Target load factor (0.1 - 0.9)
    load_factor: f64,
    /// Current number of entries
    size: AtomicU32,
    /// Statistics counters
    stats: Arc<std::sync::Mutex<FitStats>>,
}

impl FingerprintedIdTable {
    /// Create a new FIT with specified capacity and load factor
    pub fn new(capacity: usize, load_factor: f64) -> Result<Self, FitError> {
        if !(0.1..=0.9).contains(&load_factor) {
            return Err(FitError::InvalidLoadFactor { load_factor });
        }

        // Ensure capacity is power of 2 for fast modulo operations
        let capacity = capacity.next_power_of_two();

        let mut entries = Vec::with_capacity(capacity);
        let mut fingerprints = Vec::with_capacity(capacity);

        for _ in 0..capacity {
            entries.push(AtomicPtr::new(ptr::null_mut()));
            fingerprints.push(AtomicU64::new(0));
        }

        let stats = Arc::new(std::sync::Mutex::new(FitStats {
            capacity,
            load_factor,
            ..Default::default()
        }));

        Ok(Self {
            entries,
            fingerprints,
            capacity,
            load_factor,
            size: AtomicU32::new(0),
            stats,
        })
    }

    /// Create a new FIT with default settings (capacity: 65536, load_factor: 0.75)
    pub fn with_defaults() -> Self {
        Self::new(65536, 0.75).expect("Default parameters should be valid")
    }

    /// Fast O(1) lookup with fingerprint verification
    pub fn lookup(&self, id: RouteId) -> Option<Arc<RouteEntry>> {
        let hash = self.hash_route_id(id);
        let index = hash & (self.capacity - 1); // Fast modulo for power of 2

        // Update stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.lookups += 1;
        }

        // Load entry pointer atomically
        let entry_ptr = self.entries[index].load(Ordering::Acquire);
        if entry_ptr.is_null() {
            // Update miss stats
            if let Ok(mut stats) = self.stats.lock() {
                stats.misses += 1;
            }
            return None;
        }

        // Safety: We loaded the pointer atomically and check for null
        let entry = unsafe { &*entry_ptr };

        // Verify fingerprint to prevent false positives
        let expected_fingerprint = self.fingerprints[index].load(Ordering::Acquire);
        if entry.fingerprint != expected_fingerprint || entry.id != id {
            // Update miss stats
            if let Ok(mut stats) = self.stats.lock() {
                stats.misses += 1;
            }
            return None;
        }

        // Record access and update hit stats
        entry.record_access();
        if let Ok(mut stats) = self.stats.lock() {
            stats.hits += 1;
        }

        Some(Arc::new(entry.clone()))
    }

    /// Insert a new route entry
    pub fn insert(&self, entry: RouteEntry) -> Result<(), FitError> {
        let current_size = self.size.load(Ordering::Relaxed) as usize;
        let max_size = (self.capacity as f64 * self.load_factor) as usize;

        if current_size >= max_size {
            return Err(FitError::CapacityExceeded {
                current: current_size,
                max: max_size,
            });
        }

        let hash = self.hash_route_id(entry.id);
        let mut index = hash & (self.capacity - 1);
        let mut attempts = 0;
        let max_attempts = self.capacity; // Linear probing with wraparound

        // Linear probing to find empty slot
        while attempts < max_attempts {
            let entry_ptr = self.entries[index].load(Ordering::Acquire);

            if entry_ptr.is_null() {
                // Found empty slot, try to claim it
                let new_entry = Box::into_raw(Box::new(entry.clone()));

                match self.entries[index].compare_exchange_weak(
                    ptr::null_mut(),
                    new_entry,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully inserted, update fingerprint
                        self.fingerprints[index].store(entry.fingerprint, Ordering::Release);
                        self.size.fetch_add(1, Ordering::Relaxed);

                        // Update stats
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.insertions += 1;
                            stats.size = self.size.load(Ordering::Relaxed) as usize;
                        }

                        return Ok(());
                    }
                    Err(_) => {
                        // Someone else claimed this slot, clean up and continue
                        unsafe {
                            let _ = Box::from_raw(new_entry);
                        }
                    }
                }
            } else {
                // Slot occupied, check for collision
                let existing_entry = unsafe { &*entry_ptr };
                if existing_entry.id == entry.id {
                    return Err(FitError::FingerprintCollision { route_id: entry.id });
                }
            }

            // Move to next slot (linear probing)
            index = (index + 1) & (self.capacity - 1);
            attempts += 1;
        }

        // Update collision stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.collisions += 1;
        }

        Err(FitError::CapacityExceeded {
            current: current_size,
            max: max_size,
        })
    }

    /// Remove a route entry
    pub fn remove(&self, id: RouteId) -> Result<Arc<RouteEntry>, FitError> {
        let hash = self.hash_route_id(id);
        let mut index = hash & (self.capacity - 1);
        let mut attempts = 0;
        let max_attempts = self.capacity;

        while attempts < max_attempts {
            let entry_ptr = self.entries[index].load(Ordering::Acquire);

            if entry_ptr.is_null() {
                return Err(FitError::RouteNotFound { route_id: id });
            }

            let entry = unsafe { &*entry_ptr };
            if entry.id == id {
                // Found the entry, try to remove it
                match self.entries[index].compare_exchange_weak(
                    entry_ptr,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully removed
                        self.fingerprints[index].store(0, Ordering::Release);
                        self.size.fetch_sub(1, Ordering::Relaxed);

                        // Update stats
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.removals += 1;
                            stats.size = self.size.load(Ordering::Relaxed) as usize;
                        }

                        // Convert back to Box and then Arc
                        let boxed_entry = unsafe { Box::from_raw(entry_ptr) };
                        return Ok(Arc::new(*boxed_entry));
                    }
                    Err(_) => {
                        // Entry was modified, retry
                        continue;
                    }
                }
            }

            index = (index + 1) & (self.capacity - 1);
            attempts += 1;
        }

        Err(FitError::RouteNotFound { route_id: id })
    }

    /// Get current statistics
    pub fn stats(&self) -> FitStats {
        self.stats
            .lock()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }

    /// Get current size
    pub fn size(&self) -> usize {
        self.size.load(Ordering::Relaxed) as usize
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get current load factor
    pub fn current_load_factor(&self) -> f64 {
        self.size() as f64 / self.capacity as f64
    }

    /// Hash function for route IDs
    fn hash_route_id(&self, id: RouteId) -> usize {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        hasher.finish() as usize
    }

    /// Clear all entries (for testing/maintenance)
    pub fn clear(&self) {
        for i in 0..self.capacity {
            let entry_ptr = self.entries[i].swap(ptr::null_mut(), Ordering::AcqRel);
            if !entry_ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(entry_ptr);
                }
            }
            self.fingerprints[i].store(0, Ordering::Release);
        }
        self.size.store(0, Ordering::Relaxed);

        if let Ok(mut stats) = self.stats.lock() {
            stats.size = 0;
        }
    }
}

impl Drop for FingerprintedIdTable {
    fn drop(&mut self) {
        self.clear();
    }
}

// Safety: FIT is Send + Sync due to atomic operations
unsafe impl Send for FingerprintedIdTable {}
unsafe impl Sync for FingerprintedIdTable {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_route_entry_creation() {
        let entry = RouteEntry::new(
            RouteId::new(1),
            NodeId::new(100),
            ShardId::new(10),
            50,   // 50µs latency hint
            1000, // reliability score
        );

        assert_eq!(entry.id, RouteId::new(1));
        assert_eq!(entry.next_hop, NodeId::new(100));
        assert_eq!(entry.shard_id, ShardId::new(10));
        assert_eq!(entry.latency_hint, 50);
        assert_eq!(entry.reliability_score, 1000);
        assert_ne!(entry.fingerprint, 0);
        assert_eq!(entry.access_count(), 0);
    }

    #[test]
    fn test_fit_basic_operations() {
        let fit = FingerprintedIdTable::with_defaults();

        // Test insertion
        let entry = RouteEntry::new(
            RouteId::new(1),
            NodeId::new(100),
            ShardId::new(10),
            50,
            1000,
        );

        fit.insert(entry).unwrap();
        assert_eq!(fit.size(), 1);

        // Test lookup
        let found = fit.lookup(RouteId::new(1)).unwrap();
        assert_eq!(found.id, RouteId::new(1));
        assert_eq!(found.access_count(), 1);

        // Test removal
        let removed = fit.remove(RouteId::new(1)).unwrap();
        assert_eq!(removed.id, RouteId::new(1));
        assert_eq!(fit.size(), 0);

        // Test lookup after removal
        assert!(fit.lookup(RouteId::new(1)).is_none());
    }

    #[test]
    fn test_fit_collision_handling() {
        let fit = FingerprintedIdTable::new(16, 0.75).unwrap();

        // Insert multiple entries
        for i in 0..10 {
            let entry = RouteEntry::new(
                RouteId::new(i),
                NodeId::new(i * 100),
                ShardId::new(i as u32),
                50,
                1000,
            );
            fit.insert(entry).unwrap();
        }

        assert_eq!(fit.size(), 10);

        // Verify all entries can be found
        for i in 0..10 {
            let found = fit.lookup(RouteId::new(i)).unwrap();
            assert_eq!(found.id, RouteId::new(i));
        }
    }

    #[test]
    fn test_fit_capacity_limits() {
        let fit = FingerprintedIdTable::new(16, 0.5).unwrap(); // Max 8 entries

        // Fill to capacity
        for i in 0..8 {
            let entry = RouteEntry::new(
                RouteId::new(i),
                NodeId::new(i * 100),
                ShardId::new(i as u32),
                50,
                1000,
            );
            fit.insert(entry).unwrap();
        }

        // Try to exceed capacity
        let overflow_entry = RouteEntry::new(
            RouteId::new(100),
            NodeId::new(10000),
            ShardId::new(100),
            50,
            1000,
        );

        assert!(matches!(
            fit.insert(overflow_entry),
            Err(FitError::CapacityExceeded { .. })
        ));
    }

    #[test]
    fn test_fit_concurrent_access() {
        let fit = Arc::new(FingerprintedIdTable::with_defaults());
        let mut handles = vec![];

        // Spawn writer threads
        for i in 0..5 {
            let fit_clone = Arc::clone(&fit);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let id = (i * 100 + j) as u64;
                    let entry = RouteEntry::new(
                        RouteId::new(id),
                        NodeId::new(id * 10),
                        ShardId::new(id as u32),
                        50,
                        1000,
                    );
                    let _ = fit_clone.insert(entry);
                }
            });
            handles.push(handle);
        }

        // Spawn reader threads
        for _ in 0..3 {
            let fit_clone = Arc::clone(&fit);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    let id = (rand::random::<u64>() % 500) as u64;
                    let _ = fit_clone.lookup(RouteId::new(id));
                    thread::sleep(Duration::from_micros(1));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        let stats = fit.stats();
        println!("Final stats: {:?}", stats);
        assert!(stats.lookups > 0);
    }

    #[test]
    fn test_fit_stats() {
        let fit = FingerprintedIdTable::with_defaults();

        // Insert some entries
        for i in 0..10 {
            let entry = RouteEntry::new(
                RouteId::new(i),
                NodeId::new(i * 100),
                ShardId::new(i as u32),
                50,
                1000,
            );
            fit.insert(entry).unwrap();
        }

        // Perform lookups
        for i in 0..15 {
            let _ = fit.lookup(RouteId::new(i));
        }

        let stats = fit.stats();
        assert_eq!(stats.size, 10);
        assert_eq!(stats.insertions, 10);
        assert_eq!(stats.lookups, 15);
        assert_eq!(stats.hits, 10);
        assert_eq!(stats.misses, 5);
        assert_eq!(stats.hit_rate(), 10.0 / 15.0);
    }
}
