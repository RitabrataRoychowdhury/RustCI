# Getting Started with RustCI

Welcome to RustCI! This comprehensive getting started guide will take you from zero to running your first CI/CD pipeline.

## What is RustCI?

RustCI is a high-performance, distributed CI/CD system built in Rust. It provides:

- **Fast Pipeline Execution**: Optimized for speed and efficiency
- **Distributed Architecture**: Scale across multiple nodes
- **Multiple Runner Types**: Docker, Kubernetes, and native execution
- **Flexible Configuration**: YAML-based pipeline definitions
- **Enterprise Features**: Security, observability, and high availability

## Prerequisites

Before you begin, ensure you have:

- **Operating System**: Linux, macOS, or Windows
- **Memory**: At least 2GB RAM (4GB recommended)
- **Storage**: 10GB free disk space
- **Network**: Internet connection for downloading dependencies

### Optional Prerequisites
- **Docker**: For Docker-based pipelines
- **Kubernetes**: For Kubernetes runner support
- **Git**: For source code management integration

## Installation Options

Choose the installation method that best fits your environment:

### Option 1: Quick Install (Recommended for Testing)
```bash
# Download and install RustCI
curl -sSL https://install.rustci.dev | bash

# Verify installation
rustci --version
```

### Option 2: Package Manager Install
```bash
# Ubuntu/Debian
sudo apt update && sudo apt install rustci

# macOS with Homebrew
brew install rustci

# Windows with Chocolatey
choco install rustci
```

### Option 3: From Source
```bash
# Clone the repository
git clone https://github.com/rustci/rustci.git
cd rustci

# Build and install
cargo build --release
sudo cp target/release/rustci /usr/local/bin/
```

For detailed installation instructions, see the [Installation Guide](../installation.md).

## Your First Pipeline

Let's create and run your first RustCI pipeline:

### Step 1: Create a Project Directory
```bash
mkdir my-first-pipeline
cd my-first-pipeline
```

### Step 2: Create a Pipeline Configuration
Create a file named `pipeline.yaml`:

```yaml
# pipeline.yaml
name: "My First Pipeline"
version: "1.0"

stages:
  - name: "hello"
    jobs:
      - name: "say-hello"
        runner: "native"
        steps:
          - name: "greet"
            run: "echo 'Hello from RustCI!'"
          - name: "show-date"
            run: "date"
```

### Step 3: Run the Pipeline
```bash
# Run the pipeline
rustci run pipeline.yaml

# Expected output:
# ✅ Pipeline 'My First Pipeline' started
# ✅ Stage 'hello' started
# ✅ Job 'say-hello' started
# Hello from RustCI!
# Thu Jan  9 10:30:00 UTC 2025
# ✅ Job 'say-hello' completed successfully
# ✅ Stage 'hello' completed successfully
# ✅ Pipeline 'My First Pipeline' completed successfully
```

Congratulations! You've just run your first RustCI pipeline! 🎉

## Understanding the Pipeline

Let's break down what happened:

### Pipeline Structure
```yaml
name: "My First Pipeline"    # Human-readable pipeline name
version: "1.0"              # Pipeline version for tracking

stages:                     # Stages run sequentially
  - name: "hello"           # Stage name
    jobs:                   # Jobs within a stage run in parallel
      - name: "say-hello"   # Job name
        runner: "native"    # Runner type (native, docker, kubernetes)
        steps:              # Steps within a job run sequentially
          - name: "greet"   # Step name
            run: "echo 'Hello from RustCI!'"  # Command to execute
```

### Key Concepts
- **Pipeline**: The complete CI/CD workflow
- **Stages**: Sequential phases of the pipeline
- **Jobs**: Parallel units of work within a stage
- **Steps**: Individual commands or actions within a job
- **Runners**: Execution environments (native, Docker, Kubernetes)

## Next Steps

Now that you have RustCI running, here are some next steps:

### 1. Explore More Examples
- [Simple Pipeline Examples](../pipeline-examples/simple-pipeline.yaml)
- [Build and Test Pipeline](../pipeline-examples/build-test-pipeline.yaml)
- [Docker-based Pipeline](../pipeline-examples/docker-deployment-pipeline.yaml)

### 2. Learn About Configuration
- [Configuration Guide](../configuration.md) - Complete configuration reference
- [Runner Configuration](../pipeline-examples/native-runner-config.yaml) - Configure different runners

### 3. Set Up for Your Project
- Add RustCI to your existing project
- Configure source code integration
- Set up automated triggers

### 4. Advanced Features
- [Multi-stage Pipelines](../pipeline-examples/advanced-pipeline.yaml)
- [Kubernetes Integration](../pipeline-examples/kubernetes-test-pipeline.yaml)
- [Security Scanning](../pipeline-examples/security-scan-pipeline.yaml)

## Common Next Actions

### For Application Development
```yaml
# Build and test pipeline example
name: "Build and Test"
version: "1.0"

stages:
  - name: "build"
    jobs:
      - name: "compile"
        runner: "docker"
        image: "node:18"
        steps:
          - name: "install-deps"
            run: "npm install"
          - name: "build"
            run: "npm run build"
  
  - name: "test"
    jobs:
      - name: "unit-tests"
        runner: "docker"
        image: "node:18"
        steps:
          - name: "run-tests"
            run: "npm test"
```

### For Infrastructure Deployment
```yaml
# Deployment pipeline example
name: "Deploy to Production"
version: "1.0"

stages:
  - name: "deploy"
    jobs:
      - name: "kubernetes-deploy"
        runner: "kubernetes"
        steps:
          - name: "apply-manifests"
            run: "kubectl apply -f k8s/"
          - name: "wait-for-rollout"
            run: "kubectl rollout status deployment/my-app"
```

## Getting Help

### Documentation
- **[User Guide](../README.md)** - Complete user documentation
- **[Configuration Reference](../configuration.md)** - All configuration options
- **[Pipeline Examples](../pipeline-examples/)** - Working pipeline examples
- **[Troubleshooting](../troubleshooting.md)** - Common issues and solutions

### Advanced Topics
- **[API Documentation](../../api/README.md)** - Programmatic integration
- **[Architecture Overview](../../architecture/README.md)** - Understanding RustCI internals
- **[Deployment Guides](../../deployment/README.md)** - Production deployment options

### Community
- **GitHub Discussions**: Ask questions and share experiences
- **GitHub Issues**: Report bugs and request features
- **Discord**: Real-time community chat

### Professional Support
- **Enterprise Support**: Available for production deployments
- **Training**: Custom training sessions available
- **Consulting**: Architecture and implementation consulting

---

**Ready to dive deeper?** Check out the [Pipeline Examples](../pipeline-examples/) or read the [Configuration Guide](../configuration.md) to learn about advanced features.