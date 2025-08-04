//! Example projections for event sourcing
//!
//! This module contains example projections that demonstrate how to build
//! read models from event streams using the event sourcing infrastructure.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::core::patterns::event_sourcing::{EventProjection, EventStoreEntry};
use crate::core::patterns::events::{PipelineCompletedEvent, PipelineStartedEvent};
use crate::error::{AppError, Result};

/// Pipeline statistics projection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    pub total_pipelines: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub average_duration_ms: f64,
    pub pipelines_by_status: HashMap<String, u64>,
    pub last_updated: DateTime<Utc>,
}

impl Default for PipelineStats {
    fn default() -> Self {
        Self {
            total_pipelines: 0,
            successful_executions: 0,
            failed_executions: 0,
            average_duration_ms: 0.0,
            pipelines_by_status: HashMap::new(),
            last_updated: Utc::now(),
        }
    }
}

/// Pipeline statistics projection implementation
pub struct PipelineStatsProjection {
    stats: Arc<RwLock<PipelineStats>>,
    execution_durations: Arc<RwLock<Vec<u64>>>, // For calculating average
}

impl Default for PipelineStatsProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineStatsProjection {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(PipelineStats::default())),
            execution_durations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> PipelineStats {
        self.stats.read().await.clone()
    }

    /// Update average duration calculation
    async fn update_average_duration(&self, duration_ms: u64) {
        let mut durations = self.execution_durations.write().await;
        durations.push(duration_ms);

        // Keep only last 1000 durations for rolling average
        if durations.len() > 1000 {
            durations.remove(0);
        }

        let average = durations.iter().sum::<u64>() as f64 / durations.len() as f64;

        let mut stats = self.stats.write().await;
        stats.average_duration_ms = average;
        stats.last_updated = Utc::now();
    }
}

#[async_trait]
impl EventProjection for PipelineStatsProjection {
    fn projection_name(&self) -> &'static str {
        "PipelineStats"
    }

    async fn handle_event(&self, event: &EventStoreEntry) -> Result<()> {
        match event.event_type.as_str() {
            "pipeline.started" => {
                let mut stats = self.stats.write().await;
                stats.total_pipelines += 1;
                *stats
                    .pipelines_by_status
                    .entry("started".to_string())
                    .or_insert(0) += 1;
                stats.last_updated = Utc::now();

                debug!(
                    projection = self.projection_name(),
                    event_id = %event.event_id,
                    "Processed pipeline started event"
                );
            }
            "pipeline.completed" => {
                // Deserialize event data to get status and duration
                if let Ok(completed_event) =
                    serde_json::from_value::<PipelineCompletedEvent>(event.event_data.clone())
                {
                    let mut stats = self.stats.write().await;

                    match completed_event.status.as_str() {
                        "success" => {
                            stats.successful_executions += 1;
                            *stats
                                .pipelines_by_status
                                .entry("success".to_string())
                                .or_insert(0) += 1;
                        }
                        "failed" => {
                            stats.failed_executions += 1;
                            *stats
                                .pipelines_by_status
                                .entry("failed".to_string())
                                .or_insert(0) += 1;
                        }
                        _ => {
                            *stats
                                .pipelines_by_status
                                .entry(completed_event.status.clone())
                                .or_insert(0) += 1;
                        }
                    }

                    stats.last_updated = Utc::now();
                    drop(stats); // Release lock before async call

                    // Update average duration
                    self.update_average_duration(completed_event.duration_ms)
                        .await;

                    debug!(
                        projection = self.projection_name(),
                        event_id = %event.event_id,
                        status = completed_event.status,
                        duration_ms = completed_event.duration_ms,
                        "Processed pipeline completed event"
                    );
                } else {
                    warn!(
                        projection = self.projection_name(),
                        event_id = %event.event_id,
                        "Failed to deserialize pipeline completed event"
                    );
                }
            }
            _ => {
                // Ignore other event types
                debug!(
                    projection = self.projection_name(),
                    event_type = event.event_type,
                    "Ignoring unhandled event type"
                );
            }
        }

        Ok(())
    }

    async fn get_state(&self) -> Result<serde_json::Value> {
        let stats = self.stats.read().await;
        serde_json::to_value(&*stats).map_err(|e| {
            AppError::InternalServerError(format!("Failed to serialize projection state: {}", e))
        })
    }

    async fn reset(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        *stats = PipelineStats::default();

        let mut durations = self.execution_durations.write().await;
        durations.clear();

        info!(
            projection = self.projection_name(),
            "Projection state reset"
        );

        Ok(())
    }

    fn handles_event_type(&self, event_type: &str) -> bool {
        matches!(event_type, "pipeline.started" | "pipeline.completed")
    }
}

/// Pipeline execution history projection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecution {
    pub pipeline_id: Uuid,
    pub execution_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub duration_ms: Option<u64>,
    pub triggered_by: String,
}

/// Pipeline execution history projection implementation
pub struct PipelineHistoryProjection {
    executions: Arc<RwLock<HashMap<Uuid, PipelineExecution>>>,
    max_history_size: usize,
}

impl PipelineHistoryProjection {
    pub fn new(max_history_size: Option<usize>) -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            max_history_size: max_history_size.unwrap_or(10000),
        }
    }

    /// Get execution history
    pub async fn get_executions(&self) -> Vec<PipelineExecution> {
        let executions = self.executions.read().await;
        let mut history: Vec<PipelineExecution> = executions.values().cloned().collect();

        // Sort by started_at (newest first)
        history.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        history
    }

    /// Get execution by ID
    pub async fn get_execution(&self, execution_id: Uuid) -> Option<PipelineExecution> {
        let executions = self.executions.read().await;
        executions.get(&execution_id).cloned()
    }

    /// Get executions for a specific pipeline
    pub async fn get_pipeline_executions(&self, pipeline_id: Uuid) -> Vec<PipelineExecution> {
        let executions = self.executions.read().await;
        let mut pipeline_executions: Vec<PipelineExecution> = executions
            .values()
            .filter(|exec| exec.pipeline_id == pipeline_id)
            .cloned()
            .collect();

        // Sort by started_at (newest first)
        pipeline_executions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        pipeline_executions
    }

    /// Cleanup old executions to maintain size limit
    async fn cleanup_old_executions(&self) {
        let mut executions = self.executions.write().await;

        if executions.len() > self.max_history_size {
            // Convert to vector and sort by started_at
            let mut exec_vec: Vec<(Uuid, PipelineExecution)> = executions.drain().collect();
            exec_vec.sort_by(|a, b| b.1.started_at.cmp(&a.1.started_at));

            // Keep only the most recent executions
            exec_vec.truncate(self.max_history_size);

            // Rebuild the HashMap
            *executions = exec_vec.into_iter().collect();

            debug!(
                projection = "PipelineHistory",
                max_size = self.max_history_size,
                current_size = executions.len(),
                "Cleaned up old pipeline executions"
            );
        }
    }
}

#[async_trait]
impl EventProjection for PipelineHistoryProjection {
    fn projection_name(&self) -> &'static str {
        "PipelineHistory"
    }

    async fn handle_event(&self, event: &EventStoreEntry) -> Result<()> {
        match event.event_type.as_str() {
            "pipeline.started" => {
                if let Ok(started_event) =
                    serde_json::from_value::<PipelineStartedEvent>(event.event_data.clone())
                {
                    let execution = PipelineExecution {
                        pipeline_id: started_event.pipeline_id,
                        execution_id: started_event.execution_id,
                        started_at: started_event.occurred_at,
                        completed_at: None,
                        status: "running".to_string(),
                        duration_ms: None,
                        triggered_by: started_event.triggered_by,
                    };

                    let mut executions = self.executions.write().await;
                    executions.insert(started_event.execution_id, execution);
                    drop(executions); // Release lock before async call

                    // Cleanup old executions
                    self.cleanup_old_executions().await;

                    debug!(
                        projection = self.projection_name(),
                        execution_id = %started_event.execution_id,
                        pipeline_id = %started_event.pipeline_id,
                        "Added pipeline execution to history"
                    );
                }
            }
            "pipeline.completed" => {
                if let Ok(completed_event) =
                    serde_json::from_value::<PipelineCompletedEvent>(event.event_data.clone())
                {
                    let mut executions = self.executions.write().await;

                    if let Some(execution) = executions.get_mut(&completed_event.execution_id) {
                        execution.completed_at = Some(completed_event.occurred_at);
                        execution.status = completed_event.status.clone();
                        execution.duration_ms = Some(completed_event.duration_ms);

                        debug!(
                            projection = self.projection_name(),
                            execution_id = %completed_event.execution_id,
                            status = completed_event.status,
                            duration_ms = completed_event.duration_ms,
                            "Updated pipeline execution in history"
                        );
                    } else {
                        warn!(
                            projection = self.projection_name(),
                            execution_id = %completed_event.execution_id,
                            "Received completion event for unknown execution"
                        );
                    }
                }
            }
            _ => {
                // Ignore other event types
            }
        }

        Ok(())
    }

    async fn get_state(&self) -> Result<serde_json::Value> {
        let executions = self.get_executions().await;
        serde_json::to_value(&executions).map_err(|e| {
            AppError::InternalServerError(format!("Failed to serialize projection state: {}", e))
        })
    }

    async fn reset(&self) -> Result<()> {
        let mut executions = self.executions.write().await;
        executions.clear();

        info!(
            projection = self.projection_name(),
            "Projection state reset"
        );

        Ok(())
    }

    fn handles_event_type(&self, event_type: &str) -> bool {
        matches!(event_type, "pipeline.started" | "pipeline.completed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patterns::event_sourcing::EventStoreEntry;

    #[tokio::test]
    async fn test_pipeline_stats_projection() {
        let projection = PipelineStatsProjection::new();

        // Create a pipeline started event
        let started_event = EventStoreEntry {
            event_id: Uuid::new_v4(),
            aggregate_id: Uuid::new_v4(),
            aggregate_type: "Pipeline".to_string(),
            event_type: "pipeline.started".to_string(),
            event_version: 1,
            sequence_number: 1,
            event_data: serde_json::json!({
                "pipeline_id": Uuid::new_v4(),
                "execution_id": Uuid::new_v4(),
                "triggered_by": "test",
                "correlation_id": Uuid::new_v4(),
                "occurred_at": Utc::now(),
                "metadata": {}
            }),
            metadata: HashMap::new(),
            correlation_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            schema_version: 1,
        };

        // Process the event
        assert!(projection.handle_event(&started_event).await.is_ok());

        // Check stats
        let stats = projection.get_stats().await;
        assert_eq!(stats.total_pipelines, 1);
        assert_eq!(stats.pipelines_by_status.get("started"), Some(&1));
    }

    #[tokio::test]
    async fn test_pipeline_history_projection() {
        let projection = PipelineHistoryProjection::new(Some(100));

        let pipeline_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        // Create a pipeline started event
        let started_event = EventStoreEntry {
            event_id: Uuid::new_v4(),
            aggregate_id: pipeline_id,
            aggregate_type: "Pipeline".to_string(),
            event_type: "pipeline.started".to_string(),
            event_version: 1,
            sequence_number: 1,
            event_data: serde_json::json!({
                "pipeline_id": pipeline_id,
                "execution_id": execution_id,
                "triggered_by": "test",
                "correlation_id": Uuid::new_v4(),
                "occurred_at": Utc::now(),
                "metadata": {}
            }),
            metadata: HashMap::new(),
            correlation_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            schema_version: 1,
        };

        // Process the event
        assert!(projection.handle_event(&started_event).await.is_ok());

        // Check history
        let executions = projection.get_executions().await;
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].execution_id, execution_id);
        assert_eq!(executions[0].status, "running");

        // Get execution by ID
        let execution = projection.get_execution(execution_id).await;
        assert!(execution.is_some());
        assert_eq!(execution.unwrap().pipeline_id, pipeline_id);
    }
}
