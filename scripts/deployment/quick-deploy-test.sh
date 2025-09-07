#!/bin/bash

# Quick Deployment Test Script
# Tests the Node.js deployment with minimal setup

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RUSTCI_API="http://localhost:8000/api"
PIPELINE_FILE="../nodejs-deployment-pipeline.yaml"

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

# Quick checks
quick_checks() {
    log_info "🔍 Running quick checks..."
    
    # Check RustCI is running
    if ! curl -s "$RUSTCI_API/healthchecker" >/dev/null 2>&1; then
        log_error "RustCI is not running at http://localhost:8000"
        log_info "Start it with: cargo run"
        exit 1
    fi
    log_success "RustCI is running"
    
    # Check token exists
    if [ ! -f "../.rustci_token" ]; then
        log_error "No authentication token found"
        log_info "Run: ../scripts/auth/get-admin-token.sh"
        exit 1
    fi
    log_success "Authentication token found"
    
    # Check pipeline file exists
    if [ ! -f "$PIPELINE_FILE" ]; then
        log_error "Pipeline file not found: $PIPELINE_FILE"
        exit 1
    fi
    log_success "Pipeline file found"
}

# Deploy and test
deploy_and_test() {
    local token=$(cat ../.rustci_token)
    
    log_info "🚀 Uploading and triggering pipeline..."
    
    # Upload pipeline
    local upload_response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -X POST "$RUSTCI_API/ci/pipelines/upload" \
        -H "Authorization: Bearer $token" \
        -F "file=@$PIPELINE_FILE" \
        -F "name=quick-nodejs-test" \
        -F "description=Quick Node.js deployment test")
    
    local upload_status=$(echo "$upload_response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local upload_body=$(echo "$upload_response" | sed '/HTTP_STATUS:/d')
    
    if [ "$upload_status" != "200" ] && [ "$upload_status" != "201" ]; then
        log_error "Failed to upload pipeline (Status: $upload_status)"
        echo "$upload_body"
        exit 1
    fi
    
    local pipeline_id=$(echo "$upload_body" | jq -r '.id' 2>/dev/null)
    if [ -z "$pipeline_id" ] || [ "$pipeline_id" = "null" ]; then
        log_error "Could not extract pipeline ID"
        exit 1
    fi
    
    log_success "Pipeline uploaded (ID: $pipeline_id)"
    
    # Trigger pipeline
    local trigger_response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -X POST "$RUSTCI_API/ci/pipelines/$pipeline_id/trigger" \
        -H "Authorization: Bearer $token" \
        -H "Content-Type: application/json" \
        -d '{
            "trigger_type": "manual",
            "branch": "main",
            "environment": {
                "BUILD_ID": "'$(date +%Y%m%d_%H%M%S)'",
                "NODE_ENV": "production"
            }
        }')
    
    local trigger_status=$(echo "$trigger_response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local trigger_body=$(echo "$trigger_response" | sed '/HTTP_STATUS:/d')
    
    if [ "$trigger_status" != "200" ] && [ "$trigger_status" != "201" ]; then
        log_error "Failed to trigger pipeline (Status: $trigger_status)"
        echo "$trigger_body"
        exit 1
    fi
    
    local execution_id=$(echo "$trigger_body" | jq -r '.execution_id' 2>/dev/null)
    log_success "Pipeline triggered (Execution ID: $execution_id)"
    
    # Wait and check result
    log_info "⏳ Waiting for execution to complete..."
    sleep 10
    
    # Check execution status
    local status_response=$(curl -s "$RUSTCI_API/ci/executions/$execution_id" \
        -H "Authorization: Bearer $token")
    
    local exec_status=$(echo "$status_response" | jq -r '.[0].status' 2>/dev/null)
    local duration=$(echo "$status_response" | jq -r '.[0].duration' 2>/dev/null)
    
    echo "Execution Status: $exec_status"
    echo "Duration: ${duration}ms"
    
    # Check if deployment actually worked
    log_info "🔍 Checking deployment results..."
    
    # Check Docker containers
    if docker ps | grep -q "nodejs-hello-world"; then
        log_success "✅ Docker containers found"
        docker ps | grep nodejs-hello-world
    else
        log_error "❌ No Docker containers found"
    fi
    
    # Check application accessibility
    if curl -s http://localhost:3000/ >/dev/null 2>&1; then
        log_success "✅ Application is accessible at http://localhost:3000"
        echo "Response: $(curl -s http://localhost:3000/)"
    else
        log_error "❌ Application is not accessible at http://localhost:3000"
    fi
    
    # Summary
    echo ""
    echo "📊 Deployment Test Summary:"
    echo "  Pipeline Status: $exec_status"
    echo "  Execution Time: ${duration}ms"
    echo "  Docker Containers: $(docker ps | grep -c nodejs-hello-world || echo 0)"
    echo "  Application URL: http://localhost:3000"
    
    if [ "$exec_status" = "success" ] && [ "$duration" -gt 1000 ] && curl -s http://localhost:3000/ >/dev/null 2>&1; then
        log_success "🎉 Deployment test PASSED!"
        return 0
    else
        log_error "❌ Deployment test FAILED!"
        return 1
    fi
}

# Main execution
main() {
    echo "🚀 Quick Node.js Deployment Test"
    echo "================================="
    echo ""
    
    quick_checks
    echo ""
    deploy_and_test
}

# Run main function
main "$@"