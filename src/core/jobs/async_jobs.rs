use crate::error::{AppError, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Type alias for complex job receiver type
type JobReceiver = Arc<RwLock<Option<mpsc::UnboundedReceiver<Box<dyn Job>>>>>;

/// Job status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

/// Job priority
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Job metadata
#[derive(Debug, Clone, Serialize)]
pub struct JobMetadata {
    pub id: Uuid,
    pub name: String,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub error_message: Option<String>,
    pub progress: Option<f64>,
}

impl JobMetadata {
    pub fn new(name: String, priority: JobPriority) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            priority,
            status: JobStatus::Pending,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            max_retries: 3,
            timeout_seconds: 300, // 5 minutes default
            tags: Vec::new(),
            metadata: HashMap::new(),
            error_message: None,
            progress: None,
        }
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn duration(&self) -> Option<Duration> {
        if let (Some(started), Some(completed)) = (self.started_at, self.completed_at) {
            Some(Duration::from_secs(completed - started))
        } else {
            None
        }
    }
}

/// Job trait that all jobs must implement
#[async_trait::async_trait]
pub trait Job: Send + Sync {
    async fn execute(&self, context: &JobContext) -> Result<JobResult>;
    fn metadata(&self) -> &JobMetadata;
    fn metadata_mut(&mut self) -> &mut JobMetadata;
}

/// Job execution context
#[derive(Clone)]
pub struct JobContext {
    pub job_id: Uuid,
    pub correlation_id: Uuid,
    pub worker_id: String,
    pub attempt: u32,
    pub cancellation_token: tokio_util::sync::CancellationToken,
    pub progress_reporter: Arc<dyn ProgressReporter>,
}

/// Job execution result
#[derive(Debug, Clone, Serialize)]
pub struct JobResult {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
    pub should_retry: bool,
}

impl JobResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
            data: None,
            should_retry: false,
        }
    }

    pub fn success_with_data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: None,
            data: Some(data),
            should_retry: false,
        }
    }

    pub fn failure(message: String, should_retry: bool) -> Self {
        Self {
            success: false,
            message: Some(message),
            data: None,
            should_retry,
        }
    }
}

/// Progress reporter trait
#[async_trait::async_trait]
pub trait ProgressReporter: Send + Sync {
    async fn report_progress(&self, job_id: Uuid, progress: f64, message: Option<String>);
}

/// Job queue for managing job execution
pub struct JobQueue {
    pending_jobs: Arc<RwLock<Vec<Box<dyn Job>>>>,
    running_jobs: Arc<DashMap<Uuid, Box<dyn Job>>>,
    completed_jobs: Arc<DashMap<Uuid, JobMetadata>>,
    failed_jobs: Arc<DashMap<Uuid, JobMetadata>>,
    job_sender: mpsc::UnboundedSender<Box<dyn Job>>,
    job_receiver: JobReceiver,
    max_concurrent_jobs: usize,
    semaphore: Arc<Semaphore>,
    metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
    correlation_tracker: Arc<crate::core::patterns::correlation::CorrelationTracker>,
}

impl JobQueue {
    pub fn new(
        max_concurrent_jobs: usize,
        metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
        correlation_tracker: Arc<crate::core::patterns::correlation::CorrelationTracker>,
    ) -> Self {
        let (job_sender, job_receiver) = mpsc::unbounded_channel();

        Self {
            pending_jobs: Arc::new(RwLock::new(Vec::new())),
            running_jobs: Arc::new(DashMap::new()),
            completed_jobs: Arc::new(DashMap::new()),
            failed_jobs: Arc::new(DashMap::new()),
            job_sender,
            job_receiver: Arc::new(RwLock::new(Some(job_receiver))),
            max_concurrent_jobs,
            semaphore: Arc::new(Semaphore::new(max_concurrent_jobs)),
            metrics_collector,
            correlation_tracker,
        }
    }

    /// Submit a job to the queue
    pub async fn submit_job(&self, job: Box<dyn Job>) -> Result<Uuid> {
        let job_id = job.metadata().id;

        // Add to pending jobs
        let mut pending = self.pending_jobs.write().await;
        pending.push(job);

        // Sort by priority (highest first)
        pending.sort_by(|a, b| b.metadata().priority.cmp(&a.metadata().priority));

        info!(job_id = %job_id, "Job submitted to queue");

        // Record metrics
        if let Some(metrics) = &self.metrics_collector {
            let mut labels = HashMap::new();
            labels.insert("status".to_string(), "submitted".to_string());
            metrics.increment_counter("jobs_total", labels);
        }

        Ok(job_id)
    }

    /// Start the job processing loop
    pub async fn start_processing(&self) -> Result<()> {
        let _receiver = {
            let mut receiver_guard = self.job_receiver.write().await;
            receiver_guard.take().ok_or_else(|| {
                AppError::InternalServerError("Job queue already started".to_string())
            })?
        };

        info!(
            max_concurrent = self.max_concurrent_jobs,
            "Starting job queue processing"
        );

        // Start the main processing loop
        let pending_jobs = Arc::clone(&self.pending_jobs);
        let running_jobs = Arc::clone(&self.running_jobs);
        let completed_jobs = Arc::clone(&self.completed_jobs);
        let failed_jobs = Arc::clone(&self.failed_jobs);
        let semaphore = Arc::clone(&self.semaphore);
        let metrics_collector = self.metrics_collector.clone();
        let correlation_tracker = Arc::clone(&self.correlation_tracker);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));

            loop {
                interval.tick().await;

                // Check for available capacity
                if let Ok(permit) = semaphore.clone().try_acquire_owned() {
                    // Get next job from pending queue
                    let job = {
                        let mut pending = pending_jobs.write().await;
                        if pending.is_empty() {
                            continue;
                        }
                        pending.remove(0)
                    };

                    let job_id = job.metadata().id;
                    let job_name = job.metadata().name.clone();

                    // Move to running jobs
                    running_jobs.insert(job_id, job);

                    // Spawn job execution task
                    let running_jobs_clone = Arc::clone(&running_jobs);
                    let completed_jobs_clone = Arc::clone(&completed_jobs);
                    let failed_jobs_clone = Arc::clone(&failed_jobs);
                    let metrics_collector_clone = metrics_collector.clone();
                    let correlation_tracker_clone = Arc::clone(&correlation_tracker);

                    tokio::spawn(async move {
                        let _permit = permit; // Keep permit until job completes

                        if let Some((_, mut job)) = running_jobs_clone.remove(&job_id) {
                            let result = Self::execute_job(
                                job.as_mut(),
                                &correlation_tracker_clone,
                                &metrics_collector_clone,
                            )
                            .await;

                            let job_metadata = job.metadata().clone();

                            match result {
                                Ok(_) => {
                                    completed_jobs_clone.insert(job_id, job_metadata);
                                    info!(job_id = %job_id, job_name = %job_name, "Job completed successfully");
                                }
                                Err(e) => {
                                    let mut failed_metadata = job_metadata;
                                    failed_metadata.status = JobStatus::Failed;
                                    failed_metadata.error_message = Some(e.to_string());
                                    failed_metadata.completed_at = Some(
                                        SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs(),
                                    );

                                    failed_jobs_clone.insert(job_id, failed_metadata);
                                    error!(job_id = %job_id, job_name = %job_name, error = %e, "Job failed");
                                }
                            }
                        }
                    });
                }
            }
        });

        Ok(())
    }

    /// Execute a single job
    async fn execute_job(
        job: &mut dyn Job,
        correlation_tracker: &Arc<crate::core::patterns::correlation::CorrelationTracker>,
        metrics_collector: &Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
    ) -> Result<()> {
        let job_id = job.metadata().id;
        let job_name = job.metadata().name.clone();
        let start_time = Instant::now();

        // Update job status
        {
            let metadata = job.metadata_mut();
            metadata.status = JobStatus::Running;
            metadata.started_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
        }

        // Create job context
        let context = JobContext {
            job_id,
            correlation_id: Uuid::new_v4(),
            worker_id: format!("worker-{}", fastrand::u32(..)),
            attempt: job.metadata().retry_count + 1,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            progress_reporter: Arc::new(DefaultProgressReporter::new(job_id)),
        };

        // Start correlation tracking
        correlation_tracker
            .start_correlation(context.correlation_id)
            .await;

        // Execute job with timeout
        let timeout_duration = Duration::from_secs(job.metadata().timeout_seconds);
        let execution_result = tokio::time::timeout(timeout_duration, job.execute(&context)).await;

        let result = match execution_result {
            Ok(Ok(job_result)) => {
                if job_result.success {
                    // Update job status
                    let metadata = job.metadata_mut();
                    metadata.status = JobStatus::Completed;
                    metadata.completed_at = Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    );

                    Ok(())
                } else {
                    Err(AppError::InternalServerError(
                        job_result
                            .message
                            .unwrap_or_else(|| "Job execution failed".to_string()),
                    ))
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // Timeout occurred
                Err(AppError::TimeoutError(format!(
                    "Job {} timed out after {} seconds",
                    job_name,
                    job.metadata().timeout_seconds
                )))
            }
        };

        // End correlation tracking
        correlation_tracker.end_correlation().await;

        // Record metrics
        if let Some(metrics) = metrics_collector {
            let duration = start_time.elapsed();
            let mut labels = HashMap::new();
            labels.insert("job_name".to_string(), job_name.clone());
            labels.insert(
                "status".to_string(),
                if result.is_ok() { "success" } else { "failure" }.to_string(),
            );

            metrics.record_timing("job_execution_duration_seconds", duration, labels.clone());
            metrics.increment_counter("job_executions_total", labels);
        }

        result
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: Uuid) -> Option<JobStatus> {
        // Check running jobs
        if let Some(job) = self.running_jobs.get(&job_id) {
            return Some(job.metadata().status.clone());
        }

        // Check completed jobs
        if let Some(metadata) = self.completed_jobs.get(&job_id) {
            return Some(metadata.status.clone());
        }

        // Check failed jobs
        if let Some(metadata) = self.failed_jobs.get(&job_id) {
            return Some(metadata.status.clone());
        }

        // Check pending jobs
        let pending = self.pending_jobs.read().await;
        for job in pending.iter() {
            if job.metadata().id == job_id {
                return Some(job.metadata().status.clone());
            }
        }

        None
    }

    /// Get job metadata
    pub async fn get_job_metadata(&self, job_id: Uuid) -> Option<JobMetadata> {
        // Check running jobs
        if let Some(job) = self.running_jobs.get(&job_id) {
            return Some(job.metadata().clone());
        }

        // Check completed jobs
        if let Some(metadata) = self.completed_jobs.get(&job_id) {
            return Some(metadata.clone());
        }

        // Check failed jobs
        if let Some(metadata) = self.failed_jobs.get(&job_id) {
            return Some(metadata.clone());
        }

        // Check pending jobs
        let pending = self.pending_jobs.read().await;
        for job in pending.iter() {
            if job.metadata().id == job_id {
                return Some(job.metadata().clone());
            }
        }

        None
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<()> {
        // Try to remove from pending jobs first
        {
            let mut pending = self.pending_jobs.write().await;
            if let Some(pos) = pending.iter().position(|job| job.metadata().id == job_id) {
                let mut job = pending.remove(pos);
                job.metadata_mut().status = JobStatus::Cancelled;
                info!(job_id = %job_id, "Job cancelled from pending queue");
                return Ok(());
            }
        }

        // If job is running, we can't easily cancel it (would need cancellation token support)
        if self.running_jobs.contains_key(&job_id) {
            warn!(job_id = %job_id, "Cannot cancel running job - cancellation tokens not fully implemented");
            return Err(AppError::BadRequest(
                "Cannot cancel running job".to_string(),
            ));
        }

        Err(AppError::NotFound(format!("Job {} not found", job_id)))
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self) -> QueueStats {
        let pending_count = self.pending_jobs.read().await.len();
        let running_count = self.running_jobs.len();
        let completed_count = self.completed_jobs.len();
        let failed_count = self.failed_jobs.len();

        QueueStats {
            pending_jobs: pending_count,
            running_jobs: running_count,
            completed_jobs: completed_count,
            failed_jobs: failed_count,
            total_capacity: self.max_concurrent_jobs,
            available_capacity: self.semaphore.available_permits(),
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone, Serialize)]
pub struct QueueStats {
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub completed_jobs: usize,
    pub failed_jobs: usize,
    pub total_capacity: usize,
    pub available_capacity: usize,
}

/// Default progress reporter implementation
pub struct DefaultProgressReporter {
    job_id: Uuid,
}

impl DefaultProgressReporter {
    pub fn new(job_id: Uuid) -> Self {
        Self { job_id }
    }
}

#[async_trait::async_trait]
impl ProgressReporter for DefaultProgressReporter {
    async fn report_progress(&self, job_id: Uuid, progress: f64, message: Option<String>) {
        debug!(
            job_id = %job_id,
            progress = progress,
            message = ?message,
            "Job progress update"
        );
    }
}

/// Example job implementation
pub struct ExampleJob {
    metadata: JobMetadata,
    work_duration: Duration,
}

impl ExampleJob {
    pub fn new(name: String, work_duration: Duration) -> Self {
        Self {
            metadata: JobMetadata::new(name, JobPriority::Normal),
            work_duration,
        }
    }
}

#[async_trait::async_trait]
impl Job for ExampleJob {
    async fn execute(&self, context: &JobContext) -> Result<JobResult> {
        info!(job_id = %context.job_id, "Starting example job execution");

        // Simulate work with progress reporting
        let steps = 10;
        for i in 0..steps {
            if context.cancellation_token.is_cancelled() {
                return Ok(JobResult::failure("Job was cancelled".to_string(), false));
            }

            tokio::time::sleep(self.work_duration / steps as u32).await;

            let progress = (i + 1) as f64 / steps as f64;
            context
                .progress_reporter
                .report_progress(
                    context.job_id,
                    progress,
                    Some(format!("Step {} of {}", i + 1, steps)),
                )
                .await;
        }

        info!(job_id = %context.job_id, "Example job completed successfully");
        Ok(JobResult::success())
    }

    fn metadata(&self) -> &JobMetadata {
        &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut JobMetadata {
        &mut self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patterns::correlation::CorrelationTracker;

    #[tokio::test]
    async fn test_job_metadata() {
        let metadata = JobMetadata::new("test_job".to_string(), JobPriority::High)
            .with_max_retries(5)
            .with_timeout(60)
            .with_tags(vec!["test".to_string(), "example".to_string()]);

        assert_eq!(metadata.name, "test_job");
        assert_eq!(metadata.priority, JobPriority::High);
        assert_eq!(metadata.max_retries, 5);
        assert_eq!(metadata.timeout_seconds, 60);
        assert_eq!(metadata.tags, vec!["test", "example"]);
    }

    #[tokio::test]
    async fn test_job_queue() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let queue = JobQueue::new(2, None, correlation_tracker);

        let job = Box::new(ExampleJob::new(
            "test_job".to_string(),
            Duration::from_millis(100),
        ));
        let job_id = job.metadata().id;

        let submitted_id = queue.submit_job(job).await.unwrap();
        assert_eq!(submitted_id, job_id);

        let status = queue.get_job_status(job_id).await;
        assert_eq!(status, Some(JobStatus::Pending));

        let stats = queue.get_queue_stats().await;
        assert_eq!(stats.pending_jobs, 1);
        assert_eq!(stats.running_jobs, 0);
    }

    #[tokio::test]
    async fn test_job_priority_ordering() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let queue = JobQueue::new(1, None, correlation_tracker);

        // Submit jobs with different priorities
        let low_job = Box::new(ExampleJob::new(
            "low".to_string(),
            Duration::from_millis(10),
        ));
        let high_job = Box::new(ExampleJob::new(
            "high".to_string(),
            Duration::from_millis(10),
        ));
        let critical_job = Box::new(ExampleJob::new(
            "critical".to_string(),
            Duration::from_millis(10),
        ));

        // Modify priorities
        let mut low_job = low_job;
        low_job.metadata.priority = JobPriority::Low;

        let mut high_job = high_job;
        high_job.metadata.priority = JobPriority::High;

        let mut critical_job = critical_job;
        critical_job.metadata.priority = JobPriority::Critical;

        // Submit in reverse priority order
        queue.submit_job(low_job).await.unwrap();
        queue.submit_job(high_job).await.unwrap();
        queue.submit_job(critical_job).await.unwrap();

        // Check that jobs are ordered by priority
        let pending = queue.pending_jobs.read().await;
        assert_eq!(pending[0].metadata().priority, JobPriority::Critical);
        assert_eq!(pending[1].metadata().priority, JobPriority::High);
        assert_eq!(pending[2].metadata().priority, JobPriority::Low);
    }
}
