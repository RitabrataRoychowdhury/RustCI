#!/bin/bash

# AMD64 Image Building Script
# Builds RustCI Docker images for AMD64 architecture using buildx
# Designed for cross-compilation from ARM Mac to AMD64 VPS deployment

set -euo pipefail

# Configuration
DEFAULT_IMAGE_NAME="rustci"
DEFAULT_TAG="local-amd64"
PLATFORM="linux/amd64"
BUILDER_NAME="multiarch-builder"
BUILD_CONTEXT="."
DOCKERFILE="Dockerfile"

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

# Show usage information
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Build AMD64 Docker images using buildx for cross-architecture deployment.

OPTIONS:
    -i, --image NAME        Image name (default: $DEFAULT_IMAGE_NAME)
    -t, --tag TAG          Image tag (default: $DEFAULT_TAG)
    -f, --dockerfile FILE  Dockerfile path (default: $DOCKERFILE)
    -c, --context PATH     Build context path (default: $BUILD_CONTEXT)
    --no-cache             Build without using cache
    --push                 Push to registry instead of loading locally
    --registry URL         Registry URL for push (requires --push)
    -h, --help             Show this help message

EXAMPLES:
    # Basic AMD64 build
    $0

    # Custom image name and tag
    $0 -i myapp -t v1.0.0

    # Build and push to registry
    $0 --push --registry docker.io/myuser

    # Build without cache
    $0 --no-cache

REQUIREMENTS:
    - Docker buildx must be set up (run setup-buildx-multiarch.sh first)
    - Builder '$BUILDER_NAME' must be active
    - Dockerfile must exist in the specified context

EOF
}

# Parse command line arguments
parse_args() {
    IMAGE_NAME="$DEFAULT_IMAGE_NAME"
    TAG="$DEFAULT_TAG"
    USE_CACHE=true
    PUSH_IMAGE=false
    REGISTRY_URL=""
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -i|--image)
                IMAGE_NAME="$2"
                shift 2
                ;;
            -t|--tag)
                TAG="$2"
                shift 2
                ;;
            -f|--dockerfile)
                DOCKERFILE="$2"
                shift 2
                ;;
            -c|--context)
                BUILD_CONTEXT="$2"
                shift 2
                ;;
            --no-cache)
                USE_CACHE=false
                shift
                ;;
            --push)
                PUSH_IMAGE=true
                shift
                ;;
            --registry)
                REGISTRY_URL="$2"
                shift 2
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
    
    # Construct full image name
    if [[ -n "$REGISTRY_URL" && "$PUSH_IMAGE" == true ]]; then
        FULL_IMAGE_NAME="${REGISTRY_URL}/${IMAGE_NAME}:${TAG}"
    else
        FULL_IMAGE_NAME="${IMAGE_NAME}:${TAG}"
    fi
}

# Validate prerequisites
validate_prerequisites() {
    log_info "Validating prerequisites..."
    
    # Check Docker daemon
    if ! docker info >/dev/null 2>&1; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    # Check if buildx is available
    if ! docker buildx version >/dev/null 2>&1; then
        log_error "Docker buildx is not available"
        exit 1
    fi
    
    # Check if builder exists and is active
    local active_builder_line=$(docker buildx ls | grep '\*' | head -1)
    local active_builder=$(echo "$active_builder_line" | awk '{print $1}' | sed 's/\*//' || echo "")
    if [[ "$active_builder" != "$BUILDER_NAME" ]]; then
        log_error "Builder '$BUILDER_NAME' is not active. Current: $active_builder"
        log_info "Run 'scripts/deployment/setup-buildx-multiarch.sh' to set up the builder"
        exit 1
    fi
    
    # Check if Dockerfile exists
    if [[ ! -f "$BUILD_CONTEXT/$DOCKERFILE" ]]; then
        log_error "Dockerfile not found: $BUILD_CONTEXT/$DOCKERFILE"
        exit 1
    fi
    
    # Check platform support
    local platforms_output=$(docker buildx inspect | grep "Platforms:" | sed 's/Platforms: //')
    if [[ "$platforms_output" != *"linux/amd64"* ]]; then
        log_error "Builder does not support linux/amd64 platform"
        exit 1
    fi
    
    log_success "Prerequisites validated"
}

# Build the image
build_image() {
    log_info "Building AMD64 image: $FULL_IMAGE_NAME"
    log_info "Platform: $PLATFORM"
    log_info "Context: $BUILD_CONTEXT"
    log_info "Dockerfile: $DOCKERFILE"
    
    # Construct build command
    local build_cmd="docker buildx build"
    build_cmd+=" --platform $PLATFORM"
    build_cmd+=" -f $BUILD_CONTEXT/$DOCKERFILE"
    build_cmd+=" -t $FULL_IMAGE_NAME"
    
    if [[ "$USE_CACHE" == false ]]; then
        build_cmd+=" --no-cache"
        log_info "Cache disabled"
    fi
    
    if [[ "$PUSH_IMAGE" == true ]]; then
        build_cmd+=" --push"
        log_info "Image will be pushed to registry"
    else
        build_cmd+=" --load"
        log_info "Image will be loaded locally"
    fi
    
    build_cmd+=" $BUILD_CONTEXT"
    
    log_info "Executing: $build_cmd"
    echo ""
    
    # Record start time
    local start_time=$(date +%s)
    
    # Execute build
    if eval "$build_cmd"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "Build completed successfully in ${duration}s"
    else
        log_error "Build failed"
        exit 1
    fi
}

# Verify the built image (only for local builds)
verify_image() {
    if [[ "$PUSH_IMAGE" == true ]]; then
        log_info "Image pushed to registry - skipping local verification"
        return 0
    fi
    
    log_info "Verifying built image..."
    
    # Check if image exists locally
    if ! docker image inspect "$FULL_IMAGE_NAME" >/dev/null 2>&1; then
        log_error "Built image not found locally: $FULL_IMAGE_NAME"
        exit 1
    fi
    
    # Verify architecture
    local arch_info=$(docker image inspect --format='{{.Architecture}}/{{.Os}}' "$FULL_IMAGE_NAME")
    log_info "Image architecture: $arch_info"
    
    if [[ "$arch_info" == "amd64/linux" ]]; then
        log_success "✓ Correct architecture: amd64/linux"
    else
        log_error "✗ Incorrect architecture: $arch_info (expected: amd64/linux)"
        exit 1
    fi
    
    # Get image size
    local image_size=$(docker image inspect --format='{{.Size}}' "$FULL_IMAGE_NAME")
    local size_mb=$((image_size / 1024 / 1024))
    log_info "Image size: ${size_mb}MB"
    
    # Get image ID
    local image_id=$(docker image inspect --format='{{.Id}}' "$FULL_IMAGE_NAME" | cut -d: -f2 | cut -c1-12)
    log_info "Image ID: $image_id"
    
    log_success "Image verification completed"
}

# Display next steps
show_next_steps() {
    echo ""
    log_success "AMD64 image build completed successfully!"
    echo ""
    log_info "Next steps:"
    
    if [[ "$PUSH_IMAGE" == true ]]; then
        echo "  1. Image is available in registry: $FULL_IMAGE_NAME"
        echo "  2. Deploy to AMD64 VPS using: docker pull $FULL_IMAGE_NAME"
    else
        echo "  1. Test locally: scripts/deployment/test-amd64-compatibility.sh -i $FULL_IMAGE_NAME"
        echo "  2. Transfer to VPS: scripts/deployment/transfer-image-to-vps.sh -i $FULL_IMAGE_NAME"
        echo "  3. Validate architecture: scripts/deployment/validate-image-architecture.sh -i $FULL_IMAGE_NAME"
    fi
    
    echo ""
    log_info "Image details:"
    echo "  Name: $FULL_IMAGE_NAME"
    echo "  Platform: $PLATFORM"
    echo "  Builder: $BUILDER_NAME"
}

# Main execution
main() {
    log_info "Starting AMD64 image build process..."
    echo ""
    
    validate_prerequisites
    build_image
    verify_image
    show_next_steps
}

# Handle script interruption
trap 'log_error "Build interrupted"; exit 1' INT TERM

# Parse arguments and run
parse_args "$@"
main