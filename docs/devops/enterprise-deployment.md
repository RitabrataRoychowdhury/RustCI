# Enterprise Deployment Guide

**Production-ready deployment for enterprise environments**

## 🏢 Enterprise Architecture

### Multi-Tier Deployment

```mermaid
graph TB
    subgraph "Load Balancer Tier"
        LB[HAProxy/NGINX]
    end
    
    subgraph "Application Tier"
        API1[RustCI API 1]
        API2[RustCI API 2]
        API3[RustCI API 3]
    end
    
    subgraph "Processing Tier"
        VE1[Valkyrie Engine 1]
        VE2[Valkyrie Engine 2]
        VE3[Valkyrie Engine 3]
    end
    
    subgraph "Runner Tier"
        DR[Docker Runners]
        KR[Kubernetes Runners]
        NR[Native Runners]
    end
    
    subgraph "Data Tier"
        MDB[(MongoDB Cluster)]
        REDIS[(Redis Cluster)]
    end
    
    subgraph "Monitoring Tier"
        PROM[Prometheus]
        GRAF[Grafana]
        JAEGER[Jaeger]
    end
    
    LB --> API1
    LB --> API2
    LB --> API3
    
    API1 --> VE1
    API2 --> VE2
    API3 --> VE3
    
    VE1 --> DR
    VE2 --> KR
    VE3 --> NR
    
    API1 --> MDB
    API2 --> MDB
    API3 --> MDB
    
    VE1 --> REDIS
    VE2 --> REDIS
    VE3 --> REDIS
    
    API1 --> PROM
    VE1 --> PROM
    PROM --> GRAF
    VE1 --> JAEGER
```

## 🚀 Quick Enterprise Setup

### Prerequisites Checklist

- [ ] **Kubernetes Cluster** (v1.24+)
- [ ] **MongoDB** (v5.0+) or Atlas
- [ ] **Redis** (v6.0+) for caching
- [ ] **Load Balancer** (HAProxy/NGINX/AWS ALB)
- [ ] **SSL Certificates** (Let's Encrypt/Corporate CA)
- [ ] **Monitoring Stack** (Prometheus/Grafana)
- [ ] **Log Aggregation** (ELK/Fluentd)
- [ ] **Backup Solution** (Velero/Custom)

### 1-Command Enterprise Deployment

```bash
# Deploy complete enterprise stack
curl -sSL https://deploy.rustci.dev/enterprise | bash -s -- \
  --domain=rustci.company.com \
  --replicas=5 \
  --monitoring=true \
  --backup=true \
  --ssl=letsencrypt
```

## 📦 Kubernetes Deployment

### Namespace Setup

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: rustci-enterprise
  labels:
    name: rustci-enterprise
    tier: production
---
apiVersion: v1
kind: ResourceQuota
metadata:
  name: rustci-quota
  namespace: rustci-enterprise
spec:
  hard:
    requests.cpu: "20"
    requests.memory: 40Gi
    limits.cpu: "40"
    limits.memory: 80Gi
    persistentvolumeclaims: "10"
```

### ConfigMap & Secrets

```yaml
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rustci-config
  namespace: rustci-enterprise
data:
  config.yaml: |
    server:
      host: "0.0.0.0"
      port: 8080
      workers: 8
    
    database:
      uri: "mongodb://mongodb-service:27017"
      database: "rustci_enterprise"
      pool_size: 50
    
    valkyrie:
      enabled: true
      performance:
        max_connections: 50000
        worker_threads: 16
      security:
        encryption: true
        authentication: true
    
    runners:
      kubernetes:
        enabled: true
        namespace: "rustci-runners"
        max_concurrent: 500
      docker:
        enabled: true
        max_concurrent: 200
    
    observability:
      metrics:
        enabled: true
        endpoint: "/metrics"
      tracing:
        enabled: true
        jaeger_endpoint: "http://jaeger-collector:14268/api/traces"

---
apiVersion: v1
kind: Secret
metadata:
  name: rustci-secrets
  namespace: rustci-enterprise
type: Opaque
data:
  mongodb-uri: <base64-encoded-mongodb-uri>
  jwt-secret: <base64-encoded-jwt-secret>
  github-oauth-client-id: <base64-encoded-client-id>
  github-oauth-client-secret: <base64-encoded-client-secret>
```

### High-Availability Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rustci-api
  namespace: rustci-enterprise
  labels:
    app: rustci-api
    tier: application
spec:
  replicas: 5
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 2
      maxUnavailable: 1
  selector:
    matchLabels:
      app: rustci-api
  template:
    metadata:
      labels:
        app: rustci-api
        tier: application
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - rustci-api
              topologyKey: kubernetes.io/hostname
      containers:
      - name: rustci-api
        image: rustci:enterprise-v1.0.0
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: RUST_LOG
          value: "info"
        - name: CONFIG_PATH
          value: "/etc/rustci/config.yaml"
        envFrom:
        - secretRef:
            name: rustci-secrets
        volumeMounts:
        - name: config
          mountPath: /etc/rustci
        - name: tls-certs
          mountPath: /etc/ssl/certs/rustci
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        startupProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 30
      volumes:
      - name: config
        configMap:
          name: rustci-config
      - name: tls-certs
        secret:
          secretName: rustci-tls-certs
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      serviceAccountName: rustci-service-account
```

### Service & Ingress

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: rustci-api-service
  namespace: rustci-enterprise
  labels:
    app: rustci-api
spec:
  selector:
    app: rustci-api
  ports:
  - port: 80
    targetPort: 8080
    name: http
  type: ClusterIP

---
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: rustci-ingress
  namespace: rustci-enterprise
  annotations:
    kubernetes.io/ingress.class: "nginx"
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
    nginx.ingress.kubernetes.io/proxy-body-size: "100m"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "600"
    nginx.ingress.kubernetes.io/rate-limit: "1000"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
spec:
  tls:
  - hosts:
    - rustci.company.com
    secretName: rustci-tls-secret
  rules:
  - host: rustci.company.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: rustci-api-service
            port:
              number: 80
```

### Horizontal Pod Autoscaler

```yaml
# hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rustci-api-hpa
  namespace: rustci-enterprise
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rustci-api
  minReplicas: 5
  maxReplicas: 50
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  - type: Pods
    pods:
      metric:
        name: valkyrie_connections_active
      target:
        type: AverageValue
        averageValue: "1000"
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 100
        periodSeconds: 15
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 10
        periodSeconds: 60
```

## 🗄️ Database Setup

### MongoDB Replica Set

```yaml
# mongodb-statefulset.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: mongodb
  namespace: rustci-enterprise
spec:
  serviceName: mongodb-service
  replicas: 3
  selector:
    matchLabels:
      app: mongodb
  template:
    metadata:
      labels:
        app: mongodb
    spec:
      containers:
      - name: mongodb
        image: mongo:7
        ports:
        - containerPort: 27017
        env:
        - name: MONGO_INITDB_ROOT_USERNAME
          value: "admin"
        - name: MONGO_INITDB_ROOT_PASSWORD
          valueFrom:
            secretKeyRef:
              name: mongodb-secret
              key: password
        - name: MONGO_REPLICA_SET_NAME
          value: "rs0"
        volumeMounts:
        - name: mongodb-data
          mountPath: /data/db
        - name: mongodb-config
          mountPath: /data/configdb
        resources:
          requests:
            memory: "4Gi"
            cpu: "1000m"
          limits:
            memory: "8Gi"
            cpu: "2000m"
  volumeClaimTemplates:
  - metadata:
      name: mongodb-data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "fast-ssd"
      resources:
        requests:
          storage: 100Gi
  - metadata:
      name: mongodb-config
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "fast-ssd"
      resources:
        requests:
          storage: 10Gi
```

### Redis Cluster

```yaml
# redis-cluster.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis-cluster
  namespace: rustci-enterprise
spec:
  serviceName: redis-cluster-service
  replicas: 6
  selector:
    matchLabels:
      app: redis-cluster
  template:
    metadata:
      labels:
        app: redis-cluster
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        ports:
        - containerPort: 6379
        - containerPort: 16379
        command:
        - redis-server
        - /etc/redis/redis.conf
        - --cluster-enabled
        - "yes"
        - --cluster-config-file
        - /data/nodes.conf
        - --cluster-node-timeout
        - "5000"
        - --appendonly
        - "yes"
        volumeMounts:
        - name: redis-data
          mountPath: /data
        - name: redis-config
          mountPath: /etc/redis
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
      volumes:
      - name: redis-config
        configMap:
          name: redis-config
  volumeClaimTemplates:
  - metadata:
      name: redis-data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "fast-ssd"
      resources:
        requests:
          storage: 20Gi
```

## 📊 Monitoring & Observability

### Prometheus Configuration

```yaml
# prometheus-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: rustci-enterprise
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    
    rule_files:
      - "/etc/prometheus/rules/*.yml"
    
    scrape_configs:
      - job_name: 'rustci-api'
        kubernetes_sd_configs:
        - role: pod
          namespaces:
            names:
            - rustci-enterprise
        relabel_configs:
        - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
          action: keep
          regex: true
        - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
          action: replace
          target_label: __metrics_path__
          regex: (.+)
        - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
          action: replace
          regex: ([^:]+)(?::\d+)?;(\d+)
          replacement: $1:$2
          target_label: __address__
      
      - job_name: 'valkyrie-protocol'
        static_configs:
        - targets: ['rustci-api-service:8080']
        metrics_path: '/api/v1/valkyrie/metrics'
        scrape_interval: 5s
    
    alerting:
      alertmanagers:
      - static_configs:
        - targets:
          - alertmanager:9093
```

### Grafana Dashboards

```json
{
  "dashboard": {
    "id": null,
    "title": "RustCI Enterprise Dashboard",
    "tags": ["rustci", "enterprise"],
    "timezone": "browser",
    "panels": [
      {
        "id": 1,
        "title": "Pipeline Success Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(rustci_pipelines_total{status=\"success\"}[5m]) / rate(rustci_pipelines_total[5m]) * 100",
            "legendFormat": "Success Rate %"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "unit": "percent",
            "min": 0,
            "max": 100,
            "thresholds": {
              "steps": [
                {"color": "red", "value": 0},
                {"color": "yellow", "value": 80},
                {"color": "green", "value": 95}
              ]
            }
          }
        }
      },
      {
        "id": 2,
        "title": "Valkyrie Protocol Performance",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, valkyrie_latency_seconds)",
            "legendFormat": "P50 Latency"
          },
          {
            "expr": "histogram_quantile(0.95, valkyrie_latency_seconds)",
            "legendFormat": "P95 Latency"
          },
          {
            "expr": "histogram_quantile(0.99, valkyrie_latency_seconds)",
            "legendFormat": "P99 Latency"
          }
        ],
        "yAxes": [
          {
            "unit": "s",
            "min": 0
          }
        ]
      },
      {
        "id": 3,
        "title": "System Resources",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(container_cpu_usage_seconds_total{pod=~\"rustci-api-.*\"}[5m]) * 100",
            "legendFormat": "CPU Usage %"
          },
          {
            "expr": "container_memory_usage_bytes{pod=~\"rustci-api-.*\"} / container_spec_memory_limit_bytes * 100",
            "legendFormat": "Memory Usage %"
          }
        ]
      }
    ],
    "time": {
      "from": "now-1h",
      "to": "now"
    },
    "refresh": "5s"
  }
}
```

## 🔒 Security Configuration

### RBAC Setup

```yaml
# rbac.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: rustci-service-account
  namespace: rustci-enterprise

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: rustci-cluster-role
rules:
- apiGroups: [""]
  resources: ["pods", "services", "configmaps", "secrets"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["apps"]
  resources: ["deployments", "replicasets"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["batch"]
  resources: ["jobs", "cronjobs"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: rustci-cluster-role-binding
subjects:
- kind: ServiceAccount
  name: rustci-service-account
  namespace: rustci-enterprise
roleRef:
  kind: ClusterRole
  name: rustci-cluster-role
  apiGroup: rbac.authorization.k8s.io
```

### Network Policies

```yaml
# network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: rustci-network-policy
  namespace: rustci-enterprise
spec:
  podSelector:
    matchLabels:
      app: rustci-api
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    - podSelector:
        matchLabels:
          app: prometheus
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: mongodb
    ports:
    - protocol: TCP
      port: 27017
  - to:
    - podSelector:
        matchLabels:
          app: redis-cluster
    ports:
    - protocol: TCP
      port: 6379
  - to: []
    ports:
    - protocol: TCP
      port: 443
    - protocol: TCP
      port: 80
```

## 🔄 Backup & Disaster Recovery

### Automated Backup Script

```bash
#!/bin/bash
# backup-rustci.sh

BACKUP_DIR="/backups/rustci"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RETENTION_DAYS=30

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Backup MongoDB
echo "🗄️ Backing up MongoDB..."
kubectl exec -n rustci-enterprise mongodb-0 -- mongodump \
  --uri="mongodb://admin:${MONGO_PASSWORD}@localhost:27017/rustci_enterprise?authSource=admin" \
  --archive="/tmp/mongodb_backup_${TIMESTAMP}.archive"

kubectl cp rustci-enterprise/mongodb-0:/tmp/mongodb_backup_${TIMESTAMP}.archive \
  "$BACKUP_DIR/mongodb_backup_${TIMESTAMP}.archive"

# Backup Kubernetes configurations
echo "⚙️ Backing up Kubernetes configurations..."
kubectl get all,configmaps,secrets,pvc -n rustci-enterprise -o yaml > \
  "$BACKUP_DIR/k8s_config_${TIMESTAMP}.yaml"

# Backup persistent volumes
echo "💾 Backing up persistent volumes..."
kubectl get pv -o yaml > "$BACKUP_DIR/persistent_volumes_${TIMESTAMP}.yaml"

# Compress backups
echo "🗜️ Compressing backups..."
tar -czf "$BACKUP_DIR/rustci_backup_${TIMESTAMP}.tar.gz" \
  "$BACKUP_DIR/mongodb_backup_${TIMESTAMP}.archive" \
  "$BACKUP_DIR/k8s_config_${TIMESTAMP}.yaml" \
  "$BACKUP_DIR/persistent_volumes_${TIMESTAMP}.yaml"

# Clean up individual files
rm "$BACKUP_DIR/mongodb_backup_${TIMESTAMP}.archive" \
   "$BACKUP_DIR/k8s_config_${TIMESTAMP}.yaml" \
   "$BACKUP_DIR/persistent_volumes_${TIMESTAMP}.yaml"

# Upload to cloud storage (example: AWS S3)
aws s3 cp "$BACKUP_DIR/rustci_backup_${TIMESTAMP}.tar.gz" \
  "s3://company-backups/rustci/"

# Clean up old backups
find "$BACKUP_DIR" -name "rustci_backup_*.tar.gz" -mtime +$RETENTION_DAYS -delete

echo "✅ Backup completed: rustci_backup_${TIMESTAMP}.tar.gz"
```

### Disaster Recovery Procedure

```bash
#!/bin/bash
# restore-rustci.sh

BACKUP_FILE="$1"
RESTORE_NAMESPACE="rustci-enterprise-restore"

if [ -z "$BACKUP_FILE" ]; then
    echo "Usage: $0 <backup_file.tar.gz>"
    exit 1
fi

echo "🔄 Starting disaster recovery..."

# Create restore namespace
kubectl create namespace "$RESTORE_NAMESPACE"

# Extract backup
TEMP_DIR=$(mktemp -d)
tar -xzf "$BACKUP_FILE" -C "$TEMP_DIR"

# Restore Kubernetes configurations
echo "⚙️ Restoring Kubernetes configurations..."
sed "s/rustci-enterprise/$RESTORE_NAMESPACE/g" \
  "$TEMP_DIR/k8s_config_"*.yaml | kubectl apply -f -

# Wait for MongoDB to be ready
echo "⏳ Waiting for MongoDB to be ready..."
kubectl wait --for=condition=ready pod -l app=mongodb \
  -n "$RESTORE_NAMESPACE" --timeout=300s

# Restore MongoDB data
echo "🗄️ Restoring MongoDB data..."
kubectl cp "$TEMP_DIR/mongodb_backup_"*.archive \
  "$RESTORE_NAMESPACE/mongodb-0:/tmp/restore.archive"

kubectl exec -n "$RESTORE_NAMESPACE" mongodb-0 -- mongorestore \
  --uri="mongodb://admin:${MONGO_PASSWORD}@localhost:27017/rustci_enterprise?authSource=admin" \
  --archive="/tmp/restore.archive"

# Clean up
rm -rf "$TEMP_DIR"

echo "✅ Disaster recovery completed in namespace: $RESTORE_NAMESPACE"
echo "🔍 Verify the restoration and switch traffic when ready"
```

## 📈 Performance Tuning

### JVM-like Tuning for Rust

```yaml
# performance-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rustci-performance-config
data:
  performance.yaml: |
    # Rust-specific optimizations
    runtime:
      worker_threads: 16
      max_blocking_threads: 512
      thread_stack_size: "2MB"
      thread_keep_alive: "60s"
    
    # Memory management
    memory:
      heap_size: "4GB"
      stack_size: "8MB"
      gc_threshold: "1GB"
    
    # Network optimizations
    network:
      tcp_nodelay: true
      tcp_keepalive: true
      so_reuseport: true
      backlog: 1024
      max_connections: 50000
    
    # Valkyrie protocol tuning
    valkyrie:
      buffer_size: 131072
      batch_size: 1000
      compression_level: 6
      zero_copy: true
      simd_optimizations: true
```

---

## 🎯 Production Readiness Checklist

### Security
- [ ] TLS/SSL certificates configured
- [ ] RBAC policies implemented
- [ ] Network policies configured
- [ ] Secrets management setup
- [ ] Security scanning enabled
- [ ] Audit logging configured

### High Availability
- [ ] Multi-replica deployment (≥3)
- [ ] Pod anti-affinity rules
- [ ] Health checks configured
- [ ] Auto-scaling enabled
- [ ] Load balancer configured
- [ ] Database clustering setup

### Monitoring
- [ ] Prometheus metrics collection
- [ ] Grafana dashboards configured
- [ ] Alerting rules setup
- [ ] Log aggregation enabled
- [ ] Distributed tracing configured
- [ ] SLA monitoring implemented

### Backup & Recovery
- [ ] Automated backup schedule
- [ ] Backup verification process
- [ ] Disaster recovery plan
- [ ] Recovery time objectives defined
- [ ] Data retention policies
- [ ] Cross-region backup replication

### Performance
- [ ] Resource limits configured
- [ ] Performance testing completed
- [ ] Capacity planning done
- [ ] Caching strategy implemented
- [ ] Database optimization
- [ ] Network optimization

### Compliance
- [ ] Data encryption at rest
- [ ] Data encryption in transit
- [ ] Access logging enabled
- [ ] Compliance reporting setup
- [ ] Data retention policies
- [ ] Privacy controls implemented

**Enterprise deployment ready!** 🚀