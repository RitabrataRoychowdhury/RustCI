#!/bin/bash

# Manual Node.js Deployment Test Script
# This script manually tests the RustCI API endpoints

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RUSTCI_URL="http://localhost:8000"
PIPELINE_FILE="nodejs-deployment-pipeline.yaml"
TOKEN_FILE=".rustci_token"

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${YELLOW}[STEP]${NC} $1"
}

# Check if RustCI is running
check_rustci() {
    log_step "Checking if RustCI is running..."
    if curl -s "$RUSTCI_URL/healthchecker" >/dev/null 2>&1; then
        log_success "RustCI is running"
    else
        log_error "RustCI is not running at $RUSTCI_URL"
        log_info "Start it with: cargo run --bin RustAutoDevOps"
        exit 1
    fi
}

# Get authentication token
get_token() {
    log_step "Getting authentication token..."
    
    if [ -f "$TOKEN_FILE" ]; then
        TOKEN=$(cat "$TOKEN_FILE")
        log_success "Using existing token"
    else
        log_error "No token file found. Run: ./scripts/get-admin-token.sh"
        exit 1
    fi
    
    # Test token validity
    response=$(curl -s -w "%{http_code}" -o /tmp/token_test \
        -H "Authorization: Bearer $TOKEN" \
        "$RUSTCI_URL/api/sessions/me")
    
    if [ "$response" = "200" ]; then
        log_success "Token is valid"
    else
        log_error "Token is invalid. Run: ./scripts/get-admin-token.sh"
        exit 1
    fi
}

# Upload pipeline
upload_pipeline() {
    log_step "Uploading pipeline YAML..."
    
    if [ ! -f "$PIPELINE_FILE" ]; then
        log_error "Pipeline file not found: $PIPELINE_FILE"
        exit 1
    fi
    
    response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -X POST "$RUSTCI_URL/api/ci/pipelines/upload" \
        -H "Authorization: Bearer $TOKEN" \
        -F "file=@$PIPELINE_FILE" \
        -F "name=nodejs-hello-world-pipeline" \
        -F "description=Node.js Hello World deployment pipeline")
    
    status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ] || [ "$status" = "201" ]; then
        log_success "Pipeline uploaded successfully"
        echo "$body" | jq . 2>/dev/null || echo "$body"
        
        # Extract pipeline ID
        PIPELINE_ID=$(echo "$body" | jq -r '.id' 2>/dev/null || echo "")
        if [ -n "$PIPELINE_ID" ] && [ "$PIPELINE_ID" != "null" ]; then
            echo "$PIPELINE_ID" > /tmp/pipeline_id
            log_info "Pipeline ID: $PIPELINE_ID"
        else
            log_error "Could not extract pipeline ID"
            exit 1
        fi
    else
        log_error "Failed to upload pipeline (Status: $status)"
        echo "$body"
        exit 1
    fi
}

# List pipelines
list_pipelines() {
    log_step "Listing all pipelines..."
    
    response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -H "Authorization: Bearer $TOKEN" \
        "$RUSTCI_URL/api/ci/pipelines")
    
    status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ]; then
        log_success "Pipelines listed successfully"
        echo "$body" | jq . 2>/dev/null || echo "$body"
    else
        log_error "Failed to list pipelines (Status: $status)"
        echo "$body"
    fi
}

# Trigger pipeline
trigger_pipeline() {
    log_step "Triggering pipeline execution..."
    
    if [ ! -f "/tmp/pipeline_id" ]; then
        log_error "Pipeline ID not found. Upload pipeline first."
        exit 1
    fi
    
    PIPELINE_ID=$(cat /tmp/pipeline_id)
    BUILD_ID=$(date +%Y%m%d_%H%M%S)
    
    response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -X POST "$RUSTCI_URL/api/ci/pipelines/$PIPELINE_ID/trigger" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "{
            \"trigger_type\": \"manual\",
            \"branch\": \"main\",
            \"environment\": {
                \"BUILD_ID\": \"$BUILD_ID\",
                \"NODE_ENV\": \"production\"
            }
        }")
    
    status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ] || [ "$status" = "201" ]; then
        log_success "Pipeline triggered successfully"
        echo "$body" | jq . 2>/dev/null || echo "$body"
        
        # Extract execution ID
        EXECUTION_ID=$(echo "$body" | jq -r '.execution_id' 2>/dev/null || echo "")
        if [ -n "$EXECUTION_ID" ] && [ "$EXECUTION_ID" != "null" ]; then
            echo "$EXECUTION_ID" > /tmp/execution_id
            log_info "Execution ID: $EXECUTION_ID"
        else
            log_error "Could not extract execution ID"
        fi
    else
        log_error "Failed to trigger pipeline (Status: $status)"
        echo "$body"
        exit 1
    fi
}

# Monitor execution
monitor_execution() {
    log_step "Monitoring pipeline execution..."
    
    if [ ! -f "/tmp/execution_id" ]; then
        log_error "Execution ID not found. Trigger pipeline first."
        return 1
    fi
    
    EXECUTION_ID=$(cat /tmp/execution_id)
    max_attempts=60  # 5 minutes with 5-second intervals
    attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        log_info "Checking execution status (attempt $attempt/$max_attempts)..."
        
        response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
            -H "Authorization: Bearer $TOKEN" \
            "$RUSTCI_URL/api/ci/executions/$EXECUTION_ID")
        
        status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
        body=$(echo "$response" | sed '/HTTP_STATUS:/d')
        
        if [ "$status" = "200" ]; then
            echo "$body" | jq . 2>/dev/null || echo "$body"
            
            # Check if execution is complete
            exec_status=$(echo "$body" | jq -r '.status' 2>/dev/null || echo "")
            if [ "$exec_status" = "completed" ] || [ "$exec_status" = "success" ]; then
                log_success "Pipeline execution completed successfully!"
                return 0
            elif [ "$exec_status" = "failed" ] || [ "$exec_status" = "error" ]; then
                log_error "Pipeline execution failed!"
                return 1
            fi
        else
            log_error "Failed to get execution status (Status: $status)"
            echo "$body"
        fi
        
        sleep 5
        ((attempt++))
    done
    
    log_error "Pipeline monitoring timed out"
    return 1
}

# List executions
list_executions() {
    log_step "Listing all executions..."
    
    response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -H "Authorization: Bearer $TOKEN" \
        "$RUSTCI_URL/api/ci/executions")
    
    status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ]; then
        log_success "Executions listed successfully"
        echo "$body" | jq . 2>/dev/null || echo "$body"
    else
        log_error "Failed to list executions (Status: $status)"
        echo "$body"
    fi
}

# Verify deployment
verify_deployment() {
    log_step "Verifying deployment..."
    
    # Check if the application is running
    if docker ps | grep -q "nodejs-hello-world-prod"; then
        log_success "Application container is running"
        
        # Test the deployed application
        log_info "Testing deployed application..."
        sleep 5
        
        if curl -s http://localhost:3000/ | grep -i "hello" >/dev/null 2>&1; then
            log_success "✅ Deployment verification passed!"
            log_info "Application is accessible at http://localhost:3000"
            
            # Show the response
            echo "Application response:"
            curl -s http://localhost:3000/
        else
            log_error "❌ Application is not responding correctly"
            return 1
        fi
    else
        log_error "Application container is not running"
        return 1
    fi
}

# Show deployment status
show_status() {
    log_step "Showing deployment status..."
    
    echo "=== Docker Containers ==="
    docker ps | grep nodejs-hello-world || echo "No Node.js containers found"
    
    echo ""
    echo "=== Docker Images ==="
    docker images | grep nodejs-hello-world || echo "No Node.js images found"
    
    echo ""
    echo "=== Application Test ==="
    if curl -s http://localhost:3000/ >/dev/null 2>&1; then
        echo "✅ Application is accessible at http://localhost:3000"
        echo "Response: $(curl -s http://localhost:3000/)"
    else
        echo "❌ Application is not accessible"
    fi
}

# Main execution
main() {
    echo "🚀 Manual Node.js Deployment Test"
    echo "=================================="
    
    check_rustci
    get_token
    upload_pipeline
    list_pipelines
    trigger_pipeline
    
    if monitor_execution; then
        verify_deployment
        show_status
        log_success "🎉 Deployment test completed successfully!"
    else
        log_error "❌ Deployment test failed"
        exit 1
    fi
}

# Cleanup function
cleanup() {
    log_step "Cleaning up temporary files..."
    rm -f /tmp/pipeline_id /tmp/execution_id /tmp/token_test
}

# Handle script arguments
case "${1:-deploy}" in
    "deploy")
        main
        ;;
    "status")
        get_token
        show_status
        ;;
    "list-pipelines")
        get_token
        list_pipelines
        ;;
    "list-executions")
        get_token
        list_executions
        ;;
    "cleanup")
        cleanup
        ;;
    *)
        echo "Usage: $0 [deploy|status|list-pipelines|list-executions|cleanup]"
        exit 1
        ;;
esac

# Cleanup on exit
trap cleanup EXIT