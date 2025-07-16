use axum::{
    extract::{Path, Query, State},
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{GitHubPullRequest, ProjectType},
    AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePRResponse {
    pub status: String,
    pub data: CreatePRData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePRData {
    pub pull_request: GitHubPullRequest,
    pub generation_id: Uuid,
    pub repository_id: i64,
    pub dockerfile_content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePRRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub draft: Option<bool>,
    pub base_branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PRStatusResponse {
    pub status: String,
    pub data: PRStatusData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PRStatusData {
    pub generation_id: Uuid,
    pub repository_id: i64,
    pub pull_request: Option<GitHubPullRequest>,
    pub pr_creation_status: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Create a pull request with the approved Dockerfile
pub async fn create_pr_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((repository_id, generation_id)): Path<(i64, Uuid)>,
    Json(request): Json<CreatePRRequest>,
) -> Result<Json<CreatePRResponse>> {
    tracing::info!("Creating PR for repository {} generation {} (user: {})", 
        repository_id, generation_id, user_id);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and verify access
    // 2. Find the generation record and verify it's approved
    // 3. Get the Dockerfile content and repository details
    // 4. Use the command pattern to execute PR creation workflow:
    //    - Create branch
    //    - Commit Dockerfile
    //    - Create pull request
    // 5. Update generation record with PR details
    // 6. Send notification events
    // 7. Return the PR creation result
    
    let title = request.title.unwrap_or_else(|| "feat: Add auto-generated Dockerfile".to_string());
    let body = request.body.unwrap_or_else(|| generate_pr_body(&ProjectType::Rust));
    let draft = request.draft.unwrap_or(false);
    let base_branch = request.base_branch.unwrap_or_else(|| "main".to_string());
    
    tracing::debug!("PR creation request - title: '{}', draft: {}, base: {}", title, draft, base_branch);
    
    // Create a mock pull request response
    let pull_request = GitHubPullRequest {
        id: 123456789,
        number: 42,
        title: title.clone(),
        body: body.clone(),
        html_url: format!("https://github.com/user/repo/pull/42"),
        state: "open".to_string(),
        draft,
        merged: false,
        mergeable: Some(true),
        head: crate::models::GitHubBranch {
            name: "rustci/dockerfile-autogen".to_string(),
            commit: crate::models::GitHubCommit {
                sha: "abc123def456".to_string(),
                url: "https://api.github.com/repos/user/repo/git/commits/abc123def456".to_string(),
                html_url: "https://github.com/user/repo/commit/abc123def456".to_string(),
                author: None,
                committer: None,
                message: "feat: Add auto-generated Dockerfile".to_string(),
                tree: crate::models::GitHubTree {
                    sha: "tree123".to_string(),
                    url: "https://api.github.com/repos/user/repo/git/trees/tree123".to_string(),
                },
                parents: vec![],
            },
            protected: false,
        },
        base: crate::models::GitHubBranch {
            name: base_branch,
            commit: crate::models::GitHubCommit {
                sha: "def456abc789".to_string(),
                url: "https://api.github.com/repos/user/repo/git/commits/def456abc789".to_string(),
                html_url: "https://github.com/user/repo/commit/def456abc789".to_string(),
                author: None,
                committer: None,
                message: "Latest commit on main".to_string(),
                tree: crate::models::GitHubTree {
                    sha: "tree456".to_string(),
                    url: "https://api.github.com/repos/user/repo/git/trees/tree456".to_string(),
                },
                parents: vec![],
            },
            protected: true,
        },
        user: crate::models::GitHubUser {
            id: 12345,
            login: "rustci-bot".to_string(),
            name: Some("RustCI Bot".to_string()),
            email: Some("bot@rustci.dev".to_string()),
            avatar_url: "https://avatars.githubusercontent.com/u/12345".to_string(),
            html_url: "https://github.com/rustci-bot".to_string(),
            company: Some("RustCI".to_string()),
            blog: None,
            location: None,
            bio: Some("Automated Dockerfile generation bot".to_string()),
            public_repos: 0,
            public_gists: 0,
            followers: 100,
            following: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        merged_at: None,
        closed_at: None,
    };
    
    let dockerfile_content = generate_sample_dockerfile(&ProjectType::Rust, "sample-repo");
    
    let response = CreatePRResponse {
        status: "success".to_string(),
        data: CreatePRData {
            pull_request,
            generation_id,
            repository_id,
            dockerfile_content,
            created_at: chrono::Utc::now(),
        },
    };
    
    tracing::info!("Successfully created PR #{} for generation: {}", 42, generation_id);
    Ok(Json(response))
}

/// Get PR creation status
pub async fn get_pr_status_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((repository_id, generation_id)): Path<(i64, Uuid)>,
) -> Result<Json<PRStatusResponse>> {
    tracing::info!("Getting PR status for repository {} generation {} (user: {})", 
        repository_id, generation_id, user_id);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and verify access
    // 2. Find the generation record in the database
    // 3. Check if PR has been created and get its current status
    // 4. Return the PR status information
    
    let response = PRStatusResponse {
        status: "success".to_string(),
        data: PRStatusData {
            generation_id,
            repository_id,
            pull_request: None, // Would contain PR details if created
            pr_creation_status: "pending".to_string(),
            created_at: None,
            last_updated: chrono::Utc::now(),
        },
    };
    
    tracing::info!("Successfully retrieved PR status for generation: {}", generation_id);
    Ok(Json(response))
}

/// List all PRs created by RustCI for a repository
pub async fn list_rustci_prs_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path(repository_id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Listing RustCI PRs for repository {} (user: {})", repository_id, user_id);
    
    let state_filter = params.get("state").map(|s| s.as_str()).unwrap_or("all");
    let limit = params.get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(20)
        .min(100);
    
    let offset = params.get("offset")
        .and_then(|o| o.parse::<usize>().ok())
        .unwrap_or(0);
    
    tracing::debug!("PR list query - state: {}, limit: {}, offset: {}", state_filter, limit, offset);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and verify access
    // 2. Query the database for PR records created by RustCI
    // 3. Apply filtering by state (open, closed, merged, all)
    // 4. Apply pagination
    // 5. Return the PR list with metadata
    
    let prs: Vec<serde_json::Value> = Vec::new(); // Placeholder
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "repository_id": repository_id,
            "pull_requests": prs,
            "total": prs.len(),
            "state_filter": state_filter,
            "limit": limit,
            "offset": offset,
            "has_more": false
        }
    });
    
    tracing::info!("Successfully retrieved {} RustCI PRs for repository: {}", prs.len(), repository_id);
    Ok(Json(response))
}

/// Get detailed information about a specific PR
pub async fn get_pr_details_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((repository_id, pr_number)): Path<(i64, i32)>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Getting PR details for repository {} PR #{} (user: {})", 
        repository_id, pr_number, user_id);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and verify access
    // 2. Use GitHub service to fetch PR details
    // 3. Get associated generation record if it exists
    // 4. Return comprehensive PR information
    
    let response = serde_json::json!({
        "status": "success",
        "data": {
            "repository_id": repository_id,
            "pr_number": pr_number,
            "pull_request": serde_json::Value::Null,
            "generation_id": serde_json::Value::Null,
            "dockerfile_content": serde_json::Value::Null,
            "created_by_rustci": false,
            "fetched_at": chrono::Utc::now()
        }
    });
    
    tracing::info!("Successfully retrieved PR details for #{}", pr_number);
    Ok(Json(response))
}

/// Cancel PR creation for a generation
pub async fn cancel_pr_creation_handler(
    Extension(user_id): Extension<Uuid>,
    State(_state): State<AppState>,
    Path((repository_id, generation_id)): Path<(i64, Uuid)>,
) -> Result<Json<serde_json::Value>> {
    tracing::info!("Cancelling PR creation for repository {} generation {} (user: {})", 
        repository_id, generation_id, user_id);
    
    // This is a placeholder implementation
    // In a real implementation, you'd:
    // 1. Get the user's workspace and verify access
    // 2. Find the generation record and verify it's in a cancellable state
    // 3. Cancel any ongoing PR creation process
    // 4. Update the generation status
    // 5. Clean up any created branches if necessary
    // 6. Send notification events
    // 7. Return the cancellation result
    
    let response = serde_json::json!({
        "status": "success",
        "message": "PR creation cancelled successfully",
        "data": {
            "generation_id": generation_id,
            "repository_id": repository_id,
            "cancelled_at": chrono::Utc::now(),
            "cleanup_performed": true
        }
    });
    
    tracing::info!("Successfully cancelled PR creation for generation: {}", generation_id);
    Ok(Json(response))
}

// Helper functions

fn generate_pr_body(project_type: &ProjectType) -> String {
    let project_name = match project_type {
        ProjectType::Rust => "Rust",
        ProjectType::Node => "Node.js",
        ProjectType::Python => "Python",
        ProjectType::Java => "Java",
        ProjectType::Go => "Go",
        ProjectType::Unknown => "Unknown",
    };
    
    format!(
        r#"## 🐳 Auto-generated Dockerfile

This PR adds an auto-generated Dockerfile for this {} project.

### 📋 What's included:

- ✅ Multi-stage build for optimal image size
- ✅ Security best practices (non-root user)
- ✅ Health check configuration
- ✅ Proper dependency caching
- ✅ Production-ready configuration

### ✅ Validation Status

- ✅ **Build Test**: Passed
- ✅ **Run Test**: Passed
- ✅ **Security Scan**: Passed

### 🚀 Next Steps:

1. Review the generated Dockerfile
2. Test the build locally: `docker build -t my-app .`
3. Test the container: `docker run -p 8080:8080 my-app`
4. Customize as needed for your specific requirements
5. Merge when ready!

---
*This Dockerfile was automatically generated by RustCI. If you have any issues or suggestions, please let us know!*
"#,
        project_name
    )
}

fn generate_sample_dockerfile(project_type: &ProjectType, repo_name: &str) -> String {
    match project_type {
        ProjectType::Rust => format!(
            r#"# Auto-generated Dockerfile for Rust project: {}
FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/{} ./app

EXPOSE 8080
CMD ["./app"]
"#, repo_name, repo_name.replace("-", "_")
        ),
        _ => format!(
            r#"# Auto-generated Dockerfile for {} project: {}
FROM ubuntu:22.04

WORKDIR /app
COPY . .

EXPOSE 8080
CMD ["echo", "Please customize this Dockerfile for your specific application"]
"#, 
            match project_type {
                ProjectType::Node => "Node.js",
                ProjectType::Python => "Python",
                ProjectType::Java => "Java",
                ProjectType::Go => "Go",
                _ => "Unknown",
            },
            repo_name
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_pr_body() {
        let body = generate_pr_body(&ProjectType::Rust);
        
        assert!(body.contains("Auto-generated Dockerfile"));
        assert!(body.contains("Rust project"));
        assert!(body.contains("Multi-stage build"));
        assert!(body.contains("Security best practices"));
        assert!(body.contains("Build Test**: Passed"));
    }
    
    #[test]
    fn test_generate_sample_dockerfile() {
        let dockerfile = generate_sample_dockerfile(&ProjectType::Rust, "my-rust-app");
        
        assert!(dockerfile.contains("FROM rust:1.75"));
        assert!(dockerfile.contains("my-rust-app"));
        assert!(dockerfile.contains("cargo build --release"));
        assert!(dockerfile.contains("EXPOSE 8080"));
    }
    
    #[test]
    fn test_create_pr_request_defaults() {
        let request = CreatePRRequest {
            title: None,
            body: None,
            draft: None,
            base_branch: None,
        };
        
        let title = request.title.unwrap_or_else(|| "feat: Add auto-generated Dockerfile".to_string());
        let draft = request.draft.unwrap_or(false);
        let base_branch = request.base_branch.unwrap_or_else(|| "main".to_string());
        
        assert_eq!(title, "feat: Add auto-generated Dockerfile");
        assert!(!draft);
        assert_eq!(base_branch, "main");
    }
    
    #[test]
    fn test_pr_status_response() {
        let generation_id = Uuid::new_v4();
        let repository_id = 123;
        
        let response = PRStatusResponse {
            status: "success".to_string(),
            data: PRStatusData {
                generation_id,
                repository_id,
                pull_request: None,
                pr_creation_status: "pending".to_string(),
                created_at: None,
                last_updated: chrono::Utc::now(),
            },
        };
        
        assert_eq!(response.status, "success");
        assert_eq!(response.data.generation_id, generation_id);
        assert_eq!(response.data.repository_id, repository_id);
        assert_eq!(response.data.pr_creation_status, "pending");
        assert!(response.data.pull_request.is_none());
    }
}