//! Job queue implementation for managing job execution
//!
//! This module provides a priority-based job queue with support for
//! job scheduling, prioritization, and execution tracking.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::domain::entities::{Job, JobId, JobPriority, JobStatus, RunnerId};
use crate::error::{AppError, Result};

/// Job queue entry with priority and metadata
#[derive(Debug, Clone)]
struct QueuedJob {
    job: Job,
    queued_at: DateTime<Utc>,
    attempts: u32,
    last_attempt: Option<DateTime<Utc>>,
    assigned_runner: Option<RunnerId>,
}

impl PartialEq for QueuedJob {
    fn eq(&self, other: &Self) -> bool {
        self.job.id == other.job.id
    }
}

impl Eq for QueuedJob {}

impl PartialOrd for QueuedJob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedJob {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority jobs come first
        match self.job.priority.cmp(&other.job.priority) {
            std::cmp::Ordering::Equal => {
                // If same priority, older jobs come first
                other.queued_at.cmp(&self.queued_at)
            }
            other => other,
        }
    }
}

/// Job queue statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobQueueStats {
    pub total_queued: u64,
    pub total_processed: u64,
    pub total_failed: u64,
    pub current_queue_size: usize,
    pub avg_queue_time: f64,
    pub jobs_by_priority: HashMap<String, u64>,
    pub jobs_by_status: HashMap<String, u64>,
}

/// Job queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobQueueConfig {
    /// Maximum queue size
    pub max_queue_size: usize,
    /// Maximum retry attempts
    pub max_retry_attempts: u32,
    /// Retry delay in seconds
    pub retry_delay_seconds: u64,
    /// Enable job prioritization
    pub enable_prioritization: bool,
    /// Queue processing interval in milliseconds
    pub processing_interval_ms: u64,
}

impl Default for JobQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            max_retry_attempts: 3,
            retry_delay_seconds: 30,
            enable_prioritization: true,
            processing_interval_ms: 100,
        }
    }
}

/// Job queue trait for different queue implementations
#[async_trait]
pub trait JobQueue: Send + Sync {
    /// Enqueue a job
    async fn enqueue(&self, job: Job) -> Result<()>;

    /// Dequeue the next job
    async fn dequeue(&self) -> Result<Option<Job>>;

    /// Peek at the next job without removing it
    async fn peek(&self) -> Result<Option<Job>>;

    /// Get job by ID
    async fn get_job(&self, job_id: JobId) -> Result<Option<Job>>;

    /// Remove job from queue
    async fn remove_job(&self, job_id: JobId) -> Result<bool>;

    /// Get queue size
    async fn size(&self) -> Result<usize>;

    /// Check if queue is empty
    async fn is_empty(&self) -> Result<bool>;

    /// Get queue statistics
    async fn get_stats(&self) -> Result<JobQueueStats>;

    /// Clear the queue
    async fn clear(&self) -> Result<()>;

    /// Get jobs by status
    async fn get_jobs_by_status(&self, status: JobStatus) -> Result<Vec<Job>>;

    /// Get jobs by priority
    async fn get_jobs_by_priority(&self, priority: JobPriority) -> Result<Vec<Job>>;

    /// Update job status
    async fn update_job_status(&self, job_id: JobId, status: JobStatus) -> Result<()>;

    /// Retry failed job
    async fn retry_job(&self, job_id: JobId) -> Result<()>;
}

/// In-memory priority job queue implementation
pub struct InMemoryJobQueue {
    /// Priority queue for jobs
    queue: Arc<Mutex<BinaryHeap<QueuedJob>>>,
    /// Job lookup by ID
    jobs: Arc<RwLock<HashMap<JobId, QueuedJob>>>,
    /// Queue configuration
    config: JobQueueConfig,
    /// Queue statistics
    stats: Arc<RwLock<JobQueueStats>>,
    /// Failed jobs for retry
    failed_jobs: Arc<Mutex<VecDeque<QueuedJob>>>,
}

impl InMemoryJobQueue {
    /// Create a new in-memory job queue
    pub fn new(config: JobQueueConfig) -> Self {
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(JobQueueStats::default())),
            failed_jobs: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Create with default configuration
    pub fn with_default_config() -> Self {
        Self::new(JobQueueConfig::default())
    }

    /// Get queue configuration
    pub fn config(&self) -> &JobQueueConfig {
        &self.config
    }

    /// Process retry queue
    pub async fn process_retries(&self) -> Result<()> {
        let mut failed_jobs = self.failed_jobs.lock().await;
        let now = Utc::now();
        let retry_delay = chrono::Duration::seconds(self.config.retry_delay_seconds as i64);

        let mut jobs_to_retry = Vec::new();

        // Check for jobs ready to retry
        while let Some(job) = failed_jobs.front() {
            if let Some(last_attempt) = job.last_attempt {
                if now - last_attempt >= retry_delay {
                    jobs_to_retry.push(failed_jobs.pop_front().unwrap());
                } else {
                    break;
                }
            } else {
                jobs_to_retry.push(failed_jobs.pop_front().unwrap());
            }
        }

        // Re-queue jobs for retry
        for mut job in jobs_to_retry {
            if job.attempts < self.config.max_retry_attempts {
                let job_id = job.job.id;
                let attempt_count = job.attempts + 1;
                job.attempts += 1;
                job.last_attempt = Some(now);

                let mut queue = self.queue.lock().await;
                let mut jobs = self.jobs.write().await;

                queue.push(job.clone());
                jobs.insert(job.job.id, job);

                info!("Retrying job {} (attempt {})", job_id, attempt_count);
            } else {
                warn!("Job {} exceeded max retry attempts", job.job.id);

                // Update statistics
                let mut stats = self.stats.write().await;
                stats.total_failed += 1;
            }
        }

        Ok(())
    }

    /// Get jobs ready for retry
    pub async fn get_retry_jobs(&self) -> Result<Vec<Job>> {
        let failed_jobs = self.failed_jobs.lock().await;
        Ok(failed_jobs.iter().map(|qj| qj.job.clone()).collect())
    }

    /// Update queue statistics
    async fn update_stats(&self, operation: &str) {
        let mut stats = self.stats.write().await;

        match operation {
            "enqueue" => stats.total_queued += 1,
            "dequeue" => stats.total_processed += 1,
            "failed" => stats.total_failed += 1,
            _ => {}
        }

        // Update current queue size
        let queue = self.queue.try_lock();
        if let Ok(q) = queue {
            stats.current_queue_size = q.len();
        }
    }
}

#[async_trait]
impl JobQueue for InMemoryJobQueue {
    async fn enqueue(&self, job: Job) -> Result<()> {
        // Check queue size limit
        {
            let queue = self.queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                return Err(AppError::ResourceExhausted(format!(
                    "Job queue is full (max: {})",
                    self.config.max_queue_size
                )));
            }
        }

        let queued_job = QueuedJob {
            job: job.clone(),
            queued_at: Utc::now(),
            attempts: 0,
            last_attempt: None,
            assigned_runner: None,
        };

        let mut queue = self.queue.lock().await;
        let mut jobs = self.jobs.write().await;

        queue.push(queued_job.clone());
        jobs.insert(job.id, queued_job);

        self.update_stats("enqueue").await;

        debug!("Enqueued job: {} with priority: {:?}", job.id, job.priority);
        Ok(())
    }

    async fn dequeue(&self) -> Result<Option<Job>> {
        let mut queue = self.queue.lock().await;

        if let Some(queued_job) = queue.pop() {
            let job = queued_job.job.clone();

            // Keep job in lookup table for status tracking
            let mut jobs = self.jobs.write().await;
            let mut updated_job = queued_job;
            updated_job.last_attempt = Some(Utc::now());
            jobs.insert(job.id, updated_job);

            self.update_stats("dequeue").await;

            debug!("Dequeued job: {}", job.id);
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn peek(&self) -> Result<Option<Job>> {
        let queue = self.queue.lock().await;
        Ok(queue.peek().map(|qj| qj.job.clone()))
    }

    async fn get_job(&self, job_id: JobId) -> Result<Option<Job>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.get(&job_id).map(|qj| qj.job.clone()))
    }

    async fn remove_job(&self, job_id: JobId) -> Result<bool> {
        let mut jobs = self.jobs.write().await;
        let removed = jobs.remove(&job_id).is_some();

        if removed {
            // Also remove from priority queue (expensive operation)
            let mut queue = self.queue.lock().await;
            let mut temp_jobs = Vec::new();

            while let Some(job) = queue.pop() {
                if job.job.id != job_id {
                    temp_jobs.push(job);
                }
            }

            for job in temp_jobs {
                queue.push(job);
            }

            debug!("Removed job: {}", job_id);
        }

        Ok(removed)
    }

    async fn size(&self) -> Result<usize> {
        let queue = self.queue.lock().await;
        Ok(queue.len())
    }

    async fn is_empty(&self) -> Result<bool> {
        let queue = self.queue.lock().await;
        Ok(queue.is_empty())
    }

    async fn get_stats(&self) -> Result<JobQueueStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    async fn clear(&self) -> Result<()> {
        let mut queue = self.queue.lock().await;
        let mut jobs = self.jobs.write().await;
        let mut failed_jobs = self.failed_jobs.lock().await;

        queue.clear();
        jobs.clear();
        failed_jobs.clear();

        // Reset statistics
        let mut stats = self.stats.write().await;
        *stats = JobQueueStats::default();

        info!("Job queue cleared");
        Ok(())
    }

    async fn get_jobs_by_status(&self, _status: JobStatus) -> Result<Vec<Job>> {
        // For in-memory queue, we don't track job status after dequeue
        // This would be implemented in a persistent queue
        Ok(Vec::new())
    }

    async fn get_jobs_by_priority(&self, priority: JobPriority) -> Result<Vec<Job>> {
        let jobs = self.jobs.read().await;
        let filtered_jobs: Vec<Job> = jobs
            .values()
            .filter(|qj| qj.job.priority == priority)
            .map(|qj| qj.job.clone())
            .collect();

        Ok(filtered_jobs)
    }

    async fn update_job_status(&self, job_id: JobId, status: JobStatus) -> Result<()> {
        let mut jobs = self.jobs.write().await;

        if let Some(queued_job) = jobs.get_mut(&job_id) {
            // Update job status (this would be more comprehensive in a real implementation)
            debug!("Updated job {} status to {:?}", job_id, status);

            // If job failed, move to retry queue
            if matches!(status, JobStatus::Failed) {
                let mut failed_jobs = self.failed_jobs.lock().await;
                failed_jobs.push_back(queued_job.clone());
                self.update_stats("failed").await;
            }
        }

        Ok(())
    }

    async fn retry_job(&self, job_id: JobId) -> Result<()> {
        let jobs = self.jobs.read().await;

        if let Some(queued_job) = jobs.get(&job_id) {
            if queued_job.attempts < self.config.max_retry_attempts {
                let mut retry_job = queued_job.clone();
                let attempt_count = retry_job.attempts + 1;
                retry_job.attempts += 1;
                retry_job.last_attempt = Some(Utc::now());

                drop(jobs); // Release read lock

                let mut queue = self.queue.lock().await;
                let mut jobs = self.jobs.write().await;

                queue.push(retry_job.clone());
                jobs.insert(job_id, retry_job);

                info!("Retrying job: {} (attempt {})", job_id, attempt_count);
                Ok(())
            } else {
                Err(AppError::BadRequest(format!(
                    "Job {} has exceeded maximum retry attempts",
                    job_id
                )))
            }
        } else {
            Err(AppError::NotFound(format!("Job {} not found", job_id)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{BackoffStrategy, JobRequirements, JobStep, RetryPolicy};
    use std::time::Duration;
    use uuid::Uuid;

    fn create_test_job(name: &str, priority: JobPriority) -> Job {
        Job {
            id: Uuid::new_v4(),
            pipeline_id: Uuid::new_v4(),
            name: name.to_string(),
            steps: vec![JobStep {
                name: "test-step".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: None,
                continue_on_error: false,
            }],
            requirements: JobRequirements::default(),
            priority,
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
    async fn test_job_queue_enqueue_dequeue() {
        let queue = InMemoryJobQueue::with_default_config();
        let job = create_test_job("test-job", JobPriority::Normal);
        let job_id = job.id;

        queue.enqueue(job).await.unwrap();
        assert_eq!(queue.size().await.unwrap(), 1);
        assert!(!queue.is_empty().await.unwrap());

        let dequeued = queue.dequeue().await.unwrap();
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().id, job_id);
        assert_eq!(queue.size().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_job_priority_ordering() {
        let queue = InMemoryJobQueue::with_default_config();

        let low_job = create_test_job("low-priority", JobPriority::Low);
        let high_job = create_test_job("high-priority", JobPriority::High);
        let normal_job = create_test_job("normal-priority", JobPriority::Normal);

        // Enqueue in random order
        queue.enqueue(low_job).await.unwrap();
        queue.enqueue(high_job.clone()).await.unwrap();
        queue.enqueue(normal_job.clone()).await.unwrap();

        // Should dequeue in priority order: High, Normal, Low
        let first = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(first.id, high_job.id);

        let second = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(second.id, normal_job.id);

        let third = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(third.name, "low-priority");
    }

    #[tokio::test]
    async fn test_job_queue_peek() {
        let queue = InMemoryJobQueue::with_default_config();
        let job = create_test_job("test-job", JobPriority::Normal);
        let job_id = job.id;

        queue.enqueue(job).await.unwrap();

        let peeked = queue.peek().await.unwrap();
        assert!(peeked.is_some());
        assert_eq!(peeked.unwrap().id, job_id);

        // Queue should still have the job
        assert_eq!(queue.size().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_job_queue_remove() {
        let queue = InMemoryJobQueue::with_default_config();
        let job = create_test_job("test-job", JobPriority::Normal);
        let job_id = job.id;

        queue.enqueue(job).await.unwrap();
        assert_eq!(queue.size().await.unwrap(), 1);

        let removed = queue.remove_job(job_id).await.unwrap();
        assert!(removed);
        assert_eq!(queue.size().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_job_queue_stats() {
        let queue = InMemoryJobQueue::with_default_config();
        let job = create_test_job("test-job", JobPriority::Normal);

        queue.enqueue(job).await.unwrap();
        queue.dequeue().await.unwrap();

        let stats = queue.get_stats().await.unwrap();
        assert_eq!(stats.total_queued, 1);
        assert_eq!(stats.total_processed, 1);
    }

    #[tokio::test]
    async fn test_job_queue_clear() {
        let queue = InMemoryJobQueue::with_default_config();
        let job = create_test_job("test-job", JobPriority::Normal);

        queue.enqueue(job).await.unwrap();
        assert_eq!(queue.size().await.unwrap(), 1);

        queue.clear().await.unwrap();
        assert_eq!(queue.size().await.unwrap(), 0);
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_job_queue_max_size() {
        let config = JobQueueConfig {
            max_queue_size: 2,
            ..Default::default()
        };
        let queue = InMemoryJobQueue::new(config);

        let job1 = create_test_job("job1", JobPriority::Normal);
        let job2 = create_test_job("job2", JobPriority::Normal);
        let job3 = create_test_job("job3", JobPriority::Normal);

        queue.enqueue(job1).await.unwrap();
        queue.enqueue(job2).await.unwrap();

        // Third job should fail due to size limit
        let result = queue.enqueue(job3).await;
        assert!(result.is_err());
    }
}
