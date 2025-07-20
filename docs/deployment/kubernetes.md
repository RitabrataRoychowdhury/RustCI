# Kubernetes Deployment Guide

This guide shows how to deploy RustCI to Kubernetes using the provided Helm chart and how to use the Kubernetes connector for pipeline execution.

## Prerequisites

### Required Tools

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

### Enable Kubernetes

**Docker Desktop:**
1. Open Docker Desktop
2. Go to **Settings** → **Kubernetes**
3. Check **"Enable Kubernetes"**
4. Click **"Apply & Restart"**
5. Wait for Kubernetes to start (green indicator)

**Minikube (Alternative):**
```bash
# Install minikube
brew install minikube

# Start minikube
minikube start

# Enable ingress addon
minikube addons enable ingress
```

### Verify Setup

```bash
# Check Kubernetes cluster
kubectl cluster-info

# Check nodes
kubectl get nodes

# Verify Helm
helm version
```

## Deploying RustCI to Kubernetes

### Step 1: Build Docker Image

```bash
# Navigate to project root
cd /path/to/rustci

# Build the Docker image
docker build -t rustci:latest .

# Verify image
docker images | grep rustci
```

### Step 2: Deploy with Helm

```bash
# Create namespace (recommended)
kubectl create namespace rustci

# Deploy RustCI
helm install rustci ./helm/rustci -n rustci

# Or with custom values for local development
helm install rustci ./helm/rustci \
  --set image.pullPolicy=Never \
  --set service.type=NodePort \
  -n rustci
```

### Step 3: Verify Deployment

```bash
# Check all resources
kubectl get all -n rustci

# Check pods
kubectl get pods -n rustci

# Check services
kubectl get svc -n rustci

# Check persistent volumes
kubectl get pvc -n rustci
```

### Step 4: Access RustCI

```bash
# Port forward to access locally
kubectl port-forward svc/rustci 8080:8000 -n rustci

# Access RustCI at: http://localhost:8080
# Health check: http://localhost:8080/api/healthchecker
```

## Configuration Options

### Local Development Values

Create `values-local.yaml`:

```yaml
# values-local.yaml
image:
  pullPolicy: Never  # Use local image

service:
  type: NodePort     # Easy local access

persistence:
  enabled: true
  size: 5Gi         # Smaller for local

cicd:
  workspace:
    size: 10Gi      # Smaller workspace
  cache:
    size: 5Gi       # Smaller cache

resources:
  requests:
    cpu: 200m       # Lower requirements
    memory: 256Mi
  limits:
    cpu: 500m
    memory: 512Mi
```

Deploy with local values:
```bash
helm install rustci ./helm/rustci -f values-local.yaml -n rustci
```

### Production Values

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

## Using the Kubernetes Connector

The Kubernetes connector allows RustCI to execute pipeline steps as Kubernetes Jobs.

### Basic Kubernetes Pipeline

Create `k8s-pipeline.yaml`:

```yaml
name: "Kubernetes Pipeline"
description: "Execute steps in Kubernetes"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Kubernetes Jobs"
    steps:
      - name: "simple-job"
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

### Advanced Kubernetes Features

```yaml
name: "Advanced Kubernetes Pipeline"
description: "Advanced K8s features with PVC and lifecycle hooks"

stages:
  - name: "Build with PVC"
    steps:
      - name: "maven-build"
        step_type: kubernetes
        config:
          image: "maven:3.8-openjdk-17"
          script: |
            echo "Building with Maven"
            mvn clean package -DskipTests
            echo "Build completed"
          namespace: "ci-cd"
          parameters:
            use_pvc: true
            storage_class: "fast-ssd"
            storage_size: "20Gi"
            resource_requests:
              cpu: "500m"
              memory: "1Gi"
            resource_limits:
              cpu: "2"
              memory: "4Gi"
            service_account: "ci-service-account"
        timeout: 1200

  - name: "Deploy"
    steps:
      - name: "kubectl-deploy"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl apply -f k8s/deployment.yaml
            kubectl apply -f k8s/service.yaml
            kubectl rollout status deployment/myapp --timeout=300s
          namespace: "production"
          parameters:
            service_account: "deployment-sa"
        timeout: 600
```

### Multi-Cloud Kubernetes

```yaml
name: "Multi-Cloud Kubernetes"
description: "Deploy to multiple K8s clusters"

stages:
  - name: "Deploy to AWS EKS"
    steps:
      - name: "deploy-eks"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl config use-context aws-eks-prod
            kubectl set image deployment/myapp myapp=myapp:${BUILD_TAG}
            kubectl rollout status deployment/myapp
          namespace: "production"
          parameters:
            service_account: "eks-deployment-sa"
        timeout: 900

  - name: "Deploy to GCP GKE"
    steps:
      - name: "deploy-gke"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl config use-context gcp-gke-prod
            kubectl set image deployment/myapp myapp=gcr.io/project/myapp:${BUILD_TAG}
            kubectl rollout status deployment/myapp
          namespace: "production"
          parameters:
            service_account: "gke-deployment-sa"
        timeout: 900

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
```

## Testing Kubernetes Connector

### Upload and Test Pipeline

```bash
# Upload the Kubernetes pipeline
curl -X POST http://localhost:8080/api/ci/pipelines/upload \
  -F "pipeline=@k8s-pipeline.yaml"

# Get pipeline ID from response, then trigger
curl -X POST http://localhost:8080/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{"trigger_type": "manual"}'
```

### Monitor Execution

```bash
# Watch Kubernetes jobs created by RustCI
kubectl get jobs -w

# Watch pods
kubectl get pods -w

# Check logs of RustCI application
kubectl logs -f deployment/rustci -n rustci

# Check logs of CI jobs (replace job-name)
kubectl logs job/ci-{job-name}
```

## Kubernetes Connector Configuration

### Available Parameters

```yaml
step_type: kubernetes
config:
  image: "ubuntu:latest"              # Container image
  command: "echo 'Hello K8s'"         # Command to execute
  script: |                           # Multi-line script
    echo "Starting K8s job"
    kubectl get pods
  namespace: "default"                # Kubernetes namespace
  parameters:
    use_pvc: true                     # Use PersistentVolumeClaim
    storage_class: "fast-ssd"         # Storage class for PVC
    storage_size: "10Gi"              # PVC size
    service_account: "ci-sa"          # Service account for RBAC
    resource_requests:                # Resource requests
      cpu: "100m"
      memory: "128Mi"
    resource_limits:                  # Resource limits
      cpu: "500m"
      memory: "512Mi"
```

### RBAC Configuration

The Helm chart includes proper RBAC configuration:

```yaml
# ServiceAccount
apiVersion: v1
kind: ServiceAccount
metadata:
  name: rustci
  namespace: rustci

---
# ClusterRole
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: rustci
rules:
- apiGroups: [""]
  resources: ["pods", "pods/log", "persistentvolumeclaims"]
  verbs: ["get", "list", "create", "delete", "watch"]
- apiGroups: ["batch"]
  resources: ["jobs"]
  verbs: ["get", "list", "create", "delete", "watch"]

---
# ClusterRoleBinding
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: rustci
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: rustci
subjects:
- kind: ServiceAccount
  name: rustci
  namespace: rustci
```

## Monitoring and Debugging

### View Logs

```bash
# RustCI application logs
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

## Common Operations

### Update Deployment

```bash
# Rebuild image
docker build -t rustci:latest .

# Upgrade Helm deployment
helm upgrade rustci ./helm/rustci -n rustci

# Force pod restart
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

## Cleanup

### Remove Deployment

```bash
# Uninstall Helm release
helm uninstall rustci -n rustci

# Delete namespace (removes everything)
kubectl delete namespace rustci

# Clean up local Docker image
docker rmi rustci:latest
```

## Troubleshooting

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

4. **RBAC Issues**
   ```bash
   # Check service account
   kubectl get sa -n rustci
   
   # Check role bindings
   kubectl get rolebindings,clusterrolebindings | grep rustci
   ```

5. **Network Issues**
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

# Check cluster info
kubectl cluster-info dump
```

## Production Considerations

### Security

1. **Use non-root containers**
2. **Implement network policies**
3. **Use secrets for sensitive data**
4. **Enable RBAC with minimal permissions**
5. **Scan images for vulnerabilities**

### Performance

1. **Set appropriate resource limits**
2. **Use horizontal pod autoscaling**
3. **Configure persistent volume performance**
4. **Monitor resource usage**

### High Availability

1. **Deploy across multiple nodes**
2. **Use pod disruption budgets**
3. **Configure health checks**
4. **Implement backup strategies**

## Next Steps

1. **Set up monitoring** with Prometheus/Grafana
2. **Configure log aggregation** with ELK stack
3. **Implement backup strategies** for persistent data
4. **Set up CI/CD** for RustCI itself
5. **Configure multi-environment** deployments

Your Kubernetes deployment is now ready for production! 🚀