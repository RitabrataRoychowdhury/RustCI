//! Integration tests for Docker runner
//!
//! These tests require Docker to be running and accessible.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use RustAutoDevOps::core::event_loop::EventDemultiplexer;
use RustAutoDevOps::domain::entities::{
    BackoffStrategy, HealthStatus, Job, JobPriority, JobRequirements, JobStatus, JobStep,
    RetryPolicy, Runner,
};
use RustAutoDevOps::infrastructure::runners::{
    DockerCleanupSettings, DockerResourceLimits, DockerRunner, DockerRunnerConfig, ImagePullPolicy,
};

fn create_test_job(name: &str, command: &str, args: Vec<&str>) -> Job {
    Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: name.to_string(),
        steps: vec![JobStep {
            name: "test-step".to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(30)),
            continue_on_error: false,
        }],
        requirements: JobRequirements::default(),
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(300),
        retry_policy: RetryPolicy {
            max_retries: 1,
            retry_delay: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Fixed,
        },
        metadata: HashMap::new(),
        created_at: Utc::now(),
        scheduled_at: None,
    }
}

fn create_docker_config() -> DockerRunnerConfig {
    DockerRunnerConfig {
        name: "test-docker-runner".to_string(),
        max_concurrent_jobs: 2,
        docker_endpoint: "unix:///var/run/docker.sock".to_string(),
        default_image: "alpine:latest".to_string(),
        network: None,
        volume_mounts: Vec::new(),
        environment: HashMap::new(),
        resource_limits: DockerResourceLimits {
            memory_limit: Some(128 * 1024 * 1024), // 128MB
            cpu_limit: Some(500_000_000),          // 0.5 CPU
            cpu_shares: Some(512),
            disk_limit: Some(100 * 1024 * 1024), // 100MB
        },
        cleanup_settings: DockerCleanupSettings {
            remove_containers: true,
            remove_volumes: true,
            removal_timeout_seconds: 10,
        },
        image_pull_policy: ImagePullPolicy::IfNotPresent,
    }
}

async fn check_docker_available() -> bool {
    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    match DockerRunner::new(config, event_demux).await {
        Ok(runner) => runner.check_docker_connectivity().await.unwrap_or(false),
        Err(_) => false,
    }
}

#[tokio::test]
async fn test_docker_runner_creation() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let metadata = runner.get_metadata();

    assert_eq!(metadata.name, "test-docker-runner");
    assert!(metadata.supported_job_types.contains(&"docker".to_string()));
}

#[tokio::test]
async fn test_docker_runner_health_check() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let health = runner.health_check().await.unwrap();

    assert_eq!(health, HealthStatus::Healthy);
}

#[tokio::test]
async fn test_docker_runner_capacity() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let capacity = runner.get_capacity().await.unwrap();

    assert_eq!(capacity.max_concurrent_jobs, 2);
    assert_eq!(capacity.current_jobs, 0);
    assert_eq!(capacity.available_slots, 2);
}

#[tokio::test]
async fn test_docker_runner_simple_job_execution() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let job = create_test_job("echo-test", "echo", vec!["Hello Docker!"]);

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Hello Docker!"));
    assert_eq!(result.step_results.len(), 1);
    assert_eq!(result.step_results[0].status, JobStatus::Success);
}

#[tokio::test]
async fn test_docker_runner_failing_job() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let job = create_test_job("fail-test", "sh", vec!["-c", "exit 1"]);

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_failed());
    assert_eq!(result.exit_code, Some(1));
    assert_eq!(result.step_results.len(), 1);
    assert_eq!(result.step_results[0].status, JobStatus::Failed);
}

#[tokio::test]
async fn test_docker_runner_with_custom_image() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let mut job = create_test_job("ubuntu-test", "cat", vec!["/etc/os-release"]);

    // Set custom image in job metadata
    job.metadata
        .insert("image".to_string(), "ubuntu:22.04".to_string());

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Ubuntu"));
}

#[tokio::test]
async fn test_docker_runner_with_environment_variables() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let mut job = create_test_job("env-test", "sh", vec!["-c", "echo $TEST_VAR"]);

    // Set environment variable in job metadata
    job.metadata
        .insert("env.TEST_VAR".to_string(), "Hello Environment!".to_string());

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Hello Environment!"));
}

#[tokio::test]
async fn test_docker_runner_job_timeout() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let mut job = create_test_job("timeout-test", "sleep", vec!["10"]);

    // Set short timeout
    job.steps[0].timeout = Some(Duration::from_secs(2));

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_failed());
    assert!(result.error_message.is_some());
    assert!(result.error_message.as_ref().unwrap().contains("timed out"));
}

#[tokio::test]
async fn test_docker_runner_can_handle_job() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let job = create_test_job("handle-test", "echo", vec!["test"]);

    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);
}

#[tokio::test]
async fn test_docker_runner_stats() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();
    let stats = runner.get_stats().await;

    assert_eq!(stats.total_containers_created, 0);
    assert_eq!(stats.successful_executions, 0);
    assert_eq!(stats.failed_executions, 0);
}

#[tokio::test]
async fn test_docker_runner_shutdown() {
    if !check_docker_available().await {
        println!("Docker not available, skipping test");
        return;
    }

    let config = create_docker_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = DockerRunner::new(config, event_demux).await.unwrap();

    // Shutdown should complete without error
    let result = runner.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_docker_runner_image_pull_policies() {
    // Test different image pull policies
    let always_policy = ImagePullPolicy::Always;
    let if_not_present_policy = ImagePullPolicy::IfNotPresent;
    let never_policy = ImagePullPolicy::Never;

    assert!(matches!(always_policy, ImagePullPolicy::Always));
    assert!(matches!(
        if_not_present_policy,
        ImagePullPolicy::IfNotPresent
    ));
    assert!(matches!(never_policy, ImagePullPolicy::Never));
}

#[tokio::test]
async fn test_docker_runner_resource_limits() {
    let limits = DockerResourceLimits {
        memory_limit: Some(256 * 1024 * 1024),
        cpu_limit: Some(1_000_000_000),
        cpu_shares: Some(1024),
        disk_limit: Some(500 * 1024 * 1024),
    };

    assert_eq!(limits.memory_limit, Some(256 * 1024 * 1024));
    assert_eq!(limits.cpu_limit, Some(1_000_000_000));
    assert_eq!(limits.cpu_shares, Some(1024));
    assert_eq!(limits.disk_limit, Some(500 * 1024 * 1024));
}

#[tokio::test]
async fn test_docker_runner_cleanup_settings() {
    let cleanup = DockerCleanupSettings {
        remove_containers: true,
        remove_volumes: false,
        removal_timeout_seconds: 60,
    };

    assert!(cleanup.remove_containers);
    assert!(!cleanup.remove_volumes);
    assert_eq!(cleanup.removal_timeout_seconds, 60);
}
