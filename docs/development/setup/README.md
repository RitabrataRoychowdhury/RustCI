# Development Setup Guide

This guide will help you set up a local development environment for RustCI.

## Prerequisites

### System Requirements
- **Operating System**: Linux, macOS, or Windows (with WSL2)
- **Memory**: Minimum 8GB RAM, recommended 16GB+
- **Storage**: At least 10GB free space
- **Network**: Internet connection for downloading dependencies

### Required Software
- **Rust**: Latest stable version (1.70+)
- **Docker**: For container-based runners and testing
- **Git**: For version control
- **MongoDB**: For data storage (can use Docker)

### Optional Software
- **Kubernetes**: For Kubernetes runner development (minikube, k3s, or kind)
- **IDE**: VS Code with Rust extension, or your preferred editor

## Installation Steps

### 1. Install Rust

```bash
# Install Rust using rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Reload your shell or run:
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### 2. Install Docker

#### Linux (Ubuntu/Debian)
```bash
# Update package index
sudo apt-get update

# Install Docker
sudo apt-get install docker.io docker-compose

# Add user to docker group
sudo usermod -aG docker $USER

# Restart shell or logout/login
```

#### macOS
```bash
# Install Docker Desktop
brew install --cask docker

# Or download from https://docker.com/products/docker-desktop
```

#### Windows
Download and install Docker Desktop from https://docker.com/products/docker-desktop

### 3. Clone Repository

```bash
# Clone the RustCI repository
git clone https://github.com/rustci/rustci.git
cd rustci

# Verify you're on the main branch
git branch
```

### 4. Build Project

```bash
# Build the project
cargo build

# Run tests to verify everything works
cargo test

# Build in release mode (optional)
cargo build --release
```

### 5. Set Up Database

#### Option A: MongoDB with Docker (Recommended)
```bash
# Start MongoDB container
docker run -d --name rustci-mongo \
  -p 27017:27017 \
  -e MONGO_INITDB_ROOT_USERNAME=rustci \
  -e MONGO_INITDB_ROOT_PASSWORD=password \
  mongo:latest

# Verify MongoDB is running
docker ps | grep rustci-mongo
```

#### Option B: Local MongoDB Installation
```bash
# Ubuntu/Debian
sudo apt-get install mongodb

# macOS
brew install mongodb-community

# Start MongoDB service
sudo systemctl start mongodb  # Linux
brew services start mongodb-community  # macOS
```

### 6. Configure Environment

```bash
# Copy example configuration
cp config/examples/config.example.yaml config/rustci.yaml

# Edit configuration for development
nano config/rustci.yaml
```

Update the configuration for development:

```yaml
global:
  environment: "development"
  log_level: "debug"

server:
  port: 8080

storage:
  type: "mongodb"
  connection_string: "mongodb://rustci:password@localhost:27017/rustci"

communication_protocol:
  security:
    encryption: false
    authentication: false

runners:
  docker:
    enabled: true
  native:
    enabled: true
  kubernetes:
    enabled: false
```

### 7. Start Development Server

```bash
# Start RustCI server
cargo run

# Or use the binary if built in release mode
./target/release/rustci

# Verify server is running
curl http://localhost:8080/health
```

## Development Workflow

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test module
cargo test integration

# Run tests with output
cargo test -- --nocapture

# Run tests in parallel
cargo test --jobs 4
```

### Code Formatting and Linting

```bash
# Format code
cargo fmt

# Check formatting without changing files
cargo fmt --check

# Run clippy for linting
cargo clippy

# Run clippy with all features
cargo clippy --all-features
```

### Building Documentation

```bash
# Build and open documentation
cargo doc --open

# Build documentation for all dependencies
cargo doc --document-private-items
```

### Development Tools

#### VS Code Setup
Install the following extensions:
- `rust-lang.rust-analyzer` - Rust language support
- `vadimcn.vscode-lldb` - Debugging support
- `serayuzgur.crates` - Cargo.toml management

#### Debugging
```bash
# Build with debug symbols
cargo build

# Run with debugger (if using VS Code)
# Use the Debug configuration in launch.json
```

## Testing Your Setup

### 1. Create a Test Pipeline

Create `test-pipeline.yaml`:

```yaml
name: "test-pipeline"
version: "1.0"

stages:
  - name: "hello"
    runner: "native"
    steps:
      - name: "echo"
        command: "echo 'Hello from RustCI!'"
```

### 2. Run the Pipeline

```bash
# Submit pipeline
curl -X POST http://localhost:8080/api/v1/pipelines \
  -H "Content-Type: application/yaml" \
  --data-binary @test-pipeline.yaml

# Check pipeline status
curl http://localhost:8080/api/v1/pipelines
```

### 3. Verify Logs

```bash
# Check server logs
tail -f logs/rustci.log

# Or if running in terminal, check console output
```

## Common Issues and Solutions

### Issue: Cargo build fails with linking errors

**Solution:**
```bash
# Install build essentials
sudo apt-get install build-essential  # Linux
xcode-select --install  # macOS
```

### Issue: MongoDB connection fails

**Solution:**
```bash
# Check MongoDB is running
docker ps | grep mongo

# Check connection string in config
# Ensure username/password match container setup
```

### Issue: Docker permission denied

**Solution:**
```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Restart shell or logout/login
```

### Issue: Port 8080 already in use

**Solution:**
```bash
# Find process using port 8080
lsof -i :8080

# Kill the process or change port in config
```

## Next Steps

After setting up your development environment:

1. Read the [Contributing Guidelines](../../CONTRIBUTING.md)
2. Explore the [Architecture Documentation](../../architecture/README.md)
3. Check out [Development Best Practices](../best-practices.md)
4. Join our [Community Discussions](https://github.com/rustci/rustci/discussions)

## Getting Help

If you encounter issues during setup:

- Check the [troubleshooting guide](../../user/troubleshooting.md)
- Search existing [issues](https://github.com/rustci/rustci/issues)
- Ask questions in [discussions](https://github.com/rustci/rustci/discussions)
- Join our [Discord community](https://discord.gg/rustci)