//! Control Plane API Handlers
//!
//! This module implements REST API handlers for control plane orchestration,
//! job scheduling, and node management operations.

use axum::{
    extract::{Path, Query, State},
    response::Json,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    core::{
        cluster::control_plane::ControlPlaneState,
        networking::security::{AuditAction, AuditEvent, Permission, SecurityContext},
    },
    domain::entities::{
        runner::{
            BackoffStrategy, Job, JobId, JobPriority, JobRequirements, JobResult, JobStatus,
            ResourceRequirements as DomainResourceRequirements, RetryPolicy,
        },
        NodeId, NodeStatus as DomainNodeStatus,
    },
    error::Result,
    AppState,
};

/// Request payload for scheduling a job
#[derive(Debug, Deserialize)]
pub struct ScheduleJobRequest {
    /// Job name/identifier
    pub name: String,
    /// Job type (e.g., "pipeline", "build", "deploy")
    pub job_type: String,
    /// Job payload/configuration
    pub payload: serde_json::Value,
    /// Job priority
    #[serde(default)]
    pub priority: JobPriority,
    /// Resource requirements
    pub resource_requirements: Option<ResourceRequirements>,
    /// Node affinity rules
    pub node_affinity: Option<NodeAffinity>,
    /// Timeout in seconds
    pub timeout_seconds: Option<u64>,
    /// Retry configuration
    pub retry_config: Option<RetryConfiguration>,
    /// Job metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Resource requirements for job scheduling
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// CPU cores required
    pub cpu_cores: Option<f64>,
    /// Memory in MB required
    pub memory_mb: Option<u64>,
    /// Disk space in MB required
    pub disk_mb: Option<u64>,
    /// GPU units required
    pub gpu_units: Option<u32>,
    /// Custom resource requirements
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

/// Node affinity rules for job placement
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeAffinity {
    /// Required node labels
    #[serde(default)]
    pub required_labels: HashMap<String, String>,
    /// Preferred node labels (soft constraints)
    #[serde(default)]
    pub preferred_labels: HashMap<String, String>,
    /// Anti-affinity rules (avoid these nodes)
    #[serde(default)]
    pub anti_affinity: Vec<String>,
}

/// Retry configuration for job execution
#[derive(Debug, Serialize, Deserialize)]
pub struct RetryConfiguration {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Initial retry delay in seconds
    pub initial_delay_seconds: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Maximum delay between retries
    pub max_delay_seconds: u64,
}

/// Response for job scheduling
#[derive(Debug, Serialize)]
pub struct ScheduleJobResponse {
    /// Assigned job ID
    pub job_id: JobId,
    /// Scheduled node ID (if immediately assigned)
    pub scheduled_node_id: Option<NodeId>,
    /// Job status
    pub status: JobStatus,
    /// Estimated start time
    pub estimated_start_time: Option<DateTime<Utc>>,
    /// Queue position (if queued)
    pub queue_position: Option<u32>,
    /// Scheduling metadata
    pub metadata: HashMap<String, String>,
}

/// Request payload for node registration
#[derive(Debug, Deserialize)]
pub struct RegisterNodeRequest {
    /// Node name
    pub name: String,
    /// Node type (e.g., "native", "docker", "kubernetes")
    pub node_type: String,
    /// Node capabilities
    pub capabilities: NodeCapabilities,
    /// Node metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// Authentication token for node communication
    pub auth_token: Option<String>,
}

/// Node capabilities for registration
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Available CPU cores
    pub cpu_cores: f64,
    /// Available memory in MB
    pub memory_mb: u64,
    /// Available disk space in MB
    pub disk_mb: u64,
    /// Available GPU units
    pub gpu_units: u32,
    /// Supported job types
    pub supported_job_types: Vec<String>,
    /// Node labels/tags
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Custom capabilities
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

/// Response for node registration
#[derive(Debug, Serialize)]
pub struct RegisterNodeResponse {
    /// Assigned node ID
    pub node_id: NodeId,
    /// Registration status
    pub status: String,
    /// Node configuration
    pub config: NodeConfiguration,
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
}

/// Node configuration provided after registration
#[derive(Debug, Serialize)]
pub struct NodeConfiguration {
    /// Control plane endpoints
    pub control_plane_endpoints: Vec<String>,
    /// Job polling interval in seconds
    pub job_polling_interval: u64,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,
    /// Node-specific settings
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

/// Query parameters for node status
#[derive(Debug, Deserialize)]
pub struct NodeStatusQuery {
    /// Filter by node type
    pub node_type: Option<String>,
    /// Filter by status
    pub status: Option<String>,
    /// Include detailed metrics
    #[serde(default)]
    pub include_metrics: bool,
    /// Include recent heartbeats
    #[serde(default)]
    pub include_heartbeats: bool,
}

/// Response for node status
#[derive(Debug, Serialize)]
pub struct NodeStatusResponse {
    /// Total number of nodes
    pub total_nodes: u32,
    /// Nodes by status
    pub nodes: Vec<NodeStatusInfo>,
    /// Aggregate statistics
    pub statistics: NodeStatistics,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Individual node status information
#[derive(Debug, Serialize)]
pub struct NodeStatusInfo {
    /// Node ID
    pub node_id: NodeId,
    /// Node name
    pub name: String,
    /// Node type
    pub node_type: String,
    /// Current status
    pub status: DomainNodeStatus,
    /// Node capabilities
    pub capabilities: NodeCapabilities,
    /// Current resource usage
    pub resource_usage: Option<ResourceUsage>,
    /// Last heartbeat time
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// Running jobs count
    pub running_jobs: u32,
    /// Node uptime in seconds
    pub uptime_seconds: u64,
    /// Node metadata
    pub metadata: HashMap<String, String>,
}

/// Resource usage information
#[derive(Debug, Serialize)]
pub struct ResourceUsage {
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Memory usage in MB
    pub memory_usage_mb: u64,
    /// Disk usage in MB
    pub disk_usage_mb: u64,
    /// GPU usage percentage
    pub gpu_usage_percent: f64,
    /// Network I/O statistics
    pub network_io: Option<NetworkIOStats>,
}

/// Network I/O statistics
#[derive(Debug, Serialize)]
pub struct NetworkIOStats {
    /// Bytes received
    pub bytes_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Packets sent
    pub packets_sent: u64,
}

/// Node statistics
#[derive(Debug, Serialize)]
pub struct NodeStatistics {
    /// Active nodes
    pub active_nodes: u32,
    /// Failed nodes
    pub failed_nodes: u32,
    /// Total CPU cores
    pub total_cpu_cores: f64,
    /// Total memory in MB
    pub total_memory_mb: u64,
    /// Average CPU usage
    pub avg_cpu_usage: f64,
    /// Average memory usage
    pub avg_memory_usage: f64,
}

/// Query parameters for job status
#[derive(Debug, Deserialize)]
pub struct JobStatusQuery {
    /// Filter by job status
    pub status: Option<JobStatus>,
    /// Filter by job type
    pub job_type: Option<String>,
    /// Filter by node ID
    pub node_id: Option<NodeId>,
    /// Include execution history
    #[serde(default)]
    pub include_history: bool,
    /// Include job logs
    #[serde(default)]
    pub include_logs: bool,
    /// Limit number of results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Response for job status
#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    /// Job ID
    pub job_id: JobId,
    /// Job name
    pub name: String,
    /// Job type
    pub job_type: String,
    /// Current status
    pub status: JobStatus,
    /// Assigned node ID
    pub assigned_node_id: Option<NodeId>,
    /// Job priority
    pub priority: JobPriority,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Started timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Completed timestamp
    pub completed_at: Option<DateTime<Utc>>,
    /// Execution duration in seconds
    pub duration_seconds: Option<u64>,
    /// Job result
    pub result: Option<JobResult>,
    /// Execution history
    pub execution_history: Option<Vec<JobExecutionRecord>>,
    /// Job logs
    pub logs: Option<Vec<JobLogEntry>>,
    /// Job metadata
    pub metadata: HashMap<String, String>,
}

/// Job execution record for history
#[derive(Debug, Serialize)]
pub struct JobExecutionRecord {
    /// Execution attempt number
    pub attempt: u32,
    /// Node ID where executed
    pub node_id: NodeId,
    /// Execution start time
    pub started_at: DateTime<Utc>,
    /// Execution end time
    pub ended_at: Option<DateTime<Utc>>,
    /// Execution status
    pub status: JobStatus,
    /// Execution result
    pub result: Option<JobResult>,
    /// Error message if failed
    pub error_message: Option<String>,
}

/// Job log entry
#[derive(Debug, Serialize)]
pub struct JobLogEntry {
    /// Log timestamp
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: String,
    /// Log message
    pub message: String,
    /// Log source (e.g., "stdout", "stderr", "system")
    pub source: String,
}

/// Query parameters for metrics
#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    /// Metrics format ("prometheus" or "json")
    #[serde(default = "default_metrics_format")]
    pub format: String,
    /// Include node metrics
    #[serde(default = "default_true")]
    pub include_nodes: bool,
    /// Include job metrics
    #[serde(default = "default_true")]
    pub include_jobs: bool,
    /// Include control plane metrics
    #[serde(default = "default_true")]
    pub include_control_plane: bool,
    /// Time range for metrics (in seconds from now)
    pub time_range_seconds: Option<u64>,
}

fn default_metrics_format() -> String {
    "json".to_string()
}

fn default_true() -> bool {
    true
}

/// Metrics response
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    /// Metrics format
    pub format: String,
    /// Collection timestamp
    pub timestamp: DateTime<Utc>,
    /// Control plane metrics
    pub control_plane: Option<ControlPlaneMetricsData>,
    /// Node metrics
    pub nodes: Option<Vec<NodeMetricsData>>,
    /// Job metrics
    pub jobs: Option<JobMetricsData>,
    /// System metrics
    pub system: Option<SystemMetricsData>,
}

/// Control plane metrics data
#[derive(Debug, Serialize)]
pub struct ControlPlaneMetricsData {
    /// Control plane state
    pub state: ControlPlaneState,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Total jobs processed
    pub total_jobs_processed: u64,
    /// Jobs per second
    pub jobs_per_second: f64,
    /// Average job processing time
    pub avg_job_processing_time_ms: f64,
    /// Active nodes
    pub active_nodes: u32,
    /// Memory usage in MB
    pub memory_usage_mb: f64,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
}

/// Node metrics data
#[derive(Debug, Serialize)]
pub struct NodeMetricsData {
    /// Node ID
    pub node_id: NodeId,
    /// Node name
    pub name: String,
    /// Node type
    pub node_type: String,
    /// Resource usage
    pub resource_usage: ResourceUsage,
    /// Jobs executed
    pub jobs_executed: u64,
    /// Average job duration
    pub avg_job_duration_ms: f64,
    /// Node uptime
    pub uptime_seconds: u64,
}

/// Job metrics data
#[derive(Debug, Serialize)]
pub struct JobMetricsData {
    /// Total jobs
    pub total_jobs: u64,
    /// Jobs by status
    pub jobs_by_status: HashMap<String, u64>,
    /// Jobs by type
    pub jobs_by_type: HashMap<String, u64>,
    /// Average job duration
    pub avg_job_duration_ms: f64,
    /// Job success rate
    pub success_rate: f64,
    /// Jobs per hour
    pub jobs_per_hour: f64,
}

/// System metrics data
#[derive(Debug, Serialize)]
pub struct SystemMetricsData {
    /// Total CPU cores
    pub total_cpu_cores: f64,
    /// Total memory in MB
    pub total_memory_mb: u64,
    /// Total disk space in MB
    pub total_disk_mb: u64,
    /// System load average
    pub load_average: f64,
    /// Network throughput
    pub network_throughput_mbps: f64,
}

/// Schedule a job to be executed on available runner
///
/// This endpoint accepts job scheduling requests and distributes them
/// to available runners based on resource requirements and affinity rules.
pub async fn schedule_job(
    State(state): State<AppState>,
    Extension(security_context): Extension<SecurityContext>,
    Json(request): Json<ScheduleJobRequest>,
) -> Result<Json<ScheduleJobResponse>> {
    debug!("Received job scheduling request: {:?}", request.name);

    // Check permissions
    security_context.require_permission(&Permission::ScheduleJobs)?;

    // Create control plane job from request
    let job_id = Uuid::new_v4();

    // Convert control plane job request to domain job
    let _job = Job {
        id: job_id,
        pipeline_id: Uuid::new_v4(), // Generate a pipeline ID for this job
        name: request.name.clone(),
        steps: vec![], // TODO: Convert payload to job steps
        requirements: JobRequirements {
            required_tags: vec![request.job_type.clone()],
            excluded_tags: vec![],
            min_resources: request.resource_requirements.as_ref().map(|r| {
                DomainResourceRequirements {
                    min_cpu: (r.cpu_cores.unwrap_or(1.0) * 1000.0) as u32, // Convert cores to millicores
                    min_memory: r.memory_mb.unwrap_or(512) as u32,
                    min_disk: Some(r.disk_mb.unwrap_or(1024) as u32),
                }
            }),
            runner_type: None,
            node_affinity: request
                .node_affinity
                .as_ref()
                .map(|a| a.required_labels.keys().cloned().collect())
                .unwrap_or_default(),
        },
        priority: request.priority.clone(),
        timeout: Duration::from_secs(request.timeout_seconds.unwrap_or(3600)),
        retry_policy: RetryPolicy {
            max_retries: request
                .retry_config
                .as_ref()
                .map(|r| r.max_attempts)
                .unwrap_or(3),
            retry_delay: Duration::from_secs(
                request
                    .retry_config
                    .as_ref()
                    .map(|r| r.initial_delay_seconds)
                    .unwrap_or(30),
            ),
            backoff_strategy: BackoffStrategy::Exponential {
                multiplier: request
                    .retry_config
                    .as_ref()
                    .map(|r| r.backoff_multiplier)
                    .unwrap_or(2.0),
            },
        },
        metadata: request.metadata.clone(),
        created_at: Utc::now(),
        scheduled_at: None,
    };

    // TODO: Implement actual job scheduling logic
    // For now, return a mock response
    let response = ScheduleJobResponse {
        job_id,
        scheduled_node_id: None, // Will be assigned by scheduler
        status: JobStatus::Queued,
        estimated_start_time: Some(Utc::now() + chrono::Duration::seconds(30)),
        queue_position: Some(1),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("scheduler".to_string(), "control-plane".to_string());
            meta.insert("priority".to_string(), format!("{:?}", request.priority));
            meta
        },
    };

    // Log audit event
    if let Some(audit_logger) = &state.audit_logger {
        let audit_event = AuditEvent::new(
            AuditAction::ScheduleJob,
            "job".to_string(),
            Some(security_context.user_id),
            Some(security_context.session_id.clone()),
        )
        .with_resource_id(job_id.to_string())
        .with_details(
            "job_name".to_string(),
            serde_json::Value::String(request.name.clone()),
        )
        .with_details(
            "job_type".to_string(),
            serde_json::Value::String(request.job_type.clone()),
        );

        let audit_logger_clone = std::sync::Arc::clone(audit_logger);
        tokio::spawn(async move {
            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                warn!("Failed to log audit event: {}", e);
            }
        });
    }

    info!(
        job_id = %job_id,
        job_name = %request.name,
        job_type = %request.job_type,
        user_id = %security_context.user_id,
        "Job scheduled successfully"
    );

    Ok(Json(response))
}

/// Register a new runner node
///
/// This endpoint allows runner nodes to register themselves with the control plane
/// and receive configuration for job execution.
pub async fn register_node(
    State(state): State<AppState>,
    Extension(security_context): Extension<SecurityContext>,
    Json(request): Json<RegisterNodeRequest>,
) -> Result<Json<RegisterNodeResponse>> {
    debug!("Received node registration request: {:?}", request.name);

    // Check permissions
    security_context.require_permission(&Permission::ManageNodes)?;

    // TODO: Validate authentication token for node registration
    // if let Some(token) = &request.auth_token {
    //     // Validate mTLS or node authentication token
    // }

    // Generate node ID
    let node_id = Uuid::new_v4();

    // TODO: Implement actual node registration logic
    // For now, return a mock response
    let response = RegisterNodeResponse {
        node_id,
        status: "registered".to_string(),
        config: NodeConfiguration {
            control_plane_endpoints: vec![format!(
                "http://{}:{}",
                state.env.server.host, state.env.server.port
            )],
            job_polling_interval: 30,
            max_concurrent_jobs: 10,
            settings: HashMap::new(),
        },
        heartbeat_interval: 30,
        registered_at: Utc::now(),
    };

    // Log audit event
    if let Some(audit_logger) = &state.audit_logger {
        let audit_event = AuditEvent::new(
            AuditAction::RegisterNode,
            "node".to_string(),
            Some(security_context.user_id),
            Some(security_context.session_id.clone()),
        )
        .with_resource_id(node_id.to_string())
        .with_details(
            "node_name".to_string(),
            serde_json::Value::String(request.name.clone()),
        )
        .with_details(
            "node_type".to_string(),
            serde_json::Value::String(request.node_type.clone()),
        );

        let audit_logger_clone = std::sync::Arc::clone(audit_logger);
        tokio::spawn(async move {
            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                warn!("Failed to log audit event: {}", e);
            }
        });
    }

    info!(
        node_id = %node_id,
        node_name = %request.name,
        node_type = %request.node_type,
        user_id = %security_context.user_id,
        "Node registered successfully"
    );

    Ok(Json(response))
}

/// Get status/heartbeat of nodes
///
/// This endpoint provides information about all registered nodes,
/// their current status, and resource utilization.
pub async fn get_node_status(
    State(state): State<AppState>,
    Extension(security_context): Extension<SecurityContext>,
    Query(query): Query<NodeStatusQuery>,
) -> Result<Json<NodeStatusResponse>> {
    debug!("Received node status request with query: {:?}", query);

    // Check permissions
    security_context.require_permission(&Permission::ViewNodes)?;

    // TODO: Implement actual node status retrieval
    // For now, return a mock response
    let response = NodeStatusResponse {
        total_nodes: 3,
        nodes: vec![NodeStatusInfo {
            node_id: Uuid::new_v4(),
            name: "worker-node-1".to_string(),
            node_type: "native".to_string(),
            status: DomainNodeStatus::Active,
            capabilities: NodeCapabilities {
                cpu_cores: 4.0,
                memory_mb: 8192,
                disk_mb: 102400,
                gpu_units: 0,
                supported_job_types: vec!["build".to_string(), "test".to_string()],
                labels: HashMap::new(),
                custom: HashMap::new(),
            },
            resource_usage: Some(ResourceUsage {
                cpu_usage_percent: 45.2,
                memory_usage_mb: 3072,
                disk_usage_mb: 25600,
                gpu_usage_percent: 0.0,
                network_io: None,
            }),
            last_heartbeat: Some(Utc::now() - chrono::Duration::seconds(15)),
            running_jobs: 2,
            uptime_seconds: 86400,
            metadata: HashMap::new(),
        }],
        statistics: NodeStatistics {
            active_nodes: 3,
            failed_nodes: 0,
            total_cpu_cores: 12.0,
            total_memory_mb: 24576,
            avg_cpu_usage: 35.5,
            avg_memory_usage: 42.1,
        },
        last_updated: Utc::now(),
    };

    // Log audit event
    if let Some(audit_logger) = &state.audit_logger {
        let audit_event = AuditEvent::new(
            AuditAction::ViewNodeStatus,
            "node_status".to_string(),
            Some(security_context.user_id),
            Some(security_context.session_id.clone()),
        )
        .with_details(
            "include_metrics".to_string(),
            serde_json::Value::Bool(query.include_metrics),
        )
        .with_details(
            "include_heartbeats".to_string(),
            serde_json::Value::Bool(query.include_heartbeats),
        );

        let audit_logger_clone = std::sync::Arc::clone(audit_logger);
        tokio::spawn(async move {
            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                warn!("Failed to log audit event: {}", e);
            }
        });
    }

    info!(
        user_id = %security_context.user_id,
        "Node status retrieved successfully"
    );

    Ok(Json(response))
}

/// Fetch job execution status
///
/// This endpoint provides detailed information about job execution status,
/// including history and logs if requested.
pub async fn get_job_status(
    State(state): State<AppState>,
    Extension(security_context): Extension<SecurityContext>,
    Path(job_id): Path<JobId>,
    Query(query): Query<JobStatusQuery>,
) -> Result<Json<JobStatusResponse>> {
    debug!("Received job status request for job: {}", job_id);

    // Check permissions
    security_context.require_permission(&Permission::ViewJobs)?;

    // TODO: Implement actual job status retrieval
    // For now, return a mock response
    let response = JobStatusResponse {
        job_id,
        name: "example-build-job".to_string(),
        job_type: "build".to_string(),
        status: JobStatus::Running,
        assigned_node_id: Some(Uuid::new_v4()),
        priority: JobPriority::Normal,
        created_at: Utc::now() - chrono::Duration::minutes(5),
        started_at: Some(Utc::now() - chrono::Duration::minutes(3)),
        completed_at: None,
        duration_seconds: Some(120),
        result: None,
        execution_history: if query.include_history {
            Some(vec![JobExecutionRecord {
                attempt: 1,
                node_id: Uuid::new_v4(),
                started_at: Utc::now() - chrono::Duration::minutes(3),
                ended_at: None,
                status: JobStatus::Running,
                result: None,
                error_message: None,
            }])
        } else {
            None
        },
        logs: if query.include_logs {
            Some(vec![JobLogEntry {
                timestamp: Utc::now() - chrono::Duration::minutes(2),
                level: "INFO".to_string(),
                message: "Job started successfully".to_string(),
                source: "system".to_string(),
            }])
        } else {
            None
        },
        metadata: HashMap::new(),
    };

    // Log audit event
    if let Some(audit_logger) = &state.audit_logger {
        let audit_event = AuditEvent::new(
            AuditAction::ViewJob,
            "job".to_string(),
            Some(security_context.user_id),
            Some(security_context.session_id.clone()),
        )
        .with_resource_id(job_id.to_string())
        .with_details(
            "include_history".to_string(),
            serde_json::Value::Bool(query.include_history),
        )
        .with_details(
            "include_logs".to_string(),
            serde_json::Value::Bool(query.include_logs),
        );

        let audit_logger_clone = std::sync::Arc::clone(audit_logger);
        tokio::spawn(async move {
            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                warn!("Failed to log audit event: {}", e);
            }
        });
    }

    info!(
        job_id = %job_id,
        user_id = %security_context.user_id,
        "Job status retrieved successfully"
    );

    Ok(Json(response))
}

/// Return Prometheus/JSON metrics for runners and jobs
///
/// This endpoint provides comprehensive metrics about the control plane,
/// nodes, jobs, and system performance in either JSON or Prometheus format.
pub async fn get_metrics(
    State(state): State<AppState>,
    Extension(security_context): Extension<SecurityContext>,
    Query(query): Query<MetricsQuery>,
) -> Result<Json<MetricsResponse>> {
    debug!("Received metrics request with format: {}", query.format);

    // Check permissions
    security_context.require_permission(&Permission::ViewMetrics)?;

    // TODO: Implement actual metrics collection
    // For now, return a mock response
    let response = MetricsResponse {
        format: query.format.clone(),
        timestamp: Utc::now(),
        control_plane: if query.include_control_plane {
            Some(ControlPlaneMetricsData {
                state: ControlPlaneState::Running,
                uptime_seconds: 86400,
                total_jobs_processed: 1250,
                jobs_per_second: 2.5,
                avg_job_processing_time_ms: 45000.0,
                active_nodes: 3,
                memory_usage_mb: 512.0,
                cpu_usage_percent: 15.2,
            })
        } else {
            None
        },
        nodes: if query.include_nodes {
            Some(vec![NodeMetricsData {
                node_id: Uuid::new_v4(),
                name: "worker-node-1".to_string(),
                node_type: "native".to_string(),
                resource_usage: ResourceUsage {
                    cpu_usage_percent: 45.2,
                    memory_usage_mb: 3072,
                    disk_usage_mb: 25600,
                    gpu_usage_percent: 0.0,
                    network_io: None,
                },
                jobs_executed: 425,
                avg_job_duration_ms: 42000.0,
                uptime_seconds: 86400,
            }])
        } else {
            None
        },
        jobs: if query.include_jobs {
            Some(JobMetricsData {
                total_jobs: 1250,
                jobs_by_status: {
                    let mut status_map = HashMap::new();
                    status_map.insert("completed".to_string(), 1100);
                    status_map.insert("running".to_string(), 25);
                    status_map.insert("queued".to_string(), 75);
                    status_map.insert("failed".to_string(), 50);
                    status_map
                },
                jobs_by_type: {
                    let mut type_map = HashMap::new();
                    type_map.insert("build".to_string(), 600);
                    type_map.insert("test".to_string(), 400);
                    type_map.insert("deploy".to_string(), 250);
                    type_map
                },
                avg_job_duration_ms: 45000.0,
                success_rate: 0.92,
                jobs_per_hour: 52.0,
            })
        } else {
            None
        },
        system: Some(SystemMetricsData {
            total_cpu_cores: 12.0,
            total_memory_mb: 24576,
            total_disk_mb: 1048576,
            load_average: 1.25,
            network_throughput_mbps: 125.5,
        }),
    };

    // Log audit event
    if let Some(audit_logger) = &state.audit_logger {
        let audit_event = AuditEvent::new(
            AuditAction::ViewMetrics,
            "metrics".to_string(),
            Some(security_context.user_id),
            Some(security_context.session_id.clone()),
        )
        .with_details(
            "format".to_string(),
            serde_json::Value::String(query.format.clone()),
        )
        .with_details(
            "include_nodes".to_string(),
            serde_json::Value::Bool(query.include_nodes),
        )
        .with_details(
            "include_jobs".to_string(),
            serde_json::Value::Bool(query.include_jobs),
        )
        .with_details(
            "include_control_plane".to_string(),
            serde_json::Value::Bool(query.include_control_plane),
        );

        let audit_logger_clone = std::sync::Arc::clone(audit_logger);
        tokio::spawn(async move {
            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                warn!("Failed to log audit event: {}", e);
            }
        });
    }

    info!(
        user_id = %security_context.user_id,
        format = %query.format,
        "Metrics retrieved successfully"
    );

    Ok(Json(response))
}
