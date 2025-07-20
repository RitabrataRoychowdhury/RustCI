# RustCI

A high-performance CI/CD platform built in Rust that serves as a modern alternative to Jenkins.

## 🚀 Features

- **Fast Pipeline Execution** - Built in Rust for maximum performance
- **YAML Configuration** - Simple, readable pipeline definitions
- **Multiple Deployment Types** - Local, Docker, and hybrid deployments
- **GitHub Integration** - Seamless repository cloning and webhook support
- **Real-time Monitoring** - Track pipeline executions in real-time
- **RESTful API** - Complete API for integration and automation
- **Multipart File Upload** - Upload YAML configurations as files
- **Service Registry** - Track and manage deployed services

## 📋 Quick Start

### Prerequisites

- Rust 1.70+ installed
- Docker (for container deployments)
- MongoDB (for data persistence)
- Git (for repository operations)

### Installation

1. **Clone the repository**:
```bash
git clone https://github.com/RitabrataRoychowdhury/RustCI.git
cd RustCI
```

2. **Set up environment variables**:
```bash
cp .env.example .env
# Edit .env with your configuration
```

3. **Build and run**:
```bash
cargo build --release
cargo run
```

The server will start on `http://localhost:8000`

### Docker Deployment

```bash
# Build the Docker image
docker build -t rustci:latest .

# Run with Docker Compose
docker-compose up -d
```

## 🔧 Configuration

### Environment Variables

```bash
# Server Configuration
PORT=8000
RUST_ENV=development
RUST_LOG=info

# Database
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=rustci

# Authentication
JWT_SECRET=your-secret-key
JWT_EXPIRED_IN=1d

# GitHub OAuth (optional)
GITHUB_OAUTH_CLIENT_ID=your-client-id
GITHUB_OAUTH_CLIENT_SECRET=your-client-secret
GITHUB_OAUTH_REDIRECT_URL=http://localhost:8000/api/sessions/oauth/github/callback

# Client
CLIENT_ORIGIN=http://localhost:3000
```

## 📖 API Documentation

### Pipeline Management

#### Create Pipeline (JSON)
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"My Pipeline\"\ndescription: \"Test pipeline\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Build\"\n    steps:\n      - name: \"build-step\"\n        step_type: shell\n        config:\n          command: \"echo Building...\"\nenvironment: {}\ntimeout: 3600\nretry_count: 0"
  }'
```

#### Create Pipeline (Multipart File Upload)
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@pipeline.yaml"
```

#### List Pipelines
```bash
curl -X GET http://localhost:8000/api/ci/pipelines
```

#### Trigger Pipeline
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{pipeline_id}/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "NODE_ENV": "production"
    }
  }'
```

### Execution Management

#### Get Execution Status
```bash
curl -X GET http://localhost:8000/api/ci/executions/{execution_id}
```

#### List Executions
```bash
curl -X GET http://localhost:8000/api/ci/executions
```

#### Cancel Execution
```bash
curl -X DELETE http://localhost:8000/api/ci/executions/{execution_id}/cancel
```

### Webhook Support

#### GitHub Webhook
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{pipeline_id}/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "refs/heads/main",
    "after": "commit-hash",
    "repository": {
      "full_name": "user/repo",
      "clone_url": "https://github.com/user/repo.git"
    }
  }'
```

## 📝 Pipeline Configuration

### Basic Pipeline Structure

```yaml
name: "My Application Pipeline"
description: "Build and deploy my application"

triggers:
  - trigger_type: manual
    config: {}
  - trigger_type: webhook
    config:
      webhook_url: "/webhook/my-app"

stages:
  - name: "Source"
    steps:
      - name: "clone-repository"
        step_type: github
        config:
          repository_url: "https://github.com/user/repo.git"
          branch: "main"

  - name: "Build"
    steps:
      - name: "build-application"
        step_type: shell
        config:
          command: "npm install && npm run build"

  - name: "Deploy"
    steps:
      - name: "deploy-docker"
        step_type: docker
        config:
          image: "my-app"
          dockerfile: "Dockerfile"

environment:
  NODE_ENV: "production"
  PORT: "3000"

timeout: 3600
retry_count: 1
```

### Deployment Types

#### Local Directory Deployment
```yaml
- name: "deploy-local"
  step_type: custom
  config:
    deployment_type: "local_directory"
    target_directory: "/opt/myapp"
```

#### Docker Container Deployment
```yaml
- name: "deploy-docker"
  step_type: docker
  config:
    image: "myapp:latest"
    dockerfile: "Dockerfile"
    ports:
      - "8080:8080"
    environment:
      - "NODE_ENV=production"
```

#### Local Service Deployment
```yaml
- name: "deploy-service"
  step_type: custom
  config:
    deployment_type: "local_service"
    service_name: "myapp"
    port: 8080
```

## 🏗️ Architecture

### System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web API       │    │   CI Engine     │    │   Deployment    │
│   (Axum)        │───▶│   (Pipeline     │───▶│   Manager       │
│                 │    │    Execution)   │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Database      │    │   File System   │    │   Docker        │
│   (MongoDB)     │    │   (Workspace)   │    │   (Containers)  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Request Flow

1. **API Request** → Route Handler
2. **Authentication** → Middleware Validation
3. **Pipeline Creation** → CI Engine
4. **Execution** → Pipeline Manager
5. **Deployment** → Deployment Manager
6. **Monitoring** → Real-time Status Updates

## 🧪 Testing

### Health Check
```bash
curl -X GET http://localhost:8000/api/healthchecker
```

### Run Tests
```bash
cargo test
```

### Integration Tests
```bash
cargo test --test integration
```

## 🔒 Security

- JWT-based authentication
- Input validation and sanitization
- File upload size limits
- Docker container isolation
- Environment variable protection

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 📚 Documentation

Complete documentation is available in the [`docs/`](./docs/) directory:

### Getting Started
- **[Installation & Setup](./docs/getting-started/installation.md)** - Complete installation guide for all platforms
- **[Quick Start Guide](./docs/getting-started/quick-start.md)** - Get up and running in minutes

### API Reference
- **[Complete API Documentation](./docs/api/README.md)** - Full API reference with cURL examples

### Deployment Guides
- **[Kubernetes Deployment](./docs/deployment/kubernetes.md)** - Deploy to Kubernetes clusters
- **[General Deployment Guide](./docs/deployment/guide.md)** - All deployment options and configurations

### Architecture & Development
- **[System Architecture](./docs/architecture/system-design.md)** - Comprehensive system design documentation
- **[Connector System](./docs/architecture/connectors.md)** - Extensible connector architecture guide

### Examples & Integration
- **[Pipeline Examples](./docs/examples/README.md)** - Working pipeline examples for different use cases
- **[Integration Validation](./docs/integration/validation.md)** - Testing and validation procedures

## 🆘 Support

- **Documentation**: Complete guides in [`docs/`](./docs/)
- **Examples**: Working examples in [`docs/examples/`](./docs/examples/)
- **Issues**: Report bugs and request features via GitHub Issues

## 🗺️ Roadmap

- [ ] Web UI Dashboard
- [ ] Agent-based Deployments
- [ ] Kubernetes Integration
- [ ] Multi-node Clustering
- [ ] Advanced Monitoring
- [ ] Plugin System

---

**RustCI** - Built with ❤️ in Rust for speed, reliability, and developer happiness.