#!/bin/bash

# Cross-Platform Setup Test Script
# This script tests the Mac-to-Windows CI/CD setup

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WINDOWS_HOST="${WINDOWS_HOST:-}"
WINDOWS_USER="${WINDOWS_USER:-}"
DEPLOYMENT_PORT="3000"
SSH_TIMEOUT="10"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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

# Check if required variables are set
check_environment() {
    log_info "Checking environment variables..."
    
    if [[ -z "$WINDOWS_HOST" ]]; then
        log_error "WINDOWS_HOST environment variable is not set"
        echo "Please set it with: export WINDOWS_HOST=\"your-windows-ip\""
        exit 1
    fi
    
    if [[ -z "$WINDOWS_USER" ]]; then
        log_error "WINDOWS_USER environment variable is not set"
        echo "Please set it with: export WINDOWS_USER=\"your-windows-username\""
        exit 1
    fi
    
    log_success "Environment variables are set"
    echo "  WINDOWS_HOST: $WINDOWS_HOST"
    echo "  WINDOWS_USER: $WINDOWS_USER"
}

# Test network connectivity
test_network() {
    log_info "Testing network connectivity..."
    
    # Test ping
    log_info "Testing ping to $WINDOWS_HOST..."
    if ping -c 3 -W 3000 "$WINDOWS_HOST" &> /dev/null; then
        log_success "Ping test passed"
    else
        log_error "Ping test failed - cannot reach $WINDOWS_HOST"
        return 1
    fi
    
    # Test SSH port
    log_info "Testing SSH port (22) connectivity..."
    if timeout "$SSH_TIMEOUT" bash -c "</dev/tcp/$WINDOWS_HOST/22" &> /dev/null; then
        log_success "SSH port is reachable"
    else
        log_error "SSH port (22) is not reachable"
        return 1
    fi
    
    # Test deployment server port
    log_info "Testing deployment server port ($DEPLOYMENT_PORT)..."
    if timeout "$SSH_TIMEOUT" bash -c "</dev/tcp/$WINDOWS_HOST/$DEPLOYMENT_PORT" &> /dev/null; then
        log_success "Deployment server port is reachable"
    else
        log_warning "Deployment server port ($DEPLOYMENT_PORT) is not reachable"
        log_info "This is expected if the Windows deployment server is not running yet"
    fi
}

# Test SSH authentication
test_ssh() {
    log_info "Testing SSH authentication..."
    
    if ssh -o ConnectTimeout="$SSH_TIMEOUT" -o BatchMode=yes "$WINDOWS_USER@$WINDOWS_HOST" "echo 'SSH authentication successful'" 2>/dev/null; then
        log_success "SSH key authentication is working"
    else
        log_warning "SSH key authentication failed, trying with password prompt..."
        
        if ssh -o ConnectTimeout="$SSH_TIMEOUT" "$WINDOWS_USER@$WINDOWS_HOST" "echo 'SSH authentication successful'" 2>/dev/null; then
            log_success "SSH password authentication is working"
        else
            log_error "SSH authentication failed"
            log_info "Please ensure:"
            log_info "1. OpenSSH Server is running on Windows"
            log_info "2. SSH keys are set up or password authentication is enabled"
            log_info "3. Windows firewall allows SSH connections"
            return 1
        fi
    fi
}

# Test Windows deployment server
test_deployment_server() {
    log_info "Testing Windows deployment server..."
    
    # Test health endpoint
    local health_url="http://$WINDOWS_HOST:$DEPLOYMENT_PORT/health"
    log_info "Testing health endpoint: $health_url"
    
    if curl -s -f --connect-timeout 10 "$health_url" &> /dev/null; then
        log_success "Deployment server is responding"
        
        # Get and display health response
        local health_response=$(curl -s "$health_url" 2>/dev/null || echo "")
        if [[ -n "$health_response" ]]; then
            echo "Health response: $health_response"
        fi
        
        # Test status endpoint
        local status_url="http://$WINDOWS_HOST:$DEPLOYMENT_PORT/status"
        log_info "Testing status endpoint: $status_url"
        
        if curl -s -f --connect-timeout 10 "$status_url" &> /dev/null; then
            log_success "Status endpoint is working"
        else
            log_warning "Status endpoint is not responding"
        fi
        
    else
        log_error "Deployment server is not responding"
        log_info "Please ensure the Windows deployment server is running:"
        log_info "  windows-deploy-server.bat start"
        return 1
    fi
}

# Test Docker on Mac
test_docker() {
    log_info "Testing Docker on Mac..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        return 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        return 1
    fi
    
    log_success "Docker is available and running"
    
    # Test Docker-in-Docker capability
    log_info "Testing Docker-in-Docker capability..."
    if docker run --rm --privileged docker:dind docker --version &> /dev/null; then
        log_success "Docker-in-Docker is working"
    else
        log_warning "Docker-in-Docker test failed"
        log_info "This might work anyway when the full runner is started"
    fi
}

# Test file transfer
test_file_transfer() {
    log_info "Testing file transfer to Windows..."
    
    # Create a test file
    local test_file="/tmp/rustci-test-$(date +%s).txt"
    echo "Test file created on Mac at $(date)" > "$test_file"
    
    # Try to copy file to Windows
    if scp -o ConnectTimeout="$SSH_TIMEOUT" -o StrictHostKeyChecking=no "$test_file" "$WINDOWS_USER@$WINDOWS_HOST:C:/temp/" &> /dev/null; then
        log_success "File transfer to Windows successful"
        
        # Verify file exists on Windows
        if ssh -o ConnectTimeout="$SSH_TIMEOUT" "$WINDOWS_USER@$WINDOWS_HOST" "dir C:\\temp\\$(basename "$test_file")" &> /dev/null; then
            log_success "File verified on Windows"
            
            # Clean up
            ssh -o ConnectTimeout="$SSH_TIMEOUT" "$WINDOWS_USER@$WINDOWS_HOST" "del C:\\temp\\$(basename "$test_file")" &> /dev/null || true
        else
            log_warning "File transfer succeeded but verification failed"
        fi
    else
        log_error "File transfer to Windows failed"
        return 1
    fi
    
    # Clean up local test file
    rm -f "$test_file"
}

# Test RustCI server
test_rustci_server() {
    log_info "Testing RustCI server..."
    
    local rustci_server="${RUSTCI_SERVER:-http://localhost:8080}"
    local health_url="$rustci_server/health"
    
    if curl -s -f --connect-timeout 10 "$health_url" &> /dev/null; then
        log_success "RustCI server is responding"
    else
        log_warning "RustCI server is not responding at $rustci_server"
        log_info "This is expected if RustCI server is not running"
        log_info "Start it with: cargo run --bin rustci-server"
    fi
}

# Run comprehensive test
run_comprehensive_test() {
    log_info "Running comprehensive cross-platform setup test..."
    echo "=================================================="
    
    local tests_passed=0
    local tests_failed=0
    
    # Run all tests
    local test_functions=(
        "check_environment"
        "test_network"
        "test_ssh"
        "test_docker"
        "test_file_transfer"
        "test_deployment_server"
        "test_rustci_server"
    )
    
    for test_func in "${test_functions[@]}"; do
        echo ""
        if $test_func; then
            ((tests_passed++))
        else
            ((tests_failed++))
        fi
    done
    
    echo ""
    echo "=================================================="
    log_info "Test Summary:"
    echo "  Tests Passed: $tests_passed"
    echo "  Tests Failed: $tests_failed"
    echo "  Total Tests:  $((tests_passed + tests_failed))"
    
    if [[ $tests_failed -eq 0 ]]; then
        log_success "All tests passed! Your cross-platform setup is ready."
        echo ""
        log_info "Next steps:"
        echo "1. Start the Mac DinD runner: ./scripts/runners/mac-dind-runner.sh start"
        echo "2. Run a test pipeline: rustci run mac-to-windows-pipeline.yaml"
    else
        log_warning "Some tests failed. Please address the issues above."
        echo ""
        log_info "Common solutions:"
        echo "1. Ensure Windows deployment server is running"
        echo "2. Check SSH configuration and authentication"
        echo "3. Verify network connectivity and firewall settings"
        echo "4. Start RustCI server if needed"
    fi
    
    return $tests_failed
}

# Show help
show_help() {
    echo "Cross-Platform Setup Test Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "COMMANDS:"
    echo "  test        - Run comprehensive test (default)"
    echo "  network     - Test network connectivity only"
    echo "  ssh         - Test SSH authentication only"
    echo "  docker      - Test Docker functionality only"
    echo "  transfer    - Test file transfer only"
    echo "  server      - Test deployment server only"
    echo "  rustci      - Test RustCI server only"
    echo "  help        - Show this help"
    echo ""
    echo "ENVIRONMENT VARIABLES:"
    echo "  WINDOWS_HOST - IP address of Windows machine (required)"
    echo "  WINDOWS_USER - Username for Windows machine (required)"
    echo "  RUSTCI_SERVER - RustCI server URL (optional, default: http://localhost:8080)"
    echo ""
    echo "EXAMPLES:"
    echo "  export WINDOWS_HOST=\"192.168.1.100\""
    echo "  export WINDOWS_USER=\"myuser\""
    echo "  $0 test"
    echo ""
}

# Main execution
case "${1:-test}" in
    "test")
        run_comprehensive_test
        ;;
    "network")
        check_environment && test_network
        ;;
    "ssh")
        check_environment && test_ssh
        ;;
    "docker")
        test_docker
        ;;
    "transfer")
        check_environment && test_file_transfer
        ;;
    "server")
        check_environment && test_deployment_server
        ;;
    "rustci")
        test_rustci_server
        ;;
    "help"|"-h"|"--help")
        show_help
        ;;
    *)
        log_error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac