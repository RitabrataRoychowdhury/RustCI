//! Lock-Free Data Structures for High Concurrency
//!
//! This module provides lock-free data structures optimized for the Valkyrie Protocol's
//! high-performance requirements. These structures enable concurrent access without
//! traditional locking mechanisms, reducing contention and improving scalability.

pub mod queue;
pub mod stack;
pub mod map;
pub mod ring_buffer;
pub mod counter;
pub mod pool;
pub mod registry;

// Re-export main components
pub use queue::{
    LockFreeQueue, QueueStats, QueueError, QueueNode,
    MpscQueue, SpscQueue, MpmcQueue
};

pub use stack::{
    LockFreeStack, StackStats, StackError, StackNode,
    TreiberStack, EliminationStack
};

pub use map::{
    LockFreeMap, MapStats, MapError, MapEntry,
    ConcurrentHashMap, SkipListMap
};

pub use ring_buffer::{
    LockFreeRingBuffer, RingBufferStats, RingBufferError,
    SpscRingBuffer, MpmcRingBuffer
};

pub use counter::{
    AtomicCounter, CounterStats, CounterError,
    RelaxedCounter, SeqCstCounter, AcqRelCounter
};

pub use pool::{
    LockFreePool, PoolStats, PoolError, PoolNode,
    ObjectPool, BufferPool, ConnectionPool
};

pub use registry::{
    LockFreeRegistry, RegistryStats, RegistryError,
    NodeRegistry, ServiceRegistry, ConnectionRegistry
};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Memory ordering strategies for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOrdering {
    /// Relaxed ordering - no synchronization or ordering constraints
    Relaxed,
    /// Acquire-Release ordering - synchronizes with release operations
    AcquireRelease,
    /// Sequential consistency - strongest ordering guarantee
    SequentiallyConsistent,
}

impl From<MemoryOrdering> for Ordering {
    fn from(ordering: MemoryOrdering) -> Self {
        match ordering {
            MemoryOrdering::Relaxed => Ordering::Relaxed,
            MemoryOrdering::AcquireRelease => Ordering::AcqRel,
            MemoryOrdering::SequentiallyConsistent => Ordering::SeqCst,
        }
    }
}

/// Performance metrics for lock-free operations
#[derive(Debug)]
pub struct LockFreeMetrics {
    /// Total number of operations performed
    pub operations: AtomicUsize,
    /// Number of successful operations
    pub successes: AtomicUsize,
    /// Number of failed operations (retries, contention)
    pub failures: AtomicUsize,
    /// Number of CAS (Compare-And-Swap) operations
    pub cas_operations: AtomicUsize,
    /// Number of successful CAS operations
    pub cas_successes: AtomicUsize,
    /// Average contention level (0.0 to 1.0)
    pub contention_level: AtomicUsize, // Stored as fixed-point (multiply by 1000)
    /// Last operation timestamp
    pub last_operation: AtomicUsize, // Stored as nanoseconds since epoch
}

impl Default for LockFreeMetrics {
    fn default() -> Self {
        Self {
            operations: AtomicUsize::new(0),
            successes: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
            cas_operations: AtomicUsize::new(0),
            cas_successes: AtomicUsize::new(0),
            contention_level: AtomicUsize::new(0),
            last_operation: AtomicUsize::new(0),
        }
    }
}

impl LockFreeMetrics {
    /// Record a successful operation
    pub fn record_success(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp();
    }

    /// Record a failed operation
    pub fn record_failure(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.failures.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp();
    }

    /// Record a CAS operation
    pub fn record_cas(&self, success: bool) {
        self.cas_operations.fetch_add(1, Ordering::Relaxed);
        if success {
            self.cas_successes.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Update contention level based on retry count
    pub fn update_contention(&self, retry_count: usize) {
        // Simple exponential moving average for contention
        let current = self.contention_level.load(Ordering::Relaxed);
        let new_sample = (retry_count * 100).min(1000); // Cap at 100% contention
        let alpha = 100; // Weight for new sample (0.1 in fixed-point)
        let new_value = (current * (1000 - alpha) + new_sample * alpha) / 1000;
        self.contention_level.store(new_value, Ordering::Relaxed);
    }

    /// Update timestamp
    fn update_timestamp(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize;
        self.last_operation.store(now, Ordering::Relaxed);
    }

    /// Get success rate as percentage (0-100)
    pub fn success_rate(&self) -> f64 {
        let total = self.operations.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        let successes = self.successes.load(Ordering::Relaxed);
        (successes as f64 / total as f64) * 100.0
    }

    /// Get CAS success rate as percentage (0-100)
    pub fn cas_success_rate(&self) -> f64 {
        let total = self.cas_operations.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        let successes = self.cas_successes.load(Ordering::Relaxed);
        (successes as f64 / total as f64) * 100.0
    }

    /// Get contention level as percentage (0.0-100.0)
    pub fn contention_percentage(&self) -> f64 {
        self.contention_level.load(Ordering::Relaxed) as f64 / 10.0
    }
}

/// Configuration for lock-free data structures
#[derive(Debug, Clone)]
pub struct LockFreeConfig {
    /// Default memory ordering for operations
    pub memory_ordering: MemoryOrdering,
    /// Maximum number of retries for CAS operations
    pub max_retries: usize,
    /// Backoff strategy for retries
    pub backoff_strategy: BackoffStrategy,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Cache line size for padding (typically 64 bytes)
    pub cache_line_size: usize,
}

impl Default for LockFreeConfig {
    fn default() -> Self {
        Self {
            memory_ordering: MemoryOrdering::AcquireRelease,
            max_retries: 1000,
            backoff_strategy: BackoffStrategy::Exponential {
                initial_delay: Duration::from_nanos(1),
                max_delay: Duration::from_micros(100),
                multiplier: 2.0,
            },
            enable_metrics: true,
            cache_line_size: 64,
        }
    }
}

/// Backoff strategies for retry operations
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// No backoff - immediate retry
    None,
    /// Fixed delay between retries
    Fixed(Duration),
    /// Linear backoff - delay increases linearly
    Linear {
        initial_delay: Duration,
        increment: Duration,
        max_delay: Duration,
    },
    /// Exponential backoff - delay doubles each retry
    Exponential {
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
    },
    /// Random jitter backoff
    Jitter {
        base_delay: Duration,
        max_jitter: Duration,
    },
}

impl BackoffStrategy {
    /// Calculate delay for given retry attempt
    pub fn delay(&self, attempt: usize) -> Duration {
        match self {
            BackoffStrategy::None => Duration::ZERO,
            BackoffStrategy::Fixed(delay) => *delay,
            BackoffStrategy::Linear { initial_delay, increment, max_delay } => {
                let delay = *initial_delay + *increment * attempt as u32;
                delay.min(*max_delay)
            }
            BackoffStrategy::Exponential { initial_delay, max_delay, multiplier } => {
                let delay = initial_delay.as_nanos() as f64 * multiplier.powi(attempt as i32);
                Duration::from_nanos(delay as u64).min(*max_delay)
            }
            BackoffStrategy::Jitter { base_delay, max_jitter } => {
                let jitter = Duration::from_nanos(fastrand::u64(0..max_jitter.as_nanos() as u64));
                *base_delay + jitter
            }
        }
    }

    /// Apply backoff delay
    pub async fn backoff(&self, attempt: usize) {
        let delay = self.delay(attempt);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }

    /// Apply backoff delay (blocking version)
    pub fn backoff_blocking(&self, attempt: usize) {
        let delay = self.delay(attempt);
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
    }
}

/// Cache line padding to prevent false sharing
#[repr(align(64))]
#[derive(Debug)]
pub struct CacheLinePadded<T> {
    pub value: T,
    _padding: [u8; 0], // Zero-sized padding, alignment does the work
}

impl<T> CacheLinePadded<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            _padding: [],
        }
    }
}

impl<T> std::ops::Deref for CacheLinePadded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> std::ops::DerefMut for CacheLinePadded<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// Hazard pointer for memory reclamation in lock-free structures
pub struct HazardPointer<T> {
    pointer: AtomicUsize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> HazardPointer<T> {
    pub fn new() -> Self {
        Self {
            pointer: AtomicUsize::new(0),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Protect a pointer from reclamation
    pub fn protect(&self, ptr: *const T) {
        self.pointer.store(ptr as usize, Ordering::Release);
    }

    /// Clear protection
    pub fn clear(&self) {
        self.pointer.store(0, Ordering::Release);
    }

    /// Get protected pointer
    pub fn get(&self) -> *const T {
        self.pointer.load(Ordering::Acquire) as *const T
    }

    /// Check if pointer is protected
    pub fn is_protected(&self, ptr: *const T) -> bool {
        self.pointer.load(Ordering::Acquire) == ptr as usize
    }
}

impl<T> Default for HazardPointer<T> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<T> Send for HazardPointer<T> {}
unsafe impl<T> Sync for HazardPointer<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_ordering_conversion() {
        assert_eq!(Ordering::from(MemoryOrdering::Relaxed), Ordering::Relaxed);
        assert_eq!(Ordering::from(MemoryOrdering::AcquireRelease), Ordering::AcqRel);
        assert_eq!(Ordering::from(MemoryOrdering::SequentiallyConsistent), Ordering::SeqCst);
    }

    #[test]
    fn test_metrics_success_rate() {
        let metrics = LockFreeMetrics::default();
        
        // Initially 100% success rate (no operations)
        assert_eq!(metrics.success_rate(), 100.0);
        
        // Record some operations
        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();
        
        // Should be 66.67% success rate (2/3)
        let rate = metrics.success_rate();
        assert!((rate - 66.67).abs() < 0.1);
    }

    #[test]
    fn test_backoff_strategies() {
        let fixed = BackoffStrategy::Fixed(Duration::from_millis(10));
        assert_eq!(fixed.delay(0), Duration::from_millis(10));
        assert_eq!(fixed.delay(5), Duration::from_millis(10));

        let linear = BackoffStrategy::Linear {
            initial_delay: Duration::from_millis(1),
            increment: Duration::from_millis(2),
            max_delay: Duration::from_millis(10),
        };
        assert_eq!(linear.delay(0), Duration::from_millis(1));
        assert_eq!(linear.delay(1), Duration::from_millis(3));
        assert_eq!(linear.delay(2), Duration::from_millis(5));
        assert_eq!(linear.delay(10), Duration::from_millis(10)); // Capped at max
    }

    #[test]
    fn test_cache_line_padding() {
        let padded = CacheLinePadded::new(42u64);
        assert_eq!(*padded, 42);
        assert_eq!(std::mem::align_of::<CacheLinePadded<u64>>(), 64);
    }

    #[test]
    fn test_hazard_pointer() {
        let hazard = HazardPointer::<u64>::new();
        let value = 42u64;
        let ptr = &value as *const u64;
        
        hazard.protect(ptr);
        assert!(hazard.is_protected(ptr));
        assert_eq!(hazard.get(), ptr);
        
        hazard.clear();
        assert!(!hazard.is_protected(ptr));
        assert_eq!(hazard.get(), std::ptr::null());
    }
}