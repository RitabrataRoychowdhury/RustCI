#!/bin/bash

# RustCI Rollback Script
# Usage: ./rollback.sh [environment] [version]

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOYMENTS_DIR="$(dirname "$SCRIPT_DIR")"

# Default values
ENVIRONMENT="${1:-production}"
VERSION="${2:-previous}"

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
RustCI Rollback Script

Usage: $0 [environment] [version]

ARGUMENTS:
    environment    Target environment (production, staging, development)
    version        Version to rollback to (previous, specific-version, backup-timestamp)

EXAMPLES:
    $0                           # Rollback production to previous version
    $0 staging                   # Rollback staging to previous version
    $0 production 20241109_143000 # Rollback to specific backup

EOF
}

# Load environment variables
load_environment() {
    log_info "Loading environment configuration..."
    
    if [[ -f "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env" ]]; then
        set -a
        source "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env"
        set +a
        log_success "Environment variables loaded"
    else
        log_error "Environment file not found: $DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env"
        exit 1
    fi
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking rollback prerequisites..."
    
    # Check required tools
    local required_tools=("sshpass" "curl")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            log_error "Required tool not found: $tool"
            exit 1
        fi
    done
    
    # Check VPS connectivity
    if ! sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
        "${VPS_USERNAME}@${VPS_IP}" "echo 'Connection test successful'" &> /dev/null; then
        log_error "Cannot connect to VPS: ${VPS_IP}"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Get available backups
get_available_backups() {
    log_info "Getting available backups..."
    
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        if [ -d /opt/rustci/backups ]; then
            echo 'Available backups:'
            ls -la /opt/rustci/backups/ | grep -E '\.yml$' | tail -10
        else
            echo 'No backup directory found'
        fi
    "
}

# Perform rollback
perform_rollback() {
    log_step "Starting rollback process..."
    
    case "$ENVIRONMENT" in
        "production")
            rollback_production
            ;;
        "staging")
            rollback_staging
            ;;
        "development")
            rollback_development
            ;;
        *)
            log_error "Unknown environment: $ENVIRONMENT"
            exit 1
            ;;
    esac
}

# Production rollback (blue-green)
rollback_production() {
    log_info "Performing production rollback using blue-green strategy..."
    
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        # Determine current active slot
        if docker ps --format 'table {{.Names}}' | grep -q 'rustci-blue'; then
            CURRENT_SLOT='blue'
            ROLLBACK_SLOT='green'
        else
            CURRENT_SLOT='green'
            ROLLBACK_SLOT='blue'
        fi
        
        echo \"Current slot: \$CURRENT_SLOT, Rolling back to: \$ROLLBACK_SLOT\"
        
        # Check if rollback slot has containers
        if docker ps -a --format 'table {{.Names}}' | grep -q \"rustci-\$ROLLBACK_SLOT\"; then
            echo 'Starting rollback slot containers...'
            docker start rustci-\$ROLLBACK_SLOT || true
            docker start valkyrie-\$ROLLBACK_SLOT || true
            
            # Wait for health check
            sleep 30
            
            # Determine rollback port
            if [ \"\$ROLLBACK_SLOT\" = 'blue' ]; then
                ROLLBACK_PORT=8080
            else
                ROLLBACK_PORT=8081
            fi
            
            # Health check rollback slot with RustCI endpoints
            if curl -f http://localhost:\$ROLLBACK_PORT/api/healthchecker 2>/dev/null; then
                echo '✅ Rollback slot is healthy (primary endpoint)'
            elif curl -f http://localhost:\$ROLLBACK_PORT/health 2>/dev/null; then
                echo '✅ Rollback slot is healthy (fallback endpoint)'
            else
                echo '❌ Rollback slot failed health check on both endpoints'
                exit 1
            fi
                
                # Stop current slot
                docker stop rustci-\$CURRENT_SLOT || true
                docker stop valkyrie-\$CURRENT_SLOT || true
                
                echo '✅ Rollback completed successfully'
            else
                echo '❌ Rollback slot failed health check'
                exit 1
            fi
        else
            echo '❌ No previous deployment found for rollback'
            exit 1
        fi
    "
}

# Staging rollback
rollback_staging() {
    log_info "Performing staging rollback..."
    
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        # Check if backup exists
        if [ -f /opt/rustci/backups/rustci-staging-backup.tar ]; then
            echo 'Loading backup image...'
            docker load -i /opt/rustci/backups/rustci-staging-backup.tar
            
            # Stop current staging
            docker stop rustci-staging valkyrie-staging 2>/dev/null || true
            docker rm rustci-staging valkyrie-staging 2>/dev/null || true
            
            # Start rollback version
            docker run -d --name rustci-staging \
                -p 8082:8000 \
                -e RUST_ENV=staging \
                rustci:staging-backup
            
            docker run -d --name valkyrie-staging \
                -p 9092:9090 \
                -e VALKYRIE_ENV=staging \
                rustci:staging-backup
            
            echo '✅ Staging rollback completed'
        else
            echo '❌ No staging backup found'
            exit 1
        fi
    "
}

# Development rollback
rollback_development() {
    log_info "Performing development rollback..."
    
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no "${VPS_USERNAME}@${VPS_IP}" "
        # For development, just restart with last known good image
        docker stop rustci-dev valkyrie-dev 2>/dev/null || true
        docker rm rustci-dev valkyrie-dev 2>/dev/null || true
        
        # Use previous tag if available
        if docker images | grep -q 'rustci.*previous'; then
            echo 'Using previous development image...'
            docker tag rustci:previous rustci:dev
        else
            echo 'No previous image found, using current'
        fi
        
        # Restart development containers
        docker run -d --name rustci-dev \
            -p 8083:8000 \
            -e RUST_ENV=development \
            rustci:dev
        
        docker run -d --name valkyrie-dev \
            -p 9093:9090 \
            -e VALKYRIE_ENV=development \
            rustci:dev
        
        echo '✅ Development rollback completed'
    "
}

# Verify rollback
verify_rollback() {
    log_step "Verifying rollback..."
    
    local port
    case "$ENVIRONMENT" in
        "production") port="8080" ;;
        "staging") port="8082" ;;
        "development") port="8083" ;;
    esac
    
    log_info "Testing application on port $port..."
    
    for i in {1..10}; do
        # Try primary RustCI health endpoint first, then fallback
        if curl -f "http://${VPS_IP}:${port}/api/healthchecker" &> /dev/null; then
            log_success "✅ Rollback verification successful (primary endpoint)"
            return 0
        elif curl -f "http://${VPS_IP}:${port}/health" &> /dev/null; then
            log_success "✅ Rollback verification successful (fallback endpoint)"
            return 0
        fi
        echo "Waiting for RustCI application to respond... ($i/10)"
        sleep 10
    done
    
    log_error "❌ Rollback verification failed"
    return 1
}

# Main execution
main() {
    log_info "RustCI Rollback Script"
    echo "======================"
    echo "Environment: $ENVIRONMENT"
    echo "Version: $VERSION"
    echo ""
    
    load_environment
    check_prerequisites
    
    if [[ "$VERSION" == "list" ]]; then
        get_available_backups
        exit 0
    fi
    
    perform_rollback
    
    if verify_rollback; then
        log_success "🎉 Rollback completed successfully!"
        log_info "Application is available at: http://${VPS_IP}:${port}"
    else
        log_error "❌ Rollback verification failed"
        exit 1
    fi
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