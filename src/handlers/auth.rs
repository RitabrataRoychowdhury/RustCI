use crate::{
    error::{AppError, Result},
    models::{FilteredUser, User, UserData, UserLoginResponse, UserResponse},
    token::generate_jwt_token,
    AppState,
};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct OAuthQuery {
    code: String,
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubOAuthToken {
    access_token: String,
    token_type: String,
    scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUserResult {
    id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: String,
}

pub async fn google_oauth_handler() -> impl IntoResponse {
    let html_content = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OAuth Login</title>
</head>
<body>
    <div style="max-width: 400px; margin: 100px auto; text-align: center; font-family: Arial, sans-serif;">
        <h1>OAuth Authentication</h1>
        <p>Choose your authentication provider:</p>
        <a href="/api/sessions/oauth/github" 
           style="display: inline-block; padding: 10px 20px; background-color: #333; color: white; text-decoration: none; border-radius: 5px; margin: 10px;">
            Login with GitHub
        </a>
    </div>
</body>
</html>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(html_content.to_string())
        .unwrap()
}

pub async fn github_oauth_handler(State(data): State<AppState>) -> impl IntoResponse {
    let github_auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email",
        data.env.github_oauth_client_id, data.env.github_oauth_redirect_url
    );

    Redirect::to(&github_auth_url)
}

pub async fn github_oauth_callback(
    Query(query): Query<OAuthQuery>,
    State(data): State<AppState>,
) -> Result<impl IntoResponse> {
    let github_token = request_github_token(&query.code, &data).await?;
    let github_user = get_github_user(&github_token.access_token).await?;

    let user = User {
        id: Uuid::new_v4(),
        email: github_user.email.unwrap_or_default(),
        name: github_user.name.unwrap_or(github_user.login),
        photo: github_user.avatar_url,
        verified: true,
        provider: "GitHub".to_string(),
        role: "user".to_string(),
        created_at: Some(chrono::Utc::now()),
        updated_at: Some(chrono::Utc::now()),
    };

    let token = generate_jwt_token(
        user.id,
        data.env.jwt_secret.clone(),
        data.env.jwt_expires_in.clone(),
    )?;

    let cookie = Cookie::build(("token", token.clone()))
        .path("/")
        .max_age(time::Duration::seconds(data.env.jwt_maxage as i64))
        .same_site(SameSite::Lax)
        .http_only(true);

    let mut response = Json(UserLoginResponse {
        status: "success".to_string(),
        token,
    })
    .into_response();

    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());

    Ok(response)
}

async fn request_github_token(
    authorization_code: &str,
    data: &AppState,
) -> Result<GitHubOAuthToken> {
    let client = reqwest::Client::new();

    let params = [
        ("client_id", data.env.github_oauth_client_id.as_str()),
        ("client_secret", data.env.github_oauth_client_secret.as_str()),
        ("code", authorization_code),
    ];

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::ExternalServiceError(format!("Failed to request GitHub token: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::ExternalServiceError(
            "Failed to get access token from GitHub".to_string(),
        ));
    }

    let oauth_response = response
        .json::<GitHubOAuthToken>()
        .await
        .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse GitHub token response: {}", e)))?;

    Ok(oauth_response)
}

async fn get_github_user(access_token: &str) -> Result<GitHubUserResult> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "rust-oauth-app")
        .send()
        .await
        .map_err(|e| AppError::ExternalServiceError(format!("Failed to get GitHub user: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::ExternalServiceError(
            "Failed to get user info from GitHub".to_string(),
        ));
    }

    let user_info = response
        .json::<GitHubUserResult>()
        .await
        .map_err(|e| AppError::ExternalServiceError(format!("Failed to parse GitHub user response: {}", e)))?;

    Ok(user_info)
}

pub async fn get_me_handler(user_id: uuid::Uuid) -> Result<impl IntoResponse> {
    // In a real application, you would fetch the user from the database
    // For now, we'll create a mock user
    let user = User {
        id: user_id,
        name: "Mock User".to_string(),
        email: "mock@example.com".to_string(),
        photo: "https://via.placeholder.com/150".to_string(),
        verified: true,
        provider: "GitHub".to_string(),
        role: "user".to_string(),
        created_at: Some(chrono::Utc::now()),
        updated_at: Some(chrono::Utc::now()),
    };

    let json_response = UserResponse {
        status: "success".to_string(),
        data: UserData {
            user: user.filter_user(),
        },
    };

    Ok(Json(json_response))
}

pub async fn logout_handler() -> Result<impl IntoResponse> {
    let cookie = Cookie::build(("token", ""))
        .path("/")
        .max_age(time::Duration::seconds(-1))
        .same_site(SameSite::Lax)
        .http_only(true);

    let mut response = Json(json!({"status": "success"})).into_response();

    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());

    Ok(response)
}