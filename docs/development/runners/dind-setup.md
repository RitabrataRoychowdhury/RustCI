# Docker-in-Docker (DIND) Setup Guide

This guide covers setting up and using Docker-in-Docker for isolated RustCI runner testing and development.

## Overview

Docker-in-Docker (DIND) provides a completely isolated Docker environment for testing RustCI deployments without affecting your host Docker installation. This is particularly useful for:

- Testing deployment pipelines safely
- Simulating production environments
- Cross-platform development (Mac to Windows deployments)
- CI/CD pipeline validation

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Host Machine                            │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │   RustCI Server │    │        DIND Container           │ │
│  │   (Port 8080)   │    │  ┌─────────────────────────────┐ │ │
│  │                 │    │  │      Docker Daemon         │ │ │
│  │                 │    │  │      (Port 2376)           │ │ │
│  │                 │    │  └─────────────────────────────┘ │ │
│  │                 │    │  ┌─────────────────────────────┐ │ │
│  │                 │    │  │    Runner Container        │ │ │
│  │                 │    │  │                             │ │ │
│  │                 │    │  └─────────────────────────────┘ │ │
│  └─────────────────┘    └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Basic DIND Setup

```bash
# Start DIND environment
./scripts/runners/setup-dind-environment.sh start

# Test DIND functionality
./scripts/runners/setup-dind-environment.sh test

# Check status
./scripts/runners/setup-dind-environment.sh status
```

### 2. Mac to Windows DIND Runner

For cross-platform testing (Mac development, Windows deployment):

```bash
# Set environment variables
export WINDOWS_HOST="192.168.1.100"  # Your Windows machine IP
export WINDOWS_USER="your-username"   # Your Windows username

# Start Mac DIND runner
./scripts/runners/mac-dind-runner.sh start

# Check runner status
./scripts/runners/mac-dind-runner.sh status
```

## Detailed Setup Instructions

### Prerequisites

1. **Docker Desktop** (Mac/Windows) or **Docker Engine** (Linux)
2. **SSH access** to target Windows machine (if using cross-platform setup)
3. **Network connectivity** between machines

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `DIND_CONTAINER_NAME` | DIND container name | `rustci-dind` | No |
| `DIND_PORT` | Docker daemon port | `2376` | No |
| `DIND_NETWORK` | Docker network name | `rustci-network` | No |
| `WINDOWS_HOST` | Windows target IP | - | Yes (for cross-platform) |
| `WINDOWS_USER` | Windows username | - | Yes (for cross-platform) |
| `RUSTCI_SERVER` | RustCI server URL | `http://localhost:8080` | No |

### Step-by-Step Setup

#### 1. Start DIND Environment

```bash
# Create and start DIND container
./scripts/runners/setup-dind-environment.sh start
```

This will:
- Create a Docker network (`rustci-network`)
- Start a privileged DIND container
- Wait for Docker daemon to be ready
- Test basic Docker functionality

#### 2. Verify DIND Installation

```bash
# Check DIND status
./scripts/runners/setup-dind-environment.sh status

# Test Docker commands inside DIND
docker exec rustci-dind docker version
docker exec rustci-dind docker run --rm hello-world
```

#### 3. Set Up Cross-Platform Runner (Optional)

For Mac to Windows deployments:

```bash
# Configure Windows target
export WINDOWS_HOST="192.168.1.100"
export WINDOWS_USER="administrator"

# Start Mac DIND runner
./scripts/runners/mac-dind-runner.sh start
```

## Configuration Files

### DIND Container Configuration

The DIND container is configured with:

```yaml
# Equivalent docker-compose.yml
version: '3.8'
services:
  rustci-dind:
    image: docker:dind
    privileged: true
    networks:
      - rustci-network
    ports:
      - "2376:2376"
    environment:
      - DOCKER_TLS_CERTDIR=/certs
    volumes:
      - rustci-dind-certs:/certs/client
      - rustci-dind-data:/var/lib/docker
```

### Runner Configuration

```yaml
# /tmp/runner-config.yaml
runner:
  name: "mac-dind-runner"
  type: "docker"
  host: "0.0.0.0"
  port: 8081
  max_concurrent_jobs: 3

docker:
  privileged: true
  network: "rustci-network"
  volumes:
    - "/var/run/docker.sock:/var/run/docker.sock"

deployment:
  windows_host: "${WINDOWS_HOST}"
  windows_user: "${WINDOWS_USER}"

logging:
  level: "info"
  format: "json"
```

## Testing and Validation

### 1. Basic DIND Tests

```bash
# Test Docker functionality
./scripts/runners/setup-dind-environment.sh test

# Manual testing
docker exec rustci-dind docker pull alpine:latest
docker exec rustci-dind docker run --rm alpine:latest echo "Hello from DIND"
```

### 2. Runner Integration Tests

```bash
# Test runner registration
./scripts/runners/test-rustci-runner-integration.sh

# Test container connectivity
./scripts/runners/test-runner-container-connection.sh
```

### 3. Cross-Platform Deployment Test

```bash
# Run the test pipeline
rustci run mac-to-windows-pipeline.yaml

# Or use the deployment test script
./scripts/deployment/quick-deploy-test.sh
```

## API Testing

### Runner Registration API

```bash
# Register DIND runner
curl -X POST http://localhost:8080/api/runners/register \
  -H "Content-Type: application/json" \
  -d '{
    "name": "dind-runner",
    "type": "docker",
    "host": "localhost",
    "port": 2376,
    "capabilities": ["docker", "isolation"],
    "metadata": {
      "os": "linux",
      "arch": "amd64",
      "docker_version": "24.0.0"
    }
  }'
```

### Runner Status API

```bash
# Get runner status
curl http://localhost:8080/api/runners/dind-runner/status

# Get runner health
curl http://localhost:8080/api/runners/dind-runner/health

# List all runners
curl http://localhost:8080/api/runners
```

### Job Execution API

```bash
# Submit job to DIND runner
curl -X POST http://localhost:8080/api/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-job",
    "runner": "dind-runner",
    "steps": [
      {
        "name": "hello-world",
        "run": "echo Hello from DIND runner"
      }
    ]
  }'
```

## Troubleshooting

### Common Issues

#### 1. DIND Container Won't Start

**Symptoms:**
- Container exits immediately
- "Cannot connect to Docker daemon" errors

**Solutions:**
```bash
# Check Docker daemon status
docker info

# Restart Docker Desktop/Engine
# On Mac: Restart Docker Desktop
# On Linux: sudo systemctl restart docker

# Clean up and retry
./scripts/runners/setup-dind-environment.sh cleanup
./scripts/runners/setup-dind-environment.sh start
```

#### 2. Network Connectivity Issues

**Symptoms:**
- Cannot reach DIND container
- Port binding failures

**Solutions:**
```bash
# Check network configuration
docker network ls
docker network inspect rustci-network

# Check port availability
netstat -an | grep 2376

# Use different port if needed
export DIND_PORT=2377
./scripts/runners/setup-dind-environment.sh start
```

#### 3. Cross-Platform SSH Issues

**Symptoms:**
- SSH connection failures to Windows
- Permission denied errors

**Solutions:**
```bash
# Test SSH connectivity
ssh -v $WINDOWS_USER@$WINDOWS_HOST

# Set up SSH keys
ssh-copy-id $WINDOWS_USER@$WINDOWS_HOST

# Check Windows OpenSSH server
# On Windows: Get-Service sshd
```

#### 4. Runner Registration Failures

**Symptoms:**
- Runner not appearing in RustCI
- Registration API errors

**Solutions:**
```bash
# Check RustCI server status
curl http://localhost:8080/health

# Verify runner configuration
cat /tmp/runner-config.yaml

# Check runner logs
docker logs mac-dind-runner
```

### Debug Commands

```bash
# DIND container logs
docker logs rustci-dind

# Runner container logs
docker logs mac-dind-runner

# Network inspection
docker network inspect rustci-network

# Container inspection
docker inspect rustci-dind

# Test Docker daemon inside DIND
docker exec rustci-dind docker info
```

## Performance Optimization

### Resource Limits

```bash
# Start DIND with resource limits
docker run -d \
  --name rustci-dind \
  --privileged \
  --memory=4g \
  --cpus=2 \
  --network rustci-network \
  -p 2376:2376 \
  docker:dind
```

### Volume Optimization

```bash
# Use tmpfs for better performance
docker run -d \
  --name rustci-dind \
  --privileged \
  --tmpfs /tmp \
  --tmpfs /var/lib/docker \
  docker:dind
```

### Network Optimization

```bash
# Use host networking for better performance
docker run -d \
  --name rustci-dind \
  --privileged \
  --network host \
  docker:dind
```

## Security Considerations

### Container Security

1. **Privileged Mode**: DIND requires privileged mode, which has security implications
2. **Network Isolation**: Use dedicated networks for DIND containers
3. **Volume Mounts**: Limit volume mounts to necessary directories only
4. **Resource Limits**: Apply CPU and memory limits to prevent resource exhaustion

### SSH Security

1. **Key-based Authentication**: Use SSH keys instead of passwords
2. **Network Segmentation**: Isolate DIND network from production networks
3. **Firewall Rules**: Configure appropriate firewall rules for SSH access
4. **User Permissions**: Use least-privilege principles for SSH users

## Advanced Configuration

### Custom Docker Daemon Configuration

```json
// /etc/docker/daemon.json inside DIND
{
  "storage-driver": "overlay2",
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  },
  "registry-mirrors": ["https://mirror.gcr.io"]
}
```

### Multi-Architecture Support

```bash
# Enable buildx for multi-arch builds
docker exec rustci-dind docker buildx create --use
docker exec rustci-dind docker buildx inspect --bootstrap
```

### Custom Runner Images

```dockerfile
# Custom DIND runner image
FROM docker:dind

# Install additional tools
RUN apk add --no-cache \
    openssh-client \
    curl \
    jq \
    git

# Copy runner configuration
COPY runner-config.yaml /config/

# Custom entrypoint
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
```

## Cleanup and Maintenance

### Regular Cleanup

```bash
# Clean up DIND environment
./scripts/runners/setup-dind-environment.sh cleanup

# Remove unused Docker resources
docker system prune -f

# Clean up volumes
docker volume prune -f
```

### Automated Maintenance

```bash
#!/bin/bash
# maintenance.sh - Run daily

# Clean up old containers
docker container prune -f --filter "until=24h"

# Clean up old images
docker image prune -f --filter "until=48h"

# Clean up old volumes
docker volume prune -f

# Restart DIND if needed
if ! docker exec rustci-dind docker info &>/dev/null; then
    ./scripts/runners/setup-dind-environment.sh restart
fi
```

## Integration with CI/CD

### GitHub Actions

```yaml
# .github/workflows/dind-test.yml
name: DIND Integration Test

on: [push, pull_request]

jobs:
  dind-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup DIND
        run: ./scripts/runners/setup-dind-environment.sh start
        
      - name: Test DIND
        run: ./scripts/runners/setup-dind-environment.sh test
        
      - name: Run integration tests
        run: ./scripts/runners/test-rustci-runner-integration.sh
```

### GitLab CI

```yaml
# .gitlab-ci.yml
dind-test:
  image: docker:latest
  services:
    - docker:dind
  variables:
    DOCKER_HOST: tcp://docker:2376
    DOCKER_TLS_CERTDIR: "/certs"
  script:
    - ./scripts/runners/setup-dind-environment.sh test
    - ./scripts/runners/test-rustci-runner-integration.sh
```

This comprehensive DIND setup guide provides everything needed to set up, configure, test, and maintain Docker-in-Docker environments for RustCI runner development and testing.