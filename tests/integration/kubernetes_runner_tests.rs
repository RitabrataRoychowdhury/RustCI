//! Integration tests for Kubernetes runner
//!
//! These tests require a Kubernetes cluster (like K3s) to be running and accessible.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use RustAutoDevOps::core::event_loop::EventDemultiplexer;
use RustAutoDevOps::domain::entities::{
    BackoffStrategy, HealthStatus, Job, JobPriority, JobRequirements, JobStatus, JobStep,
    ResourceLimits, RetryPolicy, Runner, VolumeMount,
};
use RustAutoDevOps::infrastructure::runners::{
    KubernetesCleanupSettings, KubernetesPodSecurityContext, KubernetesRunner,
    KubernetesRunnerConfig, KubernetesToleration,
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
            timeout: Some(Duration::from_secs(60)),
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

fn create_kubernetes_config() -> KubernetesRunnerConfig {
    KubernetesRunnerConfig {
        name: "test-k8s-runner".to_string(),
        max_concurrent_jobs: 3,
        namespace: "default".to_string(),
        default_image: "alpine:latest".to_string(),
        service_account: None,
        resource_limits: ResourceLimits {
            cpu_limit: 500,           // 0.5 CPU
            memory_limit: 256,        // 256MB
            storage_limit: Some(512), // 512MB
        },
        volume_mounts: Vec::new(),
        environment: HashMap::new(),
        cleanup_settings: KubernetesCleanupSettings {
            remove_jobs: true,
            remove_pods: true,
            cleanup_timeout_seconds: 30,
            ttl_seconds_after_finished: Some(60),
        },
        image_pull_policy: "IfNotPresent".to_string(),
        security_context: None,
        node_selector: HashMap::new(),
        tolerations: Vec::new(),
    }
}

async fn check_kubernetes_available() -> bool {
    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    match KubernetesRunner::new(config, event_demux).await {
        Ok(runner) => runner.check_cluster_connectivity().await.unwrap_or(false),
        Err(_) => false,
    }
}

#[tokio::test]
async fn test_kubernetes_runner_creation() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let metadata = runner.get_metadata();

    assert_eq!(metadata.name, "test-k8s-runner");
    assert!(metadata
        .supported_job_types
        .contains(&"kubernetes".to_string()));
}

#[tokio::test]
async fn test_kubernetes_runner_health_check() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let health = runner.health_check().await.unwrap();

    assert_eq!(health, HealthStatus::Healthy);
}

#[tokio::test]
async fn test_kubernetes_runner_capacity() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let capacity = runner.get_capacity().await.unwrap();

    assert_eq!(capacity.max_concurrent_jobs, 3);
    assert_eq!(capacity.current_jobs, 0);
    assert_eq!(capacity.available_slots, 3);
}

#[tokio::test]
async fn test_kubernetes_runner_simple_job_execution() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let job = create_test_job("echo-test", "echo", vec!["Hello Kubernetes!"]);

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Hello Kubernetes!"));
    assert_eq!(result.step_results.len(), 1);
    assert_eq!(result.step_results[0].status, JobStatus::Success);
}

#[tokio::test]
async fn test_kubernetes_runner_failing_job() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let job = create_test_job("fail-test", "sh", vec!["-c", "exit 1"]);

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_failed());
    assert_eq!(result.exit_code, Some(1));
    assert_eq!(result.step_results.len(), 1);
    assert_eq!(result.step_results[0].status, JobStatus::Failed);
}

#[tokio::test]
async fn test_kubernetes_runner_with_custom_image() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let mut job = create_test_job("ubuntu-test", "cat", vec!["/etc/os-release"]);

    // Set custom image in job metadata
    job.metadata
        .insert("image".to_string(), "ubuntu:22.04".to_string());

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Ubuntu"));
}

#[tokio::test]
async fn test_kubernetes_runner_with_environment_variables() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let mut job = create_test_job("env-test", "sh", vec!["-c", "echo $TEST_VAR"]);

    // Set environment variable in job metadata
    job.metadata.insert(
        "env.TEST_VAR".to_string(),
        "Hello K8s Environment!".to_string(),
    );

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Hello K8s Environment!"));
}

#[tokio::test]
async fn test_kubernetes_runner_can_handle_job() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let job = create_test_job("handle-test", "echo", vec!["test"]);

    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);
}

#[tokio::test]
async fn test_kubernetes_runner_stats() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();
    let stats = runner.get_stats().await;

    assert_eq!(stats.total_jobs_created, 0);
    assert_eq!(stats.successful_executions, 0);
    assert_eq!(stats.failed_executions, 0);
}

#[tokio::test]
async fn test_kubernetes_runner_shutdown() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();

    // Shutdown should complete without error
    let result = runner.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_kubernetes_runner_with_security_context() {
    let security_context = KubernetesPodSecurityContext {
        run_as_user: Some(1000),
        run_as_group: Some(1000),
        run_as_non_root: Some(true),
        fs_group: Some(1000),
    };

    let mut config = create_kubernetes_config();
    config.security_context = Some(security_context);

    assert!(config.security_context.is_some());
    assert_eq!(
        config.security_context.as_ref().unwrap().run_as_user,
        Some(1000)
    );
}

#[tokio::test]
async fn test_kubernetes_runner_with_tolerations() {
    let toleration = KubernetesToleration {
        key: Some("node-type".to_string()),
        operator: Some("Equal".to_string()),
        value: Some("gpu".to_string()),
        effect: Some("NoSchedule".to_string()),
        toleration_seconds: Some(300),
    };

    let mut config = create_kubernetes_config();
    config.tolerations = vec![toleration];

    assert_eq!(config.tolerations.len(), 1);
    assert_eq!(config.tolerations[0].key, Some("node-type".to_string()));
}

#[tokio::test]
async fn test_kubernetes_runner_with_node_selector() {
    let mut config = create_kubernetes_config();
    config
        .node_selector
        .insert("node-type".to_string(), "worker".to_string());
    config
        .node_selector
        .insert("zone".to_string(), "us-west-1a".to_string());

    assert_eq!(config.node_selector.len(), 2);
    assert_eq!(
        config.node_selector.get("node-type"),
        Some(&"worker".to_string())
    );
}

#[tokio::test]
async fn test_kubernetes_runner_with_volume_mounts() {
    let volume_mount = VolumeMount {
        host_path: "/tmp/data".to_string(),
        container_path: "/workspace/data".to_string(),
        read_only: false,
    };

    let mut config = create_kubernetes_config();
    config.volume_mounts = vec![volume_mount];

    assert_eq!(config.volume_mounts.len(), 1);
    assert_eq!(config.volume_mounts[0].container_path, "/workspace/data");
    assert!(!config.volume_mounts[0].read_only);
}

#[tokio::test]
async fn test_kubernetes_resource_limits() {
    let limits = ResourceLimits {
        cpu_limit: 2000,           // 2 CPUs
        memory_limit: 1024,        // 1GB
        storage_limit: Some(2048), // 2GB
    };

    assert_eq!(limits.cpu_limit, 2000);
    assert_eq!(limits.memory_limit, 1024);
    assert_eq!(limits.storage_limit, Some(2048));
}

#[tokio::test]
async fn test_kubernetes_cleanup_settings() {
    let cleanup = KubernetesCleanupSettings {
        remove_jobs: true,
        remove_pods: false,
        cleanup_timeout_seconds: 120,
        ttl_seconds_after_finished: Some(600),
    };

    assert!(cleanup.remove_jobs);
    assert!(!cleanup.remove_pods);
    assert_eq!(cleanup.cleanup_timeout_seconds, 120);
    assert_eq!(cleanup.ttl_seconds_after_finished, Some(600));
}

#[tokio::test]
async fn test_kubernetes_runner_multiple_steps() {
    if !check_kubernetes_available().await {
        println!("Kubernetes not available, skipping test");
        return;
    }

    let config = create_kubernetes_config();
    let event_demux = Arc::new(EventDemultiplexer::new());

    let runner = KubernetesRunner::new(config, event_demux).await.unwrap();

    let mut job = Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: "multi-step-test".to_string(),
        steps: vec![
            JobStep {
                name: "step1".to_string(),
                command: "echo".to_string(),
                args: vec!["Step 1 completed".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(30)),
                continue_on_error: false,
            },
            JobStep {
                name: "step2".to_string(),
                command: "echo".to_string(),
                args: vec!["Step 2 completed".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(30)),
                continue_on_error: false,
            },
        ],
        requirements: JobRequirements::default(),
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(300),
        retry_policy: RetryPolicy::default(),
        metadata: HashMap::new(),
        created_at: Utc::now(),
        scheduled_at: None,
    };

    let result = runner.execute(job).await.unwrap();

    assert!(result.is_success());
    assert!(result.stdout.contains("Step 1 completed"));
    assert!(result.stdout.contains("Step 2 completed"));
}
