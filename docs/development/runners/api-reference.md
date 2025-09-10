# Runner API Reference

This document provides comprehensive API reference for RustCI runners, including registration, management, job execution, and monitoring endpoints.

## Base URL

All API endpoints are relative to the RustCI server base URL:

```
http://localhost:8080/api
```

## Authentication

Most runner APIs require authentication via JWT tokens or API keys:

```bash
# Using JWT token
curl -H "Authorization: Bearer <jwt_token>" <endpoint>

# Using API key
curl -H "X-API-Key: <api_key>" <endpoint>
```

## Runner Management APIs

### Register Runner

Register a new runner with the RustCI server.

**Endpoint:** `POST /runners/register`

**Request Body:**

```json
{
  "name": "my-docker-runner",
  "type": "docker",
  "host": "localhost",
  "port": 8081,
  "capabilities": ["docker", "isolation", "deployment"],
  "metadata": {
    "os": "linux",
    "arch": "amd64",
    "docker_version": "24.0.0",
    "max_concurrent_jobs": 5
  },
  "tags": ["production", "docker"],
  "resources": {
    "cpu_cores": 4,
    "memory_gb": 8,
    "disk_gb": 100
  }
}
```

**Response:**

```json
{
  "success": true,
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "registration_token": "reg_token_abc123",
  "message": "Runner registered successfully"
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/api/runners/register \
  -H "Content-Type: application/json" \
  -d '{
    "name": "docker-runner-1",
    "type": "docker",
    "host": "localhost",
    "port": 8081,
    "capabilities": ["docker"],
    "metadata": {
      "os": "linux",
      "arch": "amd64"
    }
  }'
```

### List Runners

Get a list of all registered runners.

**Endpoint:** `GET /runners`

**Query Parameters:**

- `status` (optional): Filter by runner status (`active`, `inactive`, `error`)
- `type` (optional): Filter by runner type (`docker`, `kubernetes`, `native`, `local`)
- `tags` (optional): Filter by tags (comma-separated)
- `limit` (optional): Maximum number of results (default: 50)
- `offset` (optional): Pagination offset (default: 0)

**Response:**

```json
{
  "runners": [
    {
      "id": "runner_123e4567-e89b-12d3-a456-426614174000",
      "name": "docker-runner-1",
      "type": "docker",
      "status": "active",
      "host": "localhost",
      "port": 8081,
      "capabilities": ["docker"],
      "metadata": {
        "os": "linux",
        "arch": "amd64",
        "last_seen": "2024-01-15T10:30:00Z"
      },
      "resources": {
        "cpu_cores": 4,
        "memory_gb": 8,
        "disk_gb": 100
      },
      "current_load": {
        "active_jobs": 2,
        "cpu_usage": 45.2,
        "memory_usage": 60.1
      }
    }
  ],
  "total_count": 1,
  "has_more": false
}
```

**Example:**

```bash
# List all runners
curl http://localhost:8080/api/runners

# List only active Docker runners
curl "http://localhost:8080/api/runners?status=active&type=docker"

# List runners with pagination
curl "http://localhost:8080/api/runners?limit=10&offset=20"
```

### Get Runner Details

Get detailed information about a specific runner.

**Endpoint:** `GET /runners/{runner_id}`

**Response:**

```json
{
  "id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "name": "docker-runner-1",
  "type": "docker",
  "status": "active",
  "host": "localhost",
  "port": 8081,
  "capabilities": ["docker", "isolation"],
  "metadata": {
    "os": "linux",
    "arch": "amd64",
    "docker_version": "24.0.0",
    "registered_at": "2024-01-15T09:00:00Z",
    "last_seen": "2024-01-15T10:30:00Z"
  },
  "resources": {
    "cpu_cores": 4,
    "memory_gb": 8,
    "disk_gb": 100
  },
  "current_load": {
    "active_jobs": 2,
    "cpu_usage": 45.2,
    "memory_usage": 60.1,
    "disk_usage": 25.8
  },
  "statistics": {
    "total_jobs_executed": 150,
    "successful_jobs": 145,
    "failed_jobs": 5,
    "average_execution_time": 120.5,
    "uptime_percentage": 99.2
  }
}
```

**Example:**

```bash
curl http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000
```

### Update Runner

Update runner configuration or metadata.

**Endpoint:** `PUT /runners/{runner_id}`

**Request Body:**

```json
{
  "name": "updated-docker-runner",
  "capabilities": ["docker", "isolation", "deployment"],
  "metadata": {
    "max_concurrent_jobs": 10,
    "updated_at": "2024-01-15T11:00:00Z"
  },
  "tags": ["production", "high-capacity"]
}
```

**Response:**

```json
{
  "success": true,
  "message": "Runner updated successfully"
}
```

**Example:**

```bash
curl -X PUT http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000 \
  -H "Content-Type: application/json" \
  -d '{
    "name": "updated-docker-runner",
    "capabilities": ["docker", "deployment"]
  }'
```

### Delete Runner

Unregister a runner from the system.

**Endpoint:** `DELETE /runners/{runner_id}`

**Query Parameters:**

- `force` (optional): Force deletion even if runner has active jobs

**Response:**

```json
{
  "success": true,
  "message": "Runner deleted successfully"
}
```

**Example:**

```bash
# Normal deletion
curl -X DELETE http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000

# Force deletion
curl -X DELETE "http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000?force=true"
```

## Runner Status and Health APIs

### Get Runner Status

Get current status of a runner.

**Endpoint:** `GET /runners/{runner_id}/status`

**Response:**

```json
{
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "status": "active",
  "health": "healthy",
  "last_heartbeat": "2024-01-15T10:30:00Z",
  "current_load": {
    "active_jobs": 2,
    "queued_jobs": 1,
    "cpu_usage": 45.2,
    "memory_usage": 60.1,
    "disk_usage": 25.8
  },
  "capabilities_status": {
    "docker": "available",
    "isolation": "available",
    "deployment": "available"
  }
}
```

**Example:**

```bash
curl http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/status
```

### Get Runner Health

Perform health check on a runner.

**Endpoint:** `GET /runners/{runner_id}/health`

**Response:**

```json
{
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "health": "healthy",
  "checks": {
    "connectivity": {
      "status": "pass",
      "response_time_ms": 15
    },
    "docker_daemon": {
      "status": "pass",
      "version": "24.0.0"
    },
    "disk_space": {
      "status": "pass",
      "available_gb": 75.2
    },
    "memory": {
      "status": "pass",
      "available_gb": 3.2
    }
  },
  "last_check": "2024-01-15T10:30:00Z"
}
```

**Example:**

```bash
curl http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/health
```

### Get Runner Metrics

Get detailed metrics for a runner.

**Endpoint:** `GET /runners/{runner_id}/metrics`

**Query Parameters:**

- `period` (optional): Time period for metrics (`1h`, `24h`, `7d`, `30d`)
- `resolution` (optional): Data resolution (`1m`, `5m`, `1h`)

**Response:**

```json
{
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "period": "24h",
  "resolution": "5m",
  "metrics": {
    "cpu_usage": [
      { "timestamp": "2024-01-15T09:00:00Z", "value": 25.5 },
      { "timestamp": "2024-01-15T09:05:00Z", "value": 30.2 }
    ],
    "memory_usage": [
      { "timestamp": "2024-01-15T09:00:00Z", "value": 45.1 },
      { "timestamp": "2024-01-15T09:05:00Z", "value": 48.3 }
    ],
    "active_jobs": [
      { "timestamp": "2024-01-15T09:00:00Z", "value": 1 },
      { "timestamp": "2024-01-15T09:05:00Z", "value": 2 }
    ]
  },
  "summary": {
    "avg_cpu_usage": 35.2,
    "max_cpu_usage": 78.5,
    "avg_memory_usage": 52.1,
    "max_memory_usage": 85.2,
    "total_jobs_executed": 25
  }
}
```

**Example:**

```bash
# Get 24h metrics with 5m resolution
curl "http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/metrics?period=24h&resolution=5m"
```

## Job Execution APIs

### Submit Job to Runner

Submit a job directly to a specific runner.

**Endpoint:** `POST /runners/{runner_id}/jobs`

**Request Body:**

```json
{
  "name": "test-job",
  "steps": [
    {
      "name": "setup",
      "run": "echo 'Setting up environment'"
    },
    {
      "name": "build",
      "run": "echo 'Building application'"
    },
    {
      "name": "test",
      "run": "echo 'Running tests'"
    }
  ],
  "environment": {
    "NODE_ENV": "test",
    "DEBUG": "true"
  },
  "timeout": 300,
  "retry_count": 2
}
```

**Response:**

```json
{
  "success": true,
  "job_id": "job_456e7890-e89b-12d3-a456-426614174000",
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "status": "queued",
  "estimated_start_time": "2024-01-15T10:35:00Z"
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "hello-world",
    "steps": [
      {
        "name": "greet",
        "run": "echo Hello World"
      }
    ]
  }'
```

### Get Runner Jobs

Get list of jobs for a specific runner.

**Endpoint:** `GET /runners/{runner_id}/jobs`

**Query Parameters:**

- `status` (optional): Filter by job status
- `limit` (optional): Maximum number of results
- `offset` (optional): Pagination offset

**Response:**

```json
{
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "jobs": [
    {
      "job_id": "job_456e7890-e89b-12d3-a456-426614174000",
      "name": "test-job",
      "status": "completed",
      "created_at": "2024-01-15T10:00:00Z",
      "started_at": "2024-01-15T10:01:00Z",
      "completed_at": "2024-01-15T10:05:00Z",
      "duration": 240,
      "exit_code": 0
    }
  ],
  "total_count": 1
}
```

**Example:**

```bash
curl http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/jobs
```

## Runner Configuration APIs

### Get Runner Configuration

Get current configuration of a runner.

**Endpoint:** `GET /runners/{runner_id}/config`

**Response:**

```json
{
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "configuration": {
    "max_concurrent_jobs": 5,
    "job_timeout": 3600,
    "cleanup_policy": "always",
    "resource_limits": {
      "cpu_limit": "2000m",
      "memory_limit": "4Gi",
      "disk_limit": "10Gi"
    },
    "docker_config": {
      "privileged": false,
      "network_mode": "bridge",
      "volumes": ["/tmp:/tmp:rw"]
    }
  },
  "last_updated": "2024-01-15T09:00:00Z"
}
```

**Example:**

```bash
curl http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/config
```

### Update Runner Configuration

Update runner configuration.

**Endpoint:** `PUT /runners/{runner_id}/config`

**Request Body:**

```json
{
  "max_concurrent_jobs": 10,
  "job_timeout": 7200,
  "resource_limits": {
    "cpu_limit": "4000m",
    "memory_limit": "8Gi"
  }
}
```

**Response:**

```json
{
  "success": true,
  "message": "Configuration updated successfully",
  "restart_required": false
}
```

**Example:**

```bash
curl -X PUT http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/config \
  -H "Content-Type: application/json" \
  -d '{
    "max_concurrent_jobs": 8,
    "job_timeout": 1800
  }'
```

## Runner Control APIs

### Start Runner

Start a stopped runner.

**Endpoint:** `POST /runners/{runner_id}/start`

**Response:**

```json
{
  "success": true,
  "message": "Runner started successfully",
  "status": "starting"
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/start
```

### Stop Runner

Stop a running runner.

**Endpoint:** `POST /runners/{runner_id}/stop`

**Request Body (optional):**

```json
{
  "graceful": true,
  "timeout": 300,
  "reason": "Maintenance"
}
```

**Response:**

```json
{
  "success": true,
  "message": "Runner stopped successfully",
  "status": "stopped"
}
```

**Example:**

```bash
# Graceful stop
curl -X POST http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/stop \
  -H "Content-Type: application/json" \
  -d '{"graceful": true, "timeout": 300}'

# Immediate stop
curl -X POST http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/stop
```

### Restart Runner

Restart a runner.

**Endpoint:** `POST /runners/{runner_id}/restart`

**Request Body (optional):**

```json
{
  "graceful": true,
  "timeout": 300
}
```

**Response:**

```json
{
  "success": true,
  "message": "Runner restart initiated",
  "status": "restarting"
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/restart
```

## Runner Logs APIs

### Get Runner Logs

Get logs from a runner.

**Endpoint:** `GET /runners/{runner_id}/logs`

**Query Parameters:**

- `since` (optional): Start time for logs (ISO 8601 format)
- `until` (optional): End time for logs (ISO 8601 format)
- `level` (optional): Log level filter (`debug`, `info`, `warn`, `error`)
- `limit` (optional): Maximum number of log entries
- `follow` (optional): Stream logs in real-time

**Response:**

```json
{
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "logs": [
    {
      "timestamp": "2024-01-15T10:30:00Z",
      "level": "info",
      "message": "Job started: job_456e7890-e89b-12d3-a456-426614174000",
      "context": {
        "job_id": "job_456e7890-e89b-12d3-a456-426614174000",
        "step": "setup"
      }
    }
  ],
  "total_count": 1,
  "has_more": false
}
```

**Example:**

```bash
# Get recent logs
curl http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/logs

# Get logs for specific time range
curl "http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/logs?since=2024-01-15T10:00:00Z&until=2024-01-15T11:00:00Z"

# Stream logs (Server-Sent Events)
curl -H "Accept: text/event-stream" "http://localhost:8080/api/runners/runner_123e4567-e89b-12d3-a456-426614174000/logs?follow=true"
```

## Bulk Operations APIs

### Bulk Runner Operations

Perform operations on multiple runners.

**Endpoint:** `POST /runners/bulk`

**Request Body:**

```json
{
  "operation": "update_config",
  "filters": {
    "status": "active",
    "type": "docker",
    "tags": ["production"]
  },
  "data": {
    "max_concurrent_jobs": 8,
    "job_timeout": 1800
  }
}
```

**Response:**

```json
{
  "success": true,
  "affected_runners": 5,
  "results": [
    {
      "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
      "success": true
    }
  ]
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/api/runners/bulk \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "restart",
    "filters": {
      "status": "active",
      "type": "docker"
    }
  }'
```

## WebSocket APIs

### Real-time Runner Updates

Connect to WebSocket for real-time runner updates.

**Endpoint:** `WS /runners/ws`

**Connection:**

```javascript
const ws = new WebSocket("ws://localhost:8080/api/runners/ws");

ws.onmessage = function (event) {
  const data = JSON.parse(event.data);
  console.log("Runner update:", data);
};
```

**Message Format:**

```json
{
  "type": "runner_status_change",
  "runner_id": "runner_123e4567-e89b-12d3-a456-426614174000",
  "old_status": "active",
  "new_status": "inactive",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Error Responses

All APIs return consistent error responses:

```json
{
  "success": false,
  "error": {
    "code": "RUNNER_NOT_FOUND",
    "message": "Runner with ID 'runner_123' not found",
    "details": {
      "runner_id": "runner_123",
      "timestamp": "2024-01-15T10:30:00Z"
    }
  }
}
```

### Common Error Codes

| Code                      | Description                   |
| ------------------------- | ----------------------------- |
| `RUNNER_NOT_FOUND`        | Runner does not exist         |
| `RUNNER_UNAVAILABLE`      | Runner is not responding      |
| `INVALID_REQUEST`         | Request validation failed     |
| `AUTHENTICATION_REQUIRED` | Authentication token required |
| `AUTHORIZATION_FAILED`    | Insufficient permissions      |
| `RATE_LIMIT_EXCEEDED`     | Too many requests             |
| `INTERNAL_ERROR`          | Server internal error         |

## Rate Limiting

API endpoints are rate-limited to prevent abuse:

- **Registration**: 10 requests per minute per IP
- **Status/Health**: 100 requests per minute per runner
- **Job Submission**: 50 requests per minute per runner
- **Bulk Operations**: 5 requests per minute per user

Rate limit headers are included in responses:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1642248600
```

## SDK Examples

### Python SDK

```python
import requests
import json

class RustCIRunnerClient:
    def __init__(self, base_url, api_key=None):
        self.base_url = base_url
        self.headers = {'Content-Type': 'application/json'}
        if api_key:
            self.headers['X-API-Key'] = api_key

    def register_runner(self, runner_config):
        response = requests.post(
            f"{self.base_url}/api/runners/register",
            headers=self.headers,
            json=runner_config
        )
        return response.json()

    def get_runner_status(self, runner_id):
        response = requests.get(
            f"{self.base_url}/api/runners/{runner_id}/status",
            headers=self.headers
        )
        return response.json()

    def submit_job(self, runner_id, job_config):
        response = requests.post(
            f"{self.base_url}/api/runners/{runner_id}/jobs",
            headers=self.headers,
            json=job_config
        )
        return response.json()

# Usage
client = RustCIRunnerClient("http://localhost:8080", "your-api-key")

# Register runner
runner_config = {
    "name": "my-docker-runner",
    "type": "docker",
    "host": "localhost",
    "port": 8081,
    "capabilities": ["docker"]
}
result = client.register_runner(runner_config)
print(f"Runner registered: {result['runner_id']}")

# Submit job
job_config = {
    "name": "test-job",
    "steps": [{"name": "test", "run": "echo Hello"}]
}
job_result = client.submit_job(result['runner_id'], job_config)
print(f"Job submitted: {job_result['job_id']}")
```

### JavaScript SDK

```javascript
class RustCIRunnerClient {
  constructor(baseUrl, apiKey = null) {
    this.baseUrl = baseUrl;
    this.headers = {
      "Content-Type": "application/json",
    };
    if (apiKey) {
      this.headers["X-API-Key"] = apiKey;
    }
  }

  async registerRunner(runnerConfig) {
    const response = await fetch(`${this.baseUrl}/api/runners/register`, {
      method: "POST",
      headers: this.headers,
      body: JSON.stringify(runnerConfig),
    });
    return await response.json();
  }

  async getRunnerStatus(runnerId) {
    const response = await fetch(
      `${this.baseUrl}/api/runners/${runnerId}/status`,
      {
        headers: this.headers,
      }
    );
    return await response.json();
  }

  async submitJob(runnerId, jobConfig) {
    const response = await fetch(
      `${this.baseUrl}/api/runners/${runnerId}/jobs`,
      {
        method: "POST",
        headers: this.headers,
        body: JSON.stringify(jobConfig),
      }
    );
    return await response.json();
  }
}

// Usage
const client = new RustCIRunnerClient("http://localhost:8080", "your-api-key");

// Register runner
const runnerConfig = {
  name: "my-docker-runner",
  type: "docker",
  host: "localhost",
  port: 8081,
  capabilities: ["docker"],
};

client
  .registerRunner(runnerConfig)
  .then((result) => {
    console.log(`Runner registered: ${result.runner_id}`);

    // Submit job
    const jobConfig = {
      name: "test-job",
      steps: [{ name: "test", run: "echo Hello" }],
    };

    return client.submitJob(result.runner_id, jobConfig);
  })
  .then((jobResult) => {
    console.log(`Job submitted: ${jobResult.job_id}`);
  })
  .catch((error) => {
    console.error("Error:", error);
  });
```

This comprehensive API reference provides all the endpoints and examples needed to interact with RustCI runners programmatically.
