use crate::{
    ci::{
        config::CIPipeline,
        engine::CIEngine,
        pipeline::{PipelineExecution, TriggerInfo},
    },
    error::{AppError, Result},
    upload::create_upload_handler,
    AppState,
};
use axum::{
    extract::{Multipart, Path, Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreatePipelineRequest {
    pub yaml_content: String,
}

#[derive(Debug, Deserialize)]
pub struct TriggerPipelineRequest {
    pub trigger_type: String,
    pub branch: Option<String>,
    pub commit_hash: Option<String>,
    pub repository: Option<String>,
    pub environment: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct PipelineResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
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
    Json(request): Json<CreatePipelineRequest>,
) -> Result<Json<PipelineResponse>> {
    info!("🔄 Creating new pipeline from JSON YAML content");

    create_pipeline_from_yaml(&state, &request.yaml_content).await
}

/// Create a new CI pipeline from uploaded YAML file (multipart)
pub async fn create_pipeline_multipart(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<PipelineResponse>> {
    info!("📁 Creating new pipeline from uploaded YAML file");

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
    pipeline
        .validate()
        .map_err(AppError::ValidationError)?;

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
    Json(request): Json<TriggerPipelineRequest>,
) -> Result<Json<TriggerResponse>> {
    info!("🚀 Triggering pipeline: {}", pipeline_id);

    let trigger_info = TriggerInfo {
        trigger_type: request.trigger_type,
        triggered_by: Some("api".to_string()),
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
        "✅ Pipeline triggered successfully: {} (Execution: {})",
        pipeline_id, execution_id
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
) -> Result<Json<PipelineExecution>> {
    debug!("🔍 Getting execution status: {}", execution_id);

    let ci_engine = get_ci_engine(&state)?;
    let execution = ci_engine.get_execution(execution_id).await?;

    Ok(Json(execution))
}

/// Cancel a running pipeline execution
pub async fn cancel_execution(
    Path(execution_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    info!("🛑 Cancelling execution: {}", execution_id);

    let ci_engine = get_ci_engine(&state)?;
    ci_engine.cancel_execution(execution_id).await?;

    Ok(Json(serde_json::json!({
        "message": "Execution cancelled successfully",
        "execution_id": execution_id
    })))
}

/// List all pipelines
pub async fn list_pipelines(State(state): State<AppState>) -> Result<Json<Vec<PipelineResponse>>> {
    debug!("📋 Listing all pipelines");

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

    Ok(Json(response))
}

/// List executions for a pipeline or all executions
pub async fn list_executions(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<Vec<ExecutionResponse>>> {
    debug!("📋 Listing executions");

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

    Ok(Json(response))
}

/// Get pipeline YAML configuration
pub async fn get_pipeline_yaml(
    Path(pipeline_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<String> {
    debug!("📄 Getting pipeline YAML: {}", pipeline_id);

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
fn get_ci_engine(state: &AppState) -> Result<Arc<CIEngine>> {
    Ok(state.ci_engine.clone())
}
