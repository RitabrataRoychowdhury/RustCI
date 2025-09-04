# RustCI Documentation

Welcome to the RustCI documentation! This directory contains comprehensive documentation organized by audience and use case.

## Documentation Structure

### 👥 User Documentation (`user/`)
End-user guides and tutorials:
- [Installation Guide](user/installation.md) - How to install and set up RustCI
- [Quick Start Guide](user/quick-start.md) - Get started with your first pipeline
- [Configuration Guide](user/configuration.md) - Configuration options and examples
- [Pipeline Examples](user/pipeline-examples/) - Working pipeline examples
- [Troubleshooting Guide](user/troubleshooting.md) - Common issues and solutions

### 🔌 API Documentation (`api/`)
API reference and integration guides:
- [API Overview](api/README.md) - API documentation overview
- [Current API State](api/current-api-state.md) - Current API endpoints and status
- [Usage Guide](api/usage-guide.md) - API usage examples
- [Configuration Schema](api/configuration-schema.md) - Complete configuration reference
- [OpenAPI Specification](api/openapi-baseline.json) - Machine-readable API spec

### 🏗️ Architecture Documentation (`architecture/`)
System architecture and design:
- [Architecture Overview](architecture/README.md) - Architecture documentation index
- [System Design](architecture/system-design.md) - High-level system architecture
- [Comprehensive Architecture](architecture/comprehensive-system-architecture.md) - Detailed architecture
- [Communication Protocol](architecture/node-communication-protocol.md) - Inter-node communication
- [Connectors](architecture/connectors.md) - Connector system architecture
- [Architecture Decision Records](architecture/adrs/) - Design decisions and rationale

### 🚀 Deployment Documentation (`deployment/`)
Deployment guides and operations:
- [Deployment Overview](deployment/README.md) - Deployment documentation index
- [Deployment Guide](deployment/guide.md) - General deployment instructions
- [Kubernetes Deployment](deployment/kubernetes.md) - Deploy on Kubernetes
- [Enterprise Deployment](deployment/enterprise-deployment.md) - Enterprise-grade deployment

### 💻 Development Documentation (`development/`)
Developer guides and contribution information:
- [Development Overview](development/README.md) - Development documentation index
- [Development Setup](development/setup/README.md) - Local development environment
- [Control Plane Implementation](development/CONTROL_PLANE_IMPLEMENTATION.md) - Control plane development
- [Native Runner Enhancements](development/NATIVE_RUNNER_ENHANCEMENTS.md) - Native runner development
- [Automated Pipeline Testing](development/automated-pipeline-testing.md) - Testing framework

### 🔄 Migration Documentation (`migration/`)
Migration guides and procedures:
- [Observability Migration](migration/OBSERVABILITY_MIGRATION_GUIDE.md) - Observability system migration
- [Configuration Migration](migration/configuration-migration.md) - Configuration migration guide

### ⚙️ DevOps Documentation (`devops/`)
Operations and DevOps guides:
- [DevOps Guide](devops/README.md) - Complete DevOps and operations guide
- [YAML Compatibility](devops/yaml-compatibility.md) - YAML configuration compatibility

### 📁 Archive (`archive/`)
Historical documentation and legacy content:
- [Archive Overview](archive/README.md) - Information about archived content
- [Summaries](archive/summaries/) - Historical project summaries
- [Cleanup Documentation](archive/cleanup/) - Legacy cleanup documentation
- [Legacy Protocol Documentation](archive/valkyrie/) - Historical protocol documentation

## Getting Started

### For New Users
1. Start with the [Installation Guide](user/installation.md) or [Getting Started](user/getting-started/)
2. Follow the [Quick Start Guide](user/quick-start.md) to run your first pipeline
3. Explore [Pipeline Examples](user/pipeline-examples/) for real-world use cases
4. Configure your setup with the [Configuration Guide](user/configuration.md)
5. Need help? Check the [Troubleshooting Guide](user/troubleshooting.md)

### For Developers
1. Review the [Architecture Overview](architecture/README.md) to understand the system
2. Set up your [Development Environment](development/setup/README.md)
3. Read the [System Design](architecture/system-design.md) for technical details
4. Explore the [API Documentation](api/README.md) and [API Reference](api/reference/README.md)
5. Check [Development Guidelines](development/README.md) for contribution standards

### For Operators
1. Read the [Deployment Guide](deployment/README.md) for deployment options
2. Choose your deployment method: [Kubernetes](deployment/kubernetes.md), [Docker](deployment/guide.md), etc.
3. Review the [DevOps Guide](devops/README.md) for operational best practices
4. Set up monitoring using [Architecture Guidelines](architecture/README.md)
5. Configure security following [Enterprise Deployment](deployment/enterprise-deployment.md)

### For Contributors
1. Set up the [Development Environment](development/setup/README.md)
2. Read the [Contributing Guidelines](../CONTRIBUTING.md)
3. Explore the [Architecture Documentation](architecture/README.md)
4. Check existing [Issues](https://github.com/rustci/rustci/issues)

## Navigation Tips

- **Quick answers**: Check the [Troubleshooting Guide](user/troubleshooting.md)
- **API integration**: Start with [API Documentation](api/README.md)
- **System understanding**: Review [Architecture Documentation](architecture/README.md)
- **Deployment help**: See [Deployment Documentation](deployment/README.md)
- **Historical context**: Browse the [Archive](archive/README.md)

## Document Organization

This documentation follows a user-centric organization:

- **Audience-based structure**: Documentation is organized by who needs it
- **Progressive disclosure**: Start simple, dive deeper as needed
- **Cross-references**: Related topics are linked for easy navigation
- **Searchable content**: All documentation is indexed and searchable
- **Version controlled**: Documentation evolves with the codebase

## Contributing to Documentation

We welcome documentation improvements! To contribute:

1. Follow the existing structure and style
2. Update cross-references when adding new content
3. Test all links and examples
4. Submit pull requests with clear descriptions

## Getting Help

- **Community**: [GitHub Discussions](https://github.com/rustci/rustci/discussions)
- **Issues**: [GitHub Issues](https://github.com/rustci/rustci/issues)
- **Chat**: [Discord Community](https://discord.gg/rustci)

---

**Documentation Status**: ✅ Reorganized and up-to-date  
**Last Updated**: January 2025  
**Total Sections**: 8 major documentation categories  
**Coverage**: Complete user, developer, and operator documentation