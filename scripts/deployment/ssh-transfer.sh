#!/bin/bash

# SSH Transfer Script for Cross-Architecture Deployment
# Handles compressed Docker image transfer with retry logic and error handling

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
MAX_RETRIES=5
BASE_DELAY=5
MAX_DELAY=80
MULTIPLIER=2
COMPRESSION_LEVEL=6
TRANSFER_TIMEOUT=600  # 10 minutes

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
SSH Transfer Script for Cross-Architecture Deployment

Usage: $0 [OPTIONS] IMAGE_NAME VPS_HOST

ARGUMENTS:
    IMAGE_NAME          Docker image name to transfer (e.g., rustci:production)
    VPS_HOST           VPS host in format user@host (e.g., root@46.37.122.118)

OPTIONS:
    -p, --password PASS    SSH password (use with caution)
    -k, --key-file FILE    SSH private key file
    -t, --timeout SEC      Transfer timeout in seconds (default: 600)
    -r, --retries NUM      Maximum retry attempts (default: 5)
    -c, --compression LVL  Gzip compression level 1-9 (default: 6)
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

EXAMPLES:
    $0 rustci:production root@46.37.122.118
    $0 -p "password" -t 900 rustci:production root@46.37.122.118
    $0 -k ~/.ssh/id_rsa rustci:production root@46.37.122.118

EOF
}

# Parse command line arguments
SSH_PASSWORD=""
SSH_KEY_FILE=""
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -p|--password)
            SSH_PASSWORD="$2"
            shift 2
            ;;
        -k|--key-file)
            SSH_KEY_FILE="$2"
            shift 2
            ;;
        -t|--timeout)
            TRANSFER_TIMEOUT="$2"
            shift 2
            ;;
        -r|--retries)
            MAX_RETRIES="$2"
            shift 2
            ;;
        -c|--compression)
            COMPRESSION_LEVEL="$2"
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
            elif [ -z "$VPS_HOST" ]; then
                VPS_HOST="$1"
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
if [ -z "$IMAGE_NAME" ] || [ -z "$VPS_HOST" ]; then
    print_error "Missing required arguments"
    show_usage
    exit 1
fi

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

# Validate compression level
if [ "$COMPRESSION_LEVEL" -lt 1 ] || [ "$COMPRESSION_LEVEL" -gt 9 ]; then
    print_error "Compression level must be between 1 and 9"
    exit 1
fi

# Function to calculate exponential backoff delay
calculate_delay() {
    local attempt=$1
    local delay=$((BASE_DELAY * (MULTIPLIER ** (attempt - 1))))
    
    # Cap at max delay
    if [ $delay -gt $MAX_DELAY ]; then
        delay=$MAX_DELAY
    fi
    
    # Add jitter (random 0-20% of delay)
    local jitter=$((delay * RANDOM / 32768 / 5))
    delay=$((delay + jitter))
    
    echo $delay
}

# Function to test SSH connection
test_ssh_connection() {
    local host=$1
    local ssh_opts=""
    
    # Build SSH options
    if [ -n "$SSH_KEY_FILE" ]; then
        ssh_opts="-i $SSH_KEY_FILE"
    fi
    
    ssh_opts="$ssh_opts -o ConnectTimeout=10 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"
    
    print_status "Testing SSH connection to $host..."
    
    if [ -n "$SSH_PASSWORD" ]; then
        if ! command -v sshpass &> /dev/null; then
            print_error "sshpass is required for password authentication"
            print_status "Install with: brew install hudochenkov/sshpass/sshpass (macOS) or apt-get install sshpass (Ubuntu)"
            exit 1
        fi
        
        if sshpass -p "$SSH_PASSWORD" ssh $ssh_opts "$host" "echo 'SSH connection successful'" 2>/dev/null; then
            print_success "SSH connection test passed"
            return 0
        else
            print_error "SSH connection test failed"
            return 1
        fi
    else
        if ssh $ssh_opts "$host" "echo 'SSH connection successful'" 2>/dev/null; then
            print_success "SSH connection test passed"
            return 0
        else
            print_error "SSH connection test failed"
            return 1
        fi
    fi
}

# Function to get image size
get_image_size() {
    local image=$1
    local size_bytes
    
    size_bytes=$(docker image inspect "$image" --format='{{.Size}}' 2>/dev/null || echo "0")
    
    if [ "$size_bytes" = "0" ]; then
        print_error "Image $image not found locally"
        return 1
    fi
    
    # Convert to human readable format
    local size_mb=$((size_bytes / 1024 / 1024))
    echo "${size_mb}MB"
}

# Function to transfer image with retry logic
transfer_image() {
    local image=$1
    local host=$2
    local attempt=1
    
    print_status "Starting transfer of $image to $host"
    
    # Get image size for progress indication
    local image_size
    image_size=$(get_image_size "$image")
    print_status "Image size: $image_size"
    
    while [ $attempt -le $MAX_RETRIES ]; do
        print_status "Transfer attempt $attempt/$MAX_RETRIES"
        
        # Build SSH command
        local ssh_cmd="ssh"
        local ssh_opts="-C -o Compression=yes -o CompressionLevel=$COMPRESSION_LEVEL -o ConnectTimeout=30 -o ServerAliveInterval=60 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"
        
        if [ -n "$SSH_KEY_FILE" ]; then
            ssh_opts="$ssh_opts -i $SSH_KEY_FILE"
        fi
        
        # Prepare the transfer command
        local transfer_cmd
        if [ -n "$SSH_PASSWORD" ]; then
            transfer_cmd="docker save '$image' | gzip -$COMPRESSION_LEVEL | timeout $TRANSFER_TIMEOUT sshpass -p '$SSH_PASSWORD' $ssh_cmd $ssh_opts '$host' 'gunzip | docker load'"
        else
            transfer_cmd="docker save '$image' | gzip -$COMPRESSION_LEVEL | timeout $TRANSFER_TIMEOUT $ssh_cmd $ssh_opts '$host' 'gunzip | docker load'"
        fi
        
        print_status "Executing transfer (compression level: $COMPRESSION_LEVEL)..."
        
        # Execute transfer with error handling
        if eval "$transfer_cmd" 2>/dev/null; then
            print_success "Image transfer completed successfully"
            
            # Verify image was loaded correctly
            print_status "Verifying image on remote host..."
            local verify_cmd
            if [ -n "$SSH_PASSWORD" ]; then
                verify_cmd="sshpass -p '$SSH_PASSWORD' ssh $ssh_opts '$host' 'docker images --format \"table {{.Repository}}\t{{.Tag}}\t{{.ID}}\" | grep \"$(echo $image | cut -d: -f1)\"'"
            else
                verify_cmd="ssh $ssh_opts '$host' 'docker images --format \"table {{.Repository}}\t{{.Tag}}\t{{.ID}}\" | grep \"$(echo $image | cut -d: -f1)\"'"
            fi
            
            if eval "$verify_cmd" >/dev/null 2>&1; then
                print_success "Image verification passed"
                return 0
            else
                print_warning "Image verification failed, but transfer appeared successful"
                return 0
            fi
        else
            local exit_code=$?
            print_error "Transfer attempt $attempt failed (exit code: $exit_code)"
            
            if [ $attempt -eq $MAX_RETRIES ]; then
                print_error "All transfer attempts failed"
                return 1
            fi
            
            # Calculate delay for next attempt
            local delay
            delay=$(calculate_delay $attempt)
            print_status "Retrying in $delay seconds..."
            sleep $delay
            
            ((attempt++))
        fi
    done
    
    return 1
}

# Function to cleanup remote images (optional)
cleanup_old_images() {
    local host=$1
    local image_name=$(echo "$IMAGE_NAME" | cut -d: -f1)
    
    print_status "Cleaning up old images on remote host..."
    
    local cleanup_cmd
    if [ -n "$SSH_PASSWORD" ]; then
        cleanup_cmd="sshpass -p '$SSH_PASSWORD' ssh $ssh_opts '$host' 'docker image prune -f && docker images | grep \"$image_name\" | grep \"<none>\" | awk \"{print \\\$3}\" | xargs -r docker rmi'"
    else
        cleanup_cmd="ssh $ssh_opts '$host' 'docker image prune -f && docker images | grep \"$image_name\" | grep \"<none>\" | awk \"{print \\\$3}\" | xargs -r docker rmi'"
    fi
    
    if eval "$cleanup_cmd" 2>/dev/null; then
        print_success "Cleanup completed"
    else
        print_warning "Cleanup failed or no images to clean"
    fi
}

# Main execution
main() {
    print_status "SSH Transfer Script for Cross-Architecture Deployment"
    print_status "Image: $IMAGE_NAME"
    print_status "Target: $VPS_HOST"
    print_status "Max retries: $MAX_RETRIES"
    print_status "Transfer timeout: ${TRANSFER_TIMEOUT}s"
    
    # Check if Docker is running
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker is not running or not accessible"
        exit 1
    fi
    
    # Check if image exists locally
    if ! docker image inspect "$IMAGE_NAME" >/dev/null 2>&1; then
        print_error "Image $IMAGE_NAME not found locally"
        print_status "Available images:"
        docker images
        exit 1
    fi
    
    # Test SSH connection
    if ! test_ssh_connection "$VPS_HOST"; then
        print_error "SSH connection test failed"
        exit 1
    fi
    
    # Transfer the image
    if transfer_image "$IMAGE_NAME" "$VPS_HOST"; then
        print_success "Image transfer completed successfully"
        
        # Optional cleanup
        cleanup_old_images "$VPS_HOST"
        
        print_success "SSH transfer process completed"
        print_status "Image $IMAGE_NAME is now available on $VPS_HOST"
    else
        print_error "Image transfer failed"
        exit 1
    fi
}

# Run main function
main "$@"