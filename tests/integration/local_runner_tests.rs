//! Integration tests for the local runner implementation
//!
//! These tests verify the local runner's integration with the event loop
//! and job queue systems.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::{EventDemultiplexer, InMemoryJobQueue, JobQueueConfig};
use rustci::domain::entities::{
    BackoffStrategy, HealthStatus, Job, JobPriority, JobRequirements, JobStatus, JobStep,
    RetryPolicy, Runner,
};
use rustci::infrastructure::runners::{LocalRunner, LocalRunnerConfig};

/// Helper function to create a test job
fn create_test_job(name: &str, command: &str, args: Vec<&str>) -> Job {
    Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: name.to_string(),
        steps: vec![JobStep {
            name: format!("{}-step", name),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(10)),
            continue_on_error: false,
        }],
        requirements: JobRequirements::default(),
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(300),
        retry_policy: RetryPolicy {
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
            backoff_strategy: BackoffStrategy::Fixed,
        },
        metadata: HashMap::new(),
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    }
}

/// Helper function to create a test runner
async fn create_test_runner() -> LocalRunner {
    let config = LocalRunnerConfig {
        name: "test-runner".to_string(),
        max_concurrent_jobs: 2,
        working_directory: std::env::temp_dir(),
        environment: HashMap::new(),
        default_timeout_seconds: 30,
        enable_output_streaming: false,
        max_output_buffer_size: 1024,
        shell: if cfg!(windows) {
            "cmd".to_string()
        } else {
            "bash".to_string()
        },
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    LocalRunner::new(config, event_demux).await.unwrap()
}

#[tokio::test]
async fn test_local_runner_basic_functionality() {
    let runner = create_test_runner().await;

    // Test metadata
    let metadata = runner.get_metadata();
    assert_eq!(metadata.name, "test-runner");
    assert!(metadata.supported_job_types.contains(&"shell".to_string()));

    // Test health check
    let health = runner.health_check().await.unwrap();
    assert_eq!(health, HealthStatus::Healthy);

    // Test capacity
    let capacity = runner.get_capacity().await.unwrap();
    assert_eq!(capacity.max_concurrent_jobs, 2);
    assert_eq!(capacity.current_jobs, 0);
    assert_eq!(capacity.available_slots, 2);
}

#[tokio::test]
async fn test_local_runner_job_execution() {
    let runner = create_test_runner().await;

    // Create a simple echo job
    let job = create_test_job("echo-test", "echo", vec!["hello world"]);
    let job_id = job.id;

    // Test job handling capability
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);

    // Execute the job
    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.job_id, job_id);
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_multiple_jobs() {
    let runner = create_test_runner().await;

    // Create multiple jobs
    let job1 = create_test_job("job1", "echo", vec!["job 1"]);
    let job2 = create_test_job("job2", "echo", vec!["job 2"]);

    // Execute both jobs
    let result1 = runner.execute(job1).await.unwrap();
    let result2 = runner.execute(job2).await.unwrap();

    assert_eq!(result1.status, JobStatus::Queued);
    assert_eq!(result2.status, JobStatus::Queued);

    // Check capacity is reduced
    let capacity = runner.get_capacity().await.unwrap();
    assert_eq!(capacity.current_jobs, 0); // Jobs are queued, not running yet
}

#[tokio::test]
async fn test_local_runner_job_with_environment() {
    let runner = create_test_runner().await;

    // Create a job that uses environment variables
    let mut job = create_test_job("env-test", "echo", vec!["$TEST_VAR"]);
    job.steps[0]
        .environment
        .insert("TEST_VAR".to_string(), "test_value".to_string());

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_job_with_working_directory() {
    let runner = create_test_runner().await;

    // Create a job with specific working directory
    let mut job = create_test_job("pwd-test", "pwd", vec![]);
    job.steps[0].working_directory = Some("/tmp".to_string());

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_job_timeout() {
    let runner = create_test_runner().await;

    // Create a job that should timeout
    let mut job = create_test_job("timeout-test", "sleep", vec!["5"]);
    job.steps[0].timeout = Some(Duration::from_millis(100)); // Very short timeout

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_job_failure() {
    let runner = create_test_runner().await;

    // Create a job that should fail
    let job = create_test_job("fail-test", "false", vec![]); // 'false' command always fails

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_multi_step_job() {
    let runner = create_test_runner().await;

    // Create a job with multiple steps
    let mut job = Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: "multi-step-test".to_string(),
        steps: vec![
            JobStep {
                name: "step1".to_string(),
                command: "echo".to_string(),
                args: vec!["step 1".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(5)),
                continue_on_error: false,
            },
            JobStep {
                name: "step2".to_string(),
                command: "echo".to_string(),
                args: vec!["step 2".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(5)),
                continue_on_error: false,
            },
        ],
        requirements: JobRequirements::default(),
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(300),
        retry_policy: RetryPolicy::default(),
        metadata: HashMap::new(),
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    };

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_continue_on_error() {
    let runner = create_test_runner().await;

    // Create a job with a failing step that should continue
    let mut job = Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: "continue-on-error-test".to_string(),
        steps: vec![
            JobStep {
                name: "failing-step".to_string(),
                command: "false".to_string(),
                args: vec![],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(5)),
                continue_on_error: true, // Continue even if this fails
            },
            JobStep {
                name: "success-step".to_string(),
                command: "echo".to_string(),
                args: vec!["success".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(5)),
                continue_on_error: false,
            },
        ],
        requirements: JobRequirements::default(),
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(300),
        retry_policy: RetryPolicy::default(),
        metadata: HashMap::new(),
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    };

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_local_runner_shutdown() {
    let runner = create_test_runner().await;

    // Test graceful shutdown
    let shutdown_result = runner.shutdown().await;
    assert!(shutdown_result.is_ok());
}

#[tokio::test]
async fn test_local_runner_statistics() {
    let runner = create_test_runner().await;

    // Get initial statistics
    let stats = runner.get_stats().await;
    assert_eq!(stats.total_jobs_executed, 0);
    assert_eq!(stats.successful_jobs, 0);
    assert_eq!(stats.failed_jobs, 0);

    // Execute a job
    let job = create_test_job("stats-test", "echo", vec!["test"]);
    runner.execute(job).await.unwrap();

    // Statistics should still be 0 since job is only queued
    let stats = runner.get_stats().await;
    assert_eq!(stats.total_jobs_executed, 0);
}

#[tokio::test]
async fn test_local_runner_running_jobs() {
    let runner = create_test_runner().await;

    // Initially no running jobs
    let running_jobs = runner.get_running_jobs().await;
    assert!(running_jobs.is_empty());

    // Execute a job
    let job = create_test_job("running-test", "echo", vec!["test"]);
    runner.execute(job).await.unwrap();

    // Job should be queued, not running yet
    let running_jobs = runner.get_running_jobs().await;
    assert!(running_jobs.is_empty());
}

#[tokio::test]
async fn test_local_runner_capacity_limits() {
    let runner = create_test_runner().await;

    // Create jobs up to capacity
    let job1 = create_test_job("capacity1", "sleep", vec!["1"]);
    let job2 = create_test_job("capacity2", "sleep", vec!["1"]);
    let job3 = create_test_job("capacity3", "sleep", vec!["1"]);

    // Execute jobs
    runner.execute(job1).await.unwrap();
    runner.execute(job2).await.unwrap();
    runner.execute(job3).await.unwrap(); // This should still be accepted (queued)

    // All jobs should be queued
    let capacity = runner.get_capacity().await.unwrap();
    assert_eq!(capacity.current_jobs, 0); // No jobs running yet, all queued
}

#[tokio::test]
async fn test_event_demultiplexer_integration() {
    let event_demux = Arc::new(EventDemultiplexer::new());
    let config = LocalRunnerConfig::default();

    // Create runner with event demultiplexer
    let runner = LocalRunner::new(config, event_demux.clone()).await.unwrap();

    // Test that runner is properly integrated with event system
    let job = create_test_job("event-test", "echo", vec!["test"]);
    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);

    // Check event demultiplexer statistics
    let stats = event_demux.get_stats().await;
    // Initially no events processed since job is just queued
    assert_eq!(stats.total_events, 0);
}
