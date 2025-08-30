//! Tests for Background Job Management System

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::resources::background_jobs::{
    BackgroundJobManager, JobHandler, JobPriority, JobState, RetryPolicy,
};

/// Test job handler that simulates different job behaviors
struct TestJobHandler {
    job_type: String,
    should_fail: bool,
    execution_delay: Duration,
    fail_count: Arc<tokio::sync::Mutex<u32>>,
    max_failures: u32,
}

impl TestJobHandler {
    fn new(job_type: &str, should_fail: bool, execution_delay: Duration) -> Self {
        Self {
            job_type: job_type.to_string(),
            should_fail,
            execution_delay,
            fail_count: Arc::new(tokio::sync::Mutex::new(0)),
            max_failures: 0,
        }
    }

    fn new_with_intermittent_failures(job_type: &str, max_failures: u32, execution_delay: Duration) -> Self {
        Self {
            job_type: job_type.to_string(),
            should_fail: false,
            execution_delay,
            fail_count: Arc::new(tokio::sync::Mutex::new(0)),
            max_failures,
        }
    }
}

#[async_trait::async_trait]
impl JobHandler for TestJobHandler {
    async fn execute(&self, job_id: Uuid, job_data: serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        sleep(self.execution_delay).await;
        
        if self.should_fail {
            return Err("Configured to fail".into());
        }

        // Handle intermittent failures
        if self.max_failures > 0 {
            let mut fail_count = self.fail_count.lock().await;
            if *fail_count < self.max_failures {
                *fail_count += 1;
                return Err(format!("Intermittent failure #{}", *fail_count).into());
            }
        }
        
        Ok(serde_json::json!({
            "job_id": job_id,
            "input": job_data,
            "result": "success",
            "processed_at": chrono::Utc::now().to_rfc3339()
        }))
    }

    fn get_job_type(&self) -> &str {
        &self.job_type
    }

    fn get_timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    async fn on_progress(&self, job_id: Uuid, progress: f64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Job {} progress: {:.1}%", job_id, progress * 100.0);
        Ok(())
    }
}

#[tokio::test]
async fn test_background_job_manager_creation() {
    let manager = BackgroundJobManager::new(4, Duration::from_secs(3600));
    let stats = manager.get_job_stats().await;
    
    assert_eq!(stats.total_jobs, 0);
    assert_eq!(stats.completed_jobs, 0);
    assert_eq!(stats.failed_jobs, 0);
    assert_eq!(stats.cancelled_jobs, 0);
    assert_eq!(stats.queue_size, 0);
    assert_eq!(stats.active_jobs, 0);
}

#[tokio::test]
async fn test_job_handler_registration() {
    let manager = BackgroundJobManager::new(2, Duration::from_secs(60));
    let handler = Arc::new(TestJobHandler::new("test_job", false, Duration::from_millis(10)));
    
    manager.register_handler(handler).await;
    
    // Handler registration is internal, so we test it indirectly through job execution
    let job_data = serde_json::json!({"test": "data"});
    let job_id = manager.submit_job(
        "test_job".to_string(),
        job_data,
        JobPriority::Normal,
        None,
        None,
        None,
    ).await.unwrap();
    
    assert!(job_id != Uuid::nil());
}

#[tokio::test]
async fn test_job_submission_and_status_tracking() {
    let manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    
    let job_data = serde_json::json!({"message": "hello world"});
    let mut tags = HashMap::new();
    tags.insert("environment".to_string(), "test".to_string());
    
    let job_id = manager.submit_job(
        "test_job".to_string(),
        job_data.clone(),
        JobPriority::High,
        Some(RetryPolicy::default()),
        Some(Duration::from_secs(60)),
        Some(tags.clone()),
    ).await.unwrap();
    
    // Check job status
    let status = manager.get_job_status(job_id).await;
    assert!(status.is_some());
    
    let status = status.unwrap();
    assert_eq!(status.id, job_id);
    assert_eq!(status.job_type, "test_job");
    assert_eq!(status.priority, JobPriority::High);
    assert_eq!(status.state, JobState::Queued);
    assert_eq!(status.tags.get("environment"), Some(&"test".to_string()));
    assert_eq!(status.retry_count, 0);
}

#[tokio::test]
async fn test_job_execution_success() {
    let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    let handler = Arc::new(TestJobHandler::new("success_job", false, Duration::from_millis(20)));
    
    manager.register_handler(handler).await;
    manager.start().await.unwrap();
    
    let job_data = serde_json::json!({"operation": "test_operation"});
    let job_id = manager.submit_job(
        "success_job".to_string(),
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
    assert_eq!(result.job_id, job_id);
    assert!(result.success);
    assert!(result.result_data.is_some());
    assert!(result.error_message.is_none());
    assert!(result.execution_time > Duration::from_millis(0));
    
    manager.stop().await;
}

#[tokio::test]
async fn test_job_execution_failure() {
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
    
    let result = manager.get_job_result(job_id).await;
    assert!(result.is_some());
    
    let result = result.unwrap();
    assert!(!result.success);
    assert!(result.result_data.is_none());
    assert!(result.error_message.is_some());
    assert_eq!(result.error_message.unwrap(), "Configured to fail");
    
    manager.stop().await;
}

#[tokio::test]
async fn test_job_priority_ordering() {
    let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    let handler = Arc::new(TestJobHandler::new("priority_job", false, Duration::from_millis(30)));
    
    manager.register_handler(handler).await;
    
    // Submit jobs in reverse priority order
    let low_job = manager.submit_job("priority_job".to_string(), serde_json::json!({"priority": "low"}), JobPriority::Low, None, None, None).await.unwrap();
    let critical_job = manager.submit_job("priority_job".to_string(), serde_json::json!({"priority": "critical"}), JobPriority::Critical, None, None, None).await.unwrap();
    let normal_job = manager.submit_job("priority_job".to_string(), serde_json::json!({"priority": "normal"}), JobPriority::Normal, None, None, None).await.unwrap();
    let high_job = manager.submit_job("priority_job".to_string(), serde_json::json!({"priority": "high"}), JobPriority::High, None, None, None).await.unwrap();
    
    manager.start().await.unwrap();
    
    // Wait for all jobs to complete
    sleep(Duration::from_millis(200)).await;
    
    // Check completion order (critical should be first, low should be last)
    let critical_result = manager.get_job_result(critical_job).await.unwrap();
    let high_result = manager.get_job_result(high_job).await.unwrap();
    let normal_result = manager.get_job_result(normal_job).await.unwrap();
    let low_result = manager.get_job_result(low_job).await.unwrap();
    
    assert!(critical_result.completed_at <= high_result.completed_at);
    assert!(high_result.completed_at <= normal_result.completed_at);
    assert!(normal_result.completed_at <= low_result.completed_at);
    
    manager.stop().await;
}

#[tokio::test]
async fn test_job_cancellation_queued() {
    let manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    let handler = Arc::new(TestJobHandler::new("cancellable_job", false, Duration::from_millis(10)));
    
    manager.register_handler(handler).await;
    
    let job_id = manager.submit_job(
        "cancellable_job".to_string(),
        serde_json::json!({"test": "data"}),
        JobPriority::Normal,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Cancel job before starting workers
    let cancel_result = manager.cancel_job(job_id).await;
    assert!(cancel_result.is_ok());
    
    let result = manager.get_job_result(job_id).await;
    assert!(result.is_some());
    
    let result = result.unwrap();
    assert!(!result.success);
    assert_eq!(result.error_message.unwrap(), "Job cancelled");
}

#[tokio::test]
async fn test_job_cancellation_nonexistent() {
    let manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    
    let nonexistent_job_id = Uuid::new_v4();
    let cancel_result = manager.cancel_job(nonexistent_job_id).await;
    
    assert!(cancel_result.is_err());
    assert!(cancel_result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_submit_job_and_wait() {
    let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    let handler = Arc::new(TestJobHandler::new("wait_job", false, Duration::from_millis(20)));
    
    manager.register_handler(handler).await;
    manager.start().await.unwrap();
    
    let job_data = serde_json::json!({"sync": "test"});
    let result = manager.submit_job_and_wait(
        "wait_job".to_string(),
        job_data,
        JobPriority::Normal,
        None,
        None,
        None,
    ).await.unwrap();
    
    assert!(result.success);
    assert!(result.result_data.is_some());
    assert!(result.execution_time > Duration::from_millis(0));
    
    manager.stop().await;
}

#[tokio::test]
async fn test_job_statistics_tracking() {
    let mut manager = BackgroundJobManager::new(2, Duration::from_secs(60));
    let success_handler = Arc::new(TestJobHandler::new("success_job", false, Duration::from_millis(10)));
    let failure_handler = Arc::new(TestJobHandler::new("failure_job", true, Duration::from_millis(10)));
    
    manager.register_handler(success_handler).await;
    manager.register_handler(failure_handler).await;
    manager.start().await.unwrap();
    
    // Submit multiple jobs
    for i in 0..3 {
        manager.submit_job(
            "success_job".to_string(),
            serde_json::json!({"iteration": i}),
            JobPriority::Normal,
            None,
            None,
            None,
        ).await.unwrap();
    }
    
    for i in 0..2 {
        manager.submit_job(
            "failure_job".to_string(),
            serde_json::json!({"iteration": i}),
            JobPriority::Normal,
            None,
            None,
            None,
        ).await.unwrap();
    }
    
    // Wait for jobs to complete
    sleep(Duration::from_millis(150)).await;
    
    let stats = manager.get_job_stats().await;
    assert_eq!(stats.total_jobs, 5);
    assert_eq!(stats.completed_jobs, 3);
    assert_eq!(stats.failed_jobs, 2);
    assert_eq!(stats.cancelled_jobs, 0);
    assert!(stats.average_execution_time > Duration::from_millis(0));
    assert_eq!(stats.queue_size, 0); // All jobs should be processed
    assert_eq!(stats.active_jobs, 0); // No jobs should be running
    
    manager.stop().await;
}

#[tokio::test]
async fn test_multiple_workers() {
    let mut manager = BackgroundJobManager::new(3, Duration::from_secs(60));
    let handler = Arc::new(TestJobHandler::new("parallel_job", false, Duration::from_millis(50)));
    
    manager.register_handler(handler).await;
    manager.start().await.unwrap();
    
    // Submit multiple jobs that should be processed in parallel
    let mut job_ids = Vec::new();
    for i in 0..6 {
        let job_id = manager.submit_job(
            "parallel_job".to_string(),
            serde_json::json!({"batch": i}),
            JobPriority::Normal,
            None,
            None,
            None,
        ).await.unwrap();
        job_ids.push(job_id);
    }
    
    let start_time = std::time::Instant::now();
    
    // Wait for all jobs to complete
    sleep(Duration::from_millis(200)).await;
    
    let elapsed = start_time.elapsed();
    
    // Verify all jobs completed
    for job_id in job_ids {
        let result = manager.get_job_result(job_id).await;
        assert!(result.is_some());
        assert!(result.unwrap().success);
    }
    
    // With 3 workers, 6 jobs should complete faster than sequential execution
    // Sequential would take ~300ms, parallel should take ~100ms
    assert!(elapsed < Duration::from_millis(250));
    
    manager.stop().await;
}

#[tokio::test]
async fn test_retry_policy_configuration() {
    let manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    
    let retry_policy = RetryPolicy {
        max_attempts: 5,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_secs(1),
        backoff_multiplier: 1.5,
        retry_on_failure: true,
    };
    
    let job_id = manager.submit_job(
        "retry_job".to_string(),
        serde_json::json!({"test": "retry"}),
        JobPriority::Normal,
        Some(retry_policy.clone()),
        None,
        None,
    ).await.unwrap();
    
    let status = manager.get_job_status(job_id).await.unwrap();
    assert_eq!(status.retry_policy.max_attempts, 5);
    assert_eq!(status.retry_policy.initial_delay, Duration::from_millis(10));
    assert_eq!(status.retry_policy.backoff_multiplier, 1.5);
    assert!(status.retry_policy.retry_on_failure);
}

#[tokio::test]
async fn test_job_timeout_configuration() {
    let manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    
    let timeout = Duration::from_secs(30);
    let job_id = manager.submit_job(
        "timeout_job".to_string(),
        serde_json::json!({"test": "timeout"}),
        JobPriority::Normal,
        None,
        Some(timeout),
        None,
    ).await.unwrap();
    
    let status = manager.get_job_status(job_id).await.unwrap();
    assert_eq!(status.timeout, Some(timeout));
}

#[tokio::test]
async fn test_job_tags_and_metadata() {
    let manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    
    let mut tags = HashMap::new();
    tags.insert("environment".to_string(), "test".to_string());
    tags.insert("user_id".to_string(), "12345".to_string());
    tags.insert("batch_id".to_string(), "batch_001".to_string());
    
    let job_id = manager.submit_job(
        "tagged_job".to_string(),
        serde_json::json!({"operation": "process_data"}),
        JobPriority::High,
        None,
        None,
        Some(tags.clone()),
    ).await.unwrap();
    
    let status = manager.get_job_status(job_id).await.unwrap();
    assert_eq!(status.tags.get("environment"), Some(&"test".to_string()));
    assert_eq!(status.tags.get("user_id"), Some(&"12345".to_string()));
    assert_eq!(status.tags.get("batch_id"), Some(&"batch_001".to_string()));
    assert_eq!(status.priority, JobPriority::High);
}

#[tokio::test]
async fn test_no_handler_for_job_type() {
    let mut manager = BackgroundJobManager::new(1, Duration::from_secs(60));
    manager.start().await.unwrap();
    
    // Submit job without registering handler
    let job_id = manager.submit_job(
        "unknown_job".to_string(),
        serde_json::json!({"test": "data"}),
        JobPriority::Normal,
        None,
        None,
        None,
    ).await.unwrap();
    
    // Wait for job to be processed
    sleep(Duration::from_millis(100)).await;
    
    let result = manager.get_job_result(job_id).await.unwrap();
    assert!(!result.success);
    assert!(result.error_message.is_some());
    assert!(result.error_message.unwrap().contains("No handler found"));
    
    manager.stop().await;
}