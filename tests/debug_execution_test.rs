// Debug test to trace pipeline execution
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

// This would be a test to add to verify execution
#[tokio::test]
async fn test_pipeline_execution_debug() {
    // Create a simple test pipeline with a command that should take time
    let mut pipeline = CIPipeline::new("debug-test".to_string());
    pipeline.stages = vec![
        Stage {
            name: "debug-stage".to_string(),
            steps: vec![
                Step {
                    name: "debug-step".to_string(),
                    step_type: StepType::Shell,
                    config: StepConfig {
                        command: Some("echo 'Starting debug test' && sleep 5 && echo 'Debug test completed'".to_string()),
                        ..Default::default()
                    },
                    condition: None,
                    continue_on_error: Some(false),
                    timeout: Some(30),
                }
            ],
            condition: None,
            parallel: Some(false),
            timeout: Some(60),
            retry_count: Some(0),
            environment: None,
        }
    ];

    // Create execution context
    let context = ExecutionContext {
        execution_id: Uuid::new_v4(),
        pipeline_id: pipeline.id.unwrap(),
        correlation_id: Uuid::new_v4(),
        pipeline,
        trigger_info: TriggerInfo {
            trigger_type: "manual".to_string(),
            triggered_by: Some("debug-test".to_string()),
            commit_hash: None,
            branch: None,
            repository: None,
            webhook_payload: None,
        },
        environment: HashMap::new(),
        started_at: Utc::now(),
    };

    // Test direct executor execution
    let connector_manager = ConnectorManager::new();
    let executor = PipelineExecutor::new(
        connector_manager,
        std::path::PathBuf::from("/tmp/debug-cache"),
        std::path::PathBuf::from("/tmp/debug-deploy"),
    );

    let workspace = Workspace {
        id: Uuid::new_v4(),
        path: std::path::PathBuf::from("/tmp/debug-workspace"),
        execution_id: context.execution_id,
    };

    // Create workspace directory
    std::fs::create_dir_all(&workspace.path).unwrap();

    let start_time = std::time::Instant::now();
    
    // Execute the step directly
    let execution = Arc::new(tokio::sync::RwLock::new(
        PipelineExecution::new(context.pipeline_id, context.trigger_info.clone())
    ));

    let result = executor.execute_step(
        execution,
        "debug-stage",
        &context.pipeline.stages[0].steps[0],
        &workspace,
        &context.environment,
    ).await;

    let duration = start_time.elapsed();
    
    println!("Debug test result: {:?}", result);
    println!("Debug test duration: {:?}", duration);
    
    // This should take at least 5 seconds due to the sleep command
    assert!(duration.as_secs() >= 5, "Execution should take at least 5 seconds, took: {:?}", duration);
    assert!(result.is_ok(), "Execution should succeed");
}