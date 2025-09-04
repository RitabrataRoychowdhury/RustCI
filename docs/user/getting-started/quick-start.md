# Quick Start Guide

Get up and running with RustCI in under 5 minutes! This guide assumes you have RustCI installed. If not, see the [Installation Guide](installation.md).

## Prerequisites Check

Verify RustCI is installed and working:

```bash
# Check version
rustci --version

# Run system check
rustci doctor
```

If these commands work, you're ready to go! 🚀

## 5-Minute Quick Start

### Step 1: Create Your First Pipeline (1 minute)

Create a new directory and pipeline file:

```bash
# Create project directory
mkdir hello-rustci && cd hello-rustci

# Create a simple pipeline
cat > pipeline.yaml << 'EOF'
name: "Hello RustCI"
version: "1.0"

stages:
  - name: "greet"
    jobs:
      - name: "hello-world"
        runner: "native"
        steps:
          - name: "say-hello"
            run: "echo 'Hello from RustCI! 🚀'"
          - name: "show-info"
            run: |
              echo "Pipeline: Hello RustCI"
              echo "Runner: Native"
              echo "Date: $(date)"
              echo "User: $(whoami)"
EOF
```

### Step 2: Run Your Pipeline (30 seconds)

```bash
# Execute the pipeline
rustci run pipeline.yaml
```

Expected output:
```
✅ Pipeline 'Hello RustCI' started
✅ Stage 'greet' started  
✅ Job 'hello-world' started
Hello from RustCI! 🚀
Pipeline: Hello RustCI
Runner: Native
Date: Thu Jan  9 10:30:00 UTC 2025
User: your-username
✅ Job 'hello-world' completed successfully
✅ Stage 'greet' completed successfully  
✅ Pipeline 'Hello RustCI' completed successfully
```

Congratulations! You just ran your first RustCI pipeline! 🎉

### Step 3: Try a Multi-Stage Pipeline (2 minutes)

Let's create a more realistic pipeline with multiple stages:

```bash
cat > build-pipeline.yaml << 'EOF'
name: "Build and Test"
version: "1.0"

stages:
  - name: "prepare"
    jobs:
      - name: "setup"
        runner: "native"
        steps:
          - name: "create-workspace"
            run: "mkdir -p workspace && echo 'Workspace created'"
          - name: "show-environment"
            run: |
              echo "=== Environment Info ==="
              echo "OS: $(uname -s)"
              echo "Architecture: $(uname -m)"
              echo "Shell: $SHELL"

  - name: "build"
    jobs:
      - name: "compile"
        runner: "native"
        steps:
          - name: "simulate-build"
            run: |
              echo "🔨 Building application..."
              sleep 2
              echo "✅ Build completed successfully"
              echo "build-artifact-v1.0.0" > workspace/artifact.txt

  - name: "test"
    jobs:
      - name: "unit-tests"
        runner: "native"
        steps:
          - name: "run-tests"
            run: |
              echo "🧪 Running unit tests..."
              sleep 1
              echo "✅ All tests passed (5/5)"
      
      - name: "integration-tests"
        runner: "native"
        steps:
          - name: "run-integration-tests"
            run: |
              echo "🔗 Running integration tests..."
              sleep 1
              echo "✅ Integration tests passed (3/3)"

  - name: "deploy"
    jobs:
      - name: "deploy-staging"
        runner: "native"
        steps:
          - name: "deploy"
            run: |
              echo "🚀 Deploying to staging..."
              if [ -f workspace/artifact.txt ]; then
                echo "Deploying artifact: $(cat workspace/artifact.txt)"
                echo "✅ Deployment successful"
              else
                echo "❌ No artifact found"
                exit 1
              fi
EOF
```

Run the multi-stage pipeline:

```bash
rustci run build-pipeline.yaml
```

This pipeline demonstrates:
- **Sequential stages**: prepare → build → test → deploy
- **Parallel jobs**: unit-tests and integration-tests run simultaneously
- **Workspace sharing**: artifacts created in one stage used in another
- **Conditional logic**: deployment checks for build artifacts

### Step 4: Explore Docker Runner (1.5 minutes)

If you have Docker installed, try a Docker-based pipeline:

```bash
cat > docker-pipeline.yaml << 'EOF'
name: "Docker Example"
version: "1.0"

stages:
  - name: "docker-demo"
    jobs:
      - name: "node-app"
        runner: "docker"
        image: "node:18-alpine"
        steps:
          - name: "node-info"
            run: |
              echo "Node.js version: $(node --version)"
              echo "NPM version: $(npm --version)"
          - name: "create-app"
            run: |
              echo 'console.log("Hello from Node.js in Docker!");' > app.js
              node app.js
      
      - name: "python-app"
        runner: "docker"
        image: "python:3.11-alpine"
        steps:
          - name: "python-info"
            run: |
              echo "Python version: $(python --version)"
              echo "Pip version: $(pip --version)"
          - name: "run-python"
            run: |
              python -c "print('Hello from Python in Docker!')"
              python -c "import sys; print(f'Python path: {sys.executable}')"
EOF
```

Run the Docker pipeline:

```bash
# Check if Docker is available
docker --version

# Run the pipeline
rustci run docker-pipeline.yaml
```

## Understanding What You Built

### Pipeline Structure
```yaml
name: "Pipeline Name"        # Human-readable name
version: "1.0"              # Version for tracking changes

stages:                     # Stages run sequentially
  - name: "stage-name"      # Stage identifier
    jobs:                   # Jobs run in parallel within a stage
      - name: "job-name"    # Job identifier
        runner: "native"    # Execution environment
        steps:              # Steps run sequentially within a job
          - name: "step-name"  # Step identifier
            run: "command"     # Command to execute
```

### Key Concepts You Used

1. **Stages**: Sequential phases (prepare → build → test → deploy)
2. **Jobs**: Parallel execution units within stages
3. **Steps**: Individual commands within jobs
4. **Runners**: Execution environments (native, docker, kubernetes)
5. **Workspace**: Shared storage between stages

## Next Steps - Choose Your Path

### 🏗️ For Application Development
```bash
# Try a real build pipeline
curl -o app-pipeline.yaml https://raw.githubusercontent.com/rustci/examples/main/nodejs-app-pipeline.yaml
rustci run app-pipeline.yaml
```

### 🐳 For Docker Enthusiasts
```bash
# Try advanced Docker features
curl -o docker-advanced.yaml https://raw.githubusercontent.com/rustci/examples/main/docker-advanced-pipeline.yaml
rustci run docker-advanced.yaml
```

### ☸️ For Kubernetes Users
```bash
# Try Kubernetes runner (requires kubectl configured)
curl -o k8s-pipeline.yaml https://raw.githubusercontent.com/rustci/examples/main/kubernetes-pipeline.yaml
rustci run k8s-pipeline.yaml
```

### 🔧 For Configuration Enthusiasts
Explore the [Configuration Guide](../configuration.md) to:
- Set up persistent configuration
- Configure different runners
- Set up environment variables
- Configure security settings

## Common Next Actions

### 1. Add to Your Project
```bash
# In your existing project directory
cd /path/to/your/project

# Create a pipeline for your project
rustci init --template=<language>  # e.g., nodejs, python, rust, go
```

### 2. Set Up Continuous Integration
```bash
# Create a CI pipeline that runs on code changes
cat > .rustci/ci-pipeline.yaml << 'EOF'
name: "Continuous Integration"
version: "1.0"

triggers:
  - on: "push"
    branches: ["main", "develop"]
  - on: "pull_request"

stages:
  - name: "ci"
    jobs:
      - name: "test"
        runner: "docker"
        image: "node:18"
        steps:
          - name: "install"
            run: "npm ci"
          - name: "test"
            run: "npm test"
          - name: "lint"
            run: "npm run lint"
EOF
```

### 3. Explore Advanced Features
- **[Pipeline Examples](../pipeline-examples/)** - Real-world examples
- **[Configuration Guide](../configuration.md)** - Advanced configuration
- **[API Documentation](../../api/README.md)** - Programmatic integration

## Troubleshooting Quick Start

### Pipeline Fails to Run
```bash
# Check pipeline syntax
rustci validate pipeline.yaml

# Run with verbose output
rustci run --verbose pipeline.yaml

# Check logs
rustci logs --pipeline "Hello RustCI"
```

### Docker Issues
```bash
# Check Docker is running
docker ps

# Test Docker access
docker run hello-world

# Check RustCI Docker integration
rustci test-runner docker
```

### Permission Issues
```bash
# Check file permissions
ls -la pipeline.yaml

# Make sure RustCI can execute
which rustci
rustci --version
```

## Getting Help

### Quick References
- **Commands**: `rustci --help`
- **Pipeline syntax**: `rustci help pipeline`
- **Runner options**: `rustci help runners`

### Community Support
- **GitHub Discussions**: Ask questions and share experiences
- **GitHub Issues**: Report bugs and request features
- **Discord**: Real-time community chat

### Documentation
- **[User Guide](../README.md)** - Complete user documentation
- **[Troubleshooting](../troubleshooting.md)** - Common issues and solutions
- **[Examples](../pipeline-examples/)** - Working pipeline examples

---

**🎉 Congratulations!** You've successfully completed the RustCI quick start. You're now ready to build more complex pipelines and integrate RustCI into your development workflow.

**What's next?** Check out the [Pipeline Examples](../pipeline-examples/) to see real-world use cases, or dive into the [Configuration Guide](../configuration.md) to customize RustCI for your needs.