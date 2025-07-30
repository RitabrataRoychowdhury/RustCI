//! Unit tests for runner pool manager
//!
//! This module contains comprehensive tests for the runner pool manager,
//! load balancing, scheduling, and failover scenarios.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::load_balancer::{
    FirstAvailableLoadBalancer, LeastLoadedLoadBalancer, LoadBalancer, RandomLoadBalancer,
    RoundRobinLoadBalancer, WeightedLoadBalancer,
};
use rustci::core::runner_pool::{
    DefaultRunnerPoolManager, LoadBalancingStrategy, RunnerPoolConfig, RunnerPoolManager,
    RunnerRegistration, RunnerStats, SchedulingConstraints, SchedulingContext,
};
use rustci::domain::entities::{
    BackoffStrategy, HealthStatus, Job, JobPriority, JobRequirements, JobStep, RetryPolicy, Runner,
    RunnerCapacity, RunnerEntity, RunnerStatus, RunnerType,
};
use rustci::error::{AppError, Result};

// Mock runner implementation for testing
struct MockRunner {
    id: Uuid,
    name: String,
    capacity: RunnerCapacity,
    health_status: HealthStatus,
    execution_delay: Duration,
    should_fail: bool,
}

impl MockRunner {
    fn new(name: &str, max_jobs: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            capacity: RunnerCapacity {
                max_concurrent_jobs: max_jobs,
                current_jobs: 0,
                available_slots: max_jobs,
                cpu_usage: 20.0,
                memory_usage: 30.0,
                disk_usage: 40.0,
            },
            health_status: HealthStatus::Healthy,
            execution_delay: Duration::from_millis(100),
            should_fail: false,
        }
    }

    fn with_load(mut self, current_jobs: u32, cpu_usage: f64, memory_usage: f64) -> Self {
        self.capacity.current_jobs = current_jobs;
        self.capacity.available_slots = self
            .capacity
            .max_concurrent_jobs
            .saturating_sub(current_jobs);
        self.capacity.cpu_usage = cpu_usage;
        self.capacity.memory_usage = memory_usage;
        self
    }

    fn with_health(mut self, health_status: HealthStatus) -> Self {
        self.health_status = health_status;
        self
    }

    fn with_failure(mut self, should_fail: bool) -> Self {
        self.should_fail = should_fail;
        self
    }
}

#[async_trait::async_trait]
impl Runner for MockRunner {
    async fn execute(&self, job: Job) -> Result<rustci::domain::entities::JobResult> {
        sleep(self.execution_delay).await;

        if self.should_fail {
            return Err(AppError::InternalServerError(
                "Mock runner failure".to_string(),
            ));
        }

        let mut result = rustci::domain::entities::JobResult::new(job.id);
        result.complete(rustci::domain::entities::JobStatus::Success, Some(0));
        result.stdout = "Mock execution successful".to_string();
        Ok(result)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(self.health_status.clone())
    }

    async fn get_capacity(&self) -> Result<RunnerCapacity> {
        Ok(self.capacity.clone())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    fn get_metadata(&self) -> rustci::domain::entities::RunnerMetadata {
        rustci::domain::entities::RunnerMetadata {
            id: self.id,
            name: self.name.clone(),
            runner_type: RunnerType::Local {
                max_concurrent_jobs: self.capacity.max_concurrent_jobs,
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
            retry_delay: Duration::from_secs(1), // Short delay for tests
            backoff_strategy: BackoffStrategy::Fixed,
        },
        metadata: HashMap::new(),
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    }
}

fn create_runner_entity(name: &str, runner_type: RunnerType) -> RunnerEntity {
    let mut entity = RunnerEntity::new(name.to_string(), runner_type);
    entity.status = RunnerStatus::Active;
    entity
}

#[tokio::test]
async fn test_runner_pool_manager_creation() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 0);
    assert_eq!(stats.active_runners, 0);
}

#[tokio::test]
async fn test_runner_registration() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner = Arc::new(MockRunner::new("test-runner", 4));
    let entity = create_runner_entity(
        "test-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let runner_id = entity.id;

    pool_manager.register_runner(runner, entity).await.unwrap();

    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 1);
    assert_eq!(stats.active_runners, 1);

    let retrieved_runner = pool_manager.get_runner(runner_id).await.unwrap();
    assert!(retrieved_runner.is_some());
    assert_eq!(retrieved_runner.unwrap().entity.name, "test-runner");
}

#[tokio::test]
async fn test_runner_deregistration() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner = Arc::new(MockRunner::new("test-runner", 4));
    let entity = create_runner_entity(
        "test-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let runner_id = entity.id;

    pool_manager.register_runner(runner, entity).await.unwrap();

    let stats_before = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats_before.total_runners, 1);

    pool_manager.deregister_runner(runner_id).await.unwrap();

    let stats_after = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats_after.total_runners, 0);

    let retrieved_runner = pool_manager.get_runner(runner_id).await.unwrap();
    assert!(retrieved_runner.is_none());
}

#[tokio::test]
async fn test_job_submission_and_execution() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner = Arc::new(MockRunner::new("test-runner", 4));
    let entity = create_runner_entity(
        "test-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager.register_runner(runner, entity).await.unwrap();
    pool_manager.start().await.unwrap();

    let job = create_test_job("test-job", JobPriority::Normal);
    pool_manager.submit_job(job).await.unwrap();

    // Wait for job processing
    sleep(Duration::from_millis(200)).await;

    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_jobs_processed, 1);

    pool_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_health_check_monitoring() {
    let config = RunnerPoolConfig {
        health_check_interval: 1, // 1 second for testing
        ..Default::default()
    };
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let healthy_runner = Arc::new(MockRunner::new("healthy-runner", 4));
    let unhealthy_runner = Arc::new(MockRunner::new("unhealthy-runner", 4).with_health(
        HealthStatus::Unhealthy {
            reason: "Test failure".to_string(),
        },
    ));

    let healthy_entity = create_runner_entity(
        "healthy-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let unhealthy_entity = create_runner_entity(
        "unhealthy-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager
        .register_runner(healthy_runner, healthy_entity)
        .await
        .unwrap();
    pool_manager
        .register_runner(unhealthy_runner, unhealthy_entity)
        .await
        .unwrap();

    pool_manager.health_check_all().await.unwrap();

    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 2);
    assert_eq!(stats.active_runners, 1); // Only healthy runner should be active
    assert_eq!(stats.failed_runners, 1);
}

#[tokio::test]
async fn test_pool_capacity_limit() {
    let config = RunnerPoolConfig {
        max_runners: 2,
        ..Default::default()
    };
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner1 = Arc::new(MockRunner::new("runner1", 4));
    let runner2 = Arc::new(MockRunner::new("runner2", 4));
    let runner3 = Arc::new(MockRunner::new("runner3", 4));

    let entity1 = create_runner_entity(
        "runner1",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let entity2 = create_runner_entity(
        "runner2",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let entity3 = create_runner_entity(
        "runner3",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager
        .register_runner(runner1, entity1)
        .await
        .unwrap();
    pool_manager
        .register_runner(runner2, entity2)
        .await
        .unwrap();

    // Third runner should fail due to capacity limit
    let result = pool_manager.register_runner(runner3, entity3).await;
    assert!(result.is_err());

    let stats = pool_manager.get_stats().await.unwrap();
    assert_eq!(stats.total_runners, 2);
}

// Load Balancer Tests

#[tokio::test]
async fn test_round_robin_load_balancer() {
    let balancer = RoundRobinLoadBalancer::new();
    let runner1 = Uuid::new_v4();
    let runner2 = Uuid::new_v4();
    let runner3 = Uuid::new_v4();

    let available_runners = vec![runner1, runner2, runner3];
    let capacities = HashMap::new();
    let context = SchedulingContext {
        job: create_test_job("test-job", JobPriority::Normal),
        available_runners: available_runners.clone(),
        runner_capacities: capacities,
        strategy: LoadBalancingStrategy::RoundRobin,
        constraints: SchedulingConstraints::default(),
    };
    let runners = HashMap::new();

    // Test round-robin selection
    let selected1 = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();
    let selected2 = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();
    let selected3 = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();
    let selected4 = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(selected1, runner1);
    assert_eq!(selected2, runner2);
    assert_eq!(selected3, runner3);
    assert_eq!(selected4, runner1); // Should wrap around
}

#[tokio::test]
async fn test_least_loaded_load_balancer() {
    let balancer = LeastLoadedLoadBalancer::new();
    let runner1 = Uuid::new_v4();
    let runner2 = Uuid::new_v4();
    let runner3 = Uuid::new_v4();

    let available_runners = vec![runner1, runner2, runner3];
    let mut capacities = HashMap::new();

    // Runner1: 50% job load, 30% CPU, 40% memory = 40% combined
    capacities.insert(
        runner1,
        RunnerCapacity {
            max_concurrent_jobs: 4,
            current_jobs: 2,
            available_slots: 2,
            cpu_usage: 30.0,
            memory_usage: 40.0,
            disk_usage: 50.0,
        },
    );

    // Runner2: 25% job load, 20% CPU, 30% memory = 25% combined
    capacities.insert(
        runner2,
        RunnerCapacity {
            max_concurrent_jobs: 4,
            current_jobs: 1,
            available_slots: 3,
            cpu_usage: 20.0,
            memory_usage: 30.0,
            disk_usage: 40.0,
        },
    );

    // Runner3: 75% job load, 60% CPU, 70% memory = 67.5% combined
    capacities.insert(
        runner3,
        RunnerCapacity {
            max_concurrent_jobs: 4,
            current_jobs: 3,
            available_slots: 1,
            cpu_usage: 60.0,
            memory_usage: 70.0,
            disk_usage: 80.0,
        },
    );

    let context = SchedulingContext {
        job: create_test_job("test-job", JobPriority::Normal),
        available_runners,
        runner_capacities: capacities,
        strategy: LoadBalancingStrategy::LeastLoaded,
        constraints: SchedulingConstraints::default(),
    };
    let runners = HashMap::new();

    let selected = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected, runner2); // Should select least loaded
}

#[tokio::test]
async fn test_weighted_load_balancer() {
    let balancer = WeightedLoadBalancer::new();
    let runner1 = Uuid::new_v4();
    let runner2 = Uuid::new_v4();

    let available_runners = vec![runner1, runner2];
    let mut capacities = HashMap::new();

    // Runner1: High availability
    capacities.insert(
        runner1,
        RunnerCapacity {
            max_concurrent_jobs: 4,
            current_jobs: 1,
            available_slots: 3,
            cpu_usage: 10.0,
            memory_usage: 20.0,
            disk_usage: 30.0,
        },
    );

    // Runner2: Low availability
    capacities.insert(
        runner2,
        RunnerCapacity {
            max_concurrent_jobs: 4,
            current_jobs: 3,
            available_slots: 1,
            cpu_usage: 80.0,
            memory_usage: 90.0,
            disk_usage: 85.0,
        },
    );

    let context = SchedulingContext {
        job: create_test_job("test-job", JobPriority::Normal),
        available_runners,
        runner_capacities: capacities,
        strategy: LoadBalancingStrategy::Weighted,
        constraints: SchedulingConstraints::default(),
    };
    let runners = HashMap::new();

    // Run multiple times to test weighted selection
    let mut runner1_count = 0;
    let mut runner2_count = 0;

    for _ in 0..100 {
        let selected = balancer
            .select_runner(&context, &runners)
            .await
            .unwrap()
            .unwrap();
        if selected == runner1 {
            runner1_count += 1;
        } else {
            runner2_count += 1;
        }
    }

    // Runner1 should be selected more often due to higher weight
    assert!(runner1_count > runner2_count);
    assert!(runner1_count > 60); // Should be significantly more
}

#[tokio::test]
async fn test_first_available_load_balancer() {
    let balancer = FirstAvailableLoadBalancer::new();
    let runner1 = Uuid::new_v4();
    let runner2 = Uuid::new_v4();
    let runner3 = Uuid::new_v4();

    let available_runners = vec![runner1, runner2, runner3];
    let capacities = HashMap::new();
    let context = SchedulingContext {
        job: create_test_job("test-job", JobPriority::Normal),
        available_runners,
        runner_capacities: capacities,
        strategy: LoadBalancingStrategy::FirstAvailable,
        constraints: SchedulingConstraints::default(),
    };
    let runners = HashMap::new();

    let selected = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected, runner1); // Should always select first
}

#[tokio::test]
async fn test_random_load_balancer() {
    let balancer = RandomLoadBalancer::new();
    let runner1 = Uuid::new_v4();
    let runner2 = Uuid::new_v4();

    let available_runners = vec![runner1, runner2];
    let capacities = HashMap::new();
    let context = SchedulingContext {
        job: create_test_job("test-job", JobPriority::Normal),
        available_runners: available_runners.clone(),
        runner_capacities: capacities,
        strategy: LoadBalancingStrategy::Random,
        constraints: SchedulingConstraints::default(),
    };
    let runners = HashMap::new();

    let selected = balancer
        .select_runner(&context, &runners)
        .await
        .unwrap()
        .unwrap();
    assert!(available_runners.contains(&selected));
}

// Failover and Error Handling Tests

#[tokio::test]
async fn test_runner_failover() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let healthy_runner = Arc::new(MockRunner::new("healthy-runner", 4));
    let failing_runner = Arc::new(MockRunner::new("failing-runner", 4).with_failure(true));

    let healthy_entity = create_runner_entity(
        "healthy-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let failing_entity = create_runner_entity(
        "failing-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager
        .register_runner(healthy_runner, healthy_entity)
        .await
        .unwrap();
    pool_manager
        .register_runner(failing_runner, failing_entity)
        .await
        .unwrap();
    pool_manager.start().await.unwrap();

    // Submit multiple jobs - some should succeed, some should fail and retry
    for i in 0..5 {
        let job = create_test_job(&format!("job-{}", i), JobPriority::Normal);
        pool_manager.submit_job(job).await.unwrap();
    }

    // Wait for job processing
    sleep(Duration::from_millis(500)).await;

    let stats = pool_manager.get_stats().await.unwrap();
    // Some jobs should have been processed successfully
    assert!(stats.total_jobs_processed > 0);

    pool_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_job_retry_mechanism() {
    let config = RunnerPoolConfig {
        max_job_retries: 2,
        job_retry_delay: 1, // 1 second for testing
        ..Default::default()
    };
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let failing_runner = Arc::new(MockRunner::new("failing-runner", 4).with_failure(true));
    let failing_entity = create_runner_entity(
        "failing-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager
        .register_runner(failing_runner, failing_entity)
        .await
        .unwrap();
    pool_manager.start().await.unwrap();

    let job = create_test_job("retry-job", JobPriority::Normal);
    pool_manager.submit_job(job).await.unwrap();

    // Wait for job processing and retries
    sleep(Duration::from_millis(1500)).await;

    let stats = pool_manager.get_stats().await.unwrap();
    // Job should have failed after retries
    assert!(stats.total_jobs_failed > 0);

    pool_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_priority_job_scheduling() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner = Arc::new(MockRunner::new("test-runner", 1)); // Only 1 concurrent job
    let entity = create_runner_entity(
        "test-runner",
        RunnerType::Local {
            max_concurrent_jobs: 1,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager.register_runner(runner, entity).await.unwrap();
    pool_manager.start().await.unwrap();

    // Submit jobs with different priorities
    let low_job = create_test_job("low-priority", JobPriority::Low);
    let high_job = create_test_job("high-priority", JobPriority::High);
    let critical_job = create_test_job("critical-priority", JobPriority::Critical);

    pool_manager.submit_job(low_job).await.unwrap();
    pool_manager.submit_job(high_job).await.unwrap();
    pool_manager.submit_job(critical_job).await.unwrap();

    // Wait for job processing
    sleep(Duration::from_millis(500)).await;

    let stats = pool_manager.get_stats().await.unwrap();
    // All jobs should eventually be processed
    assert!(stats.total_jobs_processed > 0);

    pool_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_job_execution() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner = Arc::new(MockRunner::new("concurrent-runner", 10)); // High concurrency
    let entity = create_runner_entity(
        "concurrent-runner",
        RunnerType::Local {
            max_concurrent_jobs: 10,
            working_directory: "/tmp".to_string(),
        },
    );

    pool_manager.register_runner(runner, entity).await.unwrap();
    pool_manager.start().await.unwrap();

    // Submit many jobs concurrently
    let mut handles = Vec::new();
    for i in 0..20 {
        let pool_manager = pool_manager.clone();
        let handle = tokio::spawn(async move {
            let job = create_test_job(&format!("concurrent-job-{}", i), JobPriority::Normal);
            pool_manager.submit_job(job).await
        });
        handles.push(handle);
    }

    // Wait for all submissions to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Wait for job processing
    sleep(Duration::from_millis(300)).await;

    let stats = pool_manager.get_stats().await.unwrap();
    assert!(stats.total_jobs_processed > 0);

    pool_manager.stop().await.unwrap();
}

#[tokio::test]
async fn test_runner_statistics_tracking() {
    let config = RunnerPoolConfig::default();
    let pool_manager = DefaultRunnerPoolManager::new(config);

    let runner = Arc::new(MockRunner::new("stats-runner", 4));
    let entity = create_runner_entity(
        "stats-runner",
        RunnerType::Local {
            max_concurrent_jobs: 4,
            working_directory: "/tmp".to_string(),
        },
    );
    let runner_id = entity.id;

    pool_manager.register_runner(runner, entity).await.unwrap();
    pool_manager.start().await.unwrap();

    // Submit and process some jobs
    for i in 0..5 {
        let job = create_test_job(&format!("stats-job-{}", i), JobPriority::Normal);
        pool_manager.submit_job(job).await.unwrap();
    }

    // Wait for job processing
    sleep(Duration::from_millis(300)).await;

    let stats = pool_manager.get_stats().await.unwrap();
    assert!(stats.total_jobs_processed > 0);
    assert!(stats.avg_job_execution_time > 0.0);
    assert!(stats.pool_utilization >= 0.0);

    let runner_info = pool_manager.get_runner(runner_id).await.unwrap();
    assert!(runner_info.is_some());

    pool_manager.stop().await.unwrap();
}
