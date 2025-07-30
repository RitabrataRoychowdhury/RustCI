# Current API State Documentation

## Overview

This document provides a comprehensive analysis of the current RustCI API state, including working endpoints, authentication behavior, and identified gaps.

**Generated on:** 2025-07-27  
**Server Version:** 0.1.0  
**Base URL:** http://localhost:8000

## Server Status

✅ **Server Status:** Running successfully on port 8000  
✅ **Health Check:** Working (`/health`)  
✅ **Basic Routing:** Functional  
⚠️ **Authentication:** Required for most endpoints  

## Authentication Behavior

### Current Authentication System

The system uses **JWT-based authentication** with the following characteristics:

- **Token Sources:** 
  - Cookie: `token`
  - Authorization header: `Bearer <token>`
- **Middleware:** Enhanced security middleware with RBAC support
- **Public Endpoints:** Limited (health check, OAuth initiation)
- **Protected Endpoints:** All API endpoints require authentication

### Authentication Flow

1. **OAuth Initiation:** `/api/sessions/oauth/github` - ✅ Working
2. **OAuth Callback:** `/api/sessions/oauth/github/callback` - ❓ Untested
3. **Token Verification:** JWT tokens validated on each request
4. **Security Context:** Creates SecurityContext with roles and permissions

### Current Authentication Gaps

❌ **Missing Runner Lifecycle Authentication:** No specific runner registration/management auth  
❌ **Missing API Testing Tools:** No helper scripts for authenticated requests  
❌ **Missing Documentation:** No cURL examples with auth headers  
❌ **Missing Error Documentation:** Auth error responses not documented  

## API Endpoints Analysis

### Working Endpoints

#### Health & Status
- `GET /health` - ✅ **Working** (Public)
  ```bash
  curl http://localhost:8000/health
  ```
  - Returns comprehensive health status
  - Includes database, memory, disk checks
  - System information included

#### Authentication Endpoints
- `GET /api/sessions/oauth/github` - ✅ **Working** (Public)
  - Redirects to GitHub OAuth
  - Generates state parameter
  - Configured with client ID

#### Protected Endpoints (Require Authentication)
- `GET /api-docs/openapi.json` - 🔒 **Protected**
- `GET /api/ci/test` - 🔒 **Protected**
- All CI pipeline endpoints - 🔒 **Protected**
- All cluster management endpoints - 🔒 **Protected**

### CI Pipeline Endpoints

Based on code analysis, the following CI endpoints exist but require authentication:

#### Pipeline Management
- `POST /api/ci/pipelines` - Create pipeline from JSON
- `POST /api/ci/pipelines/upload` - Create pipeline from file upload
- `GET /api/ci/pipelines` - List all pipelines
- `GET /api/ci/pipelines/{id}/yaml` - Get pipeline YAML

#### Pipeline Execution
- `POST /api/ci/pipelines/{id}/trigger` - Trigger pipeline
- `POST /api/ci/pipelines/{id}/webhook` - Webhook trigger
- `GET /api/ci/executions` - List executions
- `GET /api/ci/executions/{id}` - Get execution details
- `DELETE /api/ci/executions/{id}/cancel` - Cancel execution

### Cluster Management Endpoints

#### Node Management
- `POST /api/cluster/nodes` - Join node to cluster
- `DELETE /api/cluster/nodes/{id}` - Remove node from cluster
- `GET /api/cluster/nodes` - List cluster nodes
- `GET /api/cluster/nodes/{id}/health` - Get node health

#### Runner Management (Placeholder Implementations)
- `GET /api/cluster/runners` - List runners
- `POST /api/cluster/runners` - Create runner
- `GET /api/cluster/runners/{id}/status` - Get runner status
- `DELETE /api/cluster/runners/{id}` - Delete runner
- `GET /api/cluster/runners/{id}/metrics` - Get runner metrics

#### Job Management (Placeholder Implementations)
- `GET /api/cluster/jobs` - List jobs
- `GET /api/cluster/jobs/{id}/status` - Get job status
- `POST /api/cluster/jobs/{id}/cancel` - Cancel job
- `POST /api/cluster/jobs/{id}/retry` - Retry job

## Missing Runner Lifecycle APIs

The following critical runner lifecycle endpoints are **NOT IMPLEMENTED**:

❌ `POST /api/runners` - Register runner  
❌ `POST /api/runners/{id}/jobs` - Trigger job on runner  
❌ `GET /api/runners/{id}/jobs/{job_id}/logs` - Fetch job logs  
❌ `GET /api/runners/{id}/jobs/{job_id}/artifacts` - Fetch job artifacts  
❌ `DELETE /api/runners/{id}` - Deregister runner  

## Error Response Analysis

### Authentication Errors

**401 Unauthorized Response:**
```json
{
  "error": "Authentication required",
  "status": 401,
  "timestamp": "2025-07-27T11:11:48.410270+00:00"
}
```

### Security Middleware Behavior

The security middleware logs the following events:
- Authentication attempts
- Authorization failures
- Critical security events
- Request completion with timing

## OpenAPI/Swagger Documentation

### Current State
- **Endpoint:** `/api-docs/openapi.json`
- **Status:** 🔒 Protected (requires authentication)
- **Issue:** Cannot access documentation without token

### Schema Analysis

Based on code review, the OpenAPI schema includes:

#### Defined Schemas
- Domain entities (ClusterNode, RunnerEntity, Job, etc.)
- Error responses (ApiError)
- Request/response models for CI and cluster operations

#### Missing Documentation
- Security scheme definitions
- Authentication examples
- Error response examples
- Working cURL examples

## Test Scripts Analysis

### Existing Scripts

#### `scripts/test-runner.sh`
- ✅ Comprehensive test runner
- ✅ Supports unit, integration, load, chaos tests
- ✅ Coverage reporting
- ❌ No API testing capabilities

#### `scripts/create_ssh_linux_server.sh`
- ✅ Creates Docker container with SSH access
- ✅ Docker-in-Docker support
- ⚠️ May have hanging issues (needs verification)

#### `scripts/k8s-test-server.sh`
- ✅ Creates k3s cluster in container
- ✅ SSH access configured
- ⚠️ May have hanging issues (needs verification)

### Missing Test Scripts

❌ **API Authentication Helper:** No script to obtain JWT tokens  
❌ **Runner Registration Script:** No script to register runners  
❌ **Job Trigger Script:** No script to trigger jobs  
❌ **Log Fetching Script:** No script to fetch logs  
❌ **Artifact Download Script:** No script to download artifacts  
❌ **End-to-End API Test:** No comprehensive API workflow test  

## Recommendations

### Immediate Actions Required

1. **Fix Authentication Integration**
   - Update existing handlers to use proper auth middleware
   - Implement missing runner lifecycle endpoints
   - Add SecurityContext extraction

2. **Create API Testing Tools**
   - Build helper scripts for common operations
   - Add authentication examples
   - Create end-to-end test scripts

3. **Update Documentation**
   - Make OpenAPI spec publicly accessible
   - Add security scheme documentation
   - Include working cURL examples

4. **Fix Runner Scripts**
   - Debug hanging issues in k8s-test-server.sh
   - Improve error handling in create_ssh_linux_server.sh
   - Add comprehensive logging

### Testing Strategy

1. **Authentication Testing**
   - Test OAuth flow end-to-end
   - Verify JWT token generation and validation
   - Test role-based access control

2. **API Endpoint Testing**
   - Test all CI pipeline operations
   - Test cluster management operations
   - Test error scenarios

3. **Runner Integration Testing**
   - Test fake runner registration
   - Test job execution flow
   - Test log and artifact retrieval

## Current Working cURL Examples

### Health Check (Public)
```bash
curl -X GET http://localhost:8000/health
```

### GitHub OAuth Initiation (Public)
```bash
curl -X GET http://localhost:8000/api/sessions/oauth/github
# Returns 303 redirect to GitHub
```

### Protected Endpoint (Requires Token)
```bash
# This will fail without authentication
curl -X GET http://localhost:8000/api/ci/test
# Returns: {"error":"Authentication required","status":401}

# With authentication (token needed)
curl -X GET \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  http://localhost:8000/api/ci/test
```

## Next Steps

1. Implement missing runner lifecycle APIs
2. Create authentication helper scripts
3. Fix local fake runner scripts
4. Generate comprehensive API documentation
5. Build end-to-end testing infrastructure