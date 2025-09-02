# Valkyrie High-Performance Routing - Production Deployment Guide

This guide provides comprehensive instructions for deploying Valkyrie High-Performance Routing in production environments with optimal performance, security, and reliability.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [System Requirements](#system-requirements)
3. [Pre-Deployment Checklist](#pre-deployment-checklist)
4. [Deployment Methods](#deployment-methods)
5. [Configuration](#configuration)
6. [Security Setup](#security-setup)
7. [Monitoring and Observability](#monitoring-and-observability)
8. [Performance Tuning](#performance-tuning)
9. [High Availability Setup](#high-availability-setup)
10. [Troubleshooting](#troubleshooting)
11. [Maintenance](#maintenance)

## Prerequisites

### Software Requirements

- **Operating System**: Linux (Ubuntu 20.04+ or RHEL 8+)
- **Container Runtime**: Docker 20.10+ and Docker Compose 2.0+
- **Memory**: Minimum 8GB RAM, Recommended 16GB+
- **CPU**: Minimum 4 cores, Recommended 8+ cores
- **Storage**: Minimum 100GB SSD, Recommended 500GB+ NVMe SSD
- **Network**: Gigabit Ethernet or higher

### Hardware Recommendations

#### Minimum Production Setup
- **CPU**: 4 cores @ 2.4GHz
- **RAM**: 8GB
- **Storage**: 100GB SSD
- **Network**: 1 Gbps

#### High-Performance Setup
- **CPU**: 16 cores @ 3.0GHz+ (Intel Xeon or AMD EPYC)
- **RAM**: 32GB+ DDR4
- **Storage**: 1TB+ NVMe SSD
- **Network**: 10 Gbps+

#### Ultra-High-Performance Setup
- **CPU**: 32+ cores @ 3.5GHz+ with NUMA awareness
- **RAM**: 64GB+ DDR4 with ECC
- **Storage**: 2TB+ NVMe SSD RAID 0
- **Network**: 25 Gbps+ with SR-IOV support

## System Requirements

### Kernel Parameters

Add the following to `/etc/sysctl.conf`:

```bash
# Network performance optimizations
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.core.netdev_max_backlog = 5000
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728
net.ipv4.tcp_congestion_control = bbr
net.ipv4.tcp_slow_start_after_idle = 0

# File descriptor limits
fs.file-max = 1000000
fs.nr_open = 1000000

# Memory management
vm.swappiness = 1
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5

# CPU scheduling
kernel.sched_migration_cost_ns = 5000000
```

Apply with: `sudo sysctl -p`

### System Limits

Add to `/etc/security/limits.conf`:

```
* soft nofile 1000000
* hard nofile 1000000
* soft nproc 1000000
* hard nproc 1000000
```

### CPU Isolation (Optional for Ultra-High-Performance)

For dedicated routing workloads, isolate CPU cores:

```bash
# Add to GRUB_CMDLINE_LINUX in /etc/default/grub
isolcpus=2-15 nohz_full=2-15 rcu_nocbs=2-15
```

## Pre-Deployment Checklist

### Infrastructure Checklist

- [ ] Hardware meets minimum requirements
- [ ] Operating system is updated and patched
- [ ] Kernel parameters are optimized
- [ ] System limits are configured
- [ ] Docker and Docker Compose are installed
- [ ] SSL certificates are obtained and valid
- [ ] DNS records are configured
- [ ] Firewall rules are configured
- [ ] Load balancer is configured (if applicable)
- [ ] Monitoring infrastructure is ready

### Security Checklist

- [ ] TLS certificates are valid and properly configured
- [ ] API keys and secrets are generated and secured
- [ ] Rate limiting is configured
- [ ] Network segmentation is implemented
- [ ] Access controls are configured
- [ ] Security scanning is completed
- [ ] Backup and recovery procedures are tested

### Performance Checklist

- [ ] Performance requirements are defined (P99 < 82µs, 1M+ ops/sec)
- [ ] Baseline performance tests are completed
- [ ] Resource allocation is optimized
- [ ] Cache configuration is tuned
- [ ] Connection pooling is configured
- [ ] Lock-free data structures are optimized

## Deployment Methods

### Method 1: Docker Compose (Recommended for Single Node)

1. **Clone the repository and navigate to deployment directory:**
   ```bash
   git clone <repository-url>
   cd valkyrie-high-performance-routing/deployment/production
   ```

2. **Configure environment variables:**
   ```bash
   cp .env.example .env
   # Edit .env with your specific configuration
   ```

3. **Generate SSL certificates:**
   ```bash
   ./scripts/generate-ssl-certs.sh
   ```

4. **Start the services:**
   ```bash
   docker-compose up -d
   ```

5. **Verify deployment:**
   ```bash
   ./scripts/verify-deployment.sh
   ```

### Method 2: Kubernetes (Recommended for Multi-Node)

1. **Apply Kubernetes manifests:**
   ```bash
   kubectl apply -f kubernetes/namespace.yaml
   kubectl apply -f kubernetes/configmap.yaml
   kubectl apply -f kubernetes/secrets.yaml
   kubectl apply -f kubernetes/deployment.yaml
   kubectl apply -f kubernetes/service.yaml
   kubectl apply -f kubernetes/ingress.yaml
   ```

2. **Verify deployment:**
   ```bash
   kubectl get pods -n valkyrie-routing
   kubectl logs -f deployment/valkyrie-routing -n valkyrie-routing
   ```

### Method 3: Bare Metal (Maximum Performance)

1. **Build the application:**
   ```bash
   cargo build --release --features production
   ```

2. **Install systemd service:**
   ```bash
   sudo cp scripts/valkyrie-routing.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable valkyrie-routing
   ```

3. **Start the service:**
   ```bash
   sudo systemctl start valkyrie-routing
   ```

## Configuration

### Production Configuration Template

The production configuration is located at `config/production.toml`. Key sections:

#### Performance Configuration
```toml
[performance]
mode = "high"
enable_simd = true
enable_zero_copy = true
enable_hot_path_optimization = true
performance_budget = "82us"
throughput_target = 1000000

[performance.memory]
max_heap_size = "6GB"
enable_jemalloc = true
gc_threshold = "4GB"

[performance.cpu]
enable_cpu_affinity = true
cpu_cores = [0, 1, 2, 3, 4, 5, 6, 7]
enable_numa_awareness = true
```

#### Routing Configuration
```toml
[routing]
strategy = "high_performance"
max_routes = 1000000
route_cache_size = 100000
connection_pool_size = 1000
max_concurrent_requests = 50000

[routing.lockfree]
fit_capacity = 65536
fit_load_factor = 0.75
cra_initial_capacity = 32768
rcu_grace_period = "10ms"
```

### Environment-Specific Overrides

Create environment-specific configuration files:

- `config/production-us-east.toml` - US East region
- `config/production-eu-west.toml` - EU West region
- `config/production-asia.toml` - Asia Pacific region

## Security Setup

### TLS Configuration

1. **Generate or obtain SSL certificates:**
   ```bash
   # Using Let's Encrypt
   certbot certonly --standalone -d your-domain.com
   
   # Or generate self-signed for testing
   openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
   ```

2. **Configure TLS in production.toml:**
   ```toml
   [security]
   enable_tls = true
   tls_cert_path = "/app/config/ssl/cert.pem"
   tls_key_path = "/app/config/ssl/key.pem"
   min_tls_version = "1.2"
   ```

### Authentication Setup

1. **Generate JWT secret:**
   ```bash
   openssl rand -base64 32
   ```

2. **Configure authentication:**
   ```toml
   [security.auth]
   enable_jwt = true
   jwt_secret = "${JWT_SECRET}"
   jwt_expiry = "24h"
   enable_api_keys = true
   ```

### Rate Limiting

```toml
[security.rate_limiting]
enable = true
default_rate = "1000/min"
api_rate = "10000/min"
burst_size = 100
```

## Monitoring and Observability

### Metrics Collection

Valkyrie exposes Prometheus metrics on port 9090:

- **Routing Performance**: `valkyrie_routing_request_duration_seconds`
- **Throughput**: `valkyrie_routing_requests_total`
- **Error Rates**: `valkyrie_routing_errors_total`
- **Memory Usage**: `valkyrie_memory_usage_bytes`
- **Cache Performance**: `valkyrie_cache_hits_total`, `valkyrie_cache_misses_total`

### Grafana Dashboards

Import the provided Grafana dashboards:

1. **Performance Dashboard**: `monitoring/grafana/dashboards/performance.json`
2. **System Dashboard**: `monitoring/grafana/dashboards/system.json`
3. **Error Dashboard**: `monitoring/grafana/dashboards/errors.json`

### Alerting Rules

Key alerts are configured in `monitoring/alert_rules.yml`:

- **Critical**: P99 latency > 82µs
- **Critical**: Throughput < 1M ops/sec
- **Warning**: Error rate > 5%
- **Warning**: Memory usage > 90%

### Log Aggregation

Logs are structured in JSON format and can be ingested by:

- **ELK Stack**: Elasticsearch, Logstash, Kibana
- **Grafana Loki**: For cloud-native log aggregation
- **Fluentd**: For flexible log routing

## Performance Tuning

### CPU Optimization

1. **Enable CPU affinity:**
   ```toml
   [performance.cpu]
   enable_cpu_affinity = true
   cpu_cores = [0, 1, 2, 3, 4, 5, 6, 7]  # Adjust based on your system
   ```

2. **NUMA awareness:**
   ```bash
   # Check NUMA topology
   numactl --hardware
   
   # Run with NUMA binding
   numactl --cpunodebind=0 --membind=0 ./valkyrie-routing
   ```

### Memory Optimization

1. **Use jemalloc:**
   ```toml
   [performance.memory]
   enable_jemalloc = true
   ```

2. **Configure huge pages:**
   ```bash
   echo 1024 > /proc/sys/vm/nr_hugepages
   ```

### Network Optimization

1. **Enable receive packet steering (RPS):**
   ```bash
   echo f > /sys/class/net/eth0/queues/rx-0/rps_cpus
   ```

2. **Configure interrupt affinity:**
   ```bash
   echo 2 > /proc/irq/24/smp_affinity  # Adjust IRQ number
   ```

### Lock-Free Data Structure Tuning

1. **FIT (Fingerprinted ID Table) optimization:**
   ```toml
   [routing.lockfree]
   fit_capacity = 65536        # Power of 2, adjust based on route count
   fit_load_factor = 0.75      # Balance between memory and performance
   ```

2. **CRA (Compressed Radix Arena) optimization:**
   ```toml
   [routing.lockfree]
   cra_initial_capacity = 32768
   cra_enable_compression = true
   ```

3. **RCU (Read-Copy-Update) tuning:**
   ```toml
   [routing.lockfree]
   rcu_grace_period = "10ms"
   rcu_max_pending_updates = 10000
   ```

## High Availability Setup

### Load Balancer Configuration

#### Nginx Configuration
```nginx
upstream valkyrie_backend {
    least_conn;
    server valkyrie-1:8080 max_fails=3 fail_timeout=30s;
    server valkyrie-2:8080 max_fails=3 fail_timeout=30s;
    server valkyrie-3:8080 max_fails=3 fail_timeout=30s;
}

server {
    listen 80;
    listen 443 ssl http2;
    
    location / {
        proxy_pass http://valkyrie_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_connect_timeout 5s;
        proxy_send_timeout 10s;
        proxy_read_timeout 10s;
    }
    
    location /health {
        access_log off;
        proxy_pass http://valkyrie_backend;
    }
}
```

#### HAProxy Configuration
```
backend valkyrie_servers
    balance roundrobin
    option httpchk GET /health
    server valkyrie-1 valkyrie-1:8080 check inter 5s
    server valkyrie-2 valkyrie-2:8080 check inter 5s
    server valkyrie-3 valkyrie-3:8080 check inter 5s
```

### Database Clustering (if applicable)

For stateful deployments requiring persistence:

1. **MongoDB Replica Set:**
   ```yaml
   # docker-compose.yml
   mongodb-primary:
     image: mongo:5
     command: mongod --replSet rs0
   
   mongodb-secondary:
     image: mongo:5
     command: mongod --replSet rs0
   ```

2. **Redis Cluster:**
   ```yaml
   redis-cluster:
     image: redis:7-alpine
     command: redis-server --cluster-enabled yes
   ```

## Troubleshooting

### Common Issues

#### High Latency
1. **Check CPU usage and affinity**
2. **Verify memory allocation**
3. **Review cache hit rates**
4. **Check network configuration**

#### Low Throughput
1. **Increase connection pool size**
2. **Optimize worker thread count**
3. **Check for lock contention**
4. **Review system limits**

#### Memory Issues
1. **Monitor heap usage**
2. **Check for memory leaks**
3. **Verify garbage collection settings**
4. **Review cache sizes**

### Diagnostic Commands

```bash
# Check system resources
htop
iotop
nethogs

# Check application metrics
curl http://localhost:9090/metrics

# Check logs
docker-compose logs -f valkyrie-routing

# Performance profiling
perf record -g ./valkyrie-routing
perf report

# Memory profiling
valgrind --tool=massif ./valkyrie-routing
```

### Performance Debugging

1. **Enable detailed metrics:**
   ```toml
   [observability.metrics]
   enable_detailed_metrics = true
   ```

2. **Use tracing for request analysis:**
   ```toml
   [observability.tracing]
   sampling_rate = 1.0  # Temporarily increase for debugging
   ```

3. **Profile lock-free data structures:**
   ```bash
   # Enable profiling in configuration
   enable_profiling = true
   ```

## Maintenance

### Regular Maintenance Tasks

#### Daily
- [ ] Check system health dashboards
- [ ] Review error logs
- [ ] Monitor performance metrics
- [ ] Verify backup completion

#### Weekly
- [ ] Review capacity planning metrics
- [ ] Update security patches
- [ ] Rotate log files
- [ ] Test alerting systems

#### Monthly
- [ ] Performance baseline review
- [ ] Security audit
- [ ] Disaster recovery testing
- [ ] Configuration review

### Backup and Recovery

1. **Configuration backup:**
   ```bash
   # Backup configuration
   tar -czf config-backup-$(date +%Y%m%d).tar.gz config/
   
   # Backup to remote storage
   aws s3 cp config-backup-$(date +%Y%m%d).tar.gz s3://backup-bucket/
   ```

2. **State backup (if applicable):**
   ```bash
   # Backup Redis state
   redis-cli BGSAVE
   
   # Backup MongoDB
   mongodump --out /backup/mongodb-$(date +%Y%m%d)
   ```

### Updates and Rollbacks

1. **Rolling update:**
   ```bash
   # Update one instance at a time
   docker-compose up -d --no-deps valkyrie-routing-1
   # Wait for health check
   docker-compose up -d --no-deps valkyrie-routing-2
   ```

2. **Rollback procedure:**
   ```bash
   # Rollback to previous version
   docker-compose down
   docker-compose up -d --scale valkyrie-routing=3
   ```

### Capacity Planning

Monitor these metrics for capacity planning:

- **CPU utilization**: Target < 70% average
- **Memory usage**: Target < 80% of available
- **Network bandwidth**: Target < 60% of available
- **Request latency**: P99 < 82µs
- **Throughput**: > 1M ops/sec sustained

## Support and Documentation

### Additional Resources

- **API Documentation**: `/docs/api/`
- **Architecture Guide**: `/docs/architecture.md`
- **Performance Tuning**: `/docs/performance.md`
- **Security Guide**: `/docs/security.md`

### Getting Help

- **GitHub Issues**: Report bugs and feature requests
- **Documentation**: Comprehensive guides and references
- **Community**: Join our Discord/Slack for discussions
- **Professional Support**: Contact for enterprise support options

---

**Note**: This deployment guide assumes familiarity with Linux system administration, Docker, and basic networking concepts. For production deployments, consider engaging with your infrastructure team or a qualified systems administrator.