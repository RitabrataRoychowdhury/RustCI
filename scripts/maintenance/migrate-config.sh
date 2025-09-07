#!/bin/bash

# Valkyrie Configuration Migration Script
# This script migrates existing RustCI configuration to Valkyrie format

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
RUSTCI_CONFIG_PATH=""
VALKYRIE_CONFIG_PATH="config/valkyrie.yaml"
BACKUP_DIR="config/backup"
DRY_RUN=false
VERBOSE=false
FORCE=false

# Print colored output
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

# Show usage information
show_usage() {
    cat << EOF
Valkyrie Configuration Migration Script

USAGE:
    $0 [OPTIONS] --source <RUSTCI_CONFIG>

OPTIONS:
    -s, --source <PATH>         Path to existing RustCI configuration file
    -o, --output <PATH>         Output path for Valkyrie configuration (default: config/valkyrie.yaml)
    -b, --backup-dir <PATH>     Backup directory (default: config/backup)
    -d, --dry-run              Show what would be migrated without making changes
    -v, --verbose              Enable verbose output
    -f, --force                Overwrite existing Valkyrie configuration
    -h, --help                 Show this help message

EXAMPLES:
    # Migrate from RustCI config
    $0 --source rustci.yaml

    # Dry run to see what would be migrated
    $0 --source rustci.yaml --dry-run

    # Migrate with custom output path
    $0 --source rustci.yaml --output my-valkyrie.yaml

    # Force overwrite existing configuration
    $0 --source rustci.yaml --force

ENVIRONMENT VARIABLES:
    RUSTCI_CONFIG_PATH         Default source configuration path
    VALKYRIE_CONFIG_PATH       Default output configuration path
    MIGRATION_BACKUP_DIR       Default backup directory

EOF
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -s|--source)
                RUSTCI_CONFIG_PATH="$2"
                shift 2
                ;;
            -o|--output)
                VALKYRIE_CONFIG_PATH="$2"
                shift 2
                ;;
            -b|--backup-dir)
                BACKUP_DIR="$2"
                shift 2
                ;;
            -d|--dry-run)
                DRY_RUN=true
                shift
                ;;
            -v|--verbose)
                VERBOSE=true
                shift
                ;;
            -f|--force)
                FORCE=true
                shift
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
}

# Validate prerequisites
validate_prerequisites() {
    print_info "Validating prerequisites..."
    
    # Check if source configuration exists
    if [[ -z "$RUSTCI_CONFIG_PATH" ]]; then
        print_error "Source configuration path not specified"
        show_usage
        exit 1
    fi
    
    if [[ ! -f "$RUSTCI_CONFIG_PATH" ]]; then
        print_error "Source configuration file not found: $RUSTCI_CONFIG_PATH"
        exit 1
    fi
    
    # Check if output file exists and force flag
    if [[ -f "$VALKYRIE_CONFIG_PATH" && "$FORCE" != true ]]; then
        print_error "Output file already exists: $VALKYRIE_CONFIG_PATH"
        print_info "Use --force to overwrite or specify a different output path"
        exit 1
    fi
    
    # Check required tools
    local required_tools=("yq" "jq")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            print_warning "$tool not found, some features may be limited"
        fi
    done
    
    print_success "Prerequisites validated"
}

# Create backup of existing configuration
create_backup() {
    if [[ "$DRY_RUN" == true ]]; then
        print_info "[DRY RUN] Would create backup directory: $BACKUP_DIR"
        return
    fi
    
    print_info "Creating backup..."
    
    # Create backup directory
    mkdir -p "$BACKUP_DIR"
    
    # Backup source configuration
    local backup_file="$BACKUP_DIR/rustci-config-$(date +%Y%m%d-%H%M%S).yaml"
    cp "$RUSTCI_CONFIG_PATH" "$backup_file"
    print_success "Source configuration backed up to: $backup_file"
    
    # Backup existing Valkyrie configuration if it exists
    if [[ -f "$VALKYRIE_CONFIG_PATH" ]]; then
        local valkyrie_backup="$BACKUP_DIR/valkyrie-config-$(date +%Y%m%d-%H%M%S).yaml"
        cp "$VALKYRIE_CONFIG_PATH" "$valkyrie_backup"
        print_success "Existing Valkyrie configuration backed up to: $valkyrie_backup"
    fi
}

# Extract configuration values from RustCI config
extract_rustci_config() {
    local config_file="$1"
    
    print_info "Extracting RustCI configuration..."
    
    # Initialize variables
    local server_port=""
    local server_bind=""
    local database_url=""
    local jwt_secret=""
    local log_level=""
    local enable_metrics=""
    
    # Extract values using different methods based on available tools
    if command -v yq &> /dev/null; then
        # Use yq for YAML parsing
        server_port=$(yq eval '.server.port // 3000' "$config_file" 2>/dev/null || echo "3000")
        server_bind=$(yq eval '.server.bind // "0.0.0.0"' "$config_file" 2>/dev/null || echo "0.0.0.0")
        database_url=$(yq eval '.database.url // ""' "$config_file" 2>/dev/null || echo "")
        jwt_secret=$(yq eval '.auth.jwt_secret // ""' "$config_file" 2>/dev/null || echo "")
        log_level=$(yq eval '.logging.level // "info"' "$config_file" 2>/dev/null || echo "info")
        enable_metrics=$(yq eval '.metrics.enabled // false' "$config_file" 2>/dev/null || echo "false")
    else
        # Fallback to grep-based extraction
        server_port=$(grep -E "^\s*port:" "$config_file" | sed 's/.*port:\s*//' | tr -d ' ' || echo "3000")
        server_bind=$(grep -E "^\s*bind:" "$config_file" | sed 's/.*bind:\s*//' | tr -d ' "' || echo "0.0.0.0")
        log_level=$(grep -E "^\s*level:" "$config_file" | sed 's/.*level:\s*//' | tr -d ' "' || echo "info")
    fi
    
    # Store extracted values in global variables for use in migration
    export EXTRACTED_SERVER_PORT="$server_port"
    export EXTRACTED_SERVER_BIND="$server_bind"
    export EXTRACTED_DATABASE_URL="$database_url"
    export EXTRACTED_JWT_SECRET="$jwt_secret"
    export EXTRACTED_LOG_LEVEL="$log_level"
    export EXTRACTED_ENABLE_METRICS="$enable_metrics"
    
    if [[ "$VERBOSE" == true ]]; then
        print_info "Extracted configuration:"
        echo "  Server Port: $server_port"
        echo "  Server Bind: $server_bind"
        echo "  Database URL: $database_url"
        echo "  JWT Secret: [REDACTED]"
        echo "  Log Level: $log_level"
        echo "  Enable Metrics: $enable_metrics"
    fi
}

# Generate Valkyrie configuration
generate_valkyrie_config() {
    print_info "Generating Valkyrie configuration..."
    
    # Determine environment based on extracted values or defaults
    local environment="development"
    if [[ "$EXTRACTED_ENABLE_METRICS" == "true" ]]; then
        environment="production"
    fi
    
    # Generate the configuration content
    cat > "$VALKYRIE_CONFIG_PATH" << EOF
# Valkyrie Protocol Configuration
# Migrated from RustCI configuration: $RUSTCI_CONFIG_PATH
# Migration date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")

# Global settings
global:
  environment: $environment
  log_level: $EXTRACTED_LOG_LEVEL
  enable_metrics: $EXTRACTED_ENABLE_METRICS
  metrics_endpoint: "http://localhost:9090/metrics"
  config_reload_interval: 30

# Server configuration
server:
  bind_address: "$EXTRACTED_SERVER_BIND"
  port: $EXTRACTED_SERVER_PORT
  max_connections: 10000
  connection_timeout_ms: 30000
  enable_tls: false
  enable_graceful_shutdown: true
  graceful_shutdown_timeout: 30

# Client configuration
client:
  endpoint: "tcp://localhost:8080"
  connect_timeout_ms: 5000
  request_timeout_ms: 30000
  max_connections: 10
  enable_pooling: true
  auto_reconnect: true
  reconnect_interval_ms: 1000
  retry_policy:
    max_attempts: 3
    initial_delay_ms: 100
    max_delay_ms: 5000
    backoff_multiplier: 2.0
    enable_jitter: true

# Transport configuration
transport:
  enabled_transports:
    - tcp
    - quic
    - unix
    - websocket
  default_transport: tcp
  tcp:
    keep_alive: true
    keep_alive_interval: 30
    no_delay: true
    send_buffer_size: 65536
    recv_buffer_size: 65536
  quic:
    max_concurrent_streams: 1000
    idle_timeout: 300
    enable_0rtt: true
  unix:
    socket_path: "/tmp/valkyrie.sock"
    socket_permissions: 0o600
  websocket:
    max_message_size: 16777216
    enable_compression: true

# Security configuration
security:
  enable_encryption: true
  encryption_algorithm: chacha20poly1305
  enable_authentication: true
  authentication_method: jwt
  jwt:
    secret_key: "\${JWT_SECRET_KEY}"
    expiration_time: 3600
    issuer: "valkyrie-protocol"
    audience: "valkyrie-clients"

# Plugin system configuration
plugins:
  enabled: true
  plugin_dir: "plugins"
  enable_hot_reload: true
  health_check_interval: 30
  max_startup_time: 60
  auto_restart: true
  max_restart_attempts: 3
  valkyrie:
    enabled: true
    endpoint: "\${VALKYRIE_ENDPOINT:tcp://localhost:8080}"
    connection_timeout_ms: 5000
    request_timeout_ms: 30000
    max_connections: 100
    enable_pooling: true
    health_check_interval: 30
    enable_metrics: true
    enable_http_fallback: true

# Fallback system configuration
fallback:
  enabled: true
  detection_timeout_ms: 5000
  max_attempts: 3
  retry_interval_ms: 1000
  http:
    base_url: "\${HTTP_FALLBACK_URL:http://localhost:$EXTRACTED_SERVER_PORT}"
    timeout_ms: 30000
    max_retries: 3
    enable_compression: true

# Observability configuration
observability:
  enabled: $EXTRACTED_ENABLE_METRICS
  metrics:
    enabled: $EXTRACTED_ENABLE_METRICS
    collection_interval: 10
    prometheus_endpoint: "/metrics"
    enable_custom_metrics: true
  tracing:
    enabled: true
    backend: jaeger
    jaeger:
      agent_endpoint: "localhost:6831"
      service_name: "valkyrie-protocol"
    sampling_rate: 0.1
  logging:
    format: json
    structured: true
    rotation:
      enabled: true
      max_size_mb: 100
      max_files: 10

# Performance tuning
performance:
  thread_pool:
    core_threads: 0
    max_threads: 1000
    keep_alive_time: 60
  memory:
    enable_pooling: true
    buffer_pool_size: 1000
    max_buffer_size: 1048576
  network:
    tcp_fast_open: true
    reuse_address: true
    reuse_port: true
    send_buffer_size: 262144
    recv_buffer_size: 262144

# Environment-specific overrides
overrides:
  development:
    global:
      log_level: debug
    server:
      port: $EXTRACTED_SERVER_PORT
    security:
      enable_encryption: false
      enable_authentication: false
  
  staging:
    global:
      log_level: info
    server:
      port: $EXTRACTED_SERVER_PORT
    security:
      enable_encryption: true
      enable_authentication: true
  
  production:
    global:
      log_level: warn
      enable_metrics: true
    server:
      port: 443
      enable_tls: true
    security:
      enable_encryption: true
      enable_authentication: true
      encryption_algorithm: aes256gcm
EOF
}

# Validate generated configuration
validate_generated_config() {
    print_info "Validating generated configuration..."
    
    # Check if the file was created
    if [[ ! -f "$VALKYRIE_CONFIG_PATH" ]]; then
        print_error "Generated configuration file not found"
        return 1
    fi
    
    # Basic YAML syntax validation
    if command -v yq &> /dev/null; then
        if ! yq eval '.' "$VALKYRIE_CONFIG_PATH" > /dev/null 2>&1; then
            print_error "Generated configuration has invalid YAML syntax"
            return 1
        fi
    fi
    
    # Check for required sections
    local required_sections=("global" "server" "client" "transport" "security")
    for section in "${required_sections[@]}"; do
        if command -v yq &> /dev/null; then
            if [[ $(yq eval ".$section" "$VALKYRIE_CONFIG_PATH" 2>/dev/null) == "null" ]]; then
                print_warning "Missing required section: $section"
            fi
        fi
    done
    
    print_success "Configuration validation completed"
}

# Show migration summary
show_migration_summary() {
    print_info "Migration Summary:"
    echo "  Source: $RUSTCI_CONFIG_PATH"
    echo "  Output: $VALKYRIE_CONFIG_PATH"
    echo "  Backup Directory: $BACKUP_DIR"
    echo "  Environment: $(yq eval '.global.environment' "$VALKYRIE_CONFIG_PATH" 2>/dev/null || echo "unknown")"
    echo "  Server Port: $(yq eval '.server.port' "$VALKYRIE_CONFIG_PATH" 2>/dev/null || echo "unknown")"
    echo "  Log Level: $(yq eval '.global.log_level' "$VALKYRIE_CONFIG_PATH" 2>/dev/null || echo "unknown")"
    
    print_success "Migration completed successfully!"
    print_info "Next steps:"
    echo "  1. Review the generated configuration: $VALKYRIE_CONFIG_PATH"
    echo "  2. Set required environment variables (JWT_SECRET_KEY, etc.)"
    echo "  3. Validate the configuration: cargo run --bin valkyrie-tools config validate"
    echo "  4. Test the configuration: cargo run --bin valkyrie-server check"
}

# Main migration function
main() {
    print_info "Starting Valkyrie configuration migration..."
    
    # Parse command line arguments
    parse_args "$@"
    
    # Use environment variables as defaults if not specified
    RUSTCI_CONFIG_PATH="${RUSTCI_CONFIG_PATH:-${RUSTCI_CONFIG_PATH:-}}"
    VALKYRIE_CONFIG_PATH="${VALKYRIE_CONFIG_PATH:-${VALKYRIE_CONFIG_PATH:-config/valkyrie.yaml}}"
    BACKUP_DIR="${BACKUP_DIR:-${MIGRATION_BACKUP_DIR:-config/backup}}"
    
    # Validate prerequisites
    validate_prerequisites
    
    # Create backup
    create_backup
    
    # Extract RustCI configuration
    extract_rustci_config "$RUSTCI_CONFIG_PATH"
    
    if [[ "$DRY_RUN" == true ]]; then
        print_info "[DRY RUN] Would generate Valkyrie configuration at: $VALKYRIE_CONFIG_PATH"
        print_info "[DRY RUN] Configuration would include:"
        echo "  Environment: development/production (based on metrics setting)"
        echo "  Server: $EXTRACTED_SERVER_BIND:$EXTRACTED_SERVER_PORT"
        echo "  Log Level: $EXTRACTED_LOG_LEVEL"
        echo "  Metrics: $EXTRACTED_ENABLE_METRICS"
        print_info "[DRY RUN] Migration simulation completed"
        return 0
    fi
    
    # Create output directory if it doesn't exist
    mkdir -p "$(dirname "$VALKYRIE_CONFIG_PATH")"
    
    # Generate Valkyrie configuration
    generate_valkyrie_config
    
    # Validate generated configuration
    validate_generated_config
    
    # Show migration summary
    show_migration_summary
}

# Run main function with all arguments
main "$@"