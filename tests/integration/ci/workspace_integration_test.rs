use crate::ci::{
    config::{CIPipeline, Stage, Step, StepConfig, StepType},
    template_engine::PipelineTemplateEngine,
    workspace::WorkspaceManager,
};
use std::collections::HashMap;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_git_clone_workspace_fix() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager = WorkspaceManager::new(temp_dir.path().to_path_buf());
    
    let execution_id = Uuid::new_v4();
    
    // Create workspace with enhanced context
    let workspace_context = workspace_manager
        .create_workspace_with_context(execution_id)
        .await
        .unwrap();
    
    // Create a test pipeline with the problematic Git clone command
    let mut pipeline = CIPipeline::new("test-git-clone".to_string());
    pipeline.description = Some("Test Git clone fix".to_string());
    pipeline.stages = vec![Stage {
        name: "fetch-code".to_string(),
        condition: None,
        parallel: Some(false),
        timeout: Some(600),
        retry_count: Some(0),
        environment: None,
        steps: vec![Step {
            name: "fetch-code".to_string(),
            step_type: StepType::Shell,
            config: StepConfig {
                command: Some(
                    "rm -rf /tmp/rustci && \
                    git clone https://github.com/RitabrataRoychowdhury/RustCI.git /tmp/rustci && \
                    cd /tmp/rustci && \
                    echo \"Repository cloned successfully\" && \
                    ls -la".to_string()
                ),
                ..Default::default()
            },
            condition: None,
            continue_on_error: Some(false),
            timeout: Some(300),
        }],
    }];
    
    pipeline.timeout = Some(3600);
    pipeline.retry_count = Some(0);
    
    // Process pipeline with template engine
    let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
    template_engine.process_pipeline(&mut pipeline).unwrap();
    
    // Verify that the Git clone command uses the correct workspace path
    let git_step = &pipeline.stages[0].steps[0];
    let processed_command = git_step.config.command.as_ref().unwrap();
    
    // Should not contain hardcoded /tmp/rustci
    assert!(!processed_command.contains("/tmp/rustci"));
    
    // Should contain the actual source directory path
    assert!(processed_command.contains(&*workspace_context.source_directory.to_string_lossy()));
    
    // Should ensure directory exists
    assert!(processed_command.contains("mkdir -p"));
    
    println!("✅ Original command: rm -rf /tmp/rustci && git clone https://github.com/RitabrataRoychowdhury/RustCI.git /tmp/rustci");
    println!("✅ Processed command: {}", processed_command);
    
    // Verify workspace directories exist
    assert!(workspace_context.workspace_path.exists());
    assert!(workspace_context.source_directory.exists());
    assert!(workspace_context.build_directory.exists());
    assert!(workspace_context.artifacts_directory.exists());
    
    // Verify environment variables are set correctly
    let env_vars = workspace_context.get_environment_variables();
    assert!(env_vars.contains_key("SOURCE_DIR"));
    assert!(env_vars.contains_key("BUILD_DIR"));
    assert!(env_vars.contains_key("ARTIFACTS_DIR"));
    assert!(env_vars.contains_key("WORKSPACE_PATH"));
    assert!(env_vars.contains_key("EXECUTION_ID"));
    
    println!("✅ Workspace context test passed");
    println!("   - Workspace path: {:?}", workspace_context.workspace_path);
    println!("   - Source directory: {:?}", workspace_context.source_directory);
    println!("   - Build directory: {:?}", workspace_context.build_directory);
    println!("   - Artifacts directory: {:?}", workspace_context.artifacts_directory);
}

#[tokio::test]
async fn test_docker_build_workspace_fix() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_manager = WorkspaceManager::new(temp_dir.path().to_path_buf());
    
    let execution_id = Uuid::new_v4();
    let workspace_context = workspace_manager
        .create_workspace_with_context(execution_id)
        .await
        .unwrap();
    
    // Create a test pipeline with Docker build command
    let mut pipeline = CIPipeline::new("test-docker-build".to_string());
    pipeline.description = Some("Test Docker build fix".to_string());
    pipeline.stages = vec![Stage {
        name: "build-docker-image".to_string(),
        condition: None,
        parallel: Some(false),
        timeout: Some(600),
        retry_count: Some(0),
        environment: None,
        steps: vec![Step {
            name: "build-docker-image".to_string(),
            step_type: StepType::Shell,
            config: StepConfig {
                command: Some(
                    "cd /tmp/rustci && \
                    echo \"Building Docker image...\" && \
                    docker build -t rustci:latest . && \
                    echo \"Docker image built successfully\"".to_string()
                ),
                ..Default::default()
            },
            condition: None,
            continue_on_error: Some(false),
            timeout: Some(300),
        }],
    }];
    
    pipeline.timeout = Some(3600);
    pipeline.retry_count = Some(0);
    
    // Process pipeline with template engine
    let template_engine = PipelineTemplateEngine::new(workspace_context.clone());
    template_engine.process_pipeline(&mut pipeline).unwrap();
    
    // Verify that the Docker build command uses the correct workspace path
    let docker_step = &pipeline.stages[0].steps[0];
    let processed_command = docker_step.config.command.as_ref().unwrap();
    
    // Should not contain hardcoded /tmp/rustci
    assert!(!processed_command.contains("/tmp/rustci"));
    
    // Should contain the actual source directory path
    assert!(processed_command.contains(&*workspace_context.source_directory.to_string_lossy()));
    
    // Should ensure directory exists
    assert!(processed_command.contains("mkdir -p"));
    
    println!("✅ Docker build workspace fix test passed");
    println!("   - Processed command: {}", processed_command);
}

#[test]
fn test_workspace_variable_injection() {
    use std::path::PathBuf;
    
    let workspace_path = PathBuf::from("/tmp/test-workspace");
    let source_dir = workspace_path.join("source");
    let build_dir = workspace_path.join("build");
    let artifacts_dir = workspace_path.join("artifacts");
    
    let workspace_context = crate::ci::workspace::WorkspaceContext {
        workspace_path: workspace_path.clone(),
        execution_id: Uuid::new_v4(),
        environment_variables: HashMap::new(),
        working_directory: source_dir.clone(),
        source_directory: source_dir,
        build_directory: build_dir,
        artifacts_directory: artifacts_dir,
    };
    
    // Test variable injection
    let command = "echo ${SOURCE_DIR} && ls ${BUILD_DIR} && cp file ${ARTIFACTS_DIR}";
    let processed = workspace_context.inject_into_command(command);
    
    assert!(processed.contains("/tmp/test-workspace/source"));
    assert!(processed.contains("/tmp/test-workspace/build"));
    assert!(processed.contains("/tmp/test-workspace/artifacts"));
    
    println!("✅ Variable injection test passed");
    println!("   - Original: {}", command);
    println!("   - Processed: {}", processed);
}