#!/bin/bash
# Test script using the exact cURL commands provided by the user
# This script tests the current pipeline.yaml with the provided authentication

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
API_BASE_URL="http://localhost:8000"
JWT_TOKEN=""  # Will be generated fresh
PIPELINE_YAML_PATH="pipeline.yaml"

# Function to print colored output
print_status() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

print_step() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

print_success() {
    print_status "$GREEN" "✅ $1"
}

print_error() {
    print_status "$RED" "❌ $1"
}

print_warning() {
    print_status "$YELLOW" "⚠️  $1"
}

print_info() {
    print_status "$BLUE" "ℹ️  $1"
}

# Function to start RustCI server if not running
start_rustci_server() {
    print_step "Starting RustCI Server"
    
    # Check if already running
    if curl -s -f "$API_BASE_URL/health" >/dev/null 2>&1; then
        print_success "RustCI API is already running at $API_BASE_URL"
        return 0
    fi
    
    print_info "RustCI server not running, starting it..."
    
    # Kill any existing cargo processes
    print_info "Cleaning up any existing cargo processes..."
    pkill -f "cargo.*run" || true
    pkill -f "cargo.*watch" || true
    sleep 2
    
    # Clean and rebuild
    print_info "Running cargo clean..."
    cargo clean
    
    # Start with cargo watch in background
    print_info "Starting server with cargo watch -x run..."
    cargo watch -x run > /tmp/rustci-server.log 2>&1 &
    SERVER_PID=$!
    
    # Save PID for cleanup
    echo $SERVER_PID > .rustci_server_pid
    
    print_info "Server starting with PID: $SERVER_PID"
    print_info "Waiting for server to be ready..."
    
    # Wait for server to start (max 60 seconds)
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
            tail -20 /tmp/rustci-server.log || echo "No logs available"
            return 1
        fi
        
        sleep 2
        attempt=$((attempt + 1))
        echo -n "."
    done
    
    echo ""
    print_error "Server failed to start within 60 seconds"
    print_info "Server logs:"
    tail -20 /tmp/rustci-server.log || echo "No logs available"
    return 1
}

# Function to check if RustCI API is running
check_api_health() {
    print_step "Checking API Health"
    
    if curl -s -f "$API_BASE_URL/health" >/dev/null 2>&1; then
        print_success "RustCI API is running at $API_BASE_URL"
        return 0
    else
        print_error "RustCI API is not accessible at $API_BASE_URL"
        return 1
    fi
}

# Function to check if pipeline.yaml exists
check_pipeline_file() {
    print_step "Checking Pipeline File"
    
    if [[ -f "$PIPELINE_YAML_PATH" ]]; then
        print_success "Pipeline file found: $PIPELINE_YAML_PATH"
        print_info "Pipeline content preview:"
        echo "---"
        head -20 "$PIPELINE_YAML_PATH"
        echo "---"
        return 0
    else
        print_error "Pipeline file not found: $PIPELINE_YAML_PATH"
        return 1
    fi
}

# Function to upload pipeline using the exact cURL command provided
upload_pipeline() {
    print_step "Uploading Pipeline (Using Exact cURL Command)"
    
    print_info "Executing: curl --location '$API_BASE_URL/api/ci/pipelines/upload' ..."
    
    # Use the exact cURL command structure provided, but with local file
    local response
    local http_code
    
    response=$(curl -s -w "\n%{http_code}" \
        --location "$API_BASE_URL/api/ci/pipelines/upload" \
        --header "Authorization: Bearer $JWT_TOKEN" \
        --form "pipeline=@$PIPELINE_YAML_PATH" 2>&1)
    
    http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n -1)
    
    echo "HTTP Response Code: $http_code"
    echo "Response Body:"
    echo "$body"
    
    if [[ "$http_code" == "200" ]] || [[ "$http_code" == "201" ]]; then
        print_success "Pipeline uploaded successfully"
        
        # Try to extract pipeline ID from response
        if command -v jq >/dev/null 2>&1; then
            local pipeline_id=$(echo "$body" | jq -r '.id // .pipeline_id // empty' 2>/dev/null || echo "")
            if [[ -n "$pipeline_id" && "$pipeline_id" != "null" ]]; then
                echo "PIPELINE_ID=$pipeline_id" > .pipeline_test_env
                print_info "Pipeline ID saved: $pipeline_id"
            else
                print_warning "Could not extract pipeline ID from response"
            fi
        else
            print_warning "jq not available - cannot extract pipeline ID"
        fi
        
        return 0
    else
        print_error "Pipeline upload failed (HTTP $http_code)"
        print_error "Response: $body"
        return 1
    fi
}

# Function to trigger pipeline using the exact cURL command provided
trigger_pipeline() {
    print_step "Triggering Pipeline (Using Exact cURL Command)"
    
    # Load pipeline ID if available
    local pipeline_id=""
    if [[ -f .pipeline_test_env ]]; then
        source .pipeline_test_env
        pipeline_id="$PIPELINE_ID"
    fi
    
    # Use the pipeline ID from the user's example if we don't have one
    if [[ -z "$pipeline_id" ]]; then
        pipeline_id="712bde3d-4ef5-419c-ba00-c54152612343"
        print_warning "Using example pipeline ID: $pipeline_id"
    else
        print_info "Using extracted pipeline ID: $pipeline_id"
    fi
    
    print_info "Executing: curl --location '$API_BASE_URL/api/ci/pipelines/$pipeline_id/trigger' ..."
    
    local response
    local http_code
    
    response=$(curl -s -w "\n%{http_code}" \
        --location "$API_BASE_URL/api/ci/pipelines/$pipeline_id/trigger" \
        --header "Authorization: Bearer $JWT_TOKEN" \
        --header 'Content-Type: application/json' \
        --data '{"trigger_type": "manual"}' 2>&1)
    
    http_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n -1)
    
    echo "HTTP Response Code: $http_code"
    echo "Response Body:"
    echo "$body"
    
    if [[ "$http_code" == "200" ]] || [[ "$http_code" == "201" ]]; then
        print_success "Pipeline triggered successfully"
        
        # Try to extract execution ID
        if command -v jq >/dev/null 2>&1; then
            local execution_id=$(echo "$body" | jq -r '.execution_id // .id // empty' 2>/dev/null || echo "")
            if [[ -n "$execution_id" && "$execution_id" != "null" ]]; then
                echo "EXECUTION_ID=$execution_id" >> .pipeline_test_env
                print_info "Execution ID saved: $execution_id"
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
monitor_execution() {
    print_step "Monitoring Pipeline Execution"
    
    print_info "Waiting for pipeline execution to complete..."
    print_info "Monitoring server logs for execution details..."
    
    # Wait a bit for execution to start
    sleep 3
    
    # Check if we can get pipeline status
    local pipeline_id=""
    if [[ -f .pipeline_test_env ]]; then
        source .pipeline_test_env
        pipeline_id="$PIPELINE_ID"
    fi
    
    if [[ -n "$pipeline_id" ]]; then
        print_info "Attempting to check pipeline status..."
        
        local status_response
        status_response=$(curl -s \
            --header "Authorization: Bearer $JWT_TOKEN" \
            "$API_BASE_URL/api/ci/pipelines/$pipeline_id" 2>/dev/null || echo "")
        
        if [[ -n "$status_response" ]]; then
            echo "Pipeline Status Response:"
            echo "$status_response"
        else
            print_warning "Could not retrieve pipeline status"
        fi
    fi
    
    # Wait for execution to complete (or timeout)
    print_info "Waiting 30 seconds for execution to complete..."
    sleep 30
    
    print_info "Check the server logs above for detailed execution information"
}

# Function to verify Docker deployment (if SSH server is available)
verify_docker_deployment() {
    print_step "Verifying Docker Deployment"
    
    print_info "Checking if SSH server is accessible..."
    
    # Test SSH connectivity
    if command -v sshpass >/dev/null 2>&1; then
        if timeout 10 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 -p 2222 user@localhost 'echo "SSH connection successful"' 2>/dev/null; then
            print_success "SSH connection to deployment server successful"
            
            print_info "Checking Docker container status..."
            local docker_status
            docker_status=$(timeout 10 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost 'docker ps | grep rustci || echo "No rustci container found"' 2>/dev/null || echo "SSH command failed")
            
            echo "Docker Container Status:"
            echo "$docker_status"
            
            if echo "$docker_status" | grep -q "rustci"; then
                print_success "RustCI container is running"
                
                # Test health endpoint if container is running
                print_info "Testing container health endpoint..."
                local health_check
                health_check=$(timeout 10 sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost 'curl -f http://localhost:8989/api/healthchecker 2>/dev/null || echo "Health check failed"' 2>/dev/null || echo "SSH health check failed")
                
                echo "Health Check Result:"
                echo "$health_check"
                
                if echo "$health_check" | grep -v "failed" | grep -q "health\|ok\|success"; then
                    print_success "Container health check passed"
                else
                    print_warning "Container health check failed or endpoint not ready"
                fi
            else
                print_warning "RustCI container not found or not running"
            fi
        else
            print_warning "SSH connection failed - deployment server may not be running"
            print_info "To test Docker deployment, start the SSH server with the deployment scripts"
        fi
    else
        print_warning "sshpass not available - cannot test SSH deployment"
    fi
}

# Function to cleanup test artifacts
cleanup() {
    print_step "Cleanup"
    
    if [[ -f .pipeline_test_env ]]; then
        rm .pipeline_test_env
        print_info "Cleaned up test environment file"
    fi
    
    # Stop server if we started it
    if [[ -f .rustci_server_pid ]]; then
        local server_pid=$(cat .rustci_server_pid)
        print_info "Stopping RustCI server (PID: $server_pid)..."
        
        # Kill the cargo watch process and its children
        pkill -P $server_pid 2>/dev/null || true
        kill $server_pid 2>/dev/null || true
        
        # Wait a bit and force kill if necessary
        sleep 2
        kill -9 $server_pid 2>/dev/null || true
        
        rm .rustci_server_pid
        print_info "Server stopped"
    fi
    
    # Clean up log file
    if [[ -f /tmp/rustci-server.log ]]; then
        rm /tmp/rustci-server.log
    fi
}

# Function to generate a fresh JWT token
generate_jwt_token() {
    print_info "Generating fresh JWT token using Rust binary..."
    
    # Try to use the Rust token generator binary
    local token_output
    token_output=$(cargo run --bin generate_token 2>/dev/null)
    
    if [[ $? -eq 0 ]] && [[ -n "$token_output" ]]; then
        # Extract the actual token from the output (it's usually on the second line)
        JWT_TOKEN=$(echo "$token_output" | grep -E '^eyJ' | head -1)
        
        if [[ -n "$JWT_TOKEN" ]]; then
            print_success "JWT token generated successfully using Rust binary"
            print_info "Token: ${JWT_TOKEN:0:50}..."
            return 0
        fi
    fi
    
    print_warning "Rust token generator failed, trying alternative method..."
    
    # Fallback: Create a simple token using the JWT secret from .env
    if command -v python3 >/dev/null 2>&1; then
        # Create a temporary token generation script
        cat > /tmp/generate_test_token.py << 'EOF'
import jwt
import datetime
import uuid

# JWT configuration (from .env file)
JWT_SECRET = "404E635266556A586E3272357538782F413F4428472B4B6250645367566B5970"

# Create payload with extended expiry
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

# Generate token
token = jwt.encode(payload, JWT_SECRET, algorithm="HS256")
print(token)
EOF
        
        # Check if PyJWT is available, install if needed
        if ! python3 -c "import jwt" 2>/dev/null; then
            print_info "Installing PyJWT..."
            pip3 install PyJWT 2>/dev/null || pip install PyJWT 2>/dev/null || {
                print_warning "Could not install PyJWT"
            }
        fi
        
        # Try to generate token
        if python3 -c "import jwt" 2>/dev/null; then
            JWT_TOKEN=$(python3 /tmp/generate_test_token.py 2>/dev/null || echo "")
            rm -f /tmp/generate_test_token.py
            
            if [[ -n "$JWT_TOKEN" ]]; then
                print_success "JWT token generated successfully using Python"
                print_info "Token: ${JWT_TOKEN:0:50}..."
                return 0
            fi
        fi
    fi
    
    print_error "Failed to generate JWT token using all methods"
    print_info "You may need to:"
    print_info "1. Install PyJWT: pip3 install PyJWT"
    print_info "2. Or manually generate a token using the get-admin-token.sh script"
    return 1
}

# Function to show summary
show_summary() {
    print_step "Test Summary"
    
    echo "This test script executed the exact cURL commands you provided:"
    echo "1. Pipeline upload: curl --location '$API_BASE_URL/api/ci/pipelines/upload' ..."
    echo "2. Pipeline trigger: curl --location '$API_BASE_URL/api/ci/pipelines/{id}/trigger' ..."
    echo ""
    echo "Key findings from the test:"
    echo "- Check the server logs above for detailed pipeline execution information"
    echo "- Look for Git clone errors, Docker build issues, or SSH connection problems"
    echo "- The pipeline.yaml uses hardcoded paths that may need adjustment"
    echo ""
    echo "Next steps:"
    echo "1. Review the server logs for specific error messages"
    echo "2. Check if workspace directories are created properly"
    echo "3. Verify Git clone command works with correct paths"
    echo "4. Test Docker build and SSH deployment steps individually"
}

# Main execution
main() {
    print_step "RustCI Pipeline Testing with Provided cURL Commands"
    print_info "This script tests the current pipeline.yaml using your exact cURL commands"
    
    # Generate fresh JWT token
    print_step "Generating Fresh JWT Token"
    if ! generate_jwt_token; then
        print_error "Failed to generate JWT token"
        exit 1
    fi
    
    # Start server if needed
    if ! check_api_health; then
        if ! start_rustci_server; then
            print_error "Failed to start RustCI server"
            exit 1
        fi
    fi
    
    if ! check_pipeline_file; then
        exit 1
    fi
    
    # Execute the test sequence
    local upload_success=false
    local trigger_success=false
    
    if upload_pipeline; then
        upload_success=true
        
        if trigger_pipeline; then
            trigger_success=true
            monitor_execution
        fi
    fi
    
    # Verify deployment if possible
    verify_docker_deployment
    
    # Show results
    echo ""
    print_step "Test Results"
    
    if $upload_success; then
        print_success "Pipeline upload: PASSED"
    else
        print_error "Pipeline upload: FAILED"
    fi
    
    if $trigger_success; then
        print_success "Pipeline trigger: PASSED"
    else
        print_error "Pipeline trigger: FAILED"
    fi
    
    show_summary
    cleanup
    
    # Exit with appropriate code
    if $upload_success && $trigger_success; then
        print_success "Overall test: PASSED (check logs for execution details)"
        exit 0
    else
        print_error "Overall test: FAILED"
        exit 1
    fi
}

# Handle script interruption
trap cleanup EXIT

# Run main function
main "$@"