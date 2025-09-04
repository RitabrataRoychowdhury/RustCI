# API Reference

This section contains detailed API reference documentation for all RustCI endpoints.

## API Overview

RustCI provides a comprehensive REST API for managing CI/CD pipelines, workspaces, and executions. The API is organized into the following main categories:

### Core Resources
- **[Users](users.md)** - User management and authentication
- **[Workspaces](workspaces.md)** - Project workspace management
- **[Pipelines](pipelines.md)** - Pipeline configuration and management
- **[Executions](executions.md)** - Pipeline execution control and monitoring

### System Resources
- **[Health](health.md)** - System health and status endpoints
- **[Webhooks](webhooks.md)** - Webhook integration endpoints
- **[WebSocket](websocket.md)** - Real-time communication endpoints

## Quick Reference

### Base URL
```
https://api.rustci.dev/v1
```

### Authentication
All API requests require authentication using JWT tokens:
```
Authorization: Bearer <your-jwt-token>
```

### Common Response Format
All API responses follow a consistent JSON format:

#### Success Response
```json
{
  "data": { ... },
  "meta": {
    "timestamp": "2025-01-09T10:00:00Z",
    "version": "1.0.0"
  }
}
```

#### Error Response
```json
{
  "error": {
    "type": "ValidationError",
    "message": "Invalid request parameters",
    "details": { ... },
    "timestamp": "2025-01-09T10:00:00Z",
    "correlation_id": "uuid"
  }
}
```

### HTTP Status Codes
- `200` - OK (Success)
- `201` - Created
- `202` - Accepted (Async operation started)
- `204` - No Content (Success with no response body)
- `400` - Bad Request
- `401` - Unauthorized
- `403` - Forbidden
- `404` - Not Found
- `409` - Conflict
- `422` - Unprocessable Entity
- `429` - Too Many Requests
- `500` - Internal Server Error

### Rate Limiting
- **Per User**: 1000 requests/hour, 100 requests/minute
- **Headers**: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`

## API Endpoints by Category

### Authentication & Users
```
POST   /auth/github              # GitHub OAuth login
GET    /auth/github/callback     # OAuth callback
GET    /api/users/me             # Get current user
PUT    /api/users/me             # Update current user
```

### Workspaces
```
GET    /api/workspaces           # List workspaces
POST   /api/workspaces           # Create workspace
GET    /api/workspaces/{id}      # Get workspace
PUT    /api/workspaces/{id}      # Update workspace
DELETE /api/workspaces/{id}      # Delete workspace
```

### Pipelines
```
GET    /api/workspaces/{id}/pipelines     # List pipelines
POST   /api/workspaces/{id}/pipelines     # Create pipeline
GET    /api/pipelines/{id}                # Get pipeline
PUT    /api/pipelines/{id}                # Update pipeline
DELETE /api/pipelines/{id}                # Delete pipeline
```

### Executions
```
POST   /api/pipelines/{id}/executions     # Start execution
GET    /api/executions/{id}               # Get execution
POST   /api/executions/{id}/cancel        # Cancel execution
GET    /api/executions/{id}/logs/{stage}/{step}  # Get logs
```

### System
```
GET    /health                   # Health check
POST   /webhooks/github          # GitHub webhook
WS     /ws/executions/{id}       # WebSocket connection
```

## SDK and Integration Examples

### cURL Examples
```bash
# Get user information
curl -H "Authorization: Bearer $TOKEN" \
  https://api.rustci.dev/v1/api/users/me

# Create a workspace
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"my-project","repository_url":"https://github.com/user/repo"}' \
  https://api.rustci.dev/v1/api/workspaces
```

### JavaScript/Node.js
```javascript
const RustCIClient = require('@rustci/client');

const client = new RustCIClient({
  baseURL: 'https://api.rustci.dev/v1',
  token: process.env.RUSTCI_TOKEN
});

// List workspaces
const workspaces = await client.workspaces.list();

// Start pipeline execution
const execution = await client.executions.start(pipelineId, {
  trigger: 'manual',
  branch: 'main'
});
```

### Python
```python
import rustci

client = rustci.Client(
    base_url='https://api.rustci.dev/v1',
    token=os.environ['RUSTCI_TOKEN']
)

# List workspaces
workspaces = client.workspaces.list()

# Start pipeline execution
execution = client.executions.start(pipeline_id, {
    'trigger': 'manual',
    'branch': 'main'
})
```

## OpenAPI Specification

The complete API specification is available in OpenAPI 3.0 format:
- **[OpenAPI JSON](../openapi-baseline.json)** - Machine-readable specification
- **Interactive Documentation** - Available at `https://api.rustci.dev/docs`

## Versioning

The RustCI API uses semantic versioning:
- **Current Version**: v1.0.0
- **API Version**: v1 (in URL path)
- **Backward Compatibility**: Maintained within major versions

## Support

- **[Usage Guide](../usage-guide.md)** - Common usage patterns and examples
- **[Configuration Schema](../configuration-schema.md)** - Configuration reference
- **[GitHub Issues](https://github.com/rustci/rustci/issues)** - Report API issues
- **[Community Discussions](https://github.com/rustci/rustci/discussions)** - Ask questions

---

**Next Steps**: Choose a specific resource from the list above to explore detailed endpoint documentation.