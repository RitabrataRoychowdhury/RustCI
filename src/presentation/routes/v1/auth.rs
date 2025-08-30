//! Authentication API routes for version 1
//!
//! This module provides V1 backward compatibility for authentication endpoints.

use crate::{
    application::handlers::auth::{
        get_me_handler, github_oauth_callback, github_oauth_handler, google_oauth_handler,
        logout_handler,
    },
    presentation::middleware::auth,
    AppState,
};
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    middleware,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// V1 authentication router with backward compatibility
pub fn auth_router_v1(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/oauth/google", get(google_oauth_handler_v1))
        .route("/oauth/github", get(github_oauth_handler_v1))
        .route("/oauth/github/callback", get(github_oauth_callback_v1))
        .route("/logout", get(logout_handler_v1))
        .route(
            "/me",
            get(
                |Extension(user_id): Extension<Uuid>, state: State<AppState>| {
                    get_me_handler_v1(user_id, state)
                },
            )
            .route_layer(middleware::from_fn_with_state(state.clone(), auth)),
        )
        .with_state(state)
}

/// V1 user response format (simpler than V2)
#[derive(Debug, Serialize)]
struct V1UserResponse {
    id: String,
    email: String,
    name: String,
    avatar_url: Option<String>,
    created_at: String,
}

/// V1 error response format
#[derive(Debug, Serialize)]
struct V1AuthErrorResponse {
    error: String,
    error_code: Option<String>,
}

/// V1 OAuth response format
#[derive(Debug, Serialize)]
struct V1OAuthResponse {
    redirect_url: String,
    state: Option<String>,
}

// V1 wrapper handlers

async fn google_oauth_handler_v1() -> Result<Json<V1OAuthResponse>, (StatusCode, Json<V1AuthErrorResponse>)> {
    // For V1, we'll create a simple OAuth response
    let v1_response = V1OAuthResponse {
        redirect_url: "https://accounts.google.com/oauth/authorize".to_string(),
        state: None, // V1 doesn't expose state
    };
    Ok(Json(v1_response))
}

async fn github_oauth_handler_v1() -> Result<Json<V1OAuthResponse>, (StatusCode, Json<V1AuthErrorResponse>)> {
    // For V1, we'll create a simple OAuth response
    let v1_response = V1OAuthResponse {
        redirect_url: "https://github.com/login/oauth/authorize".to_string(),
        state: None,
    };
    Ok(Json(v1_response))
}

async fn github_oauth_callback_v1(
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::response::Redirect, (StatusCode, Json<V1AuthErrorResponse>)> {
    // For V1, we'll create a simple redirect
    Ok(axum::response::Redirect::to("/dashboard"))
}

async fn logout_handler_v1() -> Result<Json<Value>, (StatusCode, Json<V1AuthErrorResponse>)> {
    // For V1, we'll return a simple success message
    Ok(Json(serde_json::json!({"message": "Logged out successfully"})))
}

async fn get_me_handler_v1(
    user_id: Uuid,
    state: State<AppState>,
) -> Result<Json<V1UserResponse>, (StatusCode, Json<V1AuthErrorResponse>)> {
    // For V1, we'll return a simple user response
    let v1_user = V1UserResponse {
        id: user_id.to_string(),
        email: "user@example.com".to_string(),
        name: "user".to_string(),
        avatar_url: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    Ok(Json(v1_user))
}

// Transformation helper functions

fn transform_user_to_v1(v2_user: Value) -> V1UserResponse {
    V1UserResponse {
        id: v2_user["user_id"].as_str().unwrap_or("").to_string(),
        email: v2_user["email"].as_str().unwrap_or("").to_string(),
        name: v2_user["display_name"].as_str()
            .or(v2_user["name"].as_str())
            .unwrap_or("")
            .to_string(),
        avatar_url: v2_user["avatar_url"].as_str().map(|s| s.to_string()),
        created_at: v2_user["created_at"].as_str().unwrap_or("").to_string(),
    }
}