// Valkyrie-Enhanced API Routes
// Task 3.2: Unified Control Plane Integration

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::application::handlers::valkyrie_control_plane::ValkyrieControlPlaneHandler;
use crate::application::services::valkyrie_integration::ValkyrieIntegrationService;
use crate::config::valkyrie_integration::ValkyrieIntegrationConfig;
use crate::error::{AppError, Result};
use crate::infrastructure::runners::{ValkyrieJob, JobSubmissionResult, AdapterPerformanceSnapshot};

/// Valkyrie-enhanced API routes with backward compatibility
pub fn create_valkyrie_routes(
    control_plane_handler: Arc<ValkyrieControlPlaneHandler>,
) -> Router<Arc<ValkyrieControlPlaneHandler>> {
    Router::new()
        // Job management endpoints (enhanced)
        .route("/api/v2/jobs", post(submit_job_enhanced))
        .route("/api/v2/jobs/:job_id", get(get_job_status_enhanced))
        .route("/api/v2/jobs/:job_id", delete(cancel_job_enhanced))
        .route("/api/v2/jobs", get(list_jobs_enhanced))
        
        // Runner management endpoints (enhanced)
        .route("/api/v2/runners", get(list_runners_enhanced))
        .route("/api/v2/runners/:runner_id", get(get_runner_details_enhanced))
        .route("/api/v2/runners/:runner_id/status", put(update_runner_status_enhanced))
        .route("/api/v2/runners/register", post(register_runner_enhanced))
        
        // Performance and monitoring endpoints
        .route("/api/v2/performance/metrics", get(get_performance_metrics))
        .route("/api/v2/performance/health", get(get_system_health))
        .route("/api/v2/performance/diagnostics", get(get_system_diagnostics))
        
        // Configuration management endpoints
        .route("/api/v2/config/valkyrie", get(get_valkyrie_config))
        .route("/api/v2/config/valkyrie", put(update_valkyrie_config))
        .route("/api/v2/config/valkyrie/reload", post(reload_valkyrie_config))
        .route("/api/v2/config/valkyrie/validate", post(validate_valkyrie_config))
        
        // Integration status endpoints
        .route("/api/v2/integration/status", get(get_integration_status))
        .route("/api/v2/integration/capabilities", get(get_integration_capabilities))
        .route("/api/v2/integration/fallback", post(toggle_fallback_mode))
        
        // Backward compatibility endpoints (v1 API with Valkyrie enhancement)
        .route("/api/v1/jobs", post(submit_job_compatible))
        .route("/api/v1/jobs/:job_id", get(get_job_status_compatible))
        .route("/api/v1/jobs/:job_id", delete(cancel_job_compatible))
        .route("/api/v1/runners", get(list_runners_compatible))
        
        .with_state(control_plane_handler)
}

// Enhanced API Request/Response Types

#[derive(Debug, Serialize, Deserialize)]
pub struct EnhancedJobSubmissionRequest {
    // Standard job fields
    pub job_type: String,
    pub priority: Option<u8>,
    pub payload: serde_json::Value,
    pub requirements: Option<JobRequirementsRequest>,
    pub metadata: Option<HashMap<String, String>>,
    
    // Valkyrie-specific enhancements
    pub qos_requirements: Option<QoSRequirementsRequest>,
    pub routing_hints: Option<RoutingHintsRequest>,
    pub performance_targets: Option<PerformanceTargetsRequest>,
    pub deadline: Option<String>, // ISO 8601 timestamp
    pub correlation_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobRequirementsRequest {
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u32>,
    pub storage_gb: Option<u32>,
    pub gpu_count: Option<u32>,
    pub max_execution_time: Option<u64>, // seconds
    pub required_capabilities: Option<Vec<String>>,
    pub preferred_regions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QoSRequirementsRequest {
    pub max_latency: Option<u64>, // milliseconds
    pub min_throughput: Option<u64>, // bytes/second
    pub reliability_threshold: Option<f64>, // 0.0 to 1.0
    pub priority_class: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingHintsRequest {
    pub preferred_runners: Option<Vec<String>>,
    pub excluded_runners: Option<Vec<String>>,
    pub geographic_preference: Option<String>,
    pub affinity_rules: Option<Vec<AffinityRuleRequest>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AffinityRuleRequest {
    pub rule_type: String, // "required", "preferred", "anti_affinity"
    pub key: String,
    pub values: Vec<String>,
    pub weight: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceTargetsRequest {
    pub max_dispatch_latency: Option<u64>, // microseconds
    pub target_completion_time: Option<u64>, // seconds
    pub resource_efficiency_target: Option<f64>, // 0.0 to 1.0
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnhancedJobSubmissionResponse {
    pub job_id: String,
    pub status: String,
    pub estimated_start_time: Option<String>,
    pub estimated_completion_time: Option<String>,
    pub assigned_runner: Option<String>,
    pub dispatch_latency_us: Option<u64>,
    pub qos_guarantees: Option<QoSGuaranteesResponse>,
    pub routing_path: Option<RoutingPathResponse>,
    pub performance_metrics: Option<JobPerformanceResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QoSGuaranteesResponse {
    pub max_latency: Option<u64>,
    pub min_throughput: Option<u64>,
    pub reliability_guarantee: f64,
    pub priority_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingPathResponse {
    pub route_id: String,
    pub hops: Vec<String>,
    pub estimated_latency: Option<u64>,
    pub protocol_used: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobPerformanceResponse {
    pub queue_time_us: Option<u64>,
    pub routing_time_us: Option<u64>,
    pub dispatch_time_us: Option<u64>,
    pub total_time_us: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnhancedJobStatusResponse {
    pub job_id: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub runner_id: Option<String>,
    pub result: Option<JobResultResponse>,
    pub performance_metrics: Option<JobPerformanceResponse>,
    pub qos_metrics: Option<QoSMetricsResponse>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResultResponse {
    pub exit_code: i32,
    pub output: String,
    pub artifacts: Vec<String>,
    pub execution_time_ms: u64,
    pub resource_usage: ResourceUsageResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceUsageResponse {
    pub cpu_time_ms: u64,
    pub memory_peak_mb: u64,
    pub storage_used_mb: u64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QoSMetricsResponse {
    pub actual_latency: Option<u64>,
    pub actual_throughput: Option<u64>,
    pub reliability_achieved: f64,
    pub sla_compliance: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnhancedRunnerListResponse {
    pub runners: Vec<EnhancedRunnerInfo>,
    pub total_count: u32,
    pub healthy_count: u32,
    pub total_capacity: u32,
    pub current_utilization: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnhancedRunnerInfo {
    pub runner_id: String,
    pub endpoint: String,
    pub status: String,
    pub capabilities: RunnerCapabilitiesResponse,
    pub current_load: u32,
    pub max_capacity: u32,
    pub performance_metrics: RunnerPerformanceResponse,
    pub geographic_location: Option<GeographicLocationResponse>,
    pub protocol_support: ProtocolSupportResponse,
    pub last_seen: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerCapabilitiesResponse {
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub storage_gb: u32,
    pub gpu_count: u32,
    pub supported_job_types: Vec<String>,
    pub supported_languages: Vec<String>,
    pub supported_frameworks: Vec<String>,
    pub performance_tier: String,
    pub custom_capabilities: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerPerformanceResponse {
    pub average_response_time_ms: u64,
    pub success_rate: f64,
    pub throughput_jobs_per_sec: f64,
    pub resource_utilization: ResourceUtilizationResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceUtilizationResponse {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub storage_usage: f64,
    pub network_usage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeographicLocationResponse {
    pub region: String,
    pub zone: Option<String>,
    pub datacenter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProtocolSupportResponse {
    pub valkyrie_supported: bool,
    pub valkyrie_version: Option<String>,
    pub http_supported: bool,
    pub websocket_supported: bool,
    pub preferred_protocol: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemHealthResponse {
    pub overall_status: String,
    pub valkyrie_status: String,
    pub control_plane_status: String,
    pub runner_pool_status: String,
    pub performance_status: String,
    pub components: Vec<ComponentHealthResponse>,
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealthResponse {
    pub component: String,
    pub status: String,
    pub message: Option<String>,
    pub last_check: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntegrationStatusResponse {
    pub valkyrie_enabled: bool,
    pub valkyrie_version: String,
    pub integration_mode: String, // "full", "fallback", "disabled"
    pub performance_enhancement: bool,
    pub backward_compatibility: bool,
    pub configuration_status: String,
    pub last_config_reload: Option<String>,
}

// Query parameters for list endpoints
#[derive(Debug, Deserialize)]
pub struct JobListQuery {
    pub status: Option<String>,
    pub runner_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>, // "asc" or "desc"
}

#[derive(Debug, Deserialize)]
pub struct RunnerListQuery {
    pub status: Option<String>,
    pub region: Option<String>,
    pub capabilities: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// Enhanced API Handlers

/// Submit job with Valkyrie enhancements
pub async fn submit_job_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Json(request): Json<EnhancedJobSubmissionRequest>,
) -> Result<Json<EnhancedJobSubmissionResponse>> {
    let result = handler.submit_job_enhanced(request).await?;
    Ok(Json(result))
}

/// Get job status with enhanced information
pub async fn get_job_status_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Path(job_id): Path<String>,
) -> Result<Json<EnhancedJobStatusResponse>> {
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| AppError::ValidationError("Invalid job ID format".to_string()))?;
    
    let result = handler.get_job_status_enhanced(job_id).await?;
    Ok(Json(result))
}

/// Cancel job with enhanced response
pub async fn cancel_job_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| AppError::ValidationError("Invalid job ID format".to_string()))?;
    
    handler.cancel_job_enhanced(job_id).await?;
    Ok(Json(serde_json::json!({
        "job_id": job_id.to_string(),
        "status": "cancelled",
        "message": "Job cancellation requested"
    })))
}

/// List jobs with enhanced filtering and information
pub async fn list_jobs_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Query(query): Query<JobListQuery>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.list_jobs_enhanced(query).await?;
    Ok(Json(result))
}

/// List runners with enhanced information
pub async fn list_runners_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Query(query): Query<RunnerListQuery>,
) -> Result<Json<EnhancedRunnerListResponse>> {
    let result = handler.list_runners_enhanced(query).await?;
    Ok(Json(result))
}

/// Get detailed runner information
pub async fn get_runner_details_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Path(runner_id): Path<String>,
) -> Result<Json<EnhancedRunnerInfo>> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;
    
    let result = handler.get_runner_details_enhanced(runner_id).await?;
    Ok(Json(result))
}

/// Update runner status
pub async fn update_runner_status_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Path(runner_id): Path<String>,
    Json(status): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;
    
    handler.update_runner_status_enhanced(runner_id, status).await?;
    Ok(Json(serde_json::json!({
        "runner_id": runner_id.to_string(),
        "status": "updated",
        "message": "Runner status updated successfully"
    })))
}

/// Register new runner with enhanced capabilities
pub async fn register_runner_enhanced(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Json(registration): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.register_runner_enhanced(registration).await?;
    Ok(Json(result))
}

/// Get system performance metrics
pub async fn get_performance_metrics(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.get_performance_metrics().await?;
    Ok(Json(result))
}

/// Get system health status
pub async fn get_system_health(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<SystemHealthResponse>> {
    let result = handler.get_system_health().await?;
    Ok(Json(result))
}

/// Get system diagnostics
pub async fn get_system_diagnostics(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.get_system_diagnostics().await?;
    Ok(Json(result))
}

/// Get Valkyrie configuration
pub async fn get_valkyrie_config(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.get_valkyrie_config().await?;
    Ok(Json(result))
}

/// Update Valkyrie configuration
pub async fn update_valkyrie_config(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Json(config): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    handler.update_valkyrie_config(config).await?;
    Ok(Json(serde_json::json!({
        "status": "updated",
        "message": "Valkyrie configuration updated successfully"
    })))
}

/// Reload Valkyrie configuration
pub async fn reload_valkyrie_config(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<serde_json::Value>> {
    handler.reload_valkyrie_config().await?;
    Ok(Json(serde_json::json!({
        "status": "reloaded",
        "message": "Valkyrie configuration reloaded successfully"
    })))
}

/// Validate Valkyrie configuration
pub async fn validate_valkyrie_config(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Json(config): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.validate_valkyrie_config(config).await?;
    Ok(Json(result))
}

/// Get integration status
pub async fn get_integration_status(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<IntegrationStatusResponse>> {
    let result = handler.get_integration_status().await?;
    Ok(Json(result))
}

/// Get integration capabilities
pub async fn get_integration_capabilities(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.get_integration_capabilities().await?;
    Ok(Json(result))
}

/// Toggle fallback mode
pub async fn toggle_fallback_mode(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.toggle_fallback_mode(request).await?;
    Ok(Json(result))
}

// Backward Compatibility Handlers (v1 API)

/// Submit job with v1 API compatibility
pub async fn submit_job_compatible(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.submit_job_compatible(request).await?;
    Ok(Json(result))
}

/// Get job status with v1 API compatibility
pub async fn get_job_status_compatible(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| AppError::ValidationError("Invalid job ID format".to_string()))?;
    
    let result = handler.get_job_status_compatible(job_id).await?;
    Ok(Json(result))
}

/// Cancel job with v1 API compatibility
pub async fn cancel_job_compatible(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| AppError::ValidationError("Invalid job ID format".to_string()))?;
    
    let result = handler.cancel_job_compatible(job_id).await?;
    Ok(Json(result))
}

/// List runners with v1 API compatibility
pub async fn list_runners_compatible(
    State(handler): State<Arc<ValkyrieControlPlaneHandler>>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    let result = handler.list_runners_compatible(query).await?;
    Ok(Json(result))
}