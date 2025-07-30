#!/bin/bash

# RustCI API Quick Reference Script
# Usage: ./scripts/rustci-api.sh <command> [args...]

set -e

# Configuration
RUSTCI_URL="http://localhost:8000"
TOKEN_FILE=".rustci_token"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Load token
load_token() {
    if [ -f "$TOKEN_FILE" ]; then
        cat "$TOKEN_FILE"
    else
        echo -e "${RED}❌ No token found. Run ./scripts/get-admin-token.sh first${NC}" >&2
        exit 1
    fi
}

# Show usage
show_usage() {
    echo -e "${BLUE}RustCI API Quick Reference${NC}"
    echo "Usage: $0 <command> [args...]"
    echo ""
    echo "Commands:"
    echo "  health                          - Check server health"
    echo "  me                             - Get current user info"
    echo "  pipelines                      - List all pipelines"
    echo "  upload <file> <name> [desc]    - Upload pipeline YAML"
    echo "  trigger <pipeline_id> [branch] - Trigger pipeline execution"
    echo "  executions                     - List all executions"
    echo "  execution <execution_id>       - Get execution details"
    echo "  cancel <execution_id>          - Cancel execution"
    echo "  yaml <pipeline_id>             - Get pipeline YAML"
    echo ""
    echo "Examples:"
    echo "  $0 upload docs/pipeline-examples/docker-deployment-pipeline.yaml docker-test"
    echo "  $0 trigger 550e8400-e29b-41d4-a716-446655440000 main"
    echo "  $0 execution 123e4567-e89b-12d3-a456-426614174000"
}

# Health check (public endpoint)
cmd_health() {
    echo -e "${YELLOW}🏥 Checking RustCI health...${NC}"
    curl -s "$RUSTCI_URL/healthchecker" | jq . 2>/dev/null || curl -s "$RUSTCI_URL/healthchecker"
}

# Get current user
cmd_me() {
    local token=$(load_token)
    echo -e "${YELLOW}👤 Getting current user info...${NC}"
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/sessions/me" | jq . 2>/dev/null || \
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/sessions/me"
}

# List pipelines
cmd_pipelines() {
    local token=$(load_token)
    echo -e "${YELLOW}📋 Listing pipelines...${NC}"
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/pipelines" | jq . 2>/dev/null || \
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/pipelines"
}

# Upload pipeline
cmd_upload() {
    local file="$1"
    local name="$2"
    local description="${3:-Uploaded via API script}"
    local token=$(load_token)
    
    if [ -z "$file" ] || [ -z "$name" ]; then
        echo -e "${RED}❌ Usage: $0 upload <file> <name> [description]${NC}"
        exit 1
    fi
    
    if [ ! -f "$file" ]; then
        echo -e "${RED}❌ File not found: $file${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}📤 Uploading pipeline: $file${NC}"
    response=$(curl -s -w "%{http_code}" -o /tmp/upload_response \
        -H "Authorization: Bearer $token" \
        -F "file=@$file" \
        -F "name=$name" \
        -F "description=$description" \
        "$RUSTCI_URL/api/ci/pipelines/upload")
    
    if [ "$response" = "200" ] || [ "$response" = "201" ]; then
        echo -e "${GREEN}✅ Pipeline uploaded successfully${NC}"
        cat /tmp/upload_response | jq . 2>/dev/null || cat /tmp/upload_response
        
        # Extract and display pipeline ID
        PIPELINE_ID=$(cat /tmp/upload_response | jq -r '.pipeline_id' 2>/dev/null || echo "")
        if [ -n "$PIPELINE_ID" ] && [ "$PIPELINE_ID" != "null" ]; then
            echo -e "${BLUE}💡 To trigger this pipeline:${NC}"
            echo "$0 trigger $PIPELINE_ID"
        fi
    else
        echo -e "${RED}❌ Upload failed (HTTP $response)${NC}"
        cat /tmp/upload_response
    fi
    rm -f /tmp/upload_response
}

# Trigger pipeline
cmd_trigger() {
    local pipeline_id="$1"
    local branch="${2:-main}"
    local token=$(load_token)
    
    if [ -z "$pipeline_id" ]; then
        echo -e "${RED}❌ Usage: $0 trigger <pipeline_id> [branch]${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}🚀 Triggering pipeline: $pipeline_id${NC}"
    response=$(curl -s -w "%{http_code}" -o /tmp/trigger_response \
        -X POST \
        -H "Authorization: Bearer $token" \
        -H "Content-Type: application/json" \
        -d "{\"branch\": \"$branch\"}" \
        "$RUSTCI_URL/api/ci/pipelines/$pipeline_id/trigger")
    
    if [ "$response" = "200" ] || [ "$response" = "201" ]; then
        echo -e "${GREEN}✅ Pipeline triggered successfully${NC}"
        cat /tmp/trigger_response | jq . 2>/dev/null || cat /tmp/trigger_response
        
        # Extract execution ID
        EXECUTION_ID=$(cat /tmp/trigger_response | jq -r '.execution_id' 2>/dev/null || echo "")
        if [ -n "$EXECUTION_ID" ] && [ "$EXECUTION_ID" != "null" ]; then
            echo -e "${BLUE}💡 To monitor this execution:${NC}"
            echo "$0 execution $EXECUTION_ID"
        fi
    else
        echo -e "${RED}❌ Trigger failed (HTTP $response)${NC}"
        cat /tmp/trigger_response
    fi
    rm -f /tmp/trigger_response
}

# List executions
cmd_executions() {
    local token=$(load_token)
    echo -e "${YELLOW}📊 Listing executions...${NC}"
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/executions" | jq . 2>/dev/null || \
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/executions"
}

# Get execution details
cmd_execution() {
    local execution_id="$1"
    local token=$(load_token)
    
    if [ -z "$execution_id" ]; then
        echo -e "${RED}❌ Usage: $0 execution <execution_id>${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}🔍 Getting execution details: $execution_id${NC}"
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/executions/$execution_id" | jq . 2>/dev/null || \
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/executions/$execution_id"
}

# Cancel execution
cmd_cancel() {
    local execution_id="$1"
    local token=$(load_token)
    
    if [ -z "$execution_id" ]; then
        echo -e "${RED}❌ Usage: $0 cancel <execution_id>${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}🛑 Cancelling execution: $execution_id${NC}"
    response=$(curl -s -w "%{http_code}" -o /tmp/cancel_response \
        -X DELETE \
        -H "Authorization: Bearer $token" \
        "$RUSTCI_URL/api/ci/executions/$execution_id/cancel")
    
    if [ "$response" = "200" ]; then
        echo -e "${GREEN}✅ Execution cancelled${NC}"
        cat /tmp/cancel_response | jq . 2>/dev/null || cat /tmp/cancel_response
    else
        echo -e "${RED}❌ Cancel failed (HTTP $response)${NC}"
        cat /tmp/cancel_response
    fi
    rm -f /tmp/cancel_response
}

# Get pipeline YAML
cmd_yaml() {
    local pipeline_id="$1"
    local token=$(load_token)
    
    if [ -z "$pipeline_id" ]; then
        echo -e "${RED}❌ Usage: $0 yaml <pipeline_id>${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}📄 Getting pipeline YAML: $pipeline_id${NC}"
    curl -s -H "Authorization: Bearer $token" "$RUSTCI_URL/api/ci/pipelines/$pipeline_id/yaml"
}

# Main command dispatcher
main() {
    local command="$1"
    shift
    
    case "$command" in
        "health")
            cmd_health "$@"
            ;;
        "me")
            cmd_me "$@"
            ;;
        "pipelines")
            cmd_pipelines "$@"
            ;;
        "upload")
            cmd_upload "$@"
            ;;
        "trigger")
            cmd_trigger "$@"
            ;;
        "executions")
            cmd_executions "$@"
            ;;
        "execution")
            cmd_execution "$@"
            ;;
        "cancel")
            cmd_cancel "$@"
            ;;
        "yaml")
            cmd_yaml "$@"
            ;;
        *)
            show_usage
            exit 1
            ;;
    esac
}

# Check if command provided
if [ $# -eq 0 ]; then
    show_usage
    exit 1
fi

# Run main function
main "$@"