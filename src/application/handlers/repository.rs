use crate::error::Result;
use axum::{extract::Path, response::Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct RepositoryInfo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub default_branch: String,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SyncRequest {
    pub force: Option<bool>,
    pub branch: Option<String>,
}

#[utoipa::path(get, path = "/api/repositories", tag = "repository", responses())]
pub async fn list_repositories() -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "repositories": [],
        "total": 0
    })))
}

#[utoipa::path(
    get,
    path = "/api/repositories/{repo_id}",
    tag = "repository",
    params(),
    responses()
)]
pub async fn get_repository(Path(_repo_id): Path<String>) -> Result<Json<RepositoryInfo>> {
    let repo = RepositoryInfo {
        id: 1,
        name: "sample-repo".to_string(),
        full_name: "user/sample-repo".to_string(),
        description: Some("A sample repository".to_string()),
        private: false,
        default_branch: "main".to_string(),
        language: Some("Rust".to_string()),
    };
    Ok(Json(repo))
}

#[utoipa::path(
    post,
    path = "/api/repositories/{repo_id}/sync",
    tag = "repository",
    params(
    ),
    request_body = SyncRequest,
    responses(
    )
)]
pub async fn sync_repository(
    Path(_repo_id): Path<String>,
    Json(_request): Json<SyncRequest>,
) -> Result<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "message": "Repository synchronized successfully"
    })))
}
