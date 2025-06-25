use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Json, Redirect},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    dto::{AuthResponse, OAuthState},
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

/// Initiate GitHub OAuth flow
pub async fn github_login(State(state): State<AppState>) -> Result<Redirect, StatusCode> {
    let oauth_state = Uuid::new_v4().to_string();
    
    // In a real app, you'd store this state temporarily (Redis, in-memory cache, etc.)
    // For now, we'll just generate it and validate later
    
    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=repo,user:email&state={}",
        state.config.github_client_id,
        urlencoding::encode(&state.config.github_redirect_uri),
        oauth_state
    );
    
    tracing::info!("Redirecting to GitHub OAuth: {}", auth_url);
    Ok(Redirect::to(&auth_url))
}

/// Handle GitHub OAuth callback
pub async fn github_callback(
    Query(params): Query<GitHubCallbackQuery>,
    State(state): State<AppState>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check for OAuth error
    if let Some(error) = params.error {
        tracing::error!("GitHub OAuth error: {}", error);
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
        tracing::error!("Missing authorization code in callback");
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "missing_code".to_string(),
                message: "Authorization code is required".to_string(),
            }),
        )
    })?;

    // TODO: Validate state parameter to prevent CSRF attacks
    // For now, we'll skip this validation but it's important for production

    // Exchange code for access token and get user info
    let oauth_service = GitHubOAuthService::new(&state.http_client, &state.config);
    
    match oauth_service.exchange_code_for_user(code).await {
        Ok(user) => {
            tracing::info!("Successfully authenticated user: {}", user.login);
            Ok(Json(AuthResponse {
                user,
                message: "Successfully authenticated with GitHub".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to exchange code for user: {}", e);
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
    // In a real app, you'd extract the access token from headers/cookies
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<crate::domain::User>, (StatusCode, Json<ErrorResponse>)> {
    let access_token = params.get("access_token").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing_token".to_string(),
                message: "Access token is required".to_string(),
            }),
        )
    })?;

    let oauth_service = GitHubOAuthService::new(&state.http_client, &state.config);
    
    match oauth_service.get_user_info(access_token).await {
        Ok(user_info) => {
            let user = crate::domain::User::new(
                user_info.id,
                user_info.login,
                user_info.name,
                user_info.email,
                user_info.avatar_url,
                user_info.html_url,
                access_token.clone(),
            );
            Ok(Json(user))
        }
        Err(e) => {
            tracing::error!("Failed to get user info: {}", e);
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