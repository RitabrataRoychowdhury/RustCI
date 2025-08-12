//! Client statistics for the Valkyrie Protocol API

use serde::{Deserialize, Serialize};
use crate::valkyrie::core::EngineStats;

/// Client statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStats {
    /// Number of active connections
    pub active_connections: usize,
    /// Number of registered handlers
    pub handlers_registered: usize,
    /// Engine statistics
    pub engine_stats: EngineStats,
}

impl ClientStats {
    /// Create new client stats
    pub fn new(active_connections: usize, handlers_registered: usize, engine_stats: EngineStats) -> Self {
        Self {
            active_connections,
            handlers_registered,
            engine_stats,
        }
    }
}