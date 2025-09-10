#!/bin/bash

# Test Script for DIND to Fake Server Deployment
# Quick validation of the deployment setup

set -euo pipefail

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

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_test() {
    echo -e "${YELLOW}[TEST]${NC} $1"
}

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    log_test "Running: $test_name"
    
    if eval "$test_command" &> /dev/null; then
        log_success "✅ $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        log_error "❌ $test_name"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Test infrastructure components
test_infrastructure() {
    log_info "Testing Infrastructure Components"
    echo "================================="
    
    run_test "Docker daemon running" "docker info"
    run_test "Fake server container exists" "docker ps | grep rustci-ec2-1"
    run_test "DIND server container exists" "docker ps | grep rustci-server-sim"
    run_test "DIND runner container exists" "docker ps | grep rustci-runner-sim"
}

# Test network connectivity
test_connectivity() {
    log_info "Testing Network Connectivity"
    echo "============================"
    
    run_test "RustCI server health" "curl -s http://localhost:8080/health"
    run_test "Fake server SSH connectivity" "docker exec rustci-ec2-1 echo 'SSH test'"
    run_test "DIND to fake server connectivity" "docker exec rustci-runner-sim ping -c 1 rustci-ec2-1"
}

# Test application deployment
test_application() {
    log_info "Testing Application Deployment"
    echo "=============================="
    
    run_test "Application container running" "docker exec rustci-ec2-1 docker ps | grep nodejs-hello-world"
    run_test "Application main endpoint" "docker exec rustci-ec2-1 curl -s http://localhost:3000/ | grep 'Hello World'"
    run_test "Application health endpoint" "docker exec rustci-ec2-1 curl -s http://localhost:3000/health | grep 'healthy'"
}

# Test API endpoints
test_apis() {
    log_info "Testing API Endpoints"
    echo "===================="
    
    run_test "RustCI runners API" "curl -s http://localhost:8080/api/runners"
    run_test "RustCI pipelines API" "curl -s http://localhost:8080/api/pipelines"
    run_test "RustCI jobs API" "curl -s http://localhost:8080/api/jobs"
}

# Interactive application test
interactive_test() {
    log_info "Interactive Application Test"
    echo "============================"
    
    echo ""
    echo "Testing the deployed application interactively..."
    
    # Test main endpoint
    echo "1. Testing main endpoint:"
    local main_response=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/ 2>/dev/null || echo "FAILED")
    echo "   Response: $main_response"
    
    # Test health endpoint
    echo "2. Testing health endpoint:"
    local health_response=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/health 2>/dev/null || echo "FAILED")
    echo "   Response: $health_response"
    
    # Show container status
    echo "3. Container status:"
    docker exec rustci-ec2-1 docker ps | grep nodejs || echo "   No nodejs containers found"
    
    # Show recent logs
    echo "4. Recent application logs:"
    docker exec rustci-ec2-1 docker logs --tail 10 nodejs-hello-world 2>/dev/null || echo "   No logs available"
}

# Performance test
performance_test() {
    log_info "Performance Test"
    echo "================"
    
    echo "Running 10 requests to test performance..."
    
    local total_time=0
    local successful_requests=0
    
    for i in {1..10}; do
        local start_time=$(date +%s.%N)
        if docker exec rustci-ec2-1 curl -s http://localhost:3000/ &> /dev/null; then
            local end_time=$(date +%s.%N)
            local duration=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0")
            total_time=$(echo "$total_time + $duration" | bc -l 2>/dev/null || echo "$total_time")
            successful_requests=$((successful_requests + 1))
            echo "  Request $i: ${duration}s"
        else
            echo "  Request $i: FAILED"
        fi
    done
    
    if [[ $successful_requests -gt 0 ]]; then
        local avg_time=$(echo "scale=3; $total_time / $successful_requests" | bc -l 2>/dev/null || echo "0")
        echo "  Average response time: ${avg_time}s"
        echo "  Success rate: $successful_requests/10"
    else
        echo "  All requests failed"
    fi
}

# Generate test report
generate_report() {
    echo ""
    log_info "Test Results Summary"
    echo "===================="
    echo "Total Tests: $TOTAL_TESTS"
    echo "Passed: $PASSED_TESTS"
    echo "Failed: $FAILED_TESTS"
    
    local success_rate=0
    if [[ $TOTAL_TESTS -gt 0 ]]; then
        success_rate=$(echo "scale=1; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l 2>/dev/null || echo "0")
    fi
    echo "Success Rate: ${success_rate}%"
    
    echo ""
    if [[ $FAILED_TESTS -eq 0 ]]; then
        log_success "🎉 All tests passed! Deployment is working correctly."
        return 0
    else
        log_error "❌ $FAILED_TESTS test(s) failed. Please check the setup."
        return 1
    fi
}

# Show usage
show_usage() {
    cat << EOF
DIND to Fake Server Deployment Test Script

This script validates the DIND to fake server deployment setup by running
comprehensive tests on all components.

Usage: $0 [COMMAND]

COMMANDS:
    test        Run all tests (default)
    quick       Run quick infrastructure tests only
    app         Test application deployment only
    perf        Run performance tests
    interactive Run interactive application test
    help        Show this help message

EXAMPLES:
    $0              # Run all tests
    $0 quick        # Quick infrastructure check
    $0 app          # Test application only
    $0 perf         # Performance testing
    $0 interactive  # Interactive testing

PREREQUISITES:
    • DIND to fake server setup must be running
    • Run: ./scripts/runners/dind-to-fake-server-setup.sh setup

EOF
}

# Main execution
main() {
    log_info "DIND to Fake Server Deployment Test"
    echo "===================================="
    
    test_infrastructure
    echo ""
    test_connectivity
    echo ""
    test_application
    echo ""
    test_apis
    echo ""
    interactive_test
    echo ""
    performance_test
    
    generate_report
}

# Handle command line arguments
case "${1:-test}" in
    "test"|"")
        main
        ;;
    "quick")
        log_info "Quick Infrastructure Test"
        echo "========================="
        test_infrastructure
        generate_report
        ;;
    "app")
        log_info "Application Test"
        echo "==============="
        test_application
        interactive_test
        generate_report
        ;;
    "perf")
        log_info "Performance Test"
        echo "==============="
        performance_test
        ;;
    "interactive")
        interactive_test
        ;;
    "help"|"-h"|"--help")
        show_usage
        ;;
    *)
        log_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac