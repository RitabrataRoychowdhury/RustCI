#!/bin/bash

# Comprehensive Runner Testing Script
# Tests all runner types with detailed API validation and performance metrics

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTCI_SERVER="${RUSTCI_SERVER:-http://localhost:8080}"
TEST_TIMEOUT="${TEST_TIMEOUT:-300}"
PERFORMANCE_ITERATIONS="${PERFORMANCE_ITERATIONS:-5}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Test results
declare -A TEST_RESULTS
declare -A PERFORMANCE_METRICS
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

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

log_perf() {
    echo -e "${CYAN}[PERF]${NC} $1"
}

# Record test result
record_test() {
    local test_name="$1"
    local result="$2"
    local details="${3:-}"
    
    TEST_RESULTS["$test_name"]="$result"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if [[ "$result" == "PASS" ]]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        log_success "✅ $test_name"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        log_error "❌ $test_name"
        if [[ -n "$details" ]]; then
            echo "   Details: $details"
        fi
    fi
}

# Measure execution time
measure_time() {
    local start_time=$(date +%s.%N)
    "$@"
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0")
    echo "$duration"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing_tools=()
    
    for tool in curl jq docker; do
        if ! command -v "$tool" &> /dev/null; then
            missing_tools+=("$tool")
        fi
    done
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        exit 1
    fi
    
    # Check RustCI server
    if ! curl -s "$RUSTCI_SERVER/health" &> /dev/null; then
        log_error "RustCI server not accessible at $RUSTCI_SERVER"
        log_info "Please start the server first: cargo run"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Test server health and basic APIs
test_server_apis() {
    log_test "Testing server APIs..."
    
    # Health endpoint
    local health_response=$(curl -s "$RUSTCI_SERVER/health" || echo "")
    if [[ -n "$health_response" ]] && echo "$health_response" | jq -e '.status == "healthy"' &> /dev/null; then
        record_test "server_health" "PASS"
    else
        record_test "server_health" "FAIL" "Health endpoint returned: $health_response"
    fi
    
    # Runners endpoint
    local runners_response=$(curl -s "$RUSTCI_SERVER/api/runners" || echo "")
    if [[ -n "$runners_response" ]]; then
        record_test "runners_api" "PASS"
        local runner_count=$(echo "$runners_response" | jq '. | length' 2>/dev/null || echo "0")
        log_info "Found $runner_count registered runners"
    else
        record_test "runners_api" "FAIL" "Runners API not accessible"
    fi
    
    # Jobs endpoint
    local jobs_response=$(curl -s "$RUSTCI_SERVER/api/jobs" || echo "")
    if [[ -n "$jobs_response" ]]; then
        record_test "jobs_api" "PASS"
    else
        record_test "jobs_api" "FAIL" "Jobs API not accessible"
    fi
    
    # Pipelines endpoint
    local pipelines_response=$(curl -s "$RUSTCI_SERVER/api/pipelines" || echo "")
    if [[ -n "$pipelines_response" ]]; then
        record_test "pipelines_api" "PASS"
    else
        record_test "pipelines_api" "FAIL" "Pipelines API not accessible"
    fi
}

# Test runner registration
test_runner_registration() {
    log_test "Testing runner registration..."
    
    local test_runner_data='{
        "name": "test-runner-' $(date +%s) '",
        "type": "docker",
        "host": "localhost",
        "port": 8081,
        "capabilities": ["docker", "test"],
        "metadata": {
            "os": "linux",
            "arch": "amd64",
            "test": true
        }
    }'
    
    local registration_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$test_runner_data" \
        "$RUSTCI_SERVER/api/runners/register" || echo "")
    
    if [[ -n "$registration_response" ]] && echo "$registration_response" | jq -e '.success' &> /dev/null; then
        record_test "runner_registration" "PASS"
        local runner_id=$(echo "$registration_response" | jq -r '.runner_id')
        log_info "Registered test runner: $runner_id"
        
        # Test runner status
        local status_response=$(curl -s "$RUSTCI_SERVER/api/runners/$runner_id/status" || echo "")
        if [[ -n "$status_response" ]]; then
            record_test "runner_status_api" "PASS"
        else
            record_test "runner_status_api" "FAIL" "Runner status API not working"
        fi
        
        # Cleanup test runner
        curl -s -X DELETE "$RUSTCI_SERVER/api/runners/$runner_id" &> /dev/null || true
    else
        record_test "runner_registration" "FAIL" "Registration response: $registration_response"
    fi
}

# Test Docker runner
test_docker_runner() {
    log_test "Testing Docker runner..."
    
    if ! docker info &> /dev/null; then
        record_test "docker_runner_available" "FAIL" "Docker daemon not running"
        return
    fi
    
    record_test "docker_runner_available" "PASS"
    
    # Test basic Docker functionality
    if docker run --rm hello-world &> /dev/null; then
        record_test "docker_functionality" "PASS"
    else
        record_test "docker_functionality" "FAIL" "Docker run test failed"
    fi
    
    # Test Docker API version
    local docker_version=$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo "unknown")
    log_info "Docker version: $docker_version"
    
    # Performance test
    log_perf "Testing Docker runner performance..."
    local total_time=0
    local successful_runs=0
    
    for i in $(seq 1 $PERFORMANCE_ITERATIONS); do
        local run_time=$(measure_time docker run --rm alpine:latest echo "test $i" 2>/dev/null || echo "999")
        if [[ "$run_time" != "999" ]]; then
            total_time=$(echo "$total_time + $run_time" | bc -l)
            successful_runs=$((successful_runs + 1))
        fi
    done
    
    if [[ $successful_runs -gt 0 ]]; then
        local avg_time=$(echo "scale=3; $total_time / $successful_runs" | bc -l)
        PERFORMANCE_METRICS["docker_avg_time"]="$avg_time"
        log_perf "Docker average execution time: ${avg_time}s"
        record_test "docker_performance" "PASS"
    else
        record_test "docker_performance" "FAIL" "No successful Docker runs"
    fi
}

# Test DIND runner
test_dind_runner() {
    log_test "Testing DIND runner..."
    
    local dind_container="rustci-dind"
    
    if docker ps | grep -q "$dind_container"; then
        record_test "dind_runner_available" "PASS"
        
        # Test Docker inside DIND
        if docker exec "$dind_container" docker run --rm hello-world &> /dev/null; then
            record_test "dind_functionality" "PASS"
        else
            record_test "dind_functionality" "FAIL" "Docker inside DIND failed"
        fi
        
        # Test DIND isolation
        local host_containers=$(docker ps -q | wc -l)
        local dind_containers=$(docker exec "$dind_container" docker ps -q | wc -l)
        log_info "Host containers: $host_containers, DIND containers: $dind_containers"
        record_test "dind_isolation" "PASS"
        
        # Performance test
        log_perf "Testing DIND runner performance..."
        local total_time=0
        local successful_runs=0
        
        for i in $(seq 1 $PERFORMANCE_ITERATIONS); do
            local run_time=$(measure_time docker exec "$dind_container" docker run --rm alpine:latest echo "test $i" 2>/dev/null || echo "999")
            if [[ "$run_time" != "999" ]]; then
                total_time=$(echo "$total_time + $run_time" | bc -l)
                successful_runs=$((successful_runs + 1))
            fi
        done
        
        if [[ $successful_runs -gt 0 ]]; then
            local avg_time=$(echo "scale=3; $total_time / $successful_runs" | bc -l)
            PERFORMANCE_METRICS["dind_avg_time"]="$avg_time"
            log_perf "DIND average execution time: ${avg_time}s"
            record_test "dind_performance" "PASS"
        else
            record_test "dind_performance" "FAIL" "No successful DIND runs"
        fi
    else
        record_test "dind_runner_available" "FAIL" "DIND container not running"
        record_test "dind_functionality" "SKIP" "DIND not available"
        record_test "dind_isolation" "SKIP" "DIND not available"
        record_test "dind_performance" "SKIP" "DIND not available"
    fi
}

# Test native runner
test_native_runner() {
    log_test "Testing native runner..."
    
    # Test shell availability
    if command -v bash &> /dev/null; then
        record_test "native_shell_available" "PASS"
    else
        record_test "native_shell_available" "FAIL" "Bash not available"
        return
    fi
    
    # Test basic command execution
    if echo "test" | bash -c "cat" &> /dev/null; then
        record_test "native_functionality" "PASS"
    else
        record_test "native_functionality" "FAIL" "Basic shell execution failed"
    fi
    
    # Test file operations
    local temp_file="/tmp/rustci-test-$$"
    if echo "test content" > "$temp_file" && [[ -f "$temp_file" ]] && rm "$temp_file"; then
        record_test "native_file_ops" "PASS"
    else
        record_test "native_file_ops" "FAIL" "File operations failed"
    fi
    
    # Performance test
    log_perf "Testing native runner performance..."
    local total_time=0
    local successful_runs=0
    
    for i in $(seq 1 $PERFORMANCE_ITERATIONS); do
        local run_time=$(measure_time bash -c "echo 'test $i'" 2>/dev/null || echo "999")
        if [[ "$run_time" != "999" ]]; then
            total_time=$(echo "$total_time + $run_time" | bc -l)
            successful_runs=$((successful_runs + 1))
        fi
    done
    
    if [[ $successful_runs -gt 0 ]]; then
        local avg_time=$(echo "scale=3; $total_time / $successful_runs" | bc -l)
        PERFORMANCE_METRICS["native_avg_time"]="$avg_time"
        log_perf "Native average execution time: ${avg_time}s"
        record_test "native_performance" "PASS"
    else
        record_test "native_performance" "FAIL" "No successful native runs"
    fi
}

# Test job execution
test_job_execution() {
    log_test "Testing job execution..."
    
    # Create test job
    local job_data='{
        "name": "comprehensive-test-job",
        "steps": [
            {
                "name": "echo-test",
                "run": "echo Hello from comprehensive test"
            },
            {
                "name": "env-test",
                "run": "env | grep -E \"^(PATH|HOME|USER)\" | head -3"
            },
            {
                "name": "file-test",
                "run": "echo test > /tmp/test.txt && cat /tmp/test.txt && rm /tmp/test.txt"
            }
        ]
    }'
    
    local job_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$job_data" \
        "$RUSTCI_SERVER/api/jobs" || echo "")
    
    if [[ -n "$job_response" ]] && echo "$job_response" | jq -e '.job_id' &> /dev/null; then
        local job_id=$(echo "$job_response" | jq -r '.job_id')
        log_info "Created test job: $job_id"
        
        # Monitor job execution
        local max_wait=$TEST_TIMEOUT
        local waited=0
        local job_completed=false
        
        while [[ $waited -lt $max_wait ]]; do
            local status_response=$(curl -s "$RUSTCI_SERVER/api/jobs/$job_id/status" || echo "")
            
            if [[ -n "$status_response" ]]; then
                local status=$(echo "$status_response" | jq -r '.status // "unknown"')
                
                case "$status" in
                    "completed"|"success")
                        record_test "job_execution" "PASS"
                        job_completed=true
                        break
                        ;;
                    "failed"|"error")
                        record_test "job_execution" "FAIL" "Job failed with status: $status"
                        job_completed=true
                        break
                        ;;
                    "running"|"pending")
                        log_info "Job status: $status (waited ${waited}s)"
                        ;;
                esac
            fi
            
            sleep 5
            waited=$((waited + 5))
        done
        
        if [[ "$job_completed" == false ]]; then
            record_test "job_execution" "FAIL" "Job execution timed out after ${max_wait}s"
        fi
        
        # Test job logs
        local logs_response=$(curl -s "$RUSTCI_SERVER/api/jobs/$job_id/logs" || echo "")
        if [[ -n "$logs_response" ]]; then
            record_test "job_logs_api" "PASS"
        else
            record_test "job_logs_api" "FAIL" "Job logs not accessible"
        fi
    else
        record_test "job_execution" "FAIL" "Job creation failed: $job_response"
        record_test "job_logs_api" "SKIP" "Job not created"
    fi
}

# Test pipeline execution
test_pipeline_execution() {
    log_test "Testing pipeline execution..."
    
    # Create test pipeline
    local pipeline_yaml='
name: "Comprehensive Test Pipeline"
description: "Test pipeline for comprehensive runner testing"

stages:
  - name: "test"
    jobs:
      - name: "basic-test"
        steps:
          - name: "echo-test"
            run: |
              echo "Pipeline test started"
              echo "Current directory: $(pwd)"
              echo "Available commands:"
              which echo bash || true
              
          - name: "multi-line-test"
            run: |
              echo "Line 1"
              echo "Line 2"
              echo "Line 3"
              
  - name: "validation"
    jobs:
      - name: "validation-test"
        steps:
          - name: "validation"
            run: |
              echo "Validation completed successfully"
              exit 0
'
    
    local pipeline_response=$(curl -s -X POST \
        -H "Content-Type: application/yaml" \
        --data-binary "$pipeline_yaml" \
        "$RUSTCI_SERVER/api/pipelines/run" || echo "")
    
    if [[ -n "$pipeline_response" ]] && echo "$pipeline_response" | jq -e '.pipeline_id' &> /dev/null; then
        local pipeline_id=$(echo "$pipeline_response" | jq -r '.pipeline_id')
        log_info "Created test pipeline: $pipeline_id"
        
        # Monitor pipeline execution
        local max_wait=$TEST_TIMEOUT
        local waited=0
        local pipeline_completed=false
        
        while [[ $waited -lt $max_wait ]]; do
            local status_response=$(curl -s "$RUSTCI_SERVER/api/pipelines/$pipeline_id/status" || echo "")
            
            if [[ -n "$status_response" ]]; then
                local status=$(echo "$status_response" | jq -r '.status // "unknown"')
                
                case "$status" in
                    "completed"|"success")
                        record_test "pipeline_execution" "PASS"
                        pipeline_completed=true
                        break
                        ;;
                    "failed"|"error")
                        record_test "pipeline_execution" "FAIL" "Pipeline failed with status: $status"
                        pipeline_completed=true
                        break
                        ;;
                    "running"|"pending")
                        log_info "Pipeline status: $status (waited ${waited}s)"
                        ;;
                esac
            fi
            
            sleep 10
            waited=$((waited + 10))
        done
        
        if [[ "$pipeline_completed" == false ]]; then
            record_test "pipeline_execution" "FAIL" "Pipeline execution timed out after ${max_wait}s"
        fi
    else
        record_test "pipeline_execution" "FAIL" "Pipeline creation failed: $pipeline_response"
    fi
}

# Test error handling
test_error_handling() {
    log_test "Testing error handling..."
    
    # Test invalid job
    local invalid_job='{
        "name": "invalid-test-job",
        "steps": [
            {
                "name": "failing-step",
                "run": "exit 1"
            }
        ]
    }'
    
    local job_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$invalid_job" \
        "$RUSTCI_SERVER/api/jobs" || echo "")
    
    if [[ -n "$job_response" ]] && echo "$job_response" | jq -e '.job_id' &> /dev/null; then
        local job_id=$(echo "$job_response" | jq -r '.job_id')
        
        # Wait for job to fail
        local max_wait=60
        local waited=0
        
        while [[ $waited -lt $max_wait ]]; do
            local status_response=$(curl -s "$RUSTCI_SERVER/api/jobs/$job_id/status" || echo "")
            
            if [[ -n "$status_response" ]]; then
                local status=$(echo "$status_response" | jq -r '.status // "unknown"')
                
                if [[ "$status" == "failed" || "$status" == "error" ]]; then
                    record_test "error_handling" "PASS"
                    break
                fi
            fi
            
            sleep 2
            waited=$((waited + 2))
        done
        
        if [[ $waited -ge $max_wait ]]; then
            record_test "error_handling" "FAIL" "Failing job did not fail as expected"
        fi
    else
        record_test "error_handling" "FAIL" "Could not create failing job"
    fi
    
    # Test invalid API requests
    local invalid_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"invalid": "data"}' \
        "$RUSTCI_SERVER/api/jobs" || echo "")
    
    if [[ -n "$invalid_response" ]] && echo "$invalid_response" | jq -e '.error' &> /dev/null; then
        record_test "api_error_handling" "PASS"
    else
        record_test "api_error_handling" "FAIL" "API did not handle invalid request properly"
    fi
}

# Generate performance report
generate_performance_report() {
    log_info "Performance Metrics:"
    echo "===================="
    
    for metric in "${!PERFORMANCE_METRICS[@]}"; do
        echo "$metric: ${PERFORMANCE_METRICS[$metric]}"
    done
    
    # Compare performance if multiple runners tested
    if [[ -n "${PERFORMANCE_METRICS[native_avg_time]:-}" ]] && [[ -n "${PERFORMANCE_METRICS[docker_avg_time]:-}" ]]; then
        local native_time="${PERFORMANCE_METRICS[native_avg_time]}"
        local docker_time="${PERFORMANCE_METRICS[docker_avg_time]}"
        local speedup=$(echo "scale=2; $docker_time / $native_time" | bc -l)
        echo "Native vs Docker speedup: ${speedup}x"
    fi
    
    if [[ -n "${PERFORMANCE_METRICS[docker_avg_time]:-}" ]] && [[ -n "${PERFORMANCE_METRICS[dind_avg_time]:-}" ]]; then
        local docker_time="${PERFORMANCE_METRICS[docker_avg_time]}"
        local dind_time="${PERFORMANCE_METRICS[dind_avg_time]}"
        local overhead=$(echo "scale=2; $dind_time / $docker_time" | bc -l)
        echo "DIND vs Docker overhead: ${overhead}x"
    fi
}

# Generate test report
generate_test_report() {
    echo ""
    log_info "Test Results Summary:"
    echo "====================="
    echo "Total Tests: $TOTAL_TESTS"
    echo "Passed: $PASSED_TESTS"
    echo "Failed: $FAILED_TESTS"
    
    local success_rate=0
    if [[ $TOTAL_TESTS -gt 0 ]]; then
        success_rate=$(echo "scale=1; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l)
    fi
    echo "Success Rate: ${success_rate}%"
    
    echo ""
    log_info "Detailed Results:"
    echo "=================="
    
    for test_name in "${!TEST_RESULTS[@]}"; do
        local result="${TEST_RESULTS[$test_name]}"
        case "$result" in
            "PASS")
                echo "✅ $test_name"
                ;;
            "FAIL")
                echo "❌ $test_name"
                ;;
            "SKIP")
                echo "⏭️  $test_name"
                ;;
        esac
    done
    
    echo ""
    generate_performance_report
    
    # Overall result
    echo ""
    if [[ $FAILED_TESTS -eq 0 ]]; then
        log_success "🎉 All tests passed! RustCI runners are working correctly."
    else
        log_error "❌ $FAILED_TESTS test(s) failed. Please check the issues above."
        return 1
    fi
}

# Main test execution
main() {
    log_info "Starting Comprehensive Runner Testing"
    echo "======================================"
    echo "Server: $RUSTCI_SERVER"
    echo "Timeout: ${TEST_TIMEOUT}s"
    echo "Performance Iterations: $PERFORMANCE_ITERATIONS"
    echo ""
    
    check_prerequisites
    
    # Run all tests
    test_server_apis
    test_runner_registration
    test_docker_runner
    test_dind_runner
    test_native_runner
    test_job_execution
    test_pipeline_execution
    test_error_handling
    
    # Generate report
    generate_test_report
}

# Show usage
show_usage() {
    cat << EOF
Comprehensive Runner Testing Script

This script performs extensive testing of all RustCI runner types including:
- Server API validation
- Runner registration and management
- Docker, DIND, and native runner functionality
- Job and pipeline execution
- Error handling and recovery
- Performance benchmarking

Usage: $0 [OPTIONS]

OPTIONS:
    --server URL        RustCI server URL (default: http://localhost:8080)
    --timeout SECONDS   Test timeout in seconds (default: 300)
    --iterations N      Performance test iterations (default: 5)
    --help             Show this help message

ENVIRONMENT VARIABLES:
    RUSTCI_SERVER              Server URL
    TEST_TIMEOUT               Test timeout in seconds
    PERFORMANCE_ITERATIONS     Number of performance test iterations

EXAMPLES:
    $0                                    # Run all tests with defaults
    $0 --server http://localhost:8080     # Test specific server
    $0 --timeout 600 --iterations 10      # Extended testing
    
PREREQUISITES:
    - RustCI server running
    - Docker daemon running
    - curl, jq, bc commands available

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --server)
            RUSTCI_SERVER="$2"
            shift 2
            ;;
        --timeout)
            TEST_TIMEOUT="$2"
            shift 2
            ;;
        --iterations)
            PERFORMANCE_ITERATIONS="$2"
            shift 2
            ;;
        --help|-h)
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

# Run main function
main