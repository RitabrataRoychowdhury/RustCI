//! Lock-Free Queue Implementations
//!
//! This module provides various lock-free queue implementations optimized for
//! different use cases in the Valkyrie Protocol.

use super::{CacheLinePadded, HazardPointer, LockFreeConfig, LockFreeMetrics};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Errors that can occur during queue operations
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is empty")]
    Empty,
    #[error("Queue is full (capacity: {capacity})")]
    Full { capacity: usize },
    #[error("Operation failed after {retries} retries")]
    MaxRetriesExceeded { retries: usize },
    #[error("Memory allocation failed")]
    AllocationFailed,
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
}

/// Statistics for queue operations
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// Current number of elements in the queue
    pub size: usize,
    /// Maximum capacity of the queue (0 for unbounded)
    pub capacity: usize,
    /// Total number of enqueue operations
    pub enqueues: usize,
    /// Total number of dequeue operations
    pub dequeues: usize,
    /// Number of failed enqueue operations
    pub enqueue_failures: usize,
    /// Number of failed dequeue operations
    pub dequeue_failures: usize,
    /// Average contention level
    pub contention_level: f64,
}

/// Node in a lock-free queue
pub struct QueueNode<T> {
    /// The data stored in this node
    pub data: Option<T>,
    /// Pointer to the next node
    pub next: AtomicPtr<QueueNode<T>>,
}

impl<T> QueueNode<T> {
    /// Create a new queue node with data
    pub fn new(data: T) -> Self {
        Self {
            data: Some(data),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Create a sentinel node (no data)
    pub fn sentinel() -> Self {
        Self {
            data: None,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

/// Generic lock-free queue trait
pub trait LockFreeQueue<T>: Send + Sync {
    /// Enqueue an item
    fn enqueue(&self, item: T) -> Result<(), QueueError>;

    /// Dequeue an item
    fn dequeue(&self) -> Result<T, QueueError>;

    /// Try to enqueue without blocking
    fn try_enqueue(&self, item: T) -> Result<(), QueueError>;

    /// Try to dequeue without blocking
    fn try_dequeue(&self) -> Result<T, QueueError>;

    /// Check if queue is empty
    fn is_empty(&self) -> bool;

    /// Get current size (approximate)
    fn len(&self) -> usize;

    /// Get queue statistics
    fn stats(&self) -> QueueStats;

    /// Clear all items from the queue
    fn clear(&self);
}

/// Michael & Scott lock-free queue (MPMC - Multi-Producer Multi-Consumer)
pub struct MpmcQueue<T>
where
    T: Send + Sync,
{
    /// Head pointer (for dequeue operations)
    head: CacheLinePadded<AtomicPtr<QueueNode<T>>>,
    /// Tail pointer (for enqueue operations)
    tail: CacheLinePadded<AtomicPtr<QueueNode<T>>>,
    /// Configuration
    config: LockFreeConfig,
    /// Performance metrics
    metrics: LockFreeMetrics,
    /// Hazard pointers for memory reclamation
    hazard_pointers: Vec<HazardPointer<QueueNode<T>>>,
}

impl<T> MpmcQueue<T>
where
    T: Send + Sync,
{
    /// Create a new MPMC queue
    pub fn new(config: LockFreeConfig) -> Self {
        let sentinel = Box::into_raw(Box::new(QueueNode::sentinel()));
        let num_threads = num_cpus::get() * 2; // Estimate for hazard pointers

        Self {
            head: CacheLinePadded::new(AtomicPtr::new(sentinel)),
            tail: CacheLinePadded::new(AtomicPtr::new(sentinel)),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: (0..num_threads).map(|_| HazardPointer::new()).collect(),
        }
    }

    /// Get a hazard pointer for the current thread
    fn get_hazard_pointer(&self) -> &HazardPointer<QueueNode<T>> {
        let thread_id = std::thread::current().id();
        // Use a hash of the thread ID instead of the unstable as_u64()
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let index = (hasher.finish() as usize) % self.hazard_pointers.len();
        &self.hazard_pointers[index]
    }

    /// Safely reclaim memory for a node
    fn reclaim_node(&self, node: *mut QueueNode<T>) {
        // Check if any hazard pointer is protecting this node
        let is_protected = self.hazard_pointers.iter().any(|hp| hp.is_protected(node));

        if !is_protected {
            unsafe {
                let _ = Box::from_raw(node);
            }
        }
        // If protected, we'll try to reclaim it later
        // In a production system, you'd add it to a retirement list
    }
}

impl<T: Send + Sync> LockFreeQueue<T> for MpmcQueue<T> {
    fn enqueue(&self, item: T) -> Result<(), QueueError> {
        let new_node = Box::into_raw(Box::new(QueueNode::new(item)));
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                unsafe {
                    let _ = Box::from_raw(new_node);
                }
                self.metrics.record_failure();
                return Err(QueueError::MaxRetriesExceeded { retries });
            }

            let tail = self.tail.load(Ordering::Acquire);
            let hazard = self.get_hazard_pointer();
            hazard.protect(tail);

            // Double-check that tail hasn't changed
            if self.tail.load(Ordering::Acquire) != tail {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let next = unsafe { (*tail).next.load(Ordering::Acquire) };

            if tail == self.tail.load(Ordering::Acquire) {
                if next.is_null() {
                    // Try to link new node at the end of the list
                    let cas_result = unsafe {
                        (*tail).next.compare_exchange_weak(
                            next,
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                    };

                    self.metrics.record_cas(cas_result.is_ok());

                    if cas_result.is_ok() {
                        // Try to swing tail to the new node
                        let _ = self.tail.compare_exchange_weak(
                            tail,
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                        hazard.clear();
                        self.metrics.record_success();
                        return Ok(());
                    }
                } else {
                    // Help advance tail
                    let _ = self.tail.compare_exchange_weak(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                }
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn dequeue(&self) -> Result<T, QueueError> {
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(QueueError::MaxRetriesExceeded { retries });
            }

            let head = self.head.load(Ordering::Acquire);
            let hazard = self.get_hazard_pointer();
            hazard.protect(head);

            // Double-check that head hasn't changed
            if self.head.load(Ordering::Acquire) != head {
                retries += 1;
                self.config.backoff_strategy.backoff_blocking(retries);
                continue;
            }

            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*head).next.load(Ordering::Acquire) };

            if head == self.head.load(Ordering::Acquire) {
                if head == tail {
                    if next.is_null() {
                        // Queue is empty
                        hazard.clear();
                        self.metrics.record_failure();
                        return Err(QueueError::Empty);
                    }
                    // Help advance tail
                    let _ = self.tail.compare_exchange_weak(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                } else {
                    if next.is_null() {
                        retries += 1;
                        self.config.backoff_strategy.backoff_blocking(retries);
                        continue;
                    }

                    // Read data before CAS
                    let data = unsafe { (*next).data.take() };

                    if data.is_none() {
                        retries += 1;
                        self.config.backoff_strategy.backoff_blocking(retries);
                        continue;
                    }

                    // Try to swing head to the next node
                    let cas_result = self.head.compare_exchange_weak(
                        head,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );

                    self.metrics.record_cas(cas_result.is_ok());

                    if cas_result.is_ok() {
                        hazard.clear();
                        self.reclaim_node(head);
                        self.metrics.record_success();
                        return Ok(data.unwrap());
                    }
                }
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn try_enqueue(&self, item: T) -> Result<(), QueueError> {
        // For lock-free queues, try_enqueue is the same as enqueue
        // but with a single attempt
        let mut config = self.config.clone();
        config.max_retries = 1;

        let temp_queue = MpmcQueue {
            head: CacheLinePadded::new(AtomicPtr::new(self.head.load(Ordering::Acquire))),
            tail: CacheLinePadded::new(AtomicPtr::new(self.tail.load(Ordering::Acquire))),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: Vec::new(),
        };

        temp_queue.enqueue(item)
    }

    fn try_dequeue(&self) -> Result<T, QueueError> {
        // For lock-free queues, try_dequeue is the same as dequeue
        // but with a single attempt
        let mut config = self.config.clone();
        config.max_retries = 1;

        let temp_queue = MpmcQueue {
            head: CacheLinePadded::new(AtomicPtr::new(self.head.load(Ordering::Acquire))),
            tail: CacheLinePadded::new(AtomicPtr::new(self.tail.load(Ordering::Acquire))),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: Vec::new(),
        };

        temp_queue.dequeue()
    }

    fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail && unsafe { (*head).next.load(Ordering::Acquire).is_null() }
    }

    fn len(&self) -> usize {
        // Approximate size by traversing the list
        // Note: This is expensive and not guaranteed to be accurate in concurrent scenarios
        let mut count = 0;
        let mut current = self.head.load(Ordering::Acquire);

        while !current.is_null() {
            let next = unsafe { (*current).next.load(Ordering::Acquire) };
            if next.is_null() {
                break;
            }
            current = next;
            count += 1;
        }

        count
    }

    fn stats(&self) -> QueueStats {
        QueueStats {
            size: self.len(),
            capacity: 0, // Unbounded
            enqueues: self.metrics.successes.load(Ordering::Relaxed),
            dequeues: self.metrics.successes.load(Ordering::Relaxed),
            enqueue_failures: self.metrics.failures.load(Ordering::Relaxed),
            dequeue_failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
        }
    }

    fn clear(&self) {
        while self.try_dequeue().is_ok() {
            // Keep dequeuing until empty
        }
    }
}

impl<T> Drop for MpmcQueue<T>
where
    T: Send + Sync,
{
    fn drop(&mut self) {
        // Clear all remaining items
        self.clear();

        // Clean up sentinel node
        let head = self.head.load(Ordering::Acquire);
        if !head.is_null() {
            unsafe {
                let _ = Box::from_raw(head);
            }
        }
    }
}

unsafe impl<T> Send for MpmcQueue<T> where T: Send + Sync {}

unsafe impl<T> Sync for MpmcQueue<T> where T: Send + Sync {}

/// Single-Producer Single-Consumer queue (optimized for single-threaded scenarios)
pub struct SpscQueue<T>
where
    T: Send + Sync,
{
    /// Ring buffer for data
    buffer: Vec<Option<T>>,
    /// Head index (for dequeue)
    head: CacheLinePadded<AtomicUsize>,
    /// Tail index (for enqueue)
    tail: CacheLinePadded<AtomicUsize>,
    /// Capacity mask (capacity must be power of 2)
    mask: usize,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

impl<T> SpscQueue<T>
where
    T: Send + Sync,
{
    /// Create a new SPSC queue with given capacity (must be power of 2)
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");

        Self {
            buffer: (0..capacity).map(|_| None).collect(),
            head: CacheLinePadded::new(AtomicUsize::new(0)),
            tail: CacheLinePadded::new(AtomicUsize::new(0)),
            mask: capacity - 1,
            metrics: LockFreeMetrics::default(),
        }
    }
}

impl<T: Send + Sync> LockFreeQueue<T> for SpscQueue<T> {
    fn enqueue(&self, item: T) -> Result<(), QueueError> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & self.mask;

        if next_tail == self.head.load(Ordering::Acquire) {
            self.metrics.record_failure();
            return Err(QueueError::Full {
                capacity: self.mask + 1,
            });
        }

        // Safety: We're the only producer, so this index is safe to write
        unsafe {
            let buffer_ptr = self.buffer.as_ptr() as *mut Option<T>;
            (*buffer_ptr.add(tail)) = Some(item);
        }

        self.tail.store(next_tail, Ordering::Release);
        self.metrics.record_success();
        Ok(())
    }

    fn dequeue(&self) -> Result<T, QueueError> {
        let head = self.head.load(Ordering::Relaxed);

        if head == self.tail.load(Ordering::Acquire) {
            self.metrics.record_failure();
            return Err(QueueError::Empty);
        }

        // Safety: We're the only consumer, so this index is safe to read
        let item = unsafe {
            let buffer_ptr = self.buffer.as_ptr() as *mut Option<T>;
            (*buffer_ptr.add(head)).take()
        };

        if let Some(item) = item {
            self.head.store((head + 1) & self.mask, Ordering::Release);
            self.metrics.record_success();
            Ok(item)
        } else {
            self.metrics.record_failure();
            Err(QueueError::Empty)
        }
    }

    fn try_enqueue(&self, item: T) -> Result<(), QueueError> {
        self.enqueue(item)
    }

    fn try_dequeue(&self) -> Result<T, QueueError> {
        self.dequeue()
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Relaxed)
    }

    fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        (tail.wrapping_sub(head)) & self.mask
    }

    fn stats(&self) -> QueueStats {
        QueueStats {
            size: self.len(),
            capacity: self.mask + 1,
            enqueues: self.metrics.successes.load(Ordering::Relaxed),
            dequeues: self.metrics.successes.load(Ordering::Relaxed),
            enqueue_failures: self.metrics.failures.load(Ordering::Relaxed),
            dequeue_failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: 0.0, // No contention in SPSC
        }
    }

    fn clear(&self) {
        while self.try_dequeue().is_ok() {
            // Keep dequeuing until empty
        }
    }
}

/// Multi-Producer Single-Consumer queue
pub struct MpscQueue<T>
where
    T: Send + Sync,
{
    /// Inner MPMC queue (we just restrict usage)
    inner: MpmcQueue<T>,
}

impl<T> MpscQueue<T>
where
    T: Send + Sync,
{
    pub fn new(config: LockFreeConfig) -> Self {
        Self {
            inner: MpmcQueue::new(config),
        }
    }
}

impl<T: Send + Sync> LockFreeQueue<T> for MpscQueue<T> {
    fn enqueue(&self, item: T) -> Result<(), QueueError> {
        self.inner.enqueue(item)
    }

    fn dequeue(&self) -> Result<T, QueueError> {
        self.inner.dequeue()
    }

    fn try_enqueue(&self, item: T) -> Result<(), QueueError> {
        self.inner.try_enqueue(item)
    }

    fn try_dequeue(&self) -> Result<T, QueueError> {
        self.inner.try_dequeue()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn stats(&self) -> QueueStats {
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
    fn test_spsc_queue_basic_operations() {
        let queue = SpscQueue::new(8);

        // Test enqueue
        assert!(queue.enqueue(1).is_ok());
        assert!(queue.enqueue(2).is_ok());
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());

        // Test dequeue
        assert_eq!(queue.dequeue().unwrap(), 1);
        assert_eq!(queue.dequeue().unwrap(), 2);
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());

        // Test empty dequeue
        assert!(matches!(queue.dequeue(), Err(QueueError::Empty)));
    }

    #[test]
    fn test_spsc_queue_capacity() {
        let queue = SpscQueue::new(4);

        // Fill to capacity
        for i in 0..3 {
            assert!(queue.enqueue(i).is_ok());
        }

        // Should be full now (capacity - 1 due to ring buffer implementation)
        assert!(matches!(queue.enqueue(99), Err(QueueError::Full { .. })));
    }

    #[test]
    fn test_mpmc_queue_concurrent() {
        let queue = Arc::new(MpmcQueue::new(LockFreeConfig::default()));
        let num_producers = 4;
        let num_consumers = 2;
        let items_per_producer = 100;

        let mut handles = Vec::new();

        // Spawn producers
        for producer_id in 0..num_producers {
            let queue_clone = Arc::clone(&queue);
            let handle = thread::spawn(move || {
                for i in 0..items_per_producer {
                    let item = producer_id * items_per_producer + i;
                    while queue_clone.enqueue(item).is_err() {
                        thread::yield_now();
                    }
                }
            });
            handles.push(handle);
        }

        // Spawn consumers
        let consumed = Arc::new(AtomicUsize::new(0));
        for _ in 0..num_consumers {
            let queue_clone = Arc::clone(&queue);
            let consumed_clone = Arc::clone(&consumed);
            let handle = thread::spawn(move || loop {
                match queue_clone.dequeue() {
                    Ok(_) => {
                        consumed_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(QueueError::Empty) => {
                        if consumed_clone.load(Ordering::Relaxed)
                            >= num_producers * items_per_producer
                        {
                            break;
                        }
                        thread::yield_now();
                    }
                    Err(_) => thread::yield_now(),
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were consumed
        assert_eq!(
            consumed.load(Ordering::Relaxed),
            num_producers * items_per_producer
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_stats() {
        let queue = SpscQueue::new(8);

        let initial_stats = queue.stats();
        assert_eq!(initial_stats.size, 0);
        assert_eq!(initial_stats.capacity, 8);

        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();

        let stats = queue.stats();
        assert_eq!(stats.size, 2);
        assert!(stats.enqueues >= 2);
    }
}
