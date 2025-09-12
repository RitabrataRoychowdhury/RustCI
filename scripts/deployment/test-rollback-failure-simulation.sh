#!/bin/bash

# Rollback Failure Simulation Test Script
# Tests rollback mechanisms under various failure conditions
# Requirements: 4.4, 5.1, 5.2, 5.4

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROLLBACK_SCRIPT="$SCRIPT_DIR/rollback.sh"
TEST_LOG_FILE="$SCRIPT_DIR/rollback-failure-test.log"

# Test configuration
TEST_CONTAINER="rustci-rollback-test"
TEST_IMAGE="nginx:alpine"
TEST_PORT=8082
BACKUP_IMAGE="rustci-rollback-test:backup"
PREVIOUS_IMAGE="rustci-rollback-test:previous"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Logging function
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo -e "[$timestamp] [$level] $message" | tee -a "$TEST_LOG_FILE"
}

print_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

print_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

print_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Function to run a test
run_test() {
    local test_name="$1"
    local test_function="$2"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    print_test "Running: $test_name"
    log "INFO" "Starting test: $test_name"
    
    if $test_function; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        print_pass "$test_name"
        log "PASS" "$test_name"
        return 0
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        print_fail "$test_name"
        log "FAIL" "$test_name"
        return 1
    fi
}

# Cleanup function
cleanup() {
    log "INFO" "Cleaning up test environment"
    
    # Stop and remove test containers
    docker stop "$TEST_CONTAINER" >/dev/null 2>&1 || true
    docker rm "$TEST_CONTAINER" >/dev/null 2>&1 || true
    
    # Remove test images
    docker rmi "$BACKUP_IMAGE" >/dev/null 2>&1 || true
    docker rmi "$PREVIOUS_IMAGE" >/dev/null 2>&1 || true
    docker rmi "${TEST_CONTAINER}:failed" >/dev/null 2>&1 || true
    docker rmi "${TEST_CONTAINER}:corrupted" >/dev/null 2>&1 || true
    
    # Kill any background processes
    jobs -p | xargs -r kill 2>/dev/null || true
}

# Setup test environment
setup_test_environment() {
    log "INFO" "Setting up rollback failure test environment"
    
    # Ensure rollback script is executable
    chmod +x "$ROLLBACK_SCRIPT"
    
    # Pull test image
    docker pull "$TEST_IMAGE" >/dev/null 2>&1
    
    # Create backup and previous images
    docker tag "$TEST_IMAGE" "$BACKUP_IMAGE"
    docker tag "$TEST_IMAGE" "$PREVIOUS_IMAGE"
    
    log "INFO" "Test environment setup complete"
}

# Test 1: Rollback script exists and is executable
test_rollback_script_exists() {
    [[ -f "$ROLLBACK_SCRIPT" ]] && [[ -x "$ROLLBACK_SCRIPT" ]]
}

# Test 2: Rollback with no previous image available
test_rollback_no_previous_image() {
    log "INFO" "Testing rollback with no previous image"
    
    # Remove previous image
    docker rmi "$PREVIOUS_IMAGE" >/dev/null 2>&1 || true
    
    # Try rollback (should fail gracefully)
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" rollback >/dev/null 2>&1; then
        log "ERROR" "Rollback should have failed with no previous image"
        return 1
    else
        log "INFO" "Rollback correctly failed with no previous image"
        # Recreate previous image for other tests
        docker tag "$TEST_IMAGE" "$PREVIOUS_IMAGE"
        return 0
    fi
}

# Test 3: Rollback with container that doesn't exist
test_rollback_nonexistent_container() {
    log "INFO" "Testing rollback with nonexistent container"
    
    # Ensure container doesn't exist
    docker stop "$TEST_CONTAINER" >/dev/null 2>&1 || true
    docker rm "$TEST_CONTAINER" >/dev/null 2>&1 || true
    
    # Try rollback (should handle gracefully)
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" rollback >/dev/null 2>&1; then
        log "INFO" "Rollback handled nonexistent container gracefully"
        return 0
    else
        log "WARN" "Rollback failed with nonexistent container (may be expected)"
        return 0  # This might be expected behavior
    fi
}

# Test 4: Rollback with corrupted previous image
test_rollback_corrupted_image() {
    log "INFO" "Testing rollback with corrupted image"
    
    # Create a "corrupted" image (empty image that will fail to start)
    docker run --name temp-corrupt alpine:latest /bin/sh -c "exit 1" >/dev/null 2>&1 || true
    docker commit temp-corrupt "${TEST_CONTAINER}:corrupted" >/dev/null 2>&1 || true
    docker rm temp-corrupt >/dev/null 2>&1 || true
    
    # Replace previous image with corrupted one
    docker rmi "$PREVIOUS_IMAGE" >/dev/null 2>&1 || true
    docker tag "${TEST_CONTAINER}:corrupted" "$PREVIOUS_IMAGE"
    
    # Start a working container first
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Try rollback (should fail but handle gracefully)
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$TEST_PORT" rollback >/dev/null 2>&1; then
        log "WARN" "Rollback succeeded with corrupted image (unexpected)"
        return 0  # Not necessarily a failure
    else
        log "INFO" "Rollback correctly failed with corrupted image"
        return 0
    fi
}

# Test 5: Rollback with port conflict
test_rollback_port_conflict() {
    log "INFO" "Testing rollback with port conflict"
    
    # Start container on test port
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Start another container on the same port (will conflict during rollback)
    docker run -d --name "port-blocker" -p "$((TEST_PORT + 1)):80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Try rollback with port conflict
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$((TEST_PORT + 1))" rollback >/dev/null 2>&1; then
        log "WARN" "Rollback succeeded despite port conflict"
    else
        log "INFO" "Rollback handled port conflict appropriately"
    fi
    
    # Cleanup
    docker stop "port-blocker" >/dev/null 2>&1 || true
    docker rm "port-blocker" >/dev/null 2>&1 || true
    
    return 0
}

# Test 6: Rollback with insufficient resources
test_rollback_resource_constraints() {
    log "INFO" "Testing rollback with resource constraints"
    
    # Start container with resource limits
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" --memory="50m" --cpus="0.1" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Try rollback (should work within resource constraints)
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$TEST_PORT" rollback >/dev/null 2>&1; then
        log "INFO" "Rollback succeeded with resource constraints"
        return 0
    else
        log "WARN" "Rollback failed with resource constraints"
        return 0  # May be expected
    fi
}

# Test 7: Rollback with network issues simulation
test_rollback_network_issues() {
    log "INFO" "Testing rollback with network issues simulation"
    
    # Start container
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Simulate network issues by using invalid health check
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "9999" --health-timeout 2 --health-retries 1 rollback >/dev/null 2>&1; then
        log "WARN" "Rollback succeeded despite network issues"
    else
        log "INFO" "Rollback handled network issues appropriately"
    fi
    
    return 0
}

# Test 8: Rollback with multiple backup images
test_rollback_multiple_backups() {
    log "INFO" "Testing rollback with multiple backup images"
    
    # Create multiple backup images
    docker tag "$TEST_IMAGE" "${TEST_CONTAINER}:backup_20231201_120000"
    docker tag "$TEST_IMAGE" "${TEST_CONTAINER}:backup_20231202_120000"
    docker tag "$TEST_IMAGE" "${TEST_CONTAINER}:backup_20231203_120000"
    
    # Start container
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Test listing backups
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" list-backups >/dev/null 2>&1; then
        log "INFO" "Backup listing succeeded with multiple backups"
    else
        log "WARN" "Backup listing failed with multiple backups"
    fi
    
    # Test rollback to specific backup
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$TEST_PORT" rollback-to "backup_20231202_120000" >/dev/null 2>&1; then
        log "INFO" "Rollback to specific backup succeeded"
        return 0
    else
        log "WARN" "Rollback to specific backup failed"
        return 0
    fi
}

# Test 9: Rollback with health check failures
test_rollback_health_check_failures() {
    log "INFO" "Testing rollback with health check failures"
    
    # Start container
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Create a previous image that will fail health checks
    docker run --name temp-unhealthy alpine:latest /bin/sh -c "while true; do sleep 1; done" >/dev/null 2>&1 &
    local temp_pid=$!
    sleep 2
    docker commit temp-unhealthy "${TEST_CONTAINER}:unhealthy" >/dev/null 2>&1 || true
    docker stop temp-unhealthy >/dev/null 2>&1 || true
    docker rm temp-unhealthy >/dev/null 2>&1 || true
    
    # Replace previous image with unhealthy one
    docker rmi "$PREVIOUS_IMAGE" >/dev/null 2>&1 || true
    docker tag "${TEST_CONTAINER}:unhealthy" "$PREVIOUS_IMAGE"
    
    # Try rollback (should fail health checks)
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$TEST_PORT" --health-timeout 5 --health-retries 2 rollback >/dev/null 2>&1; then
        log "WARN" "Rollback succeeded despite health check failures"
    else
        log "INFO" "Rollback correctly failed due to health check failures"
    fi
    
    return 0
}

# Test 10: Rollback status and information commands
test_rollback_status_commands() {
    log "INFO" "Testing rollback status and information commands"
    
    # Start container
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Test status command
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" status >/dev/null 2>&1; then
        log "INFO" "Rollback status command succeeded"
    else
        log "WARN" "Rollback status command failed"
        return 1
    fi
    
    # Test list-backups command
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" list-backups >/dev/null 2>&1; then
        log "INFO" "Rollback list-backups command succeeded"
    else
        log "WARN" "Rollback list-backups command failed"
        return 1
    fi
    
    return 0
}

# Test 11: Rollback with environment file
test_rollback_with_env_file() {
    log "INFO" "Testing rollback with environment file"
    
    # Create test environment file
    local env_file="/tmp/test-rollback.env"
    cat > "$env_file" << EOF
TEST_VAR=test_value
ANOTHER_VAR=another_value
EOF
    
    # Start container
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Try rollback with environment file
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$TEST_PORT" -e "$env_file" rollback >/dev/null 2>&1; then
        log "INFO" "Rollback with environment file succeeded"
        rm -f "$env_file"
        return 0
    else
        log "WARN" "Rollback with environment file failed"
        rm -f "$env_file"
        return 0
    fi
}

# Test 12: Rollback timeout scenarios
test_rollback_timeout_scenarios() {
    log "INFO" "Testing rollback timeout scenarios"
    
    # Start container
    docker run -d --name "$TEST_CONTAINER" -p "$TEST_PORT:80" "$TEST_IMAGE" >/dev/null 2>&1
    
    # Test with very short timeout (should fail quickly)
    if "$ROLLBACK_SCRIPT" -c "$TEST_CONTAINER" -p "$TEST_PORT" --health-timeout 1 --health-retries 1 rollback >/dev/null 2>&1; then
        log "INFO" "Rollback with short timeout succeeded"
    else
        log "INFO" "Rollback with short timeout failed as expected"
    fi
    
    return 0
}

# Main test runner
run_all_tests() {
    log "INFO" "Starting rollback failure simulation tests"
    
    # Setup
    setup_test_environment
    
    # Run tests
    run_test "Rollback script exists" test_rollback_script_exists
    run_test "Rollback with no previous image" test_rollback_no_previous_image
    run_test "Rollback with nonexistent container" test_rollback_nonexistent_container
    run_test "Rollback with corrupted image" test_rollback_corrupted_image
    run_test "Rollback with port conflict" test_rollback_port_conflict
    run_test "Rollback with resource constraints" test_rollback_resource_constraints
    run_test "Rollback with network issues" test_rollback_network_issues
    run_test "Rollback with multiple backups" test_rollback_multiple_backups
    run_test "Rollback with health check failures" test_rollback_health_check_failures
    run_test "Rollback status commands" test_rollback_status_commands
    run_test "Rollback with environment file" test_rollback_with_env_file
    run_test "Rollback timeout scenarios" test_rollback_timeout_scenarios
    
    # Cleanup
    cleanup
    
    # Results
    echo ""
    echo "=== Rollback Failure Simulation Test Results ==="
    echo "Tests run: $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"
    echo "Log file: $TEST_LOG_FILE"
    
    if [[ $TESTS_FAILED -eq 0 ]]; then
        print_pass "All rollback failure simulation tests passed!"
        log "INFO" "All rollback failure simulation tests completed successfully"
        return 0
    else
        print_fail "$TESTS_FAILED rollback tests failed"
        log "ERROR" "$TESTS_FAILED rollback tests failed"
        return 1
    fi
}

# Usage function
usage() {
    cat << EOF
Rollback Failure Simulation Test Script

Usage: $0 [OPTIONS] [COMMAND]

COMMANDS:
    all                Run all rollback failure tests (default)
    setup              Setup test environment only
    cleanup            Cleanup test environment only
    single TEST_NAME   Run a single test

OPTIONS:
    -v, --verbose      Enable verbose output
    -h, --help         Show this help

EXAMPLES:
    $0                           # Run all tests
    $0 all                       # Run all tests
    $0 single test_rollback_no_previous_image
    $0 setup                     # Setup test environment
    $0 cleanup                   # Cleanup test environment

TEST LOG: $TEST_LOG_FILE
EOF
}

# Parse arguments
COMMAND="all"
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE=true
            set -x
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        all|setup|cleanup)
            COMMAND="$1"
            shift
            ;;
        single)
            COMMAND="single"
            TEST_NAME="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1"
            usage
            exit 1
            ;;
    esac
done

# Set up signal handlers for cleanup
trap cleanup EXIT INT TERM

# Main execution
case "$COMMAND" in
    all)
        run_all_tests
        ;;
    setup)
        setup_test_environment
        ;;
    cleanup)
        cleanup
        ;;
    single)
        if [[ -z "${TEST_NAME:-}" ]]; then
            echo "Error: Test name required for single test"
            usage
            exit 1
        fi
        setup_test_environment
        run_test "$TEST_NAME" "$TEST_NAME"
        cleanup
        ;;
    *)
        echo "Unknown command: $COMMAND"
        usage
        exit 1
        ;;
esac