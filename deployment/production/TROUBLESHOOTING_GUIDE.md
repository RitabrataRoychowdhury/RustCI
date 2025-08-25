# Valkyrie High-Performance Routing - Troubleshooting Guide

This comprehensive guide helps diagnose and resolve common issues with Valkyrie High-Performance Routing in production environments.

## Table of Contents

1. [Quick Diagnostic Commands](#quick-diagnostic-commands)
2. [Performance Issues](#performance-issues)
3. [Memory Issues](#memory-issues)
4. [Network Issues](#network-issues)
5. [Configuration Issues](#configuration-issues)
6. [Lock-Free Data Structure Issues](#lock-free-data-structure-issues)
7. [Monitoring and Alerting Issues](#monitoring-and-alerting-issues)
8. [Common Error Messages](#common-error-messages)
9. [Advanced Debugging](#advanced-debugging)

## Quick Diagnostic Commands

### System Health Check
```bash
# Quick health assessment
curl -f http://localhost:8080/health
echo "Exit code: $?"

# Service status
systemctl status valkyrie-routing
# or
docker-compose ps valkyrie-routing

# Resource usage
top -p $(pgrep valkyrie-routing)
```

### Performance Metrics
```bash
# Get current performance metrics
curl -s http://localhost:9090/metrics | grep -E "valkyrie_routing_(request_duration|requests_total|errors_total)"

# Check P99 latency
curl -s http://localhost:9090/metrics | grep 'quantile="0.99"' | grep valkyrie_routing_request_duration

# Check throughput
curl -s http://localhost:9090/metrics | grep valkyrie_routing_requests_total | tail -1
```

### Log Analysis
```bash
# Recent errors
journalctl -u valkyrie-routing --since "10 minutes ago" | grep -i error

# Application logs
tail -f /app/logs/valkyrie.log | grep -E "ERROR|WARN|PANIC"

# Performance logs
grep "performance" /app/logs/valkyrie.log | tail -20
```

## Performance Issues

### Issue: High P99 Latency (>82µs)

#### Symptoms
- P99 latency consistently above 82µs threshold
- Grafana dashboards showing latency spikes
- User reports of slow responses

#### Diagnostic Steps
```bash
# 1. Check current latency distribution
curl -s http://localhost:9090/metrics | grep valkyrie_routing_request_duration_seconds_bucket

# 2. Analyze CPU usage
top -H -p $(pgrep valkyrie-routing)

# 3. Check memory pressure
cat /proc/$(pgrep valkyrie-routing)/status | grep -E "VmRSS|VmSize|VmPeak"

# 4. Examine lock-free structure performance
curl -s http://localhost:9090/metrics | grep -E "fit_collisions|cra_fragmentation|rcu_grace_period"
```

#### Common Causes and Solutions

**Cause 1: High CPU Contention**
```bash
# Check CPU affinity
taskset -p $(pgrep valkyrie-routing)

# Solution: Set CPU affinity to dedicated cores
taskset -cp 0-7 $(pgrep valkyrie-routing)

# Update configuration for permanent fix
echo 'cpu_cores = [0, 1, 2, 3, 4, 5, 6, 7]' >> /app/config/production.toml
```

**Cause 2: Memory Allocation Overhead**
```bash
# Check allocation patterns
perf record -e syscalls:sys_enter_mmap -p $(pgrep valkyrie-routing) sleep 10
perf report

# Solution: Enable jemalloc
export LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libjemalloc.so.2
# or update configuration
echo 'enable_jemalloc = true' >> /app/config/production.toml
```

**Cause 3: Lock-Free Structure Degradation**
```bash
# Check FIT collision rate
COLLISIONS=$(curl -s http://localhost:9090/metrics | grep valkyrie_fit_collisions_total | awk '{print $2}')
OPERATIONS=$(curl -s http://localhost:9090/metrics | grep valkyrie_fit_operations_total | awk '{print $2}')
echo "Collision rate: $(echo "scale=4; $COLLISIONS / $OPERATIONS * 100" | bc)%"

# Solution: Increase FIT capacity or reduce load factor
[routing.lockfree]
fit_capacity = 131072  # Double the capacity
fit_load_factor = 0.6  # Reduce load factor
```

### Issue: Low Throughput (<1M ops/sec)

#### Symptoms
- Sustained throughput below 1M operations per second
- Request queues building up
- Increased response times under load

#### Diagnostic Steps
```bash
# 1. Check request rate
curl -s http://localhost:9090/metrics | grep rate | grep valkyrie_routing_requests

# 2. Analyze connection pool utilization
curl -s http://localhost:9090/metrics | grep valkyrie_connection_pool

# 3. Check for bottlenecks
ss -tuln | grep :8080 | wc -l  # Active connections
```

#### Solutions

**Solution 1: Scale Connection Pool**
```toml
[routing]
connection_pool_size = 2000
max_concurrent_requests = 100000
```

**Solution 2: Optimize Worker Threads**
```toml
[server]
worker_threads = 16  # 2x CPU cores
blocking_threads = 32
```

**Solution 3: Enable Performance Optimizations**
```toml
[performance]
enable_simd = true
enable_zero_copy = true
enable_hot_path_optimization = true
```

## Memory Issues

### Issue: Memory Leak

#### Symptoms
- Continuously increasing memory usage
- Out of memory errors after extended runtime
- System becoming unresponsive

#### Diagnostic Steps
```bash
# 1. Monitor memory growth
watch -n 5 'cat /proc/$(pgrep valkyrie-routing)/status | grep VmRSS'

# 2. Check for memory fragmentation
cat /proc/$(pgrep valkyrie-routing)/smaps | awk '/^Size:/ {size+=$2} /^Rss:/ {rss+=$2} END {print "Fragmentation:", (size-rss)/size*100 "%"}'

# 3. Generate memory profile
kill -USR1 $(pgrep valkyrie-routing)  # Trigger heap dump
```

#### Solutions

**Solution 1: Enable Memory Profiling**
```toml
[performance.memory]
enable_memory_profiling = true
gc_threshold = "4GB"
```

**Solution 2: Adjust Cache Sizes**
```toml
[routing]
route_cache_size = 50000  # Reduce if too large
route_cache_ttl = "300s"

[caching.policies]
max_cache_memory = "2GB"  # Set explicit limit
```

**Solution 3: Force Garbage Collection**
```bash
# Manual GC trigger
kill -USR2 $(pgrep valkyrie-routing)

# Restart service if leak persists
systemctl restart valkyrie-routing
```

### Issue: High Memory Fragmentation

#### Symptoms
- High virtual memory usage relative to RSS
- Allocation failures despite available memory
- Performance degradation over time

#### Solutions
```bash
# Enable jemalloc for better allocation patterns
export MALLOC_CONF="dirty_decay_ms:1000,muzzy_decay_ms:5000"

# Use huge pages for large allocations
echo 1024 > /proc/sys/vm/nr_hugepages

# Configure memory compaction
echo 1 > /proc/sys/vm/compact_memory
```

## Network Issues

### Issue: Connection Timeouts

#### Symptoms
- Client connection timeouts
- "Connection refused" errors
- Network-related error spikes

#### Diagnostic Steps
```bash
# 1. Check listening sockets
ss -tuln | grep :8080

# 2. Check connection queue
ss -s | grep TCP

# 3. Monitor network errors
netstat -i | grep -E "RX-ERR|TX-ERR"

# 4. Check firewall rules
iptables -L -n | grep 8080
```

#### Solutions

**Solution 1: Increase Connection Limits**
```bash
# System-wide limits
echo 'net.core.somaxconn = 65536' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_max_syn_backlog = 65536' >> /etc/sysctl.conf
sysctl -p
```

**Solution 2: Optimize TCP Settings**
```bash
# TCP optimization
echo 'net.ipv4.tcp_fin_timeout = 30' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_keepalive_time = 1200' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_keepalive_probes = 3' >> /etc/sysctl.conf
```

**Solution 3: Application Configuration**
```toml
[server]
max_connections = 50000
connection_timeout = "30s"
keepalive_timeout = "60s"
```

### Issue: High Network Latency

#### Diagnostic Steps
```bash
# 1. Check network interface statistics
cat /proc/net/dev

# 2. Monitor packet drops
netstat -i

# 3. Check interrupt distribution
cat /proc/interrupts | grep eth0

# 4. Test network latency
ping -c 10 target-host
```

#### Solutions
```bash
# Enable receive packet steering
echo f > /sys/class/net/eth0/queues/rx-0/rps_cpus

# Optimize interrupt handling
echo 2 > /proc/irq/24/smp_affinity

# Enable TCP BBR congestion control
echo 'net.ipv4.tcp_congestion_control = bbr' >> /etc/sysctl.conf
```

## Configuration Issues

### Issue: Configuration Validation Failures

#### Symptoms
- Service fails to start with configuration errors
- Hot reload operations failing
- Validation warnings in logs

#### Diagnostic Steps
```bash
# 1. Validate configuration syntax
./valkyrie-routing --validate-config /app/config/production.toml

# 2. Check configuration conflicts
./valkyrie-routing --check-conflicts /app/config/production.toml

# 3. Review validation logs
grep "validation" /app/logs/valkyrie.log
```

#### Common Configuration Errors

**Error 1: Invalid Performance Budget**
```
Error: Performance budget "100us" exceeds requirement of 82µs
```
**Solution:**
```toml
[performance]
performance_budget = "75us"  # Must be ≤ 82µs
```

**Error 2: Memory Configuration Conflict**
```
Error: Total memory allocation exceeds system limit
```
**Solution:**
```toml
[performance.memory]
max_heap_size = "4GB"  # Reduce heap size

[caching.policies]
max_cache_memory = "2GB"  # Reduce cache memory
```

**Error 3: Port Conflicts**
```
Error: Port conflict between HTTP and metrics endpoint
```
**Solution:**
```toml
[server]
port = 8080

[observability.metrics]
port = 9090  # Use different port
```

### Issue: Hot Reload Failures

#### Diagnostic Steps
```bash
# 1. Check file permissions
ls -la /app/config/production.toml

# 2. Verify file watcher
lsof | grep inotify

# 3. Check reload logs
grep "hot.reload" /app/logs/valkyrie.log
```

#### Solutions
```bash
# Fix file permissions
chmod 644 /app/config/production.toml
chown valkyrie:valkyrie /app/config/production.toml

# Restart file watcher
systemctl restart valkyrie-routing

# Manual reload trigger
kill -HUP $(pgrep valkyrie-routing)
```

## Lock-Free Data Structure Issues

### Issue: High FIT Collision Rate

#### Symptoms
- FIT collision rate > 10%
- Degraded lookup performance
- Increased P99 latency

#### Diagnostic Steps
```bash
# Check collision metrics
curl -s http://localhost:9090/metrics | grep valkyrie_fit_collisions

# Calculate collision rate
COLLISIONS=$(curl -s http://localhost:9090/metrics | grep valkyrie_fit_collisions_total | awk '{print $2}')
OPERATIONS=$(curl -s http://localhost:9090/metrics | grep valkyrie_fit_operations_total | awk '{print $2}')
echo "Collision rate: $(echo "scale=2; $COLLISIONS / $OPERATIONS * 100" | bc)%"
```

#### Solutions
```toml
# Increase FIT capacity
[routing.lockfree]
fit_capacity = 131072  # Power of 2, larger capacity

# Reduce load factor
fit_load_factor = 0.6  # Lower load factor = fewer collisions

# Optimize hash function
fit_hash_function = "xxhash"  # Use faster hash function
```

### Issue: CRA Memory Fragmentation

#### Symptoms
- High CRA fragmentation ratio (>30%)
- Increased memory usage
- Slower traversal times

#### Diagnostic Steps
```bash
# Check fragmentation metrics
curl -s http://localhost:9090/metrics | grep valkyrie_cra_fragmentation_ratio
```

#### Solutions
```toml
# Enable compression
[routing.lockfree]
cra_enable_compression = true

# Increase initial capacity
cra_initial_capacity = 65536

# Trigger defragmentation
cra_defrag_threshold = 0.25
```

### Issue: RCU Grace Period Too Long

#### Symptoms
- RCU grace period > 50ms
- Slow update operations
- Memory not being reclaimed promptly

#### Diagnostic Steps
```bash
# Check RCU metrics
curl -s http://localhost:9090/metrics | grep valkyrie_rcu_grace_period_seconds
```

#### Solutions
```toml
# Reduce grace period
[routing.lockfree]
rcu_grace_period = "5ms"

# Increase update frequency
rcu_max_pending_updates = 5000

# Enable aggressive reclamation
rcu_aggressive_reclaim = true
```

## Monitoring and Alerting Issues

### Issue: Missing Metrics

#### Symptoms
- Prometheus scraping failures
- Missing data in Grafana dashboards
- Alert rules not firing

#### Diagnostic Steps
```bash
# 1. Check metrics endpoint
curl -f http://localhost:9090/metrics

# 2. Verify Prometheus configuration
curl -f http://prometheus:9090/api/v1/targets

# 3. Check network connectivity
telnet valkyrie-routing 9090
```

#### Solutions
```bash
# Restart metrics collection
systemctl restart valkyrie-routing

# Check firewall rules
iptables -L | grep 9090

# Verify configuration
[observability.metrics]
enable = true
port = 9090
path = "/metrics"
```

### Issue: Alert Fatigue

#### Symptoms
- Too many false positive alerts
- Important alerts being ignored
- Alert storm conditions

#### Solutions
```yaml
# Adjust alert thresholds
- alert: HighLatency
  expr: histogram_quantile(0.99, rate(valkyrie_routing_request_duration_seconds_bucket[5m])) > 0.000090  # Increased threshold
  for: 5m  # Increased duration

# Add alert grouping
route:
  group_by: ['alertname', 'instance']
  group_wait: 30s
  group_interval: 5m
```

## Common Error Messages

### "Performance budget exceeded"
**Cause**: P99 latency above configured threshold
**Solution**: Optimize performance or increase budget limit

### "Connection pool exhausted"
**Cause**: All connections in use
**Solution**: Increase pool size or optimize connection usage

### "Memory allocation failed"
**Cause**: Out of memory or fragmentation
**Solution**: Increase memory limits or enable jemalloc

### "Lock-free structure corruption detected"
**Cause**: Concurrent modification bug
**Solution**: Update to latest version or report bug

### "Configuration validation failed"
**Cause**: Invalid configuration parameters
**Solution**: Fix configuration and reload

## Advanced Debugging

### Performance Profiling
```bash
# CPU profiling with perf
perf record -g -p $(pgrep valkyrie-routing) sleep 30
perf report

# Memory profiling with valgrind
valgrind --tool=massif --massif-out-file=massif.out ./valkyrie-routing
ms_print massif.out

# Lock contention analysis
perf record -e syscalls:sys_enter_futex -p $(pgrep valkyrie-routing) sleep 10
```

### Network Debugging
```bash
# Packet capture
tcpdump -i eth0 -w capture.pcap port 8080

# Connection analysis
ss -tuln | grep :8080

# Bandwidth monitoring
iftop -i eth0
```

### System Tracing
```bash
# System call tracing
strace -p $(pgrep valkyrie-routing) -e trace=network

# File system monitoring
inotifywait -m /app/config/

# Process monitoring
htop -p $(pgrep valkyrie-routing)
```

### Core Dump Analysis
```bash
# Enable core dumps
ulimit -c unlimited
echo '/tmp/core.%e.%p' > /proc/sys/kernel/core_pattern

# Analyze core dump
gdb ./valkyrie-routing /tmp/core.valkyrie-routing.12345
(gdb) bt
(gdb) info registers
```

## Getting Help

### Information to Collect
When reporting issues, please collect:

1. **System Information**:
   ```bash
   uname -a
   cat /proc/cpuinfo | grep "model name" | head -1
   free -h
   df -h
   ```

2. **Application Logs**:
   ```bash
   journalctl -u valkyrie-routing --since "1 hour ago" > valkyrie.log
   ```

3. **Performance Metrics**:
   ```bash
   curl -s http://localhost:9090/metrics > metrics.txt
   ```

4. **Configuration**:
   ```bash
   cat /app/config/production.toml > config.toml
   ```

### Support Channels
- **GitHub Issues**: For bugs and feature requests
- **Documentation**: Check the comprehensive docs
- **Community Forum**: For general questions
- **Professional Support**: For enterprise customers

---

**Last Updated**: [Current Date]
**Version**: 1.0
**Maintained By**: Platform Engineering Team