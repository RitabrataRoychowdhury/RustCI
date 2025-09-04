use crate::{
    core::networking::security::{JwtClaims, JwtManager, Role},
    domain::entities::{User, UserData, UserLoginResponse, UserResponse},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Query, State},
    http::{header, Response, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time;
use tracing::{error, info};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct OAuthQuery {
    code: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    pub status: String,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserProfile {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GitHubOAuthToken {
    access_token: String,
    token_type: String,
    scope: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GitHubUserResult {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
}

#[utoipa::path(get, path = "/api/sessions/oauth/google", tag = "oauth", responses())]
pub async fn google_oauth_handler() -> impl IntoResponse {
    let html_content = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DevOps CI - OAuth Login</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .container {
            background: white;
            padding: 2rem;
            border-radius: 10px;
            box-shadow: 0 10px 25px rgba(0,0,0,0.1);
            text-align: center;
            max-width: 400px;
            width: 90%;
        }
        h1 {
            color: #333;
            margin-bottom: 0.5rem;
        }
        .subtitle {
            color: #666;
            margin-bottom: 2rem;
        }
        .oauth-button {
            display: inline-flex;
            align-items: center;
            padding: 12px 24px;
            background-color: #24292e;
            color: white;
            text-decoration: none;
            border-radius: 6px;
            font-weight: 500;
            transition: background-color 0.2s;
            margin: 10px;
        }
        .oauth-button:hover {
            background-color: #1a1e22;
        }
        .github-icon {
            margin-right: 8px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 DevOps CI</h1>
        <p class="subtitle">Secure OAuth Authentication</p>
        <p>Choose your authentication provider:</p>
        <a href="/api/sessions/oauth/github" class="oauth-button">
            <svg class="github-icon" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
                <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.012 8.012 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>
            </svg>
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

#[utoipa::path(get, path = "/api/sessions/oauth/github", tag = "oauth", responses())]
pub async fn github_oauth_handler(State(data): State<AppState>) -> impl IntoResponse {
    let state = uuid::Uuid::new_v4().to_string();
    let github_auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email&state={}",
        data.env.security.oauth.github.client_id,
        urlencoding::encode(&data.env.security.oauth.github.redirect_url),
        state
    );

    info!("🔗 Redirecting to GitHub OAuth: {}", github_auth_url);
    Redirect::to(&github_auth_url)
}

#[utoipa::path(
    get,
    path = "/api/sessions/oauth/github/callback",
    tag = "oauth",
    params(),
    responses()
)]
pub async fn github_oauth_callback(
    Query(query): Query<OAuthQuery>,
    State(data): State<AppState>,
) -> Result<impl IntoResponse> {
    info!("📥 GitHub OAuth callback received");

    let github_token = request_github_token(&query.code, &data).await?;
    let github_user = get_github_user(&github_token.access_token).await?;

    info!("✅ GitHub user authenticated: {}", github_user.login);

    // Find or create user in MongoDB
    let user = data.db.find_or_create_oauth_user(&github_user).await?;

    // Create or update workspace for the user
    let workspace_result =
        create_or_update_workspace(&user, &github_token.access_token, &data).await;
    match workspace_result {
        Ok(workspace_id) => {
            info!("✅ Workspace created/updated: {}", workspace_id);
        }
        Err(e) => {
            // Log the error but don't fail the authentication
            error!("⚠️ Failed to create/update workspace: {}", e);
        }
    }

    // Create JWT with enhanced security and RBAC
    let jwt_manager = JwtManager::new(
        data.env.security.jwt.secret.clone(),
        data.env.security.jwt.expires_in_seconds,
    );

    let claims = JwtClaims::new(
        user.id,
        user.email.clone(),
        vec![Role::Developer], // Default role for OAuth users
        data.env.security.jwt.expires_in_seconds,
    );

    let token = jwt_manager.create_token(&claims)?;

    let cookie = Cookie::build(("token", token.clone()))
        .path("/")
        .max_age(time::Duration::seconds(
            data.env.security.jwt.expires_in_seconds,
        ))
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

    info!("✅ User successfully authenticated and token generated");
    Ok(response)
}

async fn request_github_token(
    authorization_code: &str,
    data: &AppState,
) -> Result<GitHubOAuthToken> {
    info!("🔄 Requesting GitHub access token");

    let client = reqwest::Client::new();

    let params = [
        (
            "client_id",
            data.env.security.oauth.github.client_id.as_str(),
        ),
        (
            "client_secret",
            data.env.security.oauth.github.client_secret.as_str(),
        ),
        ("code", authorization_code),
    ];

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "DevOps-CI/1.0")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            error!("❌ Failed to request GitHub token: {}", e);
            AppError::ExternalServiceError(format!("Failed to request GitHub token: {}", e))
        })?;

    if !response.status().is_success() {
        error!(
            "❌ GitHub token request failed with status: {}",
            response.status()
        );
        return Err(AppError::ExternalServiceError(
            "Failed to get access token from GitHub".to_string(),
        ));
    }

    let oauth_response = response.json::<GitHubOAuthToken>().await.map_err(|e| {
        error!("❌ Failed to parse GitHub token response: {}", e);
        AppError::ExternalServiceError(format!("Failed to parse GitHub token response: {}", e))
    })?;

    info!("✅ Successfully obtained GitHub access token");
    Ok(oauth_response)
}

async fn get_github_user(access_token: &str) -> Result<GitHubUserResult> {
    info!("🔄 Fetching GitHub user information");

    let client = reqwest::Client::new();

    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "DevOps-CI/1.0")
        .send()
        .await
        .map_err(|e| {
            error!("❌ Failed to get GitHub user: {}", e);
            AppError::ExternalServiceError(format!("Failed to get GitHub user: {}", e))
        })?;

    if !response.status().is_success() {
        error!(
            "❌ GitHub user request failed with status: {}",
            response.status()
        );
        return Err(AppError::ExternalServiceError(
            "Failed to get user info from GitHub".to_string(),
        ));
    }

    let user_info = response.json::<GitHubUserResult>().await.map_err(|e| {
        error!("❌ Failed to parse GitHub user response: {}", e);
        AppError::ExternalServiceError(format!("Failed to parse GitHub user response: {}", e))
    })?;

    info!("✅ Successfully fetched GitHub user: {}", user_info.login);
    Ok(user_info)
}

#[utoipa::path(
    get,
    path = "/api/sessions/me",
    tag = "auth",
    security(
        ("jwt_auth" = [])
    ),
    responses(
    )
)]
pub async fn get_me_handler(
    user_id: uuid::Uuid,
    State(data): State<AppState>,
) -> Result<impl IntoResponse> {
    info!("🔄 Fetching user profile for ID: {}", user_id);

    // Find user in MongoDB by UUID
    // Convert UUID to string for MongoDB query (serde serializes UUID as string by default)
    let user_id_str = user_id.to_string();
    let user = data
        .db
        .database
        .collection::<User>("users")
        .find_one(mongodb::bson::doc! {"id": user_id_str}, None)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to find user: {}", e)))?;

    match user {
        Some(user) => {
            info!("✅ Found user: {}", user.email);
            let json_response = UserResponse {
                status: "success".to_string(),
                data: UserData {
                    user: user.filter_user(),
                },
            };
            Ok(Json(json_response))
        }
        None => {
            error!("❌ User not found with ID: {}", user_id);
            Err(AppError::NotFound("User not found".to_string()))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/sessions/logout",
    tag = "auth",
    security(
        ("jwt_auth" = [])
    ),
    responses(
    )
)]
pub async fn logout_handler() -> Result<impl IntoResponse> {
    info!("🔄 User logout requested");

    let cookie = Cookie::build(("token", ""))
        .path("/")
        .max_age(time::Duration::seconds(-1))
        .same_site(SameSite::Lax)
        .http_only(true);

    let mut response =
        Json(json!({"status": "success", "message": "Successfully logged out"})).into_response();

    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());

    info!("✅ User successfully logged out");
    Ok(response)
}

/// Create or update workspace for authenticated user
async fn create_or_update_workspace(
    user: &User,
    github_token: &str,
    _data: &AppState,
) -> Result<uuid::Uuid> {
    info!("🏗️ Creating/updating workspace for user: {}", user.email);

    // In a real implementation, this would:
    // 1. Check if workspace already exists for the user
    // 2. Create new workspace or update existing one
    // 3. Encrypt and store the GitHub token
    // 4. Set up default workspace settings
    // 5. Initialize workspace with user's repositories

    // For now, simulate workspace creation
    let workspace_id = uuid::Uuid::new_v4();

    // Simulate encrypted token storage (in real implementation, use proper encryption)
    let _encrypted_token = format!("encrypted_{}", github_token.len()); // Placeholder

    info!(
        "✅ Workspace created/updated successfully: {}",
        workspace_id
    );
    Ok(workspace_id)
}
#
[utoipa::path(
    post,
    path = "/api/sessions/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
    )
)]
pub async fn login(_request: Json<LoginRequest>) -> Result<impl IntoResponse> {
    // This is a placeholder for email/password login
    // Currently only OAuth is implemented
    Ok(Json(json!({
        "status": "error",
        "message": "Email/password login not implemented"
    })))
}

// Functions are already public and available for import
