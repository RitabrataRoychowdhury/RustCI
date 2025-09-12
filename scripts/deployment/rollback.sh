#!/bin/bash

# Rollback Utility Script
# Provides rollback functionality using image tagging strategy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
CONTAINER_NAME="rustci"
PREVIOUS_TAG="previous"
BACKUP_TAG="backup"
CONTAINER_PORT=8080
HEALTH_CHECK_TIMEOUT=30
HEALTH_CHECK_RETRIES=3
HEALTH_CHECK_INTERVAL=10

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
Rollback Utility Script

Usage: $0 [OPTIONS] [COMMAND]

COMMANDS:
    rollback               Rollback to previous version (default)
    list-backups          List available backup images
    rollback-to TAG       Rollback to specific backup tag
    status                Show current deployment status

OPTIONS:
    -c, --container NAME   Container name (default: rustci)
    -p, --port PORT        Container port (default: 8080)
    --health-timeout SEC   Health check timeout per request (default: 30)
    --health-retries NUM   Health check retry attempts (default: 3)
    -e, --env-file FILE    Environment file to use
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

EXAMPLES:
    $0                                    # Rollback to previous version
    $0 rollback                          # Same as above
    $0 list-backups                      # List available backups
    $0 rollback-to backup_20231209_143022 # Rollback to specific backup
    $0 status                            # Show current status

ROLLBACK PROCESS:
    1. Stop current container
    2. Start container with previous/specified image
    3. Perform health checks
    4. Verify rollback success

EOF
}

# Parse command line arguments
COMMAND="rollback"
ENV_FILE=""
VERBOSE=false
ROLLBACK_TAG=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--container)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        -p|--port)
            CONTAINER_PORT="$2"
            shift 2
            ;;
        --health-timeout)
            HEALTH_CHECK_TIMEOUT="$2"
            shift 2
            ;;
        --health-retries)
            HEALTH_CHECK_RETRIES="$2"
            shift 2
            ;;
        -e|--env-file)
            ENV_FILE="$2"
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
        rollback|list-backups|status)
            COMMAND="$1"
            shift
            ;;
        rollback-to)
            COMMAND="rollback-to"
            ROLLBACK_TAG="$2"
            shift 2
            ;;
        -*)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
        *)
            print_error "Unknown argument: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

# Function to check if container exists
container_exists() {
    local name=$1
    docker ps -a --format '{{.Names}}' | grep -q "^${name}$"
}

# Function to check if container is running
container_running() {
    local name=$1
    docker ps --format '{{.Names}}' | grep -q "^${name}$"
}

# Function to get container image
get_container_image() {
    local name=$1
    docker inspect "$name" --format='{{.Config.Image}}' 2>/dev/null || echo ""
}

# Function to get image base name
get_image_base() {
    local current_image
    
    if container_exists "$CONTAINER_NAME"; then
        current_image=$(get_container_image "$CONTAINER_NAME")
        if [ -n "$current_image" ]; then
            echo "$current_image" | cut -d: -f1
            return 0
        fi
    fi
    
    # Fallback: try to find rustci images
    local rustci_images
    rustci_images=$(docker images --format "{{.Repository}}" | grep "rustci" | head -1)
    
    if [ -n "$rustci_images" ]; then
        echo "$rustci_images"
        return 0
    fi
    
    print_error "Could not determine image base name"
    return 1
}

# Function to list available backup images
list_backups() {
    print_status "Available backup images:"
    
    local image_base
    image_base=$(get_image_base)
    
    if [ $? -ne 0 ]; then
        return 1
    fi
    
    echo ""
    echo "Current deployment:"
    if container_exists "$CONTAINER_NAME"; then
        local current_image
        current_image=$(get_container_image "$CONTAINER_NAME")
        echo "  Container: $CONTAINER_NAME"
        echo "  Image: $current_image"
        echo "  Status: $(container_running "$CONTAINER_NAME" && echo "Running" || echo "Stopped")"
    else
        echo "  No container found with name: $CONTAINER_NAME"
    fi
    
    echo ""
    echo "Available images for rollback:"
    
    # List previous image
    local previous_image="${image_base}:${PREVIOUS_TAG}"
    if docker image inspect "$previous_image" >/dev/null 2>&1; then
        local created=$(docker image inspect "$previous_image" --format='{{.Created}}' | cut -d'T' -f1)
        echo "  Previous: $previous_image (created: $created)"
    else
        echo "  Previous: Not available"
    fi
    
    # List backup images
    local backup_images
    backup_images=$(docker images --format "{{.Repository}}:{{.Tag}}\t{{.CreatedAt}}" | grep "${image_base}:${BACKUP_TAG}_" | sort -k2 -r)
    
    if [ -n "$backup_images" ]; then
        echo "  Backups:"
        echo "$backup_images" | while IFS=$'\t' read -r image created; do
            echo "    $image (created: $created)"
        done
    else
        echo "  Backups: None available"
    fi
    
    echo ""
}

# Function to show deployment status
show_status() {
    print_status "Deployment Status:"
    
    echo "  Container Name: $CONTAINER_NAME"
    echo "  Port: $CONTAINER_PORT"
    
    if container_exists "$CONTAINER_NAME"; then
        local current_image
        current_image=$(get_container_image "$CONTAINER_NAME")
        local container_id
        container_id=$(docker ps -a --filter name="$CONTAINER_NAME" --format '{{.ID}}')
        local status
        status=$(docker ps -a --filter name="$CONTAINER_NAME" --format '{{.Status}}')
        
        echo "  Image: $current_image"
        echo "  Container ID: $container_id"
        echo "  Status: $status"
        
        if container_running "$CONTAINER_NAME"; then
            echo "  Health Endpoints:"
            echo "    Primary: http://localhost:$CONTAINER_PORT/api/healthchecker"
            echo "    Fallback: http://localhost:$CONTAINER_PORT/health"
        fi
    else
        echo "  Status: Container not found"
    fi
    
    echo ""
    echo "Verification Commands:"
    echo "  docker ps --filter name=$CONTAINER_NAME"
    echo "  docker logs $CONTAINER_NAME"
    if container_running "$CONTAINER_NAME"; then
        echo "  curl -f http://localhost:$CONTAINER_PORT/api/healthchecker"
    fi
}

# Function to use integrated health checker if available
use_integrated_health_checker() {
    local host="${1:-localhost}"
    local port="${2:-$CONTAINER_PORT}"
    
    local script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    local health_checker="${script_dir}/health-checker.sh"
    
    if [[ -f "$health_checker" ]] && [[ -x "$health_checker" ]]; then
        print_status "Using integrated health checker"
        if "$health_checker" check "$host" "$port"; then
            return 0
        else
            return 1
        fi
    else
        return 2  # Health checker not available
    fi
}

# Function to perform health check
perform_health_check() {
    local endpoint=$1
    local timeout=${2:-$HEALTH_CHECK_TIMEOUT}
    
    local url="http://localhost:$CONTAINER_PORT$endpoint"
    
    if curl -f -s --max-time "$timeout" "$url" >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Function to wait for container to be healthy
wait_for_health() {
    print_status "Performing health checks..."
    
    # Try integrated health checker first
    local use_integrated=false
    if use_integrated_health_checker "localhost" "$CONTAINER_PORT"; then
        print_success "Integrated health check passed"
        return 0
    elif [[ $? -eq 2 ]]; then
        print_status "Integrated health checker not available, using fallback method"
        use_integrated=false
    else
        print_warning "Integrated health checker failed, trying fallback method"
        use_integrated=false
    fi
    
    # Fallback to manual health checks
    local attempt=1
    local max_attempts=$HEALTH_CHECK_RETRIES
    
    # Wait a bit for container to start
    print_status "Waiting for container to initialize..."
    sleep 10
    
    while [ $attempt -le $max_attempts ]; do
        print_status "Health check attempt $attempt/$max_attempts"
        
        # Check if container is still running
        if ! container_running "$CONTAINER_NAME"; then
            print_error "Container stopped unexpectedly"
            return 1
        fi
        
        # Try primary health endpoint
        if perform_health_check "/api/healthchecker"; then
            print_success "Primary health check passed (/api/healthchecker)"
            
            # Also test fallback endpoint
            if perform_health_check "/health"; then
                print_success "Fallback health check passed (/health)"
                print_success "All health checks passed"
                return 0
            else
                print_warning "Fallback health check failed, but primary passed"
                return 0
            fi
        else
            print_warning "Primary health check failed, trying fallback..."
            
            # Try fallback health endpoint
            if perform_health_check "/health"; then
                print_success "Fallback health check passed (/health)"
                return 0
            else
                print_error "Both health checks failed"
            fi
        fi
        
        if [ $attempt -lt $max_attempts ]; then
            print_status "Waiting ${HEALTH_CHECK_INTERVAL}s before next attempt..."
            sleep $HEALTH_CHECK_INTERVAL
        fi
        
        ((attempt++))
    done
    
    print_error "Health checks failed after $max_attempts attempts"
    return 1
}

# Function to start container with specified image
start_container() {
    local image=$1
    
    print_status "Starting container with image: $image"
    
    # Build docker run command
    local docker_cmd="docker run -d --name $CONTAINER_NAME"
    
    # Add port mapping
    docker_cmd="$docker_cmd -p $CONTAINER_PORT:$CONTAINER_PORT"
    
    # Add environment file if specified
    if [ -n "$ENV_FILE" ] && [ -f "$ENV_FILE" ]; then
        docker_cmd="$docker_cmd --env-file $ENV_FILE"
        print_status "Using environment file: $ENV_FILE"
    fi
    
    # Add restart policy
    docker_cmd="$docker_cmd --restart unless-stopped"
    
    # Add health check
    docker_cmd="$docker_cmd --health-cmd='curl -f http://localhost:$CONTAINER_PORT/api/healthchecker || curl -f http://localhost:$CONTAINER_PORT/health || exit 1'"
    docker_cmd="$docker_cmd --health-interval=30s --health-timeout=10s --health-retries=3"
    
    # Add image
    docker_cmd="$docker_cmd $image"
    
    print_status "Executing: $docker_cmd"
    
    if eval "$docker_cmd"; then
        print_success "Container started successfully"
        return 0
    else
        print_error "Failed to start container"
        return 1
    fi
}

# Function to stop and remove current container
stop_current_container() {
    print_status "Stopping current container..."
    
    if container_running "$CONTAINER_NAME"; then
        print_status "Stopping container: $CONTAINER_NAME"
        docker stop "$CONTAINER_NAME" || true
        print_success "Container stopped"
    else
        print_status "Container $CONTAINER_NAME is not running"
    fi
    
    # Remove container if it exists
    if container_exists "$CONTAINER_NAME"; then
        print_status "Removing container: $CONTAINER_NAME"
        docker rm "$CONTAINER_NAME" || true
        print_success "Container removed"
    fi
}

# Function to perform rollback
perform_rollback() {
    local target_image=$1
    
    print_status "Starting rollback process..."
    print_status "Target image: $target_image"
    
    # Check if target image exists
    if ! docker image inspect "$target_image" >/dev/null 2>&1; then
        print_error "Target image not found: $target_image"
        return 1
    fi
    
    # Stop current container
    stop_current_container
    
    # Start container with target image
    if ! start_container "$target_image"; then
        print_error "Failed to start rollback container"
        return 1
    fi
    
    # Wait for health checks
    if ! wait_for_health; then
        print_error "Rollback health checks failed"
        return 1
    fi
    
    print_success "Rollback completed successfully"
    print_status "Service restored to: $target_image"
    
    return 0
}

# Main function
main() {
    print_status "Rollback Utility Script"
    
    # Check if Docker is running
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker is not running or not accessible"
        exit 1
    fi
    
    case $COMMAND in
        "rollback")
            print_status "Rolling back to previous version..."
            
            local image_base
            image_base=$(get_image_base)
            
            if [ $? -ne 0 ]; then
                exit 1
            fi
            
            local previous_image="${image_base}:${PREVIOUS_TAG}"
            
            if ! docker image inspect "$previous_image" >/dev/null 2>&1; then
                print_error "No previous image found: $previous_image"
                print_status "Available images:"
                docker images | grep "$image_base"
                exit 1
            fi
            
            if perform_rollback "$previous_image"; then
                print_success "Rollback to previous version completed"
            else
                print_error "Rollback failed"
                exit 1
            fi
            ;;
            
        "rollback-to")
            if [ -z "$ROLLBACK_TAG" ]; then
                print_error "Rollback tag not specified"
                show_usage
                exit 1
            fi
            
            print_status "Rolling back to specific version: $ROLLBACK_TAG"
            
            local image_base
            image_base=$(get_image_base)
            
            if [ $? -ne 0 ]; then
                exit 1
            fi
            
            local target_image="${image_base}:${ROLLBACK_TAG}"
            
            if perform_rollback "$target_image"; then
                print_success "Rollback to $ROLLBACK_TAG completed"
            else
                print_error "Rollback failed"
                exit 1
            fi
            ;;
            
        "list-backups")
            list_backups
            ;;
            
        "status")
            show_status
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