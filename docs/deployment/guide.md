# RustCI Deployment Guide

This comprehensive guide covers all deployment options for RustCI, from local development to production Kubernetes clusters.

## Overview

RustCI supports multiple deployment strategies:
- **Local Development**: Direct binary execution
- **Docker**: Containerized deployment
- **Kubernetes**: Scalable cloud deployment
- **Hybrid**: Combined local and container deployment

## Quick Start

### Local Development Setup

```bash
# Clone and build
git clone https://github.com/your-org/rustci.git
cd rustci
cargo build --release

# Configure environment
cp .env.example .env
# Edit .env with your settings

# Start MongoDB
brew services start mongodb-community
# or
sudo systemctl start mongod

# Run RustCI
./target/release/rustci
```

### Docker Deployment

```bash
# Using docker-compose (recommended)
docker-compose up -d

# Or build and run manually
docker build -t rustci:latest .
docker run -d --name rustci -p 8000:8000 rustci:latest
```

### Kubernetes Deployment

```bash
# Using Helm chart
helm install rustci ./helm/rustci

# With custom namespace
kubectl create namespace rustci
helm install rustci ./helm/rustci -n rustci
```

## Environment Configuration

### Required Environment Variables

```env
# Database
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=rustci

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8000

# Authentication
JWT_SECRET=your-super-secret-jwt-key-here
JWT_EXPIRES_IN=24h

# GitHub OAuth (Optional)
GITHUB_OAUTH_CLIENT_ID=your-github-client-id
GITHUB_OAUTH_CLIENT_SECRET=your-github-client-secret
GITHUB_OAUTH_REDIRECT_URL=http://localhost:8000/api/sessions/oauth/github/callback

# Logging
RUST_LOG=info
```

### Optional Configuration

```env
# Docker Integration
DOCKER_HOST=unix:///var/run/docker.sock

# Kubernetes Integration
KUBECONFIG=/path/to/your/kubeconfig

# Performance Tuning
WORKER_THREADS=4
MAX_CONNECTIONS=100
```

## CI/CD Pipeline Features

### Repository Integration

RustCI supports automatic repository cloning with GitHub OAuth:

```yaml
# Pipeline with repository cloning
name: "Build and Deploy"
description: "Clone, build, and deploy application"

triggers:
  - trigger_type: git_push
    config:
      branch_patterns: ["main", "develop"]

stages:
  - name: "Clone"
    steps:
      - name: "clone-repo"
        step_type: github
        config:
          repository_url: "https://github.com/user/repo"
          branch: "main"

  - name: "Build"
    steps:
      - name: "build-app"
        step_type: shell
        config:
          command: "npm ci && npm run build"
```

### Multi-Language Support

Automatic project detection and building:

- **Node.js**: Detected by `package.json`
- **Python**: Detected by `requirements.txt` or `pyproject.toml`
- **Rust**: Detected by `Cargo.toml`
- **Java**: Detected by `pom.xml` or `build.gradle`
- **Go**: Detected by `go.mod`
- **.NET**: Detected by `*.csproj` or `*.sln`

### Deployment Types

#### Local Directory Deployment

```yaml
- name: "deploy-local"
  step_type: custom
  config:
    deployment_type: "directory"
    target_directory: "/opt/my-app"
```

#### Docker Container Deployment

```yaml
- name: "deploy-docker"
  step_type: docker
  config:
    image: "my-app"
    dockerfile: "Dockerfile"
    environment:
      PORTS: "8080:8080"
```

#### Kubernetes Deployment

```yaml
- name: "deploy-k8s"
  step_type: kubernetes
  config:
    image: "my-app:latest"
    namespace: "production"
    parameters:
      resource_requests:
        cpu: "100m"
        memory: "128Mi"
```

## Production Deployment

### Docker Production Setup

Create `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  rustci:
    image: rustci:latest
    ports:
      - "8000:8000"
    environment:
      - MONGODB_URI=mongodb://mongo:27017
      - RUST_LOG=info
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - rustci_data:/app/data
    depends_on:
      - mongo
    restart: unless-stopped

  mongo:
    image: mongo:6.0
    ports:
      - "27017:27017"
    volumes:
      - mongo_data:/data/db
    restart: unless-stopped

volumes:
  rustci_data:
  mongo_data:
```

Deploy:

```bash
docker-compose -f docker-compose.prod.yml up -d
```

### Kubernetes Production Setup

Create `values-production.yaml`:

```yaml
replicaCount: 3

image:
  repository: your-registry.com/rustci
  tag: "v1.0.0"
  pullPolicy: IfNotPresent

service:
  type: LoadBalancer
  port: 8000

ingress:
  enabled: true
  className: "nginx"
  annotations:
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

persistence:
  enabled: true
  storageClass: "fast-ssd"
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

mongodb:
  enabled: true
  auth:
    enabled: true
    rootPassword: "secure-password"
  persistence:
    enabled: true
    size: 100Gi
```

Deploy:

```bash
helm install rustci ./helm/rustci -f values-production.yaml -n rustci
```

## Security Configuration

### Authentication Setup

1. **GitHub OAuth Setup**:
   - Go to GitHub Settings → Developer settings → OAuth Apps
   - Create new OAuth App with callback URL: `http://your-domain/api/sessions/oauth/github/callback`
   - Add Client ID and Secret to environment variables

2. **JWT Configuration**:
   ```env
   JWT_SECRET=your-super-secret-jwt-key-minimum-32-characters
   JWT_EXPIRES_IN=24h
   ```

3. **Database Security**:
   ```env
   MONGODB_URI=mongodb://username:password@localhost:27017/rustci?authSource=admin
   ```

### Network Security

#### Docker Network Isolation

```yaml
# docker-compose.yml
networks:
  rustci-network:
    driver: bridge

services:
  rustci:
    networks:
      - rustci-network
  mongo:
    networks:
      - rustci-network
```

#### Kubernetes Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: rustci-network-policy
spec:
  podSelector:
    matchLabels:
      app: rustci
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: nginx-ingress
    ports:
    - protocol: TCP
      port: 8000
```

## Monitoring and Observability

### Health Checks

RustCI provides built-in health endpoints:

```bash
# Application health
curl http://localhost:8000/api/healthchecker

# Database connectivity
curl http://localhost:8000/api/health/database

# System resources
curl http://localhost:8000/api/health/system
```

### Logging Configuration

```env
# Structured logging levels
RUST_LOG=info,rustci=debug,tower_http=debug

# Log format (json for production)
LOG_FORMAT=json

# Log file rotation
LOG_FILE=/var/log/rustci/app.log
LOG_MAX_SIZE=100MB
LOG_MAX_FILES=10
```

### Metrics Collection

Enable Prometheus metrics:

```env
METRICS_ENABLED=true
METRICS_PORT=9090
METRICS_PATH=/metrics
```

## Performance Optimization

### Database Optimization

```javascript
// MongoDB indexes for better performance
db.pipelines.createIndex({"name": 1})
db.pipelines.createIndex({"created_at": -1})
db.executions.createIndex({"pipeline_id": 1, "created_at": -1})
db.executions.createIndex({"status": 1})
db.services.createIndex({"status": 1, "last_heartbeat": -1})
```

### Resource Tuning

```env
# Worker thread configuration
TOKIO_WORKER_THREADS=8
MAX_BLOCKING_THREADS=16

# Connection pooling
DATABASE_MAX_CONNECTIONS=20
DATABASE_MIN_CONNECTIONS=5
DATABASE_CONNECT_TIMEOUT=30s

# Request limits
MAX_REQUEST_SIZE=10MB
REQUEST_TIMEOUT=30s
```

### Caching Strategy

```env
# Redis caching (optional)
REDIS_URL=redis://localhost:6379
CACHE_TTL=3600
CACHE_MAX_SIZE=1000

# Pipeline cache
PIPELINE_CACHE_ENABLED=true
EXECUTION_CACHE_ENABLED=true
```

## Backup and Recovery

### Database Backup

```bash
# MongoDB backup
mongodump --uri="mongodb://localhost:27017/rustci" --out=/backup/$(date +%Y%m%d)

# Automated backup script
#!/bin/bash
BACKUP_DIR="/backup/rustci"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p $BACKUP_DIR
mongodump --uri="$MONGODB_URI" --out="$BACKUP_DIR/$DATE"
tar -czf "$BACKUP_DIR/rustci_backup_$DATE.tar.gz" -C "$BACKUP_DIR" "$DATE"
rm -rf "$BACKUP_DIR/$DATE"
```

### Kubernetes Backup

```bash
# Backup persistent volumes
kubectl get pvc -n rustci -o yaml > rustci-pvc-backup.yaml

# Backup configuration
helm get values rustci -n rustci > rustci-values-backup.yaml
kubectl get secret -n rustci -o yaml > rustci-secrets-backup.yaml
```

## Troubleshooting

### Common Issues

1. **Port Already in Use**
   ```bash
   # Find and kill process
   lsof -i :8000
   kill -9 <PID>
   ```

2. **MongoDB Connection Failed**
   ```bash
   # Check MongoDB status
   brew services list | grep mongodb
   sudo systemctl status mongod
   ```

3. **Docker Permission Denied**
   ```bash
   # Add user to docker group
   sudo usermod -aG docker $USER
   newgrp docker
   ```

4. **Kubernetes Pod Issues**
   ```bash
   # Debug pod
   kubectl describe pod <pod-name> -n rustci
   kubectl logs <pod-name> -n rustci
   ```

### Log Analysis

```bash
# Enable debug logging
export RUST_LOG=debug

# Follow logs in real-time
tail -f /var/log/rustci/app.log

# Search for errors
grep -i error /var/log/rustci/app.log

# Kubernetes logs
kubectl logs -f deployment/rustci -n rustci
```

### Performance Debugging

```bash
# Check resource usage
docker stats rustci
kubectl top pods -n rustci

# Database performance
mongo --eval "db.runCommand({serverStatus: 1})"

# Connection monitoring
netstat -an | grep :8000
```

## Scaling and High Availability

### Horizontal Scaling

#### Docker Swarm

```yaml
version: '3.8'
services:
  rustci:
    image: rustci:latest
    deploy:
      replicas: 3
      update_config:
        parallelism: 1
        delay: 10s
      restart_policy:
        condition: on-failure
    ports:
      - "8000:8000"
```

#### Kubernetes Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rustci-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rustci
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

### Load Balancing

#### Nginx Configuration

```nginx
upstream rustci_backend {
    server rustci-1:8000;
    server rustci-2:8000;
    server rustci-3:8000;
}

server {
    listen 80;
    server_name rustci.yourdomain.com;

    location / {
        proxy_pass http://rustci_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

## Migration and Upgrades

### Version Upgrade Process

1. **Backup current deployment**
2. **Test new version in staging**
3. **Rolling update in production**

```bash
# Kubernetes rolling update
kubectl set image deployment/rustci rustci=rustci:v2.0.0 -n rustci
kubectl rollout status deployment/rustci -n rustci

# Rollback if needed
kubectl rollout undo deployment/rustci -n rustci
```

### Database Migration

```bash
# Export data
mongoexport --uri="mongodb://localhost:27017/rustci" --collection=pipelines --out=pipelines.json

# Import to new database
mongoimport --uri="mongodb://localhost:27017/rustci_new" --collection=pipelines --file=pipelines.json
```

## Support and Maintenance

### Regular Maintenance Tasks

1. **Log rotation and cleanup**
2. **Database optimization**
3. **Security updates**
4. **Performance monitoring**
5. **Backup verification**

### Monitoring Checklist

- [ ] Application health endpoints responding
- [ ] Database connectivity working
- [ ] Disk space sufficient
- [ ] Memory usage within limits
- [ ] CPU usage normal
- [ ] Network connectivity stable
- [ ] SSL certificates valid
- [ ] Backup processes running

This deployment guide provides comprehensive coverage for all deployment scenarios. Choose the appropriate section based on your deployment needs and environment.