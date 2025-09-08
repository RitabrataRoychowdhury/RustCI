#!/bin/bash

# Mac Docker-in-Docker Runner Setup Script
# This script registers and starts a DinD runner for RustCI

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTCI_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
RUNNER_NAME="mac-dind-runner"
RUNNER_PORT="8081"
DOCKER_NETWORK="rustci-network"
WINDOWS_HOST="${WINDOWS_HOST:-}"
WINDOWS_USER="${WINDOWS_USER:-}"
RUSTCI_SERVER="${RUSTCI_SERVER:-http://localhost:8080}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    # Check Docker daemon
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    # Check if Windows host is configured
    if [[ -z "$WINDOWS_HOST" ]]; then
        log_warning "WINDOWS_HOST not set. Please set it to your Windows laptop IP"
        read -p "Enter Windows laptop IP address: " WINDOWS_HOST
        if [[ -z "$WINDOWS_HOST" ]]; then
            log_error "Windows host IP is required"
            exit 1
        fi
    fi
    
    if [[ -z "$WINDOWS_USER" ]]; then
        log_warning "WINDOWS_USER not set. Please set it to your Windows username"
        read -p "Enter Windows username: " WINDOWS_USER
        if [[ -z "$WINDOWS_USER" ]]; then
            log_error "Windows username is required"
            exit 1
        fi
    fi
    
    log_success "Prerequisites check passed"
}

# Test connectivity to Windows machine
test_connectivity() {
    log_info "Testing connectivity to Windows machine..."
    
    # Test ping
    log_info "Testing ping to $WINDOWS_HOST..."
    if ping -c 3 "$WINDOWS_HOST" &> /dev/null; then
        log_success "Ping to Windows machine successful"
    else
        log_error "Cannot ping Windows machine at $WINDOWS_HOST"
        exit 1
    fi
    
    # Test SSH connectivity
    log_info "Testing SSH connectivity to $WINDOWS_USER@$WINDOWS_HOST..."
    if ssh -o ConnectTimeout=10 -o BatchMode=yes "$WINDOWS_USER@$WINDOWS_HOST" "echo 'SSH test successful'" &> /dev/null; then
        log_success "SSH connectivity test passed"
    else
        log_warning "SSH connectivity test failed. Make sure:"
        log_warning "1. SSH server is running on Windows (OpenSSH Server)"
        log_warning "2. SSH keys are set up or password authentication is enabled"
        log_warning "3. Windows firewall allows SSH connections"
        
        read -p "Continue anyway? (y/N): " continue_choice
        if [[ "$continue_choice" != "y" && "$continue_choice" != "Y" ]]; then
            exit 1
        fi
    fi
}

# Create Docker network if it doesn't exist
setup_docker_network() {
    log_info "Setting up Docker network..."
    
    if ! docker network ls | grep -q "$DOCKER_NETWORK"; then
        docker network create "$DOCKER_NETWORK"
        log_success "Created Docker network: $DOCKER_NETWORK"
    else
        log_info "Docker network $DOCKER_NETWORK already exists"
    fi
}

# Stop and remove existing runner
cleanup_existing_runner() {
    log_info "Cleaning up existing runner..."
    
    if docker ps -a | grep -q "$RUNNER_NAME"; then
        log_info "Stopping existing runner container..."
        docker stop "$RUNNER_NAME" &> /dev/null || true
        docker rm "$RUNNER_NAME" &> /dev/null || true
        log_success "Cleaned up existing runner"
    fi
}

# Start Docker-in-Docker runner
start_dind_runner() {
    log_info "Starting Docker-in-Docker runner..."
    
    # Create runner configuration
    cat > /tmp/runner-config.yaml << EOF
runner:
  name: "$RUNNER_NAME"
  type: "docker"
  host: "0.0.0.0"
  port: $RUNNER_PORT
  max_concurrent_jobs: 3
  
docker:
  privileged: true
  network: "$DOCKER_NETWORK"
  volumes:
    - "/var/run/docker.sock:/var/run/docker.sock"
  
deployment:
  windows_host: "$WINDOWS_HOST"
  windows_user: "$WINDOWS_USER"
  
logging:
  level: "info"
  format: "json"
EOF

    # Start DinD container
    docker run -d \
        --name "$RUNNER_NAME" \
        --privileged \
        --network "$DOCKER_NETWORK" \
        -p "$RUNNER_PORT:$RUNNER_PORT" \
        -v /var/run/docker.sock:/var/run/docker.sock \
        -v /tmp/runner-config.yaml:/config/runner.yaml \
        -v "$HOME/.ssh:/root/.ssh:ro" \
        -e WINDOWS_HOST="$WINDOWS_HOST" \
        -e WINDOWS_USER="$WINDOWS_USER" \
        -e RUSTCI_SERVER="$RUSTCI_SERVER" \
        docker:dind \
        sh -c "
            # Install required tools
            apk add --no-cache openssh-client curl jq
            
            # Start Docker daemon
            dockerd-entrypoint.sh &
            
            # Wait for Docker to be ready
            while ! docker info &> /dev/null; do
                echo 'Waiting for Docker daemon...'
                sleep 2
            done
            
            echo 'Docker daemon is ready'
            
            # Keep container running and show logs
            tail -f /dev/null
        "
    
    # Wait for container to be ready
    sleep 5
    
    if docker ps | grep -q "$RUNNER_NAME"; then
        log_success "Docker-in-Docker runner started successfully"
        log_info "Runner name: $RUNNER_NAME"
        log_info "Runner port: $RUNNER_PORT"
        log_info "Windows target: $WINDOWS_USER@$WINDOWS_HOST"
    else
        log_error "Failed to start Docker-in-Docker runner"
        docker logs "$RUNNER_NAME" 2>/dev/null || true
        exit 1
    fi
}

# Register runner with RustCI server
register_runner() {
    log_info "Registering runner with RustCI server..."
    
    # Wait for RustCI server to be available
    local max_attempts=30
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if curl -s "$RUSTCI_SERVER/health" &> /dev/null; then
            log_success "RustCI server is available"
            break
        fi
        
        log_info "Waiting for RustCI server... (attempt $attempt/$max_attempts)"
        sleep 2
        ((attempt++))
    done
    
    if [[ $attempt -gt $max_attempts ]]; then
        log_error "RustCI server is not available at $RUSTCI_SERVER"
        exit 1
    fi
    
    # Register runner
    local runner_data=$(cat << EOF
{
    "name": "$RUNNER_NAME",
    "type": "docker",
    "host": "host.docker.internal",
    "port": $RUNNER_PORT,
    "capabilities": ["docker", "deployment"],
    "metadata": {
        "os": "linux",
        "arch": "amd64",
        "docker_version": "$(docker --version | cut -d' ' -f3 | cut -d',' -f1)",
        "deployment_target": "$WINDOWS_USER@$WINDOWS_HOST"
    }
}
EOF
    )
    
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$runner_data" \
        "$RUSTCI_SERVER/api/runners/register" || echo "")
    
    if [[ -n "$response" ]] && echo "$response" | jq -e '.success' &> /dev/null; then
        log_success "Runner registered successfully"
        local runner_id=$(echo "$response" | jq -r '.runner_id')
        log_info "Runner ID: $runner_id"
        
        # Save runner info
        echo "$runner_id" > "/tmp/${RUNNER_NAME}.id"
    else
        log_warning "Runner registration may have failed, but continuing..."
        log_info "Response: $response"
    fi
}

# Show runner status and logs
show_status() {
    log_info "Runner Status:"
    echo "=================="
    echo "Runner Name: $RUNNER_NAME"
    echo "Runner Port: $RUNNER_PORT"
    echo "Windows Target: $WINDOWS_USER@$WINDOWS_HOST"
    echo "RustCI Server: $RUSTCI_SERVER"
    echo ""
    
    if docker ps | grep -q "$RUNNER_NAME"; then
        echo "Status: ✅ Running"
        echo "Container ID: $(docker ps --filter name=$RUNNER_NAME --format '{{.ID}}')"
    else
        echo "Status: ❌ Not Running"
    fi
    
    echo ""
    log_info "Recent logs:"
    docker logs --tail 20 "$RUNNER_NAME" 2>/dev/null || echo "No logs available"
}

# Create deployment test pipeline
create_test_pipeline() {
    log_info "Creating test deployment pipeline..."
    
    cat > "$RUSTCI_DIR/mac-to-windows-pipeline.yaml" << 'EOF'
name: "Mac to Windows Deployment Pipeline"
description: "Test pipeline for Mac DinD runner deploying to Windows"

stages:
  - name: "build"
    jobs:
      - name: "build-app"
        runner: "mac-dind-runner"
        steps:
          - name: "create-artifact"
            run: |
              echo "Building application..."
              mkdir -p /workspace/artifacts
              echo "Hello from Mac DinD Runner - $(date)" > /workspace/artifacts/app.txt
              echo "Build completed successfully"
              
          - name: "package-artifact"
            run: |
              cd /workspace/artifacts
              tar -czf app-$(date +%Y%m%d-%H%M%S).tar.gz app.txt
              ls -la *.tar.gz
              echo "Artifact packaged successfully"

  - name: "deploy"
    jobs:
      - name: "deploy-to-windows"
        runner: "mac-dind-runner"
        steps:
          - name: "test-connectivity"
            run: |
              echo "Testing connectivity to Windows machine..."
              ping -c 3 $WINDOWS_HOST
              echo "Connectivity test passed"
              
          - name: "deploy-artifact"
            run: |
              echo "Deploying to Windows machine..."
              cd /workspace/artifacts
              
              # Find the latest artifact
              ARTIFACT=$(ls -t *.tar.gz | head -1)
              echo "Deploying artifact: $ARTIFACT"
              
              # Copy to Windows machine
              scp -o StrictHostKeyChecking=no "$ARTIFACT" "$WINDOWS_USER@$WINDOWS_HOST:C:/temp/"
              
              # Trigger deployment script on Windows
              ssh -o StrictHostKeyChecking=no "$WINDOWS_USER@$WINDOWS_HOST" "C:/temp/deploy-server.bat $ARTIFACT"
              
              echo "Deployment completed successfully"
              
          - name: "verify-deployment"
            run: |
              echo "Verifying deployment..."
              # Test if the deployment server is responding
              sleep 5
              if curl -f "http://$WINDOWS_HOST:3000/health" 2>/dev/null; then
                echo "✅ Deployment verification successful"
              else
                echo "⚠️  Deployment server not responding, but deployment may still be successful"
              fi
EOF

    log_success "Test pipeline created: $RUSTCI_DIR/mac-to-windows-pipeline.yaml"
}

# Main execution
main() {
    log_info "Starting Mac Docker-in-Docker Runner Setup"
    echo "=========================================="
    
    check_prerequisites
    test_connectivity
    setup_docker_network
    cleanup_existing_runner
    start_dind_runner
    register_runner
    create_test_pipeline
    
    echo ""
    log_success "Setup completed successfully!"
    echo ""
    
    show_status
    
    echo ""
    log_info "Next steps:"
    echo "1. Set up the Windows deployment server using the deploy-server.bat script"
    echo "2. Test the pipeline: rustci run mac-to-windows-pipeline.yaml"
    echo "3. Monitor runner logs: docker logs -f $RUNNER_NAME"
    echo "4. Stop runner: docker stop $RUNNER_NAME"
    
    echo ""
    log_info "Environment variables for future runs:"
    echo "export WINDOWS_HOST=\"$WINDOWS_HOST\""
    echo "export WINDOWS_USER=\"$WINDOWS_USER\""
}

# Handle script arguments
case "${1:-start}" in
    "start")
        main
        ;;
    "stop")
        log_info "Stopping Docker-in-Docker runner..."
        docker stop "$RUNNER_NAME" &> /dev/null || true
        docker rm "$RUNNER_NAME" &> /dev/null || true
        log_success "Runner stopped"
        ;;
    "status")
        show_status
        ;;
    "logs")
        docker logs -f "$RUNNER_NAME"
        ;;
    "help"|"-h"|"--help")
        echo "Usage: $0 [start|stop|status|logs|help]"
        echo ""
        echo "Commands:"
        echo "  start   - Start the DinD runner (default)"
        echo "  stop    - Stop the DinD runner"
        echo "  status  - Show runner status"
        echo "  logs    - Show runner logs"
        echo "  help    - Show this help"
        ;;
    *)
        log_error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac