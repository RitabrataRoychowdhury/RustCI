# RustCI Pipeline Examples

This directory contains various pipeline examples demonstrating different use cases and deployment strategies for RustCI.

## Pipeline Structure

All pipelines follow the RustCI YAML format:

```yaml
name: "Pipeline Name"
description: "Pipeline description"
triggers:
  - trigger_type: manual|webhook|scheduled
    config: {}

stages:
  - name: "Stage Name"
    steps:
      - name: "step-name"
        step_type: shell|manual_approval
        config:
          deployment_type: custom  # Optional, prevents auto-detection
          command: |
            # Shell commands here

environment: {}
timeout: 1800
retry_count: 0
```

## Available Pipeline Examples

### Multi-Tier Pipeline System

RustCI supports four progressive pipeline types that automatically adapt to your project's complexity:

#### `minimal-pipeline.yaml` - **Minimal Type**
- **Purpose**: Simplest possible pipeline with auto-detection
- **Required Fields**: Only `repo` and `server`
- **Features**: Auto-detects project type, builds with Docker by default
- **Use Case**: Quick deployments without complex configuration
- **Triggers**: Manual, Webhook
- **Execution**: Auto-detection → Build → Docker Deploy

#### `simple-pipeline.yaml` - **Simple Type**
- **Purpose**: GitHub Actions style linear execution
- **Required Fields**: `steps` array with `run` commands
- **Features**: Sequential step execution, familiar syntax
- **Use Case**: Straightforward CI/CD workflows
- **Triggers**: Manual, Git Push, Pull Request
- **Execution**: Format → Lint → Test → Build → Deploy → Verify

#### `standard-pipeline.yaml` - **Standard Type**
- **Purpose**: GitLab CI / Jenkins style declarative pipelines
- **Required Fields**: `stages` and `jobs` mappings
- **Features**: Organized stages, parallel job execution within stages
- **Use Case**: Enterprise workflows with complex dependencies
- **Triggers**: Manual, Git Push, Scheduled
- **Execution**: Prepare → Build → Test → Security → Deploy → Verify

#### `advanced-pipeline.yaml` - **Advanced Type**
- **Purpose**: Full-featured pipelines with matrix strategies
- **Required Fields**: Advanced features (`matrix`, `variables`, `include`, `cache`)
- **Features**: Matrix builds, includes, variables, caching, multi-platform
- **Use Case**: Complex CI/CD with multiple environments and configurations
- **Triggers**: Manual, Git Push, Pull Request, Scheduled
- **Execution**: Multi-matrix parallel execution across platforms and environments

### 1. Application Deployment

#### `docker-deployment-pipeline.yaml`
- **Purpose**: Deploy test application using Docker
- **Triggers**: Manual
- **Features**: Build, deploy, and verify Docker container
- **Use Case**: Simple containerized application deployment

#### `k3s-deployment-pipeline.yaml`
- **Purpose**: Deploy test application to K3s cluster
- **Triggers**: Manual
- **Features**: Build image, load to K3s, deploy with Kubernetes manifests
- **Use Case**: Kubernetes-based deployment testing

#### `multi-env-deployment-pipeline.yaml`
- **Purpose**: Deploy to staging and production environments
- **Triggers**: Manual
- **Features**: Multi-stage deployment with approval gates
- **Use Case**: Production deployment workflow

### 2. Development and Testing

#### `build-test-pipeline.yaml`
- **Purpose**: Build Rust application and run comprehensive tests
- **Triggers**: Manual, Webhook (push/PR)
- **Features**: Format check, linting, unit tests, integration tests, coverage
- **Use Case**: CI pipeline for code quality assurance

#### `performance-test-pipeline.yaml`
- **Purpose**: Run load tests and performance benchmarks
- **Triggers**: Manual
- **Features**: Load testing with K6, stress testing, memory profiling
- **Use Case**: Performance validation and monitoring

#### `security-scan-pipeline.yaml`
- **Purpose**: Security vulnerability scanning
- **Triggers**: Manual, Scheduled (daily)
- **Features**: Dependency audit, container scanning, secrets detection
- **Use Case**: Security compliance and vulnerability management

### 3. Operations and Maintenance

#### `database-migration-pipeline.yaml`
- **Purpose**: Run database migrations and schema updates
- **Triggers**: Manual
- **Features**: Backup, migration execution, validation, rollback support
- **Use Case**: Database schema management

#### `backup-restore-pipeline.yaml`
- **Purpose**: Database backup and restore operations
- **Triggers**: Manual, Scheduled (daily)
- **Features**: MongoDB backup, compression, cloud storage, restore capability
- **Use Case**: Data protection and disaster recovery

## Pipeline Features

### Multi-Tier Pipeline Types

#### Type Detection
RustCI automatically detects pipeline type based on YAML structure:
- **Minimal**: Contains only `repo` and `server` fields
- **Simple**: Contains `steps` array
- **Standard**: Contains `stages` and `jobs` mappings  
- **Advanced**: Contains `matrix`, `variables`, `include`, or `cache`

#### Explicit Type Declaration
You can explicitly set the pipeline type:
```yaml
type: minimal  # or simple, standard, advanced
```

### Step Types
- **shell**: Execute shell commands
- **manual_approval**: Require human approval before proceeding

### Trigger Types
- **manual**: Manually triggered pipelines
- **webhook**: Triggered by Git events (push, pull_request)
- **scheduled**: Cron-based scheduling
- **git_push**: Triggered by Git push events
- **pull_request**: Triggered by pull request events

### Configuration Options
- **type**: Explicit pipeline type (minimal, simple, standard, advanced)
- **deployment_type: custom**: Prevents automatic project type detection
- **timeout**: Maximum execution time in seconds
- **retry_count**: Number of retry attempts on failure
- **environment**: Environment variables for the pipeline

### Advanced Features (Advanced Type Only)
- **matrix**: Multi-dimensional build matrices
- **variables**: Global pipeline variables
- **include**: Include external pipeline configurations
- **cache**: Dependency and build caching
- **jobs**: Complex job definitions with stage mapping

### Common Patterns

#### Multi-line Commands
```yaml
command: |
  command1 && \
  command2 && \
  command3
```

#### Conditional Execution
```yaml
command: |
  if [ -n "${VARIABLE}" ]; then
    echo "Variable is set"
  else
    echo "Variable not set"
  fi
```

#### Background Processes
```yaml
command: |
  kubectl port-forward service/app 8080:8080 &
  sleep 10
  curl http://localhost:8080/health
  pkill -f 'kubectl port-forward'
```

#### Error Handling
```yaml
command: |
  docker rm -f container-name || true  # Continue on error
```

## Environment Variables

Common environment variables used across pipelines:

- `MONGODB_URI`: MongoDB connection string
- `MONGODB_DATABASE`: Database name
- `JWT_SECRET`: JWT signing secret
- `RUST_ENV`: Environment (development/staging/production)
- `PORT`: Application port
- `BUILD_ID`: Unique build identifier
- `BACKUP_BUCKET`: S3 bucket for backups

## Best Practices

### General Guidelines
1. **Use descriptive names** for pipelines and steps
2. **Include verification steps** after deployments
3. **Handle errors gracefully** with `|| true` where appropriate
4. **Set appropriate timeouts** for long-running operations
5. **Use environment variables** for configuration
6. **Include cleanup steps** to prevent resource leaks
7. **Add deployment_type: custom** for deployment steps
8. **Use multi-line commands** for complex operations

### Multi-Tier Pipeline Best Practices

#### Minimal Pipelines
- Keep it simple with just `repo` and `server`
- Let auto-detection handle project type
- Use for quick prototypes and simple deployments

#### Simple Pipelines  
- Use clear, descriptive step names
- Keep steps focused on single responsibilities
- Ideal for linear CI/CD workflows

#### Standard Pipelines
- Organize jobs into logical stages
- Use parallel execution within stages for efficiency
- Map jobs to stages clearly with `stage` field

#### Advanced Pipelines
- Use matrix strategies for multi-platform builds
- Leverage caching for faster builds
- Include external configurations for reusability
- Use variables for configuration management

### Migration Path
- **Start Minimal**: Begin with minimal pipelines for quick setup
- **Grow to Simple**: Add linear steps as workflows become more complex
- **Evolve to Standard**: Organize into stages when parallel execution is needed
- **Advance**: Use matrix and advanced features for complex multi-environment deployments

## Testing Pipelines

To test these pipelines:

1. Ensure the test application is built: `cd test-app && cargo build`
2. Upload the pipeline YAML to RustCI
3. Trigger the pipeline manually
4. Monitor execution logs and verify results

## Customization

These examples can be customized for your specific needs:

- Modify environment variables
- Add additional verification steps
- Change deployment targets
- Adjust timeouts and retry counts
- Add notification integrations