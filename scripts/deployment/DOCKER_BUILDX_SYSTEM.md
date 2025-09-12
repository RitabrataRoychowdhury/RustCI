# Docker Buildx Cross-Compilation System

This directory contains scripts for Docker buildx cross-compilation, specifically designed for building AMD64 images on ARM Mac systems for deployment to AMD64 VPS servers.

## Overview

The Docker buildx cross-compilation system enables developers to build Docker images for different architectures than their host system. This is particularly useful for:

- Building AMD64 images on ARM Mac (Apple Silicon)
- Cross-platform deployment scenarios
- Multi-architecture container support

## Components

### 1. Setup Script: `setup-buildx-multiarch.sh`

Sets up Docker buildx with multi-platform support.

**Features:**
- Creates and configures multiarch-builder
- Supports linux/amd64 and linux/arm64 platforms
- Validates builder setup and platform support
- Provides usage examples

**Usage:**
```bash
./scripts/deployment/setup-buildx-multiarch.sh
```

**Requirements:**
- Docker daemon running
- Docker buildx plugin available

### 2. Build Script: `build-amd64-image.sh`

Builds AMD64 Docker images using buildx.

**Features:**
- Cross-compilation from ARM to AMD64
- Configurable image names and tags
- Support for local loading or registry push
- Build caching options
- Architecture verification

**Usage:**
```bash
# Basic AMD64 build
./scripts/deployment/build-amd64-image.sh

# Custom image name and tag
./scripts/deployment/build-amd64-image.sh -i myapp -t v1.0.0

# Build without cache
./scripts/deployment/build-amd64-image.sh --no-cache

# Build and push to registry
./scripts/deployment/build-amd64-image.sh --push --registry docker.io/myuser
```

**Options:**
- `-i, --image NAME`: Image name (default: rustci)
- `-t, --tag TAG`: Image tag (default: local-amd64)
- `-f, --dockerfile FILE`: Dockerfile path
- `-c, --context PATH`: Build context path
- `--no-cache`: Build without cache
- `--push`: Push to registry instead of loading locally
- `--registry URL`: Registry URL for push

### 3. Validation Script: `validate-image-architecture.sh`

Validates Docker image architecture for compatibility.

**Features:**
- Architecture and OS verification
- Platform compatibility checks
- Detailed image information
- JSON output support
- Automated validation for CI/CD

**Usage:**
```bash
# Validate default image
./scripts/deployment/validate-image-architecture.sh

# Validate specific image
./scripts/deployment/validate-image-architecture.sh -i myapp:v1.0.0

# JSON output for automation
./scripts/deployment/validate-image-architecture.sh --json

# Detailed information
./scripts/deployment/validate-image-architecture.sh --detailed
```

**Validation Checks:**
- ✓ Image exists locally or in registry
- ✓ Architecture matches expected value (amd64)
- ✓ Operating system matches expected value (linux)
- ✓ Platform compatibility verification

### 4. Compatibility Testing Script: `test-amd64-compatibility.sh`

Tests AMD64 containers for cross-architecture compatibility.

**Features:**
- Container startup verification
- Runtime architecture checks
- Process execution testing
- Network connectivity validation
- Health endpoint verification
- Resource usage monitoring

**Usage:**
```bash
# Basic compatibility test
./scripts/deployment/test-amd64-compatibility.sh

# Test specific image
./scripts/deployment/test-amd64-compatibility.sh -i myapp:amd64

# Quick test (skip health checks)
./scripts/deployment/test-amd64-compatibility.sh --quick

# Verbose output with logs
./scripts/deployment/test-amd64-compatibility.sh --verbose
```

**Test Categories:**
1. Image architecture verification
2. Container startup test
3. Runtime architecture check (uname -m = x86_64)
4. Process execution test
5. Network connectivity test
6. Health endpoint verification
7. Resource usage check

### 5. Integration Test Script: `test-buildx-system.sh`

Comprehensive integration test for the entire buildx system.

**Features:**
- End-to-end system testing
- All component verification
- Test environment creation
- Automated cleanup
- Pass/fail reporting

**Usage:**
```bash
./scripts/deployment/test-buildx-system.sh
```

## Quick Start Guide

### 1. Initial Setup

```bash
# Set up Docker buildx multiarch builder
./scripts/deployment/setup-buildx-multiarch.sh
```

### 2. Build AMD64 Image

```bash
# Build RustCI for AMD64
./scripts/deployment/build-amd64-image.sh -i rustci -t production-amd64
```

### 3. Validate Architecture

```bash
# Verify the built image
./scripts/deployment/validate-image-architecture.sh -i rustci:production-amd64
```

### 4. Test Compatibility

```bash
# Test AMD64 compatibility
./scripts/deployment/test-amd64-compatibility.sh -i rustci:production-amd64
```

### 5. Run Integration Tests

```bash
# Test entire system
./scripts/deployment/test-buildx-system.sh
```

## Architecture Verification

The system performs multiple levels of architecture verification:

### Image Level
```bash
docker image inspect --format='{{.Architecture}}/{{.Os}}' rustci:local-amd64
# Expected: amd64/linux
```

### Runtime Level
```bash
docker run --rm rustci:local-amd64 uname -m
# Expected: x86_64
```

### Platform Compatibility
```bash
docker buildx inspect | grep "Platforms:"
# Should include: linux/amd64
```

## Common Use Cases

### 1. ARM Mac to AMD64 VPS Deployment

```bash
# 1. Setup buildx
./scripts/deployment/setup-buildx-multiarch.sh

# 2. Build AMD64 image
./scripts/deployment/build-amd64-image.sh -i rustci -t vps-deploy

# 3. Validate architecture
./scripts/deployment/validate-image-architecture.sh -i rustci:vps-deploy

# 4. Test compatibility
./scripts/deployment/test-amd64-compatibility.sh -i rustci:vps-deploy

# 5. Save for transfer
docker save rustci:vps-deploy | gzip > rustci-vps-deploy.tar.gz
```

### 2. Multi-Platform Registry Push

```bash
# Build and push multi-platform image
./scripts/deployment/build-amd64-image.sh \
  --push \
  --registry docker.io/myuser \
  -i rustci \
  -t multiarch
```

### 3. CI/CD Integration

```bash
# Automated validation in CI
./scripts/deployment/validate-image-architecture.sh \
  -i $IMAGE_NAME \
  --json > architecture-report.json
```

## Troubleshooting

### Common Issues

1. **Builder not found**
   ```bash
   # Solution: Run setup script
   ./scripts/deployment/setup-buildx-multiarch.sh
   ```

2. **Platform not supported**
   ```bash
   # Check supported platforms
   docker buildx inspect | grep "Platforms:"
   
   # Recreate builder if needed
   docker buildx rm multiarch-builder
   ./scripts/deployment/setup-buildx-multiarch.sh
   ```

3. **Image architecture mismatch**
   ```bash
   # Verify build command used correct platform
   docker image inspect --format='{{.Architecture}}' IMAGE_NAME
   
   # Rebuild with explicit platform
   ./scripts/deployment/build-amd64-image.sh -i IMAGE_NAME
   ```

4. **Container won't start on target platform**
   ```bash
   # Test compatibility locally first
   ./scripts/deployment/test-amd64-compatibility.sh -i IMAGE_NAME
   
   # Check for architecture-specific dependencies
   docker run --rm IMAGE_NAME ldd /path/to/binary
   ```

### Debug Mode

Enable verbose output for debugging:

```bash
# Verbose build
./scripts/deployment/build-amd64-image.sh --verbose

# Verbose compatibility test
./scripts/deployment/test-amd64-compatibility.sh --verbose

# Detailed validation
./scripts/deployment/validate-image-architecture.sh --detailed
```

## Requirements Mapping

This implementation satisfies the following requirements from the cross-architecture deployment spec:

- **Requirement 1.1**: AMD64 image creation using `docker buildx build --platform linux/amd64`
- **Requirement 1.2**: Architecture verification using `docker image inspect --format='{{.Architecture}}/{{.Os}}'`
- **Requirement 1.4**: Runtime architecture verification using `uname -m` returning "x86_64"
- **Requirement 2.4**: Local container testing for compatibility verification

## Integration with Pipeline

These scripts integrate with the RustCI pipeline through:

1. **prepare** stage: Setup buildx builder
2. **build-image** stage: Build AMD64 image
3. **transfer-and-deploy** stage: Validate before transfer
4. **smoke-test** stage: Compatibility verification

## Security Considerations

- Scripts validate Docker daemon availability
- Architecture verification prevents deployment of incompatible images
- Cleanup functions prevent resource leaks
- Error handling prevents partial deployments

## Performance Optimization

- Build caching support for faster rebuilds
- Compressed image transfer preparation
- Parallel platform builds when supported
- Resource usage monitoring during tests

## Maintenance

Regular maintenance tasks:

```bash
# Clean up old builders
docker buildx ls
docker buildx rm OLD_BUILDER_NAME

# Update builder platforms
docker buildx create --name multiarch-builder --driver docker-container --use

# Verify system health
./scripts/deployment/test-buildx-system.sh
```