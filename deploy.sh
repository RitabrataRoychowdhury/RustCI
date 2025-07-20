#!/bin/bash

# RustCI Kubernetes Deployment Script
# This script automates the build and deployment process

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
IMAGE_NAME="rustci"
IMAGE_TAG="latest"
NAMESPACE="rustci"
RELEASE_NAME="rustci"
HELM_CHART="./helm/rustci"

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check if Docker is running
    if ! docker info >/dev/null 2>&1; then
        log_error "Docker is not running. Please start Docker Desktop."
        exit 1
    fi
    
    # Check if kubectl is available
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed. Please install kubectl."
        exit 1
    fi
    
    # Check if Helm is available
    if ! command -v helm &> /dev/null; then
        log_error "Helm is not installed. Please install Helm."
        exit 1
    fi
    
    # Check if Kubernetes is running
    if ! kubectl cluster-info >/dev/null 2>&1; then
        log_error "Kubernetes cluster is not accessible. Please enable Kubernetes in Docker Desktop."
        exit 1
    fi
    
    log_success "All prerequisites are met!"
}

build_image() {
    log_info "Building Docker image..."
    
    if docker build -t ${IMAGE_NAME}:${IMAGE_TAG} .; then
        log_success "Docker image built successfully: ${IMAGE_NAME}:${IMAGE_TAG}"
    else
        log_error "Failed to build Docker image"
        exit 1
    fi
}

create_namespace() {
    log_info "Creating namespace: ${NAMESPACE}"
    
    if kubectl get namespace ${NAMESPACE} >/dev/null 2>&1; then
        log_warning "Namespace ${NAMESPACE} already exists"
    else
        kubectl create namespace ${NAMESPACE}
        log_success "Namespace ${NAMESPACE} created"
    fi
}

deploy_helm() {
    log_info "Deploying with Helm..."
    
    # Check if release already exists
    if helm list -n ${NAMESPACE} | grep -q ${RELEASE_NAME}; then
        log_info "Release ${RELEASE_NAME} exists, upgrading..."
        helm upgrade ${RELEASE_NAME} ${HELM_CHART} \
            --namespace ${NAMESPACE} \
            --set image.pullPolicy=Never \
            --set image.tag=${IMAGE_TAG}
    else
        log_info "Installing new release: ${RELEASE_NAME}"
        helm install ${RELEASE_NAME} ${HELM_CHART} \
            --namespace ${NAMESPACE} \
            --set image.pullPolicy=Never \
            --set image.tag=${IMAGE_TAG}
    fi
    
    log_success "Helm deployment completed!"
}

wait_for_deployment() {
    log_info "Waiting for deployment to be ready..."
    
    if kubectl wait --for=condition=available --timeout=300s deployment/${RELEASE_NAME} -n ${NAMESPACE}; then
        log_success "Deployment is ready!"
    else
        log_error "Deployment failed to become ready within 5 minutes"
        log_info "Checking pod status..."
        kubectl get pods -n ${NAMESPACE}
        kubectl describe pods -l app.kubernetes.io/name=${RELEASE_NAME} -n ${NAMESPACE}
        exit 1
    fi
}

show_status() {
    log_info "Deployment status:"
    echo
    kubectl get all -n ${NAMESPACE}
    echo
    
    log_info "To access the application:"
    echo "kubectl port-forward svc/${RELEASE_NAME} 8080:8000 -n ${NAMESPACE}"
    echo "Then visit: http://localhost:8080"
    echo
    
    log_info "To check logs:"
    echo "kubectl logs -f deployment/${RELEASE_NAME} -n ${NAMESPACE}"
    echo
    
    log_info "To test the health endpoint:"
    echo "curl http://localhost:8080/api/healthchecker"
}

cleanup() {
    log_info "Cleaning up deployment..."
    
    if helm list -n ${NAMESPACE} | grep -q ${RELEASE_NAME}; then
        helm uninstall ${RELEASE_NAME} -n ${NAMESPACE}
        log_success "Helm release uninstalled"
    fi
    
    if kubectl get namespace ${NAMESPACE} >/dev/null 2>&1; then
        kubectl delete namespace ${NAMESPACE}
        log_success "Namespace deleted"
    fi
    
    log_info "Cleanup completed!"
}

# Main script
case "${1:-deploy}" in
    "deploy")
        log_info "Starting RustCI deployment..."
        check_prerequisites
        build_image
        create_namespace
        deploy_helm
        wait_for_deployment
        show_status
        log_success "RustCI deployed successfully! 🚀"
        ;;
    
    "build")
        log_info "Building Docker image only..."
        check_prerequisites
        build_image
        ;;
    
    "upgrade")
        log_info "Upgrading existing deployment..."
        check_prerequisites
        build_image
        deploy_helm
        wait_for_deployment
        show_status
        ;;
    
    "status")
        log_info "Checking deployment status..."
        show_status
        ;;
    
    "logs")
        log_info "Showing application logs..."
        kubectl logs -f deployment/${RELEASE_NAME} -n ${NAMESPACE}
        ;;
    
    "cleanup")
        cleanup
        ;;
    
    "help"|"-h"|"--help")
        echo "RustCI Deployment Script"
        echo
        echo "Usage: $0 [command]"
        echo
        echo "Commands:"
        echo "  deploy    - Build image and deploy to Kubernetes (default)"
        echo "  build     - Build Docker image only"
        echo "  upgrade   - Upgrade existing deployment"
        echo "  status    - Show deployment status"
        echo "  logs      - Show application logs"
        echo "  cleanup   - Remove deployment and namespace"
        echo "  help      - Show this help message"
        echo
        echo "Examples:"
        echo "  $0                # Deploy RustCI"
        echo "  $0 deploy         # Deploy RustCI"
        echo "  $0 upgrade        # Upgrade existing deployment"
        echo "  $0 cleanup        # Clean up deployment"
        ;;
    
    *)
        log_error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac