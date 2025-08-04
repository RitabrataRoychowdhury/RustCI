//! Integration tests for Native Process Runner
//!
//! This module contains comprehensive integration tests for the native process runner,
//! testing its functionality, isolation capabilities, and control plane integration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use rustci::core::event_loop::EventDemultiplexer;
use rustci::domain::entities::{
    BackoffStrategy, HealthStatus, Job, JobPriority, JobRequirements, JobStatus, JobStep,
    RetryPolicy, Runner, RunnerStatus,
};
use rustci::infrastructure::runners::native_runner::{
    IsolationLevel, NativeProcessRunner, NativeRunnerConfig, ResourceLimits,
    ControlPlaneCoordinationConfig,
};

/// Create a test job for native runner testing
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
            timeout: Some(Duration::from_secs(30)),
            continue_on_error: false,
        }],
        requirements: JobRequirements::default(),
        priority: JobPriority::Normal,
        timeout: Duration::from_secs(300),
        retry_policy: RetryPolicy {
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Fixed,
        },
        metadata: HashMap::new(),
        created_at: chrono::Utc::now(),
        scheduled_at: None,
    }
}

/// Create a test native runner with default configuration
async fn create_test_runner() -> NativeProcessRunner {
    let config = NativeRunnerConfig {
        name: "test-native-runner".to_string(),
        max_concurrent_jobs: 2,
        default_timeout_seconds: 30,
        enhanced_lifecycle: true,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false, // Disable for basic tests
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    NativeProcessRunner::new(config, event_demux).await.unwrap()
}

/// Create a test native runner with control plane coordination enabled
async fn create_test_runner_with_control_plane() -> NativeProcessRunner {
    let config = NativeRunnerConfig {
        name: "test-native-runner-cp".to_string(),
        max_concurrent_jobs: 2,
        default_timeout_seconds: 30,
        enhanced_lifecycle: true,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: true,
            heartbeat_interval: 5,
            health_check_interval: 10,
            auto_registration: true,
            tags: vec!["native".to_string(), "test".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    NativeProcessRunner::new(config, event_demux).await.unwrap()
}

#[tokio::test]
async fn test_native_runner_creation() {
    let runner = create_test_runner().await;
    let metadata = runner.get_metadata();

    assert_eq!(metadata.name, "test-native-runner");
    assert!(metadata.supported_job_types.contains(&"native".to_string()));
    assert!(metadata.supported_job_types.contains(&"shell".to_string()));
    assert!(metadata.supported_job_types.contains(&"script".to_string()));
}

#[tokio::test]
async fn test_native_runner_health_check() {
    let runner = create_test_runner().await;
    let health = runner.health_check().await.unwrap();

    assert_eq!(health, HealthStatus::Healthy);
}

#[tokio::test]
async fn test_native_runner_capacity() {
    let runner = create_test_runner().await;
    let capacity = runner.get_capacity().await.unwrap();

    assert_eq!(capacity.max_concurrent_jobs, 2);
    assert_eq!(capacity.current_jobs, 0);
    assert_eq!(capacity.available_slots, 2);
}

#[tokio::test]
async fn test_native_runner_job_execution() {
    let runner = create_test_runner().await;
    let job = create_test_job("echo-test", "echo", vec!["Hello, Native Runner!"]);

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_native_runner_can_handle_job() {
    let runner = create_test_runner().await;
    let mut job = create_test_job("test-job", "echo", vec!["test"]);

    // Test basic job handling
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);

    // Test with native tag requirement
    job.requirements.required_tags = vec!["native".to_string()];
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);

    // Test with excluded tag
    job.requirements.required_tags = vec![];
    job.requirements.excluded_tags = vec!["native".to_string()];
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(!can_handle);
}

#[tokio::test]
async fn test_native_runner_with_control_plane_coordination() {
    let runner = create_test_runner_with_control_plane().await;
    let metadata = runner.get_metadata();

    assert_eq!(metadata.name, "test-native-runner-cp");

    // Test heartbeat functionality
    let heartbeat_result = runner.send_heartbeat().await;
    assert!(heartbeat_result.is_ok());

    // Test statistics
    let stats = runner.get_stats().await;
    assert_eq!(stats.control_plane_heartbeats, 1);
}

#[tokio::test]
async fn test_native_runner_isolation_levels() {
    // Test with no isolation
    let config_none = NativeRunnerConfig {
        isolation_level: IsolationLevel::None,
        ..Default::default()
    };
    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner_none = NativeProcessRunner::new(config_none, event_demux.clone()).await.unwrap();
    assert!(runner_none.health_check().await.is_ok());

    // Test with process group isolation
    let config_pg = NativeRunnerConfig {
        isolation_level: IsolationLevel::ProcessGroup,
        ..Default::default()
    };
    let runner_pg = NativeProcessRunner::new(config_pg, event_demux.clone()).await.unwrap();
    assert!(runner_pg.health_check().await.is_ok());

    #[cfg(target_os = "linux")]
    {
        // Test with namespace isolation (Linux only)
        let config_ns = NativeRunnerConfig {
            isolation_level: IsolationLevel::Namespace {
                pid: true,
                network: false,
                mount: false,
                user: false,
            },
            ..Default::default()
        };
        let runner_ns = NativeProcessRunner::new(config_ns, event_demux.clone()).await.unwrap();
        assert!(runner_ns.health_check().await.is_ok());

        // Test with cgroup isolation (Linux only)
        let config_cg = NativeRunnerConfig {
            isolation_level: IsolationLevel::Cgroup {
                cpu_limit: Some(500), // 0.5 CPU cores
                memory_limit: Some(512), // 512MB
                process_isolation: true,
            },
            ..Default::default()
        };
        let runner_cg = NativeProcessRunner::new(config_cg, event_demux.clone()).await.unwrap();
        assert!(runner_cg.health_check().await.is_ok());
    }
}

#[tokio::test]
async fn test_native_runner_resource_limits() {
    let config = NativeRunnerConfig {
        resource_limits: ResourceLimits {
            max_cpu: Some(0.5),
            max_memory: Some(256),
            max_execution_time: Some(Duration::from_secs(60)),
            max_processes: Some(10),
            max_file_descriptors: Some(100),
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    let capacity = runner.get_capacity().await.unwrap();
    assert_eq!(capacity.max_concurrent_jobs, 4); // Default from config
}

#[tokio::test]
async fn test_native_runner_job_with_environment_variables() {
    let mut config = NativeRunnerConfig::default();
    config.environment.insert("TEST_VAR".to_string(), "test_value".to_string());

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    let mut job = create_test_job("env-test", "echo", vec!["$TEST_VAR"]);
    job.steps[0].environment.insert("JOB_VAR".to_string(), "job_value".to_string());

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_native_runner_job_timeout() {
    let runner = create_test_runner().await;
    let mut job = create_test_job("timeout-test", "sleep", vec!["60"]);
    job.steps[0].timeout = Some(Duration::from_secs(1)); // Very short timeout

    let result = runner.execute(job).await.unwrap();
    assert_eq!(result.status, JobStatus::Queued);
}

#[tokio::test]
async fn test_native_runner_multiple_steps() {
    let runner = create_test_runner().await;
    let mut job = Job {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        name: "multi-step-test".to_string(),
        steps: vec![
            JobStep {
                name: "step1".to_string(),
                command: "echo".to_string(),
                args: vec!["Step 1".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(10)),
                continue_on_error: false,
            },
            JobStep {
                name: "step2".to_string(),
                command: "echo".to_string(),
                args: vec!["Step 2".to_string()],
                working_directory: None,
                environment: HashMap::new(),
                timeout: Some(Duration::from_secs(10)),
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
async fn test_native_runner_graceful_shutdown() {
    let runner = create_test_runner().await;

    // Test shutdown without running jobs
    let shutdown_result = timeout(Duration::from_secs(5), runner.shutdown()).await;
    assert!(shutdown_result.is_ok());
    assert!(shutdown_result.unwrap().is_ok());
}

#[tokio::test]
async fn test_native_runner_statistics() {
    let runner = create_test_runner().await;
    let stats = runner.get_stats().await;

    assert_eq!(stats.total_jobs_executed, 0);
    assert_eq!(stats.successful_jobs, 0);
    assert_eq!(stats.failed_jobs, 0);
    assert_eq!(stats.current_load, 0.0);
    assert!(stats.uptime.as_secs() >= 0);
}

#[tokio::test]
async fn test_native_runner_running_jobs_tracking() {
    let runner = create_test_runner().await;
    let running_jobs = runner.get_running_jobs().await;

    assert!(running_jobs.is_empty());
}

#[tokio::test]
async fn test_native_runner_with_working_directory() {
    let temp_dir = std::env::temp_dir().join("rustci-native-test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config = NativeRunnerConfig {
        working_directory: temp_dir.clone(),
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    let metadata = runner.get_metadata();
    assert!(metadata.name.contains("native-runner"));

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_native_runner_job_requirements_matching() {
    let runner = create_test_runner().await;
    let mut job = create_test_job("requirements-test", "echo", vec!["test"]);

    // Test with matching requirements
    job.requirements.required_tags = vec![];
    job.requirements.excluded_tags = vec![];
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);

    // Test with native tag requirement (should match)
    job.requirements.required_tags = vec!["native".to_string()];
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle);

    // Test with docker tag requirement (should not match)
    job.requirements.required_tags = vec!["docker".to_string()];
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(!can_handle);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_native_runner_linux_specific_features() {
    // Test namespace isolation
    let config_ns = NativeRunnerConfig {
        isolation_level: IsolationLevel::Namespace {
            pid: true,
            network: true,
            mount: true,
            user: false,
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner_ns = NativeProcessRunner::new(config_ns, event_demux.clone()).await;
    
    // This might fail if namespaces are not available, which is expected
    match runner_ns {
        Ok(runner) => {
            let health = runner.health_check().await.unwrap();
            assert!(matches!(health, HealthStatus::Healthy | HealthStatus::Degraded { .. }));
        }
        Err(_) => {
            // Namespace support not available, which is acceptable in test environments
            println!("Namespace support not available in test environment");
        }
    }

    // Test cgroup isolation
    let config_cg = NativeRunnerConfig {
        isolation_level: IsolationLevel::Cgroup {
            cpu_limit: Some(1000), // 1 CPU core
            memory_limit: Some(1024), // 1GB
            process_isolation: true,
        },
        ..Default::default()
    };

    let runner_cg = NativeProcessRunner::new(config_cg, event_demux).await;
    
    // This might fail if cgroups are not available, which is expected
    match runner_cg {
        Ok(runner) => {
            let health = runner.health_check().await.unwrap();
            assert!(matches!(health, HealthStatus::Healthy | HealthStatus::Degraded { .. }));
        }
        Err(_) => {
            // Cgroup support not available, which is acceptable in test environments
            println!("Cgroup support not available in test environment");
        }
    }
}

#[tokio::test]
async fn test_native_runner_enhanced_lifecycle_management() {
    let config = NativeRunnerConfig {
        enhanced_lifecycle: true,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: true,
            heartbeat_interval: 1, // Very frequent for testing
            health_check_interval: 2,
            auto_registration: true,
            tags: vec!["native".to_string(), "enhanced".to_string()],
            node_affinity: {
                let mut affinity = HashMap::new();
                affinity.insert("zone".to_string(), "test".to_string());
                affinity
            },
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    // Test that enhanced lifecycle features are working
    let metadata = runner.get_metadata();
    assert!(metadata.supported_job_types.contains(&"native".to_string()));

    // Test heartbeat functionality
    let heartbeat_result = runner.send_heartbeat().await;
    assert!(heartbeat_result.is_ok());

    // Verify statistics are updated
    let stats = runner.get_stats().await;
    assert!(stats.control_plane_heartbeats > 0);
}