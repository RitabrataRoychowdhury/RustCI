# Pipeline Configuration Guide

## Overview

This guide explains the updated RustCI deployment pipeline configuration that addresses the deployment architecture issues identified in the system.

## Key Improvements

### 1. Security Enhancements

- **Environment Variables**: All sensitive data (passwords, secrets, API keys) are now loaded from environment variables instead of being hardcoded
- **Credential Management**: Sensitive values must be set externally and are not committed to version control
- **Configuration Files**: Environment-specific configuration files in `deployments/environments/`

### 2. Docker Image Strategy

- **Local Build**: Docker images are built locally instead of cloning repositories on the VPS
- **Secure Transfer**: Images are transferred using Docker save/load with gzip compression
- **Binary Verification**: Ensures the correct `rustci` binary is used (not `RustAutoDevOps`)

### 3. Enhanced Error Handling

- **Retry Logic**: Improved retry mechanisms for network operations and health checks
- **Rollback Support**: Automatic rollback capability when deployments fail
- **Backup Creation**: Creates backup images before deployment for rollback purposes

### 4. Health Check Improvements

- **Multiple Endpoints**: Tests both `/api/healthchecker` and `/health` endpoints
- **Extended Timeouts**: Longer wait times for application startup
- **Comprehensive Validation**: Checks container status, logs, and endpoint responses

## Configuration Files

### Environment Configuration

Location: `deployments/environments/production.env`

```bash
# VPS Configuration
VPS_IP=46.37.122.118
VPS_USERNAME=root
# VPS_PASSWORD=<set-this-externally>

# RustCI Application Configuration
# MONGODB_URI=<set-this-externally>
MONGODB_DATABASE=dqms
# JWT_SECRET=<set-this-externally>
# ... other configuration
```

### Pipeline Configuration

Location: `pipeline.yaml`

Key sections:
- **Environment Variables**: Uses `${VAR_NAME}` syntax for external configuration
- **Pre-deployment**: Validates environment and VPS connectivity
- **Build**: Local Docker build with validation
- **Deploy**: Secure image transfer and deployment
- **Health Check**: Comprehensive endpoint testing
- **Rollback**: Automatic rollback on failure

## Usage

### Setting Environment Variables

Before running the pipeline, set the required environment variables:

```bash
export VPS_PASSWORD="your-vps-password"
export MONGODB_URI="your-mongodb-connection-string"
export JWT_SECRET="your-jwt-secret"
export GITHUB_OAUTH_CLIENT_ID="your-github-client-id"
export GITHUB_OAUTH_CLIENT_SECRET="your-github-client-secret"
export GITHUB_OAUTH_REDIRECT_URL="your-redirect-url"
export CLIENT_ORIGIN="your-client-origin"
```

### Running the Pipeline

1. **Using RustCI CLI** (if available):
   ```bash
   rustci pipeline run --file pipeline.yaml --env production
   ```

2. **Using Deployment Script**:
   ```bash
   ./deployments/scripts/deploy.sh production blue-green
   ```

### Health Checks

Run health checks after deployment:

```bash
./deployments/scripts/health-check.sh production
```

## Pipeline Stages

### 1. Pre-deployment
- Validates environment variables
- Tests VPS connectivity
- Checks prerequisites (Docker, sshpass, etc.)

### 2. Build
- Validates workspace (Dockerfile, Cargo.toml)
- Builds Docker image locally
- Validates image by running test container

### 3. Deploy
- Creates backup of current deployment
- Transfers Docker image to VPS
- Deploys new RustCI container with proper configuration

### 4. Health Check
- Waits for application startup
- Tests primary health endpoint (`/api/healthchecker`)
- Tests fallback health endpoint (`/health`)
- Verifies container status

### 5. Post-deployment
- Final health verification
- Cleanup of resources
- Success confirmation

## Error Handling and Rollback

### Automatic Rollback

The pipeline includes automatic rollback functionality:

```yaml
rollback:
  enabled: true
  steps:
    - name: "rollback-deployment"
      step_type: shell
      config:
        command: "echo '🔄 Rolling back deployment...' && ..."
```

### Failure Scenarios

1. **Build Failures**: Pipeline stops at build stage
2. **Transfer Failures**: Retries image transfer up to 3 times
3. **Health Check Failures**: Triggers automatic rollback
4. **Configuration Errors**: Validates configuration before deployment

## Security Considerations

### Credential Management

- Never commit sensitive values to version control
- Use environment variables for all secrets
- Consider using secret management systems in production

### Network Security

- SSH connections use proper timeout and retry settings
- Docker image transfer uses secure SSH tunneling
- Health checks use appropriate timeouts

### Container Security

- Containers run as non-root user (configured in Dockerfile)
- Proper resource limits and restart policies
- Security scanning of Docker images (recommended)

## Troubleshooting

### Common Issues

1. **Environment Variables Not Set**
   ```
   Error: VPS_PASSWORD not set
   Solution: Export required environment variables
   ```

2. **VPS Connection Failed**
   ```
   Error: Cannot connect to VPS
   Solution: Check VPS_IP, credentials, and network connectivity
   ```

3. **Health Check Timeout**
   ```
   Error: Health endpoints not responding
   Solution: Check application logs, increase timeout, verify endpoints
   ```

### Debugging Commands

```bash
# Check container status
docker ps | grep rustci

# View container logs
docker logs rustci-production

# Test health endpoints manually
curl -f http://VPS_IP:8080/api/healthchecker
curl -f http://VPS_IP:8080/health

# Check system resources
docker stats
```

## Migration from Old Configuration

### Changes Made

1. **Removed Hardcoded Credentials**: All sensitive data moved to environment variables
2. **Enhanced Error Handling**: Added retry logic and rollback mechanisms
3. **Improved Health Checks**: Multiple endpoints with better timeout handling
4. **Security Improvements**: Proper credential management and validation

### Migration Steps

1. Set up environment variables for your deployment
2. Update any custom deployment scripts to use new configuration
3. Test deployment in staging environment first
4. Monitor health checks and logs during deployment

## Best Practices

1. **Environment Variables**: Always use environment variables for sensitive data
2. **Testing**: Test deployments in staging before production
3. **Monitoring**: Monitor health endpoints and application logs
4. **Backups**: Ensure backup creation is working before deployment
5. **Rollback**: Test rollback procedures regularly
6. **Security**: Regularly rotate secrets and credentials