# Troubleshooting Guide

This guide helps you diagnose and resolve common issues with RustCI.

## Common Issues

### Installation and Setup

#### Issue: Cargo build fails
**Symptoms:**
- Build errors during `cargo build`
- Missing dependencies
- Linking errors

**Solutions:**
```bash
# Update Rust toolchain
rustup update

# Clean and rebuild
cargo clean
cargo build

# Install system dependencies (Linux)
sudo apt-get install build-essential pkg-config libssl-dev

# Install system dependencies (macOS)
xcode-select --install
brew install openssl pkg-config
```

#### Issue: Database connection fails
**Symptoms:**
- "Connection refused" errors
- Authentication failures
- Timeout errors

**Solutions:**
```bash
# Check MongoDB is running
docker ps | grep mongo
systemctl status mongodb

# Test connection manually
mongo mongodb://localhost:27017/rustci

# Verify connection string in config
# Check username/password match
```

### Server Issues

#### Issue: Server won't start
**Symptoms:**
- Server exits immediately
- Port binding errors
- Configuration errors

**Solutions:**
```bash
# Check port availability
lsof -i :8080
netstat -tulpn | grep 8080

# Validate configuration
rustci config validate

# Check logs for detailed errors
tail -f logs/rustci.log
```

#### Issue: High memory usage
**Symptoms:**
- Server consuming excessive memory
- Out of memory errors
- Slow performance

**Solutions:**
```bash
# Monitor memory usage
top -p $(pgrep rustci)

# Adjust configuration
# Reduce max_connections
# Enable memory pooling
# Tune garbage collection
```

### Pipeline Issues

#### Issue: Pipeline fails to start
**Symptoms:**
- Pipeline stuck in "pending" state
- No runners available
- Resource allocation errors

**Solutions:**
```bash
# Check runner status
curl http://localhost:8080/api/v1/runners

# Verify runner configuration
# Check Docker daemon is running
# Ensure sufficient resources
```

#### Issue: Pipeline timeout
**Symptoms:**
- Pipeline exceeds time limit
- Steps hang indefinitely
- No progress updates

**Solutions:**
```yaml
# Increase timeout in pipeline config
timeout_seconds: 7200

# Add step-level timeouts
steps:
  - name: "long-running-task"
    timeout: 3600
    command: "..."
```

### Docker Runner Issues

#### Issue: Docker permission denied
**Symptoms:**
- "Permission denied" when accessing Docker
- "Cannot connect to Docker daemon"

**Solutions:**
```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Restart shell or logout/login
# Or use newgrp docker

# Verify Docker access
docker ps
```

#### Issue: Container cleanup fails
**Symptoms:**
- Containers not being removed
- Disk space issues
- Resource leaks

**Solutions:**
```bash
# Manual cleanup
docker container prune -f
docker image prune -f

# Enable auto-cleanup in config
runners:
  docker:
    cleanup_containers: true
    cleanup_images: true
```

### Kubernetes Runner Issues

#### Issue: Pod creation fails
**Symptoms:**
- Pods stuck in "Pending" state
- Resource quota exceeded
- Image pull errors

**Solutions:**
```bash
# Check cluster resources
kubectl describe nodes
kubectl get pods -n rustci-runners

# Verify RBAC permissions
kubectl auth can-i create pods --as=system:serviceaccount:rustci:rustci-runner

# Check resource quotas
kubectl describe quota -n rustci-runners
```

### Communication Protocol Issues

#### Issue: High latency
**Symptoms:**
- Slow API responses
- Pipeline delays
- Timeout errors

**Solutions:**
```yaml
# Tune performance settings
communication_protocol:
  performance:
    worker_threads: 16
    max_connections: 50000
    buffer_size: 65536
```

#### Issue: Connection drops
**Symptoms:**
- Frequent disconnections
- "Connection reset" errors
- Intermittent failures

**Solutions:**
```yaml
# Enable keep-alive
communication_protocol:
  transport:
    tcp:
      keep_alive: true
      keep_alive_interval: 30
```

## Diagnostic Commands

### System Information
```bash
# Check system resources
free -h
df -h
top

# Check network connectivity
ping localhost
netstat -tulpn | grep rustci
```

### RustCI Status
```bash
# Health check
curl http://localhost:8080/health

# System metrics
curl http://localhost:8080/metrics

# Runner status
curl http://localhost:8080/api/v1/runners

# Pipeline status
curl http://localhost:8080/api/v1/pipelines
```

### Log Analysis
```bash
# View recent logs
tail -f logs/rustci.log

# Search for errors
grep -i error logs/rustci.log

# Filter by component
grep "communication_protocol" logs/rustci.log
```

## Performance Tuning

### Memory Optimization
```yaml
performance:
  memory:
    enable_pooling: true
    buffer_pool_size: 1000
    max_buffer_size: 1048576
```

### Network Optimization
```yaml
performance:
  network:
    tcp_fast_open: true
    send_buffer_size: 262144
    recv_buffer_size: 262144
```

### Thread Pool Tuning
```yaml
performance:
  thread_pool:
    core_threads: 0  # Auto-detect
    max_threads: 1000
    keep_alive_time: 60
```

## Monitoring and Alerting

### Key Metrics to Monitor
- CPU and memory usage
- Pipeline success/failure rates
- API response times
- Database connection pool
- Runner availability

### Setting Up Alerts
```yaml
# Example Prometheus alerts
groups:
  - name: rustci
    rules:
      - alert: HighErrorRate
        expr: rate(rustci_errors_total[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
```

## Getting Help

### Self-Service Resources
1. Check this troubleshooting guide
2. Review [configuration documentation](configuration.md)
3. Search [GitHub issues](https://github.com/rustci/rustci/issues)
4. Read [architecture documentation](../architecture/README.md)

### Community Support
1. [GitHub Discussions](https://github.com/rustci/rustci/discussions)
2. [Discord Community](https://discord.gg/rustci)
3. [Stack Overflow](https://stackoverflow.com/questions/tagged/rustci)

### Reporting Issues
When reporting issues, please include:
- RustCI version (`rustci --version`)
- Operating system and version
- Configuration file (sanitized)
- Error logs and stack traces
- Steps to reproduce

### Professional Support
For enterprise customers:
- Priority support tickets
- Dedicated support engineer
- Custom troubleshooting assistance
- Performance optimization consulting

## Related Documentation

- [Configuration Guide](configuration.md) - Configuration options and examples
- [Installation Guide](installation.md) - Installation instructions
- [API Documentation](../api/README.md) - API reference
- [Architecture Documentation](../architecture/README.md) - System architecture