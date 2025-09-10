#!/bin/bash

# DIND to Fake Server Deployment Setup Script
# Sets up complete environment for deploying from DIND runner to fake server

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTCI_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
PIPELINE_FILE="$RUSTCI_DIR/dind-to-fake-server-pipeline.yaml"

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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing_tools=()
    
    for tool in docker curl jq; do
        if ! command -v "$tool" &> /dev/null; then
            missing_tools+=("$tool")
        fi
    done
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Setup fake servers
setup_fake_servers() {
    log_step "Setting up fake servers..."
    
    if [[ -f "$SCRIPT_DIR/setup-fake-ec2.sh" ]]; then
        log_info "Starting fake EC2 environment..."
        "$SCRIPT_DIR/setup-fake-ec2.sh" start
        
        # Wait for servers to be ready
        log_info "Waiting for fake servers to be ready..."
        sleep 45
        
        # Test connectivity
        log_info "Testing fake server connectivity..."
        if docker exec rustci-ec2-1 echo "Fake server is ready" &> /dev/null; then
            log_success "Fake servers are ready"
        else
            log_error "Fake servers are not responding"
            exit 1
        fi
    else
        log_error "Fake EC2 setup script not found"
        exit 1
    fi
}

# Setup DIND simulation
setup_dind_simulation() {
    log_step "Setting up DIND server-runner simulation..."
    
    if [[ -f "$SCRIPT_DIR/dind-server-runner-simulation.sh" ]]; then
        log_info "Starting DIND simulation..."
        "$SCRIPT_DIR/dind-server-runner-simulation.sh" start
        
        # Wait for simulation to be ready
        log_info "Waiting for DIND simulation to be ready..."
        sleep 60
        
        # Test DIND simulation
        log_info "Testing DIND simulation..."
        if curl -s http://localhost:8080/health &> /dev/null; then
            log_success "DIND simulation is ready"
        else
            log_error "DIND simulation is not responding"
            exit 1
        fi
    else
        log_error "DIND simulation script not found"
        exit 1
    fi
}

# Verify network connectivity between DIND and fake servers
verify_network_connectivity() {
    log_step "Verifying network connectivity..."
    
    # Test if DIND runner can reach fake server
    log_info "Testing DIND to fake server connectivity..."
    
    if docker exec rustci-runner-sim ping -c 3 rustci-ec2-1 &> /dev/null; then
        log_success "DIND runner can reach fake server"
    else
        log_warning "Direct ping failed, checking if containers are on same network..."
        
        # Check if both containers are on the same network
        local dind_networks=$(docker inspect rustci-runner-sim --format '{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}')
        local fake_networks=$(docker inspect rustci-ec2-1 --format '{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}')
        
        log_info "DIND networks: $dind_networks"
        log_info "Fake server networks: $fake_networks"
        
        # Connect DIND runner to fake server network if needed
        if ! echo "$dind_networks" | grep -q "rustci-ec2-network"; then
            log_info "Connecting DIND runner to fake server network..."
            docker network connect rustci-ec2-network rustci-runner-sim
            sleep 5
            
            if docker exec rustci-runner-sim ping -c 3 rustci-ec2-1 &> /dev/null; then
                log_success "Network connectivity established"
            else
                log_error "Failed to establish network connectivity"
                exit 1
            fi
        fi
    fi
}

# Deploy the pipeline
deploy_pipeline() {
    log_step "Deploying the pipeline..."
    
    if [[ ! -f "$PIPELINE_FILE" ]]; then
        log_error "Pipeline file not found: $PIPELINE_FILE"
        exit 1
    fi
    
    log_info "Submitting pipeline to RustCI server..."
    
    # Submit pipeline via API
    local response=$(curl -s -X POST \
        -H "Content-Type: application/yaml" \
        --data-binary @"$PIPELINE_FILE" \
        "http://localhost:8080/api/pipelines/run" || echo "")
    
    if [[ -n "$response" ]]; then
        log_success "Pipeline submitted successfully"
        echo "Response: $response"
        
        # Extract pipeline ID if available
        local pipeline_id=$(echo "$response" | jq -r '.pipeline_id // .id // empty' 2>/dev/null || echo "")
        if [[ -n "$pipeline_id" && "$pipeline_id" != "null" ]]; then
            log_info "Pipeline ID: $pipeline_id"
            echo "$pipeline_id" > /tmp/dind-fake-pipeline-id
            
            # Monitor pipeline execution
            monitor_pipeline "$pipeline_id"
        else
            log_warning "Could not extract pipeline ID, but pipeline may be running"
        fi
    else
        log_error "Failed to submit pipeline"
        exit 1
    fi
}

# Monitor pipeline execution
monitor_pipeline() {
    local pipeline_id="$1"
    local max_wait=1800  # 30 minutes
    local waited=0
    
    log_info "Monitoring pipeline execution..."
    
    while [[ $waited -lt $max_wait ]]; do
        local status_response=$(curl -s "http://localhost:8080/api/pipelines/$pipeline_id/status" || echo "")
        
        if [[ -n "$status_response" ]]; then
            local status=$(echo "$status_response" | jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")
            echo "Pipeline status: $status (waited ${waited}s)"
            
            case "$status" in
                "completed"|"success")
                    log_success "Pipeline completed successfully!"
                    return 0
                    ;;
                "failed"|"error")
                    log_error "Pipeline failed!"
                    echo "Error details: $status_response"
                    return 1
                    ;;
                "running"|"pending"|"in_progress")
                    log_info "Pipeline is still running..."
                    ;;
                *)
                    log_info "Pipeline status: $status"
                    ;;
            esac
        else
            log_warning "Could not get pipeline status"
        fi
        
        sleep 30
        waited=$((waited + 30))
    done
    
    log_error "Pipeline monitoring timed out after $max_wait seconds"
    return 1
}

# Verify deployment
verify_deployment() {
    log_step "Verifying deployment..."
    
    log_info "Checking if application is running on fake server..."
    
    # Test application on fake server
    local app_response=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/ 2>/dev/null || echo "FAILED")
    
    if echo "$app_response" | grep -q "Hello World"; then
        log_success "✅ Application is running and responding correctly!"
        echo "Application response: $app_response"
        
        # Test health endpoint
        local health_response=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/health 2>/dev/null || echo "FAILED")
        if echo "$health_response" | grep -q "healthy"; then
            log_success "✅ Health endpoint is working correctly!"
            echo "Health response: $health_response"
        else
            log_warning "Health endpoint not responding correctly"
        fi
        
        return 0
    else
        log_error "❌ Application is not responding correctly"
        echo "Response received: $app_response"
        
        # Debug information
        log_info "Debugging deployment..."
        docker exec rustci-ec2-1 docker ps -a | grep nodejs || echo "No nodejs containers found"
        docker exec rustci-ec2-1 docker logs nodejs-hello-world 2>/dev/null || echo "No logs available"
        
        return 1
    fi
}

# Show deployment status
show_status() {
    log_info "Deployment Status Summary"
    echo "=========================="
    
    echo ""
    echo "🖥️  Infrastructure Status:"
    
    # Check fake servers
    if docker ps | grep -q rustci-ec2-1; then
        echo "  ✅ Fake Server (rustci-ec2-1): Running"
        echo "     SSH: ssh root@localhost -p 2201 (password: rustci123)"
    else
        echo "  ❌ Fake Server: Not running"
    fi
    
    # Check DIND simulation
    if docker ps | grep -q rustci-server-sim; then
        echo "  ✅ RustCI Server: Running (http://localhost:8080)"
    else
        echo "  ❌ RustCI Server: Not running"
    fi
    
    if docker ps | grep -q rustci-runner-sim; then
        echo "  ✅ DIND Runner: Running"
    else
        echo "  ❌ DIND Runner: Not running"
    fi
    
    echo ""
    echo "🚀 Application Status:"
    
    # Check application on fake server
    local app_status=$(docker exec rustci-ec2-1 curl -s http://localhost:3000/ 2>/dev/null || echo "FAILED")
    if echo "$app_status" | grep -q "Hello World"; then
        echo "  ✅ Node.js Application: Running on fake server"
        echo "     URL: http://localhost:3000 (from within fake server)"
    else
        echo "  ❌ Node.js Application: Not responding"
    fi
    
    echo ""
    echo "🔧 Access Information:"
    echo "  • RustCI Server: http://localhost:8080"
    echo "  • Fake Server SSH: ssh root@localhost -p 2201"
    echo "  • Test Application: docker exec rustci-ec2-1 curl http://localhost:3000/"
    
    echo ""
    echo "📋 Next Steps:"
    echo "  1. SSH into fake server: ssh root@localhost -p 2201"
    echo "  2. Check application: curl http://localhost:3000/"
    echo "  3. View logs: docker logs nodejs-hello-world"
    echo "  4. Monitor containers: docker ps"
}

# Cleanup function
cleanup() {
    log_step "Cleaning up environment..."
    
    # Stop DIND simulation
    if [[ -f "$SCRIPT_DIR/dind-server-runner-simulation.sh" ]]; then
        "$SCRIPT_DIR/dind-server-runner-simulation.sh" cleanup
    fi
    
    # Stop fake servers
    if [[ -f "$SCRIPT_DIR/setup-fake-ec2.sh" ]]; then
        "$SCRIPT_DIR/setup-fake-ec2.sh" cleanup
    fi
    
    # Clean up temporary files
    rm -f /tmp/dind-fake-pipeline-id
    
    log_success "Cleanup completed"
}

# Show usage
show_usage() {
    cat << EOF
DIND to Fake Server Deployment Setup

This script sets up a complete environment for testing deployment from a DIND runner
to a fake server, including:
- Fake EC2-like servers for deployment targets
- DIND server-runner simulation environment
- Network connectivity between components
- Automated pipeline deployment and monitoring

Usage: $0 [COMMAND]

COMMANDS:
    setup       Set up complete environment and deploy (default)
    deploy      Deploy pipeline only (requires existing setup)
    verify      Verify deployment status
    status      Show current status
    cleanup     Clean up all resources
    help        Show this help message

EXAMPLES:
    $0              # Complete setup and deployment
    $0 deploy       # Deploy pipeline to existing environment
    $0 verify       # Check if deployment is working
    $0 status       # Show current status
    $0 cleanup      # Clean up everything

COMPONENTS:
    • Fake Servers: Docker containers simulating EC2 instances
    • DIND Runner: Docker-in-Docker runner for isolated builds
    • RustCI Server: Pipeline orchestration server
    • Node.js App: Simple Hello World application for testing

ACCESS:
    • RustCI Server: http://localhost:8080
    • Fake Server SSH: ssh root@localhost -p 2201 (password: rustci123)
    • Application: curl http://localhost:3000/ (from within fake server)

EOF
}

# Main execution
main() {
    log_info "Starting DIND to Fake Server Deployment Setup"
    echo "=============================================="
    
    check_prerequisites
    setup_fake_servers
    setup_dind_simulation
    verify_network_connectivity
    deploy_pipeline
    
    if verify_deployment; then
        log_success "🎉 Complete deployment setup successful!"
        show_status
    else
        log_error "❌ Deployment verification failed"
        show_status
        exit 1
    fi
}

# Handle command line arguments
case "${1:-setup}" in
    "setup"|"")
        main
        ;;
    "deploy")
        check_prerequisites
        if ! curl -s http://localhost:8080/health &> /dev/null; then
            log_error "RustCI server is not running. Run 'setup' first."
            exit 1
        fi
        deploy_pipeline
        verify_deployment
        ;;
    "verify")
        verify_deployment
        ;;
    "status")
        show_status
        ;;
    "cleanup")
        cleanup
        ;;
    "help"|"-h"|"--help")
        show_usage
        ;;
    *)
        log_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac