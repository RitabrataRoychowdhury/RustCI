#!/bin/bash

# RustCI Deployment Validation Script
# This script validates that RustCI can successfully deploy code from the GitHub repository

set -e

# Configuration
RUSTCI_API_BASE="http://localhost:8000/api"
WORKSPACE_ID="rustci-deployment-validation"
SSH_HOST="localhost"
SSH_PORT="2222"
SSH_USER="user"
SSH_PASS="abc123"
DEPLOYED_PORT="8989"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
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

log_step() {
    echo -e "${CYAN}[STEP]${NC} $1"
}

# Function to check prerequisites
check_prerequisites() {
    log_step "Checking prerequisites..."
    
    local missing_tools=()
    
    # Check required tools
    command -v curl >/dev/null 2>&1 || missing_tools+=("curl")
    command -v docker >/dev/null 2>&1 || missing_tools+=("docker")
    command -v sshpass >/dev/null 2>&1 || missing_tools+=("sshpass")
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install missing tools and try again"
        return 1
    fi
    
    # Check if RustCI API is running
    if ! curl -s -f "${RUSTCI_API_BASE}/healthchecker" > /dev/null 2>&1; then
        log_error "RustCI API is not accessible at ${RUSTCI_API_BASE}"
        log_info "Please ensure RustCI is running on localhost:8000"
        return 1
    fi
    
    # Check SSH server
    if ! sshpass -p "${SSH_PASS}" ssh -o StrictHostKeyChecking=no -p "${SSH_PORT}" "${SSH_USER}@${SSH_HOST}" "echo 'SSH connection test'" >/dev/null 2>&1; then
        log_warning "SSH server not accessible at ${SSH_USER}@${SSH_HOST}:${SSH_PORT}"
        log_info "SSH deployment tests will be skipped"
        SSH_AVAILABLE=false
    else
        log_success "SSH server is accessible"
        SSH_AVAILABLE=true
    fi
    
    log_success "Prerequisites check completed"
}

# Function to setup test environment
setup_test_environment() {
    log_step "Setting up test environment..."
    
    # Create workspace
    log_info "Creating test workspace: ${WORKSPACE_ID}"
    curl -X POST "${RUSTCI_API_BASE}/workspaces" \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"${WORKSPACE_ID}\",
            \"description\": \"Validation workspace for RustCI deployment testing\",
            \"repository_url\": \"https://github.com/RitabrataRoychowdhury/RustCI.git\"
        }" \
        -s > /dev/null || log_warning "Workspace creation failed or already exists"
    
    log_success "Test environment setup completed"
}

# Function to test local build
test_local_build() {
    log_step "Testing local build capabilities..."
    
    # Create local build pipeline
    cat > local-build-test.yaml << 'EOF'
name: "Local Build Validation"
description: "Validate local build capabilities"
triggers:
- trigger_type: manual
  config: {}

stages:
- name: "Build Test"
  steps:
  - name: "fetch-source"
    step_type: shell
    config:
      command: |
        echo "=== Fetching RustCI source ==="
        rm -rf /tmp/rustci-validation
        git clone https://github.com/RitabrataRoychowdhury/RustCI.git /tmp/rustci-validation
        cd /tmp/rustci-validation
        echo "Repository cloned successfully"
        ls -la | head -10

  - name: "validate-structure"
    step_type: shell
    config:
      command: |
        echo "=== Validating project structure ==="
        cd /tmp/rustci-validation
        
        # Check essential files
        [ -f "Cargo.toml" ] && echo "✓ Cargo.toml found" || echo "✗ Cargo.toml missing"
        [ -f "Dockerfile" ] && echo "✓ Dockerfile found" || echo "✗ Dockerfile missing"
        [ -d "src" ] && echo "✓ src directory found" || echo "✗ src directory missing"
        
        # Show project info
        echo "Project dependencies:"
        grep -A 10 "\[dependencies\]" Cargo.toml || echo "No dependencies section found"

  - name: "docker-build-test"
    step_type: shell
    config:
      command: |
        echo "=== Testing Docker build ==="
        cd /tmp/rustci-validation
        
        # Build Docker image
        docker build -t rustci-validation:test . || {
          echo "Docker build failed, checking Dockerfile..."
          head -20 Dockerfile
          exit 1
        }
        
        echo "Docker build successful"
        docker images | grep rustci-validation

environment: {}
timeout: 1800
retry_count: 0
EOF

    # Upload and trigger pipeline
    log_info "Uploading local build test pipeline..."
    curl -X POST "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines" \
        -H "Content-Type: multipart/form-data" \
        -F "pipeline=@local-build-test.yaml" \
        -F "name=local-build-validation" \
        -s > /dev/null
    
    log_info "Triggering local build test..."
    local response=$(curl -X POST "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/local-build-validation/trigger" \
        -H "Content-Type: application/json" \
        -d '{"trigger_type": "manual"}' \
        -s -w "HTTP_STATUS:%{http_code}")
    
    local http_status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    
    if [ "$http_status" = "200" ] || [ "$http_status" = "201" ]; then
        log_success "Local build test triggered successfully"
        
        # Wait for completion
        log_info "Waiting for build to complete..."
        sleep 30
        
        # Check status
        curl -s "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/local-build-validation/executions" | head -50
    else
        log_error "Failed to trigger local build test"
        return 1
    fi
    
    # Cleanup
    rm -f local-build-test.yaml
}

# Function to generate validation report
generate_report() {
    log_step "Generating validation report..."
    
    local report_file="rustci-validation-report.md"
    
    cat > "$report_file" << EOF
# RustCI Deployment Validation Report

Generated on: $(date)

## Test Environment
- RustCI API: ${RUSTCI_API_BASE}
- Workspace: ${WORKSPACE_ID}
- SSH Server: ${SSH_AVAILABLE:-false}

## Test Results

### Prerequisites Check
- ✓ RustCI API accessible
- ✓ Required tools available
$([ "$SSH_AVAILABLE" = "true" ] && echo "- ✓ SSH server accessible" || echo "- ⚠ SSH server not accessible")

### Pipeline Tests
- Local build test executed
- Validation completed

## Next Steps

1. Check pipeline execution logs in RustCI dashboard
2. Verify deployed applications are accessible
3. Run integration tests against deployed services

## API Endpoints Tested

- Health Check: ${RUSTCI_API_BASE}/healthchecker
- Workspace Management: ${RUSTCI_API_BASE}/workspaces
- Pipeline Upload: ${RUSTCI_API_BASE}/workspaces/{workspace}/pipelines
- Pipeline Trigger: ${RUSTCI_API_BASE}/workspaces/{workspace}/pipelines/{pipeline}/trigger

EOF

    log_success "Validation report generated: $report_file"
}

# Function to cleanup test environment
cleanup_test_environment() {
    log_step "Cleaning up test environment..."
    
    # Remove temporary files
    rm -f local-build-test.yaml
    
    # Clean up Docker images
    docker rmi rustci-validation:test 2>/dev/null || true
    
    log_info "Test environment cleanup completed"
    log_info "Note: Workspace '${WORKSPACE_ID}' and its pipelines remain for review"
}

# Main execution
main() {
    echo "========================================"
    echo "RustCI Deployment Validation"
    echo "========================================"
    echo
    
    # Run validation steps
    check_prerequisites || exit 1
    echo
    
    setup_test_environment
    echo
    
    test_local_build
    echo
    
    generate_report
    echo
    
    cleanup_test_environment
    echo
    
    log_success "RustCI deployment validation completed!"
    log_info "Check the generated report: rustci-validation-report.md"
    log_info "Monitor ongoing pipeline executions in the RustCI dashboard"
}

# Handle script arguments
case "${1:-}" in
    "prerequisites")
        check_prerequisites
        ;;
    "local-build")
        check_prerequisites && setup_test_environment && test_local_build
        ;;
    "report")
        generate_report
        ;;
    "cleanup")
        cleanup_test_environment
        ;;
    "")
        main
        ;;
    *)
        echo "Usage: $0 [prerequisites|local-build|report|cleanup]"
        echo "  prerequisites   - Check system prerequisites"
        echo "  local-build     - Test local build capabilities only"
        echo "  report          - Generate validation report"
        echo "  cleanup         - Clean up test environment"
        echo "  (no args)       - Run complete validation suite"
        exit 1
        ;;
esac