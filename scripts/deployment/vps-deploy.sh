#!/bin/bash

# VPS Deployment Script with Container Backup and Swap Mechanisms
# Handles deployment with rollback functionality using image tagging strategy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
CONTAINER_NAME="rustci"
PRODUCTION_TAG="production"
PREVIOUS_TAG="previous"
BACKUP_TAG="backup"
HEALTH_CHECK_TIMEOUT=30
HEALTH_CHECK_RETRIES=3
HEALTH_CHECK_INTERVAL=10
DEPLOYMENT_TIMEOUT=300  # 5 minutes
ROLLBACK_ENABLED=true

# Health check endpoints
PRIMARY_HEALTH_ENDPOINT="/api/healthchecker"
FALLBACK_HEALTH_ENDPOINT="/health"
CONTAINER_PORT=8080

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
VPS Deployment Script with Container Backup and Swap Mechanisms

Usage: $0 [OPTIONS] IMAGE_NAME

ARGUMENTS:
    IMAGE_NAME          Docker image name to deploy (e.g., rustci:production)

OPTIONS:
    -c, --container NAME   Container name (default: rustci)
    -p, --port PORT        Container port (default: 8080)
    -t, --timeout SEC      Deployment timeout in seconds (default: 300)
    --health-timeout SEC   Health check timeout per request (default: 30)
    --health-retries NUM   Health check retry attempts (default: 3)
    --no-rollback          Disable automatic rollback on failure
    -e, --env-file FILE    Environment file to use
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

EXAMPLES:
    $0 rustci:production
    $0 -c rustci-app -p 8080 rustci:production
    $0 --no-rollback -e .env.production rustci:production

DEPLOYMENT PROCESS:
    1. Backup current container (if exists)
    2. Tag current image as 'previous'
    3. Stop current container
    4. Start new container with new image
    5. Perform health checks
    6. Rollback if health checks fail (unless disabled)

EOF
}

# Parse command line arguments
ENV_FILE=""
VERBOSE=false

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
        -t|--timeout)
            DEPLOYMENT_TIMEOUT="$2"
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
        --no-rollback)
            ROLLBACK_ENABLED=false
            shift
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
        -*)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
        *)
            if [ -z "$IMAGE_NAME" ]; then
                IMAGE_NAME="$1"
            else
                print_error "Too many arguments"
                show_usage
                exit 1
            fi
            shift
            ;;
    esac
done

# Validate required arguments
if [ -z "$IMAGE_NAME" ]; then
    print_error "Missing required argument: IMAGE_NAME"
    show_usage
    exit 1
fi

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

# Function to backup current deployment
backup_current_deployment() {
    print_status "Backing up current deployment..."
    
    if container_exists "$CONTAINER_NAME"; then
        local current_image
        current_image=$(get_container_image "$CONTAINER_NAME")
        
        if [ -n "$current_image" ]; then
            print_status "Current container image: $current_image"
            
            # Tag current image as previous
            local image_base=$(echo "$current_image" | cut -d: -f1)
            docker tag "$current_image" "${image_base}:${PREVIOUS_TAG}"
            print_success "Tagged current image as ${image_base}:${PREVIOUS_TAG}"
            
            # Create backup tag with timestamp
            local timestamp=$(date +%Y%m%d_%H%M%S)
            docker tag "$current_image" "${image_base}:${BACKUP_TAG}_${timestamp}"
            print_success "Created backup tag: ${image_base}:${BACKUP_TAG}_${timestamp}"
        else
            print_warning "Could not determine current container image"
        fi
    else
        print_status "No existing container to backup"
    fi
}

# Function to stop current container
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

# Function to start new container
start_new_container() {
    local image=$1
    
    print_status "Starting new container with image: $image"
    
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
    docker_cmd="$docker_cmd --health-cmd='curl -f http://localhost:$CONTAINER_PORT$PRIMARY_HEALTH_ENDPOINT || curl -f http://localhost:$CONTAINER_PORT$FALLBACK_HEALTH_ENDPOINT || exit 1'"
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
    
    local attempt=1
    local max_attempts=$HEALTH_CHECK_RETRIES
    local consecutive_failures=0
    local max_consecutive_failures=3
    
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
        if perform_health_check "$PRIMARY_HEALTH_ENDPOINT"; then
            print_success "Primary health check passed ($PRIMARY_HEALTH_ENDPOINT)"
            consecutive_failures=0
            
            # Also test fallback endpoint
            if perform_health_check "$FALLBACK_HEALTH_ENDPOINT"; then
                print_success "Fallback health check passed ($FALLBACK_HEALTH_ENDPOINT)"
                print_success "All health checks passed"
                return 0
            else
                print_warning "Fallback health check failed, but primary passed"
                return 0
            fi
        else
            print_warning "Primary health check failed, trying fallback..."
            
            # Try fallback health endpoint
            if perform_health_check "$FALLBACK_HEALTH_ENDPOINT"; then
                print_success "Fallback health check passed ($FALLBACK_HEALTH_ENDPOINT)"
                consecutive_failures=0
                return 0
            else
                print_error "Both health checks failed"
                ((consecutive_failures++))
                
                if [ $consecutive_failures -ge $max_consecutive_failures ]; then
                    print_error "Too many consecutive health check failures"
                    return 1
                fi
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

# Function to rollback deployment
rollback_deployment() {
    print_error "Deployment failed, initiating rollback..."
    
    if [ "$ROLLBACK_ENABLED" = false ]; then
        print_warning "Rollback is disabled, manual intervention required"
        return 1
    fi
    
    # Stop failed container
    if container_running "$CONTAINER_NAME"; then
        print_status "Stopping failed container..."
        docker stop "$CONTAINER_NAME" || true
        docker rm "$CONTAINER_NAME" || true
    fi
    
    # Find previous image
    local image_base=$(echo "$IMAGE_NAME" | cut -d: -f1)
    local previous_image="${image_base}:${PREVIOUS_TAG}"
    
    if docker image inspect "$previous_image" >/dev/null 2>&1; then
        print_status "Rolling back to previous image: $previous_image"
        
        if start_new_container "$previous_image"; then
            print_status "Rollback container started, checking health..."
            
            if wait_for_health; then
                print_success "Rollback completed successfully"
                print_status "Service restored to previous version"
                return 0
            else
                print_error "Rollback health checks failed"
                return 1
            fi
        else
            print_error "Failed to start rollback container"
            return 1
        fi
    else
        print_error "No previous image found for rollback"
        print_error "Manual intervention required"
        return 1
    fi
}

# Function to cleanup old images
cleanup_old_images() {
    print_status "Cleaning up old images..."
    
    # Keep only the last 3 backup images
    local image_base=$(echo "$IMAGE_NAME" | cut -d: -f1)
    local backup_images
    backup_images=$(docker images --format "{{.Repository}}:{{.Tag}}" | grep "${image_base}:${BACKUP_TAG}_" | sort -r | tail -n +4)
    
    if [ -n "$backup_images" ]; then
        print_status "Removing old backup images..."
        echo "$backup_images" | xargs -r docker rmi
        print_success "Old backup images removed"
    fi
    
    # Remove dangling images
    docker image prune -f >/dev/null 2>&1 || true
}

# Function to display deployment status
show_deployment_status() {
    print_status "Deployment Status:"
    echo "  Container Name: $CONTAINER_NAME"
    echo "  Image: $IMAGE_NAME"
    echo "  Port: $CONTAINER_PORT"
    echo "  Health Endpoints:"
    echo "    Primary: http://localhost:$CONTAINER_PORT$PRIMARY_HEALTH_ENDPOINT"
    echo "    Fallback: http://localhost:$CONTAINER_PORT$FALLBACK_HEALTH_ENDPOINT"
    
    if container_running "$CONTAINER_NAME"; then
        echo "  Status: Running"
        echo "  Container ID: $(docker ps --filter name=$CONTAINER_NAME --format '{{.ID}}')"
    else
        echo "  Status: Not Running"
    fi
    
    echo ""
    echo "Verification Commands:"
    echo "  docker ps --filter name=$CONTAINER_NAME"
    echo "  docker logs $CONTAINER_NAME"
    echo "  curl -f http://localhost:$CONTAINER_PORT$PRIMARY_HEALTH_ENDPOINT"
}

# Main deployment function
deploy() {
    local start_time=$(date +%s)
    
    print_status "Starting VPS deployment..."
    print_status "Image: $IMAGE_NAME"
    print_status "Container: $CONTAINER_NAME"
    print_status "Port: $CONTAINER_PORT"
    print_status "Rollback enabled: $ROLLBACK_ENABLED"
    
    # Check if Docker is running
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker is not running or not accessible"
        exit 1
    fi
    
    # Check if image exists
    if ! docker image inspect "$IMAGE_NAME" >/dev/null 2>&1; then
        print_error "Image $IMAGE_NAME not found"
        print_status "Available images:"
        docker images
        exit 1
    fi
    
    # Backup current deployment
    backup_current_deployment
    
    # Stop current container
    stop_current_container
    
    # Start new container
    if ! start_new_container "$IMAGE_NAME"; then
        print_error "Failed to start new container"
        if [ "$ROLLBACK_ENABLED" = true ]; then
            rollback_deployment
        fi
        exit 1
    fi
    
    # Wait for health checks
    if ! wait_for_health; then
        print_error "Health checks failed"
        if [ "$ROLLBACK_ENABLED" = true ]; then
            rollback_deployment
        fi
        exit 1
    fi
    
    # Cleanup old images
    cleanup_old_images
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    print_success "Deployment completed successfully in ${duration}s"
    
    # Show deployment status
    show_deployment_status
}

# Set up timeout for entire deployment
timeout_handler() {
    print_error "Deployment timed out after ${DEPLOYMENT_TIMEOUT}s"
    if [ "$ROLLBACK_ENABLED" = true ]; then
        rollback_deployment
    fi
    exit 1
}

# Set up signal handlers
trap timeout_handler ALRM
trap 'kill -ALRM $$' EXIT

# Start timeout
(sleep $DEPLOYMENT_TIMEOUT && kill -ALRM $$) &
TIMEOUT_PID=$!

# Run deployment
deploy

# Cancel timeout
kill $TIMEOUT_PID 2>/dev/null || true
trap - EXIT ALRM

print_success "VPS deployment process completed successfully"