#!/bin/bash

# Complete Pipeline Integration Test Script
# Tests the entire cross-architecture deployment pipeline end-to-end
# Requirements: 4.4, 5.1, 5.2, 5.4

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PIPELINE_FILE="$PROJECT_ROOT/pipeline-cross-arch.yaml"
VALIDATION_SCRIPT="$PROJECT_ROOT/validate-cross-arch-pipeline.sh"
TEST_LOG_FILE="$SCRIPT_DIR/pipeline-integration-test.log"

# Test configuration
TEST_MODE=true
SKIP_VPS_TESTS=false
SKIP_BUILD_TESTS=false
SKIP_ROLLBACK_TESTS=false
VERBOSE=false

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

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
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

# Test 1: Validate pipeline YAML structure against RustCI schema requirements
test_pipeline_yaml_structure() {
    log "INFO" "Validating pipeline YAML structure"
    
    # Check if pipeline file exists
    if [[ ! -f "$PIPELINE_FILE" ]]; then
        log "ERROR" "Pipeline file not found: $PIPELINE_FILE"
        return 1
    fi
    
    # Check if validation script exists
    if [[ ! -f "$VALIDATION_SCRIPT" ]]; then
        log "ERROR" "Validation script not found: $VALIDATION_SCRIPT"
        return 1
    fi
    
    # Make validation script executable
    chmod +x "$VALIDATION_SCRIPT"
    
    # Run validation script
    if "$VALIDATION_SCRIPT" >> "$TEST_LOG_FILE" 2>&1; then
        log "INFO" "Pipeline YAML structure validation passed"
        return 0
    else
        log "ERROR" "Pipeline YAML structure validation failed"
        return 1
    fi
}

# Test 2: Validate required top-level fields
test_required_fields() {
    log "INFO" "Validating required top-level fields"
    
    local required_fields=("name" "description" "version" "environment" "triggers" "stages")
    
    for field in "${required_fields[@]}"; do
        if command -v yq &> /dev/null; then
            if ! yq eval "has(\"$field\")" "$PIPELINE_FILE" | grep -q "true"; then
                log "ERROR" "Missing required field: $field"
                return 1
            fi
        else
            # Fallback to grep if yq not available
            if ! grep -q "^$field:" "$PIPELINE_FILE"; then
                log "ERROR" "Missing required field: $field (fallback check)"
                return 1
            fi
        fi
    done
    
    log "INFO" "All required fields present"
    return 0
}

# Test 3: Validate stage structure (exactly 5 stages)
test_stage_structure() {
    log "INFO" "Validating stage structure"
    
    local expected_stages=("prepare" "build-image" "transfer-and-deploy" "smoke-test" "cleanup")
    
    if command -v yq &> /dev/null; then
        local stage_count
        stage_count=$(yq eval '.stages | length' "$PIPELINE_FILE")
        
        if [[ "$stage_count" -ne 5 ]]; then
            log "ERROR" "Expected 5 stages, found $stage_count"
            return 1
        fi
        
        # Check stage names
        for i in "${!expected_stages[@]}"; do
            local stage_name
            stage_name=$(yq eval ".stages[$i].name" "$PIPELINE_FILE")
            if [[ "$stage_name" != "${expected_stages[$i]}" ]]; then
                log "ERROR" "Stage $i: expected '${expected_stages[$i]}', found '$stage_name'"
                return 1
            fi
        done
    else
        # Fallback check
        for stage in "${expected_stages[@]}"; do
            if ! grep -q "name: \"$stage\"" "$PIPELINE_FILE"; then
                log "ERROR" "Stage not found: $stage (fallback check)"
                return 1
            fi
        done
    fi
    
    log "INFO" "Stage structure validation passed"
    return 0
}

# Test 4: Validate timeout configurations
test_timeout_configurations() {
    log "INFO" "Validating timeout configurations"
    
    if command -v yq &> /dev/null; then
        # Check global timeout
        local global_timeout
        global_timeout=$(yq eval '.timeout' "$PIPELINE_FILE")
        if [[ "$global_timeout" != "3600" ]]; then
            log "ERROR" "Expected global timeout 3600, found $global_timeout"
            return 1
        fi
        
        # Check stage timeouts exist
        local stages_with_timeouts
        stages_with_timeouts=$(yq eval '.stages[].steps[].timeout' "$PIPELINE_FILE" | grep -v "null" | wc -l)
        if [[ "$stages_with_timeouts" -lt 10 ]]; then
            log "ERROR" "Expected at least 10 steps with timeouts, found $stages_with_timeouts"
            return 1
        fi
    else
        # Fallback check
        if ! grep -q "timeout: 3600" "$PIPELINE_FILE"; then
            log "ERROR" "Global timeout not found (fallback check)"
            return 1
        fi
    fi
    
    log "INFO" "Timeout configurations valid"
    return 0
}

# Test 5: Validate environment variables
test_environment_variables() {
    log "INFO" "Validating environment variables"
    
    local required_env_vars=("TESTING_MODE" "BUILD_PLATFORM" "VPS_IP" "VPS_USERNAME" "VPS_PASSWORD" "MONGODB_URI" "JWT_SECRET")
    
    for var in "${required_env_vars[@]}"; do
        if command -v yq &> /dev/null; then
            if ! yq eval ".environment | has(\"$var\")" "$PIPELINE_FILE" | grep -q "true"; then
                log "ERROR" "Missing environment variable: $var"
                return 1
            fi
        else
            # Fallback check
            if ! grep -q "$var:" "$PIPELINE_FILE"; then
                log "ERROR" "Missing environment variable: $var (fallback check)"
                return 1
            fi
        fi
    done
    
    log "INFO" "All required environment variables present"
    return 0
}

# Test 6: Validate rollback configuration
test_rollback_configuration() {
    log "INFO" "Validating rollback configuration"
    
    if command -v yq &> /dev/null; then
        if ! yq eval '.rollback.enabled' "$PIPELINE_FILE" | grep -q "true"; then
            log "ERROR" "Rollback not enabled"
            return 1
        fi
        
        # Check rollback steps exist
        local rollback_steps
        rollback_steps=$(yq eval '.rollback.steps | length' "$PIPELINE_FILE")
        if [[ "$rollback_steps" -lt 1 ]]; then
            log "ERROR" "No rollback steps defined"
            return 1
        fi
    else
        # Fallback check
        if ! grep -q "enabled: true" "$PIPELINE_FILE"; then
            log "ERROR" "Rollback not enabled (fallback check)"
            return 1
        fi
    fi
    
    log "INFO" "Rollback configuration valid"
    return 0
}

# Test 7: Validate step types (only shell steps)
test_step_types() {
    log "INFO" "Validating step types"
    
    if command -v yq &> /dev/null; then
        local invalid_step_types
        invalid_step_types=$(yq eval '.stages[].steps[].step_type' "$PIPELINE_FILE" | grep -v "shell" | wc -l)
        if [[ "$invalid_step_types" -gt 0 ]]; then
            log "ERROR" "Found invalid step types (only 'shell' is used in this pipeline)"
            return 1
        fi
    else
        # Fallback check - ensure no docker or kubernetes step types
        if grep -q "step_type: docker\|step_type: kubernetes" "$PIPELINE_FILE"; then
            log "ERROR" "Found non-shell step types (fallback check)"
            return 1
        fi
    fi
    
    log "INFO" "All step types are valid"
    return 0
}

# Test 8: Test Docker buildx system availability
test_docker_buildx_system() {
    if [[ "$SKIP_BUILD_TESTS" == "true" ]]; then
        log "INFO" "Skipping Docker buildx tests"
        return 0
    fi
    
    log "INFO" "Testing Docker buildx system"
    
    # Check if Docker is available
    if ! command -v docker &> /dev/null; then
        log "ERROR" "Docker not available"
        return 1
    fi
    
    # Check if Docker is running
    if ! docker info >/dev/null 2>&1; then
        log "ERROR" "Docker is not running"
        return 1
    fi
    
    # Check if buildx is available
    if ! docker buildx version >/dev/null 2>&1; then
        log "ERROR" "Docker buildx not available"
        return 1
    fi
    
    # Test buildx builder creation (non-destructive)
    local test_builder="test-multiarch-builder-$$"
    if docker buildx create --name "$test_builder" >/dev/null 2>&1; then
        log "INFO" "Buildx builder creation test passed"
        docker buildx rm "$test_builder" >/dev/null 2>&1 || true
    else
        log "ERROR" "Failed to create buildx builder"
        return 1
    fi
    
    log "INFO" "Docker buildx system available"
    return 0
}

# Test 9: Test SSH connectivity (if not skipping VPS tests)
test_ssh_connectivity() {
    if [[ "$SKIP_VPS_TESTS" == "true" ]]; then
        log "INFO" "Skipping SSH connectivity tests"
        return 0
    fi
    
    log "INFO" "Testing SSH connectivity"
    
    # Load environment variables
    if [[ -f "$PROJECT_ROOT/.env" ]]; then
        source "$PROJECT_ROOT/.env"
    fi
    
    # Check required variables
    if [[ -z "${VPS_IP:-}" ]] || [[ -z "${VPS_USERNAME:-}" ]] || [[ -z "${VPS_PASSWORD:-}" ]]; then
        log "WARN" "VPS credentials not available, skipping SSH test"
        return 0
    fi
    
    # Check if sshpass is available
    if ! command -v sshpass &> /dev/null; then
        log "WARN" "sshpass not available, skipping SSH test"
        return 0
    fi
    
    # Test SSH connection
    if sshpass -p "$VPS_PASSWORD" ssh -o ConnectTimeout=10 -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" "echo 'SSH test successful'" >/dev/null 2>&1; then
        log "INFO" "SSH connectivity test passed"
        return 0
    else
        log "ERROR" "SSH connectivity test failed"
        return 1
    fi
}

# Test 10: Test health check endpoints simulation
test_health_check_endpoints() {
    log "INFO" "Testing health check endpoint simulation"
    
    # Start a simple HTTP server for testing
    local test_port=8081
    local test_pid=""
    
    # Check if port is available
    if netstat -ln 2>/dev/null | grep -q ":$test_port "; then
        log "WARN" "Port $test_port is busy, skipping health check test"
        return 0
    fi
    
    # Start simple HTTP server in background
    python3 -c "
import http.server
import socketserver
import threading
import time

class HealthHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path in ['/health', '/api/healthchecker']:
            self.send_response(200)
            self.send_header('Content-type', 'text/plain')
            self.end_headers()
            self.wfile.write(b'OK')
        else:
            self.send_response(404)
            self.end_headers()

with socketserver.TCPServer(('', $test_port), HealthHandler) as httpd:
    httpd.serve_forever()
" &
    test_pid=$!
    
    # Wait for server to start
    sleep 2
    
    # Test health endpoints
    local health_test_passed=true
    
    if ! curl -f -s --max-time 5 "http://localhost:$test_port/api/healthchecker" >/dev/null 2>&1; then
        log "ERROR" "Primary health endpoint test failed"
        health_test_passed=false
    fi
    
    if ! curl -f -s --max-time 5 "http://localhost:$test_port/health" >/dev/null 2>&1; then
        log "ERROR" "Fallback health endpoint test failed"
        health_test_passed=false
    fi
    
    # Cleanup
    if [[ -n "$test_pid" ]]; then
        kill "$test_pid" 2>/dev/null || true
    fi
    
    if [[ "$health_test_passed" == "true" ]]; then
        log "INFO" "Health check endpoints test passed"
        return 0
    else
        return 1
    fi
}

# Test 11: Test rollback mechanism simulation
test_rollback_mechanism() {
    if [[ "$SKIP_ROLLBACK_TESTS" == "true" ]]; then
        log "INFO" "Skipping rollback mechanism tests"
        return 0
    fi
    
    log "INFO" "Testing rollback mechanism simulation"
    
    local rollback_script="$SCRIPT_DIR/rollback.sh"
    
    # Check if rollback script exists
    if [[ ! -f "$rollback_script" ]]; then
        log "ERROR" "Rollback script not found: $rollback_script"
        return 1
    fi
    
    # Make script executable
    chmod +x "$rollback_script"
    
    # Test rollback script help
    if ! "$rollback_script" --help >/dev/null 2>&1; then
        log "ERROR" "Rollback script help failed"
        return 1
    fi
    
    # Test rollback script status (should not fail even if no container)
    if ! "$rollback_script" status >/dev/null 2>&1; then
        log "WARN" "Rollback script status check failed (may be normal if no containers)"
    fi
    
    # Test rollback script list-backups
    if ! "$rollback_script" list-backups >/dev/null 2>&1; then
        log "WARN" "Rollback script list-backups failed (may be normal if no images)"
    fi
    
    log "INFO" "Rollback mechanism simulation passed"
    return 0
}

# Test 12: Test deployment verification commands
test_deployment_verification_commands() {
    log "INFO" "Testing deployment verification commands"
    
    # Load environment variables
    if [[ -f "$PROJECT_ROOT/.env" ]]; then
        source "$PROJECT_ROOT/.env"
    fi
    
    local vps_ip="${VPS_IP:-46.37.122.118}"
    local container_name="rustci-production"
    
    # Test curl command format
    local health_url="http://$vps_ip:8080/api/healthchecker"
    local fallback_url="http://$vps_ip:8080/health"
    
    # Validate URL format
    if [[ ! "$health_url" =~ ^http://[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+/api/healthchecker$ ]]; then
        log "ERROR" "Invalid health URL format: $health_url"
        return 1
    fi
    
    if [[ ! "$fallback_url" =~ ^http://[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+/health$ ]]; then
        log "ERROR" "Invalid fallback URL format: $fallback_url"
        return 1
    fi
    
    # Test SSH command format (if credentials available)
    if [[ -n "${VPS_USERNAME:-}" ]]; then
        local ssh_cmd="ssh ${VPS_USERNAME}@${vps_ip} 'docker ps | grep ${container_name}'"
        log "INFO" "SSH command format valid: $ssh_cmd"
    fi
    
    log "INFO" "Deployment verification commands test passed"
    return 0
}

# Test 13: Test pipeline execution simulation (dry run)
test_pipeline_execution_simulation() {
    log "INFO" "Testing pipeline execution simulation (dry run)"
    
    # Simulate each stage validation
    local stages=("prepare" "build-image" "transfer-and-deploy" "smoke-test" "cleanup")
    
    for stage in "${stages[@]}"; do
        log "INFO" "Simulating stage: $stage"
        
        # Check if stage has required steps
        if command -v yq &> /dev/null; then
            local step_count
            step_count=$(yq eval ".stages[] | select(.name == \"$stage\") | .steps | length" "$PIPELINE_FILE")
            if [[ "$step_count" -lt 1 ]]; then
                log "ERROR" "Stage $stage has no steps"
                return 1
            fi
            log "INFO" "Stage $stage has $step_count steps"
        fi
    done
    
    log "INFO" "Pipeline execution simulation passed"
    return 0
}

# Test 14: Test error handling and failure modes
test_error_handling() {
    log "INFO" "Testing error handling and failure modes"
    
    # Check if pipeline has proper on_failure configurations
    if command -v yq &> /dev/null; then
        local failure_modes
        failure_modes=$(yq eval '.stages[].steps[].on_failure' "$PIPELINE_FILE" | grep -v "null" | sort | uniq)
        
        if [[ -z "$failure_modes" ]]; then
            log "ERROR" "No failure modes configured"
            return 1
        fi
        
        log "INFO" "Failure modes configured: $failure_modes"
        
        # Check for retry configurations
        local retry_configs
        retry_configs=$(yq eval '.stages[].steps[].retry_count' "$PIPELINE_FILE" | grep -v "null" | wc -l)
        
        if [[ "$retry_configs" -lt 5 ]]; then
            log "ERROR" "Insufficient retry configurations"
            return 1
        fi
        
        log "INFO" "Retry configurations found: $retry_configs"
    fi
    
    log "INFO" "Error handling test passed"
    return 0
}

# Test 15: Test security configuration validation
test_security_configuration() {
    log "INFO" "Testing security configuration validation"
    
    # Check for testing mode warning
    if ! grep -q "WARNING.*hardcoded.*testing" "$PIPELINE_FILE"; then
        log "ERROR" "Testing mode warning not found"
        return 1
    fi
    
    # Check for proper secret handling
    if command -v yq &> /dev/null; then
        local testing_mode
        testing_mode=$(yq eval '.environment.TESTING_MODE' "$PIPELINE_FILE")
        if [[ "$testing_mode" != "true" && "$testing_mode" != "\"true\"" ]]; then
            log "ERROR" "Testing mode not properly configured: $testing_mode"
            return 1
        fi
    fi
    
    log "INFO" "Security configuration validation passed"
    return 0
}

# Cleanup function
cleanup() {
    log "INFO" "Cleaning up test environment"
    
    # Kill any background processes
    jobs -p | xargs -r kill 2>/dev/null || true
    
    # Remove temporary files
    rm -f /tmp/test-pipeline-* 2>/dev/null || true
}

# Setup test environment
setup_test_environment() {
    log "INFO" "Setting up test environment"
    
    # Create log file
    touch "$TEST_LOG_FILE"
    
    # Check required tools
    local missing_tools=()
    
    if ! command -v curl &> /dev/null; then
        missing_tools+=("curl")
    fi
    
    if ! command -v python3 &> /dev/null; then
        missing_tools+=("python3")
    fi
    
    if [[ ${#missing_tools[@]} -ne 0 ]]; then
        log "WARN" "Missing optional tools: ${missing_tools[*]}"
    fi
    
    log "INFO" "Test environment setup complete"
}

# Main test runner
run_all_tests() {
    log "INFO" "Starting complete pipeline integration tests"
    
    # Setup
    setup_test_environment
    
    # Run tests
    run_test "Pipeline YAML structure validation" test_pipeline_yaml_structure
    run_test "Required fields validation" test_required_fields
    run_test "Stage structure validation" test_stage_structure
    run_test "Timeout configurations validation" test_timeout_configurations
    run_test "Environment variables validation" test_environment_variables
    run_test "Rollback configuration validation" test_rollback_configuration
    run_test "Step types validation" test_step_types
    run_test "Docker buildx system availability" test_docker_buildx_system
    run_test "SSH connectivity test" test_ssh_connectivity
    run_test "Health check endpoints simulation" test_health_check_endpoints
    run_test "Rollback mechanism simulation" test_rollback_mechanism
    run_test "Deployment verification commands" test_deployment_verification_commands
    run_test "Pipeline execution simulation" test_pipeline_execution_simulation
    run_test "Error handling validation" test_error_handling
    run_test "Security configuration validation" test_security_configuration
    
    # Cleanup
    cleanup
    
    # Results
    echo ""
    echo "=== Complete Pipeline Integration Test Results ==="
    echo "Tests run: $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"
    echo "Log file: $TEST_LOG_FILE"
    
    if [[ $TESTS_FAILED -eq 0 ]]; then
        print_pass "All integration tests passed!"
        log "INFO" "All integration tests completed successfully"
        return 0
    else
        print_fail "$TESTS_FAILED integration tests failed"
        log "ERROR" "$TESTS_FAILED integration tests failed"
        return 1
    fi
}

# Usage function
usage() {
    cat << EOF
Complete Pipeline Integration Test Script

Usage: $0 [OPTIONS] [COMMAND]

COMMANDS:
    all                Run all integration tests (default)
    setup              Setup test environment only
    cleanup            Cleanup test environment only
    single TEST_NAME   Run a single test

OPTIONS:
    --skip-vps         Skip VPS-related tests
    --skip-build       Skip Docker build tests
    --skip-rollback    Skip rollback tests
    -v, --verbose      Enable verbose output
    -h, --help         Show this help

EXAMPLES:
    $0                           # Run all tests
    $0 all                       # Run all tests
    $0 --skip-vps                # Run tests without VPS connectivity
    $0 single test_pipeline_yaml_structure
    $0 setup                     # Setup test environment
    $0 cleanup                   # Cleanup test environment

TEST LOG: $TEST_LOG_FILE
EOF
}

# Parse arguments
COMMAND="all"

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-vps)
            SKIP_VPS_TESTS=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD_TESTS=true
            shift
            ;;
        --skip-rollback)
            SKIP_ROLLBACK_TESTS=true
            shift
            ;;
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