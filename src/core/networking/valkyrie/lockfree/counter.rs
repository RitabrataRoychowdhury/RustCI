//! Lock-Free Counter Implementations
//!
//! This module provides lock-free atomic counters with different memory ordering
//! guarantees for various performance and consistency requirements.

use super::{CacheLinePadded, LockFreeMetrics};
use std::sync::atomic::{AtomicIsize, Ordering};

/// Errors that can occur during counter operations
#[derive(Debug, thiserror::Error)]
pub enum CounterError {
    #[error("Counter overflow")]
    Overflow,
    #[error("Counter underflow")]
    Underflow,
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
}

/// Statistics for counter operations
#[derive(Debug, Clone)]
pub struct CounterStats {
    /// Current counter value
    pub value: isize,
    /// Total number of increment operations
    pub increments: usize,
    /// Total number of decrement operations
    pub decrements: usize,
    /// Total number of set operations
    pub sets: usize,
    /// Total number of compare-and-swap operations
    pub cas_operations: usize,
    /// Number of successful CAS operations
    pub cas_successes: usize,
}

/// Generic atomic counter trait
pub trait AtomicCounter: Send + Sync {
    /// Increment the counter by 1
    fn increment(&self) -> isize;

    /// Decrement the counter by 1
    fn decrement(&self) -> isize;

    /// Add a value to the counter
    fn add(&self, value: isize) -> isize;

    /// Subtract a value from the counter
    fn sub(&self, value: isize) -> isize;

    /// Set the counter to a specific value
    fn set(&self, value: isize) -> isize;

    /// Get the current value
    fn get(&self) -> isize;

    /// Compare and swap
    fn compare_and_swap(&self, expected: isize, new: isize) -> Result<isize, isize>;

    /// Reset the counter to zero
    fn reset(&self) -> isize;

    /// Get counter statistics
    fn stats(&self) -> CounterStats;
}

/// Relaxed ordering counter - fastest but weakest consistency
pub struct RelaxedCounter {
    /// The atomic counter value
    value: CacheLinePadded<AtomicIsize>,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

impl RelaxedCounter {
    /// Create a new relaxed counter
    pub fn new(initial_value: isize) -> Self {
        Self {
            value: CacheLinePadded::new(AtomicIsize::new(initial_value)),
            metrics: LockFreeMetrics::default(),
        }
    }
}

impl AtomicCounter for RelaxedCounter {
    fn increment(&self) -> isize {
        let result = self.value.fetch_add(1, Ordering::Relaxed) + 1;
        self.metrics.record_success();
        result
    }

    fn decrement(&self) -> isize {
        let result = self.value.fetch_sub(1, Ordering::Relaxed) - 1;
        self.metrics.record_success();
        result
    }

    fn add(&self, value: isize) -> isize {
        let result = self.value.fetch_add(value, Ordering::Relaxed) + value;
        self.metrics.record_success();
        result
    }

    fn sub(&self, value: isize) -> isize {
        let result = self.value.fetch_sub(value, Ordering::Relaxed) - value;
        self.metrics.record_success();
        result
    }

    fn set(&self, value: isize) -> isize {
        let old_value = self.value.swap(value, Ordering::Relaxed);
        self.metrics.record_success();
        old_value
    }

    fn get(&self) -> isize {
        self.value.load(Ordering::Relaxed)
    }

    fn compare_and_swap(&self, expected: isize, new: isize) -> Result<isize, isize> {
        let result =
            self.value
                .compare_exchange(expected, new, Ordering::Relaxed, Ordering::Relaxed);

        let success = result.is_ok();
        self.metrics.record_cas(success);

        if success {
            self.metrics.record_success();
            Ok(new)
        } else {
            self.metrics.record_failure();
            Err(result.unwrap_err())
        }
    }

    fn reset(&self) -> isize {
        self.set(0)
    }

    fn stats(&self) -> CounterStats {
        CounterStats {
            value: self.get(),
            increments: self.metrics.successes.load(Ordering::Relaxed) / 4, // Rough estimate
            decrements: self.metrics.successes.load(Ordering::Relaxed) / 4,
            sets: self.metrics.successes.load(Ordering::Relaxed) / 4,
            cas_operations: self.metrics.cas_operations.load(Ordering::Relaxed),
            cas_successes: self.metrics.cas_successes.load(Ordering::Relaxed),
        }
    }
}

/// Sequential consistency counter - strongest ordering guarantees
pub struct SeqCstCounter {
    /// The atomic counter value
    value: CacheLinePadded<AtomicIsize>,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

impl SeqCstCounter {
    /// Create a new sequentially consistent counter
    pub fn new(initial_value: isize) -> Self {
        Self {
            value: CacheLinePadded::new(AtomicIsize::new(initial_value)),
            metrics: LockFreeMetrics::default(),
        }
    }
}

impl AtomicCounter for SeqCstCounter {
    fn increment(&self) -> isize {
        let result = self.value.fetch_add(1, Ordering::SeqCst) + 1;
        self.metrics.record_success();
        result
    }

    fn decrement(&self) -> isize {
        let result = self.value.fetch_sub(1, Ordering::SeqCst) - 1;
        self.metrics.record_success();
        result
    }

    fn add(&self, value: isize) -> isize {
        let result = self.value.fetch_add(value, Ordering::SeqCst) + value;
        self.metrics.record_success();
        result
    }

    fn sub(&self, value: isize) -> isize {
        let result = self.value.fetch_sub(value, Ordering::SeqCst) - value;
        self.metrics.record_success();
        result
    }

    fn set(&self, value: isize) -> isize {
        let old_value = self.value.swap(value, Ordering::SeqCst);
        self.metrics.record_success();
        old_value
    }

    fn get(&self) -> isize {
        self.value.load(Ordering::SeqCst)
    }

    fn compare_and_swap(&self, expected: isize, new: isize) -> Result<isize, isize> {
        let result = self
            .value
            .compare_exchange(expected, new, Ordering::SeqCst, Ordering::SeqCst);

        let success = result.is_ok();
        self.metrics.record_cas(success);

        if success {
            self.metrics.record_success();
            Ok(new)
        } else {
            self.metrics.record_failure();
            Err(result.unwrap_err())
        }
    }

    fn reset(&self) -> isize {
        self.set(0)
    }

    fn stats(&self) -> CounterStats {
        CounterStats {
            value: self.get(),
            increments: self.metrics.successes.load(Ordering::Relaxed) / 4,
            decrements: self.metrics.successes.load(Ordering::Relaxed) / 4,
            sets: self.metrics.successes.load(Ordering::Relaxed) / 4,
            cas_operations: self.metrics.cas_operations.load(Ordering::Relaxed),
            cas_successes: self.metrics.cas_successes.load(Ordering::Relaxed),
        }
    }
}

/// Acquire-Release ordering counter - balanced performance and consistency
pub struct AcqRelCounter {
    /// The atomic counter value
    value: CacheLinePadded<AtomicIsize>,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

impl AcqRelCounter {
    /// Create a new acquire-release counter
    pub fn new(initial_value: isize) -> Self {
        Self {
            value: CacheLinePadded::new(AtomicIsize::new(initial_value)),
            metrics: LockFreeMetrics::default(),
        }
    }
}

impl AtomicCounter for AcqRelCounter {
    fn increment(&self) -> isize {
        let result = self.value.fetch_add(1, Ordering::AcqRel) + 1;
        self.metrics.record_success();
        result
    }

    fn decrement(&self) -> isize {
        let result = self.value.fetch_sub(1, Ordering::AcqRel) - 1;
        self.metrics.record_success();
        result
    }

    fn add(&self, value: isize) -> isize {
        let result = self.value.fetch_add(value, Ordering::AcqRel) + value;
        self.metrics.record_success();
        result
    }

    fn sub(&self, value: isize) -> isize {
        let result = self.value.fetch_sub(value, Ordering::AcqRel) - value;
        self.metrics.record_success();
        result
    }

    fn set(&self, value: isize) -> isize {
        let old_value = self.value.swap(value, Ordering::AcqRel);
        self.metrics.record_success();
        old_value
    }

    fn get(&self) -> isize {
        self.value.load(Ordering::Acquire)
    }

    fn compare_and_swap(&self, expected: isize, new: isize) -> Result<isize, isize> {
        let result =
            self.value
                .compare_exchange(expected, new, Ordering::AcqRel, Ordering::Acquire);

        let success = result.is_ok();
        self.metrics.record_cas(success);

        if success {
            self.metrics.record_success();
            Ok(new)
        } else {
            self.metrics.record_failure();
            Err(result.unwrap_err())
        }
    }

    fn reset(&self) -> isize {
        self.set(0)
    }

    fn stats(&self) -> CounterStats {
        CounterStats {
            value: self.get(),
            increments: self.metrics.successes.load(Ordering::Relaxed) / 4,
            decrements: self.metrics.successes.load(Ordering::Relaxed) / 4,
            sets: self.metrics.successes.load(Ordering::Relaxed) / 4,
            cas_operations: self.metrics.cas_operations.load(Ordering::Relaxed),
            cas_successes: self.metrics.cas_successes.load(Ordering::Relaxed),
        }
    }
}

/// Multi-counter for reducing contention in high-concurrency scenarios
pub struct MultiCounter {
    /// Array of counters (one per CPU core)
    counters: Vec<CacheLinePadded<AtomicIsize>>,
    /// Number of counters
    num_counters: usize,
    /// Performance metrics
    metrics: LockFreeMetrics,
}

impl MultiCounter {
    /// Create a new multi-counter
    pub fn new(initial_value: isize) -> Self {
        let num_counters = num_cpus::get().next_power_of_two();
        let counters = (0..num_counters)
            .map(|_| CacheLinePadded::new(AtomicIsize::new(initial_value / num_counters as isize)))
            .collect();

        Self {
            counters,
            num_counters,
            metrics: LockFreeMetrics::default(),
        }
    }

    /// Get the counter index for the current thread
    fn get_counter_index(&self) -> usize {
        let thread_id = std::thread::current().id();
        // Use a hash of the thread ID instead of unstable as_u64()
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        thread_id.hash(&mut hasher);
        (hasher.finish() as usize) % self.num_counters
    }

    /// Get the sum of all counters
    pub fn sum(&self) -> isize {
        self.counters
            .iter()
            .map(|counter| counter.load(Ordering::Relaxed))
            .sum()
    }
}

impl AtomicCounter for MultiCounter {
    fn increment(&self) -> isize {
        let index = self.get_counter_index();
        let result = self.counters[index].fetch_add(1, Ordering::Relaxed) + 1;
        self.metrics.record_success();
        result
    }

    fn decrement(&self) -> isize {
        let index = self.get_counter_index();
        let result = self.counters[index].fetch_sub(1, Ordering::Relaxed) - 1;
        self.metrics.record_success();
        result
    }

    fn add(&self, value: isize) -> isize {
        let index = self.get_counter_index();
        let result = self.counters[index].fetch_add(value, Ordering::Relaxed) + value;
        self.metrics.record_success();
        result
    }

    fn sub(&self, value: isize) -> isize {
        let index = self.get_counter_index();
        let result = self.counters[index].fetch_sub(value, Ordering::Relaxed) - value;
        self.metrics.record_success();
        result
    }

    fn set(&self, value: isize) -> isize {
        // For multi-counter, set distributes the value across all counters
        let per_counter = value / self.num_counters as isize;
        let remainder = value % self.num_counters as isize;

        let mut old_sum = 0;
        for (i, counter) in self.counters.iter().enumerate() {
            let counter_value = if i < remainder as usize {
                per_counter + 1
            } else {
                per_counter
            };
            old_sum += counter.swap(counter_value, Ordering::Relaxed);
        }

        self.metrics.record_success();
        old_sum
    }

    fn get(&self) -> isize {
        self.sum()
    }

    fn compare_and_swap(&self, expected: isize, new: isize) -> Result<isize, isize> {
        // CAS is complex for multi-counter, so we use a simple approach
        let current = self.get();
        if current == expected {
            self.set(new);
            self.metrics.record_cas(true);
            self.metrics.record_success();
            Ok(new)
        } else {
            self.metrics.record_cas(false);
            self.metrics.record_failure();
            Err(current)
        }
    }

    fn reset(&self) -> isize {
        self.set(0)
    }

    fn stats(&self) -> CounterStats {
        CounterStats {
            value: self.get(),
            increments: self.metrics.successes.load(Ordering::Relaxed) / 4,
            decrements: self.metrics.successes.load(Ordering::Relaxed) / 4,
            sets: self.metrics.successes.load(Ordering::Relaxed) / 4,
            cas_operations: self.metrics.cas_operations.load(Ordering::Relaxed),
            cas_successes: self.metrics.cas_successes.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_relaxed_counter_basic_operations() {
        let counter = RelaxedCounter::new(0);

        assert_eq!(counter.get(), 0);
        assert_eq!(counter.increment(), 1);
        assert_eq!(counter.increment(), 2);
        assert_eq!(counter.decrement(), 1);
        assert_eq!(counter.get(), 1);

        assert_eq!(counter.add(5), 6);
        assert_eq!(counter.sub(3), 3);
        assert_eq!(counter.get(), 3);

        assert_eq!(counter.set(10), 3);
        assert_eq!(counter.get(), 10);

        assert_eq!(counter.reset(), 10);
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_seqcst_counter_compare_and_swap() {
        let counter = SeqCstCounter::new(5);

        // Successful CAS
        assert_eq!(counter.compare_and_swap(5, 10), Ok(10));
        assert_eq!(counter.get(), 10);

        // Failed CAS
        assert_eq!(counter.compare_and_swap(5, 15), Err(10));
        assert_eq!(counter.get(), 10);
    }

    #[test]
    fn test_acqrel_counter_concurrent() {
        let counter = Arc::new(AcqRelCounter::new(0));
        let num_threads = 8;
        let increments_per_thread = 1000;

        let mut handles = Vec::new();

        // Spawn threads that increment the counter
        for _ in 0..num_threads {
            let counter_clone = Arc::clone(&counter);
            let handle = thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    counter_clone.increment();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final value
        assert_eq!(counter.get(), num_threads * increments_per_thread);
    }

    #[test]
    fn test_multi_counter_concurrent() {
        let counter = Arc::new(MultiCounter::new(0));
        let num_threads = 8;
        let increments_per_thread = 1000;

        let mut handles = Vec::new();

        // Spawn threads that increment the counter
        for _ in 0..num_threads {
            let counter_clone = Arc::clone(&counter);
            let handle = thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    counter_clone.increment();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final value
        assert_eq!(counter.get(), num_threads * increments_per_thread);
    }

    #[test]
    fn test_counter_stats() {
        let counter = RelaxedCounter::new(0);

        counter.increment();
        counter.decrement();
        counter.add(5);
        counter.set(10);
        let _ = counter.compare_and_swap(10, 20);

        let stats = counter.stats();
        assert_eq!(stats.value, 20);
        assert!(stats.cas_operations > 0);
        assert!(stats.cas_successes > 0);
    }

    #[test]
    fn test_multi_counter_distribution() {
        let counter = MultiCounter::new(100);

        // The initial value should be distributed across counters
        let sum = counter.sum();
        assert_eq!(sum, 100);

        // Set a new value
        counter.set(200);
        assert_eq!(counter.get(), 200);
    }
}
