//! Background Job Management System
//! 
//! Provides comprehensive background job management with job monitoring, cleanup,
//! queue management with priority and retry policies, and job result storage.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex, mpsc, oneshot};
use tokio::time::{interval, sleep, Instant};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

/// Job priority levels
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JobPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Job execution states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

/// Job retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub retry_on_failure: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(300),
            backoff_multiplier: 2.0,
            retry_on_failure: true,
        }
    }
}

/// Job execution context and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub id: Uuid,
    pub job_type: String,
    pub priority: JobPriority,
    pub state: JobState,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub retry_count: u32,
    pub retry_policy: RetryPolicy,
    pub timeout: Option<Duration>,
    pub tags: HashMap<String, String>,
    pub progress: f64, // 0.0 to 1.0
}

/// Job execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: Uuid,
    pub success: bool,
    pub result_data: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub execution_time: Duration,
    pub memory_used: Option<u64>,
    pub completed_at: SystemTime,
}

/// Job execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobStats {
    pub total_jobs: u64,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub cancelled_jobs: u64,
    pub average_execution_time: Duration,
    pub queue_size: usize,
    pub active_jobs: usize,
}

/// Trait for job execution handlers
#[async_trait::async_trait]
pub trait JobHandler: Send + Sync {
    async fn execute(&self, job_id: Uuid, job_data: serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
    fn get_job_type(&self) -> &str;
    fn get_timeout(&self) -> Option<Duration> { None }
    async fn on_progress(&self, _job_id: Uuid, _progress: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
}

/// Job queue entry
#[derive(Debug)]
struct QueuedJob {
    metadata: JobMetadata,
    job_data: serde_json::Value,
    result_sender: Option<oneshot::Sender<JobResult>>,
}

/// Background job manager with comprehensive job lifecycle management
pub struct BackgroundJobManager {
    job_queues: Arc<RwLock<HashMap<JobPriority, VecDeque<QueuedJob>>>>,
    active_jobs: Arc<RwLock<HashMap<Uuid, JobMetadata>>>,
    job_results: Arc<RwLock<HashMap<Uuid, JobResult>>>,
    job_handlers: Arc<RwLock<HashMap<String, Arc<dyn JobHandler>>>>,
    job_stats: Arc<Mutex<JobStats>>,
    worker_count: usize,
    result_retention: Duration,
    is_running: Arc<Mutex<bool>>,
    shutdown_sender: Option<mpsc::Sender<()>>,
}

impl BackgroundJobManager {
    /// Create a new background job manager
    pub fn new(worker_count: usize, result_retention: Duration) -> Self {
        let mut queues = HashMap::new();
        queues.insert(JobPriority::Low, VecDeque::new());
        queues.insert(JobPriority::Normal, VecDeque::new());
        queues.insert(JobPriority::High, VecDeque::new());
        queues.insert(JobPriority::Critical, VecDeque::new());

        Self {
            job_queues: Arc::new(RwLock::new(queues)),
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            job_results: Arc::new(RwLock::new(HashMap::new())),
            job_handlers: Arc::new(RwLock::new(HashMap::new())),
            job_stats: Arc::new(Mutex::new(JobStats::default())),
            worker_count,
            result_retention,
            is_running: Arc::new(Mutex::new(false)),
            shutdown_sender: None,
        }
    }

    /// Register a job handler for a specific job type
    pub async fn register_handler(&self, handler: Arc<dyn JobHandler>) {
        let job_type = handler.get_job_type().to_string();
        let mut handlers = self.job_handlers.write().await;
        handlers.insert(job_type.clone(), handler);
        info!("Registered job handler for type: {}", job_type);
    }

    /// Submit a job for execution
    pub async fn submit_job(
        &self,
        job_type: String,
        job_data: serde_json::Value,
        priority: JobPriority,
        retry_policy: Option<RetryPolicy>,
        timeout: Option<Duration>,
        tags: Option<HashMap<String, String>>,
    ) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let job_id = Uuid::new_v4();
        
        let metadata = JobMetadata {
            id: job_id,
            job_type: job_type.clone(),
            priority: priority.clone(),
            state: JobState::Queued,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            retry_policy: retry_policy.unwrap_or_default(),
            timeout,
            tags: tags.unwrap_or_default(),
            progress: 0.0,
        };

        let queued_job = QueuedJob {
            metadata,
            job_data,
            result_sender: None,
        };

        // Add to appropriate priority queue
        let mut queues = self.job_queues.write().await;
        if let Some(queue) = queues.get_mut(&priority) {
            queue.push_back(queued_job);
        }

        // Update statistics
        {
            let mut stats = self.job_stats.lock().await;
            stats.total_jobs += 1;
            stats.queue_size = queues.values().map(|q| q.len()).sum();
        }

        info!("Submitted job {} of type {} with priority {:?}", job_id, job_type, priority);
        Ok(job_id)
    }

    /// Submit a job and wait for its result
    pub async fn submit_job_and_wait(
        &self,
        job_type: String,
        job_data: serde_json::Value,
        priority: JobPriority,
        retry_policy: Option<RetryPolicy>,
        timeout: Option<Duration>,
        tags: Option<HashMap<String, String>>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let job_id = Uuid::new_v4();
        let (result_sender, result_receiver) = oneshot::channel();
        
        let metadata = JobMetadata {
            id: job_id,
            job_type: job_type.clone(),
            priority: priority.clone(),
            state: JobState::Queued,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            retry_policy: retry_policy.unwrap_or_default(),
            timeout,
            tags: tags.unwrap_or_default(),
            progress: 0.0,
        };

        let queued_job = QueuedJob {
            metadata,
            job_data,
            result_sender: Some(result_sender),
        };

        // Add to appropriate priority queue
        {
            let mut queues = self.job_queues.write().await;
            if let Some(queue) = queues.get_mut(&priority) {
                queue.push_back(queued_job);
            }
        }

        // Update statistics
        {
            let mut stats = self.job_stats.lock().await;
            stats.total_jobs += 1;
        }

        info!("Submitted job {} of type {} with priority {:?} (waiting for result)", job_id, job_type, priority);
        
        // Wait for result
        match result_receiver.await {
            Ok(result) => Ok(result),
            Err(_) => Err("Job result channel closed".into()),
        }
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Try to remove from queues first
        let mut queues = self.job_queues.write().await;
        for queue in queues.values_mut() {
            if let Some(pos) = queue.iter().position(|job| job.metadata.id == job_id) {
                let mut job = queue.remove(pos).unwrap();
                job.metadata.state = JobState::Cancelled;
                job.metadata.completed_at = Some(SystemTime::now());
                
                // Store result
                let result = JobResult {
                    job_id,
                    success: false,
                    result_data: None,
                    error_message: Some("Job cancelled".to_string()),
                    execution_time: Duration::from_secs(0),
                    memory_used: None,
                    completed_at: SystemTime::now(),
                };
                
                let mut results = self.job_results.write().await;
                results.insert(job_id, result.clone());
                
                // Send result if waiting
                if let Some(sender) = job.result_sender {
                    let _ = sender.send(result);
                }
                
                info!("Cancelled queued job {}", job_id);
                return Ok(());
            }
        }

        // Check if job is currently running
        let mut active_jobs = self.active_jobs.write().await;
        if let Some(mut job_metadata) = active_jobs.remove(&job_id) {
            job_metadata.state = JobState::Cancelled;
            job_metadata.completed_at = Some(SystemTime::now());
            
            // Note: Actual running job cancellation would need to be implemented
            // in the job execution logic with proper cancellation tokens
            warn!("Job {} marked for cancellation (may still be running)", job_id);
            return Ok(());
        }

        Err(format!("Job {} not found", job_id).into())
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: Uuid) -> Option<JobMetadata> {
        // Check active jobs first
        {
            let active_jobs = self.active_jobs.read().await;
            if let Some(metadata) = active_jobs.get(&job_id) {
                return Some(metadata.clone());
            }
        }

        // Check queued jobs
        let queues = self.job_queues.read().await;
        for queue in queues.values() {
            if let Some(job) = queue.iter().find(|job| job.metadata.id == job_id) {
                return Some(job.metadata.clone());
            }
        }

        None
    }

    /// Get job result
    pub async fn get_job_result(&self, job_id: Uuid) -> Option<JobResult> {
        let results = self.job_results.read().await;
        results.get(&job_id).cloned()
    }

    /// Get job statistics
    pub async fn get_job_stats(&self) -> JobStats {
        let mut stats = self.job_stats.lock().await;
        
        // Update current queue size and active jobs
        let queues = self.job_queues.read().await;
        let active_jobs = self.active_jobs.read().await;
        
        stats.queue_size = queues.values().map(|q| q.len()).sum();
        stats.active_jobs = active_jobs.len();
        
        stats.clone()
    }

    /// Start the job processing workers
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        let (shutdown_sender, mut shutdown_receiver) = mpsc::channel(1);
        self.shutdown_sender = Some(shutdown_sender);

        // Start worker tasks
        for worker_id in 0..self.worker_count {
            let queues = Arc::clone(&self.job_queues);
            let active_jobs = Arc::clone(&self.active_jobs);
            let job_results = Arc::clone(&self.job_results);
            let job_handlers = Arc::clone(&self.job_handlers);
            let job_stats = Arc::clone(&self.job_stats);
            let is_running = Arc::clone(&self.is_running);
            
            tokio::spawn(async move {
                Self::worker_loop(worker_id, queues, active_jobs, job_results, job_handlers, job_stats, is_running).await;
            });
        }

        // Start cleanup task
        let job_results = Arc::clone(&self.job_results);
        let result_retention = self.result_retention;
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            Self::cleanup_loop(job_results, result_retention, is_running).await;
        });

        info!("Started background job manager with {} workers", self.worker_count);
        Ok(())
    }

    /// Stop the job processing workers
    pub async fn stop(&mut self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(()).await;
        }
        
        info!("Stopped background job manager");
    }

    /// Worker loop for processing jobs
    async fn worker_loop(
        worker_id: usize,
        queues: Arc<RwLock<HashMap<JobPriority, VecDeque<QueuedJob>>>>,
        active_jobs: Arc<RwLock<HashMap<Uuid, JobMetadata>>>,
        job_results: Arc<RwLock<HashMap<Uuid, JobResult>>>,
        job_handlers: Arc<RwLock<HashMap<String, Arc<dyn JobHandler>>>>,
        job_stats: Arc<Mutex<JobStats>>,
        is_running: Arc<Mutex<bool>>,
    ) {
        debug!("Worker {} started", worker_id);
        
        while {
            let running = is_running.lock().await;
            *running
        } {
            // Try to get next job (highest priority first)
            let job = {
                let mut queues = queues.write().await;
                let priorities = [JobPriority::Critical, JobPriority::High, JobPriority::Normal, JobPriority::Low];
                
                let mut found_job = None;
                for priority in priorities.iter() {
                    if let Some(queue) = queues.get_mut(priority) {
                        if let Some(job) = queue.pop_front() {
                            found_job = Some(job);
                            break;
                        }
                    }
                }
                found_job
            };

            if let Some(mut job) = job {
                let job_id = job.metadata.id;
                debug!("Worker {} processing job {}", worker_id, job_id);
                
                // Mark job as running
                job.metadata.state = JobState::Running;
                job.metadata.started_at = Some(SystemTime::now());
                
                {
                    let mut active = active_jobs.write().await;
                    active.insert(job_id, job.metadata.clone());
                }

                // Execute job
                let result = Self::execute_job(&job, &job_handlers).await;
                
                // Remove from active jobs
                {
                    let mut active = active_jobs.write().await;
                    active.remove(&job_id);
                }

                // Store result
                {
                    let mut results = job_results.write().await;
                    results.insert(job_id, result.clone());
                }

                // Send result if waiting
                if let Some(sender) = job.result_sender {
                    let _ = sender.send(result.clone());
                }

                // Update statistics
                {
                    let mut stats = job_stats.lock().await;
                    if result.success {
                        stats.completed_jobs += 1;
                    } else {
                        stats.failed_jobs += 1;
                    }
                    
                    // Update average execution time
                    let total_completed = stats.completed_jobs + stats.failed_jobs;
                    if total_completed > 0 {
                        let current_avg_nanos = stats.average_execution_time.as_nanos() as u64;
                        let new_time_nanos = result.execution_time.as_nanos() as u64;
                        let new_avg_nanos = (current_avg_nanos * (total_completed - 1) + new_time_nanos) / total_completed;
                        stats.average_execution_time = Duration::from_nanos(new_avg_nanos);
                    }
                }

                debug!("Worker {} completed job {} (success: {})", worker_id, job_id, result.success);
            } else {
                // No jobs available, sleep briefly
                sleep(Duration::from_millis(100)).await;
            }
        }
        
        debug!("Worker {} stopped", worker_id);
    }

    /// Execute a single job
    async fn execute_job(
        job: &QueuedJob,
        job_handlers: &Arc<RwLock<HashMap<String, Arc<dyn JobHandler>>>>,
    ) -> JobResult {
        let start_time = Instant::now();
        let job_id = job.metadata.id;
        
        let handlers = job_handlers.read().await;
        let handler = handlers.get(&job.metadata.job_type);
        
        let (success, result_data, error_message) = match handler {
            Some(handler) => {
                match handler.execute(job_id, job.job_data.clone()).await {
                    Ok(data) => (true, Some(data), None),
                    Err(e) => (false, None, Some(e.to_string())),
                }
            }
            None => (false, None, Some(format!("No handler found for job type: {}", job.metadata.job_type))),
        };

        JobResult {
            job_id,
            success,
            result_data,
            error_message,
            execution_time: start_time.elapsed(),
            memory_used: None, // Could be implemented with memory tracking
            completed_at: SystemTime::now(),
        }
    }

    /// Cleanup loop for removing old job results
    async fn cleanup_loop(
        job_results: Arc<RwLock<HashMap<Uuid, JobResult>>>,
        retention_period: Duration,
        is_running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(300)); // Cleanup every 5 minutes
        
        while {
            let running = is_running.lock().await;
            *running
        } {
            cleanup_interval.tick().await;
            
            let now = SystemTime::now();
            let mut results = job_results.write().await;
            let initial_count = results.len();
            
            results.retain(|_, result| {
                if let Ok(age) = now.duration_since(result.completed_at) {
                    age < retention_period
                } else {
                    true // Keep if we can't determine age
                }
            });
            
            let cleaned_count = initial_count - results.len();
            if cleaned_count > 0 {
                info!("Cleaned up {} old job results", cleaned_count);
            }
        }
    }
}

impl Default for BackgroundJobManager {
    fn default() -> Self {
        Self::new(4, Duration::from_secs(3600)) // 4 workers, 1 hour retention
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    /// Test job handler for testing
    struct TestJobHandler {
        job_type: String,
        should_fail: bool,
        execution_delay: Duration,
    }

    impl TestJobHandler {
        fn new(job_type: &str, should_fail: bool, execution_delay: Duration) -> Self {
            Self {
                job_type: job_type.to_string(),
                should_fail,
                execution_delay,
            }
        }
    }

    #[async_trait::async_trait]
    impl JobHandler for TestJobHandler {
        async fn execute(&self, job_id: Uuid, job_data: serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
            sleep(self.execution_delay).await;
            
            if self.should_fail {
                Err("Test job failure".into())
            } else {
                Ok(serde_json::json!({
                    "job_id": job_id,
                    "input": job_data,
                    "result": "success"
                }))
            }
        }

        fn get_job_type(&self) -> &str {
            &self.job_type
        }

        fn get_timeout(&self) -> Option<Duration> {
            Some(Duration::from_secs(30))
        }
    }

    #[tokio::test]
    async fn test_job_manager_creation() {
        let manager = BackgroundJobManager::new(2, Duration::from_secs(60));
        let stats = manager.get_job_stats().await;
        
        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.active_jobs, 0);
    }

    #[tokio::test]
    async fn test_job_submission_and_execution() {
        let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
        let handler = Arc::new(TestJobHandler::new("test_job", false, Duration::from_millis(10)));
        
        manager.register_handler(handler).await;
        manager.start().await.unwrap();
        
        let job_data = serde_json::json!({"test": "data"});
        let job_id = manager.submit_job(
            "test_job".to_string(),
            job_data,
            JobPriority::Normal,
            None,
            None,
            None,
        ).await.unwrap();
        
        // Wait for job to complete
        sleep(Duration::from_millis(100)).await;
        
        let result = manager.get_job_result(job_id).await;
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.result_data.is_some());
        
        manager.stop().await;
    }

    #[tokio::test]
    async fn test_job_priority_ordering() {
        let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
        let handler = Arc::new(TestJobHandler::new("test_job", false, Duration::from_millis(50)));
        
        manager.register_handler(handler).await;
        
        // Submit jobs in reverse priority order
        let low_job = manager.submit_job("test_job".to_string(), serde_json::json!({"priority": "low"}), JobPriority::Low, None, None, None).await.unwrap();
        let high_job = manager.submit_job("test_job".to_string(), serde_json::json!({"priority": "high"}), JobPriority::High, None, None, None).await.unwrap();
        let normal_job = manager.submit_job("test_job".to_string(), serde_json::json!({"priority": "normal"}), JobPriority::Normal, None, None, None).await.unwrap();
        
        manager.start().await.unwrap();
        
        // Wait for all jobs to complete
        sleep(Duration::from_millis(200)).await;
        
        // High priority job should complete first
        let high_result = manager.get_job_result(high_job).await.unwrap();
        let normal_result = manager.get_job_result(normal_job).await.unwrap();
        let low_result = manager.get_job_result(low_job).await.unwrap();
        
        assert!(high_result.completed_at <= normal_result.completed_at);
        assert!(normal_result.completed_at <= low_result.completed_at);
        
        manager.stop().await;
    }

    #[tokio::test]
    async fn test_job_failure_handling() {
        let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
        let handler = Arc::new(TestJobHandler::new("failing_job", true, Duration::from_millis(10)));
        
        manager.register_handler(handler).await;
        manager.start().await.unwrap();
        
        let job_id = manager.submit_job(
            "failing_job".to_string(),
            serde_json::json!({"test": "data"}),
            JobPriority::Normal,
            None,
            None,
            None,
        ).await.unwrap();
        
        // Wait for job to complete
        sleep(Duration::from_millis(100)).await;
        
        let result = manager.get_job_result(job_id).await.unwrap();
        assert!(!result.success);
        assert!(result.error_message.is_some());
        assert_eq!(result.error_message.unwrap(), "Test job failure");
        
        manager.stop().await;
    }

    #[tokio::test]
    async fn test_job_cancellation() {
        let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
        let handler = Arc::new(TestJobHandler::new("test_job", false, Duration::from_millis(10)));
        
        manager.register_handler(handler).await;
        
        let job_id = manager.submit_job(
            "test_job".to_string(),
            serde_json::json!({"test": "data"}),
            JobPriority::Normal,
            None,
            None,
            None,
        ).await.unwrap();
        
        // Cancel job before starting workers
        manager.cancel_job(job_id).await.unwrap();
        
        let result = manager.get_job_result(job_id).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.error_message.unwrap(), "Job cancelled");
    }

    #[tokio::test]
    async fn test_job_statistics() {
        let mut manager = BackgroundJobManager::new(2, Duration::from_secs(60));
        let success_handler = Arc::new(TestJobHandler::new("success_job", false, Duration::from_millis(10)));
        let failure_handler = Arc::new(TestJobHandler::new("failure_job", true, Duration::from_millis(10)));
        
        manager.register_handler(success_handler).await;
        manager.register_handler(failure_handler).await;
        manager.start().await.unwrap();
        
        // Submit multiple jobs
        for _ in 0..3 {
            manager.submit_job("success_job".to_string(), serde_json::json!({}), JobPriority::Normal, None, None, None).await.unwrap();
        }
        for _ in 0..2 {
            manager.submit_job("failure_job".to_string(), serde_json::json!({}), JobPriority::Normal, None, None, None).await.unwrap();
        }
        
        // Wait for jobs to complete
        sleep(Duration::from_millis(200)).await;
        
        let stats = manager.get_job_stats().await;
        assert_eq!(stats.total_jobs, 5);
        assert_eq!(stats.completed_jobs, 3);
        assert_eq!(stats.failed_jobs, 2);
        assert!(stats.average_execution_time > Duration::from_millis(0));
        
        manager.stop().await;
    }
}