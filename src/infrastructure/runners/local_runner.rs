//! Local runner implementation for executing jobs on the local machine
//!
//! This module provides a local runner that executes jobs directly on the
//! host machine using process spawning and integrates with the event loop.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{timeout, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::event_loop::{Event, EventDemultiplexer, EventHandler, EventPayload, EventType};
use crate::core::job_queue::{InMemoryJobQueue, JobQueue};
use crate::domain::entities::{
    HealthStatus, Job, JobId, JobResult, JobStatus, Runner, RunnerCapacity, RunnerId,
    RunnerMetadata, RunnerType, StepResult,
};
use crate::error::{AppError, Result};

/// Local runner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRunnerConfig {
    /// Runner name
    pub name: String,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Working directory for job execution
    pub working_directory: PathBuf,
    /// Environment variables to set for all jobs
    pub environment: HashMap<String, String>,
    /// Default job timeout in seconds
    pub default_timeout_seconds: u64,
    /// Enable job output streaming
    pub enable_output_streaming: bool,
    /// Maximum output buffer size
    pub max_output_buffer_size: usize,
    /// Shell to use for command execution
    pub shell: String,
}

impl Default for LocalRunnerConfig {
    fn default() -> Self {
        Self {
            name: "local-runner".to_string(),
            max_concurrent_jobs: 4,
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
            environment: HashMap::new(),
            default_timeout_seconds: 3600, // 1 hour
            enable_output_streaming: true,
            max_output_buffer_size: 1024 * 1024, // 1MB
            shell: if cfg!(windows) {
                "cmd".to_string()
            } else {
                "bash".to_string()
            },
        }
    }
}

/// Local runner implementation
pub struct LocalRunner {
    /// Runner ID
    id: RunnerId,
    /// Runner configuration
    config: LocalRunnerConfig,
    /// Currently running jobs
    running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    /// Job queue
    job_queue: Arc<dyn JobQueue>,
    /// Event demultiplexer for event handling
    event_demux: Arc<EventDemultiplexer>,
    /// Runner statistics
    stats: Arc<RwLock<LocalRunnerStats>>,
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

/// Information about a currently running job
#[derive(Debug, Clone)]
struct RunningJob {
    job: Job,
    started_at: DateTime<Utc>,
    process_id: Option<u32>,
    current_step: usize,
    output_buffer: String,
    error_buffer: String,
}

/// Local runner statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalRunnerStats {
    pub total_jobs_executed: u64,
    pub successful_jobs: u64,
    pub failed_jobs: u64,
    pub avg_execution_time: Duration,
    pub current_load: f64,
    pub uptime: Duration,
    pub last_job_completed: Option<DateTime<Utc>>,
}

/// Job completion event handler
pub struct JobCompletionHandler {
    runner_id: RunnerId,
    running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    stats: Arc<RwLock<LocalRunnerStats>>,
}

impl LocalRunner {
    /// Create a new local runner
    pub async fn new(
        config: LocalRunnerConfig,
        event_demux: Arc<EventDemultiplexer>,
    ) -> Result<Self> {
        let id = Uuid::new_v4();
        let job_queue = Arc::new(InMemoryJobQueue::with_default_config());
        let running_jobs = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(LocalRunnerStats::default()));

        // Register job completion handler
        let completion_handler = Arc::new(JobCompletionHandler {
            runner_id: id,
            running_jobs: running_jobs.clone(),
            stats: stats.clone(),
        });

        event_demux.register_handler(completion_handler).await?;

        // Ensure working directory exists
        if !config.working_directory.exists() {
            tokio::fs::create_dir_all(&config.working_directory)
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!(
                        "Failed to create working directory: {}",
                        e
                    ))
                })?;
        }

        info!(
            "Created local runner: {} with working directory: {:?}",
            config.name, config.working_directory
        );

        Ok(Self {
            id,
            config,
            running_jobs,
            job_queue,
            event_demux,
            stats,
            shutdown: Arc::new(Mutex::new(false)),
        })
    }

    /// Start the runner event loop
    pub async fn start(&self) -> Result<()> {
        info!("Starting local runner: {}", self.config.name);

        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            // Check for shutdown signal
            {
                let shutdown = self.shutdown.lock().await;
                if *shutdown {
                    info!("Local runner shutting down");
                    break;
                }
            }

            // Process queued jobs if we have capacity
            if self.has_capacity().await? {
                if let Some(job) = self.job_queue.dequeue().await? {
                    self.spawn_job_execution(job).await?;
                }
            }

            // Clean up completed jobs
            self.cleanup_completed_jobs().await?;
        }

        Ok(())
    }

    /// Check if runner has capacity for more jobs
    async fn has_capacity(&self) -> Result<bool> {
        let running_jobs = self.running_jobs.read().await;
        Ok(running_jobs.len() < self.config.max_concurrent_jobs as usize)
    }

    /// Spawn job execution in background
    async fn spawn_job_execution(&self, job: Job) -> Result<()> {
        let job_id = job.id;
        let running_job = RunningJob {
            job: job.clone(),
            started_at: Utc::now(),
            process_id: None,
            current_step: 0,
            output_buffer: String::new(),
            error_buffer: String::new(),
        };

        // Add to running jobs
        {
            let mut running_jobs = self.running_jobs.write().await;
            running_jobs.insert(job_id, running_job);
        }

        // Spawn execution task
        let _runner_id = self.id;
        let config = self.config.clone();
        let running_jobs = self.running_jobs.clone();
        let event_demux = self.event_demux.clone();

        tokio::spawn(async move {
            let result =
                Self::execute_job_internal(job.clone(), config, running_jobs.clone()).await;

            // Send completion event
            let completion_event = Event::new(
                EventType::JobCompletion,
                EventPayload::JobCompletion {
                    job_id,
                    result: match result {
                        Ok(job_result) => crate::core::event_loop::JobCompletionResult {
                            success: job_result.is_success(),
                            exit_code: job_result.exit_code,
                            output: job_result.stdout,
                            error: job_result.error_message,
                            duration: job_result.duration.unwrap_or_default(),
                        },
                        Err(e) => crate::core::event_loop::JobCompletionResult {
                            success: false,
                            exit_code: Some(-1),
                            output: String::new(),
                            error: Some(e.to_string()),
                            duration: Duration::default(),
                        },
                    },
                },
            );

            if let Err(e) = event_demux.submit_event(completion_event).await {
                error!("Failed to submit job completion event: {}", e);
            }
        });

        debug!("Spawned execution for job: {}", job_id);
        Ok(())
    }

    /// Internal job execution logic
    async fn execute_job_internal(
        job: Job,
        config: LocalRunnerConfig,
        running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    ) -> Result<JobResult> {
        let job_id = job.id;
        let mut result = JobResult::new(job_id);
        result.start();

        debug!("Executing job: {} with {} steps", job_id, job.steps.len());

        let mut step_results = Vec::new();
        let mut overall_success = true;

        for (step_index, step) in job.steps.iter().enumerate() {
            // Update current step
            {
                let mut running_jobs = running_jobs.write().await;
                if let Some(running_job) = running_jobs.get_mut(&job_id) {
                    running_job.current_step = step_index;
                }
            }

            let step_start = Instant::now();
            debug!("Executing step: {} for job: {}", step.name, job_id);

            let step_result = Self::execute_step(step, &config).await;
            let step_duration = step_start.elapsed();

            let step_success = step_result.is_ok();
            let (exit_code, stdout, stderr, error_message) = match step_result {
                Ok((code, out, err)) => (Some(code), out, err, None),
                Err(e) => (Some(-1), String::new(), String::new(), Some(e.to_string())),
            };

            let step_result = StepResult {
                name: step.name.clone(),
                status: if step_success {
                    JobStatus::Success
                } else {
                    JobStatus::Failed
                },
                exit_code,
                stdout: stdout.clone(),
                stderr: stderr.clone(),
                duration: step_duration,
                error_message,
            };

            step_results.push(step_result);

            // Update output buffers
            {
                let mut running_jobs = running_jobs.write().await;
                if let Some(running_job) = running_jobs.get_mut(&job_id) {
                    running_job.output_buffer.push_str(&stdout);
                    running_job.error_buffer.push_str(&stderr);
                }
            }

            if !step_success {
                overall_success = false;
                if !step.continue_on_error {
                    warn!(
                        "Step {} failed for job {}, stopping execution",
                        step.name, job_id
                    );
                    break;
                }
            }
        }

        // Complete the job result
        let final_status = if overall_success {
            JobStatus::Success
        } else {
            JobStatus::Failed
        };
        let exit_code = if overall_success { Some(0) } else { Some(1) };
        result.complete(final_status.clone(), exit_code);
        result.step_results = step_results;

        // Set final output
        {
            let running_jobs = running_jobs.read().await;
            if let Some(running_job) = running_jobs.get(&job_id) {
                result.stdout = running_job.output_buffer.clone();
                result.stderr = running_job.error_buffer.clone();
            }
        }

        info!("Job {} completed with status: {:?}", job_id, final_status);
        Ok(result)
    }

    /// Execute a single job step
    async fn execute_step(
        step: &crate::domain::entities::JobStep,
        config: &LocalRunnerConfig,
    ) -> Result<(i32, String, String)> {
        let working_dir = step
            .working_directory
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| config.working_directory.clone());

        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &step.command]);
            cmd
        } else {
            let mut cmd = Command::new(&config.shell);
            cmd.args(["-c", &format!("{} {}", step.command, step.args.join(" "))]);
            cmd
        };

        // Set working directory
        cmd.current_dir(&working_dir);

        // Set environment variables
        for (key, value) in &config.environment {
            cmd.env(key, value);
        }
        for (key, value) in &step.environment {
            cmd.env(key, value);
        }

        // Configure stdio
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Apply timeout
        let timeout_duration = step
            .timeout
            .unwrap_or_else(|| Duration::from_secs(config.default_timeout_seconds));

        let execution_result = timeout(timeout_duration, async {
            let mut child = cmd.spawn().map_err(|e| {
                AppError::InternalServerError(format!("Failed to spawn process: {}", e))
            })?;

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let stdout_reader = BufReader::new(stdout);
            let stderr_reader = BufReader::new(stderr);

            let mut stdout_lines = stdout_reader.lines();
            let mut stderr_lines = stderr_reader.lines();

            let mut stdout_output = String::new();
            let mut stderr_output = String::new();

            // Read output streams
            loop {
                tokio::select! {
                    line = stdout_lines.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                stdout_output.push_str(&line);
                                stdout_output.push('\n');
                                if config.enable_output_streaming {
                                    debug!("STDOUT: {}", line);
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                error!("Error reading stdout: {}", e);
                                break;
                            }
                        }
                    }
                    line = stderr_lines.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                stderr_output.push_str(&line);
                                stderr_output.push('\n');
                                if config.enable_output_streaming {
                                    debug!("STDERR: {}", line);
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                error!("Error reading stderr: {}", e);
                                break;
                            }
                        }
                    }
                    status = child.wait() => {
                        match status {
                            Ok(exit_status) => {
                                let exit_code = exit_status.code().unwrap_or(-1);
                                return Ok((exit_code, stdout_output, stderr_output));
                            }
                            Err(e) => {
                                return Err(AppError::InternalServerError(
                                    format!("Process execution failed: {}", e)
                                ));
                            }
                        }
                    }
                }
            }

            // Wait for process to complete
            let exit_status = child.wait().await.map_err(|e| {
                AppError::InternalServerError(format!("Failed to wait for process: {}", e))
            })?;

            let exit_code = exit_status.code().unwrap_or(-1);
            Ok((exit_code, stdout_output, stderr_output))
        })
        .await;

        match execution_result {
            Ok(result) => result,
            Err(_) => Err(AppError::TimeoutError(format!(
                "Step '{}' timed out after {:?}",
                step.name, timeout_duration
            ))),
        }
    }

    /// Clean up completed jobs
    async fn cleanup_completed_jobs(&self) -> Result<()> {
        // This would be more sophisticated in a real implementation
        // For now, we rely on the event handler to clean up
        Ok(())
    }

    /// Get runner statistics
    pub async fn get_stats(&self) -> LocalRunnerStats {
        self.stats.read().await.clone()
    }

    /// Get currently running jobs
    pub async fn get_running_jobs(&self) -> Vec<Job> {
        let running_jobs = self.running_jobs.read().await;
        running_jobs.values().map(|rj| rj.job.clone()).collect()
    }

    /// Send a heartbeat event
    pub async fn send_heartbeat(&self) -> Result<()> {
        let capacity = self.get_capacity().await?;
        let status = if capacity.available_slots > 0 {
            crate::domain::entities::RunnerStatus::Idle
        } else {
            crate::domain::entities::RunnerStatus::Busy
        };

        let heartbeat_event = Event::new(
            EventType::RunnerHeartbeat,
            EventPayload::RunnerHeartbeat {
                runner_id: self.id,
                status,
            },
        );

        self.event_demux.submit_event(heartbeat_event).await?;
        Ok(())
    }

    /// Start heartbeat timer
    pub async fn start_heartbeat(&self, interval: Duration) -> Result<()> {
        let runner_id = self.id;
        let event_demux = self.event_demux.clone();

        let callback = Box::new(move || {
            let event_demux = event_demux.clone();
            let runner_id = runner_id;

            tokio::spawn(async move {
                let heartbeat_event = Event::new(
                    EventType::RunnerHeartbeat,
                    EventPayload::RunnerHeartbeat {
                        runner_id,
                        status: crate::domain::entities::RunnerStatus::Active,
                    },
                );

                if let Err(e) = event_demux.submit_event(heartbeat_event).await {
                    error!("Failed to send heartbeat: {}", e);
                }
            });

            Ok(())
        });

        self.event_demux
            .schedule_recurring_timer(format!("heartbeat-{}", self.id), interval, callback)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl crate::domain::entities::runner::Runner for LocalRunner {
    async fn execute(&self, job: Job) -> Result<JobResult> {
        debug!("Queuing job for execution: {}", job.id);

        // Add job to queue
        self.job_queue.enqueue(job.clone()).await?;

        // For this implementation, we return a placeholder result
        // In a real system, this would be handled asynchronously
        let mut result = JobResult::new(job.id);
        result.status = JobStatus::Queued;
        Ok(result)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Check if working directory is accessible
        if !self.config.working_directory.exists() {
            return Ok(HealthStatus::Unhealthy {
                reason: "Working directory does not exist".to_string(),
            });
        }

        // Check current load
        let running_jobs = self.running_jobs.read().await;
        let load = running_jobs.len() as f64 / self.config.max_concurrent_jobs as f64;

        if load >= 1.0 {
            Ok(HealthStatus::Degraded {
                reason: "Runner at full capacity".to_string(),
            })
        } else {
            Ok(HealthStatus::Healthy)
        }
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        let running_jobs = self.running_jobs.read().await;
        let current_jobs = running_jobs.len() as u32;
        let max_jobs = self.config.max_concurrent_jobs;

        Ok(RunnerCapacity {
            max_concurrent_jobs: max_jobs,
            current_jobs,
            available_slots: max_jobs.saturating_sub(current_jobs),
            cpu_usage: 0.0,    // Would be implemented with system monitoring
            memory_usage: 0.0, // Would be implemented with system monitoring
            disk_usage: 0.0,   // Would be implemented with system monitoring
        })
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down local runner: {}", self.config.name);

        // Set shutdown flag
        {
            let mut shutdown = self.shutdown.lock().await;
            *shutdown = true;
        }

        // Wait for running jobs to complete (with timeout)
        let timeout_duration = Duration::from_secs(30);
        let start = Instant::now();

        while start.elapsed() < timeout_duration {
            let running_jobs = self.running_jobs.read().await;
            if running_jobs.is_empty() {
                break;
            }
            drop(running_jobs);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Force cleanup remaining jobs
        {
            let mut running_jobs = self.running_jobs.write().await;
            running_jobs.clear();
        }

        info!("Local runner shutdown complete");
        Ok(())
    }

    fn get_metadata(&self) -> RunnerMetadata {
        RunnerMetadata {
            id: self.id,
            name: self.config.name.clone(),
            runner_type: RunnerType::Local {
                max_concurrent_jobs: self.config.max_concurrent_jobs,
                working_directory: self.config.working_directory.to_string_lossy().to_string(),
            },
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_job_types: vec!["shell".to_string(), "script".to_string()],
        }
    }

    async fn can_handle_job(&self, _job: &Job) -> Result<bool> {
        // Check if we have capacity
        if !self.has_capacity().await? {
            return Ok(false);
        }

        // Check if job requirements match our capabilities
        // For local runner, we can handle most basic jobs
        Ok(true)
    }
}

#[async_trait]
impl EventHandler for JobCompletionHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::JobCompletion { job_id, result } = event.payload {
            debug!("Handling job completion for: {}", job_id);

            // Remove from running jobs
            {
                let mut running_jobs = self.running_jobs.write().await;
                running_jobs.remove(&job_id);
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_jobs_executed += 1;
                stats.last_job_completed = Some(Utc::now());

                if result.success {
                    stats.successful_jobs += 1;
                } else {
                    stats.failed_jobs += 1;
                }

                // Update average execution time
                let total_time = stats.avg_execution_time.as_nanos() as u64
                    * (stats.total_jobs_executed - 1)
                    + result.duration.as_nanos() as u64;
                stats.avg_execution_time =
                    Duration::from_nanos(total_time / stats.total_jobs_executed);
            }

            info!("Job {} completed successfully: {}", job_id, result.success);
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::JobCompletion]
    }

    fn name(&self) -> &str {
        "job-completion-handler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{BackoffStrategy, JobRequirements, JobStep, RetryPolicy};
    use std::collections::HashMap;

    fn create_test_job() -> Job {
        Job {
            id: Uuid::new_v4(),
            pipeline_id: Uuid::new_v4(),
            name: "test-job".to_string(),
            steps: vec![JobStep {
                name: "echo-step".to_string(),
                command: "echo".to_string(),
                args: vec!["hello world".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(10)),
                continue_on_error: false,
            }],
            requirements: JobRequirements::default(),
            priority: crate::domain::entities::JobPriority::Normal,
            timeout: Duration::from_secs(300),
            retry_policy: RetryPolicy {
                max_retries: 3,
                retry_delay: Duration::from_secs(30),
                backoff_strategy: BackoffStrategy::Fixed,
            },
            metadata: HashMap::new(),
            created_at: Utc::now(),
            scheduled_at: None,
        }
    }

    #[tokio::test]
    async fn test_local_runner_creation() {
        let config = LocalRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = LocalRunner::new(config, event_demux).await.unwrap();
        let metadata = runner.get_metadata();

        assert_eq!(metadata.name, "local-runner");
        assert!(matches!(metadata.runner_type, RunnerType::Local { .. }));
    }

    #[tokio::test]
    async fn test_local_runner_health_check() {
        let config = LocalRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = LocalRunner::new(config, event_demux).await.unwrap();
        let health = runner.health_check().await.unwrap();

        assert_eq!(health, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_local_runner_capacity() {
        let config = LocalRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = LocalRunner::new(config, event_demux).await.unwrap();
        let capacity = runner.get_capacity().await.unwrap();

        assert_eq!(capacity.max_concurrent_jobs, 4);
        assert_eq!(capacity.current_jobs, 0);
        assert_eq!(capacity.available_slots, 4);
    }

    #[tokio::test]
    async fn test_local_runner_job_execution() {
        let config = LocalRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = LocalRunner::new(config, event_demux).await.unwrap();
        let job = create_test_job();

        let result = runner.execute(job).await.unwrap();
        assert_eq!(result.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_local_runner_can_handle_job() {
        let config = LocalRunnerConfig::default();
        let event_demux = Arc::new(EventDemultiplexer::new());

        let runner = LocalRunner::new(config, event_demux).await.unwrap();
        let job = create_test_job();

        let can_handle = runner.can_handle_job(&job).await.unwrap();
        assert!(can_handle);
    }

    #[tokio::test]
    async fn test_step_execution() {
        let config = LocalRunnerConfig::default();
        let step = crate::domain::entities::JobStep {
            name: "test-step".to_string(),
            command: "echo".to_string(),
            args: vec!["test output".to_string()],
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(5)),
            continue_on_error: false,
        };

        let result = LocalRunner::execute_step(&step, &config).await.unwrap();
        assert_eq!(result.0, 0); // Exit code should be 0
        assert!(result.1.contains("test output")); // Should contain our output
    }
}
