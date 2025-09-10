#!/bin/bash

# RustCI Runner Integration Test Script
# This script tests the integration between Mac DinD runner and RustCI server

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTCI_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
RUNNER_NAME="mac-dind-runner"
RUSTCI_SERVER="${RUSTCI_SERVER:-http://localhost:8080}"
WINDOWS_HOST="${WINDOWS_HOST:-}"
WINDOWS_USER="${WINDOWS_USER:-}"
TEST_PIPELINE_NAME="runner-integration-test"

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

log_test() {
    echo -e "${PURPLE}[TEST]${NC} $1"
}

# Check if RustCI server is running
check_rustci_server() {
    log_info "Checking RustCI server availability..."
    
    local max_attempts=30
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if curl -s -f --connect-timeout 5 "$RUSTCI_SERVER/health" &> /dev/null; then
            log_success "RustCI server is available at $RUSTCI_SERVER"
            return 0
        fi
        
        if [[ $attempt -eq 1 ]]; then
            log_info "RustCI server not available, attempting to start it..."
            start_rustci_server
        fi
        
        log_info "Waiting for RustCI server... (attempt $attempt/$max_attempts)"
        sleep 2
        ((attempt++))
    done
    
    log_error "RustCI server is not available at $RUSTCI_SERVER"
    log_info "Please start RustCI server manually:"
    log_info "  cd $RUSTCI_DIR && cargo run --bin rustci-server"
    return 1
}

# Start RustCI server
start_rustci_server() {
    log_info "Starting RustCI server..."
    
    # Check if server is already running
    if pgrep -f "rustci-server" > /dev/null; then
        log_info "RustCI server process already running"
        return 0
    fi
    
    # Start server in background
    cd "$RUSTCI_DIR"
    nohup cargo run --bin rustci-server > /tmp/rustci-server.log 2>&1 &
    local server_pid=$!
    
    log_info "Started RustCI server with PID: $server_pid"
    echo "$server_pid" > /tmp/rustci-server.pid
    
    # Wait for server to be ready
    sleep 10
}

# Check if DinD runner is running
check_dind_runner() {
    log_info "Checking DinD runner status..."
    
    if docker ps | grep -q "$RUNNER_NAME"; then
        log_success "DinD runner is running"
        return 0
    else
        log_warning "DinD runner is not running"
        log_info "Starting DinD runner..."
        
        # Use the existing script to start the runner
        if [[ -f "$SCRIPT_DIR/mac-dind-runner.sh" ]]; then
            "$SCRIPT_DIR/mac-dind-runner.sh" start
            return $?
        else
            log_error "mac-dind-runner.sh script not found"
            return 1
        fi
    fi
}

# Test runner registration with RustCI server
test_runner_registration() {
    log_test "Testing runner registration with RustCI server..."
    
    # Get runner info
    local runner_data=$(cat << EOF
{
    "name": "$RUNNER_NAME",
    "type": "docker",
    "host": "localhost",
    "port": 8081,
    "capabilities": ["docker", "deployment"],
    "metadata": {
        "os": "linux",
        "arch": "amd64",
        "test_mode": true
    }
}
EOF
    )
    
    # Register runner
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$runner_data" \
        "$RUSTCI_SERVER/api/runners/register" 2>/dev/null || echo "")
    
    if [[ -n "$response" ]]; then
        log_success "Runner registration response received"
        log_info "Response: $response"
        
        # Check if registration was successful
        if echo "$response" | grep -q "success\|id\|registered"; then
            log_success "Runner registration appears successful"
            return 0
        else
            log_warning "Runner registration may have failed"
            return 1
        fi
    else
        log_error "No response from runner registration endpoint"
        return 1
    fi
}

# Create test pipeline
create_test_pipeline() {
    log_info "Creating test pipeline..."
    
    cat > "/tmp/${TEST_PIPELINE_NAME}.yaml" << 'EOF'
name: "Runner Integration Test Pipeline"
description: "Test pipeline for Mac DinD runner integration"

stages:
  - name: "setup"
    jobs:
      - name: "environment-check"
        runner: "mac-dind-runner"
        steps:
          - name: "check-docker"
            run: |
              echo "=== Docker Environment Check ==="
              docker --version
              docker info --format '{{.ServerVersion}}'
              echo "Docker daemon is running in DinD container"
              
          - name: "check-system"
            run: |
              echo "=== System Information ==="
              uname -a
              whoami
              pwd
              ls -la /
              echo "System check completed"

  - name: "build"
    jobs:
      - name: "test-build"
        runner: "mac-dind-runner"
        steps:
          - name: "create-workspace"
            run: |
              echo "=== Creating Test Workspace ==="
              mkdir -p /workspace/test-app
              cd /workspace/test-app
              echo "Test application created at $(date)" > app.txt
              echo "#!/bin/sh" > run.sh
              echo "echo 'Running test application...'" >> run.sh
              echo "cat app.txt" >> run.sh
              chmod +x run.sh
              ls -la
              
          - name: "test-docker-build"
            run: |
              echo "=== Testing Docker Build Capability ==="
              cd /workspace/test-app
              cat > Dockerfile << 'DOCKERFILE'
FROM alpine:latest
COPY . /app
WORKDIR /app
RUN chmod +x run.sh
CMD ["./run.sh"]
DOCKERFILE
              
              docker build -t test-app:latest .
              echo "Docker build completed successfully"
              
          - name: "test-docker-run"
            run: |
              echo "=== Testing Docker Run Capability ==="
              docker run --rm test-app:latest
              echo "Docker run test completed successfully"

  - name: "package"
    jobs:
      - name: "create-artifact"
        runner: "mac-dind-runner"
        steps:
          - name: "package-application"
            run: |
              echo "=== Packaging Application ==="
              cd /workspace/test-app
              tar -czf ../test-app-$(date +%Y%m%d-%H%M%S).tar.gz .
              ls -la ../test-app-*.tar.gz
              echo "Application packaged successfully"

  - name: "deploy"
    jobs:
      - name: "test-deployment"
        runner: "mac-dind-runner"
        steps:
          - name: "deployment-simulation"
            run: |
              echo "=== Deployment Simulation ==="
              cd /workspace
              ARTIFACT=$(ls -t test-app-*.tar.gz | head -1)
              echo "Deploying artifact: $ARTIFACT"
              
              # Simulate deployment steps
              mkdir -p /tmp/deployment
              cp "$ARTIFACT" /tmp/deployment/
              cd /tmp/deployment
              tar -xzf "$ARTIFACT"
              
              echo "Deployment simulation completed"
              echo "Files deployed:"
              ls -la
              
              # Test the deployed application
              if [[ -f "run.sh" ]]; then
                echo "Testing deployed application:"
                ./run.sh
              fi
EOF

    log_success "Test pipeline created: /tmp/${TEST_PIPELINE_NAME}.yaml"
}

# Test pipeline execution
test_pipeline_execution() {
    log_test "Testing pipeline execution..."
    
    # Create the test pipeline
    create_test_pipeline
    
    # Submit pipeline to RustCI server
    log_info "Submitting pipeline to RustCI server..."
    
    local pipeline_content=$(cat "/tmp/${TEST_PIPELINE_NAME}.yaml")
    local pipeline_data=$(cat << EOF
{
    "name": "$TEST_PIPELINE_NAME",
    "yaml_content": $(echo "$pipeline_content" | jq -Rs .)
}
EOF
    )
    
    local submit_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$pipeline_data" \
        "$RUSTCI_SERVER/api/ci/pipelines" 2>/dev/null || echo "")
    
    if [[ -n "$submit_response" ]]; then
        log_success "Pipeline submitted successfully"
        log_info "Response: $submit_response"
        
        # Extract pipeline ID if available
        local pipeline_id=$(echo "$submit_response" | jq -r '.id // .pipeline_id // empty' 2>/dev/null || echo "")
        
        if [[ -n "$pipeline_id" ]]; then
            log_info "Pipeline ID: $pipeline_id"
            
            # Monitor pipeline execution
            monitor_pipeline_execution "$pipeline_id"
        else
            log_warning "Could not extract pipeline ID from response"
        fi
    else
        log_error "Failed to submit pipeline"
        return 1
    fi
}

# Monitor pipeline execution
monitor_pipeline_execution() {
    local pipeline_id="$1"
    log_info "Monitoring pipeline execution for ID: $pipeline_id"
    
    local max_wait=300  # 5 minutes
    local wait_time=0
    local check_interval=10
    
    while [[ $wait_time -lt $max_wait ]]; do
        local status_response=$(curl -s "$RUSTCI_SERVER/api/ci/pipelines/$pipeline_id/status" 2>/dev/null || echo "")
        
        if [[ -n "$status_response" ]]; then
            local status=$(echo "$status_response" | jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")
            log_info "Pipeline status: $status"
            
            case "$status" in
                "completed"|"success"|"finished")
                    log_success "Pipeline execution completed successfully"
                    return 0
                    ;;
                "failed"|"error")
                    log_error "Pipeline execution failed"
                    log_info "Status response: $status_response"
                    return 1
                    ;;
                "running"|"in_progress"|"executing")
                    log_info "Pipeline is still running... (waited ${wait_time}s)"
                    ;;
                *)
                    log_info "Pipeline status: $status (waited ${wait_time}s)"
                    ;;
            esac
        else
            log_warning "Could not get pipeline status"
        fi
        
        sleep $check_interval
        wait_time=$((wait_time + check_interval))
    done
    
    log_warning "Pipeline monitoring timed out after ${max_wait}s"
    return 1
}

# Test Windows deployment integration
test_windows_integration() {
    if [[ -n "$WINDOWS_HOST" && -n "$WINDOWS_USER" ]]; then
        log_test "Testing Windows deployment integration..."
        
        # Test connectivity
        if ping -c 3 "$WINDOWS_HOST" &> /dev/null; then
            log_success "Windows machine is reachable"
        else
            log_error "Cannot reach Windows machine"
            return 1
        fi
        
        # Test SSH
        if ssh -o ConnectTimeout=10 -o BatchMode=yes "$WINDOWS_USER@$WINDOWS_HOST" "echo 'SSH test successful'" &> /dev/null; then
            log_success "SSH connection to Windows working"
        else
            log_error "SSH connection to Windows failed"
            return 1
        fi
        
        # Test deployment server
        if curl -s -f --connect-timeout 10 "http://$WINDOWS_HOST:3000/health" &> /dev/null; then
            log_success "Windows deployment server is responding"
        else
            log_warning "Windows deployment server is not responding"
            log_info "Make sure to run: windows-deploy-server.bat start"
        fi
        
        # Test end-to-end deployment
        log_info "Testing end-to-end deployment to Windows..."
        
        # Create test artifact
        local test_artifact="/tmp/integration-test-$(date +%Y%m%d-%H%M%S).tar.gz"
        echo "Integration test artifact created on Mac at $(date)" > /tmp/test-app.txt
        tar -czf "$test_artifact" -C /tmp test-app.txt
        
        # Deploy to Windows
        if scp -o ConnectTimeout=10 -o StrictHostKeyChecking=no "$test_artifact" "$WINDOWS_USER@$WINDOWS_HOST:C:/temp/"; then
            log_success "Test artifact deployed to Windows"
            
            # Trigger deployment
            if ssh -o ConnectTimeout=10 "$WINDOWS_USER@$WINDOWS_HOST" "if exist C:\\temp\\windows-deploy-server.bat (C:\\temp\\windows-deploy-server.bat deploy $(basename "$test_artifact")) else (echo 'Deployment server not found')"; then
                log_success "Windows deployment completed"
            else
                log_warning "Windows deployment trigger failed"
            fi
        else
            log_error "Failed to deploy test artifact to Windows"
        fi
        
        # Cleanup
        rm -f "$test_artifact" /tmp/test-app.txt
    else
        log_info "Windows integration test skipped (WINDOWS_HOST/WINDOWS_USER not set)"
    fi
}

# Show integration status
show_integration_status() {
    log_info "RustCI Runner Integration Status:"
    echo "=================================="
    
    echo ""
    echo "RustCI Server:"
    if curl -s -f --connect-timeout 5 "$RUSTCI_SERVER/health" &> /dev/null; then
        echo "  Status: ✅ Running"
        echo "  URL: $RUSTCI_SERVER"
        local health_response=$(curl -s "$RUSTCI_SERVER/health" 2>/dev/null || echo "")
        if [[ -n "$health_response" ]]; then
            echo "  Health: $health_response"
        fi
    else
        echo "  Status: ❌ Not Running"
        echo "  URL: $RUSTCI_SERVER"
    fi
    
    echo ""
    echo "DinD Runner:"
    if docker ps | grep -q "$RUNNER_NAME"; then
        echo "  Status: ✅ Running"
        echo "  Container: $RUNNER_NAME"
        echo "  Image: $(docker ps --filter name=$RUNNER_NAME --format '{{.Image}}')"
    else
        echo "  Status: ❌ Not Running"
        echo "  Container: $RUNNER_NAME"
    fi
    
    echo ""
    echo "Windows Integration:"
    if [[ -n "$WINDOWS_HOST" && -n "$WINDOWS_USER" ]]; then
        echo "  Target: $WINDOWS_USER@$WINDOWS_HOST"
        if ping -c 1 "$WINDOWS_HOST" &> /dev/null; then
            echo "  Connectivity: ✅ Reachable"
        else
            echo "  Connectivity: ❌ Not Reachable"
        fi
        
        if curl -s -f --connect-timeout 5 "http://$WINDOWS_HOST:3000/health" &> /dev/null; then
            echo "  Deploy Server: ✅ Running"
        else
            echo "  Deploy Server: ❌ Not Running"
        fi
    else
        echo "  Target: Not configured"
        echo "  Set WINDOWS_HOST and WINDOWS_USER environment variables"
    fi
    
    echo ""
    echo "Docker Environment:"
    echo "  Docker Version: $(docker --version 2>/dev/null || echo 'Not available')"
    echo "  Docker Status: $(docker info --format '{{.ServerVersion}}' 2>/dev/null && echo 'Running' || echo 'Not running')"
    echo "  Networks: $(docker network ls | grep rustci | wc -l) RustCI networks"
    echo "  Containers: $(docker ps | grep rustci | wc -l) RustCI containers running"
}

# Run comprehensive integration test
run_comprehensive_test() {
    log_info "Running comprehensive RustCI runner integration test..."
    echo "========================================================"
    
    local tests_passed=0
    local tests_failed=0
    
    # Run tests
    local test_functions=(
        "check_rustci_server"
        "check_dind_runner"
        "test_runner_registration"
        "test_pipeline_execution"
        "test_windows_integration"
    )
    
    for test_func in "${test_functions[@]}"; do
        echo ""
        if $test_func; then
            ((tests_passed++))
        else
            ((tests_failed++))
        fi
    done
    
    echo ""
    echo "========================================================"
    log_info "Integration Test Summary:"
    echo "  Tests Passed: $tests_passed"
    echo "  Tests Failed: $tests_failed"
    echo "  Total Tests:  $((tests_passed + tests_failed))"
    
    echo ""
    show_integration_status
    
    echo ""
    if [[ $tests_failed -eq 0 ]]; then
        log_success "All integration tests passed! Your RustCI setup is working."
        echo ""
        log_info "Your setup supports:"
        echo "1. ✅ RustCI server communication"
        echo "2. ✅ DinD runner registration and execution"
        echo "3. ✅ Pipeline submission and monitoring"
        echo "4. ✅ Docker-based build and deployment"
        if [[ -n "$WINDOWS_HOST" ]]; then
            echo "5. ✅ Cross-platform deployment to Windows"
        fi
    else
        log_warning "Some integration tests failed. Please check the issues above."
        echo ""
        log_info "Common solutions:"
        echo "1. Start RustCI server: cargo run --bin rustci-server"
        echo "2. Start DinD runner: ./scripts/runners/mac-dind-runner.sh start"
        echo "3. Configure Windows deployment: set WINDOWS_HOST and WINDOWS_USER"
        echo "4. Check network connectivity and firewall settings"
    fi
    
    # Cleanup
    rm -f "/tmp/${TEST_PIPELINE_NAME}.yaml"
    
    return $tests_failed
}

# Show help
show_help() {
    echo "RustCI Runner Integration Test Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "COMMANDS:"
    echo "  test        - Run comprehensive integration test (default)"
    echo "  status      - Show integration status"
    echo "  pipeline    - Test pipeline execution only"
    echo "  windows     - Test Windows integration only"
    echo "  help        - Show this help"
    echo ""
    echo "ENVIRONMENT VARIABLES:"
    echo "  RUSTCI_SERVER - RustCI server URL (default: http://localhost:8080)"
    echo "  WINDOWS_HOST  - IP address of Windows machine (optional)"
    echo "  WINDOWS_USER  - Username for Windows machine (optional)"
    echo ""
    echo "EXAMPLES:"
    echo "  $0 test                    # Run all integration tests"
    echo "  $0 status                  # Show current status"
    echo "  $0 pipeline                # Test pipeline execution"
    echo "  WINDOWS_HOST=192.168.1.100 WINDOWS_USER=myuser $0 test"
    echo ""
}

# Main execution
case "${1:-test}" in
    "test")
        run_comprehensive_test
        ;;
    "status")
        show_integration_status
        ;;
    "pipeline")
        check_rustci_server && check_dind_runner && test_pipeline_execution
        ;;
    "windows")
        test_windows_integration
        ;;
    "help"|"-h"|"--help")
        show_help
        ;;
    *)
        log_error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac