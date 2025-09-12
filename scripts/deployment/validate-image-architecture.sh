#!/bin/bash

# Image Architecture Validation Script
# Validates Docker image architecture using docker image inspect
# Ensures images are compatible with target deployment platforms

set -euo pipefail

# Configuration
DEFAULT_IMAGE_NAME="rustci:local-amd64"
EXPECTED_ARCH="amd64"
EXPECTED_OS="linux"

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

Validate Docker image architecture for cross-platform deployment compatibility.

OPTIONS:
    -i, --image NAME       Image name to validate (default: $DEFAULT_IMAGE_NAME)
    -a, --arch ARCH        Expected architecture (default: $EXPECTED_ARCH)
    -o, --os OS            Expected OS (default: $EXPECTED_OS)
    --detailed             Show detailed image information
    --json                 Output results in JSON format
    -h, --help             Show this help message

EXAMPLES:
    # Validate default image
    $0

    # Validate specific image
    $0 -i myapp:v1.0.0

    # Validate for ARM64
    $0 -i myapp:arm64 -a arm64

    # Show detailed information
    $0 --detailed

    # JSON output for automation
    $0 --json

VALIDATION CHECKS:
    ✓ Image exists locally or in registry
    ✓ Architecture matches expected value
    ✓ Operating system matches expected value
    ✓ Image metadata is accessible
    ✓ Platform compatibility verification

EOF
}

# Parse command line arguments
parse_args() {
    IMAGE_NAME="$DEFAULT_IMAGE_NAME"
    EXPECTED_ARCH="amd64"
    EXPECTED_OS="linux"
    SHOW_DETAILED=false
    JSON_OUTPUT=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -i|--image)
                IMAGE_NAME="$2"
                shift 2
                ;;
            -a|--arch)
                EXPECTED_ARCH="$2"
                shift 2
                ;;
            -o|--os)
                EXPECTED_OS="$2"
                shift 2
                ;;
            --detailed)
                SHOW_DETAILED=true
                shift
                ;;
            --json)
                JSON_OUTPUT=true
                shift
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
}

# Check if image exists
check_image_exists() {
    log_info "Checking if image exists: $IMAGE_NAME"
    
    if docker image inspect "$IMAGE_NAME" >/dev/null 2>&1; then
        log_success "✓ Image found locally"
        return 0
    else
        log_warning "Image not found locally, attempting to pull..."
        if docker pull "$IMAGE_NAME" >/dev/null 2>&1; then
            log_success "✓ Image pulled successfully"
            return 0
        else
            log_error "✗ Image not found locally or in registry: $IMAGE_NAME"
            return 1
        fi
    fi
}

# Extract image information
extract_image_info() {
    log_info "Extracting image information..."
    
    # Get basic architecture and OS info
    ACTUAL_ARCH=$(docker image inspect --format='{{.Architecture}}' "$IMAGE_NAME" 2>/dev/null || echo "unknown")
    ACTUAL_OS=$(docker image inspect --format='{{.Os}}' "$IMAGE_NAME" 2>/dev/null || echo "unknown")
    
    # Get additional metadata
    IMAGE_ID=$(docker image inspect --format='{{.Id}}' "$IMAGE_NAME" 2>/dev/null | cut -d: -f2 | cut -c1-12 || echo "unknown")
    IMAGE_SIZE=$(docker image inspect --format='{{.Size}}' "$IMAGE_NAME" 2>/dev/null || echo "0")
    CREATED_DATE=$(docker image inspect --format='{{.Created}}' "$IMAGE_NAME" 2>/dev/null || echo "unknown")
    
    # Calculate size in MB
    if [[ "$IMAGE_SIZE" != "0" && "$IMAGE_SIZE" != "unknown" ]]; then
        SIZE_MB=$((IMAGE_SIZE / 1024 / 1024))
    else
        SIZE_MB="unknown"
    fi
    
    # Get platform string
    PLATFORM_STRING="${ACTUAL_OS}/${ACTUAL_ARCH}"
    
    log_success "Image information extracted"
}

# Validate architecture
validate_architecture() {
    log_info "Validating architecture..."
    
    local validation_passed=true
    
    # Check architecture
    if [[ "$ACTUAL_ARCH" == "$EXPECTED_ARCH" ]]; then
        log_success "✓ Architecture matches: $ACTUAL_ARCH"
    else
        log_error "✗ Architecture mismatch: expected $EXPECTED_ARCH, got $ACTUAL_ARCH"
        validation_passed=false
    fi
    
    # Check OS
    if [[ "$ACTUAL_OS" == "$EXPECTED_OS" ]]; then
        log_success "✓ Operating system matches: $ACTUAL_OS"
    else
        log_error "✗ OS mismatch: expected $EXPECTED_OS, got $ACTUAL_OS"
        validation_passed=false
    fi
    
    # Check platform string
    local expected_platform="${EXPECTED_OS}/${EXPECTED_ARCH}"
    if [[ "$PLATFORM_STRING" == "$expected_platform" ]]; then
        log_success "✓ Platform string correct: $PLATFORM_STRING"
    else
        log_error "✗ Platform string incorrect: expected $expected_platform, got $PLATFORM_STRING"
        validation_passed=false
    fi
    
    return $([ "$validation_passed" = true ] && echo 0 || echo 1)
}

# Show detailed information
show_detailed_info() {
    if [[ "$SHOW_DETAILED" == false ]]; then
        return 0
    fi
    
    echo ""
    log_info "Detailed Image Information:"
    echo "  Image Name: $IMAGE_NAME"
    echo "  Image ID: $IMAGE_ID"
    echo "  Architecture: $ACTUAL_ARCH"
    echo "  Operating System: $ACTUAL_OS"
    echo "  Platform: $PLATFORM_STRING"
    echo "  Size: ${SIZE_MB}MB"
    echo "  Created: $CREATED_DATE"
    
    # Get additional Docker inspect info
    echo ""
    log_info "Docker Inspect Summary:"
    docker image inspect "$IMAGE_NAME" --format='  Config.Env: {{.Config.Env}}' 2>/dev/null || echo "  Config.Env: unavailable"
    docker image inspect "$IMAGE_NAME" --format='  Config.ExposedPorts: {{.Config.ExposedPorts}}' 2>/dev/null || echo "  Config.ExposedPorts: unavailable"
    docker image inspect "$IMAGE_NAME" --format='  Config.Cmd: {{.Config.Cmd}}' 2>/dev/null || echo "  Config.Cmd: unavailable"
    docker image inspect "$IMAGE_NAME" --format='  Config.WorkingDir: {{.Config.WorkingDir}}' 2>/dev/null || echo "  Config.WorkingDir: unavailable"
}

# Output JSON results
output_json() {
    if [[ "$JSON_OUTPUT" == false ]]; then
        return 0
    fi
    
    local validation_result
    if validate_architecture >/dev/null 2>&1; then
        validation_result="passed"
    else
        validation_result="failed"
    fi
    
    cat << EOF
{
  "image_name": "$IMAGE_NAME",
  "image_id": "$IMAGE_ID",
  "actual_architecture": "$ACTUAL_ARCH",
  "actual_os": "$ACTUAL_OS",
  "expected_architecture": "$EXPECTED_ARCH",
  "expected_os": "$EXPECTED_OS",
  "platform_string": "$PLATFORM_STRING",
  "size_bytes": $IMAGE_SIZE,
  "size_mb": $SIZE_MB,
  "created_date": "$CREATED_DATE",
  "validation_result": "$validation_result",
  "architecture_match": $([ "$ACTUAL_ARCH" == "$EXPECTED_ARCH" ] && echo "true" || echo "false"),
  "os_match": $([ "$ACTUAL_OS" == "$EXPECTED_OS" ] && echo "true" || echo "false")
}
EOF
}

# Perform compatibility checks
perform_compatibility_checks() {
    log_info "Performing compatibility checks..."
    
    # Check for common architecture compatibility issues
    case "$ACTUAL_ARCH" in
        "amd64")
            log_info "AMD64 architecture - compatible with most cloud providers and VPS"
            ;;
        "arm64")
            log_info "ARM64 architecture - compatible with ARM-based servers and Apple Silicon"
            ;;
        "386")
            log_warning "32-bit x86 architecture - limited compatibility"
            ;;
        *)
            log_warning "Unknown or uncommon architecture: $ACTUAL_ARCH"
            ;;
    esac
    
    # Check OS compatibility
    case "$ACTUAL_OS" in
        "linux")
            log_success "Linux OS - excellent container compatibility"
            ;;
        "windows")
            log_warning "Windows OS - requires Windows container host"
            ;;
        *)
            log_warning "Uncommon OS: $ACTUAL_OS"
            ;;
    esac
    
    # Size warnings
    if [[ "$SIZE_MB" != "unknown" && "$SIZE_MB" -gt 1000 ]]; then
        log_warning "Large image size (${SIZE_MB}MB) - consider optimization"
    elif [[ "$SIZE_MB" != "unknown" && "$SIZE_MB" -lt 50 ]]; then
        log_warning "Very small image size (${SIZE_MB}MB) - verify completeness"
    fi
}

# Main validation function
main() {
    if [[ "$JSON_OUTPUT" == false ]]; then
        log_info "Starting image architecture validation..."
        echo ""
    fi
    
    # Perform validation steps
    if ! check_image_exists; then
        exit 1
    fi
    
    extract_image_info
    
    if [[ "$JSON_OUTPUT" == true ]]; then
        output_json
        exit 0
    fi
    
    # Validate and show results
    if validate_architecture; then
        log_success "Architecture validation passed!"
        perform_compatibility_checks
        show_detailed_info
        
        echo ""
        log_success "Image $IMAGE_NAME is compatible with $EXPECTED_OS/$EXPECTED_ARCH platform"
        exit 0
    else
        log_error "Architecture validation failed!"
        show_detailed_info
        
        echo ""
        log_error "Image $IMAGE_NAME is NOT compatible with $EXPECTED_OS/$EXPECTED_ARCH platform"
        exit 1
    fi
}

# Handle script interruption
trap 'log_error "Validation interrupted"; exit 1' INT TERM

# Parse arguments and run
parse_args "$@"
main