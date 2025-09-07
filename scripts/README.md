# RustCI Scripts Directory

This directory contains organized scripts for managing and testing RustCI.

## Directory Structure

```
scripts/
├── auth/                   # Authentication and token management
│   ├── get-admin-token.sh     # Get JWT token for API access
│   └── generate-token.sh      # Generate authentication tokens
├── runners/                # Runner environment management
│   ├── setup-dind-environment.sh    # Docker-in-Docker setup
│   ├── setup-fake-ec2.sh          # Fake EC2 instances for testing
│   ├── manage-runners.sh           # Check and manage all runners
│   └── k8s-test-server.sh          # Kubernetes test environment
├── deployment/             # Deployment testing scripts
│   ├── quick-deploy-test.sh        # Quick Node.js deployment test
│   ├── test-nodejs-deployment.sh   # Full Node.js deployment test
│   ├── deploy.sh                   # General deployment script
│   └── unified-deployment-test.sh  # Comprehensive deployment tests
├── testing/                # Testing and validation scripts
│   ├── test-*.sh                   # Various test scripts
│   └── validate-*.sh               # Validation scripts
├── performance/            # Performance testing and benchmarking
│   ├── performance-*.sh            # Performance test scripts
│   ├── benchmark-*.sh              # Benchmarking scripts
│   └── valkyrie-*.sh              # Valkyrie performance tests
└── maintenance/            # Maintenance and utility scripts
    ├── cleanup-*.sh                # Cleanup scripts
    ├── migrate-*.sh                # Migration scripts
    └── fix-*.sh                    # Fix and repair scripts
```

## Quick Start

### 1. Setup Authentication
```bash
./auth/get-admin-token.sh
```

### 2. Setup Runners
```bash
./runners/manage-runners.sh setup
```

### 3. Test Deployment
```bash
./deployment/quick-deploy-test.sh
```

## Common Workflows

### Full Environment Setup
```bash
# 1. Get authentication token
./auth/get-admin-token.sh

# 2. Setup all available runners
./runners/manage-runners.sh setup

# 3. Check runner status
./runners/manage-runners.sh status

# 4. Test deployment
./deployment/quick-deploy-test.sh
```

### Troubleshooting
```bash
# Check runner status
./runners/manage-runners.sh status

# Test runner connectivity
./runners/manage-runners.sh test

# Run comprehensive tests
./testing/test-all-functionalities.sh
```

### Performance Testing
```bash
# Run performance benchmarks
./performance/performance-comparison.sh

# Test Valkyrie performance
./performance/valkyrie-test.sh
```

## Script Categories

### Authentication Scripts (`auth/`)
- **get-admin-token.sh**: Interactive script to get JWT tokens via OAuth
- **generate-token.sh**: Generate tokens programmatically

### Runner Scripts (`runners/`)
- **manage-runners.sh**: Central script to manage all runner types
- **setup-dind-environment.sh**: Setup Docker-in-Docker for isolated execution
- **setup-fake-ec2.sh**: Create fake EC2-like instances for testing
- **k8s-test-server.sh**: Setup Kubernetes test environment

### Deployment Scripts (`deployment/`)
- **quick-deploy-test.sh**: Fast deployment test for development
- **test-nodejs-deployment.sh**: Comprehensive Node.js deployment test
- **deploy.sh**: General deployment script
- **unified-deployment-test.sh**: Test multiple deployment scenarios

### Testing Scripts (`testing/`)
- Various test scripts for different components and scenarios
- Validation scripts for configuration and setup

### Performance Scripts (`performance/`)
- Performance benchmarking and load testing
- Valkyrie protocol performance tests
- System performance analysis

### Maintenance Scripts (`maintenance/`)
- Cleanup and maintenance utilities
- Migration scripts for configuration changes
- Fix scripts for common issues

## Usage Examples

### Check System Status
```bash
# Check if RustCI is running
curl http://localhost:8000/healthchecker

# Check all runners
./runners/manage-runners.sh status

# Check authentication
./auth/get-admin-token.sh
```

### Run Tests
```bash
# Quick deployment test
./deployment/quick-deploy-test.sh

# Full test suite
./testing/test-all-functionalities.sh

# Performance tests
./performance/performance-comparison.sh
```

### Setup Development Environment
```bash
# Complete setup
./runners/manage-runners.sh setup
./auth/get-admin-token.sh
./deployment/quick-deploy-test.sh
```

## Prerequisites

- **Docker**: Required for most runner types and deployment tests
- **curl**: Required for API interactions
- **jq**: Required for JSON processing (install with `brew install jq` or `apt install jq`)
- **RustCI**: Must be running on localhost:8000

## Environment Variables

Some scripts use these environment variables:
- `RUSTCI_URL`: RustCI server URL (default: http://localhost:8000)
- `JWT_TOKEN`: Authentication token (usually read from .rustci_token file)
- `DOCKER_HOST`: Docker daemon host (for remote Docker)

## Troubleshooting

### Common Issues

1. **"RustCI not running"**: Start RustCI with `cargo run`
2. **"No authentication token"**: Run `./auth/get-admin-token.sh`
3. **"Docker not available"**: Start Docker daemon
4. **"Permission denied"**: Make scripts executable with `chmod +x script.sh`

### Getting Help

Each script supports `--help` or `-h` flag:
```bash
./runners/manage-runners.sh --help
./deployment/quick-deploy-test.sh --help
```