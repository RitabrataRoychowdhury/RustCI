# Configuration Migration Guide

This guide helps you migrate configuration files from older versions of RustCI to the current version.

## Migration Overview

RustCI configuration has evolved to provide better organization, security, and functionality. This guide covers:

- Breaking changes in configuration structure
- Deprecated configuration options
- New configuration features
- Step-by-step migration instructions

## Version Migration Paths

### From v1.x to v2.x

#### Major Changes

1. **Communication Protocol Rebranding**
   - `valkyrie` section renamed to `communication_protocol`
   - Environment variables prefixed with `RUSTCI_COMMUNICATION_` instead of `VALKYRIE_`

2. **Security Configuration Restructure**
   - Security settings moved under respective components
   - New authentication methods added
   - TLS configuration simplified

3. **Runner Configuration Updates**
   - Runner-specific settings consolidated
   - New native runner options
   - Kubernetes runner enhancements

#### Breaking Changes

| Old Configuration | New Configuration | Notes |
|------------------|-------------------|-------|
| `valkyrie.enabled` | `communication_protocol.enabled` | Section renamed |
| `valkyrie.transport` | `communication_protocol.transport` | Same functionality |
| `security.global` | `communication_protocol.security` | Moved to component level |
| `runners.pool_size` | `runners.docker.max_concurrent` | More specific configuration |

### Migration Steps

#### Step 1: Backup Current Configuration

```bash
# Backup your current configuration
cp config/rustci.yaml config/rustci.yaml.backup
```

#### Step 2: Update Section Names

**Old Configuration:**
```yaml
valkyrie:
  enabled: true
  transport: tcp
  port: 8080
```

**New Configuration:**
```yaml
communication_protocol:
  enabled: true
  transport: tcp
  port: 8080
```

#### Step 3: Migrate Security Settings

**Old Configuration:**
```yaml
security:
  global:
    encryption: true
    authentication: jwt
    jwt_secret: "your-secret"
```

**New Configuration:**
```yaml
communication_protocol:
  security:
    encryption: true
    authentication: true
    authentication_method: jwt
    jwt:
      secret_key: "your-secret"
```

#### Step 4: Update Runner Configuration

**Old Configuration:**
```yaml
runners:
  pool_size: 10
  docker_enabled: true
  kubernetes_enabled: false
```

**New Configuration:**
```yaml
runners:
  docker:
    enabled: true
    max_concurrent: 10
  kubernetes:
    enabled: false
  native:
    enabled: true
```

#### Step 5: Update Environment Variables

Update your environment variables to use the new naming convention:

| Old Variable | New Variable |
|-------------|-------------|
| `VALKYRIE_TOKEN_SECRET` | `RUSTCI_COMMUNICATION_TOKEN_SECRET` |
| `VALKYRIE_ENABLED` | `RUSTCI_COMMUNICATION_ENABLED` |
| `VALKYRIE_PORT` | `RUSTCI_COMMUNICATION_PORT` |

#### Step 6: Validate Configuration

```bash
# Validate your new configuration
rustci config validate --config config/rustci.yaml

# Test configuration loading
rustci config test --config config/rustci.yaml
```

## Automated Migration Tool

RustCI provides an automated migration tool to help with configuration updates:

```bash
# Run the migration tool
rustci migrate config --from v1 --to v2 --input config/rustci.yaml.backup --output config/rustci.yaml

# Verify the migration
rustci config diff --old config/rustci.yaml.backup --new config/rustci.yaml
```

## Common Migration Issues

### Issue 1: Invalid Section Names

**Error:**
```
Configuration error: Unknown section 'valkyrie'
```

**Solution:**
Rename `valkyrie` to `communication_protocol` in your configuration file.

### Issue 2: Deprecated Security Configuration

**Error:**
```
Configuration warning: 'security.global' is deprecated
```

**Solution:**
Move security settings to component-specific sections:

```yaml
# Instead of:
security:
  global:
    encryption: true

# Use:
communication_protocol:
  security:
    encryption: true
```

### Issue 3: Runner Pool Configuration

**Error:**
```
Configuration error: 'runners.pool_size' is no longer supported
```

**Solution:**
Use runner-specific configuration:

```yaml
# Instead of:
runners:
  pool_size: 10

# Use:
runners:
  docker:
    max_concurrent: 10
```

## Configuration Schema Changes

### New Features in v2.x

1. **Enhanced Security Options**
   - mTLS support
   - API key authentication
   - Post-quantum cryptography (experimental)

2. **Improved Observability**
   - Structured logging
   - Distributed tracing
   - Custom metrics

3. **Advanced Runner Features**
   - Native runner with isolation
   - Kubernetes resource limits
   - Container cleanup policies

### Deprecated Features

The following features are deprecated and will be removed in future versions:

- `security.global` - Use component-specific security settings
- `runners.pool_size` - Use runner-specific `max_concurrent` settings
- `valkyrie.*` - Use `communication_protocol.*` settings

## Testing Your Migration

### Configuration Validation

```bash
# Validate configuration syntax
rustci config validate

# Test configuration loading
rustci config test

# Check for deprecated options
rustci config lint
```

### Functional Testing

```bash
# Start RustCI with new configuration
rustci server start --config config/rustci.yaml

# Run a test pipeline
rustci pipeline run --file test-pipeline.yaml

# Check system health
rustci health check
```

## Rollback Procedure

If you encounter issues with the new configuration:

```bash
# Stop RustCI
rustci server stop

# Restore backup configuration
cp config/rustci.yaml.backup config/rustci.yaml

# Start with old configuration
rustci server start --config config/rustci.yaml
```

## Getting Help

If you encounter issues during migration:

1. Check the [troubleshooting guide](../user/troubleshooting.md)
2. Review the [configuration schema](../api/configuration-schema.md)
3. Join our [community discussions](https://github.com/rustci/rustci/discussions)
4. Open an [issue](https://github.com/rustci/rustci/issues) if you find bugs

## Related Documentation

- [Configuration Guide](../user/configuration.md) - Complete configuration reference
- [Configuration Schema](../api/configuration-schema.md) - Detailed schema documentation
- [Deployment Guide](../deployment/README.md) - Deployment-specific configuration
- [API Documentation](../api/README.md) - API reference