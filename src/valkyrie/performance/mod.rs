//! Valkyrie Protocol Performance Optimizations
//!
//! This module provides performance optimization components including
//! zero-copy operations and SIMD processing.

pub mod zero_copy;
pub mod simd;

// Re-export performance components
pub use zero_copy::{ZeroCopyBuffer, ZeroCopyProcessor};
pub use simd::{SimdProcessor, SimdOptimizations};