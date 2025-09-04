# Valkyrie Protocol API Reference

This document provides comprehensive API reference for the Valkyrie Protocol, including all public interfaces, message types, and configuration options.

## Core API

### ValkyrieEngine

The main protocol engine that manages connections, message routing, and protocol operations.

```rust
pub struct ValkyrieEngine {
    // Internal fields are private
}

impl ValkyrieEngine {
    /// Create a new Valkyrie Protocol engine
    pub fn new(config: ValkyrieConfig) -> Result<Self, ValkyrieError>;
    
    /// Start the protocol engine
    pub async fn start(&mut self) -> Result<(), ValkyrieError>;
    
    /// Stop the protocol engine gracefully
    pub async fn stop(&mut self) -> Result<(), ValkyrieError>;
    
    /// Create a new connection to a remote endpoint
    pub async fn connect(&self, endpoint: Endpoint) -> Result<ConnectionHandle, ValkyrieError>;
    
    /// Listen for incoming connections
    pub async fn listen(&self, bind_address: SocketAddr) -> Result<Listener, ValkyrieError>;
    
    /// Send a message to a specific connection
    pub async fn send_message(
        &self,
        connection: ConnectionId,
        message: ValkyrieMessage,
    ) -> Result<(), ValkyrieError>;
    
    /// Broadcast a message to multiple connections
    pub async fn broadcast(
        &self,
        connections: Vec<ConnectionId>,
        message: ValkyrieMessage,
    ) -> Result<BroadcastResult, ValkyrieError>;
    
    /// Register a message handler
    pub fn register_handler<H>(&self, message_type: MessageType, handler: H)
    where
        H: MessageHandler + Send + Sync + 'static;
    
    /// Get engine statistics
    pub fn stats(&self) -> EngineStats;
    
    /// Get active connections
    pub fn connections(&self) -> Vec<ConnectionInfo>;
}
```

### ValkyrieClient

High-level client for connecting to Valkyrie nodes.

```rust
pub struct ValkyrieClient {
    // Internal fields are private
}

impl ValkyrieClient {
    /// Create a new client with default configuration
    pub async fn new() -> Result<Self, ValkyrieError>;
    
    /// Create a new client with custom configuration
    pub async fn with_config(config: ClientConfig) -> Result<Self, ValkyrieError>;
    
    /// Connect to a remote endpoint
    pub async fn connect(&self, endpoint: Endpoint) -> Result<Connection, ValkyrieError>;
    
    /// Connect with custom connection options
    pub async fn connect_with_options(
        &self,
        endpoint: Endpoint,
        options: ConnectionOptions,
    ) -> Result<Connection, ValkyrieError>;
    
    /// Disconnect from all endpoints
    pub async fn disconnect_all(&self) -> Result<(), ValkyrieError>;
}
```

### Connection

Represents an active connection to a remote node.

```rust
pub struct Connection {
    // Internal fields are private
}

impl Connection {
    /// Send a message and wait for response
    pub async fn send_request(&self, message: ValkyrieMessage) -> Result<ValkyrieMessage, ValkyrieError>;
    
    /// Send a message without waiting for response
    pub async fn send(&self, message: ValkyrieMessage) -> Result<(), ValkyrieError>;
    
    /// Open a new stream
    pub async fn open_stream(&self, stream_type: StreamType) -> Result<Stream, ValkyrieError>;
    
    /// Close the connection
    pub async fn close(&self) -> Result<(), ValkyrieError>;
    
    /// Get connection information
    pub fn info(&self) -> ConnectionInfo;
    
    /// Check if connection is active
    pub fn is_active(&self) -> bool;
}
```

## Message Types

### ValkyrieMessage

The core message structure used throughout the protocol.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieMessage {
    /// Message header with protocol metadata
    pub header: MessageHeader,
    /// Message payload
    pub payload: MessagePayload,
    /// Message signature for integrity verification
    pub signature: Option<MessageSignature>,
    /// Tracing context for distributed tracing
    pub trace_context: Option<TraceContext>,
}

impl ValkyrieMessage {
    /// Create a new message
    pub fn new(message_type: MessageType, payload: MessagePayload) -> Self;
    
    /// Create a ping message
    pub fn ping() -> Self;
    
    /// Create a pong message
    pub fn pong() -> Self;
    
    /// Create an error message
    pub fn error(error: ValkyrieError) -> Self;
    
    /// Create a job request message
    pub fn job_request(job_spec: JobSpec) -> Self;
    
    /// Get message size in bytes
    pub fn size(&self) -> usize;
    
    /// Validate message integrity
    pub fn validate(&self) -> Result<(), ValkyrieError>;
}
```

### MessageHeader

Contains protocol metadata and routing information.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Protocol magic number and version
    pub protocol_info: ProtocolInfo,
    /// Message type and subtype
    pub message_type: MessageType,
    /// Stream identifier for multiplexing
    pub stream_id: StreamId,
    /// Message flags and control bits
    pub flags: MessageFlags,
    /// Message priority for QoS
    pub priority: MessagePriority,
    /// Message timestamp
    pub timestamp: DateTime<Utc>,
    /// Message TTL for expiration
    pub ttl: Option<Duration>,
    /// Correlation ID for request-response matching
    pub correlation_id: Option<Uuid>,
    /// Routing information
    pub routing: RoutingInfo,
}
```

### MessageType

Enumeration of all supported message types.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    // Core protocol messages (0x00-0x0F)
    Hello = 0x00,
    HelloAck = 0x01,
    Ping = 0x02,
    Pong = 0x03,
    Error = 0x04,
    Close = 0x05,
    Upgrade = 0x06,
    Negotiate = 0x07,
    
    // Authentication and security (0x10-0x1F)
    AuthChallenge = 0x10,
    AuthResponse = 0x11,
    AuthSuccess = 0x12,
    AuthFailure = 0x13,
    KeyExchange = 0x14,
    CertificateRequest = 0x15,
    
    // Job management (0x20-0x2F)
    JobRequest = 0x20,
    JobAccept = 0x21,
    JobReject = 0x22,
    JobStart = 0x23,
    JobProgress = 0x24,
    JobComplete = 0x25,
    JobFailed = 0x26,
    JobCancel = 0x27,
    JobMigrate = 0x28,
    JobPause = 0x29,
    JobResume = 0x2A,
    
    // Streaming and data transfer (0x30-0x3F)
    StreamOpen = 0x30,
    StreamData = 0x31,
    StreamClose = 0x32,
    StreamAck = 0x33,
    StreamReset = 0x34,
    FileTransferStart = 0x35,
    FileTransferChunk = 0x36,
    FileTransferComplete = 0x37,
    
    // Observability and monitoring (0x40-0x4F)
    MetricsReport = 0x40,
    LogEntry = 0x41,
    TraceSpan = 0x42,
    HealthCheck = 0x43,
    StatusUpdate = 0x44,
    AlertNotification = 0x45,
    
    // Cluster management (0x50-0x5F)
    NodeJoin = 0x50,
    NodeLeave = 0x51,
    NodeUpdate = 0x52,
    ClusterState = 0x53,
    LeaderElection = 0x54,
    ConsensusProposal = 0x55,
    ConsensusVote = 0x56,
    
    // Service discovery and routing (0x60-0x6F)
    ServiceRegister = 0x60,
    ServiceDeregister = 0x61,
    ServiceQuery = 0x62,
    ServiceResponse = 0x63,
    RouteUpdate = 0x64,
    LoadBalanceUpdate = 0x65,
    
    // Custom and extensible (0xFF00+)
    Custom(u16) = 0xFF00,
}
```

## Configuration

### ValkyrieConfig

Main configuration structure for the protocol engine.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieConfig {
    /// Protocol version and compatibility settings
    pub protocol: ProtocolConfig,
    /// Transport layer configuration
    pub transport: TransportLayerConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Stream multiplexing configuration
    pub streaming: StreamingConfig,
    /// Performance tuning parameters
    pub performance: PerformanceConfig,
    /// Observability configuration
    pub observability: ObservabilityConfig,
    /// Feature flags
    pub features: FeatureFlags,
}

impl ValkyrieConfig {
    /// Create a new configuration builder
    pub fn builder() -> ValkyrieConfigBuilder;
    
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ValkyrieError>;
    
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ValkyrieError>;
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ValkyrieError>;
}
```

### SecurityConfig

Security-related configuration options.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Security mode
    pub mode: SecurityMode,
    /// Authentication configuration
    pub authentication: AuthConfig,
    /// Encryption configuration
    pub encryption: EncryptionConfig,
    /// Certificate configuration
    pub certificates: CertConfig,
    /// Intrusion detection settings
    pub intrusion_detection: IdsConfig,
    /// Audit logging configuration
    pub audit: AuditConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityMode {
    /// No security (development only)
    Plaintext,
    /// Token-based authentication
    Token { secret: String },
    /// Noise protocol with Curve25519
    Noise { private_key: String },
    /// Mutual TLS
    MutualTls { cert_path: String, key_path: String },
    /// Post-quantum cryptography
    PostQuantum { key_bundle: String },
    /// Zero-trust architecture
    ZeroTrust { config: ZeroTrustConfig },
}
```

### TransportLayerConfig

Transport layer configuration.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportLayerConfig {
    /// Primary transport
    pub primary: TransportConfig,
    /// Fallback transports
    pub fallbacks: Vec<TransportConfig>,
    /// Transport selection strategy
    pub selection_strategy: TransportSelectionStrategy,
    /// Connection pooling settings
    pub connection_pooling: ConnectionPoolConfig,
    /// Load balancing configuration
    pub load_balancing: LoadBalancingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport type
    pub transport_type: TransportType,
    /// Bind address
    pub bind_address: SocketAddr,
    /// Transport-specific options
    pub options: TransportOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportType {
    Tcp,
    Quic,
    WebSocket,
    UnixSocket,
    Rdma,
    SharedMemory,
}
```

## Streaming API

### Stream

Represents a multiplexed stream within a connection.

```rust
pub struct Stream {
    // Internal fields are private
}

impl Stream {
    /// Send data on the stream
    pub async fn send(&mut self, data: Bytes) -> Result<(), ValkyrieError>;
    
    /// Receive data from the stream
    pub async fn recv(&mut self) -> Result<Option<Bytes>, ValkyrieError>;
    
    /// Close the stream
    pub async fn close(&mut self) -> Result<(), ValkyrieError>;
    
    /// Get stream information
    pub fn info(&self) -> StreamInfo;
    
    /// Set stream priority
    pub fn set_priority(&mut self, priority: StreamPriority);
    
    /// Get current flow control window
    pub fn flow_window(&self) -> FlowWindow;
}
```

### StreamType

Different types of streams with specific characteristics.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamType {
    /// Control stream for protocol messages
    Control,
    /// Job execution stream
    Job,
    /// Data transfer stream
    Data,
    /// Log streaming
    Log,
    /// Metrics streaming
    Metrics,
    /// File transfer
    FileTransfer,
    /// Custom stream type
    Custom(u16),
}
```

## HTTP Bridge API

### BridgeConfig

Configuration for the HTTP/HTTPS bridge.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Enable the HTTP bridge
    pub enabled: bool,
    /// HTTP port
    pub http_port: u16,
    /// HTTPS port
    pub https_port: u16,
    /// Automatic protocol upgrade
    pub auto_upgrade: bool,
    /// TLS configuration
    pub tls: Option<TlsConfig>,
    /// CORS configuration
    pub cors: CorsConfig,
    /// Rate limiting
    pub rate_limiting: RateLimitConfig,
}
```

### HTTP Endpoints

The bridge exposes RESTful endpoints that map to Valkyrie protocol operations:

#### Job Management

```
POST   /api/v1/jobs              # Create new job
GET    /api/v1/jobs              # List jobs
GET    /api/v1/jobs/{id}         # Get job details
PUT    /api/v1/jobs/{id}         # Update job
DELETE /api/v1/jobs/{id}         # Cancel job
POST   /api/v1/jobs/{id}/pause   # Pause job
POST   /api/v1/jobs/{id}/resume  # Resume job
```

#### Node Management

```
GET    /api/v1/nodes             # List nodes
GET    /api/v1/nodes/{id}        # Get node details
POST   /api/v1/nodes/{id}/drain  # Drain node
POST   /api/v1/nodes/{id}/cordon # Cordon node
```

#### Health and Metrics

```
GET    /health                   # Health check
GET    /metrics                  # Prometheus metrics
GET    /api/v1/status            # Detailed status
```

## Error Handling

### ValkyrieError

Comprehensive error type covering all protocol operations.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ValkyrieError {
    // Protocol-level errors
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
    
    #[error("Invalid message format: {details}")]
    InvalidMessageFormat { details: String },
    
    #[error("Message too large: {size} bytes exceeds limit of {limit}")]
    MessageTooLarge { size: usize, limit: usize },
    
    // Transport errors
    #[error("Transport error: {transport_type}: {message}")]
    TransportError { transport_type: String, message: String },
    
    #[error("Connection failed: {endpoint}: {reason}")]
    ConnectionFailed { endpoint: String, reason: String },
    
    #[error("Connection lost: {connection_id}")]
    ConnectionLost { connection_id: ConnectionId },
    
    // Security errors
    #[error("Authentication failed: {method}: {reason}")]
    AuthenticationFailed { method: String, reason: String },
    
    #[error("Authorization denied: {operation} on {resource}")]
    AuthorizationDenied { operation: String, resource: String },
    
    #[error("Encryption error: {cipher}: {message}")]
    EncryptionError { cipher: String, message: String },
    
    // Stream errors
    #[error("Stream error: {stream_id}: {error_type}")]
    StreamError { stream_id: StreamId, error_type: StreamErrorType },
    
    #[error("Flow control violation: {stream_id}: {details}")]
    FlowControlViolation { stream_id: StreamId, details: String },
    
    // Job execution errors
    #[error("Job execution failed: {job_id}: {reason}")]
    JobExecutionFailed { job_id: JobId, reason: String },
    
    #[error("Resource exhausted: {resource}: {details}")]
    ResourceExhausted { resource: String, details: String },
    
    // System errors
    #[error("Internal error: {component}: {message}")]
    InternalError { component: String, message: String },
    
    #[error("Configuration error: {parameter}: {message}")]
    ConfigurationError { parameter: String, message: String },
    
    // Network errors
    #[error("Network partition detected: {affected_nodes:?}")]
    NetworkPartition { affected_nodes: Vec<NodeId> },
    
    #[error("Timeout: {operation} timed out after {duration:?}")]
    Timeout { operation: String, duration: Duration },
}
```

## Observability

### Metrics

The protocol exposes comprehensive metrics via Prometheus:

```rust
/// Get current metrics snapshot
pub fn metrics() -> MetricsSnapshot;

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Connection metrics
    pub connections: ConnectionMetrics,
    /// Message metrics
    pub messages: MessageMetrics,
    /// Stream metrics
    pub streams: StreamMetrics,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Error metrics
    pub errors: ErrorMetrics,
}
```

### Tracing

OpenTelemetry integration for distributed tracing:

```rust
use opentelemetry::trace::Tracer;

// Automatic tracing for all protocol operations
#[tracing::instrument(skip(self, message))]
async fn handle_message(&self, message: ValkyrieMessage) -> Result<()> {
    // Implementation automatically traced
}
```

## Language Bindings

### Rust (Native)

```rust
use valkyrie_protocol::*;

// Full native API access
let engine = ValkyrieEngine::new(config).await?;
```

### Python

```python
import valkyrie_protocol

# Async/await support
engine = await valkyrie_protocol.ValkyrieEngine.new(config)
await engine.start()
```

### JavaScript/TypeScript

```typescript
import { ValkyrieEngine, ValkyrieConfig } from 'valkyrie-protocol';

// Promise-based API
const engine = await ValkyrieEngine.new(config);
await engine.start();
```

### Go

```go
import "github.com/rustci/valkyrie-protocol-go"

// Idiomatic Go with channels
engine, err := valkyrie.NewEngine(config)
if err != nil {
    return err
}
```

### Java

```java
import dev.rustci.valkyrie.ValkyrieEngine;
import dev.rustci.valkyrie.ValkyrieConfig;

// Reactive streams with Project Reactor
ValkyrieEngine engine = ValkyrieEngine.create(config);
engine.start().block();
```

### C++

```cpp
#include <valkyrie/engine.hpp>

// Modern C++20 with coroutines
auto engine = co_await valkyrie::Engine::create(config);
co_await engine.start();
```

## Examples

### Basic Client-Server

```rust
// Server
#[tokio::main]
async fn main() -> Result<(), ValkyrieError> {
    let config = ValkyrieConfig::builder()
        .bind_address("127.0.0.1:8080".parse()?)
        .build();
    
    let mut engine = ValkyrieEngine::new(config).await?;
    engine.start().await?;
    
    // Handle incoming connections
    tokio::signal::ctrl_c().await?;
    engine.stop().await?;
    Ok(())
}

// Client
#[tokio::main]
async fn main() -> Result<(), ValkyrieError> {
    let client = ValkyrieClient::new().await?;
    let endpoint = Endpoint::parse("valkyrie://127.0.0.1:8080")?;
    let connection = client.connect(endpoint).await?;
    
    let message = ValkyrieMessage::ping();
    let response = connection.send_request(message).await?;
    
    println!("Response: {:?}", response);
    Ok(())
}
```

### Job Execution

```rust
// Submit a job
let job_spec = JobSpec::builder()
    .pipeline("build-test-deploy")
    .branch("main")
    .environment("production")
    .build();

let job_request = ValkyrieMessage::job_request(job_spec);
let response = connection.send_request(job_request).await?;

match response.header.message_type {
    MessageType::JobAccept => println!("Job accepted"),
    MessageType::JobReject => println!("Job rejected"),
    _ => println!("Unexpected response"),
}
```

### Stream Processing

```rust
// Open a data stream
let stream = connection.open_stream(StreamType::Data).await?;

// Send data
stream.send(Bytes::from("Hello, Valkyrie!")).await?;

// Receive data
while let Some(data) = stream.recv().await? {
    println!("Received: {:?}", data);
}

stream.close().await?;
```

This API reference provides comprehensive coverage of the Valkyrie Protocol's public interface. For more examples and detailed usage patterns, see the [Getting Started Guide](getting-started.md) and [examples directory](../../examples/).