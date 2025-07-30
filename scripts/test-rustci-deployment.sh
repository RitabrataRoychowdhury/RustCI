#!/bin/bash

# RustCI Pipeline Testing Script
# This script uploads and triggers various RustCI deployment pipelines

set -e

# Configuration
RUSTCI_API_BASE="http://localhost:8000/api"
WORKSPACE_ID="rustci-test-workspace"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
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

# Function to check if RustCI API is running
check_api() {
    log_info "Checking if RustCI API is running..."
    if curl -s -f "${RUSTCI_API_BASE}/healthchecker" > /dev/null 2>&1; then
        log_success "RustCI API is running"
        return 0
    else
        log_error "RustCI API is not accessible at ${RUSTCI_API_BASE}"
        log_info "Please ensure RustCI is running on localhost:8000"
        return 1
    fi
}

# Function to create workspace
create_workspace() {
    log_info "Creating workspace: ${WORKSPACE_ID}"
    
    curl -X POST "${RUSTCI_API_BASE}/workspaces" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"${WORKSPACE_ID}\",
            \"description\": \"Test workspace for RustCI deployment pipelines\",
            \"repository_url\": \"https://github.com/RitabrataRoychowdhury/RustCI.git\"
        }" \
        -w "\nHTTP Status: %{http_code}\n" || log_warning "Workspace creation failed or already exists"
}

# Function to upload pipeline
upload_pipeline() {
    local pipeline_file=$1
    local pipeline_name=$2
    
    log_info "Uploading pipeline: ${pipeline_name} from ${pipeline_file}"
    
    if [ ! -f "${pipeline_file}" ]; then
        log_error "Pipeline file ${pipeline_file} not found"
        return 1
    fi
    
    curl -X POST "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines" \
        -H "Content-Type: multipart/form-data" \
        -F "pipeline=@${pipeline_file}" \
        -F "name=${pipeline_name}" \
        -w "\nHTTP Status: %{http_code}\n" || log_error "Failed to upload ${pipeline_name}"
}

# Function to trigger pipeline
trigger_pipeline() {
    local pipeline_name=$1
    
    log_info "Triggering pipeline: ${pipeline_name}"
    
    local response=$(curl -X POST "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/${pipeline_name}/trigger" \
        -H "Content-Type: application/json" \
        -d '{"trigger_type": "manual"}' \
        -w "\nHTTP_STATUS:%{http_code}" -s)
    
    local http_status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$http_status" = "200" ] || [ "$http_status" = "201" ]; then
        log_success "Pipeline ${pipeline_name} triggered successfully"
        echo "Response: $body"
        
        # Extract execution ID if available
        local execution_id=$(echo "$body" | grep -o '"execution_id":"[^"]*"' | cut -d'"' -f4)
        if [ -n "$execution_id" ]; then
            log_info "Execution ID: ${execution_id}"
            return 0
        fi
    else
        log_error "Failed to trigger ${pipeline_name}. HTTP Status: ${http_status}"
        echo "Response: $body"
        return 1
    fi
}

# Function to check pipeline status
check_pipeline_status() {
    local pipeline_name=$1
    local execution_id=$2
    
    if [ -n "$execution_id" ]; then
        log_info "Checking status for execution: ${execution_id}"
        curl -s "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/${pipeline_name}/executions/${execution_id}" \
            -w "\nHTTP Status: %{http_code}\n"
    else
        log_info "Checking latest execution for pipeline: ${pipeline_name}"
        curl -s "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/${pipeline_name}/executions" \
            -w "\nHTTP Status: %{http_code}\n"
    fi
}

# Function to list all pipelines
list_pipelines() {
    log_info "Listing all pipelines in workspace: ${WORKSPACE_ID}"
    curl -s "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines" \
        -w "\nHTTP Status: %{http_code}\n"
}

# Main execution
main() {
    log_info "Starting RustCI deployment pipeline tests"
    
    # Check if API is running
    if ! check_api; then
        exit 1
    fi
    
    # Create workspace
    create_workspace
    
    echo
    log_info "=== Uploading Pipelines ==="
    
    # Upload pipelines (using the main pipeline.yaml file)
    upload_pipeline "pipeline.yaml" "rustci-main-pipeline"
    
    echo
    log_info "=== Listing Uploaded Pipelines ==="
    list_pipelines
    
    echo
    log_info "=== Triggering Pipelines ==="
    
    # Trigger main pipeline
    log_info "Starting main pipeline..."
    if trigger_pipeline "rustci-main-pipeline"; then
        sleep 5
        check_pipeline_status "rustci-main-pipeline"
    fi
    
    echo
    log_success "Pipeline test initiated. Check the RustCI dashboard for execution details."
    log_info "You can also check pipeline status using:"
    log_info "curl ${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/{pipeline-name}/executions"
}

# Handle script arguments
case "${1:-}" in
    "check")
        check_api
        ;;
    "create-workspace")
        create_workspace
        ;;
    "upload")
        if [ -z "$2" ] || [ -z "$3" ]; then
            log_error "Usage: $0 upload <pipeline-file> <pipeline-name>"
            exit 1
        fi
        upload_pipeline "$2" "$3"
        ;;
    "trigger")
        if [ -z "$2" ]; then
            log_error "Usage: $0 trigger <pipeline-name>"
            exit 1
        fi
        trigger_pipeline "$2"
        ;;
    "status")
        if [ -z "$2" ]; then
            log_error "Usage: $0 status <pipeline-name> [execution-id]"
            exit 1
        fi
        check_pipeline_status "$2" "$3"
        ;;
    "list")
        list_pipelines
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [check|create-workspace|upload|trigger|status|list]"
        echo "  check                           - Check if RustCI API is running"
        echo "  create-workspace               - Create test workspace"
        echo "  upload <file> <name>           - Upload specific pipeline"
        echo "  trigger <name>                 - Trigger specific pipeline"
        echo "  status <name> [execution-id]   - Check pipeline status"
        echo "  list                           - List all pipelines"
        echo "  (no args)                      - Run complete test suite"
        exit 1
        ;;
esac