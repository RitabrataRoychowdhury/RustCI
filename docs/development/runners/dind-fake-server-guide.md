# DIND to Fake Server Deployment Guide

This guide provides step-by-step instructions for deploying a Node.js Hello World application using a DIND (Docker-in-Docker) runner to a fake server environment.

## Overview

The setup creates a complete CI/CD pipeline that:
1. **DIND Runner**: Builds and packages the Node.js application in an isolated Docker environment
2. **Fake Server**: Simulates a production server using Docker containers
3. **RustCI Pipeline**: Orchestrates the entire deployment process
4. **Network Integration**: Ensures seamless communication between components

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Host Machine                            │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │   RustCI Server │    │        DIND Runner              │ │
│  │   (Port 8080)   │◄──►│  ┌─────────────────────────────┐ │ │
│  │                 │    │  │      Docker Daemon         │ │ │
│  │                 │    │  │   (Builds Node.js App)     │ │ │
│  │                 │    │  └─────────────────────────────┘ │ │
│  └─────────────────┘    └─────────────────────────────────┘ │
│           │                           │                     │
│           │                           ▼                     │
│           │              ┌─────────────────────────────────┐ │
│           └─────────────►│        Fake Server              │ │
│                          │  ┌─────────────────────────────┐ │ │
│                          │  │    Node.js Application     │ │ │
│                          │  │      (Port 3000)           │ │ │
│                          │  └─────────────────────────────┘ │ │
│                          └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Prerequisites

### System Requirements
- **Docker Desktop** or **Docker Engine** (latest version)
- **Rust 1.70+** with Cargo
- **curl**, **jq**, **bc** (command-line tools)
- **Git** (for repository operations)
- **4GB+ RAM** (for running multiple containers)
- **10GB+ disk space** (for Docker images and containers)

### Installation Check
```bash
# Verify Docker
docker --version
docker info

# Verify Rust
cargo --version

# Verify tools
curl --version
jq --version
bc --version
```

## Quick Start

### 1. Complete Setup and Deployment

Run the comprehensive setup script that handles everything:

```bash
# Navigate to RustCI directory
cd /path/to/rustci

# Run complete setup
./scripts/runners/dind-to-fake-server-setup.sh setup
```

This single command will:
- Set up fake EC2-like servers
- Start DIND server-runner simulation
- Configure network connectivity
- Deploy the Node.js pipeline
- Verify the deployment

### 2. Verify Deployment

```bash
# Test the deployment
./scripts/runners/test-dind-fake-deployment.sh

# Check status
./scripts/runners/dind-to-fake-server-setup.sh status
```

### 3. Access the Application

```bash
# SSH into the fake server
ssh root@localhost -p 2201
# Password: rustci123

# Test the application from within the fake server
curl http://localhost:3000/
curl http://localhost:3000/health

# Check container status
docker ps | grep nodejs
```

## Step-by-Step Manual Setup

If you prefer to understand each component, follow these detailed steps:

### Step 1: Start RustCI Server

```bash
# Terminal 1: Start the main RustCI server
cd /path/to/rustci
cargo run
```

Keep this terminal open. The server will run on `http://localhost:8080`.

### Step 2: Set Up Fake Servers

```bash
# Terminal 2: Set up fake EC2-like servers
./scripts/runners/setup-fake-ec2.sh start

# Verify fake servers are running
./scripts/runners/setup-fake-ec2.sh status
```

This creates 3 fake servers:
- `rustci-ec2-1`: SSH port 2201, HTTP port 8081
- `rustci-ec2-2`: SSH port 2202, HTTP port 8082  
- `rustci-ec2-3`: SSH port 2203, HTTP port 8083

### Step 3: Set Up DIND Simulation

```bash
# Terminal 3: Set up DIND server-runner simulation
./scripts/runners/dind-server-runner-simulation.sh start

# Wait for services to be ready (about 2 minutes)
./scripts/runners/dind-server-runner-simulation.sh status
```

This creates:
- `rustci-server-sim`: RustCI server container
- `rustci-runner-sim`: DIND runner container

### Step 4: Configure Network Connectivity

```bash
# Connect DIND runner to fake server network
docker network connect rustci-ec2-network rustci-runner-sim

# Test connectivity
docker exec rustci-runner-sim ping -c 3 rustci-ec2-1
```

### Step 5: Deploy the Pipeline

```bash
# Submit the pipeline
curl -X POST http://localhost:8080/api/pipelines/run \
  -H "Content-Type: application/yaml" \
  --data-binary @dind-to-fake-server-pipeline.yaml
```

### Step 6: Monitor Deployment

```bash
# Check pipeline status
curl http://localhost:8080/api/pipelines

# Check jobs
curl http://localhost:8080/api/jobs

# Monitor logs
./scripts/runners/dind-server-runner-simulation.sh logs
```

## Pipeline Details

The `dind-to-fake-server-pipeline.yaml` includes these stages:

### Stage 1: Source and Build
- **clone-and-build**: Creates Node.js Hello World application
- **build-docker-image**: Builds Docker image in DIND environment
- **test-locally**: Tests the application within DIND

### Stage 2: Package and Transfer
- **package-application**: Packages Docker image and deployment scripts
- **transfer-to-fake-server**: Transfers package via SSH to fake server

### Stage 3: Deploy to Fake Server
- **deploy-application**: Deploys and starts the application on fake server

### Stage 4: Verify Deployment
- **health-check**: Verifies application is running correctly
- **deployment-summary**: Provides access information

## Testing and Validation

### Comprehensive Testing

```bash
# Run all tests
./scripts/runners/test-dind-fake-deployment.sh test

# Quick infrastructure check
./scripts/runners/test-dind-fake-deployment.sh quick

# Application-specific tests
./scripts/runners/test-dind-fake-deployment.sh app

# Performance testing
./scripts/runners/test-dind-fake-deployment.sh perf
```

### Manual Testing

```bash
# Test application endpoints
docker exec rustci-ec2-1 curl http://localhost:3000/
docker exec rustci-ec2-1 curl http://localhost:3000/health

# Check container status
docker exec rustci-ec2-1 docker ps

# View application logs
docker exec rustci-ec2-1 docker logs nodejs-hello-world

# Test from host machine (via SSH)
ssh root@localhost -p 2201 "curl http://localhost:3000/"
```

### API Testing

```bash
# Test RustCI APIs
curl http://localhost:8080/health
curl http://localhost:8080/api/runners
curl http://localhost:8080/api/pipelines
curl http://localhost:8080/api/jobs

# Test runner status
curl http://localhost:8080/api/runners/{runner_id}/status
curl http://localhost:8080/api/runners/{runner_id}/health
```

## Troubleshooting

### Common Issues

#### 1. Containers Won't Start

**Symptoms:**
- Docker containers exit immediately
- "Cannot connect to Docker daemon" errors

**Solutions:**
```bash
# Check Docker daemon
docker info

# Restart Docker Desktop/Engine
# Clean up and retry
./scripts/runners/dind-to-fake-server-setup.sh cleanup
./scripts/runners/dind-to-fake-server-setup.sh setup
```

#### 2. Network Connectivity Issues

**Symptoms:**
- DIND runner can't reach fake server
- SSH connection failures

**Solutions:**
```bash
# Check network configuration
docker network ls
docker network inspect rustci-ec2-network
docker network inspect rustci-simulation

# Reconnect networks
docker network connect rustci-ec2-network rustci-runner-sim

# Test connectivity
docker exec rustci-runner-sim ping rustci-ec2-1
```

#### 3. Pipeline Execution Failures

**Symptoms:**
- Pipeline gets stuck in "pending" state
- Jobs fail with timeout errors

**Solutions:**
```bash
# Check runner status
curl http://localhost:8080/api/runners

# Check server logs
docker logs rustci-server-sim

# Check runner logs
docker logs rustci-runner-sim

# Restart simulation
./scripts/runners/dind-server-runner-simulation.sh restart
```

#### 4. Application Not Responding

**Symptoms:**
- curl requests to app fail
- Container not running on fake server

**Solutions:**
```bash
# Check container status on fake server
docker exec rustci-ec2-1 docker ps -a

# Check application logs
docker exec rustci-ec2-1 docker logs nodejs-hello-world

# Manually restart application
docker exec rustci-ec2-1 docker restart nodejs-hello-world

# Check port binding
docker exec rustci-ec2-1 netstat -tlnp | grep 3000
```

### Debug Commands

```bash
# Infrastructure debugging
docker ps --all
docker network ls
docker volume ls

# Container debugging
docker logs rustci-server-sim
docker logs rustci-runner-sim
docker logs rustci-ec2-1

# Network debugging
docker exec rustci-runner-sim ip addr show
docker exec rustci-ec2-1 ip addr show
docker network inspect rustci-ec2-network

# Application debugging
docker exec rustci-ec2-1 docker exec nodejs-hello-world ps aux
docker exec rustci-ec2-1 docker exec nodejs-hello-world netstat -tlnp
```

## Advanced Usage

### Custom Application Deployment

To deploy your own application instead of the Hello World example:

1. **Modify the pipeline** (`dind-to-fake-server-pipeline.yaml`):
   - Update the source code creation section
   - Modify the Dockerfile as needed
   - Adjust environment variables

2. **Update deployment script**:
   - Change container name and port mappings
   - Add any required environment variables
   - Modify health check endpoints

### Multiple Fake Servers

Deploy to multiple fake servers:

```bash
# Deploy to different servers
export FAKE_SERVER_HOST="rustci-ec2-2"
export FAKE_SERVER_PORT="2202"

# Run deployment
./scripts/runners/dind-to-fake-server-setup.sh deploy
```

### Production-Like Environment

For more realistic testing:

1. **Use different networks** for isolation
2. **Add load balancers** between components
3. **Implement service discovery** mechanisms
4. **Add monitoring and logging** aggregation

### Performance Optimization

```bash
# Increase resources for containers
docker update --memory=2g --cpus=2 rustci-runner-sim

# Use faster storage
docker volume create --driver local \
  --opt type=tmpfs \
  --opt device=tmpfs \
  --opt o=size=1g \
  fast-storage
```

## Cleanup

### Complete Cleanup

```bash
# Clean up everything
./scripts/runners/dind-to-fake-server-setup.sh cleanup

# Verify cleanup
docker ps -a
docker network ls
docker volume ls
```

### Selective Cleanup

```bash
# Clean up only fake servers
./scripts/runners/setup-fake-ec2.sh cleanup

# Clean up only DIND simulation
./scripts/runners/dind-server-runner-simulation.sh cleanup
```

## Integration with CI/CD

### GitHub Actions

```yaml
name: DIND to Fake Server Test

on: [push, pull_request]

jobs:
  test-deployment:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Environment
        run: |
          ./scripts/runners/dind-to-fake-server-setup.sh setup
          
      - name: Run Tests
        run: |
          ./scripts/runners/test-dind-fake-deployment.sh test
          
      - name: Cleanup
        if: always()
        run: |
          ./scripts/runners/dind-to-fake-server-setup.sh cleanup
```

### GitLab CI

```yaml
test-dind-deployment:
  image: docker:latest
  services:
    - docker:dind
  script:
    - ./scripts/runners/dind-to-fake-server-setup.sh setup
    - ./scripts/runners/test-dind-fake-deployment.sh test
  after_script:
    - ./scripts/runners/dind-to-fake-server-setup.sh cleanup
```

## Next Steps

1. **Explore the pipeline**: Modify `dind-to-fake-server-pipeline.yaml` for your needs
2. **Add more stages**: Include testing, security scanning, or notifications
3. **Scale the setup**: Add more fake servers or runners
4. **Integrate monitoring**: Add Prometheus, Grafana, or ELK stack
5. **Implement security**: Add TLS, authentication, and authorization

This setup provides a solid foundation for understanding and testing CI/CD pipelines with RustCI in a completely isolated environment.