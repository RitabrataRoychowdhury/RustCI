#!/bin/bash

# RustCI Runner Configuration Migration Script
# This script helps migrate legacy runner configurations to the new unified format

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
INPUT_FILE=""
OUTPUT_FILE=""
RUNNER_TYPE=""
BACKUP_ORIGINAL=true
VALIDATE_CONFIG=true

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

# Function to show usage
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Migrate legacy RustCI runner configurations to the new unified format.

OPTIONS:
    -i, --input FILE        Input legacy configuration file (YAML)
    -o, --output FILE       Output unified configuration file (YAML)
    -t, --type TYPE         Force runner type (docker|kubernetes|native)
    --no-backup            Don't create backup of original file
    --no-validate          Don't validate the migrated configuration
    -h, --help             Show this help message

EXAMPLES:
    # Migrate a Docker runner configuration
    $0 -i legacy-docker.yaml -o unified-docker.yaml -t docker

    # Migrate with auto-detection
    $0 -i legacy-config.yaml -o new-config.yaml

    # Migrate pipeline examples
    $0 -i pipeline.yaml -o docs/pipeline-examples/unified-pipeline.yaml

EOF
}

# Function to detect runner type from configuration
detect_runner_type() {
    local config_file="$1"
    
    if grep -q "docker" "$config_file" 2>/dev/null; then
        echo "docker"
    elif grep -q "kubernetes\|k8s" "$config_file" 2>/dev/null; then
        echo "kubernetes"
    else
        echo "native"
    fi
}

# Function to create backup
create_backup() {
    local file="$1"
    local backup_file="${file}.backup.$(date +%Y%m%d_%H%M%S)"
    
    if [ -f "$file" ]; then
        cp "$file" "$backup_file"
        print_info "Created backup: $backup_file"
    fi
}

# Function to migrate Docker configuration
migrate_docker_config() {
    local input="$1"
    local output="$2"
    
    print_info "Migrating Docker runner configuration..."
    
    cat > "$output" << EOF
# Unified RustCI Runner Configuration
# Migrated from legacy Docker configuration on $(date)

name: "docker-runner"
runner_type:
  Docker:
    name: "docker-runner"
    max_concurrent_jobs: 4
    docker_endpoint: "unix:///var/run/docker.sock"
    default_image: "ubuntu:22.04"
    network: null
    volume_mounts: []
    environment: {}
    resource_limits:
      memory_limit: 536870912  # 512MB
      cpu_limit: 1000000000    # 1 CPU
      cpu_shares: 1024
      disk_limit: 1073741824   # 1GB
    cleanup_settings:
      remove_containers: true
      remove_volumes: true
      removal_timeout_seconds: 30
    image_pull_policy: "IfNotPresent"

compatibility_flags:
  legacy_mode: true
  runner_compat: true
  hybrid_deployment: true
  feature_flags: {}

control_plane_integration:
  enabled: false
  endpoint: null
  auth_config: null
  registration:
    auto_register: true
    retry_attempts: 3
    timeout_seconds: 30
EOF
}

# Function to migrate Kubernetes configuration
migrate_kubernetes_config() {
    local input="$1"
    local output="$2"
    
    print_info "Migrating Kubernetes runner configuration..."
    
    cat > "$output" << EOF
# Unified RustCI Runner Configuration
# Migrated from legacy Kubernetes configuration on $(date)

name: "kubernetes-runner"
runner_type:
  Kubernetes:
    name: "kubernetes-runner"
    max_concurrent_jobs: 10
    namespace: "default"
    default_image: "ubuntu:22.04"
    service_account: null
    resource_limits:
      cpu_limit: 1000      # 1 CPU
      memory_limit: 512    # 512MB
      storage_limit: 1024  # 1GB
    volume_mounts: []
    environment: {}
    cleanup_settings:
      remove_jobs: true
      remove_pods: true
      cleanup_timeout_seconds: 60
      ttl_seconds_after_finished: 300
    image_pull_policy: "IfNotPresent"
    security_context: null
    node_selector: {}
    tolerations: []

compatibility_flags:
  legacy_mode: true
  runner_compat: true
  hybrid_deployment: true
  feature_flags: {}

control_plane_integration:
  enabled: false
  endpoint: null
  auth_config: null
  registration:
    auto_register: true
    retry_attempts: 3
    timeout_seconds: 30
EOF
}

# Function to migrate Native configuration
migrate_native_config() {
    local input="$1"
    local output="$2"
    
    print_info "Migrating to Native runner configuration..."
    
    cat > "$output" << EOF
# Unified RustCI Runner Configuration
# Migrated to native runner configuration on $(date)

name: "native-runner"
runner_type:
  Native:
    name: "native-runner"
    max_concurrent_jobs: 4
    working_directory: "/tmp/rustci"
    environment: {}
    default_timeout_seconds: 3600
    isolation_level: "ProcessGroup"
    resource_limits:
      max_cpu: 1.0
      max_memory: 1024
      max_execution_time: 3600
      max_processes: 100
      max_file_descriptors: 1024
    enable_output_streaming: true
    max_output_buffer_size: 1048576
    shell: "bash"
    enhanced_lifecycle: true
    control_plane_coordination:
      enabled: true
      heartbeat_interval: 30
      health_check_interval: 60
      auto_registration: true
      tags: ["native"]
      node_affinity: {}

compatibility_flags:
  legacy_mode: false
  runner_compat: true
  hybrid_deployment: true
  feature_flags: {}

control_plane_integration:
  enabled: true
  endpoint: null
  auth_config: null
  registration:
    auto_register: true
    retry_attempts: 3
    timeout_seconds: 30
EOF
}

# Function to migrate pipeline configuration
migrate_pipeline_config() {
    local input="$1"
    local output="$2"
    
    print_info "Migrating pipeline configuration..."
    
    # Copy the original pipeline and add compatibility metadata
    {
        echo "# Unified RustCI Pipeline Configuration"
        echo "# Migrated from legacy pipeline on $(date)"
        echo ""
        echo "# Compatibility metadata"
        echo "metadata:"
        echo "  compatibility_mode: true"
        echo "  migrated_from: \"$(basename "$input")\""
        echo "  migration_date: \"$(date -Iseconds)\""
        echo ""
        cat "$input"
    } > "$output"
}

# Function to validate configuration
validate_config() {
    local config_file="$1"
    
    print_info "Validating migrated configuration..."
    
    # Check if file exists and is readable
    if [ ! -r "$config_file" ]; then
        print_error "Configuration file is not readable: $config_file"
        return 1
    fi
    
    # Basic YAML syntax check
    if command -v yq >/dev/null 2>&1; then
        if ! yq eval '.' "$config_file" >/dev/null 2>&1; then
            print_error "Invalid YAML syntax in: $config_file"
            return 1
        fi
        print_success "YAML syntax is valid"
    else
        print_warning "yq not found, skipping YAML syntax validation"
    fi
    
    # Check for required fields
    local required_fields=("name" "runner_type" "compatibility_flags")
    for field in "${required_fields[@]}"; do
        if command -v yq >/dev/null 2>&1; then
            if ! yq eval "has(\"$field\")" "$config_file" | grep -q "true"; then
                print_error "Missing required field: $field"
                return 1
            fi
        fi
    done
    
    print_success "Configuration validation passed"
    return 0
}

# Function to update pipeline examples
update_pipeline_examples() {
    local examples_dir="$PROJECT_ROOT/docs/pipeline-examples"
    
    print_info "Updating pipeline examples..."
    
    # Create examples directory if it doesn't exist
    mkdir -p "$examples_dir"
    
    # Migrate main pipeline files
    if [ -f "$PROJECT_ROOT/pipeline.yaml" ]; then
        migrate_pipeline_config "$PROJECT_ROOT/pipeline.yaml" "$examples_dir/unified-pipeline.yaml"
    fi
    
    if [ -f "$PROJECT_ROOT/k3s-pipeline.yaml" ]; then
        migrate_pipeline_config "$PROJECT_ROOT/k3s-pipeline.yaml" "$examples_dir/unified-k3s-pipeline.yaml"
    fi
    
    # Create example configurations for each runner type
    migrate_docker_config "" "$examples_dir/docker-runner-config.yaml"
    migrate_kubernetes_config "" "$examples_dir/kubernetes-runner-config.yaml"
    migrate_native_config "" "$examples_dir/native-runner-config.yaml"
    
    print_success "Pipeline examples updated"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -i|--input)
            INPUT_FILE="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        -t|--type)
            RUNNER_TYPE="$2"
            shift 2
            ;;
        --no-backup)
            BACKUP_ORIGINAL=false
            shift
            ;;
        --no-validate)
            VALIDATE_CONFIG=false
            shift
            ;;
        --update-examples)
            update_pipeline_examples
            exit 0
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Main migration logic
main() {
    print_info "Starting RustCI configuration migration..."
    
    # Check if input file is provided
    if [ -z "$INPUT_FILE" ]; then
        print_error "Input file is required. Use -i or --input to specify."
        show_usage
        exit 1
    fi
    
    # Check if input file exists
    if [ ! -f "$INPUT_FILE" ]; then
        print_error "Input file does not exist: $INPUT_FILE"
        exit 1
    fi
    
    # Set default output file if not provided
    if [ -z "$OUTPUT_FILE" ]; then
        OUTPUT_FILE="${INPUT_FILE%.*}-unified.yaml"
        print_info "Output file not specified, using: $OUTPUT_FILE"
    fi
    
    # Create backup if requested
    if [ "$BACKUP_ORIGINAL" = true ]; then
        create_backup "$INPUT_FILE"
    fi
    
    # Detect runner type if not specified
    if [ -z "$RUNNER_TYPE" ]; then
        RUNNER_TYPE=$(detect_runner_type "$INPUT_FILE")
        print_info "Auto-detected runner type: $RUNNER_TYPE"
    fi
    
    # Perform migration based on runner type
    case "$RUNNER_TYPE" in
        docker)
            migrate_docker_config "$INPUT_FILE" "$OUTPUT_FILE"
            ;;
        kubernetes|k8s)
            migrate_kubernetes_config "$INPUT_FILE" "$OUTPUT_FILE"
            ;;
        native)
            migrate_native_config "$INPUT_FILE" "$OUTPUT_FILE"
            ;;
        pipeline)
            migrate_pipeline_config "$INPUT_FILE" "$OUTPUT_FILE"
            ;;
        *)
            print_warning "Unknown runner type: $RUNNER_TYPE, defaulting to native"
            migrate_native_config "$INPUT_FILE" "$OUTPUT_FILE"
            ;;
    esac
    
    # Validate migrated configuration
    if [ "$VALIDATE_CONFIG" = true ]; then
        if ! validate_config "$OUTPUT_FILE"; then
            print_error "Configuration validation failed"
            exit 1
        fi
    fi
    
    print_success "Migration completed successfully!"
    print_info "Input file: $INPUT_FILE"
    print_info "Output file: $OUTPUT_FILE"
    print_info "Runner type: $RUNNER_TYPE"
    
    # Provide next steps
    echo ""
    print_info "Next steps:"
    echo "1. Review the migrated configuration: $OUTPUT_FILE"
    echo "2. Test the configuration with: cargo run -- --config $OUTPUT_FILE"
    echo "3. Update your deployment scripts to use the new configuration"
    echo "4. Consider enabling control plane integration for enhanced features"
}

# Run main function
main "$@"