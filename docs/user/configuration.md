# Configuration Guide

This guide covers advanced configuration options for RustCI, including environment variables, configuration files, and deployment-specific settings.

## Configuration Files

RustCI uses YAML configuration files for most settings. The main configuration files are:

- `config/rustci.yaml` - Main application configuration
- `config/deployment/rustci.yaml` - Production deployment configuration
- `.env` - Environment variables

## Environment Variables

### Core Settings

```bash
# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8000
RUST_LOG=info

# Database Configuration
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=rustci

# Authentication
JWT_SECRET=your-super-secret-jwt-key-here
JWT_EXPIRES_IN=24h
```

### GitHub Integration

```bash
# GitHub OAuth (Optional)
GITHUB_OAUTH_CLIENT_ID=your-github-client-id
GITHUB_OAUTH_CLIENT_SECRET=your-github-client-secret
GITHUB_OAUTH_REDIRECT_URL=http://localhost:8000/api/sessions/oauth/github/callback
```

### Docker Configuration

```bash
# Docker Settings
DOCKER_HOST=unix:///var/run/docker.sock
DOCKER_API_VERSION=1.41
```

### Kubernetes Configuration

```bash
# Kubernetes Settings
KUBECONFIG=/path/to/your/kubeconfig
KUBERNETES_NAMESPACE=rustci
```

## Advanced Configuration

### Performance Tuning

```yaml
# config/rustci.yaml
performance:
  worker_threads: 0  # Auto-detect based on CPU cores
  max_blocking_threads: 512
  thread_stack_size: 2097152  # 2MB
  
  # Memory Configuration
  memory_pool_size: 1073741824  # 1GB
  enable_memory_pool: true
  gc_interval: "60s"
```

### Security Configuration

```yaml
security:
  # TLS Configuration
  tls:
    enabled: true
    cert_file: "/etc/rustci/tls/tls.crt"
    key_file: "/etc/rustci/tls/tls.key"
    ca_file: "/etc/rustci/tls/ca.crt"
    min_version: "1.3"
```

### Observability Configuration

```yaml
observability:
  # Metrics Configuration
  metrics:
    enabled: true
    bind_address: "0.0.0.0:9090"
    path: "/metrics"
    update_interval: "15s"
  
  # Logging Configuration
  logging:
    level: "info"
    format: "json"
    output: "stdout"
```

## Deployment-Specific Configuration

### Docker Compose

```yaml
# docker-compose.yaml
version: '3.8'
services:
  rustci:
    image: rustci:latest
    environment:
      - MONGODB_URI=mongodb://mongo:27017
      - RUST_LOG=info
    volumes:
      - ./config/rustci.yaml:/app/config.yaml
```

### Kubernetes

```yaml
# kubernetes-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rustci-config
data:
  config.yaml: |
    server:
      host: "0.0.0.0"
      port: 8000
    database:
      uri: "mongodb://mongo:27017"
```

## Configuration Validation

RustCI validates configuration on startup. Common validation errors:

### Invalid YAML Syntax

```bash
Error: Invalid YAML syntax in config file
  --> config/rustci.yaml:15:3
   |
15 |   invalid: yaml: content
   |   ^^^^^^^ unexpected token
```

### Missing Required Fields

```bash
Error: Missing required configuration field
  --> config/rustci.yaml
   |
   | Missing field: database.uri
   | This field is required for database connectivity
```

### Invalid Values

```bash
Error: Invalid configuration value
  --> config/rustci.yaml:8:12
   |
8  |   port: "invalid"
   |         ^^^^^^^^^ expected integer, found string
```

## Environment-Specific Configuration

### Development

```yaml
# config/development.yaml
development:
  enabled: true
  debug_mode: true
  hot_reload: true
  mock_external_services: true
```

### Production

```yaml
# config/production.yaml
production:
  security:
    tls:
      enabled: true
      verify_client_cert: true
  
  performance:
    worker_threads: 16
    enable_optimizations: true
  
  observability:
    metrics:
      enabled: true
    tracing:
      enabled: true
      sample_rate: 0.1
```

### Testing

```yaml
# config/testing.yaml
testing:
  database:
    uri: "mongodb://localhost:27017/rustci_test"
  
  logging:
    level: "debug"
  
  features:
    enable_test_endpoints: true
```

## Configuration Best Practices

### Security

1. **Never commit secrets** to version control
2. **Use environment variables** for sensitive data
3. **Enable TLS** in production
4. **Rotate secrets** regularly

### Performance

1. **Tune worker threads** based on your workload
2. **Configure memory limits** appropriately
3. **Enable compression** for network traffic
4. **Use connection pooling** for databases

### Monitoring

1. **Enable metrics collection**
2. **Configure structured logging**
3. **Set up health checks**
4. **Monitor resource usage**

## Troubleshooting Configuration

### Configuration Not Loading

```bash
# Check file permissions
ls -la config/rustci.yaml

# Validate YAML syntax
yamllint config/rustci.yaml

# Check environment variables
env | grep RUSTCI
```

### Database Connection Issues

```bash
# Test MongoDB connection
mongo --eval "db.adminCommand('ismaster')" $MONGODB_URI

# Check network connectivity
telnet mongodb-host 27017
```

### TLS Certificate Issues

```bash
# Verify certificate validity
openssl x509 -in /etc/rustci/tls/tls.crt -text -noout

# Check certificate chain
openssl verify -CAfile /etc/rustci/tls/ca.crt /etc/rustci/tls/tls.crt
```

## Configuration Schema

For a complete reference of all configuration options, see the [Configuration Schema](../api/configuration-schema.md).

## Migration from Legacy Configuration

If you're migrating from an older version of RustCI, see the [Migration Guide](../migration/configuration-migration.md) for step-by-step instructions.