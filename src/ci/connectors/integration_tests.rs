//! Integration tests for the connector system
//! 
//! These tests verify that connectors work correctly with the broader CI system
//! including the pipeline executor, workspace management, and database operations.

#[cfg(test)]
mod integration_tests {
    use crate::ci::{
        config::{CIPipeline, Stage, Step, StepConfig, StepType, Trigger, TriggerType, TriggerConfig},
        connectors::{ConnectorManager, ConnectorType, ExecutionResult},
        workspace::Workspace,
    };
    use crate::error::Result;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn create_test_workspace() -> Workspace {
        Workspace {
            id: Uuid::new_v4(),
            path: PathBuf::from("/tmp/test-workspace"),
            execution_id: Uuid::new_v4(),
        }
    }

    fn create_test_pipeline() -> CIPipeline {
        CIPipeline {
            mongo_id: None,
            id: Some(Uuid::new_v4()),
            name: "Test Pipeline".to_string(),
            description: Some("Integration test pipeline".to_string()),
            triggers: vec![Trigger {
                trigger_type: TriggerType::Manual,
                config: TriggerConfig {
                    webhook_url: None,
                    cron_expression: None,
                    branch_patterns: None,
                    repository: None,
                    events: None,
                },
            }],
            stages: vec![
                Stage {
                    name: "Build".to_string(),
                    condition: None,
                    parallel: Some(false),
                    steps: vec![
                        Step {
                            name: "docker-build".to_string(),
                            step_type: StepType::Docker,
                            config: StepConfig {
                                image: Some("alpine:latest".to_string()),
                                command: Some("echo 'Building application'".to_string()),
                                ..Default::default()
                            },
                            condition: None,
                            continue_on_error: None,
                            timeout: Some(60),
                        },
                        Step {
                            name: "kubernetes-deploy".to_string(),
                            step_type: StepType::Kubernetes,
                            config: StepConfig {
                                image: Some("kubectl:latest".to_string()),
                                command: Some("echo 'Deploying to Kubernetes'".to_string()),
                                namespace: Some("default".to_string()),
                                ..Default::default()
                            },
                            condition: None,
                            continue_on_error: None,
                            timeout: Some(120),
                        },
                    ],
                    environment: None,
                    timeout: None,
                    retry_count: None,
                },
            ],
            environment: {
                let mut env = HashMap::new();
                env.insert("CI".to_string(), "true".to_string());
                env.insert("BUILD_ENV".to_string(), "test".to_string());
                env
            },
            notifications: None,
            timeout: Some(3600),
            retry_count: Some(1),
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    #[tokio::test]
    async fn test_connector_manager_integration() {
        let mut connector_manager = ConnectorManager::new();
        
        // Test that all expected connectors can be created
        let connector_types = vec![
            ConnectorType::Docker,
            ConnectorType::Kubernetes,
            ConnectorType::AWS,
            ConnectorType::Azure,
            ConnectorType::GCP,
            ConnectorType::GitHub,
            ConnectorType::GitLab,
        ];
        
        for connector_type in connector_types {
            let connector = connector_manager.get_connector(connector_type.clone());
            assert!(connector.is_ok(), "Failed to get connector: {}", connector_type);
            
            let connector = connector.unwrap();
            assert_eq!(connector.connector_type(), connector_type);
        }
        
        // Verify cache is populated
        let stats = connector_manager.get_stats();
        assert_eq!(stats.get("connectors_cached"), Some(&7));
    }

    #[tokio::test]
    async fn test_connector_validation_integration() {
        let mut connector_manager = ConnectorManager::new();
        
        // Test Docker connector validation
        let docker_step = Step {
            name: "test-docker".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                image: Some("alpine:latest".to_string()),
                command: Some("echo hello".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: Some(60),
        };
        
        let docker_connector = connector_manager.get_connector(ConnectorType::Docker).unwrap();
        assert!(docker_connector.validate_config(&docker_step).is_ok());
        
        // Test Kubernetes connector validation
        let k8s_step = Step {
            name: "test-k8s".to_string(),
            step_type: StepType::Kubernetes,
            config: StepConfig {
                image: Some("ubuntu:latest".to_string()),
                command: Some("echo hello".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: Some(120),
        };
        
        let k8s_connector = connector_manager.get_connector(ConnectorType::Kubernetes).unwrap();
        assert!(k8s_connector.validate_config(&k8s_step).is_ok());
    }

    #[tokio::test]
    async fn test_connector_type_determination() {
        let connector_manager = ConnectorManager::new();
        
        let test_cases = vec![
            (StepType::Docker, ConnectorType::Docker),
            (StepType::Kubernetes, ConnectorType::Kubernetes),
            (StepType::AWS, ConnectorType::AWS),
            (StepType::Azure, ConnectorType::Azure),
            (StepType::GCP, ConnectorType::GCP),
            (StepType::GitHub, ConnectorType::GitHub),
            (StepType::GitLab, ConnectorType::GitLab),
            (StepType::Shell, ConnectorType::Docker), // Shell defaults to Docker
        ];
        
        for (step_type, expected_connector_type) in test_cases {
            let step = Step {
                name: "test".to_string(),
                step_type,
                config: StepConfig::default(),
                condition: None,
                continue_on_error: None,
                timeout: None,
            };
            
            let determined_type = connector_manager.determine_connector_type(&step).unwrap();
            assert_eq!(determined_type, expected_connector_type);
        }
    }

    #[tokio::test]
    async fn test_custom_connector_type_determination() {
        let connector_manager = ConnectorManager::new();
        
        // Test custom step with plugin name
        let custom_step = Step {
            name: "test-custom".to_string(),
            step_type: StepType::Custom,
            config: StepConfig {
                plugin_name: Some("my-custom-plugin".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let determined_type = connector_manager.determine_connector_type(&custom_step).unwrap();
        assert_eq!(determined_type, ConnectorType::Custom("my-custom-plugin".to_string()));
        
        // Test custom step without plugin name (should fail)
        let invalid_custom_step = Step {
            name: "test-custom".to_string(),
            step_type: StepType::Custom,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let result = connector_manager.determine_connector_type(&invalid_custom_step);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connector_lifecycle_hooks() {
        let mut connector_manager = ConnectorManager::new();
        let workspace = create_test_workspace();
        let env: HashMap<String, String> = HashMap::new();
        
        let step = Step {
            name: "test-lifecycle".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                image: Some("alpine:latest".to_string()),
                command: Some("echo 'test'".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: Some(60),
        };
        
        let connector = connector_manager.get_connector(ConnectorType::Docker).unwrap();
        
        // Test pre-execution hook
        let pre_result = connector.pre_execute(&step).await;
        assert!(pre_result.is_ok());
        
        // Test post-execution hook
        let execution_result = ExecutionResult::success("test output".to_string());
        let post_result = connector.post_execute(&step, &execution_result).await;
        assert!(post_result.is_ok());
    }

    #[tokio::test]
    async fn test_connector_error_handling() {
        let mut connector_manager = ConnectorManager::new();
        
        // Test getting unsupported connector type
        let result = connector_manager.get_connector(ConnectorType::Custom("unsupported".to_string()));
        assert!(result.is_err());
        
        // Test validation with invalid configuration
        let docker_connector = connector_manager.get_connector(ConnectorType::Docker).unwrap();
        let invalid_step = Step {
            name: "".to_string(), // Empty name should fail validation
            step_type: StepType::Docker,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let validation_result = docker_connector.validate_config(&invalid_step);
        assert!(validation_result.is_err());
    }

    #[tokio::test]
    async fn test_connector_manager_stats_and_management() {
        let mut connector_manager = ConnectorManager::new();
        
        // Initial state
        let initial_stats = connector_manager.get_stats();
        assert_eq!(initial_stats.get("factories_registered"), Some(&1));
        assert_eq!(initial_stats.get("connectors_cached"), Some(&0));
        
        // Get some connectors to populate cache
        let _docker = connector_manager.get_connector(ConnectorType::Docker).unwrap();
        let _k8s = connector_manager.get_connector(ConnectorType::Kubernetes).unwrap();
        
        let stats_after_cache = connector_manager.get_stats();
        assert_eq!(stats_after_cache.get("connectors_cached"), Some(&2));
        
        // Test available connectors list
        let available = connector_manager.list_available_connectors();
        assert!(available.contains(&ConnectorType::Docker));
        assert!(available.contains(&ConnectorType::Kubernetes));
        assert!(available.contains(&ConnectorType::AWS));
        assert!(available.contains(&ConnectorType::Azure));
        assert!(available.contains(&ConnectorType::GCP));
        assert!(available.contains(&ConnectorType::GitHub));
        assert!(available.contains(&ConnectorType::GitLab));
        
        // Test cache clearing
        connector_manager.clear_cache();
        let stats_after_clear = connector_manager.get_stats();
        assert_eq!(stats_after_clear.get("connectors_cached"), Some(&0));
        
        // Test factory management
        assert_eq!(connector_manager.factory_count(), 1);
        assert!(connector_manager.has_factory("built-in"));
        assert!(!connector_manager.has_factory("non-existent"));
    }

    #[tokio::test]
    async fn test_execution_result_comprehensive() {
        // Test all ExecutionResult creation methods
        let success_result = ExecutionResult::success("success output".to_string());
        assert!(success_result.is_success());
        assert_eq!(success_result.exit_code, 0);
        assert_eq!(success_result.stdout, "success output");
        assert!(success_result.stderr.is_empty());
        
        let failure_result = ExecutionResult::failure(1, "error message".to_string());
        assert!(!failure_result.is_success());
        assert_eq!(failure_result.exit_code, 1);
        assert!(failure_result.stdout.is_empty());
        assert_eq!(failure_result.stderr, "error message");
        
        let custom_result = ExecutionResult::new(2, "custom output".to_string(), "custom error".to_string());
        assert!(!custom_result.is_success());
        assert_eq!(custom_result.exit_code, 2);
        assert_eq!(custom_result.stdout, "custom output");
        assert_eq!(custom_result.stderr, "custom error");
        
        // Test metadata operations
        let result_with_metadata = custom_result
            .with_metadata("key1".to_string(), "value1".to_string())
            .with_metadata("key2".to_string(), "value2".to_string());
        
        assert_eq!(result_with_metadata.metadata.len(), 2);
        assert_eq!(result_with_metadata.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(result_with_metadata.metadata.get("key2"), Some(&"value2".to_string()));
    }

    // Mock workspace manager for testing
    struct MockWorkspaceManager;

    impl MockWorkspaceManager {
        fn new() -> Self {
            Self
        }
        
        async fn create_workspace(&self, execution_id: Uuid) -> Result<Workspace> {
            Ok(Workspace {
                id: Uuid::new_v4(),
                path: PathBuf::from(format!("/tmp/workspace-{}", execution_id)),
                execution_id,
            })
        }
        
        async fn cleanup_workspace(&self, _execution_id: Uuid) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_connector_system_end_to_end() {
        // This test simulates a complete pipeline execution using connectors
        let mut connector_manager = ConnectorManager::new();
        let workspace_manager = MockWorkspaceManager::new();
        let pipeline = create_test_pipeline();
        
        // Create workspace
        let workspace = workspace_manager.create_workspace(Uuid::new_v4()).await.unwrap();
        
        // Execute each stage
        for stage in &pipeline.stages {
            for step in &stage.steps {
                // Determine connector type
                let connector_type = connector_manager.determine_connector_type(step).unwrap();
                
                // Get connector
                let connector = connector_manager.get_connector(connector_type).unwrap();
                
                // Validate configuration
                let validation_result = connector.validate_config(step);
                assert!(validation_result.is_ok(), "Validation failed for step: {}", step.name);
                
                // Execute pre-hook
                let pre_hook_result = connector.pre_execute(step).await;
                assert!(pre_hook_result.is_ok(), "Pre-hook failed for step: {}", step.name);
                
                // Note: We don't actually execute the step here since it would require
                // real Docker/Kubernetes environments, but we've validated the integration points
                
                // Simulate execution result
                let execution_result = ExecutionResult::success(format!("Executed {}", step.name));
                
                // Execute post-hook
                let post_hook_result = connector.post_execute(step, &execution_result).await;
                assert!(post_hook_result.is_ok(), "Post-hook failed for step: {}", step.name);
            }
        }
        
        // Cleanup workspace
        let cleanup_result = workspace_manager.cleanup_workspace(workspace.execution_id).await;
        assert!(cleanup_result.is_ok());
    }

    #[tokio::test]
    async fn test_connector_system_performance() {
        use std::time::Instant;
        
        let mut connector_manager = ConnectorManager::new();
        
        // Test connector creation performance
        let start = Instant::now();
        for _ in 0..100 {
            let _connector = connector_manager.get_connector(ConnectorType::Docker).unwrap();
        }
        let duration = start.elapsed();
        
        // Should be very fast due to caching
        assert!(duration.as_millis() < 100, "Connector retrieval too slow: {:?}", duration);
        
        // Verify only one connector was actually created (cached)
        let stats = connector_manager.get_stats();
        assert_eq!(stats.get("connectors_cached"), Some(&1));
    }

    #[tokio::test]
    async fn test_connector_system_memory_usage() {
        let mut connector_manager = ConnectorManager::new();
        
        // Create all connector types
        let connector_types = vec![
            ConnectorType::Docker,
            ConnectorType::Kubernetes,
            ConnectorType::AWS,
            ConnectorType::Azure,
            ConnectorType::GCP,
            ConnectorType::GitHub,
            ConnectorType::GitLab,
        ];
        
        for connector_type in &connector_types {
            let _connector = connector_manager.get_connector(connector_type.clone()).unwrap();
        }
        
        // Verify all connectors are cached
        let stats = connector_manager.get_stats();
        assert_eq!(stats.get("connectors_cached"), Some(&connector_types.len()));
        
        // Clear cache and verify memory is freed
        connector_manager.clear_cache();
        let stats_after_clear = connector_manager.get_stats();
        assert_eq!(stats_after_clear.get("connectors_cached"), Some(&0));
    }
}