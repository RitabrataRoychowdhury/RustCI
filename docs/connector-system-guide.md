# Connector System Guide

## Overview

The Connector System is a modular, extensible architecture for executing CI/CD steps across different platforms and services. It implements the Strategy, Factory, and Facade design patterns to provide a clean, testable, and maintainable solution for CI/CD pipeline execution.

## Architecture

### Design Patterns

1. **Strategy Pattern**: Each connector implements the `Connector` trait, allowing runtime selection of deployment strategies
2. **Factory Pattern**: `ConnectorFactory` handles the creation and configuration of connector instances
3. **Facade Pattern**: `ConnectorManager` provides a simplified interface for connector operations

### Core Components

```
src/ci/connectors/
├── mod.rs                    # Public API and re-exports
├── manager.rs                # ConnectorManager (Facade)
├── factory.rs                # ConnectorFactory (Factory Pattern)
├── traits.rs                 # Core traits and types
├── docker/
│   ├── mod.rs
│   └── connector.rs          # DockerConnector implementation
├── kubernetes/
│   ├── mod.rs
│   ├── connector.rs          # KubernetesConnector implementation
│   ├── job_manager.rs        # K8s job lifecycle management
│   ├── yaml_generator.rs     # YAML generation utilities
│   ├── validation.rs         # K8s-specific validation
│   └── lifecycle_hooks.rs    # MongoDB integration hooks
├── cloud/
│   ├── mod.rs
│   ├── aws.rs               # AWSConnector implementation
│   ├── azure.rs             # AzureConnector implementation
│   └── gcp.rs               # GCPConnector implementation
└── git/
    ├── mod.rs
    ├── github.rs            # GitHubConnector implementation
    └── gitlab.rs            # GitLabConnector implementation
```

## Core Traits and Types

### Connector Trait

All connectors implement the `Connector` trait:

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult>;

    fn connector_type(&self) -> ConnectorType;
    fn name(&self) -> &str;
    fn validate_config(&self, step: &Step) -> Result<()>;
    
    // Optional hooks
    async fn pre_execute(&self, step: &Step) -> Result<()>;
    async fn post_execute(&self, step: &Step, result: &ExecutionResult) -> Result<()>;
}
```

### ConnectorType Enum

```rust
pub enum ConnectorType {
    Docker,
    Kubernetes,
    AWS,
    Azure,
    GCP,
    GitHub,
    GitLab,
    Custom(String),
}
```

### ExecutionResult

```rust
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub metadata: HashMap<String, String>,
}
```

## Usage

### Basic Usage

```rust
use crate::ci::connectors::{ConnectorManager, ConnectorType};

// Create connector manager
let mut manager = ConnectorManager::new();

// Get a connector
let connector = manager.get_connector(ConnectorType::Docker)?;

// Execute a step
let result = connector.execute_step(&step, &workspace, &env).await?;
```

### Advanced Usage with Custom Factory

```rust
use crate::ci::connectors::{ConnectorManager, ConnectorFactory};

// Create custom factory
struct MyCustomFactory;

impl ConnectorFactory for MyCustomFactory {
    fn create_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>> {
        // Custom connector creation logic
    }
    
    fn supports_type(&self, connector_type: &ConnectorType) -> bool {
        // Define supported types
    }
    
    fn name(&self) -> &str {
        "my-custom-factory"
    }
}

// Register custom factory
let mut manager = ConnectorManager::new();
manager.register_factory(Arc::new(MyCustomFactory));
```

## Connector Implementations

### Docker Connector

The Docker connector handles containerized execution:

- **Features**:
  - Container execution with custom images
  - Docker image building from Dockerfiles
  - Volume mounting and environment variable injection
  - Proper cleanup and error handling

- **Configuration**:
```yaml
step_type: docker
config:
  image: "node:18-alpine"
  command: "npm ci && npm run build"
  dockerfile: "Dockerfile.build"  # Optional
  build_context: "."             # Optional
  working_directory: "/app"      # Optional
  environment:
    NODE_ENV: "production"
```

### Kubernetes Connector

The Kubernetes connector provides advanced orchestration:

- **Features**:
  - Job creation and lifecycle management
  - YAML generation and validation
  - Log collection and monitoring
  - Resource cleanup and error handling
  - PVC support for cloud compatibility
  - Lifecycle hooks with MongoDB integration
  - RBAC and security context support

- **Configuration**:
```yaml
step_type: kubernetes
config:
  image: "ubuntu:latest"
  command: "echo 'Deploying application'"
  namespace: "production"
  parameters:
    use_pvc: true
    storage_class: "fast-ssd"
    storage_size: "10Gi"
    resource_requests:
      cpu: "100m"
      memory: "128Mi"
    resource_limits:
      cpu: "500m"
      memory: "512Mi"
    service_account: "ci-service-account"
```

### Cloud Connectors

Cloud connectors provide integration with major cloud providers:

#### AWS Connector
```yaml
step_type: aws
config:
  service: "ecs"
  action: "update-service"
  parameters:
    cluster: "production"
    service: "web-service"
    task_definition: "web-app:latest"
```

#### Azure Connector
```yaml
step_type: azure
config:
  service: "container-instances"
  action: "create-or-update"
  parameters:
    resource_group: "production-rg"
    container_group: "web-app"
    image: "myregistry.azurecr.io/web-app:latest"
```

#### GCP Connector
```yaml
step_type: gcp
config:
  service: "cloud-run"
  action: "deploy"
  parameters:
    service_name: "web-service"
    image: "gcr.io/project/web-app:latest"
    region: "us-central1"
```

### Git Connectors

Git connectors handle repository operations:

#### GitHub Connector
```yaml
step_type: github
config:
  action: "create-release"
  repository: "user/app"
  tag: "v1.0.0"
  name: "Release v1.0.0"
  body: "Automated release from CI/CD pipeline"
```

#### GitLab Connector
```yaml
step_type: gitlab
config:
  action: "mirror"
  source_repo: "https://github.com/user/app"
  target_repo: "https://gitlab.com/user/app"
  branch: "main"
```

## Testing

### Unit Tests

The connector system includes comprehensive unit tests:

```bash
# Run all connector tests
cargo test connectors

# Run specific test
cargo test test_connector_manager_creation
```

### Integration Tests

Integration tests verify connector system integration:

```bash
# Run integration tests
cargo test integration_tests
```

### Mock Implementations

Mock implementations are provided for testing:

```rust
use crate::ci::connectors::tests::mocks::{MockConnector, MockKubernetesClient, MockDockerClient};

// Create mock connector
let mock_connector = MockConnector::new("test-mock".to_string(), ConnectorType::Docker);
mock_connector.set_next_result(ExecutionResult::success("mock success".to_string()));
```

## Error Handling

### Error Types

The connector system uses structured error handling:

```rust
pub enum AppError {
    ConnectorConfigError(String),
    KubernetesError(String),
    PermissionDenied(String),
    ValidationError(String),
    ExternalServiceError(String),
    Unimplemented(String),
    // ... other error types
}
```

### Error Context

Connectors provide detailed error context:
- Operation being performed
- Configuration used
- External service responses
- Suggested remediation steps

## Performance Considerations

### Caching

- Connectors are cached after first creation
- Cache can be cleared when needed
- Lazy loading reduces startup time

### Resource Management

- Proper cleanup of temporary resources
- Connection pooling for external APIs
- Memory-efficient operations

## Security

### Credential Management

- Secure handling of authentication tokens
- Environment variable sanitization
- Audit logging for sensitive operations

### Resource Isolation

- Proper namespace isolation for Kubernetes
- Container security contexts
- Network policy enforcement

### Input Validation

- Comprehensive validation of all inputs
- Sanitization of user-provided commands
- Protection against injection attacks

## Extending the System

### Creating Custom Connectors

1. Implement the `Connector` trait:

```rust
pub struct MyCustomConnector;

#[async_trait]
impl Connector for MyCustomConnector {
    async fn execute_step(
        &self,
        step: &Step,
        workspace: &Workspace,
        env: &HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        // Implementation
    }
    
    fn connector_type(&self) -> ConnectorType {
        ConnectorType::Custom("my-connector".to_string())
    }
    
    fn name(&self) -> &str {
        "my-connector"
    }
}
```

2. Create a custom factory:

```rust
pub struct MyConnectorFactory;

impl ConnectorFactory for MyConnectorFactory {
    fn create_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>> {
        match connector_type {
            ConnectorType::Custom(name) if name == "my-connector" => {
                Ok(Arc::new(MyCustomConnector))
            }
            _ => Err(AppError::Unimplemented(format!("Unsupported type: {}", connector_type)))
        }
    }
    
    fn supports_type(&self, connector_type: &ConnectorType) -> bool {
        matches!(connector_type, ConnectorType::Custom(name) if name == "my-connector")
    }
    
    fn name(&self) -> &str {
        "my-connector-factory"
    }
}
```

3. Register the factory:

```rust
let mut manager = ConnectorManager::new();
manager.register_factory(Arc::new(MyConnectorFactory));
```

## Best Practices

### Configuration

- Use environment variables for sensitive data
- Validate all configuration before execution
- Provide sensible defaults

### Error Handling

- Always provide meaningful error messages
- Include context about what operation failed
- Suggest remediation steps when possible

### Logging

- Use structured logging with appropriate levels
- Include correlation IDs for tracing
- Log both successes and failures

### Testing

- Write comprehensive unit tests
- Use mock implementations for external dependencies
- Test error conditions and edge cases

### Performance

- Cache expensive operations
- Use connection pooling for external services
- Clean up resources promptly

## Troubleshooting

### Common Issues

1. **Connector Not Found**
   - Ensure the connector type is supported
   - Check if custom factories are registered

2. **Configuration Validation Errors**
   - Verify all required fields are provided
   - Check field formats and constraints

3. **External Service Errors**
   - Verify credentials and permissions
   - Check network connectivity
   - Review service-specific logs

### Debugging

- Enable debug logging: `RUST_LOG=debug`
- Check connector manager stats
- Review execution metadata

### Monitoring

- Monitor connector execution times
- Track success/failure rates
- Alert on unusual error patterns

## API Reference

### ConnectorManager

```rust
impl ConnectorManager {
    pub fn new() -> Self;
    pub fn register_factory(&mut self, factory: Arc<dyn ConnectorFactory>);
    pub fn get_connector(&mut self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>>;
    pub async fn execute_step(&mut self, step: &Step, workspace: &Workspace, env: &HashMap<String, String>) -> Result<ExecutionResult>;
    pub fn determine_connector_type(&self, step: &Step) -> Result<ConnectorType>;
    pub fn list_available_connectors(&self) -> Vec<ConnectorType>;
    pub fn get_stats(&self) -> HashMap<String, usize>;
    pub fn clear_cache(&mut self);
}
```

### ExecutionResult

```rust
impl ExecutionResult {
    pub fn new(exit_code: i32, stdout: String, stderr: String) -> Self;
    pub fn with_metadata(self, key: String, value: String) -> Self;
    pub fn success(stdout: String) -> Self;
    pub fn failure(exit_code: i32, stderr: String) -> Self;
    pub fn is_success(&self) -> bool;
}
```

## Version Information

- **Connector System Version**: 1.0.0
- **Default Timeout**: 300 seconds
- **Supported Rust Version**: 1.70+

## Contributing

When contributing to the connector system:

1. Follow the established patterns
2. Add comprehensive tests
3. Update documentation
4. Ensure backward compatibility
5. Add appropriate error handling

## License

This connector system is part of the RustAutoDevOps project and follows the same license terms.