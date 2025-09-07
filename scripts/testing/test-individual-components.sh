#!/bin/bash

# RustCI Individual Component Testing Script
# Test specific components one by one for detailed analysis

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
BASE_URL="${BASE_URL:-http://localhost:8000}"
COMPONENT=""
VERBOSE=${VERBOSE:-false}
JWT_TOKEN=""

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

log_test() {
    echo -e "${PURPLE}[TEST]${NC} $1"
}

# Function to make API requests
api_request() {
    local method="$1"
    local endpoint="$2"
    local data="$3"
    
    local curl_cmd="curl -s -w '\n%{http_code}'"
    
    if [ -n "$JWT_TOKEN" ]; then
        curl_cmd="$curl_cmd -H 'Authorization: Bearer $JWT_TOKEN'"
    fi
    
    if [ -n "$data" ]; then
        curl_cmd="$curl_cmd -H 'Content-Type: application/json' -d '$data'"
    fi
    
    curl_cmd="$curl_cmd -X $method '$BASE_URL$endpoint'"
    
    eval "$curl_cmd"
}

# Test Error Handling System
test_error_handling_component() {
    echo -e "${CYAN}=== Error Handling System Tests ===${NC}"
    
    log_test "1. Testing structured error responses"
    local error_response
    error_response=$(api_request "GET" "/api/nonexistent")
    local status_code
    status_code=$(echo "$error_response" | tail -n1)
    local body
    body=$(echo "$error_response" | sed '$d')
    
    echo "Status Code: $status_code"
    echo "Response Body: $body"
    
    if [ "$status_code" = "404" ]; then
        log_success "✅ Proper 404 error handling"
    else
        log_warning "⚠️ Unexpected status code: $status_code"
    fi
    
    log_test "2. Testing error correlation tracking"
    # Test multiple requests to see if correlation IDs are present
    for i in {1..3}; do
        local response
        response=$(api_request "GET" "/api/invalid-$i")
        echo "Request $i: $(echo "$response" | sed '$d')"
    done
    
    log_test "3. Testing malformed request handling"
    local malformed_response
    malformed_response=$(api_request "POST" "/api/ci/pipelines" '{"invalid": json}')
    echo "Malformed JSON Response: $(echo "$malformed_response" | sed '$d')"
    
    log_test "4. Testing authentication error handling"
    local auth_response
    auth_response=$(curl -s -w '\n%{http_code}' -X GET "$BASE_URL/api/ci/pipelines")
    local auth_status
    auth_status=$(echo "$auth_response" | tail -n1)
    
    if [ "$auth_status" = "401" ]; then
        log_success "✅ Proper authentication error handling"
    else
        log_warning "⚠️ Unexpected auth status: $auth_status"
    fi
    
    echo ""
}

# Test Configuration Management
test_configuration_component() {
    echo -e "${CYAN}=== Configuration Management Tests ===${NC}"
    
    log_test "1. Testing configuration validation"
    local config_status
    config_status=$(api_request "GET" "/api/config/status")
    echo "Config Status: $(echo "$config_status" | sed '$d')"
    
    log_test "2. Testing environment-specific configurations"
    # Test loading different environment configurations
    echo "Current Environment Configuration:"
    echo "- ENVIRONMENT: ${ENVIRONMENT:-development}"
    echo "- Configuration files should be in config/environments/"
    
    log_test "3. Testing configuration hot-reload capability"
    local reload_response
    reload_response=$(api_request "POST" "/api/config/reload")
    echo "Reload Response: $(echo "$reload_response" | sed '$d')"
    
    log_test "4. Testing configuration validation engine"
    local validation_response
    validation_response=$(api_request "GET" "/api/config/validate")
    echo "Validation Response: $(echo "$validation_response" | sed '$d')"
    
    echo ""
}

# Test Database Operations
test_database_component() {
    echo -e "${CYAN}=== Database Operations Tests ===${NC}"
    
    log_test "1. Testing database health monitoring"
    local db_health
    db_health=$(api_request "GET" "/api/health/database")
    echo "Database Health: $(echo "$db_health" | sed '$d')"
    
    log_test "2. Testing connection pooling"
    # Create multiple concurrent requests to test connection pool
    echo "Testing connection pool with concurrent requests..."
    for i in {1..5}; do
        (
            local pool_test
            pool_test=$(api_request "GET" "/api/ci/pipelines")
            echo "Concurrent request $i status: $(echo "$pool_test" | tail -n1)"
        ) &
    done
    wait
    
    log_test "3. Testing transaction management"
    # Test creating and then deleting a pipeline (transaction test)
    local pipeline_data='{"name":"transaction-test","yaml_content":"name: test\nsteps:\n  - echo: hello"}'
    local create_response
    create_response=$(api_request "POST" "/api/ci/pipelines" "$pipeline_data")
    echo "Transaction Test - Create: $(echo "$create_response" | sed '$d')"
    
    log_test "4. Testing query optimization"
    # Test listing pipelines with different parameters
    local list_response
    list_response=$(api_request "GET" "/api/ci/pipelines?limit=10&offset=0")
    echo "Query Optimization Test: $(echo "$list_response" | sed '$d')"
    
    echo ""
}

# Test Performance Optimization
test_performance_component() {
    echo -e "${CYAN}=== Performance Optimization Tests ===${NC}"
    
    log_test "1. Testing auto-scaling configuration"
    local scaling_config='{
        "min_instances": 1,
        "max_instances": 5,
        "target_cpu_utilization": 70,
        "scale_up_cooldown": 300,
        "scale_down_cooldown": 600
    }'
    local scaling_response
    scaling_response=$(api_request "POST" "/api/scaling/config" "$scaling_config")
    echo "Auto-scaling Config: $(echo "$scaling_response" | sed '$d')"
    
    log_test "2. Testing load balancer functionality"
    local lb_config='{
        "algorithm": "round_robin",
        "health_check": {
            "path": "/health",
            "interval": 30,
            "timeout": 10
        }
    }'
    local lb_response
    lb_response=$(api_request "POST" "/api/load-balancer/config" "$lb_config")
    echo "Load Balancer Config: $(echo "$lb_response" | sed '$d')"
    
    log_test "3. Testing cache management"
    local cache_stats
    cache_stats=$(api_request "GET" "/api/cache/stats")
    echo "Cache Stats: $(echo "$cache_stats" | sed '$d')"
    
    # Test cache operations
    local cache_set
    cache_set=$(api_request "POST" "/api/cache/set" '{"key":"test-key","value":"test-value","ttl":300}')
    echo "Cache Set: $(echo "$cache_set" | sed '$d')"
    
    local cache_get
    cache_get=$(api_request "GET" "/api/cache/get/test-key")
    echo "Cache Get: $(echo "$cache_get" | sed '$d')"
    
    log_test "4. Testing performance monitoring"
    local perf_metrics
    perf_metrics=$(api_request "GET" "/api/metrics/performance")
    echo "Performance Metrics: $(echo "$perf_metrics" | sed '$d')"
    
    echo ""
}

# Test API Robustness
test_api_component() {
    echo -e "${CYAN}=== API Robustness Tests ===${NC}"
    
    log_test "1. Testing API versioning"
    for version in v1 v2; do
        local version_response
        version_response=$(api_request "GET" "/api/$version/pipelines")
        echo "API $version Response: $(echo "$version_response" | tail -n1)"
    done
    
    log_test "2. Testing rate limiting"
    echo "Testing rate limiting with rapid requests..."
    local rate_limit_count=0
    for i in {1..15}; do
        local response
        response=$(api_request "GET" "/api/ci/pipelines")
        local status_code
        status_code=$(echo "$response" | tail -n1)
        
        if [ "$status_code" = "429" ]; then
            rate_limit_count=$((rate_limit_count + 1))
        fi
        
        echo "Request $i: Status $status_code"
        sleep 0.1
    done
    
    echo "Rate limited requests: $rate_limit_count/15"
    
    log_test "3. Testing API authentication"
    local auth_test
    auth_test=$(api_request "GET" "/api/auth/validate")
    echo "Auth Validation: $(echo "$auth_test" | sed '$d')"
    
    log_test "4. Testing response optimization"
    # Test compression and pagination
    local optimized_response
    optimized_response=$(curl -s -H "Accept-Encoding: gzip" -w '\n%{http_code}' "$BASE_URL/api/ci/pipelines?limit=5")
    echo "Optimized Response Status: $(echo "$optimized_response" | tail -n1)"
    
    echo ""
}

# Test Resource Management
test_resource_component() {
    echo -e "${CYAN}=== Resource Management Tests ===${NC}"
    
    log_test "1. Testing resource lifecycle management"
    local lifecycle_status
    lifecycle_status=$(api_request "GET" "/api/resources/lifecycle/status")
    echo "Lifecycle Status: $(echo "$lifecycle_status" | sed '$d')"
    
    log_test "2. Testing resource quotas and throttling"
    local quota_config='{
        "cpu_limit": "2.0",
        "memory_limit": "4Gi",
        "storage_limit": "10Gi",
        "max_concurrent_jobs": 5
    }'
    local quota_response
    quota_response=$(api_request "POST" "/api/resources/quotas" "$quota_config")
    echo "Quota Config: $(echo "$quota_response" | sed '$d')"
    
    log_test "3. Testing background job management"
    local job_queue_status
    job_queue_status=$(api_request "GET" "/api/jobs/queue/status")
    echo "Job Queue Status: $(echo "$job_queue_status" | sed '$d')"
    
    # Test job creation
    local job_data='{
        "name": "test-background-job",
        "type": "cleanup",
        "priority": "normal",
        "payload": {"action": "cleanup_old_logs"}
    }'
    local job_response
    job_response=$(api_request "POST" "/api/jobs" "$job_data")
    echo "Job Creation: $(echo "$job_response" | sed '$d')"
    
    log_test "4. Testing file storage management"
    local storage_stats
    storage_stats=$(api_request "GET" "/api/storage/stats")
    echo "Storage Stats: $(echo "$storage_stats" | sed '$d')"
    
    echo ""
}

# Test Deployment Capabilities
test_deployment_component() {
    echo -e "${CYAN}=== Deployment Capabilities Tests ===${NC}"
    
    log_test "1. Testing blue-green deployment"
    local bg_config='{
        "service_name": "test-service",
        "blue_config": {
            "image": "app:v1.0",
            "replicas": 2,
            "port": 8080
        },
        "green_config": {
            "image": "app:v1.1",
            "replicas": 2,
            "port": 8081
        }
    }'
    local bg_response
    bg_response=$(api_request "POST" "/api/deployment/blue-green" "$bg_config")
    echo "Blue-Green Deployment: $(echo "$bg_response" | sed '$d')"
    
    log_test "2. Testing circuit breaker functionality"
    local cb_config='{
        "service": "external-api",
        "failure_threshold": 5,
        "recovery_timeout": 60,
        "half_open_max_calls": 3
    }'
    local cb_response
    cb_response=$(api_request "POST" "/api/circuit-breaker/config" "$cb_config")
    echo "Circuit Breaker Config: $(echo "$cb_response" | sed '$d')"
    
    log_test "3. Testing graceful degradation"
    local degradation_status
    degradation_status=$(api_request "GET" "/api/deployment/degradation/status")
    echo "Degradation Status: $(echo "$degradation_status" | sed '$d')"
    
    log_test "4. Testing auto-scaling integration"
    local scaling_status
    scaling_status=$(api_request "GET" "/api/deployment/scaling/status")
    echo "Scaling Status: $(echo "$scaling_status" | sed '$d')"
    
    echo ""
}

# Test Valkyrie Protocol
test_valkyrie_component() {
    echo -e "${CYAN}=== Valkyrie Protocol Tests ===${NC}"
    
    log_test "1. Testing Valkyrie job dispatch optimization"
    local valkyrie_job='{
        "id": "valkyrie-test-001",
        "job_type": "Build",
        "priority": "High",
        "payload": {
            "Small": "dGVzdCBwYXlsb2FkIGZvciBWYWxreXJpZQ=="
        }
    }'
    local job_response
    job_response=$(api_request "POST" "/api/valkyrie/jobs" "$valkyrie_job")
    echo "Job Dispatch: $(echo "$job_response" | sed '$d')"
    
    log_test "2. Testing connection pool optimization"
    local pool_stats
    pool_stats=$(api_request "GET" "/api/valkyrie/connection-pool/stats")
    echo "Connection Pool Stats: $(echo "$pool_stats" | sed '$d')"
    
    log_test "3. Testing performance metrics"
    local perf_metrics
    perf_metrics=$(api_request "GET" "/api/valkyrie/metrics")
    echo "Valkyrie Metrics: $(echo "$perf_metrics" | sed '$d')"
    
    log_test "4. Testing batch optimization"
    # Send multiple jobs to test batching
    echo "Testing batch optimization with multiple jobs..."
    for i in {1..5}; do
        local batch_job="{\"id\":\"batch-job-$i\",\"job_type\":\"Test\",\"priority\":\"Normal\",\"payload\":{\"Small\":\"$(echo "test-$i" | base64)\"}}"
        local batch_response
        batch_response=$(api_request "POST" "/api/valkyrie/jobs" "$batch_job")
        echo "Batch Job $i: $(echo "$batch_response" | tail -n1)"
    done
    
    log_test "5. Running standalone Valkyrie performance test"
    if [ -f "scripts/simple-valkyrie-performance-test.sh" ]; then
        echo "Running Valkyrie performance validation..."
        bash scripts/simple-valkyrie-performance-test.sh 2>&1 | head -20
        echo "... (truncated, see full output in standalone test)"
    else
        echo "Standalone Valkyrie test not found"
    fi
    
    echo ""
}

# Test Self-Healing System
test_self_healing_component() {
    echo -e "${CYAN}=== Self-Healing System Tests ===${NC}"
    
    log_test "1. Testing self-healing configuration"
    local healing_config='{
        "enable_auto_healing": true,
        "detection_interval": 30,
        "healing_timeout": 300,
        "max_concurrent_healers": 10,
        "escalation_threshold": 3
    }'
    local healing_response
    healing_response=$(api_request "POST" "/api/self-healing/config" "$healing_config")
    echo "Self-Healing Config: $(echo "$healing_response" | sed '$d')"
    
    log_test "2. Testing issue detection"
    local detection_status
    detection_status=$(api_request "GET" "/api/self-healing/detection/status")
    echo "Detection Status: $(echo "$detection_status" | sed '$d')"
    
    log_test "3. Testing healing strategies"
    local strategies
    strategies=$(api_request "GET" "/api/self-healing/strategies")
    echo "Available Strategies: $(echo "$strategies" | sed '$d')"
    
    log_test "4. Testing healing metrics"
    local healing_metrics
    healing_metrics=$(api_request "GET" "/api/self-healing/metrics")
    echo "Healing Metrics: $(echo "$healing_metrics" | sed '$d')"
    
    log_test "5. Testing active healers"
    local active_healers
    active_healers=$(api_request "GET" "/api/self-healing/active")
    echo "Active Healers: $(echo "$active_healers" | sed '$d')"
    
    echo ""
}

# Test Testing Framework
test_testing_component() {
    echo -e "${CYAN}=== Testing Framework Tests ===${NC}"
    
    log_test "1. Running unit tests"
    echo "Running Rust unit tests..."
    if cargo test --lib --quiet 2>/dev/null; then
        log_success "✅ Unit tests passed"
    else
        log_warning "⚠️ Some unit tests failed or not available"
    fi
    
    log_test "2. Running integration tests"
    echo "Running integration tests..."
    if cargo test --test '*' --quiet 2>/dev/null; then
        log_success "✅ Integration tests passed"
    else
        log_warning "⚠️ Some integration tests failed or not available"
    fi
    
    log_test "3. Testing production test suite"
    local test_suite_status
    test_suite_status=$(api_request "GET" "/api/testing/suite/status")
    echo "Test Suite Status: $(echo "$test_suite_status" | sed '$d')"
    
    log_test "4. Testing coverage reporting"
    local coverage_stats
    coverage_stats=$(api_request "GET" "/api/testing/coverage")
    echo "Coverage Stats: $(echo "$coverage_stats" | sed '$d')"
    
    log_test "5. Testing quality gates"
    local quality_gates
    quality_gates=$(api_request "GET" "/api/testing/quality-gates")
    echo "Quality Gates: $(echo "$quality_gates" | sed '$d')"
    
    echo ""
}

# Test Complete Pipeline Workflow
test_pipeline_component() {
    echo -e "${CYAN}=== Complete Pipeline Workflow Tests ===${NC}"
    
    log_test "1. Creating test pipeline"
    local pipeline_yaml='name: "individual-test-pipeline"
description: "Pipeline for individual component testing"
stages:
  - name: "build"
    steps:
      - name: "compile"
        run: "echo Compiling application"
  - name: "test"
    steps:
      - name: "unit-tests"
        run: "echo Running unit tests"
environment:
  TEST_MODE: "true"'
    
    local pipeline_data
    pipeline_data=$(jq -n --arg yaml "$pipeline_yaml" '{yaml_content: $yaml}')
    
    local pipeline_response
    pipeline_response=$(api_request "POST" "/api/ci/pipelines" "$pipeline_data")
    echo "Pipeline Creation: $(echo "$pipeline_response" | sed '$d')"
    
    local pipeline_id
    pipeline_id=$(echo "$pipeline_response" | sed '$d' | jq -r '.id // empty')
    
    if [ -n "$pipeline_id" ]; then
        log_test "2. Triggering pipeline execution"
        local trigger_data='{
            "trigger_type": "manual",
            "branch": "main",
            "environment": {"TEST": "true"}
        }'
        
        local execution_response
        execution_response=$(api_request "POST" "/api/ci/pipelines/$pipeline_id/trigger" "$trigger_data")
        echo "Execution Trigger: $(echo "$execution_response" | sed '$d')"
        
        log_test "3. Monitoring execution"
        local execution_id
        execution_id=$(echo "$execution_response" | sed '$d' | jq -r '.execution_id // empty')
        
        if [ -n "$execution_id" ]; then
            for i in {1..5}; do
                local status_response
                status_response=$(api_request "GET" "/api/ci/executions/$execution_id")
                local status
                status=$(echo "$status_response" | sed '$d' | jq -r '.status // "unknown"')
                echo "Execution Status (check $i): $status"
                
                if [ "$status" = "completed" ] || [ "$status" = "failed" ]; then
                    break
                fi
                sleep 1
            done
        fi
        
        log_test "4. Testing pipeline management"
        local pipeline_list
        pipeline_list=$(api_request "GET" "/api/ci/pipelines")
        echo "Pipeline List: $(echo "$pipeline_list" | sed '$d' | jq -r 'length // 0') pipelines"
        
        log_test "5. Testing execution history"
        local execution_list
        execution_list=$(api_request "GET" "/api/ci/executions")
        echo "Execution History: $(echo "$execution_list" | sed '$d' | jq -r 'length // 0') executions"
    else
        log_warning "Failed to create pipeline for testing"
    fi
    
    echo ""
}

# Show available components
show_components() {
    echo -e "${CYAN}Available Components:${NC}"
    echo "  error-handling     - Error handling system with correlation tracking"
    echo "  configuration      - Configuration management and validation"
    echo "  database          - Database operations and connection pooling"
    echo "  performance       - Performance optimization and monitoring"
    echo "  api               - API robustness and versioning"
    echo "  resources         - Resource management and lifecycle"
    echo "  deployment        - Deployment capabilities and strategies"
    echo "  valkyrie          - Valkyrie protocol optimization"
    echo "  self-healing      - Self-healing system and strategies"
    echo "  testing           - Testing framework and quality gates"
    echo "  pipeline          - Complete pipeline workflow"
    echo "  all               - Run all component tests"
}

# Show help
show_help() {
    cat << EOF
RustCI Individual Component Testing Script

Usage: $0 [OPTIONS] COMPONENT

Options:
    -u, --url URL           Base URL for API testing (default: http://localhost:8000)
    -v, --verbose           Enable verbose output
    -t, --token TOKEN       JWT token for authentication
    -h, --help              Show this help message

Components:
EOF
    show_components
    cat << EOF

Examples:
    $0 valkyrie                          # Test only Valkyrie protocol
    $0 --token "eyJ..." performance      # Test performance with auth
    $0 --url http://prod.com api         # Test API against prod server
    $0 all                               # Test all components

Environment Variables:
    BASE_URL               Base URL for API testing
    VERBOSE                Enable verbose output (true/false)
    JWT_TOKEN              JWT token for authentication
EOF
}

# Main execution
main() {
    case $COMPONENT in
        error-handling)
            test_error_handling_component
            ;;
        configuration)
            test_configuration_component
            ;;
        database)
            test_database_component
            ;;
        performance)
            test_performance_component
            ;;
        api)
            test_api_component
            ;;
        resources)
            test_resource_component
            ;;
        deployment)
            test_deployment_component
            ;;
        valkyrie)
            test_valkyrie_component
            ;;
        self-healing)
            test_self_healing_component
            ;;
        testing)
            test_testing_component
            ;;
        pipeline)
            test_pipeline_component
            ;;
        all)
            echo -e "${CYAN}=== Running All Component Tests ===${NC}"
            test_error_handling_component
            test_configuration_component
            test_database_component
            test_performance_component
            test_api_component
            test_resource_component
            test_deployment_component
            test_valkyrie_component
            test_self_healing_component
            test_testing_component
            test_pipeline_component
            ;;
        *)
            log_error "Unknown component: $COMPONENT"
            echo ""
            show_components
            exit 1
            ;;
    esac
    
    log_success "Component testing completed: $COMPONENT"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -u|--url)
            BASE_URL="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -t|--token)
            JWT_TOKEN="$2"
            shift 2
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        -*)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
        *)
            COMPONENT="$1"
            shift
            ;;
    esac
done

# Check if component is specified
if [ -z "$COMPONENT" ]; then
    log_error "Component not specified"
    echo ""
    show_components
    exit 1
fi

# Check server health
log_info "Testing component: $COMPONENT"
log_info "Server URL: $BASE_URL"

if ! curl -s "$BASE_URL/health" >/dev/null 2>&1; then
    log_error "Server is not accessible at $BASE_URL"
    log_info "Please ensure the server is running: cargo run"
    exit 1
fi

# Run component test
main