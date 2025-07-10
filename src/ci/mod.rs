pub mod engine;
pub mod pipeline;
pub mod executor;
pub mod connectors;
pub mod config;
pub mod schedulers;
pub mod workspace;

// Re-export main types that will be used by the application
// pub use engine::CIEngine;
// pub use pipeline::{PipelineExecution, TriggerInfo, ExecutionStatus, LogLevel};
// pub use config::CIPipeline;
// pub use connectors::{ConnectorManager, ConnectorType};
// pub use schedulers::CIScheduler;
// pub use workspace::{Workspace, WorkspaceManager};