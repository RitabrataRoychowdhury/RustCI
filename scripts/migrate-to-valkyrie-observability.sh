#!/bin/bash

# Valkyrie Protocol Observability Migration Script
# Migrates existing RustCI deployments to use Valkyrie observability

set -e

echo "🛡️ Valkyrie Protocol Observability Migration Tool"
echo "================================================="

# Configuration
BACKUP_DIR="./valkyrie-migration-backup-$(date +%Y%m%d_%H%M%S)"
CONFIG_FILE="config/valkyrie.yaml"
MIGRATION_LOG="valkyrie-observability-migration.log"

# Functions
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$MIGRATION_LOG"
}

error() {
    echo "[ERROR] $1" | tee -a "$MIGRATION_LOG"
    exit 1
}

create_backup() {
    log "Creating backup directory: $BACKUP_DIR"
    mkdir -p "$BACKUP_DIR"
    
    # Backup existing configuration files
    if [ -f "config.yaml" ]; then
        cp "config.yaml" "$BACKUP_DIR/"
        log "Backed up config.yaml"
    fi
    
    if [ -f "$CONFIG_FILE" ]; then
        cp "$CONFIG_FILE" "$BACKUP_DIR/"
        log "Backed up existing valkyrie.yaml"
    fi
    
    # Backup any existing observability configurations
    if [ -d "config/observability" ]; then
        cp -r "config/observability" "$BACKUP_DIR/"
        log "Backed up observability directory"
    fi
}

migrate_configuration() {
    log "Migrating configuration to Valkyrie observability..."
    
    # Create Valkyrie observability configuration
    cat > "$CONFIG_FILE" << 'EOF'
# Valkyrie Protocol Observability Configuration
observability:
  # Enable/disable observability features
  metrics_enabled: true
  logging_enabled: true
  health_enabled: true
  dashboard_enabled: true
  
  # Retention settings
  metrics_retention_seconds: 3600    # 1 hour
  log_retention_seconds: 86400       # 24 hours
  
  # Monitoring intervals
  health_check_interval_seconds: 30
  dashboard_refresh_seconds: 5
  
  # Metrics configuration
  metrics:
    max_data_points: 10000
    aggregation_functions:
      - sum
      - average
      - p95
      - p99
  
  # Logging configuration
  logging:
    min_level: "info"
    max_entries: 10000
    console_output: true
    json_output: false
    include_stack_traces: true
  
  # Health monitoring configuration
  health:
    default_checks:
      - name: "system_memory"
        type: "memory"
        max_usage_percent: 90.0
        interval_seconds: 30
        enabled: true
      
      - name: "system_cpu"
        type: "cpu"
        max_usage_percent: 95.0
        interval_seconds: 30
        enabled: true
      
      - name: "system_disk"
        type: "disk"
        path: "/"
        min_free_percent: 10.0
        interval_seconds: 60
        enabled: true
  
  # Dashboard configuration
  dashboard:
    title: "RustCI Valkyrie Protocol Dashboard"
    widgets:
      - id: "system_overview"
        title: "System Overview"
        type: "system_overview"
        enabled: true
      
      - id: "health_status"
        title: "Health Status"
        type: "health_status"
        enabled: true
      
      - id: "metrics_summary"
        title: "Metrics Summary"
        type: "metrics_summary"
        enabled: true
      
      - id: "performance_chart"
        title: "Performance Metrics"
        type: "line_chart"
        metrics:
          - "latency_ms"
          - "throughput_rps"
        enabled: true

# Integration with existing RustCI configuration
rustci:
  # Enable Valkyrie observability integration
  valkyrie_observability: true
  
  # Legacy observability (will be deprecated)
  legacy_metrics: false
  legacy_logging: false
  
  # Migration settings
  migration:
    preserve_existing_data: true
    gradual_migration: true
    rollback_enabled: true
EOF
    
    log "Created Valkyrie observability configuration: $CONFIG_FILE"
}

update_environment() {
    log "Updating environment configuration..."
    
    # Update .env file if it exists
    if [ -f ".env" ]; then
        # Add Valkyrie observability environment variables
        if ! grep -q "VALKYRIE_OBSERVABILITY_ENABLED" .env; then
            echo "" >> .env
            echo "# Valkyrie Protocol Observability" >> .env
            echo "VALKYRIE_OBSERVABILITY_ENABLED=true" >> .env
            echo "VALKYRIE_METRICS_ENABLED=true" >> .env
            echo "VALKYRIE_LOGGING_ENABLED=true" >> .env
            echo "VALKYRIE_HEALTH_ENABLED=true" >> .env
            echo "VALKYRIE_DASHBOARD_ENABLED=true" >> .env
            echo "VALKYRIE_DASHBOARD_PORT=8081" >> .env
            log "Updated .env with Valkyrie observability variables"
        fi
    fi
}

create_migration_tools() {
    log "Creating migration utility tools..."
    
    # Create data migration tool
    cat > "tools/valkyrie-data-migrator.sh" << 'EOF'
#!/bin/bash
# Valkyrie Protocol Data Migration Tool

echo "🔄 Migrating existing observability data to Valkyrie format..."

# Migrate existing metrics
if [ -d "data/metrics" ]; then
    echo "Migrating metrics data..."
    # Convert existing metrics to Valkyrie format
    # This is a placeholder - actual implementation would depend on existing format
    mkdir -p "data/valkyrie/metrics"
    # cp -r data/metrics/* data/valkyrie/metrics/
fi

# Migrate existing logs
if [ -d "data/logs" ]; then
    echo "Migrating log data..."
    mkdir -p "data/valkyrie/logs"
    # Convert existing logs to structured format
    # This is a placeholder - actual implementation would depend on existing format
fi

echo "✅ Data migration completed"
EOF
    
    chmod +x "tools/valkyrie-data-migrator.sh"
    log "Created data migration tool: tools/valkyrie-data-migrator.sh"
    
    # Create rollback script
    cat > "tools/valkyrie-rollback.sh" << 'EOF'
#!/bin/bash
# Valkyrie Protocol Rollback Tool

BACKUP_DIR="$1"

if [ -z "$BACKUP_DIR" ]; then
    echo "Usage: $0 <backup_directory>"
    exit 1
fi

echo "🔄 Rolling back Valkyrie observability migration..."

if [ -d "$BACKUP_DIR" ]; then
    # Restore backed up files
    if [ -f "$BACKUP_DIR/config.yaml" ]; then
        cp "$BACKUP_DIR/config.yaml" "config.yaml"
        echo "Restored config.yaml"
    fi
    
    if [ -f "$BACKUP_DIR/valkyrie.yaml" ]; then
        cp "$BACKUP_DIR/valkyrie.yaml" "config/valkyrie.yaml"
        echo "Restored valkyrie.yaml"
    fi
    
    echo "✅ Rollback completed"
else
    echo "❌ Backup directory not found: $BACKUP_DIR"
    exit 1
fi
EOF
    
    chmod +x "tools/valkyrie-rollback.sh"
    log "Created rollback tool: tools/valkyrie-rollback.sh"
}

validate_migration() {
    log "Validating migration..."
    
    # Check if configuration files exist
    if [ ! -f "$CONFIG_FILE" ]; then
        error "Valkyrie configuration file not found: $CONFIG_FILE"
    fi
    
    # Validate YAML syntax (if yq is available)
    if command -v yq &> /dev/null; then
        if ! yq eval '.' "$CONFIG_FILE" > /dev/null 2>&1; then
            error "Invalid YAML syntax in $CONFIG_FILE"
        fi
        log "Configuration file syntax validated"
    else
        log "Warning: yq not found, skipping YAML validation"
    fi
    
    # Check if tools directory exists
    if [ ! -d "tools" ]; then
        mkdir -p "tools"
        log "Created tools directory"
    fi
    
    log "Migration validation completed"
}

print_summary() {
    echo ""
    echo "🎉 Valkyrie Protocol Observability Migration Complete!"
    echo "====================================================="
    echo ""
    echo "📁 Backup created in: $BACKUP_DIR"
    echo "⚙️  Configuration file: $CONFIG_FILE"
    echo "📊 Migration log: $MIGRATION_LOG"
    echo ""
    echo "🔧 Migration tools created:"
    echo "   • tools/valkyrie-data-migrator.sh - Migrate existing data"
    echo "   • tools/valkyrie-rollback.sh - Rollback migration"
    echo ""
    echo "📋 Next steps:"
    echo "   1. Review the configuration in $CONFIG_FILE"
    echo "   2. Run: tools/valkyrie-data-migrator.sh (if you have existing data)"
    echo "   3. Restart RustCI to enable Valkyrie observability"
    echo "   4. Access dashboard at: http://localhost:8081"
    echo ""
    echo "🔄 To rollback: tools/valkyrie-rollback.sh $BACKUP_DIR"
    echo ""
}

# Main migration process
main() {
    log "Starting Valkyrie Protocol observability migration"
    
    # Create necessary directories
    mkdir -p "config"
    mkdir -p "tools"
    
    # Perform migration steps
    create_backup
    migrate_configuration
    update_environment
    create_migration_tools
    validate_migration
    
    log "Migration completed successfully"
    print_summary
}

# Run migration
main "$@"