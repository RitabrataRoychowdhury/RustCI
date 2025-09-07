#!/bin/bash

# Node.js Hello World Deployment Test Script
# Tests deployment of https://github.com/dockersamples/helloworld-demo-node.git

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
RUSTCI_API_BASE="http://localhost:8000/api"
NODEJS_REPO="https://github.com/dockersamples/helloworld-demo-node.git"
WORKSPACE_ID="nodejs-hello-world-test"
PIPELINE_NAME="nodejs-hello-world-pipeline"
TEST_RESULTS_DIR="test-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Authentication token (will be prompted)
AUTH_TOKEN=""
SKIP_AUTH=false

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

log_section() {
    echo -e "${MAGENTA}[SECTION]${NC} $1"
}

# Function to guide user through OAuth process
guide_oauth_process() {
    log_info "🔐 Getting Authentication Token via OAuth"
    echo ""
    log_info "Follow these steps to get your JWT token:"
    echo ""
    log_info "1. Open your browser and go to: http://localhost:8000/api/sessions/oauth/github"
    log_info "2. You'll be redirected to GitHub for authentication"
    log_info "3. After successful login, you'll be redirected back with a JSON response"
    log_info "4. Look for the 'token' field in the JSON response"
    log_info "5. Copy the token value (it starts with 'eyJ')"
    echo ""
    log_warning "Example JSON response:"
    echo '{"status":"success","token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."}'
    echo ""
    log_info "Copy the token value (without quotes) and paste it when prompted."
    echo ""
}

# Function to prompt for authentication token
prompt_for_token() {
    log_section "Authentication Setup"
    
    if [ -z "$AUTH_TOKEN" ]; then
        echo ""
        log_info "RustCI requires authentication for API access."
        log_info "You can get a token by:"
        log_info "1. Running: ./scripts/get-admin-token.sh (recommended)"
        log_info "2. Manual OAuth process (guided)"
        echo ""
        
        # Check if there's a saved token from get-admin-token.sh
        if [ -f ".rustci_token" ]; then
            log_info "Found saved token file. Would you like to use it? (y/n)"
            read -r use_saved
            if [ "$use_saved" = "y" ] || [ "$use_saved" = "Y" ]; then
                AUTH_TOKEN=$(cat .rustci_token)
                log_success "Using saved token"
                return
            fi
        fi
        
        # Ask user which method they prefer
        while true; do
            echo "Choose authentication method:"
            echo "1) Use get-admin-token.sh script (recommended)"
            echo "2) Manual OAuth process"
            echo -n "Enter choice (1 or 2): "
            read -r auth_choice
            
            case "$auth_choice" in
                "1")
                    log_info "Running get-admin-token.sh script..."
                    if [ -f "./scripts/get-admin-token.sh" ]; then
                        ./scripts/get-admin-token.sh
                        if [ -f ".rustci_token" ]; then
                            AUTH_TOKEN=$(cat .rustci_token)
                            log_success "Token obtained from script"
                            return
                        else
                            log_error "Script completed but no token file found"
                            log_info "Falling back to manual token entry..."
                            break
                        fi
                    else
                        log_error "get-admin-token.sh script not found"
                        log_info "Falling back to manual token entry..."
                        break
                    fi
                    ;;
                "2")
                    guide_oauth_process
                    break
                    ;;
                *)
                    log_error "Invalid choice. Please enter 1 or 2."
                    continue
                    ;;
            esac
        done
        
        # Prompt for token with hidden input
        echo -n "Please enter your JWT token: "
        read -s AUTH_TOKEN
        echo ""
        
        if [ -z "$AUTH_TOKEN" ]; then
            log_error "No token provided. Cannot proceed without authentication."
            log_info "Run './scripts/get-admin-token.sh' to get a token first."
            exit 1
        fi
        
        # Basic validation - JWT tokens start with 'eyJ'
        if [[ ! "$AUTH_TOKEN" =~ ^eyJ ]]; then
            log_warning "Token doesn't look like a JWT (should start with 'eyJ')"
            log_info "Make sure you copied the token correctly from the OAuth response"
        fi
        
        log_success "Token received"
    fi
}

# Function to test authentication
test_authentication() {
    log_step "Testing authentication"
    
    local response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -H "Authorization: Bearer ${AUTH_TOKEN}" \
        "${RUSTCI_API_BASE}/sessions/me" 2>/dev/null)
    
    local status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ]; then
        log_success "Authentication successful"
        echo "User info: $body"
        return 0
    else
        log_error "Authentication failed (Status: $status)"
        echo "Response: $body"
        log_info "Please check your token and try again"
        
        # Try alternative endpoints for debugging
        log_info "Trying alternative authentication test..."
        local alt_response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
            -H "Authorization: Bearer ${AUTH_TOKEN}" \
            "${RUSTCI_API_BASE}/healthchecker" 2>/dev/null)
        
        local alt_status=$(echo "$alt_response" | grep "HTTP_STATUS:" | cut -d: -f2)
        if [ "$alt_status" = "200" ]; then
            log_warning "Health check works, but authentication endpoint failed"
            log_info "This might be a token format issue. Make sure you're using the JWT token from OAuth."
        fi
        
        return 1
    fi
}

# Function to check prerequisites
check_prerequisites() {
    log_section "Checking Prerequisites"
    
    # Check if RustCI is running
    if ! curl -s -f "${RUSTCI_API_BASE}/healthchecker" >/dev/null 2>&1; then
        log_error "RustCI API is not accessible at ${RUSTCI_API_BASE}"
        log_info "Please ensure RustCI is running: cargo run --bin RustAutoDevOps"
        exit 1
    fi
    
    # Prompt for and test authentication (unless skipped)
    if [ "$SKIP_AUTH" = false ]; then
        prompt_for_token
        if ! test_authentication; then
            exit 1
        fi
    else
        log_warning "Skipping authentication (--skip-auth specified)"
        log_warning "API calls will likely fail without proper authentication"
    fi
    
    # Check if DinD is available
    if docker ps | grep -q "rustci-dind"; then
        log_success "DinD container is running"
    else
        log_warning "DinD container not found. Will use local Docker."
    fi
    
    # Check Docker
    if ! docker info >/dev/null 2>&1; then
        log_error "Docker is not running"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Function to create workspace
create_workspace() {
    log_step "Creating workspace: ${WORKSPACE_ID}"
    
    local curl_headers=("-H" "Content-Type: application/json")
    if [ -n "$AUTH_TOKEN" ]; then
        curl_headers+=("-H" "Authorization: Bearer ${AUTH_TOKEN}")
    fi
    
    local response=$(curl -X POST "${RUSTCI_API_BASE}/workspaces" \
        "${curl_headers[@]}" \
        -d "{
            \"name\": \"${WORKSPACE_ID}\",
            \"description\": \"Node.js Hello World deployment test workspace\",
            \"repository_url\": \"${NODEJS_REPO}\"
        }" \
        -s -w "HTTP_STATUS:%{http_code}")
    
    local status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ] || [ "$status" = "201" ]; then
        log_success "Workspace created successfully"
        echo "Response: $body"
    else
        log_warning "Workspace creation failed or already exists (Status: $status)"
        echo "Response: $body"
    fi
}

# Function to create pipeline configuration
create_pipeline_config() {
    log_step "Creating pipeline configuration"
    
    mkdir -p "$TEST_RESULTS_DIR"
    
    cat > "${TEST_RESULTS_DIR}/nodejs-pipeline.yaml" << 'EOF'
name: "Node.js Hello World Pipeline"
description: "Deploy Node.js Hello World application from GitHub"

triggers:
  - trigger_type: manual
    config: {}
  - trigger_type: push
    config:
      branches: ["main", "master"]

stages:
  - name: "Source Checkout"
    steps:
      - name: "clone-repository"
        step_type: shell
        config:
          command: |
            echo "=== Cloning Node.js Hello World Repository ==="
            rm -rf /tmp/nodejs-hello-world
            git clone https://github.com/dockersamples/helloworld-demo-node.git /tmp/nodejs-hello-world
            cd /tmp/nodejs-hello-world
            echo "Repository cloned successfully"
            ls -la
            echo "=== Repository Contents ==="
            find . -type f -name "*.js" -o -name "*.json" -o -name "Dockerfile" | head -10

  - name: "Build and Test"
    steps:
      - name: "docker-build"
        step_type: shell
        config:
          command: |
            echo "=== Building Docker Image ==="
            cd /tmp/nodejs-hello-world
            
            # Show Dockerfile contents
            echo "=== Dockerfile Contents ==="
            cat Dockerfile
            
            # Build Docker image
            docker build -t nodejs-hello-world:${BUILD_ID:-latest} .
            
            echo "=== Docker Image Built Successfully ==="
            docker images | grep nodejs-hello-world

      - name: "test-application"
        step_type: shell
        config:
          command: |
            echo "=== Testing Application ==="
            cd /tmp/nodejs-hello-world
            
            # Run container in background
            docker run -d --name nodejs-test-${BUILD_ID:-latest} -p 8080:8080 nodejs-hello-world:${BUILD_ID:-latest}
            
            # Wait for application to start
            sleep 10
            
            # Test the application
            echo "Testing application endpoint..."
            if curl -f http://localhost:8080/ | grep -i "hello"; then
                echo "✅ Application test passed!"
            else
                echo "❌ Application test failed!"
                exit 1
            fi
            
            # Cleanup test container
            docker stop nodejs-test-${BUILD_ID:-latest} || true
            docker rm nodejs-test-${BUILD_ID:-latest} || true

  - name: "Deploy"
    steps:
      - name: "deploy-application"
        step_type: shell
        config:
          command: |
            echo "=== Deploying Application ==="
            
            # Stop any existing deployment
            docker stop nodejs-hello-world-prod 2>/dev/null || true
            docker rm nodejs-hello-world-prod 2>/dev/null || true
            
            # Deploy new version
            docker run -d \
              --name nodejs-hello-world-prod \
              --restart unless-stopped \
              -p 3000:8080 \
              nodejs-hello-world:${BUILD_ID:-latest}
            
            echo "=== Application Deployed Successfully ==="
            echo "Application is running on http://localhost:3000"
            
            # Verify deployment
            sleep 5
            if curl -f http://localhost:3000/ >/dev/null 2>&1; then
                echo "✅ Deployment verification passed!"
            else
                echo "❌ Deployment verification failed!"
                exit 1
            fi

  - name: "Cleanup"
    steps:
      - name: "cleanup-resources"
        step_type: shell
        config:
          command: |
            echo "=== Cleaning Up Resources ==="
            
            # Remove temporary files
            rm -rf /tmp/nodejs-hello-world
            
            # Keep the deployed container running but clean up build artifacts
            docker image prune -f
            
            echo "✅ Cleanup completed"

environment:
  NODE_ENV: "production"
  PORT: "8080"
  BUILD_ID: "${TIMESTAMP}"

timeout: 1800
retry_count: 1
EOF

    log_success "Pipeline configuration created: ${TEST_RESULTS_DIR}/nodejs-pipeline.yaml"
}

# Function to upload pipeline
upload_pipeline() {
    log_step "Uploading pipeline to RustCI"
    
    local response=$(curl -X POST "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines" \
        -H "Authorization: Bearer ${AUTH_TOKEN}" \
        -F "pipeline=@${TEST_RESULTS_DIR}/nodejs-pipeline.yaml" \
        -F "name=${PIPELINE_NAME}" \
        -s -w "HTTP_STATUS:%{http_code}")
    
    local status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ] || [ "$status" = "201" ]; then
        log_success "Pipeline uploaded successfully"
        echo "Response: $body"
    else
        log_error "Failed to upload pipeline (Status: $status)"
        echo "Response: $body"
        return 1
    fi
}

# Function to trigger pipeline
trigger_pipeline() {
    log_step "Triggering pipeline execution"
    
    local response=$(curl -X POST "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/${PIPELINE_NAME}/trigger" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${AUTH_TOKEN}" \
        -d "{
            \"trigger_type\": \"manual\",
            \"environment\": {
                \"BUILD_ID\": \"${TIMESTAMP}\",
                \"NODE_ENV\": \"production\"
            }
        }" \
        -s -w "HTTP_STATUS:%{http_code}")
    
    local status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    local body=$(echo "$response" | sed '/HTTP_STATUS:/d')
    
    if [ "$status" = "200" ] || [ "$status" = "201" ]; then
        log_success "Pipeline triggered successfully"
        echo "Response: $body"
        
        # Extract execution ID if available
        local execution_id=$(echo "$body" | grep -o '"execution_id":"[^"]*"' | cut -d'"' -f4)
        if [ -n "$execution_id" ]; then
            log_info "Execution ID: ${execution_id}"
            echo "$execution_id" > "${TEST_RESULTS_DIR}/execution_id.txt"
        fi
        
        return 0
    else
        log_error "Failed to trigger pipeline (Status: $status)"
        echo "Response: $body"
        return 1
    fi
}

# Function to monitor pipeline execution
monitor_pipeline() {
    log_step "Monitoring pipeline execution"
    
    local execution_id=""
    if [ -f "${TEST_RESULTS_DIR}/execution_id.txt" ]; then
        execution_id=$(cat "${TEST_RESULTS_DIR}/execution_id.txt")
    fi
    
    local max_attempts=60  # 5 minutes with 5-second intervals
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        log_info "Checking execution status (attempt $attempt/$max_attempts)..."
        
        local status_response
        if [ -n "$execution_id" ]; then
            status_response=$(curl -s -H "Authorization: Bearer ${AUTH_TOKEN}" \
                "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/${PIPELINE_NAME}/executions/${execution_id}")
        else
            status_response=$(curl -s -H "Authorization: Bearer ${AUTH_TOKEN}" \
                "${RUSTCI_API_BASE}/workspaces/${WORKSPACE_ID}/pipelines/${PIPELINE_NAME}/executions")
        fi
        
        echo "Status response: $status_response"
        
        # Check if execution is complete (this is a simplified check)
        if echo "$status_response" | grep -q '"status":"completed"'; then
            log_success "Pipeline execution completed successfully!"
            return 0
        elif echo "$status_response" | grep -q '"status":"failed"'; then
            log_error "Pipeline execution failed!"
            return 1
        fi
        
        sleep 5
        ((attempt++))
    done
    
    log_warning "Pipeline monitoring timed out. Check RustCI dashboard for status."
    return 1
}

# Function to verify deployment
verify_deployment() {
    log_step "Verifying deployment"
    
    # Check if the application is running
    if docker ps | grep -q "nodejs-hello-world-prod"; then
        log_success "Application container is running"
        
        # Test the deployed application
        log_info "Testing deployed application..."
        sleep 5
        
        if curl -f http://localhost:3000/ | grep -i "hello"; then
            log_success "✅ Deployment verification passed!"
            log_info "Application is accessible at http://localhost:3000"
            return 0
        else
            log_error "❌ Application is not responding correctly"
            return 1
        fi
    else
        log_error "Application container is not running"
        return 1
    fi
}

# Function to show deployment status
show_deployment_status() {
    log_section "Deployment Status"
    
    echo "=== Docker Containers ==="
    docker ps | grep nodejs-hello-world || echo "No Node.js containers found"
    
    echo ""
    echo "=== Docker Images ==="
    docker images | grep nodejs-hello-world || echo "No Node.js images found"
    
    echo ""
    echo "=== Application Test ==="
    if curl -s http://localhost:3000/ >/dev/null 2>&1; then
        echo "✅ Application is accessible at http://localhost:3000"
    else
        echo "❌ Application is not accessible"
    fi
}

# Function to cleanup deployment
cleanup_deployment() {
    log_step "Cleaning up deployment"
    
    # Stop and remove containers
    docker stop nodejs-hello-world-prod 2>/dev/null || true
    docker rm nodejs-hello-world-prod 2>/dev/null || true
    
    # Remove images
    docker rmi nodejs-hello-world:${TIMESTAMP} 2>/dev/null || true
    docker rmi nodejs-hello-world:latest 2>/dev/null || true
    
    # Clean up temporary files
    rm -rf /tmp/nodejs-hello-world
    
    log_success "Cleanup completed"
}

# Function to show usage
show_usage() {
    cat << EOF
Node.js Hello World Deployment Test Script

Usage: $0 [OPTIONS] [COMMAND]

OPTIONS:
    -t, --token TOKEN    Authentication token (will prompt if not provided)
    --skip-auth         Skip authentication (for testing - will likely fail)
    -h, --help          Show this help message

COMMANDS:
    deploy      Run complete deployment test (default)
    status      Show current deployment status
    verify      Verify existing deployment
    cleanup     Clean up deployment resources
    help        Show this help message

EXAMPLES:
    $0                                    # Run complete deployment test (will prompt for token)
    $0 -t "your_token_here" deploy        # Run with provided token
    $0 --token "your_token_here"          # Run with provided token
    $0 status                             # Show deployment status
    $0 verify                             # Verify deployment
    $0 cleanup                            # Clean up resources

GETTING A TOKEN:
    Method 1 (Recommended):
    1. Run: ./scripts/get-admin-token.sh
    
    Method 2 (Manual OAuth):
    1. Open http://localhost:8000/api/sessions/oauth/github in your browser
    2. Complete GitHub OAuth authentication
    3. Copy the JWT token from the JSON response (starts with 'eyJ')
    4. Paste the token when prompted by this script

PREREQUISITES:
    - RustCI server running on localhost:8000
    - Docker installed and running
    - Internet connection for GitHub access
    - Valid authentication token

REPOSITORY:
    https://github.com/dockersamples/helloworld-demo-node.git

EOF
}

# Main execution function
main() {
    log_info "🚀 Starting Node.js Hello World deployment test"
    
    check_prerequisites
    create_workspace
    create_pipeline_config
    upload_pipeline
    
    if trigger_pipeline; then
        monitor_pipeline
        verify_deployment
    else
        log_error "Failed to trigger pipeline"
        exit 1
    fi
    
    show_deployment_status
    
    log_success "🎉 Node.js Hello World deployment test completed!"
    log_info "Application should be running at http://localhost:3000"
}

# Parse command line arguments
COMMAND="deploy"
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--token)
            AUTH_TOKEN="$2"
            shift 2
            ;;
        --skip-auth)
            SKIP_AUTH=true
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        deploy|status|verify|cleanup|help)
            COMMAND="$1"
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Handle command line arguments
case "$COMMAND" in
    "deploy")
        main
        ;;
    "status")
        show_deployment_status
        ;;
    "verify")
        verify_deployment
        ;;
    "cleanup")
        cleanup_deployment
        ;;
    "help")
        show_usage
        ;;
    *)
        log_error "Unknown command: $COMMAND"
        show_usage
        exit 1
        ;;
esac