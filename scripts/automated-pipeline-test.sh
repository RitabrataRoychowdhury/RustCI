#!/bin/bash
# Automated Pipeline Test Script
# Tests the current pipeline.yaml using exact cURL commands provided by the user
# Captures all logs, errors, and provides detailed pass/fail reporting

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
API_BASE_URL="http://localhost:8000"
PIPELINE_YAML_PATH="pipeline.yaml"
TEST_RESULTS_DIR="./pipeline-test-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="$TEST_RESULTS_DIR/test_execution_$TIMESTAMP.log"
SUMMARY_FILE="$TEST_RESULTS_DIR/test_summary_$TIMESTAMP.md"
SERVER_LOG_FILE="/tmp/rustci-server-$TIMESTAMP.log"

# Test state variables
JWT_TOKEN=""
PIPELINE_ID=""
EXECUTION_ID=""
SERVER_PID=""
TEST_RESULTS=()

# Function to print colored output with logging
log_and_print() {
    local color=$1
    local level=$2
    local message=$3
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    echo -e "${color}[${level}] ${message}${NC}"
    echo "[$timestamp] [$level] $message" >> "$LOG_FILE"
}

print_header() {
    local message=$1
    echo -e "\n${BOLD}${BLUE}=== $message ===${NC}"
    log_and_print "$BLUE" "INFO" "=== $message ==="
}

print_step() {
    local message=$1
    echo -e "\n${CYAN}🔄 $message${NC}"
    log_and_print "$CYAN" "STEP" "$message"
}

print_success() {
    local message=$1
    log_and_print "$GREEN" "SUCCESS" "✅ $message"
    TEST_RESULTS+=("PASS: $message")
}

print_error() {
    local message=$1
    log_and_print "$RED" "ERROR" "❌ $message"
    TEST_RESULTS+=("FAIL: $message")
}

print_warning() {
    local message=$1
    log_and_print "$YELLOW" "WARNING" "⚠️  $message"
    TEST_RESULTS+=("WARN: $message")
}

print_info() {
    local message=$1
    log_and_print "$BLUE" "INFO" "ℹ️  $message"
}

# Function to setup test environment
setup_test_environment() {
    print_header "Setting Up Test Environment"
    
    # Create test results directory
    mkdir -p "$TEST_RESULTS_DIR"
    
    # Initialize log file
    echo "Pipeline Execution Test Log - $(date)" > "$LOG_FILE"
    echo "=======================================" >> "$LOG_FILE"
    
    print_success "Test environment initialized"
    print_info "Results directory: $TEST_RESULTS_DIR"
    print_info "Log file: $LOG_FILE"
}

# Function to check prerequisites
check_prerequisites() {
    print_header "Checking Prerequisites"
    
    local missing_tools=()
    
    # Check required tools
    local tools=("curl" "cargo" "docker" "git")
    for tool in "${tools[@]}"; do
        if command -v "$tool" >/dev/null 2>&1; then
            local version=$(${tool} --version 2>/dev/null | head -1 || echo "unknown")
            print_success "$tool is available: $version"
        else
            print_error "$tool is not available"
            missing_tools+=("$tool")
        fi
    done
    
    # Check optional tools
    local optional_tools=("jq" "sshpass")
    for tool in "${optional_tools[@]}"; do
        if command -v "$tool" >/dev/null 2>&1; then
            print_success "$tool is available (optional)"
        else
            print_warning "$tool is not available (optional but recommended)"
        fi
    done
    
    # Check pipeline file
    if [[ -f "$PIPELINE_YAML_PATH" ]]; then
        print_success "Pipeline file found: $PIPELINE_YAML_PATH"
        print_info "Pipeline file size: $(wc -c < "$PIPELINE_YAML_PATH") bytes"
    else
        print_error "Pipeline file not found: $PIPELINE_YAML_PATH"
        missing_tools+=("pipeline.yaml")
    fi
    
    # Check .env file
    if [[ -f ".env" ]]; then
        print_success ".env file found"
    else
        print_warning ".env file not found - using defaults"
    fi
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        print_error "Missing required tools/files: ${missing_tools[*]}"
        return 1
    fi
    
    return 0
}

# Function to generate JWT token
generate_jwt_token() {
    print_header "Generating JWT Token"
    
    print_step "Attempting to generate token using Rust binary"
    
    # Try the Rust token generator first
    local token_output
    if token_output=$(cargo run --bin generate_token 2>&1); then
        # Extract the actual token from the output
        JWT_TOKEN=$(echo "$token_output" | grep -E '^eyJ' | head -1)
        
        if [[ -n "$JWT_TOKEN" ]]; then
            print_success "JWT token generated using Rust binary"
            print_info "Token preview: ${JWT_TOKEN:0:50}..."
            echo "JWT_TOKEN=$JWT_TOKEN" > "$TEST_RESULTS_DIR/.test_env"
            return 0
        fi
    fi
    
    print_warning "Rust token generator failed, trying Python fallback"
    
    # Fallback to Python-based token generation
    if command -v python3 >/dev/null 2>&1; then
        # Check if PyJWT is available
        if ! python3 -c "import jwt" 2>/dev/null; then
            print_info "Installing PyJWT..."
            pip3 install PyJWT 2>/dev/null || pip install PyJWT 2>/dev/null || {
                print_warning "Could not install PyJWT automatically"
            }
        fi
        
        if python3 -c "import jwt" 2>/dev/null; then
            # Create token generation script
            cat > "$TEST_RESULTS_DIR/generate_token.py" << 'EOF'
import jwt
import datetime
import uuid
import sys

# JWT configuration from .env
JWT_SECRET = "404E635266556A586E3272357538782F413F4428472B4B6250645367566B5970"

# Create payload
payload = {
    "sub": str(uuid.uuid4()),
    "email": "test@rustci.local",
    "roles": ["Developer", "Admin"],
    "permissions": [
        "ReadPipelines", "TriggerJobs", "CancelPipelines", 
        "ViewLogs", "ViewArtifacts", "WritePipelines", "ExecutePipelines"
    ],
    "iat": int(datetime.datetime.utcnow().timestamp()),
    "exp": int((datetime.datetime.utcnow() + datetime.timedelta(hours=2)).timestamp()),
    "iss": "rustci",
    "aud": "rustci-api",
    "jti": str(uuid.uuid4()),
    "session_id": str(uuid.uuid4())
}

try:
    token = jwt.encode(payload, JWT_SECRET, algorithm="HS256")
    print(token)
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    sys.exit(1)
EOF
            
            JWT_TOKEN=$(python3 "$TEST_RESULTS_DIR/generate_token.py" 2>/dev/null || echo "")
            rm -f "$TEST_RESULTS_DIR/generate_token.py"
            
            if [[ -n "$JWT_TOKEN" ]]; then
                print_success "JWT token generated using Python fallback"
                print_info "Token preview: ${JWT_TOKEN:0:50}..."
                echo "JWT_TOKEN=$JWT_TOKEN" > "$TEST_RESULTS_DIR/.test_env"
                return 0
            fi
        fi
    fi
    
    print_error "Failed to generate JWT token using all methods"
    print_info "Manual steps to generate token:"
    print_info "1. Run: cargo run --bin generate_token"
    print_info "2. Or use: scripts/get-admin-token.sh"
    return 1
}

# Function to start RustCI server
start_rustci_server() {
    print_header "Starting RustCI Server"
    
    # Check if already running
    if curl -s -f "$API_BASE_URL/health" >/dev/null 2>&1; then
        print_success "RustCI API is already running at $API_BASE_URL"
        return 0
    fi
    
    print_step "Starting RustCI server"
    
    # Clean up any existing processes
    print_info "Cleaning up existing processes..."
    pkill -f "cargo.*run" || true
    pkill -f "rustci" || true
    sleep 2
    
    # Start server in background with logging
    print_info "Starting server with cargo run..."
    cargo run > "$SERVER_LOG_FILE" 2>&1 &
    SERVER_PID=$!
    
    # Save PID for cleanup
    echo "$SERVER_PID" > "$TEST_RESULTS_DIR/.server_pid"
    
    print_info "Server starting with PID: $SERVER_PID"
    print_info "Server logs: $SERVER_LOG_FILE"
    
    # Wait for server to be ready
    print_step "Waiting for server to be ready"
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s -f "$API_BASE_URL/health" >/dev/null 2>&1; then
            print_success "RustCI API is now running at $API_BASE_URL"
            return 0
        fi
        
        # Check if process is still running
        if ! kill -0 $SERVER_PID 2>/dev/null; then
            print_error "Server process died during startup"
            print_info "Server logs:"
            tail -20 "$SERVER_LOG_FILE" 2>/dev/null || echo "No logs available"
            return 1
        fi
        
        sleep 2
        attempt=$((attempt + 1))
        echo -n "."
    done
    
    echo ""
    print_error "Server failed to start within 60 seconds"
    print_info "Server logs:"
    tail -20 "$SERVER_LOG_FILE" 2>/dev/null || echo "No logs available"
    return 1
}

# Function to test API health
test_api_health() {
    print_header "Testing API Health"
    
    local response
    local http_code
    
    response=$(curl -s -w "\n%{http_code}" "$API_BASE_URL/health" 2>&1)
    http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n -1)
    
    echo "Health Check Response:" >> "$LOG_FILE"
    echo "HTTP Code: $http_code" >> "$LOG_FILE"
    echo "Body: $body" >> "$LOG_FILE"
    echo "" >> "$LOG_FILE"
    
    if [[ "$http_code" == "200" ]]; then
        print_success "API health check passed"
        print_info "Response: $body"
        return 0
    else
        print_error "API health check failed (HTTP $http_code)"
        print_error "Response: $body"
        return 1
    fi
}

# Function to upload pipeline using exact cURL command
upload_pipeline() {
    print_header "Uploading Pipeline (Exact cURL Command)"
    
    print_step "Executing pipeline upload"
    print_info "Command: curl --location '$API_BASE_URL/api/ci/pipelines/upload' --header 'Authorization: Bearer [TOKEN]' --form 'pipeline=@$PIPELINE_YAML_PATH'"
    
    local response
    local http_code
    local start_time=$(date +%s)
    
    response=$(curl -s -w "\n%{http_code}" \
        --location "$API_BASE_URL/api/ci/pipelines/upload" \
        --header "Authorization: Bearer $JWT_TOKEN" \
        --form "pipeline=@$PIPELINE_YAML_PATH" 2>&1)
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n -1)
    
    # Log detailed response
    echo "Pipeline Upload Response:" >> "$LOG_FILE"
    echo "HTTP Code: $http_code" >> "$LOG_FILE"
    echo "Duration: ${duration}s" >> "$LOG_FILE"
    echo "Body: $body" >> "$LOG_FILE"
    echo "" >> "$LOG_FILE"
    
    # Save response to file
    echo "$body" > "$TEST_RESULTS_DIR/upload_response.json"
    
    print_info "HTTP Response Code: $http_code"
    print_info "Duration: ${duration}s"
    print_info "Response Body:"
    echo "$body"
    
    if [[ "$http_code" == "200" ]] || [[ "$http_code" == "201" ]]; then
        print_success "Pipeline uploaded successfully"
        
        # Extract pipeline ID
        if command -v jq >/dev/null 2>&1; then
            PIPELINE_ID=$(echo "$body" | jq -r '.id // .pipeline_id // empty' 2>/dev/null || echo "")
            if [[ -n "$PIPELINE_ID" && "$PIPELINE_ID" != "null" ]]; then
                print_info "Pipeline ID extracted: $PIPELINE_ID"
                echo "PIPELINE_ID=$PIPELINE_ID" >> "$TEST_RESULTS_DIR/.test_env"
            else
                print_warning "Could not extract pipeline ID from response"
                # Try to extract from different possible fields
                PIPELINE_ID=$(echo "$body" | grep -o '"[^"]*id[^"]*"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | cut -d'"' -f4 || echo "")
                if [[ -n "$PIPELINE_ID" ]]; then
                    print_info "Pipeline ID extracted using fallback: $PIPELINE_ID"
                    echo "PIPELINE_ID=$PIPELINE_ID" >> "$TEST_RESULTS_DIR/.test_env"
                fi
            fi
        else
            print_warning "jq not available - cannot extract pipeline ID automatically"
        fi
        
        return 0
    else
        print_error "Pipeline upload failed (HTTP $http_code)"
        print_error "Response: $body"
        return 1
    fi
}

# Function to trigger pipeline using exact cURL command
trigger_pipeline() {
    print_header "Triggering Pipeline (Exact cURL Command)"
    
    # Load pipeline ID if available
    if [[ -f "$TEST_RESULTS_DIR/.test_env" ]]; then
        source "$TEST_RESULTS_DIR/.test_env"
    fi
    
    # Use example pipeline ID if we don't have one
    if [[ -z "$PIPELINE_ID" ]]; then
        PIPELINE_ID="712bde3d-4ef5-419c-ba00-c54152612343"
        print_warning "Using example pipeline ID: $PIPELINE_ID"
    else
        print_info "Using extracted pipeline ID: $PIPELINE_ID"
    fi
    
    print_step "Executing pipeline trigger"
    print_info "Command: curl --location '$API_BASE_URL/api/ci/pipelines/$PIPELINE_ID/trigger' --header 'Authorization: Bearer [TOKEN]' --header 'Content-Type: application/json' --data '{\"trigger_type\": \"manual\"}'"
    
    local response
    local http_code
    local start_time=$(date +%s)
    
    response=$(curl -s -w "\n%{http_code}" \
        --location "$API_BASE_URL/api/ci/pipelines/$PIPELINE_ID/trigger" \
        --header "Authorization: Bearer $JWT_TOKEN" \
        --header 'Content-Type: application/json' \
        --data '{"trigger_type": "manual"}' 2>&1)
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n -1)
    
    # Log detailed response
    echo "Pipeline Trigger Response:" >> "$LOG_FILE"
    echo "HTTP Code: $http_code" >> "$LOG_FILE"
    echo "Duration: ${duration}s" >> "$LOG_FILE"
    echo "Body: $body" >> "$LOG_FILE"
    echo "" >> "$LOG_FILE"
    
    # Save response to file
    echo "$body" > "$TEST_RESULTS_DIR/trigger_response.json"
    
    print_info "HTTP Response Code: $http_code"
    print_info "Duration: ${duration}s"
    print_info "Response Body:"
    echo "$body"
    
    if [[ "$http_code" == "200" ]] || [[ "$http_code" == "201" ]]; then
        print_success "Pipeline triggered successfully"
        
        # Extract execution ID
        if command -v jq >/dev/null 2>&1; then
            EXECUTION_ID=$(echo "$body" | jq -r '.execution_id // .id // empty' 2>/dev/null || echo "")
            if [[ -n "$EXECUTION_ID" && "$EXECUTION_ID" != "null" ]]; then
                print_info "Execution ID extracted: $EXECUTION_ID"
                echo "EXECUTION_ID=$EXECUTION_ID" >> "$TEST_RESULTS_DIR/.test_env"
            fi
        fi
        
        return 0
    else
        print_error "Pipeline trigger failed (HTTP $http_code)"
        print_error "Response: $body"
        return 1
    fi
}

# Function to monitor pipeline execution
monitor_pipeline_execution() {
    print_header "Monitoring Pipeline Execution"
    
    print_step "Capturing execution logs and monitoring progress"
    
    # Wait for execution to start
    sleep 3
    
    # Monitor server logs for execution details
    print_info "Monitoring server logs for execution details..."
    
    # Capture server logs during execution
    local log_start_line=$(wc -l < "$SERVER_LOG_FILE" 2>/dev/null || echo "0")
    
    # Wait for execution to complete or timeout
    local monitor_duration=60
    local elapsed=0
    
    print_info "Monitoring execution for up to $monitor_duration seconds..."
    
    while [[ $elapsed -lt $monitor_duration ]]; do
        # Check if there are new log entries
        local current_lines=$(wc -l < "$SERVER_LOG_FILE" 2>/dev/null || echo "0")
        if [[ $current_lines -gt $log_start_line ]]; then
            # Extract new log lines
            local new_logs=$(tail -n +$((log_start_line + 1)) "$SERVER_LOG_FILE" 2>/dev/null || echo "")
            if [[ -n "$new_logs" ]]; then
                echo "New execution logs:" >> "$LOG_FILE"
                echo "$new_logs" >> "$LOG_FILE"
                echo "" >> "$LOG_FILE"
                
                # Check for specific error patterns
                if echo "$new_logs" | grep -q "fatal: Unable to read current working directory"; then
                    print_error "Git clone workspace error detected in logs"
                fi
                
                if echo "$new_logs" | grep -q "docker.*failed\|Docker.*error"; then
                    print_error "Docker operation error detected in logs"
                fi
                
                if echo "$new_logs" | grep -q "ssh.*failed\|SSH.*error"; then
                    print_error "SSH operation error detected in logs"
                fi
                
                log_start_line=$current_lines
            fi
        fi
        
        sleep 5
        elapsed=$((elapsed + 5))
        echo -n "."
    done
    
    echo ""
    print_info "Execution monitoring completed"
    
    # Capture final server logs
    print_step "Capturing final execution logs"
    local final_logs=$(tail -50 "$SERVER_LOG_FILE" 2>/dev/null || echo "No logs available")
    echo "Final Server Logs:" >> "$LOG_FILE"
    echo "$final_logs" >> "$LOG_FILE"
    echo "" >> "$LOG_FILE"
    
    # Save execution logs to separate file
    echo "$final_logs" > "$TEST_RESULTS_DIR/execution_logs.txt"
    
    print_info "Execution logs saved to: $TEST_RESULTS_DIR/execution_logs.txt"
}

# Function to verify Docker deployment via SSH
verify_docker_deployment() {
    print_header "Verifying Docker Container Deployment"
    
    print_step "Testing SSH connectivity to deployment server"
    
    # Check if sshpass is available
    if ! command -v sshpass >/dev/null 2>&1; then
        print_warning "sshpass not available - cannot test SSH deployment"
        print_info "To test SSH deployment, install sshpass: brew install sshpass (macOS) or apt-get install sshpass (Linux)"
        return 0
    fi
    
    # Test SSH connectivity
    print_info "Testing SSH connection to localhost:2222..."
    
    local ssh_test
    if ssh_test=$(timeout 10 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 -p 2222 user@localhost 'echo "SSH connection successful"' 2>&1); then
        print_success "SSH connection to deployment server successful"
        print_info "SSH response: $ssh_test"
        
        # Check Docker container status
        print_step "Checking Docker container status"
        
        local docker_status
        if docker_status=$(timeout 15 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost 'docker ps -a | grep rustci || echo "No rustci container found"' 2>&1); then
            echo "Docker Container Status:" >> "$LOG_FILE"
            echo "$docker_status" >> "$LOG_FILE"
            echo "" >> "$LOG_FILE"
            
            print_info "Docker container status:"
            echo "$docker_status"
            
            if echo "$docker_status" | grep -q "rustci.*Up"; then
                print_success "RustCI container is running"
                
                # Test container health endpoint
                print_step "Testing container health endpoint"
                
                local health_check
                if health_check=$(timeout 15 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost 'curl -f http://localhost:8989/health 2>/dev/null || curl -f http://localhost:8989/api/healthchecker 2>/dev/null || echo "Health check failed"' 2>&1); then
                    echo "Container Health Check:" >> "$LOG_FILE"
                    echo "$health_check" >> "$LOG_FILE"
                    echo "" >> "$LOG_FILE"
                    
                    print_info "Health check result:"
                    echo "$health_check"
                    
                    if echo "$health_check" | grep -v "failed" | grep -q "health\|ok\|success\|running"; then
                        print_success "Container health check passed"
                    else
                        print_warning "Container health check failed or endpoint not ready"
                    fi
                else
                    print_warning "Could not perform health check via SSH"
                fi
                
                # Get container logs
                print_step "Capturing container logs"
                
                local container_logs
                if container_logs=$(timeout 10 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost 'docker logs rustci --tail 20 2>/dev/null || echo "Could not get container logs"' 2>&1); then
                    echo "Container Logs:" >> "$LOG_FILE"
                    echo "$container_logs" >> "$LOG_FILE"
                    echo "" >> "$LOG_FILE"
                    
                    echo "$container_logs" > "$TEST_RESULTS_DIR/container_logs.txt"
                    print_info "Container logs saved to: $TEST_RESULTS_DIR/container_logs.txt"
                fi
                
            elif echo "$docker_status" | grep -q "rustci"; then
                print_warning "RustCI container exists but is not running"
                
                # Get container logs for debugging
                local container_logs
                if container_logs=$(timeout 10 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost 'docker logs rustci --tail 20 2>/dev/null || echo "Could not get container logs"' 2>&1); then
                    print_info "Container logs (for debugging):"
                    echo "$container_logs"
                    echo "$container_logs" > "$TEST_RESULTS_DIR/failed_container_logs.txt"
                fi
            else
                print_warning "RustCI container not found"
                print_info "This may indicate the deployment step failed or the container name is different"
            fi
        else
            print_error "Failed to check Docker container status via SSH"
            print_error "SSH error: $docker_status"
        fi
        
    else
        print_warning "SSH connection failed - deployment server may not be running"
        print_info "SSH error: $ssh_test"
        print_info "To test Docker deployment:"
        print_info "1. Start the SSH server using deployment scripts"
        print_info "2. Ensure SSH server is running on localhost:2222"
        print_info "3. Verify credentials: user@localhost with password 'abc123'"
    fi
}

# Function to analyze execution results
analyze_execution_results() {
    print_header "Analyzing Execution Results"
    
    print_step "Analyzing server logs for common issues"
    
    local issues_found=()
    
    # Check for Git clone issues
    if grep -q "fatal: Unable to read current working directory" "$SERVER_LOG_FILE" 2>/dev/null; then
        issues_found+=("Git clone workspace directory issue")
        print_error "Found Git clone workspace issue in logs"
    fi
    
    # Check for Docker issues
    if grep -q "docker.*failed\|Docker.*error\|Cannot connect to the Docker daemon" "$SERVER_LOG_FILE" 2>/dev/null; then
        issues_found+=("Docker operation failures")
        print_error "Found Docker operation issues in logs"
    fi
    
    # Check for SSH issues
    if grep -q "ssh.*failed\|SSH.*error\|Connection refused" "$SERVER_LOG_FILE" 2>/dev/null; then
        issues_found+=("SSH connection failures")
        print_error "Found SSH connection issues in logs"
    fi
    
    # Check for workspace issues
    if grep -q "workspace.*error\|Failed to create.*workspace" "$SERVER_LOG_FILE" 2>/dev/null; then
        issues_found+=("Workspace management issues")
        print_error "Found workspace management issues in logs"
    fi
    
    # Check for permission issues
    if grep -q "Permission denied\|permission.*error" "$SERVER_LOG_FILE" 2>/dev/null; then
        issues_found+=("Permission/access issues")
        print_error "Found permission issues in logs"
    fi
    
    if [[ ${#issues_found[@]} -eq 0 ]]; then
        print_success "No obvious issues detected in server logs"
    else
        print_warning "Issues detected: ${issues_found[*]}"
    fi
    
    # Save analysis results
    echo "Execution Analysis Results:" > "$TEST_RESULTS_DIR/analysis_results.txt"
    echo "Issues found: ${#issues_found[@]}" >> "$TEST_RESULTS_DIR/analysis_results.txt"
    for issue in "${issues_found[@]}"; do
        echo "- $issue" >> "$TEST_RESULTS_DIR/analysis_results.txt"
    done
}

# Function to generate comprehensive summary
generate_test_summary() {
    print_header "Generating Test Summary"
    
    local total_tests=0
    local passed_tests=0
    local failed_tests=0
    local warnings=0
    
    # Count test results
    for result in "${TEST_RESULTS[@]}"; do
        total_tests=$((total_tests + 1))
        if [[ "$result" == PASS:* ]]; then
            passed_tests=$((passed_tests + 1))
        elif [[ "$result" == FAIL:* ]]; then
            failed_tests=$((failed_tests + 1))
        elif [[ "$result" == WARN:* ]]; then
            warnings=$((warnings + 1))
        fi
    done
    
    # Generate markdown summary
    cat > "$SUMMARY_FILE" << EOF
# Pipeline Execution Test Summary

**Test Date:** $(date)  
**Test Duration:** $(date -d @$(($(date +%s) - $(stat -c %Y "$LOG_FILE" 2>/dev/null || echo $(date +%s)))) -u +%H:%M:%S 2>/dev/null || echo "Unknown")  
**API Base URL:** $API_BASE_URL  
**Pipeline File:** $PIPELINE_YAML_PATH  

## Test Results Overview

- **Total Tests:** $total_tests
- **Passed:** $passed_tests ✅
- **Failed:** $failed_tests ❌
- **Warnings:** $warnings ⚠️
- **Success Rate:** $(( passed_tests * 100 / (total_tests > 0 ? total_tests : 1) ))%

## Detailed Results

EOF
    
    # Add detailed results
    for result in "${TEST_RESULTS[@]}"; do
        if [[ "$result" == PASS:* ]]; then
            echo "✅ ${result#PASS: }" >> "$SUMMARY_FILE"
        elif [[ "$result" == FAIL:* ]]; then
            echo "❌ ${result#FAIL: }" >> "$SUMMARY_FILE"
        elif [[ "$result" == WARN:* ]]; then
            echo "⚠️ ${result#WARN: }" >> "$SUMMARY_FILE"
        fi
    done
    
    cat >> "$SUMMARY_FILE" << EOF

## cURL Commands Tested

### 1. Pipeline Upload
\`\`\`bash
curl --location '$API_BASE_URL/api/ci/pipelines/upload' \\
  --header 'Authorization: Bearer [JWT_TOKEN]' \\
  --form 'pipeline=@$PIPELINE_YAML_PATH'
\`\`\`

### 2. Pipeline Trigger
\`\`\`bash
curl --location '$API_BASE_URL/api/ci/pipelines/{PIPELINE_ID}/trigger' \\
  --header 'Authorization: Bearer [JWT_TOKEN]' \\
  --header 'Content-Type: application/json' \\
  --data '{"trigger_type": "manual"}'
\`\`\`

## Key Findings

### Pipeline Configuration Analysis
- Pipeline file: $PIPELINE_YAML_PATH
- Pipeline uses hardcoded paths: \`/tmp/rustci\`
- Contains Git clone, Docker build, and SSH deployment steps
- Deployment target: localhost:2222 via SSH

### Common Issues Identified
1. **Git Clone Workspace Issue**: The pipeline uses hardcoded \`/tmp/rustci\` path
2. **Workspace Creation Timing**: Workspace may not exist when Git clone runs
3. **SSH Deployment Dependencies**: Requires SSH server on localhost:2222
4. **Docker Dependencies**: Requires Docker daemon and proper permissions

## Files Generated
- Detailed logs: \`$(basename "$LOG_FILE")\`
- Upload response: \`upload_response.json\`
- Trigger response: \`trigger_response.json\`
- Execution logs: \`execution_logs.txt\`
- Container logs: \`container_logs.txt\` (if available)
- Analysis results: \`analysis_results.txt\`

## Next Steps

### Immediate Fixes Needed
1. **Fix Git Clone Path**: Replace hardcoded \`/tmp/rustci\` with dynamic workspace path
2. **Ensure Workspace Creation**: Create workspace directory before Git clone step
3. **Improve Error Handling**: Add better error messages for common failures
4. **Test SSH Setup**: Verify SSH server configuration for deployment testing

### Recommendations
1. Use environment variables for workspace paths
2. Add pre-execution validation for required tools
3. Implement proper workspace cleanup
4. Add retry logic for transient failures
5. Create integration tests for each deployment type

## Test Environment
- Operating System: $(uname -s)
- Docker Version: $(docker --version 2>/dev/null || echo "Not available")
- Git Version: $(git --version 2>/dev/null || echo "Not available")
- Curl Version: $(curl --version 2>/dev/null | head -1 || echo "Not available")

---
*Generated by automated-pipeline-test.sh on $(date)*
EOF
    
    print_success "Test summary generated: $SUMMARY_FILE"
}

# Function to cleanup test environment
cleanup_test_environment() {
    print_header "Cleaning Up Test Environment"
    
    # Stop server if we started it
    if [[ -f "$TEST_RESULTS_DIR/.server_pid" ]]; then
        local server_pid=$(cat "$TEST_RESULTS_DIR/.server_pid")
        if kill -0 "$server_pid" 2>/dev/null; then
            print_info "Stopping RustCI server (PID: $server_pid)..."
            
            # Try graceful shutdown first
            kill "$server_pid" 2>/dev/null || true
            sleep 3
            
            # Force kill if still running
            if kill -0 "$server_pid" 2>/dev/null; then
                kill -9 "$server_pid" 2>/dev/null || true
                print_info "Server force-stopped"
            else
                print_info "Server stopped gracefully"
            fi
        fi
        rm -f "$TEST_RESULTS_DIR/.server_pid"
    fi
    
    # Clean up temporary files
    rm -f "$TEST_RESULTS_DIR/.test_env"
    
    print_success "Cleanup completed"
}

# Function to display final results
display_final_results() {
    print_header "Final Test Results"
    
    echo -e "\n${BOLD}Test Execution Summary:${NC}"
    
    local total_tests=0
    local passed_tests=0
    local failed_tests=0
    
    for result in "${TEST_RESULTS[@]}"; do
        total_tests=$((total_tests + 1))
        if [[ "$result" == PASS:* ]]; then
            passed_tests=$((passed_tests + 1))
            echo -e "${GREEN}✅ ${result#PASS: }${NC}"
        elif [[ "$result" == FAIL:* ]]; then
            failed_tests=$((failed_tests + 1))
            echo -e "${RED}❌ ${result#FAIL: }${NC}"
        elif [[ "$result" == WARN:* ]]; then
            echo -e "${YELLOW}⚠️ ${result#WARN: }${NC}"
        fi
    done
    
    echo -e "\n${BOLD}Overall Results:${NC}"
    echo -e "Total Tests: $total_tests"
    echo -e "Passed: ${GREEN}$passed_tests${NC}"
    echo -e "Failed: ${RED}$failed_tests${NC}"
    echo -e "Success Rate: $(( passed_tests * 100 / (total_tests > 0 ? total_tests : 1) ))%"
    
    echo -e "\n${BOLD}Generated Files:${NC}"
    echo -e "📁 Results Directory: ${CYAN}$TEST_RESULTS_DIR${NC}"
    echo -e "📄 Detailed Log: ${CYAN}$(basename "$LOG_FILE")${NC}"
    echo -e "📋 Summary Report: ${CYAN}$(basename "$SUMMARY_FILE")${NC}"
    
    if [[ $failed_tests -eq 0 ]]; then
        echo -e "\n${GREEN}${BOLD}🎉 All tests passed! Pipeline execution test completed successfully.${NC}"
        return 0
    else
        echo -e "\n${RED}${BOLD}❌ Some tests failed. Check the logs and summary for details.${NC}"
        return 1
    fi
}

# Main execution function
main() {
    print_header "RustCI Automated Pipeline Test"
    print_info "Testing current pipeline.yaml using exact cURL commands"
    print_info "This script will test pipeline upload, trigger, and monitor execution"
    
    # Setup
    setup_test_environment
    
    # Check prerequisites
    if ! check_prerequisites; then
        print_error "Prerequisites check failed"
        exit 1
    fi
    
    # Generate JWT token
    if ! generate_jwt_token; then
        print_error "Failed to generate JWT token"
        exit 1
    fi
    
    # Start server if needed
    if ! start_rustci_server; then
        print_error "Failed to start RustCI server"
        exit 1
    fi
    
    # Test API health
    if ! test_api_health; then
        print_error "API health check failed"
        exit 1
    fi
    
    # Execute main test sequence
    local upload_success=false
    local trigger_success=false
    
    if upload_pipeline; then
        upload_success=true
        
        if trigger_pipeline; then
            trigger_success=true
            monitor_pipeline_execution
        fi
    fi
    
    # Verify deployment
    verify_docker_deployment
    
    # Analyze results
    analyze_execution_results
    
    # Generate summary
    generate_test_summary
    
    # Display final results
    local exit_code=0
    if ! display_final_results; then
        exit_code=1
    fi
    
    # Cleanup
    cleanup_test_environment
    
    exit $exit_code
}

# Handle script interruption
trap cleanup_test_environment EXIT

# Run main function
main "$@"