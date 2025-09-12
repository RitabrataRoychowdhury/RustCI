#!/bin/bash

# Docker Buildx System Integration Test
# Tests the complete Docker buildx cross-compilation system
# Verifies all components work together correctly

set -euo pipefail

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

# Test configuration
TEST_IMAGE_NAME="rustci-buildx-test"
TEST_TAG="amd64-test"
FULL_TEST_IMAGE="${TEST_IMAGE_NAME}:${TEST_TAG}"

# Cleanup function
cleanup() {
    log_info "Cleaning up test resources..."
    
    # Remove test image
    docker rmi "$FULL_TEST_IMAGE" >/dev/null 2>&1 || true
    
    # Remove test container
    docker rm -f "rustci-buildx-test-container" >/dev/null 2>&1 || true
    
    log_success "Cleanup completed"
}

# Create a simple test Dockerfile
create_test_dockerfile() {
    log_info "Creating test Dockerfile..."
    
    cat > Dockerfile.buildx-test << 'EOF'
FROM alpine:latest

# Install basic tools for testing
RUN apk add --no-cache curl bash

# Create a simple test application
RUN echo '#!/bin/bash' > /test-app.sh && \
    echo 'echo "Architecture: $(uname -m)"' >> /test-app.sh && \
    echo 'echo "OS: $(uname -s)"' >> /test-app.sh && \
    echo 'echo "Test application running successfully"' >> /test-app.sh && \
    chmod +x /test-app.sh

# Expose a port for testing
EXPOSE 8080

# Set the test script as entrypoint
CMD ["/test-app.sh"]
EOF
    
    log_success "Test Dockerfile created"
}

# Test 1: Setup buildx multiarch
test_buildx_setup() {
    log_info "Test 1: Testing buildx multiarch setup..."
    
    if ./scripts/deployment/setup-buildx-multiarch.sh; then
        log_success "✓ Buildx setup completed successfully"
        return 0
    else
        log_error "✗ Buildx setup failed"
        return 1
    fi
}

# Test 2: Build AMD64 image
test_amd64_build() {
    log_info "Test 2: Testing AMD64 image build..."
    
    if ./scripts/deployment/build-amd64-image.sh \
        -i "$TEST_IMAGE_NAME" \
        -t "$TEST_TAG" \
        -f "Dockerfile.buildx-test" \
        -c "."; then
        log_success "✓ AMD64 image build completed successfully"
        return 0
    else
        log_error "✗ AMD64 image build failed"
        return 1
    fi
}

# Test 3: Validate image architecture
test_architecture_validation() {
    log_info "Test 3: Testing image architecture validation..."
    
    if ./scripts/deployment/validate-image-architecture.sh \
        -i "$FULL_TEST_IMAGE" \
        -a "amd64" \
        -o "linux"; then
        log_success "✓ Architecture validation passed"
        return 0
    else
        log_error "✗ Architecture validation failed"
        return 1
    fi
}

# Test 4: AMD64 compatibility testing
test_amd64_compatibility() {
    log_info "Test 4: Testing AMD64 compatibility..."
    
    if ./scripts/deployment/test-amd64-compatibility.sh \
        -i "$FULL_TEST_IMAGE" \
        -n "rustci-buildx-test-container" \
        --quick; then
        log_success "✓ AMD64 compatibility test passed"
        return 0
    else
        log_error "✗ AMD64 compatibility test failed"
        return 1
    fi
}

# Test 5: Verify buildx builder is working
test_builder_functionality() {
    log_info "Test 5: Testing builder functionality..."
    
    # Check if multiarch-builder is active
    local active_builder_line=$(docker buildx ls | grep '\*' | head -1)
    local active_builder=$(echo "$active_builder_line" | awk '{print $1}' | sed 's/\*//')
    if [[ "$active_builder" == "multiarch-builder" ]]; then
        log_success "✓ Multiarch builder is active"
    else
        log_error "✗ Multiarch builder is not active: $active_builder"
        return 1
    fi
    
    # Check supported platforms
    local platforms=$(docker buildx inspect | grep "Platforms:" | sed 's/Platforms: //')
    if [[ "$platforms" == *"linux/amd64"* ]]; then
        log_success "✓ AMD64 platform supported"
    else
        log_error "✗ AMD64 platform not supported"
        return 1
    fi
    
    return 0
}

# Test 6: JSON output validation
test_json_output() {
    log_info "Test 6: Testing JSON output..."
    
    local json_output
    if json_output=$(./scripts/deployment/validate-image-architecture.sh \
        -i "$FULL_TEST_IMAGE" \
        --json 2>/dev/null); then
        
        # Check if output is valid JSON
        if echo "$json_output" | python3 -m json.tool >/dev/null 2>&1; then
            log_success "✓ JSON output is valid"
            
            # Check specific fields
            if echo "$json_output" | grep -q '"validation_result": "passed"'; then
                log_success "✓ JSON validation result is correct"
                return 0
            else
                log_error "✗ JSON validation result is incorrect"
                return 1
            fi
        else
            log_error "✗ JSON output is invalid"
            return 1
        fi
    else
        log_error "✗ JSON output generation failed"
        return 1
    fi
}

# Run all tests
run_integration_tests() {
    local test_results=()
    local total_tests=6
    local passed_tests=0
    
    # Create test environment
    create_test_dockerfile
    
    # Run tests
    test_buildx_setup && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_amd64_build && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_architecture_validation && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_amd64_compatibility && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_builder_functionality && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    test_json_output && { test_results+=("✓"); ((passed_tests++)); } || test_results+=("✗")
    
    # Show results summary
    echo ""
    log_info "Integration Test Results:"
    echo "  1. Buildx Setup:           ${test_results[0]}"
    echo "  2. AMD64 Build:            ${test_results[1]}"
    echo "  3. Architecture Validation: ${test_results[2]}"
    echo "  4. AMD64 Compatibility:    ${test_results[3]}"
    echo "  5. Builder Functionality:  ${test_results[4]}"
    echo "  6. JSON Output:            ${test_results[5]}"
    echo ""
    log_info "Tests passed: $passed_tests/$total_tests"
    
    # Cleanup test files
    rm -f Dockerfile.buildx-test
    
    # Determine overall result
    if [[ $passed_tests -eq $total_tests ]]; then
        log_success "All integration tests PASSED!"
        log_info "Docker buildx cross-compilation system is working correctly"
        return 0
    else
        log_error "Some integration tests FAILED!"
        log_error "Docker buildx cross-compilation system needs attention"
        return 1
    fi
}

# Show system information
show_system_info() {
    log_info "System Information:"
    echo "  Architecture: $(uname -m)"
    echo "  OS: $(uname -s)"
    echo "  Docker Version: $(docker --version 2>/dev/null || echo 'Not available')"
    echo "  Buildx Version: $(docker buildx version 2>/dev/null || echo 'Not available')"
    echo ""
}

# Main execution
main() {
    log_info "Starting Docker Buildx System Integration Test..."
    echo ""
    
    show_system_info
    
    # Set up cleanup trap
    trap cleanup EXIT INT TERM
    
    # Run integration tests
    if run_integration_tests; then
        echo ""
        log_success "Docker buildx cross-compilation system is ready for use!"
        log_info "You can now build AMD64 images on ARM Mac for VPS deployment"
        exit 0
    else
        echo ""
        log_error "Docker buildx cross-compilation system has issues"
        log_error "Please check the failed tests and resolve any problems"
        exit 1
    fi
}

# Run main function
main "$@"