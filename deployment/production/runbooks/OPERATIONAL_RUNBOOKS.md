# Valkyrie High-Performance Routing - Operational Runbooks

This document contains step-by-step procedures for common operational scenarios, incident response, and troubleshooting for Valkyrie High-Performance Routing in production environments.

## Table of Contents

1. [Emergency Response](#emergency-response)
2. [Performance Issues](#performance-issues)
3. [System Health Checks](#system-health-checks)
4. [Capacity Management](#capacity-management)
5. [Configuration Management](#configuration-management)
6. [Backup and Recovery](#backup-and-recovery)
7. [Security Incidents](#security-incidents)
8. [Monitoring and Alerting](#monitoring-and-alerting)

## Emergency Response

### Service Down - Complete Outage

**Severity**: P0 (Critical)
**Response Time**: Immediate (< 5 minutes)

#### Symptoms
- Health check endpoint returning 5xx errors
- No response from service endpoints
- All monitoring dashboards showing service as down

#### Immediate Actions (0-5 minutes)
1. **Verify the outage:**
   ```bash
   curl -f http://valkyrie-routing:8080/health
   # Expected: HTTP 200 with health status
   ```

2. **Check service status:**
   ```bash
   docker-compose ps valkyrie-routing
   # or
   kubectl get pods -n valkyrie-routing
   ```

3. **Check recent logs:**
   ```bash
   docker-compose logs --tail=100 valkyrie-routing
   # or
   kubectl logs -f deployment/valkyrie-routing -n valkyrie-routing --tail=100
   ```

4. **Attempt service restart:**
   ```bash
   docker-compose restart valkyrie-routing
   # or
   kubectl rollout restart deployment/valkyrie-routing -n valkyrie-routing
   ```

#### Investigation Actions (5-15 minutes)
1. **Check system resources:**
   ```bash
   # CPU and memory usage
   htop
   
   # Disk space
   df -h
   
   # Network connectivity
   netstat -tuln | grep 8080
   ```

2. **Review error logs:**
   ```bash
   # Application errors
   grep -i "error\|panic\|fatal" /app/logs/valkyrie.log
   
   # System errors
   journalctl -u valkyrie-routing --since "10 minutes ago"
   ```

3. **Check dependencies:**
   ```bash
   # Redis connectivity
   redis-cli ping
   
   # Database connectivity (if applicable)
   mongosh --eval "db.adminCommand('ping')"
   ```

#### Recovery Actions
1. **If restart successful:**
   - Monitor service for 10 minutes
   - Verify performance metrics return to normal
   - Document incident in post-mortem

2. **If restart fails:**
   - Escalate to engineering team
   - Consider rollback to previous version
   - Implement emergency traffic routing

### High Latency Alert

**Severity**: P1 (High)
**Response Time**: < 15 minutes

#### Symptoms
- P99 latency > 82µs for more than 2 minutes
- Grafana dashboard showing latency spikes
- User reports of slow responses

#### Investigation Steps
1. **Check current performance:**
   ```bash
   # Get current metrics
   curl http://valkyrie-routing:9090/metrics | grep valkyrie_routing_request_duration
   ```

2. **Identify bottlenecks:**
   ```bash
   # CPU usage by process
   top -p $(pgrep valkyrie-routing)
   
   # Memory usage
   cat /proc/$(pgrep valkyrie-routing)/status | grep -E "VmRSS|VmSize"
   
   # Network connections
   ss -tuln | grep :8080
   ```

3. **Check lock-free data structures:**
   ```bash
   # FIT collision rate
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_fit_collisions
   
   # CRA fragmentation
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_cra_fragmentation
   
   # RCU grace period
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_rcu_grace_period
   ```

#### Mitigation Actions
1. **Immediate optimizations:**
   ```bash
   # Increase worker threads (if CPU bound)
   # Update configuration and reload
   echo 'worker_threads = 16' >> /app/config/production.toml
   
   # Clear caches (if memory bound)
   redis-cli FLUSHALL
   ```

2. **Scale horizontally:**
   ```bash
   # Docker Compose
   docker-compose up -d --scale valkyrie-routing=3
   
   # Kubernetes
   kubectl scale deployment valkyrie-routing --replicas=3 -n valkyrie-routing
   ```

### Low Throughput Alert

**Severity**: P1 (High)
**Response Time**: < 15 minutes

#### Symptoms
- Throughput < 1M ops/sec for more than 5 minutes
- Request queue building up
- Increased response times

#### Investigation Steps
1. **Check request patterns:**
   ```bash
   # Request rate
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_routing_requests_total
   
   # Error rate
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_routing_errors_total
   ```

2. **Analyze bottlenecks:**
   ```bash
   # Connection pool utilization
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_connection_pool
   
   # Cache hit rate
   curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_cache_hits
   ```

#### Mitigation Actions
1. **Optimize connection handling:**
   ```toml
   # Update configuration
   [routing]
   connection_pool_size = 2000
   max_concurrent_requests = 100000
   ```

2. **Improve caching:**
   ```toml
   # Increase cache sizes
   [routing]
   route_cache_size = 200000
   route_cache_ttl = "600s"
   ```

## Performance Issues

### Memory Leak Investigation

#### Detection
```bash
# Monitor memory growth over time
watch -n 5 'cat /proc/$(pgrep valkyrie-routing)/status | grep VmRSS'

# Check for memory fragmentation
cat /proc/$(pgrep valkyrie-routing)/smaps | grep -E "Size|Rss" | awk '{sum+=$2} END {print sum " KB"}'
```

#### Analysis
```bash
# Enable memory profiling
export VALKYRIE_MEMORY_PROFILING=true

# Generate heap dump
kill -USR1 $(pgrep valkyrie-routing)

# Analyze with valgrind (development environment)
valgrind --tool=massif --massif-out-file=massif.out ./valkyrie-routing
ms_print massif.out
```

#### Mitigation
```bash
# Restart service to reclaim memory
docker-compose restart valkyrie-routing

# Adjust memory limits
echo 'max_heap_size = "4GB"' >> /app/config/production.toml
```

### CPU Bottleneck Resolution

#### Detection
```bash
# Check CPU usage
top -p $(pgrep valkyrie-routing)

# Check CPU affinity
taskset -p $(pgrep valkyrie-routing)

# Profile CPU usage
perf record -g -p $(pgrep valkyrie-routing) sleep 30
perf report
```

#### Optimization
```bash
# Set CPU affinity to dedicated cores
taskset -cp 0-7 $(pgrep valkyrie-routing)

# Enable NUMA awareness
numactl --cpunodebind=0 --membind=0 valkyrie-routing

# Optimize scheduler
echo performance > /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

### Lock Contention Analysis

#### Detection
```bash
# Check lock-free metrics
curl -s http://valkyrie-routing:9090/metrics | grep -E "fit_collisions|cra_fragmentation|rcu_grace_period"

# Profile with perf
perf record -e cycles:u -g -p $(pgrep valkyrie-routing) sleep 10
perf report --sort=symbol
```

#### Optimization
```toml
# Tune lock-free parameters
[routing.lockfree]
fit_capacity = 131072  # Increase capacity
fit_load_factor = 0.6  # Reduce load factor
rcu_grace_period = "5ms"  # Reduce grace period
```

## System Health Checks

### Daily Health Check Procedure

#### Automated Health Check Script
```bash
#!/bin/bash
# daily-health-check.sh

echo "=== Valkyrie Daily Health Check $(date) ==="

# Service status
echo "1. Service Status:"
curl -f http://valkyrie-routing:8080/health || echo "FAIL: Health check failed"

# Performance metrics
echo "2. Performance Metrics:"
LATENCY=$(curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_routing_request_duration_seconds | grep quantile=\"0.99\" | awk '{print $2}')
THROUGHPUT=$(curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_routing_requests_total | tail -1 | awk '{print $2}')

echo "P99 Latency: ${LATENCY}s (target: <0.000082s)"
echo "Total Requests: ${THROUGHPUT}"

# Resource usage
echo "3. Resource Usage:"
CPU=$(top -bn1 -p $(pgrep valkyrie-routing) | tail -1 | awk '{print $9}')
MEM=$(cat /proc/$(pgrep valkyrie-routing)/status | grep VmRSS | awk '{print $2}')
echo "CPU Usage: ${CPU}%"
echo "Memory Usage: ${MEM} KB"

# Error rate
echo "4. Error Rate:"
ERRORS=$(curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_routing_errors_total | tail -1 | awk '{print $2}')
echo "Total Errors: ${ERRORS}"

# Dependencies
echo "5. Dependencies:"
redis-cli ping > /dev/null && echo "Redis: OK" || echo "Redis: FAIL"

echo "=== Health Check Complete ==="
```

#### Weekly Deep Health Check
```bash
#!/bin/bash
# weekly-health-check.sh

# Performance baseline comparison
echo "Performance Baseline Check:"
./scripts/performance-test.sh > /tmp/current-perf.txt
diff /tmp/baseline-perf.txt /tmp/current-perf.txt || echo "Performance deviation detected"

# Configuration validation
echo "Configuration Validation:"
./valkyrie-routing --validate-config /app/config/production.toml

# Security scan
echo "Security Scan:"
nmap -sS -O localhost
```

## Capacity Management

### Traffic Spike Response

#### Detection
```bash
# Monitor request rate
watch -n 1 'curl -s http://valkyrie-routing:9090/metrics | grep rate | grep valkyrie_routing_requests'

# Check queue depth
curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_request_queue_depth
```

#### Scaling Actions
```bash
# Horizontal scaling (Docker Compose)
docker-compose up -d --scale valkyrie-routing=5

# Horizontal scaling (Kubernetes)
kubectl scale deployment valkyrie-routing --replicas=5 -n valkyrie-routing

# Vertical scaling (increase resources)
docker-compose down
# Edit docker-compose.yml to increase CPU/memory limits
docker-compose up -d
```

### Capacity Planning

#### Weekly Capacity Review
```bash
#!/bin/bash
# capacity-review.sh

echo "=== Weekly Capacity Review ==="

# Calculate average metrics over the week
WEEK_AGO=$(date -d '7 days ago' +%s)
NOW=$(date +%s)

# Query Prometheus for weekly averages
curl -G 'http://prometheus:9090/api/v1/query_range' \
  --data-urlencode 'query=avg_over_time(valkyrie_routing_requests_total[7d])' \
  --data-urlencode "start=${WEEK_AGO}" \
  --data-urlencode "end=${NOW}" \
  --data-urlencode 'step=3600'

# Generate capacity report
echo "Current capacity utilization:"
echo "CPU: $(kubectl top nodes | awk 'NR>1 {sum+=$3} END {print sum/NR}')%"
echo "Memory: $(kubectl top nodes | awk 'NR>1 {sum+=$5} END {print sum/NR}')%"
```

## Configuration Management

### Hot Configuration Reload

#### Procedure
```bash
# 1. Validate new configuration
./valkyrie-routing --validate-config /app/config/new-production.toml

# 2. Backup current configuration
cp /app/config/production.toml /app/config/backup-$(date +%Y%m%d-%H%M%S).toml

# 3. Apply new configuration
cp /app/config/new-production.toml /app/config/production.toml

# 4. Trigger hot reload
kill -HUP $(pgrep valkyrie-routing)

# 5. Verify configuration applied
curl http://valkyrie-routing:8080/config/status
```

#### Rollback Procedure
```bash
# 1. Identify last good configuration
ls -la /app/config/backup-*.toml | tail -1

# 2. Restore configuration
cp /app/config/backup-YYYYMMDD-HHMMSS.toml /app/config/production.toml

# 3. Trigger reload
kill -HUP $(pgrep valkyrie-routing)

# 4. Verify rollback
curl http://valkyrie-routing:8080/health
```

### Configuration Validation

#### Pre-deployment Validation
```bash
#!/bin/bash
# validate-config.sh

CONFIG_FILE="$1"

echo "Validating configuration: $CONFIG_FILE"

# Syntax validation
./valkyrie-routing --validate-config "$CONFIG_FILE" || exit 1

# Performance validation
BUDGET=$(grep performance_budget "$CONFIG_FILE" | cut -d'"' -f2)
if [[ "$BUDGET" > "82us" ]]; then
    echo "ERROR: Performance budget exceeds 82µs requirement"
    exit 1
fi

# Security validation
if ! grep -q "enable_tls = true" "$CONFIG_FILE"; then
    echo "WARNING: TLS not enabled"
fi

echo "Configuration validation passed"
```

## Backup and Recovery

### Configuration Backup

#### Daily Backup Script
```bash
#!/bin/bash
# backup-config.sh

BACKUP_DIR="/backup/valkyrie/$(date +%Y/%m/%d)"
mkdir -p "$BACKUP_DIR"

# Backup configuration files
tar -czf "$BACKUP_DIR/config-$(date +%H%M%S).tar.gz" /app/config/

# Backup to remote storage
aws s3 cp "$BACKUP_DIR/config-$(date +%H%M%S).tar.gz" \
  s3://valkyrie-backups/config/

# Cleanup old backups (keep 30 days)
find /backup/valkyrie -type f -mtime +30 -delete
```

### State Recovery

#### Cache Recovery
```bash
# Warm up caches after restart
curl -X POST http://valkyrie-routing:8080/admin/cache/warmup

# Verify cache performance
curl -s http://valkyrie-routing:9090/metrics | grep cache_hit_rate
```

#### Route Table Recovery
```bash
# Reload route configuration
curl -X POST http://valkyrie-routing:8080/admin/routes/reload

# Verify route count
curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_active_routes
```

## Security Incidents

### Suspicious Traffic Detection

#### Investigation
```bash
# Check access logs for anomalies
tail -f /var/log/nginx/access.log | grep -E "40[0-9]|50[0-9]"

# Analyze request patterns
awk '{print $1}' /var/log/nginx/access.log | sort | uniq -c | sort -nr | head -20

# Check rate limiting effectiveness
curl -s http://valkyrie-routing:9090/metrics | grep rate_limit
```

#### Mitigation
```bash
# Block suspicious IPs
iptables -A INPUT -s SUSPICIOUS_IP -j DROP

# Increase rate limiting
# Update configuration with stricter limits
[security.rate_limiting]
default_rate = "100/min"
burst_size = 10
```

### Certificate Expiry

#### Monitoring
```bash
# Check certificate expiry
openssl x509 -in /app/config/ssl/cert.pem -noout -dates

# Automated check script
#!/bin/bash
CERT_FILE="/app/config/ssl/cert.pem"
EXPIRY_DATE=$(openssl x509 -in "$CERT_FILE" -noout -enddate | cut -d= -f2)
EXPIRY_EPOCH=$(date -d "$EXPIRY_DATE" +%s)
CURRENT_EPOCH=$(date +%s)
DAYS_LEFT=$(( (EXPIRY_EPOCH - CURRENT_EPOCH) / 86400 ))

if [ $DAYS_LEFT -lt 30 ]; then
    echo "WARNING: Certificate expires in $DAYS_LEFT days"
fi
```

#### Renewal
```bash
# Renew Let's Encrypt certificate
certbot renew --dry-run
certbot renew

# Update configuration
cp /etc/letsencrypt/live/your-domain.com/fullchain.pem /app/config/ssl/cert.pem
cp /etc/letsencrypt/live/your-domain.com/privkey.pem /app/config/ssl/key.pem

# Reload configuration
kill -HUP $(pgrep valkyrie-routing)
```

## Monitoring and Alerting

### Alert Response Procedures

#### High Error Rate Alert
```bash
# 1. Check error distribution
curl -s http://valkyrie-routing:9090/metrics | grep valkyrie_routing_errors | sort

# 2. Analyze error logs
grep -E "ERROR|WARN" /app/logs/valkyrie.log | tail -100

# 3. Check upstream dependencies
curl -f http://upstream-service:8080/health

# 4. Implement circuit breaker if needed
curl -X POST http://valkyrie-routing:8080/admin/circuit-breaker/enable
```

#### Memory Usage Alert
```bash
# 1. Check memory breakdown
cat /proc/$(pgrep valkyrie-routing)/smaps | grep -A1 "heap"

# 2. Force garbage collection
kill -USR2 $(pgrep valkyrie-routing)

# 3. Analyze memory leaks
valgrind --tool=memcheck --leak-check=full ./valkyrie-routing

# 4. Restart if necessary
docker-compose restart valkyrie-routing
```

### Custom Monitoring Scripts

#### Performance Monitoring
```bash
#!/bin/bash
# performance-monitor.sh

while true; do
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    LATENCY=$(curl -s http://valkyrie-routing:9090/metrics | grep quantile=\"0.99\" | awk '{print $2}')
    THROUGHPUT=$(curl -s http://valkyrie-routing:9090/metrics | grep rate | head -1 | awk '{print $2}')
    
    echo "$TIMESTAMP,P99_LATENCY,$LATENCY,THROUGHPUT,$THROUGHPUT" >> /var/log/valkyrie-performance.csv
    
    sleep 60
done
```

---

## Emergency Contacts

- **On-Call Engineer**: +1-XXX-XXX-XXXX
- **Platform Team**: platform-team@company.com
- **Security Team**: security@company.com
- **Management Escalation**: engineering-manager@company.com

## Documentation Updates

This runbook should be reviewed and updated:
- After each incident (within 24 hours)
- Monthly during team retrospectives
- When new features or configurations are deployed
- When infrastructure changes are made

**Last Updated**: [Current Date]
**Version**: 1.0
**Reviewed By**: [Team Lead Name]