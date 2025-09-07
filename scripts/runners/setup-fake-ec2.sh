#!/bin/bash

# Setup Fake EC2-like Environment for RustCI Testing
# Creates multiple Docker containers that simulate EC2 instances

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
NETWORK_NAME="rustci-ec2-network"
BASE_NAME="rustci-ec2"
NUM_INSTANCES=3

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker is not running"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Create network
create_network() {
    log_info "Creating network: $NETWORK_NAME"
    
    if docker network ls | grep -q "$NETWORK_NAME"; then
        log_info "Network already exists"
    else
        docker network create "$NETWORK_NAME"
        log_success "Network created"
    fi
}

# Create fake EC2 instances
create_instances() {
    log_info "Creating $NUM_INSTANCES fake EC2 instances..."
    
    for i in $(seq 1 $NUM_INSTANCES); do
        local instance_name="${BASE_NAME}-${i}"
        
        if docker ps -a | grep -q "$instance_name"; then
            log_info "Instance $instance_name already exists, removing..."
            docker stop "$instance_name" 2>/dev/null || true
            docker rm "$instance_name" 2>/dev/null || true
        fi
        
        # Create instance with SSH, Docker, and common tools
        docker run -d \
            --name "$instance_name" \
            --network "$NETWORK_NAME" \
            --privileged \
            -p "$((2200 + i)):22" \
            -p "$((8080 + i)):8080" \
            -e SSH_ENABLE_ROOT=true \
            -e SSH_ENABLE_PASSWORD_AUTH=true \
            -e SSH_ROOT_PASSWORD=rustci123 \
            -v /var/run/docker.sock:/var/run/docker.sock \
            --restart unless-stopped \
            ubuntu:22.04 \
            bash -c "
                apt-get update && 
                apt-get install -y openssh-server docker.io curl git build-essential nodejs npm python3 && 
                service ssh start && 
                echo 'root:rustci123' | chpasswd && 
                mkdir -p /var/run/sshd && 
                sed -i 's/#PermitRootLogin prohibit-password/PermitRootLogin yes/' /etc/ssh/sshd_config && 
                sed -i 's/#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config && 
                service ssh restart && 
                tail -f /dev/null
            "
        
        log_success "Created instance: $instance_name (SSH: localhost:$((2200 + i)), HTTP: localhost:$((8080 + i)))"
    done
}

# Show status
show_status() {
    log_info "Fake EC2 Environment Status:"
    echo ""
    echo "Network: $NETWORK_NAME"
    echo "Instances:"
    
    for i in $(seq 1 $NUM_INSTANCES); do
        local instance_name="${BASE_NAME}-${i}"
        if docker ps | grep -q "$instance_name"; then
            echo "  ✅ $instance_name - SSH: localhost:$((2200 + i)) - HTTP: localhost:$((8080 + i))"
        else
            echo "  ❌ $instance_name - Not running"
        fi
    done
    
    echo ""
    echo "SSH Access: ssh root@localhost -p PORT (password: rustci123)"
    echo "Example: ssh root@localhost -p 2201"
}

# Test connectivity
test_connectivity() {
    log_info "Testing connectivity to instances..."
    
    for i in $(seq 1 $NUM_INSTANCES); do
        local instance_name="${BASE_NAME}-${i}"
        local port=$((2200 + i))
        
        if docker exec "$instance_name" echo "Instance $i is responsive" 2>/dev/null; then
            log_success "Instance $instance_name is responsive"
        else
            log_error "Instance $instance_name is not responsive"
        fi
    done
}

# Cleanup
cleanup() {
    log_info "Cleaning up fake EC2 environment..."
    
    for i in $(seq 1 $NUM_INSTANCES); do
        local instance_name="${BASE_NAME}-${i}"
        docker stop "$instance_name" 2>/dev/null || true
        docker rm "$instance_name" 2>/dev/null || true
    done
    
    docker network rm "$NETWORK_NAME" 2>/dev/null || true
    log_success "Cleanup completed"
}

# Show usage
show_usage() {
    cat << EOF
Fake EC2 Environment Setup Script

Usage: $0 [COMMAND]

COMMANDS:
    start       Start fake EC2 environment (default)
    stop        Stop all instances
    status      Show environment status
    test        Test connectivity to instances
    cleanup     Remove all instances and network
    help        Show this help message

EXAMPLES:
    $0              # Start environment
    $0 status       # Show status
    $0 cleanup      # Remove everything

ACCESS:
    SSH: ssh root@localhost -p PORT (password: rustci123)
    Ports: 2201, 2202, 2203 for SSH
    HTTP: localhost:8081, 8082, 8083

EOF
}

# Main execution
main() {
    check_prerequisites
    create_network
    create_instances
    
    # Wait for instances to be ready
    log_info "Waiting for instances to be ready..."
    sleep 30
    
    test_connectivity
    show_status
    
    log_success "🎉 Fake EC2 environment is ready!"
}

# Handle command line arguments
case "${1:-start}" in
    "start"|"")
        main
        ;;
    "stop")
        log_info "Stopping all instances..."
        for i in $(seq 1 $NUM_INSTANCES); do
            docker stop "${BASE_NAME}-${i}" 2>/dev/null || true
        done
        log_success "All instances stopped"
        ;;
    "status")
        show_status
        ;;
    "test")
        test_connectivity
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