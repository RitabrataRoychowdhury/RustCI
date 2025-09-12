#!/bin/bash

# Deployment Verification Script
# Provides comprehensive verification commands and health check URLs
# Requirements: 4.4, 5.1, 5.2, 5.4

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Default configuration
VPS_IP=""
VPS_USERNAME=""
VPS_PASSWORD=""
CONTAINER_NAME="rustci-production"
CONTAINER_PORT=8080
HEALTH_TIMEOUT=10
HEALTH_RETRIES=3
VERBOSE=false
OUTPUT_FORMAT="text"  # text, json, markdown

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

# Function to show usage
show_usage() {
    cat << EOF
Deployment Verification Script

Usage: $0 [OPTIONS] [COMMAND]

COMMANDS:
    verify             Perform complete deployment verification (default)
    health             Check health endpoints only
    status             Show deployment status
    urls               Show access URLs and verification commands
    test-local         Test local deployment
    test-remote        Test remote deployment
    generate-report    Generate verification report

OPTIONS:
    --vps-ip IP            VPS IP address (default: from .env)
    --vps-user USER        VPS username (default: from .env)
    --vps-password PASS    VPS password (default: from .env)
    -c, --container NAME   Container name (default: rustci-production)
    -p, --port PORT        Container port (default: 8080)
    --timeout SECONDS      Health check timeout (default: 10)
    --retries COUNT        Health check retries (default: 3)
    --format FORMAT        Output format: text, json, markdown (default: text)
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

EXAMPLES:
    $0                                    # Complete verification
    $0 verify                            # Same as above
    $0 health                            # Health checks only
    $0 status                            # Deployment status
    $0 urls                              # Show URLs and commands
    $0 --format json verify              # JSON output
    $0 --format markdown generate-report # Markdown report

VERIFICATION PROCESS:
    1. Check VPS connectivity
    2. Verify container status
    3. Test health endpoints
    4. Validate service functionality
    5. Generate verification report

EOF
}

# Load environment variables
load_environment() {
    if [[ -f "$PROJECT_ROOT/.env" ]]; then
        print_status "Loading environment from .env file..."
        # Export variables from .env file
        set -a
        source "$PROJECT_ROOT/.env"
        set +a
        print_success "Environment loaded"
    else
        print_warning ".env file not found"
    fi
}

# Parse command line arguments
COMMAND="verify"

while [[ $# -gt 0 ]]; do
    case $1 in
        --vps-ip)
            VPS_IP="$2"
            shift 2
            ;;
        --vps-user)
            VPS_USERNAME="$2"
            shift 2
            ;;
        --vps-password)
            VPS_PASSWORD="$2"
            shift 2
            ;;
        -c|--container)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        -p|--port)
            CONTAINER_PORT="$2"
            shift 2
            ;;
        --timeout)
            HEALTH_TIMEOUT="$2"
            shift 2
            ;;
        --retries)
            HEALTH_RETRIES="$2"
            shift 2
            ;;
        --format)
            OUTPUT_FORMAT="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        verify|health|status|urls|test-local|test-remote|generate-report)
            COMMAND="$1"
            shift
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Load environment variables
load_environment

# Use environment variables if not provided via command line
VPS_IP=${VPS_IP:-$VPS_IP}
VPS_USERNAME=${VPS_USERNAME:-$VPS_USERNAME}
VPS_PASSWORD=${VPS_PASSWORD:-$VPS_PASSWORD}

# Enable verbose output if requested
if [[ "$VERBOSE" == "true" ]]; then
    set -x
fi

# Validation function
validate_configuration() {
    local errors=()
    
    if [[ -z "$VPS_IP" ]]; then
        errors+=("VPS_IP is required")
    fi
    
    if [[ -z "$VPS_USERNAME" ]]; then
        errors+=("VPS_USERNAME is required")
    fi
    
    if [[ -z "$VPS_PASSWORD" ]]; then
        errors+=("VPS_PASSWORD is required")
    fi
    
    if [[ ${#errors[@]} -ne 0 ]]; then
        print_error "Configuration validation failed:"
        for error in "${errors[@]}"; do
            echo "  - $error"
        done
        return 1
    fi
    
    return 0
}

# Function to test VPS connectivity
test_vps_connectivity() {
    print_status "Testing VPS connectivity..."
    
    if ! command -v sshpass &> /dev/null; then
        print_error "sshpass not available for password authentication"
        return 1
    fi
    
    if sshpass -p "$VPS_PASSWORD" ssh -o ConnectTimeout=10 -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" "echo 'VPS connection successful'" >/dev/null 2>&1; then
        print_success "VPS connectivity test passed"
        return 0
    else
        print_error "VPS connectivity test failed"
        return 1
    fi
}

# Function to check container status
check_container_status() {
    print_status "Checking container status..."
    
    local container_info
    container_info=$(sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" "docker ps --filter name=$CONTAINER_NAME --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'" 2>/dev/null)
    
    if echo "$container_info" | grep -q "$CONTAINER_NAME"; then
        print_success "Container $CONTAINER_NAME is running"
        if [[ "$VERBOSE" == "true" ]]; then
            echo "$container_info"
        fi
        return 0
    else
        print_error "Container $CONTAINER_NAME is not running"
        
        # Check if container exists but is stopped
        local stopped_container
        stopped_container=$(sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" "docker ps -a --filter name=$CONTAINER_NAME --format '{{.Names}}\t{{.Status}}'" 2>/dev/null)
        
        if echo "$stopped_container" | grep -q "$CONTAINER_NAME"; then
            print_warning "Container exists but is stopped:"
            echo "$stopped_container"
        else
            print_error "Container does not exist"
        fi
        
        return 1
    fi
}

# Function to test health endpoints
test_health_endpoints() {
    print_status "Testing health endpoints..."
    
    local primary_url="http://$VPS_IP:$CONTAINER_PORT/api/healthchecker"
    local fallback_url="http://$VPS_IP:$CONTAINER_PORT/health"
    local primary_passed=false
    local fallback_passed=false
    
    # Test primary endpoint
    print_status "Testing primary endpoint: $primary_url"
    for ((i=1; i<=HEALTH_RETRIES; i++)); do
        if curl -f -s --max-time "$HEALTH_TIMEOUT" "$primary_url" >/dev/null 2>&1; then
            print_success "Primary health endpoint passed (attempt $i/$HEALTH_RETRIES)"
            primary_passed=true
            break
        else
            if [[ $i -lt $HEALTH_RETRIES ]]; then
                print_warning "Primary health endpoint failed (attempt $i/$HEALTH_RETRIES), retrying..."
                sleep 5
            else
                print_error "Primary health endpoint failed after $HEALTH_RETRIES attempts"
            fi
        fi
    done
    
    # Test fallback endpoint
    print_status "Testing fallback endpoint: $fallback_url"
    for ((i=1; i<=HEALTH_RETRIES; i++)); do
        if curl -f -s --max-time "$HEALTH_TIMEOUT" "$fallback_url" >/dev/null 2>&1; then
            print_success "Fallback health endpoint passed (attempt $i/$HEALTH_RETRIES)"
            fallback_passed=true
            break
        else
            if [[ $i -lt $HEALTH_RETRIES ]]; then
                print_warning "Fallback health endpoint failed (attempt $i/$HEALTH_RETRIES), retrying..."
                sleep 5
            else
                print_error "Fallback health endpoint failed after $HEALTH_RETRIES attempts"
            fi
        fi
    done
    
    # Evaluate results
    if [[ "$primary_passed" == "true" ]]; then
        print_success "Health endpoints verification passed (primary endpoint working)"
        return 0
    elif [[ "$fallback_passed" == "true" ]]; then
        print_success "Health endpoints verification passed (fallback endpoint working)"
        return 0
    else
        print_error "Health endpoints verification failed (both endpoints failed)"
        return 1
    fi
}

# Function to get deployment information
get_deployment_info() {
    local info=()
    
    # Get container information
    local container_info
    container_info=$(sshpass -p "$VPS_PASSWORD" ssh -o StrictHostKeyChecking=no "$VPS_USERNAME@$VPS_IP" "docker inspect $CONTAINER_NAME --format='{{.Config.Image}}\t{{.State.Status}}\t{{.State.StartedAt}}\t{{.RestartCount}}'" 2>/dev/null || echo "N/A")
    
    IFS=$'\t' read -r image status started_at restart_count <<< "$container_info"
    
    info+=("Container Name: $CONTAINER_NAME")
    info+=("Image: ${image:-N/A}")
    info+=("Status: ${status:-N/A}")
    info+=("Started At: ${started_at:-N/A}")
    info+=("Restart Count: ${restart_count:-N/A}")
    info+=("VPS: $VPS_USERNAME@$VPS_IP")
    info+=("Port: $CONTAINER_PORT")
    
    printf '%s\n' "${info[@]}"
}

# Function to show access URLs and verification commands
show_urls_and_commands() {
    local format="$1"
    
    case "$format" in
        "json")
            cat << EOF
{
  "deployment": {
    "vps_ip": "$VPS_IP",
    "container_name": "$CONTAINER_NAME",
    "port": $CONTAINER_PORT
  },
  "urls": {
    "application": "http://$VPS_IP:$CONTAINER_PORT",
    "health_primary": "http://$VPS_IP:$CONTAINER_PORT/api/healthchecker",
    "health_fallback": "http://$VPS_IP:$CONTAINER_PORT/health"
  },
  "verification_commands": {
    "health_check": "curl -f http://$VPS_IP:$CONTAINER_PORT/api/healthchecker",
    "container_status": "ssh $VPS_USERNAME@$VPS_IP 'docker ps --filter name=$CONTAINER_NAME'",
    "container_logs": "ssh $VPS_USERNAME@$VPS_IP 'docker logs $CONTAINER_NAME'",
    "container_inspect": "ssh $VPS_USERNAME@$VPS_IP 'docker inspect $CONTAINER_NAME'"
  }
}
EOF
            ;;
        "markdown")
            cat << EOF
# Deployment Verification

## Access Information

- **VPS**: $VPS_USERNAME@$VPS_IP
- **Container**: $CONTAINER_NAME
- **Port**: $CONTAINER_PORT

## Access URLs

- **Application**: http://$VPS_IP:$CONTAINER_PORT
- **Health Check (Primary)**: http://$VPS_IP:$CONTAINER_PORT/api/healthchecker
- **Health Check (Fallback)**: http://$VPS_IP:$CONTAINER_PORT/health

## Verification Commands

### Health Check
\`\`\`bash
curl -f http://$VPS_IP:$CONTAINER_PORT/api/healthchecker
\`\`\`

### Container Status
\`\`\`bash
ssh $VPS_USERNAME@$VPS_IP 'docker ps --filter name=$CONTAINER_NAME'
\`\`\`

### Container Logs
\`\`\`bash
ssh $VPS_USERNAME@$VPS_IP 'docker logs $CONTAINER_NAME'
\`\`\`

### Container Inspection
\`\`\`bash
ssh $VPS_USERNAME@$VPS_IP 'docker inspect $CONTAINER_NAME'
\`\`\`

### Rollback (if needed)
\`\`\`bash
ssh $VPS_USERNAME@$VPS_IP 'docker stop $CONTAINER_NAME && docker rm $CONTAINER_NAME && docker run -d --name $CONTAINER_NAME -p $CONTAINER_PORT:8000 --restart unless-stopped rustci:previous'
\`\`\`
EOF
            ;;
        *)
            echo "🔗 Access URLs:"
            echo "  • Application: http://$VPS_IP:$CONTAINER_PORT"
            echo "  • Health Check (Primary): http://$VPS_IP:$CONTAINER_PORT/api/healthchecker"
            echo "  • Health Check (Fallback): http://$VPS_IP:$CONTAINER_PORT/health"
            echo ""
            echo "🔧 Verification Commands:"
            echo "  • Health Check:"
            echo "    curl -f http://$VPS_IP:$CONTAINER_PORT/api/healthchecker"
            echo ""
            echo "  • Container Status:"
            echo "    ssh $VPS_USERNAME@$VPS_IP 'docker ps --filter name=$CONTAINER_NAME'"
            echo ""
            echo "  • Container Logs:"
            echo "    ssh $VPS_USERNAME@$VPS_IP 'docker logs $CONTAINER_NAME'"
            echo ""
            echo "  • Container Inspection:"
            echo "    ssh $VPS_USERNAME@$VPS_IP 'docker inspect $CONTAINER_NAME'"
            echo ""
            echo "  • Rollback (if needed):"
            echo "    ssh $VPS_USERNAME@$VPS_IP 'docker stop $CONTAINER_NAME && docker rm $CONTAINER_NAME && docker run -d --name $CONTAINER_NAME -p $CONTAINER_PORT:8000 --restart unless-stopped rustci:previous'"
            ;;
    esac
}

# Function to generate verification report
generate_verification_report() {
    local format="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    # Collect verification data
    local vps_connectivity="UNKNOWN"
    local container_status="UNKNOWN"
    local health_status="UNKNOWN"
    
    # Test VPS connectivity
    if test_vps_connectivity >/dev/null 2>&1; then
        vps_connectivity="PASS"
    else
        vps_connectivity="FAIL"
    fi
    
    # Check container status
    if check_container_status >/dev/null 2>&1; then
        container_status="PASS"
    else
        container_status="FAIL"
    fi
    
    # Test health endpoints
    if test_health_endpoints >/dev/null 2>&1; then
        health_status="PASS"
    else
        health_status="FAIL"
    fi
    
    case "$format" in
        "json")
            cat << EOF
{
  "verification_report": {
    "timestamp": "$timestamp",
    "deployment": {
      "vps_ip": "$VPS_IP",
      "container_name": "$CONTAINER_NAME",
      "port": $CONTAINER_PORT
    },
    "tests": {
      "vps_connectivity": "$vps_connectivity",
      "container_status": "$container_status",
      "health_endpoints": "$health_status"
    },
    "overall_status": "$(if [[ "$vps_connectivity" == "PASS" && "$container_status" == "PASS" && "$health_status" == "PASS" ]]; then echo "PASS"; else echo "FAIL"; fi)"
  }
}
EOF
            ;;
        "markdown")
            cat << EOF
# Deployment Verification Report

**Generated**: $timestamp

## Deployment Information

- **VPS IP**: $VPS_IP
- **Container Name**: $CONTAINER_NAME
- **Port**: $CONTAINER_PORT

## Test Results

| Test | Status |
|------|--------|
| VPS Connectivity | $vps_connectivity |
| Container Status | $container_status |
| Health Endpoints | $health_status |

## Overall Status

**$(if [[ "$vps_connectivity" == "PASS" && "$container_status" == "PASS" && "$health_status" == "PASS" ]]; then echo "✅ DEPLOYMENT VERIFIED"; else echo "❌ DEPLOYMENT ISSUES DETECTED"; fi)**

## Access Information

$(show_urls_and_commands "markdown")
EOF
            ;;
        *)
            echo "📋 Deployment Verification Report"
            echo "Generated: $timestamp"
            echo ""
            echo "Deployment Information:"
            echo "  VPS IP: $VPS_IP"
            echo "  Container Name: $CONTAINER_NAME"
            echo "  Port: $CONTAINER_PORT"
            echo ""
            echo "Test Results:"
            echo "  VPS Connectivity: $vps_connectivity"
            echo "  Container Status: $container_status"
            echo "  Health Endpoints: $health_status"
            echo ""
            if [[ "$vps_connectivity" == "PASS" && "$container_status" == "PASS" && "$health_status" == "PASS" ]]; then
                print_success "✅ DEPLOYMENT VERIFIED"
            else
                print_error "❌ DEPLOYMENT ISSUES DETECTED"
            fi
            echo ""
            show_urls_and_commands "text"
            ;;
    esac
}

# Function to perform complete verification
perform_complete_verification() {
    print_status "Starting complete deployment verification..."
    
    local verification_passed=true
    
    # Test VPS connectivity
    if ! test_vps_connectivity; then
        verification_passed=false
    fi
    
    # Check container status
    if ! check_container_status; then
        verification_passed=false
    fi
    
    # Test health endpoints
    if ! test_health_endpoints; then
        verification_passed=false
    fi
    
    # Show deployment information
    echo ""
    print_status "Deployment Information:"
    get_deployment_info
    
    echo ""
    if [[ "$verification_passed" == "true" ]]; then
        print_success "✅ Complete deployment verification passed"
        show_urls_and_commands "$OUTPUT_FORMAT"
        return 0
    else
        print_error "❌ Deployment verification failed"
        return 1
    fi
}

# Main function
main() {
    print_status "Deployment Verification Script"
    
    # Validate configuration for commands that need it
    if [[ "$COMMAND" != "urls" ]]; then
        if ! validate_configuration; then
            exit 1
        fi
    fi
    
    case "$COMMAND" in
        "verify")
            perform_complete_verification
            ;;
        "health")
            test_health_endpoints
            ;;
        "status")
            check_container_status
            echo ""
            print_status "Deployment Information:"
            get_deployment_info
            ;;
        "urls")
            show_urls_and_commands "$OUTPUT_FORMAT"
            ;;
        "test-local")
            print_status "Testing local deployment (localhost)..."
            VPS_IP="localhost"
            test_health_endpoints
            ;;
        "test-remote")
            print_status "Testing remote deployment..."
            perform_complete_verification
            ;;
        "generate-report")
            generate_verification_report "$OUTPUT_FORMAT"
            ;;
        *)
            print_error "Unknown command: $COMMAND"
            show_usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"