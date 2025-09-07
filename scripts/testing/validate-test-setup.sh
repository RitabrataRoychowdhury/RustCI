#!/bin/bash
# Validation script to check if the automated pipeline test setup is correct
# This script validates prerequisites without running the full pipeline test

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 RustCI Test Setup Validation${NC}"
echo "=================================="
echo ""

# Function to check tool availability
check_tool() {
    local tool=$1
    local required=$2
    
    if command -v "$tool" >/dev/null 2>&1; then
        local version=$(${tool} --version 2>/dev/null | head -1 || echo "unknown")
        echo -e "${GREEN}✅ $tool: Available ($version)${NC}"
        return 0
    else
        if [[ "$required" == "true" ]]; then
            echo -e "${RED}❌ $tool: Missing (required)${NC}"
            return 1
        else
            echo -e "${YELLOW}⚠️  $tool: Missing (optional)${NC}"
            return 0
        fi
    fi
}

# Function to check file existence
check_file() {
    local file=$1
    local description=$2
    
    if [[ -f "$file" ]]; then
        local size=$(wc -c < "$file" 2>/dev/null || echo "0")
        echo -e "${GREEN}✅ $description: Found ($size bytes)${NC}"
        return 0
    else
        echo -e "${RED}❌ $description: Missing${NC}"
        return 1
    fi
}

# Check required tools
echo -e "${BLUE}Checking Required Tools:${NC}"
missing_required=0

check_tool "curl" "true" || missing_required=$((missing_required + 1))
check_tool "cargo" "true" || missing_required=$((missing_required + 1))
check_tool "docker" "true" || missing_required=$((missing_required + 1))
check_tool "git" "true" || missing_required=$((missing_required + 1))

echo ""

# Check optional tools
echo -e "${BLUE}Checking Optional Tools:${NC}"
check_tool "jq" "false"
check_tool "sshpass" "false"
check_tool "python3" "false"

echo ""

# Check required files
echo -e "${BLUE}Checking Required Files:${NC}"
missing_files=0

check_file "pipeline.yaml" "Pipeline configuration" || missing_files=$((missing_files + 1))
check_file ".env" "Environment configuration" || missing_files=$((missing_files + 1))
check_file "scripts/automated-pipeline-test.sh" "Automated test script" || missing_files=$((missing_files + 1))
check_file "src/bin/generate_token.rs" "Token generator binary" || missing_files=$((missing_files + 1))

echo ""

# Check Rust project structure
echo -e "${BLUE}Checking Rust Project:${NC}"
if [[ -f "Cargo.toml" ]]; then
    echo -e "${GREEN}✅ Cargo.toml: Found${NC}"
    
    # Check if we can build the project
    echo -e "${BLUE}Testing Cargo build...${NC}"
    if cargo check --quiet 2>/dev/null; then
        echo -e "${GREEN}✅ Cargo check: Passed${NC}"
    else
        echo -e "${YELLOW}⚠️  Cargo check: Failed (may need dependencies)${NC}"
    fi
else
    echo -e "${RED}❌ Cargo.toml: Missing${NC}"
    missing_files=$((missing_files + 1))
fi

echo ""

# Test JWT token generation
echo -e "${BLUE}Testing JWT Token Generation:${NC}"
if cargo run --bin generate_token --quiet 2>/dev/null | grep -q "eyJ"; then
    echo -e "${GREEN}✅ Token generation: Working${NC}"
else
    echo -e "${YELLOW}⚠️  Token generation: May need build or dependencies${NC}"
fi

echo ""

# Check Docker daemon
echo -e "${BLUE}Checking Docker:${NC}"
if docker info >/dev/null 2>&1; then
    echo -e "${GREEN}✅ Docker daemon: Running${NC}"
else
    echo -e "${YELLOW}⚠️  Docker daemon: Not running or not accessible${NC}"
fi

echo ""

# Check pipeline.yaml content
echo -e "${BLUE}Analyzing Pipeline Configuration:${NC}"
if [[ -f "pipeline.yaml" ]]; then
    # Check for hardcoded paths
    if grep -q "/tmp/rustci" pipeline.yaml; then
        echo -e "${YELLOW}⚠️  Pipeline uses hardcoded paths (/tmp/rustci)${NC}"
    else
        echo -e "${GREEN}✅ Pipeline paths: No hardcoded paths detected${NC}"
    fi
    
    # Check for required steps
    if grep -q "git clone" pipeline.yaml; then
        echo -e "${GREEN}✅ Git clone step: Found${NC}"
    else
        echo -e "${YELLOW}⚠️  Git clone step: Not found${NC}"
    fi
    
    if grep -q "docker build" pipeline.yaml; then
        echo -e "${GREEN}✅ Docker build step: Found${NC}"
    else
        echo -e "${YELLOW}⚠️  Docker build step: Not found${NC}"
    fi
    
    if grep -q "sshpass" pipeline.yaml; then
        echo -e "${GREEN}✅ SSH deployment step: Found${NC}"
    else
        echo -e "${YELLOW}⚠️  SSH deployment step: Not found${NC}"
    fi
fi

echo ""

# Summary
echo -e "${BLUE}Validation Summary:${NC}"
echo "=================="

if [[ $missing_required -eq 0 && $missing_files -eq 0 ]]; then
    echo -e "${GREEN}🎉 All required components are available!${NC}"
    echo ""
    echo -e "${GREEN}You can now run the automated pipeline test:${NC}"
    echo -e "${BLUE}  ./scripts/run-pipeline-test.sh${NC}"
    echo ""
    exit 0
else
    echo -e "${RED}❌ Missing required components:${NC}"
    if [[ $missing_required -gt 0 ]]; then
        echo -e "  - $missing_required required tools"
    fi
    if [[ $missing_files -gt 0 ]]; then
        echo -e "  - $missing_files required files"
    fi
    echo ""
    echo -e "${YELLOW}Please install missing tools and ensure all files are present.${NC}"
    echo ""
    exit 1
fi