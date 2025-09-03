#!/bin/bash

# Simple test script to verify the authentication prompt works

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Test the authentication choice prompt
test_auth_prompt() {
    echo "Testing authentication prompt..."
    
    while true; do
        echo "Choose authentication method:"
        echo "1) Use get-admin-token.sh script (recommended)"
        echo "2) Manual OAuth process"
        echo -n "Enter choice (1 or 2): "
        read -r auth_choice
        
        case "$auth_choice" in
            "1")
                log_success "You chose option 1 - get-admin-token.sh script"
                break
                ;;
            "2")
                log_success "You chose option 2 - Manual OAuth process"
                break
                ;;
            *)
                log_error "Invalid choice. Please enter 1 or 2."
                continue
                ;;
        esac
    done
    
    echo "Test completed successfully!"
}

test_auth_prompt