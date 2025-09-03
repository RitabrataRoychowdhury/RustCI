#!/bin/bash

# Docker-in-Docker (DinD) Environment Setup Script
# This script sets up a DinD environment for testing RustCI deployments

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
DIND_CONTAINER_NAME="rustci-dind"
DIND_PORT="2376"
DIND_NETWORK="rustci-network"

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

# Function to check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker is not running. Please start Docker first."
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Function to create Docker network
create_network() {
    log_info "Creating Docker network: ${DIND_NETWORK}"
    
    if docker network ls | grep -q "${DIND_NETWORK}"; then
        log_warning "Network ${DIND_NETWORK} already exists"
    else
        docker network create "${DIND_NETWORK}" || {
            log_error "Failed to create network ${DIND_NETWORK}"
            exit 1
        }
        log_success "Network ${DIND_NETWORK} created"
    fi
}

# Function to start DinD container
start_dind() {
    log_info "Starting Docker-in-Docker container: ${DIND_CONTAINER_NAME}"
    
    # Stop existing container if running
    if docker ps -a | grep -q "${DIND_CONTAINER_NAME}"; then
        log_info "Stopping existing ${DIND_CONTAINER_NAME} container..."
        docker stop "${DIND_CONTAINER_NAME}" 2>/dev/null || true
        docker rm "${DIND_CONTAINER_NAME}" 2>/dev/null || true
    fi
    
    # Start new DinD container
    docker run -d \
        --name "${DIND_CONTAINER_NAME}" \
        --privileged \
        --network "${DIND_NETWORK}" \
        -p "${DIND_PORT}:2376" \
        -e DOCKER_TLS_CERTDIR=/certs \
        -v rustci-dind-certs:/certs/client \
        -v rustci-dind-data:/var/lib/docker \
        docker:dind || {
        log_error "Failed to start DinD container"
        exit 1
    }
    
    log_success "DinD container started"
}

# Function to wait for DinD to be ready
wait_for_dind() {
    log_info "Waiting for DinD to be ready..."
    
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if docker exec "${DIND_CONTAINER_NAME}" docker version &>/dev/null; then
            log_success "DinD is ready!"
            return 0
        fi
        
        log_info "Attempt $attempt/$max_attempts - waiting for DinD..."
        sleep 5
        ((attempt++))
    done
    
    log_error "DinD failed to start within expected time"
    return 1
}

# Function to test DinD functionality
test_dind() {
    log_info "Testing DinD functionality..."
    
    # Test basic Docker commands
    log_info "Testing Docker version..."
    docker exec "${DIND_CONTAINER_NAME}" docker version
    
    log_info "Testing Docker info..."
    docker exec "${DIND_CONTAINER_NAME}" docker info | head -10
    
    log_info "Testing image pull..."
    docker exec "${DIND_CONTAINER_NAME}" docker pull hello-world:latest
    
    log_info "Testing container run..."
    docker exec "${DIND_CONTAINER_NAME}" docker run --rm hello-world
    
    log_success "DinD functionality test passed"
}

# Function to show DinD status
show_status() {
    log_info "DinD Environment Status:"
    echo "  Container Name: ${DIND_CONTAINER_NAME}"
    echo "  Network: ${DIND_NETWORK}"
    echo "  Port: ${DIND_PORT}"
    echo "  Docker Host: tcp://localhost:${DIND_PORT}"
    
    if docker ps | grep -q "${DIND_CONTAINER_NAME}"; then
        echo -e "  Status: ${GREEN}Running${NC}"
    else
        echo -e "  Status: ${RED}Not Running${NC}"
    fi
}

# Function to cleanup DinD environment
cleanup() {
    log_info "Cleaning up DinD environment..."
    
    # Stop and remove container
    docker stop "${DIND_CONTAINER_NAME}" 2>/dev/null || true
    docker rm "${DIND_CONTAINER_NAME}" 2>/dev/null || true
    
    # Remove volumes (optional)
    docker volume rm rustci-dind-certs 2>/dev/null || true
    docker volume rm rustci-dind-data 2>/dev/null || true
    
    # Remove network
    docker network rm "${DIND_NETWORK}" 2>/dev/null || true
    
    log_success "Cleanup completed"
}

# Function to show usage
show_usage() {
    cat << EOF
Docker-in-Docker (DinD) Environment Setup Script

Usage: $0 [COMMAND]

COMMANDS:
    start       Start DinD environment (default)
    stop        Stop DinD container
    restart     Restart DinD environment
    status      Show DinD status
    test        Test DinD functionality
    cleanup     Remove DinD environment completely
    help        Show this help message

EXAMPLES:
    $0              # Start DinD environment
    $0 start        # Start DinD environment
    $0 test         # Test DinD functionality
    $0 status       # Show current status
    $0 cleanup      # Remove everything

ENVIRONMENT VARIABLES:
    DIND_CONTAINER_NAME    Container name (default: rustci-dind)
    DIND_PORT             Port mapping (default: 2376)
    DIND_NETWORK          Docker network (default: rustci-network)

EOF
}

# Main execution
main() {
    check_prerequisites
    create_network
    start_dind
    wait_for_dind
    test_dind
    show_status
    
    log_success "🎉 DinD environment is ready for RustCI testing!"
    log_info "You can now use DOCKER_HOST=tcp://localhost:${DIND_PORT} for remote Docker operations"
}

# Handle command line arguments
case "${1:-start}" in
    "start"|"")
        main
        ;;
    "stop")
        log_info "Stopping DinD container..."
        docker stop "${DIND_CONTAINER_NAME}" 2>/dev/null || log_warning "Container not running"
        log_success "DinD container stopped"
        ;;
    "restart")
        log_info "Restarting DinD environment..."
        docker stop "${DIND_CONTAINER_NAME}" 2>/dev/null || true
        docker rm "${DIND_CONTAINER_NAME}" 2>/dev/null || true
        main
        ;;
    "status")
        show_status
        ;;
    "test")
        if docker ps | grep -q "${DIND_CONTAINER_NAME}"; then
            test_dind
        else
            log_error "DinD container is not running. Start it first with: $0 start"
            exit 1
        fi
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