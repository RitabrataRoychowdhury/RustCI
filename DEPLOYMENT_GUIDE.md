# RustAutoDevOps Deployment Guide

A comprehensive guide to deploy applications using the RustAutoDevOps platform with a Node.js Hello World example.

## Prerequisites

- Docker and Docker Compose installed
- Git repository access
- Basic understanding of YAML configuration
- Network access for GitHub OAuth (if using authentication)

## Quick Start Overview

RustAutoDevOps is a production-grade CI/CD platform built in Rust that provides:
- Automated pipeline execution
- Blue-green deployments
- Auto-scaling and load balancing
- Performance monitoring
- Security features with multi-factor authentication
- Resource management and cleanup

## Step 1: Environment Setup

### 1.1 Clone and Prepare the Platform

```bash
# Clone the RustAutoDevOps platform
git clone <your-rustci-repo>
cd RustAutoDevOps

# Ensure Docker is running
docker --version
docker-compose --version
```

### 1.2 Configure Environment Variables

Create or update the `.env` file in the root directory:

```bash
# Copy the example environment file
cp .env.example .env
```

Edit `.env` with your configuration:
```env
# Database Configuration
MONGODB_URI=mongodb://localhost:27017/rustci
DATABASE_NAME=rustci

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8000

# GitHub OAuth (Optional - for authentication)
GITHUB_CLIENT_ID=your_github_client_id
GITHUB_CLIENT_SECRET=your_github_client_secret
GITHUB_REDIRECT_URL=http://localhost:8000/api/sessions/oauth/github/callback

# Security
JWT_SECRET=your_super_secure_jwt_secret_at_least_32_characters_long

# Features
ENABLE_METRICS=true
ENABLE_HOT_RELOAD=true
ENABLE_SECURITY_FEATURES=true

# Performance
MAX_CONCURRENT_JOBS=10
WORKER_THREADS=4
```

## Step 2: Start the Platform Infrastructure

### 2.1 Start Dependencies with Docker Compose

```bash
# Start MongoDB and other dependencies
docker-compose up -d mongodb

# Wait for MongoDB to be ready
sleep 10

# Verify MongoDB is running
docker-compose ps
```

### 2.2 Build and Start the Platform

```bash
# Build the platform (this may take a few minutes)
cargo build --release

# Start the platform in development mode
ENVIRONMENT=development cargo run --bin RustAutoDevOps
```

The platform will start on `http://localhost:8000`

## Step 3: Prepare Your Application Repository

### 3.1 Fork the Example Repository

Fork or clone the Node.js hello world repository:
```bash
git clone https://github.com/danawoodman/docker-node-hello-world.git
cd docker-node-hello-world
```

### 3.2 Verify the Application Structure

Ensure your repository has:
- `Dockerfile` (✓ already present)
- `package.json` (✓ already present)
- Application source code (✓ `server.js` present)

### 3.3 Add Pipeline Configuration

Create a `.rustci.yaml` file in your repository root:

```yaml
# .rustci.yaml
name: "nodejs-hello-world-pipeline"
description: "CI/CD pipeline for Node.js Hello World application"

# Repository configuration
repository:
  url: "https://github.com/danawoodman/docker-node-hello-world.git"
  branch: "master"

# Build configuration
build:
  dockerfile: "Dockerfile"
  context: "."
  
# Pipeline stages
stages:
  - name: "install"
    steps:
      - name: "Install dependencies"
        run: "npm ci"
        
  - name: "test"
    steps:
      - name: "Run tests"
        run: "npm test"
      - name: "Run linting"
        run: "npm run lint || echo 'Linting skipped - no lint script'"
        
  - name: "build"
    steps:
      - name: "Build Docker image"
        run: "docker build -t nodejs-hello-world:${BUILD_NUMBER} ."
        
  - name: "deploy"
    steps:
      - name: "Deploy application"
        run: "docker run -d -p 3000:3000 --name hello-world-${BUILD_NUMBER} nodejs-hello-world:${BUILD_NUMBER}"
    conditions:
      - branch: "master"

# Triggers
triggers:
  push: true
  pull_request: true
  schedule: "0 2 * * *"  # Daily at 2 AM

# Environment variables
environment:
  NODE_ENV: "production"
  PORT: "3000"

# Resource limits
resources:
  memory: "512Mi"
  cpu: "0.5"

# Deployment strategy
deployment:
  strategy: "blue-green"
  health_check:
    path: "/"
    port: 3000
    timeout: 30
    
# Notifications (optional)
notifications:
  slack:
    webhook_url: "${SLACK_WEBHOOK_URL}"
    channel: "#deployments"
```

## Step 4: Create and Configure Pipeline via API

### 4.1 Create a New Pipeline

Use the REST API to create your pipeline:

```bash
# Create pipeline
curl -X POST http://localhost:8000/api/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "name": "nodejs-hello-world-pipeline",
    "description": "CI/CD pipeline for Node.js Hello World application",
    "repository_url": "https://github.com/danawoodman/docker-node-hello-world.git",
    "branch": "master",
    "config": {
      "stages": [
        {
          "name": "install",
          "steps": [
            {"run": "npm ci"}
          ]
        },
        {
          "name": "test", 
          "steps": [
            {"run": "npm test || echo \"No tests found\""},
            {"run": "npm run lint || echo \"No lint script found\""}
          ]
        },
        {
          "name": "build",
          "steps": [
            {"run": "docker build -t nodejs-hello-world:latest ."}
          ]
        },
        {
          "name": "deploy",
          "steps": [
            {"run": "docker run -d -p 3000:3000 --name hello-world nodejs-hello-world:latest"}
          ],
          "when": "branch == \"master\""
        }
      ]
    },
    "triggers": {
      "push": true,
      "pull_request": true,
      "schedule": "0 2 * * *"
    },
    "environment": {
      "NODE_ENV": "production",
      "PORT": "3000"
    },
    "tags": ["nodejs", "web", "hello-world"]
  }'
```

### 4.2 Upload Pipeline Configuration File

If you prefer to upload the YAML configuration:

```bash
# Upload the .rustci.yaml file
curl -X POST http://localhost:8000/api/pipelines/upload \
  -H "Content-Type: multipart/form-data" \
  -F "config=@.rustci.yaml" \
  -F "repository_url=https://github.com/danawoodman/docker-node-hello-world.git"
```

## Step 5: Configure Deployment Environment

### 5.1 Set Up Docker-in-Docker (DinD) Server

Create a DinD environment for isolated builds:

```bash
# Create DinD container
docker run -d \
  --name rustci-dind \
  --privileged \
  -p 2376:2376 \
  -e DOCKER_TLS_CERTDIR=/certs \
  -v rustci-dind-certs:/certs/client \
  docker:dind

# Wait for DinD to be ready
sleep 15

# Verify DinD is running
docker exec rustci-dind docker version
```

### 5.2 Configure Build Environment

Create environment-specific configuration:

```bash
# Create build environment configuration
curl -X POST http://localhost:8000/api/environments \
  -H "Content-Type: application/json" \
  -d '{
    "name": "nodejs-build-env",
    "description": "Node.js build environment with Docker support",
    "docker_host": "tcp://localhost:2376",
    "environment_variables": {
      "NODE_VERSION": "18",
      "NPM_CONFIG_CACHE": "/tmp/.npm",
      "DOCKER_BUILDKIT": "1"
    },
    "resource_limits": {
      "memory": "1Gi",
      "cpu": "1.0",
      "disk": "5Gi"
    }
  }'
```

## Step 6: Execute Pipeline

### 6.1 Trigger Pipeline Execution

```bash
# Get pipeline ID (from previous create response or list pipelines)
PIPELINE_ID=$(curl -s http://localhost:8000/api/pipelines | jq -r '.[0].id')

# Trigger pipeline execution
curl -X POST http://localhost:8000/api/pipelines/${PIPELINE_ID}/execute \
  -H "Content-Type: application/json" \
  -d '{
    "branch": "master",
    "commit_sha": "latest",
    "triggered_by": "manual",
    "environment": "nodejs-build-env"
  }'
```

### 6.2 Monitor Pipeline Execution

```bash
# Get execution status
EXECUTION_ID=$(curl -s http://localhost:8000/api/pipelines/${PIPELINE_ID}/executions | jq -r '.[0].id')

# Monitor execution progress
curl http://localhost:8000/api/executions/${EXECUTION_ID}/status

# Get execution logs
curl http://localhost:8000/api/executions/${EXECUTION_ID}/logs
```

## Step 7: Configure Advanced Features

### 7.1 Set Up Auto-Scaling

```bash
# Configure auto-scaling for your application
curl -X POST http://localhost:8000/api/scaling/policies \
  -H "Content-Type: application/json" \
  -d '{
    "name": "nodejs-hello-world-scaling",
    "target_service": "hello-world",
    "min_instances": 1,
    "max_instances": 5,
    "metrics": {
      "cpu_threshold": 70,
      "memory_threshold": 80,
      "response_time_threshold": 1000
    },
    "scaling_rules": {
      "scale_up_cooldown": 300,
      "scale_down_cooldown": 600,
      "scale_up_step": 1,
      "scale_down_step": 1
    }
  }'
```

### 7.2 Configure Load Balancing

```bash
# Set up load balancer
curl -X POST http://localhost:8000/api/load-balancers \
  -H "Content-Type: application/json" \
  -d '{
    "name": "nodejs-hello-world-lb",
    "algorithm": "round_robin",
    "health_check": {
      "path": "/",
      "port": 3000,
      "interval": 30,
      "timeout": 10,
      "healthy_threshold": 2,
      "unhealthy_threshold": 3
    },
    "endpoints": [
      {
        "host": "localhost",
        "port": 3000,
        "weight": 100
      }
    ]
  }'
```

### 7.3 Set Up Monitoring

```bash
# Configure monitoring and alerts
curl -X POST http://localhost:8000/api/monitoring/alerts \
  -H "Content-Type: application/json" \
  -d '{
    "name": "nodejs-hello-world-alerts",
    "service": "hello-world",
    "rules": [
      {
        "metric": "response_time",
        "threshold": 2000,
        "operator": "greater_than",
        "severity": "warning"
      },
      {
        "metric": "error_rate",
        "threshold": 5,
        "operator": "greater_than", 
        "severity": "critical"
      }
    ],
    "notifications": {
      "email": ["admin@example.com"],
      "slack": "#alerts"
    }
  }'
```

## Step 8: Verify Deployment

### 8.1 Check Application Status

```bash
# Check if the application is running
curl http://localhost:3000

# Expected response: "Hello World"

# Check application health via load balancer
curl http://localhost:8000/api/health/hello-world
```

### 8.2 View Metrics and Logs

```bash
# Get application metrics
curl http://localhost:8000/api/metrics/hello-world

# Get recent logs
curl http://localhost:8000/api/logs/hello-world?limit=100

# Get performance metrics
curl http://localhost:8000/api/performance/hello-world
```

## Step 9: Blue-Green Deployment (Advanced)

### 9.1 Configure Blue-Green Deployment

```bash
# Set up blue-green deployment
curl -X POST http://localhost:8000/api/deployments/blue-green \
  -H "Content-Type: application/json" \
  -d '{
    "service_name": "hello-world",
    "blue_config": {
      "image": "nodejs-hello-world:v1.0",
      "port": 3000,
      "instances": 2
    },
    "green_config": {
      "image": "nodejs-hello-world:v1.1", 
      "port": 3001,
      "instances": 2
    },
    "switch_criteria": {
      "health_check_passes": 3,
      "performance_threshold": 1000,
      "error_rate_threshold": 1
    }
  }'
```

### 9.2 Execute Blue-Green Switch

```bash
# Switch traffic from blue to green
curl -X POST http://localhost:8000/api/deployments/blue-green/switch \
  -H "Content-Type: application/json" \
  -d '{
    "deployment_id": "your-deployment-id",
    "target_environment": "green",
    "rollback_on_failure": true
  }'
```

## Step 10: Cleanup and Maintenance

### 10.1 Resource Cleanup

```bash
# Clean up old deployments
curl -X POST http://localhost:8000/api/cleanup/deployments \
  -H "Content-Type: application/json" \
  -d '{
    "older_than_days": 7,
    "keep_latest": 3
  }'

# Clean up Docker images
curl -X POST http://localhost:8000/api/cleanup/images \
  -H "Content-Type: application/json" \
  -d '{
    "unused_for_days": 3,
    "exclude_tags": ["latest", "stable"]
  }'
```

### 10.2 Backup Configuration

```bash
# Backup pipeline configurations
curl http://localhost:8000/api/pipelines/export > pipelines-backup.json

# Backup environment configurations  
curl http://localhost:8000/api/environments/export > environments-backup.json
```

## Troubleshooting

### Common Issues

1. **Pipeline fails at build stage**
   - Check Docker daemon is running
   - Verify Dockerfile syntax
   - Check resource limits

2. **Application not accessible**
   - Verify port mappings
   - Check firewall settings
   - Ensure load balancer is configured

3. **High resource usage**
   - Review auto-scaling policies
   - Check for memory leaks
   - Optimize Docker images

### Useful Commands

```bash
# Check platform status
curl http://localhost:8000/api/health

# List all pipelines
curl http://localhost:8000/api/pipelines

# Get system metrics
curl http://localhost:8000/api/system/metrics

# View active deployments
curl http://localhost:8000/api/deployments/active
```

## API Reference Summary

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/pipelines` | POST | Create pipeline |
| `/api/pipelines/upload` | POST | Upload YAML config |
| `/api/pipelines/{id}/execute` | POST | Execute pipeline |
| `/api/environments` | POST | Create environment |
| `/api/scaling/policies` | POST | Configure auto-scaling |
| `/api/load-balancers` | POST | Set up load balancer |
| `/api/deployments/blue-green` | POST | Blue-green deployment |
| `/api/monitoring/alerts` | POST | Configure monitoring |
| `/api/health/{service}` | GET | Check service health |
| `/api/metrics/{service}` | GET | Get service metrics |

## Configuration Files Reference

- `.env` - Environment variables
- `.rustci.yaml` - Pipeline configuration
- `config/environments/` - Environment-specific configs
- `docker-compose.yaml` - Infrastructure setup

This guide provides a complete workflow for deploying applications using RustAutoDevOps. The platform offers enterprise-grade features including auto-scaling, load balancing, blue-green deployments, and comprehensive monitoring.