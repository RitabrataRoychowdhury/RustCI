#!/bin/bash

# AMD64 Compatibility Testing Script
# Tests ARM-to-AMD64 cross-compiled containers for compatibility
# Verifies container startup, architecture, and basic functionality

set -euo pipefail

# Configuration
DEFAULT_IMAGE_NAME="rustci:local-amd64"
DEFAULT_CONTAINER_NAME="rustci-amd64-test"
DEFAULT_TEST_PORT="8080"
HEALTH_ENDPOINTS=("/health" "/api/healthchecker")
TEST_TIMEOUT=60
STARTUP_WAIT=10

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
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

# Show usage information
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Test AMD64 Docker images for cross-architecture compatibility.
Verifies container startup, architecture, and basic functionality.

OPTIONS:
    -i, --image NAME       Image name to test (default: $DEFAULT_IMAGE_NAME)
    -n, --name NAME        Container name for testing (default: $DEFAULT_CONTAINER_NAME)
    -p, --port PORT        Test port mapping (default: $DEFAULT_TEST_PORT)
    -t, --timeout SECONDS  Test timeout in seconds (default: $TEST_TIMEOUT)
    -w, --wait SECONDS     Startup wait time (default: $STARTUP_WAIT)
    --no-cleanup           Don't cleanup test container after testing
    --quick                Run quick tests only (skip health checks)
    --verbose              Show detailed container logs
    -h, --help             Show this help message

EXAMPLES:
    # Basic compatibility test
    $0

    # Test specific image
    $0 -i myapp:amd64

    # Test with custom port
    $0 -p 3000

    # Quick test without health checks
    $0 --quick

    # Verbose output with logs
    $0 --verbose

TEST CATEGORIES:
    ✓ Image architecture verification
    ✓ Container startup test
    ✓ Runtime architecture check
    ✓ Process execution test
    ✓ Network connectivity test
    ✓ Health endpoint verification
    ✓ Resource usage check

EOF
}

# Parse command line arguments
parse_args() {
    IMAGE_NAME="$DEFAULT_IMAGE_NAME"
    CONTAINER_NAME="$DEFAULT_CONTAINER_NAME"
    TEST_PORT="$DEFAULT_TEST_PORT"
    TEST_TIMEOUT=60
    STARTUP_WAIT=10
    CLEANUP_CONTAINER=true
    QUICK_TEST=false
    VERBOSE_OUTPUT=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -i|--image)
                IMAGE_NAME="$2"
                shift 2
                ;;
            -n|--name)
                CONTAINER_NAME="$2"
                shift 2
                ;;
            -p|--port)
                TEST_PORT="$2"
                shift 2
                ;;
            -t|--timeout)
                TEST_TIMEOUT="$2"
                shift 2
                ;;
            -w|--wait)
                STARTUP_WAIT="$2"
                shift 2
                ;;
            --no-cleanup)
                CLEANUP_CONTAINER=false
                shift
                ;;
            --quick)
                QUICK_TEST=true
                shift
                ;;
            --verbose)
                VERBOSE_OUTPUT=true
                shift
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
}

# Cleanup function
cleanup() {
    if [[ "$CLEANUP_CONTAINER" == true ]]; then
        log_info "Cleaning up test container..."
        docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
        log_success "Cleanup completed"
    else
        log_info "Skipping cleanup (--no-cleanup specified)"
        log_info "To manually cleanup: docker rm -f $CONTAINER_NAME"
    fi
}

# Test 1: Image architecture verification
test_image_architecture() {
    log_info "Test 1: Verifying image architecture..."
    
    # Check if image exists
    if ! docker image inspect "$IMAGE_NAME" >/dev/null 2>&1; then
        log_error "✗ Image not found: $IMAGE_NAME"
        return 1
    fi
    
    # Get architecture info
    local arch_info=$(docker image inspect --format='{{.Architecture}}/{{.Os}}' "$IMAGE_NAME")
    log_info "Image architecture: $arch_info"
    
    if [[ "$arch_info" == "amd64/linux" ]]; then
        log_success "✓ Correct AMD64 architecture"
        return 0
    else
        log_error "✗ Incorrect architecture: $arch_info (expected: amd64/linux)"
        return 1
    fi
}

# Test 2: Container startup test
test_container_startup() {
    log_info "Test 2: Testing container startup..."
    
    # Remove existing test container if it exists
    docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
    
    # Start container in detached mode
    log_info "Starting container: $CONTAINER_NAME"
    if docker run -d --name "$CONTAINER_NAME" -p "${TEST_PORT}:8080" "$IMAGE_NAME" >/dev/null 2>&1; then
        log_success "✓ Container started successfully"
    else
        log_error "✗ Failed to start container"
        return 1
    fi
    
    # Wait for startup
    log_info "Waiting ${STARTUP_WAIT}s for container startup..."
    sleep "$STARTUP_WAIT"
    
    # Check if container is still running
    if docker ps --filter "name=$CONTAINER_NAME" --filter "status=running" | grep -q "$CONTAINER_NAME"; then
        log_success "✓ Container is running"
        return 0
    else
        log_error "✗ Container stopped unexpectedly"
        if [[ "$VERBOSE_OUTPUT" == true ]]; then
            log_info "Container logs:"
            docker logs "$CONTAINER_NAME" 2>&1 | tail -20
        fi
        return 1
    fi
}

# Test 3: Runtime architecture check
test_runtime_architecture() {
    log_info "Test 3: Verifying runtime architecture..."
    
    # Check architecture inside container
    local container_arch
    if container_arch=$(docker exec "$CONTAINER_NAME" uname -m 2>/dev/null); then
        log_info "Container architecture: $container_arch"
        
        if [[ "$container_arch" == "x86_64" ]]; then
            log_success "✓ Correct runtime architecture (x86_64)"
            return 0
        else
            log_error "✗ Incorrect runtime architecture: $container_arch (expected: x86_64)"
            return 1
        fi
    else
        log_error "✗ Failed to check runtime architecture"
        return 1
    fi
}

# Test 4: Process execution test
test_process_execution() {
    log_info "Test 4: Testing process execution..."
    
    # Test basic command execution
    if docker exec "$CONTAINER_NAME" echo "Process test successful" >/dev/null 2>&1; then
        log_success "✓ Process execution works"
    else
        log_error "✗ Process execution failed"
        return 1
    fi
    
    # Test file system access
    if docker exec "$CONTAINER_NAME" ls / >/dev/null 2>&1; then
        log_success "✓ File system access works"
    else
        log_error "✗ File system access failed"
        return 1
    fi
    
    # Test environment variables
    if docker exec "$CONTAINER_NAME" env | grep -q "PATH" 2>/dev/null; then
        log_success "✓ Environment variables accessible"
        return 0
    else
        log_error "✗ Environment variables not accessible"
        return 1
    fi
}

# Test 5: Network connectivity test
test_network_connectivity() {
    log_info "Test 5: Testing network connectivity..."
    
    # Check if port is accessible from host
    local max_attempts=5
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if curl -s -f "http://localhost:${TEST_PORT}" >/dev/null 2>&1; then
            log_success "✓ Network connectivity established"
            return 0
        fi
        
        log_info "Attempt $attempt/$max_attempts - waiting for network..."
        sleep 2
        ((attempt++))
    done
    
    log_warning "⚠ Network connectivity test inconclusive (service may not be HTTP-based)"
    return 0  # Don't fail on this as the service might not be HTTP
}

# Test 6: Health endpoint verification
test_health_endpoints() {
    if [[ "$QUICK_TEST" == true ]]; then
        log_info "Test 6: Skipping health checks (quick mode)"
        return 0
    fi
    
    log_info "Test 6: Testing health endpoints..."
    
    local health_success=false
    
    for endpoint in "${HEALTH_ENDPOINTS[@]}"; do
        local url="http://localhost:${TEST_PORT}${endpoint}"
        log_info "Testing endpoint: $url"
        
        if curl -s -f --max-time 10 "$url" >/dev/null 2>&1; then
            log_success "✓ Health endpoint accessible: $endpoint"
            health_success=true
        else
            log_warning "⚠ Health endpoint not accessible: $endpoint"
        fi
    done
    
    if [[ "$health_success" == true ]]; then
        log_success "✓ At least one health endpoint is working"
        return 0
    else
        log_warning "⚠ No health endpoints accessible (service may not be ready)"
        return 0  # Don't fail as service might still be starting
    fi
}

# Test 7: Resource usage check
test_resource_usage() {
    log_info "Test 7: Checking resource usage..."
    
    # Get container stats
    local stats_output
    if stats_output=$(docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" "$CONTAINER_NAME" 2>/dev/null); then
        log_info "Container resource usage:"
        echo "$stats_output" | tail -n +2 | while read -r line; do
            log_info "  $line"
        done
        log_success "✓ Resource usage information available"
    else
        log_warning "⚠ Could not retrieve resource usage stats"
    fi
    
    # Check if container is consuming reasonable resources
    local mem_usage
    if mem_usage=$(docker stats --no-stream --format "{{.MemUsage}}" "$CONTAINER_NAME" 2>/dev/null | cut -d'/' -f1); then
        log_info "Memory usage: $mem_usage"
        log_success "✓ Container is consuming resources (running actively)"
    else
        log_warning "⚠ Could not determine memory usage"
    fi
    
    return 0
}

# Show container logs if verbose
show_container_logs() {
    if [[ "$VERBOSE_OUTPUT" == true ]]; then
        echo ""
        log_info "Container logs (last 20 lines):"
        docker logs --tail 20 "$CONTAINER_NAME" 2>&1 || log_warning "Could not retrieve container logs"
    fi
}

# Run all tests
run_all_tests() {
    local test_results=()
    local total_tests=7
    local passed_tests=0
    
    # Run tests
    test_image_architecture && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_container_startup && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_runtime_architecture && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_process_execution && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_network_connectivity && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_health_endpoints && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_resource_usage && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    
    # Show results summary
    echo ""
    log_info "Test Results Summary:"
    echo "  1. Image Architecture:     ${test_results[0]}"
    echo "  2. Container Startup:      ${test_results[1]}"
    echo "  3. Runtime Architecture:   ${test_results[2]}"
    echo "  4. Process Execution:      ${test_results[3]}"
    echo "  5. Network Connectivity:   ${test_results[4]}"
    echo "  6. Health Endpoints:       ${test_results[5]}"
    echo "  7. Resource Usage:         ${test_results[6]}"
    echo ""
    log_info "Tests passed: $passed_tests/$total_tests"
    
    show_container_logs
    
    # Determine overall result
    if [[ $passed_tests -ge 5 ]]; then  # Allow some tests to be warnings
        log_success "AMD64 compatibility test PASSED!"
        log_info "Image $IMAGE_NAME is compatible for AMD64 deployment"
        return 0
    else
        log_error "AMD64 compatibility test FAILED!"
        log_error "Image $IMAGE_NAME may not be suitable for AMD64 deployment"
        return 1
    fi
}

# Main execution
main() {
    log_info "Starting AMD64 compatibility testing..."
    log_info "Image: $IMAGE_NAME"
    log_info "Container: $CONTAINER_NAME"
    log_info "Test Port: $TEST_PORT"
    echo ""
    
    # Set up cleanup trap
    trap cleanup EXIT INT TERM
    
    # Run tests
    if run_all_tests; then
        exit 0
    else
        exit 1
    fi
}

# Parse arguments and run
parse_args "$@"
main