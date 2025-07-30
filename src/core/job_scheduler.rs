//! Job scheduler with retry logic and priority support
//! 
//! This module provides advanced job scheduling capabilities including
//! retry mechanisms, priority handling, and failure recovery.

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::core::job_queue::{JobQueue, InMemoryJobQueue};
use crate::core::runner_pool::{RunnerPoolManager, LoadBalancingStrategy};
use crate::domain::entities::{
    Job, JobId, JobResult, JobStatus, JobPriority, RetryPolicy, BackoffStrategy
};
use crate::error::{AppError, Result};

/// Job scheduling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSchedulerConfig {
    /// Maximum number of concurrent jobs
    pub max_concurrent_jobs: usize,
    /// Job timeout in seconds
    pub default_job_timeout: u64,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Priority queue configuration
    pub priority_config: PriorityConfig,
    /// Scheduling interval in milliseconds
    pub scheduling_interval_ms: u64,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Enable automatic retries
    pub enabled: bool,
    /// Maximum retry attempts across all jobs
    pub max_global_retries: u32,
    /// Default retry delay in seconds
    pub default_retry_delay: u64,
    /// Maximum retry delay in seconds
    pub max_retry_delay: u64,
    /// Retry backoff multiplier
    pub backoff_multiplier: f64,
}

/// Priority configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityConfig {
    /// Enable priority scheduling
    pub enabled: bool,
    /// Priority queue sizes
    pub critical_queue_size: usize,
    pub high_queue_size: usize,
    pub normal_queue_size: usize,
    pub low_queue_size: usize,
}

impl Default for JobSchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 100,
            default_job_timeout: 3600, // 1 hour
            retry_config: RetryConfig {
                enabled: true,
                max_global_retries: 1000,
                default_retry_delay: 30,
                max_retry_delay: 3600,
                backoff_multiplier: 2.0,
            },
            priority_config: PriorityConfig {
                enabled: true,
                critical_queue_size: 50,
                high_queue_size: 200,
                normal_queue_size: 500,
                low_queue_size: 1000,
            },
            scheduling_interval_ms: 100,
        }
    }
}

/// Job execution context
#[derive(Debug, Clone)]
pub struct JobExecutionContext {
    /// Job to execute
    pub job: Job,
    /// Execution attempt number
    pub attempt: u32,
    /// Scheduled execution time
    pub scheduled_at: DateTime<Utc>,
    /// Previous execution results (for retries)
    pub previous_results: Vec<JobResult>,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
}

/// Scheduled job entry
#[derive(Debug, Clone)]
struct ScheduledJob {
    /// Job execution context
    context: JobExecutionContext,
    /// Next execution time
    next_execution: DateTime<Utc>,
    /// Retry delay for this job
    retry_delay: ChronoDuration,
}

/// Job scheduler statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobSchedulerStats {
    /// Total jobs scheduled
    pub total_scheduled: u64,
    /// Total jobs completed
    pub total_completed: u64,
    /// Total jobs failed
    pub total_failed: u64,
    /// Total jobs retried
    pub total_retried: u64,
    /// Current pending jobs
    pub pending_jobs: usize,
    /// Current running jobs
    pub running_jobs: usize,
    /// Jobs by priority
    pub jobs_by_priority: HashMap<String, u64>,
    /// Average execution time
    pub avg_execution_time: f64,
    /// Success rate percentage
    pub success_rate: f64,
}

/// Job scheduler trait
#[async_trait]
pub trait JobScheduler: Send + Sync {
    /// Schedule a job for execution
    async fn schedule_job(&self, job: Job) -> Result<()>;
    
    /// Schedule a job with delay
    async fn schedule_job_with_delay(&self, job: Job, delay: ChronoDuration) -> Result<()>;
    
    /// Cancel a scheduled job
    async fn cancel_job(&self, job_id: JobId) -> Result<bool>;
    
    /// Get job status
    async fn get_job_status(&self, job_id: JobId) -> Result<Option<JobStatus>>;
    
    /// Get scheduler statistics
    async fn get_stats(&self) -> Result<JobSchedulerStats>;
    
    /// Start the scheduler
    async fn start(&self) -> Result<()>;
    
    /// Stop the scheduler
    async fn stop(&self) -> Result<()>;
    
    /// Process pending jobs
    async fn process_pending_jobs(&self) -> Result<()>;
    
    /// Retry a failed job
    async fn retry_job(&self, job_id: JobId) -> Result<()>;
}

/// Default job scheduler implementation
pub struct DefaultJobScheduler {
    /// Priority queues for different job priorities
    priority_queues: Arc<RwLock<HashMap<JobPriority, Arc<dyn JobQueue>>>>,
    /// Scheduled jobs waiting for retry
    scheduled_jobs: Arc<Mutex<VecDeque<ScheduledJob>>>,
    /// Currently running jobs
    running_jobs: Arc<RwLock<HashMap<JobId, JobExecutionContext>>>,
    /// Job results cache
    job_results: Arc<RwLock<HashMap<JobId, JobResult>>>,
    /// Runner pool manager
    runner_pool: Arc<dyn RunnerPoolManager>,
    /// Configuration
    config: JobSchedulerConfig,
    /// Statistics
    stats: Arc<RwLock<JobSchedulerStats>>,
    /// Running state
    running: Arc<Mutex<bool>>,
}

impl DefaultJobScheduler {
    /// Create a new job scheduler
    pub fn new(
        runner_pool: Arc<dyn RunnerPoolManager>,
        config: JobSchedulerConfig,
    ) -> Self {
        let mut priority_queues = HashMap::new();
        
        // Create priority queues
        priority_queues.insert(JobPriority::Critical, Arc::new(InMemoryJobQueue::with_default_config()) as Arc<dyn JobQueue>);
        priority_queues.insert(JobPriority::High, Arc::new(InMemoryJobQueue::with_default_config()) as Arc<dyn JobQueue>);
        priority_queues.insert(JobPriority::Normal, Arc::new(InMemoryJobQueue::with_default_config()) as Arc<dyn JobQueue>);
        priority_queues.insert(JobPriority::Low, Arc::new(InMemoryJobQueue::with_default_config()) as Arc<dyn JobQueue>);
        
        Self {
            priority_queues: Arc::new(RwLock::new(priority_queues)),
            scheduled_jobs: Arc::new(Mutex::new(VecDeque::new())),
            running_jobs: Arc::new(RwLock::new(HashMap::new())),
            job_results: Arc::new(RwLock::new(HashMap::new())),
            runner_pool,
            config,
            stats: Arc::new(RwLock::new(JobSchedulerStats::default())),
            running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Calculate retry delay based on backoff strategy
    fn calculate_retry_delay(&self, job: &Job, attempt: u32) -> ChronoDuration {
        let base_delay = job.retry_policy.retry_delay.as_secs() as i64;
        
        let delay_seconds = match job.retry_policy.backoff_strategy {
            BackoffStrategy::Fixed => base_delay,
            BackoffStrategy::Exponential { multiplier } => {
                let exponential_delay = base_delay as f64 * multiplier.powi(attempt as i32 - 1);
                (exponential_delay as i64).min(self.config.retry_config.max_retry_delay as i64)
            }
            BackoffStrategy::Linear { increment } => {
                let linear_delay = base_delay + (increment.as_secs() as i64 * (attempt as i64 - 1));
                linear_delay.min(self.config.retry_config.max_retry_delay as i64)
            }
        };
        
        ChronoDuration::seconds(delay_seconds)
    }
    
    /// Process jobs from priority queues
    async fn process_priority_queues(&self) -> Result<()> {
        let priority_queues = self.priority_queues.read().await;
        
        // Process queues in priority order
        let priorities = vec![JobPriority::Critical, JobPriority::High, JobPriority::Normal, JobPriority::Low];
        
        for priority in priorities {
            if let Some(queue) = priority_queues.get(&priority) {
                while let Some(job) = queue.dequeue().await? {
                    if self.can_execute_job().await {
                        self.execute_job(job).await?;
                    } else {
                        // Re-queue the job if we can't execute it now
                        queue.enqueue(job).await?;
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if we can execute another job
    async fn can_execute_job(&self) -> bool {
        let running_jobs = self.running_jobs.read().await;
        running_jobs.len() < self.config.max_concurrent_jobs
    }
    
    /// Execute a job
    async fn execute_job(&self, job: Job) -> Result<()> {
        let job_id = job.id;
        let context = JobExecutionContext {
            job: job.clone(),
            attempt: 1,
            scheduled_at: Utc::now(),
            previous_results: Vec::new(),
            metadata: HashMap::new(),
        };
        
        // Add to running jobs
        {
            let mut running_jobs = self.running_jobs.write().await;
            running_jobs.insert(job_id, context.clone());
        }
        
        // Submit to runner pool
        let runner_pool = self.runner_pool.clone();
        let stats = self.stats.clone();
        let running_jobs = self.running_jobs.clone();
        let job_results = self.job_results.clone();
        let scheduled_jobs = self.scheduled_jobs.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let start_time = Utc::now();
            
            match runner_pool.submit_job(job.clone()).await {
                Ok(()) => {
                    // Job submitted successfully, wait for completion
                    // In a real implementation, this would be handled by the runner pool
                    let execution_time = (Utc::now() - start_time).num_milliseconds() as f64 / 1000.0;
                    
                    let result = JobResult {
                        job_id,
                        status: JobStatus::Success,
                        exit_code: Some(0),
                        stdout: "Job completed successfully".to_string(),
                        stderr: String::new(),
                        started_at: start_time,
                        finished_at: Some(Utc::now()),
                        duration: Some(std::time::Duration::from_millis((execution_time * 1000.0) as u64)),
                        step_results: Vec::new(),
                        error_message: None,
                    };
                    
                    // Store result
                    {
                        let mut job_results = job_results.write().await;
                        job_results.insert(job_id, result);
                    }
                    
                    // Update statistics
                    {
                        let mut stats = stats.write().await;
                        stats.total_completed += 1;
                        stats.avg_execution_time = 
                            (stats.avg_execution_time * (stats.total_completed - 1) as f64 + execution_time) 
                            / stats.total_completed as f64;
                        stats.success_rate = (stats.total_completed as f64 / (stats.total_completed + stats.total_failed) as f64) * 100.0;
                    }
                    
                    info!("Job {} completed successfully in {:.2}s", job_id, execution_time);
                }
                Err(e) => {
                    error!("Job {} failed: {}", job_id, e);
                    
                    // Check if we should retry
                    if context.attempt < job.retry_policy.max_retries && config.retry_config.enabled {
                        let retry_delay = Self::calculate_retry_delay_static(&job, context.attempt + 1, &config);
                        let next_execution = Utc::now() + retry_delay;
                        
                        let scheduled_job = ScheduledJob {
                            context: JobExecutionContext {
                                job: job.clone(),
                                attempt: context.attempt + 1,
                                scheduled_at: next_execution,
                                previous_results: context.previous_results,
                                metadata: context.metadata,
                            },
                            next_execution,
                            retry_delay,
                        };
                        
                        {
                            let mut scheduled_jobs = scheduled_jobs.lock().await;
                            scheduled_jobs.push_back(scheduled_job);
                        }
                        
                        // Update statistics
                        {
                            let mut stats = stats.write().await;
                            stats.total_retried += 1;
                        }
                        
                        info!("Scheduled job {} for retry (attempt {}) in {:?}", job_id, context.attempt + 1, retry_delay);
                    } else {
                        // Job failed permanently
                        let result = JobResult {
                            job_id,
                            status: JobStatus::Failed,
                            exit_code: Some(1),
                            stdout: String::new(),
                            stderr: e.to_string(),
                            started_at: start_time,
                            finished_at: Some(Utc::now()),
                            duration: Some(std::time::Duration::from_millis(
                                (Utc::now() - start_time).num_milliseconds() as u64
                            )),
                            step_results: Vec::new(),
                            error_message: Some(e.to_string()),
                        };
                        
                        // Store result
                        {
                            let mut job_results = job_results.write().await;
                            job_results.insert(job_id, result);
                        }
                        
                        // Update statistics
                        {
                            let mut stats = stats.write().await;
                            stats.total_failed += 1;
                            stats.success_rate = (stats.total_completed as f64 / (stats.total_completed + stats.total_failed) as f64) * 100.0;
                        }
                    }
                }
            }
            
            // Remove from running jobs
            {
                let mut running_jobs = running_jobs.write().await;
                running_jobs.remove(&job_id);
            }
        });
        
        Ok(())
    }
    
    /// Static version of calculate_retry_delay for use in async context
    fn calculate_retry_delay_static(job: &Job, attempt: u32, config: &JobSchedulerConfig) -> ChronoDuration {
        let base_delay = job.retry_policy.retry_delay.as_secs() as i64;
        
        let delay_seconds = match job.retry_policy.backoff_strategy {
            BackoffStrategy::Fixed => base_delay,
            BackoffStrategy::Exponential { multiplier } => {
                let exponential_delay = base_delay as f64 * multiplier.powi(attempt as i32 - 1);
                (exponential_delay as i64).min(config.retry_config.max_retry_delay as i64)
            }
            BackoffStrategy::Linear { increment } => {
                let linear_delay = base_delay + (increment.as_secs() as i64 * (attempt as i64 - 1));
                linear_delay.min(config.retry_config.max_retry_delay as i64)
            }
        };
        
        ChronoDuration::seconds(delay_seconds)
    }
    
    /// Process scheduled jobs (retries)
    async fn process_scheduled_jobs(&self) -> Result<()> {
        let now = Utc::now();
        let mut jobs_to_execute = Vec::new();
        
        {
            let mut scheduled_jobs = self.scheduled_jobs.lock().await;
            
            // Find jobs ready for execution
            while let Some(scheduled_job) = scheduled_jobs.front() {
                if scheduled_job.next_execution <= now {
                    jobs_to_execute.push(scheduled_jobs.pop_front().unwrap());
                } else {
                    break;
                }
            }
        }
        
        // Execute ready jobs
        for scheduled_job in jobs_to_execute {
            if self.can_execute_job().await {
                self.execute_job(scheduled_job.context.job).await?;
            } else {
                // Re-schedule if we can't execute now
                let mut scheduled_jobs = self.scheduled_jobs.lock().await;
                scheduled_jobs.push_front(scheduled_job);
                break;
            }
        }
        
        Ok(())
    }
    
    /// Update statistics
    async fn update_stats(&self) -> Result<()> {
        let running_jobs = self.running_jobs.read().await;
        let mut stats = self.stats.write().await;
        
        stats.running_jobs = running_jobs.len();
        
        // Update pending jobs count
        let priority_queues = self.priority_queues.read().await;
        let mut pending_count = 0;
        
        for queue in priority_queues.values() {
            pending_count += queue.size().await?;
        }
        
        let scheduled_jobs = self.scheduled_jobs.lock().await;
        pending_count += scheduled_jobs.len();
        
        stats.pending_jobs = pending_count;
        
        Ok(())
    }
}

#[async_trait]
impl JobScheduler for DefaultJobScheduler {
    async fn schedule_job(&self, job: Job) -> Result<()> {
        let priority_queues = self.priority_queues.read().await;
        
        if let Some(queue) = priority_queues.get(&job.priority) {
            queue.enqueue(job.clone()).await?;
            
            // Update statistics
            let mut stats = self.stats.write().await;
            stats.total_scheduled += 1;
            
            let priority_str = format!("{:?}", job.priority);
            *stats.jobs_by_priority.entry(priority_str).or_insert(0) += 1;
            
            debug!("Scheduled job {} with priority {:?}", job.id, job.priority);
            Ok(())
        } else {
            Err(AppError::BadRequest(format!("Invalid job priority: {:?}", job.priority)))
        }
    }
    
    async fn schedule_job_with_delay(&self, job: Job, delay: ChronoDuration) -> Result<()> {
        let next_execution = Utc::now() + delay;
        
        let scheduled_job = ScheduledJob {
            context: JobExecutionContext {
                job: job.clone(),
                attempt: 1,
                scheduled_at: next_execution,
                previous_results: Vec::new(),
                metadata: HashMap::new(),
            },
            next_execution,
            retry_delay: delay,
        };
        
        {
            let mut scheduled_jobs = self.scheduled_jobs.lock().await;
            scheduled_jobs.push_back(scheduled_job);
        }
        
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_scheduled += 1;
        
        debug!("Scheduled job {} with delay {:?}", job.id, delay);
        Ok(())
    }
    
    async fn cancel_job(&self, job_id: JobId) -> Result<bool> {
        // Try to remove from priority queues
        let priority_queues = self.priority_queues.read().await;
        
        for queue in priority_queues.values() {
            if queue.remove_job(job_id).await? {
                debug!("Cancelled job {} from priority queue", job_id);
                return Ok(true);
            }
        }
        
        // Try to remove from scheduled jobs
        {
            let mut scheduled_jobs = self.scheduled_jobs.lock().await;
            let original_len = scheduled_jobs.len();
            scheduled_jobs.retain(|sj| sj.context.job.id != job_id);
            
            if scheduled_jobs.len() < original_len {
                debug!("Cancelled job {} from scheduled jobs", job_id);
                return Ok(true);
            }
        }
        
        // Check if job is currently running
        let running_jobs = self.running_jobs.read().await;
        if running_jobs.contains_key(&job_id) {
            warn!("Cannot cancel job {} - it is currently running", job_id);
            return Ok(false);
        }
        
        Ok(false)
    }
    
    async fn get_job_status(&self, job_id: JobId) -> Result<Option<JobStatus>> {
        // Check running jobs
        {
            let running_jobs = self.running_jobs.read().await;
            if running_jobs.contains_key(&job_id) {
                return Ok(Some(JobStatus::Running));
            }
        }
        
        // Check job results
        {
            let job_results = self.job_results.read().await;
            if let Some(result) = job_results.get(&job_id) {
                return Ok(Some(result.status.clone()));
            }
        }
        
        // Check scheduled jobs
        {
            let scheduled_jobs = self.scheduled_jobs.lock().await;
            for scheduled_job in scheduled_jobs.iter() {
                if scheduled_job.context.job.id == job_id {
                    return Ok(Some(JobStatus::Queued));
                }
            }
        }
        
        // Check priority queues
        let priority_queues = self.priority_queues.read().await;
        for queue in priority_queues.values() {
            if let Some(_) = queue.get_job(job_id).await? {
                return Ok(Some(JobStatus::Queued));
            }
        }
        
        Ok(None)
    }
    
    async fn get_stats(&self) -> Result<JobSchedulerStats> {
        self.update_stats().await?;
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest("Job scheduler is already running".to_string()));
        }
        
        *running = true;
        
        // Start background processing tasks
        let scheduler = Arc::new(self);
        
        // Priority queue processing task
        let priority_processor = scheduler.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_millis(priority_processor.config.scheduling_interval_ms)
            );
            
            loop {
                interval.tick().await;
                if let Err(e) = priority_processor.process_priority_queues().await {
                    error!("Error processing priority queues: {}", e);
                }
            }
        });
        
        // Scheduled jobs processing task
        let scheduled_processor = scheduler.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_millis(scheduled_processor.config.scheduling_interval_ms)
            );
            
            loop {
                interval.tick().await;
                if let Err(e) = scheduled_processor.process_scheduled_jobs().await {
                    error!("Error processing scheduled jobs: {}", e);
                }
            }
        });
        
        info!("Job scheduler started");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }
        
        *running = false;
        
        // Wait for running jobs to complete (simplified)
        let mut attempts = 0;
        while attempts < 30 { // Wait up to 30 seconds
            let running_jobs = self.running_jobs.read().await;
            if running_jobs.is_empty() {
                break;
            }
            drop(running_jobs);
            
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            attempts += 1;
        }
        
        info!("Job scheduler stopped");
        Ok(())
    }
    
    async fn process_pending_jobs(&self) -> Result<()> {
        self.process_priority_queues().await?;
        self.process_scheduled_jobs().await?;
        Ok(())
    }
    
    async fn retry_job(&self, job_id: JobId) -> Result<()> {
        // Find the job result
        let job_results = self.job_results.read().await;
        
        if let Some(result) = job_results.get(&job_id) {
            if result.is_failed() {
                // Create a new job for retry (simplified)
                let mut new_job = Job::new(
                    format!("retry-{}", result.job_id),
                    uuid::Uuid::new_v4(), // This should be the original pipeline ID
                    vec![]
                );
                new_job.id = job_id;
                
                drop(job_results);
                
                self.schedule_job(new_job).await?;
                info!("Manually retried job {}", job_id);
                Ok(())
            } else {
                Err(AppError::BadRequest(format!("Job {} is not in a failed state", job_id)))
            }
        } else {
            Err(AppError::NotFound(format!("Job {} not found", job_id)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runner_pool::{DefaultRunnerPoolManager, RunnerPoolConfig};
    use crate::domain::entities::{JobStep, JobRequirements, RetryPolicy, BackoffStrategy};
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
    async fn test_job_scheduler_creation() {
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        let config = JobSchedulerConfig::default();
        let scheduler = DefaultJobScheduler::new(runner_pool, config);
        
        let stats = scheduler.get_stats().await.unwrap();
        assert_eq!(stats.total_scheduled, 0);
        assert_eq!(stats.total_completed, 0);
    }
    
    #[tokio::test]
    async fn test_schedule_job() {
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        let config = JobSchedulerConfig::default();
        let scheduler = DefaultJobScheduler::new(runner_pool, config);
        
        let job = create_test_job("test-job", JobPriority::Normal);
        let job_id = job.id;
        
        scheduler.schedule_job(job).await.unwrap();
        
        let stats = scheduler.get_stats().await.unwrap();
        assert_eq!(stats.total_scheduled, 1);
        
        let status = scheduler.get_job_status(job_id).await.unwrap();
        assert_eq!(status, Some(JobStatus::Queued));
    }
    
    #[tokio::test]
    async fn test_schedule_job_with_delay() {
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        let config = JobSchedulerConfig::default();
        let scheduler = DefaultJobScheduler::new(runner_pool, config);
        
        let job = create_test_job("delayed-job", JobPriority::Normal);
        let job_id = job.id;
        let delay = ChronoDuration::seconds(60);
        
        scheduler.schedule_job_with_delay(job, delay).await.unwrap();
        
        let stats = scheduler.get_stats().await.unwrap();
        assert_eq!(stats.total_scheduled, 1);
        
        let status = scheduler.get_job_status(job_id).await.unwrap();
        assert_eq!(status, Some(JobStatus::Queued));
    }
    
    #[tokio::test]
    async fn test_cancel_job() {
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        let config = JobSchedulerConfig::default();
        let scheduler = DefaultJobScheduler::new(runner_pool, config);
        
        let job = create_test_job("test-job", JobPriority::Normal);
        let job_id = job.id;
        
        scheduler.schedule_job(job).await.unwrap();
        
        let cancelled = scheduler.cancel_job(job_id).await.unwrap();
        assert!(cancelled);
        
        let status = scheduler.get_job_status(job_id).await.unwrap();
        assert!(status.is_none());
    }
    
    #[tokio::test]
    async fn test_retry_delay_calculation() {
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        let config = JobSchedulerConfig::default();
        let scheduler = DefaultJobScheduler::new(runner_pool, config);
        
        let mut job = create_test_job("test-job", JobPriority::Normal);
        job.retry_policy.backoff_strategy = BackoffStrategy::Exponential { multiplier: 2.0 };
        
        let delay1 = scheduler.calculate_retry_delay(&job, 1);
        let delay2 = scheduler.calculate_retry_delay(&job, 2);
        let delay3 = scheduler.calculate_retry_delay(&job, 3);
        
        assert_eq!(delay1.num_seconds(), 30);  // Base delay
        assert_eq!(delay2.num_seconds(), 60);  // 30 * 2^1
        assert_eq!(delay3.num_seconds(), 120); // 30 * 2^2
    }
}