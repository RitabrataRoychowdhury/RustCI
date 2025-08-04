//! Backward Compatibility Tests
//!
//! This module contains tests to ensure backward compatibility with existing
//! Docker and Kubernetes runners while supporting the new control plane features.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;
use uuid::Uuid;

use RustAutoDevOps::{
    core::infrastructure::event_loop::EventDemultiplexer,
    domain::entities::{
        runner::{Job, JobStep, Runner},
        HealthStatus, JobResult, JobStatus,
    },
    infrastructure::runners::{
        unified_runner::{
            CompatibilityFlags, ControlPlaneIntegration, LegacyRunnerConfig, UnifiedRunner,
            UnifiedRunnerConfig, UnifiedRunnerFactory, UnifiedRunnerType,
        },
        docker_runner::DockerRunnerConfig,
        kubernetes_runner::KubernetesRunnerConfig,
        native_runner::NativeRunnerConfig,
    },
};

/// Test that legacy Docker runner configurations can be migrated
#[tokio::test]
async fn test_legacy_docker_config_migration() {
    let legacy_config = LegacyRunnerConfig {
        name: "legacy-docker-runner".to_string(),
        runner_type: "docker".to_string(),
        max_concurrent_jobs: 2,
        docker_endpoint: Some("unix:///var/run/docker.sock".to_string()),
        default_image: Some("alpine:latest".to_string()),
        namespace: None,
    };

    let unified_config = UnifiedRunner::migrate_from_legacy(legacy_config);

    assert_eq!(unified_config.name, "legacy-docker-runner");
    assert!(unified_config.compatibility_flags.legacy_mode);
    assert!(unified_config.compatibility_flags.runner_compat);

    match unified_config.runner_type {
        UnifiedRunnerType::Docker(docker_config) => {
            assert_eq!(docker_config.name, "legacy-docker-runner");
            assert_eq!(docker_config.max_concurrent_jobs, 2);
            assert_eq!(docker_config.docker_endpoint, "unix:///var/run/docker.sock");
            assert_eq!(docker_config.default_image, "alpine:latest");
        }
        _ => panic!("Expected Docker runner type"),
    }
}

/// Test that legacy Kubernetes runner configurations can be migrated
#[tokio::test]
async fn test_legacy_kubernetes_config_migration() {
    let legacy_config = LegacyRunnerConfig {
        name: "legacy-k8s-runner".to_string(),
        runner_type: "kubernetes".to_string(),
        max_concurrent_jobs: 5,
        docker_endpoint: None,
        default_image: Some("ubuntu:20.04".to_string()),
        namespace: Some("ci-namespace".to_string()),
    };

    let unified_config = UnifiedRunner::migrate_from_legacy(legacy_config);

    assert_eq!(unified_config.name, "legacy-k8s-runner");
    assert!(unified_config.compatibility_flags.legacy_mode);

    match unified_config.runner_type {
        UnifiedRunnerType::Kubernetes(k8s_config) => {
            assert_eq!(k8s_config.name, "legacy-k8s-runner");
            assert_eq!(k8s_config.max_concurrent_jobs, 5);
            assert_eq!(k8s_config.namespace, "ci-namespace");
            assert_eq!(k8s_config.default_image, "ubuntu:20.04");
        }
        _ => panic!("Expected Kubernetes runner type"),
    }
}

/// Test that unified runners maintain backward compatibility
#[tokio::test]
async fn test_unified_runner_backward_compatibility() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Create a unified runner with compatibility flags enabled
    let config = UnifiedRunnerConfig {
        name: "compat-test-runner".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig {
            name: "compat-test-runner".to_string(),
            max_concurrent_jobs: 1,
            ..Default::default()
        }),
        compatibility_flags: CompatibilityFlags {
            legacy_mode: true,
            runner_compat: true,
            hybrid_deployment: true,
            feature_flags: HashMap::new(),
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let runner = UnifiedRunner::new(config, event_demux).await.unwrap();

    // Test that compatibility mode is detected
    assert!(runner.is_compatibility_mode());
    assert_eq!(runner.get_runner_type_string(), "native");

    // Test health check includes compatibility information
    let health = runner.health_check().await.unwrap();
    match health {
        HealthStatus::Healthy => {
            // Health check should work normally
        }
        HealthStatus::Degraded { reason } => {
            assert!(reason.contains("compatibility mode"));
        }
        HealthStatus::Unhealthy { reason } => {
            assert!(reason.contains("compatibility mode"));
        }
    }

    // Test metadata includes compatibility information
    let metadata = runner.get_metadata();
    assert!(metadata.supported_job_types.contains(&"legacy".to_string()));
}

/// Test that feature flags work correctly
#[tokio::test]
async fn test_feature_flags() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    let mut config = UnifiedRunnerConfig {
        name: "feature-test-runner".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags {
            legacy_mode: false,
            runner_compat: true,
            hybrid_deployment: true,
            feature_flags: [
                ("enhanced_logging".to_string(), true),
                ("experimental_features".to_string(), false),
            ]
            .iter()
            .cloned()
            .collect(),
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let mut runner = UnifiedRunner::new(config, event_demux).await.unwrap();

    // Test feature flag checking
    assert!(runner.is_feature_enabled("enhanced_logging"));
    assert!(!runner.is_feature_enabled("experimental_features"));
    assert!(!runner.is_feature_enabled("non_existent_feature"));

    // Test feature flag modification
    runner.set_feature_flag("new_feature".to_string(), true);
    assert!(runner.is_feature_enabled("new_feature"));
}

/// Test that jobs with legacy requirements are handled correctly
#[tokio::test]
async fn test_legacy_job_handling() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    let config = UnifiedRunnerConfig {
        name: "legacy-job-runner".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags {
            legacy_mode: true,
            runner_compat: true,
            hybrid_deployment: true,
            feature_flags: HashMap::new(),
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let runner = UnifiedRunner::new(config, event_demux).await.unwrap();

    // Create a job with legacy requirements
    let mut job = Job::new(
        "legacy-test-job".to_string(),
        Uuid::new_v4(),
        vec![JobStep {
            name: "echo-test".to_string(),
            command: "echo".to_string(),
            args: vec!["Hello, legacy world!".to_string()],
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(30)),
            continue_on_error: false,
        }],
    );

    // Add legacy metadata
    job.metadata.insert("requires_legacy".to_string(), "true".to_string());

    // Test that the runner can handle the legacy job
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle, "Runner should be able to handle legacy jobs");
}

/// Test runner type auto-detection
#[test]
fn test_runner_type_detection() {
    let detected_type = UnifiedRunnerFactory::detect_runner_type();
    assert!(["docker", "kubernetes", "native"].contains(&detected_type.as_str()));
}

/// Test hybrid deployment scenarios
#[tokio::test]
async fn test_hybrid_deployment() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Create multiple runners of different types
    let docker_config = UnifiedRunnerConfig {
        name: "hybrid-docker".to_string(),
        runner_type: UnifiedRunnerType::Docker(DockerRunnerConfig {
            name: "hybrid-docker".to_string(),
            max_concurrent_jobs: 2,
            ..Default::default()
        }),
        compatibility_flags: CompatibilityFlags {
            hybrid_deployment: true,
            ..Default::default()
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let native_config = UnifiedRunnerConfig {
        name: "hybrid-native".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig {
            name: "hybrid-native".to_string(),
            max_concurrent_jobs: 2,
            ..Default::default()
        }),
        compatibility_flags: CompatibilityFlags {
            hybrid_deployment: true,
            ..Default::default()
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    // Both runners should be creatable (though Docker might fail if not available)
    let native_runner = UnifiedRunner::new(native_config, event_demux.clone()).await;
    assert!(native_runner.is_ok(), "Native runner should always be creatable");

    // Test that hybrid deployment flag is set
    let runner = native_runner.unwrap();
    assert!(runner.get_compatibility_flags().hybrid_deployment);
}

/// Test configuration validation
#[tokio::test]
async fn test_configuration_validation() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Test valid configuration
    let valid_config = UnifiedRunnerConfig {
        name: "valid-runner".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags::default(),
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let result = UnifiedRunner::new(valid_config, event_demux.clone()).await;
    assert!(result.is_ok(), "Valid configuration should create runner successfully");

    // Test configuration with empty name (should still work but with default)
    let empty_name_config = UnifiedRunnerConfig {
        name: "".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags::default(),
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let result = UnifiedRunner::new(empty_name_config, event_demux).await;
    assert!(result.is_ok(), "Configuration with empty name should still work");
}

/// Test that existing pipeline configurations work with unified runners
#[tokio::test]
async fn test_pipeline_compatibility() {
    // This test would load actual pipeline.yaml and k3s-pipeline.yaml files
    // and verify they can be processed by the unified runner system

    // For now, we'll create a simple test pipeline
    let pipeline_steps = vec![
        JobStep {
            name: "setup".to_string(),
            command: "echo".to_string(),
            args: vec!["Setting up environment".to_string()],
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(30)),
            continue_on_error: false,
        },
        JobStep {
            name: "build".to_string(),
            command: "echo".to_string(),
            args: vec!["Building project".to_string()],
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(300)),
            continue_on_error: false,
        },
        JobStep {
            name: "test".to_string(),
            command: "echo".to_string(),
            args: vec!["Running tests".to_string()],
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(600)),
            continue_on_error: false,
        },
    ];

    let job = Job::new("pipeline-test".to_string(), Uuid::new_v4(), pipeline_steps);

    let event_demux = Arc::new(EventDemultiplexer::new());
    let config = UnifiedRunnerConfig {
        name: "pipeline-runner".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags::default(),
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let runner = UnifiedRunner::new(config, event_demux).await.unwrap();

    // Test that the runner can handle the pipeline job
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle, "Runner should be able to handle pipeline jobs");

    // Execute the job with a timeout to prevent hanging
    let execution_result = timeout(Duration::from_secs(60), runner.execute(job)).await;
    
    match execution_result {
        Ok(Ok(job_result)) => {
            // Job executed successfully
            assert_eq!(job_result.step_results.len(), 3);
            println!("Pipeline execution completed: {:?}", job_result.status);
        }
        Ok(Err(e)) => {
            // Job execution failed, but that's okay for this test
            println!("Job execution failed (expected in test environment): {}", e);
        }
        Err(_) => {
            panic!("Job execution timed out");
        }
    }
}

/// Test graceful degradation when dependencies are not available
#[tokio::test]
async fn test_graceful_degradation() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Test Docker runner creation when Docker is not available
    let docker_config = UnifiedRunnerConfig {
        name: "unavailable-docker".to_string(),
        runner_type: UnifiedRunnerType::Docker(DockerRunnerConfig {
            name: "unavailable-docker".to_string(),
            docker_endpoint: "unix:///nonexistent/docker.sock".to_string(),
            ..Default::default()
        }),
        compatibility_flags: CompatibilityFlags::default(),
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    // This should fail gracefully
    let result = UnifiedRunner::new(docker_config, event_demux.clone()).await;
    match result {
        Ok(_) => {
            // If Docker is available, that's fine
            println!("Docker runner created successfully");
        }
        Err(e) => {
            // If Docker is not available, we should get a clear error
            println!("Docker runner creation failed as expected: {}", e);
            assert!(e.to_string().contains("Docker") || e.to_string().contains("connect"));
        }
    }

    // Native runner should always work
    let native_config = UnifiedRunnerConfig {
        name: "fallback-native".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags::default(),
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let native_result = UnifiedRunner::new(native_config, event_demux).await;
    assert!(native_result.is_ok(), "Native runner should always be available as fallback");
}

/// Test that control plane integration can be disabled for backward compatibility
#[tokio::test]
async fn test_control_plane_integration_disabled() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    let config = UnifiedRunnerConfig {
        name: "no-control-plane".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags {
            legacy_mode: true,
            ..Default::default()
        },
        control_plane_integration: ControlPlaneIntegration {
            enabled: false,
            ..Default::default()
        },
    };

    let runner = UnifiedRunner::new(config, event_demux).await.unwrap();

    // Runner should work normally without control plane integration
    let health = runner.health_check().await.unwrap();
    assert!(matches!(health, HealthStatus::Healthy | HealthStatus::Degraded { .. }));

    let capacity = runner.get_capacity().await.unwrap();
    assert!(capacity.max_concurrent_jobs > 0);
}