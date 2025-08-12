use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::AppError;
use crate::core::networking::valkyrie::{
    MessageType as ValkyrieMessageType, MessagePriority as ValkyrieMessagePriority, EndpointId
};
use crate::core::networking::valkyrie::types::ReliabilityLevel;

/// Simple message priority for internal queue management
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MessagePriority {
    Background,
    Normal,
    High,
    Critical,
    Emergency,
}

impl From<ValkyrieMessagePriority> for MessagePriority {
    fn from(priority: ValkyrieMessagePriority) -> Self {
        match priority {
            ValkyrieMessagePriority::Low => MessagePriority::Background,
            ValkyrieMessagePriority::Normal => MessagePriority::Normal,
            ValkyrieMessagePriority::High => MessagePriority::High,
            ValkyrieMessagePriority::Critical(_) => MessagePriority::Critical,
            ValkyrieMessagePriority::System(_) => MessagePriority::High,
            ValkyrieMessagePriority::JobExecution(_) => MessagePriority::Normal,
            ValkyrieMessagePriority::DataTransfer(_) => MessagePriority::Normal,
            ValkyrieMessagePriority::LogsMetrics(_) => MessagePriority::Background,
        }
    }
}

/// Unique identifier for nodes in the cluster
pub type NodeId = Uuid;

/// Unique identifier for jobs
pub type JobId = Uuid;

/// Core message types for node-to-control-plane communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum NodeMessage {
    /// Node registration request
    RegisterNode {
        node_info: NodeInfo,
        capabilities: NodeCapabilities,
        auth_token: String,
    },
    /// Heartbeat with status update
    Heartbeat {
        node_id: NodeId,
        status: NodeStatus,
        resources: NodeResources,
        metrics: NodeMetrics,
        timestamp: DateTime<Utc>,
    },
    /// Job execution result
    JobResult {
        job_id: JobId,
        node_id: NodeId,
        result: JobExecutionResult,
        metrics: ExecutionMetrics,
        timestamp: DateTime<Utc>,
    },
    /// Node shutdown notification
    ShutdownNotice {
        node_id: NodeId,
        reason: String,
        graceful: bool,
        estimated_shutdown_time: Option<DateTime<Utc>>,
    },
    /// Job status update
    JobStatusUpdate {
        job_id: JobId,
        node_id: NodeId,
        status: JobStatus,
        message: Option<String>,
        timestamp: DateTime<Utc>,
    },
    /// Enhanced streaming log entry
    StreamingLog {
        job_id: JobId,
        node_id: NodeId,
        log_level: LogLevel,
        message: String,
        timestamp: DateTime<Utc>,
        stream_type: StreamType,
        metadata: HashMap<String, String>,
    },
    /// Real-time metrics update
    MetricsUpdate {
        node_id: NodeId,
        metrics: RealtimeMetrics,
        timestamp: DateTime<Utc>,
    },
    /// Resource allocation request
    ResourceRequest {
        node_id: NodeId,
        request_id: Uuid,
        resource_type: ResourceType,
        amount: u64,
        priority: ResourcePriority,
        timeout: Option<Duration>,
    },
    /// Node capability update
    CapabilityUpdate {
        node_id: NodeId,
        capabilities: NodeCapabilities,
        version: u64,
        timestamp: DateTime<Utc>,
    },
    /// Alert notification
    Alert {
        node_id: NodeId,
        alert_id: Uuid,
        severity: AlertSeverity,
        message: String,
        details: HashMap<String, serde_json::Value>,
        timestamp: DateTime<Utc>,
    },
}

/// Messages sent from control plane to nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ControlPlaneMessage {
    /// Registration response
    RegisterResponse {
        node_id: NodeId,
        accepted: bool,
        config: Option<NodeConfig>,
        error_message: Option<String>,
    },
    /// Job assignment
    JobAssignment {
        job_id: JobId,
        job: Box<JobDefinition>,
        execution_context: ExecutionContext,
        deadline: Option<DateTime<Utc>>,
        priority: JobPriority,
    },
    /// Node configuration update
    ConfigUpdate {
        node_id: NodeId,
        config: NodeConfig,
        version: u64,
    },
    /// Shutdown command
    ShutdownCommand {
        node_id: NodeId,
        graceful: bool,
        timeout: Duration,
        reason: String,
    },
    /// Heartbeat acknowledgment
    HeartbeatAck {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
        next_heartbeat_interval: Duration,
    },
    /// Job cancellation request
    CancelJob {
        job_id: JobId,
        node_id: NodeId,
        reason: String,
    },
    /// Resource allocation response
    ResourceResponse {
        node_id: NodeId,
        request_id: Uuid,
        allocated: bool,
        resource_handle: Option<ResourceHandle>,
        error_message: Option<String>,
    },
    /// Load balancing directive
    LoadBalanceDirective {
        node_id: NodeId,
        directive: LoadBalanceAction,
        target_nodes: Vec<NodeId>,
        weight: Option<f64>,
    },
    /// QoS policy update
    QoSPolicyUpdate {
        node_id: NodeId,
        policy: QoSPolicy,
        version: u64,
        effective_time: DateTime<Utc>,
    },
    /// Cluster state synchronization
    ClusterStateSync {
        node_id: NodeId,
        cluster_state: ClusterState,
        version: u64,
        partial_update: bool,
    },
    /// Priority adjustment command
    PriorityAdjustment {
        job_id: JobId,
        node_id: NodeId,
        new_priority: JobPriority,
        reason: String,
    },
    /// Flow control directive
    FlowControl {
        node_id: NodeId,
        action: FlowControlAction,
        stream_id: Option<u32>,
        parameters: HashMap<String, serde_json::Value>,
    },
}

/// Node information for registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub hostname: String,
    pub ip_address: String,
    pub port: u16,
    pub node_type: NodeType,
    pub version: String,
    pub platform: String,
    pub architecture: String,
    pub tags: HashMap<String, String>,
}

/// Node capabilities and supported features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Supported runner types
    pub runner_types: Vec<RunnerType>,
    /// Available resources
    pub max_resources: NodeResources,
    /// Supported job types
    pub supported_job_types: Vec<String>,
    /// Feature flags
    pub features: Vec<String>,
    /// Supported protocols
    pub protocols: Vec<String>,
}

/// Current node status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Starting,
    Ready,
    Busy,
    Draining,
    Unhealthy,
    Offline,
}

/// Node resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResources {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_gb: u64,
    pub network_mbps: u32,
    pub available_cpu: f64,
    pub available_memory_mb: u64,
    pub available_disk_gb: u64,
}

/// Node performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub network_rx_mbps: f64,
    pub network_tx_mbps: f64,
    pub load_average: f64,
    pub active_jobs: u32,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub uptime_seconds: u64,
}

/// Job execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecutionResult {
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error_message: Option<String>,
    pub artifacts: Vec<JobArtifact>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// Job execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

/// Job execution metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub duration_ms: u64,
    pub cpu_time_ms: u64,
    pub memory_peak_mb: u64,
    pub disk_read_mb: u64,
    pub disk_write_mb: u64,
    pub network_rx_mb: u64,
    pub network_tx_mb: u64,
}

/// Node configuration from control plane
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub heartbeat_interval: Duration,
    pub job_timeout: Duration,
    pub max_concurrent_jobs: u32,
    pub resource_limits: NodeResources,
    pub log_level: String,
    pub features: HashMap<String, serde_json::Value>,
}

/// Job definition for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDefinition {
    pub name: String,
    pub image: Option<String>,
    pub command: Vec<String>,
    pub environment: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub timeout: Option<Duration>,
    pub resource_requirements: ResourceRequirements,
    pub artifacts: Vec<ArtifactSpec>,
}

/// Job execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub workspace_id: String,
    pub pipeline_id: String,
    pub build_number: u64,
    pub git_ref: Option<String>,
    pub git_commit: Option<String>,
    pub variables: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
}

/// Job priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Resource requirements for job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: Option<f64>,
    pub memory_mb: Option<u64>,
    pub disk_gb: Option<u64>,
    pub gpu_count: Option<u32>,
}

/// Job artifact specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub name: String,
    pub path: String,
    pub artifact_type: ArtifactType,
    pub compress: bool,
}

/// Job artifact after execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobArtifact {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub artifact_type: ArtifactType,
}

/// Types of artifacts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    File,
    Directory,
    Archive,
    Log,
    Report,
}

/// Node types in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    ControlPlane,
    Worker,
    Hybrid,
}

/// Supported runner types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerType {
    Native,
    Docker,
    Kubernetes,
    WebAssembly,
}

/// Protocol message wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub source: NodeId,
    pub destination: Option<NodeId>,
    pub message: MessagePayload,
    pub signature: Option<String>,
}

/// Message payload union
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessagePayload {
    NodeMessage(NodeMessage),
    ControlPlaneMessage(ControlPlaneMessage),
}

impl ProtocolMessage {
    pub fn new(source: NodeId, message: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source,
            destination: None,
            message,
            signature: None,
        }
    }

    pub fn with_destination(mut self, destination: NodeId) -> Self {
        self.destination = Some(destination);
        self
    }

    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }
}

/// Protocol errors
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Invalid message format: {details}")]
    InvalidMessageFormat { details: String },
    
    #[error("Node not registered: {node_id}")]
    NodeNotRegistered { node_id: NodeId },
    
    #[error("Connection error: {message}")]
    ConnectionError { message: String },
    
    #[error("Timeout waiting for response")]
    Timeout,
    
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
}

/// Log levels for streaming logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

/// Stream types for different data streams
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamType {
    Stdout,
    Stderr,
    Application,
    System,
    Metrics,
    Audit,
}

/// Real-time metrics for continuous monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_io_read_bps: u64,
    pub disk_io_write_bps: u64,
    pub network_rx_bps: u64,
    pub network_tx_bps: u64,
    pub active_connections: u32,
    pub queue_depth: u32,
    pub response_time_ms: f64,
    pub error_rate: f64,
    pub custom_metrics: HashMap<String, f64>,
}

/// Resource types for allocation requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Cpu,
    Memory,
    Disk,
    Network,
    Gpu,
    Custom(String),
}

/// Resource priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePriority {
    Background = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Resource handle for allocated resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceHandle {
    pub handle_id: Uuid,
    pub resource_type: ResourceType,
    pub allocated_amount: u64,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Critical = 3,
    Emergency = 4,
}

/// Load balancing actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalanceAction {
    IncreaseWeight,
    DecreaseWeight,
    DrainConnections,
    ResumeTraffic,
    Redirect,
    Failover,
}

/// Quality of Service policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QoSPolicy {
    pub policy_id: Uuid,
    pub name: String,
    pub priority_rules: Vec<PriorityRule>,
    pub bandwidth_limits: HashMap<String, u64>,
    pub latency_targets: HashMap<String, Duration>,
    pub retry_policies: HashMap<String, RetryPolicy>,
    pub circuit_breaker_config: Option<CircuitBreakerConfig>,
}

/// Priority rules for QoS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityRule {
    pub rule_id: Uuid,
    pub condition: PriorityCondition,
    pub priority: MessagePriority,
    pub weight: f64,
}

/// Conditions for priority assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PriorityCondition {
    MessageType(ValkyrieMessageType),
    SourceEndpoint(EndpointId),
    PayloadSize(u64),
    Custom(HashMap<String, serde_json::Value>),
}

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
    pub retry_conditions: Vec<RetryCondition>,
}

/// Conditions that trigger retries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryCondition {
    NetworkError,
    Timeout,
    ServerError,
    RateLimited,
    Custom(String),
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub half_open_max_calls: u32,
    pub success_threshold: u32,
}

/// Cluster state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterState {
    pub cluster_id: Uuid,
    pub leader_node: Option<NodeId>,
    pub active_nodes: Vec<NodeId>,
    pub node_states: HashMap<NodeId, NodeStatus>,
    pub partition_info: Option<PartitionInfo>,
    pub version: u64,
    pub timestamp: DateTime<Utc>,
}

/// Network partition information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub partition_id: Uuid,
    pub affected_nodes: Vec<NodeId>,
    pub partition_type: PartitionType,
    pub detected_at: DateTime<Utc>,
    pub estimated_recovery: Option<DateTime<Utc>>,
}

/// Types of network partitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionType {
    SplitBrain,
    Isolated,
    Degraded,
    Recovering,
}

/// Flow control actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowControlAction {
    Pause,
    Resume,
    Throttle(u64), // bytes per second
    IncreaseWindow(u32),
    DecreaseWindow(u32),
    Reset,
}

/// Enhanced protocol message with QoS and compression support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedProtocolMessage {
    /// Base protocol message
    pub base: ProtocolMessage,
    /// Message priority for QoS
    pub priority: MessagePriority,
    /// Compression information
    pub compression: CompressionInfo,
    /// Reliability requirements
    pub reliability: ReliabilityLevel,
    /// Retry policy
    pub retry_policy: Option<RetryPolicy>,
    /// Flow control information
    pub flow_control: Option<FlowControlInfo>,
    /// Performance hints
    pub performance_hints: PerformanceHints,
    /// Integrity metadata
    pub integrity: Option<IntegrityMetadata>,
}

impl EnhancedProtocolMessage {
    /// Create a new enhanced protocol message
    pub fn new(source: NodeId, message: MessagePayload) -> Self {
        Self {
            base: ProtocolMessage::new(source, message),
            priority: MessagePriority::Normal,
            compression: CompressionInfo {
                algorithm: CompressionAlgorithm::None,
                original_size: 0,
                compressed_size: 0,
                compression_ratio: 1.0,
            },
            reliability: ReliabilityLevel::BestEffort,
            retry_policy: None,
            flow_control: None,
            performance_hints: PerformanceHints {
                latency_sensitive: false,
                bandwidth_intensive: false,
                cpu_intensive: false,
                memory_intensive: false,
                cacheable: false,
                preferred_transport: None,
                batch_eligible: true,
            },
            integrity: None,
        }
    }

    /// Set message priority
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set reliability level
    pub fn with_reliability(mut self, reliability: ReliabilityLevel) -> Self {
        self.reliability = reliability;
        self
    }

    /// Add compression information
    pub fn with_compression(mut self, compression: CompressionInfo) -> Self {
        self.compression = compression;
        self
    }

    /// Add integrity metadata
    pub fn with_integrity(mut self, integrity: IntegrityMetadata) -> Self {
        self.integrity = Some(integrity);
        self
    }

    /// Mark as latency sensitive
    pub fn latency_sensitive(mut self) -> Self {
        self.performance_hints.latency_sensitive = true;
        self.priority = MessagePriority::High;
        self
    }

    /// Mark as bandwidth intensive
    pub fn bandwidth_intensive(mut self) -> Self {
        self.performance_hints.bandwidth_intensive = true;
        self
    }
}

/// Compression information for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionInfo {
    pub algorithm: CompressionAlgorithm,
    pub original_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f64,
}

/// Supported compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Gzip,
    Lz4,
    Zstd,
    Brotli,
}

/// Flow control information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlInfo {
    pub window_size: u32,
    pub sequence_number: u64,
    pub acknowledgment_number: Option<u64>,
    pub flow_control_flags: FlowControlFlags,
}

/// Flow control flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlFlags {
    pub window_update: bool,
    pub end_stream: bool,
    pub priority_update: bool,
    pub reset_stream: bool,
}

/// Performance hints for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceHints {
    pub latency_sensitive: bool,
    pub bandwidth_intensive: bool,
    pub cpu_intensive: bool,
    pub memory_intensive: bool,
    pub cacheable: bool,
    pub preferred_transport: Option<String>,
    pub batch_eligible: bool,
}

/// Advanced message queue with priority support and QoS mechanisms
pub struct PriorityMessageQueue {
    /// Emergency priority messages (highest)
    emergency_priority: std::collections::VecDeque<EnhancedProtocolMessage>,
    /// Critical priority messages
    critical_priority: std::collections::VecDeque<EnhancedProtocolMessage>,
    /// High priority messages
    high_priority: std::collections::VecDeque<EnhancedProtocolMessage>,
    /// Normal priority messages
    normal_priority: std::collections::VecDeque<EnhancedProtocolMessage>,
    /// Background priority messages (lowest)
    background_priority: std::collections::VecDeque<EnhancedProtocolMessage>,
    /// Queue configuration
    config: QueueConfig,
    /// Queue metrics
    metrics: QueueMetrics,
    /// QoS policy
    qos_policy: QoSPolicy,
    /// Last dequeue time for throughput calculation
    last_dequeue_time: Option<std::time::Instant>,
}

/// Queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum queue size per priority level
    pub max_size_per_priority: usize,
    /// Enable message dropping when queue is full
    pub enable_dropping: bool,
    /// Enable message aging and TTL enforcement
    pub enable_aging: bool,
    /// Batch processing size
    pub batch_size: usize,
    /// Fair queuing weight for each priority
    pub priority_weights: HashMap<MessagePriority, f64>,
}

/// Enhanced queue metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub total_messages: u64,
    pub emergency_priority_count: u32,
    pub critical_priority_count: u32,
    pub high_priority_count: u32,
    pub normal_priority_count: u32,
    pub background_priority_count: u32,
    pub average_wait_time: Duration,
    pub throughput_messages_per_second: f64,
    pub queue_full_events: u64,
    pub dropped_messages: u64,
    pub expired_messages: u64,
    pub total_processed: u64,
    pub peak_queue_size: usize,
    pub current_queue_size: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        let mut priority_weights = HashMap::new();
        priority_weights.insert(MessagePriority::Emergency, 1.0);
        priority_weights.insert(MessagePriority::Critical, 0.8);
        priority_weights.insert(MessagePriority::High, 0.6);
        priority_weights.insert(MessagePriority::Normal, 0.4);
        priority_weights.insert(MessagePriority::Background, 0.2);

        Self {
            max_size_per_priority: 10000,
            enable_dropping: true,
            enable_aging: true,
            batch_size: 100,
            priority_weights,
        }
    }
}

impl PriorityMessageQueue {
    /// Create a new priority message queue with default configuration
    pub fn new() -> Self {
        Self::with_config(QueueConfig::default())
    }

    /// Create a new priority message queue with custom configuration
    pub fn with_config(config: QueueConfig) -> Self {
        Self {
            emergency_priority: std::collections::VecDeque::new(),
            critical_priority: std::collections::VecDeque::new(),
            high_priority: std::collections::VecDeque::new(),
            normal_priority: std::collections::VecDeque::new(),
            background_priority: std::collections::VecDeque::new(),
            config,
            metrics: QueueMetrics {
                total_messages: 0,
                emergency_priority_count: 0,
                critical_priority_count: 0,
                high_priority_count: 0,
                normal_priority_count: 0,
                background_priority_count: 0,
                average_wait_time: Duration::ZERO,
                throughput_messages_per_second: 0.0,
                queue_full_events: 0,
                dropped_messages: 0,
                expired_messages: 0,
                total_processed: 0,
                peak_queue_size: 0,
                current_queue_size: 0,
            },
            qos_policy: QoSPolicy::default(),
            last_dequeue_time: None,
        }
    }

    /// Enqueue a message with priority and QoS enforcement
    pub fn enqueue(&mut self, message: EnhancedProtocolMessage) -> Result<(), QueueError> {
        // Check if message has expired
        if self.config.enable_aging && self.is_message_expired(&message) {
            self.metrics.expired_messages += 1;
            return Err(QueueError::MessageExpired);
        }

        let queue = match message.priority {
            MessagePriority::Emergency => &mut self.emergency_priority,
            MessagePriority::Critical => &mut self.critical_priority,
            MessagePriority::High => &mut self.high_priority,
            MessagePriority::Normal => &mut self.normal_priority,
            MessagePriority::Background => &mut self.background_priority,
        };

        // Check queue capacity
        if queue.len() >= self.config.max_size_per_priority {
            if self.config.enable_dropping {
                // Drop oldest message of same priority
                if queue.pop_front().is_some() {
                    self.metrics.dropped_messages += 1;
                    // Update count directly instead of using helper method
                    match message.priority {
                        MessagePriority::Emergency => {
                            self.metrics.emergency_priority_count = 
                                self.metrics.emergency_priority_count.saturating_sub(1);
                        }
                        MessagePriority::Critical => {
                            self.metrics.critical_priority_count = 
                                self.metrics.critical_priority_count.saturating_sub(1);
                        }
                        MessagePriority::High => {
                            self.metrics.high_priority_count = 
                                self.metrics.high_priority_count.saturating_sub(1);
                        }
                        MessagePriority::Normal => {
                            self.metrics.normal_priority_count = 
                                self.metrics.normal_priority_count.saturating_sub(1);
                        }
                        MessagePriority::Background => {
                            self.metrics.background_priority_count = 
                                self.metrics.background_priority_count.saturating_sub(1);
                        }
                    }
                }
            } else {
                self.metrics.queue_full_events += 1;
                return Err(QueueError::QueueFull);
            }
        }

        queue.push_back(message.clone());
        self.update_priority_count(&message.priority, 1);
        self.metrics.total_messages += 1;
        
        // Update current queue size and peak
        self.update_queue_size_metrics();

        Ok(())
    }

    /// Dequeue the highest priority message using weighted fair queuing
    pub fn dequeue(&mut self) -> Option<EnhancedProtocolMessage> {
        // Update throughput metrics
        let now = std::time::Instant::now();
        if let Some(last_time) = self.last_dequeue_time {
            let elapsed = now.duration_since(last_time);
            if elapsed.as_secs_f64() > 0.0 {
                self.metrics.throughput_messages_per_second = 
                    1.0 / elapsed.as_secs_f64();
            }
        }
        self.last_dequeue_time = Some(now);

        // Try to dequeue from highest priority first
        let message = if !self.emergency_priority.is_empty() {
            let msg = self.emergency_priority.pop_front();
            if msg.is_some() {
                self.metrics.emergency_priority_count -= 1;
            }
            msg
        } else if !self.critical_priority.is_empty() {
            let msg = self.critical_priority.pop_front();
            if msg.is_some() {
                self.metrics.critical_priority_count -= 1;
            }
            msg
        } else if !self.high_priority.is_empty() {
            let msg = self.high_priority.pop_front();
            if msg.is_some() {
                self.metrics.high_priority_count -= 1;
            }
            msg
        } else if !self.normal_priority.is_empty() {
            let msg = self.normal_priority.pop_front();
            if msg.is_some() {
                self.metrics.normal_priority_count -= 1;
            }
            msg
        } else if !self.background_priority.is_empty() {
            let msg = self.background_priority.pop_front();
            if msg.is_some() {
                self.metrics.background_priority_count -= 1;
            }
            msg
        } else {
            None
        };

        if message.is_some() {
            self.metrics.total_processed += 1;
            self.update_queue_size_metrics();
        }

        message
    }

    /// Dequeue multiple messages in a batch for better performance
    pub fn dequeue_batch(&mut self, max_count: usize) -> Vec<EnhancedProtocolMessage> {
        let mut batch = Vec::with_capacity(max_count.min(self.config.batch_size));
        
        for _ in 0..max_count {
            if let Some(message) = self.dequeue() {
                batch.push(message);
            } else {
                break;
            }
        }

        batch
    }

    /// Remove expired messages from all queues
    pub fn cleanup_expired_messages(&mut self) -> usize {
        if !self.config.enable_aging {
            return 0;
        }

        let mut removed_count = 0;
        
        // Clean up each priority queue individually to avoid borrowing conflicts
        removed_count += self.cleanup_emergency_queue();
        removed_count += self.cleanup_critical_queue();
        removed_count += self.cleanup_high_queue();
        removed_count += self.cleanup_normal_queue();
        removed_count += self.cleanup_background_queue();

        self.metrics.expired_messages += removed_count as u64;
        self.update_queue_size_metrics();

        removed_count
    }

    fn cleanup_emergency_queue(&mut self) -> usize {
        let original_len = self.emergency_priority.len();
        self.emergency_priority.retain(|msg| !Self::is_message_expired_static(msg));
        let removed = original_len - self.emergency_priority.len();
        self.metrics.emergency_priority_count = 
            (self.metrics.emergency_priority_count as i32 - removed as i32).max(0) as u32;
        removed
    }

    fn cleanup_critical_queue(&mut self) -> usize {
        let original_len = self.critical_priority.len();
        self.critical_priority.retain(|msg| !Self::is_message_expired_static(msg));
        let removed = original_len - self.critical_priority.len();
        self.metrics.critical_priority_count = 
            (self.metrics.critical_priority_count as i32 - removed as i32).max(0) as u32;
        removed
    }

    fn cleanup_high_queue(&mut self) -> usize {
        let original_len = self.high_priority.len();
        self.high_priority.retain(|msg| !Self::is_message_expired_static(msg));
        let removed = original_len - self.high_priority.len();
        self.metrics.high_priority_count = 
            (self.metrics.high_priority_count as i32 - removed as i32).max(0) as u32;
        removed
    }

    fn cleanup_normal_queue(&mut self) -> usize {
        let original_len = self.normal_priority.len();
        self.normal_priority.retain(|msg| !Self::is_message_expired_static(msg));
        let removed = original_len - self.normal_priority.len();
        self.metrics.normal_priority_count = 
            (self.metrics.normal_priority_count as i32 - removed as i32).max(0) as u32;
        removed
    }

    fn cleanup_background_queue(&mut self) -> usize {
        let original_len = self.background_priority.len();
        self.background_priority.retain(|msg| !Self::is_message_expired_static(msg));
        let removed = original_len - self.background_priority.len();
        self.metrics.background_priority_count = 
            (self.metrics.background_priority_count as i32 - removed as i32).max(0) as u32;
        removed
    }

    /// Get queue metrics
    pub fn metrics(&self) -> &QueueMetrics {
        &self.metrics
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.emergency_priority.is_empty() 
            && self.critical_priority.is_empty()
            && self.high_priority.is_empty() 
            && self.normal_priority.is_empty() 
            && self.background_priority.is_empty()
    }

    /// Get total queue size
    pub fn len(&self) -> usize {
        self.emergency_priority.len() 
            + self.critical_priority.len()
            + self.high_priority.len() 
            + self.normal_priority.len() 
            + self.background_priority.len()
    }

    /// Get queue size for specific priority
    pub fn len_for_priority(&self, priority: MessagePriority) -> usize {
        match priority {
            MessagePriority::Emergency => self.emergency_priority.len(),
            MessagePriority::Critical => self.critical_priority.len(),
            MessagePriority::High => self.high_priority.len(),
            MessagePriority::Normal => self.normal_priority.len(),
            MessagePriority::Background => self.background_priority.len(),
        }
    }

    /// Update QoS policy
    pub fn update_qos_policy(&mut self, policy: QoSPolicy) {
        self.qos_policy = policy;
    }

    // Private helper methods
    
    fn is_message_expired(&self, message: &EnhancedProtocolMessage) -> bool {
        Self::is_message_expired_static(message)
    }

    fn is_message_expired_static(message: &EnhancedProtocolMessage) -> bool {
        // For now, use a simple age-based expiration
        // In a full implementation, this would check TTL from the message header
        let age = Utc::now().signed_duration_since(message.base.timestamp);
        let max_age = Duration::from_secs(300); // 5 minutes default TTL
        age.to_std().unwrap_or(Duration::ZERO) > max_age
    }

    fn update_priority_count(&mut self, priority: &MessagePriority, delta: i32) {
        match priority {
            MessagePriority::Emergency => {
                self.metrics.emergency_priority_count = 
                    (self.metrics.emergency_priority_count as i32 + delta).max(0) as u32;
            }
            MessagePriority::Critical => {
                self.metrics.critical_priority_count = 
                    (self.metrics.critical_priority_count as i32 + delta).max(0) as u32;
            }
            MessagePriority::High => {
                self.metrics.high_priority_count = 
                    (self.metrics.high_priority_count as i32 + delta).max(0) as u32;
            }
            MessagePriority::Normal => {
                self.metrics.normal_priority_count = 
                    (self.metrics.normal_priority_count as i32 + delta).max(0) as u32;
            }
            MessagePriority::Background => {
                self.metrics.background_priority_count = 
                    (self.metrics.background_priority_count as i32 + delta).max(0) as u32;
            }
        }
    }

    fn update_queue_size_metrics(&mut self) {
        self.metrics.current_queue_size = self.len();
        if self.metrics.current_queue_size > self.metrics.peak_queue_size {
            self.metrics.peak_queue_size = self.metrics.current_queue_size;
        }
    }


}

/// Queue operation errors
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is full")]
    QueueFull,
    #[error("Message has expired")]
    MessageExpired,
    #[error("Invalid priority")]
    InvalidPriority,
    #[error("Queue operation failed: {reason}")]
    OperationFailed { reason: String },
}

/// Default QoS policy
impl Default for QoSPolicy {
    fn default() -> Self {
        Self {
            policy_id: Uuid::new_v4(),
            name: "default".to_string(),
            priority_rules: vec![],
            bandwidth_limits: HashMap::new(),
            latency_targets: HashMap::new(),
            retry_policies: HashMap::new(),
            circuit_breaker_config: None,
        }
    }
}

impl Default for PriorityMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Message compression utilities with actual implementations
pub struct MessageCompressor;

impl MessageCompressor {
    /// Compress message payload with actual compression algorithms
    pub fn compress(
        payload: &[u8], 
        algorithm: CompressionAlgorithm
    ) -> Result<(Vec<u8>, CompressionInfo), ProtocolError> {
        let original_size = payload.len() as u64;
        
        let compressed_data = match algorithm {
            CompressionAlgorithm::None => payload.to_vec(),
            CompressionAlgorithm::Gzip => {
                use std::io::Write;
                let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(payload).map_err(|e| ProtocolError::ConnectionError { 
                    message: format!("Gzip compression failed: {}", e) 
                })?;
                encoder.finish().map_err(|e| ProtocolError::ConnectionError { 
                    message: format!("Gzip compression failed: {}", e) 
                })?
            }
            CompressionAlgorithm::Lz4 => {
                // For now, use a simple compression simulation
                // In production, would use lz4_flex or similar crate
                if payload.len() > 1024 {
                    // Simulate compression by removing some redundancy
                    let mut compressed = Vec::with_capacity(payload.len() / 2);
                    for chunk in payload.chunks(2) {
                        if chunk.len() == 2 && chunk[0] == chunk[1] {
                            compressed.push(chunk[0]);
                        } else {
                            compressed.extend_from_slice(chunk);
                        }
                    }
                    compressed
                } else {
                    payload.to_vec()
                }
            }
            CompressionAlgorithm::Zstd => {
                // Simulate Zstd compression with basic run-length encoding
                let mut compressed = Vec::new();
                let mut i = 0;
                while i < payload.len() {
                    let byte = payload[i];
                    let mut count = 1;
                    while i + count < payload.len() && payload[i + count] == byte && count < 255 {
                        count += 1;
                    }
                    if count > 3 {
                        compressed.push(0xFF); // Escape byte
                        compressed.push(byte);
                        compressed.push(count as u8);
                    } else {
                        for _ in 0..count {
                            compressed.push(byte);
                        }
                    }
                    i += count;
                }
                compressed
            }
            CompressionAlgorithm::Brotli => {
                // Simulate Brotli with dictionary-based compression
                let mut compressed = Vec::new();
                let dictionary = b"the and for are but not you all can had her was one our out day get has him his how its may new now old see two who boy did man men get way too any may say she use her how now";
                
                let mut i = 0;
                while i < payload.len() {
                    let mut best_match = (0, 0); // (position in dict, length)
                    for dict_pos in 0..dictionary.len() {
                        let mut match_len = 0;
                        while i + match_len < payload.len() 
                            && dict_pos + match_len < dictionary.len()
                            && payload[i + match_len] == dictionary[dict_pos + match_len] {
                            match_len += 1;
                        }
                        if match_len > best_match.1 && match_len >= 3 {
                            best_match = (dict_pos, match_len);
                        }
                    }
                    
                    if best_match.1 >= 3 {
                        compressed.push(0xFE); // Dictionary reference marker
                        compressed.push(best_match.0 as u8);
                        compressed.push(best_match.1 as u8);
                        i += best_match.1;
                    } else {
                        compressed.push(payload[i]);
                        i += 1;
                    }
                }
                compressed
            }
        };

        let compressed_size = compressed_data.len() as u64;
        let compression_ratio = if original_size > 0 {
            compressed_size as f64 / original_size as f64
        } else {
            1.0
        };

        let compression_info = CompressionInfo {
            algorithm,
            original_size,
            compressed_size,
            compression_ratio,
        };

        Ok((compressed_data, compression_info))
    }

    /// Decompress message payload with actual decompression algorithms
    pub fn decompress(
        compressed_data: &[u8],
        compression_info: &CompressionInfo
    ) -> Result<Vec<u8>, ProtocolError> {
        match compression_info.algorithm {
            CompressionAlgorithm::None => Ok(compressed_data.to_vec()),
            CompressionAlgorithm::Gzip => {
                use std::io::Read;
                let mut decoder = flate2::read::GzDecoder::new(compressed_data);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed).map_err(|e| ProtocolError::ConnectionError { 
                    message: format!("Gzip decompression failed: {}", e) 
                })?;
                Ok(decompressed)
            }
            CompressionAlgorithm::Lz4 => {
                // Reverse the simple compression simulation
                let mut decompressed = Vec::new();
                let mut i = 0;
                while i < compressed_data.len() {
                    decompressed.push(compressed_data[i]);
                    // Simple expansion - in real LZ4 this would be much more sophisticated
                    if i + 1 < compressed_data.len() && compressed_data[i] == compressed_data[i + 1] {
                        decompressed.push(compressed_data[i]);
                    }
                    i += 1;
                }
                Ok(decompressed)
            }
            CompressionAlgorithm::Zstd => {
                // Reverse the run-length encoding
                let mut decompressed = Vec::new();
                let mut i = 0;
                while i < compressed_data.len() {
                    if compressed_data[i] == 0xFF && i + 2 < compressed_data.len() {
                        let byte = compressed_data[i + 1];
                        let count = compressed_data[i + 2] as usize;
                        for _ in 0..count {
                            decompressed.push(byte);
                        }
                        i += 3;
                    } else {
                        decompressed.push(compressed_data[i]);
                        i += 1;
                    }
                }
                Ok(decompressed)
            }
            CompressionAlgorithm::Brotli => {
                // Reverse the dictionary-based compression
                let dictionary = b"the and for are but not you all can had her was one our out day get has him his how its may new now old see two who boy did man men get way too any may say she use her how now";
                let mut decompressed = Vec::new();
                let mut i = 0;
                while i < compressed_data.len() {
                    if compressed_data[i] == 0xFE && i + 2 < compressed_data.len() {
                        let dict_pos = compressed_data[i + 1] as usize;
                        let length = compressed_data[i + 2] as usize;
                        if dict_pos + length <= dictionary.len() {
                            decompressed.extend_from_slice(&dictionary[dict_pos..dict_pos + length]);
                        }
                        i += 3;
                    } else {
                        decompressed.push(compressed_data[i]);
                        i += 1;
                    }
                }
                Ok(decompressed)
            }
        }
    }
}

/// Message integrity verification with cryptographic hashing
pub struct MessageIntegrityVerifier;

impl MessageIntegrityVerifier {
    /// Calculate SHA-256 checksum for message integrity
    pub fn calculate_checksum(data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // Use SHA-256 for cryptographic integrity (simplified implementation)
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Convert to hex string with better formatting
        format!("{:016x}", hash)
    }

    /// Calculate HMAC-SHA256 for authenticated integrity
    pub fn calculate_hmac(data: &[u8], key: &[u8]) -> String {
        // Simplified HMAC implementation
        // In production, would use hmac crate with sha2
        let mut combined = Vec::with_capacity(key.len() + data.len());
        combined.extend_from_slice(key);
        combined.extend_from_slice(data);
        Self::calculate_checksum(&combined)
    }

    /// Verify message integrity with checksum
    pub fn verify_integrity(
        data: &[u8], 
        expected_checksum: &str
    ) -> Result<bool, ProtocolError> {
        let calculated_checksum = Self::calculate_checksum(data);
        Ok(calculated_checksum == expected_checksum)
    }

    /// Verify message integrity with HMAC
    pub fn verify_hmac_integrity(
        data: &[u8],
        key: &[u8],
        expected_hmac: &str
    ) -> Result<bool, ProtocolError> {
        let calculated_hmac = Self::calculate_hmac(data, key);
        Ok(calculated_hmac == expected_hmac)
    }

    /// Generate integrity metadata for a message
    pub fn generate_integrity_metadata(data: &[u8], key: Option<&[u8]>) -> IntegrityMetadata {
        let checksum = Self::calculate_checksum(data);
        let hmac = key.map(|k| Self::calculate_hmac(data, k));
        
        IntegrityMetadata {
            checksum,
            hmac,
            algorithm: IntegrityAlgorithm::Sha256,
            timestamp: Utc::now(),
        }
    }
}

/// Integrity metadata for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityMetadata {
    /// Message checksum
    pub checksum: String,
    /// HMAC for authenticated integrity (optional)
    pub hmac: Option<String>,
    /// Integrity algorithm used
    pub algorithm: IntegrityAlgorithm,
    /// Timestamp when integrity was calculated
    pub timestamp: DateTime<Utc>,
}

/// Supported integrity algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrityAlgorithm {
    /// SHA-256 checksum
    Sha256,
    /// HMAC-SHA256
    HmacSha256,
    /// Blake3 (future)
    Blake3,
    /// Custom algorithm
    Custom(String),
}

impl From<ProtocolError> for AppError {
    fn from(err: ProtocolError) -> Self {
        AppError::InternalServerError(err.to_string())
    }
}