//! Metrics Collection - Collects and aggregates pipeline execution metrics
//!
//! This module provides comprehensive metrics collection for pipeline executions,
//! including performance metrics, resource usage, and business metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Metrics collector for pipeline executions
#[derive(Debug)]
pub struct MetricsCollector {
    active_collections: Arc<RwLock<HashMap<Uuid, MetricsCollection>>>,
    aggregated_metrics: Arc<RwLock<AggregatedMetrics>>,
}

/// Metrics collection for a single execution
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MetricsCollection {
    execution_id: Uuid,
    started_at: DateTime<Utc>,
    pipeline_metrics: PipelineMetrics,
    stage_metrics: HashMap<String, StageMetrics>,
    step_metrics: HashMap<String, StepMetrics>,
    resource_samples: Vec<ResourceSample>,
}

/// Pipeline-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetrics {
    pub execution_id: Uuid,
    pub pipeline_name: String,
    pub total_duration_ms: u64,
    pub queue_time_ms: u64,
    pub setup_time_ms: u64,
    pub execution_time_ms: u64,
    pub cleanup_time_ms: u64,
    pub total_stages: usize,
    pub completed_stages: usize,
    pub failed_stages: usize,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub success_rate: f64,
    pub throughput_steps_per_minute: f64,
    pub resource_efficiency: f64,
    pub cost_estimate: f64,
}

impl Default for PipelineMetrics {
    fn default() -> Self {
        Self {
            execution_id: Uuid::nil(),
            pipeline_name: String::new(),
            total_duration_ms: 0,
            queue_time_ms: 0,
            setup_time_ms: 0,
            execution_time_ms: 0,
            cleanup_time_ms: 0,
            total_stages: 0,
            completed_stages: 0,
            failed_stages: 0,
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            success_rate: 0.0,
            throughput_steps_per_minute: 0.0,
            resource_efficiency: 0.0,
            cost_estimate: 0.0,
        }
    }
}

/// Stage-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageMetrics {
    pub stage_name: String,
    pub execution_id: Uuid,
    pub duration_ms: u64,
    pub setup_time_ms: u64,
    pub execution_time_ms: u64,
    pub cleanup_time_ms: u64,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub parallel_execution: bool,
    pub max_parallel_steps: usize,
    pub average_step_duration_ms: f64,
    pub resource_usage: StageResourceUsage,
    pub cache_hit_rate: f64,
    pub retry_count: usize,
}

impl Default for StageMetrics {
    fn default() -> Self {
        Self {
            stage_name: String::new(),
            execution_id: Uuid::nil(),
            duration_ms: 0,
            setup_time_ms: 0,
            execution_time_ms: 0,
            cleanup_time_ms: 0,
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            parallel_execution: false,
            max_parallel_steps: 1,
            average_step_duration_ms: 0.0,
            resource_usage: StageResourceUsage::default(),
            cache_hit_rate: 0.0,
            retry_count: 0,
        }
    }
}

/// Step-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepMetrics {
    pub step_name: String,
    pub stage_name: String,
    pub execution_id: Uuid,
    pub step_type: String,
    pub duration_ms: u64,
    pub queue_time_ms: u64,
    pub execution_time_ms: u64,
    pub exit_code: Option<i32>,
    pub retry_count: usize,
    pub cache_hit: bool,
    pub resource_usage: StepResourceUsage,
    pub output_size_bytes: u64,
    pub error_message: Option<String>,
    pub performance_score: f64,
}

impl Default for StepMetrics {
    fn default() -> Self {
        Self {
            step_name: String::new(),
            stage_name: String::new(),
            execution_id: Uuid::nil(),
            step_type: String::new(),
            duration_ms: 0,
            queue_time_ms: 0,
            execution_time_ms: 0,
            exit_code: None,
            retry_count: 0,
            cache_hit: false,
            resource_usage: StepResourceUsage::default(),
            output_size_bytes: 0,
            error_message: None,
            performance_score: 0.0,
        }
    }
}

/// Resource usage for stages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResourceUsage {
    pub peak_memory_mb: u64,
    pub average_memory_mb: u64,
    pub peak_cpu_percent: f64,
    pub average_cpu_percent: f64,
    pub disk_read_mb: u64,
    pub disk_write_mb: u64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

impl Default for StageResourceUsage {
    fn default() -> Self {
        Self {
            peak_memory_mb: 0,
            average_memory_mb: 0,
            peak_cpu_percent: 0.0,
            average_cpu_percent: 0.0,
            disk_read_mb: 0,
            disk_write_mb: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
        }
    }
}

/// Resource usage for steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResourceUsage {
    pub memory_mb: u64,
    pub cpu_percent: f64,
    pub disk_read_mb: u64,
    pub disk_write_mb: u64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
    pub file_descriptors: u32,
}

impl Default for StepResourceUsage {
    fn default() -> Self {
        Self {
            memory_mb: 0,
            cpu_percent: 0.0,
            disk_read_mb: 0,
            disk_write_mb: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
            file_descriptors: 0,
        }
    }
}

/// Resource sample for time-series data
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ResourceSample {
    timestamp: DateTime<Utc>,
    memory_mb: u64,
    cpu_percent: f64,
    disk_io_mb_per_sec: f64,
    network_io_mb_per_sec: f64,
}

/// Aggregated metrics across all executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub average_execution_time_ms: f64,
    pub median_execution_time_ms: f64,
    pub p95_execution_time_ms: f64,
    pub p99_execution_time_ms: f64,
    pub total_steps_executed: u64,
    pub average_steps_per_execution: f64,
    pub overall_success_rate: f64,
    pub throughput_executions_per_hour: f64,
    pub resource_utilization: AggregatedResourceMetrics,
    pub cost_metrics: CostMetrics,
    pub trend_data: TrendData,
}

impl Default for AggregatedMetrics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            average_execution_time_ms: 0.0,
            median_execution_time_ms: 0.0,
            p95_execution_time_ms: 0.0,
            p99_execution_time_ms: 0.0,
            total_steps_executed: 0,
            average_steps_per_execution: 0.0,
            overall_success_rate: 0.0,
            throughput_executions_per_hour: 0.0,
            resource_utilization: AggregatedResourceMetrics::default(),
            cost_metrics: CostMetrics::default(),
            trend_data: TrendData::default(),
        }
    }
}

/// Aggregated resource metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResourceMetrics {
    pub average_memory_utilization: f64,
    pub peak_memory_utilization: f64,
    pub average_cpu_utilization: f64,
    pub peak_cpu_utilization: f64,
    pub total_disk_io_gb: f64,
    pub total_network_io_gb: f64,
    pub resource_efficiency_score: f64,
}

impl Default for AggregatedResourceMetrics {
    fn default() -> Self {
        Self {
            average_memory_utilization: 0.0,
            peak_memory_utilization: 0.0,
            average_cpu_utilization: 0.0,
            peak_cpu_utilization: 0.0,
            total_disk_io_gb: 0.0,
            total_network_io_gb: 0.0,
            resource_efficiency_score: 0.0,
        }
    }
}

/// Cost metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetrics {
    pub total_cost_usd: f64,
    pub average_cost_per_execution: f64,
    pub cost_per_successful_execution: f64,
    pub compute_cost_usd: f64,
    pub storage_cost_usd: f64,
    pub network_cost_usd: f64,
    pub cost_trend_7d: f64,
    pub cost_trend_30d: f64,
}

impl Default for CostMetrics {
    fn default() -> Self {
        Self {
            total_cost_usd: 0.0,
            average_cost_per_execution: 0.0,
            cost_per_successful_execution: 0.0,
            compute_cost_usd: 0.0,
            storage_cost_usd: 0.0,
            network_cost_usd: 0.0,
            cost_trend_7d: 0.0,
            cost_trend_30d: 0.0,
        }
    }
}

/// Trend data for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendData {
    pub execution_count_24h: Vec<u64>,
    pub success_rate_24h: Vec<f64>,
    pub average_duration_24h: Vec<f64>,
    pub resource_usage_24h: Vec<f64>,
    pub error_rate_24h: Vec<f64>,
}

impl Default for TrendData {
    fn default() -> Self {
        Self {
            execution_count_24h: vec![0; 24],
            success_rate_24h: vec![0.0; 24],
            average_duration_24h: vec![0.0; 24],
            resource_usage_24h: vec![0.0; 24],
            error_rate_24h: vec![0.0; 24],
        }
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            active_collections: Arc::new(RwLock::new(HashMap::new())),
            aggregated_metrics: Arc::new(RwLock::new(AggregatedMetrics::default())),
        }
    }

    /// Start collecting metrics for an execution
    pub async fn start_collection(&self, execution_id: Uuid) -> Result<()> {
        debug!(execution_id = %execution_id, "Starting metrics collection");

        let collection = MetricsCollection {
            execution_id,
            started_at: Utc::now(),
            pipeline_metrics: PipelineMetrics::default(),
            stage_metrics: HashMap::new(),
            step_metrics: HashMap::new(),
            resource_samples: Vec::new(),
        };

        let mut active = self.active_collections.write().await;
        active.insert(execution_id, collection);

        debug!(execution_id = %execution_id, "Metrics collection started");
        Ok(())
    }

    /// Stop collecting metrics for an execution
    pub async fn stop_collection(&self, execution_id: Uuid) -> Result<PipelineMetrics> {
        debug!(execution_id = %execution_id, "Stopping metrics collection");

        let collection = {
            let mut active = self.active_collections.write().await;
            active.remove(&execution_id)
        };

        if let Some(mut collection) = collection {
            // Finalize metrics calculation
            self.finalize_metrics(&mut collection).await;

            // Update aggregated metrics
            self.update_aggregated_metrics(&collection.pipeline_metrics)
                .await;

            debug!(execution_id = %execution_id, "Metrics collection stopped");
            Ok(collection.pipeline_metrics)
        } else {
            warn!(execution_id = %execution_id, "Metrics collection not found");
            Err(AppError::NotFound(
                "Metrics collection not found".to_string(),
            ))
        }
    }

    /// Record pipeline metrics
    pub async fn record_pipeline_metrics(
        &self,
        execution_id: Uuid,
        pipeline_name: String,
        total_stages: usize,
        total_steps: usize,
    ) -> Result<()> {
        let mut active = self.active_collections.write().await;
        if let Some(collection) = active.get_mut(&execution_id) {
            collection.pipeline_metrics.execution_id = execution_id;
            collection.pipeline_metrics.pipeline_name = pipeline_name;
            collection.pipeline_metrics.total_stages = total_stages;
            collection.pipeline_metrics.total_steps = total_steps;
        } else {
            warn!(execution_id = %execution_id, "Metrics collection not found for pipeline metrics");
        }
        Ok(())
    }

    /// Record stage metrics
    pub async fn record_stage_metrics(
        &self,
        execution_id: Uuid,
        stage_name: String,
        duration_ms: u64,
        total_steps: usize,
        completed_steps: usize,
        failed_steps: usize,
        parallel_execution: bool,
    ) -> Result<()> {
        let mut active = self.active_collections.write().await;
        if let Some(collection) = active.get_mut(&execution_id) {
            let stage_metrics = StageMetrics {
                stage_name: stage_name.clone(),
                execution_id,
                duration_ms,
                setup_time_ms: 0, // Would be measured in real implementation
                execution_time_ms: duration_ms,
                cleanup_time_ms: 0,
                total_steps,
                completed_steps,
                failed_steps,
                parallel_execution,
                max_parallel_steps: if parallel_execution { total_steps } else { 1 },
                average_step_duration_ms: if total_steps > 0 {
                    duration_ms as f64 / total_steps as f64
                } else {
                    0.0
                },
                resource_usage: StageResourceUsage::default(),
                cache_hit_rate: 0.0, // Would be calculated from actual cache data
                retry_count: 0,
            };

            collection.stage_metrics.insert(stage_name, stage_metrics);

            // Update pipeline metrics
            collection.pipeline_metrics.completed_stages += 1;
            if failed_steps > 0 {
                collection.pipeline_metrics.failed_stages += 1;
            }
        } else {
            warn!(execution_id = %execution_id, "Metrics collection not found for stage metrics");
        }
        Ok(())
    }

    /// Record step metrics
    pub async fn record_step_metrics(
        &self,
        execution_id: Uuid,
        stage_name: String,
        step_name: String,
        step_type: String,
        duration_ms: u64,
        exit_code: Option<i32>,
        retry_count: usize,
        output_size_bytes: u64,
        error_message: Option<String>,
    ) -> Result<()> {
        let mut active = self.active_collections.write().await;
        if let Some(collection) = active.get_mut(&execution_id) {
            let step_key = format!("{}::{}", stage_name, step_name);
            let step_metrics = StepMetrics {
                step_name: step_name.clone(),
                stage_name: stage_name.clone(),
                execution_id,
                step_type,
                duration_ms,
                queue_time_ms: 0, // Would be measured in real implementation
                execution_time_ms: duration_ms,
                exit_code,
                retry_count,
                cache_hit: false, // Would be determined from cache system
                resource_usage: StepResourceUsage::default(),
                output_size_bytes,
                error_message,
                performance_score: self.calculate_step_performance_score(duration_ms, exit_code),
            };

            collection.step_metrics.insert(step_key, step_metrics);

            // Update pipeline metrics
            collection.pipeline_metrics.completed_steps += 1;
            if exit_code.is_some() && exit_code != Some(0) {
                collection.pipeline_metrics.failed_steps += 1;
            }
        } else {
            warn!(execution_id = %execution_id, "Metrics collection not found for step metrics");
        }
        Ok(())
    }

    /// Record resource sample
    pub async fn record_resource_sample(
        &self,
        execution_id: Uuid,
        memory_mb: u64,
        cpu_percent: f64,
        disk_io_mb_per_sec: f64,
        network_io_mb_per_sec: f64,
    ) -> Result<()> {
        let mut active = self.active_collections.write().await;
        if let Some(collection) = active.get_mut(&execution_id) {
            let sample = ResourceSample {
                timestamp: Utc::now(),
                memory_mb,
                cpu_percent,
                disk_io_mb_per_sec,
                network_io_mb_per_sec,
            };

            collection.resource_samples.push(sample);

            // Keep only the last 1000 samples to prevent memory issues
            if collection.resource_samples.len() > 1000 {
                collection
                    .resource_samples
                    .drain(0..collection.resource_samples.len() - 1000);
            }
        } else {
            warn!(execution_id = %execution_id, "Metrics collection not found for resource sample");
        }
        Ok(())
    }

    /// Get current pipeline metrics
    pub async fn get_pipeline_metrics(&self, execution_id: Uuid) -> Result<PipelineMetrics> {
        let active = self.active_collections.read().await;
        if let Some(collection) = active.get(&execution_id) {
            Ok(collection.pipeline_metrics.clone())
        } else {
            Err(AppError::NotFound("Pipeline metrics not found".to_string()))
        }
    }

    /// Get stage metrics
    pub async fn get_stage_metrics(
        &self,
        execution_id: Uuid,
        stage_name: &str,
    ) -> Result<StageMetrics> {
        let active = self.active_collections.read().await;
        if let Some(collection) = active.get(&execution_id) {
            if let Some(metrics) = collection.stage_metrics.get(stage_name) {
                Ok(metrics.clone())
            } else {
                Err(AppError::NotFound("Stage metrics not found".to_string()))
            }
        } else {
            Err(AppError::NotFound(
                "Metrics collection not found".to_string(),
            ))
        }
    }

    /// Get step metrics
    pub async fn get_step_metrics(
        &self,
        execution_id: Uuid,
        stage_name: &str,
        step_name: &str,
    ) -> Result<StepMetrics> {
        let active = self.active_collections.read().await;
        if let Some(collection) = active.get(&execution_id) {
            let step_key = format!("{}::{}", stage_name, step_name);
            if let Some(metrics) = collection.step_metrics.get(&step_key) {
                Ok(metrics.clone())
            } else {
                Err(AppError::NotFound("Step metrics not found".to_string()))
            }
        } else {
            Err(AppError::NotFound(
                "Metrics collection not found".to_string(),
            ))
        }
    }

    /// Get aggregated metrics
    pub async fn get_aggregated_metrics(&self) -> AggregatedMetrics {
        let aggregated = self.aggregated_metrics.read().await;
        aggregated.clone()
    }

    /// Finalize metrics calculation
    async fn finalize_metrics(&self, collection: &mut MetricsCollection) {
        let duration = Utc::now() - collection.started_at;
        collection.pipeline_metrics.total_duration_ms = duration.num_milliseconds() as u64;
        collection.pipeline_metrics.execution_time_ms =
            collection.pipeline_metrics.total_duration_ms;

        // Calculate success rate
        if collection.pipeline_metrics.total_steps > 0 {
            collection.pipeline_metrics.success_rate = (collection.pipeline_metrics.completed_steps
                - collection.pipeline_metrics.failed_steps)
                as f64
                / collection.pipeline_metrics.total_steps as f64;
        }

        // Calculate throughput
        if collection.pipeline_metrics.total_duration_ms > 0 {
            collection.pipeline_metrics.throughput_steps_per_minute =
                collection.pipeline_metrics.completed_steps as f64
                    / (collection.pipeline_metrics.total_duration_ms as f64 / 60000.0);
        }

        // Calculate resource efficiency (simplified)
        collection.pipeline_metrics.resource_efficiency =
            self.calculate_resource_efficiency(&collection.resource_samples);

        // Estimate cost (simplified)
        collection.pipeline_metrics.cost_estimate = self
            .estimate_execution_cost(&collection.pipeline_metrics, &collection.resource_samples);
    }

    /// Update aggregated metrics
    async fn update_aggregated_metrics(&self, pipeline_metrics: &PipelineMetrics) {
        let mut aggregated = self.aggregated_metrics.write().await;

        aggregated.total_executions += 1;

        if pipeline_metrics.success_rate > 0.8 {
            // Consider > 80% success rate as successful
            aggregated.successful_executions += 1;
        } else {
            aggregated.failed_executions += 1;
        }

        aggregated.total_steps_executed += pipeline_metrics.completed_steps as u64;

        // Update averages (simplified - in production, you'd use proper statistical methods)
        aggregated.average_execution_time_ms = (aggregated.average_execution_time_ms
            * (aggregated.total_executions - 1) as f64
            + pipeline_metrics.total_duration_ms as f64)
            / aggregated.total_executions as f64;

        aggregated.average_steps_per_execution =
            aggregated.total_steps_executed as f64 / aggregated.total_executions as f64;

        aggregated.overall_success_rate =
            aggregated.successful_executions as f64 / aggregated.total_executions as f64;

        // Update cost metrics
        aggregated.cost_metrics.total_cost_usd += pipeline_metrics.cost_estimate;
        aggregated.cost_metrics.average_cost_per_execution =
            aggregated.cost_metrics.total_cost_usd / aggregated.total_executions as f64;
    }

    /// Calculate step performance score
    fn calculate_step_performance_score(&self, duration_ms: u64, exit_code: Option<i32>) -> f64 {
        let base_score = if exit_code == Some(0) || exit_code.is_none() {
            100.0
        } else {
            0.0
        };

        // Adjust score based on duration (faster is better)
        let duration_penalty = (duration_ms as f64 / 1000.0).min(50.0); // Max 50 point penalty

        (base_score - duration_penalty).max(0.0)
    }

    /// Calculate resource efficiency
    fn calculate_resource_efficiency(&self, samples: &[ResourceSample]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }

        let avg_cpu = samples.iter().map(|s| s.cpu_percent).sum::<f64>() / samples.len() as f64;
        let avg_memory_utilization =
            samples.iter().map(|s| s.memory_mb as f64).sum::<f64>() / samples.len() as f64;

        // Simple efficiency calculation (in production, this would be more sophisticated)
        ((avg_cpu + avg_memory_utilization / 1024.0) / 2.0).min(100.0)
    }

    /// Estimate execution cost
    fn estimate_execution_cost(
        &self,
        metrics: &PipelineMetrics,
        samples: &[ResourceSample],
    ) -> f64 {
        // Simplified cost calculation
        let duration_hours = metrics.total_duration_ms as f64 / (1000.0 * 60.0 * 60.0);
        let compute_cost_per_hour = 0.10; // $0.10 per hour base rate

        let avg_resource_usage = if !samples.is_empty() {
            samples
                .iter()
                .map(|s| (s.cpu_percent + s.memory_mb as f64 / 1024.0) / 2.0)
                .sum::<f64>()
                / samples.len() as f64
        } else {
            50.0 // Default assumption
        };

        duration_hours * compute_cost_per_hour * (avg_resource_usage / 100.0)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_lifecycle() {
        let collector = MetricsCollector::new();
        let execution_id = Uuid::new_v4();

        // Start collection
        let result = collector.start_collection(execution_id).await;
        assert!(result.is_ok());

        // Record some metrics
        collector
            .record_pipeline_metrics(execution_id, "test-pipeline".to_string(), 2, 4)
            .await
            .unwrap();

        collector
            .record_stage_metrics(execution_id, "build".to_string(), 5000, 2, 2, 0, false)
            .await
            .unwrap();

        collector
            .record_step_metrics(
                execution_id,
                "build".to_string(),
                "compile".to_string(),
                "shell".to_string(),
                2500,
                Some(0),
                0,
                1024,
                None,
            )
            .await
            .unwrap();

        // Stop collection
        let result = collector.stop_collection(execution_id).await;
        assert!(result.is_ok());

        let metrics = result.unwrap();
        assert_eq!(metrics.pipeline_name, "test-pipeline");
        assert_eq!(metrics.total_stages, 2);
        assert_eq!(metrics.total_steps, 4);
        assert_eq!(metrics.completed_stages, 1);
        assert_eq!(metrics.completed_steps, 1);
    }

    #[tokio::test]
    async fn test_aggregated_metrics() {
        let collector = MetricsCollector::new();

        let initial_metrics = collector.get_aggregated_metrics().await;
        assert_eq!(initial_metrics.total_executions, 0);

        // Simulate completing an execution
        let execution_id = Uuid::new_v4();
        collector.start_collection(execution_id).await.unwrap();
        collector
            .record_pipeline_metrics(execution_id, "test-pipeline".to_string(), 1, 1)
            .await
            .unwrap();
        collector.stop_collection(execution_id).await.unwrap();

        let updated_metrics = collector.get_aggregated_metrics().await;
        assert_eq!(updated_metrics.total_executions, 1);
    }

    #[test]
    fn test_performance_score_calculation() {
        let collector = MetricsCollector::new();

        // Successful step with good performance
        let score1 = collector.calculate_step_performance_score(1000, Some(0));
        assert!(score1 > 90.0);

        // Failed step
        let score2 = collector.calculate_step_performance_score(1000, Some(1));
        assert_eq!(score2, 0.0);

        // Slow but successful step
        let score3 = collector.calculate_step_performance_score(60000, Some(0));
        assert!(score3 < 60.0);
    }
}
