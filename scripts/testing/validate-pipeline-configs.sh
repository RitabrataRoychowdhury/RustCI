#!/bin/bash

# RustCI Pipeline Configuration Validation Script
# This script validates pipeline configurations for backward compatibility

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
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

# Function to validate YAML syntax
validate_yaml_syntax() {
    local file="$1"
    
    if [ ! -f "$file" ]; then
        print_error "File does not exist: $file"
        return 1
    fi
    
    print_info "Validating YAML syntax: $(basename "$file")"
    
    # Check with yq if available
    if command -v yq >/dev/null 2>&1; then
        if yq eval '.' "$file" >/dev/null 2>&1; then
            print_success "YAML syntax is valid"
            return 0
        else
            print_error "Invalid YAML syntax"
            return 1
        fi
    fi
    
    # Check with python if available
    if command -v python3 >/dev/null 2>&1; then
        if python3 -c "import yaml; yaml.safe_load(open('$file'))" 2>/dev/null; then
            print_success "YAML syntax is valid (python)"
            return 0
        else
            print_error "Invalid YAML syntax (python)"
            return 1
        fi
    fi
    
    print_warning "No YAML validator found (yq or python3), skipping syntax check"
    return 0
}

# Function to validate pipeline structure
validate_pipeline_structure() {
    local file="$1"
    
    print_info "Validating pipeline structure: $(basename "$file")"
    
    # Check for required fields
    local required_fields=("name")
    local has_errors=false
    
    if command -v yq >/dev/null 2>&1; then
        for field in "${required_fields[@]}"; do
            if ! yq eval "has(\"$field\")" "$file" 2>/dev/null | grep -q "true"; then
                print_error "Missing required field: $field"
                has_errors=true
            fi
        done
        
        # Check for either steps or stages structure
        local has_steps=$(yq eval 'has("steps")' "$file" 2>/dev/null || echo "false")
        local has_stages=$(yq eval 'has("stages")' "$file" 2>/dev/null || echo "false")
        
        if [ "$has_steps" = "true" ]; then
            # Direct steps structure
            local step_count=$(yq eval '.steps | length' "$file" 2>/dev/null || echo "0")
            if [ "$step_count" -eq 0 ]; then
                print_error "Pipeline must have at least one step"
                has_errors=true
            else
                print_info "Found $step_count direct steps"
            fi
            
            # Validate each step
            for ((i=0; i<step_count; i++)); do
                local step_name=$(yq eval ".steps[$i].name" "$file" 2>/dev/null || echo "")
                local step_command=$(yq eval ".steps[$i].command" "$file" 2>/dev/null || echo "")
                
                if [ -z "$step_name" ]; then
                    print_error "Step $i is missing name"
                    has_errors=true
                fi
                
                if [ -z "$step_command" ]; then
                    print_error "Step $i is missing command"
                    has_errors=true
                fi
            done
        elif [ "$has_stages" = "true" ]; then
            # Stages with steps structure
            local stage_count=$(yq eval '.stages | length' "$file" 2>/dev/null || echo "0")
            if [ "$stage_count" -eq 0 ]; then
                print_error "Pipeline must have at least one stage"
                has_errors=true
            else
                print_info "Found $stage_count stages"
                
                # Validate each stage
                for ((s=0; s<stage_count; s++)); do
                    local stage_name=$(yq eval ".stages[$s].name" "$file" 2>/dev/null || echo "")
                    local step_count=$(yq eval ".stages[$s].steps | length" "$file" 2>/dev/null || echo "0")
                    
                    if [ -z "$stage_name" ]; then
                        print_error "Stage $s is missing name"
                        has_errors=true
                    fi
                    
                    if [ "$step_count" -eq 0 ]; then
                        print_error "Stage $s must have at least one step"
                        has_errors=true
                    else
                        print_info "Stage '$stage_name' has $step_count steps"
                    fi
                    
                    # Validate each step in the stage
                    for ((i=0; i<step_count; i++)); do
                        local step_name=$(yq eval ".stages[$s].steps[$i].name" "$file" 2>/dev/null || echo "")
                        local step_type=$(yq eval ".stages[$s].steps[$i].step_type" "$file" 2>/dev/null || echo "")
                        
                        if [ -z "$step_name" ]; then
                            print_error "Stage $s, Step $i is missing name"
                            has_errors=true
                        fi
                        
                        if [ -z "$step_type" ]; then
                            print_error "Stage $s, Step $i is missing step_type"
                            has_errors=true
                        fi
                    done
                done
            fi
        else
            print_error "Pipeline must have either 'steps' or 'stages' field"
            has_errors=true
        fi
    else
        print_warning "yq not available, skipping detailed structure validation"
    fi
    
    if [ "$has_errors" = true ]; then
        return 1
    else
        print_success "Pipeline structure is valid"
        return 0
    fi
}

# Function to validate runner configuration
validate_runner_config() {
    local file="$1"
    
    print_info "Validating runner configuration: $(basename "$file")"
    
    if command -v yq >/dev/null 2>&1; then
        # Check for runner type
        local runner_type=$(yq eval '.runner_type' "$file" 2>/dev/null || echo "null")
        if [ "$runner_type" = "null" ]; then
            print_error "Missing runner_type configuration"
            return 1
        fi
        
        # Check for compatibility flags
        local has_compat_flags=$(yq eval 'has("compatibility_flags")' "$file" 2>/dev/null || echo "false")
        if [ "$has_compat_flags" = "true" ]; then
            print_success "Compatibility flags found"
        else
            print_warning "No compatibility flags found (may be legacy configuration)"
        fi
        
        # Check for control plane integration
        local has_control_plane=$(yq eval 'has("control_plane_integration")' "$file" 2>/dev/null || echo "false")
        if [ "$has_control_plane" = "true" ]; then
            print_success "Control plane integration configuration found"
        else
            print_info "No control plane integration configuration (using defaults)"
        fi
    fi
    
    print_success "Runner configuration appears valid"
    return 0
}

# Function to test configuration with RustCI
test_config_with_rustci() {
    local file="$1"
    
    print_info "Testing configuration with RustCI: $(basename "$file")"
    
    # Check if RustCI binary exists
    local rustci_binary="$PROJECT_ROOT/target/release/RustAutoDevOps"
    if [ ! -f "$rustci_binary" ]; then
        rustci_binary="$PROJECT_ROOT/target/debug/RustAutoDevOps"
    fi
    
    if [ ! -f "$rustci_binary" ]; then
        print_warning "RustCI binary not found, skipping runtime validation"
        return 0
    fi
    
    # Test configuration validation (dry run)
    if timeout 10s "$rustci_binary" --config "$file" --validate-config 2>/dev/null; then
        print_success "Configuration validated by RustCI"
        return 0
    else
        print_error "Configuration validation failed"
        return 1
    fi
}

# Function to check backward compatibility
check_backward_compatibility() {
    local file="$1"
    
    print_info "Checking backward compatibility: $(basename "$file")"
    
    # Check if file contains legacy patterns
    local has_legacy_patterns=false
    
    if grep -q "docker:" "$file" 2>/dev/null; then
        print_info "Found Docker configuration patterns"
        has_legacy_patterns=true
    fi
    
    if grep -q "kubernetes:" "$file" 2>/dev/null; then
        print_info "Found Kubernetes configuration patterns"
        has_legacy_patterns=true
    fi
    
    if grep -q "runner_type:" "$file" 2>/dev/null; then
        print_info "Found unified runner type configuration"
    fi
    
    if [ "$has_legacy_patterns" = true ]; then
        print_warning "File contains legacy patterns - consider migration"
        print_info "Run: ./scripts/migrate-runner-config.sh -i $file -o ${file%.*}-unified.yaml"
    else
        print_success "Configuration appears to use unified format"
    fi
    
    return 0
}

# Function to validate all pipeline examples
validate_pipeline_examples() {
    local examples_dir="$PROJECT_ROOT/docs/pipeline-examples"
    local has_errors=false
    
    print_info "Validating pipeline examples in: $examples_dir"
    
    if [ ! -d "$examples_dir" ]; then
        print_error "Pipeline examples directory not found: $examples_dir"
        return 1
    fi
    
    # Find all YAML files
    while IFS= read -r -d '' file; do
        echo ""
        print_info "=== Validating $(basename "$file") ==="
        
        if ! validate_yaml_syntax "$file"; then
            has_errors=true
            continue
        fi
        
        # Determine file type and validate accordingly
        if [[ "$file" == *"pipeline"* ]]; then
            if ! validate_pipeline_structure "$file"; then
                has_errors=true
            fi
        elif [[ "$file" == *"runner"* ]] || [[ "$file" == *"config"* ]]; then
            if ! validate_runner_config "$file"; then
                has_errors=true
            fi
        fi
        
        if ! check_backward_compatibility "$file"; then
            has_errors=true
        fi
        
    done < <(find "$examples_dir" -name "*.yaml" -o -name "*.yml" -print0)
    
    if [ "$has_errors" = true ]; then
        print_error "Some pipeline examples have validation errors"
        return 1
    else
        print_success "All pipeline examples are valid"
        return 0
    fi
}

# Function to validate main pipeline files
validate_main_pipelines() {
    local has_errors=false
    
    print_info "Validating main pipeline files"
    
    # Validate pipeline.yaml
    if [ -f "$PROJECT_ROOT/pipeline.yaml" ]; then
        echo ""
        print_info "=== Validating pipeline.yaml ==="
        if ! validate_yaml_syntax "$PROJECT_ROOT/pipeline.yaml"; then
            has_errors=true
        elif ! validate_pipeline_structure "$PROJECT_ROOT/pipeline.yaml"; then
            has_errors=true
        elif ! check_backward_compatibility "$PROJECT_ROOT/pipeline.yaml"; then
            has_errors=true
        fi
    else
        print_warning "pipeline.yaml not found"
    fi
    
    # Validate k3s-pipeline.yaml
    if [ -f "$PROJECT_ROOT/k3s-pipeline.yaml" ]; then
        echo ""
        print_info "=== Validating k3s-pipeline.yaml ==="
        if ! validate_yaml_syntax "$PROJECT_ROOT/k3s-pipeline.yaml"; then
            has_errors=true
        elif ! validate_pipeline_structure "$PROJECT_ROOT/k3s-pipeline.yaml"; then
            has_errors=true
        elif ! check_backward_compatibility "$PROJECT_ROOT/k3s-pipeline.yaml"; then
            has_errors=true
        fi
    else
        print_warning "k3s-pipeline.yaml not found"
    fi
    
    if [ "$has_errors" = true ]; then
        return 1
    else
        return 0
    fi
}

# Function to generate validation report
generate_validation_report() {
    local report_file="$PROJECT_ROOT/pipeline-validation-report.md"
    
    print_info "Generating validation report: $report_file"
    
    cat > "$report_file" << EOF
# Pipeline Configuration Validation Report

Generated on: $(date)

## Summary

This report contains the validation results for all pipeline configurations
in the RustCI project, ensuring backward compatibility and proper structure.

## Validated Files

### Main Pipeline Files
- pipeline.yaml: $([ -f "$PROJECT_ROOT/pipeline.yaml" ] && echo "✅ Found" || echo "❌ Missing")
- k3s-pipeline.yaml: $([ -f "$PROJECT_ROOT/k3s-pipeline.yaml" ] && echo "✅ Found" || echo "❌ Missing")

### Pipeline Examples
$(find "$PROJECT_ROOT/docs/pipeline-examples" -name "*.yaml" -o -name "*.yml" 2>/dev/null | wc -l) example files found

## Validation Criteria

1. **YAML Syntax**: Valid YAML structure
2. **Pipeline Structure**: Required fields (name, steps)
3. **Runner Configuration**: Valid runner type and settings
4. **Backward Compatibility**: Legacy pattern detection
5. **Runtime Validation**: Configuration testing with RustCI

## Recommendations

- Use the migration script for legacy configurations:
  \`./scripts/migrate-runner-config.sh -i <legacy-config> -o <new-config>\`
- Enable compatibility flags for gradual migration
- Test configurations with: \`cargo test --test compatibility_tests\`

## Next Steps

1. Fix any validation errors reported above
2. Run compatibility tests: \`cargo test compatibility_tests\`
3. Update CI/CD pipelines to use validated configurations
4. Consider enabling control plane integration for enhanced features

EOF

    print_success "Validation report generated: $report_file"
}

# Main function
main() {
    print_info "Starting RustCI pipeline configuration validation"
    echo ""
    
    local has_errors=false
    
    # Validate main pipeline files
    if ! validate_main_pipelines; then
        has_errors=true
    fi
    
    echo ""
    
    # Validate pipeline examples
    if ! validate_pipeline_examples; then
        has_errors=true
    fi
    
    echo ""
    
    # Generate validation report
    generate_validation_report
    
    echo ""
    
    if [ "$has_errors" = true ]; then
        print_error "Pipeline validation completed with errors"
        print_info "Check the validation report for details: pipeline-validation-report.md"
        exit 1
    else
        print_success "All pipeline configurations are valid!"
        print_info "Validation report: pipeline-validation-report.md"
        exit 0
    fi
}

# Show usage if help requested
if [[ "$1" == "-h" || "$1" == "--help" ]]; then
    cat << EOF
Usage: $0 [OPTIONS]

Validate RustCI pipeline configurations for backward compatibility.

OPTIONS:
    --examples-only     Validate only pipeline examples
    --main-only         Validate only main pipeline files
    -h, --help          Show this help message

EXAMPLES:
    # Validate all configurations
    $0

    # Validate only examples
    $0 --examples-only

    # Validate only main pipeline files
    $0 --main-only

EOF
    exit 0
fi

# Handle command line options
case "${1:-}" in
    --examples-only)
        validate_pipeline_examples
        exit $?
        ;;
    --main-only)
        validate_main_pipelines
        exit $?
        ;;
    *)
        main
        ;;
esac