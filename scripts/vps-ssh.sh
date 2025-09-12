#!/bin/bash

# VPS SSH Helper Script
# This script handles SSH connections to the VPS with proper escaping

set -euo pipefail

# Check required environment variables
if [ -z "${VPS_IP:-}" ] || [ -z "${VPS_USERNAME:-}" ] || [ -z "${VPS_PASSWORD:-}" ]; then
    echo "❌ Missing required environment variables: VPS_IP, VPS_USERNAME, VPS_PASSWORD"
    exit 1
fi

# Function to execute SSH command
execute_ssh() {
    local command="$1"
    echo "🔗 Executing SSH command on ${VPS_USERNAME}@${VPS_IP}"
    echo "📝 Command: $command"
    
    sshpass -p "${VPS_PASSWORD}" ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "${VPS_USERNAME}@${VPS_IP}" "$command"
}

# Function to test VPS connection
test_connection() {
    echo "🔍 Testing VPS connection..."
    execute_ssh "echo 'VPS connection successful'"
}

# Function to check prerequisites
check_prerequisites() {
    echo "📋 Checking VPS prerequisites..."
    execute_ssh "
        if ! command -v docker &> /dev/null; then
            echo 'Installing Docker...'
            curl -fsSL https://get.docker.com -o get-docker.sh
            sh get-docker.sh
            systemctl enable docker
            systemctl start docker
        fi
        
        if ! command -v sshpass &> /dev/null; then
            apt-get update
            apt-get install -y sshpass
        fi
        
        mkdir -p /opt/rustci/{current,backups}
        echo 'Prerequisites check completed'
    "
}

# Function to backup current deployment
backup_deployment() {
    echo "💾 Creating backup of current deployment..."
    execute_ssh "
        if docker ps | grep rustci-production; then
            docker commit rustci-production rustci:backup-\$(date +%Y%m%d-%H%M%S)
            echo 'Backup created'
        else
            echo 'No current deployment to backup'
        fi
    "
}

# Function to deploy RustCI
deploy_rustci() {
    echo "🚀 Deploying RustCI to VPS..."
    execute_ssh "
        docker stop rustci-production 2>/dev/null || true
        docker rm rustci-production 2>/dev/null || true
        
        docker run -d --name rustci-production -p 8080:8000 \
            -e MONGODB_URI=\"${MONGODB_URI}\" \
            -e MONGODB_DATABASE=\"${MONGODB_DATABASE}\" \
            -e JWT_SECRET=\"${JWT_SECRET}\" \
            -e JWT_EXPIRED_IN=\"${JWT_EXPIRED_IN}\" \
            -e JWT_SIGNUP_EXPIRED_IN=\"${JWT_SIGNUP_EXPIRED_IN}\" \
            -e JWT_REFRESH_EXPIRED_IN=\"${JWT_REFRESH_EXPIRED_IN}\" \
            -e GITHUB_OAUTH_CLIENT_ID=\"${GITHUB_OAUTH_CLIENT_ID}\" \
            -e GITHUB_OAUTH_CLIENT_SECRET=\"${GITHUB_OAUTH_CLIENT_SECRET}\" \
            -e GITHUB_OAUTH_REDIRECT_URL=\"${GITHUB_OAUTH_REDIRECT_URL}\" \
            -e CLIENT_ORIGIN=\"${CLIENT_ORIGIN}\" \
            -e PORT=${PORT} \
            -e RUST_ENV=${RUST_ENV} \
            -e RUST_LOG=${RUST_LOG} \
            -e ENABLE_METRICS=${ENABLE_METRICS} \
            -v /var/run/docker.sock:/var/run/docker.sock \
            --restart unless-stopped \
            rustci:production
            
        echo '✅ RustCI deployment completed'
    "
}

# Function to check deployment status
check_deployment() {
    echo "📊 Checking deployment status..."
    execute_ssh "
        if docker ps | grep rustci-production; then
            echo '✅ RustCI container is running'
            docker logs --tail 20 rustci-production
        else
            echo '❌ RustCI container not found'
            docker ps -a | grep rustci || echo 'No rustci containers found'
            exit 1
        fi
    "
}

# Function to cleanup
cleanup() {
    echo "🧹 Cleaning up resources..."
    execute_ssh "
        docker ps | grep rustci-production && echo '✅ Cleanup completed - RustCI is running'
    "
}

# Function to rollback deployment
rollback_deployment() {
    echo "🔄 Rolling back deployment..."
    execute_ssh "
        BACKUP_IMAGE=\$(docker images | grep 'rustci:backup' | head -1 | awk '{print \$1\":\" \$2}')
        if [ ! -z \"\$BACKUP_IMAGE\" ]; then
            echo \"Rolling back to \$BACKUP_IMAGE\"
            docker stop rustci-production 2>/dev/null || true
            docker rm rustci-production 2>/dev/null || true
            docker run -d --name rustci-production -p 8080:8000 --restart unless-stopped \$BACKUP_IMAGE
            echo '✅ Rollback completed'
        else
            echo '❌ No backup image found for rollback'
        fi
    "
}

# Main execution based on command line argument
case "${1:-}" in
    "test")
        test_connection
        ;;
    "prerequisites")
        check_prerequisites
        ;;
    "backup")
        backup_deployment
        ;;
    "deploy")
        deploy_rustci
        ;;
    "status")
        check_deployment
        ;;
    "cleanup")
        cleanup
        ;;
    "rollback")
        rollback_deployment
        ;;
    *)
        echo "Usage: $0 {test|prerequisites|backup|deploy|status|cleanup|rollback}"
        echo ""
        echo "Commands:"
        echo "  test          - Test VPS connection"
        echo "  prerequisites - Check and install prerequisites"
        echo "  backup        - Backup current deployment"
        echo "  deploy        - Deploy RustCI"
        echo "  status        - Check deployment status"
        echo "  cleanup       - Cleanup resources"
        echo "  rollback      - Rollback to previous deployment"
        exit 1
        ;;
esac