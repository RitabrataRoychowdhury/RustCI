#!/bin/bash

# Pipeline Health Integration Script
# Demonstrates how to integrate health checking system into the cross-architecture deployment pipeline
# Requirements: 2.3, 2.4, 4.3, 4.4

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HEALTH_CHECKER="${SCRIPT_DIR}/health-checker.sh"
DEPLOYMENT_MONITOR="${SCRIPT_DIR}/deployment-monitor.sh"
ROLLBACK_SCRIPT="${SCRIPT_DIR}/rollback.sh"

# Configuration from environment or defaults
VPS_IP="${VPS_IP:-46.37.122.118}"
VPS_PORT="${VPS_PORT:-8080}"
CONTAINER_NAME="${CONTAINER_NAME:-rustci}"
TESTING_MODE="${TESTING_MODE:-true}"

# Logging
LOG_FILE="${SCRIPT_DIR}/pipeline-health.log"

log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

# Pipeline stage: Post-deployment health verification
pipeline_post_deployment_health_check() {
    log "INFO" "=== Pipeline Stage: Post-Deployment Health Check ==="
    
    # Wait for deployment to stabilize
    log "INFO" "Waiting for deployment to stabilize..."
    sleep 30
    
    # Run comprehensive health check
    log "INFO" "Running comprehensive health verification..."
    if "$HEALTH_CHECKER" monitor "$VPS_IP" "$VPS_PORT" "$CONTAINER_NAME"; then
        log "INFO" "✓ Post-deployment health check PASSED"
        return 0
    else
        log "ERROR" "✗ Post-deployment health check FAILED"
        return 1
    fi
}

# Pipeline stage: Smoke test with health monitoring
pipeline_smoke_test() {
    log "INFO" "=== Pipeline Stage: Smoke Test ==="
    
    # Verify deployment readiness
    log "INFO" "Verifying deployment readiness..."
    if "$DEPLOYMENT_MONITOR" readiness "$VPS_IP" "$VPS_PORT" "$CONTAINER_NAME"; then
        log "INFO" "✓ Deployment readiness check PASSED"
    else
        log "ERROR" "✗ Deployment readiness check FAILED"
        return 1
    fi
    
    # Run smoke test monitoring for 2 minutes
    log "INFO" "Running smoke test monitoring (120 seconds)..."
    if "$DEPLOYMENT_MONITOR" monitor "$VPS_IP" "$VPS_PORT" "$CONTAINER_NAME" 120; then
        log "INFO" "✓ Smoke test monitoring PASSED"
        return 0
    else
        log "ERROR" "✗ Smoke test monitoring FAILED"
        return 1
    fi
}

# Pipeline stage: Health check with automatic rollback
pipeline_health_check_with_rollback() {
    log "INFO" "=== Pipeline Stage: Health Check with Rollback ==="
    
    # Configure health checker for rollback trigger
    export ROLLBACK_THRESHOLD=3
    export HEALTH_RETRIES=5
    
    log "INFO" "Running health check with rollback capability..."
    local health_result
    "$HEALTH_CHECKER" monitor "$VPS_IP" "$VPS_PORT" "$CONTAINER_NAME"
    health_result=$?
    
    case $health_result in
        0)
            log "INFO" "✓ Health check PASSED - deployment is stable"
            return 0
            ;;
        2)
            log "WARN" "Health check triggered rollback threshold"
            log "INFO" "Initiating automatic rollback..."
            
            if "$ROLLBACK_SCRIPT" rollback; then
                log "INFO" "✓ Automatic rollback completed successfully"
                
                # Verify rollback health
                log "INFO" "Verifying rollback health..."
                if "$HEALTH_CHECKER" check "$VPS_IP" "$VPS_PORT"; then
                    log "INFO" "✓ Rollback health verification PASSED"
                    return 0
                else
                    log "ERROR" "✗ Rollback health verification FAILED"
                    return 1
                fi
            else
                log "ERROR" "✗ Automatic rollback FAILED"
                return 1
            fi
            ;;
        *)
            log "ERROR" "✗ Health check FAILED without triggering rollback"
            return 1
            ;;
    esac
}

# Pipeline stage: Generate deployment report
pipeline_generate_reports() {
    log "INFO" "=== Pipeline Stage: Generate Reports ==="
    
    local timestamp=$(date '+%Y%m%d_%H%M%S')
    local health_report="health-report-${timestamp}.txt"
    local deployment_report="deployment-report-${timestamp}.txt"
    
    # Generate health report
    log "INFO" "Generating health report..."
    if "$HEALTH_CHECKER" report "$VPS_IP" "$VPS_PORT" "$health_report"; then
        log "INFO" "✓ Health report generated: $health_report"
    else
        log "WARN" "Health report generation failed"
    fi
    
    # Generate deployment status report
    log "INFO" "Generating deployment status report..."
    if "$DEPLOYMENT_MONITOR" report "$VPS_IP" "$VPS_PORT" "$CONTAINER_NAME" "$deployment_report"; then
        log "INFO" "✓ Deployment report generated: $deployment_report"
    else
        log "WARN" "Deployment report generation failed"
    fi
    
    # Output verification URLs
    log "INFO" "=== Verification URLs ==="
    log "INFO" "Primary health endpoint: http://${VPS_IP}:${VPS_PORT}/api/healthchecker"
    log "INFO" "Fallback health endpoint: http://${VPS_IP}:${VPS_PORT}/health"
    log "INFO" "Manual verification commands:"
    log "INFO" "  curl -f http://${VPS_IP}:${VPS_PORT}/api/healthchecker"
    log "INFO" "  curl -f http://${VPS_IP}:${VPS_PORT}/health"
    log "INFO" "  docker ps --filter name=${CONTAINER_NAME}"
    log "INFO" "  docker logs ${CONTAINER_NAME}"
    
    return 0
}

# Pipeline stage: Continuous monitoring setup
pipeline_setup_continuous_monitoring() {
    log "INFO" "=== Pipeline Stage: Setup Continuous Monitoring ==="
    
    # Create monitoring service script
    local monitor_service="${SCRIPT_DIR}/continuous-monitor.sh"
    
    cat > "$monitor_service" << 'EOF'
#!/bin/bash
# Continuous monitoring service for deployed RustCI

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HEALTH_CHECKER="${SCRIPT_DIR}/health-checker.sh"

VPS_IP="${VPS_IP:-46.37.122.118}"
VPS_PORT="${VPS_PORT:-8080}"

# Run continuous monitoring
exec "$HEALTH_CHECKER" continuous "$VPS_IP" "$VPS_PORT"
EOF
    
    chmod +x "$monitor_service"
    
    log "INFO" "✓ Continuous monitoring service created: $monitor_service"
    log "INFO" "To start continuous monitoring, run: $monitor_service"
    
    return 0
}

# Main pipeline integration function
run_pipeline_health_integration() {
    log "INFO" "Starting pipeline health integration"
    log "INFO" "Target: $VPS_IP:$VPS_PORT"
    log "INFO" "Container: $CONTAINER_NAME"
    log "INFO" "Testing mode: $TESTING_MODE"
    
    # Ensure all scripts are available and executable
    for script in "$HEALTH_CHECKER" "$DEPLOYMENT_MONITOR" "$ROLLBACK_SCRIPT"; do
        if [[ ! -f "$script" ]]; then
            log "ERROR" "Required script not found: $script"
            return 1
        fi
        
        if [[ ! -x "$script" ]]; then
            chmod +x "$script"
        fi
    done
    
    # Run pipeline stages
    local stage_results=()
    
    # Stage 1: Post-deployment health check
    if pipeline_post_deployment_health_check; then
        stage_results+=("post-deployment:PASS")
    else
        stage_results+=("post-deployment:FAIL")
        log "ERROR" "Post-deployment health check failed - stopping pipeline"
        return 1
    fi
    
    # Stage 2: Smoke test
    if pipeline_smoke_test; then
        stage_results+=("smoke-test:PASS")
    else
        stage_results+=("smoke-test:FAIL")
        log "ERROR" "Smoke test failed - stopping pipeline"
        return 1
    fi
    
    # Stage 3: Health check with rollback capability
    if pipeline_health_check_with_rollback; then
        stage_results+=("health-rollback:PASS")
    else
        stage_results+=("health-rollback:FAIL")
        log "ERROR" "Health check with rollback failed"
        return 1
    fi
    
    # Stage 4: Generate reports (always run)
    if pipeline_generate_reports; then
        stage_results+=("reports:PASS")
    else
        stage_results+=("reports:FAIL")
    fi
    
    # Stage 5: Setup continuous monitoring (always run)
    if pipeline_setup_continuous_monitoring; then
        stage_results+=("monitoring:PASS")
    else
        stage_results+=("monitoring:FAIL")
    fi
    
    # Summary
    log "INFO" "=== Pipeline Health Integration Summary ==="
    for result in "${stage_results[@]}"; do
        log "INFO" "  $result"
    done
    
    # Check if all critical stages passed
    local critical_failures=0
    for result in "${stage_results[@]}"; do
        if [[ "$result" =~ (post-deployment|smoke-test|health-rollback):FAIL ]]; then
            critical_failures=$((critical_failures + 1))
        fi
    done
    
    if [[ $critical_failures -eq 0 ]]; then
        log "INFO" "✓ Pipeline health integration completed successfully"
        return 0
    else
        log "ERROR" "✗ Pipeline health integration failed ($critical_failures critical failures)"
        return 1
    fi
}

# Validate pipeline configuration
validate_pipeline_config() {
    log "INFO" "Validating pipeline configuration..."
    
    # Check required environment variables
    local required_vars=("VPS_IP" "VPS_PORT")
    for var in "${required_vars[@]}"; do
        if [[ -z "${!var:-}" ]]; then
            log "ERROR" "Required environment variable not set: $var"
            return 1
        fi
    done
    
    # Check network connectivity
    log "INFO" "Testing network connectivity to $VPS_IP:$VPS_PORT..."
    if nc -z "$VPS_IP" "$VPS_PORT" 2>/dev/null; then
        log "INFO" "✓ Network connectivity test passed"
    else
        log "WARN" "Network connectivity test failed - deployment may not be ready"
    fi
    
    # Check Docker availability
    if ! docker info >/dev/null 2>&1; then
        log "ERROR" "Docker is not available"
        return 1
    fi
    
    log "INFO" "✓ Pipeline configuration validation completed"
    return 0
}

# Usage function
usage() {
    cat << EOF
Pipeline Health Integration Script

Usage: $0 [COMMAND] [OPTIONS]

COMMANDS:
    run                Run full pipeline health integration (default)
    validate           Validate pipeline configuration only
    post-deploy        Run post-deployment health check only
    smoke-test         Run smoke test only
    health-rollback    Run health check with rollback only
    reports            Generate reports only
    monitoring         Setup continuous monitoring only

OPTIONS:
    --vps-ip IP        VPS IP address (default: $VPS_IP)
    --vps-port PORT    VPS port (default: $VPS_PORT)
    --container NAME   Container name (default: $CONTAINER_NAME)
    --testing-mode     Enable testing mode (default: $TESTING_MODE)
    -h, --help         Show this help

EXAMPLES:
    $0                                    # Run full integration
    $0 run                               # Run full integration
    $0 validate                          # Validate configuration
    $0 post-deploy                       # Post-deployment check only
    $0 --vps-ip 192.168.1.100 run       # Custom VPS IP

ENVIRONMENT VARIABLES:
    VPS_IP             Target VPS IP address
    VPS_PORT           Target VPS port
    CONTAINER_NAME     Container name
    TESTING_MODE       Enable testing mode (true/false)

LOG FILE: $LOG_FILE
EOF
}

# Parse arguments
COMMAND="run"

while [[ $# -gt 0 ]]; do
    case $1 in
        --vps-ip)
            VPS_IP="$2"
            shift 2
            ;;
        --vps-port)
            VPS_PORT="$2"
            shift 2
            ;;
        --container)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        --testing-mode)
            TESTING_MODE="true"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        run|validate|post-deploy|smoke-test|health-rollback|reports|monitoring)
            COMMAND="$1"
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            usage
            exit 1
            ;;
    esac
done

# Main execution
case "$COMMAND" in
    run)
        validate_pipeline_config && run_pipeline_health_integration
        ;;
    validate)
        validate_pipeline_config
        ;;
    post-deploy)
        validate_pipeline_config && pipeline_post_deployment_health_check
        ;;
    smoke-test)
        validate_pipeline_config && pipeline_smoke_test
        ;;
    health-rollback)
        validate_pipeline_config && pipeline_health_check_with_rollback
        ;;
    reports)
        pipeline_generate_reports
        ;;
    monitoring)
        pipeline_setup_continuous_monitoring
        ;;
    *)
        echo "Unknown command: $COMMAND"
        usage
        exit 1
        ;;
esac