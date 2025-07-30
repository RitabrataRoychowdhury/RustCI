use crate::error::Result;
use axum::{extract::Path, response::Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(
    post,
    path = "/api/workspaces",
    tag = "workspace",
    request_body = CreateWorkspaceRequest,
    responses(
    )
)]
pub async fn create_workspace(
    Json(_request): Json<CreateWorkspaceRequest>,
) -> Result<Json<WorkspaceResponse>> {
    let workspace = WorkspaceResponse {
        id: Uuid::new_v4(),
        name: "Sample Workspace".to_string(),
        description: Some("A sample workspace".to_string()),
        status: "active".to_string(),
        created_at: chrono::Utc::now(),
    };
    Ok(Json(workspace))
}

#[utoipa::path(
    get,
    path = "/api/workspaces/{workspace_id}",
    tag = "workspace",
    params(),
    responses()
)]
pub async fn get_workspace(Path(_workspace_id): Path<String>) -> Result<Json<WorkspaceResponse>> {
    let workspace = WorkspaceResponse {
        id: Uuid::new_v4(),
        name: "Sample Workspace".to_string(),
        description: Some("A sample workspace".to_string()),
        status: "active".to_string(),
        created_at: chrono::Utc::now(),
    };
    Ok(Json(workspace))
}

#[utoipa::path(get, path = "/api/workspaces", tag = "workspace", responses())]
pub async fn list_workspaces() -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "workspaces": [],
        "total": 0
    })))
}

#[utoipa::path(
    delete,
    path = "/api/workspaces/{workspace_id}",
    tag = "workspace",
    params(),
    responses()
)]
pub async fn delete_workspace(
    Path(_workspace_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "message": "Workspace deleted successfully"
    })))
}
