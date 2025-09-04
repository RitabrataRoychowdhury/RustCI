# Quick Start Guide

Get up and running with RustCI in minutes! This guide will walk you through creating and running your first pipeline.

## Prerequisites

- RustCI installed and running (see [Installation Guide](./installation.md))
- MongoDB running
- Docker installed (for Docker connector examples)

## Step 1: Verify Installation

First, make sure RustCI is running:

```bash
# Check health endpoint
curl http://localhost:8000/api/healthchecker

# Expected response:
# {"status":"success","message":"Build Simple CRUD API with Rust and Axum"}
```

## Step 2: Create Your First Pipeline

### Option A: Using JSON API

Create a simple "Hello World" pipeline:

```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Hello World Pipeline\"\ndescription: \"My first RustCI pipeline\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Greet\"\n    steps:\n      - name: \"hello-step\"\n        step_type: shell\n        config:\n          command: \"echo Hello from RustCI!\"\nenvironment: {}\ntimeout: 300\nretry_count: 0"
  }'
```

### Option B: Using File Upload

Create a YAML file `hello-pipeline.yaml`:

```yaml
name: "Hello World Pipeline"
description: "My first RustCI pipeline"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Greet"
    steps:
      - name: "hello-step"
        step_type: shell
        config:
          command: "echo 'Hello from RustCI!'"

environment: {}
timeout: 300
retry_count: 0
```

Upload the pipeline:

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@hello-pipeline.yaml"
```

Both methods will return a response like:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Hello World Pipeline",
  "description": "My first RustCI pipeline",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

**Save the pipeline ID** - you'll need it for the next step!

## Step 3: Trigger the Pipeline

Use the pipeline ID from the previous step:

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {}
  }'
```

Response:

```json
{
  "execution_id": "660e8400-e29b-41d4-a716-446655440001",
  "message": "Pipeline triggered successfully"
}
```

## Step 4: Monitor Execution

Check the execution status using the execution ID:

```bash
curl http://localhost:8000/api/ci/executions/660e8400-e29b-41d4-a716-446655440001
```

Response:

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440001",
  "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "started_at": "2024-01-15T10:35:00Z",
  "finished_at": "2024-01-15T10:35:05Z",
  "duration": 5,
  "stages": [
    {
      "name": "Greet",
      "status": "completed",
      "steps": [
        {
          "name": "hello-step",
          "status": "completed",
          "logs": ["Hello from RustCI!"]
        }
      ]
    }
  ]
}
```

🎉 **Congratulations!** You've successfully created and executed your first RustCI pipeline!

## Step 5: Try a Docker Pipeline

Now let's create a more advanced pipeline using Docker:

Create `docker-pipeline.yaml`:

```yaml
name: "Docker Build Pipeline"
description: "Build and run a Node.js application in Docker"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Build"
    steps:
      - name: "node-build"
        step_type: docker
        config:
          image: "node:18-alpine"
          command: "npm --version && node --version"
          working_directory: "/app"
        timeout: 120

  - name: "Test"
    steps:
      - name: "run-tests"
        step_type: docker
        config:
          image: "node:18-alpine"
          script: |
            echo "Running tests..."
            echo "✅ All tests passed!"
          working_directory: "/app"
        timeout: 180

environment:
  NODE_ENV: "development"

timeout: 600
retry_count: 1
```

Upload and run the Docker pipeline:

```bash
# Upload pipeline
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@docker-pipeline.yaml"

# Trigger pipeline (replace with your pipeline ID)
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "NODE_ENV": "production"
    }
  }'
```

## Step 6: List Your Pipelines

See all your created pipelines:

```bash
curl http://localhost:8000/api/ci/pipelines
```

## Step 7: List Executions

See all pipeline executions:

```bash
# All executions
curl http://localhost:8000/api/ci/executions

# Executions for a specific pipeline
curl "http://localhost:8000/api/ci/executions?pipeline_id=550e8400-e29b-41d4-a716-446655440000"
```

## Common Pipeline Patterns

### Multi-Stage Pipeline

```yaml
name: "Multi-Stage Pipeline"
description: "Build, test, and deploy"

stages:
  - name: "Build"
    steps:
      - name: "compile"
        step_type: shell
        config:
          command: "echo 'Building application...'"

  - name: "Test"
    parallel: true
    steps:
      - name: "unit-tests"
        step_type: shell
        config:
          command: "echo 'Running unit tests...'"
      
      - name: "integration-tests"
        step_type: shell
        config:
          command: "echo 'Running integration tests...'"

  - name: "Deploy"
    steps:
      - name: "deploy-staging"
        step_type: shell
        config:
          command: "echo 'Deploying to staging...'"
```

### Environment Variables

```yaml
name: "Environment Pipeline"
description: "Using environment variables"

stages:
  - name: "Environment Test"
    steps:
      - name: "show-env"
        step_type: shell
        config:
          script: |
            echo "Environment: $NODE_ENV"
            echo "Version: $APP_VERSION"
            echo "Database: $DATABASE_URL"

environment:
  NODE_ENV: "production"
  APP_VERSION: "1.0.0"
  DATABASE_URL: "mongodb://localhost:27017/myapp"
```

### Conditional Execution

```yaml
name: "Conditional Pipeline"
description: "Steps that run based on conditions"

stages:
  - name: "Build"
    steps:
      - name: "build-app"
        step_type: shell
        config:
          command: "echo 'Building...'"

  - name: "Deploy"
    condition: "${BRANCH} == 'main'"
    steps:
      - name: "deploy-prod"
        step_type: shell
        config:
          command: "echo 'Deploying to production...'"
```

## Webhook Integration

Set up GitHub webhooks to trigger pipelines automatically:

1. **Create a webhook-triggered pipeline**:

```yaml
name: "Webhook Pipeline"
description: "Triggered by GitHub webhooks"

triggers:
  - trigger_type: webhook
    config:
      webhook_url: "/webhook/my-app"

stages:
  - name: "Deploy"
    steps:
      - name: "deploy-on-push"
        step_type: shell
        config:
          script: |
            echo "Deploying commit: $CI_COMMIT_SHA"
            echo "Branch: $CI_BRANCH"
            echo "Repository: $CI_REPOSITORY"
```

2. **Configure GitHub webhook**:
   - Go to your GitHub repository → Settings → Webhooks
   - Add webhook URL: `http://your-server:8000/api/ci/pipelines/{PIPELINE_ID}/webhook`
   - Select "Push events"
   - Content type: `application/json`

3. **Trigger via webhook**:

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "refs/heads/main",
    "after": "abc123def456789",
    "repository": {
      "full_name": "user/repo",
      "clone_url": "https://github.com/user/repo.git"
    },
    "pusher": {
      "name": "developer"
    }
  }'
```

## Next Steps

Now that you've created your first pipelines, explore more advanced features:

1. **[Configuration Guide](./configuration.md)** - Advanced configuration options
2. **[API Documentation](../api/README.md)** - Complete API reference
3. **[Connector System](../architecture/connectors.md)** - Learn about different connectors
4. **[Pipeline Examples](../pipeline-examples/README.md)** - More complex pipeline examples
5. **[Kubernetes Deployment](../deployment/kubernetes.md)** - Deploy to Kubernetes

## Troubleshooting

For detailed troubleshooting information, see the [Troubleshooting Guide](troubleshooting.md).

### Common Issues

#### Pipeline Creation Fails

```bash
# Check YAML syntax
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{"yaml_content": "invalid yaml content"}'

# Response will include validation errors
```

### Execution Fails

```bash
# Check execution details for error messages
curl http://localhost:8000/api/ci/executions/{EXECUTION_ID}

# Look at the logs in the response
```

### Docker Issues

```bash
# Verify Docker is running
docker ps

# Check Docker daemon logs
sudo journalctl -u docker.service
```

### Permission Issues

```bash
# Ensure user is in docker group
sudo usermod -aG docker $USER
newgrp docker

# Test Docker access
docker run hello-world
```

Happy building with RustCI! 🚀