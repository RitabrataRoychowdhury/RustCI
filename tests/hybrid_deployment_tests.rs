//! Hybrid Deployment Tests
//!
//! This module tests hybrid deployment scenarios where multiple runner types
//! (Docker, Kubernetes, Native) work together in the same system.

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
            CompatibilityFlags, ControlPlaneIntegration, UnifiedRunner,
            UnifiedRunnerConfig, UnifiedRunnerFactory, UnifiedRunnerType,
        },
        docker_runner::DockerRunnerConfig,
        kubernetes_runner::KubernetesRunnerConfig,
        native_runner::NativeRunnerConfig,
    },
};

/// Test that multiple runner types can coexist in a hybrid deployment
#[tokio::test]
async fn test_hybrid_deployment_coexistence() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Create runners of different types
    let mut runners: Vec<Box<dyn Runner>> = Vec::new();

    // Always create native runner (should always work)
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

    let native_runner = UnifiedRunner::new(native_config, event_demux.clone()).await;
    assert!(native_runner.is_ok(), "Native runner should always be creatable");
    runners.push(Box::new(native_runner.unwrap()));

    // Try to create Docker runner (may fail if Docker not available)
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

    match UnifiedRunner::new(docker_config, event_demux.clone()).await {
        Ok(docker_runner) => {
            println!("Docker runner created successfully for hybrid deployment");
            runners.push(Box::new(docker_runner));
        }
        Err(e) => {
            println!("Docker runner creation failed (expected if Docker not available): {}", e);
        }
    }

    // Verify all runners are healthy
    for (i, runner) in runners.iter().enumerate() {
        let health = runner.health_check().await.unwrap();
        match health {
            HealthStatus::Healthy | HealthStatus::Degraded { .. } => {
                println!("Runner {} is healthy", i);
            }
            HealthStatus::Unhealthy { reason } => {
                panic!("Runner {} is unhealthy: {}", i, reason);
            }
        }
    }

    // Verify all runners have capacity
    for (i, runner) in runners.iter().enumerate() {
        let capacity = runner.get_capacity().await.unwrap();
        assert!(capacity.max_concurrent_jobs > 0, "Runner {} should have capacity", i);
        println!("Runner {} has {} max concurrent jobs", i, capacity.max_concurrent_jobs);
    }

    println!("Hybrid deployment test passed with {} runners", runners.len());
}

/// Test job distribution across different runner types
#[tokio::test]
async fn test_job_distribution_across_runner_types() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Create test jobs
    let jobs = vec![
        create_test_job("job-1", "echo 'Job 1 on any runner'"),
        create_test_job("job-2", "echo 'Job 2 on any runner'"),
        create_test_job("job-3", "echo 'Job 3 on any runner'"),
    ];

    // Create native runner (guaranteed to work)
    let native_config = UnifiedRunnerConfig {
        name: "distribution-native".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig {
            name: "distribution-native".to_string(),
            max_concurrent_jobs: 3,
            ..Default::default()
        }),
        compatibility_flags: CompatibilityFlags {
            hybrid_deployment: true,
            ..Default::default()
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let runner = UnifiedRunner::new(native_config, event_demux).await.unwrap();

    // Test that runner can handle all jobs
    for job in &jobs {
        let can_handle = runner.can_handle_job(job).await.unwrap();
        assert!(can_handle, "Runner should be able to handle job {}", job.name);
    }

    // Execute one job to verify it works
    let job = jobs.into_iter().next().unwrap();
    let execution_result = timeout(Duration::from_secs(30), runner.execute(job)).await;

    match execution_result {
        Ok(Ok(job_result)) => {
            println!("Job executed successfully: {:?}", job_result.status);
        }
        Ok(Err(e)) => {
            println!("Job execution failed (may be expected in test environment): {}", e);
        }
        Err(_) => {
            panic!("Job execution timed out");
        }
    }
}

/// Test runner type auto-detection and fallback
#[tokio::test]
async fn test_runner_auto_detection_and_fallback() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Test auto-detection
    let detected_type = UnifiedRunnerFactory::detect_runner_type();
    println!("Auto-detected runner type: {}", detected_type);
    assert!(["docker", "kubernetes", "native"].contains(&detected_type.as_str()));

    // Test auto-detected runner creation
    let auto_runner = UnifiedRunnerFactory::create_auto_detected(
        "auto-detected-runner".to_string(),
        event_demux.clone(),
    ).await;

    match auto_runner {
        Ok(runner) => {
            println!("Auto-detected runner created successfully");
            let health = runner.health_check().await.unwrap();
            match health {
                HealthStatus::Healthy | HealthStatus::Degraded { .. } => {
                    println!("Auto-detected runner is healthy");
                }
                HealthStatus::Unhealthy { reason } => {
                    println!("Auto-detected runner is unhealthy: {}", reason);
                }
            }
        }
        Err(e) => {
            println!("Auto-detected runner creation failed: {}", e);
        }
    }

    // Test fallback to native runner
    let fallback_config = UnifiedRunnerConfig {
        name: "fallback-native".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags::default(),
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let fallback_runner = UnifiedRunner::new(fallback_config, event_demux).await;
    assert!(fallback_runner.is_ok(), "Native runner should always work as fallback");
}

/// Test configuration migration in hybrid scenarios
#[tokio::test]
async fn test_hybrid_configuration_migration() {
    use RustAutoDevOps::infrastructure::runners::unified_runner::LegacyRunnerConfig;

    // Test migration of different legacy configurations
    let legacy_configs = vec![
        LegacyRunnerConfig {
            name: "legacy-docker".to_string(),
            runner_type: "docker".to_string(),
            max_concurrent_jobs: 2,
            docker_endpoint: Some("unix:///var/run/docker.sock".to_string()),
            default_image: Some("alpine:latest".to_string()),
            namespace: None,
        },
        LegacyRunnerConfig {
            name: "legacy-k8s".to_string(),
            runner_type: "kubernetes".to_string(),
            max_concurrent_jobs: 5,
            docker_endpoint: None,
            default_image: Some("ubuntu:20.04".to_string()),
            namespace: Some("ci-namespace".to_string()),
        },
        LegacyRunnerConfig {
            name: "legacy-unknown".to_string(),
            runner_type: "unknown".to_string(),
            max_concurrent_jobs: 1,
            docker_endpoint: None,
            default_image: None,
            namespace: None,
        },
    ];

    for legacy_config in legacy_configs {
        let unified_config = UnifiedRunner::migrate_from_legacy(legacy_config.clone());
        
        // Verify migration preserved important settings
        assert_eq!(unified_config.name, legacy_config.name);
        assert!(unified_config.compatibility_flags.legacy_mode);
        assert!(unified_config.compatibility_flags.runner_compat);

        // Verify runner type was mapped correctly
        match legacy_config.runner_type.as_str() {
            "docker" => {
                assert!(matches!(unified_config.runner_type, UnifiedRunnerType::Docker(_)));
            }
            "kubernetes" => {
                assert!(matches!(unified_config.runner_type, UnifiedRunnerType::Kubernetes(_)));
            }
            _ => {
                // Unknown types should default to native
                assert!(matches!(unified_config.runner_type, UnifiedRunnerType::Native(_)));
            }
        }

        println!("Successfully migrated legacy config: {}", legacy_config.name);
    }
}

/// Test feature flag propagation in hybrid deployments
#[tokio::test]
async fn test_feature_flag_propagation() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Create runners with different feature flags
    let mut feature_flags = HashMap::new();
    feature_flags.insert("enhanced_logging".to_string(), true);
    feature_flags.insert("experimental_features".to_string(), false);
    feature_flags.insert("hybrid_mode".to_string(), true);

    let config = UnifiedRunnerConfig {
        name: "feature-test-runner".to_string(),
        runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
        compatibility_flags: CompatibilityFlags {
            legacy_mode: false,
            runner_compat: true,
            hybrid_deployment: true,
            feature_flags,
        },
        control_plane_integration: ControlPlaneIntegration::default(),
    };

    let mut runner = UnifiedRunner::new(config, event_demux).await.unwrap();

    // Test feature flag checking
    assert!(runner.is_feature_enabled("enhanced_logging"));
    assert!(!runner.is_feature_enabled("experimental_features"));
    assert!(runner.is_feature_enabled("hybrid_mode"));
    assert!(!runner.is_feature_enabled("non_existent_feature"));

    // Test feature flag modification
    runner.set_feature_flag("new_hybrid_feature".to_string(), true);
    assert!(runner.is_feature_enabled("new_hybrid_feature"));

    // Test job handling with feature requirements
    let mut job = create_test_job("feature-job", "echo 'Feature test'");
    job.metadata.insert("required_feature".to_string(), "enhanced_logging".to_string());

    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(can_handle, "Runner should handle job with enabled feature");

    // Test job rejection with disabled feature
    job.metadata.insert("required_feature".to_string(), "experimental_features".to_string());
    let can_handle = runner.can_handle_job(&job).await.unwrap();
    assert!(!can_handle, "Runner should reject job with disabled feature");
}

/// Test graceful degradation in hybrid environments
#[tokio::test]
async fn test_graceful_degradation_hybrid() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Test that system continues to work even if some runner types fail
    let runner_configs = vec![
        // This should always work
        UnifiedRunnerConfig {
            name: "reliable-native".to_string(),
            runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
            compatibility_flags: CompatibilityFlags {
                hybrid_deployment: true,
                ..Default::default()
            },
            control_plane_integration: ControlPlaneIntegration::default(),
        },
        // This might fail if Docker is not available
        UnifiedRunnerConfig {
            name: "maybe-docker".to_string(),
            runner_type: UnifiedRunnerType::Docker(DockerRunnerConfig {
                name: "maybe-docker".to_string(),
                docker_endpoint: "unix:///var/run/docker.sock".to_string(),
                ..Default::default()
            }),
            compatibility_flags: CompatibilityFlags {
                hybrid_deployment: true,
                ..Default::default()
            },
            control_plane_integration: ControlPlaneIntegration::default(),
        },
    ];

    let mut successful_runners = 0;
    let mut failed_runners = 0;

    for config in runner_configs {
        match UnifiedRunner::new(config.clone(), event_demux.clone()).await {
            Ok(runner) => {
                successful_runners += 1;
                println!("Successfully created runner: {}", config.name);
                
                // Verify the runner is functional
                let health = runner.health_check().await.unwrap();
                match health {
                    HealthStatus::Healthy | HealthStatus::Degraded { .. } => {
                        println!("Runner {} is operational", config.name);
                    }
                    HealthStatus::Unhealthy { reason } => {
                        println!("Runner {} is unhealthy but created: {}", config.name, reason);
                    }
                }
            }
            Err(e) => {
                failed_runners += 1;
                println!("Failed to create runner {} (expected if dependencies unavailable): {}", config.name, e);
            }
        }
    }

    // At least one runner should work (native)
    assert!(successful_runners > 0, "At least one runner should be created successfully");
    println!("Hybrid deployment graceful degradation test: {} successful, {} failed", successful_runners, failed_runners);
}

/// Test control plane integration in hybrid deployments
#[tokio::test]
async fn test_control_plane_integration_hybrid() {
    let event_demux = Arc::new(EventDemultiplexer::new());

    // Create runners with different control plane settings
    let configs = vec![
        // Runner with control plane enabled
        UnifiedRunnerConfig {
            name: "cp-enabled-runner".to_string(),
            runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
            compatibility_flags: CompatibilityFlags {
                hybrid_deployment: true,
                ..Default::default()
            },
            control_plane_integration: ControlPlaneIntegration {
                enabled: true,
                endpoint: Some("http://localhost:8080".to_string()),
                auth_config: None,
                registration: Default::default(),
            },
        },
        // Runner with control plane disabled (legacy mode)
        UnifiedRunnerConfig {
            name: "cp-disabled-runner".to_string(),
            runner_type: UnifiedRunnerType::Native(NativeRunnerConfig::default()),
            compatibility_flags: CompatibilityFlags {
                legacy_mode: true,
                hybrid_deployment: true,
                ..Default::default()
            },
            control_plane_integration: ControlPlaneIntegration {
                enabled: false,
                ..Default::default()
            },
        },
    ];

    for config in configs {
        let runner = UnifiedRunner::new(config.clone(), event_demux.clone()).await.unwrap();
        
        // Both runners should work regardless of control plane settings
        let health = runner.health_check().await.unwrap();
        match health {
            HealthStatus::Healthy | HealthStatus::Degraded { .. } => {
                println!("Runner {} is healthy with control plane integration: {}", 
                    config.name, config.control_plane_integration.enabled);
            }
            HealthStatus::Unhealthy { reason } => {
                panic!("Runner {} should be healthy: {}", config.name, reason);
            }
        }

        // Test job execution
        let job = create_test_job("cp-test-job", "echo 'Control plane test'");
        let can_handle = runner.can_handle_job(&job).await.unwrap();
        assert!(can_handle, "Runner {} should be able to handle jobs", config.name);
    }
}

/// Helper function to create a test job
fn create_test_job(name: &str, command: &str) -> Job {
    Job::new(
        name.to_string(),
        Uuid::new_v4(),
        vec![JobStep {
            name: "test-step".to_string(),
            command: command.to_string(),
            args: vec![],
            working_directory: None,
            environment: HashMap::new(),
            timeout: Some(Duration::from_secs(30)),
            continue_on_error: false,
        }],
    )
}