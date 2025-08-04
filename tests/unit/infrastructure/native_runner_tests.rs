//! Unit tests for Native Process Runner
//!
//! This module contains unit tests for the native process runner components,
//! focusing on individual functions and isolated behavior.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rustci::core::event_loop::EventDemultiplexer;
use rustci::infrastructure::runners::native_runner::{
    ControlPlaneCoordinationConfig, IsolationLevel, NativeProcessRunner, NativeRunnerConfig,
    ResourceLimits,
};

#[tokio::test]
async fn test_native_runner_config_default() {
    let config = NativeRunnerConfig::default();

    assert_eq!(config.name, "native-runner");
    assert_eq!(config.max_concurrent_jobs, 4);
    assert_eq!(config.default_timeout_seconds, 3600);
    assert_eq!(config.isolation_level, IsolationLevel::ProcessGroup);
    assert!(config.enhanced_lifecycle);
    assert!(config.control_plane_coordination.enabled);
    assert_eq!(config.max_output_buffer_size, 1024 * 1024);
}

#[tokio::test]
async fn test_isolation_level_variants() {
    // Test None isolation
    let isolation_none = IsolationLevel::None;
    assert_eq!(isolation_none, IsolationLevel::None);

    // Test ProcessGroup isolation
    let isolation_pg = IsolationLevel::ProcessGroup;
    assert_eq!(isolation_pg, IsolationLevel::ProcessGroup);

    #[cfg(target_os = "linux")]
    {
        // Test Namespace isolation
        let isolation_ns = IsolationLevel::Namespace {
            pid: true,
            network: false,
            mount: true,
            user: false,
        };
        if let IsolationLevel::Namespace { pid, network, mount, user } = isolation_ns {
            assert!(pid);
            assert!(!network);
            assert!(mount);
            assert!(!user);
        }

        // Test Cgroup isolation
        let isolation_cg = IsolationLevel::Cgroup {
            cpu_limit: Some(500),
            memory_limit: Some(1024),
            process_isolation: true,
        };
        if let IsolationLevel::Cgroup { cpu_limit, memory_limit, process_isolation } = isolation_cg {
            assert_eq!(cpu_limit, Some(500));
            assert_eq!(memory_limit, Some(1024));
            assert!(process_isolation);
        }
    }
}

#[tokio::test]
async fn test_resource_limits_configuration() {
    let limits = ResourceLimits {
        max_cpu: Some(2.0),
        max_memory: Some(2048),
        max_execution_time: Some(Duration::from_secs(7200)),
        max_processes: Some(200),
        max_file_descriptors: Some(2048),
    };

    assert_eq!(limits.max_cpu, Some(2.0));
    assert_eq!(limits.max_memory, Some(2048));
    assert_eq!(limits.max_execution_time, Some(Duration::from_secs(7200)));
    assert_eq!(limits.max_processes, Some(200));
    assert_eq!(limits.max_file_descriptors, Some(2048));
}

#[tokio::test]
async fn test_resource_limits_default() {
    let limits = ResourceLimits::default();

    assert_eq!(limits.max_cpu, Some(1.0));
    assert_eq!(limits.max_memory, Some(1024));
    assert_eq!(limits.max_execution_time, Some(Duration::from_secs(3600)));
    assert_eq!(limits.max_processes, Some(100));
    assert_eq!(limits.max_file_descriptors, Some(1024));
}

#[tokio::test]
async fn test_control_plane_coordination_config() {
    let config = ControlPlaneCoordinationConfig {
        enabled: true,
        heartbeat_interval: 15,
        health_check_interval: 30,
        auto_registration: false,
        tags: vec!["custom".to_string(), "test".to_string()],
        node_affinity: {
            let mut affinity = HashMap::new();
            affinity.insert("region".to_string(), "us-west".to_string());
            affinity.insert("zone".to_string(), "a".to_string());
            affinity
        },
    };

    assert!(config.enabled);
    assert_eq!(config.heartbeat_interval, 15);
    assert_eq!(config.health_check_interval, 30);
    assert!(!config.auto_registration);
    assert_eq!(config.tags.len(), 2);
    assert!(config.tags.contains(&"custom".to_string()));
    assert!(config.tags.contains(&"test".to_string()));
    assert_eq!(config.node_affinity.len(), 2);
    assert_eq!(config.node_affinity.get("region"), Some(&"us-west".to_string()));
    assert_eq!(config.node_affinity.get("zone"), Some(&"a".to_string()));
}

#[tokio::test]
async fn test_control_plane_coordination_config_default() {
    let config = ControlPlaneCoordinationConfig::default();

    assert!(config.enabled);
    assert_eq!(config.heartbeat_interval, 30);
    assert_eq!(config.health_check_interval, 60);
    assert!(config.auto_registration);
    assert_eq!(config.tags.len(), 1);
    assert!(config.tags.contains(&"native".to_string()));
    assert!(config.node_affinity.is_empty());
}

#[tokio::test]
async fn test_native_runner_config_customization() {
    let temp_dir = std::env::temp_dir().join("rustci-test");
    let mut env_vars = HashMap::new();
    env_vars.insert("CUSTOM_VAR".to_string(), "custom_value".to_string());

    let config = NativeRunnerConfig {
        name: "custom-native-runner".to_string(),
        max_concurrent_jobs: 8,
        working_directory: temp_dir.clone(),
        environment: env_vars.clone(),
        default_timeout_seconds: 1800,
        isolation_level: IsolationLevel::None,
        resource_limits: ResourceLimits {
            max_cpu: Some(4.0),
            max_memory: Some(4096),
            max_execution_time: Some(Duration::from_secs(1800)),
            max_processes: Some(500),
            max_file_descriptors: Some(4096),
        },
        enable_output_streaming: false,
        max_output_buffer_size: 2 * 1024 * 1024, // 2MB
        shell: "zsh".to_string(),
        enhanced_lifecycle: false,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false,
            ..Default::default()
        },
    };

    assert_eq!(config.name, "custom-native-runner");
    assert_eq!(config.max_concurrent_jobs, 8);
    assert_eq!(config.working_directory, temp_dir);
    assert_eq!(config.environment.len(), 1);
    assert_eq!(config.environment.get("CUSTOM_VAR"), Some(&"custom_value".to_string()));
    assert_eq!(config.default_timeout_seconds, 1800);
    assert_eq!(config.isolation_level, IsolationLevel::None);
    assert_eq!(config.resource_limits.max_cpu, Some(4.0));
    assert!(!config.enable_output_streaming);
    assert_eq!(config.max_output_buffer_size, 2 * 1024 * 1024);
    assert_eq!(config.shell, "zsh");
    assert!(!config.enhanced_lifecycle);
    assert!(!config.control_plane_coordination.enabled);
}

#[tokio::test]
async fn test_native_runner_creation_with_custom_config() {
    let config = NativeRunnerConfig {
        name: "test-runner".to_string(),
        max_concurrent_jobs: 1,
        enhanced_lifecycle: false,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    let metadata = runner.get_metadata();
    assert_eq!(metadata.name, "test-runner");
    assert!(metadata.supported_job_types.contains(&"native".to_string()));
}

#[tokio::test]
async fn test_native_runner_stats_initialization() {
    let config = NativeRunnerConfig {
        enhanced_lifecycle: false,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    let stats = runner.get_stats().await;
    assert_eq!(stats.total_jobs_executed, 0);
    assert_eq!(stats.successful_jobs, 0);
    assert_eq!(stats.failed_jobs, 0);
    assert_eq!(stats.current_load, 0.0);
    assert_eq!(stats.isolation_failures, 0);
    assert_eq!(stats.resource_limit_violations, 0);
    assert_eq!(stats.control_plane_heartbeats, 0);
    assert!(stats.last_job_completed.is_none());
}

#[tokio::test]
async fn test_isolation_level_default_by_platform() {
    let default_isolation = IsolationLevel::default();

    // On all platforms, default should be ProcessGroup
    assert_eq!(default_isolation, IsolationLevel::ProcessGroup);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_linux_specific_isolation_levels() {
    // Test that Linux-specific isolation levels can be created
    let namespace_isolation = IsolationLevel::Namespace {
        pid: true,
        network: true,
        mount: false,
        user: true,
    };

    match namespace_isolation {
        IsolationLevel::Namespace { pid, network, mount, user } => {
            assert!(pid);
            assert!(network);
            assert!(!mount);
            assert!(user);
        }
        _ => panic!("Expected namespace isolation"),
    }

    let cgroup_isolation = IsolationLevel::Cgroup {
        cpu_limit: Some(2000), // 2 CPU cores
        memory_limit: Some(4096), // 4GB
        process_isolation: false,
    };

    match cgroup_isolation {
        IsolationLevel::Cgroup { cpu_limit, memory_limit, process_isolation } => {
            assert_eq!(cpu_limit, Some(2000));
            assert_eq!(memory_limit, Some(4096));
            assert!(!process_isolation);
        }
        _ => panic!("Expected cgroup isolation"),
    }
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn test_non_linux_isolation_levels() {
    // On non-Linux platforms, only None and ProcessGroup should be available
    let none_isolation = IsolationLevel::None;
    let pg_isolation = IsolationLevel::ProcessGroup;

    assert_eq!(none_isolation, IsolationLevel::None);
    assert_eq!(pg_isolation, IsolationLevel::ProcessGroup);
}

#[tokio::test]
async fn test_native_runner_working_directory_creation() {
    let temp_dir = std::env::temp_dir().join("rustci-native-test-wd");
    
    // Ensure directory doesn't exist
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    let config = NativeRunnerConfig {
        working_directory: temp_dir.clone(),
        enhanced_lifecycle: false,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    // Verify directory was created
    assert!(temp_dir.exists());
    assert!(temp_dir.is_dir());

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test]
async fn test_native_runner_shell_configuration() {
    let config = NativeRunnerConfig {
        shell: "custom-shell".to_string(),
        enhanced_lifecycle: false,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    // The shell configuration is internal, but we can verify the runner was created successfully
    let metadata = runner.get_metadata();
    assert!(metadata.name.contains("native-runner"));
}

#[tokio::test]
async fn test_native_runner_environment_variables() {
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR1".to_string(), "value1".to_string());
    env_vars.insert("TEST_VAR2".to_string(), "value2".to_string());

    let config = NativeRunnerConfig {
        environment: env_vars.clone(),
        enhanced_lifecycle: false,
        control_plane_coordination: ControlPlaneCoordinationConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let event_demux = Arc::new(EventDemultiplexer::new());
    let runner = NativeProcessRunner::new(config, event_demux).await.unwrap();

    // Environment variables are internal, but we can verify the runner was created successfully
    let capacity = runner.get_capacity().await.unwrap();
    assert_eq!(capacity.max_concurrent_jobs, 4); // Default value
}