#!/bin/bash

# RustCI Test Runner Script
# Comprehensive testing script for different test types

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
TEST_TYPE="all"
VERBOSE=false
COVERAGE=false
PARALLEL=true
FILTER=""

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
RustCI Test Runner Script

Usage: $0 [OPTIONS]

OPTIONS:
    -t, --type TYPE        Test type: unit, integration, load, chaos, all (default: all)
    -f, --filter FILTER    Filter tests by name pattern
    -c, --coverage         Generate code coverage report
    -s, --sequential       Run tests sequentially (not in parallel)
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

TEST TYPES:
    unit                   Run unit tests only
    integration           Run integration tests only
    load                  Run load/performance tests
    chaos                 Run chaos engineering tests
    all                   Run all test types

EXAMPLES:
    $0                                    # Run all tests
    $0 -t unit -c                        # Run unit tests with coverage
    $0 -t integration -f cluster         # Run integration tests matching 'cluster'
    $0 -t load -v                        # Run load tests with verbose output

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--type)
            TEST_TYPE="$2"
            shift 2
            ;;
        -f|--filter)
            FILTER="$2"
            shift 2
            ;;
        -c|--coverage)
            COVERAGE=true
            shift
            ;;
        -s|--sequential)
            PARALLEL=false
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

# Validate test type
case $TEST_TYPE in
    unit|integration|load|chaos|all)
        ;;
    *)
        print_error "Invalid test type: $TEST_TYPE"
        show_usage
        exit 1
        ;;
esac

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

print_status "Starting RustCI test runner..."
print_status "Test type: $TEST_TYPE"

# Check prerequisites
check_prerequisites() {
    print_status "Checking prerequisites..."
    
    # Check if required tools are installed
    local missing_tools=()
    
    if ! command -v cargo &> /dev/null; then
        missing_tools+=("cargo (Rust)")
    fi
    
    if [ "$COVERAGE" = true ] && ! command -v cargo-tarpaulin &> /dev/null; then
        print_warning "cargo-tarpaulin not found, installing..."
        cargo install cargo-tarpaulin
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        print_error "Missing required tools:"
        for tool in "${missing_tools[@]}"; do
            echo "  - $tool"
        done
        exit 1
    fi
    
    print_success "All prerequisites satisfied"
}

# Setup test environment
setup_test_environment() {
    print_status "Setting up test environment..."
    
    # Set test environment variables
    export RUST_ENV=test
    export RUST_LOG=debug
    export MONGODB_URI=mongodb://localhost:27017
    export MONGODB_DATABASE=rustci_test
    
    # Start test dependencies if needed
    if [ "$TEST_TYPE" = "integration" ] || [ "$TEST_TYPE" = "all" ]; then
        print_status "Starting test dependencies..."
        
        # Start MongoDB for integration tests
        if ! docker ps | grep -q rustci-test-mongodb; then
            docker run -d --name rustci-test-mongodb \
                -p 27018:27017 \
                -e MONGO_INITDB_DATABASE=rustci_test \
                mongo:7.0
            
            # Wait for MongoDB to be ready
            sleep 10
        fi
        
        export MONGODB_URI=mongodb://localhost:27018
    fi
    
    print_success "Test environment setup complete"
}

# Run unit tests
run_unit_tests() {
    print_status "Running unit tests..."
    
    local cargo_args="test --lib"
    
    if [ -n "$FILTER" ]; then
        cargo_args="$cargo_args $FILTER"
    fi
    
    if [ "$PARALLEL" = false ]; then
        cargo_args="$cargo_args -- --test-threads=1"
    fi
    
    if [ "$VERBOSE" = true ]; then
        cargo_args="$cargo_args -- --nocapture"
    fi
    
    if [ "$COVERAGE" = true ]; then
        print_status "Running unit tests with coverage..."
        cargo tarpaulin --lib --out Html --output-dir target/coverage/unit
        print_status "Coverage report generated: target/coverage/unit/tarpaulin-report.html"
    else
        cargo $cargo_args
    fi
    
    print_success "Unit tests completed"
}

# Run integration tests
run_integration_tests() {
    print_status "Running integration tests..."
    
    local cargo_args="test --test integration"
    
    if [ -n "$FILTER" ]; then
        cargo_args="$cargo_args $FILTER"
    fi
    
    if [ "$PARALLEL" = false ]; then
        cargo_args="$cargo_args -- --test-threads=1"
    fi
    
    if [ "$VERBOSE" = true ]; then
        cargo_args="$cargo_args -- --nocapture"
    fi
    
    cargo $cargo_args
    
    print_success "Integration tests completed"
}

# Run load tests
run_load_tests() {
    print_status "Running load/performance tests..."
    
    # Check if load test dependencies are available
    if [ ! -f "tests/load/mod.rs" ]; then
        print_warning "Load tests not found, skipping..."
        return
    fi
    
    local cargo_args="test --test load"
    
    if [ -n "$FILTER" ]; then
        cargo_args="$cargo_args $FILTER"
    fi
    
    if [ "$VERBOSE" = true ]; then
        cargo_args="$cargo_args -- --nocapture"
    fi
    
    # Set load test specific environment
    export LOAD_TEST_DURATION=60
    export LOAD_TEST_CONCURRENCY=10
    
    cargo $cargo_args
    
    print_success "Load tests completed"
}

# Run chaos tests
run_chaos_tests() {
    print_status "Running chaos engineering tests..."
    
    # Check if chaos test dependencies are available
    if [ ! -f "tests/chaos/mod.rs" ]; then
        print_warning "Chaos tests not found, skipping..."
        return
    fi
    
    local cargo_args="test --test chaos"
    
    if [ -n "$FILTER" ]; then
        cargo_args="$cargo_args $FILTER"
    fi
    
    if [ "$VERBOSE" = true ]; then
        cargo_args="$cargo_args -- --nocapture"
    fi
    
    # Set chaos test specific environment
    export CHAOS_TEST_DURATION=30
    export CHAOS_TEST_INTENSITY=medium
    
    cargo $cargo_args
    
    print_success "Chaos tests completed"
}

# Generate test report
generate_test_report() {
    print_status "Generating test report..."
    
    local report_dir="target/test-reports"
    mkdir -p "$report_dir"
    
    # Generate JUnit XML report if possible
    if command -v cargo-junit &> /dev/null; then
        cargo junit --name rustci-tests > "$report_dir/junit.xml"
        print_status "JUnit report generated: $report_dir/junit.xml"
    fi
    
    # Generate test summary
    cat > "$report_dir/summary.txt" << EOF
RustCI Test Summary
==================
Date: $(date)
Test Type: $TEST_TYPE
Filter: ${FILTER:-"none"}
Coverage: $COVERAGE
Parallel: $PARALLEL

Test Results:
EOF
    
    if [ -f "target/coverage/unit/tarpaulin-report.html" ]; then
        echo "Coverage Report: target/coverage/unit/tarpaulin-report.html" >> "$report_dir/summary.txt"
    fi
    
    print_success "Test report generated: $report_dir/summary.txt"
}

# Cleanup function
cleanup_test_environment() {
    print_status "Cleaning up test environment..."
    
    # Stop test containers
    if docker ps | grep -q rustci-test-mongodb; then
        docker stop rustci-test-mongodb
        docker rm rustci-test-mongodb
    fi
    
    print_success "Test environment cleaned up"
}

# Set up signal handlers
trap cleanup_test_environment EXIT INT TERM

# Main test logic
main() {
    check_prerequisites
    setup_test_environment
    
    case $TEST_TYPE in
        unit)
            run_unit_tests
            ;;
        integration)
            run_integration_tests
            ;;
        load)
            run_load_tests
            ;;
        chaos)
            run_chaos_tests
            ;;
        all)
            run_unit_tests
            run_integration_tests
            run_load_tests
            run_chaos_tests
            ;;
    esac
    
    generate_test_report
}

# Run main function
main

print_success "RustCI test runner completed successfully!"