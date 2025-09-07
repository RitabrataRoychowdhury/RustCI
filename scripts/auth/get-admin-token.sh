#!/bin/bash

# RustCI Admin Token Helper Script
# This script helps you get an admin token and test the API

set -e

# Configuration
RUSTCI_URL="http://localhost:8000"
TOKEN_FILE=".rustci_token"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 RustCI Admin Token Helper${NC}"
echo "=================================="

# Function to check if RustCI is running
check_rustci_running() {
    echo -e "${YELLOW}📡 Checking if RustCI is running...${NC}"
    if curl -s "$RUSTCI_URL/healthchecker" > /dev/null; then
        echo -e "${GREEN}✅ RustCI is running at $RUSTCI_URL${NC}"
        return 0
    else
        echo -e "${RED}❌ RustCI is not running at $RUSTCI_URL${NC}"
        echo "Please start RustCI first:"
        echo "  cargo run --bin RustAutoDevOps"
        echo "  or"
        echo "  docker-compose up -d"
        return 1
    fi
}

# Function to get OAuth URL
get_oauth_url() {
    echo -e "${YELLOW}🔗 Getting GitHub OAuth URL...${NC}"
    
    # Get the redirect URL from the OAuth endpoint
    OAUTH_REDIRECT=$(curl -s -I "$RUSTCI_URL/api/sessions/oauth/github" | grep -i location | cut -d' ' -f2 | tr -d '\r')
    
    if [ -n "$OAUTH_REDIRECT" ]; then
        echo -e "${GREEN}✅ OAuth URL obtained${NC}"
        echo -e "${BLUE}Please visit this URL to authenticate:${NC}"
        echo "$OAUTH_REDIRECT"
        echo ""
        echo -e "${YELLOW}After completing OAuth, you'll be redirected to a page with your JWT token.${NC}"
        echo -e "${YELLOW}Copy the token from the response and paste it when prompted.${NC}"
        return 0
    else
        echo -e "${RED}❌ Failed to get OAuth URL${NC}"
        return 1
    fi
}

# Function to save token
save_token() {
    local token="$1"
    echo "$token" > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
    echo -e "${GREEN}✅ Token saved to $TOKEN_FILE${NC}"
}

# Function to load token
load_token() {
    if [ -f "$TOKEN_FILE" ]; then
        cat "$TOKEN_FILE"
        return 0
    else
        return 1
    fi
}

# Function to validate token
validate_token() {
    local token="$1"
    echo -e "${YELLOW}🔍 Validating token...${NC}"
    
    response=$(curl -s -w "%{http_code}" -o /tmp/token_validation \
        -H "Authorization: Bearer $token" \
        "$RUSTCI_URL/api/sessions/me")
    
    if [ "$response" = "200" ]; then
        echo -e "${GREEN}✅ Token is valid${NC}"
        echo -e "${BLUE}User info:${NC}"
        cat /tmp/token_validation | jq . 2>/dev/null || cat /tmp/token_validation
        rm -f /tmp/token_validation
        return 0
    else
        echo -e "${RED}❌ Token is invalid or expired (HTTP $response)${NC}"
        rm -f /tmp/token_validation
        return 1
    fi
}

# Function to test API endpoints
test_api() {
    local token="$1"
    echo -e "${YELLOW}🧪 Testing API endpoints...${NC}"
    
    # Test health check (public)
    echo -e "${BLUE}Testing health check...${NC}"
    curl -s "$RUSTCI_URL/healthchecker" | jq . 2>/dev/null || echo "Health check OK"
    
    # Test pipeline list
    echo -e "${BLUE}Testing pipeline list...${NC}"
    response=$(curl -s -w "%{http_code}" -o /tmp/pipeline_list \
        -H "Authorization: Bearer $token" \
        "$RUSTCI_URL/api/ci/pipelines")
    
    if [ "$response" = "200" ]; then
        echo -e "${GREEN}✅ Pipeline list endpoint working${NC}"
        cat /tmp/pipeline_list | jq . 2>/dev/null || cat /tmp/pipeline_list
    else
        echo -e "${RED}❌ Pipeline list failed (HTTP $response)${NC}"
        cat /tmp/pipeline_list
    fi
    rm -f /tmp/pipeline_list
    
    # Test executions list
    echo -e "${BLUE}Testing executions list...${NC}"
    response=$(curl -s -w "%{http_code}" -o /tmp/executions_list \
        -H "Authorization: Bearer $token" \
        "$RUSTCI_URL/api/ci/executions")
    
    if [ "$response" = "200" ]; then
        echo -e "${GREEN}✅ Executions list endpoint working${NC}"
        cat /tmp/executions_list | jq . 2>/dev/null || cat /tmp/executions_list
    else
        echo -e "${RED}❌ Executions list failed (HTTP $response)${NC}"
        cat /tmp/executions_list
    fi
    rm -f /tmp/executions_list
}

# Function to upload example pipeline
upload_example_pipeline() {
    local token="$1"
    echo -e "${YELLOW}📤 Uploading example pipeline...${NC}"
    
    # Check if example pipeline exists
    if [ ! -f "docs/pipeline-examples/docker-deployment-pipeline.yaml" ]; then
        echo -e "${RED}❌ Example pipeline not found${NC}"
        echo "Please ensure you're running this script from the RustCI root directory"
        return 1
    fi
    
    response=$(curl -s -w "%{http_code}" -o /tmp/upload_response \
        -H "Authorization: Bearer $token" \
        -F "file=@docs/pipeline-examples/docker-deployment-pipeline.yaml" \
        -F "name=docker-test-example" \
        -F "description=Docker deployment example pipeline" \
        "$RUSTCI_URL/api/ci/pipelines/upload")
    
    if [ "$response" = "200" ] || [ "$response" = "201" ]; then
        echo -e "${GREEN}✅ Pipeline uploaded successfully${NC}"
        cat /tmp/upload_response | jq . 2>/dev/null || cat /tmp/upload_response
        
        # Extract pipeline ID for triggering
        PIPELINE_ID=$(cat /tmp/upload_response | jq -r '.pipeline_id' 2>/dev/null || echo "")
        if [ -n "$PIPELINE_ID" ] && [ "$PIPELINE_ID" != "null" ]; then
            echo -e "${BLUE}Pipeline ID: $PIPELINE_ID${NC}"
            echo "You can trigger this pipeline with:"
            echo "curl -X POST $RUSTCI_URL/api/ci/pipelines/$PIPELINE_ID/trigger \\"
            echo "  -H \"Authorization: Bearer $token\" \\"
            echo "  -H \"Content-Type: application/json\" \\"
            echo "  -d '{\"branch\": \"main\"}'"
        fi
    else
        echo -e "${RED}❌ Pipeline upload failed (HTTP $response)${NC}"
        cat /tmp/upload_response
    fi
    rm -f /tmp/upload_response
}

# Main script logic
main() {
    # Check if RustCI is running
    if ! check_rustci_running; then
        exit 1
    fi
    
    # Check if we already have a valid token
    if token=$(load_token) && validate_token "$token"; then
        echo -e "${GREEN}✅ Using existing valid token${NC}"
    else
        echo -e "${YELLOW}🔑 Need to get new token${NC}"
        
        # Ask user which method they prefer
        echo ""
        echo "Choose token generation method:"
        echo "1) Generate token using built-in generator (recommended)"
        echo "2) Use GitHub OAuth"
        echo -n "Enter choice (1 or 2): "
        read -r choice
        
        case "$choice" in
            "1")
                echo -e "${YELLOW}🔧 Generating token using built-in generator...${NC}"
                token_output=$(cargo run --bin generate_token 2>/dev/null)
                if [ $? -eq 0 ]; then
                    # Extract token from output (it's on the second line)
                    token=$(echo "$token_output" | sed -n '2p')
                    if [ -n "$token" ]; then
                        echo -e "${GREEN}✅ Token generated successfully${NC}"
                        if validate_token "$token"; then
                            save_token "$token"
                        else
                            echo -e "${RED}❌ Generated token is invalid${NC}"
                            exit 1
                        fi
                    else
                        echo -e "${RED}❌ Failed to extract token from generator output${NC}"
                        exit 1
                    fi
                else
                    echo -e "${RED}❌ Failed to generate token${NC}"
                    exit 1
                fi
                ;;
            "2")
                # Get OAuth URL
                if ! get_oauth_url; then
                    exit 1
                fi
                
                # Prompt for token
                echo ""
                echo -e "${YELLOW}Please paste your JWT token here:${NC}"
                read -r token
                
                if [ -z "$token" ]; then
                    echo -e "${RED}❌ No token provided${NC}"
                    exit 1
                fi
                
                # Validate the token
                if validate_token "$token"; then
                    save_token "$token"
                else
                    echo -e "${RED}❌ Invalid token provided${NC}"
                    exit 1
                fi
                ;;
            *)
                echo -e "${RED}❌ Invalid choice${NC}"
                exit 1
                ;;
        esac
    fi
    
    # Test API endpoints
    test_api "$token"
    
    # Ask if user wants to upload example pipeline
    echo ""
    echo -e "${YELLOW}Would you like to upload an example pipeline? (y/n)${NC}"
    read -r upload_choice
    
    if [ "$upload_choice" = "y" ] || [ "$upload_choice" = "Y" ]; then
        upload_example_pipeline "$token"
    fi
    
    echo ""
    echo -e "${GREEN}🎉 Setup complete!${NC}"
    echo -e "${BLUE}Your token is saved in: $TOKEN_FILE${NC}"
    echo -e "${BLUE}You can now use the RustCI API with:${NC}"
    echo "export JWT_TOKEN=\"$token\""
    echo ""
    echo -e "${BLUE}Example commands:${NC}"
    echo "# List pipelines"
    echo "curl -H \"Authorization: Bearer \$JWT_TOKEN\" $RUSTCI_URL/api/ci/pipelines"
    echo ""
    echo "# Upload pipeline"
    echo "curl -X POST -H \"Authorization: Bearer \$JWT_TOKEN\" \\"
    echo "  -F \"file=@your-pipeline.yaml\" \\"
    echo "  $RUSTCI_URL/api/ci/pipelines/upload"
}

# Run main function
main "$@"