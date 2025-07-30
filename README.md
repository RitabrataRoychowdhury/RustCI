# RustCI - High-Performance CI/CD Platform

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com)

RustCI is a high-performance, scalable CI/CD platform built in Rust that serves as a modern alternative to Jenkins. The platform focuses on speed, reliability, and ease of configuration with support for multi-node clusters and various deployment strategies.

## 🚀 Features

- **High Performance**: Built in Rust for maximum performance and low resource usage
- **Multi-Node Clustering**: Distributed execution across multiple nodes with automatic failover
- **Multiple Runner Types**: Support for local, Docker, and Kubernetes runners
- **Event-Driven Architecture**: Libuv-inspired event demultiplexing for high concurrency
- **Real-time Monitoring**: Comprehensive metrics with Prometheus integration
- **GitHub Integration**: Seamless OAuth authentication and webhook support
- **API Documentation**: Interactive Swagger UI for easy API exploration
- **Clean Architecture**: Well-organized codebase following clean architecture principles

## 📋 Table of Contents

- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Deployment Options](#deployment-options)
- [Configuration](#configuration)
- [API Documentation](#api-documentation)
- [Monitoring & Metrics](#monitoring--metrics)
- [Development](#development)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)

## 🏗️ Architecture

RustCI follows a clean architecture pattern with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   REST API  │  │ Swagger UI  │  │    Middleware       │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                   Application Layer                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Handlers   │  │  Services   │  │    Use Cases        │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                     Domain Layer                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Entities   │  │ Repositories│  │   Domain Services   │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                 Infrastructure Layer                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Database   │  │   Runners   │  │    External APIs    │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Core Components

- **Event Loop**: Libuv-inspired event demultiplexing for high-performance async operations
- **Runner Pool**: Manages multiple runner types (Local, Docker, Kubernetes)
- **Cluster Coordinator**: Handles multi-node coordination and job distribution
- **Job Queue**: Efficient job scheduling with priority and retry support
- **Observability**: Comprehensive monitoring, metrics, and health checks

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ (Edition 2021)
- Docker and Docker Compose
- MongoDB (for persistence)
- Git

### Local Development

1. **Clone the repository**
   ```bash
   git clone https://github.com/your-org/rustci.git
   cd rustci
   ```

2. **Set up environment variables**
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

3. **Start dependencies**
   ```bash
   docker-compose up -d mongodb
   ```

4. **Run the application**
   ```bash
   cargo run
   ```

5. **Access the application**
   - API: http://localhost:8080
   - Health Check: http://localhost:8080/health
   - Swagger UI: http://localhost:8080/swagger-ui
   - GitHub OAuth: http://localhost:8080/api/sessions/oauth/github

## 🚢 Deployment Options

### Single Node Deployment

For development and small-scale deployments:

```bash
docker-compose up -d
```

### Multi-Node Cluster Deployment

For production environments with high availability:

```bash
# Set up environment variables
export GITHUB_OAUTH_CLIENT_ID=your_client_id
export GITHUB_OAUTH_CLIENT_SECRET=your_client_secret

# Deploy multi-node cluster
docker-compose -f docker-compose.multi-node.yaml up -d
```

This deploys:
- 3 Master nodes (High Availability)
- 2 Worker nodes (Scalable execution)
- MongoDB (Persistence)
- Prometheus (Metrics)
- HAProxy (Load Balancing)

### Kubernetes Deployment

For cloud-native deployments:

```bash
# Apply Kubernetes manifests
kubectl apply -f deployment/kubernetes/

# Or use Helm
helm install rustci ./helm/rustci
```

### K3s on macOS

For local Kubernetes testing:

```bash
# Install K3s
curl -sfL https://get.k3s.io | sh -

# Deploy RustCI
kubectl apply -f deployment/k3s/
```

## ⚙️ Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_ENV` | Environment (development/production) | `development` |
| `SERVER_HOST` | Server bind address | `0.0.0.0` |
| `SERVER_PORT` | Server port | `8080` |
| `MONGODB_URI` | MongoDB connection string | `mongodb://localhost:27017` |
| `MONGODB_DATABASE` | Database name | `rustci` |
| `JWT_SECRET` | JWT signing secret | Required |
| `GITHUB_OAUTH_CLIENT_ID` | GitHub OAuth client ID | Required |
| `GITHUB_OAUTH_CLIENT_SECRET` | GitHub OAuth client secret | Required |
| `NODE_ROLE` | Node role (master/worker/hybrid) | `master` |
| `CLUSTER_NODES` | Comma-separated cluster node addresses | - |
| `MAX_CONCURRENT_JOBS` | Maximum concurrent jobs per node | `4` |
| `ENABLE_METRICS` | Enable Prometheus metrics | `true` |
| `LOG_LEVEL` | Logging level | `info` |

### Configuration Files

- `config.example.yaml` - Basic configuration template
- `config.enhanced.example.yaml` - Advanced configuration with all options
- `config/prometheus.yml` - Prometheus scraping configuration
- `config/haproxy.cfg` - Load balancer configuration

## 📚 API Documentation

RustCI provides comprehensive API documentation through Swagger UI:

- **Swagger UI**: http://localhost:8080/swagger-ui
- **OpenAPI Spec**: http://localhost:8080/api-docs/openapi.json

### Key API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/sessions/oauth/github` | GET | GitHub OAuth login |
| `/api/ci/pipelines` | POST | Create pipeline |
| `/api/cluster/status` | GET | Cluster status |
| `/api/cluster/nodes` | GET | List cluster nodes |
| `/metrics` | GET | Prometheus metrics |

## 📊 Monitoring & Metrics

### Prometheus Metrics

RustCI exports comprehensive metrics for monitoring:

- **Runner Metrics**: Active runners, job capacity, health status
- **Job Metrics**: Queue length, execution times, success/failure rates
- **Cluster Metrics**: Node health, load distribution, failover events
- **System Metrics**: CPU, memory, disk usage
- **HTTP Metrics**: Request rates, response times, error rates

### Accessing Metrics

- **Prometheus**: http://localhost:9090 (in multi-node setup)
- **Metrics Endpoint**: http://localhost:8080/metrics
- **HAProxy Stats**: http://localhost:8404/stats

### Key Metrics

```
# Runner metrics
rustci_runners_total
rustci_runners_active
rustci_runners_idle
rustci_runners_failed

# Job metrics
rustci_jobs_total
rustci_jobs_running
rustci_jobs_queued
rustci_jobs_completed
rustci_jobs_failed

# Cluster metrics
rustci_cluster_nodes_total
rustci_cluster_nodes_healthy
rustci_cluster_nodes_unhealthy

# HTTP metrics
rustci_http_requests_total
rustci_http_request_duration_seconds
```

## 🛠️ Development

### Building from Source

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with hot reload
cargo watch -x run
```

### Code Quality

```bash
# Format code
cargo fmt

# Run lints
cargo clippy -- -D warnings

# Check without building
cargo check
```

### Testing

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration

# Load tests
cargo test --test load

# Chaos tests
cargo test --test chaos
```

## 🔧 Troubleshooting

### Common Issues

#### 1. Connection Refused Errors

**Problem**: Cannot connect to MongoDB or other services

**Solution**:
```bash
# Check if MongoDB is running
docker ps | grep mongo

# Restart MongoDB
docker-compose restart mongodb

# Check logs
docker-compose logs mongodb
```

#### 2. GitHub OAuth Not Working

**Problem**: OAuth authentication fails

**Solution**:
1. Verify GitHub OAuth app configuration
2. Check redirect URL matches exactly
3. Ensure environment variables are set:
   ```bash
   echo $GITHUB_OAUTH_CLIENT_ID
   echo $GITHUB_OAUTH_CLIENT_SECRET
   ```

#### 3. High Memory Usage

**Problem**: Application consuming too much memory

**Solution**:
1. Check for memory leaks in logs
2. Adjust job concurrency:
   ```bash
   export MAX_CONCURRENT_JOBS=2
   ```
3. Monitor metrics: http://localhost:8080/metrics

#### 4. Cluster Nodes Not Joining

**Problem**: Worker nodes cannot join cluster

**Solution**:
1. Check network connectivity between nodes
2. Verify `CLUSTER_NODES` environment variable
3. Check firewall settings
4. Review cluster coordinator logs:
   ```bash
   docker logs rustci-master-1
   ```

#### 5. Jobs Stuck in Queue

**Problem**: Jobs not being executed

**Solution**:
1. Check runner status: `GET /api/cluster/runners`
2. Verify runner capacity
3. Check job requirements vs available runners
4. Review job queue metrics

### Debug Mode

Enable debug logging for troubleshooting:

```bash
export LOG_LEVEL=debug
export RUST_LOG=rustci=debug
```

### Health Checks

Monitor system health:

```bash
# Overall health
curl http://localhost:8080/health

# Cluster status
curl http://localhost:8080/api/cluster/status

# Individual node health
curl http://localhost:8080/api/cluster/nodes/{node_id}/health
```

### Log Analysis

Important log patterns to watch for:

```bash
# Connection issues
grep -i "connection" logs/rustci.log

# Authentication failures
grep -i "auth" logs/rustci.log

# Job execution errors
grep -i "job.*error" logs/rustci.log

# Cluster coordination issues
grep -i "cluster.*failed" logs/rustci.log
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run the test suite
6. Submit a pull request

### Code Style

- Follow Rust conventions
- Use `cargo fmt` for formatting
- Ensure `cargo clippy` passes
- Add documentation for public APIs
- Include tests for new features

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by Jenkins and modern CI/CD platforms
- Built with the amazing Rust ecosystem
- Event loop design inspired by libuv
- Clean architecture principles from Uncle Bob

---

For more information, visit our [documentation](docs/) or join our [community discussions](https://github.com/your-org/rustci/discussions).