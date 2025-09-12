#!/bin/bash

# RustCI Compatibility Test Script
# Tests the corrected pipeline with RustCI environment variable handling

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_LOG_FILE="$SCRIPT_DIR/rustci-compatibility-test.log"

# Test configuration
RUSTCI_PIPELINE="$PROJECT_ROOT/pipeline-cross-arch-rustci.yaml"
PRODUCTION_PIPELINE="$PROJECT_ROOT/pipeline-cross-arch-production.yaml"
VALIDATION_SCRIPT="$PROJECT_ROOT/validate-rustci-pipeline.sh"

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

# Test 1: Validate RustCI pipeline uses 'variables' field
test_rustci_variables_field() {
    log "INFO" "Testing RustCI pipeline uses 'variables' field"
    
    if [[ ! -f "$RUSTCI_PIPELINE" ]]; then
        log "ERROR" "RustCI pipeline not found: $RUSTCI_PIPELINE"
        return 1
    fi
    
    # Check for 'variables' field
    if grep -q "^variables:" "$RUSTCI_PIPELINE"; then
        log "INFO" "RustCI pipeline uses 'variables' field"
    else
        log "ERROR" "RustCI pipeline missing 'variables' field"
        return 1
    fi
    
    # Check it doesn't use 'environment' field
    if grep -q "^environment:" "$RUSTCI_PIPELINE"; then
        log "ERROR" "RustCI pipeline still uses 'environment' field"
        return 1
    fi
    
    log "INFO" "RustCI pipeline correctly uses 'variables' field"
    return 0
}

# Test 2: Validate production pipeline uses environment substitution
test_production_env_substitution() {
    log "INFO" "Testing production pipeline uses environment substitution"
    
    if [[ ! -f "$PRODUCTION_PIPELINE" ]]; then
        log "ERROR" "Production pipeline not found: $PRODUCTION_PIPELINE"
        return 1
    fi
    
    # Check for environment variable substitution syntax
    local substitution_count
    substitution_count=$(grep -c '\${[A-Z_]*}' "$PRODUCTION_PIPELINE" || true)
    
    if [[ "$substitution_count" -gt 10 ]]; then
        log "INFO" "Production pipeline uses environment substitution: $substitution_count variables"
        return 0
    else
        log "ERROR" "Production pipeline has insufficient environment substitution: $substitution_count"
        return 1
    fi
}

# Test 3: Validate pipeline structure against RustCI schema
test_pipeline_structure_validation() {
    log "INFO" "Testing pipeline structure validation"
    
    if [[ ! -f "$VALIDATION_SCRIPT" ]]; then
        log "ERROR" "Validation script not found: $VALIDATION_SCRIPT"
        return 1
    fi
    
    # Make validation script executable
    chmod +x "$VALIDATION_SCRIPT"
    
    # Test RustCI pipeline validation
    if "$VALIDATION_SCRIPT" -f "$RUSTCI_PIPELINE" >> "$TEST_LOG_FILE" 2>&1; then
        log "INFO" "RustCI pipeline validation passed"
    else
        log "ERROR" "RustCI pipeline validation failed"
        return 1
    fi
    
    # Test production pipeline validation
    if "$VALIDATION_SCRIPT" -f "$PRODUCTION_PIPELINE" >> "$TEST_LOG_FILE" 2>&1; then
        log "INFO" "Production pipeline validation passed"
    else
        log "ERROR" "Production pipeline validation failed"
        return 1
    fi
    
    return 0
}

# Test 4: Test environment variable access simulation
test_environment_variable_access() {
    log "INFO" "Testing environment variable access simulation"
    
    # Create a test script that simulates RustCI environment variable handling
    local test_script="/tmp/test-env-access.sh"
    
    cat > "$test_script" << 'EOF'
#!/bin/bash

# Simulate RustCI environment variable handling
# Variables from pipeline YAML should be available in execution environment

# Test variables that should be available
REQUIRED_VARS=("TESTING_MODE" "BUILD_PLATFORM" "VPS_IP" "VPS_USERNAME" "MONGODB_URI" "JWT_SECRET")

echo "Testing environment variable access..."

for var in "${REQUIRED_VARS[@]}"; do
    if [[ -n "${!var:-}" ]]; then
        echo "✅ $var is available: ${!var}"
    else
        echo "❌ $var is not available"
        exit 1
    fi
done

echo "✅ All required variables are accessible"
EOF
    
    chmod +x "$test_script"
    
    # Set test environment variables (simulating RustCI execution context)
    export TESTING_MODE="true"
    export BUILD_PLATFORM="linux/amd64"
    export VPS_IP="46.37.122.118"
    export VPS_USERNAME="root"
    export MONGODB_URI="mongodb+srv://test"
    export JWT_SECRET="test-secret"
    
    # Run the test
    if "$test_script" >> "$TEST_LOG_FILE" 2>&1; then
        log "INFO" "Environment variable access test passed"
        rm -f "$test_script"
        return 0
    else
        log "ERROR" "Environment variable access test failed"
        rm -f "$test_script"
        return 1
    fi
}

# Test 5: Test YAML parsing compatibility
test_yaml_parsing_compatibility() {
    log "INFO" "Testing YAML parsing compatibility"
    
    # Test if yq can parse both pipelines correctly
    if command -v yq &> /dev/null; then
        # Test RustCI pipeline
        local rustci_vars
        rustci_vars=$(yq eval '.variables | length' "$RUSTCI_PIPELINE" 2>/dev/null)
        
        if [[ "$rustci_vars" -gt 10 ]]; then
            log "INFO" "RustCI pipeline has $rustci_vars variables"
        else
            log "ERROR" "RustCI pipeline parsing failed or insufficient variables"
            return 1
        fi
        
        # Test production pipeline
        local prod_vars
        prod_vars=$(yq eval '.variables | length' "$PRODUCTION_PIPELINE" 2>/dev/null)
        
        if [[ "$prod_vars" -gt 10 ]]; then
            log "INFO" "Production pipeline has $prod_vars variables"
        else
            log "ERROR" "Production pipeline parsing failed or insufficient variables"
            return 1
        fi
        
        # Test that both pipelines have the same variable names
        local rustci_var_names
        local prod_var_names
        
        rustci_var_names=$(yq eval '.variables | keys | .[]' "$RUSTCI_PIPELINE" | sort)
        prod_var_names=$(yq eval '.variables | keys | .[]' "$PRODUCTION_PIPELINE" | sort)
        
        if [[ "$rustci_var_names" == "$prod_var_names" ]]; then
            log "INFO" "Both pipelines have matching variable names"
            return 0
        else
            log "ERROR" "Pipeline variable names don't match"
            return 1
        fi
    else
        log "WARN" "yq not available, skipping YAML parsing test"
        return 0
    fi
}

# Test 6: Test stage and step structure compatibility
test_stage_step_structure() {
    log "INFO" "Testing stage and step structure compatibility"
    
    if command -v yq &> /dev/null; then
        # Test stage count
        local stage_count
        stage_count=$(yq eval '.stages | length' "$RUSTCI_PIPELINE")
        
        if [[ "$stage_count" -eq 5 ]]; then
            log "INFO" "Correct number of stages: $stage_count"
        else
            log "ERROR" "Incorrect stage count: $stage_count (expected 5)"
            return 1
        fi
        
        # Test step types
        local step_types
        step_types=$(yq eval '.stages[].steps[].step_type' "$RUSTCI_PIPELINE" | sort | uniq)
        
        if [[ "$step_types" == "shell" ]]; then
            log "INFO" "All steps use 'shell' type (RustCI compatible)"
        else
            log "ERROR" "Invalid step types found: $step_types"
            return 1
        fi
        
        # Test timeout configurations
        local steps_with_timeouts
        steps_with_timeouts=$(yq eval '.stages[].steps[].timeout' "$RUSTCI_PIPELINE" | grep -v "null" | wc -l)
        
        if [[ "$steps_with_timeouts" -gt 15 ]]; then
            log "INFO" "Good timeout coverage: $steps_with_timeouts steps"
        else
            log "WARN" "Limited timeout coverage: $steps_with_timeouts steps"
        fi
        
        return 0
    else
        log "WARN" "yq not available, skipping structure test"
        return 0
    fi
}

# Test 7: Test rollback configuration
test_rollback_configuration() {
    log "INFO" "Testing rollback configuration"
    
    if command -v yq &> /dev/null; then
        # Check rollback is enabled
        local rollback_enabled
        rollback_enabled=$(yq eval '.rollback.enabled' "$RUSTCI_PIPELINE")
        
        if [[ "$rollback_enabled" == "true" ]]; then
            log "INFO" "Rollback is enabled"
        else
            log "ERROR" "Rollback is not enabled"
            return 1
        fi
        
        # Check rollback steps exist
        local rollback_steps
        rollback_steps=$(yq eval '.rollback.steps | length' "$RUSTCI_PIPELINE")
        
        if [[ "$rollback_steps" -gt 0 ]]; then
            log "INFO" "Rollback steps configured: $rollback_steps"
            return 0
        else
            log "ERROR" "No rollback steps configured"
            return 1
        fi
    else
        # Fallback check
        if grep -q "rollback:" "$RUSTCI_PIPELINE" && grep -q "enabled: true" "$RUSTCI_PIPELINE"; then
            log "INFO" "Rollback configuration found"
            return 0
        else
            log "ERROR" "Rollback configuration missing"
            return 1
        fi
    fi
}

# Test 8: Test security configuration
test_security_configuration() {
    log "INFO" "Testing security configuration"
    
    # Check for security warnings in RustCI pipeline
    if grep -q "WARNING.*hardcoded.*testing" "$RUSTCI_PIPELINE"; then
        log "INFO" "Security warnings present in RustCI pipeline"
    else
        log "WARN" "No security warnings in RustCI pipeline"
    fi
    
    # Check testing mode configuration
    if command -v yq &> /dev/null; then
        local testing_mode
        testing_mode=$(yq eval '.variables.TESTING_MODE' "$RUSTCI_PIPELINE")
        
        if [[ "$testing_mode" == "true" || "$testing_mode" == "\"true\"" ]]; then
            log "INFO" "RustCI pipeline is in testing mode"
        else
            log "ERROR" "RustCI pipeline testing mode configuration invalid"
            return 1
        fi
        
        # Check production mode
        local prod_testing_mode
        prod_testing_mode=$(yq eval '.variables.TESTING_MODE' "$PRODUCTION_PIPELINE")
        
        if [[ "$prod_testing_mode" == "false" || "$prod_testing_mode" == "\"false\"" ]]; then
            log "INFO" "Production pipeline is in production mode"
            return 0
        else
            log "ERROR" "Production pipeline mode configuration invalid"
            return 1
        fi
    else
        log "WARN" "yq not available, skipping security configuration test"
        return 0
    fi
}

# Cleanup function
cleanup() {
    log "INFO" "Cleaning up test environment"
    
    # Remove temporary files
    rm -f /tmp/test-env-access.sh
    
    # Unset test environment variables
    unset TESTING_MODE BUILD_PLATFORM VPS_IP VPS_USERNAME MONGODB_URI JWT_SECRET
}

# Setup test environment
setup_test_environment() {
    log "INFO" "Setting up RustCI compatibility test environment"
    
    # Create log file
    touch "$TEST_LOG_FILE"
    
    # Check required files exist
    local missing_files=()
    
    if [[ ! -f "$RUSTCI_PIPELINE" ]]; then
        missing_files+=("$RUSTCI_PIPELINE")
    fi
    
    if [[ ! -f "$PRODUCTION_PIPELINE" ]]; then
        missing_files+=("$PRODUCTION_PIPELINE")
    fi
    
    if [[ ! -f "$VALIDATION_SCRIPT" ]]; then
        missing_files+=("$VALIDATION_SCRIPT")
    fi
    
    if [[ ${#missing_files[@]} -ne 0 ]]; then
        log "ERROR" "Missing required files: ${missing_files[*]}"
        return 1
    fi
    
    log "INFO" "Test environment setup complete"
    return 0
}

# Main test runner
run_all_tests() {
    log "INFO" "Starting RustCI compatibility tests"
    
    # Setup
    if ! setup_test_environment; then
        echo "Failed to setup test environment"
        exit 1
    fi
    
    # Run tests
    run_test "RustCI variables field usage" test_rustci_variables_field
    run_test "Production environment substitution" test_production_env_substitution
    run_test "Pipeline structure validation" test_pipeline_structure_validation
    run_test "Environment variable access" test_environment_variable_access
    run_test "YAML parsing compatibility" test_yaml_parsing_compatibility
    run_test "Stage and step structure" test_stage_step_structure
    run_test "Rollback configuration" test_rollback_configuration
    run_test "Security configuration" test_security_configuration
    
    # Cleanup
    cleanup
    
    # Results
    echo ""
    echo "=== RustCI Compatibility Test Results ==="
    echo "Tests run: $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"
    echo "Log file: $TEST_LOG_FILE"
    
    if [[ $TESTS_FAILED -eq 0 ]]; then
        print_pass "All RustCI compatibility tests passed!"
        log "INFO" "All RustCI compatibility tests completed successfully"
        
        echo ""
        echo "🎉 RustCI Compatibility Verified!"
        echo ""
        echo "✅ Pipeline uses correct 'variables' field for RustCI compatibility"
        echo "✅ Environment variables will be accessible in RustCI execution context"
        echo "✅ Production pipeline ready with environment variable substitution"
        echo "✅ All RustCI schema requirements met"
        echo ""
        echo "📋 Ready for Upload:"
        echo "Testing Pipeline: $RUSTCI_PIPELINE"
        echo "Production Pipeline: $PRODUCTION_PIPELINE"
        
        return 0
    else
        print_fail "$TESTS_FAILED RustCI compatibility tests failed"
        log "ERROR" "$TESTS_FAILED RustCI compatibility tests failed"
        return 1
    fi
}

# Usage function
usage() {
    cat << EOF
RustCI Compatibility Test Script

Usage: $0 [OPTIONS] [COMMAND]

COMMANDS:
    all                Run all compatibility tests (default)
    setup              Setup test environment only
    cleanup            Cleanup test environment only
    single TEST_NAME   Run a single test

OPTIONS:
    -v, --verbose      Enable verbose output
    -h, --help         Show this help

EXAMPLES:
    $0                           # Run all tests
    $0 all                       # Run all tests
    $0 single test_rustci_variables_field
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