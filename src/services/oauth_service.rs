use reqwest::Client;
use serde_json::json;

use crate::{
    config::Config,
    domain::User,
    dto::{GitHubEmail, GitHubTokenRequest, GitHubTokenResponse, GitHubUser},
    infrastructure::github_client::GitHubClient,
};

pub struct GitHubOAuthService<'a> {
    client: &'a Client,
    config: &'a Config,
    github_client: GitHubClient<'a>,
}

impl<'a> GitHubOAuthService<'a> {
    pub fn new(client: &'a Client, config: &'a Config) -> Self {
        let github_client = GitHubClient::new(client);
        Self {
            client,
            config,
            github_client,
        }
    }

    /// Exchange authorization code for access token and return complete user info
    pub async fn exchange_code_for_user(&self, code: String) -> Result<User, Box<dyn std::error::Error>> {
        // Step 1: Exchange code for access token
        let access_token = self.exchange_code_for_token(code).await?;
        
        // Step 2: Get user info with the access token
        let github_user = self.github_client.get_user(&access_token).await?;
        
        // Step 3: Get user email if not public
        let email = if github_user.email.is_none() {
            self.github_client.get_user_primary_email(&access_token).await.ok()
        } else {
            github_user.email
        };

        // Step 4: Create domain user
        let user = User::new(
            github_user.id,
            github_user.login,
            github_user.name,
            email,
            github_user.avatar_url,
            github_user.html_url,
            access_token,
        );

        Ok(user)
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code_for_token(&self, code: String) -> Result<String, Box<dyn std::error::Error>> {
        let token_request = GitHubTokenRequest {
            client_id: self.config.github_client_id.clone(),
            client_secret: self.config.github_client_secret.clone(),
            code,
            redirect_uri: self.config.github_redirect_uri.clone(),
        };

        let response = self
            .client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .header("User-Agent", "DevOps-CI/1.0")
            .json(&token_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to exchange code for token: {}", error_text).into());
        }

        let token_response: GitHubTokenResponse = response.json().await?;
        Ok(token_response.access_token)
    }

    /// Get user information using access token
    pub async fn get_user_info(&self, access_token: &str) -> Result<GitHubUser, Box<dyn std::error::Error>> {
        self.github_client.get_user(access_token).await
    }

    /// Check if the access token is valid
    pub async fn validate_token(&self, access_token: &str) -> bool {
        self.github_client.get_user(access_token).await.is_ok()
    }
}