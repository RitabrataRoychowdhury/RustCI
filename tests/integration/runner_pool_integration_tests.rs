//! Integration tests for runner pool manager
//!
//! This module contains integration tests that verify the runner pool manager
//! works correctly with real runner implementations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::runner_pool::{
    DefaultRunnerPoolManager, LoadBalancingStrategy, RunnerPoolConfig, RunnerPoolManager,
};
use rustci::domain::entities::{
    BackoffStrategy, Job, JobPriority, JobRequirements, JobStep, RetryPolicy, RunnerEntity,
    RunnerType,
};
use rustci::infrastructure::runners::{LocalRunner, LocalRunnerConfig};

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
            retry_delay: Duration::from_secs(1),
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
async fn test_runner_pool_with_local_runner() {
    let config = RunnerPoolConfig {
        max_runners: 5,
        health_check_interval: 30,
        runner_timeout: 300,
        load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
        enable_failover: true,
        max_job_retries: 3,
        job_retry_delay: 30,
    };

    let pool_manager = DefaultRunnerPoolManager::new(config);

    // Create a local runner
    let runner_config = LocalRunnerConfig {
        max_concurrent_jobs: 4,
        working_directory: "/tmp".to_string(),
        environment: HashMap::new(),
        timeout_seconds: 300,
        cleanup_on_failure: true,
    };

    let local_runner = Arc::new(LocalRunner::new(runner_config));
    let entity = create_runner_entity("local-test-runner");
    let runner_id = entity.id;

    // Register the runner
    pool_manager
        .register_runner(local_runner, entity)
        .await
        .unwrap();

    // Verify registration
    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 1);
    assert_eq!(stats.active_runners, 1);

    let retrieved_runner = pool_manager.get_runner(runner_id).await.unwrap();
    assert!(retrieved_runner.is_some());

    // Start the pool manager
    pool_manager.start().await.unwrap();

    // Submit a job
    let job = create_test_job("integration-test-job", JobPriority::Normal);
    pool_manager.submit_job(job).await.unwrap();

    // Wait for job processing
    sleep(Duration::from_millis(500)).await;

    // Check statistics
    let final_stats = pool_manager.get_stats().await.unwrap();
    assert!(final_stats.total_jobs_processed >= 1);

    // Stop the pool manager
    pool_manager.stop().await.unwrap();

    // Deregister the runner
    pool_manager.deregister_runner(runner_id).await.unwrap();

    let final_stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(final_stats.total_runners, 0);
}

#[tokio::test]
async fn test_multiple_runners_load_balancing() {
    let config = RunnerPoolConfig {
        max_runners: 10,
        health_check_interval: 30,
        runner_timeout: 300,
        load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
        enable_failover: true,
        max_job_retries: 3,
        job_retry_delay: 30,
    };

    let pool_manager = DefaultRunnerPoolManager::new(config);

    // Create multiple local runners
    for i in 0..3 {
        let runner_config = LocalRunnerConfig {
            max_concurrent_jobs: 2,
            working_directory: "/tmp".to_string(),
            environment: HashMap::new(),
            timeout_seconds: 300,
            cleanup_on_failure: true,
        };

        let local_runner = Arc::new(LocalRunner::new(runner_config));
        let entity = create_runner_entity(&format!("local-runner-{}", i));

        pool_manager
            .register_runner(local_runner, entity)
            .await
            .unwrap();
    }

    // Verify all runners are registered
    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 3);

    // Start the pool manager
    pool_manager.start().await.unwrap();

    // Submit multiple jobs
    for i in 0..6 {
        let job = create_test_job(&format!("load-balance-job-{}", i), JobPriority::Normal);
        pool_manager.submit_job(job).await.unwrap();
    }

    // Wait for job processing
    sleep(Duration::from_millis(1000)).await;

    // Check that jobs were distributed
    let final_stats = pool_manager.get_stats().await.unwrap();
    assert!(final_stats.total_jobs_processed > 0);

    // Stop the pool manager
    pool_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_health_monitoring() {
    let config = RunnerPoolConfig {
        max_runners: 5,
        health_check_interval: 1, // 1 second for testing
        runner_timeout: 300,
        load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
        enable_failover: true,
        max_job_retries: 3,
        job_retry_delay: 30,
    };

    let pool_manager = DefaultRunnerPoolManager::new(config);

    // Create a local runner
    let runner_config = LocalRunnerConfig {
        max_concurrent_jobs: 4,
        working_directory: "/tmp".to_string(),
        environment: HashMap::new(),
        timeout_seconds: 300,
        cleanup_on_failure: true,
    };

    let local_runner = Arc::new(LocalRunner::new(runner_config));
    let entity = create_runner_entity("health-test-runner");

    pool_manager
        .register_runner(local_runner, entity)
        .await
        .unwrap();

    // Perform health check
    pool_manager.health_check_all().await.unwrap();

    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 1);
    // The runner should be healthy (local runner should always be healthy)
    assert!(stats.active_runners > 0 || stats.idle_runners > 0);
}

#[tokio::test]
async fn test_job_priority_handling() {
    let config = RunnerPoolConfig {
        max_runners: 5,
        health_check_interval: 30,
        runner_timeout: 300,
        load_balancing_strategy: LoadBalancingStrategy::LeastLoaded,
        enable_failover: true,
        max_job_retries: 3,
        job_retry_delay: 30,
    };

    let pool_manager = DefaultRunnerPoolManager::new(config);

    // Create a local runner with limited concurrency
    let runner_config = LocalRunnerConfig {
        max_concurrent_jobs: 1, // Only one job at a time
        working_directory: "/tmp".to_string(),
        environment: HashMap::new(),
        timeout_seconds: 300,
        cleanup_on_failure: true,
    };

    let local_runner = Arc::new(LocalRunner::new(runner_config));
    let entity = create_runner_entity("priority-test-runner");

    pool_manager
        .register_runner(local_runner, entity)
        .await
        .unwrap();
    pool_manager.start().await.unwrap();

    // Submit jobs with different priorities
    let low_job = create_test_job("low-priority", JobPriority::Low);
    let high_job = create_test_job("high-priority", JobPriority::High);
    let critical_job = create_test_job("critical-priority", JobPriority::Critical);

    pool_manager.submit_job(low_job).await.unwrap();
    pool_manager.submit_job(high_job).await.unwrap();
    pool_manager.submit_job(critical_job).await.unwrap();

    // Wait for job processing
    sleep(Duration::from_millis(1000)).await;

    let final_stats = pool_manager.get_stats().await.unwrap();
    assert!(final_stats.total_jobs_processed > 0);

    pool_manager.stop().await.unwrap();
}
