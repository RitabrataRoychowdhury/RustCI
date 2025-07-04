use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubTokenRequest {
    pub client_id: String,
    pub client_secret: String,
    pub code: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
    pub visibility: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub private: bool,
    pub fork: bool,
    pub archived: bool,
    pub disabled: bool,
    pub default_branch: String,
    pub language: Option<String>,
    pub stargazers_count: u32,
    pub watchers_count: u32,
    pub forks_count: u32,
    pub open_issues_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub pushed_at: Option<String>,
    pub owner: GitHubRepoOwner,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRepoOwner {
    pub id: u64,
    pub login: String,
    pub avatar_url: String,
    pub html_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub user: crate::domain::User,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub name: String,
    pub active: bool,
    pub events: Vec<String>,
    pub config: WebhookConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub content_type: String,
    pub insecure_ssl: String,
    pub secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubWebhook {
    pub id: u64,
    pub name: String,
    pub active: bool,
    pub events: Vec<String>,
    pub config: WebhookConfig,
    pub created_at: String,
    pub updated_at: String,
}