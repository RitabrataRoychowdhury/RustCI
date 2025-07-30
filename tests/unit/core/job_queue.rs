//! Unit tests for the job queue implementation

use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use rustci::core::job_queue::{InMemoryJobQueue, JobQueue, JobQueueConfig};
use rustci::domain::entities::{
    BackoffStrategy, Job, JobPriority, JobRequirements, JobStatus, JobStep, RetryPolicy,
};

/// Helper function to create a test job
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
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    }
}

#[tokio::test]
async fn test_job_queue_creation() {
    let queue = InMemoryJobQueue::with_default_config();

    assert_eq!(queue.size().await.unwrap(), 0);
    assert!(queue.is_empty().await.unwrap());

    let stats = queue.get_stats().await.unwrap();
    assert_eq!(stats.total_queued, 0);
    assert_eq!(stats.total_processed, 0);
    assert_eq!(stats.current_queue_size, 0);
}

#[tokio::test]
async fn test_job_queue_enqueue_dequeue() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("test-job", JobPriority::Normal);
    let job_id = job.id;

    // Enqueue job
    queue.enqueue(job).await.unwrap();

    assert_eq!(queue.size().await.unwrap(), 1);
    assert!(!queue.is_empty().await.unwrap());

    // Dequeue job
    let dequeued = queue.dequeue().await.unwrap();
    assert!(dequeued.is_some());
    assert_eq!(dequeued.unwrap().id, job_id);

    assert_eq!(queue.size().await.unwrap(), 0);
    assert!(queue.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_job_priority_ordering() {
    let queue = InMemoryJobQueue::with_default_config();

    let low_job = create_test_job("low-priority", JobPriority::Low);
    let high_job = create_test_job("high-priority", JobPriority::High);
    let normal_job = create_test_job("normal-priority", JobPriority::Normal);
    let critical_job = create_test_job("critical-priority", JobPriority::Critical);

    // Enqueue in random order
    queue.enqueue(low_job).await.unwrap();
    queue.enqueue(high_job.clone()).await.unwrap();
    queue.enqueue(normal_job.clone()).await.unwrap();
    queue.enqueue(critical_job.clone()).await.unwrap();

    // Should dequeue in priority order: Critical, High, Normal, Low
    let first = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(first.id, critical_job.id);

    let second = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(second.id, high_job.id);

    let third = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(third.id, normal_job.id);

    let fourth = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(fourth.name, "low-priority");
}

#[tokio::test]
async fn test_job_queue_peek() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("peek-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();

    // Peek should return the job without removing it
    let peeked = queue.peek().await.unwrap();
    assert!(peeked.is_some());
    assert_eq!(peeked.unwrap().id, job_id);

    // Queue should still contain the job
    assert_eq!(queue.size().await.unwrap(), 1);

    // Dequeue should still work
    let dequeued = queue.dequeue().await.unwrap();
    assert!(dequeued.is_some());
    assert_eq!(dequeued.unwrap().id, job_id);
}

#[tokio::test]
async fn test_job_queue_get_job() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("get-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();

    // Get job by ID
    let retrieved = queue.get_job(job_id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, job_id);

    // Non-existent job should return None
    let non_existent = queue.get_job(Uuid::new_v4()).await.unwrap();
    assert!(non_existent.is_none());
}

#[tokio::test]
async fn test_job_queue_remove_job() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("remove-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();
    assert_eq!(queue.size().await.unwrap(), 1);

    // Remove job
    let removed = queue.remove_job(job_id).await.unwrap();
    assert!(removed);
    assert_eq!(queue.size().await.unwrap(), 0);

    // Removing non-existent job should return false
    let not_removed = queue.remove_job(Uuid::new_v4()).await.unwrap();
    assert!(!not_removed);
}

#[tokio::test]
async fn test_job_queue_clear() {
    let queue = InMemoryJobQueue::with_default_config();

    // Add multiple jobs
    for i in 0..5 {
        let job = create_test_job(&format!("job-{}", i), JobPriority::Normal);
        queue.enqueue(job).await.unwrap();
    }

    assert_eq!(queue.size().await.unwrap(), 5);

    // Clear queue
    queue.clear().await.unwrap();

    assert_eq!(queue.size().await.unwrap(), 0);
    assert!(queue.is_empty().await.unwrap());

    // Statistics should be reset
    let stats = queue.get_stats().await.unwrap();
    assert_eq!(stats.total_queued, 0);
    assert_eq!(stats.total_processed, 0);
}

#[tokio::test]
async fn test_job_queue_statistics() {
    let queue = InMemoryJobQueue::with_default_config();

    // Enqueue jobs
    for i in 0..3 {
        let job = create_test_job(&format!("stats-job-{}", i), JobPriority::Normal);
        queue.enqueue(job).await.unwrap();
    }

    // Dequeue jobs
    for _ in 0..2 {
        queue.dequeue().await.unwrap();
    }

    let stats = queue.get_stats().await.unwrap();
    assert_eq!(stats.total_queued, 3);
    assert_eq!(stats.total_processed, 2);
    assert_eq!(stats.current_queue_size, 1);
}

#[tokio::test]
async fn test_job_queue_max_size_limit() {
    let config = JobQueueConfig {
        max_queue_size: 2,
        ..Default::default()
    };
    let queue = InMemoryJobQueue::new(config);

    let job1 = create_test_job("job1", JobPriority::Normal);
    let job2 = create_test_job("job2", JobPriority::Normal);
    let job3 = create_test_job("job3", JobPriority::Normal);

    // First two jobs should succeed
    queue.enqueue(job1).await.unwrap();
    queue.enqueue(job2).await.unwrap();

    // Third job should fail due to size limit
    let result = queue.enqueue(job3).await;
    assert!(result.is_err());

    assert_eq!(queue.size().await.unwrap(), 2);
}

#[tokio::test]
async fn test_job_queue_get_jobs_by_priority() {
    let queue = InMemoryJobQueue::with_default_config();

    let high_job1 = create_test_job("high1", JobPriority::High);
    let high_job2 = create_test_job("high2", JobPriority::High);
    let normal_job = create_test_job("normal", JobPriority::Normal);

    queue.enqueue(high_job1.clone()).await.unwrap();
    queue.enqueue(high_job2.clone()).await.unwrap();
    queue.enqueue(normal_job).await.unwrap();

    // Get high priority jobs
    let high_jobs = queue.get_jobs_by_priority(JobPriority::High).await.unwrap();
    assert_eq!(high_jobs.len(), 2);

    // Get normal priority jobs
    let normal_jobs = queue
        .get_jobs_by_priority(JobPriority::Normal)
        .await
        .unwrap();
    assert_eq!(normal_jobs.len(), 1);

    // Get low priority jobs (should be empty)
    let low_jobs = queue.get_jobs_by_priority(JobPriority::Low).await.unwrap();
    assert!(low_jobs.is_empty());
}

#[tokio::test]
async fn test_job_queue_update_job_status() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("status-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();

    // Update job status
    queue
        .update_job_status(job_id, JobStatus::Running)
        .await
        .unwrap();
    queue
        .update_job_status(job_id, JobStatus::Success)
        .await
        .unwrap();

    // This is mainly testing that the method doesn't fail
    // In a real implementation, we'd verify the status was actually updated
}

#[tokio::test]
async fn test_job_queue_retry_job() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("retry-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();

    // Dequeue the job (simulating processing)
    queue.dequeue().await.unwrap();

    // Mark job as failed and retry
    queue
        .update_job_status(job_id, JobStatus::Failed)
        .await
        .unwrap();
    queue.retry_job(job_id).await.unwrap();

    // Job should be back in the queue
    assert_eq!(queue.size().await.unwrap(), 1);
}

#[tokio::test]
async fn test_job_queue_retry_limit() {
    let config = JobQueueConfig {
        max_retry_attempts: 2,
        ..Default::default()
    };
    let queue = InMemoryJobQueue::new(config);
    let job = create_test_job("retry-limit-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();
    queue.dequeue().await.unwrap();

    // Retry job multiple times
    queue
        .update_job_status(job_id, JobStatus::Failed)
        .await
        .unwrap();
    queue.retry_job(job_id).await.unwrap(); // Attempt 1
    queue.dequeue().await.unwrap();

    queue
        .update_job_status(job_id, JobStatus::Failed)
        .await
        .unwrap();
    queue.retry_job(job_id).await.unwrap(); // Attempt 2
    queue.dequeue().await.unwrap();

    // Third retry should fail
    queue
        .update_job_status(job_id, JobStatus::Failed)
        .await
        .unwrap();
    let result = queue.retry_job(job_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_job_queue_process_retries() {
    let config = JobQueueConfig {
        retry_delay_seconds: 1, // Short delay for testing
        ..Default::default()
    };
    let queue = InMemoryJobQueue::new(config);
    let job = create_test_job("process-retry-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();
    queue.dequeue().await.unwrap();

    // Mark job as failed
    queue
        .update_job_status(job_id, JobStatus::Failed)
        .await
        .unwrap();

    // Process retries (should not retry immediately due to delay)
    queue.process_retries().await.unwrap();
    assert_eq!(queue.size().await.unwrap(), 0);

    // Wait for retry delay and process again
    tokio::time::sleep(Duration::from_secs(2)).await;
    queue.process_retries().await.unwrap();

    // Job should be back in queue for retry
    assert_eq!(queue.size().await.unwrap(), 1);
}

#[tokio::test]
async fn test_job_queue_get_retry_jobs() {
    let queue = InMemoryJobQueue::with_default_config();
    let job = create_test_job("get-retry-job", JobPriority::Normal);
    let job_id = job.id;

    queue.enqueue(job).await.unwrap();
    queue.dequeue().await.unwrap();

    // Initially no retry jobs
    let retry_jobs = queue.get_retry_jobs().await.unwrap();
    assert!(retry_jobs.is_empty());

    // Mark job as failed
    queue
        .update_job_status(job_id, JobStatus::Failed)
        .await
        .unwrap();

    // Should now have retry jobs
    let retry_jobs = queue.get_retry_jobs().await.unwrap();
    assert_eq!(retry_jobs.len(), 1);
    assert_eq!(retry_jobs[0].id, job_id);
}

#[tokio::test]
async fn test_job_queue_configuration() {
    let config = JobQueueConfig {
        max_queue_size: 100,
        max_retry_attempts: 5,
        retry_delay_seconds: 60,
        enable_prioritization: true,
        processing_interval_ms: 50,
    };

    let queue = InMemoryJobQueue::new(config.clone());

    assert_eq!(queue.config().max_queue_size, 100);
    assert_eq!(queue.config().max_retry_attempts, 5);
    assert_eq!(queue.config().retry_delay_seconds, 60);
    assert!(queue.config().enable_prioritization);
    assert_eq!(queue.config().processing_interval_ms, 50);
}

#[tokio::test]
async fn test_job_queue_concurrent_operations() {
    let queue = InMemoryJobQueue::with_default_config();
    let queue = std::sync::Arc::new(queue);

    // Spawn multiple tasks to enqueue jobs concurrently
    let mut handles = Vec::new();

    for i in 0..10 {
        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            let job = create_test_job(&format!("concurrent-job-{}", i), JobPriority::Normal);
            queue_clone.enqueue(job).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // All jobs should be enqueued
    assert_eq!(queue.size().await.unwrap(), 10);

    // Dequeue all jobs
    let mut dequeued_count = 0;
    while queue.dequeue().await.unwrap().is_some() {
        dequeued_count += 1;
    }

    assert_eq!(dequeued_count, 10);
    assert!(queue.is_empty().await.unwrap());
}
