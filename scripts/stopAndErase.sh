#!/bin/bash

# RustCI Cleanup Script
# Stops and removes all RustCI containers, volumes, and networks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Default values
REMOVE_VOLUMES=false
REMOVE_IMAGES=false
FORCE=false

# Function to show usage
show_usage() {
    cat << EOF
RustCI Cleanup Script

Usage: $0 [OPTIONS]

OPTIONS:
    -v, --volumes          Remove volumes (WARNING: This will delete all data)
    -i, --images           Remove Docker images
    -f, --force            Force removal without confirmation
    -h, --help             Show this help message

EXAMPLES:
    $0                     # Stop containers only
    $0 -v                  # Stop containers and remove volumes
    $0 -v -i               # Stop containers, remove volumes and images
    $0 -f -v -i            # Force cleanup everything

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--volumes)
            REMOVE_VOLUMES=true
            shift
            ;;
        -i|--images)
            REMOVE_IMAGES=true
            shift
            ;;
        -f|--force)
            FORCE=true
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

print_status "Starting RustCI cleanup..."

# Confirmation prompt
if [ "$FORCE" = false ]; then
    echo -n "This will stop and remove RustCI containers"
    if [ "$REMOVE_VOLUMES" = true ]; then
        echo -n " and volumes (DATA WILL BE LOST)"
    fi
    if [ "$REMOVE_IMAGES" = true ]; then
        echo -n " and images"
    fi
    echo ". Continue? (y/N): "
    read -r response
    if [[ ! "$response" =~ ^[Yy]$ ]]; then
        print_status "Cleanup cancelled"
        exit 0
    fi
fi

# Stop and remove single-node deployment
if [ -f "docker-compose.yaml" ]; then
    print_status "Stopping single-node deployment..."
    docker-compose down
    
    if [ "$REMOVE_VOLUMES" = true ]; then
        print_status "Removing single-node volumes..."
        docker-compose down -v
    fi
fi

# Stop and remove multi-node deployment
if [ -f "docker-compose.multi-node.yaml" ]; then
    print_status "Stopping multi-node deployment..."
    docker-compose -f docker-compose.multi-node.yaml down
    
    if [ "$REMOVE_VOLUMES" = true ]; then
        print_status "Removing multi-node volumes..."
        docker-compose -f docker-compose.multi-node.yaml down -v
    fi
fi

# Remove any remaining RustCI containers
print_status "Removing any remaining RustCI containers..."
docker ps -a --filter "name=rustci" --format "{{.Names}}" | while read -r container; do
    if [ -n "$container" ]; then
        print_status "Removing container: $container"
        docker rm -f "$container" 2>/dev/null || true
    fi
done

# Remove test containers
print_status "Removing test containers..."
docker rm -f rustci-test-mongodb 2>/dev/null || true

# Remove volumes if requested
if [ "$REMOVE_VOLUMES" = true ]; then
    print_warning "Removing RustCI volumes (data will be lost)..."
    
    # Remove named volumes
    docker volume ls --filter "name=rustci" --format "{{.Name}}" | while read -r volume; do
        if [ -n "$volume" ]; then
            print_status "Removing volume: $volume"
            docker volume rm "$volume" 2>/dev/null || true
        fi
    done
    
    # Remove local data directories
    if [ -d "data" ]; then
        print_status "Removing local data directory..."
        rm -rf data
    fi
    
    # Remove target directory (build artifacts)
    if [ -d "target" ]; then
        print_status "Removing target directory..."
        rm -rf target
    fi
fi

# Remove images if requested
if [ "$REMOVE_IMAGES" = true ]; then
    print_status "Removing RustCI Docker images..."
    
    # Remove RustCI images
    docker images --filter "reference=rustci*" --format "{{.Repository}}:{{.Tag}}" | while read -r image; do
        if [ -n "$image" ]; then
            print_status "Removing image: $image"
            docker rmi "$image" 2>/dev/null || true
        fi
    done
    
    # Remove dangling images
    print_status "Removing dangling images..."
    docker image prune -f
fi

# Remove networks
print_status "Removing RustCI networks..."
docker network ls --filter "name=rustci" --format "{{.Name}}" | while read -r network; do
    if [ -n "$network" ]; then
        print_status "Removing network: $network"
        docker network rm "$network" 2>/dev/null || true
    fi
done

# Clean up any remaining Docker resources
print_status "Cleaning up remaining Docker resources..."
docker system prune -f

# Remove log files
if [ -d "logs" ]; then
    print_status "Removing log files..."
    rm -rf logs
fi

# Remove temporary files
print_status "Removing temporary files..."
find . -name "*.tmp" -delete 2>/dev/null || true
find . -name "*.log" -delete 2>/dev/null || true

print_success "RustCI cleanup completed!"

# Show what was cleaned up
print_status "Cleanup summary:"
print_status "✓ Stopped and removed all RustCI containers"
print_status "✓ Removed RustCI networks"
print_status "✓ Cleaned up temporary files"

if [ "$REMOVE_VOLUMES" = true ]; then
    print_status "✓ Removed all volumes and data"
fi

if [ "$REMOVE_IMAGES" = true ]; then
    print_status "✓ Removed Docker images"
fi

print_status ""
print_status "To start RustCI again, run:"
print_status "  ./scripts/deploy.sh                    # Local development"
print_status "  ./scripts/deploy.sh -t single-node     # Single node"
print_status "  ./scripts/deploy.sh -t multi-node      # Multi-node cluster"