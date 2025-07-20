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

## Quick Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/healthchecker` | GET | Health check |
| `/ci/pipelines` | POST | Create pipeline |
| `/ci/pipelines/upload` | POST | Upload pipeline YAML |
| `/ci/pipelines` | GET | List pipelines |
| `/ci/pipelines/{id}/trigger` | POST | Trigger pipeline |
| `/ci/pipelines/{id}/webhook` | POST | Webhook trigger |
| `/ci/executions/{id}` | GET | Get execution |
| `/ci/executions` | GET | List executions |
| `/ci/executions/{id}/cancel` | DELETE | Cancel execution |
| `/sessions/oauth/github` | GET | GitHub OAuth |
| `/sessions/me` | GET | Current user |

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

## SDK Examples

### JavaScript/Node.js

```javascript
const axios = require('axios');

class RustCIClient {
  constructor(baseUrl = 'http://localhost:8000/api') {
    this.baseUrl = baseUrl;
    this.token = null;
  }

  setToken(token) {
    this.token = token;
  }

  async createPipeline(yamlContent) {
    const response = await axios.post(`${this.baseUrl}/ci/pipelines`, {
      yaml_content: yamlContent
    }, {
      headers: this.token ? { Authorization: `Bearer ${this.token}` } : {}
    });
    return response.data;
  }

  async triggerPipeline(pipelineId, options = {}) {
    const response = await axios.post(
      `${this.baseUrl}/ci/pipelines/${pipelineId}/trigger`,
      {
        trigger_type: 'manual',
        ...options
      },
      {
        headers: this.token ? { Authorization: `Bearer ${this.token}` } : {}
      }
    );
    return response.data;
  }

  async getExecution(executionId) {
    const response = await axios.get(
      `${this.baseUrl}/ci/executions/${executionId}`,
      {
        headers: this.token ? { Authorization: `Bearer ${this.token}` } : {}
      }
    );
    return response.data;
  }
}

// Usage
const client = new RustCIClient();
const pipeline = await client.createPipeline('name: "Test"\nstages: []');
const execution = await client.triggerPipeline(pipeline.id);
```

### Python

```python
import requests
import json

class RustCIClient:
    def __init__(self, base_url='http://localhost:8000/api'):
        self.base_url = base_url
        self.token = None
    
    def set_token(self, token):
        self.token = token
    
    def _headers(self):
        headers = {'Content-Type': 'application/json'}
        if self.token:
            headers['Authorization'] = f'Bearer {self.token}'
        return headers
    
    def create_pipeline(self, yaml_content):
        response = requests.post(
            f'{self.base_url}/ci/pipelines',
            json={'yaml_content': yaml_content},
            headers=self._headers()
        )
        return response.json()
    
    def trigger_pipeline(self, pipeline_id, **options):
        data = {'trigger_type': 'manual', **options}
        response = requests.post(
            f'{self.base_url}/ci/pipelines/{pipeline_id}/trigger',
            json=data,
            headers=self._headers()
        )
        return response.json()
    
    def get_execution(self, execution_id):
        response = requests.get(
            f'{self.base_url}/ci/executions/{execution_id}',
            headers=self._headers()
        )
        return response.json()

# Usage
client = RustCIClient()
pipeline = client.create_pipeline('name: "Test"\nstages: []')
execution = client.trigger_pipeline(pipeline['id'])
```

## Next Steps

- [Pipeline Examples](../examples/pipelines.md) - Working pipeline examples
- [Connector Documentation](../architecture/connectors.md) - Learn about connectors
- [Deployment Guides](../deployment/README.md) - Deploy RustCI to production