#!/bin/bash

# Valkyrie Protocol Test Suite
# Comprehensive testing script for protocol validation and bridge testing

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CARGO_BIN="cargo"
TEST_TIMEOUT="300s"
VERBOSE=${VERBOSE:-false}
PARALLEL_JOBS=${PARALLEL_JOBS:-4}

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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check if cargo is installed
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo is not installed. Please install Rust and Cargo."
        exit 1
    fi
    
    # Check if the project is a Rust project
    if [ ! -f "Cargo.toml" ]; then
        log_error "Not in a Rust project directory. Please run from the project root."
        exit 1
    fi
    
    # Check if Valkyrie tests exist
    if [ ! -d "tests/valkyrie" ]; then
        log_error "Valkyrie test directory not found. Please ensure tests/valkyrie/ exists."
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Build the project
build_project() {
    log_info "Building project..."
    
    if [ "$VERBOSE" = true ]; then
        $CARGO_BIN build --release
    else
        $CARGO_BIN build --release > /dev/null 2>&1
    fi
    
    if [ $? -eq 0 ]; then
        log_success "Project built successfully"
    else
        log_error "Project build failed"
        exit 1
    fi
}

# Run core protocol tests
run_protocol_tests() {
    log_info "Running core protocol tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    if [ "$PARALLEL_JOBS" -gt 1 ]; then
        test_args="$test_args --test-threads $PARALLEL_JOBS"
    fi
    
    $CARGO_BIN test $test_args protocol_tests
    
    if [ $? -eq 0 ]; then
        log_success "Core protocol tests passed"
    else
        log_error "Core protocol tests failed"
        return 1
    fi
}

# Run transport compatibility tests
run_transport_tests() {
    log_info "Running multi-transport compatibility tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    $CARGO_BIN test $test_args transport_tests
    
    if [ $? -eq 0 ]; then
        log_success "Transport compatibility tests passed"
    else
        log_error "Transport compatibility tests failed"
        return 1
    fi
}

# Run HTTP/HTTPS bridge tests
run_bridge_tests() {
    log_info "Running HTTP/HTTPS bridge integration tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    # Start a test HTTP server if needed
    if command -v python3 &> /dev/null; then
        log_info "Starting test HTTP server..."
        python3 -m http.server 8888 > /dev/null 2>&1 &
        HTTP_SERVER_PID=$!
        sleep 2
    fi
    
    $CARGO_BIN test $test_args bridge_tests
    local bridge_result=$?
    
    # Cleanup HTTP server
    if [ ! -z "$HTTP_SERVER_PID" ]; then
        kill $HTTP_SERVER_PID > /dev/null 2>&1 || true
    fi
    
    if [ $bridge_result -eq 0 ]; then
        log_success "HTTP/HTTPS bridge tests passed"
    else
        log_error "HTTP/HTTPS bridge tests failed"
        return 1
    fi
}

# Run security tests
run_security_tests() {
    log_info "Running security penetration tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    $CARGO_BIN test $test_args security_tests
    
    if [ $? -eq 0 ]; then
        log_success "Security tests passed"
    else
        log_error "Security tests failed"
        return 1
    fi
}

# Run compatibility tests
run_compatibility_tests() {
    log_info "Running Kubernetes/Docker compatibility tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    # Check if Docker is available
    if command -v docker &> /dev/null; then
        log_info "Docker detected, running Docker compatibility tests"
        export DOCKER_AVAILABLE=true
    else
        log_warning "Docker not available, skipping Docker-specific tests"
        export DOCKER_AVAILABLE=false
    fi
    
    # Check if kubectl is available
    if command -v kubectl &> /dev/null; then
        log_info "kubectl detected, running Kubernetes compatibility tests"
        export KUBECTL_AVAILABLE=true
    else
        log_warning "kubectl not available, skipping Kubernetes-specific tests"
        export KUBECTL_AVAILABLE=false
    fi
    
    $CARGO_BIN test $test_args compatibility_tests
    
    if [ $? -eq 0 ]; then
        log_success "Compatibility tests passed"
    else
        log_error "Compatibility tests failed"
        return 1
    fi
}

# Run chaos engineering tests
run_chaos_tests() {
    log_info "Running chaos engineering tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    # Chaos tests may take longer
    test_args="--test valkyrie --timeout 600s"
    
    $CARGO_BIN test $test_args chaos_tests
    
    if [ $? -eq 0 ]; then
        log_success "Chaos engineering tests passed"
    else
        log_error "Chaos engineering tests failed"
        return 1
    fi
}

# Run performance tests (basic)
run_basic_performance_tests() {
    log_info "Running basic performance tests..."
    
    local test_args="--test valkyrie --timeout $TEST_TIMEOUT --release"
    
    if [ "$VERBOSE" = true ]; then
        test_args="$test_args --nocapture"
    fi
    
    $CARGO_BIN test $test_args performance_tests::test_sub_millisecond_latency
    $CARGO_BIN test $test_args performance_tests::test_high_throughput
    
    if [ $? -eq 0 ]; then
        log_success "Basic performance tests passed"
    else
        log_error "Basic performance tests failed"
        return 1
    fi
}

# Generate test report
generate_test_report() {
    log_info "Generating test report..."
    
    local report_file="valkyrie-test-report-$(date +%Y%m%d-%H%M%S).txt"
    
    cat > "$report_file" << EOF
Valkyrie Protocol Test Report
Generated: $(date)
========================================

Test Environment:
- OS: $(uname -s)
- Architecture: $(uname -m)
- Rust Version: $(rustc --version)
- Cargo Version: $(cargo --version)

Test Configuration:
- Timeout: $TEST_TIMEOUT
- Parallel Jobs: $PARALLEL_JOBS
- Verbose: $VERBOSE

Test Results:
EOF
    
    # Run tests with JSON output for report
    $CARGO_BIN test --test valkyrie -- --format json >> "$report_file" 2>&1 || true
    
    log_success "Test report generated: $report_file"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    
    # Kill any background processes
    if [ ! -z "$HTTP_SERVER_PID" ]; then
        kill $HTTP_SERVER_PID > /dev/null 2>&1 || true
    fi
    
    # Clean up temporary files
    rm -f /tmp/valkyrie-test-* > /dev/null 2>&1 || true
    
    log_success "Cleanup completed"
}

# Main test execution
run_all_tests() {
    local failed_tests=()
    
    log_info "Starting Valkyrie Protocol test suite..."
    
    # Run each test suite
    run_protocol_tests || failed_tests+=("protocol")
    run_transport_tests || failed_tests+=("transport")
    run_bridge_tests || failed_tests+=("bridge")
    run_security_tests || failed_tests+=("security")
    run_compatibility_tests || failed_tests+=("compatibility")
    run_chaos_tests || failed_tests+=("chaos")
    run_basic_performance_tests || failed_tests+=("performance")
    
    # Report results
    if [ ${#failed_tests[@]} -eq 0 ]; then
        log_success "All Valkyrie Protocol tests passed!"
        return 0
    else
        log_error "The following test suites failed: ${failed_tests[*]}"
        return 1
    fi
}

# Help function
show_help() {
    cat << EOF
Valkyrie Protocol Test Suite

Usage: $0 [OPTIONS] [TEST_SUITE]

Options:
    -v, --verbose       Enable verbose output
    -j, --jobs N        Number of parallel test jobs (default: 4)
    -t, --timeout TIME  Test timeout (default: 300s)
    -h, --help          Show this help message

Test Suites:
    protocol            Core protocol functionality tests
    transport           Multi-transport compatibility tests
    bridge              HTTP/HTTPS bridge integration tests
    security            Security penetration tests
    compatibility       Kubernetes/Docker compatibility tests
    chaos               Chaos engineering tests
    performance         Basic performance tests
    all                 Run all test suites (default)

Examples:
    $0                          # Run all tests
    $0 protocol                 # Run only protocol tests
    $0 -v bridge               # Run bridge tests with verbose output
    $0 --jobs 8 performance    # Run performance tests with 8 parallel jobs

Environment Variables:
    VERBOSE             Enable verbose output (true/false)
    PARALLEL_JOBS       Number of parallel test jobs
    DOCKER_AVAILABLE    Override Docker availability detection
    KUBECTL_AVAILABLE   Override kubectl availability detection
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -j|--jobs)
            PARALLEL_JOBS="$2"
            shift 2
            ;;
        -t|--timeout)
            TEST_TIMEOUT="$2"
            shift 2
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        protocol|transport|bridge|security|compatibility|chaos|performance)
            TEST_SUITE="$1"
            shift
            ;;
        all)
            TEST_SUITE="all"
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Set default test suite
TEST_SUITE=${TEST_SUITE:-all}

# Set up signal handlers
trap cleanup EXIT
trap 'log_error "Test interrupted"; exit 130' INT TERM

# Main execution
main() {
    check_prerequisites
    build_project
    
    case $TEST_SUITE in
        protocol)
            run_protocol_tests
            ;;
        transport)
            run_transport_tests
            ;;
        bridge)
            run_bridge_tests
            ;;
        security)
            run_security_tests
            ;;
        compatibility)
            run_compatibility_tests
            ;;
        chaos)
            run_chaos_tests
            ;;
        performance)
            run_basic_performance_tests
            ;;
        all)
            run_all_tests
            ;;
        *)
            log_error "Unknown test suite: $TEST_SUITE"
            exit 1
            ;;
    esac
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        log_success "Valkyrie Protocol tests completed successfully!"
    else
        log_error "Valkyrie Protocol tests failed!"
    fi
    
    # Generate report if requested
    if [ "$GENERATE_REPORT" = true ]; then
        generate_test_report
    fi
    
    exit $exit_code
}

# Run main function
main "$@"