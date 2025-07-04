use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, Redirect},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{info, error, debug};

use crate::{
    dto::{AuthResponse, GitHubRepo},
    services::oauth_service::GitHubOAuthService,
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct GitHubCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RepoParams {
    pub owner: String,
    pub repo: String,
}

/// Initiate GitHub OAuth flow
pub async fn github_login(State(state): State<AppState>) -> Result<Redirect, StatusCode> {
    let oauth_state = Uuid::new_v4().to_string();
    
    // In production, store this state in Redis or a secure session store
    debug!("Generated OAuth state: {}", oauth_state);
    
    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=repo,user:email&state={}",
        state.config.github_client_id,
        urlencoding::encode(&state.config.github_redirect_uri),
        oauth_state
    );
    
    info!("🔗 Redirecting to GitHub OAuth: {}", auth_url);
    Ok(Redirect::to(&auth_url))
}

/// Handle GitHub OAuth callback
pub async fn github_callback(
    Query(params): Query<GitHubCallbackQuery>,
    State(state): State<AppState>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("GitHub callback received with params: {:?}", params);
    
    // Check for OAuth error
    if let Some(error) = params.error {
        error!("GitHub OAuth error: {}", error);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "oauth_error".to_string(),
                message: format!("GitHub OAuth error: {}", error),
            }),
        ));
    }

    // Extract authorization code
    let code = params.code.ok_or_else(|| {
        error!("Missing authorization code in callback");
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "missing_code".to_string(),
                message: "Authorization code is required".to_string(),
            }),
        )
    })?;

    // TODO: Validate state parameter to prevent CSRF attacks
    if let Some(state_param) = &params.state {
        debug!("Received state parameter: {}", state_param);
        // In production, validate this against stored state
    }

    // Exchange code for access token and get user info
    let oauth_service = GitHubOAuthService::new(&state.http_client, &state.config);
    
    match oauth_service.exchange_code_for_user(code).await {
        Ok(user) => {
            info!("✅ Successfully authenticated user: {} (ID: {})", user.login, user.id);
            Ok(Json(AuthResponse {
                user,
                message: "Successfully authenticated with GitHub".to_string(),
            }))
        }
        Err(e) => {
            error!("❌ Failed to exchange code for user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "authentication_failed".to_string(),
                    message: "Failed to authenticate with GitHub".to_string(),
                }),
            ))
        }
    }
}

/// Get current user info (requires authentication)
pub async fn get_user(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<crate::domain::User>, (StatusCode, Json<ErrorResponse>)> {
    let access_token = params.get("access_token").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing_token".to_string(),
                message: "Access token is required. Use: /auth/user?access_token=YOUR_TOKEN".to_string(),
            }),
        )
    })?;

    debug!("Getting user info for token: {}...", &access_token[..8]);

    let oauth_service = GitHubOAuthService::new(&state.http_client, &state.config);
    
    match oauth_service.get_user_info(access_token).await {
        Ok(user_info) => {
            let user = crate::domain::User::new(
                user_info.id,
                user_info.login.clone(),
                user_info.name,
                user_info.email,
                user_info.avatar_url,
                user_info.html_url,
                access_token.clone(),
            );
            info!("✅ Retrieved user info for: {}", user_info.login);
            Ok(Json(user))
        }
        Err(e) => {
            error!("❌ Failed to get user info: {}", e);
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "invalid_token".to_string(),
                    message: "Invalid or expired access token".to_string(),
                }),
            ))
        }
    }
}

/// Get user's repositories
pub async fn get_user_repos(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<GitHubRepo>>, (StatusCode, Json<ErrorResponse>)> {
    let access_token = params.get("access_token").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing_token".to_string(),
                message: "Access token is required. Use: /api/repos?access_token=YOUR_TOKEN".to_string(),
            }),
        )
    })?;

    let per_page = params
        .get("per_page")
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(30);

    debug!("Getting repositories for token: {}... (per_page: {})", &access_token[..8], per_page);

    let oauth_service = GitHubOAuthService::new(&state.http_client, &state.config);
    
    match oauth_service.get_user_repositories(access_token, Some(per_page)).await {
        Ok(repos) => {
            info!("✅ Retrieved {} repositories", repos.len());
            Ok(Json(repos))
        }
        Err(e) => {
            error!("❌ Failed to get repositories: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "fetch_repos_failed".to_string(),
                    message: "Failed to fetch repositories from GitHub".to_string(),
                }),
            ))
        }
    }
}

/// Get specific repository information
pub async fn get_repo_info(
    Path(RepoParams { owner, repo }): Path<RepoParams>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GitHubRepo>, (StatusCode, Json<ErrorResponse>)> {
    let access_token = params.get("access_token").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing_token".to_string(),
                message: "Access token is required. Use: /api/repos/owner/repo?access_token=YOUR_TOKEN".to_string(),
            }),
        )
    })?;

    debug!("Getting repository info for {}/{} with token: {}...", owner, repo, &access_token[..8]);

    let oauth_service = GitHubOAuthService::new(&state.http_client, &state.config);
    
    match oauth_service.get_repository_info(access_token, &owner, &repo).await {
        Ok(repo_info) => {
            info!("✅ Retrieved repository info for {}/{}", owner, repo);
            Ok(Json(repo_info))
        }
        Err(e) => {
            error!("❌ Failed to get repository {}/{}: {}", owner, repo, e);
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "repo_not_found".to_string(),
                    message: format!("Repository {}/{} not found or access denied", owner, repo),
                }),
            ))
        }
    }
}