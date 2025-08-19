//! Observability, monitoring, and diagnostics
//!
//! This module contains components for metrics collection, monitoring,
//! logging, tracing, profiling, and audit functionality.

pub mod audit;
pub mod distributed_tracing;
pub mod logging;
pub mod metrics;
pub mod monitoring;
pub mod observability;
pub mod observability_integration;
pub mod profiling;
pub mod prometheus_metrics;
pub mod structured_logging;

// Re-export commonly used types
pub use metrics::MetricsCollector;
