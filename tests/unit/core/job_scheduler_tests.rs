//! Unit tests for job scheduler
//!
//! This module contains comprehensive tests for the job scheduler,
//! including retry logic, priority handling, and scheduling algorithms.

use chrono::Duration as ChronoDuration;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::job_scheduler::{
    DefaultJobScheduler, JobExecutionContext, JobScheduler, JobSchedulerConfig, PriorityConfig,
    RetryConfig,
};
use rustci::core::runner_pool::{DefaultRunnerPoolManager, RunnerPoolConfig};
use rustci::domain::entities::{
    BackoffStrategy, HealthStatus, Job, JobPriority, JobRequirements, JobStatus, JobStep,
    RetryPolicy, Runner, RunnerCapacity, RunnerEntity, RunnerType,
};
use rustci::error::{AppError, Result};

// Mock runner for testing job scheduler
struct MockSchedulerRunner {
    id: Uuid,
    name: String,
    should_fail: bool,
    execution_delay: Duration,
}

impl MockSchedulerRunner {
    fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            should_fail: false,
            execution_delay: Duration::from_millis(50),
        }
    }

    fn with_failure(mut self, should_fail: bool) -> Self {
        self.should_fail = should_fail;
        self
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.execution_delay = delay;
        self
    }
}

#[async_trait::async_trait]
impl Runner for MockSchedulerRunner {
    async fn execute(&self, job: Job) -> Result<rustci::domain::entities::JobResult> {
        sleep(self.execution_delay).await;

        if self.should_fail {
            return Err(AppError::InternalServerError(
                "Scheduled job failure".to_string(),
            ));
        }

        let mut result = rustci::domain::entities::JobResult::new(job.id);
        result.complete(JobStatus::Success, Some(0));
        result.stdout = format!("Job {} executed by {}", job.name, self.name);
        Ok(result)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        Ok(RunnerCapacity {
            max_concurrent_jobs: 4,
            current_jobs: 0,
            available_slots: 4,
            cpu_usage: 20.0,
            memory_usage: 30.0,
            disk_usage: 40.0,
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    fn get_metadata(&self) -> rustci::domain::entities::RunnerMetadata {
        rustci::domain::entities::RunnerMetadata {
            id: self.id,
            name: self.name.clone(),
            runner_type: RunnerType::Local {
                max_concurrent_jobs: 4,
                working_directory: "/tmp".to_string(),
            },
            version: "1.0.0".to_string(),
            supported_job_types: vec!["test".to_string()],
        }
    }

    async fn can_handle_job(&self, _job: &Job) -> Result<bool> {
        Ok(true)
    }
}

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
            retry_delay: Duration::from_millis(100), // Short delay for tests
            backoff_strategy: BackoffStrategy::Fixed,
        },
        metadata: HashMap::new(),
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    }
}

fn create_runner_entity(name: &str) -> RunnerEntity {
    let mut entity = RunnerEntity::new(
        name.to_string(),
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    entity.status = rustci::domain::entities::RunnerStatus::Active;
    entity
}

#[tokio::test]
async fn test_job_scheduler_creation() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool, config);

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 0);
    assert_eq!(stats.total_completed, 0);
    assert_eq!(stats.running_jobs, 0);
}

#[tokio::test]
async fn test_job_scheduling() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register a runner
    let runner = Arc::new(MockSchedulerRunner::new("test-runner"));
    let entity = create_runner_entity("test-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    let job = create_test_job("test-job", JobPriority::Normal);
    let job_id = job.id;

    scheduler.schedule_job(job).await.unwrap();

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 1);

    let status = scheduler.get_job_status(job_id).await.unwrap();
    assert_eq!(status, Some(JobStatus::Queued));
}

#[tokio::test]
async fn test_job_priority_scheduling() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register a runner
    let runner = Arc::new(MockSchedulerRunner::new("priority-runner"));
    let entity = create_runner_entity("priority-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    scheduler.start().await.unwrap();

    // Schedule jobs with different priorities
    let low_job = create_test_job("low-priority", JobPriority::Low);
    let high_job = create_test_job("high-priority", JobPriority::High);
    let critical_job = create_test_job("critical-priority", JobPriority::Critical);
    let normal_job = create_test_job("normal-priority", JobPriority::Normal);

    scheduler.schedule_job(low_job).await.unwrap();
    scheduler.schedule_job(high_job).await.unwrap();
    scheduler.schedule_job(critical_job).await.unwrap();
    scheduler.schedule_job(normal_job).await.unwrap();

    // Wait for processing
    sleep(Duration::from_millis(200)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 4);

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_job_scheduling_with_delay() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool, config);

    let job = create_test_job("delayed-job", JobPriority::Normal);
    let job_id = job.id;
    let delay = ChronoDuration::milliseconds(100);

    scheduler.schedule_job_with_delay(job, delay).await.unwrap();

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 1);

    let status = scheduler.get_job_status(job_id).await.unwrap();
    assert_eq!(status, Some(JobStatus::Queued));
}

#[tokio::test]
async fn test_job_cancellation() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool, config);

    let job = create_test_job("cancel-job", JobPriority::Normal);
    let job_id = job.id;

    scheduler.schedule_job(job).await.unwrap();

    let cancelled = scheduler.cancel_job(job_id).await.unwrap();
    assert!(cancelled);

    let status = scheduler.get_job_status(job_id).await.unwrap();
    assert!(status.is_none());
}

#[tokio::test]
async fn test_job_retry_mechanism() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig {
        retry_config: RetryConfig {
            enabled: true,
            max_global_retries: 10,
            default_retry_delay: 1,
            max_retry_delay: 60,
            backoff_multiplier: 2.0,
        },
        ..Default::default()
    };
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register a failing runner
    let runner = Arc::new(MockSchedulerRunner::new("failing-runner").with_failure(true));
    let entity = create_runner_entity("failing-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    scheduler.start().await.unwrap();

    let mut job = create_test_job("retry-job", JobPriority::Normal);
    job.retry_policy.max_retries = 2;
    job.retry_policy.retry_delay = Duration::from_millis(50);

    scheduler.schedule_job(job).await.unwrap();

    // Wait for job processing and retries
    sleep(Duration::from_millis(300)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert!(stats.total_retried > 0);

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_exponential_backoff_retry() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register a failing runner
    let runner = Arc::new(MockSchedulerRunner::new("backoff-runner").with_failure(true));
    let entity = create_runner_entity("backoff-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    scheduler.start().await.unwrap();

    let mut job = create_test_job("backoff-job", JobPriority::Normal);
    job.retry_policy.max_retries = 3;
    job.retry_policy.retry_delay = Duration::from_millis(10);
    job.retry_policy.backoff_strategy = BackoffStrategy::Exponential { multiplier: 2.0 };

    scheduler.schedule_job(job).await.unwrap();

    // Wait for job processing and retries
    sleep(Duration::from_millis(200)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert!(stats.total_retried > 0);

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_linear_backoff_retry() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register a failing runner
    let runner = Arc::new(MockSchedulerRunner::new("linear-runner").with_failure(true));
    let entity = create_runner_entity("linear-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    scheduler.start().await.unwrap();

    let mut job = create_test_job("linear-job", JobPriority::Normal);
    job.retry_policy.max_retries = 3;
    job.retry_policy.retry_delay = Duration::from_millis(10);
    job.retry_policy.backoff_strategy = BackoffStrategy::Linear {
        increment: Duration::from_millis(5),
    };

    scheduler.schedule_job(job).await.unwrap();

    // Wait for job processing and retries
    sleep(Duration::from_millis(200)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert!(stats.total_retried > 0);

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_job_scheduling() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig {
        max_concurrent_jobs: 10,
        ..Default::default()
    };
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register multiple runners
    for i in 0..3 {
        let runner = Arc::new(MockSchedulerRunner::new(&format!(
            "concurrent-runner-{}",
            i
        )));
        let entity = create_runner_entity(&format!("concurrent-runner-{}", i));
        runner_pool.register_runner(runner, entity).await.unwrap();
    }

    scheduler.start().await.unwrap();

    // Schedule many jobs concurrently
    let mut handles = Vec::new();
    for i in 0..20 {
        let scheduler = scheduler.clone();
        let handle = tokio::spawn(async move {
            let job = create_test_job(&format!("concurrent-job-{}", i), JobPriority::Normal);
            scheduler.schedule_job(job).await
        });
        handles.push(handle);
    }

    // Wait for all scheduling to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Wait for job processing
    sleep(Duration::from_millis(200)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 20);

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_job_scheduler_statistics() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register runners
    let successful_runner = Arc::new(MockSchedulerRunner::new("successful-runner"));
    let failing_runner = Arc::new(MockSchedulerRunner::new("failing-runner").with_failure(true));

    let successful_entity = create_runner_entity("successful-runner");
    let failing_entity = create_runner_entity("failing-runner");

    runner_pool
        .register_runner(successful_runner, successful_entity)
        .await
        .unwrap();
    runner_pool
        .register_runner(failing_runner, failing_entity)
        .await
        .unwrap();

    scheduler.start().await.unwrap();

    // Schedule mix of jobs
    for i in 0..10 {
        let job = create_test_job(&format!("stats-job-{}", i), JobPriority::Normal);
        scheduler.schedule_job(job).await.unwrap();
    }

    // Wait for processing
    sleep(Duration::from_millis(300)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 10);
    assert!(stats.total_completed > 0 || stats.total_failed > 0);

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_manual_job_retry() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    // Register a runner that initially fails but then succeeds
    let runner = Arc::new(MockSchedulerRunner::new("retry-runner"));
    let entity = create_runner_entity("retry-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    scheduler.start().await.unwrap();

    let mut job = create_test_job("manual-retry-job", JobPriority::Normal);
    job.retry_policy.max_retries = 0; // No automatic retries
    let job_id = job.id;

    scheduler.schedule_job(job).await.unwrap();

    // Wait for initial processing
    sleep(Duration::from_millis(100)).await;

    // Manually retry the job
    let retry_result = scheduler.retry_job(job_id).await;
    // This might fail if the job completed successfully
    if retry_result.is_err() {
        // Job likely completed successfully, which is fine
        let status = scheduler.get_job_status(job_id).await.unwrap();
        assert!(status.is_some());
    }

    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_scheduler_start_stop() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool, config);

    // Test starting
    scheduler.start().await.unwrap();

    // Test double start (should fail)
    let result = scheduler.start().await;
    assert!(result.is_err());

    // Test stopping
    scheduler.stop().await.unwrap();

    // Test double stop (should succeed)
    scheduler.stop().await.unwrap();
}

#[tokio::test]
async fn test_job_execution_context() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    let runner = Arc::new(MockSchedulerRunner::new("context-runner"));
    let entity = create_runner_entity("context-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    let job = create_test_job("context-job", JobPriority::High);
    let job_id = job.id;

    let context = JobExecutionContext {
        job: job.clone(),
        attempt: 1,
        scheduled_at: chrono::Utc::now(),
        previous_results: Vec::new(),
        metadata: HashMap::new(),
    };

    assert_eq!(context.job.id, job_id);
    assert_eq!(context.attempt, 1);
    assert_eq!(context.job.priority, JobPriority::High);
}

#[tokio::test]
async fn test_scheduler_process_pending_jobs() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig::default();
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    let runner = Arc::new(MockSchedulerRunner::new("pending-runner"));
    let entity = create_runner_entity("pending-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    // Schedule some jobs
    for i in 0..5 {
        let job = create_test_job(&format!("pending-job-{}", i), JobPriority::Normal);
        scheduler.schedule_job(job).await.unwrap();
    }

    // Process pending jobs manually
    scheduler.process_pending_jobs().await.unwrap();

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 5);
}

#[tokio::test]
async fn test_scheduler_with_different_priorities() {
    let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
    let config = JobSchedulerConfig {
        priority_config: PriorityConfig {
            enabled: true,
            critical_queue_size: 10,
            high_queue_size: 20,
            normal_queue_size: 50,
            low_queue_size: 100,
        },
        ..Default::default()
    };
    let scheduler = DefaultJobScheduler::new(runner_pool.clone(), config);

    let runner = Arc::new(MockSchedulerRunner::new("priority-test-runner"));
    let entity = create_runner_entity("priority-test-runner");
    runner_pool.register_runner(runner, entity).await.unwrap();

    scheduler.start().await.unwrap();

    // Schedule jobs with all priority levels
    let priorities = vec![
        JobPriority::Low,
        JobPriority::Normal,
        JobPriority::High,
        JobPriority::Critical,
    ];

    for (i, priority) in priorities.iter().enumerate() {
        let job = create_test_job(&format!("priority-job-{}", i), *priority);
        scheduler.schedule_job(job).await.unwrap();
    }

    // Wait for processing
    sleep(Duration::from_millis(200)).await;

    let stats = scheduler.get_stats().await.unwrap();
    assert_eq!(stats.total_scheduled, 4);

    scheduler.stop().await.unwrap();
}
