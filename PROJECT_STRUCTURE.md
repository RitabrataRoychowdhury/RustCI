# RustCI Project Structure

This document outlines the organization and structure of the RustCI codebase.

## 📁 Root Directory

```
RustCI/
├── 📄 README.md                    # Project overview and documentation
├── 📄 CHANGELOG.md                 # Version history and changes
├── 📄 CONTRIBUTING.md              # Contribution guidelines
├── 📄 LICENSE                      # MIT license
├── 📄 Cargo.toml                   # Rust project configuration
├── 📄 Cargo.lock                   # Dependency lock file
├── 📄 Dockerfile                   # Container build configuration
├── 📄 docker-compose.yaml          # Single-node deployment
├── 📄 docker-compose.multi-node.yaml # Multi-node cluster deployment
├── 📄 pipeline.yaml                # CI/CD pipeline configuration
├── 📄 .env                         # Environment variables (local)
├── 📄 .gitignore                   # Git ignore patterns
├── 📄 .dockerignore                # Docker ignore patterns
└── 📁 [directories...]             # See detailed structure below
```

## 🏗️ Core Directories

### `/src` - Source Code
```
src/
├── 📁 application/          # Application layer (handlers, services)
├── 📁 core/                # Core business logic and domain
├── 📁 infrastructure/      # External integrations and persistence
├── 📁 error/               # Error handling and types
├── 📁 bin/                 # Binary executables
└── 📄 lib.rs               # Library root
```

### `/tests` - Test Suite
```
tests/
├── 📁 unit/                # Unit tests
├── 📁 integration/         # Integration tests
├── 📁 performance/         # Performance and load tests
└── 📁 e2e/                 # End-to-end tests
```

### `/docs` - Documentation
```
docs/
├── 📁 api/                 # API documentation
├── 📁 architecture/        # Architecture diagrams and docs
├── 📁 deployment/          # Deployment guides
├── 📁 development/         # Development guides
└── 📁 user/                # User documentation
```

### `/config` - Configuration
```
config/
├── 📄 config.example.yaml      # Basic configuration template
├── 📄 config.enhanced.example.yaml # Advanced configuration
├── 📄 prometheus.yml           # Prometheus configuration
└── 📄 haproxy.cfg             # Load balancer configuration
```

### `/deployment` - Deployment Manifests
```
deployment/
├── 📁 kubernetes/          # Kubernetes manifests
├── 📁 k3s/                 # K3s deployment files
├── 📁 docker/              # Docker-specific configs
└── 📁 terraform/           # Infrastructure as code
```

### `/helm` - Helm Charts
```
helm/
└── 📁 rustci/              # Helm chart for Kubernetes deployment
    ├── 📄 Chart.yaml
    ├── 📄 values.yaml
    └── 📁 templates/
```

### `/scripts` - Automation Scripts
```
scripts/
├── 📄 setup.sh             # Environment setup
├── 📄 build.sh             # Build automation
├── 📄 test.sh              # Test automation
├── 📄 deploy.sh            # Deployment automation
└── 📁 ci/                  # CI/CD specific scripts
```

### `/examples` - Usage Examples
```
examples/
├── 📄 basic_pipeline.rs    # Basic pipeline example
├── 📄 advanced_config.rs   # Advanced configuration
├── 📄 custom_runner.rs     # Custom runner implementation
└── 📁 integrations/        # Integration examples
```

### `/monitoring` - Observability
```
monitoring/
├── 📄 prometheus.yml       # Prometheus configuration
├── 📄 grafana-dashboard.json # Grafana dashboards
└── 📁 alerts/              # Alert rules
```

### `/benchmarks` - Performance Benchmarks
```
benchmarks/
├── 📄 job_execution.rs     # Job execution benchmarks
├── 📄 cluster_coordination.rs # Cluster performance
└── 📄 api_throughput.rs    # API performance benchmarks
```

## 🧩 Module Organization

### Application Layer (`src/application/`)
```
application/
├── 📁 handlers/            # HTTP request handlers
│   ├── 📄 ci.rs           # CI/CD endpoints
│   ├── 📄 auth.rs         # Authentication endpoints
│   ├── 📄 cluster.rs      # Cluster management endpoints
│   └── 📄 health.rs       # Health check endpoints
├── 📁 services/           # Application services
│   ├── 📄 pipeline_service.rs # Pipeline orchestration
│   ├── 📄 job_service.rs     # Job management
│   └── 📄 cluster_service.rs # Cluster coordination
└── 📁 middleware/         # HTTP middleware
    ├── 📄 auth.rs         # Authentication middleware
    ├── 📄 logging.rs      # Request logging
    └── 📄 cors.rs         # CORS handling
```

### Core Layer (`src/core/`)
```
core/
├── 📁 domain/             # Domain entities and logic
│   ├── 📄 job.rs         # Job entity and logic
│   ├── 📄 pipeline.rs    # Pipeline entity
│   ├── 📄 runner.rs      # Runner entity
│   └── 📄 cluster.rs     # Cluster entity
├── 📁 performance/       # Performance optimizations
│   ├── 📄 auto_scaler.rs # Auto-scaling logic
│   ├── 📄 load_balancer.rs # Load balancing
│   ├── 📄 cache_manager.rs # Caching strategies
│   └── 📄 memory_efficient.rs # Memory optimizations
├── 📁 security/          # Security implementations
│   ├── 📄 auth.rs        # Authentication logic
│   ├── 📄 encryption.rs  # Encryption services
│   ├── 📄 audit_logger.rs # Audit logging
│   └── 📄 input_sanitizer.rs # Input validation
└── 📁 config/            # Configuration management
    ├── 📄 manager.rs     # Configuration manager
    ├── 📄 validation.rs  # Configuration validation
    └── 📄 hot_reload.rs  # Hot reload functionality
```

### Infrastructure Layer (`src/infrastructure/`)
```
infrastructure/
├── 📁 database/          # Database implementations
│   ├── 📄 mongodb.rs    # MongoDB integration
│   ├── 📄 migrations.rs # Database migrations
│   └── 📄 connection_pool.rs # Connection pooling
├── 📁 runners/           # Job runner implementations
│   ├── 📄 local.rs      # Local runner
│   ├── 📄 docker.rs     # Docker runner
│   └── 📄 kubernetes.rs # Kubernetes runner
├── 📁 external/          # External service integrations
│   ├── 📄 github.rs     # GitHub API integration
│   ├── 📄 prometheus.rs # Prometheus metrics
│   └── 📄 oauth.rs      # OAuth providers
└── 📁 messaging/         # Message queue implementations
    ├── 📄 redis.rs      # Redis pub/sub
    └── 📄 rabbitmq.rs   # RabbitMQ integration
```

## 🔧 Configuration Files

### Essential Configuration
- **Cargo.toml**: Rust project metadata and dependencies
- **Dockerfile**: Container build instructions
- **docker-compose.yaml**: Local development environment
- **docker-compose.multi-node.yaml**: Production cluster setup

### Environment Configuration
- **.env**: Local environment variables
- **config/**: Environment-specific configurations
- **deployment/**: Deployment-specific configurations

### CI/CD Configuration
- **pipeline.yaml**: Main CI/CD pipeline
- **.github/workflows/**: GitHub Actions workflows
- **scripts/ci/**: CI/CD automation scripts

## 📊 Quality Assurance

### Testing Structure
```
tests/
├── 📁 unit/
│   ├── 📁 core/          # Core logic tests
│   ├── 📁 application/   # Application layer tests
│   └── 📁 infrastructure/ # Infrastructure tests
├── 📁 integration/       # Cross-component tests
├── 📁 performance/       # Load and stress tests
└── 📁 e2e/              # End-to-end scenarios
```

### Code Quality Tools
- **cargo fmt**: Code formatting
- **cargo clippy**: Linting and best practices
- **cargo audit**: Security vulnerability scanning
- **cargo tarpaulin**: Code coverage analysis

## 🚀 Deployment Artifacts

### Container Images
- **rustci:latest**: Main application image
- **rustci:debug**: Debug build with additional tooling
- **rustci:minimal**: Minimal production image

### Kubernetes Resources
- **Deployments**: Application deployments
- **Services**: Network services
- **ConfigMaps**: Configuration data
- **Secrets**: Sensitive configuration
- **Ingress**: External access configuration

### Helm Charts
- **rustci**: Main application chart
- **rustci-monitoring**: Monitoring stack
- **rustci-storage**: Persistent storage

## 📈 Monitoring and Observability

### Metrics
- **Application Metrics**: Business logic metrics
- **System Metrics**: Resource utilization
- **Custom Metrics**: Domain-specific measurements

### Logging
- **Structured Logging**: JSON-formatted logs
- **Correlation IDs**: Request tracing
- **Log Levels**: Configurable verbosity

### Health Checks
- **Liveness Probes**: Application health
- **Readiness Probes**: Service readiness
- **Startup Probes**: Initialization status

## 🔐 Security Considerations

### Secrets Management
- Environment variables for sensitive data
- Kubernetes secrets for production
- Encrypted configuration files

### Access Control
- Role-based access control (RBAC)
- OAuth integration for authentication
- API key management for service-to-service communication

### Network Security
- TLS encryption for all communications
- Network policies for Kubernetes
- Firewall rules for traditional deployments

---

This structure provides a solid foundation for a production-grade Rust application with clear separation of concerns, comprehensive testing, and robust deployment capabilities.