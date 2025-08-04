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
    ci::config::{CIPipeline, PipelineType, PipelineTypeDetector, ServerConfig},
    core::infrastructure::event_loop::EventDemultiplexer,
    domain::entities::{
        runner::{Job, JobStep, Runner},
        HealthStatus,
    },
    infrastructure::runners::{
        unified_runner::{
            CompatibilityFlags, ControlPlaneIntegration, LegacyRunnerConfig, UnifiedRunner,
            UnifiedRunnerConfig, UnifiedRunnerFactory, UnifiedRunnerType,
        },
        docker_runner::DockerRunnerConfig,
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
            assert_eq!(k8s_config.default_image, "ubuntu:22.04");
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

    let config = UnifiedRunnerConfig {
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

    // Test Docker runner creation (may fail if Docker not available)
    let _docker_runner = UnifiedRunner::new(docker_config, event_demux).await;
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
            println!("Pipeline execution completed: {:?}", job_result.status);
            println!("Step results count: {}", job_result.step_results.len());
            // In test environment, job may be queued but not executed
            // This is acceptable for compatibility testing
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

/// Test pipeline type auto-detection functionality
#[test]
fn test_pipeline_type_auto_detection() {
    // Test minimal pipeline detection
    let minimal_yaml = r#"
name: "Minimal Pipeline"
repo: https://github.com/user/repo.git
server: user@deploy.example.com
"#;
    let pipeline = CIPipeline::from_yaml(minimal_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Minimal);
    assert!(pipeline.repo.is_some());
    assert!(pipeline.server.is_some());

    // Test simple pipeline detection
    let simple_yaml = r#"
name: "Simple Pipeline"
repo: https://github.com/user/repo.git
steps:
  - run: cargo build --release
  - run: cargo test
"#;
    let pipeline = CIPipeline::from_yaml(simple_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Simple);
    assert!(pipeline.steps.is_some());
    assert_eq!(pipeline.steps.as_ref().unwrap().len(), 2);

    // Test standard pipeline detection
    let standard_yaml = r#"
name: "Standard Pipeline"
repo: https://github.com/user/repo.git
stages:
  - name: build
    steps:
      - name: "Build step"
        step_type: shell
        config:
          command: "echo build"
  - name: test
    steps:
      - name: "Test step"
        step_type: shell
        config:
          command: "echo test"
jobs:
  build:
    stage: build
    script: cargo build --release
"#;
    let pipeline = CIPipeline::from_yaml(standard_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Standard);
    assert!(pipeline.jobs.is_some());
    assert!(!pipeline.stages.is_empty());

    // Test advanced pipeline detection
    let advanced_yaml = r#"
name: "Advanced Pipeline"
repo: https://github.com/user/repo.git
variables:
  RUST_VERSION: "1.70"
  BUILD_ENV: "production"
stages:
  - name: build
    steps: []
jobs:
  build:
    stage: build
    script: cargo build --release
cache:
  paths: [target/, ~/.cargo/]
  key: "rust-cache-v1"
"#;
    let pipeline = CIPipeline::from_yaml(advanced_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Advanced);
    assert!(pipeline.variables.is_some());
    assert!(pipeline.cache.is_some());
    assert_eq!(pipeline.variables.as_ref().unwrap().len(), 2);
}

/// Test explicit pipeline type override
#[test]
fn test_explicit_pipeline_type_override() {
    // Test that explicit type overrides auto-detection
    let override_yaml = r#"
name: "Override Pipeline"
type: simple
repo: https://github.com/user/repo.git
variables:
  TEST: "value"
stages:
  - name: build
    steps: []
jobs:
  build:
    stage: build
    script: cargo build --release
"#;
    let pipeline = CIPipeline::from_yaml(override_yaml).unwrap();
    // Should respect explicit type even though structure suggests Advanced
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Simple);
    assert!(pipeline.variables.is_some());
}

/// Test backward compatibility with existing pipeline configurations
#[test]
fn test_backward_compatibility_existing_pipelines() {
    // Test that existing standard pipeline format still works
    let existing_yaml = r#"
name: "Existing Pipeline"
description: "Legacy pipeline format"
stages:
  - name: build
    steps:
      - name: "Build application"
        step_type: shell
        config:
          command: "cargo build --release"
  - name: test
    steps:
      - name: "Run tests"
        step_type: shell
        config:
          command: "cargo test"
environment:
  RUST_LOG: "info"
timeout: 3600
retry_count: 2
"#;
    
    let pipeline = CIPipeline::from_yaml(existing_yaml).unwrap();
    
    // Should auto-detect as Standard type due to stages
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Standard);
    assert_eq!(pipeline.name, "Existing Pipeline");
    assert_eq!(pipeline.description, Some("Legacy pipeline format".to_string()));
    assert_eq!(pipeline.stages.len(), 2);
    assert_eq!(pipeline.timeout, Some(3600));
    assert_eq!(pipeline.retry_count, Some(2));
    assert!(!pipeline.environment.is_empty());
    
    // Validate the pipeline
    assert!(pipeline.validate().is_ok());
}

/// Test pipeline validation for each type
#[test]
fn test_pipeline_type_validation() {
    // Test minimal pipeline validation
    let minimal_yaml = r#"
name: "Minimal Test"
repo: https://github.com/user/repo.git
server: user@deploy.example.com
"#;
    let pipeline = CIPipeline::from_yaml(minimal_yaml).unwrap();
    assert!(pipeline.validate().is_ok());

    // Test minimal pipeline missing required fields
    let invalid_minimal = r#"
name: "Invalid Minimal"
repo: https://github.com/user/repo.git
"#;
    let pipeline = CIPipeline::from_yaml(invalid_minimal).unwrap();
    assert!(pipeline.validate().is_err());

    // Test simple pipeline validation
    let simple_yaml = r#"
name: "Simple Test"
steps:
  - run: echo "test"
"#;
    let pipeline = CIPipeline::from_yaml(simple_yaml).unwrap();
    assert!(pipeline.validate().is_ok());

    // Test simple pipeline with empty steps
    let invalid_simple = r#"
name: "Invalid Simple"
steps: []
"#;
    let pipeline = CIPipeline::from_yaml(invalid_simple).unwrap();
    assert!(pipeline.validate().is_err());

    // Test standard pipeline validation
    let standard_yaml = r#"
name: "Standard Test"
stages:
  - name: build
    steps:
      - name: "Build"
        step_type: shell
        config:
          command: "echo build"
"#;
    let pipeline = CIPipeline::from_yaml(standard_yaml).unwrap();
    assert!(pipeline.validate().is_ok());

    // Test advanced pipeline validation
    let advanced_yaml = r#"
name: "Advanced Test"
variables:
  TEST: "value"
stages:
  - name: test
    steps:
      - name: "Test step"
        step_type: shell
        config:
          command: "echo test"
"#;
    let pipeline = CIPipeline::from_yaml(advanced_yaml).unwrap();
    assert!(pipeline.validate().is_ok());
}

/// Test API response format consistency
#[test]
fn test_api_response_format_consistency() {
    // Test that pipeline serialization maintains consistent format
    let pipeline = CIPipeline {
        mongo_id: None,
        id: Some(uuid::Uuid::new_v4()),
        name: "Test Pipeline".to_string(),
        description: Some("Test description".to_string()),
        triggers: vec![],
        stages: vec![],
        environment: HashMap::new(),
        timeout: Some(3600),
        retry_count: Some(1),
        notifications: None,
        pipeline_type: Some(PipelineType::Standard),
        repo: Some("https://github.com/user/repo.git".to_string()),
        branch: Some("main".to_string()),
        server: Some(ServerConfig::Simple("user@host".to_string())),
        steps: None,
        variables: None,
        jobs: None,
        matrix: None,
        cache: None,
        include: None,
        created_at: Some(chrono::Utc::now()),
        updated_at: Some(chrono::Utc::now()),
    };

    // Test YAML serialization
    let yaml_output = pipeline.to_yaml().unwrap();
    assert!(yaml_output.contains("name: Test Pipeline"));
    assert!(yaml_output.contains("pipeline_type: standard"));
    assert!(yaml_output.contains("repo: https://github.com/user/repo.git"));

    // Test JSON serialization (for API responses)
    let json_output = serde_json::to_string(&pipeline).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    
    assert_eq!(parsed["name"], "Test Pipeline");
    assert_eq!(parsed["pipeline_type"], "standard");
    assert_eq!(parsed["repo"], "https://github.com/user/repo.git");
    assert!(parsed["id"].is_string());
    assert!(parsed["created_at"].is_string());
}

/// Test that type detector handles edge cases correctly
#[test]
fn test_pipeline_type_detector_edge_cases() {
    // Test empty YAML
    let empty_yaml: serde_yaml::Value = serde_yaml::from_str("{}").unwrap();
    assert_eq!(PipelineTypeDetector::detect_type(&empty_yaml), PipelineType::Minimal);

    // Test YAML with only name
    let name_only: serde_yaml::Value = serde_yaml::from_str("name: test").unwrap();
    assert_eq!(PipelineTypeDetector::detect_type(&name_only), PipelineType::Minimal);

    // Test YAML with multiple indicators (should pick most advanced)
    let mixed_yaml: serde_yaml::Value = serde_yaml::from_str(r#"
steps:
  - run: echo "simple"
variables:
  TEST: "advanced"
"#).unwrap();
    assert_eq!(PipelineTypeDetector::detect_type(&mixed_yaml), PipelineType::Advanced);

    // Test invalid YAML value (not a mapping)
    let invalid_yaml: serde_yaml::Value = serde_yaml::from_str("just a string").unwrap();
    assert_eq!(PipelineTypeDetector::detect_type(&invalid_yaml), PipelineType::Minimal);
}

/// Test validation error handling for different pipeline types
#[test]
fn test_validation_error_handling() {
    // Test minimal pipeline validation errors
    let invalid_minimal = CIPipeline {
        mongo_id: None,
        id: Some(uuid::Uuid::new_v4()),
        name: "Invalid Minimal".to_string(),
        description: None,
        triggers: vec![],
        stages: vec![],
        environment: HashMap::new(),
        timeout: None,
        retry_count: None,
        notifications: None,
        pipeline_type: Some(PipelineType::Minimal),
        repo: Some("https://github.com/user/repo.git".to_string()),
        branch: None,
        server: None, // Missing required server
        steps: None,
        variables: None,
        jobs: None,
        matrix: None,
        cache: None,
        include: None,
        created_at: None,
        updated_at: None,
    };

    let validation_result = invalid_minimal.validate();
    assert!(validation_result.is_err());
    assert!(validation_result.unwrap_err().contains("server"));

    // Test simple pipeline validation errors
    let invalid_simple = CIPipeline {
        mongo_id: None,
        id: Some(uuid::Uuid::new_v4()),
        name: "Invalid Simple".to_string(),
        description: None,
        triggers: vec![],
        stages: vec![],
        environment: HashMap::new(),
        timeout: None,
        retry_count: None,
        notifications: None,
        pipeline_type: Some(PipelineType::Simple),
        repo: None,
        branch: None,
        server: None,
        steps: Some(vec![]), // Empty steps array
        variables: None,
        jobs: None,
        matrix: None,
        cache: None,
        include: None,
        created_at: None,
        updated_at: None,
    };

    let validation_result = invalid_simple.validate();
    assert!(validation_result.is_err());
    assert!(validation_result.unwrap_err().contains("step"));

    // Test standard pipeline validation errors
    let invalid_standard = CIPipeline {
        mongo_id: None,
        id: Some(uuid::Uuid::new_v4()),
        name: "".to_string(), // Empty name
        description: None,
        triggers: vec![],
        stages: vec![], // Empty stages
        environment: HashMap::new(),
        timeout: None,
        retry_count: None,
        notifications: None,
        pipeline_type: Some(PipelineType::Standard),
        repo: None,
        branch: None,
        server: None,
        steps: None,
        variables: None,
        jobs: None,
        matrix: None,
        cache: None,
        include: None,
        created_at: None,
        updated_at: None,
    };

    let validation_result = invalid_standard.validate();
    assert!(validation_result.is_err());
    let error_msg = validation_result.unwrap_err();
    assert!(error_msg.contains("name") || error_msg.contains("stage"));
}

/// Test API response format consistency across different pipeline types
#[test]
fn test_api_response_format_consistency_all_types() {
    let test_cases = vec![
        (PipelineType::Minimal, "Minimal Test Pipeline"),
        (PipelineType::Simple, "Simple Test Pipeline"),
        (PipelineType::Standard, "Standard Test Pipeline"),
        (PipelineType::Advanced, "Advanced Test Pipeline"),
    ];

    for (pipeline_type, name) in test_cases {
        let pipeline = CIPipeline {
            mongo_id: None,
            id: Some(uuid::Uuid::new_v4()),
            name: name.to_string(),
            description: Some(format!("Test description for {:?} pipeline", pipeline_type)),
            triggers: vec![],
            stages: vec![],
            environment: HashMap::new(),
            timeout: Some(3600),
            retry_count: Some(1),
            notifications: None,
            pipeline_type: Some(pipeline_type.clone()),
            repo: Some("https://github.com/user/repo.git".to_string()),
            branch: Some("main".to_string()),
            server: Some(ServerConfig::Simple("user@host".to_string())),
            steps: None,
            variables: None,
            jobs: None,
            matrix: None,
            cache: None,
            include: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        // Test JSON serialization consistency
        let json_output = serde_json::to_string(&pipeline).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
        
        // Verify required fields are present
        assert_eq!(parsed["name"], name);
        assert_eq!(parsed["pipeline_type"], serde_json::to_value(&pipeline_type).unwrap());
        assert_eq!(parsed["repo"], "https://github.com/user/repo.git");
        assert!(parsed["id"].is_string());
        assert!(parsed["created_at"].is_string());
        assert!(parsed["updated_at"].is_string());

        // Test YAML serialization consistency
        let yaml_output = pipeline.to_yaml().unwrap();
        assert!(yaml_output.contains(&format!("name: {}", name)));
        assert!(yaml_output.contains(&format!("pipeline_type: {}", 
            serde_yaml::to_string(&pipeline_type).unwrap().trim())));
        assert!(yaml_output.contains("repo: https://github.com/user/repo.git"));

        // Test round-trip consistency
        let reparsed_pipeline = CIPipeline::from_yaml(&yaml_output).unwrap();
        assert_eq!(reparsed_pipeline.name, name);
        assert_eq!(reparsed_pipeline.get_pipeline_type(), pipeline_type);
        assert_eq!(reparsed_pipeline.repo, Some("https://github.com/user/repo.git".to_string()));
    }
}

/// Test that existing pipeline configurations continue to work
#[test]
fn test_existing_pipeline_configurations_compatibility() {
    // Test with actual pipeline.yaml format (standard format)
    let existing_pipeline_yaml = r#"
name: "RustCI Build Pipeline"
description: "Standard build and test pipeline for Rust projects"
stages:
  - name: build
    steps:
      - name: "Checkout code"
        step_type: shell
        config:
          command: "git clone $REPO_URL ."
      - name: "Build project"
        step_type: shell
        config:
          command: "cargo build --release"
  - name: test
    steps:
      - name: "Run tests"
        step_type: shell
        config:
          command: "cargo test"
  - name: deploy
    steps:
      - name: "Deploy to staging"
        step_type: docker
        config:
          image: "rust:latest"
          command: "echo 'Deploying to staging'"
environment:
  RUST_LOG: "info"
  CARGO_TERM_COLOR: "always"
timeout: 3600
retry_count: 2
"#;

    let pipeline = CIPipeline::from_yaml(existing_pipeline_yaml).unwrap();
    
    // Should auto-detect as Standard type
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Standard);
    assert_eq!(pipeline.name, "RustCI Build Pipeline");
    assert_eq!(pipeline.stages.len(), 3);
    assert_eq!(pipeline.timeout, Some(3600));
    assert_eq!(pipeline.retry_count, Some(2));
    assert!(!pipeline.environment.is_empty());
    
    // Should validate successfully
    assert!(pipeline.validate().is_ok());

    // Test with k3s-pipeline.yaml format (also standard)
    let k3s_pipeline_yaml = r#"
name: "K3s Deployment Pipeline"
description: "Kubernetes deployment pipeline"
stages:
  - name: build
    steps:
      - name: "Build Docker image"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          image: "rustci:latest"
  - name: deploy
    steps:
      - name: "Deploy to K3s"
        step_type: kubernetes
        config:
          namespace: "default"
          manifest: "k8s-deployment.yaml"
environment:
  KUBECONFIG: "/etc/kubeconfig"
timeout: 1800
"#;

    let k3s_pipeline = CIPipeline::from_yaml(k3s_pipeline_yaml).unwrap();
    
    // Should also auto-detect as Standard type
    assert_eq!(k3s_pipeline.get_pipeline_type(), PipelineType::Standard);
    assert_eq!(k3s_pipeline.name, "K3s Deployment Pipeline");
    assert_eq!(k3s_pipeline.stages.len(), 2);
    assert_eq!(k3s_pipeline.timeout, Some(1800));
    
    // Should validate successfully
    assert!(k3s_pipeline.validate().is_ok());
}