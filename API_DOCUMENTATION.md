# RustCI API Documentation

Complete API reference for the RustCI platform with working cURL examples.

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

## Example Pipeline YAML

```yaml
name: "Full Stack Application"
description: "Build and deploy a full stack application"

triggers:
  - trigger_type: manual
    config: {}
  - trigger_type: webhook
    config:
      webhook_url: "/webhook/fullstack-app"
  - trigger_type: git_push
    config:
      branch_patterns: ["main", "develop"]

stages:
  - name: "Source"
    steps:
      - name: "clone-repository"
        step_type: github
        config:
          repository_url: "https://github.com/user/fullstack-app.git"
          branch: "main"
          token: "${GITHUB_TOKEN}"

  - name: "Build Frontend"
    steps:
      - name: "install-dependencies"
        step_type: shell
        config:
          command: "cd frontend && npm install"
      - name: "build-frontend"
        step_type: shell
        config:
          command: "cd frontend && npm run build"

  - name: "Build Backend"
    steps:
      - name: "build-rust-backend"
        step_type: shell
        config:
          command: "cd backend && cargo build --release"

  - name: "Test"
    steps:
      - name: "run-tests"
        step_type: shell
        config:
          command: "cargo test && cd frontend && npm test"

  - name: "Deploy"
    steps:
      - name: "deploy-frontend"
        step_type: docker
        config:
          image: "fullstack-frontend"
          dockerfile: "frontend/Dockerfile"
          ports: ["80:80"]
      - name: "deploy-backend"
        step_type: docker
        config:
          image: "fullstack-backend"
          dockerfile: "backend/Dockerfile"
          ports: ["8000:8000"]

environment:
  NODE_ENV: "production"
  RUST_ENV: "production"
  DATABASE_URL: "${DATABASE_URL}"
  JWT_SECRET: "${JWT_SECRET}"

timeout: 3600
retry_count: 1
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
