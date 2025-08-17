// Async Job Queue with Priority Processing and Retry Logic
// Part of Task 3.1: High-Performance Runner Adapter

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore, mpsc, Notify};
use tokio::time::{sleep, timeout};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use super::valkyrie_adapter::{
    ValkyrieJob, JobId, JobPriority, ValkyrieAdapterError,
    JobStatus, JobPerformanceMetrics,
};

/// High-performance async job queue with priority processing
pub struct AsyncJobQueue {
    // Priority queues for different job priorities
    priority_queues: Arc<RwLock<HashMap<JobPriority, tokio::sync::broadcast::Sender<QueuedJob>>>>,
    // Job status tracking
    job_status: Arc<RwLock<HashMap<JobId, JobStatus>>>,
    // Queue configuration
    config: JobQueueConfig,
    // Performance metrics
    metrics: Arc<JobQueueMetrics>,
    // Capacity control
    capacity_semaphore: Arc<Semaphore>,
    // Notification for queue changes
    queue_notify: Arc<Notify>,
    // Retry management
    retry_manager: Arc<RetryManager>,
}

/// Configuration for the job queue
#[derive(Debug, Clone)]
pub struct JobQueueConfig {
    pub max_capacity: usize,
    pub max_retry_attempts: u32,
    pub retry_backoff_base: Duration,
    pub retry_backoff_max: Duration,
    pub queue_timeout: Duration,
    pub priority_boost_threshold: Duration,
    pub metrics_enabled: bool,
}

impl Default for JobQueueConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            max_retry_attempts: 3,
            retry_backoff_base: Duration::from_millis(100),
            retry_backoff_max: Duration::from_secs(30),
            queue_timeout: Duration::from_secs(60),
            priority_boost_threshold: Duration::from_secs(30),
            metrics_enabled: true,
        }
    }
}

/// Job wrapper for queue management
#[derive(Debug, Clone)]
pub struct QueuedJob {
    pub job: ValkyrieJob,
    pub queued_at: Instant,
    pub retry_count: u32,
    pub next_retry_at: Option<Instant>,
    pub priority_boost: u32,
    pub correlation_id: Option<String>,
}

impl QueuedJob {
    pub fn new(job: ValkyrieJob) -> Self {
        Self {
            job,
            queued_at: Instant::now(),
            retry_count: 0,
            next_retry_at: None,
            priority_boost: 0,
            correlation_id: None,
        }
    }
    
    /// Get effective priority considering boosts and age
    pub fn effective_priority(&self) -> EffectivePriority {
        let base_priority = self.job.priority as u32;
        let age_boost = self.queued_at.elapsed().as_secs() as u32 / 10; // Boost every 10 seconds
        let retry_boost = self.retry_count * 10; // Boost retries
        let manual_boost = self.priority_boost;
        
        let total_priority = base_priority.saturating_sub(age_boost + retry_boost + manual_boost);
        
        EffectivePriority {
            value: total_priority,
            original: self.job.priority,
            boosts: age_boost + retry_boost + manual_boost,
        }
    }
}

/// Effective priority with boost tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePriority {
    pub value: u32,
    pub original: JobPriority,
    pub boosts: u32,
}

impl PartialOrd for EffectivePriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EffectivePriority {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower values have higher priority (reverse ordering)
        other.value.cmp(&self.value)
    }
}

/// Priority queue entry for heap management
#[derive(Debug, Clone)]
pub struct PriorityQueueEntry {
    pub job: QueuedJob,
    pub effective_priority: EffectivePriority,
    pub insertion_order: u64,
}

impl PartialEq for PriorityQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.effective_priority == other.effective_priority
            && self.insertion_order == other.insertion_order
    }
}

impl Eq for PriorityQueueEntry {}

impl PartialOrd for PriorityQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityQueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by effective priority
        match self.effective_priority.cmp(&other.effective_priority) {
            Ordering::Equal => {
                // If priorities are equal, use insertion order (FIFO)
                self.insertion_order.cmp(&other.insertion_order)
            }
            other => other,
        }
    }
}

/// Job queue metrics for monitoring
#[derive(Debug, Default)]
pub struct JobQueueMetrics {
    pub total_enqueued: AtomicU64,
    pub total_dequeued: AtomicU64,
    pub total_retries: AtomicU64,
    pub total_timeouts: AtomicU64,
    pub current_queue_size: AtomicU32,
    pub average_queue_time: AtomicU64, // in microseconds
    pub priority_distribution: [AtomicU32; 5], // One for each priority level
}

impl JobQueueMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_enqueue(&self, priority: JobPriority) {
        self.total_enqueued.fetch_add(1, AtomicOrdering::Relaxed);
        self.current_queue_size.fetch_add(1, AtomicOrdering::Relaxed);
        self.priority_distribution[priority as usize].fetch_add(1, AtomicOrdering::Relaxed);
    }
    
    pub fn record_dequeue(&self, queue_time: Duration) {
        self.total_dequeued.fetch_add(1, AtomicOrdering::Relaxed);
        self.current_queue_size.fetch_sub(1, AtomicOrdering::Relaxed);
        
        // Update average queue time using exponential moving average
        let queue_time_micros = queue_time.as_micros() as u64;
        let current_avg = self.average_queue_time.load(AtomicOrdering::Relaxed);
        let new_avg = if current_avg == 0 {
            queue_time_micros
        } else {
            // EMA with alpha = 0.1
            (current_avg * 9 + queue_time_micros) / 10
        };
        self.average_queue_time.store(new_avg, AtomicOrdering::Relaxed);
    }
    
    pub fn record_retry(&self) {
        self.total_retries.fetch_add(1, AtomicOrdering::Relaxed);
    }
    
    pub fn record_timeout(&self) {
        self.total_timeouts.fetch_add(1, AtomicOrdering::Relaxed);
    }
    
    pub fn get_snapshot(&self) -> JobQueueSnapshot {
        JobQueueSnapshot {
            total_enqueued: self.total_enqueued.load(AtomicOrdering::Relaxed),
            total_dequeued: self.total_dequeued.load(AtomicOrdering::Relaxed),
            total_retries: self.total_retries.load(AtomicOrdering::Relaxed),
            total_timeouts: self.total_timeouts.load(AtomicOrdering::Relaxed),
            current_queue_size: self.current_queue_size.load(AtomicOrdering::Relaxed),
            average_queue_time: Duration::from_micros(
                self.average_queue_time.load(AtomicOrdering::Relaxed)
            ),
            priority_distribution: [
                self.priority_distribution[0].load(AtomicOrdering::Relaxed),
                self.priority_distribution[1].load(AtomicOrdering::Relaxed),
                self.priority_distribution[2].load(AtomicOrdering::Relaxed),
                self.priority_distribution[3].load(AtomicOrdering::Relaxed),
                self.priority_distribution[4].load(AtomicOrdering::Relaxed),
            ],
        }
    }
}

/// Snapshot of job queue metrics
#[derive(Debug, Clone)]
pub struct JobQueueSnapshot {
    pub total_enqueued: u64,
    pub total_dequeued: u64,
    pub total_retries: u64,
    pub total_timeouts: u64,
    pub current_queue_size: u32,
    pub average_queue_time: Duration,
    pub priority_distribution: [u32; 5],
}

/// Retry management for failed jobs
pub struct RetryManager {
    retry_queue: Arc<RwLock<BinaryHeap<RetryEntry>>>,
    config: JobQueueConfig,
}

#[derive(Debug, Clone)]
pub struct RetryEntry {
    pub job: QueuedJob,
    pub retry_at: Instant,
    pub attempt: u32,
}

impl PartialEq for RetryEntry {
    fn eq(&self, other: &Self) -> bool {
        self.retry_at == other.retry_at
    }
}

impl Eq for RetryEntry {}

impl PartialOrd for RetryEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RetryEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.retry_at.cmp(&self.retry_at)
    }
}

impl RetryManager {
    pub fn new(config: JobQueueConfig) -> Self {
        Self {
            retry_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            config,
        }
    }
    
    /// Schedule a job for retry
    pub async fn schedule_retry(&self, mut job: QueuedJob) -> Result<(), ValkyrieAdapterError> {
        if job.retry_count >= self.config.max_retry_attempts {
            return Err(ValkyrieAdapterError::Internal(
                format!("Job {} exceeded maximum retry attempts", job.job.id)
            ));
        }
        
        job.retry_count += 1;
        
        // Calculate exponential backoff
        let backoff_duration = self.calculate_backoff(job.retry_count);
        let retry_at = Instant::now() + backoff_duration;
        job.next_retry_at = Some(retry_at);
        
        let retry_count = job.retry_count;
        let retry_entry = RetryEntry {
            job,
            retry_at,
            attempt: retry_count,
        };
        
        let mut retry_queue = self.retry_queue.write().await;
        retry_queue.push(retry_entry);
        
        Ok(())
    }
    
    /// Get jobs ready for retry
    pub async fn get_ready_retries(&self) -> Vec<QueuedJob> {
        let mut retry_queue = self.retry_queue.write().await;
        let mut ready_jobs = Vec::new();
        let now = Instant::now();
        
        while let Some(entry) = retry_queue.peek() {
            if entry.retry_at <= now {
                if let Some(entry) = retry_queue.pop() {
                    ready_jobs.push(entry.job);
                }
            } else {
                break;
            }
        }
        
        ready_jobs
    }
    
    /// Calculate exponential backoff with jitter
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_millis = self.config.retry_backoff_base.as_millis() as u64;
        let backoff_millis = base_millis * (2_u64.pow(attempt.saturating_sub(1)));
        
        // Add jitter (±25%)
        let jitter_range = backoff_millis / 4;
        let jitter = fastrand::u64(0..=jitter_range * 2);
        let final_backoff = backoff_millis.saturating_sub(jitter_range).saturating_add(jitter);
        
        // Cap at maximum backoff
        let max_millis = self.config.retry_backoff_max.as_millis() as u64;
        Duration::from_millis(final_backoff.min(max_millis))
    }
}

impl AsyncJobQueue {
    /// Create a new async job queue
    pub fn new(config: JobQueueConfig) -> Self {
        let capacity_semaphore = Arc::new(Semaphore::new(config.max_capacity));
        let metrics = Arc::new(JobQueueMetrics::new());
        let retry_manager = Arc::new(RetryManager::new(config.clone()));
        
        // Initialize priority queues
        let mut priority_queues = HashMap::new();
        for priority in [
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Low,
            JobPriority::Background,
        ] {
            let (tx, _rx) = tokio::sync::broadcast::channel(1000);
            priority_queues.insert(priority, tx);
        }
        
        Self {
            priority_queues: Arc::new(RwLock::new(priority_queues)),
            job_status: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics,
            capacity_semaphore,
            queue_notify: Arc::new(Notify::new()),
            retry_manager,
        }
    }
    
    /// Enqueue a job with the specified priority
    pub async fn enqueue(
        &self,
        job: ValkyrieJob,
        priority: JobPriority,
    ) -> Result<(), ValkyrieAdapterError> {
        // Check capacity
        let _permit = self.capacity_semaphore
            .try_acquire()
            .map_err(|_| ValkyrieAdapterError::QueueFull)?;
        
        let queued_job = QueuedJob::new(job.clone());
        
        // Update job status
        {
            let mut status_map = self.job_status.write().await;
            status_map.insert(job.id, JobStatus::Queued { queued_at: Instant::now() });
        }
        
        // Send to appropriate priority queue
        {
            let priority_queues = self.priority_queues.read().await;
            if let Some(sender) = priority_queues.get(&priority) {
                sender.send(queued_job)
                    .map_err(|_| ValkyrieAdapterError::Internal("Queue sender closed".to_string()))?;
            } else {
                return Err(ValkyrieAdapterError::Internal(
                    format!("Priority queue not found for priority: {:?}", priority)
                ));
            }
        }
        
        // Record metrics
        if self.config.metrics_enabled {
            self.metrics.record_enqueue(priority);
        }
        
        // Notify waiting dequeuers
        self.queue_notify.notify_waiters();
        
        Ok(())
    }
    
    /// Dequeue the highest priority job
    pub async fn dequeue(&self) -> Result<QueuedJob, ValkyrieAdapterError> {
        let timeout_duration = self.config.queue_timeout;
        
        timeout(timeout_duration, self.dequeue_internal())
            .await
            .map_err(|_| {
                self.metrics.record_timeout();
                ValkyrieAdapterError::DispatchTimeout
            })?
    }
    
    /// Internal dequeue implementation
    async fn dequeue_internal(&self) -> Result<QueuedJob, ValkyrieAdapterError> {
        loop {
            // Check for retry jobs first
            let retry_jobs = self.retry_manager.get_ready_retries().await;
            if !retry_jobs.is_empty() {
                for retry_job in retry_jobs {
                    // Re-enqueue retry jobs
                    self.enqueue(retry_job.job.clone(), retry_job.job.priority).await?;
                }
            }
            
            // Try to dequeue from priority queues in order
            let priority_queues = self.priority_queues.read().await;
            
            for priority in [
                JobPriority::Critical,
                JobPriority::High,
                JobPriority::Normal,
                JobPriority::Low,
                JobPriority::Background,
            ] {
                if let Some(sender) = priority_queues.get(&priority) {
                    let mut receiver = sender.subscribe();
                    
                    // Try to receive without blocking
                    if let Ok(job) = receiver.try_recv() {
                        // Update job status
                        {
                            let mut status_map = self.job_status.write().await;
                            status_map.insert(
                                job.job.id,
                                JobStatus::Routing { routing_started: Instant::now() }
                            );
                        }
                        
                        // Record metrics
                        if self.config.metrics_enabled {
                            let queue_time = job.queued_at.elapsed();
                            self.metrics.record_dequeue(queue_time);
                        }
                        
                        return Ok(job);
                    }
                }
            }
            
            // No jobs available, wait for notification
            self.queue_notify.notified().await;
        }
    }
    
    /// Update job status
    pub async fn update_job_status(&self, job_id: JobId, status: JobStatus) {
        let mut status_map = self.job_status.write().await;
        status_map.insert(job_id, status);
    }
    
    /// Get job status
    pub async fn get_job_status(&self, job_id: JobId) -> Option<JobStatus> {
        let status_map = self.job_status.read().await;
        status_map.get(&job_id).cloned()
    }
    
    /// Schedule a job for retry
    pub async fn schedule_retry(&self, job: QueuedJob) -> Result<(), ValkyrieAdapterError> {
        self.retry_manager.schedule_retry(job).await?;
        self.metrics.record_retry();
        Ok(())
    }
    
    /// Get queue metrics snapshot
    pub fn get_metrics(&self) -> JobQueueSnapshot {
        self.metrics.get_snapshot()
    }
    
    /// Get current queue size
    pub async fn size(&self) -> u32 {
        self.metrics.current_queue_size.load(AtomicOrdering::Relaxed)
    }
    
    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.size().await == 0
    }
    
    /// Get queue capacity utilization
    pub async fn utilization(&self) -> f64 {
        let current_size = self.size().await as f64;
        let max_capacity = self.config.max_capacity as f64;
        current_size / max_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_job_queue_priority_ordering() {
        let config = JobQueueConfig::default();
        let queue = AsyncJobQueue::new(config);
        
        // Create jobs with different priorities
        let critical_job = ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Critical,
            ..Default::default()
        };
        
        let normal_job = ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Normal,
            ..Default::default()
        };
        
        let background_job = ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Background,
            ..Default::default()
        };
        
        // Enqueue in reverse priority order
        queue.enqueue(background_job.clone(), JobPriority::Background).await.unwrap();
        queue.enqueue(normal_job.clone(), JobPriority::Normal).await.unwrap();
        queue.enqueue(critical_job.clone(), JobPriority::Critical).await.unwrap();
        
        // Dequeue should return critical job first
        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.job.id, critical_job.id);
        
        // Then normal job
        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.job.id, normal_job.id);
        
        // Finally background job
        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.job.id, background_job.id);
    }
    
    #[tokio::test]
    async fn test_job_queue_capacity_limit() {
        let config = JobQueueConfig {
            max_capacity: 2,
            ..Default::default()
        };
        let queue = AsyncJobQueue::new(config);
        
        let job1 = ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Normal,
            ..Default::default()
        };
        
        let job2 = ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Normal,
            ..Default::default()
        };
        
        let job3 = ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Normal,
            ..Default::default()
        };
        
        // First two jobs should succeed
        assert!(queue.enqueue(job1, JobPriority::Normal).await.is_ok());
        assert!(queue.enqueue(job2, JobPriority::Normal).await.is_ok());
        
        // Third job should fail due to capacity limit
        assert!(matches!(
            queue.enqueue(job3, JobPriority::Normal).await,
            Err(ValkyrieAdapterError::QueueFull)
        ));
    }
    
    #[tokio::test]
    async fn test_retry_mechanism() {
        let config = JobQueueConfig {
            max_retry_attempts: 2,
            retry_backoff_base: Duration::from_millis(10),
            ..Default::default()
        };
        let retry_manager = RetryManager::new(config);
        
        let job = QueuedJob::new(ValkyrieJob {
            id: Uuid::new_v4(),
            priority: JobPriority::Normal,
            ..Default::default()
        });
        
        // Schedule first retry
        assert!(retry_manager.schedule_retry(job.clone()).await.is_ok());
        
        // Wait for retry to be ready
        sleep(Duration::from_millis(20)).await;
        
        let ready_retries = retry_manager.get_ready_retries().await;
        assert_eq!(ready_retries.len(), 1);
        assert_eq!(ready_retries[0].retry_count, 1);
    }
}