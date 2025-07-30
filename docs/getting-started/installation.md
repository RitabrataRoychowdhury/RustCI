# Installation & Setup

This guide will help you install and set up RustCI on your system.

## Prerequisites

### System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Memory**: Minimum 2GB RAM, recommended 4GB+
- **Storage**: 10GB free space for application and workspaces
- **Network**: Internet access for downloading dependencies

### Required Software

#### Rust Development Environment

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Docker (Required for Docker connector)

```bash
# macOS
brew install --cask docker

# Ubuntu/Debian
sudo apt-get update
sudo apt-get install docker.io docker-compose

# Start Docker service
sudo systemctl start docker
sudo systemctl enable docker
```

#### MongoDB (Database)

```bash
# macOS
brew tap mongodb/brew
brew install mongodb-community

# Ubuntu/Debian
wget -qO - https://www.mongodb.org/static/pgp/server-6.0.asc | sudo apt-key add -
echo "deb [ arch=amd64,arm64 ] https://repo.mongodb.org/apt/ubuntu focal/mongodb-org/6.0 multiverse" | sudo tee /etc/apt/sources.list.d/mongodb-org-6.0.list
sudo apt-get update
sudo apt-get install -y mongodb-org

# Start MongoDB
sudo systemctl start mongod
sudo systemctl enable mongod
```

#### Kubernetes (Optional, for Kubernetes connector)

```bash
# Install kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Install Helm (for Kubernetes deployment)
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# For local development, enable Kubernetes in Docker Desktop
# Or install minikube
curl -LO https://storage.googleapis.com/minikube/releases/latest/minikube-linux-amd64
sudo install minikube-linux-amd64 /usr/local/bin/minikube
```

## Installation Methods

### Method 1: Build from Source (Recommended for Development)

```bash
# Clone the repository
git clone https://github.com/your-org/rustci.git
cd rustci

# Build the application
cargo build --release

# The binary will be available at target/release/rustci
```

### Method 2: Docker Installation

```bash
# Pull the Docker image
docker pull rustci:latest

# Or build locally
docker build -t rustci:latest .
```

### Method 3: Kubernetes Installation

```bash
# Using Helm chart
helm install rustci ./helm/rustci

# Or with custom values
helm install rustci ./helm/rustci -f values-production.yaml
```

## Configuration

### Environment Variables

Create a `.env` file in the project root:

```bash
# Copy the example environment file
cp .env.example .env

# Edit the configuration
nano .env
```

Required environment variables:

```env
# Database Configuration
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=rustci

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8000

# Authentication
JWT_SECRET=your-super-secret-jwt-key-here
JWT_EXPIRES_IN=24h

# GitHub OAuth (Optional, for GitHub integration)
GITHUB_OAUTH_CLIENT_ID=your-github-client-id
GITHUB_OAUTH_CLIENT_SECRET=your-github-client-secret
GITHUB_OAUTH_REDIRECT_URL=http://localhost:8000/api/sessions/oauth/github/callback

# Docker Configuration (Optional)
DOCKER_HOST=unix:///var/run/docker.sock

# Kubernetes Configuration (Optional)
KUBECONFIG=/path/to/your/kubeconfig

# Logging
RUST_LOG=info
```

### GitHub OAuth Setup (Optional)

If you want GitHub integration:

1. Go to GitHub Settings → Developer settings → OAuth Apps
2. Click "New OAuth App"
3. Fill in the details:
   - Application name: `RustCI`
   - Homepage URL: `http://localhost:8000`
   - Authorization callback URL: `http://localhost:8000/api/sessions/oauth/github/callback`
4. Copy the Client ID and Client Secret to your `.env` file

## Running RustCI

### Development Mode

```bash
# Run with hot reload (install cargo-watch first)
cargo install cargo-watch
cargo watch -x run

# Or run directly
cargo run
```

### Production Mode

```bash
# Build optimized binary
cargo build --release

# Run the binary
./target/release/rustci

# Or with systemd service
sudo systemctl start rustci
```

### Docker Mode

```bash
# Using docker-compose (recommended)
docker-compose up -d

# Or run directly
docker run -d \
  --name rustci \
  -p 8000:8000 \
  -v $(pwd)/.env:/app/.env \
  -v /var/run/docker.sock:/var/run/docker.sock \
  rustci:latest
```

### Kubernetes Mode

```bash
# Create namespace
kubectl create namespace rustci

# Deploy with Helm
helm install rustci ./helm/rustci -n rustci

# Check deployment status
kubectl get pods -n rustci
```

## Verification

### Health Check

```bash
# Check if RustCI is running
curl http://localhost:8000/api/healthchecker

# Expected response:
# {"status":"success","message":"Build Simple CRUD API with Rust and Axum"}
```

### Database Connection

```bash
# Check MongoDB connection
mongo --eval "db.adminCommand('ismaster')"

# Or using MongoDB Compass GUI
# Connect to: mongodb://localhost:27017
```

### Docker Integration

```bash
# Test Docker connectivity
docker ps

# Test Docker API access
curl --unix-socket /var/run/docker.sock http://localhost/version
```

### Kubernetes Integration

```bash
# Test kubectl connectivity
kubectl cluster-info

# Test Kubernetes API access
kubectl get nodes
```

## Troubleshooting

### Common Issues

#### Port Already in Use

```bash
# Find process using port 8000
lsof -i :8000

# Kill the process
kill -9 <PID>

# Or change the port in .env
SERVER_PORT=8080
```

#### MongoDB Connection Failed

```bash
# Check MongoDB status
sudo systemctl status mongod

# Start MongoDB if not running
sudo systemctl start mongod

# Check MongoDB logs
sudo journalctl -u mongod
```

#### Docker Permission Denied

```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Logout and login again, or run:
newgrp docker

# Test Docker access
docker ps
```

#### Kubernetes Connection Issues

```bash
# Check kubeconfig
kubectl config view

# Check cluster connectivity
kubectl cluster-info

# For Docker Desktop, ensure Kubernetes is enabled
```

### Log Analysis

```bash
# Enable debug logging
export RUST_LOG=debug

# Run RustCI and check logs
cargo run

# Or check systemd logs
sudo journalctl -u rustci -f
```

### Performance Tuning

#### Database Optimization

```javascript
// Connect to MongoDB
mongo

// Create indexes for better performance
use rustci
db.pipelines.createIndex({"name": 1})
db.executions.createIndex({"pipeline_id": 1, "created_at": -1})
db.services.createIndex({"status": 1, "last_heartbeat": -1})
```

#### Resource Limits

```bash
# Increase file descriptor limits
echo "* soft nofile 65536" | sudo tee -a /etc/security/limits.conf
echo "* hard nofile 65536" | sudo tee -a /etc/security/limits.conf

# Increase memory limits for Docker
# Edit /etc/docker/daemon.json
{
  "default-ulimits": {
    "memlock": {
      "Hard": -1,
      "Name": "memlock",
      "Soft": -1
    }
  }
}
```

## Next Steps

After successful installation:

1. [Quick Start Guide](./quick-start.md) - Create your first pipeline
2. [Configuration Guide](./configuration.md) - Advanced configuration options
3. [API Documentation](../api/README.md) - Learn the API
4. [Examples](../examples/pipelines.md) - See working examples

## Support

If you encounter issues during installation:

1. Check the [Troubleshooting](#troubleshooting) section above
2. Review the logs for error messages
3. Ensure all prerequisites are properly installed
4. Verify network connectivity and permissions