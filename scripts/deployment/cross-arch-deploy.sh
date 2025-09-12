#!/bin/bash

# Cross-Architecture Deployment Orchestrator
# Combines SSH transfer and VPS deployment with comprehensive error handling and rollback

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Default configuration
TESTING_MODE=true
VPS_IP=""
VPS_USERNAME=""
VPS_PASSWORD=""
IMAGE_NAME="rustci:production"
CONTAINER_NAME="rustci"
CONTAINER_PORT=8080
ENV_FILE=""
SKIP_TRANSFER=false
SKIP_HEALTH_CHECK=false
ROLLBACK_ENABLED=true
VERBOSE=false

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
Cross-Architecture Deployment Orchestrator

Usage: $0 [OPTIONS]

OPTIONS:
    --vps-ip IP            VPS IP address (default: from .env)
    --vps-user USER        VPS username (default: from .env)
    --vps-password PASS    VPS password (default: from .env)
    -i, --image IMAGE      Docker image name (default: rustci:production)
    -c, --container NAME   Container name (default: rustci)
    -p, --port PORT        Container port (default: 8080)
    -e, --env-file FILE    Environment file to use
    --skip-transfer        Skip image transfer (assume image exists on VPS)
    --skip-health          Skip health checks
    --no-rollback          Disable automatic rollback on failure
    --production           Enable production mode (disable testing mode)
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

ENVIRONMENT VARIABLES:
    VPS_IP                 VPS IP address
    VPS_USERNAME           VPS username  
    VPS_PASSWORD           VPS password
    TESTING_MODE           Enable testing mode (default: true)

EXAMPLES:
    $0                                    # Use .env configuration
    $0 --vps-ip 46.37.122.118 --vps-user root
    $0 -i rustci:latest --production
    $0 --skip-transfer -c rustci-app

DEPLOYMENT PROCESS:
    1. Validate configuration and secrets
    2. Build AMD64 image locally (if needed)
    3. Transfer image to VPS via SSH
    4. Deploy container with backup/rollback
    5. Perform health checks
    6. Rollback on failure (if enabled)

EOF
}

# Function to load environment variables
load_environment() {
    if [ -f .env ]; then
        print_status "Loading environment from .env file..."
        # Export variables from .env file
        set -a
        source .env
        set +a
        print_success "Environment loaded"
    else
        print_warning ".env file not found"
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --vps-ip)
            VPS_IP="$2"
            shift 2
            ;;
        --vps-user)
            VPS_USERNAME="$2"
            shift 2
            ;;
        --vps-password)
            VPS_PASSWORD="$2"
            shift 2
            ;;
        -i|--image)
            IMAGE_NAME="$2"
            shift 2
            ;;
        -c|--container)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        -p|--port)
            CONTAINER_PORT="$2"
            shift 2
            ;;
        -e|--env-file)
            ENV_FILE="$2"
            shift 2
            ;;
        --skip-transfer)
            SKIP_TRANSFER=true
            shift
            ;;
        --skip-health)
            SKIP_HEALTH_CHECK=true
            shift
            ;;
        --no-rollback)
            ROLLBACK_ENABLED=false
            shift
            ;;
        --production)
            TESTING_MODE=false
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Load environment variables
load_environment

# Use environment variables if not provided via command line
VPS_IP=${VPS_IP:-$VPS_IP}
VPS_USERNAME=${VPS_USERNAME:-$VPS_USERNAME}
VPS_PASSWORD=${VPS_PASSWORD:-$VPS_PASSWORD}
TESTING_MODE=${TESTING_MODE:-true}

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

# Function to validate configuration
validate_configuration() {
    print_status "Validating configuration..."
    
    local errors=()
    
    # Check required variables
    if [ -z "$VPS_IP" ]; then
        errors+=("VPS_IP is required")
    fi
    
    if [ -z "$VPS_USERNAME" ]; then
        errors+=("VPS_USERNAME is required")
    fi
    
    if [ -z "$VPS_PASSWORD" ]; then
        errors+=("VPS_PASSWORD is required")
    fi
    
    # Check testing mode
    if [ "$TESTING_MODE" = "true" ]; then
        print_warning "WARNING: Using hardcoded secrets in testing mode"
        print_warning "This is only suitable for testing environments"
        print_warning "For production, set TESTING_MODE=false and use proper secret management"
    else
        print_status "Production mode enabled - using environment variables for secrets"
        
        # In production mode, validate additional secrets
        if [ -z "$MONGODB_URI" ]; then
            errors+=("MONGODB_URI is required in production mode")
        fi
        
        if [ -z "$JWT_SECRET" ]; then
            errors+=("JWT_SECRET is required in production mode")
        fi
        
        if [ -z "$GITHUB_OAUTH_CLIENT_SECRET" ]; then
            errors+=("GITHUB_OAUTH_CLIENT_SECRET is required in production mode")
        fi
    fi
    
    # Check if required tools are available
    local missing_tools=()
    
    if ! command -v docker &> /dev/null; then
        missing_tools+=("docker")
    fi
    
    if ! command -v curl &> /dev/null; then
        missing_tools+=("curl")
    fi
    
    if [ -n "$VPS_PASSWORD" ] && ! command -v sshpass &> /dev/null; then
        missing_tools+=("sshpass (for password authentication)")
    fi
    
    # Report errors
    if [ ${#errors[@]} -ne 0 ]; then
        print_error "Configuration validation failed:"
        for error in "${errors[@]}"; do
            echo "  - $error"
        done
        exit 1
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        print_error "Missing required tools:"
        for tool in "${missing_tools[@]}"; do
            echo "  - $tool"
        done
        exit 1
    fi
    
    print_success "Configuration validation passed"
}

# Function to check if image exists locally
check_local_image() {
    local image=$1
    
    if docker image inspect "$image" >/dev/null 2>&1; then
        print_success "Image $image found locally"
        return 0
    else
        print_warning "Image $image not found locally"
        return 1
    fi
}

# Function to build image if needed
build_image_if_needed() {
    local image=$1
    
    if check_local_image "$image"; then
        return 0
    fi
    
    print_status "Image not found locally, checking if build script exists..."
    
    local build_script="$SCRIPT_DIR/build-amd64-image.sh"
    if [ -f "$build_script" ]; then
        print_status "Running build script: $build_script"
        
        if bash "$build_script"; then
            print_success "Image built successfully"
            return 0
        else
            print_error "Image build failed"
            return 1
        fi
    else
        print_error "Build script not found: $build_script"
        print_error "Please build the image manually or ensure the build script exists"
        return 1
    fi
}

# Function to transfer image to VPS
transfer_image() {
    local image=$1
    local vps_host="${VPS_USERNAME}@${VPS_IP}"
    
    print_status "Transferring image to VPS..."
    
    local transfer_script="$SCRIPT_DIR/ssh-transfer.sh"
    if [ ! -f "$transfer_script" ]; then
        print_error "Transfer script not found: $transfer_script"
        return 1
    fi
    
    local transfer_cmd="bash $transfer_script"
    
    if [ "$VERBOSE" = true ]; then
        transfer_cmd="$transfer_cmd -v"
    fi
    
    if [ -n "$VPS_PASSWORD" ]; then
        transfer_cmd="$transfer_cmd -p '$VPS_PASSWORD'"
    fi
    
    transfer_cmd="$transfer_cmd '$image' '$vps_host'"
    
    print_status "Executing: $transfer_cmd"
    
    if eval "$transfer_cmd"; then
        print_success "Image transfer completed"
        return 0
    else
        print_error "Image transfer failed"
        return 1
    fi
}

# Function to deploy on VPS
deploy_on_vps() {
    local image=$1
    local vps_host="${VPS_USERNAME}@${VPS_IP}"
    
    print_status "Deploying on VPS..."
    
    # Prepare deployment script
    local deploy_script="$SCRIPT_DIR/vps-deploy.sh"
    if [ ! -f "$deploy_script" ]; then
        print_error "Deployment script not found: $deploy_script"
        return 1
    fi
    
    # Copy deployment script to VPS
    print_status "Copying deployment script to VPS..."
    
    local copy_cmd
    if [ -n "$VPS_PASSWORD" ]; then
        copy_cmd="sshpass -p '$VPS_PASSWORD' scp -o StrictHostKeyChecking=no '$deploy_script' '$vps_host:/tmp/vps-deploy.sh'"
    else
        copy_cmd="scp -o StrictHostKeyChecking=no '$deploy_script' '$vps_host:/tmp/vps-deploy.sh'"
    fi
    
    if ! eval "$copy_cmd"; then
        print_error "Failed to copy deployment script to VPS"
        return 1
    fi
    
    # Prepare deployment command
    local deploy_cmd="bash /tmp/vps-deploy.sh"
    
    if [ "$VERBOSE" = true ]; then
        deploy_cmd="$deploy_cmd -v"
    fi
    
    if [ -n "$CONTAINER_NAME" ]; then
        deploy_cmd="$deploy_cmd -c '$CONTAINER_NAME'"
    fi
    
    if [ -n "$CONTAINER_PORT" ]; then
        deploy_cmd="$deploy_cmd -p '$CONTAINER_PORT'"
    fi
    
    if [ "$ROLLBACK_ENABLED" = false ]; then
        deploy_cmd="$deploy_cmd --no-rollback"
    fi
    
    if [ "$SKIP_HEALTH_CHECK" = true ]; then
        deploy_cmd="$deploy_cmd --skip-health"
    fi
    
    deploy_cmd="$deploy_cmd '$image'"
    
    # Execute deployment on VPS
    print_status "Executing deployment on VPS..."
    
    local ssh_cmd
    if [ -n "$VPS_PASSWORD" ]; then
        ssh_cmd="sshpass -p '$VPS_PASSWORD' ssh -o StrictHostKeyChecking=no '$vps_host' '$deploy_cmd'"
    else
        ssh_cmd="ssh -o StrictHostKeyChecking=no '$vps_host' '$deploy_cmd'"
    fi
    
    print_status "Executing: $ssh_cmd"
    
    if eval "$ssh_cmd"; then
        print_success "VPS deployment completed"
        return 0
    else
        print_error "VPS deployment failed"
        return 1
    fi
}

# Function to perform final health check
perform_final_health_check() {
    if [ "$SKIP_HEALTH_CHECK" = true ]; then
        print_status "Skipping final health check"
        return 0
    fi
    
    print_status "Performing final health check..."
    
    local health_url="http://${VPS_IP}:${CONTAINER_PORT}/api/healthchecker"
    local fallback_url="http://${VPS_IP}:${CONTAINER_PORT}/health"
    
    # Try primary endpoint
    if curl -f -s --max-time 10 "$health_url" >/dev/null 2>&1; then
        print_success "Primary health check passed: $health_url"
        return 0
    fi
    
    # Try fallback endpoint
    if curl -f -s --max-time 10 "$fallback_url" >/dev/null 2>&1; then
        print_success "Fallback health check passed: $fallback_url"
        return 0
    fi
    
    print_error "Final health check failed"
    print_error "Tried: $health_url"
    print_error "Tried: $fallback_url"
    return 1
}

# Function to display deployment summary
show_deployment_summary() {
    print_success "Cross-Architecture Deployment Summary"
    echo ""
    echo "Configuration:"
    echo "  VPS: ${VPS_USERNAME}@${VPS_IP}"
    echo "  Image: $IMAGE_NAME"
    echo "  Container: $CONTAINER_NAME"
    echo "  Port: $CONTAINER_PORT"
    echo "  Testing Mode: $TESTING_MODE"
    echo "  Rollback Enabled: $ROLLBACK_ENABLED"
    echo ""
    echo "Access URLs:"
    echo "  Application: http://${VPS_IP}:${CONTAINER_PORT}"
    echo "  Health Check: http://${VPS_IP}:${CONTAINER_PORT}/api/healthchecker"
    echo "  Fallback Health: http://${VPS_IP}:${CONTAINER_PORT}/health"
    echo ""
    echo "Verification Commands:"
    echo "  ssh ${VPS_USERNAME}@${VPS_IP} 'docker ps --filter name=${CONTAINER_NAME}'"
    echo "  ssh ${VPS_USERNAME}@${VPS_IP} 'docker logs ${CONTAINER_NAME}'"
    echo "  curl -f http://${VPS_IP}:${CONTAINER_PORT}/api/healthchecker"
}

# Main deployment function
main() {
    local start_time=$(date +%s)
    
    print_status "Cross-Architecture Deployment Orchestrator"
    print_status "Starting deployment process..."
    
    # Validate configuration
    validate_configuration
    
    # Build image if needed
    if ! build_image_if_needed "$IMAGE_NAME"; then
        print_error "Failed to ensure image availability"
        exit 1
    fi
    
    # Transfer image to VPS
    if [ "$SKIP_TRANSFER" = false ]; then
        if ! transfer_image "$IMAGE_NAME"; then
            print_error "Image transfer failed"
            exit 1
        fi
    else
        print_status "Skipping image transfer (--skip-transfer specified)"
    fi
    
    # Deploy on VPS
    if ! deploy_on_vps "$IMAGE_NAME"; then
        print_error "VPS deployment failed"
        exit 1
    fi
    
    # Perform final health check
    if ! perform_final_health_check; then
        print_error "Final health check failed"
        exit 1
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    print_success "Cross-architecture deployment completed successfully in ${duration}s"
    
    # Show deployment summary
    show_deployment_summary
}

# Set up signal handlers for cleanup
cleanup() {
    print_status "Cleaning up..."
    # Add any cleanup logic here
}

trap cleanup EXIT INT TERM

# Run main function
main "$@"