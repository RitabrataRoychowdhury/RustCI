#!/bin/bash

# RustCI Deployment Script
# Supports local development, single-node, and multi-node deployments

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
DEPLOYMENT_TYPE="local"
ENVIRONMENT="development"
SKIP_BUILD=false
SKIP_TESTS=false
VERBOSE=false

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
RustCI Deployment Script

Usage: $0 [OPTIONS]

OPTIONS:
    -t, --type TYPE         Deployment type: local, single-node, multi-node, k3s (default: local)
    -e, --env ENV          Environment: development, staging, production (default: development)
    -s, --skip-build       Skip building the application
    -T, --skip-tests       Skip running tests
    -v, --verbose          Enable verbose output
    -h, --help             Show this help message

DEPLOYMENT TYPES:
    local                  Run locally with cargo (development)
    single-node           Single Docker container deployment
    multi-node            Multi-node cluster with load balancer
    k3s                   Kubernetes deployment on K3s

EXAMPLES:
    $0                                    # Local development
    $0 -t single-node -e production      # Single node production
    $0 -t multi-node -e staging          # Multi-node staging
    $0 -t k3s -e production              # Kubernetes production

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--type)
            DEPLOYMENT_TYPE="$2"
            shift 2
            ;;
        -e|--env)
            ENVIRONMENT="$2"
            shift 2
            ;;
        -s|--skip-build)
            SKIP_BUILD=true
            shift
            ;;
        -T|--skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
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

# Validate deployment type
case $DEPLOYMENT_TYPE in
    local|single-node|multi-node|k3s)
        ;;
    *)
        print_error "Invalid deployment type: $DEPLOYMENT_TYPE"
        show_usage
        exit 1
        ;;
esac

# Enable verbose output if requested
if [ "$VERBOSE" = true ]; then
    set -x
fi

print_status "Starting RustCI deployment..."
print_status "Deployment type: $DEPLOYMENT_TYPE"
print_status "Environment: $ENVIRONMENT"

# Check prerequisites
check_prerequisites() {
    print_status "Checking prerequisites..."
    
    # Check if required tools are installed
    local missing_tools=()
    
    if ! command -v cargo &> /dev/null; then
        missing_tools+=("cargo (Rust)")
    fi
    
    if [ "$DEPLOYMENT_TYPE" != "local" ]; then
        if ! command -v docker &> /dev/null; then
            missing_tools+=("docker")
        fi
        
        if ! command -v docker-compose &> /dev/null; then
            missing_tools+=("docker-compose")
        fi
    fi
    
    if [ "$DEPLOYMENT_TYPE" = "k3s" ]; then
        if ! command -v kubectl &> /dev/null; then
            missing_tools+=("kubectl")
        fi
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        print_error "Missing required tools:"
        for tool in "${missing_tools[@]}"; do
            echo "  - $tool"
        done
        exit 1
    fi
    
    print_success "All prerequisites satisfied"
}

# Setup environment
setup_environment() {
    print_status "Setting up environment..."
    
    # Create .env file if it doesn't exist
    if [ ! -f .env ]; then
        if [ -f .env.example ]; then
            cp .env.example .env
            print_status "Created .env from .env.example"
        else
            print_warning ".env.example not found, creating minimal .env"
            cat > .env << EOF
RUST_ENV=$ENVIRONMENT
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=rustci
JWT_SECRET=your-super-secret-jwt-key-change-in-production
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
LOG_LEVEL=info
EOF
        fi
    fi
    
    # Set environment-specific variables
    case $ENVIRONMENT in
        production)
            export RUST_ENV=production
            export LOG_LEVEL=warn
            ;;
        staging)
            export RUST_ENV=staging
            export LOG_LEVEL=info
            ;;
        development)
            export RUST_ENV=development
            export LOG_LEVEL=debug
            ;;
    esac
    
    print_success "Environment setup complete"
}

# Build application
build_application() {
    if [ "$SKIP_BUILD" = true ]; then
        print_status "Skipping build (--skip-build specified)"
        return
    fi
    
    print_status "Building RustCI application..."
    
    # Run tests first (unless skipped)
    if [ "$SKIP_TESTS" = false ]; then
        print_status "Running tests..."
        cargo test --lib
        print_success "Tests passed"
    else
        print_status "Skipping tests (--skip-tests specified)"
    fi
    
    # Build based on environment
    if [ "$ENVIRONMENT" = "production" ]; then
        print_status "Building optimized release binary..."
        cargo build --release
    else
        print_status "Building debug binary..."
        cargo build
    fi
    
    print_success "Build complete"
}

# Deploy locally
deploy_local() {
    print_status "Starting local development deployment..."
    
    # Start MongoDB if not running
    if ! docker ps | grep -q rustci-mongodb; then
        print_status "Starting MongoDB..."
        docker-compose up -d mongodb
        
        # Wait for MongoDB to be ready
        print_status "Waiting for MongoDB to be ready..."
        sleep 10
    fi
    
    # Run the application
    print_status "Starting RustCI server..."
    if [ "$ENVIRONMENT" = "production" ]; then
        ./target/release/RustAutoDevOps
    else
        cargo run
    fi
}

# Deploy single node
deploy_single_node() {
    print_status "Starting single-node deployment..."
    
    # Build Docker image
    print_status "Building Docker image..."
    docker build -t rustci:latest .
    
    # Start services
    print_status "Starting services..."
    docker-compose up -d
    
    # Wait for services to be ready
    print_status "Waiting for services to be ready..."
    sleep 30
    
    # Check health
    check_health "http://localhost:8080"
    
    print_success "Single-node deployment complete"
    print_status "Access RustCI at: http://localhost:8080"
    print_status "Swagger UI at: http://localhost:8080/swagger-ui"
}

# Deploy multi-node
deploy_multi_node() {
    print_status "Starting multi-node deployment..."
    
    # Check for required environment variables
    if [ -z "$GITHUB_OAUTH_CLIENT_ID" ] || [ -z "$GITHUB_OAUTH_CLIENT_SECRET" ]; then
        print_error "GitHub OAuth credentials required for multi-node deployment"
        print_error "Please set GITHUB_OAUTH_CLIENT_ID and GITHUB_OAUTH_CLIENT_SECRET"
        exit 1
    fi
    
    # Build Docker image
    print_status "Building Docker image..."
    docker build -t rustci:latest .
    
    # Start multi-node cluster
    print_status "Starting multi-node cluster..."
    docker-compose -f docker-compose.multi-node.yaml up -d
    
    # Wait for cluster to be ready
    print_status "Waiting for cluster to be ready..."
    sleep 60
    
    # Check health of all nodes
    print_status "Checking cluster health..."
    check_health "http://localhost:8080" "Master Node 1"
    check_health "http://localhost:8081" "Master Node 2"
    check_health "http://localhost:8082" "Master Node 3"
    check_health "http://localhost:80" "Load Balancer"
    
    print_success "Multi-node deployment complete"
    print_status "Access RustCI at: http://localhost:80 (Load Balanced)"
    print_status "Master nodes:"
    print_status "  - Node 1: http://localhost:8080"
    print_status "  - Node 2: http://localhost:8081"
    print_status "  - Node 3: http://localhost:8082"
    print_status "Monitoring:"
    print_status "  - Prometheus: http://localhost:9090"
    print_status "  - HAProxy Stats: http://localhost:8404/stats"
}

# Deploy to K3s
deploy_k3s() {
    print_status "Starting K3s deployment..."
    
    # Check if K3s is running
    if ! kubectl cluster-info &> /dev/null; then
        print_error "K3s cluster not accessible. Please ensure K3s is running."
        print_status "To install K3s: curl -sfL https://get.k3s.io | sh -"
        exit 1
    fi
    
    # Apply Kubernetes manifests
    if [ -d "deployment/k3s" ]; then
        print_status "Applying Kubernetes manifests..."
        kubectl apply -f deployment/k3s/
    elif [ -d "helm/rustci" ]; then
        print_status "Installing with Helm..."
        helm upgrade --install rustci ./helm/rustci \
            --set environment=$ENVIRONMENT \
            --set image.tag=latest
    else
        print_error "No Kubernetes deployment files found"
        exit 1
    fi
    
    # Wait for deployment
    print_status "Waiting for deployment to be ready..."
    kubectl wait --for=condition=available --timeout=300s deployment/rustci
    
    # Get service URL
    SERVICE_URL=$(kubectl get service rustci -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
    if [ -z "$SERVICE_URL" ]; then
        SERVICE_URL="localhost"
        kubectl port-forward service/rustci 8080:80 &
        PORT_FORWARD_PID=$!
        print_status "Port forwarding started (PID: $PORT_FORWARD_PID)"
    fi
    
    print_success "K3s deployment complete"
    print_status "Access RustCI at: http://$SERVICE_URL:8080"
}

# Health check function
check_health() {
    local url=$1
    local name=${2:-"Service"}
    local max_attempts=30
    local attempt=1
    
    print_status "Checking health of $name..."
    
    while [ $attempt -le $max_attempts ]; do
        if curl -f -s "$url/health" > /dev/null 2>&1; then
            print_success "$name is healthy"
            return 0
        fi
        
        print_status "Attempt $attempt/$max_attempts: $name not ready yet..."
        sleep 5
        ((attempt++))
    done
    
    print_error "$name failed to become healthy"
    return 1
}

# Cleanup function
cleanup() {
    print_status "Cleaning up..."
    
    case $DEPLOYMENT_TYPE in
        single-node)
            docker-compose down
            ;;
        multi-node)
            docker-compose -f docker-compose.multi-node.yaml down
            ;;
        k3s)
            if [ -n "$PORT_FORWARD_PID" ]; then
                kill $PORT_FORWARD_PID 2>/dev/null || true
            fi
            ;;
    esac
}

# Set up signal handlers
trap cleanup EXIT INT TERM

# Main deployment logic
main() {
    check_prerequisites
    setup_environment
    
    case $DEPLOYMENT_TYPE in
        local)
            build_application
            deploy_local
            ;;
        single-node)
            build_application
            deploy_single_node
            ;;
        multi-node)
            build_application
            deploy_multi_node
            ;;
        k3s)
            build_application
            deploy_k3s
            ;;
    esac
}

# Run main function
main

print_success "RustCI deployment completed successfully!"