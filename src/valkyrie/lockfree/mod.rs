//! Valkyrie Protocol Lock-free Data Structures
//!
//! This module provides lock-free data structures optimized for
//! high-performance concurrent operations.

pub mod compressed_radix_arena;
pub mod fingerprinted_id_table;
pub mod map;
pub mod queue;
pub mod snapshot_rcu;
pub mod stack;

// Re-export lock-free components
pub use compressed_radix_arena::{
    CompressedRadixArena, RadixNode, ServiceId, NodeIndex, CompactString, 
    FuzzyMatch, CraError, CraStats, Arena
};
pub use fingerprinted_id_table::{
    FingerprintedIdTable, RouteEntry, RouteId, ShardId, NodeId, FitError, FitStats
};
pub use map::{LockFreeMap, MapStats};
pub use queue::{LockFreeQueue, QueueStats};
pub use snapshot_rcu::{SnapshotRcuManager, UpdateOperation, RcuStats};
pub use stack::{LockFreeStack, StackStats};
