use reqwest::Client;
use tracing::{debug, info, error};

use crate::{
    config::Config,
    domain::User,
    dto::{GitHubRepo, GitHubTokenRequest, GitHubTokenResponse, GitHubUser},
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
        debug!("🔄 Exchanging authorization code for user info");
        
        // Step 1: Exchange code for access token
        let access_token = self.exchange_code_for_token(code).await?;
        info!("✅ Successfully obtained access token");
        
        // Step 2: Get user info with the access token
        let github_user = self.github_client.get_user(&access_token).await?;
        info!("✅ Retrieved user info for: {}", github_user.login);
        
        // Step 3: Get user email if not public
        let email = if github_user.email.is_none() {
            debug!("🔍 User email not public, fetching from emails endpoint");
            match self.github_client.get_user_primary_email(&access_token).await {
                Ok(email) => {
                    info!("✅ Retrieved primary email");
                    Some(email)
                }
                Err(e) => {
                    debug!("⚠️ Could not fetch user email: {}", e);
                    None
                }
            }
        } else {
            github_user.email
        };

        // Step 4: Create domain user
        let user = User::new(
            github_user.id,
            github_user.login.clone(),
            github_user.name,
            email,
            github_user.avatar_url,
            github_user.html_url,
            access_token,
        );

        info!("✅ Created user domain object for: {}", github_user.login);
        Ok(user)
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code_for_token(&self, code: String) -> Result<String, Box<dyn std::error::Error>> {
        debug!("🔄 Exchanging authorization code for access token");
        
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
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Token exchange failed with status {}: {}", status, error_text);
            return Err(format!("Failed to exchange code for token: {} - {}", status, error_text).into());
        }

        let token_response: GitHubTokenResponse = response.json().await?;
        debug!("✅ Successfully exchanged code for access token");
        Ok(token_response.access_token)
    }

    /// Get user information using access token
    pub async fn get_user_info(&self, access_token: &str) -> Result<GitHubUser, Box<dyn std::error::Error>> {
        debug!("🔄 Getting user info with access token");
        self.github_client.get_user(access_token).await
    }

    /// Get user's repositories
    pub async fn get_user_repositories(&self, access_token: &str, per_page: Option<u32>) -> Result<Vec<GitHubRepo>, Box<dyn std::error::Error>> {
        debug!("🔄 Getting user repositories");
        self.github_client.get_user_repos(access_token, per_page).await
    }

    /// Get specific repository information
    pub async fn get_repository_info(&self, access_token: &str, owner: &str, repo: &str) -> Result<GitHubRepo, Box<dyn std::error::Error>> {
        debug!("🔄 Getting repository info for {}/{}", owner, repo);
        self.github_client.get_repo(access_token, owner, repo).await
    }

    /// Check if the access token is valid
    pub async fn validate_token(&self, access_token: &str) -> bool {
        debug!("🔄 Validating access token");
        match self.github_client.get_user(access_token).await {
            Ok(_) => {
                debug!("✅ Access token is valid");
                true
            }
            Err(e) => {
                debug!("❌ Access token validation failed: {}", e);
                false
            }
        }
    }

    /// Create a webhook for a repository
    pub async fn create_repository_webhook(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        webhook_url: &str,
        events: Vec<&str>,
        secret: Option<String>,
    ) -> Result<crate::dto::GitHubWebhook, Box<dyn std::error::Error>> {
        debug!("🔄 Creating webhook for {}/{}", owner, repo);
        self.github_client.create_webhook(access_token, owner, repo, webhook_url, events, secret).await
    }
}