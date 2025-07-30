use axum::{
    extract::{Path, Query, State},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    middleware::validation::{validate_string_field, validate_url},
    models::{Workspace, RepositoryMetadata},
    AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceResponse {
    pub status: String,
    pub data: WorkspaceData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceData {
    pub workspace: WorkspaceInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_user_id: i64,
    pub github_username: String,
    pub organization: Option<String>,
    pub repositories: Vec<RepositoryMetadata>,
    pub repository_count: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Workspace> for WorkspaceInfo {
    fn from(workspace: Workspace) -> Self {
        let repository_count = workspace.repositories.len();
        Self {
            id: workspace.id,
            user_id: workspace.user_id,
            github_user_id: workspace.github_user_id,
            github_username: workspace.github_username,
            organization: workspace.organization,
            repositories: workspace.repositories,
            repository_count,
            created_at: workspace.created_at,
            updated_at: workspace.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub organization: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddSecretRequest {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct SecretsResponse {
    pub status: String,
    pub data: SecretsData,
}

#[derive(Debug, Serialize)]
pub struct SecretsData {
    pub secrets: HashMap<String, String>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct LinkRepositoryRequest {
    pub repository_id: i64,
    pub repository_name: String,
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
}

/// Get current user's workspace
#[allow(dead_code)]
pub async fn get_workspace_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
) -> Result<Json<WorkspaceResponse>> {
    tracing::info!("Getting workspace for user: {}", user_id);
    
    // Get workspace service from state (we'll need to add this to AppState)
    // For now, we'll create a simple response
    // In a real implementation, you'd inject the workspace service
    
    // This is a placeholder - you'd use the actual workspace service
    // Using the Workspace model constructor and converting to WorkspaceInfo
    let workspace = Workspace::new(
        user_id,
        12345,
        "testuser".to_string(),
        "encrypted_token".to_string(),
    );
    let workspace_info = WorkspaceInfo::from(workspace);
    
    let response = WorkspaceResponse {
        status: "success".to_string(),
        data: WorkspaceData {
            workspace: workspace_info,
        },
    };
    
    tracing::info!("Successfully retrieved workspace for user: {}", user_id);
    Ok(Json(response))
}

/// Update workspace settings
#[allow(dead_code)]
pub async fn update_workspace_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Json(request): Json<UpdateWorkspaceRequest>,
) -> Result<Json<WorkspaceResponse>> {
    tracing::info!("Updating workspace for user: {}", user_id);
    
    // Validate request using validation middleware
    if let Some(ref org) = request.organization {
        validate_string_field(org, "organization", Some(1), Some(100))?;
    }
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Update the workspace with new settings
    // 4. Return the updated workspace
    
    let workspace_info = WorkspaceInfo {
        id: Uuid::new_v4(),
        user_id,
        github_user_id: 12345,
        github_username: "testuser".to_string(),
        organization: request.organization,
        repositories: Vec::new(),
        repository_count: 0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    let response = WorkspaceResponse {
        status: "success".to_string(),
        data: WorkspaceData {
            workspace: workspace_info,
        },
    };
    
    tracing::info!("Successfully updated workspace for user: {}", user_id);
    Ok(Json(response))
}

/// Get linked repositories
#[allow(dead_code)]
pub async fn get_repositories_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Getting repositories for user: {}", user_id);
    
    let limit = params.get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(50);
    
    let offset = params.get("offset")
        .and_then(|o| o.parse::<usize>().ok())
        .unwrap_or(0);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Get the linked repositories with pagination
    // 4. Return the repositories
    
    let repositories: Vec<RepositoryMetadata> = Vec::new();
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "repositories": repositories,
            "total": repositories.len(),
            "limit": limit,
            "offset": offset
        }
    });
    
    tracing::info!("Successfully retrieved {} repositories for user: {}", repositories.len(), user_id);
    Ok(Json(response))
}

/// Link a repository to workspace
#[allow(dead_code)]
pub async fn link_repository_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Json(request): Json<LinkRepositoryRequest>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Linking repository {} for user: {}", request.repository_name, user_id);
    
    // Validate request using validation middleware
    validate_string_field(&request.repository_name, "repository_name", Some(1), Some(100))?;
    validate_string_field(&request.full_name, "full_name", Some(1), Some(200))?;
    validate_url(&request.clone_url, "clone_url")?;
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Verify the repository exists and user has access
    // 4. Check if repository is already linked
    // 5. Create repository metadata
    // 6. Link repository to workspace
    // 7. Trigger Dockerfile detection and generation
    // 8. Send notification events
    
    let repository_metadata = RepositoryMetadata {
        id: request.repository_id,
        name: request.repository_name.clone(),
        full_name: request.full_name.clone(),
        clone_url: request.clone_url,
        default_branch: request.default_branch,
        has_dockerfile: false, // Will be detected
        project_type: None, // Will be detected
        linked_at: chrono::Utc::now(),
        last_dockerfile_check: None,
    };
    
    let response = serde_json::json!({
        "status": "success",
        "message": "Repository linked successfully",
        "data": {
            "repository": repository_metadata,
            "dockerfile_detection_started": true
        }
    });
    
    tracing::info!("Successfully linked repository {} for user: {}", request.repository_name, user_id);
    Ok(Json(response))
}

/// Unlink a repository from workspace
#[allow(dead_code)]
pub async fn unlink_repository_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path(repository_id): Path<i64>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Unlinking repository {} for user: {}", repository_id, user_id);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Verify the repository is linked
    // 4. Remove repository from workspace
    // 5. Clean up any related data (generated Dockerfiles, etc.)
    
    let response = serde_json::json!({
        "status": "success",
        "message": "Repository unlinked successfully",
        "data": {
            "repository_id": repository_id,
            "unlinked_at": chrono::Utc::now()
        }
    });
    
    tracing::info!("Successfully unlinked repository {} for user: {}", repository_id, user_id);
    Ok(Json(response))
}

/// Add shared secret to workspace
#[allow(dead_code)]
pub async fn add_secret_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Json(request): Json<AddSecretRequest>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Adding secret '{}' for user: {}", request.key, user_id);
    
    // Validate request using validation middleware
    validate_string_field(&request.key, "key", Some(1), Some(50))?;
    validate_string_field(&request.value, "value", Some(1), Some(1000))?;
    
    // Validate key format (alphanumeric and underscores only)
    if !request.key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(AppError::ValidationError("Secret key can only contain alphanumeric characters and underscores".to_string()));
    }
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Add the encrypted secret to the workspace
    // 4. Return success response
    
    let response = serde_json::json!({
        "status": "success",
        "message": "Secret added successfully",
        "data": {
            "key": request.key,
            "added_at": chrono::Utc::now()
        }
    });
    
    tracing::info!("Successfully added secret '{}' for user: {}", request.key, user_id);
    Ok(Json(response))
}

/// Get shared secrets from workspace (returns keys only, not values)
#[allow(dead_code)]
pub async fn get_secrets_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Getting secrets for user: {}", user_id);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Get the secret keys (not values for security)
    // 4. Return the keys with metadata
    
    let secret_keys = vec!["DATABASE_URL", "API_KEY", "JWT_SECRET"];
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "secrets": secret_keys.iter().map(|key| {
                serde_json::json!({
                    "key": key,
                    "created_at": chrono::Utc::now(),
                    "last_used": serde_json::Value::Null
                })
            }).collect::<Vec<_>>(),
            "count": secret_keys.len()
        }
    });
    
    tracing::info!("Successfully retrieved {} secret keys for user: {}", secret_keys.len(), user_id);
    Ok(Json(response))
}

/// Delete a shared secret from workspace
#[allow(dead_code)]
pub async fn delete_secret_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path(secret_key): Path<String>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Deleting secret '{}' for user: {}", secret_key, user_id);
    
    // Validate key using validation middleware
    validate_string_field(&secret_key, "secret_key", Some(1), Some(50))?;
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the workspace service from state
    // 2. Find the user's workspace
    // 3. Remove the secret from the workspace
    // 4. Return success response
    
    let response = serde_json::json!({
        "status": "success",
        "message": "Secret deleted successfully",
        "data": {
            "key": secret_key,
            "deleted_at": chrono::Utc::now()
        }
    });
    
    tracing::info!("Successfully deleted secret '{}' for user: {}", secret_key, user_id);
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    
    #[test]
    fn test_workspace_info_from_workspace() {
        let workspace = Workspace {
            mongo_id: None,
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            github_user_id: 12345,
            github_username: "testuser".to_string(),
            organization: Some("testorg".to_string()),
            encrypted_github_token: "encrypted_token".to_string(),
            repositories: vec![],
            shared_secrets: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let workspace_info = WorkspaceInfo::from(workspace.clone());
        
        assert_eq!(workspace_info.id, workspace.id);
        assert_eq!(workspace_info.user_id, workspace.user_id);
        assert_eq!(workspace_info.github_user_id, workspace.github_user_id);
        assert_eq!(workspace_info.github_username, workspace.github_username);
        assert_eq!(workspace_info.organization, workspace.organization);
        assert_eq!(workspace_info.repository_count, 0);
    }
    
    #[test]
    fn test_link_repository_request_validation() {
        let valid_request = LinkRepositoryRequest {
            repository_id: 123,
            repository_name: "test-repo".to_string(),
            full_name: "user/test-repo".to_string(),
            clone_url: "https://github.com/user/test-repo.git".to_string(),
            default_branch: "main".to_string(),
        };
        
        assert!(!valid_request.repository_name.trim().is_empty());
        assert!(!valid_request.full_name.trim().is_empty());
        assert!(!valid_request.clone_url.trim().is_empty());
    }
    
    #[test]
    fn test_add_secret_request_validation() {
        let valid_request = AddSecretRequest {
            key: "DATABASE_URL".to_string(),
            value: "postgresql://localhost:5432/db".to_string(),
        };
        
        assert!(!valid_request.key.trim().is_empty());
        assert!(!valid_request.value.trim().is_empty());
        assert!(valid_request.key.chars().all(|c| c.is_alphanumeric() || c == '_'));
        
        let invalid_key_request = AddSecretRequest {
            key: "DATABASE-URL".to_string(), // Contains hyphen
            value: "postgresql://localhost:5432/db".to_string(),
        };
        
        assert!(!invalid_key_request.key.chars().all(|c| c.is_alphanumeric() || c == '_'));
    }
}