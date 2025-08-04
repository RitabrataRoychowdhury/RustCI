//! Execution Monitoring - Tracks pipeline execution with metrics and events
//!
//! This module provides comprehensive monitoring for pipeline executions,
//! including real-time metrics, event tracking, and performance analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::ci::pipeline::ExecutionStatus;
use crate::core::patterns::correlation::CorrelationTracker;
use crate::error::{AppError, Result};

use super::{ExecutionContext, MetricsCollector};

/// Execution monitoring system
#[derive(Debug)]
pub struct ExecutionMonitoring {
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionMonitoringData>>>,
    execution_history: Arc<RwLock<Vec<ExecutionMonitoringData>>>,
    metrics_collector: Arc<MetricsCollector>,
    correlation_tracker: Arc<CorrelationTracker>,
    max_history_size: usize,
}

/// Monitoring data for a single execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMonitoringData {
    pub execution_id: Uuid,
    pub pipeline_id: Uuid,
    pub correlation_id: Uuid,
    pub pipeline_name: String,
    pub status: ExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub stages_completed: usize,
    pub steps_completed: usize,
    pub total_stages: usize,
    pub total_steps: usize,
    pub current_stage: Option<String>,
    pub current_step: Option<String>,
    pub events: Vec<ExecutionEvent>,
    pub metrics: ExecutionMetrics,
    pub resource_usage: ResourceUsageMetrics,
    pub error: Option<String>,
}

/// Execution event for detailed tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    pub event_id: Uuid,
    pub execution_id: Uuid,
    pub event_type: ExecutionEventType,
    pub timestamp: DateTime<Utc>,
    pub stage_name: Option<String>,
    pub step_name: Option<String>,
    pub message: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEventType {
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionCancelled,
    StageStarted,
    StageCompleted,
    StageFailed,
    StepStarted,
    StepCompleted,
    StepFailed,
    ResourceAllocated,
    ResourceReleased,
    MetricRecorded,
}

/// Execution metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub total_duration_ms: u64,
    pub queue_time_ms: u64,
    pub execution_time_ms: u64,
    pub cleanup_time_ms: u64,
    pub stage_durations: HashMap<String, u64>,
    pub step_durations: HashMap<String, u64>,
    pub throughput_steps_per_second: f64,
    pub success_rate: f64,
    pub error_count: usize,
    pub warning_count: usize,
    pub retry_count: usize,
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self {
            total_duration_ms: 0,
            queue_time_ms: 0,
            execution_time_ms: 0,
            cleanup_time_ms: 0,
            stage_durations: HashMap::new(),
            step_durations: HashMap::new(),
            throughput_steps_per_second: 0.0,
            success_rate: 0.0,
            error_count: 0,
            warning_count: 0,
            retry_count: 0,
        }
    }
}

/// Resource usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageMetrics {
    pub peak_memory_mb: u64,
    pub average_memory_mb: u64,
    pub peak_cpu_percent: f64,
    pub average_cpu_percent: f64,
    pub disk_usage_mb: u64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
    pub file_descriptors_used: u32,
}

impl Default for ResourceUsageMetrics {
    fn default() -> Self {
        Self {
            peak_memory_mb: 0,
            average_memory_mb: 0,
            peak_cpu_percent: 0.0,
            average_cpu_percent: 0.0,
            disk_usage_mb: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
            file_descriptors_used: 0,
        }
    }
}

impl ExecutionMonitoring {
    /// Create a new execution monitoring system
    pub fn new(correlation_tracker: Arc<CorrelationTracker>) -> Self {
        Self {
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            execution_history: Arc::new(RwLock::new(Vec::new())),
            metrics_collector: Arc::new(MetricsCollector::new()),
            correlation_tracker,
            max_history_size: 1000,
        }
    }

    /// Start monitoring a pipeline execution
    pub async fn start_execution_monitoring(
        &self,
        execution_id: Uuid,
        context: &ExecutionContext,
    ) -> Result<()> {
        info!(
            execution_id = %execution_id,
            pipeline_id = %context.pipeline_id,
            correlation_id = %context.correlation_id,
            "Starting execution monitoring"
        );

        // Set correlation context
        self.correlation_tracker
            .set_correlation_id(context.correlation_id)
            .await;

        // Calculate total stages and steps
        let total_stages = context.pipeline.stages.len();
        let total_steps: usize = context
            .pipeline
            .stages
            .iter()
            .map(|stage| stage.steps.len())
            .sum();

        // Create monitoring data
        let monitoring_data = ExecutionMonitoringData {
            execution_id,
            pipeline_id: context.pipeline_id,
            correlation_id: context.correlation_id,
            pipeline_name: context.pipeline.name.clone(),
            status: ExecutionStatus::Running,
            started_at: context.started_at,
            completed_at: None,
            duration_ms: None,
            stages_completed: 0,
            steps_completed: 0,
            total_stages,
            total_steps,
            current_stage: None,
            current_step: None,
            events: Vec::new(),
            metrics: ExecutionMetrics::default(),
            resource_usage: ResourceUsageMetrics::default(),
            error: None,
        };

        // Add to active executions
        {
            let mut active = self.active_executions.write().await;
            active.insert(execution_id, monitoring_data);
        }

        // Record execution started event
        self.record_event(
            execution_id,
            ExecutionEventType::ExecutionStarted,
            "Pipeline execution started".to_string(),
            None,
            None,
            HashMap::new(),
        )
        .await?;

        // Start metrics collection
        self.metrics_collector
            .start_collection(execution_id)
            .await?;

        debug!(execution_id = %execution_id, "Execution monitoring started successfully");
        Ok(())
    }

    /// Complete execution monitoring
    pub async fn complete_execution_monitoring(
        &self,
        execution_id: Uuid,
        final_status: ExecutionStatus,
    ) -> Result<()> {
        info!(
            execution_id = %execution_id,
            status = ?final_status,
            "Completing execution monitoring"
        );

        let completed_data = {
            let mut active = self.active_executions.write().await;
            if let Some(mut data) = active.remove(&execution_id) {
                // Update final status and completion time
                data.status = final_status.clone();
                data.completed_at = Some(Utc::now());

                if let Some(completed_at) = data.completed_at {
                    data.duration_ms =
                        Some((completed_at - data.started_at).num_milliseconds() as u64);
                }

                // Calculate final metrics
                data.metrics = self.calculate_final_metrics(&data).await;
                data.resource_usage = self.collect_final_resource_usage(execution_id).await;

                Some(data)
            } else {
                warn!(execution_id = %execution_id, "Execution not found in active monitoring");
                None
            }
        };

        if let Some(data) = completed_data {
            // Record completion event
            let event_type = match final_status {
                ExecutionStatus::Success => ExecutionEventType::ExecutionCompleted,
                ExecutionStatus::Failed => ExecutionEventType::ExecutionFailed,
                ExecutionStatus::Cancelled => ExecutionEventType::ExecutionCancelled,
                _ => ExecutionEventType::ExecutionCompleted,
            };

            self.record_event_for_data(
                &mut data.clone(),
                event_type,
                format!(
                    "Pipeline execution completed with status: {:?}",
                    final_status
                ),
                None,
                None,
                HashMap::new(),
            )
            .await?;

            // Add to history
            self.add_to_history(data).await;

            // Stop metrics collection
            self.metrics_collector.stop_collection(execution_id).await?;

            debug!(execution_id = %execution_id, "Execution monitoring completed successfully");
        }

        Ok(())
    }

    /// Update execution progress
    pub async fn update_execution_progress(
        &self,
        execution_id: Uuid,
        current_stage: Option<String>,
        current_step: Option<String>,
        stages_completed: usize,
        steps_completed: usize,
    ) -> Result<()> {
        debug!(
            execution_id = %execution_id,
            current_stage = ?current_stage,
            current_step = ?current_step,
            stages_completed = stages_completed,
            steps_completed = steps_completed,
            "Updating execution progress"
        );

        let mut active = self.active_executions.write().await;
        if let Some(data) = active.get_mut(&execution_id) {
            data.current_stage = current_stage;
            data.current_step = current_step;
            data.stages_completed = stages_completed;
            data.steps_completed = steps_completed;
        } else {
            warn!(execution_id = %execution_id, "Execution not found for progress update");
        }

        Ok(())
    }

    /// Record an execution event
    pub async fn record_event(
        &self,
        execution_id: Uuid,
        event_type: ExecutionEventType,
        message: String,
        stage_name: Option<String>,
        step_name: Option<String>,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let mut active = self.active_executions.write().await;
        if let Some(data) = active.get_mut(&execution_id) {
            self.record_event_for_data(data, event_type, message, stage_name, step_name, metadata)
                .await
        } else {
            warn!(execution_id = %execution_id, "Execution not found for event recording");
            Ok(())
        }
    }

    async fn record_event_for_data(
        &self,
        data: &mut ExecutionMonitoringData,
        event_type: ExecutionEventType,
        message: String,
        stage_name: Option<String>,
        step_name: Option<String>,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let event = ExecutionEvent {
            event_id: Uuid::new_v4(),
            execution_id: data.execution_id,
            event_type,
            timestamp: Utc::now(),
            stage_name,
            step_name,
            message,
            metadata,
        };

        data.events.push(event);

        // Keep only the last 100 events per execution to prevent memory issues
        if data.events.len() > 100 {
            data.events.drain(0..data.events.len() - 100);
        }

        Ok(())
    }

    /// Get execution status
    pub async fn get_execution_status(&self, execution_id: Uuid) -> Result<ExecutionStatus> {
        let active = self.active_executions.read().await;
        if let Some(data) = active.get(&execution_id) {
            Ok(data.status.clone())
        } else {
            // Check history
            let history = self.execution_history.read().await;
            if let Some(data) = history.iter().find(|d| d.execution_id == execution_id) {
                Ok(data.status.clone())
            } else {
                Err(AppError::NotFound("Execution not found".to_string()))
            }
        }
    }

    /// Get execution metrics
    pub async fn get_execution_metrics(&self, execution_id: Uuid) -> Result<ExecutionMetrics> {
        let active = self.active_executions.read().await;
        if let Some(data) = active.get(&execution_id) {
            Ok(data.metrics.clone())
        } else {
            // Check history
            let history = self.execution_history.read().await;
            if let Some(data) = history.iter().find(|d| d.execution_id == execution_id) {
                Ok(data.metrics.clone())
            } else {
                Err(AppError::NotFound(
                    "Execution metrics not found".to_string(),
                ))
            }
        }
    }

    /// List active executions
    pub async fn list_active_executions(&self) -> Result<Vec<Uuid>> {
        let active = self.active_executions.read().await;
        Ok(active.keys().cloned().collect())
    }

    /// Get execution monitoring data
    pub async fn get_execution_monitoring_data(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionMonitoringData> {
        let active = self.active_executions.read().await;
        if let Some(data) = active.get(&execution_id) {
            Ok(data.clone())
        } else {
            // Check history
            let history = self.execution_history.read().await;
            if let Some(data) = history.iter().find(|d| d.execution_id == execution_id) {
                Ok(data.clone())
            } else {
                Err(AppError::NotFound(
                    "Execution monitoring data not found".to_string(),
                ))
            }
        }
    }

    /// Get execution history
    pub async fn get_execution_history(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ExecutionMonitoringData>> {
        let history = self.execution_history.read().await;
        let limit = limit.unwrap_or(100);

        Ok(history.iter().rev().take(limit).cloned().collect())
    }

    /// Get system-wide monitoring statistics
    pub async fn get_monitoring_statistics(&self) -> Result<MonitoringStatistics> {
        let active = self.active_executions.read().await;
        let history = self.execution_history.read().await;

        let total_executions = active.len() + history.len();
        let active_executions = active.len();

        let completed_executions = history
            .iter()
            .filter(|d| matches!(d.status, ExecutionStatus::Success))
            .count();

        let failed_executions = history
            .iter()
            .filter(|d| matches!(d.status, ExecutionStatus::Failed))
            .count();

        let average_duration_ms = if !history.is_empty() {
            history.iter().filter_map(|d| d.duration_ms).sum::<u64>() as f64 / history.len() as f64
        } else {
            0.0
        };

        let success_rate = if !history.is_empty() {
            completed_executions as f64 / history.len() as f64
        } else {
            0.0
        };

        Ok(MonitoringStatistics {
            total_executions,
            active_executions,
            completed_executions,
            failed_executions,
            average_duration_ms,
            success_rate,
            current_throughput: self.calculate_current_throughput().await,
        })
    }

    /// Add execution data to history
    async fn add_to_history(&self, data: ExecutionMonitoringData) {
        let mut history = self.execution_history.write().await;
        history.push(data);

        // Keep history size under limit
        if history.len() > self.max_history_size {
            let excess = history.len() - self.max_history_size;
            history.drain(0..excess);
        }
    }

    /// Calculate final metrics for completed execution
    async fn calculate_final_metrics(&self, data: &ExecutionMonitoringData) -> ExecutionMetrics {
        let mut metrics = data.metrics.clone();

        if let Some(duration) = data.duration_ms {
            metrics.total_duration_ms = duration;

            if data.total_steps > 0 {
                metrics.throughput_steps_per_second =
                    data.steps_completed as f64 / (duration as f64 / 1000.0);
            }
        }

        metrics.success_rate = if data.total_steps > 0 {
            data.steps_completed as f64 / data.total_steps as f64
        } else {
            0.0
        };

        // Count errors and warnings from events
        metrics.error_count = data
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    ExecutionEventType::ExecutionFailed
                        | ExecutionEventType::StageFailed
                        | ExecutionEventType::StepFailed
                )
            })
            .count();

        metrics
    }

    /// Collect final resource usage metrics
    async fn collect_final_resource_usage(&self, _execution_id: Uuid) -> ResourceUsageMetrics {
        // In a real implementation, this would collect actual resource usage data
        // For now, return default values
        ResourceUsageMetrics::default()
    }

    /// Calculate current system throughput
    async fn calculate_current_throughput(&self) -> f64 {
        let history = self.execution_history.read().await;

        // Calculate throughput for the last hour
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
        let recent_executions: Vec<_> = history
            .iter()
            .filter(|d| d.started_at > one_hour_ago)
            .collect();

        if recent_executions.is_empty() {
            return 0.0;
        }

        let total_steps: usize = recent_executions.iter().map(|d| d.steps_completed).sum();

        total_steps as f64 / 3600.0 // steps per second
    }
}

/// System-wide monitoring statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStatistics {
    pub total_executions: usize,
    pub active_executions: usize,
    pub completed_executions: usize,
    pub failed_executions: usize,
    pub average_duration_ms: f64,
    pub success_rate: f64,
    pub current_throughput: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{CIPipeline, Stage, Step, StepConfig, StepType};
    use crate::ci::pipeline::TriggerInfo;
    use crate::core::patterns::correlation::CorrelationTracker;

    fn create_test_context() -> ExecutionContext {
        let mut pipeline = CIPipeline::new("test-pipeline".to_string());
        pipeline.description = Some("Test pipeline".to_string());
        pipeline.stages = vec![Stage {
                name: "build".to_string(),
                condition: None,
                parallel: None,
                steps: vec![Step {
                    name: "compile".to_string(),
                    step_type: StepType::Shell,
                    config: StepConfig {
                        command: Some("echo 'Building...'".to_string()),
                        ..Default::default()
                    },
                    condition: None,
                    continue_on_error: Some(false),
                    timeout: None,
                }],
                environment: None,
                timeout: None,
                retry_count: None,
            }];
        
        pipeline.timeout = None;
        pipeline.retry_count = None;

        ExecutionContext {
            execution_id: Uuid::new_v4(),
            pipeline_id: pipeline.id.unwrap(),
            correlation_id: Uuid::new_v4(),
            pipeline,
            trigger_info: TriggerInfo {
                trigger_type: "manual".to_string(),
                triggered_by: Some("test".to_string()),
                commit_hash: None,
                branch: None,
                repository: None,
                webhook_payload: None,
            },
            environment: HashMap::new(),
            started_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_execution_monitoring_lifecycle() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let monitoring = ExecutionMonitoring::new(correlation_tracker);

        let context = create_test_context();
        let execution_id = context.execution_id;

        // Start monitoring
        let result = monitoring
            .start_execution_monitoring(execution_id, &context)
            .await;
        assert!(result.is_ok());

        // Check that execution is active
        let active = monitoring.list_active_executions().await.unwrap();
        assert!(active.contains(&execution_id));

        // Update progress
        let result = monitoring
            .update_execution_progress(
                execution_id,
                Some("build".to_string()),
                Some("compile".to_string()),
                0,
                1,
            )
            .await;
        assert!(result.is_ok());

        // Complete monitoring
        let result = monitoring
            .complete_execution_monitoring(execution_id, ExecutionStatus::Success)
            .await;
        assert!(result.is_ok());

        // Check that execution is no longer active
        let active = monitoring.list_active_executions().await.unwrap();
        assert!(!active.contains(&execution_id));

        // Check that execution is in history
        let history = monitoring.get_execution_history(Some(10)).await.unwrap();
        assert!(history.iter().any(|d| d.execution_id == execution_id));
    }

    #[tokio::test]
    async fn test_event_recording() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let monitoring = ExecutionMonitoring::new(correlation_tracker);

        let context = create_test_context();
        let execution_id = context.execution_id;

        // Start monitoring
        monitoring
            .start_execution_monitoring(execution_id, &context)
            .await
            .unwrap();

        // Record an event
        let result = monitoring
            .record_event(
                execution_id,
                ExecutionEventType::StageStarted,
                "Stage started".to_string(),
                Some("build".to_string()),
                None,
                HashMap::new(),
            )
            .await;
        assert!(result.is_ok());

        // Get monitoring data and check event was recorded
        let data = monitoring
            .get_execution_monitoring_data(execution_id)
            .await
            .unwrap();
        assert!(data.events.len() >= 2); // At least start event + our event
        assert!(data
            .events
            .iter()
            .any(|e| matches!(e.event_type, ExecutionEventType::StageStarted)));
    }

    #[tokio::test]
    async fn test_monitoring_statistics() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let monitoring = ExecutionMonitoring::new(correlation_tracker);

        let stats = monitoring.get_monitoring_statistics().await.unwrap();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.active_executions, 0);
        assert_eq!(stats.completed_executions, 0);
        assert_eq!(stats.failed_executions, 0);
    }
}
