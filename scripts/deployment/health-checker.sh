#!/bin/bash

# Health Checker Script for Cross-Architecture Deployment
# Implements multi-endpoint health verification with retry logic and rollback triggers
# Requirements: 2.3, 2.4, 4.3, 4.4

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="${SCRIPT_DIR}/health-check.log"
CONFIG_FILE="${SCRIPT_DIR}/health-check.config"

# Default configuration
DEFAULT_PRIMARY_ENDPOINT="/api/healthchecker"
DEFAULT_FALLBACK_ENDPOINT="/health"
DEFAULT_TIMEOUT=10
DEFAULT_RETRIES=3
DEFAULT_ROLLBACK_THRESHOLD=3
DEFAULT_BASE_DELAY=5
DEFAULT_MAX_DELAY=80
DEFAULT_MULTIPLIER=2

# Load configuration
load_config() {
    if [[ -f "$CONFIG_FILE" ]]; then
        source "$CONFIG_FILE"
    fi
    
    # Set defaults if not configured
    PRIMARY_ENDPOINT="${PRIMARY_ENDPOINT:-$DEFAULT_PRIMARY_ENDPOINT}"
    FALLBACK_ENDPOINT="${FALLBACK_ENDPOINT:-$DEFAULT_FALLBACK_ENDPOINT}"
    HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-$DEFAULT_TIMEOUT}"
    HEALTH_RETRIES="${HEALTH_RETRIES:-$DEFAULT_RETRIES}"
    ROLLBACK_THRESHOLD="${ROLLBACK_THRESHOLD:-$DEFAULT_ROLLBACK_THRESHOLD}"
    BASE_DELAY="${BASE_DELAY:-$DEFAULT_BASE_DELAY}"
    MAX_DELAY="${MAX_DELAY:-$DEFAULT_MAX_DELAY}"
    MULTIPLIER="${MULTIPLIER:-$DEFAULT_MULTIPLIER}"
}

# Logging function
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

# Exponential backoff delay calculation
calculate_delay() {
    local attempt="$1"
    local delay=$((BASE_DELAY * (MULTIPLIER ** (attempt - 1))))
    if [[ $delay -gt $MAX_DELAY ]]; then
        delay=$MAX_DELAY
    fi
    # Add jitter (random 0-20% of delay)
    local jitter=$((delay * RANDOM / 32768 / 5))
    echo $((delay + jitter))
}

# Health check function for a single endpoint
check_endpoint() {
    local url="$1"
    local timeout="$2"
    local attempt="$3"
    
    log "INFO" "Health check attempt $attempt for endpoint: $url"
    
    local response_code
    local response_time
    local start_time=$(date +%s.%N)
    
    if response_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time "$timeout" --connect-timeout 5 "$url" 2>/dev/null); then
        local end_time=$(date +%s.%N)
        response_time=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0")
        
        if [[ "$response_code" =~ ^2[0-9][0-9]$ ]]; then
            log "INFO" "Health check SUCCESS: $url returned $response_code (${response_time}s)"
            return 0
        else
            log "WARN" "Health check FAILED: $url returned $response_code (${response_time}s)"
            return 1
        fi
    else
        log "ERROR" "Health check FAILED: $url - connection timeout or error"
        return 1
    fi
}

# Multi-endpoint health verification with retry logic
verify_health() {
    local host="$1"
    local port="$2"
    local max_attempts="${3:-$HEALTH_RETRIES}"
    
    log "INFO" "Starting health verification for $host:$port"
    log "INFO" "Configuration: timeout=${HEALTH_TIMEOUT}s, retries=$max_attempts, rollback_threshold=$ROLLBACK_THRESHOLD"
    
    local primary_url="http://$host:$port$PRIMARY_ENDPOINT"
    local fallback_url="http://$host:$port$FALLBACK_ENDPOINT"
    
    local consecutive_failures=0
    local total_attempts=0
    
    for attempt in $(seq 1 "$max_attempts"); do
        total_attempts=$((total_attempts + 1))
        
        # Try primary endpoint first
        if check_endpoint "$primary_url" "$HEALTH_TIMEOUT" "$attempt"; then
            log "INFO" "Health verification PASSED on primary endpoint (attempt $attempt/$max_attempts)"
            return 0
        fi
        
        # Try fallback endpoint if primary fails
        log "INFO" "Primary endpoint failed, trying fallback endpoint"
        if check_endpoint "$fallback_url" "$HEALTH_TIMEOUT" "$attempt"; then
            log "INFO" "Health verification PASSED on fallback endpoint (attempt $attempt/$max_attempts)"
            return 0
        fi
        
        consecutive_failures=$((consecutive_failures + 1))
        log "WARN" "Both endpoints failed (consecutive failures: $consecutive_failures)"
        
        # Check if we should trigger rollback
        if [[ $consecutive_failures -ge $ROLLBACK_THRESHOLD ]]; then
            log "ERROR" "Rollback threshold reached ($consecutive_failures >= $ROLLBACK_THRESHOLD)"
            return 2  # Special exit code for rollback trigger
        fi
        
        # Calculate delay for next attempt (if not last attempt)
        if [[ $attempt -lt $max_attempts ]]; then
            local delay=$(calculate_delay "$attempt")
            log "INFO" "Waiting ${delay}s before next attempt..."
            sleep "$delay"
        fi
    done
    
    log "ERROR" "Health verification FAILED after $total_attempts attempts"
    return 1
}

# Deployment status verification
verify_deployment_status() {
    local host="$1"
    local port="$2"
    local container_name="${3:-rustci}"
    
    log "INFO" "Verifying deployment status for container: $container_name"
    
    # Check if container is running
    if ! docker ps --format "table {{.Names}}\t{{.Status}}" | grep -q "^$container_name"; then
        log "ERROR" "Container $container_name is not running"
        return 1
    fi
    
    # Get container info
    local container_info=$(docker inspect "$container_name" --format '{{.State.Status}},{{.State.Health.Status}},{{.Config.Image}}' 2>/dev/null || echo "unknown,unknown,unknown")
    IFS=',' read -r container_status health_status image <<< "$container_info"
    
    log "INFO" "Container status: $container_status, Health: $health_status, Image: $image"
    
    # Verify container is running
    if [[ "$container_status" != "running" ]]; then
        log "ERROR" "Container is not in running state: $container_status"
        return 1
    fi
    
    # Check container logs for errors (last 50 lines)
    log "INFO" "Checking container logs for errors..."
    local error_count=$(docker logs --tail 50 "$container_name" 2>&1 | grep -i "error\|fatal\|panic" | wc -l)
    if [[ $error_count -gt 0 ]]; then
        log "WARN" "Found $error_count error messages in recent logs"
        docker logs --tail 10 "$container_name" 2>&1 | while read -r line; do
            log "WARN" "Container log: $line"
        done
    fi
    
    # Verify network connectivity
    log "INFO" "Verifying network connectivity to $host:$port"
    if ! nc -z "$host" "$port" 2>/dev/null; then
        log "ERROR" "Cannot connect to $host:$port"
        return 1
    fi
    
    log "INFO" "Deployment status verification completed successfully"
    return 0
}

# Rollback trigger function
trigger_rollback() {
    local reason="$1"
    local rollback_script="${SCRIPT_DIR}/rollback.sh"
    
    log "ERROR" "Triggering rollback due to: $reason"
    
    if [[ ! -f "$rollback_script" ]]; then
        log "ERROR" "Rollback script not found: $rollback_script"
        return 1
    fi
    
    if [[ ! -x "$rollback_script" ]]; then
        log "ERROR" "Rollback script is not executable: $rollback_script"
        return 1
    fi
    
    log "INFO" "Executing rollback script: $rollback_script"
    if "$rollback_script"; then
        log "INFO" "Rollback completed successfully"
        return 0
    else
        log "ERROR" "Rollback failed with exit code $?"
        return 1
    fi
}

# Main health monitoring function
monitor_deployment() {
    local host="$1"
    local port="$2"
    local container_name="${3:-rustci}"
    local continuous="${4:-false}"
    
    log "INFO" "Starting deployment monitoring for $host:$port (container: $container_name)"
    
    # Initial deployment status verification
    if ! verify_deployment_status "$host" "$port" "$container_name"; then
        log "ERROR" "Initial deployment status verification failed"
        return 1
    fi
    
    # Health verification with rollback trigger
    local health_result
    verify_health "$host" "$port"
    health_result=$?
    
    case $health_result in
        0)
            log "INFO" "Health verification passed - deployment is healthy"
            ;;
        2)
            log "ERROR" "Health verification failed - triggering rollback"
            if trigger_rollback "Health check failures exceeded threshold"; then
                log "INFO" "Rollback completed, re-verifying health..."
                verify_health "$host" "$port" 1  # Single attempt after rollback
                return $?
            else
                log "ERROR" "Rollback failed - manual intervention required"
                return 1
            fi
            ;;
        *)
            log "ERROR" "Health verification failed without triggering rollback"
            return 1
            ;;
    esac
    
    # Continuous monitoring if requested
    if [[ "$continuous" == "true" ]]; then
        log "INFO" "Starting continuous monitoring (Ctrl+C to stop)"
        local monitor_interval=30
        while true; do
            sleep "$monitor_interval"
            log "INFO" "Continuous health check..."
            if ! verify_health "$host" "$port" 1; then
                log "WARN" "Continuous health check failed"
            fi
        done
    fi
    
    return 0
}

# Generate health check report
generate_report() {
    local host="$1"
    local port="$2"
    local output_file="${3:-health-report.txt}"
    
    log "INFO" "Generating health check report: $output_file"
    
    {
        echo "=== RustCI Health Check Report ==="
        echo "Generated: $(date)"
        echo "Target: $host:$port"
        echo ""
        
        echo "=== Configuration ==="
        echo "Primary Endpoint: $PRIMARY_ENDPOINT"
        echo "Fallback Endpoint: $FALLBACK_ENDPOINT"
        echo "Timeout: ${HEALTH_TIMEOUT}s"
        echo "Retries: $HEALTH_RETRIES"
        echo "Rollback Threshold: $ROLLBACK_THRESHOLD"
        echo ""
        
        echo "=== Health Check Results ==="
        verify_health "$host" "$port" 1 2>&1
        echo ""
        
        echo "=== Deployment Status ==="
        verify_deployment_status "$host" "$port" 2>&1
        echo ""
        
        echo "=== Recent Logs ==="
        if [[ -f "$LOG_FILE" ]]; then
            tail -20 "$LOG_FILE"
        else
            echo "No log file found"
        fi
    } > "$output_file"
    
    log "INFO" "Report generated: $output_file"
}

# Usage function
usage() {
    cat << EOF
Usage: $0 <command> [options]

Commands:
    check <host> <port>                 - Single health check
    monitor <host> <port> [container]   - Monitor deployment with rollback
    continuous <host> <port>            - Continuous monitoring
    report <host> <port> [output_file]  - Generate health report
    config                              - Show current configuration

Options:
    -t, --timeout <seconds>     Health check timeout (default: $DEFAULT_TIMEOUT)
    -r, --retries <count>       Number of retries (default: $DEFAULT_RETRIES)
    -T, --threshold <count>     Rollback threshold (default: $DEFAULT_ROLLBACK_THRESHOLD)
    -h, --help                  Show this help

Examples:
    $0 check localhost 8080
    $0 monitor 46.37.122.118 8080 rustci
    $0 continuous localhost 8080
    $0 report localhost 8080 health-report.txt

Configuration file: $CONFIG_FILE
Log file: $LOG_FILE
EOF
}

# Parse command line arguments
parse_args() {
    local args=()
    while [[ $# -gt 0 ]]; do
        case $1 in
            -t|--timeout)
                HEALTH_TIMEOUT="$2"
                shift 2
                ;;
            -r|--retries)
                HEALTH_RETRIES="$2"
                shift 2
                ;;
            -T|--threshold)
                ROLLBACK_THRESHOLD="$2"
                shift 2
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                args+=("$1")
                shift
                ;;
        esac
    done
    set -- "${args[@]}"
}

# Main function
main() {
    # Initialize
    load_config
    parse_args "$@"
    
    # Ensure log file exists
    touch "$LOG_FILE"
    
    # Get command
    local command="${1:-}"
    if [[ -z "$command" ]]; then
        usage
        exit 1
    fi
    
    case "$command" in
        check)
            if [[ $# -lt 3 ]]; then
                echo "Error: check command requires host and port"
                usage
                exit 1
            fi
            verify_health "$2" "$3"
            ;;
        monitor)
            if [[ $# -lt 3 ]]; then
                echo "Error: monitor command requires host and port"
                usage
                exit 1
            fi
            monitor_deployment "$2" "$3" "${4:-rustci}"
            ;;
        continuous)
            if [[ $# -lt 3 ]]; then
                echo "Error: continuous command requires host and port"
                usage
                exit 1
            fi
            monitor_deployment "$2" "$3" "rustci" "true"
            ;;
        report)
            if [[ $# -lt 3 ]]; then
                echo "Error: report command requires host and port"
                usage
                exit 1
            fi
            generate_report "$2" "$3" "${4:-health-report.txt}"
            ;;
        config)
            echo "Current Configuration:"
            echo "  Primary Endpoint: $PRIMARY_ENDPOINT"
            echo "  Fallback Endpoint: $FALLBACK_ENDPOINT"
            echo "  Timeout: ${HEALTH_TIMEOUT}s"
            echo "  Retries: $HEALTH_RETRIES"
            echo "  Rollback Threshold: $ROLLBACK_THRESHOLD"
            echo "  Base Delay: ${BASE_DELAY}s"
            echo "  Max Delay: ${MAX_DELAY}s"
            echo "  Multiplier: $MULTIPLIER"
            echo "  Config File: $CONFIG_FILE"
            echo "  Log File: $LOG_FILE"
            ;;
        *)
            echo "Error: Unknown command '$command'"
            usage
            exit 1
            ;;
    esac
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi