# RustCI API Documentation

## Overview

The RustCI API provides RESTful endpoints for managing CI/CD pipelines, workspaces, and executions. All endpoints return JSON responses and use standard HTTP status codes.

## Base URL

```
https://api.rustci.dev/v1
```

## Authentication

The API uses JWT (JSON Web Tokens) for authentication. Include the token in the Authorization header:

```
Authorization: Bearer <your-jwt-token>
```

### OAuth Flow

1. Redirect users to GitHub OAuth: `GET /auth/github`
2. Handle callback: `GET /auth/github/callback`
3. Receive JWT token in response

## Rate Limiting

- 1000 requests per hour per user
- 100 requests per minute per user
- Rate limit headers included in responses:
  - `X-RateLimit-Limit`
  - `X-RateLimit-Remaining`
  - `X-RateLimit-Reset`

## Error Handling

All errors follow a consistent format:

```json
{
  "error": {
    "type": "ValidationError",
    "message": "Invalid pipeline configuration",
    "details": {
      "field": "stages",
      "reason": "At least one stage is required"
    },
    "timestamp": "2024-01-01T10:00:00Z",
    "correlation_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

### HTTP Status Codes

- `200` - Success
- `201` - Created
- `400` - Bad Request
- `401` - Unauthorized
- `403` - Forbidden
- `404` - Not Found
- `409` - Conflict
- `422` - Unprocessable Entity
- `429` - Too Many Requests
- `500` - Internal Server Error

## Endpoints

### Health Check

#### GET /health

Check API health status.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T10:00:00Z",
  "version": "1.0.0",
  "uptime": "2h 30m 15s"
}
```

### Users

#### GET /api/users/me

Get current user information.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "username": "johndoe",
  "email": "john@example.com",
  "github_id": "123456",
  "roles": ["user"],
  "created_at": "2024-01-01T10:00:00Z",
  "last_login": "2024-01-01T12:00:00Z"
}
```

#### PUT /api/users/me

Update current user information.

**Headers:**
- `Authorization: Bearer <token>` (required)
- `Content-Type: application/json`

**Request Body:**
```json
{
  "email": "newemail@example.com"
}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "username": "johndoe",
  "email": "newemail@example.com",
  "github_id": "123456",
  "roles": ["user"],
  "created_at": "2024-01-01T10:00:00Z",
  "updated_at": "2024-01-01T14:00:00Z"
}
```

### Workspaces

#### GET /api/workspaces

List user's workspaces.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Query Parameters:**
- `page` (optional): Page number (default: 1)
- `limit` (optional): Items per page (default: 20, max: 100)
- `status` (optional): Filter by status (`active`, `inactive`)

**Response:**
```json
{
  "workspaces": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440000",
      "name": "my-project",
      "owner_id": "550e8400-e29b-41d4-a716-446655440000",
      "repository_url": "https://github.com/user/repo",
      "branch": "main",
      "status": "active",
      "created_at": "2024-01-01T10:00:00Z",
      "updated_at": "2024-01-01T10:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 1,
    "total_pages": 1
  }
}
```

#### POST /api/workspaces

Create a new workspace.

**Headers:**
- `Authorization: Bearer <token>` (required)
- `Content-Type: application/json`

**Request Body:**
```json
{
  "name": "new-project",
  "repository_url": "https://github.com/user/new-repo",
  "branch": "main"
}
```

**Response:** `201 Created`
```json
{
  "id": "770e8400-e29b-41d4-a716-446655440000",
  "name": "new-project",
  "owner_id": "550e8400-e29b-41d4-a716-446655440000",
  "repository_url": "https://github.com/user/new-repo",
  "branch": "main",
  "status": "active",
  "created_at": "2024-01-01T14:00:00Z",
  "updated_at": "2024-01-01T14:00:00Z"
}
```

#### GET /api/workspaces/{workspace_id}

Get workspace details.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Response:**
```json
{
  "id": "660e8400-e29b-41d4-a716-446655440000",
  "name": "my-project",
  "owner_id": "550e8400-e29b-41d4-a716-446655440000",
  "repository_url": "https://github.com/user/repo",
  "branch": "main",
  "status": "active",
  "created_at": "2024-01-01T10:00:00Z",
  "updated_at": "2024-01-01T10:00:00Z",
  "pipelines_count": 3,
  "last_execution": "2024-01-01T13:00:00Z"
}
```

#### PUT /api/workspaces/{workspace_id}

Update workspace.

**Headers:**
- `Authorization: Bearer <token>` (required)
- `Content-Type: application/json`

**Request Body:**
```json
{
  "name": "updated-project",
  "branch": "develop"
}
```

**Response:**
```json
{
  "id": "660e8400-e29b-41d4-a716-446655440000",
  "name": "updated-project",
  "owner_id": "550e8400-e29b-41d4-a716-446655440000",
  "repository_url": "https://github.com/user/repo",
  "branch": "develop",
  "status": "active",
  "created_at": "2024-01-01T10:00:00Z",
  "updated_at": "2024-01-01T14:30:00Z"
}
```

#### DELETE /api/workspaces/{workspace_id}

Delete workspace.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Response:** `204 No Content`

### Pipelines

#### GET /api/workspaces/{workspace_id}/pipelines

List pipelines in a workspace.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Query Parameters:**
- `page` (optional): Page number (default: 1)
- `limit` (optional): Items per page (default: 20, max: 100)

**Response:**
```json
{
  "pipelines": [
    {
      "id": "880e8400-e29b-41d4-a716-446655440000",
      "name": "build-and-test",
      "workspace_id": "660e8400-e29b-41d4-a716-446655440000",
      "stages": [
        {
          "name": "build",
          "steps": [
            {
              "name": "compile",
              "command": "cargo build",
              "args": ["--release"]
            }
          ]
        }
      ],
      "triggers": ["push", "pull_request"],
      "created_at": "2024-01-01T10:00:00Z",
      "updated_at": "2024-01-01T10:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 1,
    "total_pages": 1
  }
}
```

#### POST /api/workspaces/{workspace_id}/pipelines

Create a new pipeline.

**Headers:**
- `Authorization: Bearer <token>` (required)
- `Content-Type: application/json`

**Request Body:**
```json
{
  "name": "deploy-pipeline",
  "stages": [
    {
      "name": "build",
      "steps": [
        {
          "name": "compile",
          "command": "cargo build",
          "args": ["--release"],
          "environment": {
            "RUST_LOG": "info"
          }
        }
      ],
      "depends_on": []
    },
    {
      "name": "deploy",
      "steps": [
        {
          "name": "deploy-to-staging",
          "command": "kubectl apply",
          "args": ["-f", "k8s/staging/"]
        }
      ],
      "depends_on": ["build"]
    }
  ],
  "variables": {
    "ENVIRONMENT": "staging"
  },
  "triggers": ["push"]
}
```

**Response:** `201 Created`
```json
{
  "id": "990e8400-e29b-41d4-a716-446655440000",
  "name": "deploy-pipeline",
  "workspace_id": "660e8400-e29b-41d4-a716-446655440000",
  "stages": [...],
  "variables": {
    "ENVIRONMENT": "staging"
  },
  "triggers": ["push"],
  "created_at": "2024-01-01T14:00:00Z",
  "updated_at": "2024-01-01T14:00:00Z"
}
```

#### GET /api/pipelines/{pipeline_id}

Get pipeline details.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Response:**
```json
{
  "id": "880e8400-e29b-41d4-a716-446655440000",
  "name": "build-and-test",
  "workspace_id": "660e8400-e29b-41d4-a716-446655440000",
  "stages": [...],
  "variables": {},
  "triggers": ["push", "pull_request"],
  "created_at": "2024-01-01T10:00:00Z",
  "updated_at": "2024-01-01T10:00:00Z",
  "executions_count": 25,
  "last_execution": {
    "id": "aa0e8400-e29b-41d4-a716-446655440000",
    "status": "completed",
    "started_at": "2024-01-01T13:00:00Z",
    "completed_at": "2024-01-01T13:05:00Z"
  }
}
```

### Executions

#### POST /api/pipelines/{pipeline_id}/executions

Start a pipeline execution.

**Headers:**
- `Authorization: Bearer <token>` (required)
- `Content-Type: application/json`

**Request Body:**
```json
{
  "trigger": "manual",
  "branch": "main",
  "commit_sha": "abc123def456",
  "variables": {
    "CUSTOM_VAR": "value"
  }
}
```

**Response:** `202 Accepted`
```json
{
  "id": "bb0e8400-e29b-41d4-a716-446655440000",
  "pipeline_id": "880e8400-e29b-41d4-a716-446655440000",
  "status": "queued",
  "trigger": "manual",
  "branch": "main",
  "commit_sha": "abc123def456",
  "variables": {
    "CUSTOM_VAR": "value"
  },
  "created_at": "2024-01-01T14:00:00Z",
  "started_at": null,
  "completed_at": null
}
```

#### GET /api/executions/{execution_id}

Get execution details.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Response:**
```json
{
  "id": "bb0e8400-e29b-41d4-a716-446655440000",
  "pipeline_id": "880e8400-e29b-41d4-a716-446655440000",
  "status": "running",
  "trigger": "manual",
  "branch": "main",
  "commit_sha": "abc123def456",
  "variables": {
    "CUSTOM_VAR": "value"
  },
  "created_at": "2024-01-01T14:00:00Z",
  "started_at": "2024-01-01T14:01:00Z",
  "completed_at": null,
  "stages": [
    {
      "name": "build",
      "status": "completed",
      "started_at": "2024-01-01T14:01:00Z",
      "completed_at": "2024-01-01T14:03:00Z",
      "steps": [
        {
          "name": "compile",
          "status": "completed",
          "started_at": "2024-01-01T14:01:00Z",
          "completed_at": "2024-01-01T14:03:00Z",
          "exit_code": 0,
          "logs_url": "/api/executions/bb0e8400-e29b-41d4-a716-446655440000/logs/build/compile"
        }
      ]
    }
  ]
}
```

#### GET /api/executions/{execution_id}/logs/{stage}/{step}

Get execution logs for a specific step.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Query Parameters:**
- `follow` (optional): Stream logs in real-time (default: false)
- `tail` (optional): Number of lines to return from the end (default: all)

**Response:**
```json
{
  "logs": [
    {
      "timestamp": "2024-01-01T14:01:00Z",
      "level": "INFO",
      "message": "Starting compilation..."
    },
    {
      "timestamp": "2024-01-01T14:01:30Z",
      "level": "INFO",
      "message": "Compiling rustci v0.1.0"
    },
    {
      "timestamp": "2024-01-01T14:03:00Z",
      "level": "INFO",
      "message": "Finished release [optimized] target(s)"
    }
  ],
  "has_more": false
}
```

#### POST /api/executions/{execution_id}/cancel

Cancel a running execution.

**Headers:**
- `Authorization: Bearer <token>` (required)

**Response:**
```json
{
  "id": "bb0e8400-e29b-41d4-a716-446655440000",
  "status": "cancelled",
  "cancelled_at": "2024-01-01T14:05:00Z"
}
```

### Webhooks

#### POST /webhooks/github

GitHub webhook endpoint.

**Headers:**
- `X-GitHub-Event: push` (required)
- `X-Hub-Signature-256: sha256=...` (required)
- `Content-Type: application/json`

**Request Body:** GitHub webhook payload

**Response:** `200 OK`
```json
{
  "message": "Webhook processed successfully",
  "executions_triggered": 2
}
```

## WebSocket API

### Real-time Execution Updates

Connect to WebSocket endpoint for real-time execution updates:

```
wss://api.rustci.dev/v1/ws/executions/{execution_id}
```

**Authentication:**
Include JWT token as query parameter: `?token=<your-jwt-token>`

**Message Format:**
```json
{
  "type": "execution_update",
  "data": {
    "execution_id": "bb0e8400-e29b-41d4-a716-446655440000",
    "status": "running",
    "stage": "build",
    "step": "compile",
    "timestamp": "2024-01-01T14:02:00Z"
  }
}
```

## Related Documentation

### API Reference
- **[API Reference](reference/README.md)** - Detailed endpoint documentation
- **[Users API](reference/users.md)** - User management endpoints
- **[Configuration Schema](configuration-schema.md)** - Complete configuration reference
- **[Usage Guide](usage-guide.md)** - Common usage patterns and examples

### Integration Guides
- **[User Documentation](../user/README.md)** - End-user guides and tutorials
- **[Getting Started](../user/getting-started/)** - Quick start guides for new users
- **[Pipeline Examples](../user/pipeline-examples/)** - Working pipeline configurations

### System Documentation
- **[Architecture Documentation](../architecture/README.md)** - System architecture and design
- **[Development Documentation](../development/README.md)** - Development setup and guidelines
- **[Deployment Documentation](../deployment/README.md)** - Deployment guides and operations

## SDK Examples

### cURL

```bash
# Get user info
curl -H "Authorization: Bearer $TOKEN" \
  https://api.rustci.dev/v1/api/users/me

# Create workspace
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"my-project","repository_url":"https://github.com/user/repo"}' \
  https://api.rustci.dev/v1/api/workspaces

# Start execution
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"trigger":"manual","branch":"main"}' \
  https://api.rustci.dev/v1/api/pipelines/$PIPELINE_ID/executions
```

### JavaScript

```javascript
const client = new RustCIClient({
  baseURL: 'https://api.rustci.dev/v1',
  token: 'your-jwt-token'
});

// Get workspaces
const workspaces = await client.workspaces.list();

// Create pipeline
const pipeline = await client.pipelines.create(workspaceId, {
  name: 'build-pipeline',
  stages: [...]
});

// Start execution
const execution = await client.executions.start(pipelineId, {
  trigger: 'manual',
  branch: 'main'
});
```

## Changelog

### v1.0.0 (2024-01-01)
- Initial API release
- User management endpoints
- Workspace and pipeline management
- Execution control and monitoring
- GitHub webhook integration
- WebSocket support for real-time updates