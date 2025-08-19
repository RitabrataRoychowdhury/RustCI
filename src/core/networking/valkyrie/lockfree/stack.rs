//! Lock-Free Stack Implementations
//!
//! This module provides lock-free stack implementations using the Treiber stack algorithm
//! and elimination-based optimizations for high contention scenarios.

use super::{CacheLinePadded, HazardPointer, LockFreeConfig, LockFreeMetrics};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Errors that can occur during stack operations
#[derive(Debug, thiserror::Error)]
pub enum StackError {
    #[error("Stack is empty")]
    Empty,
    #[error("Stack is full (capacity: {capacity})")]
    Full { capacity: usize },
    #[error("Operation failed after {retries} retries")]
    MaxRetriesExceeded { retries: usize },
    #[error("Memory allocation failed")]
    AllocationFailed,
    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },
}

/// Statistics for stack operations
#[derive(Debug, Clone)]
pub struct StackStats {
    /// Current number of elements in the stack
    pub size: usize,
    /// Maximum capacity of the stack (0 for unbounded)
    pub capacity: usize,
    /// Total number of push operations
    pub pushes: usize,
    /// Total number of pop operations
    pub pops: usize,
    /// Number of failed push operations
    pub push_failures: usize,
    /// Number of failed pop operations
    pub pop_failures: usize,
    /// Average contention level
    pub contention_level: f64,
    /// Number of elimination operations (if supported)
    pub eliminations: usize,
}

/// Node in a lock-free stack
pub struct StackNode<T> {
    /// The data stored in this node
    pub data: T,
    /// Pointer to the next node
    pub next: AtomicPtr<StackNode<T>>,
}

impl<T> StackNode<T> {
    /// Create a new stack node
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

/// Generic lock-free stack trait
pub trait LockFreeStack<T>: Send + Sync {
    /// Push an item onto the stack
    fn push(&self, item: T) -> Result<(), StackError>;

    /// Pop an item from the stack
    fn pop(&self) -> Result<T, StackError>;

    /// Try to push without blocking
    fn try_push(&self, item: T) -> Result<(), StackError>;

    /// Try to pop without blocking
    fn try_pop(&self) -> Result<T, StackError>;

    /// Check if stack is empty
    fn is_empty(&self) -> bool;

    /// Get current size (approximate)
    fn len(&self) -> usize;

    /// Get stack statistics
    fn stats(&self) -> StackStats;

    /// Clear all items from the stack
    fn clear(&self);
}

/// Treiber stack - classic lock-free stack implementation
pub struct TreiberStack<T>
where
    T: Send + Sync,
{
    /// Head pointer (top of stack)
    head: CacheLinePadded<AtomicPtr<StackNode<T>>>,
    /// Configuration
    config: LockFreeConfig,
    /// Performance metrics
    metrics: LockFreeMetrics,
    /// Hazard pointers for memory reclamation
    hazard_pointers: Vec<HazardPointer<StackNode<T>>>,
    /// Current size (approximate)
    size: AtomicUsize,
}

impl<T> TreiberStack<T>
where
    T: Send + Sync,
{
    /// Create a new Treiber stack
    pub fn new(config: LockFreeConfig) -> Self {
        let num_threads = num_cpus::get() * 2; // Estimate for hazard pointers

        Self {
            head: CacheLinePadded::new(AtomicPtr::new(ptr::null_mut())),
            config,
            metrics: LockFreeMetrics::default(),
            hazard_pointers: (0..num_threads).map(|_| HazardPointer::new()).collect(),
            size: AtomicUsize::new(0),
        }
    }

    /// Get a hazard pointer for the current thread
    fn get_hazard_pointer(&self) -> &HazardPointer<StackNode<T>> {
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
    fn reclaim_node(&self, node: *mut StackNode<T>) {
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

impl<T: Send + Sync> LockFreeStack<T> for TreiberStack<T> {
    fn push(&self, item: T) -> Result<(), StackError> {
        let new_node = Box::into_raw(Box::new(StackNode::new(item)));
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                unsafe {
                    let _ = Box::from_raw(new_node);
                }
                self.metrics.record_failure();
                return Err(StackError::MaxRetriesExceeded { retries });
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
                self.size.fetch_add(1, Ordering::Relaxed);
                self.metrics.record_success();
                return Ok(());
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn pop(&self) -> Result<T, StackError> {
        let mut retries = 0;

        loop {
            if retries >= self.config.max_retries {
                self.metrics.record_failure();
                return Err(StackError::MaxRetriesExceeded { retries });
            }

            let head = self.head.load(Ordering::Acquire);
            if head.is_null() {
                self.metrics.record_failure();
                return Err(StackError::Empty);
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
                self.size.fetch_sub(1, Ordering::Relaxed);
                self.metrics.record_success();
                return Ok(data);
            }

            retries += 1;
            self.metrics.update_contention(retries);
            self.config.backoff_strategy.backoff_blocking(retries);
        }
    }

    fn try_push(&self, item: T) -> Result<(), StackError> {
        let new_node = Box::into_raw(Box::new(StackNode::new(item)));

        let head = self.head.load(Ordering::Acquire);
        unsafe {
            (*new_node).next.store(head, Ordering::Relaxed);
        }

        let cas_result =
            self.head
                .compare_exchange(head, new_node, Ordering::Release, Ordering::Relaxed);

        self.metrics.record_cas(cas_result.is_ok());

        if cas_result.is_ok() {
            self.size.fetch_add(1, Ordering::Relaxed);
            self.metrics.record_success();
            Ok(())
        } else {
            unsafe {
                let _ = Box::from_raw(new_node);
            }
            self.metrics.record_failure();
            Err(StackError::MaxRetriesExceeded { retries: 1 })
        }
    }

    fn try_pop(&self) -> Result<T, StackError> {
        let head = self.head.load(Ordering::Acquire);
        if head.is_null() {
            self.metrics.record_failure();
            return Err(StackError::Empty);
        }

        let hazard = self.get_hazard_pointer();
        hazard.protect(head);

        // Double-check that head hasn't changed
        if self.head.load(Ordering::Acquire) != head {
            self.metrics.record_failure();
            return Err(StackError::MaxRetriesExceeded { retries: 1 });
        }

        let next = unsafe { (*head).next.load(Ordering::Acquire) };

        let cas_result =
            self.head
                .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed);

        self.metrics.record_cas(cas_result.is_ok());

        if cas_result.is_ok() {
            hazard.clear();
            let data = unsafe { Box::from_raw(head).data };
            self.size.fetch_sub(1, Ordering::Relaxed);
            self.metrics.record_success();
            Ok(data)
        } else {
            self.metrics.record_failure();
            Err(StackError::MaxRetriesExceeded { retries: 1 })
        }
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire).is_null()
    }

    fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    fn stats(&self) -> StackStats {
        StackStats {
            size: self.len(),
            capacity: 0, // Unbounded
            pushes: self.metrics.successes.load(Ordering::Relaxed),
            pops: self.metrics.successes.load(Ordering::Relaxed),
            push_failures: self.metrics.failures.load(Ordering::Relaxed),
            pop_failures: self.metrics.failures.load(Ordering::Relaxed),
            contention_level: self.metrics.contention_percentage(),
            eliminations: 0, // Not supported in basic Treiber stack
        }
    }

    fn clear(&self) {
        while self.try_pop().is_ok() {
            // Keep popping until empty
        }
    }
}

impl<T> Drop for TreiberStack<T>
where
    T: Send + Sync,
{
    fn drop(&mut self) {
        self.clear();
    }
}

unsafe impl<T: Send + Sync> Send for TreiberStack<T> {}
unsafe impl<T: Send + Sync> Sync for TreiberStack<T> {}

/// Elimination-based stack for high contention scenarios
pub struct EliminationStack<T>
where
    T: Send + Sync,
{
    /// Core Treiber stack
    core_stack: TreiberStack<T>,
    /// Elimination array for pairing push/pop operations
    elimination_array: Vec<CacheLinePadded<AtomicPtr<EliminationSlot<T>>>>,
    /// Number of elimination attempts
    elimination_attempts: AtomicUsize,
    /// Number of successful eliminations
    successful_eliminations: AtomicUsize,
}

/// Slot in the elimination array
struct EliminationSlot<T> {
    /// Operation type
    operation: AtomicUsize, // 0 = empty, 1 = push, 2 = pop
    /// Data for push operations
    data: Option<T>,
    /// Result of the operation
    result: AtomicPtr<T>,
    /// Timestamp for timeout
    timestamp: AtomicUsize,
}

impl<T> EliminationSlot<T> {
    fn new() -> Self {
        Self {
            operation: AtomicUsize::new(0),
            data: None,
            result: AtomicPtr::new(ptr::null_mut()),
            timestamp: AtomicUsize::new(0),
        }
    }
}

impl<T> EliminationStack<T>
where
    T: Send + Sync,
{
    /// Create a new elimination stack
    pub fn new(config: LockFreeConfig, elimination_array_size: usize) -> Self {
        let elimination_array = (0..elimination_array_size)
            .map(|_| {
                CacheLinePadded::new(AtomicPtr::new(Box::into_raw(Box::new(
                    EliminationSlot::new(),
                ))))
            })
            .collect();

        Self {
            core_stack: TreiberStack::new(config),
            elimination_array,
            elimination_attempts: AtomicUsize::new(0),
            successful_eliminations: AtomicUsize::new(0),
        }
    }

    /// Try to eliminate a push operation with a pop operation
    fn try_eliminate_push(&self, item: T) -> Result<(), T> {
        self.elimination_attempts.fetch_add(1, Ordering::Relaxed);

        // Choose a random slot
        let slot_index = fastrand::usize(0..self.elimination_array.len());
        let slot_ptr = self.elimination_array[slot_index].load(Ordering::Acquire);

        if slot_ptr.is_null() {
            return Err(item);
        }

        let slot = unsafe { &*slot_ptr };

        // Try to find a waiting pop operation
        if slot
            .operation
            .compare_exchange(
                2, // pop operation
                0, // mark as completed
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            // Successfully paired with a pop operation
            // The pop operation will take our item
            self.successful_eliminations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(item)
        }
    }

    /// Try to eliminate a pop operation with a push operation
    fn try_eliminate_pop(&self) -> Result<T, ()> {
        self.elimination_attempts.fetch_add(1, Ordering::Relaxed);

        // Choose a random slot
        let slot_index = fastrand::usize(0..self.elimination_array.len());
        let slot_ptr = self.elimination_array[slot_index].load(Ordering::Acquire);

        if slot_ptr.is_null() {
            return Err(());
        }

        let slot = unsafe { &*slot_ptr };

        // Try to find a waiting push operation
        if slot
            .operation
            .compare_exchange(
                1, // push operation
                0, // mark as completed
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            // Successfully paired with a push operation
            self.successful_eliminations.fetch_add(1, Ordering::Relaxed);
            // In a real implementation, we'd extract the data from the push operation
            // For simplicity, we'll return an error here
            Err(())
        } else {
            Err(())
        }
    }
}

impl<T: Send + Sync> LockFreeStack<T> for EliminationStack<T> {
    fn push(&self, item: T) -> Result<(), StackError> {
        // First try elimination
        match self.try_eliminate_push(item) {
            Ok(()) => {
                self.core_stack.metrics.record_success();
                return Ok(());
            }
            Err(item) => {
                // Elimination failed, use core stack
                self.core_stack.push(item)
            }
        }
    }

    fn pop(&self) -> Result<T, StackError> {
        // First try elimination
        if let Ok(item) = self.try_eliminate_pop() {
            self.core_stack.metrics.record_success();
            return Ok(item);
        }

        // Elimination failed, use core stack
        self.core_stack.pop()
    }

    fn try_push(&self, item: T) -> Result<(), StackError> {
        // Try elimination first, then core stack
        match self.try_eliminate_push(item) {
            Ok(()) => Ok(()),
            Err(item) => self.core_stack.try_push(item),
        }
    }

    fn try_pop(&self) -> Result<T, StackError> {
        // Try elimination first, then core stack
        if let Ok(item) = self.try_eliminate_pop() {
            Ok(item)
        } else {
            self.core_stack.try_pop()
        }
    }

    fn is_empty(&self) -> bool {
        self.core_stack.is_empty()
    }

    fn len(&self) -> usize {
        self.core_stack.len()
    }

    fn stats(&self) -> StackStats {
        let mut stats = self.core_stack.stats();
        stats.eliminations = self.successful_eliminations.load(Ordering::Relaxed);
        stats
    }

    fn clear(&self) {
        self.core_stack.clear()
    }
}

impl<T> Drop for EliminationStack<T>
where
    T: Send + Sync,
{
    fn drop(&mut self) {
        self.clear();

        // Clean up elimination array
        for slot_ptr in &self.elimination_array {
            let ptr = slot_ptr.load(Ordering::Acquire);
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
    }
}

unsafe impl<T: Send + Sync> Send for EliminationStack<T> {}
unsafe impl<T: Send + Sync> Sync for EliminationStack<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_treiber_stack_basic_operations() {
        let stack = TreiberStack::new(LockFreeConfig::default());

        // Test push
        assert!(stack.push(1).is_ok());
        assert!(stack.push(2).is_ok());
        assert!(stack.push(3).is_ok());
        assert_eq!(stack.len(), 3);
        assert!(!stack.is_empty());

        // Test pop (LIFO order)
        assert_eq!(stack.pop().unwrap(), 3);
        assert_eq!(stack.pop().unwrap(), 2);
        assert_eq!(stack.pop().unwrap(), 1);
        assert_eq!(stack.len(), 0);
        assert!(stack.is_empty());

        // Test empty pop
        assert!(matches!(stack.pop(), Err(StackError::Empty)));
    }

    #[test]
    fn test_treiber_stack_concurrent() {
        let stack = Arc::new(TreiberStack::new(LockFreeConfig::default()));
        let num_threads = 8;
        let items_per_thread = 100;

        let mut handles = Vec::new();

        // Spawn threads that push items
        for thread_id in 0..num_threads {
            let stack_clone = Arc::clone(&stack);
            let handle = thread::spawn(move || {
                for i in 0..items_per_thread {
                    let item = thread_id * items_per_thread + i;
                    while stack_clone.push(item).is_err() {
                        thread::yield_now();
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all pushes to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items are in the stack
        assert_eq!(stack.len(), num_threads * items_per_thread);

        // Pop all items
        let mut popped_items = Vec::new();
        while let Ok(item) = stack.pop() {
            popped_items.push(item);
        }

        assert_eq!(popped_items.len(), num_threads * items_per_thread);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_elimination_stack_basic() {
        let stack = EliminationStack::new(LockFreeConfig::default(), 16);

        // Basic operations should work like regular stack
        assert!(stack.push(1).is_ok());
        assert!(stack.push(2).is_ok());
        assert_eq!(stack.len(), 2);

        assert_eq!(stack.pop().unwrap(), 2);
        assert_eq!(stack.pop().unwrap(), 1);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stack_stats() {
        let stack = TreiberStack::new(LockFreeConfig::default());

        let initial_stats = stack.stats();
        assert_eq!(initial_stats.size, 0);
        assert_eq!(initial_stats.capacity, 0); // Unbounded

        stack.push(1).unwrap();
        stack.push(2).unwrap();

        let stats = stack.stats();
        assert_eq!(stats.size, 2);
        assert!(stats.pushes >= 2);

        stack.pop().unwrap();
        let stats = stack.stats();
        assert_eq!(stats.size, 1);
    }

    #[test]
    fn test_try_operations() {
        let stack = TreiberStack::new(LockFreeConfig::default());

        // try_push should work
        assert!(stack.try_push(42).is_ok());
        assert_eq!(stack.len(), 1);

        // try_pop should work
        assert_eq!(stack.try_pop().unwrap(), 42);
        assert!(stack.is_empty());

        // try_pop on empty stack should fail
        assert!(matches!(stack.try_pop(), Err(StackError::Empty)));
    }
}
