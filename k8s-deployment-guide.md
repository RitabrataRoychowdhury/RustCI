# Kubernetes Deployment Guide for RustCI

This guide will help you deploy RustCI to Kubernetes using the provided Helm chart.

## Prerequisites

### 1. Install Required Tools

```bash
# Install Docker Desktop (includes Kubernetes)
brew install --cask docker

# Install kubectl
brew install kubectl

# Install Helm
brew install helm

# Optional: Install k9s for cluster monitoring
brew install k9s
```

### 2. Enable Kubernetes in Docker Desktop

1. Open Docker Desktop
2. Go to Settings → Kubernetes
3. Check "Enable Kubernetes"
4. Click "Apply & Restart"
5. Wait for Kubernetes to start

### 3. Verify Installation

```bash
# Check Kubernetes cluster
kubectl cluster-info

# Check nodes
kubectl get nodes

# Verify Helm
helm version
```

## Build and Deploy RustCI

### Step 1: Build Docker Image

```bash
# Build the RustCI Docker image
docker build -t rustci:latest .

# Verify the image
docker images | grep rustci
```

### Step 2: Deploy with Helm

```bash
# Create namespace (optional)
kubectl create namespace rustci

# Install RustCI using Helm
helm install rustci ./helm/rustci -n rustci

# Or install in default namespace
helm install rustci ./helm/rustci
```

### Step 3: Check Deployment Status

```bash
# Check pods
kubectl get pods -n rustci

# Check services
kubectl get svc -n rustci

# Check persistent volumes
kubectl get pvc -n rustci

# View logs
kubectl logs -f deployment/rustci -n rustci
```

### Step 4: Access the Application

```bash
# Port forward to access locally
kubectl port-forward svc/rustci 8080:8000 -n rustci

# Now access RustCI at: http://localhost:8080
```

## Configuration Options

### Environment Variables

The Helm chart includes all necessary environment variables from your `.env` file:

- MongoDB connection
- JWT secrets
- GitHub OAuth configuration
- Server settings

### Storage Configuration

The chart creates three types of persistent storage:

1. **Main Storage** (10Gi): Application data
2. **Workspace Storage** (20Gi): CI/CD workspaces
3. **Cache Storage** (10Gi): Build cache

### Resource Limits

Default resource configuration:
- **Requests**: 500m CPU, 512Mi memory
- **Limits**: 1000m CPU, 1Gi memory

## Testing Kubernetes Connector

### 1. Create a Test Pipeline

Create `test-k8s-pipeline.yaml`:

```yaml
name: "Kubernetes Test Pipeline"
description: "Test Kubernetes connector functionality"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Test Kubernetes"
    steps:
      - name: "simple-k8s-job"
        step_type: kubernetes
        config:
          image: "alpine:latest"
          command: "echo 'Hello from Kubernetes!' && sleep 10"
          namespace: "default"
        timeout: 120

      - name: "resource-limited-job"
        step_type: kubernetes
        config:
          image: "ubuntu:latest"
          script: |
            echo "Testing resource limits"
            echo "Available memory: $(free -h)"
            echo "Available CPU: $(nproc)"
            sleep 5
          namespace: "default"
          parameters:
            resource_requests:
              cpu: "100m"
              memory: "128Mi"
            resource_limits:
              cpu: "200m"
              memory: "256Mi"
        timeout: 180

environment: {}
timeout: 600
retry_count: 0
```

### 2. Upload and Test Pipeline

```bash
# Upload the pipeline
curl -X POST http://localhost:8080/api/ci/pipelines/upload \
  -F "pipeline=@test-k8s-pipeline.yaml"

# Get pipeline ID from response, then trigger it
curl -X POST http://localhost:8080/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{"trigger_type": "manual"}'
```

### 3. Monitor Execution

```bash
# Watch Kubernetes jobs
kubectl get jobs -w

# Watch pods
kubectl get pods -w

# Check logs of CI jobs
kubectl logs -l job-name=ci-{JOB_NAME}
```

## Advanced Configuration

### Enable Ingress

To expose RustCI externally:

```yaml
# values-production.yaml
ingress:
  enabled: true
  className: "nginx"
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
  hosts:
    - host: rustci.yourdomain.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: rustci-tls
      hosts:
        - rustci.yourdomain.com
```

```bash
# Deploy with custom values
helm upgrade rustci ./helm/rustci -f values-production.yaml -n rustci
```

### Enable Autoscaling

```yaml
# values-production.yaml
autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
```

### Production Storage Classes

```yaml
# values-production.yaml
persistence:
  storageClass: "fast-ssd"
  size: 50Gi

cicd:
  workspace:
    storageClass: "fast-ssd"
    size: 100Gi
  cache:
    storageClass: "standard"
    size: 50Gi
```

## Troubleshooting

### Common Issues

1. **Image Pull Errors**
   ```bash
   # Make sure image is available
   docker images | grep rustci
   
   # For local development, use imagePullPolicy: Never
   helm upgrade rustci ./helm/rustci --set image.pullPolicy=Never
   ```

2. **Permission Issues**
   ```bash
   # Check RBAC permissions
   kubectl get clusterroles | grep rustci
   kubectl get rolebindings -n rustci
   ```

3. **Storage Issues**
   ```bash
   # Check PVC status
   kubectl get pvc -n rustci
   kubectl describe pvc rustci-pvc -n rustci
   ```

4. **Pod Startup Issues**
   ```bash
   # Check pod events
   kubectl describe pod -l app.kubernetes.io/name=rustci -n rustci
   
   # Check logs
   kubectl logs -f deployment/rustci -n rustci
   ```

### Useful Commands

```bash
# Get all resources
kubectl get all -n rustci

# Delete deployment
helm uninstall rustci -n rustci

# Clean up namespace
kubectl delete namespace rustci

# Monitor with k9s
k9s -n rustci
```

## Security Considerations

1. **Secrets Management**: Use Kubernetes secrets for sensitive data
2. **RBAC**: The chart includes proper RBAC configuration
3. **Network Policies**: Consider implementing network policies for production
4. **Pod Security**: The chart uses non-root user and security contexts

## Next Steps

1. Set up monitoring with Prometheus/Grafana
2. Configure log aggregation with ELK stack
3. Implement backup strategies for persistent data
4. Set up CI/CD for the RustCI application itself
5. Configure multi-environment deployments (dev/staging/prod)

Happy Kubernetes deployment! 🚀