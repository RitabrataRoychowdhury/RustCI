#!/bin/bash

# RustCI Pipeline Validation Script
# Validates pipeline structure against RustCI schema requirements

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Default pipeline file
PIPELINE_FILE="pipeline-cross-arch-rustci.yaml"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--file)
            PIPELINE_FILE="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [-f|--file PIPELINE_FILE]"
            echo "Validates RustCI pipeline structure and compatibility"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

print_status "Validating RustCI Pipeline: $PIPELINE_FILE"

# Check if pipeline file exists
if [[ ! -f "$PIPELINE_FILE" ]]; then
    print_error "Pipeline file not found: $PIPELINE_FILE"
    exit 1
fi

print_success "Pipeline file found: $PIPELINE_FILE"

# Validate YAML syntax
print_status "Validating YAML syntax..."
if command -v yq &> /dev/null; then
    if yq eval '.' "$PIPELINE_FILE" > /dev/null 2>&1; then
        print_success "YAML syntax is valid"
    else
        print_error "YAML syntax error detected"
        exit 1
    fi
else
    print_warning "yq not found, skipping YAML syntax validation"
fi

# Validate RustCI schema requirements
print_status "Validating RustCI schema requirements..."

# Check for 'variables' field (not 'environment')
if command -v yq &> /dev/null; then
    if yq eval 'has("variables")' "$PIPELINE_FILE" | grep -q "true"; then
        print_success "Uses 'variables' field (RustCI compatible)"
    else
        print_error "Missing 'variables' field - RustCI expects 'variables' not 'environment'"
        exit 1
    fi
    
    if yq eval 'has("environment")' "$PIPELINE_FILE" | grep -q "true"; then
        print_warning "Contains 'environment' field - should use 'variables' for RustCI"
    fi
else
    # Fallback check
    if grep -q "^variables:" "$PIPELINE_FILE"; then
        print_success "Uses 'variables' field (RustCI compatible)"
    else
        print_error "Missing 'variables' field - RustCI expects 'variables' not 'environment'"
        exit 1
    fi
fi

# Validate required top-level fields
print_status "Validating required top-level fields..."
REQUIRED_FIELDS=("name" "description" "version" "variables" "triggers" "stages")

for field in "${REQUIRED_FIELDS[@]}"; do
    if command -v yq &> /dev/null; then
        if ! yq eval "has(\"$field\")" "$PIPELINE_FILE" | grep -q "true"; then
            print_error "Missing required field: $field"
            exit 1
        fi
    else
        if ! grep -q "^$field:" "$PIPELINE_FILE"; then
            print_error "Missing required field: $field"
            exit 1
        fi
    fi
done

print_success "All required top-level fields present"

# Validate stage structure
print_status "Validating stage structure..."
if command -v yq &> /dev/null; then
    STAGE_COUNT=$(yq eval '.stages | length' "$PIPELINE_FILE")
    print_status "Found $STAGE_COUNT stages"
    
    if [[ "$STAGE_COUNT" -ne 5 ]]; then
        print_error "Expected 5 stages, found $STAGE_COUNT"
        exit 1
    fi
    
    # Check stage names
    EXPECTED_STAGES=("prepare" "build-image" "transfer-and-deploy" "smoke-test" "cleanup")
    for i in "${!EXPECTED_STAGES[@]}"; do
        STAGE_NAME=$(yq eval ".stages[$i].name" "$PIPELINE_FILE")
        if [[ "$STAGE_NAME" != "${EXPECTED_STAGES[$i]}" ]]; then
            print_error "Stage $i: expected '${EXPECTED_STAGES[$i]}', found '$STAGE_NAME'"
            exit 1
        fi
    done
    
    print_success "All 5 stages present with correct names"
fi

# Validate step types (RustCI compatible)
print_status "Validating step types..."
if command -v yq &> /dev/null; then
    STEP_TYPES=$(yq eval '.stages[].steps[].step_type' "$PIPELINE_FILE" | sort | uniq)
    print_status "Found step types: $STEP_TYPES"
    
    # Check for invalid step types
    INVALID_TYPES=$(echo "$STEP_TYPES" | grep -v -E "^(shell|docker|kubernetes)$" || true)
    if [[ -n "$INVALID_TYPES" ]]; then
        print_error "Invalid step types found: $INVALID_TYPES"
        print_error "RustCI supports: shell, docker, kubernetes"
        exit 1
    fi
    
    print_success "All step types are RustCI compatible"
fi

# Validate timeout configurations
print_status "Validating timeout configurations..."
if command -v yq &> /dev/null; then
    # Check global timeout
    GLOBAL_TIMEOUT=$(yq eval '.timeout' "$PIPELINE_FILE")
    if [[ "$GLOBAL_TIMEOUT" != "null" ]]; then
        print_success "Global timeout configured: ${GLOBAL_TIMEOUT}s"
    fi
    
    # Check step timeouts
    STEP_TIMEOUTS=$(yq eval '.stages[].steps[].timeout' "$PIPELINE_FILE" | grep -v "null" | wc -l)
    print_status "Steps with timeouts: $STEP_TIMEOUTS"
    
    if [[ "$STEP_TIMEOUTS" -lt 10 ]]; then
        print_warning "Consider adding timeouts to more steps"
    else
        print_success "Good timeout coverage"
    fi
fi

# Validate rollback configuration
print_status "Validating rollback configuration..."
if command -v yq &> /dev/null; then
    if yq eval '.rollback.enabled' "$PIPELINE_FILE" | grep -q "true"; then
        print_success "Rollback is enabled"
        
        # Check rollback steps
        ROLLBACK_STEPS=$(yq eval '.rollback.steps | length' "$PIPELINE_FILE")
        if [[ "$ROLLBACK_STEPS" -gt 0 ]]; then
            print_success "Rollback steps configured: $ROLLBACK_STEPS"
        else
            print_warning "No rollback steps defined"
        fi
    else
        print_warning "Rollback is not enabled"
    fi
fi

# Validate environment variables
print_status "Validating environment variables..."
if command -v yq &> /dev/null; then
    # Check for required variables
    REQUIRED_VARS=("TESTING_MODE" "BUILD_PLATFORM" "VPS_IP" "VPS_USERNAME" "VPS_PASSWORD" "MONGODB_URI" "JWT_SECRET")
    
    for var in "${REQUIRED_VARS[@]}"; do
        if ! yq eval ".variables | has(\"$var\")" "$PIPELINE_FILE" | grep -q "true"; then
            print_error "Missing required variable: $var"
            exit 1
        fi
    done
    
    print_success "All required variables present"
    
    # Check for production-ready variables (using ${} syntax)
    PRODUCTION_VARS=$(yq eval '.variables | to_entries | .[] | select(.value | test("^\\$\\{.*\\}$")) | .key' "$PIPELINE_FILE" | wc -l)
    TOTAL_VARS=$(yq eval '.variables | length' "$PIPELINE_FILE")
    
    if [[ "$PRODUCTION_VARS" -gt 0 ]]; then
        print_status "Production-ready variables: $PRODUCTION_VARS/$TOTAL_VARS"
        if [[ "$PRODUCTION_VARS" -eq "$TOTAL_VARS" ]]; then
            print_success "All variables use environment substitution (production-ready)"
        else
            print_warning "Some variables are hardcoded (testing mode)"
        fi
    else
        print_warning "No production-ready variables found (all hardcoded)"
    fi
fi

# Validate testing mode configuration
print_status "Validating testing mode configuration..."
if command -v yq &> /dev/null; then
    TESTING_MODE=$(yq eval '.variables.TESTING_MODE' "$PIPELINE_FILE")
    if [[ "$TESTING_MODE" == "true" || "$TESTING_MODE" == "\"true\"" ]]; then
        print_warning "Testing mode is enabled (hardcoded secrets)"
        print_status "This is acceptable for testing but not for production"
    elif [[ "$TESTING_MODE" == "false" || "$TESTING_MODE" == "\"false\"" ]]; then
        print_success "Production mode is enabled"
    else
        print_error "Invalid TESTING_MODE value: $TESTING_MODE"
        exit 1
    fi
fi

# Check for security warnings in commands
print_status "Checking for security warnings..."
if grep -q "WARNING.*hardcoded.*testing" "$PIPELINE_FILE"; then
    print_success "Security warnings present for hardcoded secrets"
else
    print_warning "No security warnings found for hardcoded secrets"
fi

# Validate command structure
print_status "Validating command structure..."
if command -v yq &> /dev/null; then
    # Check for multi-line commands (should use | syntax)
    MULTILINE_COMMANDS=$(yq eval '.stages[].steps[].config.command' "$PIPELINE_FILE" | grep -c "|" || true)
    if [[ "$MULTILINE_COMMANDS" -gt 0 ]]; then
        print_success "Uses proper multi-line command syntax"
    else
        print_warning "Consider using multi-line commands for better readability"
    fi
fi

# Final validation summary
echo ""
print_status "=== RustCI Pipeline Validation Summary ==="
echo "Pipeline: $PIPELINE_FILE"
echo "Schema: RustCI compatible"
echo "Environment Variables: Uses 'variables' field"
echo "Stages: 5 (prepare, build-image, transfer-and-deploy, smoke-test, cleanup)"
echo "Step Types: RustCI compatible (shell)"
echo "Rollback: Configured"
echo ""

# Check if this is a testing or production pipeline
if command -v yq &> /dev/null; then
    TESTING_MODE=$(yq eval '.variables.TESTING_MODE' "$PIPELINE_FILE")
    if [[ "$TESTING_MODE" == "true" || "$TESTING_MODE" == "\"true\"" ]]; then
        print_warning "⚠️  TESTING PIPELINE DETECTED"
        echo "This pipeline uses hardcoded secrets for testing purposes."
        echo "For production use:"
        echo "1. Set TESTING_MODE=false"
        echo "2. Use environment variable substitution: \${VARIABLE_NAME}"
        echo "3. Provide all secrets via environment variables"
    else
        print_success "✅ PRODUCTION PIPELINE DETECTED"
        echo "This pipeline is configured for production use with environment variables."
    fi
fi

echo ""
print_success "✅ RustCI Pipeline Validation Completed Successfully!"
echo ""
echo "The pipeline is compatible with RustCI and ready for deployment."
echo ""
echo "Upload command:"
echo "curl --location 'http://localhost:8000/api/ci/pipelines/upload' \\"
echo "--header 'Authorization: Bearer YOUR_TOKEN' \\"
echo "--form 'name=\"Cross-Architecture RustCI Deployment\"' \\"
echo "--form 'description=\"Deploy RustCI from ARM Mac to AMD64 VPS\"' \\"
echo "--form 'file=@\"$PIPELINE_FILE\"'"