//! Lock-Free Ring Buffer Implementations
//!
//! This module provides lock-free ring buffer implementations optimized for
//! high-throughput streaming scenarios in the Valkyrie Protocol.

use super::{LockFreeMetrics, LockFreeConfig, CacheLinePadded};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Errors that can occur during ring buffer operations
#[derive(Debug, thiserror::Error)]
pub enum RingBufferError {
    #[error("Ring buffer is empty")]
    Empty,
    #[error("Ring buffer is full (capacity: {capacity})")]
    Full { capacity: usize },
    #[error("Operation failed after {retries} retries")]
    MaxRetriesExceeded { retries: usize },
    #[error("Invalid capacity: {capacity} (must be power of 2)")]
    InvalidCapacity { capacity: usize },
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
}

/// Statistics for ring buffer operations
#[derive(Debug, Clone)]
pub struct RingBufferStats {
    /// Current number of elements in the buffer
    pub size: usize,
    /// Maximum capacity of the buffer
    pub capacity: usize,
    /// Total number of write operations
    pub writes: usize,
    /// Total number of read operations
    pub reads: usize,
    /// Number of failed write operations
    pub write_failures: usize,
    /// Number of failed read operations
    pub read_failures: usize,
    /// Average contention level
    pub contention_level: f64,
    /// Utilization percentage
    pub utilization: f64,
}

/// Generic lock-free ring buffer trait
pub trait LockFreeRingBuffer<T>: Send + Sync {
    /// Write an item to the buffer
    fn write(&self, item: T) -> Result<(), RingBufferError>;
    
    /// Read an item from the buffer
    fn read(&self) -> Result<T, RingBufferError>;
    
    /// Try to write without blocking
    fn try_write(&self, item: T) -> Result<(), RingBufferError>;
    
    /// Try to read without blocking
    fn try_read(&self) -> Result<T, RingBufferError>;
    
    /// Check if buffer is empty
    fn is_empty(&self) -> bool;
    
    /// Check if buffer is full
    fn is_full(&self) -> bool;
    
    /// Get current size (approximate)
    fn len(&self) -> usize;
    
    /// Get buffer capacity
    fn capacity(&self) -> usize;
    
    /// Get buffer statistics
    fn stats(&self) -> RingBufferStats;
    
    /// Clear all items from the buffer
    fn clear(&self);
}

/// Single-Producer Single-Consumer ring buffer (optimized for single-threaded scenarios)
pub struct SpscRingBuffer<T> {
    /// Ring buffer storage
    buffer: Vec<Option<T>>,
    /// Write index (producer)
    write_index: CacheLinePadded<AtomicUsize>,
    /// Read index (consumer)
    read_index: CacheLinePadded<AtomicUsize>,
    /// Capacity mask (capacity must be power of 2)
    mask: usize,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

impl<T> SpscRingBuffer<T> {
    /// Create a new SPSC ring buffer with given capacity (must be power of 2)
    pub fn new(capacity: usize) -> Result<Self, RingBufferError> {
        if !capacity.is_power_of_two() {
            return Err(RingBufferError::InvalidCapacity { capacity });
        }
        
        Ok(Self {
            buffer: (0..capacity).map(|_| None).collect(),
            write_index: CacheLinePadded::new(AtomicUsize::new(0)),
            read_index: CacheLinePadded::new(AtomicUsize::new(0)),
            mask: capacity - 1,
            metrics: LockFreeMetrics::default(),
        })
    }
}

impl<T: Send + Sync> LockFreeRingBuffer<T> for SpscRingBuffer<T> {
    fn write(&self, item: T) -> Result<(), RingBufferError> {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let next_write_idx = (write_idx + 1) & self.mask;
        
        // Check if buffer is full
        if next_write_idx == self.read_index.load(Ordering::Acquire) {
            self.metrics.record_failure();
            return Err(RingBufferError::Full { capacity: self.mask + 1 });
        }
        
        // Safety: We're the only producer, so this index is safe to write
        unsafe {
            let buffer_ptr = self.buffer.as_ptr() as *mut Option<T>;
            (*buffer_ptr.add(write_idx)) = Some(item);
        }
        
        // Update write index with release ordering to ensure visibility
        self.write_index.store(next_write_idx, Ordering::Release);
        self.metrics.record_success();
        Ok(())
    }

    fn read(&self) -> Result<T, RingBufferError> {
        let read_idx = self.read_index.load(Ordering::Relaxed);
        
        // Check if buffer is empty
        if read_idx == self.write_index.load(Ordering::Acquire) {
            self.metrics.record_failure();
            return Err(RingBufferError::Empty);
        }
        
        // Safety: We're the only consumer, so this index is safe to read
        let item = unsafe {
            let buffer_ptr = self.buffer.as_ptr() as *mut Option<T>;
            (*buffer_ptr.add(read_idx)).take()
        };
        
        if let Some(item) = item {
            // Update read index with release ordering
            self.read_index.store((read_idx + 1) & self.mask, Ordering::Release);
            self.metrics.record_success();
            Ok(item)
        } else {
            self.metrics.record_failure();
            Err(RingBufferError::Empty)
        }
    }

    fn try_write(&self, item: T) -> Result<(), RingBufferError> {
        self.write(item)
    }

    fn try_read(&self) -> Result<T, RingBufferError> {
        self.read()
    }

    fn is_empty(&self) -> bool {
        self.read_index.load(Ordering::Relaxed) == self.write_index.load(Ordering::Relaxed)
    }

    fn is_full(&self) -> bool {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let next_write_idx = (write_idx + 1) & self.mask;
        next_write_idx == self.read_index.load(Ordering::Relaxed)
    }

    fn len(&self) -> usize {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let read_idx = self.read_index.load(Ordering::Relaxed);
        (write_idx.wrapping_sub(read_idx)) & self.mask
    }

    fn capacity(&self) -> usize {
        self.mask + 1
    }

    fn stats(&self) -> RingBufferStats {
        let size = self.len();
        let capacity = self.capacity();
        
        RingBufferStats {
            size,
            capacity,
            writes: self.metrics.successes.load(Ordering::Relaxed),
            reads: self.metrics.successes.load(Ordering::Relaxed),
            write_failures: self.metrics.failures.load(Ordering::Relaxed),
            read_failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: 0.0, // No contention in SPSC
            utilization: (size as f64 / capacity as f64) * 100.0,
        }
    }

    fn clear(&self) {
        while self.try_read().is_ok() {
            // Keep reading until empty
        }
    }
}

/// Multi-Producer Multi-Consumer ring buffer using atomic operations
pub struct MpmcRingBuffer<T> {
    /// Ring buffer storage with atomic slots
    buffer: Vec<AtomicSlot<T>>,
    /// Write index (shared by all producers)
    write_index: CacheLinePadded<AtomicUsize>,
    /// Read index (shared by all consumers)
    read_index: CacheLinePadded<AtomicUsize>,
    /// Capacity mask (capacity must be power of 2)
    mask: usize,
    /// Configuration
    config: LockFreeConfig,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

/// Atomic slot for MPMC ring buffer
struct AtomicSlot<T> {
    /// Sequence number for ordering
    sequence: AtomicUsize,
    /// The data (when available)
    data: std::cell::UnsafeCell<Option<T>>,
}

impl<T> AtomicSlot<T> {
    fn new(sequence: usize) -> Self {
        Self {
            sequence: AtomicUsize::new(sequence),
            data: std::cell::UnsafeCell::new(None),
        }
    }
}

unsafe impl<T: Send + Sync> Send for AtomicSlot<T> {}
unsafe impl<T: Send + Sync> Sync for AtomicSlot<T> {}

impl<T> MpmcRingBuffer<T> {
    /// Create a new MPMC ring buffer with given capacity (must be power of 2)
    pub fn new(capacity: usize, config: LockFreeConfig) -> Result<Self, RingBufferError> {
        if !capacity.is_power_of_two() {
            return Err(RingBufferError::InvalidCapacity { capacity });
        }
        
        let buffer = (0..capacity)
            .map(|i| AtomicSlot::new(i))
            .collect();
        
        Ok(Self {
            buffer,
            write_index: CacheLinePadded::new(AtomicUsize::new(0)),
            read_index: CacheLinePadded::new(AtomicUsize::new(0)),
            mask: capacity - 1,
            config,
            metrics: LockFreeMetrics::default(),
        })
    }
}

impl<T: Send + Sync> LockFreeRingBuffer<T> for MpmcRingBuffer<T> {
    fn write(&self, item: T) -> Result<(), RingBufferError> {
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(RingBufferError::MaxRetriesExceeded { retries });
            }

            let write_idx = self.write_index.load(Ordering::Relaxed);
            let slot = &self.buffer[write_idx & self.mask];
            let sequence = slot.sequence.load(Ordering::Acquire);

            if sequence == write_idx {
                // Try to claim this slot
                let cas_result = self.write_index.compare_exchange_weak(
                    write_idx,
                    write_idx + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );

                self.metrics.record_cas(cas_result.is_ok());

                if cas_result.is_ok() {
                    // We claimed the slot, now write the data
                    unsafe {
                        *slot.data.get() = Some(item);
                    }
                    
                    // Update sequence to signal data is ready
                    slot.sequence.store(write_idx + 1, Ordering::Release);
                    self.metrics.record_success();
                    return Ok(());
                }
            } else if sequence < write_idx {
                // Buffer is full
                self.metrics.record_failure();
                return Err(RingBufferError::Full { capacity: self.mask + 1 });
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn read(&self) -> Result<T, RingBufferError> {
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(RingBufferError::MaxRetriesExceeded { retries });
            }

            let read_idx = self.read_index.load(Ordering::Relaxed);
            let slot = &self.buffer[read_idx & self.mask];
            let sequence = slot.sequence.load(Ordering::Acquire);

            if sequence == read_idx + 1 {
                // Try to claim this slot
                let cas_result = self.read_index.compare_exchange_weak(
                    read_idx,
                    read_idx + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );

                self.metrics.record_cas(cas_result.is_ok());

                if cas_result.is_ok() {
                    // We claimed the slot, now read the data
                    let item = unsafe {
                        (*slot.data.get()).take()
                    };
                    
                    if let Some(item) = item {
                        // Update sequence to signal slot is available
                        slot.sequence.store(read_idx + self.mask + 1, Ordering::Release);
                        self.metrics.record_success();
                        return Ok(item);
                    }
                }
            } else if sequence < read_idx + 1 {
                // Buffer is empty
                self.metrics.record_failure();
                return Err(RingBufferError::Empty);
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn try_write(&self, item: T) -> Result<(), RingBufferError> {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let slot = &self.buffer[write_idx & self.mask];
        let sequence = slot.sequence.load(Ordering::Acquire);

        if sequence == write_idx {
            // Try to claim this slot
            let cas_result = self.write_index.compare_exchange(
                write_idx,
                write_idx + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );

            self.metrics.record_cas(cas_result.is_ok());

            if cas_result.is_ok() {
                // We claimed the slot, now write the data
                unsafe {
                    *slot.data.get() = Some(item);
                }
                
                // Update sequence to signal data is ready
                slot.sequence.store(write_idx + 1, Ordering::Release);
                self.metrics.record_success();
                return Ok(());
            }
        }

        self.metrics.record_failure();
        if sequence < write_idx {
            Err(RingBufferError::Full { capacity: self.mask + 1 })
        } else {
            Err(RingBufferError::MaxRetriesExceeded { retries: 1 })
        }
    }

    fn try_read(&self) -> Result<T, RingBufferError> {
        let read_idx = self.read_index.load(Ordering::Relaxed);
        let slot = &self.buffer[read_idx & self.mask];
        let sequence = slot.sequence.load(Ordering::Acquire);

        if sequence == read_idx + 1 {
            // Try to claim this slot
            let cas_result = self.read_index.compare_exchange(
                read_idx,
                read_idx + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );

            self.metrics.record_cas(cas_result.is_ok());

            if cas_result.is_ok() {
                // We claimed the slot, now read the data
                let item = unsafe {
                    (*slot.data.get()).take()
                };
                
                if let Some(item) = item {
                    // Update sequence to signal slot is available
                    slot.sequence.store(read_idx + self.mask + 1, Ordering::Release);
                    self.metrics.record_success();
                    return Ok(item);
                }
            }
        }

        self.metrics.record_failure();
        if sequence < read_idx + 1 {
            Err(RingBufferError::Empty)
        } else {
            Err(RingBufferError::MaxRetriesExceeded { retries: 1 })
        }
    }

    fn is_empty(&self) -> bool {
        let read_idx = self.read_index.load(Ordering::Relaxed);
        let write_idx = self.write_index.load(Ordering::Relaxed);
        read_idx == write_idx
    }

    fn is_full(&self) -> bool {
        let read_idx = self.read_index.load(Ordering::Relaxed);
        let write_idx = self.write_index.load(Ordering::Relaxed);
        write_idx - read_idx >= self.mask + 1
    }

    fn len(&self) -> usize {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let read_idx = self.read_index.load(Ordering::Relaxed);
        write_idx.saturating_sub(read_idx)
    }

    fn capacity(&self) -> usize {
        self.mask + 1
    }

    fn stats(&self) -> RingBufferStats {
        let size = self.len();
        let capacity = self.capacity();
        
        RingBufferStats {
            size,
            capacity,
            writes: self.metrics.successes.load(Ordering::Relaxed),
            reads: self.metrics.successes.load(Ordering::Relaxed),
            write_failures: self.metrics.failures.load(Ordering::Relaxed),
            read_failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
            utilization: (size as f64 / capacity as f64) * 100.0,
        }
    }

    fn clear(&self) {
        while self.try_read().is_ok() {
            // Keep reading until empty
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_spsc_ring_buffer_basic_operations() {
        let buffer = SpscRingBuffer::new(8).unwrap();
        
        // Test write
        assert!(buffer.write(1).is_ok());
        assert!(buffer.write(2).is_ok());
        assert_eq!(buffer.len(), 2);
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());
        
        // Test read
        assert_eq!(buffer.read().unwrap(), 1);
        assert_eq!(buffer.read().unwrap(), 2);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        
        // Test empty read
        assert!(matches!(buffer.read(), Err(RingBufferError::Empty)));
    }

    #[test]
    fn test_spsc_ring_buffer_capacity() {
        let buffer = SpscRingBuffer::new(4).unwrap();
        
        // Fill to capacity
        for i in 0..3 {
            assert!(buffer.write(i).is_ok());
        }
        
        // Should be full now (capacity - 1 due to ring buffer implementation)
        assert!(matches!(buffer.write(99), Err(RingBufferError::Full { .. })));
        assert!(buffer.is_full());
    }

    #[test]
    fn test_mpmc_ring_buffer_concurrent() {
        let buffer = Arc::new(MpmcRingBuffer::new(64, LockFreeConfig::default()).unwrap());
        let num_producers = 4;
        let num_consumers = 2;
        let items_per_producer = 100;
        
        let mut handles = Vec::new();
        
        // Spawn producers
        for producer_id in 0..num_producers {
            let buffer_clone = Arc::clone(&buffer);
            let handle = thread::spawn(move || {
                for i in 0..items_per_producer {
                    let item = producer_id * items_per_producer + i;
                    while buffer_clone.write(item).is_err() {
                        thread::yield_now();
                    }
                }
            });
            handles.push(handle);
        }
        
        // Spawn consumers
        let consumed = Arc::new(AtomicUsize::new(0));
        for _ in 0..num_consumers {
            let buffer_clone = Arc::clone(&buffer);
            let consumed_clone = Arc::clone(&consumed);
            let handle = thread::spawn(move || {
                loop {
                    match buffer_clone.read() {
                        Ok(_) => {
                            consumed_clone.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(RingBufferError::Empty) => {
                            if consumed_clone.load(Ordering::Relaxed) >= num_producers * items_per_producer {
                                break;
                            }
                            thread::yield_now();
                        }
                        Err(_) => thread::yield_now(),
                    }
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify all items were consumed
        assert_eq!(consumed.load(Ordering::Relaxed), num_producers * items_per_producer);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_ring_buffer_stats() {
        let buffer = SpscRingBuffer::new(8).unwrap();
        
        let initial_stats = buffer.stats();
        assert_eq!(initial_stats.size, 0);
        assert_eq!(initial_stats.capacity, 8);
        assert_eq!(initial_stats.utilization, 0.0);
        
        buffer.write(1).unwrap();
        buffer.write(2).unwrap();
        
        let stats = buffer.stats();
        assert_eq!(stats.size, 2);
        assert_eq!(stats.utilization, 25.0); // 2/8 * 100
        assert!(stats.writes >= 2);
    }

    #[test]
    fn test_invalid_capacity() {
        // Non-power-of-2 capacity should fail
        assert!(matches!(
            SpscRingBuffer::<i32>::new(7),
            Err(RingBufferError::InvalidCapacity { capacity: 7 })
        ));
        
        assert!(matches!(
            MpmcRingBuffer::<i32>::new(15, LockFreeConfig::default()),
            Err(RingBufferError::InvalidCapacity { capacity: 15 })
        ));
    }
}