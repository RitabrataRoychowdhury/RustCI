//! Lock-Free Pool Implementations
//!
//! This module provides lock-free object pools for efficient memory management
//! and resource reuse in high-performance scenarios.

use super::{CacheLinePadded, HazardPointer, LockFreeConfig, LockFreeMetrics};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Errors that can occur during pool operations
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("Pool is empty")]
    Empty,
    #[error("Pool is full (capacity: {capacity})")]
    Full { capacity: usize },
    #[error("Operation failed after {retries} retries")]
    MaxRetriesExceeded { retries: usize },
    #[error("Memory allocation failed")]
    AllocationFailed,
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
}

/// Statistics for pool operations
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Current number of objects in the pool
    pub available: usize,
    /// Maximum capacity of the pool
    pub capacity: usize,
    /// Total number of allocations
    pub allocations: usize,
    /// Total number of deallocations
    pub deallocations: usize,
    /// Number of failed allocations
    pub allocation_failures: usize,
    /// Number of failed deallocations
    pub deallocation_failures: usize,
    /// Average contention level
    pub contention_level: f64,
    /// Pool utilization percentage
    pub utilization: f64,
}

/// Node in a lock-free pool
pub struct PoolNode<T> {
    /// The data stored in this node
    pub data: T,
    /// Pointer to the next node
    pub next: AtomicPtr<PoolNode<T>>,
}

impl<T> PoolNode<T> {
    /// Create a new pool node
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

/// Generic lock-free pool trait
pub trait LockFreePool<T>: Send + Sync {
    /// Allocate an object from the pool
    fn allocate(&self) -> Result<T, PoolError>;

    /// Deallocate an object back to the pool
    fn deallocate(&self, item: T) -> Result<(), PoolError>;

    /// Try to allocate without blocking
    fn try_allocate(&self) -> Result<T, PoolError>;

    /// Try to deallocate without blocking
    fn try_deallocate(&self, item: T) -> Result<(), PoolError>;

    /// Check if pool is empty
    fn is_empty(&self) -> bool;

    /// Check if pool is full
    fn is_full(&self) -> bool;

    /// Get current available count
    fn available(&self) -> usize;

    /// Get pool capacity
    fn capacity(&self) -> usize;

    /// Get pool statistics
    fn stats(&self) -> PoolStats;

    /// Clear all objects from the pool
    fn clear(&self);
}

/// Lock-free object pool using Treiber stack
pub struct ObjectPool<T>
where
    T: Send + Sync + 'static,
{
    /// Head of the free list (stack)
    head: CacheLinePadded<AtomicPtr<PoolNode<T>>>,
    /// Current available count
    available_count: AtomicUsize,
    /// Maximum capacity
    max_capacity: usize,
    /// Factory function for creating new objects
    factory: Box<dyn Fn() -> T + Send + Sync>,
    /// Configuration
    config: LockFreeConfig,
    /// Performance metrics
    metrics: LockFreeMetrics,
    /// Hazard pointers for memory reclamation
    hazard_pointers: Vec<HazardPointer<PoolNode<T>>>,
}

impl<T> ObjectPool<T>
where
    T: Send + Sync + 'static,
{
    /// Create a new object pool with a factory function
    pub fn new<F>(max_capacity: usize, factory: F, config: LockFreeConfig) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let num_threads = num_cpus::get() * 2;

        Self {
            head: CacheLinePadded::new(AtomicPtr::new(ptr::null_mut())),
            available_count: AtomicUsize::new(0),
            max_capacity,
            factory: Box::new(factory),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: (0..num_threads).map(|_| HazardPointer::new()).collect(),
        }
    }

    /// Create a new object pool with initial objects
    pub fn with_initial_objects<F>(
        initial_count: usize,
        max_capacity: usize,
        factory: F,
        config: LockFreeConfig,
    ) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let pool = Self::new(max_capacity, factory, config);

        // Pre-populate the pool
        for _ in 0..initial_count {
            let obj = (pool.factory)();
            let _ = pool.deallocate(obj);
        }

        pool
    }

    /// Get a hazard pointer for the current thread
    fn get_hazard_pointer(&self) -> &HazardPointer<PoolNode<T>> {
        let thread_id = std::thread::current().id();
        // Use a hash of the thread ID instead of unstable as_u64()
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let index = (hasher.finish() as usize) % self.hazard_pointers.len();
        &self.hazard_pointers[index]
    }

    /// Safely reclaim memory for a node
    fn reclaim_node(&self, node: *mut PoolNode<T>) {
        // Check if any hazard pointer is protecting this node
        let is_protected = self.hazard_pointers.iter().any(|hp| hp.is_protected(node));

        if !is_protected {
            unsafe {
                let _ = Box::from_raw(node);
            }
        }
        // If protected, we'll try to reclaim it later
    }
}

impl<T> LockFreePool<T> for ObjectPool<T>
where
    T: Send + Sync + 'static,
{
    fn allocate(&self) -> Result<T, PoolError> {
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                // If pool is empty, create a new object
                let obj = (self.factory)();
                self.metrics.record_success();
                return Ok(obj);
            }

            let head = self.head.load(Ordering::Acquire);
            if head.is_null() {
                // Pool is empty, create a new object
                let obj = (self.factory)();
                self.metrics.record_success();
                return Ok(obj);
            }

            let hazard = self.get_hazard_pointer();
            hazard.protect(head);

            // Double-check that head hasn't changed
            if self.head.load(Ordering::Acquire) != head {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let next = unsafe { (*head).next.load(Ordering::Acquire) };

            let cas_result =
                self.head
                    .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed);

            self.metrics.record_cas(cas_result.is_ok());

            if cas_result.is_ok() {
                hazard.clear();
                let data = unsafe { Box::from_raw(head).data };
                self.available_count.fetch_sub(1, Ordering::Relaxed);
                self.metrics.record_success();
                return Ok(data);
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn deallocate(&self, item: T) -> Result<(), PoolError> {
        // Check if pool is at capacity
        if self.available_count.load(Ordering::Relaxed) >= self.max_capacity {
            self.metrics.record_failure();
            return Err(PoolError::Full {
                capacity: self.max_capacity,
            });
        }

        let new_node = Box::into_raw(Box::new(PoolNode::new(item)));
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                unsafe {
                    let _ = Box::from_raw(new_node);
                }
                self.metrics.record_failure();
                return Err(PoolError::MaxRetriesExceeded { retries });
            }

            let head = self.head.load(Ordering::Acquire);
            unsafe {
                (*new_node).next.store(head, Ordering::Relaxed);
            }

            let cas_result = self.head.compare_exchange_weak(
                head,
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
            );

            self.metrics.record_cas(cas_result.is_ok());

            if cas_result.is_ok() {
                self.available_count.fetch_add(1, Ordering::Relaxed);
                self.metrics.record_success();
                return Ok(());
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn try_allocate(&self) -> Result<T, PoolError> {
        let head = self.head.load(Ordering::Acquire);
        if head.is_null() {
            // Pool is empty, create a new object
            let obj = (self.factory)();
            self.metrics.record_success();
            return Ok(obj);
        }

        let hazard = self.get_hazard_pointer();
        hazard.protect(head);

        // Double-check that head hasn't changed
        if self.head.load(Ordering::Acquire) != head {
            hazard.clear();
            self.metrics.record_failure();
            return Err(PoolError::MaxRetriesExceeded { retries: 1 });
        }

        let next = unsafe { (*head).next.load(Ordering::Acquire) };

        let cas_result =
            self.head
                .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed);

        self.metrics.record_cas(cas_result.is_ok());

        if cas_result.is_ok() {
            hazard.clear();
            let data = unsafe { Box::from_raw(head).data };
            self.available_count.fetch_sub(1, Ordering::Relaxed);
            self.metrics.record_success();
            Ok(data)
        } else {
            hazard.clear();
            self.metrics.record_failure();
            Err(PoolError::MaxRetriesExceeded { retries: 1 })
        }
    }

    fn try_deallocate(&self, item: T) -> Result<(), PoolError> {
        // Check if pool is at capacity
        if self.available_count.load(Ordering::Relaxed) >= self.max_capacity {
            self.metrics.record_failure();
            return Err(PoolError::Full {
                capacity: self.max_capacity,
            });
        }

        let new_node = Box::into_raw(Box::new(PoolNode::new(item)));
        let head = self.head.load(Ordering::Acquire);

        unsafe {
            (*new_node).next.store(head, Ordering::Relaxed);
        }

        let cas_result =
            self.head
                .compare_exchange(head, new_node, Ordering::Release, Ordering::Relaxed);

        self.metrics.record_cas(cas_result.is_ok());

        if cas_result.is_ok() {
            self.available_count.fetch_add(1, Ordering::Relaxed);
            self.metrics.record_success();
            Ok(())
        } else {
            unsafe {
                let _ = Box::from_raw(new_node);
            }
            self.metrics.record_failure();
            Err(PoolError::MaxRetriesExceeded { retries: 1 })
        }
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire).is_null()
    }

    fn is_full(&self) -> bool {
        self.available_count.load(Ordering::Relaxed) >= self.max_capacity
    }

    fn available(&self) -> usize {
        self.available_count.load(Ordering::Relaxed)
    }

    fn capacity(&self) -> usize {
        self.max_capacity
    }

    fn stats(&self) -> PoolStats {
        let available = self.available();
        let capacity = self.capacity();

        PoolStats {
            available,
            capacity,
            allocations: self.metrics.successes.load(Ordering::Relaxed),
            deallocations: self.metrics.successes.load(Ordering::Relaxed),
            allocation_failures: self.metrics.failures.load(Ordering::Relaxed),
            deallocation_failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
            utilization: ((capacity - available) as f64 / capacity as f64) * 100.0,
        }
    }

    fn clear(&self) {
        while self.try_allocate().is_ok() {
            // Keep allocating (and dropping) until empty
        }
    }
}

impl<T> Drop for ObjectPool<T>
where
    T: Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.clear();
    }
}

unsafe impl<T: Send + Sync> Send for ObjectPool<T> {}
unsafe impl<T: Send + Sync> Sync for ObjectPool<T> {}

/// Lock-free buffer pool for byte arrays
pub struct BufferPool {
    /// Inner object pool
    inner: ObjectPool<Vec<u8>>,
    /// Buffer size
    buffer_size: usize,
}

impl BufferPool {
    /// Create a new buffer pool
    pub fn new(buffer_size: usize, max_capacity: usize, config: LockFreeConfig) -> Self {
        let factory = move || vec![0u8; buffer_size];

        Self {
            inner: ObjectPool::new(max_capacity, factory, config),
            buffer_size,
        }
    }

    /// Create a buffer pool with pre-allocated buffers
    pub fn with_initial_buffers(
        buffer_size: usize,
        initial_count: usize,
        max_capacity: usize,
        config: LockFreeConfig,
    ) -> Self {
        let factory = move || vec![0u8; buffer_size];

        Self {
            inner: ObjectPool::with_initial_objects(initial_count, max_capacity, factory, config),
            buffer_size,
        }
    }

    /// Get buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

impl LockFreePool<Vec<u8>> for BufferPool {
    fn allocate(&self) -> Result<Vec<u8>, PoolError> {
        let mut buffer = self.inner.allocate()?;
        buffer.clear(); // Clear any existing data
        buffer.resize(self.buffer_size, 0); // Ensure correct size
        Ok(buffer)
    }

    fn deallocate(&self, item: Vec<u8>) -> Result<(), PoolError> {
        self.inner.deallocate(item)
    }

    fn try_allocate(&self) -> Result<Vec<u8>, PoolError> {
        let mut buffer = self.inner.try_allocate()?;
        buffer.clear();
        buffer.resize(self.buffer_size, 0);
        Ok(buffer)
    }

    fn try_deallocate(&self, item: Vec<u8>) -> Result<(), PoolError> {
        self.inner.try_deallocate(item)
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn is_full(&self) -> bool {
        self.inner.is_full()
    }

    fn available(&self) -> usize {
        self.inner.available()
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn stats(&self) -> PoolStats {
        self.inner.stats()
    }

    fn clear(&self) {
        self.inner.clear()
    }
}

/// Lock-free connection pool for managing network connections
pub struct ConnectionPool<T>
where
    T: Send + Sync + 'static,
{
    /// Inner object pool
    inner: ObjectPool<T>,
}

impl<T> ConnectionPool<T>
where
    T: Send + Sync + 'static,
{
    /// Create a new connection pool
    pub fn new<F>(max_capacity: usize, factory: F, config: LockFreeConfig) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            inner: ObjectPool::new(max_capacity, factory, config),
        }
    }

    /// Create a connection pool with initial connections
    pub fn with_initial_connections<F>(
        initial_count: usize,
        max_capacity: usize,
        factory: F,
        config: LockFreeConfig,
    ) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            inner: ObjectPool::with_initial_objects(initial_count, max_capacity, factory, config),
        }
    }
}

impl<T> LockFreePool<T> for ConnectionPool<T>
where
    T: Send + Sync + 'static,
{
    fn allocate(&self) -> Result<T, PoolError> {
        self.inner.allocate()
    }

    fn deallocate(&self, item: T) -> Result<(), PoolError> {
        self.inner.deallocate(item)
    }

    fn try_allocate(&self) -> Result<T, PoolError> {
        self.inner.try_allocate()
    }

    fn try_deallocate(&self, item: T) -> Result<(), PoolError> {
        self.inner.try_deallocate(item)
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn is_full(&self) -> bool {
        self.inner.is_full()
    }

    fn available(&self) -> usize {
        self.inner.available()
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn stats(&self) -> PoolStats {
        self.inner.stats()
    }

    fn clear(&self) {
        self.inner.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_object_pool_basic_operations() {
        let pool = ObjectPool::new(10, || String::from("test"), LockFreeConfig::default());

        // Pool should be empty initially
        assert!(pool.is_empty());
        assert_eq!(pool.available(), 0);

        // Allocate should create new objects
        let obj1 = pool.allocate().unwrap();
        assert_eq!(obj1, "test");

        // Deallocate should add to pool
        pool.deallocate(obj1).unwrap();
        assert_eq!(pool.available(), 1);
        assert!(!pool.is_empty());

        // Next allocation should reuse the object
        let obj2 = pool.allocate().unwrap();
        assert_eq!(obj2, "test");
        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn test_object_pool_capacity_limit() {
        let pool = ObjectPool::new(2, || 42i32, LockFreeConfig::default());

        // Fill the pool to capacity
        pool.deallocate(1).unwrap();
        pool.deallocate(2).unwrap();
        assert_eq!(pool.available(), 2);
        assert!(pool.is_full());

        // Should reject additional items
        assert!(matches!(pool.deallocate(3), Err(PoolError::Full { .. })));
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(1024, 5, LockFreeConfig::default());

        assert_eq!(pool.buffer_size(), 1024);

        let buffer = pool.allocate().unwrap();
        assert_eq!(buffer.len(), 1024);
        assert_eq!(buffer.capacity(), 1024);

        pool.deallocate(buffer).unwrap();
        assert_eq!(pool.available(), 1);
    }

    #[test]
    fn test_connection_pool() {
        #[derive(Debug, PartialEq)]
        struct MockConnection {
            id: usize,
        }

        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let pool = ConnectionPool::new(
            3,
            move || {
                let id = counter_clone.fetch_add(1, Ordering::Relaxed);
                MockConnection { id }
            },
            LockFreeConfig::default(),
        );

        let conn1 = pool.allocate().unwrap();
        let conn2 = pool.allocate().unwrap();

        pool.deallocate(conn1).unwrap();
        pool.deallocate(conn2).unwrap();

        assert_eq!(pool.available(), 2);
    }

    #[test]
    fn test_pool_concurrent_access() {
        let pool = Arc::new(ObjectPool::new(100, || 42i32, LockFreeConfig::default()));
        let num_threads = 8;
        let operations_per_thread = 1000;

        let mut handles = Vec::new();

        // Spawn threads that allocate and deallocate
        for _ in 0..num_threads {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    let obj = pool_clone.allocate().unwrap();
                    assert_eq!(obj, 42);
                    pool_clone.deallocate(obj).unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Pool should have some objects available
        let stats = pool.stats();
        assert!(stats.allocations > 0);
        assert!(stats.deallocations > 0);
    }

    #[test]
    fn test_pool_with_initial_objects() {
        let pool = ObjectPool::with_initial_objects(
            5,
            10,
            || String::from("initial"),
            LockFreeConfig::default(),
        );

        assert_eq!(pool.available(), 5);
        assert!(!pool.is_empty());

        // All initial objects should be available
        for _ in 0..5 {
            let obj = pool.allocate().unwrap();
            assert_eq!(obj, "initial");
        }

        assert!(pool.is_empty());
    }

    #[test]
    fn test_pool_stats() {
        let pool = ObjectPool::new(5, || 100u32, LockFreeConfig::default());

        let initial_stats = pool.stats();
        assert_eq!(initial_stats.available, 0);
        assert_eq!(initial_stats.capacity, 5);
        assert_eq!(initial_stats.utilization, 0.0);

        // Perform some operations
        let obj1 = pool.allocate().unwrap();
        let obj2 = pool.allocate().unwrap();
        pool.deallocate(obj1).unwrap();

        let stats = pool.stats();
        assert_eq!(stats.available, 1);
        assert_eq!(stats.utilization, 80.0); // (5-1)/5 * 100
        assert!(stats.allocations >= 2);
        assert!(stats.deallocations >= 1);
    }
}
