# RustCI Test Application

A simple HTTP service built with Rust and Axum for testing RustCI deployment pipelines.

## Features

- **Health Check Endpoint**: `/health` - Returns service health status
- **Echo Service**: `/echo` - Echoes back JSON messages
- **Greeting Service**: `/greet` - Customizable greeting with query parameters
- **Info Endpoint**: `/info` - Application metadata and available endpoints
- **Structured Logging**: Request tracing and application logs
- **Docker Support**: Multi-stage Dockerfile for efficient builds
- **Kubernetes Ready**: K8s manifests with health checks and security context

## API Endpoints

### GET /
Returns basic application information and available endpoints.

### GET /health
Health check endpoint that returns:
```json
{
  "status": "healthy",
  "timestamp": "2025-01-29T10:30:00Z",
  "version": "1.0.0",
  "uptime_seconds": 3600,
  "service": "rustci-test-app"
}
```

### POST /echo
Echoes back a JSON message:

**Request:**
```json
{
  "message": "Hello, RustCI!"
}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "original_message": "Hello, RustCI!",
  "echo": "Echo: Hello, RustCI!",
  "timestamp": "2025-01-29T10:30:00Z"
}
```

### GET /greet?name=<name>&count=<count>
Greeting service with optional query parameters:
- `name` (optional): Name to greet (default: "World")
- `count` (optional): Number of greetings (default: 1)

**Example:** `/greet?name=RustCI&count=3`

## Building and Running

### Local Development
```bash
cd test-app
cargo run
```

### Docker
```bash
cd test-app
docker build -t rustci-test-app:latest .
docker run -p 8080:8080 rustci-test-app:latest
```

### Kubernetes
```bash
cd test-app
kubectl apply -f k8s-manifests/
```

## Testing

Test the health endpoint:
```bash
curl http://localhost:8080/health
```

Test the echo service:
```bash
curl -X POST http://localhost:8080/echo \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello from RustCI!"}'
```

Test the greeting service:
```bash
curl "http://localhost:8080/greet?name=RustCI&count=2"
```

## Configuration

The application uses the following environment variables:
- `RUST_LOG`: Log level configuration (default: "test_app=info")
- `PORT`: Server port (default: 8080)

## Security

- Runs as non-root user (UID 1000)
- Minimal Alpine-based runtime image
- Security context configured for Kubernetes
- Health checks configured for container orchestration