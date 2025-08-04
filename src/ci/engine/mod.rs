//! Modular CI Engine implementation
//!
//! This module provides a refactored CI engine that follows the strategy pattern
//! and integrates with SAGA orchestration for reliable pipeline execution.

pub mod execution_coordinator;
pub mod execution_strategies;
pub mod integration;
pub mod metrics;
pub mod monitoring;
pub mod orchestrator;
pub mod pipeline_manager;
pub mod saga_steps;
// Re-export main types
pub use execution_coordinator::{ExecutionCoordinator, ResourceLimits};
pub use execution_strategies::{
    ExecutionContext, ExecutionResult, ExecutionStrategyFactory, ExecutionStrategyType,
    ParallelExecutionStrategy, SequentialExecutionStrategy,
    MinimalExecutionStrategy, SimpleExecutionStrategy, StandardExecutionStrategy, AdvancedExecutionStrategy,
};
pub use metrics::MetricsCollector;
pub use monitoring::{ExecutionMetrics, ExecutionMonitoring};
pub use orchestrator::CIEngineOrchestrator;
pub use pipeline_manager::PipelineManager;
pub use saga_steps::PipelineExecutionSagaFactory;

