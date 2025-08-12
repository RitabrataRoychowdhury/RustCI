//! Valkyrie Protocol Lock-free Data Structures
//!
//! This module provides lock-free data structures optimized for
//! high-performance concurrent operations.

pub mod queue;
pub mod stack;
pub mod map;

// Re-export lock-free components
pub use queue::{LockFreeQueue, QueueStats};
pub use stack::{LockFreeStack, StackStats};
pub use map::{LockFreeMap, MapStats};