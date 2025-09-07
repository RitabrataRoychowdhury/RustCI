#!/bin/bash

# Simplified Node.js Hello World Deployment Test Script
# Tests deployment of https://github.com/dockersamples/helloworld-demo-node.git

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RUSTCI_API_BASE="http://localhost:8000/api"
AUTH_TOKEN=""

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

# Function to get token
get_token() {
    echo ""
    log_info "🔐 Authentication Required"
    echo ""
    
    # Check for saved token first
    if [ -f ".rustci_token" ]; then
        log_info "Found saved token file. Use it? (y/n)"
        read -r use_saved
        if [ "$use_saved" = "y" ] || [ "$use_saved" = "Y" ]; then
            AUTH_TOKEN=$(cat .rustci_token)
            log_success "Using saved token"
            return 0
        fi
    fi
    
    echo "How would you like to get a token?"
    echo "1) Run get-admin-token.sh script"
    echo "2) Manual OAuth (I'll guide you)"
    echo "3) I already have a token"
    
    while true; do
        echo -n "Choose (1, 2, or 3): "
        read -r choice
        
        case "$choice" in
            1)
                log_info "Running get-admin-token.sh..."
                if ./scripts/get-admin-token.sh; then
                    if [ -f ".rustci_token" ]; then
                        AUTH_TOKEN=$(cat .rustci_token)
                        log_success "Token obtained!"
                        return 0
                    fi
                fi
                log_error "Script failed. Try another method."
                ;;
            2)
                echo ""
                log_info "📱 Manual OAuth Steps:"
                echo "1. Open: http://localhost:8000/api/sessions/oauth/github"
                echo "2. Complete GitHub login"
                echo "3. Copy the token from the JSON response"
                echo ""
                break
                ;;
            3)
                log_info "Great! You can paste your token next."
                break
                ;;
            *)
                log_error "Please enter 1, 2, or 3"
                continue
                ;;
        esac
        break
    done
    
    # Get token from user
    echo ""
    echo -n "Paste your JWT token: "
    read -r AUTH_TOKEN
    
    if [ -z "$AUTH_TOKEN" ]; then
        log_error "No token provided!"
        return 1
    fi
    
    # Basic validation
    if [[ ! "$AUTH_TOKEN" =~ ^eyJ ]]; then
        log_error "Token should start with 'eyJ' (JWT format)"
        return 1
    fi
    
    log_success "Token received!"
    return 0
}

# Function to test authentication
test_auth() {
    log_info "Testing authentication..."
    
    local response=$(curl -s -w "HTTP_STATUS:%{http_code}" \
        -H "Authorization: Bearer ${AUTH_TOKEN}" \
        "${RUSTCI_API_BASE}/sessions/me")
    
    local status=$(echo "$response" | grep "HTTP_STATUS:" | cut -d: -f2)
    
    if [ "$status" = "200" ]; then
        log_success "✅ Authentication successful!"
        return 0
    else
        log_error "❌ Authentication failed (Status: $status)"
        echo "Response: $(echo "$response" | sed '/HTTP_STATUS:/d')"
        return 1
    fi
}

# Main function
main() {
    echo "🚀 Node.js Hello World Deployment Test"
    echo "======================================"
    
    # Check if RustCI is running
    if ! curl -s "${RUSTCI_API_BASE}/healthchecker" >/dev/null 2>&1; then
        log_error "RustCI is not running at http://localhost:8000"
        log_info "Start it with: cargo run --bin RustAutoDevOps"
        exit 1
    fi
    
    log_success "RustCI is running"
    
    # Get and test authentication
    if ! get_token; then
        exit 1
    fi
    
    if ! test_auth; then
        log_error "Authentication failed. Please check your token."
        exit 1
    fi
    
    log_success "🎉 Authentication successful! Ready for deployment."
    log_info "You can now run the full deployment script:"
    log_info "./scripts/test-nodejs-deployment.sh -t \"$AUTH_TOKEN\""
}

# Run main function
main "$@"