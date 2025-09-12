#!/bin/bash

# Test Script for SSH Transfer and Deployment System
# Validates all components of the secure SSH transfer and deployment system

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_IMAGE="rustci:test-$(date +%s)"
TEST_CONTAINER="rustci-test"
TEST_PORT=8081
VERBOSE=false

# Function to print colored output
print_status() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
Test Script for SSH Transfer and Deployment System

Usage: $0 [OPTIONS]

OPTIONS:
    --skip-ssh-test        Skip SSH connection tests
    --skip-transfer-test   Skip image transfer tests
    --skip-deploy-test     Skip deployment tests
    --skip-rollback-test   Skip rollback tests
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

TESTS:
    1. Script validation tests
    2. SSH connection tests
    3. Image transfer tests (dry run)
    4. Local deployment tests
    5. Rollback functionality tests
    6. Error handling tests

EOF
}

# Parse command line arguments
SKIP_SSH_TEST=false
SKIP_TRANSFER_TEST=false
SKIP_DEPLOY_TEST=false
SKIP_ROLLBACK_TEST=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-ssh-test)
            SKIP_SSH_TEST=true
            shift
            ;;
        --skip-transfer-test)
            SKIP_TRANSFER_TEST=true
            shift
            ;;
        --skip-deploy-test)
            SKIP_DEPLOY_TEST=true
            shift
            ;;
        --skip-rollback-test)
            SKIP_ROLLBACK_TEST=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    ((TESTS_RUN++))
    print_status "Running: $test_name"
    
    if eval "$test_command" >/dev/null 2>&1; then
        print_success "$test_name"
        ((TESTS_PASSED++))
        return 0
    else
        print_error "$test_name"
        ((TESTS_FAILED++))
        return 1
    fi
}

# Function to cleanup test resources
cleanup_test_resources() {
    print_status "Cleaning up test resources..."
    
    # Stop and remove test container
    docker stop "$TEST_CONTAINER" 2>/dev/null || true
    docker rm "$TEST_CONTAINER" 2>/dev/null || true
    
    # Remove test images
    docker rmi "$TEST_IMAGE" 2>/dev/null || true
    docker rmi "${TEST_IMAGE%:*}:previous" 2>/dev/null || true
    docker rmi "${TEST_IMAGE%:*}:backup_"* 2>/dev/null || true
    
    print_status "Cleanup completed"
}

# Function to create test image
create_test_image() {
    print_status "Creating test image: $TEST_IMAGE"
    
    # Create a simple test Dockerfile
    local test_dockerfile=$(mktemp)
    cat > "$test_dockerfile" << 'EOF'
FROM nginx:alpine
COPY <<EOF /usr/share/nginx/html/health
{"status": "healthy", "timestamp": "$(date -Iseconds)"}
EOF
COPY <<EOF /usr/share/nginx/html/api/healthchecker
{"status": "ok", "service": "test"}
EOF
EXPOSE 80
EOF
    
    if docker build -f "$test_dockerfile" -t "$TEST_IMAGE" . >/dev/null 2>&1; then
        rm "$test_dockerfile"
        return 0
    else
        rm "$test_dockerfile"
        return 1
    fi
}

# Test 1: Script validation tests
test_script_validation() {
    print_status "=== Script Validation Tests ==="
    
    # Test if all required scripts exist
    run_test "SSH transfer script exists" "test -f '$SCRIPT_DIR/ssh-transfer.sh'"
    run_test "VPS deploy script exists" "test -f '$SCRIPT_DIR/vps-deploy.sh'"
    run_test "Cross-arch deploy script exists" "test -f '$SCRIPT_DIR/cross-arch-deploy.sh'"
    run_test "Rollback script exists" "test -f '$SCRIPT_DIR/rollback.sh'"
    
    # Test if scripts are executable
    run_test "SSH transfer script is executable" "test -x '$SCRIPT_DIR/ssh-transfer.sh'"
    run_test "VPS deploy script is executable" "test -x '$SCRIPT_DIR/vps-deploy.sh'"
    run_test "Cross-arch deploy script is executable" "test -x '$SCRIPT_DIR/cross-arch-deploy.sh'"
    run_test "Rollback script is executable" "test -x '$SCRIPT_DIR/rollback.sh'"
    
    # Test script help functionality
    run_test "SSH transfer script help" "bash '$SCRIPT_DIR/ssh-transfer.sh' --help"
    run_test "VPS deploy script help" "bash '$SCRIPT_DIR/vps-deploy.sh' --help"
    run_test "Cross-arch deploy script help" "bash '$SCRIPT_DIR/cross-arch-deploy.sh' --help"
    run_test "Rollback script help" "bash '$SCRIPT_DIR/rollback.sh' --help"
}

# Test 2: SSH connection tests
test_ssh_connection() {
    if [ "$SKIP_SSH_TEST" = true ]; then
        print_status "=== Skipping SSH Connection Tests ==="
        return 0
    fi
    
    print_status "=== SSH Connection Tests ==="
    
    # Load environment variables
    if [ -f .env ]; then
        set -a
        source .env
        set +a
    fi
    
    if [ -n "$VPS_IP" ] && [ -n "$VPS_USERNAME" ] && [ -n "$VPS_PASSWORD" ]; then
        # Test SSH connection with password
        run_test "SSH connection with password" "sshpass -p '$VPS_PASSWORD' ssh -o ConnectTimeout=10 -o StrictHostKeyChecking=no '$VPS_USERNAME@$VPS_IP' 'echo test'"
        
        # Test Docker availability on VPS
        run_test "Docker available on VPS" "sshpass -p '$VPS_PASSWORD' ssh -o ConnectTimeout=10 -o StrictHostKeyChecking=no '$VPS_USERNAME@$VPS_IP' 'docker --version'"
    else
        print_warning "SSH connection tests skipped - VPS credentials not available"
    fi
}

# Test 3: Image transfer tests (dry run)
test_image_transfer() {
    if [ "$SKIP_TRANSFER_TEST" = true ]; then
        print_status "=== Skipping Image Transfer Tests ==="
        return 0
    fi
    
    print_status "=== Image Transfer Tests ==="
    
    # Create test image
    if ! create_test_image; then
        print_error "Failed to create test image"
        return 1
    fi
    
    # Test image size calculation
    run_test "Image size calculation" "docker image inspect '$TEST_IMAGE' --format='{{.Size}}' | grep -q '[0-9]'"
    
    # Test image export (without actual transfer)
    run_test "Image export functionality" "docker save '$TEST_IMAGE' | gzip > /tmp/test-image.tar.gz && test -f /tmp/test-image.tar.gz"
    
    # Test image import
    run_test "Image import functionality" "gunzip < /tmp/test-image.tar.gz | docker load"
    
    # Cleanup
    rm -f /tmp/test-image.tar.gz
}

# Test 4: Local deployment tests
test_local_deployment() {
    if [ "$SKIP_DEPLOY_TEST" = true ]; then
        print_status "=== Skipping Local Deployment Tests ==="
        return 0
    fi
    
    print_status "=== Local Deployment Tests ==="
    
    # Create test image if not exists
    if ! docker image inspect "$TEST_IMAGE" >/dev/null 2>&1; then
        if ! create_test_image; then
            print_error "Failed to create test image"
            return 1
        fi
    fi
    
    # Test container deployment
    run_test "Container deployment" "bash '$SCRIPT_DIR/vps-deploy.sh' -c '$TEST_CONTAINER' -p '$TEST_PORT' --no-rollback '$TEST_IMAGE'"
    
    # Wait for container to start
    sleep 5
    
    # Test container is running
    run_test "Container is running" "docker ps --filter name='$TEST_CONTAINER' --format '{{.Names}}' | grep -q '$TEST_CONTAINER'"
    
    # Test health endpoints (using nginx default)
    run_test "Health endpoint accessible" "curl -f -s http://localhost:$TEST_PORT/health"
    
    # Test container logs
    run_test "Container logs available" "docker logs '$TEST_CONTAINER' | wc -l | grep -q '[0-9]'"
}

# Test 5: Rollback functionality tests
test_rollback_functionality() {
    if [ "$SKIP_ROLLBACK_TEST" = true ]; then
        print_status "=== Skipping Rollback Tests ==="
        return 0
    fi
    
    print_status "=== Rollback Functionality Tests ==="
    
    # Create a second test image for rollback testing
    local test_image_v2="${TEST_IMAGE%:*}:test-v2"
    
    # Tag current image as previous
    run_test "Tag current image as previous" "docker tag '$TEST_IMAGE' '${TEST_IMAGE%:*}:previous'"
    
    # Test rollback script status
    run_test "Rollback status command" "bash '$SCRIPT_DIR/rollback.sh' -c '$TEST_CONTAINER' status"
    
    # Test list backups
    run_test "List backups command" "bash '$SCRIPT_DIR/rollback.sh' -c '$TEST_CONTAINER' list-backups"
    
    # Test rollback to previous (if container exists)
    if docker ps --filter name="$TEST_CONTAINER" --format '{{.Names}}' | grep -q "$TEST_CONTAINER"; then
        run_test "Rollback to previous version" "bash '$SCRIPT_DIR/rollback.sh' -c '$TEST_CONTAINER' -p '$TEST_PORT' rollback"
        
        # Wait for rollback to complete
        sleep 5
        
        # Test container is still running after rollback
        run_test "Container running after rollback" "docker ps --filter name='$TEST_CONTAINER' --format '{{.Names}}' | grep -q '$TEST_CONTAINER'"
    fi
}

# Test 6: Error handling tests
test_error_handling() {
    print_status "=== Error Handling Tests ==="
    
    # Test invalid image name
    run_test "Handle invalid image name" "! bash '$SCRIPT_DIR/vps-deploy.sh' -c 'test-invalid' 'nonexistent:image' 2>/dev/null"
    
    # Test invalid container port
    run_test "Handle invalid port" "! bash '$SCRIPT_DIR/vps-deploy.sh' -p 'invalid' '$TEST_IMAGE' 2>/dev/null"
    
    # Test missing required arguments
    run_test "Handle missing arguments in ssh-transfer" "! bash '$SCRIPT_DIR/ssh-transfer.sh' 2>/dev/null"
    
    # Test rollback with no previous image
    run_test "Handle rollback with no previous image" "! bash '$SCRIPT_DIR/rollback.sh' -c 'nonexistent-container' rollback 2>/dev/null"
}

# Function to show test summary
show_test_summary() {
    echo ""
    print_status "=== Test Summary ==="
    echo "  Tests Run: $TESTS_RUN"
    echo "  Tests Passed: $TESTS_PASSED"
    echo "  Tests Failed: $TESTS_FAILED"
    
    if [ $TESTS_FAILED -eq 0 ]; then
        print_success "All tests passed!"
        return 0
    else
        print_error "$TESTS_FAILED test(s) failed"
        return 1
    fi
}

# Main test function
main() {
    print_status "SSH Transfer and Deployment System Test Suite"
    print_status "Starting comprehensive tests..."
    
    # Check prerequisites
    if ! command -v docker &> /dev/null; then
        print_error "Docker is required for testing"
        exit 1
    fi
    
    if ! command -v curl &> /dev/null; then
        print_error "curl is required for testing"
        exit 1
    fi
    
    # Set up cleanup trap
    trap cleanup_test_resources EXIT INT TERM
    
    # Run test suites
    test_script_validation
    test_ssh_connection
    test_image_transfer
    test_local_deployment
    test_rollback_functionality
    test_error_handling
    
    # Show summary
    if show_test_summary; then
        print_success "SSH transfer and deployment system tests completed successfully"
        exit 0
    else
        print_error "Some tests failed - please review the output above"
        exit 1
    fi
}

# Run main function
main "$@"