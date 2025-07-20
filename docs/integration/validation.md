# Integration Validation Guide

This document provides comprehensive validation procedures for RustCI system integration and deployment verification.

## Overview

The integration validation process ensures that all RustCI components work together correctly across different deployment scenarios. This includes testing the CI/CD pipeline execution, connector functionality, and system reliability.

## Validation Checklist

### ✅ Core System Validation

#### Application Startup
- [ ] Application starts without errors
- [ ] All required environment variables are loaded
- [ ] Database connection is established
- [ ] Health check endpoint responds correctly

#### API Functionality
- [ ] All REST endpoints are accessible
- [ ] Authentication system works correctly
- [ ] File upload functionality operates properly
- [ ] Error handling returns appropriate responses

#### Database Integration
- [ ] MongoDB connection is stable
- [ ] CRUD operations work correctly
- [ ] Data persistence is maintained
- [ ] Indexes are created properly

### ✅ Pipeline System Validation

#### Pipeline Creation
- [ ] YAML pipelines can be created via API
- [ ] File upload creates pipelines correctly
- [ ] Pipeline validation catches syntax errors
- [ ] Pipeline metadata is stored properly

#### Pipeline Execution
- [ ] Manual triggers work correctly
- [ ] Webhook triggers function properly
- [ ] Environment variables are passed correctly
- [ ] Execution status is tracked accurately

#### Stage and Step Processing
- [ ] Sequential stages execute in order
- [ ] Parallel stages execute concurrently
- [ ] Step failures are handled correctly
- [ ] Logs are captured and stored

### ✅ Connector System Validation

#### Docker Connector
- [ ] Container creation and execution
- [ ] Image building from Dockerfiles
- [ ] Volume mounting works correctly
- [ ] Environment variable injection
- [ ] Container cleanup after execution

#### Kubernetes Connector
- [ ] Job creation and management
- [ ] PVC creation and mounting
- [ ] Resource limits are enforced
- [ ] Namespace isolation works
- [ ] Job cleanup after completion

#### Cloud Connectors
- [ ] AWS connector authentication
- [ ] Azure connector functionality
- [ ] GCP connector operations
- [ ] Error handling for cloud services

#### Git Connectors
- [ ] GitHub API integration
- [ ] Repository cloning works
- [ ] Webhook processing functions
- [ ] OAuth authentication flow

## Automated Validation Tests

### System Health Tests

```bash
#!/bin/bash
# health-check.sh - Automated health validation

echo "=== RustCI Health Check ==="

# Test application health
echo "Testing application health..."
HEALTH_RESPONSE=$(curl -s http://localhost:8000/api/healthchecker)
if echo "$HEALTH_RESPONSE" | grep -q "success"; then
    echo "✅ Application health check passed"
else
    echo "❌ Application health check failed"
    exit 1
fi

# Test database connectivity
echo "Testing database connectivity..."
DB_TEST=$(curl -s http://localhost:8000/api/health/database)
if echo "$DB_TEST" | grep -q "connected"; then
    echo "✅ Database connectivity verified"
else
    echo "❌ Database connectivity failed"
    exit 1
fi

# Test Docker connectivity
echo "Testing Docker connectivity..."
if docker ps > /dev/null 2>&1; then
    echo "✅ Docker connectivity verified"
else
    echo "❌ Docker connectivity failed"
    exit 1
fi

# Test Kubernetes connectivity (if available)
if command -v kubectl > /dev/null 2>&1; then
    echo "Testing Kubernetes connectivity..."
    if kubectl cluster-info > /dev/null 2>&1; then
        echo "✅ Kubernetes connectivity verified"
    else
        echo "⚠️  Kubernetes connectivity not available"
    fi
fi

echo "=== Health Check Complete ==="
```

### Pipeline Validation Tests

```bash
#!/bin/bash
# pipeline-validation.sh - Test pipeline functionality

echo "=== Pipeline Validation Tests ==="

# Create test pipeline
echo "Creating test pipeline..."
PIPELINE_RESPONSE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Validation Test\"\ndescription: \"Automated validation pipeline\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Test\"\n    steps:\n      - name: \"hello\"\n        step_type: shell\n        config:\n          command: \"echo Hello from validation test\"\nenvironment: {}\ntimeout: 300\nretry_count: 0"
  }')

PIPELINE_ID=$(echo "$PIPELINE_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

if [ -n "$PIPELINE_ID" ]; then
    echo "✅ Pipeline created successfully: $PIPELINE_ID"
else
    echo "❌ Pipeline creation failed"
    exit 1
fi

# Trigger pipeline execution
echo "Triggering pipeline execution..."
EXECUTION_RESPONSE=$(curl -s -X POST "http://localhost:8000/api/ci/pipelines/$PIPELINE_ID/trigger" \
  -H "Content-Type: application/json" \
  -d '{"trigger_type": "manual", "environment": {}}')

EXECUTION_ID=$(echo "$EXECUTION_RESPONSE" | grep -o '"execution_id":"[^"]*"' | cut -d'"' -f4)

if [ -n "$EXECUTION_ID" ]; then
    echo "✅ Pipeline triggered successfully: $EXECUTION_ID"
else
    echo "❌ Pipeline trigger failed"
    exit 1
fi

# Wait for execution completion
echo "Waiting for execution completion..."
for i in {1..30}; do
    EXECUTION_STATUS=$(curl -s "http://localhost:8000/api/ci/executions/$EXECUTION_ID")
    STATUS=$(echo "$EXECUTION_STATUS" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
    
    if [ "$STATUS" = "completed" ]; then
        echo "✅ Pipeline execution completed successfully"
        break
    elif [ "$STATUS" = "failed" ]; then
        echo "❌ Pipeline execution failed"
        exit 1
    fi
    
    sleep 2
done

echo "=== Pipeline Validation Complete ==="
```

### Connector Validation Tests

```bash
#!/bin/bash
# connector-validation.sh - Test connector functionality

echo "=== Connector Validation Tests ==="

# Test Docker connector
echo "Testing Docker connector..."
DOCKER_PIPELINE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Docker Test\"\nstages:\n  - name: \"Docker Test\"\n    steps:\n      - name: \"docker-step\"\n        step_type: docker\n        config:\n          image: \"alpine:latest\"\n          command: \"echo Docker connector test\""
  }')

DOCKER_PIPELINE_ID=$(echo "$DOCKER_PIPELINE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

if [ -n "$DOCKER_PIPELINE_ID" ]; then
    echo "✅ Docker connector pipeline created"
    
    # Trigger Docker pipeline
    DOCKER_EXECUTION=$(curl -s -X POST "http://localhost:8000/api/ci/pipelines/$DOCKER_PIPELINE_ID/trigger" \
      -H "Content-Type: application/json" \
      -d '{"trigger_type": "manual"}')
    
    echo "✅ Docker connector test triggered"
else
    echo "❌ Docker connector test failed"
fi

# Test Kubernetes connector (if available)
if kubectl cluster-info > /dev/null 2>&1; then
    echo "Testing Kubernetes connector..."
    K8S_PIPELINE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines \
      -H "Content-Type: application/json" \
      -d '{
        "yaml_content": "name: \"K8s Test\"\nstages:\n  - name: \"K8s Test\"\n    steps:\n      - name: \"k8s-step\"\n        step_type: kubernetes\n        config:\n          image: \"alpine:latest\"\n          command: \"echo Kubernetes connector test\"\n          namespace: \"default\""
      }')
    
    K8S_PIPELINE_ID=$(echo "$K8S_PIPELINE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    
    if [ -n "$K8S_PIPELINE_ID" ]; then
        echo "✅ Kubernetes connector pipeline created"
    else
        echo "❌ Kubernetes connector test failed"
    fi
else
    echo "⚠️  Kubernetes connector test skipped (cluster not available)"
fi

echo "=== Connector Validation Complete ==="
```

## Manual Validation Procedures

### 1. API Endpoint Testing

Test all major API endpoints manually:

```bash
# Health check
curl -X GET http://localhost:8000/api/healthchecker

# List pipelines
curl -X GET http://localhost:8000/api/ci/pipelines

# Create pipeline
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{"yaml_content": "name: \"Manual Test\"\nstages: []"}'

# Upload pipeline file
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@test-pipeline.yaml"

# Trigger pipeline
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{"trigger_type": "manual"}'

# Check execution status
curl -X GET http://localhost:8000/api/ci/executions/{EXECUTION_ID}
```

### 2. File Upload Validation

Create a test pipeline file and validate upload:

```yaml
# test-pipeline.yaml
name: "File Upload Test"
description: "Test file upload functionality"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Test Stage"
    steps:
      - name: "test-step"
        step_type: shell
        config:
          command: "echo 'File upload test successful'"

environment: {}
timeout: 300
retry_count: 0
```

Upload and verify:

```bash
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@test-pipeline.yaml"
```

### 3. Authentication Flow Testing

Test GitHub OAuth integration:

1. Navigate to `http://localhost:8000/api/sessions/oauth/github`
2. Complete GitHub OAuth flow
3. Verify JWT token is returned
4. Test authenticated endpoints with token

### 4. Webhook Integration Testing

Test webhook functionality:

```bash
# Create webhook-triggered pipeline
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Webhook Test\"\ntriggers:\n  - trigger_type: webhook\n    config: {}\nstages:\n  - name: \"Webhook Stage\"\n    steps:\n      - name: \"webhook-step\"\n        step_type: shell\n        config:\n          command: \"echo Webhook triggered successfully\""
  }'

# Trigger via webhook
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "refs/heads/main",
    "after": "abc123",
    "repository": {"full_name": "test/repo"}
  }'
```

## Performance Validation

### Load Testing

Test system performance under load:

```bash
#!/bin/bash
# load-test.sh - Basic load testing

echo "=== Load Testing ==="

# Create multiple pipelines concurrently
for i in {1..10}; do
    curl -X POST http://localhost:8000/api/ci/pipelines \
      -H "Content-Type: application/json" \
      -d "{\"yaml_content\": \"name: \\\"Load Test $i\\\"\\nstages: []\"}" &
done

wait

echo "✅ Concurrent pipeline creation test completed"

# Trigger multiple executions
PIPELINE_ID="your-test-pipeline-id"
for i in {1..5}; do
    curl -X POST "http://localhost:8000/api/ci/pipelines/$PIPELINE_ID/trigger" \
      -H "Content-Type: application/json" \
      -d '{"trigger_type": "manual"}' &
done

wait

echo "✅ Concurrent execution test completed"
echo "=== Load Testing Complete ==="
```

### Resource Monitoring

Monitor system resources during validation:

```bash
#!/bin/bash
# monitor-resources.sh - Resource monitoring during tests

echo "=== Resource Monitoring ==="

# Monitor CPU and memory usage
echo "CPU and Memory Usage:"
top -bn1 | grep "Cpu\|Mem"

# Monitor disk usage
echo "Disk Usage:"
df -h

# Monitor Docker resources
if command -v docker > /dev/null 2>&1; then
    echo "Docker Resource Usage:"
    docker stats --no-stream
fi

# Monitor Kubernetes resources
if command -v kubectl > /dev/null 2>&1; then
    echo "Kubernetes Resource Usage:"
    kubectl top nodes 2>/dev/null || echo "Kubernetes metrics not available"
    kubectl top pods --all-namespaces 2>/dev/null || echo "Pod metrics not available"
fi

echo "=== Resource Monitoring Complete ==="
```

## Security Validation

### Authentication Testing

```bash
#!/bin/bash
# security-validation.sh - Security testing

echo "=== Security Validation ==="

# Test unauthenticated access to protected endpoints
echo "Testing unauthenticated access..."
UNAUTH_RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8000/api/ci/pipelines)

if [ "$UNAUTH_RESPONSE" = "401" ] || [ "$UNAUTH_RESPONSE" = "403" ]; then
    echo "✅ Protected endpoints properly secured"
else
    echo "❌ Security issue: Protected endpoints accessible without authentication"
fi

# Test invalid token
echo "Testing invalid token..."
INVALID_TOKEN_RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" \
  -H "Authorization: Bearer invalid-token" \
  http://localhost:8000/api/ci/pipelines)

if [ "$INVALID_TOKEN_RESPONSE" = "401" ] || [ "$INVALID_TOKEN_RESPONSE" = "403" ]; then
    echo "✅ Invalid token properly rejected"
else
    echo "❌ Security issue: Invalid token accepted"
fi

echo "=== Security Validation Complete ==="
```

### Input Validation Testing

```bash
# Test malformed YAML
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{"yaml_content": "invalid: yaml: content: ["}'

# Test oversized file upload
dd if=/dev/zero of=large-file.yaml bs=1M count=20
curl -X POST http://localhost:8000/api/ci/pipelines/upload \
  -F "pipeline=@large-file.yaml"
rm large-file.yaml

# Test SQL injection attempts
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{"yaml_content": "name: \"test\"; DROP TABLE pipelines; --\""}'
```

## Integration Test Results

### Expected Test Outcomes

#### ✅ Successful Validation Results

- All health checks return 200 OK
- Pipeline creation returns valid UUID
- Pipeline execution completes successfully
- Connector tests execute without errors
- Authentication flow works correctly
- Security tests show proper protection

#### ❌ Common Failure Scenarios

1. **Database Connection Issues**
   - MongoDB not running
   - Incorrect connection string
   - Authentication failures

2. **Docker Integration Problems**
   - Docker daemon not running
   - Permission issues
   - Image pull failures

3. **Kubernetes Connectivity Issues**
   - Cluster not accessible
   - RBAC permission problems
   - Resource quota exceeded

4. **Authentication Failures**
   - Invalid OAuth configuration
   - JWT secret not set
   - Token expiration issues

## Troubleshooting Guide

### Common Issues and Solutions

#### Application Won't Start

```bash
# Check environment variables
env | grep -E "(MONGODB|JWT|GITHUB)"

# Check port availability
lsof -i :8000

# Check logs
tail -f /var/log/rustci/app.log
```

#### Database Connection Failed

```bash
# Test MongoDB connectivity
mongo --eval "db.adminCommand('ismaster')"

# Check MongoDB status
systemctl status mongod

# Verify connection string
echo $MONGODB_URI
```

#### Docker Issues

```bash
# Test Docker connectivity
docker ps

# Check Docker daemon
systemctl status docker

# Verify user permissions
groups $USER | grep docker
```

#### Kubernetes Problems

```bash
# Test cluster connectivity
kubectl cluster-info

# Check current context
kubectl config current-context

# Verify permissions
kubectl auth can-i create jobs
```

## Continuous Validation

### Automated Testing Pipeline

Create a validation pipeline that runs regularly:

```yaml
name: "System Validation Pipeline"
description: "Automated system validation and health checks"

triggers:
  - trigger_type: schedule
    config:
      cron_expression: "0 */6 * * *"  # Every 6 hours

stages:
  - name: "Health Checks"
    steps:
      - name: "system-health"
        step_type: shell
        config:
          script: |
            ./scripts/health-check.sh
            ./scripts/pipeline-validation.sh
            ./scripts/connector-validation.sh

  - name: "Performance Tests"
    steps:
      - name: "load-testing"
        step_type: shell
        config:
          script: |
            ./scripts/load-test.sh
            ./scripts/monitor-resources.sh

  - name: "Security Validation"
    steps:
      - name: "security-tests"
        step_type: shell
        config:
          script: |
            ./scripts/security-validation.sh

environment:
  VALIDATION_MODE: "automated"
  ALERT_ON_FAILURE: "true"

timeout: 1800
retry_count: 1
```

### Monitoring and Alerting

Set up monitoring for validation results:

```bash
#!/bin/bash
# setup-monitoring.sh - Configure validation monitoring

# Create validation log directory
mkdir -p /var/log/rustci/validation

# Set up log rotation
cat > /etc/logrotate.d/rustci-validation << EOF
/var/log/rustci/validation/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 rustci rustci
}
EOF

# Create validation cron job
cat > /etc/cron.d/rustci-validation << EOF
# RustCI Validation - Run every 6 hours
0 */6 * * * rustci /opt/rustci/scripts/validation-suite.sh >> /var/log/rustci/validation/validation.log 2>&1
EOF

echo "✅ Validation monitoring configured"
```

This comprehensive validation guide ensures that your RustCI deployment is working correctly across all components and use cases. Regular validation helps maintain system reliability and catch issues before they impact production workloads.