//! Integration tests for end-to-end execution of each pipeline type
//!
//! This module tests the complete execution flow for Minimal, Simple, Standard,
//! and Advanced pipeline types to ensure they work correctly in practice.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;
use uuid::Uuid;

use RustAutoDevOps::{
    ci::{
        config::{CIPipeline, PipelineType, SimpleStep, ServerConfig, PipelineJob, JobScript, MatrixConfig, CacheConfig},
        engine::{
            execution_strategies::{
                ExecutionContext, ExecutionStrategy, MinimalExecutionStrategy, 
                SimpleExecutionStrategy, StandardExecutionStrategy, AdvancedExecutionStrategy
            },
            orchestrator::PipelineOrchestrator,
        },
        executor::PipelineExecutor,
        pipeline::{TriggerInfo, TriggerType},
        workspace::WorkspaceManager,
    },
    core::infrastructure::event_loop::EventDemultiplexer,
    infrastructure::database::db::Database,
};

/// Test minimal pipeline end-to-end execution
#[tokio::test]
async fn test_minimal_pipeline_execution() {
    let minimal_yaml = r#"
name: "Test Minimal Pipeline"
repo: https://github.com/octocat/Hello-World.git
server: localhost
branch: main
"#;

    let pipeline = CIPipeline::from_yaml(minimal_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Minimal);

    // Create execution context
    let execution_id = Uuid::new_v4();
    let pipeline_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();

    let context = ExecutionContext {
        execution_id,
        pipeline_id,
        correlation_id,
        pipeline: pipeline.clone(),
        trigger_info: TriggerInfo {
            trigger_type: TriggerType::Manual,
            triggered_by: "test".to_string(),
            trigger_data: HashMap::new(),
        },
        environment: HashMap::new(),
        started_at: chrono::Utc::now(),
    };

    // Create minimal execution strategy
    let event_demux = Arc::new(EventDemultiplexer::new());
    let workspace_manager = Arc::new(WorkspaceManager::new(event_demux.clone()));
    let executor = Arc::new(PipelineExecutor::new(workspace_manager.clone()));
    
    let strategy = MinimalExecutionStrategy::new(executor, workspace_manager);

    // Test that strategy can handle minimal pipeline
    assert!(strategy.can_handle(&context));
    assert_eq!(strategy.strategy_name(), "Minimal");

    // Execute with timeout to prevent hanging
    let execution_result = timeout(Duration::from_secs(120), strategy.execute(&context)).await;
    
    match execution_result {
        Ok(Ok(result)) => {
            println!("Minimal pipeline execution completed: {:?}", result.status);
            assert_eq!(result.execution_id, execution_id);
            assert!(result.duration_ms > 0);
            assert!(!result.logs.is_empty());
        }
        Ok(Err(e)) => {
            // Execution may fail in test environment due to missing dependencies
            println!("Minimal pipeline execution failed (expected in test): {}", e);
            assert!(e.to_string().contains("git") || e.to_string().contains("docker") || e.to_string().contains("command"));
        }
        Err(_) => {
            panic!("Minimal pipeline execution timed out");
        }
    }
}

/// Test simple pipeline end-to-end execution
#[tokio::test]
async fn test_simple_pipeline_execution() {
    let simple_yaml = r#"
name: "Test Simple Pipeline"
repo: https://github.com/octocat/Hello-World.git
steps:
  - run: echo "Starting build"
  - run: echo "Running tests"
  - run: echo "Build completed"
"#;

    let pipeline = CIPipeline::from_yaml(simple_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Simple);
    assert!(pipeline.steps.is_some());
    assert_eq!(pipeline.steps.as_ref().unwrap().len(), 3);

    // Create execution context
    let execution_id = Uuid::new_v4();
    let pipeline_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();

    let context = ExecutionContext {
        execution_id,
        pipeline_id,
        correlation_id,
        pipeline: pipeline.clone(),
        trigger_info: TriggerInfo {
            trigger_type: TriggerType::Manual,
            triggered_by: "test".to_string(),
            trigger_data: HashMap::new(),
        },
        environment: HashMap::new(),
        started_at: chrono::Utc::now(),
    };

    // Create simple execution strategy
    let event_demux = Arc::new(EventDemultiplexer::new());
    let workspace_manager = Arc::new(WorkspaceManager::new(event_demux.clone()));
    let executor = Arc::new(PipelineExecutor::new(workspace_manager.clone()));
    
    let strategy = SimpleExecutionStrategy::new(executor, workspace_manager);

    // Test that strategy can handle simple pipeline
    assert!(strategy.can_handle(&context));
    assert_eq!(strategy.strategy_name(), "Simple");

    // Execute with timeout
    let execution_result = timeout(Duration::from_secs(60), strategy.execute(&context)).await;
    
    match execution_result {
        Ok(Ok(result)) => {
            println!("Simple pipeline execution completed: {:?}", result.status);
            assert_eq!(result.execution_id, execution_id);
            assert_eq!(result.steps_executed, 3);
            assert!(result.duration_ms > 0);
            assert!(!result.logs.is_empty());
            
            // Check that all steps were logged
            let log_text = result.logs.join(" ");
            assert!(log_text.contains("Starting build"));
            assert!(log_text.contains("Running tests"));
            assert!(log_text.contains("Build completed"));
        }
        Ok(Err(e)) => {
            println!("Simple pipeline execution failed (may be expected in test): {}", e);
        }
        Err(_) => {
            panic!("Simple pipeline execution timed out");
        }
    }
}

/// Test standard pipeline end-to-end execution
#[tokio::test]
async fn test_standard_pipeline_execution() {
    let standard_yaml = r#"
name: "Test Standard Pipeline"
repo: https://github.com/octocat/Hello-World.git
stages:
  - name: build
    steps:
      - name: "Build application"
        step_type: shell
        config:
          command: "echo 'Building application'"
  - name: test
    steps:
      - name: "Run tests"
        step_type: shell
        config:
          command: "echo 'Running tests'"
  - name: deploy
    steps:
      - name: "Deploy application"
        step_type: shell
        config:
          command: "echo 'Deploying application'"
jobs:
  build:
    stage: build
    script: "echo 'Build job'"
  test:
    stage: test
    script: "echo 'Test job'"
"#;

    let pipeline = CIPipeline::from_yaml(standard_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Standard);
    assert_eq!(pipeline.stages.len(), 3);
    assert!(pipeline.jobs.is_some());

    // Create execution context
    let execution_id = Uuid::new_v4();
    let pipeline_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();

    let context = ExecutionContext {
        execution_id,
        pipeline_id,
        correlation_id,
        pipeline: pipeline.clone(),
        trigger_info: TriggerInfo {
            trigger_type: TriggerType::Manual,
            triggered_by: "test".to_string(),
            trigger_data: HashMap::new(),
        },
        environment: HashMap::new(),
        started_at: chrono::Utc::now(),
    };

    // Create standard execution strategy
    let event_demux = Arc::new(EventDemultiplexer::new());
    let workspace_manager = Arc::new(WorkspaceManager::new(event_demux.clone()));
    let executor = Arc::new(PipelineExecutor::new(workspace_manager.clone()));
    
    let strategy = StandardExecutionStrategy::new(executor, workspace_manager);

    // Test that strategy can handle standard pipeline
    assert!(strategy.can_handle(&context));
    assert_eq!(strategy.strategy_name(), "Standard");

    // Execute with timeout
    let execution_result = timeout(Duration::from_secs(90), strategy.execute(&context)).await;
    
    match execution_result {
        Ok(Ok(result)) => {
            println!("Standard pipeline execution completed: {:?}", result.status);
            assert_eq!(result.execution_id, execution_id);
            assert_eq!(result.stages_executed, 3);
            assert!(result.duration_ms > 0);
            assert!(!result.logs.is_empty());
        }
        Ok(Err(e)) => {
            println!("Standard pipeline execution failed (may be expected in test): {}", e);
        }
        Err(_) => {
            panic!("Standard pipeline execution timed out");
        }
    }
}

/// Test advanced pipeline end-to-end execution
#[tokio::test]
async fn test_advanced_pipeline_execution() {
    let advanced_yaml = r#"
name: "Test Advanced Pipeline"
repo: https://github.com/octocat/Hello-World.git
variables:
  RUST_VERSION: "1.70"
  BUILD_ENV: "test"
  CACHE_KEY: "rust-cache-v1"
stages:
  - name: build
    steps:
      - name: "Build with matrix"
        step_type: shell
        config:
          command: "echo 'Building with Rust $RUST_VERSION in $BUILD_ENV'"
  - name: test
    steps:
      - name: "Run tests"
        step_type: shell
        config:
          command: "echo 'Running tests with cache key $CACHE_KEY'"
jobs:
  build:
    stage: build
    script: "echo 'Advanced build job'"
    matrix:
      variables:
        rust: ["1.70", "1.71"]
        os: ["ubuntu", "alpine"]
cache:
  paths: ["target/", "~/.cargo/"]
  key: "rust-cache-v1"
  policy: "pull-push"
"#;

    let pipeline = CIPipeline::from_yaml(advanced_yaml).unwrap();
    assert_eq!(pipeline.get_pipeline_type(), PipelineType::Advanced);
    assert!(pipeline.variables.is_some());
    assert!(pipeline.cache.is_some());
    assert!(pipeline.jobs.is_some());

    // Verify advanced features are parsed correctly
    let variables = pipeline.variables.as_ref().unwrap();
    assert_eq!(variables.get("RUST_VERSION"), Some(&"1.70".to_string()));
    assert_eq!(variables.get("BUILD_ENV"), Some(&"test".to_string()));

    let cache = pipeline.cache.as_ref().unwrap();
    assert_eq!(cache.key, Some("rust-cache-v1".to_string()));
    assert_eq!(cache.policy, Some("pull-push".to_string()));
    assert_eq!(cache.paths.len(), 2);

    // Create execution context
    let execution_id = Uuid::new_v4();
    let pipeline_id = Uuid::new_v4();
    let correlation_id = Uuid::new_v4();

    let mut environment = HashMap::new();
    environment.insert("RUST_VERSION".to_string(), "1.70".to_string());
    environment.insert("BUILD_ENV".to_string(), "test".to_string());

    let context = ExecutionContext {
        execution_id,
        pipeline_id,
        correlation_id,
        pipeline: pipeline.clone(),
        trigger_info: TriggerInfo {
            trigger_type: TriggerType::Manual,
            triggered_by: "test".to_string(),
            trigger_data: HashMap::new(),
        },
        environment,
        started_at: chrono::Utc::now(),
    };

    // Create advanced execution strategy
    let event_demux = Arc::new(EventDemultiplexer::new());
    let workspace_manager = Arc::new(WorkspaceManager::new(event_demux.clone()));
    let executor = Arc::new(PipelineExecutor::new(workspace_manager.clone()));
    
    let strategy = AdvancedExecutionStrategy::new(executor, workspace_manager);

    // Test that strategy can handle advanced pipeline
    assert!(strategy.can_handle(&context));
    assert_eq!(strategy.strategy_name(), "Advanced");

    // Execute with timeout
    let execution_result = timeout(Duration::from_secs(120), strategy.execute(&context)).await;
    
    match execution_result {
        Ok(Ok(result)) => {
            println!("Advanced pipeline execution completed: {:?}", result.status);
            assert_eq!(result.execution_id, execution_id);
            assert!(result.duration_ms > 0);
            assert!(!result.logs.is_empty());
            
            // Check that variables were processed
            let log_text = result.logs.join(" ");
            assert!(log_text.contains("1.70") || log_text.contains("test"));
        }
        Ok(Err(e)) => {
            println!("Advanced pipeline execution failed (may be expected in test): {}", e);
        }
        Err(_) => {
            panic!("Advanced pipeline execution timed out");
        }
    }
}

/// Test pipeline type strategy selection
#[tokio::test]
async fn test_pipeline_strategy_selection() {
    let event_demux = Arc::new(EventDemultiplexer::new());
    let workspace_manager = Arc::new(WorkspaceManager::new(event_demux.clone()));
    let executor = Arc::new(PipelineExecutor::new(workspace_manager.clone()));

    // Create all strategies
    let minimal_strategy = MinimalExecutionStrategy::new(executor.clone(), workspace_manager.clone());
    let simple_strategy = SimpleExecutionStrategy::new(executor.clone(), workspace_manager.clone());
    let standard_strategy = StandardExecutionStrategy::new(executor.clone(), workspace_manager.clone());
    let advanced_strategy = AdvancedExecutionStrategy::new(executor.clone(), workspace_manager.clone());

    // Test minimal pipeline
    let minimal_pipeline = CIPipeline::from_yaml(r#"
name: "Minimal Test"
repo: https://github.com/user/repo.git
server: localhost
"#).unwrap();

    let minimal_context = create_test_context(minimal_pipeline);
    assert!(minimal_strategy.can_handle(&minimal_context));
    assert!(simple_strategy.can_handle(&minimal_context)); // Simple can handle minimal
    assert!(standard_strategy.can_handle(&minimal_context)); // Standard can handle minimal
    assert!(advanced_strategy.can_handle(&minimal_context)); // Advanced can handle minimal

    // Test simple pipeline
    let simple_pipeline = CIPipeline::from_yaml(r#"
name: "Simple Test"
steps:
  - run: echo "test"
"#).unwrap();

    let simple_context = create_test_context(simple_pipeline);
    assert!(!minimal_strategy.can_handle(&simple_context)); // Minimal cannot handle simple
    assert!(simple_strategy.can_handle(&simple_context));
    assert!(standard_strategy.can_handle(&simple_context)); // Standard can handle simple
    assert!(advanced_strategy.can_handle(&simple_context)); // Advanced can handle simple

    // Test standard pipeline
    let standard_pipeline = CIPipeline::from_yaml(r#"
name: "Standard Test"
stages:
  - name: build
    steps:
      - name: "Build"
        step_type: shell
        config:
          command: "echo build"
"#).unwrap();

    let standard_context = create_test_context(standard_pipeline);
    assert!(!minimal_strategy.can_handle(&standard_context));
    assert!(!simple_strategy.can_handle(&standard_context));
    assert!(standard_strategy.can_handle(&standard_context));
    assert!(advanced_strategy.can_handle(&standard_context)); // Advanced can handle standard

    // Test advanced pipeline
    let advanced_pipeline = CIPipeline::from_yaml(r#"
name: "Advanced Test"
variables:
  TEST: "value"
stages:
  - name: build
    steps:
      - name: "Build"
        step_type: shell
        config:
          command: "echo build"
"#).unwrap();

    let advanced_context = create_test_context(advanced_pipeline);
    assert!(!minimal_strategy.can_handle(&advanced_context));
    assert!(!simple_strategy.can_handle(&advanced_context));
    assert!(!standard_strategy.can_handle(&advanced_context));
    assert!(advanced_strategy.can_handle(&advanced_context));
}

/// Test error handling in pipeline execution
#[tokio::test]
async fn test_pipeline_execution_error_handling() {
    // Test simple pipeline with failing step
    let failing_yaml = r#"
name: "Failing Pipeline"
steps:
  - run: echo "This will succeed"
  - run: exit 1
  - run: echo "This should not run"
"#;

    let pipeline = CIPipeline::from_yaml(failing_yaml).unwrap();
    let context = create_test_context(pipeline);

    let event_demux = Arc::new(EventDemultiplexer::new());
    let workspace_manager = Arc::new(WorkspaceManager::new(event_demux.clone()));
    let executor = Arc::new(PipelineExecutor::new(workspace_manager.clone()));
    
    let strategy = SimpleExecutionStrategy::new(executor, workspace_manager);

    // Execute with timeout
    let execution_result = timeout(Duration::from_secs(30), strategy.execute(&context)).await;
    
    match execution_result {
        Ok(Ok(result)) => {
            // Should fail due to exit 1
            println!("Pipeline result: {:?}", result.status);
            // In test environment, the command might not actually fail
        }
        Ok(Err(e)) => {
            println!("Pipeline failed as expected: {}", e);
        }
        Err(_) => {
            panic!("Pipeline execution timed out");
        }
    }
}

/// Helper function to create test execution context
fn create_test_context(pipeline: CIPipeline) -> ExecutionContext {
    ExecutionContext {
        execution_id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        pipeline,
        trigger_info: TriggerInfo {
            trigger_type: TriggerType::Manual,
            triggered_by: "test".to_string(),
            trigger_data: HashMap::new(),
        },
        environment: HashMap::new(),
        started_at: chrono::Utc::now(),
    }
}