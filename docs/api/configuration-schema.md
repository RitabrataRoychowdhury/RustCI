# Configuration Schema Reference

This document provides a comprehensive reference for all RustCI configuration options.

## Configuration File Structure

RustCI uses YAML configuration files with the following top-level sections:

```yaml
# Global settings
global:
  environment: "development"
  log_level: "info"
  enable_metrics: true

# Server configuration
server:
  bind_address: "0.0.0.0"
  port: 8080
  max_connections: 10000

# Communication protocol settings
communication_protocol:
  enabled: true
  transport: "tcp"
  security:
    encryption: true
    authentication: true

# Runner configuration
runners:
  docker:
    enabled: true
  kubernetes:
    enabled: false
  native:
    enabled: true

# Pipeline settings
pipelines:
  max_concurrent: 100
  timeout_seconds: 3600
  retry_attempts: 3

# Storage configuration
storage:
  type: "mongodb"
  connection_string: "mongodb://localhost:27017/rustci"

# Observability settings
observability:
  metrics:
    enabled: true
    endpoint: "/metrics"
  tracing:
    enabled: true
    backend: "jaeger"
  logging:
    level: "info"
    format: "json"
```

## Configuration Sections

### Global Settings

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `environment` | string | `"development"` | Deployment environment |
| `log_level` | string | `"info"` | Logging level (trace, debug, info, warn, error) |
| `enable_metrics` | boolean | `true` | Enable metrics collection |
| `config_reload_interval` | integer | `30` | Config hot-reload interval in seconds |

### Server Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `bind_address` | string | `"0.0.0.0"` | Server bind address |
| `port` | integer | `8080` | Server port |
| `max_connections` | integer | `10000` | Maximum concurrent connections |
| `connection_timeout_ms` | integer | `30000` | Connection timeout in milliseconds |
| `enable_tls` | boolean | `false` | Enable TLS/SSL |
| `tls_cert_path` | string | `""` | TLS certificate path |
| `tls_key_path` | string | `""` | TLS private key path |

### Communication Protocol

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable communication protocol |
| `transport` | string | `"tcp"` | Transport type (tcp, quic, unix, websocket) |
| `max_message_size` | integer | `16777216` | Maximum message size in bytes |
| `compression` | boolean | `true` | Enable message compression |

#### Security Settings

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `encryption` | boolean | `true` | Enable encryption |
| `encryption_algorithm` | string | `"chacha20poly1305"` | Encryption algorithm |
| `authentication` | boolean | `true` | Enable authentication |
| `authentication_method` | string | `"jwt"` | Authentication method (jwt, mtls, apikey) |

### Runner Configuration

#### Docker Runner

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable Docker runner |
| `socket_path` | string | `"/var/run/docker.sock"` | Docker socket path |
| `network_mode` | string | `"bridge"` | Docker network mode |
| `cleanup_containers` | boolean | `true` | Auto-cleanup containers |

#### Kubernetes Runner

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable Kubernetes runner |
| `namespace` | string | `"rustci-runners"` | Kubernetes namespace |
| `kubeconfig_path` | string | `""` | Kubeconfig file path |
| `resource_limits` | object | `{}` | Resource limits for pods |

#### Native Runner

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable native runner |
| `working_directory` | string | `"/tmp/rustci"` | Working directory |
| `environment_isolation` | boolean | `true` | Enable environment isolation |

### Pipeline Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_concurrent` | integer | `100` | Maximum concurrent pipelines |
| `timeout_seconds` | integer | `3600` | Default pipeline timeout |
| `retry_attempts` | integer | `3` | Default retry attempts |
| `artifact_retention_days` | integer | `30` | Artifact retention period |

### Storage Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `type` | string | `"mongodb"` | Storage backend type |
| `connection_string` | string | `""` | Database connection string |
| `database_name` | string | `"rustci"` | Database name |
| `connection_pool_size` | integer | `10` | Connection pool size |

### Observability Configuration

#### Metrics

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable metrics collection |
| `endpoint` | string | `"/metrics"` | Metrics endpoint path |
| `collection_interval` | integer | `10` | Collection interval in seconds |
| `custom_metrics` | boolean | `true` | Enable custom metrics |

#### Tracing

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable distributed tracing |
| `backend` | string | `"jaeger"` | Tracing backend (jaeger, zipkin, otlp) |
| `sampling_rate` | float | `0.1` | Sampling rate (0.0 to 1.0) |
| `service_name` | string | `"rustci"` | Service name for tracing |

#### Logging

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `level` | string | `"info"` | Log level |
| `format` | string | `"json"` | Log format (json, text) |
| `structured` | boolean | `true` | Enable structured logging |
| `file_path` | string | `""` | Log file path (optional) |

## Environment Variables

Configuration values can be overridden using environment variables:

| Environment Variable | Configuration Path | Description |
|---------------------|-------------------|-------------|
| `RUSTCI_LOG_LEVEL` | `global.log_level` | Logging level |
| `RUSTCI_PORT` | `server.port` | Server port |
| `RUSTCI_DATABASE_URL` | `storage.connection_string` | Database connection |
| `RUSTCI_JWT_SECRET` | `communication_protocol.security.jwt.secret` | JWT secret key |

## Configuration Validation

RustCI validates configuration on startup and provides detailed error messages for invalid configurations. Common validation rules include:

- Required fields must be present
- Numeric values must be within valid ranges
- String values must match allowed patterns
- File paths must be accessible
- Network addresses must be valid

## Examples

### Development Configuration

```yaml
global:
  environment: "development"
  log_level: "debug"

server:
  port: 8080

communication_protocol:
  security:
    encryption: false
    authentication: false

runners:
  docker:
    enabled: true
  kubernetes:
    enabled: false
```

### Production Configuration

```yaml
global:
  environment: "production"
  log_level: "warn"

server:
  port: 443
  enable_tls: true
  tls_cert_path: "/etc/ssl/certs/rustci.crt"
  tls_key_path: "/etc/ssl/private/rustci.key"

communication_protocol:
  security:
    encryption: true
    authentication: true
    encryption_algorithm: "aes256gcm"

storage:
  type: "mongodb"
  connection_string: "mongodb://mongo-cluster:27017/rustci"
  connection_pool_size: 50

observability:
  metrics:
    enabled: true
  tracing:
    enabled: true
    sampling_rate: 0.01
```

## Related Documentation

- [Configuration Guide](../user/configuration.md) - User-friendly configuration guide
- [API Reference](README.md) - Complete API documentation
- [Deployment Guide](../deployment/README.md) - Deployment-specific configuration