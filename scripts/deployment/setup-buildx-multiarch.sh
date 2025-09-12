#!/bin/bash

# Docker Buildx Multi-Platform Builder Setup Script
# This script sets up Docker buildx for cross-architecture compilation
# Specifically designed for building AMD64 images on ARM Mac systems

set -euo pipefail

# Configuration
BUILDER_NAME="multiarch-builder"
PLATFORMS="linux/amd64,linux/arm64"
DRIVER="docker-container"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
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

# Check if Docker is running
check_docker() {
    log_info "Checking Docker daemon status..."
    if ! docker info >/dev/null 2>&1; then
        log_error "Docker daemon is not running. Please start Docker and try again."
        exit 1
    fi
    log_success "Docker daemon is running"
}

# Check current architecture
check_architecture() {
    local arch=$(uname -m)
    log_info "Current system architecture: $arch"
    
    if [[ "$arch" == "arm64" ]]; then
        log_info "ARM64 system detected - buildx setup is recommended for cross-compilation"
    elif [[ "$arch" == "x86_64" ]]; then
        log_info "AMD64 system detected - buildx setup will enable multi-platform builds"
    else
        log_warning "Unknown architecture: $arch"
    fi
}

# Remove existing builder if it exists
cleanup_existing_builder() {
    log_info "Checking for existing builder: $BUILDER_NAME"
    
    if docker buildx ls | grep -E "^${BUILDER_NAME}[\*]?\s" >/dev/null 2>&1; then
        log_warning "Existing builder '$BUILDER_NAME' found. Removing..."
        docker buildx rm "$BUILDER_NAME" --force || true
        log_success "Existing builder removed"
    else
        log_info "No existing builder found"
    fi
}

# Create new multi-platform builder
create_builder() {
    log_info "Creating new multi-platform builder: $BUILDER_NAME"
    
    docker buildx create \
        --name "$BUILDER_NAME" \
        --driver "$DRIVER" \
        --platform "$PLATFORMS" \
        --use
    
    log_success "Builder '$BUILDER_NAME' created successfully"
}

# Bootstrap the builder
bootstrap_builder() {
    log_info "Bootstrapping builder (this may take a few minutes)..."
    
    docker buildx inspect --bootstrap
    
    log_success "Builder bootstrapped successfully"
}

# Verify builder setup
verify_builder() {
    log_info "Verifying builder setup..."
    
    # Check if builder is active (look for * in the name)
    local active_builder_line=$(docker buildx ls | grep '\*' | head -1)
    local active_builder=$(echo "$active_builder_line" | awk '{print $1}' | sed 's/\*//')
    if [[ "$active_builder" != "$BUILDER_NAME" ]]; then
        log_error "Builder '$BUILDER_NAME' is not active. Active builder: $active_builder"
        exit 1
    fi
    
    # Check supported platforms
    log_info "Supported platforms:"
    docker buildx inspect | grep "Platforms:" | sed 's/Platforms: /  /'
    
    # Verify specific platforms are available
    local platforms_output=$(docker buildx inspect | grep "Platforms:" | sed 's/Platforms: //')
    
    if [[ "$platforms_output" == *"linux/amd64"* ]]; then
        log_success "✓ linux/amd64 platform supported"
    else
        log_error "✗ linux/amd64 platform not supported"
        exit 1
    fi
    
    if [[ "$platforms_output" == *"linux/arm64"* ]]; then
        log_success "✓ linux/arm64 platform supported"
    else
        log_warning "✗ linux/arm64 platform not supported"
    fi
    
    log_success "Builder verification completed successfully"
}

# Display usage information
show_usage() {
    log_info "Builder setup completed! Usage examples:"
    echo ""
    echo "  Build for AMD64 only:"
    echo "    docker buildx build --platform linux/amd64 --load -t rustci:amd64 ."
    echo ""
    echo "  Build for multiple platforms:"
    echo "    docker buildx build --platform linux/amd64,linux/arm64 --push -t rustci:multiarch ."
    echo ""
    echo "  Build and load locally (single platform only):"
    echo "    docker buildx build --platform linux/amd64 --load -t rustci:local-amd64 ."
    echo ""
    log_warning "Note: --load flag only works with single platform builds"
    log_info "Use the architecture validation script to verify built images"
}

# Main execution
main() {
    log_info "Starting Docker buildx multi-platform builder setup..."
    echo ""
    
    check_docker
    check_architecture
    cleanup_existing_builder
    create_builder
    bootstrap_builder
    verify_builder
    
    echo ""
    log_success "Docker buildx multi-platform builder setup completed successfully!"
    echo ""
    show_usage
}

# Handle script interruption
trap 'log_error "Script interrupted"; exit 1' INT TERM

# Run main function
main "$@"