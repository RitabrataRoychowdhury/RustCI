//! Runner lifecycle management HTTP handlers
//!
//! This module provides HTTP API endpoints for runner lifecycle management,
//! including registration, job triggering, log fetching, artifact retrieval, and deregistration.

use crate::{
    core::networking::security::{Permission, SecurityContext},
    domain::entities::runner::{
        Job, JobId, JobPriority, JobStatus, JobStep, RunnerEntity, RunnerId, RunnerStatus,
        RunnerType,
    },
    error::{AppError, Result},
    infrastructure::repositories::{RunnerRepository, RunnerFilter},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info};
use utoipa::ToSchema;
use uuid::Uuid;

/// Request to register a new runner
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRunnerRequest {
    /// Runner name
    pub name: String,
    /// Runner type configuration
    pub runner_type: String, // Simplified for now - could be enum
    /// Maximum concurrent jobs
    pub capacity: u32,
    /// Runner tags for job matching
    pub tags: Vec<String>,
    /// Additional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Response for runner registration
#[derive(Debug, Serialize, ToSchema)]
pub struct RunnerResponse {
    /// Runner ID
    pub id: RunnerId,
    /// Runner name
    pub name: String,
    /// Runner type
    pub runner_type: String,
    /// Current status
    pub status: RunnerStatus,
    /// Maximum concurrent jobs
    pub capacity: u32,
    /// Runner tags
    pub tags: Vec<String>,
    /// Registration timestamp
    pub created_at: DateTime<Utc>,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
}

/// Request to trigger a job on a runner
#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerJobRequest {
    /// Job name
    pub name: String,
    /// Job steps to execute
    pub steps: Vec<JobStepRequest>,
    /// Job priority
    pub priority: Option<String>, // "low", "normal", "high", "critical"
    /// Job timeout in seconds
    pub timeout: Option<u64>,
    /// Additional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Job step in the request
#[derive(Debug, Deserialize, ToSchema)]
pub struct JobStepRequest {
    /// Step name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Step timeout in seconds
    pub timeout: Option<u64>,
}

/// Response for job trigger
#[derive(Debug, Serialize, ToSchema)]
pub struct JobResponse {
    /// Job ID
    pub id: JobId,
    /// Runner ID
    pub runner_id: RunnerId,
    /// Job name
    pub name: String,
    /// Job status
    pub status: JobStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Started timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Finished timestamp
    pub finished_at: Option<DateTime<Utc>>,
}

/// Query parameters for log fetching
#[derive(Debug, Deserialize)]
pub struct LogQuery {
    /// Follow logs (streaming)
    pub follow: Option<bool>,
    /// Number of lines to tail
    pub tail: Option<u32>,
}

/// Register a new runner
#[utoipa::path(
    post,
    path = "/api/runners",
    tag = "runners",
    request_body = RegisterRunnerRequest,
    responses(
        (status = 201, description = "Runner registered successfully", body = RunnerResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn register_runner(
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
    Json(request): Json<RegisterRunnerRequest>,
) -> Result<impl IntoResponse> {
    info!("🔄 Registering new runner: {}", request.name);

    // Validate admin permissions for runner registration
    security_ctx.require_permission(&Permission::ManageSystem)?;

    // Create runner type (simplified for now)
    let runner_type = match request.runner_type.as_str() {
        "local" => RunnerType::Local {
            max_concurrent_jobs: request.capacity,
            working_directory: "/tmp".to_string(),
        },
        "docker" => RunnerType::Docker {
            max_concurrent_jobs: request.capacity,
            docker_config: crate::domain::entities::runner::DockerConfig {
                endpoint: "unix:///var/run/docker.sock".to_string(),
                default_image: "ubuntu:latest".to_string(),
                network: None,
                volumes: Vec::new(),
                environment: HashMap::new(),
            },
        },
        _ => {
            return Err(AppError::ValidationError(format!(
                "Unsupported runner type: {}",
                request.runner_type
            )));
        }
    };

    // Create runner entity
    let mut runner = RunnerEntity::new(request.name.clone(), runner_type);
    runner.tags = request.tags.clone();
    if let Some(metadata) = request.metadata {
        runner.metadata = metadata;
    }

    // Store runner in database
    state.runner_repository.store_runner(&runner).await?;

    let response = RunnerResponse {
        id: runner.id,
        name: runner.name,
        runner_type: request.runner_type,
        status: runner.status,
        capacity: request.capacity,
        tags: runner.tags,
        created_at: runner.created_at,
        last_heartbeat: runner.last_heartbeat,
    };

    info!(
        "✅ Runner registered successfully: {} (ID: {})",
        request.name, runner.id
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Trigger a job on a specific runner
#[utoipa::path(
    post,
    path = "/api/runners/{runner_id}/jobs",
    tag = "runners",
    params(
        ("runner_id" = String, Path, description = "Runner ID")
    ),
    request_body = TriggerJobRequest,
    responses(
        (status = 201, description = "Job triggered successfully", body = JobResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Runner not found")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn trigger_job_on_runner(
    Path(runner_id): Path<String>,
    State(_state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
    Json(request): Json<TriggerJobRequest>,
) -> Result<impl IntoResponse> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;

    info!(
        "🚀 Triggering job '{}' on runner: {}",
        request.name, runner_id
    );

    // Validate job trigger permissions
    security_ctx.require_permission(&Permission::ExecutePipelines)?;

    // Parse priority
    let priority = match request.priority.as_deref() {
        Some("low") => JobPriority::Low,
        Some("high") => JobPriority::High,
        Some("critical") => JobPriority::Critical,
        _ => JobPriority::Normal,
    };

    // Convert request steps to job steps
    let steps: Vec<JobStep> = request
        .steps
        .into_iter()
        .map(|step| JobStep {
            name: step.name,
            command: step.command,
            args: step.args,
            working_directory: None,
            environment: HashMap::new(),
            timeout: step.timeout.map(Duration::from_secs),
            continue_on_error: false,
        })
        .collect();

    // Create job
    let mut job = Job::new(request.name.clone(), Uuid::new_v4(), steps);
    job.priority = priority;
    if let Some(timeout) = request.timeout {
        job.timeout = Duration::from_secs(timeout);
    }
    if let Some(metadata) = request.metadata {
        job.metadata = metadata;
    }

    // Submit job to repository for persistent storage and tracking
    let created_job = state.job_repository.create(&job).await?;

    let response = JobResponse {
        id: created_job.id,
        runner_id,
        name: created_job.name,
        status: JobStatus::Queued,
        created_at: created_job.created_at,
        started_at: None,
        finished_at: None,
    };

    info!(
        "✅ Job triggered successfully: {} (ID: {})",
        request.name, job.id
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Fetch logs for a specific job
#[utoipa::path(
    get,
    path = "/api/runners/{runner_id}/jobs/{job_id}/logs",
    tag = "runners",
    params(
        ("runner_id" = String, Path, description = "Runner ID"),
        ("job_id" = String, Path, description = "Job ID"),
        ("follow" = Option<bool>, Query, description = "Follow logs in real-time"),
        ("tail" = Option<u32>, Query, description = "Number of lines to tail")
    ),
    responses(
        (status = 200, description = "Job logs", body = String),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Runner or job not found")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn fetch_job_logs(
    Path((runner_id, job_id)): Path<(String, String)>,
    Query(query): Query<LogQuery>,
    State(_state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<impl IntoResponse> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| AppError::ValidationError("Invalid job ID format".to_string()))?;

    debug!(
        "📋 Fetching logs for job {} on runner {}",
        job_id, runner_id
    );

    // Validate log access permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    // TODO: Implement actual log fetching
    // For now, return mock logs
    let logs = if query.follow.unwrap_or(false) {
        // In a real implementation, this would stream logs
        format!(
            "[{}] Job {} started on runner {}\n[{}] Step 1: Building...\n[{}] Step 1: Build completed successfully\n[{}] Job completed",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            job_id,
            runner_id,
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        )
    } else {
        let tail_lines = query.tail.unwrap_or(100);
        format!(
            "Showing last {} lines for job {}\n[{}] Job completed successfully",
            tail_lines,
            job_id,
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        )
    };

    debug!("✅ Logs retrieved for job {}", job_id);

    Ok(logs)
}

/// Fetch artifacts for a specific job
#[utoipa::path(
    get,
    path = "/api/runners/{runner_id}/jobs/{job_id}/artifacts",
    tag = "runners",
    params(
        ("runner_id" = String, Path, description = "Runner ID"),
        ("job_id" = String, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job artifacts"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Runner, job, or artifacts not found")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn fetch_job_artifacts(
    Path((runner_id, job_id)): Path<(String, String)>,
    State(_state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<impl IntoResponse> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| AppError::ValidationError("Invalid job ID format".to_string()))?;

    debug!(
        "📦 Fetching artifacts for job {} on runner {}",
        job_id, runner_id
    );

    // Validate artifact access permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    // TODO: Implement actual artifact fetching
    // For now, return a simple response indicating no artifacts
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "message": "No artifacts found for this job",
                "job_id": job_id,
                "runner_id": runner_id
            })
            .to_string(),
        )
        .map_err(|e| AppError::InternalServerError(format!("Failed to build response: {}", e)))?;

    debug!("✅ Artifact fetch completed for job {}", job_id);

    Ok(response)
}

/// Deregister a runner
#[utoipa::path(
    delete,
    path = "/api/runners/{runner_id}",
    tag = "runners",
    params(
        ("runner_id" = String, Path, description = "Runner ID")
    ),
    responses(
        (status = 200, description = "Runner deregistered successfully"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Runner not found")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn deregister_runner(
    Path(runner_id): Path<String>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<impl IntoResponse> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;

    info!("🗑️ Deregistering runner: {}", runner_id);

    // Validate deregistration permissions
    security_ctx.require_permission(&Permission::ManageSystem)?;

    // Check if runner exists
    let runner = state.runner_repository.find_runner(runner_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Runner not found: {}", runner_id)))?;

    // Update runner status to deregistering
    state.runner_repository.update_runner_status(runner_id, RunnerStatus::Deregistering).await?;
    
    // TODO: In a full implementation, this would:
    // 1. Check if runner has running jobs
    // 2. Gracefully stop or wait for jobs to complete
    // 3. Clean up any associated resources
    
    // Remove runner from database
    state.runner_repository.remove_runner(runner_id).await?;

    info!("✅ Runner deregistered successfully: {}", runner_id);

    Ok(Json(serde_json::json!({
        "message": "Runner deregistered successfully",
        "runner_id": runner_id
    })))
}

/// Get runner status
#[utoipa::path(
    get,
    path = "/api/runners/{runner_id}/status",
    tag = "runners",
    params(
        ("runner_id" = String, Path, description = "Runner ID")
    ),
    responses(
        (status = 200, description = "Runner status", body = RunnerResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Runner not found")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn get_runner_status(
    Path(runner_id): Path<String>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<impl IntoResponse> {
    let runner_id = Uuid::parse_str(&runner_id)
        .map_err(|_| AppError::ValidationError("Invalid runner ID format".to_string()))?;

    debug!("🔍 Getting status for runner: {}", runner_id);

    // Validate read permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    // Fetch actual runner from database
    let runner = state.runner_repository.find_runner(runner_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Runner not found: {}", runner_id)))?;

    let runner_type_str = match &runner.runner_type {
        RunnerType::Local { .. } => "local",
        RunnerType::Docker { .. } => "docker",
        RunnerType::Kubernetes { .. } => "kubernetes",
    };

    let response = RunnerResponse {
        id: runner.id,
        name: runner.name,
        runner_type: runner_type_str.to_string(),
        status: runner.status,
        capacity: runner.capacity,
        tags: runner.tags,
        created_at: runner.created_at,
        last_heartbeat: runner.last_heartbeat,
    };

    debug!("✅ Status retrieved for runner: {}", runner_id);

    Ok(Json(response))
}

/// List all runners
#[utoipa::path(
    get,
    path = "/api/runners",
    tag = "runners",
    responses(
        (status = 200, description = "List of runners", body = Vec<RunnerResponse>),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions")
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn list_runners(
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<impl IntoResponse> {
    debug!("📋 Listing all runners");

    // Validate read permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    // Fetch actual runners from database
    let filter = RunnerFilter::default();
    let runners_entities = state.runner_repository.list_runners(filter).await?;
    
    let runners: Vec<RunnerResponse> = runners_entities
        .into_iter()
        .map(|runner| {
            let runner_type_str = match &runner.runner_type {
                RunnerType::Local { .. } => "local",
                RunnerType::Docker { .. } => "docker", 
                RunnerType::Kubernetes { .. } => "kubernetes",
            };
            
            RunnerResponse {
                id: runner.id,
                name: runner.name,
                runner_type: runner_type_str.to_string(),
                status: runner.status,
                capacity: runner.capacity,
                tags: runner.tags,
                created_at: runner.created_at,
                last_heartbeat: runner.last_heartbeat,
            }
        })
        .collect();

    debug!("✅ Listed {} runners", runners.len());

    Ok(Json(runners))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::networking::security::{Role, SecurityContext};
    use std::collections::HashSet;

    fn create_mock_security_context() -> SecurityContext {
        SecurityContext {
            user_id: Uuid::new_v4(),
            roles: vec![Role::Admin],
            permissions: Role::Admin.permissions(),
            session_id: "test-session".to_string(),
            ip_address: None,
            user_agent: None,
        }
    }

    #[test]
    fn test_register_runner_request_deserialization() {
        let json = r#"{
            "name": "test-runner",
            "runner_type": "local",
            "capacity": 4,
            "tags": ["rust", "linux"],
            "metadata": {
                "region": "us-west-1"
            }
        }"#;

        let request: RegisterRunnerRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "test-runner");
        assert_eq!(request.runner_type, "local");
        assert_eq!(request.capacity, 4);
        assert_eq!(request.tags, vec!["rust", "linux"]);
        assert!(request.metadata.is_some());
    }

    #[test]
    fn test_trigger_job_request_deserialization() {
        let json = r#"{
            "name": "test-job",
            "steps": [
                {
                    "name": "build",
                    "command": "cargo",
                    "args": ["build", "--release"],
                    "timeout": 300
                }
            ],
            "priority": "high",
            "timeout": 3600
        }"#;

        let request: TriggerJobRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.name, "test-job");
        assert_eq!(request.steps.len(), 1);
        assert_eq!(request.steps[0].name, "build");
        assert_eq!(request.priority, Some("high".to_string()));
        assert_eq!(request.timeout, Some(3600));
    }

    #[test]
    fn test_security_context_permissions() {
        let ctx = create_mock_security_context();
        assert!(ctx.has_permission(&Permission::ManageSystem));
        assert!(ctx.has_permission(&Permission::ReadPipelines));
        assert!(ctx.has_permission(&Permission::ExecutePipelines));
    }
}
