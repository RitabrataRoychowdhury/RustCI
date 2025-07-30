#[cfg(test)]
mod tests {
    use crate::ci::connectors::{
        ConnectorManager, BuiltInConnectorFactory, ConnectorType, ExecutionResult, 
        DockerConnector, KubernetesConnector, ConnectorFactory, 
        traits::{Connector, KubernetesConfig, LifecycleHook, LifecycleHookType, MongoOperation}
    };
    use crate::ci::config::{Step, StepConfig, StepType};

    #[tokio::test]
    async fn test_connector_manager_creation() {
        let manager = ConnectorManager::new();
        let stats = manager.get_stats();
        
        assert_eq!(stats.get("factories_registered"), Some(&1));
        assert_eq!(stats.get("connectors_cached"), Some(&0));
    }

    #[tokio::test]
    async fn test_built_in_factory_supports_types() {
        let factory = BuiltInConnectorFactory::new();
        
        assert!(factory.supports_type(&ConnectorType::Docker));
        assert!(factory.supports_type(&ConnectorType::Kubernetes));
        assert!(factory.supports_type(&ConnectorType::AWS));
        assert!(factory.supports_type(&ConnectorType::Azure));
        assert!(factory.supports_type(&ConnectorType::GCP));
        assert!(factory.supports_type(&ConnectorType::GitHub));
        assert!(factory.supports_type(&ConnectorType::GitLab));
        assert!(!factory.supports_type(&ConnectorType::Custom("test".to_string())));
    }

    #[tokio::test]
    async fn test_connector_creation() {
        let factory = BuiltInConnectorFactory::new();
        
        // Test successful connector creation
        let docker_connector = factory.create_connector(ConnectorType::Docker);
        assert!(docker_connector.is_ok());
        assert_eq!(docker_connector.unwrap().name(), "docker");
        
        // Test custom connector returns error
        let custom_connector = factory.create_connector(ConnectorType::Custom("test".to_string()));
        assert!(custom_connector.is_err());
    }

    #[tokio::test]
    async fn test_connector_manager_get_connector() {
        let mut manager = ConnectorManager::new();
        
        // Test getting a connector
        let connector = manager.get_connector(ConnectorType::Docker);
        assert!(connector.is_ok());
        assert_eq!(connector.unwrap().name(), "docker");
        
        // Check that connector is cached
        let stats = manager.get_stats();
        assert_eq!(stats.get("connectors_cached"), Some(&1));
    }

    #[tokio::test]
    async fn test_execution_result() {
        let result = ExecutionResult::new(0, "success".to_string(), "".to_string());
        assert!(result.is_success());
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "success");
        
        let failure = ExecutionResult::failure(1, "error".to_string());
        assert!(!failure.is_success());
        assert_eq!(failure.exit_code, 1);
        assert_eq!(failure.stderr, "error");
        
        let success = ExecutionResult::success("output".to_string());
        assert!(success.is_success());
        assert_eq!(success.stdout, "output");
    }

    #[tokio::test]
    async fn test_execution_result_with_metadata() {
        let result = ExecutionResult::new(0, "output".to_string(), "".to_string())
            .with_metadata("key".to_string(), "value".to_string());
        
        assert_eq!(result.metadata.get("key"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_connector_type_display() {
        assert_eq!(ConnectorType::Docker.to_string(), "docker");
        assert_eq!(ConnectorType::Kubernetes.to_string(), "kubernetes");
        assert_eq!(ConnectorType::AWS.to_string(), "aws");
        assert_eq!(ConnectorType::Azure.to_string(), "azure");
        assert_eq!(ConnectorType::GCP.to_string(), "gcp");
        assert_eq!(ConnectorType::GitHub.to_string(), "github");
        assert_eq!(ConnectorType::GitLab.to_string(), "gitlab");
        assert_eq!(ConnectorType::Custom("test".to_string()).to_string(), "test");
    }

    #[tokio::test]
    async fn test_connector_manager_determine_connector_type() {
        let manager = ConnectorManager::new();
        
        // Create test steps for different types
        let docker_step = Step {
            name: "test".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                image: Some("alpine".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let k8s_step = Step {
            name: "test".to_string(),
            step_type: StepType::Kubernetes,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let shell_step = Step {
            name: "test".to_string(),
            step_type: StepType::Shell,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        // Test connector type determination
        assert_eq!(manager.determine_connector_type(&docker_step).unwrap(), ConnectorType::Docker);
        assert_eq!(manager.determine_connector_type(&k8s_step).unwrap(), ConnectorType::Kubernetes);
        assert_eq!(manager.determine_connector_type(&shell_step).unwrap(), ConnectorType::Docker); // Shell defaults to Docker
    }

    #[tokio::test]
    async fn test_connector_manager_list_available_connectors() {
        let manager = ConnectorManager::new();
        let connectors = manager.list_available_connectors();
        
        assert!(connectors.contains(&ConnectorType::Docker));
        assert!(connectors.contains(&ConnectorType::Kubernetes));
        assert!(connectors.contains(&ConnectorType::AWS));
        assert!(connectors.contains(&ConnectorType::Azure));
        assert!(connectors.contains(&ConnectorType::GCP));
        assert!(connectors.contains(&ConnectorType::GitHub));
        assert!(connectors.contains(&ConnectorType::GitLab));
    }

    #[tokio::test]
    async fn test_connector_validation() {
        let connector = DockerConnector::new();
        
        // Test valid step
        let valid_step = Step {
            name: "test".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                command: Some("echo hello".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        assert!(connector.validate_config(&valid_step).is_ok());
        
        // Test invalid step (empty name)
        let invalid_step = Step {
            name: "".to_string(),
            step_type: StepType::Docker,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        assert!(connector.validate_config(&invalid_step).is_err());
    }

    #[tokio::test]
    async fn test_kubernetes_config_validation() {
        let valid_config = KubernetesConfig {
            namespace: "default".to_string(),
            timeout_seconds: 180,
            use_hostpath: true,
            resource_requests: None,
            resource_limits: None,
            repo_url: None,
            use_pvc: false,
            storage_class: None,
            storage_size: Some("10Gi".to_string()),
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            service_account: None,
            security_context: None,
        };
        
        assert!(valid_config.validate().is_ok());
        
        let invalid_config = KubernetesConfig {
            namespace: "".to_string(),
            timeout_seconds: 0,
            use_hostpath: true,
            resource_requests: None,
            resource_limits: None,
            repo_url: None,
            use_pvc: false,
            storage_class: None,
            storage_size: Some("10Gi".to_string()),
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            service_account: None,
            security_context: None,
        };
        
        assert!(invalid_config.validate().is_err());
    }

    #[tokio::test]
    async fn test_connector_manager_dynamic_factory_registration() {
        let mut manager = ConnectorManager::new();
        
        // Initially should have 1 factory (built-in)
        let initial_stats = manager.get_stats();
        assert_eq!(initial_stats.get("factories_registered"), Some(&1));
        
        // Register a custom factory
        let custom_factory = Arc::new(TestConnectorFactory::new());
        manager.register_factory(custom_factory);
        
        // Should now have 2 factories
        let updated_stats = manager.get_stats();
        assert_eq!(updated_stats.get("factories_registered"), Some(&2));
    }

    #[tokio::test]
    async fn test_connector_manager_factory_priority() {
        let mut manager = ConnectorManager::new();
        
        // Register a custom factory that supports Docker
        let custom_factory = Arc::new(TestConnectorFactory::new());
        manager.register_factory(custom_factory);
        
        // Get a Docker connector - should use the first factory that supports it
        let connector = manager.get_connector(ConnectorType::Docker);
        assert!(connector.is_ok());
        
        // The connector name should indicate which factory was used
        let connector = connector.unwrap();
        assert!(connector.name() == "docker" || connector.name() == "test-docker");
    }

    #[tokio::test]
    async fn test_connector_manager_caching_behavior() {
        let mut manager = ConnectorManager::new();
        
        // Get the same connector type multiple times
        let connector1 = manager.get_connector(ConnectorType::Docker).unwrap();
        let connector2 = manager.get_connector(ConnectorType::Docker).unwrap();
        
        // Should be the same instance (Arc cloning)
        assert_eq!(Arc::as_ptr(&connector1), Arc::as_ptr(&connector2));
        
        // Cache should contain only 1 connector
        let stats = manager.get_stats();
        assert_eq!(stats.get("connectors_cached"), Some(&1));
    }

    #[tokio::test]
    async fn test_connector_manager_unsupported_type() {
        let mut manager = ConnectorManager::new();
        
        // Try to get a custom connector that no factory supports
        let result = manager.get_connector(ConnectorType::Custom("unsupported".to_string()));
        assert!(result.is_err());
        
        // Should be a NotFound error
        assert!(result.is_err());
        if let Err(crate::error::AppError::NotFound(_)) = result {
            // Expected error type
        } else {
            panic!("Expected NotFound error");
        }
    }

    #[tokio::test]
    async fn test_connector_manager_custom_step_validation() {
        let manager = ConnectorManager::new();
        
        // Test custom step with plugin name
        let custom_step_with_plugin = Step {
            name: "test".to_string(),
            step_type: StepType::Custom,
            config: StepConfig {
                plugin_name: Some("my-plugin".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let result = manager.determine_connector_type(&custom_step_with_plugin);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConnectorType::Custom("my-plugin".to_string()));
        
        // Test custom step without plugin name
        let custom_step_without_plugin = Step {
            name: "test".to_string(),
            step_type: StepType::Custom,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let result = manager.determine_connector_type(&custom_step_without_plugin);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_built_in_factory_name() {
        let factory = BuiltInConnectorFactory::new();
        assert_eq!(factory.name(), "built-in");
    }

    #[tokio::test]
    async fn test_connector_manager_default_creation() {
        let manager = ConnectorManager::default();
        let stats = manager.get_stats();
        
        // Should have the built-in factory registered by default
        assert_eq!(stats.get("factories_registered"), Some(&1));
        assert_eq!(stats.get("connectors_cached"), Some(&0));
    }

    // Test helper: Custom factory for testing dynamic registration
    use std::sync::Arc;
    use crate::error::{AppError, Result};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use crate::ci::workspace::Workspace;

    struct TestConnectorFactory;

    impl TestConnectorFactory {
        fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl ConnectorFactory for TestConnectorFactory {
        fn create_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>> {
            match connector_type {
                ConnectorType::Docker => Ok(Arc::new(TestDockerConnector)),
                _ => Err(AppError::Unimplemented(format!("Test factory doesn't support: {}", connector_type))),
            }
        }

        fn supports_type(&self, connector_type: &ConnectorType) -> bool {
            matches!(connector_type, ConnectorType::Docker)
        }

        fn name(&self) -> &str {
            "test-factory"
        }
    }

    struct TestDockerConnector;

    #[async_trait]
    impl Connector for TestDockerConnector {
        async fn execute_step(
            &self,
            _step: &Step,
            _workspace: &Workspace,
            _env: &HashMap<String, String>,
        ) -> Result<ExecutionResult> {
            Ok(ExecutionResult::success("test output".to_string()))
        }

        fn connector_type(&self) -> ConnectorType {
            ConnectorType::Docker
        }

        fn name(&self) -> &str {
            "test-docker"
        }
    }

    // Additional comprehensive tests for all connectors
    
    #[tokio::test]
    async fn test_kubernetes_connector_creation() {
        let connector = KubernetesConnector::new();
        assert_eq!(connector.name(), "kubernetes");
        assert_eq!(connector.connector_type(), ConnectorType::Kubernetes);
    }

    #[tokio::test]
    async fn test_kubernetes_connector_validation() {
        let connector = KubernetesConnector::new();
        
        // Test valid Kubernetes step
        let valid_step = Step {
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
            timeout: Some(60),
        };
        
        assert!(connector.validate_config(&valid_step).is_ok());
        
        // Test invalid step (missing image)
        let invalid_step = Step {
            name: "test-k8s".to_string(),
            step_type: StepType::Kubernetes,
            config: StepConfig {
                command: Some("echo hello".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: Some(60),
        };
        
        assert!(connector.validate_config(&invalid_step).is_err());
    }

    #[tokio::test]
    async fn test_kubernetes_config_resource_validation() {
        use std::collections::HashMap;
        
        let mut resource_requests = HashMap::new();
        resource_requests.insert("cpu".to_string(), "100m".to_string());
        resource_requests.insert("memory".to_string(), "128Mi".to_string());
        
        let mut resource_limits = HashMap::new();
        resource_limits.insert("cpu".to_string(), "500m".to_string());
        resource_limits.insert("memory".to_string(), "512Mi".to_string());
        
        let config = KubernetesConfig {
            namespace: "test-namespace".to_string(),
            timeout_seconds: 300,
            use_hostpath: false,
            resource_requests: Some(resource_requests),
            resource_limits: Some(resource_limits),
            repo_url: Some("https://github.com/test/repo".to_string()),
            use_pvc: true,
            storage_class: Some("fast-ssd".to_string()),
            storage_size: Some("20Gi".to_string()),
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            service_account: Some("ci-service-account".to_string()),
            security_context: None,
        };
        
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_kubernetes_config_invalid_resources() {
        use std::collections::HashMap;
        
        let mut invalid_resource_requests = HashMap::new();
        invalid_resource_requests.insert("cpu".to_string(), "invalid-cpu".to_string());
        
        let config = KubernetesConfig {
            namespace: "test-namespace".to_string(),
            timeout_seconds: 300,
            use_hostpath: false,
            resource_requests: Some(invalid_resource_requests),
            resource_limits: None,
            repo_url: None,
            use_pvc: false,
            storage_class: None,
            storage_size: Some("10Gi".to_string()),
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            service_account: None,
            security_context: None,
        };
        
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_kubernetes_config_lifecycle_hooks() {
        let pre_hook = LifecycleHook {
            name: "pre-execution-hook".to_string(),
            hook_type: LifecycleHookType::PreExecution,
            mongodb_collection: "execution_logs".to_string(),
            mongodb_operation: MongoOperation::Insert,
            data: {
                let mut data = HashMap::new();
                data.insert("status".to_string(), "starting".to_string());
                data
            },
        };
        
        let post_hook = LifecycleHook {
            name: "post-execution-hook".to_string(),
            hook_type: LifecycleHookType::PostExecution,
            mongodb_collection: "execution_logs".to_string(),
            mongodb_operation: MongoOperation::Update,
            data: {
                let mut data = HashMap::new();
                data.insert("status".to_string(), "completed".to_string());
                data
            },
        };
        
        let config = KubernetesConfig {
            namespace: "default".to_string(),
            timeout_seconds: 180,
            use_hostpath: true,
            resource_requests: None,
            resource_limits: None,
            repo_url: None,
            use_pvc: false,
            storage_class: None,
            storage_size: Some("10Gi".to_string()),
            pre_hooks: vec![pre_hook],
            post_hooks: vec![post_hook],
            service_account: None,
            security_context: None,
        };
        
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_kubernetes_config_invalid_lifecycle_hook() {
        let invalid_hook = LifecycleHook {
            name: "".to_string(), // Empty name should fail validation
            hook_type: LifecycleHookType::PreExecution,
            mongodb_collection: "execution_logs".to_string(),
            mongodb_operation: MongoOperation::Insert,
            data: HashMap::new(),
        };
        
        let config = KubernetesConfig {
            namespace: "default".to_string(),
            timeout_seconds: 180,
            use_hostpath: true,
            resource_requests: None,
            resource_limits: None,
            repo_url: None,
            use_pvc: false,
            storage_class: None,
            storage_size: Some("10Gi".to_string()),
            pre_hooks: vec![invalid_hook],
            post_hooks: Vec::new(),
            service_account: None,
            security_context: None,
        };
        
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_docker_connector_validation_edge_cases() {
        let connector = DockerConnector::new();
        
        // Test step with dockerfile but empty path
        let step_with_empty_dockerfile = Step {
            name: "test".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                command: Some("echo hello".to_string()),
                dockerfile: Some("".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        assert!(connector.validate_config(&step_with_empty_dockerfile).is_err());
        
        // Test step with empty image name
        let step_with_empty_image = Step {
            name: "test".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                command: Some("echo hello".to_string()),
                image: Some("".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        assert!(connector.validate_config(&step_with_empty_image).is_err());
        
        // Test step with image containing spaces
        let step_with_invalid_image = Step {
            name: "test".to_string(),
            step_type: StepType::Docker,
            config: StepConfig {
                command: Some("echo hello".to_string()),
                image: Some("invalid image name".to_string()),
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        assert!(connector.validate_config(&step_with_invalid_image).is_err());
    }

    #[tokio::test]
    async fn test_all_connector_types_creation() {
        let factory = BuiltInConnectorFactory::new();
        
        // Test all supported connector types
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
            let connector = factory.create_connector(connector_type.clone());
            assert!(connector.is_ok(), "Failed to create connector: {}", connector_type);
            
            let connector = connector.unwrap();
            assert_eq!(connector.connector_type(), connector_type);
            assert!(!connector.name().is_empty());
        }
    }

    #[tokio::test]
    async fn test_connector_manager_cache_clearing() {
        let mut manager = ConnectorManager::new();
        
        // Get a connector to populate cache
        let _connector = manager.get_connector(ConnectorType::Docker).unwrap();
        assert_eq!(manager.get_stats().get("connectors_cached"), Some(&1));
        
        // Clear cache
        manager.clear_cache();
        assert_eq!(manager.get_stats().get("connectors_cached"), Some(&0));
    }

    #[tokio::test]
    async fn test_connector_manager_factory_methods() {
        let mut manager = ConnectorManager::new();
        
        // Test factory count
        assert_eq!(manager.factory_count(), 1);
        
        // Test has_factory
        assert!(manager.has_factory("built-in"));
        assert!(!manager.has_factory("non-existent"));
        
        // Register another factory
        let custom_factory = Arc::new(TestConnectorFactory::new());
        manager.register_factory(custom_factory);
        
        assert_eq!(manager.factory_count(), 2);
        assert!(manager.has_factory("test-factory"));
    }

    #[tokio::test]
    async fn test_execution_result_metadata_operations() {
        let mut result = ExecutionResult::new(0, "output".to_string(), "".to_string());
        
        // Test multiple metadata additions
        result = result
            .with_metadata("key1".to_string(), "value1".to_string())
            .with_metadata("key2".to_string(), "value2".to_string())
            .with_metadata("key3".to_string(), "value3".to_string());
        
        assert_eq!(result.metadata.len(), 3);
        assert_eq!(result.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(result.metadata.get("key2"), Some(&"value2".to_string()));
        assert_eq!(result.metadata.get("key3"), Some(&"value3".to_string()));
    }

    #[tokio::test]
    async fn test_kubernetes_config_from_step() {
        use serde_json::json;
        
        let step = Step {
            name: "test-k8s-step".to_string(),
            step_type: StepType::Kubernetes,
            config: StepConfig {
                image: Some("ubuntu:latest".to_string()),
                command: Some("echo hello".to_string()),
                namespace: Some("custom-namespace".to_string()),
                repository_url: Some("https://github.com/test/repo".to_string()),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("resource_requests".to_string(), json!({
                        "cpu": "100m",
                        "memory": "128Mi"
                    }));
                    params.insert("resource_limits".to_string(), json!({
                        "cpu": "500m",
                        "memory": "512Mi"
                    }));
                    params.insert("use_pvc".to_string(), json!(true));
                    params.insert("storage_class".to_string(), json!("fast-ssd"));
                    params.insert("storage_size".to_string(), json!("20Gi"));
                    params.insert("service_account".to_string(), json!("ci-service-account"));
                    Some(params)
                },
                ..Default::default()
            },
            condition: None,
            continue_on_error: None,
            timeout: Some(300),
        };
        
        let config = KubernetesConfig::from_step(&step);
        
        assert_eq!(config.namespace, "custom-namespace");
        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.repo_url, Some("https://github.com/test/repo".to_string()));
        assert!(config.use_pvc);
        assert_eq!(config.storage_class, Some("fast-ssd".to_string()));
        assert_eq!(config.storage_size, Some("20Gi".to_string()));
        assert_eq!(config.service_account, Some("ci-service-account".to_string()));
        
        // Check resource requests and limits
        assert!(config.resource_requests.is_some());
        assert!(config.resource_limits.is_some());
        
        let requests = config.resource_requests.unwrap();
        assert_eq!(requests.get("cpu"), Some(&"100m".to_string()));
        assert_eq!(requests.get("memory"), Some(&"128Mi".to_string()));
        
        let limits = config.resource_limits.unwrap();
        assert_eq!(limits.get("cpu"), Some(&"500m".to_string()));
        assert_eq!(limits.get("memory"), Some(&"512Mi".to_string()));
    }

    // Integration tests for connector manager with multiple connectors
    #[tokio::test]
    async fn test_connector_manager_integration() {
        let mut manager = ConnectorManager::new();
        
        // Test getting different connector types
        let docker_connector = manager.get_connector(ConnectorType::Docker).unwrap();
        let k8s_connector = manager.get_connector(ConnectorType::Kubernetes).unwrap();
        let aws_connector = manager.get_connector(ConnectorType::AWS).unwrap();
        
        // Verify they are different connectors
        assert_eq!(docker_connector.name(), "docker");
        assert_eq!(k8s_connector.name(), "kubernetes");
        assert_eq!(aws_connector.name(), "aws");
        
        // Verify cache contains all three
        let stats = manager.get_stats();
        assert_eq!(stats.get("connectors_cached"), Some(&3));
        
        // Test that getting the same connector returns cached instance
        let docker_connector2 = manager.get_connector(ConnectorType::Docker).unwrap();
        assert_eq!(Arc::as_ptr(&docker_connector), Arc::as_ptr(&docker_connector2));
    }

    // Mock implementations for external dependencies
    pub mod mocks {
        use super::*;
        use std::sync::{Arc, Mutex};
        
        pub struct MockKubernetesClient {
            pub job_submissions: Arc<Mutex<Vec<String>>>,
            pub job_status_responses: Arc<Mutex<Vec<bool>>>,
        }
        
        impl MockKubernetesClient {
            pub fn new() -> Self {
                Self {
                    job_submissions: Arc::new(Mutex::new(Vec::new())),
                    job_status_responses: Arc::new(Mutex::new(vec![true])), // Default to success
                }
            }
            
            pub fn submit_job(&self, job_yaml: &str) -> Result<String> {
                let job_name = format!("mock-job-{}", uuid::Uuid::new_v4());
                self.job_submissions.lock().unwrap().push(job_yaml.to_string());
                Ok(job_name)
            }
            
            pub fn get_job_status(&self, _job_name: &str) -> Result<bool> {
                let mut responses = self.job_status_responses.lock().unwrap();
                Ok(responses.pop().unwrap_or(true))
            }
            
            pub fn get_submitted_jobs(&self) -> Vec<String> {
                self.job_submissions.lock().unwrap().clone()
            }
        }
        
        pub struct MockDockerClient {
            pub executed_commands: Arc<Mutex<Vec<String>>>,
            pub command_results: Arc<Mutex<Vec<(i32, String, String)>>>,
        }
        
        impl MockDockerClient {
            pub fn new() -> Self {
                Self {
                    executed_commands: Arc::new(Mutex::new(Vec::new())),
                    command_results: Arc::new(Mutex::new(vec![(0, "success".to_string(), "".to_string())])),
                }
            }
            
            pub fn execute_command(&self, command: &str) -> Result<(i32, String, String)> {
                self.executed_commands.lock().unwrap().push(command.to_string());
                let mut results = self.command_results.lock().unwrap();
                Ok(results.pop().unwrap_or((0, "default success".to_string(), "".to_string())))
            }
            
            pub fn get_executed_commands(&self) -> Vec<String> {
                self.executed_commands.lock().unwrap().clone()
            }
            
            pub fn set_next_result(&self, exit_code: i32, stdout: String, stderr: String) {
                self.command_results.lock().unwrap().push((exit_code, stdout, stderr));
            }
        }
        
        // Mock connector for testing
        pub struct MockConnector {
            pub name: String,
            pub connector_type: ConnectorType,
            pub execution_results: Arc<Mutex<Vec<ExecutionResult>>>,
        }
        
        impl MockConnector {
            pub fn new(name: String, connector_type: ConnectorType) -> Self {
                Self {
                    name,
                    connector_type,
                    execution_results: Arc::new(Mutex::new(vec![ExecutionResult::success("mock success".to_string())])),
                }
            }
            
            pub fn set_next_result(&self, result: ExecutionResult) {
                self.execution_results.lock().unwrap().push(result);
            }
        }
        
        #[async_trait]
        impl Connector for MockConnector {
            async fn execute_step(
                &self,
                _step: &Step,
                _workspace: &Workspace,
                _env: &HashMap<String, String>,
            ) -> Result<ExecutionResult> {
                let mut results = self.execution_results.lock().unwrap();
                Ok(results.pop().unwrap_or_else(|| ExecutionResult::success("default mock success".to_string())))
            }
            
            fn connector_type(&self) -> ConnectorType {
                self.connector_type.clone()
            }
            
            fn name(&self) -> &str {
                &self.name
            }
        }
    }
    
    #[tokio::test]
    async fn test_mock_implementations() {
        use mocks::*;
        
        // Test MockKubernetesClient
        let k8s_client = MockKubernetesClient::new();
        let job_name = k8s_client.submit_job("apiVersion: batch/v1\nkind: Job").unwrap();
        assert!(job_name.starts_with("mock-job-"));
        assert_eq!(k8s_client.get_submitted_jobs().len(), 1);
        assert!(k8s_client.get_job_status(&job_name).unwrap());
        
        // Test MockDockerClient
        let docker_client = MockDockerClient::new();
        docker_client.set_next_result(0, "test output".to_string(), "".to_string());
        let (exit_code, stdout, stderr) = docker_client.execute_command("echo hello").unwrap();
        assert_eq!(exit_code, 0);
        assert_eq!(stdout, "test output");
        assert_eq!(stderr, "");
        assert_eq!(docker_client.get_executed_commands(), vec!["echo hello"]);
        
        // Test MockConnector
        let mock_connector = MockConnector::new("test-mock".to_string(), ConnectorType::Docker);
        mock_connector.set_next_result(ExecutionResult::failure(1, "mock failure".to_string()));
        
        let step = Step {
            name: "test".to_string(),
            step_type: StepType::Docker,
            config: StepConfig::default(),
            condition: None,
            continue_on_error: None,
            timeout: None,
        };
        
        let workspace = create_test_workspace();
        let env = HashMap::new();
        
        let result = mock_connector.execute_step(&step, &workspace, &env).await.unwrap();
        assert!(!result.is_success());
        assert_eq!(result.stderr, "mock failure");
    }
    
    fn create_test_workspace() -> Workspace {
        use std::path::PathBuf;
        Workspace {
            id: uuid::Uuid::new_v4(),
            path: PathBuf::from("/tmp/test"),
            execution_id: uuid::Uuid::new_v4(),
        }
    }
}