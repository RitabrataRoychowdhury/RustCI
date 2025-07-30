# RustCI Connector Test Suite

This directory contains comprehensive test configurations for validating both Docker and Kubernetes connectors in RustCI.

## Test Files Overview

### Core Test Files

- **`docker-connector-test.yaml`** - Comprehensive Docker connector testing
- **`kubernetes-connector-test.yaml`** - Kubernetes connector testing with k3s/k3d
- **`integration-test-suite.yaml`** - Complete integration test suite for both connectors

### K3D Setup Files

- **`k3d-setup.yaml`** - Automated k3d cluster creation and configuration
- **`k3d-teardown.yaml`** - Clean cluster removal and cleanup

## Prerequisites

### For Docker Connector Tests
- Docker Engine installed and running
- Docker CLI accessible

### For Kubernetes Connector Tests
- kubectl installed and configured
- Either k3d or k3s installed:
  - **k3d**: Lightweight Kubernetes in Docker (recommended for testing)
  - **k3s**: Lightweight Kubernetes distribution

### Installing k3d (Recommended)
```bash
# Install k3d
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

# Verify installation
k3d version
```

### Installing k3s (Alternative)
```bash
# Install k3s
curl -sfL https://get.k3s.io | sh -

# Verify installation
sudo k3s kubectl get nodes
```

## Running Tests

### Quick Start with k3d

1. **Setup k3d cluster:**
   ```bash
   # Run the k3d setup pipeline
   rustci run tests/k3d-setup.yaml
   ```

2. **Run Docker connector tests:**
   ```bash
   rustci run tests/docker-connector-test.yaml
   ```

3. **Run Kubernetes connector tests:**
   ```bash
   rustci run tests/kubernetes-connector-test.yaml
   ```

4. **Run complete integration suite:**
   ```bash
   rustci run tests/integration-test-suite.yaml
   ```

5. **Cleanup k3d cluster:**
   ```bash
   rustci run tests/k3d-teardown.yaml
   ```

### Manual k3d Cluster Management

```bash
# Create cluster manually
k3d cluster create rustci-test --agents 2 --port "8080:80@loadbalancer"

# List clusters
k3d cluster list

# Delete cluster
k3d cluster delete rustci-test
```

## Test Scenarios Covered

### Docker Connector Tests

1. **Image Building**
   - Dockerfile creation and validation
   - Multi-stage builds
   - Build context handling
   - Build arguments and environment variables

2. **Container Operations**
   - Container creation and startup
   - Port mapping and networking
   - Environment variable injection
   - Volume mounting

3. **Container Management**
   - Container execution (docker exec)
   - Log retrieval and monitoring
   - Container inspection
   - Graceful shutdown and cleanup

4. **Error Handling**
   - Failed builds
   - Container startup failures
   - Network connectivity issues
   - Resource cleanup on failure

### Kubernetes Connector Tests

1. **Cluster Operations**
   - Cluster connectivity verification
   - Namespace management
   - Resource validation

2. **Workload Deployment**
   - Deployment creation and management
   - Pod scheduling and readiness
   - Service discovery and networking
   - ConfigMap and Secret handling

3. **Scaling and Management**
   - Horizontal pod scaling
   - Rolling updates
   - Health checks and probes
   - Resource monitoring

4. **Advanced Features**
   - Multi-container pods
   - Volume mounting
   - Environment variable injection
   - Service mesh integration (if available)

### Integration Test Suite

The integration test suite combines both connectors to test:

1. **Cross-connector workflows**
   - Build with Docker, deploy to Kubernetes
   - Image registry integration
   - Multi-stage CI/CD pipelines

2. **Resource management**
   - Shared resource cleanup
   - Namespace isolation
   - Resource quotas and limits

3. **Error recovery**
   - Partial failure handling
   - Rollback scenarios
   - Emergency cleanup procedures

## Test Configuration Variables

### Common Variables
- `TIMESTAMP` - Auto-generated timestamp for unique resource naming
- `TEST_SUITE_ID` - Unique identifier for test runs

### Docker-specific Variables
- `IMAGE_NAME` - Docker image name for testing
- `IMAGE_TAG` - Docker image tag
- `CONTAINER_NAME` - Container name for testing

### Kubernetes-specific Variables
- `NAMESPACE` - Kubernetes namespace for test resources
- `APP_NAME` - Application name for Kubernetes resources
- `CLUSTER_NAME` - k3d cluster name

## Troubleshooting

### Common Issues

1. **k3d cluster creation fails**
   ```bash
   # Check Docker is running
   docker ps
   
   # Clean up existing clusters
   k3d cluster delete rustci-test
   ```

2. **Image not found in k3d**
   ```bash
   # Import image to k3d cluster
   k3d image import your-image:tag
   ```

3. **kubectl connection issues**
   ```bash
   # Check cluster context
   kubectl config current-context
   
   # Get cluster info
   kubectl cluster-info
   ```

4. **Port conflicts**
   ```bash
   # Check for port usage
   lsof -i :8080
   
   # Kill processes using ports
   kill -9 $(lsof -t -i:8080)
   ```

### Debug Commands

```bash
# Check k3d cluster status
k3d cluster list

# Check Kubernetes resources
kubectl get all -A

# Check Docker containers
docker ps -a

# Check Docker images
docker images

# View RustCI logs
rustci logs --follow
```

## Test Results and Reporting

Each test pipeline includes:

- **Success indicators** - Clear pass/fail status for each step
- **Detailed logging** - Comprehensive output for debugging
- **Resource cleanup** - Automatic cleanup on success or failure
- **Error reporting** - Structured error messages with context

Test results are logged to the RustCI execution logs and can be viewed through the web interface or CLI.

## Contributing

When adding new tests:

1. Follow the existing YAML structure
2. Include comprehensive cleanup steps
3. Add appropriate error handling
4. Document any new prerequisites
5. Test with both k3d and k3s when possible
6. Include variable documentation

## Security Considerations

- Test images use minimal base images (Alpine, nginx:alpine)
- No sensitive data in test configurations
- Temporary resources are properly cleaned up
- Network isolation through Kubernetes namespaces
- Resource limits to prevent resource exhaustion