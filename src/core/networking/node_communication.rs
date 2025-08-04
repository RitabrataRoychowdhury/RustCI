use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::AppError;

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

impl From<ProtocolError> for AppError {
    fn from(err: ProtocolError) -> Self {
        AppError::InternalServerError(err.to_string())
    }
}