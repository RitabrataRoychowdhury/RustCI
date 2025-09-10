# RustCI Runners Documentation

This section contains comprehensive documentation for RustCI runners, including implementation details, configuration, testing, and deployment strategies.

## Contents

### Runner Types
- [Local Runner](local-runner.md) - Direct execution on the host machine
- [Docker Runner](docker-runner.md) - Containerized execution using Docker
- [Kubernetes Runner](kubernetes-runner.md) - Kubernetes-based execution
- [Native Runner](native-runner.md) - High-performance native execution
- [Valkyrie Runner](valkyrie-runner.md) - Valkyrie protocol-based distributed execution

### Testing and Development
- [Docker-in-Docker (DIND) Setup](dind-setup.md) - Isolated testing environment
- [Runner Testing Guide](testing-guide.md) - Comprehensive testing strategies
- [Performance Testing](performance-testing.md) - Runner performance validation

### Configuration and Deployment
- [Runner Configuration](configuration.md) - Configuration options and examples
- [Deployment Strategies](deployment-strategies.md) - Production deployment patterns
- [Security Considerations](security.md) - Security best practices

### API and Integration
- [Runner API Reference](api-reference.md) - Complete API documentation
- [Integration Examples](integration-examples.md) - Real-world integration patterns

## Quick Start

### Prerequisites
- Rust 1.70+ (Edition 2021)
- Docker and Docker Compose
- Access to a Kubernetes cluster (for K8s runner)

### Basic Setup
1. **Start the RustCI server**
   ```bash
   cargo run
   ```

2. **Set up Docker-in-Docker for testing**
   ```bash
   ./scripts/runners/setup-dind-environment.sh
   ```

3. **Register a runner**
   ```bash
   ./scripts/runners/manage-runners.sh setup
   ```

4. **Test runner connectivity**
   ```bash
   ./scripts/runners/manage-runners.sh test
   ```

## Runner Architecture

RustCI supports multiple runner types to accommodate different execution environments and requirements:

```
┌─────────────────────────────────────────────────────────────┐
│                    RustCI Control Plane                    │
├─────────────────────────────────────────────────────────────┤
│                  Runner Selection Engine                   │
├─────────────────────────────────────────────────────────────┤
│  Local    │  Docker   │  Kubernetes │  Native  │ Valkyrie  │
│  Runner   │  Runner   │   Runner    │  Runner  │  Runner   │
├───────────┼───────────┼─────────────┼──────────┼───────────┤
│   Host    │Container  │    Pod      │  Process │Distributed│
│Execution  │Isolation  │ Orchestration│ Execution│ Execution │
└───────────┴───────────┴─────────────┴──────────┴───────────┘
```

### Runner Selection
The system automatically selects the most appropriate runner based on:
- Job requirements and constraints
- Runner availability and health
- Performance characteristics
- Resource utilization

### Runner Lifecycle
1. **Registration** - Runner registers with the control plane
2. **Health Monitoring** - Continuous health checks and metrics collection
3. **Job Assignment** - Intelligent job routing and load balancing
4. **Execution** - Job execution with monitoring and logging
5. **Cleanup** - Resource cleanup and state management

## Current Implementation Status

### ✅ Implemented
- **Local Runner**: Basic local execution with process isolation
- **Docker Runner**: Container-based execution with Docker API
- **Kubernetes Runner**: Pod-based execution with K8s API
- **Native Runner**: High-performance native process execution
- **Valkyrie Runner**: Distributed execution with Valkyrie protocol

### 🚧 In Progress
- **Enhanced Security**: Multi-factor authentication and input sanitization
- **Performance Optimization**: Connection pooling and resource management
- **Advanced Monitoring**: Comprehensive metrics and health checks

### 📋 Planned
- **Auto-scaling**: Dynamic runner scaling based on load
- **Multi-cloud Support**: AWS, GCP, Azure runner integration
- **GPU Support**: CUDA and OpenCL acceleration
- **WebAssembly Runner**: WASM-based execution environment

## Testing Infrastructure

### Docker-in-Docker (DIND)
For isolated testing, RustCI provides a comprehensive DIND setup:

```bash
# Start DIND environment
./scripts/runners/setup-dind-environment.sh start

# Test DIND functionality
./scripts/runners/setup-dind-environment.sh test

# Check status
./scripts/runners/setup-dind-environment.sh status
```

### Cross-Platform Testing
The testing infrastructure supports:
- **Mac to Windows**: DIND runner deploying to Windows targets
- **Linux Containers**: Multi-architecture container testing
- **Kubernetes**: Local K3s and remote cluster testing

## Performance Characteristics

| Runner Type | Startup Time | Isolation | Resource Usage | Scalability |
|-------------|--------------|-----------|----------------|-------------|
| Local       | ~10ms        | Process   | Low            | Limited     |
| Docker      | ~500ms       | Container | Medium         | High        |
| Kubernetes  | ~2s          | Pod       | Medium         | Very High   |
| Native      | ~5ms         | Process   | Very Low       | Medium      |
| Valkyrie    | ~50ms        | Distributed| Low           | Extreme     |

## Security Model

### Authentication
- JWT-based authentication for runner registration
- Mutual TLS for secure communication
- API key management for long-lived connections

### Authorization
- Role-based access control (RBAC)
- Resource-based permissions
- Audit logging for all operations

### Isolation
- Process-level isolation for local/native runners
- Container-level isolation for Docker runners
- Namespace isolation for Kubernetes runners
- Network segmentation for distributed runners

## Monitoring and Observability

### Metrics
- Job execution times and success rates
- Resource utilization (CPU, memory, disk)
- Network performance and latency
- Error rates and failure patterns

### Logging
- Structured logging with correlation IDs
- Centralized log aggregation
- Real-time log streaming
- Log retention and archival

### Health Checks
- Runner availability monitoring
- Resource health validation
- Performance degradation detection
- Automatic failover and recovery

## Getting Help

- [Troubleshooting Guide](../troubleshooting.md) - Common issues and solutions
- [API Documentation](../../api/README.md) - Complete API reference
- [GitHub Issues](https://github.com/rustci/rustci/issues) - Bug reports and feature requests
- [Community Discussions](https://github.com/rustci/rustci/discussions) - Community support

## Contributing

See [Contributing Guidelines](../../../CONTRIBUTING.md) for information on how to contribute to runner development.