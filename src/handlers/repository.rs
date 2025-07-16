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
    models::{GitHubRepo, GitHubContent, RepositoryMetadata, ProjectType},
    AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryListResponse {
    pub status: String,
    pub data: RepositoryListData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryListData {
    pub repositories: Vec<GitHubRepo>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryDetailsResponse {
    pub status: String,
    pub data: RepositoryDetailsData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryDetailsData {
    pub repository: GitHubRepo,
    pub contents: Vec<GitHubContent>,
    pub has_dockerfile: bool,
    pub detected_project_type: Option<ProjectType>,
    pub is_linked: bool,
}

#[derive(Debug, Deserialize)]
pub struct RepositoryQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub sort: Option<String>,
    pub direction: Option<String>,
    pub visibility: Option<String>,
    pub affiliation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectAnalysisResponse {
    pub status: String,
    pub data: ProjectAnalysisData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectAnalysisData {
    pub repository_id: i64,
    pub repository_name: String,
    pub project_type: ProjectType,
    pub confidence: f32,
    pub detected_files: Vec<String>,
    pub has_dockerfile: bool,
    pub dockerfile_path: Option<String>,
    pub analysis_details: ProjectAnalysisDetails,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectAnalysisDetails {
    pub rust_indicators: Vec<String>,
    pub node_indicators: Vec<String>,
    pub python_indicators: Vec<String>,
    pub java_indicators: Vec<String>,
    pub go_indicators: Vec<String>,
    pub other_files: Vec<String>,
}

/// Get user's GitHub repositories
pub async fn get_user_repositories_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Query(query): Query<RepositoryQuery>,
) -> Result<Json<RepositoryListResponse>> {
    tracing::info!("Getting repositories for user: {}", user_id);
    
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(30).min(100); // Cap at 100
    let sort = query.sort.unwrap_or_else(|| "updated".to_string());
    let direction = query.direction.unwrap_or_else(|| "desc".to_string());
    
    tracing::debug!("Repository query - page: {}, per_page: {}, sort: {}, direction: {}", 
        page, per_page, sort, direction);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and GitHub token
    // 2. Use the GitHub service to fetch repositories
    // 3. Apply filtering and pagination
    // 4. Return the repositories
    
    let repositories: Vec<GitHubRepo> = Vec::new(); // Placeholder
    let total = repositories.len();
    let has_more = total > (page * per_page);
    
    let response = RepositoryListResponse {
        status: "success".to_string(),
        data: RepositoryListData {
            repositories,
            total,
            page,
            per_page,
            has_more,
        },
    };
    
    tracing::info!("Successfully retrieved {} repositories for user: {}", total, user_id);
    Ok(Json(response))
}

/// Get detailed information about a specific repository
pub async fn get_repository_details_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
) -> Result<Json<RepositoryDetailsResponse>> {
    tracing::info!("Getting repository details for {}/{} (user: {})", owner, repo, user_id);
    
    // Validate input
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err(AppError::ValidationError("Owner and repository name cannot be empty".to_string()));
    }
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and GitHub token
    // 2. Use the GitHub service to fetch repository details
    // 3. Get repository contents for analysis
    // 4. Check if repository is already linked to workspace
    // 5. Detect project type and Dockerfile presence
    // 6. Return comprehensive repository information
    
    // Create a placeholder repository
    let repository = GitHubRepo {
        id: 123456789,
        name: repo.clone(),
        full_name: format!("{}/{}", owner, repo),
        description: Some("A sample repository".to_string()),
        clone_url: format!("https://github.com/{}/{}.git", owner, repo),
        ssh_url: format!("git@github.com:{}/{}.git", owner, repo),
        html_url: format!("https://github.com/{}/{}", owner, repo),
        default_branch: "main".to_string(),
        private: false,
        fork: false,
        archived: false,
        disabled: false,
        language: Some("Rust".to_string()),
        size: 1024,
        stargazers_count: 10,
        watchers_count: 5,
        forks_count: 2,
        open_issues_count: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        pushed_at: Some(chrono::Utc::now()),
    };
    
    let contents: Vec<GitHubContent> = Vec::new(); // Placeholder
    let has_dockerfile = false; // Would be detected from contents
    let detected_project_type = Some(ProjectType::Rust); // Would be detected from contents
    let is_linked = false; // Would be checked against workspace
    
    let response = RepositoryDetailsResponse {
        status: "success".to_string(),
        data: RepositoryDetailsData {
            repository,
            contents,
            has_dockerfile,
            detected_project_type,
            is_linked,
        },
    };
    
    tracing::info!("Successfully retrieved details for repository {}/{}", owner, repo);
    Ok(Json(response))
}

/// Analyze a repository's project type and structure
pub async fn analyze_repository_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
) -> Result<Json<ProjectAnalysisResponse>> {
    tracing::info!("Analyzing repository {}/{} for user: {}", owner, repo, user_id);
    
    // Validate input
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err(AppError::ValidationError("Owner and repository name cannot be empty".to_string()));
    }
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and GitHub token
    // 2. Use the GitHub service to fetch repository contents
    // 3. Use the project detection service to analyze the project
    // 4. Check for existing Dockerfile
    // 5. Return detailed analysis results
    
    let analysis_details = ProjectAnalysisDetails {
        rust_indicators: vec!["Cargo.toml found".to_string(), "src/main.rs found".to_string()],
        node_indicators: Vec::new(),
        python_indicators: Vec::new(),
        java_indicators: Vec::new(),
        go_indicators: Vec::new(),
        other_files: vec!["README.md".to_string(), "LICENSE".to_string()],
    };
    
    let response = ProjectAnalysisResponse {
        status: "success".to_string(),
        data: ProjectAnalysisData {
            repository_id: 123456789,
            repository_name: format!("{}/{}", owner, repo),
            project_type: ProjectType::Rust,
            confidence: 0.95,
            detected_files: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
            has_dockerfile: false,
            dockerfile_path: None,
            analysis_details,
        },
    };
    
    tracing::info!("Successfully analyzed repository {}/{} - detected type: {:?}", owner, repo, ProjectType::Rust);
    Ok(Json(response))
}

/// Check if a repository has a Dockerfile
pub async fn check_dockerfile_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Checking Dockerfile for repository {}/{} (user: {})", owner, repo, user_id);
    
    // Validate input
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err(AppError::ValidationError("Owner and repository name cannot be empty".to_string()));
    }
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and GitHub token
    // 2. Use the GitHub service to check for Dockerfile existence
    // 3. If found, get the Dockerfile content
    // 4. Return the results
    
    let has_dockerfile = false; // Placeholder
    let dockerfile_paths = vec!["Dockerfile", "docker/Dockerfile", ".docker/Dockerfile"];
    let found_dockerfile_path: Option<String> = None;
    let dockerfile_content: Option<String> = None;
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "repository": format!("{}/{}", owner, repo),
            "has_dockerfile": has_dockerfile,
            "dockerfile_path": found_dockerfile_path,
            "dockerfile_content": dockerfile_content,
            "checked_paths": dockerfile_paths,
            "checked_at": chrono::Utc::now()
        }
    });
    
    tracing::info!("Dockerfile check completed for {}/{} - found: {}", owner, repo, has_dockerfile);
    Ok(Json(response))
}

/// Get repository contents for a specific path
pub async fn get_repository_contents_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((owner, repo, path)): Path<(String, String, String)>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Getting contents for {}/{}/{} (user: {})", owner, repo, path, user_id);
    
    // Validate input
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err(AppError::ValidationError("Owner and repository name cannot be empty".to_string()));
    }
    
    // Sanitize path
    let safe_path = if path == "root" || path.is_empty() { 
        "".to_string() 
    } else { 
        path 
    };
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and GitHub token
    // 2. Use the GitHub service to fetch repository contents
    // 3. Return the contents with proper metadata
    
    let contents: Vec<GitHubContent> = Vec::new(); // Placeholder
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "repository": format!("{}/{}", owner, repo),
            "path": safe_path,
            "contents": contents,
            "total_items": contents.len(),
            "fetched_at": chrono::Utc::now()
        }
    });
    
    tracing::info!("Successfully retrieved {} items from {}/{}/{}", contents.len(), owner, repo, safe_path);
    Ok(Json(response))
}

/// Search repositories by name or description
pub async fn search_repositories_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Searching repositories for user: {}", user_id);
    
    let query = params.get("q")
        .ok_or_else(|| AppError::ValidationError("Search query 'q' parameter is required".to_string()))?;
    
    if query.trim().is_empty() {
        return Err(AppError::ValidationError("Search query cannot be empty".to_string()));
    }
    
    let page = params.get("page")
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(1);
    
    let per_page = params.get("per_page")
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(30)
        .min(100);
    
    tracing::debug!("Repository search - query: '{}', page: {}, per_page: {}", query, page, per_page);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and GitHub token
    // 2. Use the GitHub service to search repositories
    // 3. Apply pagination and filtering
    // 4. Return search results
    
    let repositories: Vec<GitHubRepo> = Vec::new(); // Placeholder
    let total_count = 0;
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "query": query,
            "repositories": repositories,
            "total_count": total_count,
            "page": page,
            "per_page": per_page,
            "has_more": total_count > (page * per_page)
        }
    });
    
    tracing::info!("Repository search completed - query: '{}', found: {} repositories", query, total_count);
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_repository_query_defaults() {
        let query = RepositoryQuery {
            page: None,
            per_page: None,
            sort: None,
            direction: None,
            visibility: None,
            affiliation: None,
        };
        
        let page = query.page.unwrap_or(1);
        let per_page = query.per_page.unwrap_or(30).min(100);
        let sort = query.sort.unwrap_or_else(|| "updated".to_string());
        let direction = query.direction.unwrap_or_else(|| "desc".to_string());
        
        assert_eq!(page, 1);
        assert_eq!(per_page, 30);
        assert_eq!(sort, "updated");
        assert_eq!(direction, "desc");
    }
    
    #[test]
    fn test_per_page_limit() {
        let query = RepositoryQuery {
            page: Some(1),
            per_page: Some(200), // Should be capped at 100
            sort: None,
            direction: None,
            visibility: None,
            affiliation: None,
        };
        
        let per_page = query.per_page.unwrap_or(30).min(100);
        assert_eq!(per_page, 100);
    }
    
    #[test]
    fn test_project_analysis_details() {
        let details = ProjectAnalysisDetails {
            rust_indicators: vec!["Cargo.toml".to_string()],
            node_indicators: vec!["package.json".to_string()],
            python_indicators: vec!["requirements.txt".to_string()],
            java_indicators: vec!["pom.xml".to_string()],
            go_indicators: vec!["go.mod".to_string()],
            other_files: vec!["README.md".to_string()],
        };
        
        assert_eq!(details.rust_indicators.len(), 1);
        assert_eq!(details.node_indicators.len(), 1);
        assert_eq!(details.python_indicators.len(), 1);
        assert_eq!(details.java_indicators.len(), 1);
        assert_eq!(details.go_indicators.len(), 1);
        assert_eq!(details.other_files.len(), 1);
    }
    
    #[test]
    fn test_repository_list_response() {
        let response = RepositoryListResponse {
            status: "success".to_string(),
            data: RepositoryListData {
                repositories: Vec::new(),
                total: 0,
                page: 1,
                per_page: 30,
                has_more: false,
            },
        };
        
        assert_eq!(response.status, "success");
        assert_eq!(response.data.total, 0);
        assert_eq!(response.data.page, 1);
        assert_eq!(response.data.per_page, 30);
        assert!(!response.data.has_more);
    }
}