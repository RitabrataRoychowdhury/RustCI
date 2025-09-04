# Middleware and Security System

This document describes the comprehensive middleware and security system implemented in RustCI.

## Overview

The middleware system provides a layered security approach with the following components:

1. **Request Validation and Sanitization**
2. **Rate Limiting and Throttling**
3. **Authentication and Authorization (RBAC)**
4. **Security Headers and CSP**
5. **Audit Logging and Monitoring**
6. **Hot-reloadable Configuration**

## Architecture

### Middleware Pipeline

The middleware pipeline processes requests in the following order:

```
Request → Pre-processing → Security → Authorization → Rate Limiting → Handler → Post-processing → Response
```

#### Phase 1: Pre-processing and Validation
- Request size validation (max 100MB)
- HTTP method validation
- Header security validation (injection prevention)
- URL path validation (directory traversal prevention)
- Query parameter sanitization

#### Phase 2: Security and Authentication
- JWT token extraction and verification
- Security context creation
- Session validation
- User authentication

#### Phase 3: Authorization and RBAC
- Role-based access control
- Permission checking
- Endpoint-specific authorization
- Resource-level permissions

#### Phase 4: Rate Limiting and Throttling
- IP-based rate limiting
- User-based rate limiting
- Endpoint-specific limits
- Sliding window algorithm

#### Phase 5: Request Processing
- Handler execution
- Business logic processing

#### Phase 6: Post-processing and Response Enhancement
- Security headers addition
- Performance headers
- Custom headers
- Response sanitization

#### Phase 7: Audit Logging
- Request/response logging
- Security event logging
- Performance metrics
- Compliance tracking

## Security Features

### Authentication

The system supports multiple authentication methods:

- **JWT Tokens**: Stateless authentication with configurable expiration
- **OAuth Integration**: GitHub OAuth support
- **Session Management**: Secure session handling with configurable timeouts

```rust
// JWT Configuration
jwt:
  secret: "your-super-secure-jwt-secret-key-at-least-32-characters-long"
  expires_in_seconds: 3600
  refresh_expires_in_seconds: 86400
  issuer: "rustci"
  audience: "rustci-api"
  algorithm: "HS256"
```

### Authorization (RBAC)

Role-Based Access Control with hierarchical roles:

- **Admin**: Full system access
- **Developer**: Pipeline management and execution
- **Viewer**: Read-only access
- **ServiceAccount**: API access for automation

```rust
// RBAC Configuration
rbac:
  enabled: true
  default_role: "Viewer"
  role_hierarchy:
    Admin:
      - "Developer"
      - "Viewer"
    Developer:
      - "Viewer"
```

### Rate Limiting

Advanced rate limiting with multiple algorithms:

- **Sliding Window**: Smooth rate limiting
- **Per-IP Limits**: Prevent abuse from single IPs
- **Per-User Limits**: Authenticated user limits
- **Endpoint-Specific Limits**: Different limits per endpoint

```rust
// Rate Limiting Configuration
rate_limiting:
  requests_per_minute: 60
  burst_size: 10
  enable_per_user_limits: true
  whitelist_ips:
    - "127.0.0.1"
    - "::1"
```

### Security Headers

Comprehensive security headers automatically added:

- **X-Content-Type-Options**: `nosniff`
- **X-Frame-Options**: `DENY`
- **X-XSS-Protection**: `1; mode=block`
- **Strict-Transport-Security**: HSTS for HTTPS
- **Content-Security-Policy**: Configurable CSP
- **Referrer-Policy**: `strict-origin-when-cross-origin`
- **Permissions-Policy**: Feature policy restrictions

### Encryption

Data encryption support:

- **AES-256-GCM**: Symmetric encryption for sensitive data
- **Base64 Encoding**: Safe data transmission
- **Key Management**: Secure key storage and rotation

```rust
// Encryption Configuration
encryption:
  key: "base64-encoded-32-byte-encryption-key"
  algorithm: "AES-256-GCM"
```

## Configuration Management

### Hot-reloadable Configuration

The system supports hot-reloading of configuration without restart:

```rust
// Enable hot reload
features:
  enable_hot_reload: true
```

Configuration changes are detected automatically and applied with validation.

### Type-safe Configuration

All configuration is validated at startup and runtime:

- **Schema Validation**: Ensures correct structure
- **Type Checking**: Validates data types
- **Range Validation**: Checks value ranges
- **Dependency Validation**: Validates related settings
- **Environment-specific Validation**: Different rules per environment

### Configuration Validation Engine

The validation engine provides:

- **Comprehensive Rules**: Over 50 validation rules
- **Environment Awareness**: Different validation per environment
- **Auto-fix Suggestions**: Automatic correction suggestions
- **Caching**: Performance optimization
- **Detailed Reporting**: Clear error messages and warnings

## Audit Logging

### Enhanced Audit System

Comprehensive audit logging with:

- **Event Filtering**: Configurable event filtering
- **Data Enrichment**: Automatic context addition
- **Real-time Alerts**: Critical event notifications
- **Buffered Logging**: Performance optimization
- **Retention Management**: Automatic cleanup

```rust
// Audit Configuration
audit:
  enabled: true
  log_all_requests: false
  log_failed_auth: true
  retention_days: 90
  sensitive_fields:
    - "password"
    - "token"
    - "secret"
```

### Audit Events

The system logs various security events:

- **Authentication Events**: Login, logout, token refresh
- **Authorization Events**: Permission checks, role changes
- **Pipeline Events**: Creation, execution, deletion
- **System Events**: Configuration changes, errors
- **Security Events**: Failed attempts, suspicious activity

## Performance Considerations

### Optimizations

- **Async Processing**: Non-blocking middleware execution
- **Connection Pooling**: Efficient database connections
- **Caching**: Configuration and validation caching
- **Buffered Logging**: Batch audit log writes
- **Lazy Loading**: On-demand resource loading

### Monitoring

Built-in performance monitoring:

- **Request Duration**: Track processing time
- **Memory Usage**: Monitor resource consumption
- **Error Rates**: Track failure rates
- **Throughput**: Measure requests per second

## Usage Examples

### Basic Setup

```rust
use middleware::MiddlewarePipelineManager;

// In main.rs
let app = Router::new()
    .route("/api/healthchecker", get(health_check_handler))
    .nest("/api/ci", ci_router(state.clone()))
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        MiddlewarePipelineManager::execute_pipeline,
    ));
```

### Custom Middleware

```rust
use middleware::{require_permission, require_role};
use core::security::{Permission, Role};

// Require specific permission
let protected_route = Router::new()
    .route("/admin", get(admin_handler))
    .layer(axum::middleware::from_fn(
        require_permission(Permission::ManageSystem)
    ));

// Require specific role
let admin_route = Router::new()
    .route("/admin", get(admin_handler))
    .layer(axum::middleware::from_fn(
        require_role(Role::Admin)
    ));
```

### Configuration Validation

```rust
use config::validation_engine::ConfigValidationEngine;

let engine = ConfigValidationEngine::new("production");
let report = engine.validate(&config).await?;

if !report.is_valid() {
    for (validator, report) in &report.validator_reports {
        for (error_key, error_msg) in &report.errors {
            eprintln!("Error: {}.{}: {}", validator, error_key, error_msg);
        }
    }
}
```

## Security Best Practices

### Production Deployment

1. **Use HTTPS**: Always use TLS in production
2. **Secure Cookies**: Enable secure cookie flags
3. **Strong Secrets**: Use cryptographically secure secrets
4. **Rate Limiting**: Configure appropriate rate limits
5. **Audit Logging**: Enable comprehensive audit logging
6. **Regular Updates**: Keep dependencies updated

### Configuration Security

1. **Environment Variables**: Use env vars for secrets
2. **Key Rotation**: Regularly rotate encryption keys
3. **Least Privilege**: Use minimal required permissions
4. **Network Security**: Restrict network access
5. **Monitoring**: Set up security monitoring

### Development Guidelines

1. **Input Validation**: Always validate user input
2. **Error Handling**: Don't leak sensitive information
3. **Logging**: Log security events appropriately
4. **Testing**: Include security tests
5. **Code Review**: Review security-related changes

## Troubleshooting

### Common Issues

1. **Authentication Failures**
   - Check JWT secret configuration
   - Verify token expiration settings
   - Ensure proper header format

2. **Rate Limiting Issues**
   - Check IP whitelist configuration
   - Verify rate limit settings
   - Monitor rate limit metrics

3. **Configuration Validation Errors**
   - Review validation error messages
   - Check environment-specific settings
   - Verify configuration file format

4. **Performance Issues**
   - Monitor middleware execution time
   - Check database connection pool
   - Review audit logging settings

### Debugging

Enable debug logging for detailed information:

```yaml
observability:
  logging:
    level: "debug"
```

Use the built-in health check endpoint:

```bash
curl http://localhost:8000/api/healthchecker
```

## API Reference

### Middleware Functions

- `MiddlewarePipelineManager::execute_pipeline`: Main pipeline executor
- `require_permission(permission)`: Permission-based middleware
- `require_role(role)`: Role-based middleware
- `rate_limit_middleware`: Rate limiting middleware
- `security_headers_middleware`: Security headers middleware

### Configuration Structs

- `AppConfiguration`: Main configuration structure
- `SecurityConfig`: Security-related configuration
- `RateLimitConfig`: Rate limiting configuration
- `AuditConfig`: Audit logging configuration

### Validation Types

- `ConfigValidationEngine`: Main validation engine
- `ValidationResult`: Validation result structure
- `ValidationError`: Error information
- `ValidationWarning`: Warning information

## Contributing

When contributing to the middleware system:

1. **Security First**: Always consider security implications
2. **Performance**: Ensure changes don't impact performance
3. **Testing**: Add comprehensive tests
4. **Documentation**: Update documentation
5. **Backward Compatibility**: Maintain API compatibility

## License

This middleware system is part of RustCI and follows the same license terms.