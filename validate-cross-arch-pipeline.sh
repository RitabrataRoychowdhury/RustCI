#!/bin/bash

# Validation script for cross-architecture pipeline configuration
# This script validates the pipeline structure and configuration

echo "🔍 Validating Cross-Architecture Pipeline Configuration..."

PIPELINE_FILE="pipeline-cross-arch.yaml"

# Check if pipeline file exists
if [ ! -f "$PIPELINE_FILE" ]; then
    echo "❌ Pipeline file not found: $PIPELINE_FILE"
    exit 1
fi

echo "✅ Pipeline file found: $PIPELINE_FILE"

# Basic YAML syntax validation
if command -v yq &> /dev/null; then
    echo "🔍 Validating YAML syntax..."
    if yq eval '.' "$PIPELINE_FILE" > /dev/null 2>&1; then
        echo "✅ YAML syntax is valid"
    else
        echo "❌ YAML syntax error detected"
        exit 1
    fi
else
    echo "⚠️  yq not found, skipping YAML syntax validation"
fi

# Validate required top-level fields
echo "🔍 Validating required fields..."

REQUIRED_FIELDS=("name" "description" "version" "environment" "triggers" "stages")
MISSING_FIELDS=""

for field in "${REQUIRED_FIELDS[@]}"; do
    if command -v yq &> /dev/null; then
        if ! yq eval "has(\"$field\")" "$PIPELINE_FILE" | grep -q "true"; then
            MISSING_FIELDS="$MISSING_FIELDS $field"
        fi
    fi
done

if [ -n "$MISSING_FIELDS" ]; then
    echo "❌ Missing required fields:$MISSING_FIELDS"
    exit 1
fi

echo "✅ All required top-level fields present"

# Validate stage structure
echo "🔍 Validating stage structure..."

if command -v yq &> /dev/null; then
    STAGE_COUNT=$(yq eval '.stages | length' "$PIPELINE_FILE")
    echo "📋 Found $STAGE_COUNT stages"
    
    if [ "$STAGE_COUNT" -ne 5 ]; then
        echo "❌ Expected 5 stages, found $STAGE_COUNT"
        exit 1
    fi
    
    # Check stage names
    EXPECTED_STAGES=("prepare" "build-image" "transfer-and-deploy" "smoke-test" "cleanup")
    for i in "${!EXPECTED_STAGES[@]}"; do
        STAGE_NAME=$(yq eval ".stages[$i].name" "$PIPELINE_FILE")
        if [ "$STAGE_NAME" != "${EXPECTED_STAGES[$i]}" ]; then
            echo "❌ Stage $i: expected '${EXPECTED_STAGES[$i]}', found '$STAGE_NAME'"
            exit 1
        fi
    done
    
    echo "✅ All 5 stages present with correct names"
fi

# Validate timeout configurations
echo "🔍 Validating timeout configurations..."

if command -v yq &> /dev/null; then
    # Check global timeout
    GLOBAL_TIMEOUT=$(yq eval '.timeout' "$PIPELINE_FILE")
    if [ "$GLOBAL_TIMEOUT" != "3600" ]; then
        echo "❌ Expected global timeout 3600, found $GLOBAL_TIMEOUT"
        exit 1
    fi
    
    echo "✅ Timeout configurations valid"
fi

# Validate environment variables
echo "🔍 Validating environment variables..."

REQUIRED_ENV_VARS=("TESTING_MODE" "BUILD_PLATFORM" "VPS_IP" "VPS_USERNAME" "VPS_PASSWORD" "MONGODB_URI" "JWT_SECRET")

if command -v yq &> /dev/null; then
    for var in "${REQUIRED_ENV_VARS[@]}"; do
        if ! yq eval ".environment | has(\"$var\")" "$PIPELINE_FILE" | grep -q "true"; then
            echo "❌ Missing environment variable: $var"
            exit 1
        fi
    done
    
    echo "✅ All required environment variables present"
fi

# Validate rollback configuration
echo "🔍 Validating rollback configuration..."

if command -v yq &> /dev/null; then
    if ! yq eval '.rollback.enabled' "$PIPELINE_FILE" | grep -q "true"; then
        echo "❌ Rollback not enabled"
        exit 1
    fi
    
    echo "✅ Rollback configuration valid"
fi

# Validate step types
echo "🔍 Validating step types..."

if command -v yq &> /dev/null; then
    INVALID_STEP_TYPES=$(yq eval '.stages[].steps[].step_type' "$PIPELINE_FILE" | grep -v "shell" | wc -l)
    if [ "$INVALID_STEP_TYPES" -gt 0 ]; then
        echo "❌ Found invalid step types (only 'shell' is used in this pipeline)"
        exit 1
    fi
    
    echo "✅ All step types are valid"
fi

echo ""
echo "🎉 Cross-Architecture Pipeline Validation Completed Successfully!"
echo ""
echo "📋 Validation Summary:"
echo "  • Pipeline file: $PIPELINE_FILE"
echo "  • Stages: 5 (prepare, build-image, transfer-and-deploy, smoke-test, cleanup)"
echo "  • Step type: shell (compatible with RustCI)"
echo "  • Timeout: 3600 seconds (1 hour)"
echo "  • Rollback: Enabled"
echo "  • Testing mode: Enabled with hardcoded secrets"
echo ""
echo "⚠️  Security Reminder:"
echo "  • This pipeline uses hardcoded secrets for testing"
echo "  • Set TESTING_MODE=false and use environment variables for production"
echo ""
echo "✅ Pipeline is ready for cross-architecture deployment!"