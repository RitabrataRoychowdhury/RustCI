# RustCI Kubernetes Deployment Guide

This guide shows how to deploy your RustCI application to Kubernetes using the Helm chart.

## 📋 Prerequisites

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
2. Go to **Settings** → **Kubernetes**
3. Check **"Enable Kubernetes"**
4. Click **"Apply & Restart"**
5. Wait for Kubernetes to start (green indicator)

### 3. Verify Setup

```bash
# Check Kubernetes cluster
kubectl cluster-info

# Should show:
# Kubernetes control plane is running at https://kubernetes.docker.internal:6443

# Check nodes
kubectl get nodes

# Should show:
# NAME             STATUS   ROLES           AGE   VERSION
# docker-desktop   Ready    control-plane   1m    v1.28.x

# Verify Helm
helm version
```

## 🔨 Build and Deploy Process

### Step 1: Build Your Application

```bash
# Navigate to your project root (where Cargo.toml is)
cd /path/to/your/RustCI

# Build the Docker image
docker build -t rustci:latest .

# Verify the image was created
docker images | grep rustci
```

### Step 2: Deploy with Helm

```bash
# Option A: Deploy to default namespace
helm install rustci ./helm/rustci

# Option B: Deploy to dedicated namespace (recommended)
kubectl create namespace rustci
helm install rustci ./helm/rustci -n rustci

# Option C: Deploy with custom values for local development
helm install rustci ./helm/rustci \
  --set image.pullPolicy=Never \
  --set service.type=NodePort \
  -n rustci
```

### Step 3: Check Deployment Status

```bash
# Check all resources
kubectl get all -n rustci

# Check pods specifically
kubectl get pods -n rustci

# Should show something like:
# NAME                      READY   STATUS    RESTARTS   AGE
# rustci-xxxxxxxxxx-xxxxx   1/1     Running   0          2m

# Check services
kubectl get svc -n rustci

# Check persistent volumes
kubectl get pvc -n rustci
```

### Step 4: Access Your Application

```bash
# Port forward to access locally
kubectl port-forward svc/rustci 8080:8000 -n rustci

# Now access RustCI at: http://localhost:8080
# Health check: http://localhost:8080/api/healthchecker
```

## 🧪 Testing Your Deployment

### 1. Health Check

```bash
# Test health endpoint
curl http://localhost:8080/api/healthchecker

# Expected response:
# {"status":"success","message":"Build Simple CRUD API with Rust and Axum"}
```

### 2. Upload Test Pipeline

```bash
# Upload the Kubernetes test pipeline
curl -X POST http://localhost:8080/api/ci/pipelines/upload \
  -F "pipeline=@examples/k8s-test-pipeline.yaml"

# Response will include pipeline ID
```

### 3. Trigger Pipeline Execution

```bash
# Replace {PIPELINE_ID} with actual ID from step 2
curl -X POST http://localhost:8080/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{"trigger_type": "manual"}'
```

### 4. Monitor Execution

```bash
# Watch Kubernetes jobs created by your CI system
kubectl get jobs -w

# Watch pods
kubectl get pods -w

# Check logs of your RustCI application
kubectl logs -f deployment/rustci -n rustci

# Check logs of CI jobs (replace job-name)
kubectl logs job/ci-{job-name}
```

## 🔧 Configuration Options

### Local Development Configuration

Create `values-local.yaml`:

```yaml
# values-local.yaml
image:
  pullPolicy: Never  # Use local image, don't pull from registry

service:
  type: NodePort     # Expose via NodePort for easy access

persistence:
  enabled: true
  size: 5Gi         # Smaller size for local testing

cicd:
  workspace:
    size: 10Gi      # Smaller workspace for local
  cache:
    size: 5Gi       # Smaller cache for local

resources:
  requests:
    cpu: 200m       # Lower resource requirements
    memory: 256Mi
  limits:
    cpu: 500m
    memory: 512Mi
```

Deploy with local values:
```bash
helm install rustci ./helm/rustci -f values-local.yaml -n rustci
```

### Production Configuration

Create `values-production.yaml`:

```yaml
# values-production.yaml
replicaCount: 3

image:
  repository: your-registry.com/rustci
  tag: "v1.0.0"

service:
  type: LoadBalancer

ingress:
  enabled: true
  className: "nginx"
  hosts:
    - host: rustci.yourdomain.com
      paths:
        - path: /
          pathType: Prefix

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

autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70

resources:
  requests:
    cpu: 1000m
    memory: 1Gi
  limits:
    cpu: 2000m
    memory: 2Gi
```

## 🔍 Monitoring and Debugging

### View Logs

```bash
# Application logs
kubectl logs -f deployment/rustci -n rustci

# Previous container logs (if pod restarted)
kubectl logs deployment/rustci -n rustci --previous

# All pod logs with labels
kubectl logs -l app.kubernetes.io/name=rustci -n rustci
```

### Debug Pod Issues

```bash
# Describe pod for events and status
kubectl describe pod -l app.kubernetes.io/name=rustci -n rustci

# Get into running pod for debugging
kubectl exec -it deployment/rustci -n rustci -- /bin/sh

# Check resource usage
kubectl top pods -n rustci
```

### Check Storage

```bash
# Check PVC status
kubectl get pvc -n rustci
kubectl describe pvc rustci-pvc -n rustci

# Check storage usage inside pod
kubectl exec deployment/rustci -n rustci -- df -h
```

## 🛠️ Common Operations

### Update Deployment

```bash
# After making code changes, rebuild image
docker build -t rustci:latest .

# Upgrade Helm deployment
helm upgrade rustci ./helm/rustci -n rustci

# Force pod restart to use new image
kubectl rollout restart deployment/rustci -n rustci
```

### Scale Application

```bash
# Scale manually
kubectl scale deployment rustci --replicas=3 -n rustci

# Or update via Helm
helm upgrade rustci ./helm/rustci --set replicaCount=3 -n rustci
```

### Backup and Restore

```bash
# Backup persistent data
kubectl exec deployment/rustci -n rustci -- tar czf - /app/data > rustci-backup.tar.gz

# Restore (be careful!)
kubectl exec -i deployment/rustci -n rustci -- tar xzf - -C / < rustci-backup.tar.gz
```

## 🧹 Cleanup

### Remove Deployment

```bash
# Uninstall Helm release
helm uninstall rustci -n rustci

# Delete namespace (removes everything)
kubectl delete namespace rustci

# Clean up local Docker image
docker rmi rustci:latest
```

## 🚨 Troubleshooting

### Common Issues

1. **Image Pull Errors**
   ```bash
   # Use local image for development
   helm upgrade rustci ./helm/rustci --set image.pullPolicy=Never -n rustci
   ```

2. **Pod Won't Start**
   ```bash
   # Check events
   kubectl describe pod -l app.kubernetes.io/name=rustci -n rustci
   
   # Check logs
   kubectl logs -l app.kubernetes.io/name=rustci -n rustci
   ```

3. **Storage Issues**
   ```bash
   # Check PVC status
   kubectl get pvc -n rustci
   
   # Check storage class
   kubectl get storageclass
   ```

4. **Network Issues**
   ```bash
   # Test service connectivity
   kubectl exec deployment/rustci -n rustci -- curl localhost:8000/api/healthchecker
   
   # Check service endpoints
   kubectl get endpoints -n rustci
   ```

### Useful Commands

```bash
# Get all resources in namespace
kubectl get all -n rustci

# Watch all resources
kubectl get all -w -n rustci

# Port forward for debugging
kubectl port-forward svc/rustci 8080:8000 -n rustci

# Monitor with k9s
k9s -n rustci
```

## 🎯 Next Steps

1. **Set up CI/CD** for the RustCI application itself
2. **Configure monitoring** with Prometheus/Grafana
3. **Set up log aggregation** with ELK stack
4. **Implement backup strategies** for persistent data
5. **Configure multi-environment** deployments (dev/staging/prod)

Your Helm chart structure is **industry standard** and production-ready! 🚀