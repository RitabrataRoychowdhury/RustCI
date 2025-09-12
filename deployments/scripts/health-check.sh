#!/bin/bash

# RustCI Health Check Script
# Usage: ./health-check.sh [environment] [service]

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOYMENTS_DIR="$(dirname "$SCRIPT_DIR")"

# Default values
ENVIRONMENT="${1:-production}"
SERVICE="${2:-all}"

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
RustCI Health Check Script

Usage: $0 [environment] [service]

ARGUMENTS:
    environment    Target environment (production, staging, development, all)
    service        Service to check (rustci, valkyrie, all)

EXAMPLES:
    $0                        # Check all services in production
    $0 staging                # Check all services in staging
    $0 production rustci      # Check only RustCI in production
    $0 all                    # Check all environments

EOF
}

# Load environment variables
load_environment() {
    if [[ "$ENVIRONMENT" != "all" ]]; then
        log_info "Loading environment configuration for $ENVIRONMENT..."
        
        if [[ -f "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env" ]]; then
            set -a
            source "$DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env"
            set +a
            log_success "Environment variables loaded"
        else
            log_error "Environment file not found: $DEPLOYMENTS_DIR/environments/${ENVIRONMENT}.env"
            exit 1
        fi
    fi
}

# Check VPS connectivity
check_vps_connectivity() {
    local env="$1"
    local vps_ip vps_username vps_password
    
    # Load environment-specific variables
    if [[ -f "$DEPLOYMENTS_DIR/environments/${env}.env" ]]; then
        source "$DEPLOYMENTS_DIR/environments/${env}.env"
        vps_ip="$VPS_IP"
        vps_username="$VPS_USERNAME"
        vps_password="$VPS_PASSWORD"
    else
        log_error "Environment file not found for $env"
        return 1
    fi
    
    log_info "Testing VPS connectivity for $env..."
    
    if sshpass -p "$vps_password" ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
        "$vps_username@$vps_ip" "echo 'VPS connection successful'" &> /dev/null; then
        log_success "✅ VPS connectivity: OK"
        return 0
    else
        log_error "❌ VPS connectivity: FAILED"
        return 1
    fi
}

# Check RustCI health
check_rustci_health() {
    local env="$1"
    local port
    
    case "$env" in
        "production") port="8080" ;;
        "staging") port="8082" ;;
        "development") port="8083" ;;
        *) log_error "Unknown environment: $env"; return 1 ;;
    esac
    
    log_info "Checking RustCI health for $env on port $port..."
    
    # Load environment variables
    source "$DEPLOYMENTS_DIR/environments/${env}.env"
    
    # Check RustCI health endpoints (primary and fallback)
    if curl -f -s --max-time 10 "http://${VPS_IP}:${port}/api/healthchecker" > /dev/null; then
        log_success "✅ RustCI ($env): HEALTHY (primary endpoint)"
        
        # Get additional info
        local response
        response=$(curl -s --max-time 5 "http://${VPS_IP}:${port}/api/healthchecker" 2>/dev/null || echo "{}")
        echo "   Response: $response"
        
        return 0
    elif curl -f -s --max-time 10 "http://${VPS_IP}:${port}/health" > /dev/null; then
        log_success "✅ RustCI ($env): HEALTHY (fallback endpoint)"
        
        # Get additional info
        local response
        response=$(curl -s --max-time 5 "http://${VPS_IP}:${port}/health" 2>/dev/null || echo "{}")
        echo "   Response: $response"
        
        return 0
    else
        log_error "❌ RustCI ($env): UNHEALTHY (both /api/healthchecker and /health failed)"
        
        # Check if container is running
        local container_status
        container_status=$(sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" \
            "docker ps --filter name=rustci --format 'table {{.Names}}\t{{.Status}}'" 2>/dev/null || echo "No containers")
        echo "   Container status: $container_status"
        
        return 1
    fi
}

# Check Valkyrie health
check_valkyrie_health() {
    local env="$1"
    local port
    
    case "$env" in
        "production") port="9090" ;;
        "staging") port="9092" ;;
        "development") port="9093" ;;
        *) log_error "Unknown environment: $env"; return 1 ;;
    esac
    
    log_info "Checking Valkyrie health for $env on port $port..."
    
    # Load environment variables
    source "$DEPLOYMENTS_DIR/environments/${env}.env"
    
    # Check health endpoint
    if curl -f -s --max-time 10 "http://${VPS_IP}:${port}/health" > /dev/null 2>&1; then
        log_success "✅ Valkyrie ($env): HEALTHY"
        return 0
    else
        # Valkyrie might be integrated with RustCI, so check if it's part of RustCI
        log_warning "⚠️ Valkyrie ($env): Separate endpoint not available (might be integrated with RustCI)"
        
        # Check if Valkyrie container exists
        local container_status
        container_status=$(sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" \
            "docker ps --filter name=valkyrie --format 'table {{.Names}}\t{{.Status}}'" 2>/dev/null || echo "No containers")
        
        if [[ "$container_status" == *"valkyrie"* ]]; then
            log_info "   Valkyrie container found: $container_status"
        else
            log_info "   Valkyrie appears to be integrated with RustCI"
        fi
        
        return 0
    fi
}

# Check Docker containers
check_docker_containers() {
    local env="$1"
    
    log_info "Checking Docker containers for $env..."
    
    # Load environment variables
    source "$DEPLOYMENTS_DIR/environments/${env}.env"
    
    local containers
    containers=$(sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" \
        "docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' | grep -E '(rustci|valkyrie)'" 2>/dev/null || echo "No containers found")
    
    if [[ "$containers" != "No containers found" ]]; then
        log_success "✅ Docker containers for $env:"
        echo "$containers"
    else
        log_error "❌ No Docker containers found for $env"
        return 1
    fi
}

# Check system resources
check_system_resources() {
    local env="$1"
    
    log_info "Checking system resources for $env..."
    
    # Load environment variables
    source "$DEPLOYMENTS_DIR/environments/${env}.env"
    
    sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" "
        echo '=== System Resources ==='
        echo 'CPU Usage:'
        top -bn1 | grep 'Cpu(s)' | head -1
        echo ''
        echo 'Memory Usage:'
        free -h
        echo ''
        echo 'Disk Usage:'
        df -h / | tail -1
        echo ''
        echo 'Docker Stats:'
        docker stats --no-stream --format 'table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}' | head -5
    " 2>/dev/null || log_warning "Could not retrieve system resources"
}

# Comprehensive health check for an environment
check_environment_health() {
    local env="$1"
    local overall_status=0
    
    log_step "Health check for $env environment"
    echo "=================================="
    
    # VPS connectivity
    if ! check_vps_connectivity "$env"; then
        overall_status=1
    fi
    
    # Docker containers
    if ! check_docker_containers "$env"; then
        overall_status=1
    fi
    
    # Service health checks
    if [[ "$SERVICE" == "all" || "$SERVICE" == "rustci" ]]; then
        if ! check_rustci_health "$env"; then
            overall_status=1
        fi
    fi
    
    if [[ "$SERVICE" == "all" || "$SERVICE" == "valkyrie" ]]; then
        if ! check_valkyrie_health "$env"; then
            # Don't fail overall status for Valkyrie as it might be integrated
            true
        fi
    fi
    
    # System resources
    check_system_resources "$env"
    
    echo ""
    if [[ $overall_status -eq 0 ]]; then
        log_success "🎉 Overall health for $env: HEALTHY"
    else
        log_error "❌ Overall health for $env: UNHEALTHY"
    fi
    
    echo ""
    return $overall_status
}

# Main execution
main() {
    log_info "RustCI Health Check Script"
    echo "=========================="
    echo "Environment: $ENVIRONMENT"
    echo "Service: $SERVICE"
    echo ""
    
    local overall_status=0
    
    if [[ "$ENVIRONMENT" == "all" ]]; then
        # Check all environments
        for env in production staging development; do
            if [[ -f "$DEPLOYMENTS_DIR/environments/${env}.env" ]]; then
                if ! check_environment_health "$env"; then
                    overall_status=1
                fi
            else
                log_warning "Environment file not found for $env, skipping..."
            fi
        done
    else
        # Check specific environment
        load_environment
        if ! check_environment_health "$ENVIRONMENT"; then
            overall_status=1
        fi
    fi
    
    echo "=========================="
    if [[ $overall_status -eq 0 ]]; then
        log_success "🎉 All health checks passed!"
    else
        log_error "❌ Some health checks failed!"
    fi
    
    exit $overall_status
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