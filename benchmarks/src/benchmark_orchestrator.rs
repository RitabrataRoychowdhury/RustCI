//! Benchmark Orchestrator for CI/CD Integration and Automated Performance Gates
//!
//! This module provides comprehensive benchmark orchestration with CI/CD integration,
//! automated performance gates, and scheduled performance validation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::unified_benchmark_engine::{UnifiedBenchmarkEngine, UnifiedBenchmarkResults, BenchmarkError};

/// Benchmark orchestrator with CI/CD integration
pub struct BenchmarkOrchestrator {
    config: OrchestratorConfig,
    execution_queue: Arc<RwLock<ExecutionQueue>>,
    performance_gates: Arc<PerformanceGates>,
    ci_cd_integration: Arc<CiCdIntegration>,
    scheduler: Arc<BenchmarkScheduler>,
    resource_manager: Arc<ResourceManager>,
}

/// Configuration for the benchmark orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Maximum concurrent benchmark executions
    pub max_concurrent_executions: usize,
    /// Default timeout for benchmark execution
    pub execution_timeout: Duration,
    /// Enable CI/CD integration
    pub enable_ci_cd_integration: bool,
    /// Enable automated performance gates
    pub enable_performance_gates: bool,
    /// Enable scheduled benchmarks
    pub enable_scheduled_benchmarks: bool,
    /// Resource limits for benchmark execution
    pub resource_limits: ResourceLimits,
    /// Performance gate configuration
    pub performance_gate_config: PerformanceGateConfig,
}

/// Resource limits for benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Maximum disk I/O rate in MB/s
    pub max_disk_io_mbps: f64,
    /// Maximum network bandwidth in Mbps
    pub max_network_mbps: f64,
}

/// Performance gate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceGateConfig {
    /// Performance thresholds that must be met
    pub thresholds: HashMap<String, PerformanceThreshold>,
    /// Action to take when gates fail
    pub failure_action: GateFailureAction,
    /// Enable automatic rollback on failure
    pub enable_rollback: bool,
    /// Notification settings
    pub notifications: GateNotificationConfig,
}

/// Performance threshold definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThreshold {
    pub metric_name: String,
    pub threshold_value: f64,
    pub comparison: ThresholdComparison,
    pub weight: f64,
    pub critical: bool,
}

/// Threshold comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThresholdComparison {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
}

/// Action to take when performance gates fail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateFailureAction {
    Block,
    Warn,
    Continue,
}

/// Gate notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateNotificationConfig {
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_channels: Vec<NotificationChannel>,
}

/// Notification channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email { recipients: Vec<String> },
    Slack { webhook_url: String, channel: String },
    Teams { webhook_url: String },
    Webhook { url: String, headers: HashMap<String, String> },
    Console,
}

/// Execution queue for managing benchmark runs
pub struct ExecutionQueue {
    pending_executions: Vec<BenchmarkExecution>,
    running_executions: HashMap<String, RunningExecution>,
    completed_executions: Vec<CompletedExecution>,
}

/// Benchmark execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkExecution {
    pub execution_id: String,
    pub execution_type: ExecutionType,
    pub priority: ExecutionPriority,
    pub config: ExecutionConfig,
    pub requested_at: DateTime<Utc>,
    pub requested_by: String,
    pub metadata: HashMap<String, String>,
}

/// Type of benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionType {
    Manual,
    Scheduled,
    CiCdTriggered,
    PerformanceGate,
    RegressionTest,
}

/// Execution priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub benchmark_suite: String,
    pub test_environment: String,
    pub resource_allocation: ResourceAllocation,
    pub timeout: Duration,
    pub retry_policy: RetryPolicy,
}

/// Resource allocation for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub cpu_cores: Option<usize>,
    pub memory_mb: Option<u64>,
    pub disk_space_mb: Option<u64>,
    pub network_bandwidth_mbps: Option<f64>,
}

/// Retry policy for failed executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub backoff_multiplier: f64,
    pub retry_on_failure_types: Vec<FailureType>,
}

/// Types of failures that can trigger retries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureType {
    ResourceExhaustion,
    NetworkTimeout,
    SystemError,
    PerformanceRegression,
}

/// Running execution tracking
pub struct RunningExecution {
    pub execution: BenchmarkExecution,
    pub started_at: DateTime<Utc>,
    pub engine: Arc<UnifiedBenchmarkEngine>,
    pub handle: tokio::task::JoinHandle<Result<UnifiedBenchmarkResults, BenchmarkError>>,
    pub resource_usage: ResourceUsageTracker,
}

/// Completed execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedExecution {
    pub execution: BenchmarkExecution,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration: Duration,
    pub result: ExecutionResult,
    pub resource_usage: ResourceUsageSummary,
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    Success {
        results: UnifiedBenchmarkResults,
        gate_status: Option<GateEvaluationResult>,
    },
    Failure {
        error: String,
        failure_type: FailureType,
        retry_count: u32,
    },
    Timeout {
        duration: Duration,
    },
    Cancelled {
        reason: String,
    },
}

/// Performance gates evaluation system
pub struct PerformanceGates {
    config: PerformanceGateConfig,
}

/// Gate evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEvaluationResult {
    pub overall_status: GateStatus,
    pub threshold_results: Vec<ThresholdEvaluationResult>,
    pub score: f64,
    pub recommendations: Vec<String>,
    pub evaluation_time: DateTime<Utc>,
}

/// Gate status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateStatus {
    Passed,
    Failed,
    Warning,
}

/// Individual threshold evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdEvaluationResult {
    pub threshold: PerformanceThreshold,
    pub actual_value: f64,
    pub passed: bool,
    pub deviation_percentage: f64,
    pub impact_score: f64,
}

/// CI/CD integration system
pub struct CiCdIntegration {
    config: CiCdConfig,
}

/// CI/CD configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiCdConfig {
    pub provider: CiCdProvider,
    pub webhook_endpoints: Vec<WebhookEndpoint>,
    pub status_reporting: StatusReportingConfig,
    pub artifact_storage: ArtifactStorageConfig,
}

/// Supported CI/CD providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CiCdProvider {
    GitHub,
    GitLab,
    Jenkins,
    CircleCI,
    TravisCI,
    AzureDevOps,
    Generic,
}

/// Webhook endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub name: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub events: Vec<WebhookEvent>,
}

/// Webhook events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookEvent {
    ExecutionStarted,
    ExecutionCompleted,
    GatePassed,
    GateFailed,
    RegressionDetected,
}

/// Status reporting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReportingConfig {
    pub report_to_pr: bool,
    pub report_to_commit: bool,
    pub create_check_run: bool,
    pub update_deployment_status: bool,
}

/// Artifact storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStorageConfig {
    pub store_results: bool,
    pub store_reports: bool,
    pub storage_backend: StorageBackend,
    pub retention_days: u32,
}

/// Storage backend options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    Local { path: String },
    S3 { bucket: String, region: String },
    Azure { container: String, account: String },
    GCS { bucket: String, project: String },
}

/// Benchmark scheduler for automated runs
pub struct BenchmarkScheduler {
    config: SchedulerConfig,
    scheduled_jobs: Arc<RwLock<Vec<ScheduledJob>>>,
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub enable_scheduler: bool,
    pub default_schedule: String, // Cron expression
    pub timezone: String,
    pub max_concurrent_scheduled: usize,
}

/// Scheduled benchmark job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub job_id: String,
    pub name: String,
    pub schedule: String, // Cron expression
    pub execution_config: ExecutionConfig,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: DateTime<Utc>,
}

/// Resource manager for execution environment
pub struct ResourceManager {
    limits: ResourceLimits,
    current_usage: Arc<RwLock<ResourceUsage>>,
    semaphore: Arc<Semaphore>,
}

/// Current resource usage
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_mb: u64,
    pub disk_io_mbps: f64,
    pub network_mbps: f64,
    pub active_executions: usize,
}

/// Resource usage tracker for individual executions
pub struct ResourceUsageTracker {
    start_time: Instant,
    peak_cpu: f64,
    peak_memory: u64,
    total_disk_io: u64,
    total_network_io: u64,
}

/// Resource usage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsageSummary {
    pub duration: Duration,
    pub peak_cpu_percent: f64,
    pub peak_memory_mb: u64,
    pub total_disk_io_mb: u64,
    pub total_network_io_mb: u64,
    pub average_cpu_percent: f64,
    pub average_memory_mb: u64,
}

impl BenchmarkOrchestrator {
    /// Create a new benchmark orchestrator
    pub fn new() -> Self {
        let config = OrchestratorConfig::default();
        let execution_queue = Arc::new(RwLock::new(ExecutionQueue::new()));
        let performance_gates = Arc::new(PerformanceGates::new(config.performance_gate_config.clone()));
        let ci_cd_integration = Arc::new(CiCdIntegration::new(CiCdConfig::default()));
        let scheduler = Arc::new(BenchmarkScheduler::new(SchedulerConfig::default()));
        let resource_manager = Arc::new(ResourceManager::new(config.resource_limits.clone()));

        Self {
            config,
            execution_queue,
            performance_gates,
            ci_cd_integration,
            scheduler,
            resource_manager,
        }
    }

    /// Submit a benchmark execution request
    pub async fn submit_execution(&self, execution: BenchmarkExecution) -> Result<String, BenchmarkError> {
        println!("🚀 Submitting benchmark execution: {}", execution.execution_id);

        // Validate execution request
        self.validate_execution_request(&execution).await?;

        // Check resource availability
        self.resource_manager.check_availability(&execution.config.resource_allocation).await?;

        // Add to execution queue
        let mut queue = self.execution_queue.write().await;
        queue.add_execution(execution.clone());

        // Trigger execution processing
        self.process_execution_queue().await?;

        Ok(execution.execution_id)
    }

    /// Process the execution queue
    async fn process_execution_queue(&self) -> Result<(), BenchmarkError> {
        let mut queue = self.execution_queue.write().await;
        
        // Check if we can start new executions
        let running_count = queue.running_executions.len();
        if running_count >= self.config.max_concurrent_executions {
            return Ok(()); // Queue is full
        }

        // Sort pending executions by priority
        queue.pending_executions.sort_by(|a, b| {
            let priority_order = |p: &ExecutionPriority| match p {
                ExecutionPriority::Critical => 0,
                ExecutionPriority::High => 1,
                ExecutionPriority::Normal => 2,
                ExecutionPriority::Low => 3,
            };
            priority_order(&a.priority).cmp(&priority_order(&b.priority))
        });

        // Start executions up to the limit
        let available_slots = self.config.max_concurrent_executions - running_count;
        let executions_to_start = queue.pending_executions
            .drain(0..available_slots.min(queue.pending_executions.len()))
            .collect::<Vec<_>>();

        drop(queue); // Release the lock before starting executions

        for execution in executions_to_start {
            self.start_execution(execution).await?;
        }

        Ok(())
    }

    /// Start a benchmark execution
    async fn start_execution(&self, execution: BenchmarkExecution) -> Result<(), BenchmarkError> {
        println!("▶️  Starting benchmark execution: {}", execution.execution_id);

        // Acquire resources
        let _permit = self.resource_manager.semaphore.acquire().await
            .map_err(|e| BenchmarkError::Execution(format!("Failed to acquire resources: {}", e)))?;

        // Create benchmark engine with execution config
        let engine = Arc::new(UnifiedBenchmarkEngine::new(
            crate::unified_benchmark_engine::UnifiedBenchmarkConfig::default()
        ));

        // Start resource usage tracking
        let resource_tracker = ResourceUsageTracker::new();

        // Notify CI/CD of execution start
        if self.config.enable_ci_cd_integration {
            self.ci_cd_integration.notify_execution_started(&execution).await?;
        }

        // Start the benchmark execution
        let engine_clone = Arc::clone(&engine);
        let execution_id = execution.execution_id.clone();
        let timeout = execution.config.timeout;

        let handle = tokio::spawn(async move {
            // Set up timeout
            let result = tokio::time::timeout(timeout, engine_clone.run_complete_suite()).await;
            
            match result {
                Ok(benchmark_result) => benchmark_result,
                Err(_) => Err(BenchmarkError::Execution("Execution timed out".to_string())),
            }
        });

        // Track the running execution
        let running_execution = RunningExecution {
            execution: execution.clone(),
            started_at: Utc::now(),
            engine,
            handle,
            resource_usage: resource_tracker,
        };

        let mut queue = self.execution_queue.write().await;
        queue.running_executions.insert(execution_id, running_execution);

        Ok(())
    }

    /// Monitor running executions and handle completion
    pub async fn monitor_executions(&self) -> Result<(), BenchmarkError> {
        let mut completed_executions = Vec::new();
        
        {
            let mut queue = self.execution_queue.write().await;
            let mut to_remove = Vec::new();

            for (execution_id, running_execution) in &mut queue.running_executions {
                if running_execution.handle.is_finished() {
                    to_remove.push(execution_id.clone());
                }
            }

            for execution_id in to_remove {
                if let Some(running_execution) = queue.running_executions.remove(&execution_id) {
                    let completed = self.handle_execution_completion(running_execution).await?;
                    completed_executions.push(completed);
                }
            }
        }

        // Process completed executions
        for completed in completed_executions {
            self.process_completed_execution(completed).await?;
        }

        // Try to start more executions
        self.process_execution_queue().await?;

        Ok(())
    }

    /// Handle execution completion
    async fn handle_execution_completion(&self, running_execution: RunningExecution) -> Result<CompletedExecution, BenchmarkError> {
        let completed_at = Utc::now();
        let duration = completed_at.signed_duration_since(running_execution.started_at)
            .to_std()
            .unwrap_or_default();

        // Get the execution result
        let result = match running_execution.handle.await {
            Ok(Ok(benchmark_results)) => {
                // Evaluate performance gates if enabled
                let gate_status = if self.config.enable_performance_gates {
                    Some(self.performance_gates.evaluate(&benchmark_results).await?)
                } else {
                    None
                };

                ExecutionResult::Success {
                    results: benchmark_results,
                    gate_status,
                }
            }
            Ok(Err(error)) => {
                ExecutionResult::Failure {
                    error: error.to_string(),
                    failure_type: FailureType::SystemError,
                    retry_count: 0,
                }
            }
            Err(_) => {
                ExecutionResult::Timeout { duration }
            }
        };

        // Get resource usage summary
        let resource_usage = running_execution.resource_usage.get_summary();

        Ok(CompletedExecution {
            execution: running_execution.execution,
            started_at: running_execution.started_at,
            completed_at,
            duration,
            result,
            resource_usage,
        })
    }

    /// Process completed execution
    async fn process_completed_execution(&self, completed: CompletedExecution) -> Result<(), BenchmarkError> {
        println!("✅ Completed benchmark execution: {}", completed.execution.execution_id);

        // Add to completed executions
        {
            let mut queue = self.execution_queue.write().await;
            queue.completed_executions.push(completed.clone());
        }

        // Notify CI/CD of completion
        if self.config.enable_ci_cd_integration {
            self.ci_cd_integration.notify_execution_completed(&completed).await?;
        }

        // Handle performance gate results
        if let ExecutionResult::Success { gate_status: Some(gate_result), .. } = &completed.result {
            self.handle_gate_result(&completed.execution, gate_result).await?;
        }

        Ok(())
    }

    /// Handle performance gate evaluation result
    async fn handle_gate_result(&self, execution: &BenchmarkExecution, gate_result: &GateEvaluationResult) -> Result<(), BenchmarkError> {
        match gate_result.overall_status {
            GateStatus::Passed => {
                println!("✅ Performance gates PASSED for execution: {}", execution.execution_id);
                
                if self.config.performance_gate_config.notifications.notify_on_success {
                    self.send_gate_notification(execution, gate_result, true).await?;
                }
            }
            GateStatus::Failed => {
                println!("❌ Performance gates FAILED for execution: {}", execution.execution_id);
                
                if self.config.performance_gate_config.notifications.notify_on_failure {
                    self.send_gate_notification(execution, gate_result, false).await?;
                }

                // Handle failure action
                match self.config.performance_gate_config.failure_action {
                    GateFailureAction::Block => {
                        return Err(BenchmarkError::Execution("Performance gates failed - blocking execution".to_string()));
                    }
                    GateFailureAction::Warn => {
                        println!("⚠️  Performance gates failed but continuing due to configuration");
                    }
                    GateFailureAction::Continue => {
                        // Do nothing, just continue
                    }
                }
            }
            GateStatus::Warning => {
                println!("⚠️  Performance gates WARNING for execution: {}", execution.execution_id);
            }
        }

        Ok(())
    }

    /// Send gate notification
    async fn send_gate_notification(&self, _execution: &BenchmarkExecution, _gate_result: &GateEvaluationResult, _success: bool) -> Result<(), BenchmarkError> {
        // Implementation for sending notifications
        Ok(())
    }

    /// Validate execution request
    async fn validate_execution_request(&self, _execution: &BenchmarkExecution) -> Result<(), BenchmarkError> {
        // Implementation for validating execution requests
        Ok(())
    }

    /// Get execution status
    pub async fn get_execution_status(&self, execution_id: &str) -> Option<ExecutionStatus> {
        let queue = self.execution_queue.read().await;
        
        // Check running executions
        if let Some(running) = queue.running_executions.get(execution_id) {
            return Some(ExecutionStatus::Running {
                started_at: running.started_at,
                duration: Utc::now().signed_duration_since(running.started_at).to_std().unwrap_or_default(),
            });
        }

        // Check completed executions
        if let Some(completed) = queue.completed_executions.iter().find(|e| e.execution.execution_id == execution_id) {
            return Some(ExecutionStatus::Completed {
                result: completed.result.clone(),
                duration: completed.duration,
            });
        }

        // Check pending executions
        if queue.pending_executions.iter().any(|e| e.execution_id == execution_id) {
            return Some(ExecutionStatus::Pending);
        }

        None
    }
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running {
        started_at: DateTime<Utc>,
        duration: Duration,
    },
    Completed {
        result: ExecutionResult,
        duration: Duration,
    },
}

impl ExecutionQueue {
    pub fn new() -> Self {
        Self {
            pending_executions: Vec::new(),
            running_executions: HashMap::new(),
            completed_executions: Vec::new(),
        }
    }

    pub fn add_execution(&mut self, execution: BenchmarkExecution) {
        self.pending_executions.push(execution);
    }
}

impl PerformanceGates {
    pub fn new(config: PerformanceGateConfig) -> Self {
        Self { config }
    }

    pub async fn evaluate(&self, results: &UnifiedBenchmarkResults) -> Result<GateEvaluationResult, BenchmarkError> {
        let mut threshold_results = Vec::new();
        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        for threshold in self.config.thresholds.values() {
            let actual_value = self.extract_metric_value(results, &threshold.metric_name)?;
            let passed = self.evaluate_threshold(threshold, actual_value);
            
            let deviation_percentage = if threshold.threshold_value != 0.0 {
                ((actual_value - threshold.threshold_value) / threshold.threshold_value * 100.0).abs()
            } else {
                0.0
            };

            let impact_score = if passed { threshold.weight } else { 0.0 };
            total_score += impact_score;
            total_weight += threshold.weight;

            threshold_results.push(ThresholdEvaluationResult {
                threshold: threshold.clone(),
                actual_value,
                passed,
                deviation_percentage,
                impact_score,
            });
        }

        let overall_score = if total_weight > 0.0 { total_score / total_weight } else { 0.0 };
        let failed_critical = threshold_results.iter().any(|r| !r.passed && r.threshold.critical);
        let failed_any = threshold_results.iter().any(|r| !r.passed);

        let overall_status = if failed_critical {
            GateStatus::Failed
        } else if failed_any {
            GateStatus::Warning
        } else {
            GateStatus::Passed
        };

        let recommendations = self.generate_recommendations(&threshold_results);

        Ok(GateEvaluationResult {
            overall_status,
            threshold_results,
            score: overall_score,
            recommendations,
            evaluation_time: Utc::now(),
        })
    }

    fn extract_metric_value(&self, results: &UnifiedBenchmarkResults, metric_name: &str) -> Result<f64, BenchmarkError> {
        match metric_name {
            "http_bridge_latency_mean" => Ok(results.http_bridge.latency_stats.mean_us),
            "protocol_core_latency_mean" => Ok(results.protocol_core.latency_stats.mean_us),
            "quic_throughput_gbps" => Ok(results.transport.quic.throughput_gbps),
            "unix_socket_throughput_gbps" => Ok(results.transport.unix_socket.throughput_gbps),
            "security_encryption_latency" => Ok(results.security.encryption_latency_us),
            "e2e_pipeline_latency" => Ok(results.end_to_end.pipeline_latency_ms),
            _ => Err(BenchmarkError::Analysis(format!("Unknown metric: {}", metric_name))),
        }
    }

    fn evaluate_threshold(&self, threshold: &PerformanceThreshold, actual_value: f64) -> bool {
        match threshold.comparison {
            ThresholdComparison::LessThan => actual_value < threshold.threshold_value,
            ThresholdComparison::LessThanOrEqual => actual_value <= threshold.threshold_value,
            ThresholdComparison::GreaterThan => actual_value > threshold.threshold_value,
            ThresholdComparison::GreaterThanOrEqual => actual_value >= threshold.threshold_value,
            ThresholdComparison::Equal => (actual_value - threshold.threshold_value).abs() < f64::EPSILON,
            ThresholdComparison::NotEqual => (actual_value - threshold.threshold_value).abs() > f64::EPSILON,
        }
    }

    fn generate_recommendations(&self, threshold_results: &[ThresholdEvaluationResult]) -> Vec<String> {
        let mut recommendations = Vec::new();

        for result in threshold_results {
            if !result.passed {
                let recommendation = match result.threshold.metric_name.as_str() {
                    "http_bridge_latency_mean" => "Consider optimizing HTTP bridge performance or increasing timeout thresholds",
                    "protocol_core_latency_mean" => "Review protocol core implementation for performance bottlenecks",
                    "quic_throughput_gbps" => "Check QUIC transport configuration and network conditions",
                    "unix_socket_throughput_gbps" => "Verify Unix socket implementation and system limits",
                    "security_encryption_latency" => "Optimize encryption algorithms or consider hardware acceleration",
                    "e2e_pipeline_latency" => "Analyze end-to-end pipeline for optimization opportunities",
                    _ => "Review metric implementation and thresholds",
                };
                
                recommendations.push(format!("{}: {}", result.threshold.metric_name, recommendation));
            }
        }

        if recommendations.is_empty() {
            recommendations.push("All performance gates passed successfully".to_string());
        }

        recommendations
    }
}

impl CiCdIntegration {
    pub fn new(config: CiCdConfig) -> Self {
        Self { config }
    }

    pub async fn notify_execution_started(&self, _execution: &BenchmarkExecution) -> Result<(), BenchmarkError> {
        // Implementation for CI/CD notification
        Ok(())
    }

    pub async fn notify_execution_completed(&self, _completed: &CompletedExecution) -> Result<(), BenchmarkError> {
        // Implementation for CI/CD notification
        Ok(())
    }
}

impl BenchmarkScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            scheduled_jobs: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl ResourceManager {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            current_usage: Arc::new(RwLock::new(ResourceUsage::default())),
            semaphore: Arc::new(Semaphore::new(10)), // Default concurrency limit
        }
    }

    pub async fn check_availability(&self, _allocation: &ResourceAllocation) -> Result<(), BenchmarkError> {
        // Implementation for resource availability checking
        Ok(())
    }
}

impl ResourceUsageTracker {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            peak_cpu: 0.0,
            peak_memory: 0,
            total_disk_io: 0,
            total_network_io: 0,
        }
    }

    pub fn get_summary(&self) -> ResourceUsageSummary {
        ResourceUsageSummary {
            duration: self.start_time.elapsed(),
            peak_cpu_percent: self.peak_cpu,
            peak_memory_mb: self.peak_memory,
            total_disk_io_mb: self.total_disk_io,
            total_network_io_mb: self.total_network_io,
            average_cpu_percent: self.peak_cpu / 2.0, // Simplified
            average_memory_mb: self.peak_memory / 2, // Simplified
        }
    }
}

// Default implementations
impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 3,
            execution_timeout: Duration::from_secs(3600), // 1 hour
            enable_ci_cd_integration: false,
            enable_performance_gates: true,
            enable_scheduled_benchmarks: false,
            resource_limits: ResourceLimits::default(),
            performance_gate_config: PerformanceGateConfig::default(),
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_percent: 80.0,
            max_memory_mb: 8192, // 8GB
            max_disk_io_mbps: 1000.0,
            max_network_mbps: 1000.0,
        }
    }
}

impl Default for PerformanceGateConfig {
    fn default() -> Self {
        let mut thresholds = HashMap::new();
        
        thresholds.insert("http_bridge_latency_mean".to_string(), PerformanceThreshold {
            metric_name: "http_bridge_latency_mean".to_string(),
            threshold_value: 500.0,
            comparison: ThresholdComparison::LessThan,
            weight: 1.0,
            critical: true,
        });
        
        thresholds.insert("protocol_core_latency_mean".to_string(), PerformanceThreshold {
            metric_name: "protocol_core_latency_mean".to_string(),
            threshold_value: 100.0,
            comparison: ThresholdComparison::LessThan,
            weight: 1.0,
            critical: true,
        });

        Self {
            thresholds,
            failure_action: GateFailureAction::Block,
            enable_rollback: false,
            notifications: GateNotificationConfig {
                notify_on_success: true,
                notify_on_failure: true,
                notification_channels: vec![NotificationChannel::Console],
            },
        }
    }
}

impl Default for CiCdConfig {
    fn default() -> Self {
        Self {
            provider: CiCdProvider::Generic,
            webhook_endpoints: Vec::new(),
            status_reporting: StatusReportingConfig {
                report_to_pr: false,
                report_to_commit: false,
                create_check_run: false,
                update_deployment_status: false,
            },
            artifact_storage: ArtifactStorageConfig {
                store_results: true,
                store_reports: true,
                storage_backend: StorageBackend::Local { path: "./benchmark_artifacts".to_string() },
                retention_days: 30,
            },
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enable_scheduler: false,
            default_schedule: "0 0 * * *".to_string(), // Daily at midnight
            timezone: "UTC".to_string(),
            max_concurrent_scheduled: 1,
        }
    }
}