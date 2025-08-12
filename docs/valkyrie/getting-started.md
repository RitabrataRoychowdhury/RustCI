# Getting Started with Valkyrie Protocol

Welcome to the Valkyrie Protocol - RustCI's flagship distributed communication framework. This guide will help you get up and running with the protocol in minutes.

## Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- Docker (optional, for containerized deployment)
- MongoDB (for persistence)

### Installation

#### Option 1: Using Cargo

```bash
# Add to your Cargo.toml
[dependencies]
valkyrie-protocol = "2.0"
tokio = { version = "1.0", features = ["full"] }
```

#### Option 2: From Source

```bash
git clone https://github.com/rustci/rustci.git
cd rustci
cargo build --release --features valkyrie
```

### Basic Usage

#### Creating a Valkyrie Engine

```rust
use valkyrie_protocol::{ValkyrieEngine, ValkyrieConfig};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = ValkyrieConfig::builder()
        .bind_address("127.0.0.1:8080".parse()?)
        .security_mode(SecurityMode::Token)
        .transport(TransportType::Tcp)
        .build();

    // Initialize engine
    let mut engine = ValkyrieEngine::new(config).await?;
    
    // Start the engine
    engine.start().await?;
    
    println!("Valkyrie Protocol engine started on 127.0.0.1:8080");
    
    // Keep running
    tokio::signal::ctrl_c().await?;
    
    // Graceful shutdown
    engine.stop().await?;
    
    Ok(())
}
```

#### Connecting to a Valkyrie Node

```rust
use valkyrie_protocol::{ValkyrieClient, Endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = ValkyrieClient::new().await?;
    
    // Connect to remote node
    let endpoint = Endpoint::parse("valkyrie://node1.example.com:8080")?;
    let connection = client.connect(endpoint).await?;
    
    // Send a message
    let message = ValkyrieMessage::ping();
    let response = connection.send_request(message).await?;
    
    println!("Received response: {:?}", response);
    
    Ok(())
}
```

### Configuration

#### Basic Configuration File

Create `valkyrie.yaml`:

```yaml
# Valkyrie Protocol Configuration
protocol:
  version: "2.0"
  bind_address: "0.0.0.0:8080"
  
transport:
  primary: "tcp"
  fallbacks: ["websocket", "unix"]
  
security:
  mode: "token"
  token_secret: "${VALKYRIE_TOKEN_SECRET}"
  
performance:
  max_connections: 10000
  message_buffer_size: 65536
  
observability:
  metrics_enabled: true
  tracing_enabled: true
  log_level: "info"
```

#### Environment Variables

```bash
# Required
export VALKYRIE_TOKEN_SECRET="your-secret-key-here"

# Optional
export VALKYRIE_BIND_ADDRESS="0.0.0.0:8080"
export VALKYRIE_LOG_LEVEL="debug"
export VALKYRIE_MAX_CONNECTIONS="50000"
```

### Docker Deployment

#### Using Docker Compose

Create `docker-compose.valkyrie.yml`:

```yaml
version: '3.8'

services:
  valkyrie-node:
    image: rustci/valkyrie:latest
    ports:
      - "8080:8080"
    environment:
      - VALKYRIE_TOKEN_SECRET=your-secret-key
      - VALKYRIE_LOG_LEVEL=info
    volumes:
      - ./valkyrie.yaml:/etc/valkyrie/config.yaml
    restart: unless-stopped
    
  mongodb:
    image: mongo:7
    ports:
      - "27017:27017"
    environment:
      - MONGO_INITDB_ROOT_USERNAME=admin
      - MONGO_INITDB_ROOT_PASSWORD=password
    volumes:
      - mongodb_data:/data/db
    restart: unless-stopped

volumes:
  mongodb_data:
```

Run with:

```bash
docker-compose -f docker-compose.valkyrie.yml up -d
```

### HTTP/HTTPS Bridge

The Valkyrie Protocol includes an HTTP/HTTPS bridge for seamless integration with existing systems:

#### Enabling the Bridge

```yaml
# In valkyrie.yaml
bridge:
  enabled: true
  http_port: 8081
  https_port: 8443
  auto_upgrade: true
  tls:
    cert_file: "/etc/ssl/certs/valkyrie.crt"
    key_file: "/etc/ssl/private/valkyrie.key"
```

#### Using HTTP API

```bash
# Send a job request via HTTP
curl -X POST http://localhost:8081/api/v1/jobs \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-token" \
  -d '{
    "pipeline": "build-test-deploy",
    "branch": "main",
    "environment": "production"
  }'
```

The bridge automatically converts HTTP requests to Valkyrie protocol messages and back.

### Kubernetes Deployment

#### Using Helm

```bash
# Add the RustCI Helm repository
helm repo add rustci https://charts.rustci.dev
helm repo update

# Install Valkyrie Protocol
helm install valkyrie rustci/valkyrie-protocol \
  --set config.security.tokenSecret="your-secret-key" \
  --set service.type=LoadBalancer
```

#### Manual Deployment

Create `valkyrie-deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: valkyrie-protocol
spec:
  replicas: 3
  selector:
    matchLabels:
      app: valkyrie-protocol
  template:
    metadata:
      labels:
        app: valkyrie-protocol
    spec:
      containers:
      - name: valkyrie
        image: rustci/valkyrie:latest
        ports:
        - containerPort: 8080
        env:
        - name: VALKYRIE_TOKEN_SECRET
          valueFrom:
            secretKeyRef:
              name: valkyrie-secret
              key: token-secret
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: valkyrie-service
spec:
  selector:
    app: valkyrie-protocol
  ports:
  - port: 8080
    targetPort: 8080
  type: LoadBalancer
```

Apply with:

```bash
kubectl apply -f valkyrie-deployment.yaml
```

### Testing Your Setup

#### Health Check

```bash
# Check if the protocol is running
curl http://localhost:8080/health

# Expected response:
{
  "status": "healthy",
  "version": "2.0.0",
  "uptime": "1h 23m 45s",
  "connections": 42
}
```

#### Protocol Test

```bash
# Use the built-in test client
cargo run --bin valkyrie-test -- --endpoint valkyrie://localhost:8080

# Or use the test script
./scripts/valkyrie-test.sh
```

### Next Steps

- [API Reference](api-reference.md) - Complete API documentation
- [Deployment Guide](deployment-guide.md) - Production deployment best practices
- [Performance Tuning](performance-tuning.md) - Optimize for your workload
- [Security Guide](security-guide.md) - Secure your Valkyrie deployment
- [Troubleshooting](troubleshooting.md) - Common issues and solutions

### Examples

Check out the [examples directory](../../examples/) for complete working examples:

- [Basic Client/Server](../../examples/valkyrie_basic.rs)
- [HTTP Bridge Integration](../../examples/valkyrie_http_bridge.rs)
- [Multi-Transport Setup](../../examples/valkyrie_multi_transport.rs)
- [Security Configuration](../../examples/valkyrie_security.rs)

### Community

- **GitHub**: [https://github.com/rustci/rustci](https://github.com/rustci/rustci)
- **Discord**: Join our #valkyrie-protocol channel
- **Documentation**: [https://docs.rustci.dev/valkyrie](https://docs.rustci.dev/valkyrie)
- **Issues**: [Report bugs and feature requests](https://github.com/rustci/rustci/issues)

Welcome to the future of distributed communication! 🚀