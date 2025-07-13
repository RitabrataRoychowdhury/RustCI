# CI/CD Deployment Guide

This guide explains how to use the enhanced CI/CD system with real deployment capabilities.

## Overview

The CI/CD system now supports:
- **Repository cloning** with GitHub OAuth tokens
- **Multi-language builds** (Node.js, Python, Rust, Java, Go, .NET, Static)
- **Local deployments** to directories and Docker containers
- **Port management** for service exposure
- **Artifact collection** and management

## Quick Start

### 1. Create a Pipeline

```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"My App\"\nstages:\n  - name: \"Build\"\n    steps:\n      - name: \"build-app\"\n        step_type: \"shell\"\n        config:\n          command: \"npm run build\""
  }'
```

### 2. Trigger a Pipeline

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{pipeline_id}/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "GITHUB_TOKEN": "your_github_token_here",
      "PORTS": "3000:3000"
    }
  }'
```

## Deployment Types

### Local Directory Deployment

Deploys built artifacts to a local directory:

```yaml
- name: "deploy-to-directory"
  step_type: custom
  config:
    target_directory: "/opt/my-app"
```

### Docker Container Deployment

Builds and runs a Docker container:

```yaml
- name: "deploy-to-docker"
  step_type: docker
  config:
    image: "my-app"
    dockerfile: "Dockerfile"
    environment:
      PORTS: "8080:8080"
      DISTROLESS: "true"
```

### Hybrid Deployment

Combines both directory and container deployment:

```yaml
- name: "deploy-hybrid"
  step_type: custom
  config:
    deployment_type: "hybrid"
    environment:
      PORTS: "3000:3000,8080:8080"
```

## Repository Integration

### GitHub Repository Cloning

```yaml
- name: "clone-repository"
  step_type: github
  config:
    repository_url: "https://github.com/user/repo"
    branch: "main"
    commit: "abc123"  # Optional specific commit
```

Environment variables needed:
- `GITHUB_TOKEN`: Your GitHub access token

### Private Repository Access

The system uses GitHub OAuth tokens for private repository access:

1. Authenticate via `/api/sessions/oauth/github`
2. Use the token in pipeline environment variables
3. The system automatically handles authentication

## Build System Support

### Node.js Projects

Automatically detected by `package.json`:

```yaml
- name: "build-nodejs"
  step_type: shell
  config:
    command: "npm ci && npm run build"
```

### Python Projects

Detected by `requirements.txt` or `pyproject.toml`:

```yaml
- name: "build-python"
  step_type: shell
  config:
    script: |
      python -m venv venv
      source venv/bin/activate
      pip install -r requirements.txt
```

### Rust Projects

Detected by `Cargo.toml`:

```yaml
- name: "build-rust"
  step_type: shell
  config:
    command: "cargo build --release"
```

### Java Projects

Detected by `pom.xml` or `build.gradle`:

```yaml
- name: "build-java"
  step_type: shell
  config:
    command: "mvn clean package -DskipTests"
```

## Port Management

The system automatically manages ports for deployed services:

### Automatic Port Allocation

```yaml
environment:
  PORTS: "0:8080"  # Host port auto-allocated
```

### Specific Port Mapping

```yaml
environment:
  PORTS: "3000:3000,8080:8080"  # Multiple port mappings
```

### Service Discovery

Deployed services are accessible at:
- `http://localhost:{allocated_port}`
- Service endpoints are returned in deployment results

## Docker Features

### Automatic Dockerfile Generation

If no Dockerfile exists, the system generates one based on project type:

```yaml
- name: "deploy-auto-docker"
  step_type: docker
  config:
    image: "my-app"
    # Dockerfile automatically generated
```

### Distroless Images

For security and smaller image sizes:

```yaml
environment:
  DISTROLESS: "true"
```

### Custom Base Images

```yaml
config:
  base_image: "node:18-alpine"
  distroless: false
```

## Environment Variables

### Pipeline-Level Variables

```yaml
environment:
  NODE_ENV: "production"
  DATABASE_URL: "${DATABASE_URL}"
```

### Step-Level Variables

```yaml
- name: "deploy-with-env"
  step_type: custom
  config:
    environment:
      SPECIFIC_VAR: "value"
```

### Runtime Variables

```bash
curl -X POST .../trigger \
  -d '{
    "environment": {
      "GITHUB_TOKEN": "token_here",
      "CUSTOM_VAR": "runtime_value"
    }
  }'
```

## Artifact Management

### Automatic Artifact Collection

The system automatically collects:
- Built binaries and executables
- Static files (HTML, CSS, JS)
- Archives (tar.gz, zip)
- Docker images
- Configuration files

### Artifact Storage

Artifacts are stored in:
- Local deployment directories
- Docker registry (if configured)
- Workspace directories during build

## Monitoring and Logs

### Execution Status

```bash
curl http://localhost:8000/api/ci/executions/{execution_id}
```

### Real-time Logs

Pipeline execution logs include:
- Build command outputs
- Deployment progress
- Error messages
- Service startup logs

### Service Health

Deployed services can be monitored via:
- HTTP health check endpoints
- Container status checks
- Process monitoring

## Advanced Features

### Parallel Execution

```yaml
- name: "Parallel Build"
  parallel: true
  steps:
    - name: "build-frontend"
      step_type: shell
      config:
        command: "npm run build"
    - name: "build-backend"
      step_type: shell
      config:
        command: "cargo build --release"
```

### Conditional Steps

```yaml
- name: "conditional-deploy"
  step_type: custom
  condition: "${BRANCH} == 'main'"
  config:
    # Only deploy on main branch
```

### Custom Build Commands

```yaml
- name: "custom-build"
  step_type: shell
  config:
    script: |
      echo "Custom build process"
      make clean
      make build
      make test
```

## Troubleshooting

### Common Issues

1. **Repository Clone Fails**
   - Check GitHub token permissions
   - Verify repository URL
   - Ensure token has repo access

2. **Build Fails**
   - Check build dependencies
   - Verify project structure
   - Review build logs

3. **Deployment Fails**
   - Check Docker daemon status
   - Verify port availability
   - Review deployment logs

4. **Port Conflicts**
   - Use automatic port allocation
   - Check for running services
   - Review port mappings

### Debug Mode

Enable verbose logging:

```bash
RUST_LOG=debug cargo run
```

### Log Analysis

Check execution logs for detailed information:
- Build command outputs
- Docker build logs
- Deployment status
- Error stack traces

## Security Considerations

### Token Management

- Store GitHub tokens securely
- Use environment variables for secrets
- Rotate tokens regularly

### Container Security

- Use distroless images when possible
- Scan images for vulnerabilities
- Limit container privileges

### Network Security

- Configure firewall rules for exposed ports
- Use HTTPS for webhooks
- Implement proper authentication

## Performance Optimization

### Build Caching

- Enable build caching for faster builds
- Use Docker layer caching
- Cache dependencies between builds

### Resource Management

- Configure parallel job limits
- Monitor system resources
- Implement build timeouts

### Storage Management

- Clean up old artifacts
- Manage workspace sizes
- Implement retention policies