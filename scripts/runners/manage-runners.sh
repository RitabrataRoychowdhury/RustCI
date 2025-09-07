#!/bin/bash

# RustCI Runner Management Script
# Manages different types of runners (Docker, Native, Fake EC2)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Check Docker runner
check_docker_runner() {
    log_info "Checking Docker runner..."
    
    if ! command -v docker &> /dev/null; then
        echo "  ❌ Docker not installed"
        return 1
    fi
    
    if ! docker info &> /dev/null; then
        echo "  ❌ Docker not running"
        return 1
    fi
    
    echo "  ✅ Docker available"
    echo "  📊 Containers: $(docker ps -q | wc -l) running"
    echo "  📊 Images: $(docker images -q | wc -l) available"
    return 0
}

# Check DinD runner
check_dind_runner() {
    log_info "Checking Docker-in-Docker runner..."
    
    if docker ps | grep -q "rustci-dind"; then
        echo "  ✅ DinD container running"
        local containers=$(docker exec rustci-dind docker ps -q | wc -l)
        echo "  📊 Containers inside DinD: $containers"
        return 0
    else
        echo "  ❌ DinD container not running"
        echo "  💡 Run: ./setup-dind-environment.sh"
        return 1
    fi
}

# Check fake EC2 runners
check_fake_ec2_runners() {
    log_info "Checking fake EC2 runners..."
    
    local count=0
    for i in {1..3}; do
        if docker ps | grep -q "rustci-ec2-$i"; then
            count=$((count + 1))
        fi
    done
    
    if [ $count -gt 0 ]; then
        echo "  ✅ $count fake EC2 instances running"
        for i in {1..3}; do
            if docker ps | grep -q "rustci-ec2-$i"; then
                echo "    - rustci-ec2-$i (SSH: localhost:$((2200 + i)))"
            fi
        done
        return 0
    else
        echo "  ❌ No fake EC2 instances running"
        echo "  💡 Run: ./setup-fake-ec2.sh"
        return 1
    fi
}

# Check native runner
check_native_runner() {
    log_info "Checking native runner..."
    
    local score=0
    local total=0
    
    # Check shell
    if command -v bash &> /dev/null; then
        echo "  ✅ Bash available"
        score=$((score + 1))
    else
        echo "  ❌ Bash not available"
    fi
    total=$((total + 1))
    
    # Check git
    if command -v git &> /dev/null; then
        echo "  ✅ Git available"
        score=$((score + 1))
    else
        echo "  ❌ Git not available"
    fi
    total=$((total + 1))
    
    # Check curl
    if command -v curl &> /dev/null; then
        echo "  ✅ Curl available"
        score=$((score + 1))
    else
        echo "  ❌ Curl not available"
    fi
    total=$((total + 1))
    
    # Check node (for Node.js deployments)
    if command -v node &> /dev/null; then
        echo "  ✅ Node.js available ($(node --version))"
        score=$((score + 1))
    else
        echo "  ⚠️  Node.js not available"
    fi
    total=$((total + 1))
    
    echo "  📊 Native runner score: $score/$total"
    
    if [ $score -ge 3 ]; then
        return 0
    else
        return 1
    fi
}

# Setup all runners
setup_all_runners() {
    log_info "Setting up all available runners..."
    
    # Setup DinD
    if [ -f "./setup-dind-environment.sh" ]; then
        log_info "Setting up DinD environment..."
        ./setup-dind-environment.sh
    fi
    
    # Setup fake EC2
    if [ -f "./setup-fake-ec2.sh" ]; then
        log_info "Setting up fake EC2 environment..."
        ./setup-fake-ec2.sh
    fi
    
    log_success "Runner setup completed"
}

# Show runner status
show_status() {
    echo "🏃 RustCI Runner Status"
    echo "======================"
    echo ""
    
    local available_runners=0
    
    if check_docker_runner; then
        available_runners=$((available_runners + 1))
    fi
    echo ""
    
    if check_dind_runner; then
        available_runners=$((available_runners + 1))
    fi
    echo ""
    
    if check_fake_ec2_runners; then
        available_runners=$((available_runners + 1))
    fi
    echo ""
    
    if check_native_runner; then
        available_runners=$((available_runners + 1))
    fi
    echo ""
    
    echo "📊 Summary: $available_runners runner types available"
    
    if [ $available_runners -eq 0 ]; then
        echo ""
        log_error "No runners available! RustCI won't be able to execute pipelines."
        echo "💡 Run: $0 setup"
    else
        echo ""
        log_success "RustCI has $available_runners runner types available for pipeline execution"
    fi
}

# Test runner connectivity
test_runners() {
    log_info "Testing runner connectivity..."
    
    # Test Docker
    if check_docker_runner &>/dev/null; then
        log_info "Testing Docker runner..."
        if docker run --rm hello-world &>/dev/null; then
            echo "  ✅ Docker runner test passed"
        else
            echo "  ❌ Docker runner test failed"
        fi
    fi
    
    # Test DinD
    if check_dind_runner &>/dev/null; then
        log_info "Testing DinD runner..."
        if docker exec rustci-dind docker run --rm hello-world &>/dev/null; then
            echo "  ✅ DinD runner test passed"
        else
            echo "  ❌ DinD runner test failed"
        fi
    fi
    
    # Test fake EC2
    if check_fake_ec2_runners &>/dev/null; then
        log_info "Testing fake EC2 runners..."
        if docker exec rustci-ec2-1 echo "test" &>/dev/null; then
            echo "  ✅ Fake EC2 runner test passed"
        else
            echo "  ❌ Fake EC2 runner test failed"
        fi
    fi
    
    # Test native
    if check_native_runner &>/dev/null; then
        log_info "Testing native runner..."
        if echo "test" | bash -c "cat" &>/dev/null; then
            echo "  ✅ Native runner test passed"
        else
            echo "  ❌ Native runner test failed"
        fi
    fi
}

# Show usage
show_usage() {
    cat << EOF
RustCI Runner Management Script

Usage: $0 [COMMAND]

COMMANDS:
    status      Show status of all runners (default)
    setup       Setup all available runners
    test        Test runner connectivity
    help        Show this help message

RUNNER TYPES:
    Docker      - Local Docker daemon
    DinD        - Docker-in-Docker for isolation
    Fake EC2    - Simulated EC2 instances
    Native      - Direct shell execution

EXAMPLES:
    $0              # Show runner status
    $0 setup        # Setup all runners
    $0 test         # Test all runners

EOF
}

# Main execution
case "${1:-status}" in
    "status"|"")
        show_status
        ;;
    "setup")
        setup_all_runners
        ;;
    "test")
        test_runners
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