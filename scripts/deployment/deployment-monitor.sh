#!/bin/bash

# Deployment Status Monitoring Script
# Provides comprehensive deployment monitoring with logging and status verification
# Requirements: 2.3, 2.4, 4.3, 4.4

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HEALTH_CHECKER="${SCRIPT_DIR}/health-checker.sh"
LOG_FILE="${SCRIPT_DIR}/deployment-monitor.log"

# Configuration
MONITOR_INTERVAL=30
MAX_LOG_SIZE=10485760  # 10MB
BACKUP_LOG_COUNT=5

# Logging function with rotation
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    # Rotate log if too large
    if [[ -f "$LOG_FILE" ]] && [[ $(stat -f%z "$LOG_FILE" 2>/dev/null || stat -c%s "$LOG_FILE" 2>/dev/null || echo 0) -gt $MAX_LOG_SIZE ]]; then
        rotate_logs
    fi
    
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

# Log rotation function
rotate_logs() {
    if [[ -f "$LOG_FILE" ]]; then
        for i in $(seq $((BACKUP_LOG_COUNT - 1)) -1 1); do
            if [[ -f "${LOG_FILE}.$i" ]]; then
                mv "${LOG_FILE}.$i" "${LOG_FILE}.$((i + 1))"
            fi
        done
        mv "$LOG_FILE" "${LOG_FILE}.1"
        touch "$LOG_FILE"
        log "INFO" "Log rotated due to size limit"
    fi
}

# Get deployment metrics
get_deployment_metrics() {
    local container_name="$1"
    
    if ! docker ps --format "{{.Names}}" | grep -q "^$container_name$"; then
        echo "status=not_running"
        return 1
    fi
    
    local stats=$(docker stats --no-stream --format "table {{.CPUPerc}},{{.MemUsage}},{{.NetIO}},{{.BlockIO}}" "$container_name" 2>/dev/null | tail -n 1)
    local uptime=$(docker inspect "$container_name" --format '{{.State.StartedAt}}' 2>/dev/null)
    local image=$(docker inspect "$container_name" --format '{{.Config.Image}}' 2>/dev/null)
    local ports=$(docker port "$container_name" 2>/dev/null | tr '\n' ';' || echo "none")
    
    # Calculate uptime in seconds
    local start_time=$(date -d "$uptime" +%s 2>/dev/null || echo 0)
    local current_time=$(date +%s)
    local uptime_seconds=$((current_time - start_time))
    
    echo "status=running,uptime=${uptime_seconds}s,image=$image,ports=$ports,stats=$stats"
}

# Check system resources
check_system_resources() {
    local host="$1"
    
    log "INFO" "Checking system resources on $host"
    
    # Check disk space
    local disk_usage=$(ssh -o ConnectTimeout=10 "$host" "df -h / | tail -1 | awk '{print \$5}'" 2>/dev/null || echo "unknown")
    log "INFO" "Disk usage: $disk_usage"
    
    # Check memory usage
    local memory_usage=$(ssh -o ConnectTimeout=10 "$host" "free -m | grep '^Mem:' | awk '{printf \"%.1f%%\", \$3/\$2 * 100.0}'" 2>/dev/null || echo "unknown")
    log "INFO" "Memory usage: $memory_usage"
    
    # Check load average
    local load_avg=$(ssh -o ConnectTimeout=10 "$host" "uptime | awk -F'load average:' '{print \$2}'" 2>/dev/null || echo "unknown")
    log "INFO" "Load average: $load_avg"
    
    # Check Docker daemon status
    local docker_status=$(ssh -o ConnectTimeout=10 "$host" "systemctl is-active docker" 2>/dev/null || echo "unknown")
    log "INFO" "Docker status: $docker_status"
    
    # Warn if disk usage is high
    if [[ "$disk_usage" =~ ^[0-9]+% ]]; then
        local usage_num=$(echo "$disk_usage" | sed 's/%//')
        if [[ $usage_num -gt 85 ]]; then
            log "WARN" "High disk usage detected: $disk_usage"
        fi
    fi
}

# Monitor deployment status
monitor_deployment_status() {
    local host="$1"
    local port="$2"
    local container_name="${3:-rustci}"
    local duration="${4:-300}"  # 5 minutes default
    
    log "INFO" "Starting deployment status monitoring for $duration seconds"
    log "INFO" "Target: $host:$port, Container: $container_name"
    
    local start_time=$(date +%s)
    local end_time=$((start_time + duration))
    local check_count=0
    local success_count=0
    local failure_count=0
    
    while [[ $(date +%s) -lt $end_time ]]; do
        check_count=$((check_count + 1))
        log "INFO" "Status check #$check_count"
        
        # Get deployment metrics
        local metrics=$(get_deployment_metrics "$container_name")
        log "INFO" "Container metrics: $metrics"
        
        # Check system resources periodically
        if [[ $((check_count % 5)) -eq 0 ]]; then
            check_system_resources "$host"
        fi
        
        # Run health check
        if "$HEALTH_CHECKER" check "$host" "$port" >/dev/null 2>&1; then
            success_count=$((success_count + 1))
            log "INFO" "Health check PASSED ($success_count/$check_count)"
        else
            failure_count=$((failure_count + 1))
            log "WARN" "Health check FAILED ($failure_count/$check_count)"
            
            # Get container logs on failure
            log "INFO" "Retrieving container logs due to health check failure"
            if docker ps --format "{{.Names}}" | grep -q "^$container_name$"; then
                docker logs --tail 10 "$container_name" 2>&1 | while read -r line; do
                    log "WARN" "Container log: $line"
                done
            fi
        fi
        
        # Calculate success rate
        local success_rate=$((success_count * 100 / check_count))
        log "INFO" "Current success rate: ${success_rate}% ($success_count/$check_count)"
        
        sleep "$MONITOR_INTERVAL"
    done
    
    # Final report
    local total_time=$(($(date +%s) - start_time))
    log "INFO" "Monitoring completed after ${total_time}s"
    log "INFO" "Final statistics: $success_count successes, $failure_count failures out of $check_count checks"
    log "INFO" "Final success rate: $((success_count * 100 / check_count))%"
    
    # Return success if success rate is above 80%
    if [[ $((success_count * 100 / check_count)) -ge 80 ]]; then
        return 0
    else
        return 1
    fi
}

# Generate deployment status report
generate_status_report() {
    local host="$1"
    local port="$2"
    local container_name="${3:-rustci}"
    local output_file="${4:-deployment-status-report.txt}"
    
    log "INFO" "Generating deployment status report: $output_file"
    
    {
        echo "=== RustCI Deployment Status Report ==="
        echo "Generated: $(date)"
        echo "Target: $host:$port"
        echo "Container: $container_name"
        echo ""
        
        echo "=== Container Status ==="
        if docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" | grep "^$container_name"; then
            echo "Container is running"
            echo ""
            echo "Container Details:"
            docker inspect "$container_name" --format '
Image: {{.Config.Image}}
Started: {{.State.StartedAt}}
Status: {{.State.Status}}
Health: {{.State.Health.Status}}
Restart Count: {{.RestartCount}}
Platform: {{.Platform}}' 2>/dev/null || echo "Could not retrieve container details"
        else
            echo "Container is not running"
        fi
        echo ""
        
        echo "=== System Resources ==="
        ssh -o ConnectTimeout=10 "$host" "
            echo 'Disk Usage:'
            df -h /
            echo ''
            echo 'Memory Usage:'
            free -h
            echo ''
            echo 'Load Average:'
            uptime
            echo ''
            echo 'Docker Status:'
            systemctl status docker --no-pager -l
        " 2>/dev/null || echo "Could not retrieve system resources"
        echo ""
        
        echo "=== Health Check Results ==="
        "$HEALTH_CHECKER" check "$host" "$port" 2>&1 || echo "Health check failed"
        echo ""
        
        echo "=== Recent Container Logs ==="
        if docker ps --format "{{.Names}}" | grep -q "^$container_name$"; then
            docker logs --tail 20 "$container_name" 2>&1
        else
            echo "Container not running - no logs available"
        fi
        echo ""
        
        echo "=== Network Connectivity ==="
        echo "Testing connectivity to $host:$port..."
        if nc -z "$host" "$port" 2>/dev/null || timeout 5 bash -c "</dev/tcp/$host/$port" 2>/dev/null; then
            echo "✓ Port $port is accessible"
        else
            echo "✗ Port $port is not accessible"
        fi
        echo ""
        
        echo "=== Recent Monitor Logs ==="
        if [[ -f "$LOG_FILE" ]]; then
            tail -30 "$LOG_FILE"
        else
            echo "No monitor log file found"
        fi
        
    } > "$output_file"
    
    log "INFO" "Status report generated: $output_file"
}

# Verify deployment readiness
verify_deployment_readiness() {
    local host="$1"
    local port="$2"
    local container_name="${3:-rustci}"
    
    log "INFO" "Verifying deployment readiness"
    
    # Check 1: Container is running
    if ! docker ps --format "{{.Names}}" | grep -q "^$container_name$"; then
        log "ERROR" "Readiness check failed: Container $container_name is not running"
        return 1
    fi
    
    # Check 2: Container health status
    local health_status=$(docker inspect "$container_name" --format '{{.State.Health.Status}}' 2>/dev/null || echo "none")
    if [[ "$health_status" == "unhealthy" ]]; then
        log "ERROR" "Readiness check failed: Container health status is unhealthy"
        return 1
    fi
    
    # Check 3: Port accessibility
    if ! nc -z "$host" "$port" 2>/dev/null && ! timeout 5 bash -c "</dev/tcp/$host/$port" 2>/dev/null; then
        log "ERROR" "Readiness check failed: Port $port is not accessible on $host"
        return 1
    fi
    
    # Check 4: Health endpoints respond
    if ! "$HEALTH_CHECKER" check "$host" "$port" >/dev/null 2>&1; then
        log "ERROR" "Readiness check failed: Health endpoints are not responding"
        return 1
    fi
    
    # Check 5: No recent error logs
    local error_count=$(docker logs --tail 20 "$container_name" 2>&1 | grep -i "error\|fatal\|panic" | wc -l)
    if [[ $error_count -gt 3 ]]; then
        log "WARN" "Readiness check warning: Found $error_count recent error messages in logs"
    fi
    
    log "INFO" "Deployment readiness verification PASSED"
    return 0
}

# Usage function
usage() {
    cat << EOF
Usage: $0 <command> [options]

Commands:
    monitor <host> <port> [container] [duration]  - Monitor deployment status
    status <host> <port> [container]              - Check current deployment status
    report <host> <port> [container] [output]     - Generate status report
    readiness <host> <port> [container]           - Verify deployment readiness
    metrics <container>                           - Show container metrics
    resources <host>                              - Check system resources

Examples:
    $0 monitor localhost 8080 rustci 300
    $0 status 46.37.122.118 8080
    $0 report localhost 8080 rustci status.txt
    $0 readiness localhost 8080
    $0 metrics rustci
    $0 resources localhost

Log file: $LOG_FILE
EOF
}

# Main function
main() {
    # Ensure health checker exists and is executable
    if [[ ! -f "$HEALTH_CHECKER" ]]; then
        echo "Error: Health checker script not found: $HEALTH_CHECKER"
        exit 1
    fi
    
    if [[ ! -x "$HEALTH_CHECKER" ]]; then
        chmod +x "$HEALTH_CHECKER"
    fi
    
    # Ensure log file exists
    touch "$LOG_FILE"
    
    local command="${1:-}"
    if [[ -z "$command" ]]; then
        usage
        exit 1
    fi
    
    case "$command" in
        monitor)
            if [[ $# -lt 3 ]]; then
                echo "Error: monitor command requires host and port"
                usage
                exit 1
            fi
            monitor_deployment_status "$2" "$3" "${4:-rustci}" "${5:-300}"
            ;;
        status)
            if [[ $# -lt 3 ]]; then
                echo "Error: status command requires host and port"
                usage
                exit 1
            fi
            verify_deployment_readiness "$2" "$3" "${4:-rustci}"
            ;;
        report)
            if [[ $# -lt 3 ]]; then
                echo "Error: report command requires host and port"
                usage
                exit 1
            fi
            generate_status_report "$2" "$3" "${4:-rustci}" "${5:-deployment-status-report.txt}"
            ;;
        readiness)
            if [[ $# -lt 3 ]]; then
                echo "Error: readiness command requires host and port"
                usage
                exit 1
            fi
            verify_deployment_readiness "$2" "$3" "${4:-rustci}"
            ;;
        metrics)
            if [[ $# -lt 2 ]]; then
                echo "Error: metrics command requires container name"
                usage
                exit 1
            fi
            get_deployment_metrics "$2"
            ;;
        resources)
            if [[ $# -lt 2 ]]; then
                echo "Error: resources command requires host"
                usage
                exit 1
            fi
            check_system_resources "$2"
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