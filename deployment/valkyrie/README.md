# Valkyrie Protocol Deployment

This directory contains deployment configurations for the Valkyrie Protocol as a standalone service.

## 🚀 Quick Start

### Docker Compose Deployment

1. **Generate certificates** (for development):
   ```bash
   docker-compose --profile init up cert-generator
   ```

2. **Start Valkyrie Protocol with observability stack**:
   ```bash
   docker-compose -f docker-compose.valkyrie.yaml up -d
   ```

3. **Access services**:
   - Valkyrie Protocol: `https://localhost:443`
   - Grafana Dashboard: `http://localhost:3000` (admin/admin)
   - Prometheus: `http://localhost:9091`
   - Jaeger Tracing: `http://localhost:16686`
   - HAProxy Stats: `http://localhost:8404/stats` (admin/admin)

### Kubernetes Deployment

1. **Install with Helm**:
   ```bash
   helm install valkyrie-protocol ./deployment/valkyrie \
     --namespace valkyrie-system \
     --create-namespace \
     --values values.yaml
   ```

2. **Access services**:
   ```bash
   # Port forward to access services
   kubectl port-forward -n valkyrie-system svc/valkyrie-protocol 8080:8080
   kubectl port-forward -n valkyrie-system svc/valkyrie-protocol-grafana 3000:3000
   ```

## 📁 Directory Structure

```
deployment/valkyrie/
├── Chart.yaml                    # Helm chart metadata
├── values.yaml                   # Helm chart values
├── docker-compose.valkyrie.yaml  # Docker Compose configuration
├── templates/                    # Kubernetes templates
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── configmap.yaml
│   └── _helpers.tpl
├── config/                       # Configuration files
│   ├── valkyrie.yaml            # Valkyrie Protocol configuration
│   ├── haproxy.cfg              # HAProxy load balancer config
│   └── prometheus.yml           # Prometheus monitoring config
├── scripts/                      # Utility scripts
│   └── generate-certs.sh        # Certificate generation script
└── README.md                     # This file
```

## ⚙️ Configuration

### Valkyrie Protocol Configuration

The main configuration file is `config/valkyrie.yaml`. Key sections:

- **Protocol**: Core protocol settings (connections, timeouts, compression)
- **Transport**: TCP, QUIC, WebSocket transport configuration
- **Security**: TLS, Noise protocol, authentication, authorization
- **Performance**: Worker threads, memory pools, optimizations
- **Observability**: Metrics, tracing, logging, health checks

### Environment Variables

Key environment variables for Docker deployment:

```bash
VALKYRIE_CONFIG_FILE=/etc/valkyrie/config.yaml
VALKYRIE_LOG_LEVEL=info
VALKYRIE_LOG_FORMAT=json
RUST_LOG=valkyrie=info
TOKIO_WORKER_THREADS=0  # Auto-detect
```

## 🔒 Security

### TLS Configuration

The deployment includes comprehensive TLS configuration:

- **TLS 1.3** minimum version
- **Strong cipher suites** (AES-256-GCM, ChaCha20-Poly1305)
- **Certificate management** with auto-generation for development
- **Client certificate verification** (optional)

### Authentication & Authorization

Multiple authentication providers supported:

- **JWT tokens** with RS256 signing
- **Mutual TLS** (mTLS) with client certificates
- **API keys** with SHA256 hashing
- **Role-based access control** (RBAC)

## 📊 Monitoring & Observability

### Metrics (Prometheus)

- **Protocol metrics**: Connection counts, message rates, latency percentiles
- **Transport metrics**: TCP/QUIC/WebSocket specific metrics
- **Security metrics**: Authentication attempts, TLS handshakes
- **Performance metrics**: CPU, memory, network utilization

### Tracing (Jaeger)

- **Distributed tracing** across all protocol operations
- **Request correlation** with trace IDs
- **Performance analysis** with span timing
- **Error tracking** and debugging

### Logging

- **Structured JSON logging** with correlation IDs
- **Configurable log levels** (debug, info, warn, error)
- **Log aggregation** ready (stdout/stderr)
- **Security audit logs** for compliance

## 🚀 Performance Tuning

### High-Performance Configuration

For maximum performance, adjust these settings:

```yaml
performance:
  worker_threads: 0  # Auto-detect CPU cores
  enable_simd: true
  enable_zero_copy: true
  enable_vectored_io: true
  memory_pool_size: 2147483648  # 2GB

transport:
  tcp:
    recv_buffer_size: 131072  # 128KB
    send_buffer_size: 131072  # 128KB
  quic:
    max_streams: 10000
    max_connection_data: 104857600  # 100MB
```

### Resource Limits

Recommended resource limits for production:

```yaml
resources:
  limits:
    cpu: 4000m      # 4 CPU cores
    memory: 8Gi     # 8GB RAM
  requests:
    cpu: 2000m      # 2 CPU cores
    memory: 4Gi     # 4GB RAM
```

## 🔧 Troubleshooting

### Common Issues

1. **Certificate errors**:
   ```bash
   # Regenerate certificates
   docker-compose --profile init up cert-generator --force-recreate
   ```

2. **Connection refused**:
   ```bash
   # Check service health
   curl -f http://localhost:8081/health/ready
   ```

3. **High memory usage**:
   ```bash
   # Adjust memory pool size in config/valkyrie.yaml
   performance:
     memory_pool_size: 1073741824  # 1GB
   ```

### Health Checks

The deployment includes comprehensive health checks:

- **Liveness probe**: `/health/live` - Service is running
- **Readiness probe**: `/health/ready` - Service is ready to accept traffic
- **Detailed health**: `/health/detailed` - Component-level health status

### Logs

View logs for troubleshooting:

```bash
# Docker Compose
docker-compose -f docker-compose.valkyrie.yaml logs -f valkyrie-server

# Kubernetes
kubectl logs -n valkyrie-system deployment/valkyrie-protocol -f
```

## 🔄 Scaling

### Horizontal Scaling

Scale Valkyrie Protocol instances:

```bash
# Docker Compose (manual scaling)
docker-compose -f docker-compose.valkyrie.yaml up -d --scale valkyrie-server=3

# Kubernetes (HPA)
kubectl autoscale deployment valkyrie-protocol \
  --cpu-percent=70 \
  --min=3 \
  --max=100 \
  -n valkyrie-system
```

### Load Balancing

The deployment includes HAProxy for load balancing:

- **Round-robin** load balancing
- **Health check** based routing
- **Session affinity** for WebSocket connections
- **SSL termination** and security headers

## 🛠️ Development

### Local Development

For local development with hot reload:

```bash
# Start dependencies only
docker-compose -f docker-compose.valkyrie.yaml up -d prometheus grafana jaeger redis

# Run Valkyrie locally
VALKYRIE_CONFIG_FILE=./deployment/valkyrie/config/valkyrie.yaml cargo run
```

### Testing

Test the deployment:

```bash
# Health check
curl -f http://localhost:8081/health/ready

# Metrics endpoint
curl http://localhost:9090/metrics

# Protocol test (requires Valkyrie client)
valkyrie-client --endpoint https://localhost:443 --test-connection
```

## 📚 Additional Resources

- [Valkyrie Protocol Documentation](../../docs/valkyrie/)
- [Performance Tuning Guide](../../docs/valkyrie/performance-tuning.md)
- [Security Configuration](../../docs/valkyrie/security.md)
- [Monitoring Setup](../../docs/valkyrie/monitoring.md)
- [Troubleshooting Guide](../../docs/valkyrie/troubleshooting.md)