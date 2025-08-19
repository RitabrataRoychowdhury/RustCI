//! Valkyrie Protocol Performance Optimizations
//!
//! This module provides performance optimization components including
//! zero-copy operations and SIMD processing.

pub mod simd;
pub mod zero_copy;

// Re-export performance components
pub use simd::{SimdOptimizations, SimdProcessor};
pub use zero_copy::{ZeroCopyBuffer, ZeroCopyProcessor};
