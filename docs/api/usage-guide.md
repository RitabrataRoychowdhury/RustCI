# RustCI API Usage Guide

This guide explains how to authenticate with RustCI and use the API to upload pipeline YAMLs and trigger executions.

## Authentication Flow

RustCI uses JWT-based authentication with GitHub OAuth. Here's how to get your admin token:

### Step 1: Start RustCI Server

First, ensure RustCI is running:

```bash
# Build and run RustCI (main binary)
cargo run --bin RustAutoDevOps

# Or using Docker
docker-compose up -d
```

The server will be available at `http://localhost:8000` by default.

**Note**: The project has multiple binaries:

- `RustAutoDevOps` - Main server application
- `generate_token` - Token generation utility

### Step 2: Get Admin Token

You have two options to get an admin token:

#### Option A: Generate Token (Easiest)

Use the built-in token generator:

```bash
# Generate a test JWT token with Developer role
cargo run --bin generate_token
```

This will output a JWT token that you can use immediately for API calls.

#### Option B: GitHub OAuth Authentication

1. **Navigate to OAuth endpoint**:

   ```bash
   curl http://localhost:8000/api/sessions/oauth/google
   ```

   Or open in browser: `http://localhost:8000/api/sessions/oauth/google`

2. **Click "Login with GitHub"** - This will redirect you to GitHub for authorization

3. **Authorize the application** - GitHub will redirect back to RustCI

4. **Extract the JWT token** from the response:
   ```json
   {
     "status": "success",
     "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..."
   }
   ```

### Step 3: Alternative - Direct Token Extraction

If you need to extract the token programmatically:

```bash
# Get OAuth URL and follow redirect
OAUTH_URL=$(curl -s -I http://localhost:8000/api/sessions/oauth/github | grep -i location | cut -d' ' -f2 | tr -d '\r')
echo "Visit this URL: $OAUTH_URL"

# After OAuth, the callback will contain your token
# You can also check the cookie in your browser's developer tools
```

## API Endpoints

### Base URL

```
http://localhost:8000
```

### Authentication Header

All protected endpoints require the JWT token:

```
Authorization: Bearer YOUR_JWT_TOKEN_HERE
```

## Pipeline Management

### 1. Upload Pipeline YAML

#### Method 1: JSON Upload

```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-pipeline",
    "description": "Test pipeline deployment",
    "yaml_content": "name: \"Test Pipeline\"\ndescription: \"Simple test\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Test\"\n    steps:\n      - name: \"echo-test\"\n        step_type: shell\n        config:\n          command: echo \"Hello World\""
  }'
```

#### Method 2: Multipart File Upload

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/docker-deployment-pipeline.yaml" \
  -F "name=docker-test-deployment" \
  -F "description=Docker deployment test pipeline"
```

### 2. List Pipelines

```bash
curl -X GET http://localhost:8000/api/ci/pipelines \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

### 3. Get Pipeline YAML

```bash
curl -X GET http://localhost:8000/api/ci/pipelines/PIPELINE_ID/yaml \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

### 4. Trigger Pipeline Execution

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/PIPELINE_ID/trigger \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "branch": "main",
    "environment_variables": {
      "BUILD_ID": "test-123",
      "ENVIRONMENT": "staging"
    }
  }'
```

## Execution Management

### 1. List Executions

```bash
curl -X GET http://localhost:8000/api/ci/executions \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

### 2. Get Execution Details

```bash
curl -X GET http://localhost:8000/api/ci/executions/EXECUTION_ID \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

### 3. Cancel Execution

```bash
curl -X DELETE http://localhost:8000/api/ci/executions/EXECUTION_ID/cancel \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

## Complete Example Workflow

Here's a complete example of uploading and triggering a pipeline:

### 1. Get Your Token

```bash
# Open browser to: http://localhost:8000/api/sessions/oauth/google
# Complete OAuth flow and extract token
export JWT_TOKEN="your_jwt_token_here"
```

### 2. Upload Docker Deployment Pipeline

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/docker-deployment-pipeline.yaml" \
  -F "name=docker-test-app" \
  -F "description=Deploy test application with Docker"
```

Response:

```json
{
  "status": "success",
  "pipeline_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "Pipeline created successfully"
}
```

### 3. Trigger the Pipeline

```bash
export PIPELINE_ID="550e8400-e29b-41d4-a716-446655440000"

curl -X POST http://localhost:8000/api/ci/pipelines/$PIPELINE_ID/trigger \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "branch": "main",
    "environment_variables": {
      "BUILD_ID": "manual-trigger-001"
    }
  }'
```

Response:

```json
{
  "status": "success",
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "message": "Pipeline execution started"
}
```

### 4. Monitor Execution

```bash
export EXECUTION_ID="123e4567-e89b-12d3-a456-426614174000"

# Check execution status
curl -X GET http://localhost:8000/api/ci/executions/$EXECUTION_ID \
  -H "Authorization: Bearer $JWT_TOKEN"
```

## Pipeline Examples

### Upload All Example Pipelines

```bash
# Docker deployment
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/docker-deployment-pipeline.yaml" \
  -F "name=docker-deployment" \
  -F "description=Docker deployment pipeline"

# K3s deployment
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/k3s-deployment-pipeline.yaml" \
  -F "name=k3s-deployment" \
  -F "description=K3s deployment pipeline"

# Build and test
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/build-test-pipeline.yaml" \
  -F "name=build-test" \
  -F "description=Build and test pipeline"

# Multi-environment deployment
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/multi-env-deployment-pipeline.yaml" \
  -F "name=multi-env-deploy" \
  -F "description=Multi-environment deployment"

# Security scan
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/security-scan-pipeline.yaml" \
  -F "name=security-scan" \
  -F "description=Security vulnerability scanning"

# Performance test
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -F "file=@docs/pipeline-examples/performance-test-pipeline.yaml" \
  -F "name=performance-test" \
  -F "description=Performance and load testing"
```

## User Management

### Get Current User Info

```bash
curl -X GET http://localhost:8000/api/sessions/me \
  -H "Authorization: Bearer $JWT_TOKEN"
```

### Logout

```bash
curl -X POST http://localhost:8000/api/sessions/logout \
  -H "Authorization: Bearer $JWT_TOKEN"
```

## Health Check (Public)

```bash
curl -X GET http://localhost:8000/healthchecker
```

## Troubleshooting

### Common Issues

1. **401 Unauthorized**

   - Check if your JWT token is valid and not expired
   - Ensure you're including the `Authorization: Bearer` header
   - Verify the token format is correct

2. **Token Expired**

   - JWT tokens expire after 1 day by default
   - Re-authenticate through GitHub OAuth to get a new token

3. **Pipeline Upload Fails**

   - Verify YAML syntax is correct
   - Check file permissions and path
   - Ensure multipart form data is properly formatted

4. **Pipeline Execution Fails**
   - Check if the pipeline ID exists
   - Verify you have execution permissions
   - Review pipeline YAML for syntax errors

### Debug Commands

```bash
# Check server health
curl -v http://localhost:8000/healthchecker

# Validate JWT token (decode without verification)
echo "YOUR_JWT_TOKEN" | cut -d'.' -f2 | base64 -d | jq .

# Check pipeline list
curl -s http://localhost:8000/api/ci/pipelines \
  -H "Authorization: Bearer $JWT_TOKEN" | jq .
```

## Environment Variables

The following environment variables affect API behavior:

- `JWT_SECRET`: Secret key for JWT signing
- `JWT_EXPIRED_IN`: Token expiration time (default: 1d)
- `GITHUB_OAUTH_CLIENT_ID`: GitHub OAuth client ID
- `GITHUB_OAUTH_CLIENT_SECRET`: GitHub OAuth client secret
- `PORT`: Server port (default: 8000)

## Security Notes

- JWT tokens contain sensitive information - keep them secure
- Tokens are valid for 1 day by default
- All API endpoints except health check require authentication
- OAuth flow requires GitHub account access
- Admin role provides full system access including user management

## Rate Limiting

Currently, there are no rate limits implemented, but in production you should consider:

- Request rate limiting per user
- Pipeline execution limits
- File upload size restrictions
- Concurrent execution limits
