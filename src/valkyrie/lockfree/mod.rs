//! Valkyrie Protocol Lock-free Data Structures
//!
//! This module provides lock-free data structures optimized for
//! high-performance concurrent operations.

pub mod map;
pub mod queue;
pub mod stack;

// Re-export lock-free components
pub use map::{LockFreeMap, MapStats};
pub use queue::{LockFreeQueue, QueueStats};
pub use stack::{LockFreeStack, StackStats};
