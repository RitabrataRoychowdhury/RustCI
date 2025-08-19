//! Integration adapters for external systems

use crate::valkyrie::Result;

/// Generic integration adapter configuration
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    /// Adapter name
    pub name: String,
    /// Adapter type
    pub adapter_type: String,
    /// Configuration parameters
    pub parameters: std::collections::HashMap<String, String>,
}

/// Generic integration adapter trait
pub trait IntegrationAdapter: Send + Sync {
    /// Start the adapter
    fn start(&self) -> Result<()>;

    /// Stop the adapter
    fn stop(&self) -> Result<()>;

    /// Get adapter status
    fn status(&self) -> AdapterStatus;
}

/// Adapter status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterStatus {
    /// Adapter is stopped
    Stopped,
    /// Adapter is starting
    Starting,
    /// Adapter is running
    Running,
    /// Adapter is stopping
    Stopping,
    /// Adapter is in error state
    Error(String),
}
