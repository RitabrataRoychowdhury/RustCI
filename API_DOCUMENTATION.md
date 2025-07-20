# RustCI API Documentation

Complete API reference for the RustCI platform with working cURL examples for Docker and Kubernetes deployments.

## Base URL

```
http://localhost:8000/api
```

## Authentication

Most endpoints require JWT authentication. Include the token in the Authorization header:

```bash
Authorization: Bearer YOUR_JWT_TOKEN
```

## Health Check

### GET /healthchecker

Check if the API is running.

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/healthchecker
```

**Response:**

```json
{
  "status": "success",
  "message": "Build Simple CRUD API with Rust and Axum"
}
```

## Pipeline Management

### POST /ci/pipelines

Create a new pipeline from JSON payload.

**Request Body:**

```json
{
  "yaml_content": "string"
}
```

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Test Pipeline\"\ndescription: \"A simple test pipeline\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Build\"\n    steps:\n      - name: \"build-step\"\n        step_type: shell\n        config:\n          command: \"echo Building application...\"\nenvironment: {}\ntimeout: 3600\nretry_count: 0"
  }'
```

**Response:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Test Pipeline",
  "description": "A simple test pipeline",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

### POST /ci/pipelines/upload

Create a new pipeline from uploaded YAML file.

**Request:** Multipart form data with file field named "pipeline"

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@pipeline.yaml"
```

**Response:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My Pipeline",
  "description": "Pipeline from uploaded file",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

### GET /ci/pipelines

List all pipelines.

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/ci/pipelines
```

**Response:**

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Test Pipeline",
    "description": "A simple test pipeline",
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T10:30:00Z"
  }
]
```

### GET /ci/pipelines/{pipeline_id}/yaml

Get pipeline YAML configuration.

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/yaml
```

**Response:**

```yaml
name: "Test Pipeline"
description: "A simple test pipeline"
triggers:
  - trigger_type: manual
    config: {}
stages:
  - name: "Build"
    steps:
      - name: "build-step"
        step_type: shell
        config:
          command: "echo Building application..."
environment: {}
timeout: 3600
retry_count: 0
```

## Pipeline Execution

### POST /ci/pipelines/{pipeline_id}/trigger

Trigger a pipeline execution.

**Request Body:**

```json
{
  "trigger_type": "string",
  "branch": "string (optional)",
  "commit_hash": "string (optional)",
  "repository": "string (optional)",
  "environment": {
    "key": "value"
  }
}
```

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "branch": "main",
    "environment": {
      "NODE_ENV": "production",
      "PORT": "3000"
    }
  }'
```

**Response:**

```json
{
  "execution_id": "660e8400-e29b-41d4-a716-446655440001",
  "message": "Pipeline triggered successfully"
}
```

### POST /ci/pipelines/{pipeline_id}/webhook

Trigger pipeline via webhook (GitHub format).

**Request Body:** GitHub webhook payload

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "refs/heads/main",
    "after": "abc123def456789",
    "repository": {
      "full_name": "user/repo",
      "clone_url": "https://github.com/user/repo.git"
    },
    "pusher": {
      "name": "username"
    }
  }'
```

**Response:**

```json
{
  "execution_id": "660e8400-e29b-41d4-a716-446655440001",
  "message": "Pipeline triggered by webhook"
}
```

## Execution Management

### GET /ci/executions/{execution_id}

Get execution status and details.

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/ci/executions/660e8400-e29b-41d4-a716-446655440001
```

**Response:**

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440001",
  "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "running",
  "started_at": "2024-01-15T10:35:00Z",
  "finished_at": null,
  "duration": null,
  "stages": [
    {
      "name": "Build",
      "status": "running",
      "steps": [
        {
          "name": "build-step",
          "status": "running",
          "started_at": "2024-01-15T10:35:00Z",
          "logs": ["Building application..."]
        }
      ]
    }
  ]
}
```

### GET /ci/executions

List all executions or filter by pipeline.

**Query Parameters:**

- `pipeline_id` (optional): Filter by pipeline ID

**cURL Examples:**

```bash
# List all executions
curl -X GET http://localhost:8000/api/ci/executions

# List executions for specific pipeline
curl -X GET "http://localhost:8000/api/ci/executions?pipeline_id=550e8400-e29b-41d4-a716-446655440000"
```

**Response:**

```json
[
  {
    "id": "660e8400-e29b-41d4-a716-446655440001",
    "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "completed",
    "started_at": "2024-01-15T10:35:00Z",
    "finished_at": "2024-01-15T10:37:00Z",
    "duration": 120
  }
]
```

### DELETE /ci/executions/{execution_id}/cancel

Cancel a running execution.

**cURL Example:**

```bash
curl -X DELETE http://localhost:8000/api/ci/executions/660e8400-e29b-41d4-a716-446655440001/cancel
```

**Response:**

```json
{
  "message": "Execution cancelled successfully",
  "execution_id": "660e8400-e29b-41d4-a716-446655440001"
}
```

## Testing Endpoints

### GET /ci/test

Test CI engine connectivity.

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/ci/test
```

**Response:**

```
CI Engine is working!
```

## Authentication Endpoints

### GET /sessions/oauth/github

Initiate GitHub OAuth flow.

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/sessions/oauth/github
```

### GET /sessions/oauth/github/callback

GitHub OAuth callback (handled by browser).

### GET /sessions/me

Get current user information (requires authentication).

**cURL Example:**

```bash
curl -X GET http://localhost:8000/api/sessions/me \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

## Error Responses

All endpoints return errors in the following format:

```json
{
  "error": "ValidationError",
  "message": "Invalid YAML configuration: missing required field 'name'",
  "details": {
    "field": "name",
    "expected": "string"
  },
  "suggestions": [
    "Add a 'name' field to your pipeline configuration",
    "Check the YAML syntax for any formatting errors"
  ],
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Common HTTP Status Codes

- `200 OK` - Request successful
- `201 Created` - Resource created successfully
- `400 Bad Request` - Invalid request data
- `401 Unauthorized` - Authentication required
- `404 Not Found` - Resource not found
- `422 Unprocessable Entity` - Validation error
- `500 Internal Server Error` - Server error

## Rate Limits

- File uploads: 10MB maximum size
- API requests: 1000 requests per hour per IP
- Webhook endpoints: 100 requests per minute

## Docker Deployment Examples

### 1. Simple Docker Container Deployment

**Pipeline YAML:**

```yaml
name: "Docker Container Deployment"
description: "Deploy application using Docker connector"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Build"
    steps:
      - name: "build-app"
        step_type: docker
        config:
          image: "node:18-alpine"
          command: "npm ci && npm run build"
          working_directory: "/app"
          environment:
            NODE_ENV: "production"
        timeout: 300

  - name: "Deploy"
    steps:
      - name: "deploy-container"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          build_context: "."
          image: "my-app:latest"
          command: "npm start"
          environment:
            PORT: "8000"
            DATABASE_URL: "${DATABASE_URL}"
        timeout: 600

environment:
  DATABASE_URL: "mongodb://localhost:27017/myapp"
  DOCKER_BUILDKIT: "1"

timeout: 1800
retry_count: 1
```

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@docker-deployment.yaml"
```

### 2. Multi-Stage Docker Build

**Pipeline YAML:**

```yaml
name: "Multi-Stage Docker Build"
description: "Build and deploy with multi-stage Dockerfile"

triggers:
  - trigger_type: git_push
    config:
      branch_patterns: ["main"]

stages:
  - name: "Build and Test"
    parallel: true
    steps:
      - name: "build-backend"
        step_type: docker
        config:
          image: "rust:1.70"
          command: "cargo build --release"
          working_directory: "/workspace"
        timeout: 900
      
      - name: "build-frontend"
        step_type: docker
        config:
          image: "node:18"
          command: "npm ci && npm run build"
          working_directory: "/workspace/frontend"
        timeout: 600

  - name: "Deploy"
    steps:
      - name: "deploy-production"
        step_type: docker
        config:
          dockerfile: "Dockerfile.prod"
          build_context: "."
          image: "myapp:${BUILD_TAG}"
          script: |
            echo "Starting production deployment"
            docker run -d --name myapp-${BUILD_TAG} \
              -p 8000:8000 \
              -e DATABASE_URL="${DATABASE_URL}" \
              myapp:${BUILD_TAG}
        timeout: 300

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  DATABASE_URL: "${DATABASE_URL}"
```

**cURL Trigger Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "git_push",
    "branch": "main",
    "commit_hash": "abc123def456",
    "environment": {
      "BUILD_TAG": "v1.2.3",
      "DATABASE_URL": "mongodb://prod-db:27017/myapp"
    }
  }'
```

## Kubernetes Deployment Examples

### 1. Basic Kubernetes Job Deployment

**Pipeline YAML:**

```yaml
name: "Kubernetes Job Deployment"
description: "Deploy application using Kubernetes connector"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Deploy to Kubernetes"
    steps:
      - name: "deploy-job"
        step_type: kubernetes
        config:
          image: "ubuntu:latest"
          command: "echo 'Deploying to Kubernetes' && sleep 10"
          namespace: "default"
        timeout: 300

      - name: "deploy-with-resources"
        step_type: kubernetes
        config:
          image: "nginx:alpine"
          script: |
            echo "Starting nginx deployment"
            nginx -g "daemon off;" &
            sleep 30
            echo "Deployment completed"
          namespace: "production"
          parameters:
            resource_requests:
              cpu: "100m"
              memory: "128Mi"
            resource_limits:
              cpu: "500m"
              memory: "512Mi"
        timeout: 600

environment:
  KUBECONFIG: "/etc/kubernetes/config"
```

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Kubernetes Job Deployment\"\ndescription: \"Deploy application using Kubernetes connector\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Deploy to Kubernetes\"\n    steps:\n      - name: \"deploy-job\"\n        step_type: kubernetes\n        config:\n          image: \"ubuntu:latest\"\n          command: \"echo '\''Deploying to Kubernetes'\'' && sleep 10\"\n          namespace: \"default\"\n        timeout: 300\nenvironment:\n  KUBECONFIG: \"/etc/kubernetes/config\""
  }'
```

### 2. Advanced Kubernetes Deployment with PVC

**Pipeline YAML:**

```yaml
name: "Advanced Kubernetes Deployment"
description: "Deploy with PVC, lifecycle hooks, and resource management"

triggers:
  - trigger_type: webhook
    config:
      webhook_url: "/webhook/k8s-deploy"

stages:
  - name: "Build and Deploy"
    steps:
      - name: "deploy-with-pvc"
        step_type: kubernetes
        config:
          image: "maven:3.8-openjdk-17"
          script: |
            echo "Starting build process"
            mvn clean package -DskipTests
            echo "Build completed, deploying artifacts"
            cp target/*.jar /workspace/artifacts/
          namespace: "ci-cd"
          parameters:
            use_pvc: true
            storage_class: "fast-ssd"
            storage_size: "20Gi"
            resource_requests:
              cpu: "500m"
              memory: "1Gi"
            resource_limits:
              cpu: "2"
              memory: "4Gi"
            service_account: "ci-service-account"
        timeout: 1200

      - name: "deploy-production"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            echo "Deploying to production namespace"
            kubectl apply -f k8s/deployment.yaml
            kubectl apply -f k8s/service.yaml
            kubectl rollout status deployment/myapp --timeout=300s
            echo "Production deployment completed"
          namespace: "production"
          parameters:
            service_account: "deployment-sa"
            resource_requests:
              cpu: "100m"
              memory: "128Mi"
        timeout: 600

environment:
  MAVEN_OPTS: "-Dmaven.repo.local=/workspace/.m2/repository"
  KUBECTL_VERSION: "1.28"

timeout: 2400
retry_count: 2
```

**cURL Webhook Trigger:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "refs/heads/main",
    "after": "abc123def456789",
    "repository": {
      "full_name": "user/k8s-app",
      "clone_url": "https://github.com/user/k8s-app.git"
    },
    "pusher": {
      "name": "developer"
    }
  }'
```

### 3. Multi-Cloud Kubernetes Deployment

**Pipeline YAML:**

```yaml
name: "Multi-Cloud Kubernetes Deployment"
description: "Deploy to multiple Kubernetes clusters"

triggers:
  - trigger_type: schedule
    config:
      cron_expression: "0 2 * * *"

stages:
  - name: "Build"
    steps:
      - name: "build-image"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          image: "myapp:${BUILD_TAG}"
          build_context: "."
        timeout: 600

  - name: "Deploy to Staging"
    steps:
      - name: "deploy-staging"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl config use-context staging-cluster
            kubectl set image deployment/myapp myapp=myapp:${BUILD_TAG}
            kubectl rollout status deployment/myapp
          namespace: "staging"
          parameters:
            service_account: "deployment-sa"
        timeout: 300

  - name: "Deploy to Production"
    steps:
      - name: "deploy-aws-eks"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl config use-context aws-eks-prod
            kubectl set image deployment/myapp myapp=myapp:${BUILD_TAG}
            kubectl rollout status deployment/myapp --timeout=600s
          namespace: "production"
          parameters:
            service_account: "eks-deployment-sa"
            resource_requests:
              cpu: "200m"
              memory: "256Mi"
            resource_limits:
              cpu: "1"
              memory: "1Gi"
        timeout: 900

      - name: "deploy-gcp-gke"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl config use-context gcp-gke-prod
            kubectl set image deployment/myapp myapp=gcr.io/project/myapp:${BUILD_TAG}
            kubectl rollout status deployment/myapp --timeout=600s
          namespace: "production"
          parameters:
            service_account: "gke-deployment-sa"
        timeout: 900

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  AWS_REGION: "us-west-2"
  GCP_PROJECT: "my-project-id"

timeout: 3600
retry_count: 1
```

**cURL Schedule Trigger:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/550e8400-e29b-41d4-a716-446655440000/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "schedule",
    "environment": {
      "BUILD_TAG": "nightly-build",
      "DEPLOY_ENV": "production"
    }
  }'
```

## Hybrid Docker + Kubernetes Pipeline

**Pipeline YAML:**

```yaml
name: "Hybrid Docker-Kubernetes Pipeline"
description: "Build with Docker, deploy to Kubernetes"

triggers:
  - trigger_type: git_push
    config:
      branch_patterns: ["main", "develop"]

stages:
  - name: "Build with Docker"
    parallel: true
    steps:
      - name: "build-backend"
        step_type: docker
        config:
          image: "rust:1.70"
          script: |
            cargo build --release
            cargo test
            strip target/release/myapp
          working_directory: "/workspace"
          environment:
            CARGO_TARGET_DIR: "/workspace/target"
        timeout: 900

      - name: "build-frontend"
        step_type: docker
        config:
          image: "node:18-alpine"
          script: |
            npm ci
            npm run build
            npm run test:unit
          working_directory: "/workspace/frontend"
        timeout: 600

  - name: "Package"
    steps:
      - name: "build-container-image"
        step_type: docker
        config:
          dockerfile: "Dockerfile.prod"
          image: "myapp:${BUILD_TAG}"
          build_context: "."
          script: |
            echo "Container image built successfully"
            docker tag myapp:${BUILD_TAG} registry.example.com/myapp:${BUILD_TAG}
            docker push registry.example.com/myapp:${BUILD_TAG}
        timeout: 300

  - name: "Deploy to Kubernetes"
    steps:
      - name: "deploy-to-cluster"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl set image deployment/myapp \
              myapp=registry.example.com/myapp:${BUILD_TAG}
            kubectl rollout status deployment/myapp --timeout=300s
            kubectl get pods -l app=myapp
          namespace: "production"
          parameters:
            use_pvc: true
            storage_size: "10Gi"
            service_account: "deployment-sa"
            resource_requests:
              cpu: "200m"
              memory: "512Mi"
            resource_limits:
              cpu: "1"
              memory: "2Gi"
        timeout: 600

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  REGISTRY_URL: "registry.example.com"
  DOCKER_BUILDKIT: "1"

timeout: 2400
retry_count: 1
```

**cURL Example:**

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@hybrid-pipeline.yaml"
```

## Connector-Specific Configuration Options

### Docker Connector Options

```yaml
step_type: docker
config:
  image: "ubuntu:latest"              # Base image (optional if dockerfile provided)
  dockerfile: "Dockerfile"            # Path to Dockerfile (optional)
  build_context: "."                  # Build context directory (default: ".")
  command: "echo 'Hello World'"       # Command to execute
  script: |                           # Multi-line script (alternative to command)
    echo "Starting process"
    ./run-app.sh
  working_directory: "/app"           # Working directory inside container
  environment:                        # Environment variables
    NODE_ENV: "production"
    PORT: "8000"
```

### Kubernetes Connector Options

```yaml
step_type: kubernetes
config:
  image: "ubuntu:latest"              # Container image
  command: "echo 'Hello K8s'"         # Command to execute
  script: |                           # Multi-line script
    echo "Starting K8s job"
    kubectl get pods
  namespace: "default"                # Kubernetes namespace
  parameters:
    use_pvc: true                     # Use PersistentVolumeClaim
    storage_class: "fast-ssd"         # Storage class for PVC
    storage_size: "10Gi"              # PVC size
    service_account: "ci-sa"          # Service account for RBAC
    resource_requests:                # Resource requests
      cpu: "100m"
      memory: "128Mi"
    resource_limits:                  # Resource limits
      cpu: "500m"
      memory: "512Mi"
```

## WebSocket Support (Future)

Real-time execution updates will be available via WebSocket:

```javascript
const ws = new WebSocket(
  "ws://localhost:8000/api/ci/executions/660e8400-e29b-41d4-a716-446655440001/stream"
);
ws.onmessage = (event) => {
  const update = JSON.parse(event.data);
  console.log("Execution update:", update);
};
```
