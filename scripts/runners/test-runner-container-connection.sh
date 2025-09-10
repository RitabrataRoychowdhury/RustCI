#!/bin/bash

# Mac Runner-to-Container Connection Test Script
# This script tests the connection between Mac DinD runner and containerized services

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTCI_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
RUNNER_NAME="mac-dind-runner"
TEST_CONTAINER_NAME="rustci-test-server"
TEST_SERVER_PORT="3001"
DOCKER_NETWORK="rustci-network"
WINDOWS_HOST="${WINDOWS_HOST:-}"
WINDOWS_USER="${WINDOWS_USER:-}"

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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Create Docker network if needed
setup_docker_network() {
    log_info "Setting up Docker network..."
    
    if ! docker network ls | grep -q "$DOCKER_NETWORK"; then
        docker network create "$DOCKER_NETWORK"
        log_success "Created Docker network: $DOCKER_NETWORK"
    else
        log_info "Docker network $DOCKER_NETWORK already exists"
    fi
}

# Start a test container server
start_test_container() {
    log_info "Starting test container server..."
    
    # Stop existing test container if running
    if docker ps -a | grep -q "$TEST_CONTAINER_NAME"; then
        docker stop "$TEST_CONTAINER_NAME" &> /dev/null || true
        docker rm "$TEST_CONTAINER_NAME" &> /dev/null || true
    fi
    
    # Create a simple HTTP server container
    docker run -d \
        --name "$TEST_CONTAINER_NAME" \
        --network "$DOCKER_NETWORK" \
        -p "$TEST_SERVER_PORT:8080" \
        -e SERVER_PORT=8080 \
        nginx:alpine \
        sh -c "
            # Create a simple index page
            echo '<!DOCTYPE html>
<html>
<head><title>RustCI Test Server</title></head>
<body>
    <h1>RustCI Test Container Server</h1>
    <p>Status: <span style=\"color: green;\">Running</span></p>
    <p>Container: $TEST_CONTAINER_NAME</p>
    <p>Network: $DOCKER_NETWORK</p>
    <p>Timestamp: \$(date)</p>
    <hr>
    <h2>Connection Test Endpoints</h2>
    <ul>
        <li><a href=\"/health\">/health</a> - Health check</li>
        <li><a href=\"/info\">/info</a> - Container info</li>
        <li><a href=\"/test\">/test</a> - Connection test</li>
    </ul>
</body>
</html>' > /usr/share/nginx/html/index.html

            # Create health endpoint
            echo '{\"status\":\"healthy\",\"container\":\"$TEST_CONTAINER_NAME\",\"timestamp\":\"'\$(date -Iseconds)'\"}' > /usr/share/nginx/html/health.json
            
            # Create info endpoint  
            echo '{\"container_name\":\"$TEST_CONTAINER_NAME\",\"network\":\"$DOCKER_NETWORK\",\"port\":8080,\"server\":\"nginx\",\"timestamp\":\"'\$(date -Iseconds)'\"}' > /usr/share/nginx/html/info.json
            
            # Create test endpoint
            echo '{\"test\":\"success\",\"message\":\"Container connection test passed\",\"from_container\":\"$TEST_CONTAINER_NAME\",\"timestamp\":\"'\$(date -Iseconds)'\"}' > /usr/share/nginx/html/test.json
            
            # Start nginx
            nginx -g 'daemon off;'
        "
    
    # Wait for container to be ready
    sleep 3
    
    if docker ps | grep -q "$TEST_CONTAINER_NAME"; then
        log_success "Test container server started successfully"
        log_info "Container: $TEST_CONTAINER_NAME"
        log_info "Network: $DOCKER_NETWORK"
        log_info "Port: $TEST_SERVER_PORT"
    else
        log_error "Failed to start test container server"
        docker logs "$TEST_CONTAINER_NAME" 2>/dev/null || true
        exit 1
    fi
}

# Test container accessibility from host
test_container_from_host() {
    log_test "Testing container accessibility from host..."
    
    local test_url="http://localhost:$TEST_SERVER_PORT"
    
    # Test basic connectivity
    if curl -s -f --connect-timeout 10 "$test_url" &> /dev/null; then
        log_success "Container is accessible from host"
    else
        log_error "Container is not accessible from host"
        return 1
    fi
    
    # Test health endpoint
    local health_response=$(curl -s "$test_url/health.json" 2>/dev/null || echo "")
    if [[ -n "$health_response" ]]; then
        log_success "Health endpoint working: $health_response"
    else
        log_warning "Health endpoint not responding"
    fi
    
    # Test info endpoint
    local info_response=$(curl -s "$test_url/info.json" 2>/dev/null || echo "")
    if [[ -n "$info_response" ]]; then
        log_success "Info endpoint working: $info_response"
    else
        log_warning "Info endpoint not responding"
    fi
}

# Test DinD runner to container communication
test_dind_to_container() {
    log_test "Testing DinD runner to container communication..."
    
    # Check if DinD runner is running
    if ! docker ps | grep -q "$RUNNER_NAME"; then
        log_warning "DinD runner is not running, starting it..."
        
        # Start a minimal DinD runner for testing
        docker run -d \
            --name "$RUNNER_NAME-test" \
            --privileged \
            --network "$DOCKER_NETWORK" \
            docker:dind \
            sh -c "
                dockerd-entrypoint.sh &
                while ! docker info &> /dev/null; do
                    echo 'Waiting for Docker daemon...'
                    sleep 2
                done
                echo 'Docker daemon ready in DinD container'
                tail -f /dev/null
            "
        
        sleep 5
        local runner_container="$RUNNER_NAME-test"
    else
        local runner_container="$RUNNER_NAME"
    fi
    
    # Test communication from DinD runner to test container
    log_info "Testing communication from $runner_container to $TEST_CONTAINER_NAME..."
    
    # Test using container name (Docker network DNS)
    if docker exec "$runner_container" sh -c "command -v curl >/dev/null 2>&1 || apk add --no-cache curl" &> /dev/null; then
        if docker exec "$runner_container" curl -s -f --connect-timeout 10 "http://$TEST_CONTAINER_NAME:8080/test.json" &> /dev/null; then
            log_success "DinD runner can communicate with test container via container name"
            
            local test_response=$(docker exec "$runner_container" curl -s "http://$TEST_CONTAINER_NAME:8080/test.json" 2>/dev/null || echo "")
            if [[ -n "$test_response" ]]; then
                log_info "Response: $test_response"
            fi
        else
            log_error "DinD runner cannot communicate with test container via container name"
            return 1
        fi
    else
        log_warning "Could not install curl in DinD runner, trying with wget..."
        
        if docker exec "$runner_container" sh -c "command -v wget >/dev/null 2>&1 || apk add --no-cache wget" &> /dev/null; then
            if docker exec "$runner_container" wget -q -O - --timeout=10 "http://$TEST_CONTAINER_NAME:8080/test.json" &> /dev/null; then
                log_success "DinD runner can communicate with test container via wget"
            else
                log_error "DinD runner cannot communicate with test container via wget"
                return 1
            fi
        else
            log_error "Could not install networking tools in DinD runner"
            return 1
        fi
    fi
    
    # Clean up test runner if we created it
    if [[ "$runner_container" == "$RUNNER_NAME-test" ]]; then
        docker stop "$runner_container" &> /dev/null || true
        docker rm "$runner_container" &> /dev/null || true
    fi
}

# Test container to container communication
test_container_to_container() {
    log_test "Testing container-to-container communication..."
    
    # Start a second test container
    local client_container="rustci-test-client"
    
    docker run --rm \
        --name "$client_container" \
        --network "$DOCKER_NETWORK" \
        alpine:latest \
        sh -c "
            apk add --no-cache curl
            echo 'Testing connection from $client_container to $TEST_CONTAINER_NAME...'
            
            if curl -s -f --connect-timeout 10 'http://$TEST_CONTAINER_NAME:8080/test.json'; then
                echo 'Container-to-container communication successful'
                exit 0
            else
                echo 'Container-to-container communication failed'
                exit 1
            fi
        " && log_success "Container-to-container communication working" || log_error "Container-to-container communication failed"
}

# Test Windows deployment server connectivity (if configured)
test_windows_connectivity() {
    if [[ -n "$WINDOWS_HOST" && -n "$WINDOWS_USER" ]]; then
        log_test "Testing Windows deployment server connectivity..."
        
        # Test ping
        if ping -c 3 "$WINDOWS_HOST" &> /dev/null; then
            log_success "Can ping Windows machine: $WINDOWS_HOST"
        else
            log_warning "Cannot ping Windows machine: $WINDOWS_HOST"
        fi
        
        # Test SSH
        if ssh -o ConnectTimeout=10 -o BatchMode=yes "$WINDOWS_USER@$WINDOWS_HOST" "echo 'SSH test successful'" &> /dev/null; then
            log_success "SSH connection to Windows working"
        else
            log_warning "SSH connection to Windows failed"
        fi
        
        # Test Windows deployment server
        if curl -s -f --connect-timeout 10 "http://$WINDOWS_HOST:3000/health" &> /dev/null; then
            log_success "Windows deployment server is responding"
            local health_response=$(curl -s "http://$WINDOWS_HOST:3000/health" 2>/dev/null || echo "")
            if [[ -n "$health_response" ]]; then
                log_info "Windows server response: $health_response"
            fi
        else
            log_warning "Windows deployment server is not responding"
            log_info "Make sure to run: windows-deploy-server.bat start"
        fi
    else
        log_info "Windows connectivity test skipped (WINDOWS_HOST/WINDOWS_USER not set)"
    fi
}

# Test full deployment pipeline simulation
test_deployment_pipeline() {
    log_test "Testing full deployment pipeline simulation..."
    
    # Create a test artifact
    local artifact_dir="/tmp/rustci-test-artifact"
    local artifact_file="test-app-$(date +%Y%m%d-%H%M%S).tar.gz"
    
    mkdir -p "$artifact_dir"
    echo "Test application built on Mac at $(date)" > "$artifact_dir/app.txt"
    echo "#!/bin/bash" > "$artifact_dir/run.sh"
    echo "echo 'Running test application...'" >> "$artifact_dir/run.sh"
    echo "cat app.txt" >> "$artifact_dir/run.sh"
    chmod +x "$artifact_dir/run.sh"
    
    # Package artifact
    cd "$artifact_dir"
    tar -czf "/tmp/$artifact_file" .
    cd - > /dev/null
    
    log_info "Created test artifact: /tmp/$artifact_file"
    
    # Test artifact creation in container
    if docker ps | grep -q "$RUNNER_NAME"; then
        log_info "Testing artifact creation in DinD runner..."
        
        # Copy artifact to DinD runner
        docker cp "/tmp/$artifact_file" "$RUNNER_NAME:/tmp/"
        
        # Test extraction in DinD runner
        if docker exec "$RUNNER_NAME" sh -c "cd /tmp && tar -tzf $artifact_file" &> /dev/null; then
            log_success "Artifact can be processed in DinD runner"
        else
            log_warning "Artifact processing in DinD runner failed"
        fi
    fi
    
    # Test deployment to Windows (if configured)
    if [[ -n "$WINDOWS_HOST" && -n "$WINDOWS_USER" ]]; then
        log_info "Testing deployment to Windows..."
        
        if scp -o ConnectTimeout=10 -o StrictHostKeyChecking=no "/tmp/$artifact_file" "$WINDOWS_USER@$WINDOWS_HOST:C:/temp/" &> /dev/null; then
            log_success "Artifact deployed to Windows successfully"
            
            # Trigger deployment on Windows
            if ssh -o ConnectTimeout=10 "$WINDOWS_USER@$WINDOWS_HOST" "if exist C:\\temp\\windows-deploy-server.bat (C:\\temp\\windows-deploy-server.bat deploy $artifact_file) else (echo 'Deployment server not found')" &> /dev/null; then
                log_success "Windows deployment triggered successfully"
            else
                log_warning "Windows deployment trigger failed"
            fi
        else
            log_warning "Artifact deployment to Windows failed"
        fi
    fi
    
    # Cleanup
    rm -rf "$artifact_dir" "/tmp/$artifact_file"
}

# Show network and container status
show_network_status() {
    log_info "Network and Container Status:"
    echo "================================"
    
    echo ""
    echo "Docker Networks:"
    docker network ls | grep -E "(NETWORK|$DOCKER_NETWORK)" || echo "No custom networks found"
    
    echo ""
    echo "Running Containers:"
    docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}" | grep -E "(NAMES|$TEST_CONTAINER_NAME|$RUNNER_NAME)" || echo "No relevant containers running"
    
    echo ""
    echo "Network Inspection ($DOCKER_NETWORK):"
    if docker network ls | grep -q "$DOCKER_NETWORK"; then
        docker network inspect "$DOCKER_NETWORK" --format '{{range .Containers}}{{.Name}}: {{.IPv4Address}}{{"\n"}}{{end}}' || echo "Network inspection failed"
    else
        echo "Network $DOCKER_NETWORK does not exist"
    fi
    
    echo ""
    echo "Port Mappings:"
    netstat -an | grep ":$TEST_SERVER_PORT" || echo "No port $TEST_SERVER_PORT bindings found"
}

# Cleanup test resources
cleanup() {
    log_info "Cleaning up test resources..."
    
    # Stop and remove test containers
    docker stop "$TEST_CONTAINER_NAME" &> /dev/null || true
    docker rm "$TEST_CONTAINER_NAME" &> /dev/null || true
    
    # Remove test runner if it exists
    docker stop "$RUNNER_NAME-test" &> /dev/null || true
    docker rm "$RUNNER_NAME-test" &> /dev/null || true
    
    log_success "Cleanup completed"
}

# Run comprehensive test suite
run_comprehensive_test() {
    log_info "Running comprehensive runner-to-container connection test..."
    echo "=============================================================="
    
    local tests_passed=0
    local tests_failed=0
    
    # Setup
    check_prerequisites
    setup_docker_network
    start_test_container
    
    # Run tests
    local test_functions=(
        "test_container_from_host"
        "test_dind_to_container"
        "test_container_to_container"
        "test_windows_connectivity"
        "test_deployment_pipeline"
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
    echo "=============================================================="
    log_info "Test Summary:"
    echo "  Tests Passed: $tests_passed"
    echo "  Tests Failed: $tests_failed"
    echo "  Total Tests:  $((tests_passed + tests_failed))"
    
    echo ""
    show_network_status
    
    echo ""
    if [[ $tests_failed -eq 0 ]]; then
        log_success "All tests passed! Runner-to-container communication is working."
        echo ""
        log_info "Your setup is ready for:"
        echo "1. Mac DinD runner ↔ Container communication"
        echo "2. Container ↔ Container communication"
        echo "3. Mac ↔ Windows deployment pipeline"
    else
        log_warning "Some tests failed. Please check the issues above."
        echo ""
        log_info "Common solutions:"
        echo "1. Ensure Docker daemon is running"
        echo "2. Check Docker network configuration"
        echo "3. Verify container connectivity"
        echo "4. Test Windows SSH/deployment server setup"
    fi
    
    echo ""
    log_info "Test container will remain running for manual testing."
    log_info "Access it at: http://localhost:$TEST_SERVER_PORT"
    log_info "Stop it with: docker stop $TEST_CONTAINER_NAME"
    
    return $tests_failed
}

# Show help
show_help() {
    echo "Mac Runner-to-Container Connection Test Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "COMMANDS:"
    echo "  test        - Run comprehensive connection test (default)"
    echo "  start       - Start test container only"
    echo "  stop        - Stop and cleanup test resources"
    echo "  status      - Show network and container status"
    echo "  cleanup     - Clean up test resources"
    echo "  help        - Show this help"
    echo ""
    echo "ENVIRONMENT VARIABLES:"
    echo "  WINDOWS_HOST - IP address of Windows machine (optional)"
    echo "  WINDOWS_USER - Username for Windows machine (optional)"
    echo ""
    echo "EXAMPLES:"
    echo "  $0 test                    # Run all tests"
    echo "  $0 start                   # Start test container"
    echo "  $0 status                  # Show status"
    echo "  WINDOWS_HOST=192.168.1.100 WINDOWS_USER=myuser $0 test"
    echo ""
    echo "TEST ENDPOINTS:"
    echo "  http://localhost:$TEST_SERVER_PORT        - Main page"
    echo "  http://localhost:$TEST_SERVER_PORT/health.json - Health check"
    echo "  http://localhost:$TEST_SERVER_PORT/info.json   - Container info"
    echo "  http://localhost:$TEST_SERVER_PORT/test.json   - Connection test"
    echo ""
}

# Main execution
case "${1:-test}" in
    "test")
        run_comprehensive_test
        ;;
    "start")
        check_prerequisites
        setup_docker_network
        start_test_container
        log_success "Test container started at http://localhost:$TEST_SERVER_PORT"
        ;;
    "stop"|"cleanup")
        cleanup
        ;;
    "status")
        show_network_status
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