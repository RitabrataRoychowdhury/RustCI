# RustCI Documentation

Welcome to RustCI - a high-performance CI/CD platform built in Rust that serves as a modern alternative to Jenkins.

## Quick Navigation

### Getting Started
- [Installation & Setup](./getting-started/installation.md)
- [Quick Start Guide](./getting-started/quick-start.md)

### API Reference
- [Complete API Documentation](./api/README.md)

### Deployment Guides
- [Kubernetes Deployment](./deployment/kubernetes.md)
- [General Deployment Guide](./deployment/guide.md)

### Architecture & Development
- [System Architecture](./architecture/system-design.md)
- [Connector System Guide](./architecture/connectors.md)

### Examples & Integration
- [Pipeline Examples](./pipeline-examples/README.md)
- [Rust Code Examples](../rust-examples/README.md)
- [Integration Validation](./integration/validation.md)

## What is RustCI?

RustCI is a high-performance CI/CD platform that focuses on:

- **Speed**: Built in Rust for maximum performance and low resource usage
- **Simplicity**: YAML-based pipeline definitions that are readable and maintainable
- **Flexibility**: Support for Docker, Kubernetes, and multi-cloud deployments
- **Integration**: Seamless GitHub integration with webhook support
- **Monitoring**: Real-time pipeline execution tracking with detailed logging

## Key Features

- RESTful API for integration and automation
- Multipart file upload for YAML configurations
- JWT-based authentication with GitHub OAuth
- MongoDB persistence layer
- Docker container support with Bollard integration
- Kubernetes connector for cloud deployments
- Multi-cloud support (AWS, Azure, GCP)
- Real-time execution monitoring

## Architecture Overview

RustCI follows a modular, connector-based architecture using Strategy, Factory, and Facade design patterns. This allows for extensible pipeline execution across different platforms while maintaining a unified interface.

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web Client    │    │   CLI Tools     │    │   Webhooks      │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   Axum Server   │
                    └─────────┬───────┘
                              │
                    ┌─────────────────┐
                    │   CI Engine     │
                    └─────────┬───────┘
                              │
          ┌───────────────────┼───────────────────┐
          │                   │                   │
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ Docker Connector│ │  K8s Connector  │ │ Cloud Connectors│
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

## Support

- **Documentation**: This comprehensive guide
- **Pipeline Examples**: YAML configuration examples in the `pipeline-examples/` directory
- **Rust Code Examples**: Rust code examples in the `../rust-examples/` directory
- **API Reference**: Complete API documentation with cURL examples

## License

RustCI is open source software. See the LICENSE file for details.