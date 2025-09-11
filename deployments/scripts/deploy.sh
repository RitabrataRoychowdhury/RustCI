#!/bin/bash

# RustCI Deployment Script
# Usage: ./deploy.sh [environment] [strategy] [config_file]

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOYMENTS_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$DEPLOYMENTS_DIR")"

# Default values
ENVIRONMENT="${1:-production}"
STRATEGY="${2:-blue-green}"
CONFIG_FILE="${3:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

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
    echo -e "${PURPLE}[STEP]${NC} $1"
}

# Show usage
show_usage() {
    cat << EOF
RustCI Deployment Script

Usage: $0 [environment] [strategy] [config_file]

ARGUMENTS:
    environment    Target environment (production, staging, development)
    strategy       Deployment strategy (blue-green, rolling, canary, recreate)
    config_file    Optional custom configuration file

EXAMPLES:
    $0                                    # Deploy to production with blue-green
    $0 staging rolling                    # Deploy to staging with rolling strategy
    $0 production canary custom.yaml     # Deploy with custom config

AVAILABLE ENVIRONMENTS:
    - production    Production VPS deployment
    - staging       Staging environment
    - development   Development environment

AVAILABLE STRATEGIES:
    - blue-green    Zero-downtime deployment with traffic switching
    - rolling       Gradual instance replacement
    - canary        Gradual traffic shifting with monitoring
    - recreate      Simple stop-and-start deployment

EOF
}

# Validate inputs
validate_inputs() {
    log_info "Validating deployment inputs..."
    
    # Check environment
    if [[ ! -f "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env" ]]; then
        log_error "Environment file not found: $DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env"
        exit 1
    fi
    
    # Check strategy
    if [[ ! -f "$DEPLOYMENTS_DIR/strategies/${STRATEGY}.yaml" ]]; then
        log_error "Strategy file not found: $DEPLOYMENTS_DIR/strategies/${STRATEGY}.yaml"
        exit 1
    fi
    
    # Check VPS-specific config if using production
    if [[ "$ENVIRONMENT" == "production" && ! -f "$DEPLOYMENTS_DIR/vps/production.yaml" ]]; then
        log_error "VPS production config not found: $DEPLOYMENTS_DIR/vps/production.yaml"
        exit 1
    fi
    
    log_success "Input validation passed"
}

# Load environment variables
load_environment() {
    log_info "Loading environment configuration..."
    
    # Load environment variables
    if [[ -f "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env" ]]; then
        set -a  # automatically export all variables
        source "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env"
        set +a
        log_success "Environment variables loaded from ${ENVIRONMENT}.env"
    fi
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking deployment prerequisites..."
    
    # Check required tools
    local required_tools=("docker" "curl" "sshpass" "jq")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            log_error "Required tool not found: $tool"
            exit 1
        fi
    done
    
    # Check VPS connectivity
    if [[ -n "${VPS_IP:-}" ]]; then
        log_info "Testing VPS connectivity..."
        if ! sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
            "${VPS_USERNAME}@${VPS_IP}" "echo 'Connection test successful'" &> /dev/null; then
            log_error "Cannot connect to VPS: ${VPS_IP}"
            exit 1
        fi
        log_success "VPS connectivity verified"
    fi
    
    log_success "Prerequisites check passed"
}

# Execute deployment
execute_deployment() {
    log_step "Starting deployment execution..."
    
    local config_file
    if [[ -n "$CONFIG_FILE" ]]; then
        config_file="$CONFIG_FILE"
    elif [[ "$ENVIRONMENT" == "production" ]]; then
        config_file="$DEPLOYMENTS_DIR/vps/production.yaml"
    else
        log_error "No configuration file specified for environment: $ENVIRONMENT"
        exit 1
    fi
    
    log_info "Using configuration file: $config_file"
    
    # Check if RustCI is available to run the pipeline
    if command -v rustci &> /dev/null; then
        log_info "Executing deployment using RustCI..."
        rustci pipeline run --file "$config_file" --env "$ENVIRONMENT"
    else
        log_warning "RustCI CLI not available, using manual execution..."
        manual_deployment "$config_file"
    fi
}

# Manual deployment execution
manual_deployment() {
    local config_file="$1"
    
    log_info "Executing manual deployment..."
    
    # This is a simplified manual execution
    # In a real scenario, you'd parse the YAML and execute steps
    
    case "$STRATEGY" in
        "blue-green")
            execute_blue_green_deployment
            ;;
        "rolling")
            execute_rolling_deployment
            ;;
        "canary")
            execute_canary_deployment
            ;;
        "recreate")
            execute_recreate_deployment
            ;;
        *)
            log_error "Unknown deployment strategy: $STRATEGY"
            exit 1
            ;;
    esac
}

# Blue-green deployment execution
execute_blue_green_deployment() {
    log_step "Executing blue-green deployment..."
    
    # Determine deployment slot
    local current_slot
    current_slot=$(sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" '
        if docker ps --format "table {{.Names}}" | grep -q "rustci-blue"; then
            echo "green"
        else
            echo "blue"
        fi
    ' 2>/dev/null || echo "blue")
    
    log_info "Deploying to $current_slot slot"
    
    # Build and transfer images
    log_info "Building and transferring Docker images..."
    cd "$PROJECT_ROOT"
    docker build -t rustci:latest .
    docker save rustci:latest | gzip | \
        sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" \
        "gunzip | docker load"
    
    # Deploy to slot
    log_info "Deploying to $current_slot slot..."
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        # Stop old container in this slot
        docker stop rustci-$current_slot 2>/dev/null || true
        docker rm rustci-$current_slot 2>/dev/null || true
        
        # Start new container
        docker run -d --name rustci-$current_slot \
            -p \$([ '$current_slot' = 'blue' ] && echo '8080' || echo '8081'):8000 \
            -e MONGODB_URI='${MONGODB_URI}' \
            -e MONGODB_DATABASE='${MONGODB_DATABASE}' \
            -e JWT_SECRET='${JWT_SECRET}' \
            -e RUST_ENV='${RUST_ENV}' \
            -e RUST_LOG='${RUST_LOG}' \
            rustci:latest
    "
    
    # Health check
    local port
    port=$([ "$current_slot" = "blue" ] && echo "8080" || echo "8081")
    log_info "Performing health check on port $port..."
    
    for i in {1..10}; do
        if curl -f "http://${VPS_IP}:${port}/api/healthchecker" &> /dev/null; then
            log_success "Health check passed"
            break
        fi
        if [ $i -eq 10 ]; then
            log_error "Health check failed"
            exit 1
        fi
        sleep 10
    done
    
    # Switch traffic (simplified - in production use proper load balancer)
    log_info "Switching traffic to $current_slot slot..."
    local old_slot
    old_slot=$([ "$current_slot" = "blue" ] && echo "green" || echo "blue")
    
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        # Stop old slot
        docker stop rustci-$old_slot 2>/dev/null || true
        docker rm rustci-$old_slot 2>/dev/null || true
    "
    
    log_success "Blue-green deployment completed successfully"
}

# Placeholder functions for other strategies
execute_rolling_deployment() {
    log_info "Rolling deployment not yet implemented in manual mode"
    log_info "Please use RustCI CLI or implement manual rolling logic"
}

execute_canary_deployment() {
    log_info "Canary deployment not yet implemented in manual mode"
    log_info "Please use RustCI CLI or implement manual canary logic"
}

execute_recreate_deployment() {
    log_step "Executing recreate deployment..."
    
    log_info "Building and transferring Docker image..."
    cd "$PROJECT_ROOT"
    docker build -t rustci:latest .
    docker save rustci:latest | gzip | \
        sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" \
        "gunzip | docker load"
    
    log_info "Stopping current deployment..."
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        docker stop rustci 2>/dev/null || true
        docker rm rustci 2>/dev/null || true
    "
    
    log_info "Starting new deployment..."
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        docker run -d --name rustci -p 8080:8000 \
            -e MONGODB_URI='${MONGODB_URI}' \
            -e MONGODB_DATABASE='${MONGODB_DATABASE}' \
            -e JWT_SECRET='${JWT_SECRET}' \
            -e RUST_ENV='${RUST_ENV}' \
            -e RUST_LOG='${RUST_LOG}' \
            rustci:latest
    "
    
    # Health check
    log_info "Performing health check..."
    for i in {1..10}; do
        if curl -f "http://${VPS_IP}:8080/api/healthchecker" &> /dev/null; then
            log_success "Health check passed"
            break
        fi
        if [ $i -eq 10 ]; then
            log_error "Health check failed"
            exit 1
        fi
        sleep 10
    done
    
    log_success "Recreate deployment completed successfully"
}

# Main execution
main() {
    log_info "RustCI Deployment Script"
    echo "========================="
    echo "Environment: $ENVIRONMENT"
    echo "Strategy: $STRATEGY"
    echo "Config: ${CONFIG_FILE:-auto}"
    echo ""
    
    validate_inputs
    load_environment
    check_prerequisites
    execute_deployment
    
    log_success "🎉 Deployment completed successfully!"
    log_info "Application should be available at: http://${VPS_IP}:8080"
}

# Handle command line arguments
case "${1:-}" in
    "help"|"-h"|"--help")
        show_usage
        exit 0
        ;;
    *)
        main
        ;;
esac