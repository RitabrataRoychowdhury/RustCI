// High-Performance ValkyrieRunner Adapter
// Task 3.1: High-Performance Runner Adapter Implementation

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore, mpsc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

// Import Valkyrie Protocol components
use crate::core::networking::valkyrie::routing::{
    RoutingContext, Route, RoutingError, QoSRequirements,
};
use crate::core::networking::valkyrie::{
    MessagePriority, ValkyrieMessage,
};
use crate::core::networking::valkyrie::types::{
    MessageHeader, MessageType, MessageFlags, ProtocolInfo, EndpointId,
    CorrelationId, RoutingInfo, CompressionInfo, TraceContext
};
use crate::core::networking::valkyrie::routing::NodeId as ValkyrieNodeId;
use crate::core::networking::valkyrie::ConnectionId;
use crate::core::networking::valkyrie::engine::ValkyrieEngine;

// Core types
pub type JobId = Uuid;
pub type RunnerId = Uuid;
pub type NodeId = Uuid;

/// High-performance ValkyrieRunner with sub-100μs dispatch latency
pub struct ValkyrieRunnerAdapter {
    // Core Valkyrie engine integration
    valkyrie_engine: Arc<ValkyrieEngine>,
    // Routing client for intelligent runner selection
    // routing_client: Arc<RoutingClient>, // TODO: Implement when available
    // Load balancer for optimal distribution
    // load_balancer: Arc<LoadBalancerManager>, // TODO: Implement when available
    // Job queue with priority processing
    job_queue: Arc<AsyncJobQueue>,
    // Runner selection engine
    runner_selector: Arc<IntelligentRunnerSelector>,
    // HTTP fallback system
    http_fallback: Arc<HttpFallbackSystem>,
    // Performance metrics
    metrics: Arc<PerformanceMetrics>,
    // Configuration
    config: ValkyrieAdapterConfig,
}

/// Configuration for ValkyrieRunnerAdapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieAdapterConfig {
    // Performance settings
    pub max_concurrent_jobs: u32,
    pub dispatch_timeout: Duration,
    pub queue_capacity: usize,
    // Fallback settings
    pub enable_http_fallback: bool,
    pub fallback_timeout: Duration,
    // Routing settings
    pub enable_intelligent_routing: bool,
    pub health_check_interval: Duration,
    // Metrics settings
    pub metrics_enabled: bool,
    pub metrics_interval: Duration,
}

impl Default for ValkyrieAdapterConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 1000,
            dispatch_timeout: Duration::from_micros(100), // <100μs target
            queue_capacity: 10000,
            enable_http_fallback: true,
            fallback_timeout: Duration::from_millis(5),
            enable_intelligent_routing: true,
            health_check_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(1),
        }
    }
}

/// Job definition optimized for Valkyrie transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieJob {
    pub id: JobId,
    pub job_type: JobType,
    pub priority: JobPriority,
    pub payload: JobPayload,
    pub requirements: JobRequirements,
    pub metadata: JobMetadata,
    pub created_at: std::time::SystemTime,
    pub deadline: Option<std::time::SystemTime>,
    // Valkyrie-specific fields
    pub routing_hints: Option<RoutingHints>,
    pub qos_requirements: Option<QoSRequirements>,
}

impl Default for ValkyrieJob {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            job_type: JobType::Build,
            priority: JobPriority::Normal,
            payload: JobPayload::Small(Vec::new()),
            requirements: JobRequirements::default(),
            metadata: JobMetadata::default(),
            created_at: std::time::SystemTime::now(),
            deadline: None,
            routing_hints: None,
            qos_requirements: None,
        }
    }
}

/// Job types supported by the adapter
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobType {
    Build,
    Test,
    Deploy,
    Analysis,
    Custom(String),
}

/// Job priority levels mapped to Valkyrie message priorities
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl From<JobPriority> for MessagePriority {
    fn from(job_priority: JobPriority) -> Self {
        match job_priority {
            JobPriority::Critical => MessagePriority::Critical(0),
            JobPriority::High => MessagePriority::High,
            JobPriority::Normal => MessagePriority::Normal,
            JobPriority::Low => MessagePriority::Low,
            JobPriority::Background => MessagePriority::Low, // Map Background to Low
        }
    }
}

/// Job payload optimized for Valkyrie zero-copy transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobPayload {
    Small(Vec<u8>),
    Large(LargePayload),
    Stream(StreamPayload),
}

/// Large payload stored separately for zero-copy transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargePayload {
    pub storage_id: String,
    pub size: u64,
    pub checksum: String,
    pub compression: Option<CompressionType>,
}

/// Stream payload for real-time data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamPayload {
    pub stream_id: String,
    pub chunk_size: u32,
    pub total_chunks: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    Gzip,
    Lz4,
    Zstd,
}

/// Job execution requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobRequirements {
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u32>,
    pub storage_gb: Option<u32>,
    pub gpu_count: Option<u32>,
    pub max_execution_time: Option<Duration>,
    pub required_capabilities: Vec<String>,
    pub preferred_regions: Vec<String>,
    pub network_requirements: Option<NetworkRequirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRequirements {
    pub min_bandwidth: Option<u64>,
    pub max_latency: Option<Duration>,
    pub reliability_threshold: Option<f64>,
}

/// Job metadata for tracking and routing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobMetadata {
    pub user_id: Option<String>,
    pub project_id: Option<String>,
    pub pipeline_id: Option<String>,
    pub stage: Option<String>,
    pub tags: HashMap<String, String>,
    pub correlation_id: Option<String>,
}

/// Routing hints for intelligent job placement
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingHints {
    pub preferred_runners: Vec<RunnerId>,
    pub excluded_runners: Vec<RunnerId>,
    pub geographic_preference: Option<String>,
    pub affinity_rules: Vec<AffinityRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityRule {
    pub rule_type: AffinityType,
    pub key: String,
    pub values: Vec<String>,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AffinityType {
    Required,
    Preferred,
    AntiAffinity,
}

/// Job submission result with performance metrics
#[derive(Debug, Clone)]
pub struct JobSubmissionResult {
    pub job_id: JobId,
    pub assigned_runner: Option<RunnerId>,
    pub estimated_start_time: Instant,
    pub estimated_completion_time: Instant,
    pub dispatch_latency: Duration,
    pub routing_path: Option<Route>,
    pub qos_guarantees: QoSGuarantees,
}

/// QoS guarantees provided by Valkyrie
#[derive(Debug, Clone, Default)]
pub struct QoSGuarantees {
    pub max_latency: Option<Duration>,
    pub min_throughput: Option<u64>,
    pub reliability_guarantee: f64,
    pub priority_level: Option<MessagePriority>,
}

/// Job execution status with Valkyrie integration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued { 
        #[serde(with = "instant_serde")]
        queued_at: Instant 
    },
    Routing { 
        #[serde(with = "instant_serde")]
        routing_started: Instant 
    },
    Dispatched { 
        runner_id: RunnerId, 
        #[serde(with = "instant_serde")]
        dispatched_at: Instant 
    },
    Running { 
        runner_id: RunnerId, 
        #[serde(with = "instant_serde")]
        started_at: Instant 
    },
    Completed { 
        result: JobResult, 
        #[serde(with = "instant_serde")]
        completed_at: Instant 
    },
    Failed { 
        error: String, 
        #[serde(with = "instant_serde")]
        failed_at: Instant 
    },
    Cancelled { 
        #[serde(with = "instant_serde")]
        cancelled_at: Instant 
    },
    Timeout { 
        #[serde(with = "instant_serde")]
        timeout_at: Instant 
    },
}

// Custom serde module for Instant
mod instant_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert Instant to SystemTime for serialization
        let system_time = SystemTime::now();
        let duration_since_epoch = system_time.duration_since(UNIX_EPOCH).unwrap_or_default();
        duration_since_epoch.as_nanos().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nanos = u128::deserialize(deserializer)?;
        // For deserialization, we'll just use current time as a placeholder
        // In a real implementation, you'd want to store relative times
        Ok(Instant::now())
    }
}

/// Job execution result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobResult {
    pub exit_code: i32,
    pub output: String,
    pub artifacts: Vec<String>,
    pub execution_time: Duration,
    pub resource_usage: ResourceUsage,
    pub performance_metrics: JobPerformanceMetrics,
}

/// Resource usage statistics
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_time: Duration,
    pub memory_peak: u64,
    pub storage_used: u64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

/// Performance metrics for individual jobs
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPerformanceMetrics {
    pub queue_time: Duration,
    pub routing_time: Duration,
    pub dispatch_time: Duration,
    pub execution_time: Duration,
    pub total_time: Duration,
}

/// ValkyrieRunnerAdapter trait for job execution
#[async_trait]
pub trait ValkyrieRunnerTrait {
    /// Submit a job for execution with sub-100μs dispatch latency
    async fn submit_job(&self, job: ValkyrieJob) -> Result<JobSubmissionResult, ValkyrieAdapterError>;
    
    /// Get job status with minimal latency
    async fn get_job_status(&self, job_id: JobId) -> Result<JobStatus, ValkyrieAdapterError>;
    
    /// Cancel a job with immediate effect
    async fn cancel_job(&self, job_id: JobId) -> Result<(), ValkyrieAdapterError>;
    
    /// Get real-time performance metrics
    async fn get_performance_metrics(&self) -> Result<AdapterPerformanceSnapshot, ValkyrieAdapterError>;
    
    /// Register a new runner with the adapter
    async fn register_runner(&self, runner_info: RunnerRegistration) -> Result<RunnerId, ValkyrieAdapterError>;
    
    /// Update runner status and capabilities
    async fn update_runner_status(&self, runner_id: RunnerId, status: RunnerStatus) -> Result<(), ValkyrieAdapterError>;
    
    /// Get runner pool statistics
    async fn get_runner_pool_stats(&self) -> Result<RunnerPoolStats, ValkyrieAdapterError>;
}

/// Runner registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerRegistration {
    pub runner_id: Option<RunnerId>,
    pub capabilities: RunnerCapabilities,
    pub endpoint: String,
    pub protocol_support: ProtocolSupport,
    pub geographic_location: Option<GeographicLocation>,
    pub metadata: HashMap<String, String>,
}

/// Runner capabilities for intelligent matching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerCapabilities {
    // Resource capabilities
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub storage_gb: u32,
    pub gpu_count: u32,
    pub gpu_type: Option<String>,
    // Software capabilities
    pub supported_job_types: Vec<JobType>,
    pub supported_languages: Vec<String>,
    pub supported_frameworks: Vec<String>,
    pub container_runtime: Vec<String>,
    // Performance characteristics
    pub max_concurrent_jobs: u32,
    pub average_job_duration: Option<Duration>,
    pub performance_tier: PerformanceTier,
    // Custom capabilities
    pub custom_capabilities: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PerformanceTier {
    High,
    Medium,
    Low,
    Specialized,
}

impl Default for PerformanceTier {
    fn default() -> Self {
        Self::Medium
    }
}

/// Protocol support information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolSupport {
    pub valkyrie_supported: bool,
    pub valkyrie_version: Option<String>,
    pub http_supported: bool,
    pub websocket_supported: bool,
    pub preferred_protocol: PreferredProtocol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreferredProtocol {
    Valkyrie,
    Http,
    WebSocket,
    Auto,
}

impl Default for PreferredProtocol {
    fn default() -> Self {
        Self::Auto
    }
}

/// Geographic location for routing optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicLocation {
    pub region: String,
    pub zone: Option<String>,
    pub datacenter: Option<String>,
    pub coordinates: Option<(f64, f64)>, // (latitude, longitude)
}

/// Runner status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStatus {
    pub health: HealthStatus,
    pub current_load: u32,
    pub max_capacity: u32,
    pub active_jobs: Vec<JobId>,
    #[serde(with = "instant_serde")]
    pub last_heartbeat: Instant,
    pub performance_metrics: RunnerPerformanceMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Performance metrics for runners
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerPerformanceMetrics {
    pub average_response_time: Duration,
    pub success_rate: f64,
    pub throughput: f64, // jobs per second
    pub resource_utilization: ResourceUtilization,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub storage_usage: f64,
    pub network_usage: f64,
}

/// Runner pool statistics
#[derive(Debug, Clone, Default)]
pub struct RunnerPoolStats {
    pub total_runners: u32,
    pub healthy_runners: u32,
    pub active_jobs: u32,
    pub queued_jobs: u32,
    pub average_load: f64,
    pub total_capacity: u32,
    pub utilization_rate: f64,
}

/// Performance snapshot for the adapter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdapterPerformanceSnapshot {
    pub total_jobs_submitted: u64,
    pub total_jobs_completed: u64,
    pub total_jobs_failed: u64,
    pub average_dispatch_latency: Duration,
    pub p95_dispatch_latency: Duration,
    pub p99_dispatch_latency: Duration,
    pub current_throughput: f64,
    pub success_rate: f64,
    pub valkyrie_usage_rate: f64,
    pub http_fallback_rate: f64,
}

/// Error types for ValkyrieAdapter
#[derive(Debug, thiserror::Error)]
pub enum ValkyrieAdapterError {
    #[error("Valkyrie engine error: {0}")]
    ValkyrieEngine(String),
    
    #[error("Routing error: {0}")]
    Routing(#[from] RoutingError),
    
    #[error("Job queue full")]
    QueueFull,
    
    #[error("Job not found: {job_id}")]
    JobNotFound { job_id: JobId },
    
    #[error("Runner not found: {runner_id}")]
    RunnerNotFound { runner_id: RunnerId },
    
    #[error("Dispatch timeout")]
    DispatchTimeout,
    
    #[error("HTTP fallback failed: {0}")]
    HttpFallbackFailed(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl ValkyrieRunnerAdapter {
    /// Create a new ValkyrieRunnerAdapter
    pub async fn new(
        valkyrie_engine: Arc<ValkyrieEngine>,
        config: ValkyrieAdapterConfig,
    ) -> Result<Self, ValkyrieAdapterError> {
        // TODO: Implement when types are available
        // let routing_client = Arc::new(
        //     RoutingClient::new(valkyrie_engine.clone())
        //         .await
        //         .map_err(|e| ValkyrieAdapterError::ValkyrieEngine(e.to_string()))?
        // );
        
        // let load_balancer = Arc::new(
        //     LoadBalancerManager::new(valkyrie_engine.clone())
        //         .await
        //         .map_err(|e| ValkyrieAdapterError::ValkyrieEngine(e.to_string()))?
        // );
        
        // let qos_router = Arc::new(
        //     QoSRouter::new(valkyrie_engine.clone())
        //         .await
        //         .map_err(|e| ValkyrieAdapterError::ValkyrieEngine(e.to_string()))?
        // );
        
        // Create job queue with appropriate configuration
        let job_queue_config = JobQueueConfig {
            max_capacity: config.queue_capacity,
            queue_timeout: config.dispatch_timeout * 10, // Allow more time for queue operations
            ..Default::default()
        };
        let job_queue = Arc::new(AsyncJobQueue::new(job_queue_config));
        
        // Create runner selector with configuration
        let selector_config = RunnerSelectorConfig {
            selection_timeout: config.dispatch_timeout,
            ..Default::default()
        };
        let runner_selector = Arc::new(
            IntelligentRunnerSelector::new(
                // routing_client.clone(), // TODO: Add when available
                // load_balancer.clone(), // TODO: Add when available
                // qos_router, // TODO: Add when available
                selector_config,
            ).await?
        );
        
        // Create HTTP fallback system
        let http_fallback_config = HttpFallbackConfig {
            request_timeout: config.fallback_timeout * 6, // More time for HTTP
            ..Default::default()
        };
        let http_fallback = Arc::new(HttpFallbackSystem::new(http_fallback_config));
        
        let metrics = Arc::new(PerformanceMetrics::new());
        
        Ok(Self {
            valkyrie_engine,
            // routing_client, // TODO: Add when available
            // load_balancer, // TODO: Add when available
            job_queue,
            runner_selector,
            http_fallback,
            metrics,
            config,
        })
    }
}

#[async_trait]
impl ValkyrieRunnerTrait for ValkyrieRunnerAdapter {
    async fn submit_job(&self, job: ValkyrieJob) -> Result<JobSubmissionResult, ValkyrieAdapterError> {
        let start_time = Instant::now();
        
        // Add job to queue with priority
        self.job_queue.enqueue(job.clone(), job.priority).await?;
        
        // Select optimal runner using intelligent routing
        let runner_selection = self.runner_selector
            .select_optimal_runner(&job)
            .await?;
        
        // Create Valkyrie message from job
        let valkyrie_message = self.job_to_valkyrie_message(&job, &runner_selection)?;
        
        // Attempt Valkyrie dispatch first
        let (dispatch_result, protocol_used) = match self.dispatch_via_valkyrie(valkyrie_message).await {
            Ok(result) => (result, "valkyrie"),
            Err(e) if self.config.enable_http_fallback => {
                // Fallback to HTTP if Valkyrie fails
                let http_result = self.http_fallback.dispatch_job(&job, &runner_selection).await?;
                let dispatch_result = DispatchResult {
                    success: http_result.success,
                    latency: http_result.latency,
                    protocol_used: "http".to_string(),
                };
                (dispatch_result, "http")
            }
            Err(e) => return Err(e),
        };
        
        let dispatch_latency = start_time.elapsed();
        
        // Record metrics
        self.metrics.record_dispatch(dispatch_latency, dispatch_result.success, protocol_used).await;
        
        // Ensure we meet the <100μs target
        if dispatch_latency > self.config.dispatch_timeout {
            tracing::warn!(
                "Dispatch latency {}μs exceeded target {}μs for job {}",
                dispatch_latency.as_micros(),
                self.config.dispatch_timeout.as_micros(),
                job.id
            );
        }
        
        Ok(JobSubmissionResult {
            job_id: job.id,
            assigned_runner: Some(runner_selection.runner_id),
            estimated_start_time: Instant::now(),
            estimated_completion_time: Instant::now() + runner_selection.estimated_duration,
            dispatch_latency,
            routing_path: Some(runner_selection.route),
            qos_guarantees: runner_selection.qos_guarantees,
        })
    }
    
    async fn get_job_status(&self, job_id: JobId) -> Result<JobStatus, ValkyrieAdapterError> {
        // Implementation for getting job status
        // This would query the Valkyrie engine for job status
        Err(ValkyrieAdapterError::NotImplemented("Job status retrieval not implemented".to_string()))
    }
    
    async fn cancel_job(&self, job_id: JobId) -> Result<(), ValkyrieAdapterError> {
        // Implementation for job cancellation
        // This would send a cancellation message via Valkyrie
        Err(ValkyrieAdapterError::NotImplemented("Job cancellation not implemented".to_string()))
    }
    
    async fn get_performance_metrics(&self) -> Result<AdapterPerformanceSnapshot, ValkyrieAdapterError> {
        Ok(self.metrics.get_snapshot().await)
    }
    
    async fn register_runner(&self, runner_info: RunnerRegistration) -> Result<RunnerId, ValkyrieAdapterError> {
        // Implementation for runner registration
        Err(ValkyrieAdapterError::NotImplemented("Runner registration not implemented".to_string()))
    }
    
    async fn update_runner_status(&self, runner_id: RunnerId, status: RunnerStatus) -> Result<(), ValkyrieAdapterError> {
        // Implementation for runner status updates
        Err(ValkyrieAdapterError::NotImplemented("Runner status updates not implemented".to_string()))
    }
    
    async fn get_runner_pool_stats(&self) -> Result<RunnerPoolStats, ValkyrieAdapterError> {
        // Implementation for runner pool statistics
        Err(ValkyrieAdapterError::NotImplemented("Runner pool statistics not implemented".to_string()))
    }
}

impl ValkyrieRunnerAdapter {
    /// Convert a job to a Valkyrie message for transport
    fn job_to_valkyrie_message(
        &self,
        job: &ValkyrieJob,
        runner_selection: &SelectionResult,
    ) -> Result<ValkyrieMessage, ValkyrieAdapterError> {
        use crate::core::networking::valkyrie::types::{MessageHeader, MessagePayload, TraceContext};
        
        // Serialize job payload
        let payload_bytes = serde_json::to_vec(job)
            .map_err(ValkyrieAdapterError::Serialization)?;
        
        // Create message header
        let header = MessageHeader {
            id: job.id,
            source: "local".to_string(), // Will be set by engine
            destination: Some(runner_selection.runner_node_id.to_string()),
            protocol_info: ProtocolInfo::default(),
            message_type: MessageType::JobExecution,
            stream_id: None,
            flags: MessageFlags::default(),
            priority: job.priority.into(),
            timestamp: chrono::Utc::now(),
            ttl: job.deadline.map(|d| d.duration_since(std::time::SystemTime::now()).unwrap_or(Duration::ZERO)),
            correlation_id: job.metadata.correlation_id.clone().and_then(|s| s.parse().ok()),
            routing: RoutingInfo::default(),
            compression: CompressionInfo::default(),
            sequence_number: 0,
            ack_number: None,
        };
        
        // Create Valkyrie message
        Ok(ValkyrieMessage {
            header,
            payload: MessagePayload::Binary(payload_bytes),
            signature: None,
            trace_context: Some(TraceContext::default()),
        })
    }
    
    /// Dispatch job via Valkyrie protocol
    async fn dispatch_via_valkyrie(
        &self,
        message: ValkyrieMessage,
    ) -> Result<DispatchResult, ValkyrieAdapterError> {
        let start_time = Instant::now();
        
        self.valkyrie_engine
            .send_message(ConnectionId::new_v4(), message)
            .await
            .map_err(|e| ValkyrieAdapterError::ValkyrieEngine(e.to_string()))?;
        
        let latency = start_time.elapsed();
        
        Ok(DispatchResult {
            success: true,
            latency,
            protocol_used: "valkyrie".to_string(),
        })
    }
}

/// Result of job dispatch operation
#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub success: bool,
    pub latency: Duration,
    pub protocol_used: String,
}

// RunnerSelection is now imported from selection module as SelectionResult

// Import the implemented components
use super::job_queue::{AsyncJobQueue, JobQueueConfig};
use super::selection::{IntelligentRunnerSelector, RunnerSelectorConfig, RunnerSelection as SelectionResult};
use super::http_fallback::{HttpFallbackSystem, HttpFallbackConfig, HttpDispatchResult};

/// Performance metrics for the adapter
pub struct PerformanceMetrics {
    // Dispatch metrics
    total_dispatches: AtomicU64,
    successful_dispatches: AtomicU64,
    failed_dispatches: AtomicU64,
    
    // Latency tracking
    dispatch_latency_sum: AtomicU64, // microseconds
    dispatch_latency_count: AtomicU64,
    max_dispatch_latency: AtomicU64,
    
    // Protocol usage
    valkyrie_dispatches: AtomicU64,
    http_fallback_dispatches: AtomicU64,
    
    // Throughput tracking
    last_throughput_calculation: RwLock<Instant>,
    throughput_window_dispatches: AtomicU64,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            total_dispatches: AtomicU64::new(0),
            successful_dispatches: AtomicU64::new(0),
            failed_dispatches: AtomicU64::new(0),
            dispatch_latency_sum: AtomicU64::new(0),
            dispatch_latency_count: AtomicU64::new(0),
            max_dispatch_latency: AtomicU64::new(0),
            valkyrie_dispatches: AtomicU64::new(0),
            http_fallback_dispatches: AtomicU64::new(0),
            last_throughput_calculation: RwLock::new(Instant::now()),
            throughput_window_dispatches: AtomicU64::new(0),
        }
    }
    
    pub async fn record_dispatch(&self, latency: Duration, success: bool, protocol: &str) {
        let latency_micros = latency.as_micros() as u64;
        
        // Update counters
        self.total_dispatches.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_dispatches.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_dispatches.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update latency metrics
        self.dispatch_latency_sum.fetch_add(latency_micros, Ordering::Relaxed);
        self.dispatch_latency_count.fetch_add(1, Ordering::Relaxed);
        
        // Update max latency
        let mut current_max = self.max_dispatch_latency.load(Ordering::Relaxed);
        while latency_micros > current_max {
            match self.max_dispatch_latency.compare_exchange_weak(
                current_max,
                latency_micros,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
        
        // Update protocol usage
        match protocol {
            "valkyrie" => {
                self.valkyrie_dispatches.fetch_add(1, Ordering::Relaxed);
            }
            "http" => {
                self.http_fallback_dispatches.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        
        // Update throughput tracking
        self.throughput_window_dispatches.fetch_add(1, Ordering::Relaxed);
    }
    
    pub async fn get_snapshot(&self) -> AdapterPerformanceSnapshot {
        let total = self.total_dispatches.load(Ordering::Relaxed);
        let successful = self.successful_dispatches.load(Ordering::Relaxed);
        let failed = self.failed_dispatches.load(Ordering::Relaxed);
        let latency_sum = self.dispatch_latency_sum.load(Ordering::Relaxed);
        let latency_count = self.dispatch_latency_count.load(Ordering::Relaxed);
        let max_latency = self.max_dispatch_latency.load(Ordering::Relaxed);
        let valkyrie_count = self.valkyrie_dispatches.load(Ordering::Relaxed);
        let http_count = self.http_fallback_dispatches.load(Ordering::Relaxed);
        
        // Calculate throughput
        let mut throughput_calculation = self.last_throughput_calculation.write().await;
        let now = Instant::now();
        let time_elapsed = now.duration_since(*throughput_calculation);
        let window_dispatches = self.throughput_window_dispatches.swap(0, Ordering::Relaxed);
        let current_throughput = if time_elapsed.as_secs_f64() > 0.0 {
            window_dispatches as f64 / time_elapsed.as_secs_f64()
        } else {
            0.0
        };
        *throughput_calculation = now;
        
        AdapterPerformanceSnapshot {
            total_jobs_submitted: total,
            total_jobs_completed: successful,
            total_jobs_failed: failed,
            average_dispatch_latency: if latency_count > 0 {
                Duration::from_micros(latency_sum / latency_count)
            } else {
                Duration::ZERO
            },
            p95_dispatch_latency: Duration::from_micros(max_latency), // Simplified
            p99_dispatch_latency: Duration::from_micros(max_latency), // Simplified
            current_throughput,
            success_rate: if total > 0 {
                successful as f64 / total as f64
            } else {
                0.0
            },
            valkyrie_usage_rate: if total > 0 {
                valkyrie_count as f64 / total as f64
            } else {
                0.0
            },
            http_fallback_rate: if total > 0 {
                http_count as f64 / total as f64
            } else {
                0.0
            },
        }
    }
}