use crate::{
    AppState,
    ci::{
        config::CIPipeline,
        engine::CIEngineOrchestrator,
        pipeline::{PipelineExecution, TriggerInfo},
    },
    core::networking::security::{Permission, SecurityContext},
    error::{AppError, Result},
    upload::create_upload_handler,
};
use axum::{
    extract::{Multipart, Path, Query, State},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePipelineRequest {
    pub yaml_content: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerPipelineRequest {
    pub trigger_type: String,
    pub branch: Option<String>,
    pub commit_hash: Option<String>,
    pub repository: Option<String>,
    pub environment: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExecutePipelineRequest {
    pub pipeline_id: String,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineStatus {
    pub status: String,
    pub progress: f32,
    pub current_step: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecutionResponse {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub status: String,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TriggerResponse {
    pub execution_id: Uuid,
    pub message: String,
}

/// Create a new CI pipeline from YAML configuration (JSON payload)
pub async fn create_pipeline(
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
    Json(request): Json<CreatePipelineRequest>,
) -> Result<Json<PipelineResponse>> {
    info!(
        user_id = %security_ctx.user_id,
        "🔄 Creating new pipeline from JSON YAML content"
    );

    // Validate pipeline creation permissions
    security_ctx.require_permission(&Permission::WritePipelines)?;

    create_pipeline_from_yaml(&state, &request.yaml_content).await
}

/// Create a new CI pipeline from uploaded YAML file (multipart)
pub async fn create_pipeline_multipart(
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
    multipart: Multipart,
) -> Result<Json<PipelineResponse>> {
    info!(
        user_id = %security_ctx.user_id,
        "📁 Creating new pipeline from uploaded YAML file"
    );

    // Validate pipeline creation permissions
    security_ctx.require_permission(&Permission::WritePipelines)?;

    // Create file upload handler
    let upload_handler = create_upload_handler();

    // Process the multipart upload
    let yaml_content = upload_handler.handle_yaml_upload(multipart).await?;

    info!(
        "✅ Successfully processed uploaded YAML file ({} bytes)",
        yaml_content.len()
    );

    create_pipeline_from_yaml(&state, &yaml_content).await
}

/// Shared pipeline creation logic
async fn create_pipeline_from_yaml(
    state: &AppState,
    yaml_content: &str,
) -> Result<Json<PipelineResponse>> {
    // Parse YAML configuration
    let pipeline = CIPipeline::from_yaml(yaml_content)
        .map_err(|e| AppError::ValidationError(format!("Invalid YAML configuration: {}", e)))?;

    // Validate pipeline
    pipeline.validate().map_err(AppError::ValidationError)?;

    // Create pipeline using CI engine
    let ci_engine = get_ci_engine(state)?;
    let pipeline_id = ci_engine.create_pipeline(pipeline.clone()).await?;

    info!(
        "✅ Pipeline created successfully: {} (ID: {})",
        pipeline.name, pipeline_id
    );

    Ok(Json(PipelineResponse {
        id: pipeline_id,
        name: pipeline.name,
        description: pipeline.description,
        created_at: pipeline.created_at,
        updated_at: pipeline.updated_at,
    }))
}

/// Trigger a pipeline execution
pub async fn trigger_pipeline(
    Path(pipeline_id): Path<Uuid>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
    Json(request): Json<TriggerPipelineRequest>,
) -> Result<Json<TriggerResponse>> {
    info!(
        user_id = %security_ctx.user_id,
        pipeline_id = %pipeline_id,
        "🚀 Triggering pipeline"
    );

    // Validate pipeline execution permissions
    security_ctx.require_permission(&Permission::ExecutePipelines)?;

    // Check repository access if specified
    if let Some(ref repository) = request.repository {
        // TODO: Implement repository access validation
        debug!(
            user_id = %security_ctx.user_id,
            repository = %repository,
            "Validating repository access"
        );
    }

    let trigger_info = TriggerInfo {
        trigger_type: request.trigger_type,
        triggered_by: Some(format!("user:{}", security_ctx.user_id)),
        commit_hash: request.commit_hash,
        branch: request.branch,
        repository: request.repository,
        webhook_payload: None,
    };

    let ci_engine = get_ci_engine(&state)?;
    let execution_id = ci_engine
        .trigger_pipeline(pipeline_id, trigger_info, request.environment)
        .await?;

    info!(
        user_id = %security_ctx.user_id,
        pipeline_id = %pipeline_id,
        execution_id = %execution_id,
        "✅ Pipeline triggered successfully"
    );

    Ok(Json(TriggerResponse {
        execution_id,
        message: "Pipeline triggered successfully".to_string(),
    }))
}

/// Get pipeline execution status
pub async fn get_execution(
    Path(execution_id): Path<Uuid>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<Json<PipelineExecution>> {
    debug!(
        user_id = %security_ctx.user_id,
        execution_id = %execution_id,
        "🔍 Getting execution status"
    );

    // Validate read permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    let ci_engine = get_ci_engine(&state)?;
    let execution = ci_engine.get_execution(execution_id).await?;

    Ok(Json(execution))
}

/// Cancel a running pipeline execution
pub async fn cancel_execution(
    Path(execution_id): Path<Uuid>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>> {
    info!(
        user_id = %security_ctx.user_id,
        execution_id = %execution_id,
        "🛑 Cancelling execution"
    );

    // Validate execution cancellation permissions
    security_ctx.require_permission(&Permission::ExecutePipelines)?;

    let ci_engine = get_ci_engine(&state)?;
    ci_engine.cancel_execution(execution_id).await?;

    info!(
        user_id = %security_ctx.user_id,
        execution_id = %execution_id,
        "✅ Execution cancelled successfully"
    );

    Ok(Json(serde_json::json!({
        "message": "Execution cancelled successfully",
        "execution_id": execution_id
    })))
}

/// List all pipelines
pub async fn list_pipelines(
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<Json<Vec<PipelineResponse>>> {
    debug!(
        user_id = %security_ctx.user_id,
        "📋 Listing all pipelines"
    );

    // Validate read permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    let ci_engine = get_ci_engine(&state)?;
    let pipelines = ci_engine.list_pipelines().await?;

    let response: Vec<PipelineResponse> = pipelines
        .into_iter()
        .map(|p| PipelineResponse {
            id: p.id.unwrap_or_else(Uuid::new_v4),
            name: p.name,
            description: p.description,
            created_at: p.created_at,
            updated_at: p.updated_at,
        })
        .collect();

    debug!(
        user_id = %security_ctx.user_id,
        count = response.len(),
        "✅ Listed pipelines"
    );

    Ok(Json(response))
}

/// List executions for a pipeline or all executions
pub async fn list_executions(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<Json<Vec<ExecutionResponse>>> {
    debug!(
        user_id = %security_ctx.user_id,
        "📋 Listing executions"
    );

    // Validate read permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    let pipeline_id = params
        .get("pipeline_id")
        .and_then(|id| Uuid::parse_str(id).ok());

    let ci_engine = get_ci_engine(&state)?;
    let executions = ci_engine.list_executions(pipeline_id).await?;

    let response: Vec<ExecutionResponse> = executions
        .into_iter()
        .map(|e| ExecutionResponse {
            id: e.id,
            pipeline_id: e.pipeline_id,
            status: format!("{:?}", e.status).to_lowercase(),
            started_at: e.started_at,
            finished_at: e.finished_at,
            duration: e.duration,
        })
        .collect();

    debug!(
        user_id = %security_ctx.user_id,
        count = response.len(),
        "✅ Listed executions"
    );

    Ok(Json(response))
}

/// Get pipeline YAML configuration
pub async fn get_pipeline_yaml(
    Path(pipeline_id): Path<Uuid>,
    State(state): State<AppState>,
    Extension(security_ctx): Extension<SecurityContext>,
) -> Result<String> {
    debug!(
        user_id = %security_ctx.user_id,
        pipeline_id = %pipeline_id,
        "📄 Getting pipeline YAML"
    );

    // Validate read permissions
    security_ctx.require_permission(&Permission::ReadPipelines)?;

    let ci_engine = get_ci_engine(&state)?;
    let pipeline = ci_engine.get_pipeline(pipeline_id).await?;

    let yaml_content = pipeline.to_yaml().map_err(|e| {
        AppError::InternalServerError(format!("Failed to serialize pipeline to YAML: {}", e))
    })?;

    Ok(yaml_content)
}

/// Handle webhook triggers
pub async fn webhook_handler(
    Path(pipeline_id): Path<Uuid>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<TriggerResponse>> {
    info!("🪝 Webhook received for pipeline: {}", pipeline_id);

    let trigger_info = TriggerInfo {
        trigger_type: "webhook".to_string(),
        triggered_by: Some("webhook".to_string()),
        commit_hash: payload
            .get("after")
            .and_then(|v| v.as_str())
            .map(String::from),
        branch: payload
            .get("ref")
            .and_then(|v| v.as_str())
            .and_then(|r| r.strip_prefix("refs/heads/"))
            .map(String::from),
        repository: payload
            .get("repository")
            .and_then(|r| r.get("full_name"))
            .and_then(|v| v.as_str())
            .map(String::from),
        webhook_payload: Some(payload),
    };

    let ci_engine = get_ci_engine(&state)?;
    let execution_id = ci_engine
        .trigger_pipeline(pipeline_id, trigger_info, None)
        .await?;

    info!(
        "✅ Webhook triggered pipeline: {} (Execution: {})",
        pipeline_id, execution_id
    );

    Ok(Json(TriggerResponse {
        execution_id,
        message: "Pipeline triggered by webhook".to_string(),
    }))
}

// Helper function to get CI engine from app state
fn get_ci_engine(state: &AppState) -> Result<Arc<CIEngineOrchestrator>> {
    Ok(state.ci_engine.clone())
}

// CI Pipeline management endpoints - using existing create_pipeline function

#[utoipa::path(
    post,
    path = "/api/ci/pipelines/{pipeline_id}/execute",
    tag = "ci",
    params(
    ),
    request_body = ExecutePipelineRequest,
    responses(
    )
)]
pub async fn execute_pipeline(
    Path(_pipeline_id): Path<String>,
    Json(_request): Json<ExecutePipelineRequest>,
) -> Result<Json<ExecutionResponse>> {
    // Placeholder implementation
    let execution = ExecutionResponse {
        id: Uuid::new_v4(),
        pipeline_id: Uuid::new_v4(),
        status: "running".to_string(),
        started_at: Some(chrono::Utc::now()),
        finished_at: None,
        duration: None,
    };
    Ok(Json(execution))
}

#[utoipa::path(
    get,
    path = "/api/ci/pipelines/{pipeline_id}/status",
    tag = "ci",
    params(),
    responses()
)]
pub async fn get_pipeline_status(Path(_pipeline_id): Path<String>) -> Result<Json<PipelineStatus>> {
    // Placeholder implementation
    let status = PipelineStatus {
        status: "running".to_string(),
        progress: 0.5,
        current_step: Some("build".to_string()),
    };
    Ok(Json(status))
}

// Using existing list_pipelines function

#[utoipa::path(
    delete,
    path = "/api/ci/pipelines/{pipeline_id}",
    tag = "ci",
    params(),
    responses()
)]
pub async fn delete_pipeline(Path(_pipeline_id): Path<String>) -> Result<Json<serde_json::Value>> {
    // Placeholder implementation
    Ok(Json(serde_json::json!({
        "message": "Pipeline deleted successfully"
    })))
}
