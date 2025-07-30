#!/bin/bash

# stopAndErase.sh - Cleanup script for RustCI deployment
# This script removes all resources created by deploy.sh

set -e

echo "[INFO] Starting cleanup of RustCI deployment..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    print_error "kubectl is not installed or not in PATH"
    exit 1
fi

# Check if helm is available
if ! command -v helm &> /dev/null; then
    print_error "helm is not installed or not in PATH"
    exit 1
fi

# Check if docker is available
if ! command -v docker &> /dev/null; then
    print_error "docker is not installed or not in PATH"
    exit 1
fi

print_info "Checking current Kubernetes context..."
CURRENT_CONTEXT=$(kubectl config current-context)
print_info "Current context: $CURRENT_CONTEXT"

# Uninstall Helm release
print_info "Uninstalling Helm release: rustci"
if helm list -n rustci | grep -q rustci; then
    helm uninstall rustci -n rustci
    print_status "Helm release 'rustci' uninstalled"
else
    print_info "Helm release 'rustci' not found, skipping..."
fi

# Wait for pods to terminate
print_info "Waiting for pods to terminate..."
kubectl wait --for=delete pods --all -n rustci --timeout=60s || true

# Delete persistent volume claims
print_info "Deleting persistent volume claims..."
if kubectl get pvc -n rustci &> /dev/null; then
    kubectl delete pvc --all -n rustci || true
    print_status "Persistent volume claims deleted"
else
    print_info "No persistent volume claims found"
fi

# Delete secrets
print_info "Deleting secrets..."
if kubectl get secrets -n rustci &> /dev/null; then
    kubectl delete secrets --all -n rustci || true
    print_status "Secrets deleted"
else
    print_info "No secrets found"
fi

# Delete service accounts and RBAC
print_info "Deleting service accounts and RBAC resources..."
if kubectl get serviceaccount -n rustci &> /dev/null; then
    kubectl delete serviceaccount --all -n rustci || true
fi
if kubectl get rolebinding -n rustci &> /dev/null; then
    kubectl delete rolebinding --all -n rustci || true
fi
if kubectl get role -n rustci &> /dev/null; then
    kubectl delete role --all -n rustci || true
fi

# Delete cluster role bindings (if any were created)
print_info "Checking for cluster role bindings..."
if kubectl get clusterrolebinding | grep -q rustci; then
    kubectl delete clusterrolebinding $(kubectl get clusterrolebinding | grep rustci | awk '{print $1}') || true
    print_status "Cluster role bindings deleted"
fi

# Delete cluster roles (if any were created)
if kubectl get clusterrole | grep -q rustci; then
    kubectl delete clusterrole $(kubectl get clusterrole | grep rustci | awk '{print $1}') || true
    print_status "Cluster roles deleted"
fi

# Delete namespace
print_info "Deleting namespace: rustci"
if kubectl get namespace rustci &> /dev/null; then
    kubectl delete namespace rustci
    print_status "Namespace 'rustci' deleted"
else
    print_info "Namespace 'rustci' not found, skipping..."
fi

# Remove Docker images
print_info "Removing Docker images..."

# Remove the built rustci image
if docker images | grep -q "rustci"; then
    print_info "Removing rustci Docker images..."
    docker rmi $(docker images rustci -q) --force || true
    print_status "RustCI Docker images removed"
else
    print_info "No rustci Docker images found"
fi

# Remove dangling images
print_info "Removing dangling Docker images..."
if docker images -f "dangling=true" -q | grep -q .; then
    docker rmi $(docker images -f "dangling=true" -q) || true
    print_status "Dangling Docker images removed"
else
    print_info "No dangling Docker images found"
fi

# Clean up Docker build cache
print_info "Cleaning Docker build cache..."
docker builder prune -f || true
print_status "Docker build cache cleaned"

# Clean up unused Docker volumes
print_info "Cleaning unused Docker volumes..."
if docker volume ls -q | grep -q .; then
    docker volume prune -f || true
    print_status "Unused Docker volumes cleaned"
else
    print_info "No unused Docker volumes found"
fi

# Clean up unused Docker networks
print_info "Cleaning unused Docker networks..."
docker network prune -f || true
print_status "Unused Docker networks cleaned"

# Final verification
print_info "Verifying cleanup..."

# Check if namespace still exists
if kubectl get namespace rustci &> /dev/null; then
    print_error "Namespace 'rustci' still exists"
else
    print_status "Namespace 'rustci' successfully removed"
fi

# Check if Docker images still exist
if docker images | grep -q "rustci"; then
    print_error "Some rustci Docker images may still exist"
    docker images | grep rustci || true
else
    print_status "All rustci Docker images removed"
fi

# Show Docker system usage
print_info "Current Docker system usage:"
docker system df

print_status "Cleanup completed successfully!"
print_info "All RustCI deployment resources have been removed."
print_info "If you want to completely reset Docker, you can also run: docker system prune -a --volumes"