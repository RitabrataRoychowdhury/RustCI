#!/bin/bash

# Simple DIND to Fake Server Deployment Script
# Works with existing RustCI server (cargo run)

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTCI_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
PIPELINE_FILE="$RUSTCI_DIR/dind-to-fake-server-pipeline.yaml"
RUSTCI_SERVER="http://localhost:8080"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${PURPLE}[STEP]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    # Check if RustCI server is running
    if ! curl -s "$RUSTCI_SERVER/health" &> /dev/null; then
        log_error "RustCI server is not running at $RUSTCI_SERVER"
        log_info "Please start it with: cargo run"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Setup fake servers
setup_fake_servers() {
    log_step "Setting up fake servers..."
    
    if [[ -f "$SCRIPT_DIR/setup-fake-ec2.sh" ]]; then
        log_info "Starting fake EC2 environment..."
        "$SCRIPT_DIR/setup-fake-ec2.sh" start
        
        # Wait for servers to be ready
        log_info "Waiting for fake servers to be ready..."
        sleep 30
        
        # Test connectivity
        if docker exec rustci-ec2-1 echo "Fake server is ready" &> /dev/null; then
            log_success "Fake servers are ready"
        else
            log_error "Fake servers are not responding"
            exit 1
        fi
    else
        log_error "Fake EC2 setup script not found"
        exit 1
    fi
}

# Register a simple DIND runner with the existing server
register_dind_runner() {
    log_step "Registering DIND runner with existing server..."
    
    # Create a simple DIND runner container
    local runner_name="simple-dind-runner"
    
    # Stop existing runner if present
    docker stop "$runner_name" 2>/dev/null || true
    docker rm "$runner_name" 2>/dev/null || true
    
    # Start DIND runner container
    docker run -d \
        --name "$runner_name" \
        --privileged \
        --network rustci-ec2-network \
        -p 8081:8081 \
        -v /tmp:/workspace \
        docker:dind \
        sh -c "
            # Start Docker daemon
            dockerd-entrypoint.sh &
            
            # Wait for Docker daemon
            while ! docker info &> /dev/null; do
                echo 'Waiting for Docker daemon...'
                sleep 2
            done
            
            # Install tools
            apk add --no-cache curl jq bash openssh-client sshpass
            
            echo 'DIND runner ready'
            
            # Keep container running
            tail -f /dev/null
        "
    
    # Wait for runner to be ready
    sleep 20
    
    if docker exec "$runner_name" docker info &> /dev/null; then
        log_success "DIND runner container is ready"
    else
        log_error "DIND runner failed to start"
        exit 1
    fi
    
    # Register runner with RustCI server via API
    log_info "Registering runner with RustCI server..."
    
    local runner_data='{
        "name": "simple-dind-runner",
        "type": "docker",
        "host": "simple-dind-runner",
        "port": 8081,
        "capabilities": ["docker", "isolation", "deployment"],
        "metadata": {
            "os": "linux",
            "arch": "amd64",
            "container": true
        }
    }'
    
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$runner_data" \
        "$RUSTCI_SERVER/api/runners/register" || echo "")
    
    if [[ -n "$response" ]]; then
        log_success "Runner registered with server"
        echo "Response: $response"
    else
        log_warning "Runner registration may have failed, continuing anyway"
    fi
}

# Deploy the pipeline
deploy_pipeline() {
    log_step "Deploying the pipeline..."
    
    if [[ ! -f "$PIPELINE_FILE" ]]; then
        log_error "Pipeline file not found: $PIPELINE_FILE"
        exit 1
    fi
    
    log_info "Submitting pipeline to RustCI server..."
    
    # Submit pipeline via API
    local response=$(curl -s -X POST \
        -H "Content-Type: application/yaml" \
        --data-binary @"$PIPELINE_FILE" \
        "$RUSTCI_SERVER/api/pipelines/run" || echo "")
    
    if [[ -n "$response" ]]; then
        log_success "Pipeline submitted successfully"
        echo "Response: $response"
        
        # Try to extract job/pipeline ID for monitoring
        local job_id=$(echo "$response" | jq -r '.job_id // .id // empty' 2>/dev/null || echo "")
        if [[ -n "$job_id" && "$job_id" != "null" ]]; then
            log_info "Job/Pipeline ID: $job_id"
            echo "$job_id" > /tmp/simple-dind-job-id
        fi
    else
        log_error "Failed to submit pipeline"
        exit 1
    fi
}

# Manual execution of pipeline steps
manual_pipeline_execution() {
    log_step "Executing pipeline steps manually..."
    
    local runner_name="simple-dind-runner"
    
    # Step 1: Build the Node.js application
    log_info "Step 1: Building Node.js application in DIND..."
    
    docker exec "$runner_name" bash -c '
        echo "=== Building Node.js Hello World Application ==="
        
        # Create workspace
        rm -rf /workspace/nodejs-app
        mkdir -p /workspace/nodejs-app
        cd /workspace/nodejs-app
        
        # Create package.json
        cat > package.json << "EOF"
{
  "name": "hello-world-app",
  "version": "1.0.0",
  "description": "Simple Hello World Node.js app",
  "main": "server.js",
  "scripts": {
    "start": "node server.js"
  },
  "dependencies": {
    "express": "^4.18.0"
  }
}
EOF
        
        # Create server.js
        cat > server.js << "EOF"
const express = require("express");
const app = express();
const port = process.env.PORT || 3000;

app.get("/", (req, res) => {
  res.json({
    message: "Hello World from Simple DIND Runner!",
    timestamp: new Date().toISOString(),
    environment: process.env.NODE_ENV || "development",
    hostname: require("os").hostname()
  });
});

app.get("/health", (req, res) => {
  res.json({ status: "healthy", uptime: process.uptime() });
});

app.listen(port, "0.0.0.0", () => {
  console.log(`Server running on port ${port}`);
});
EOF
        
        # Create Dockerfile
        cat > Dockerfile << "EOF"
FROM node:18-alpine
WORKDIR /app
COPY package.json .
RUN npm install --production
COPY server.js .
EXPOSE 3000
CMD ["npm", "start"]
EOF
        
        echo "✅ Application source prepared"
        ls -la
    '
    
    # Step 2: Build Docker image
    log_info "Step 2: Building Docker image..."
    
    docker exec "$runner_name" bash -c '
        cd /workspace/nodejs-app
        docker build -t nodejs-hello-world:latest .
        docker images | grep nodejs-hello-world
        echo "✅ Docker image built"
    '
    
    # Step 3: Test locally
    log_info "Step 3: Testing application locally..."
    
    docker exec "$runner_name" bash -c '
        cd /workspace/nodejs-app
        
        # Run test container
        docker run -d --name test-app -p 8080:3000 nodejs-hello-world:latest
        sleep 10
        
        # Test the application
        if curl -s http://localhost:8080/ | grep "Hello World"; then
            echo "✅ Local test passed"
        else
            echo "❌ Local test failed"
        fi
        
        # Cleanup test container
        docker stop test-app
        docker rm test-app
    '
    
    # Step 4: Package for deployment
    log_info "Step 4: Packaging for deployment..."
    
    docker exec "$runner_name" bash -c '
        cd /workspace/nodejs-app
        
        # Save Docker image
        docker save nodejs-hello-world:latest -o nodejs-hello-world.tar
        
        # Create deployment script
        cat > deploy.sh << "EOF"
#!/bin/bash
echo "Deploying Node.js application..."

# Stop existing container
docker stop nodejs-hello-world || true
docker rm nodejs-hello-world || true

# Load and run new container
docker load -i nodejs-hello-world.tar
docker run -d \
  --name nodejs-hello-world \
  --restart unless-stopped \
  -p 3000:3000 \
  -e NODE_ENV=production \
  nodejs-hello-world:latest

echo "Application deployed!"
sleep 5

# Test deployment
if curl -f http://localhost:3000/health; then
  echo "✅ Deployment successful"
else
  echo "❌ Deployment failed"
  exit 1
fi
EOF
        
        chmod +x deploy.sh
        
        # Create deployment package
        tar -czf deployment-package.tar.gz nodejs-hello-world.tar deploy.sh
        
        echo "✅ Deployment package created"
        ls -la *.tar.gz
    '
    
    # Step 5: Transfer to fake server
    log_info "Step 5: Transferring to fake server..."
    
    docker exec "$runner_name" bash -c '
        cd /workspace/nodejs-app
        
        # Copy to fake server
        docker cp deployment-package.tar.gz rustci-ec2-1:/tmp/
        
        echo "✅ Package transferred to fake server"
    '
    
    # Step 6: Deploy on fake server
    log_info "Step 6: Deploying on fake server..."
    
    docker exec rustci-ec2-1 bash -c '
        cd /tmp
        
        # Extract package
        tar -xzf deployment-package.tar.gz
        
        # Run deployment
        chmod +x deploy.sh
        ./deploy.sh
        
        echo "✅ Deployment completed on fake server"
    '
}

# Verify deployment
verify_deployment() {
    log_step "Verifying deployment..."
    
    # Test application on fake server
    local app_response=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/ 2>/dev/null || echo "FAILED")
    
    if echo "$app_response" | grep -q "Hello World"; then
        log_success "✅ Application is running and responding correctly!"
        echo "Application response: $app_response"
        
        # Test health endpoint
        local health_response=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/health 2>/dev/null || echo "FAILED")
        if echo "$health_response" | grep -q "healthy"; then
            log_success "✅ Health endpoint is working correctly!"
            echo "Health response: $health_response"
        fi
        
        return 0
    else
        log_error "❌ Application is not responding correctly"
        echo "Response received: $app_response"
        
        # Debug information
        log_info "Debugging deployment..."
        docker exec rustci-ec2-1 docker ps -a | grep nodejs || echo "No nodejs containers found"
        docker exec rustci-ec2-1 docker logs nodejs-hello-world 2>/dev/null || echo "No logs available"
        
        return 1
    fi
}

# Show status
show_status() {
    log_info "Deployment Status"
    echo "=================="
    
    echo ""
    echo "🖥️  Infrastructure:"
    
    # Check fake server
    if docker ps | grep -q rustci-ec2-1; then
        echo "  ✅ Fake Server: Running (SSH: ssh root@localhost -p 2201)"
    else
        echo "  ❌ Fake Server: Not running"
    fi
    
    # Check DIND runner
    if docker ps | grep -q simple-dind-runner; then
        echo "  ✅ DIND Runner: Running"
    else
        echo "  ❌ DIND Runner: Not running"
    fi
    
    # Check RustCI server
    if curl -s "$RUSTCI_SERVER/health" &> /dev/null; then
        echo "  ✅ RustCI Server: Running ($RUSTCI_SERVER)"
    else
        echo "  ❌ RustCI Server: Not running"
    fi
    
    echo ""
    echo "🚀 Application:"
    
    # Check application
    local app_status=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/ 2>/dev/null || echo "FAILED")
    if echo "$app_status" | grep -q "Hello World"; then
        echo "  ✅ Node.js App: Running on fake server"
    else
        echo "  ❌ Node.js App: Not responding"
    fi
    
    echo ""
    echo "🔧 Access:"
    echo "  • RustCI Server: $RUSTCI_SERVER"
    echo "  • Fake Server SSH: ssh root@localhost -p 2201 (password: rustci123)"
    echo "  • Test App: docker exec rustci-ec2-1 curl http://localhost:3000/"
}

# Cleanup
cleanup() {
    log_step "Cleaning up..."
    
    # Stop containers
    docker stop simple-dind-runner 2>/dev/null || true
    docker rm simple-dind-runner 2>/dev/null || true
    
    # Clean up fake servers
    if [[ -f "$SCRIPT_DIR/setup-fake-ec2.sh" ]]; then
        "$SCRIPT_DIR/setup-fake-ec2.sh" cleanup
    fi
    
    # Clean up temp files
    rm -f /tmp/simple-dind-job-id
    
    log_success "Cleanup completed"
}

# Show usage
show_usage() {
    cat << EOF
Simple DIND to Fake Server Deployment

This script provides a simplified version that works with an existing RustCI server
(started with 'cargo run') and demonstrates DIND to fake server deployment.

Usage: $0 [COMMAND]

COMMANDS:
    deploy      Deploy application (default)
    manual      Execute pipeline steps manually
    verify      Verify deployment
    status      Show status
    cleanup     Clean up resources
    help        Show this help

PREREQUISITES:
    • RustCI server running: cargo run
    • Docker daemon running
    • curl, jq available

EXAMPLES:
    $0              # Full deployment
    $0 manual       # Manual step-by-step execution
    $0 verify       # Check if deployment works
    $0 status       # Show current status

EOF
}

# Main execution
main() {
    log_info "Simple DIND to Fake Server Deployment"
    echo "====================================="
    
    check_prerequisites
    setup_fake_servers
    register_dind_runner
    
    # Try API-based deployment first, fall back to manual
    if deploy_pipeline; then
        log_info "Pipeline submitted via API, waiting for execution..."
        sleep 30
    else
        log_warning "API deployment failed, executing manually..."
        manual_pipeline_execution
    fi
    
    if verify_deployment; then
        log_success "🎉 Deployment successful!"
        show_status
    else
        log_error "❌ Deployment verification failed"
        show_status
        exit 1
    fi
}

# Handle command line arguments
case "${1:-deploy}" in
    "deploy"|"")
        main
        ;;
    "manual")
        check_prerequisites
        setup_fake_servers
        register_dind_runner
        manual_pipeline_execution
        verify_deployment
        ;;
    "verify")
        verify_deployment
        ;;
    "status")
        show_status
        ;;
    "cleanup")
        cleanup
        ;;
    "help"|"-h"|"--help")
        show_usage
        ;;
    *)
        log_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac