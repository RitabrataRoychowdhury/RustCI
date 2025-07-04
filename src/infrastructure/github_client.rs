use reqwest::Client;
use serde_json::json;
use tracing::{debug, info, error};

use crate::dto::{GitHubEmail, GitHubRepo, GitHubUser, GitHubWebhook, WebhookConfig, WebhookPayload};

pub struct GitHubClient<'a> {
    client: &'a Client,
}

impl<'a> GitHubClient<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Get user information from GitHub API
    pub async fn get_user(&self, access_token: &str) -> Result<GitHubUser, Box<dyn std::error::Error>> {
        debug!("🔄 Fetching user info from GitHub API");
        
        let response = self
            .client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to get user info: {} - {}", status, error_text);
            return Err(format!("Failed to get user info: {} - {}", status, error_text).into());
        }

        let user: GitHubUser = response.json().await?;
        info!("✅ Successfully fetched user info for: {}", user.login);
        Ok(user)
    }

    /// Get user's primary email from GitHub API
    pub async fn get_user_primary_email(&self, access_token: &str) -> Result<String, Box<dyn std::error::Error>> {
        debug!("🔄 Fetching user emails from GitHub API");
        
        let response = self
            .client
            .get("https://api.github.com/user/emails")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to get user emails: {} - {}", status, error_text);
            return Err(format!("Failed to get user emails: {} - {}", status, error_text).into());
        }

        let emails: Vec<GitHubEmail> = response.json().await?;
        
        // Find primary email or first verified email
        let primary_email = emails
            .iter()
            .find(|email| email.primary && email.verified)
            .or_else(|| emails.iter().find(|email| email.verified))
            .map(|email| email.email.clone())
            .ok_or("No verified email found")?;

        info!("✅ Found primary email");
        Ok(primary_email)
    }

    /// Get user's repositories
    pub async fn get_user_repos(&self, access_token: &str, per_page: Option<u32>) -> Result<Vec<GitHubRepo>, Box<dyn std::error::Error>> {
        let per_page = per_page.unwrap_or(30).min(100); // GitHub API limits to 100 per page
        debug!("🔄 Fetching user repositories (per_page: {})", per_page);
        
        let url = format!("https://api.github.com/user/repos?per_page={}&sort=updated&type=all", per_page);
        
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to get user repositories: {} - {}", status, error_text);
            return Err(format!("Failed to get user repositories: {} - {}", status, error_text).into());
        }

        let repos: Vec<GitHubRepo> = response.json().await?;
        info!("✅ Successfully fetched {} repositories", repos.len());
        Ok(repos)
    }

    /// Get repository information
    pub async fn get_repo(&self, access_token: &str, owner: &str, repo: &str) -> Result<GitHubRepo, Box<dyn std::error::Error>> {
        debug!("🔄 Fetching repository info for {}/{}", owner, repo);
        
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
        
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to get repository {}/{}: {} - {}", owner, repo, status, error_text);
            return Err(format!("Failed to get repository {}/{}: {} - {}", owner, repo, status, error_text).into());
        }

        let repo_info: GitHubRepo = response.json().await?;
        info!("✅ Successfully fetched repository info for {}/{}", owner, repo);
        Ok(repo_info)
    }

    /// Create a webhook for a repository
    pub async fn create_webhook(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        webhook_url: &str,
        events: Vec<&str>,
        secret: Option<String>,
    ) -> Result<GitHubWebhook, Box<dyn std::error::Error>> {
        debug!("🔄 Creating webhook for {}/{}", owner, repo);
        
        let mut config = WebhookConfig {
            url: webhook_url.to_string(),
            content_type: "json".to_string(),
            insecure_ssl: "0".to_string(),
            secret: None,
        };

        if let Some(secret_value) = secret {
            config.secret = Some(secret_value);
        }

        let webhook_payload = WebhookPayload {
            name: "web".to_string(),
            active: true,
            events: events.iter().map(|s| s.to_string()).collect(),
            config,
        };

        let url = format!("https://api.github.com/repos/{}/{}/hooks", owner, repo);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .json(&webhook_payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to create webhook for {}/{}: {} - {}", owner, repo, status, error_text);
            return Err(format!("Failed to create webhook for {}/{}: {} - {}", owner, repo, status, error_text).into());
        }

        let webhook: GitHubWebhook = response.json().await?;
        info!("✅ Successfully created webhook for {}/{} with ID: {}", owner, repo, webhook.id);
        Ok(webhook)
    }

    /// List webhooks for a repository
    pub async fn list_webhooks(&self, access_token: &str, owner: &str, repo: &str) -> Result<Vec<GitHubWebhook>, Box<dyn std::error::Error>> {
        debug!("🔄 Listing webhooks for {}/{}", owner, repo);
        
        let url = format!("https://api.github.com/repos/{}/{}/hooks", owner, repo);
        
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to list webhooks for {}/{}: {} - {}", owner, repo, status, error_text);
            return Err(format!("Failed to list webhooks for {}/{}: {} - {}", owner, repo, status, error_text).into());
        }

        let webhooks: Vec<GitHubWebhook> = response.json().await?;
        info!("✅ Successfully listed {} webhooks for {}/{}", webhooks.len(), owner, repo);
        Ok(webhooks)
    }

    /// Delete a webhook
    pub async fn delete_webhook(&self, access_token: &str, owner: &str, repo: &str, hook_id: u64) -> Result<(), Box<dyn std::error::Error>> {
        debug!("🔄 Deleting webhook {} for {}/{}", hook_id, owner, repo);
        
        let url = format!("https://api.github.com/repos/{}/{}/hooks/{}", owner, repo, hook_id);
        
        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("❌ Failed to delete webhook {} for {}/{}: {} - {}", hook_id, owner, repo, status, error_text);
            return Err(format!("Failed to delete webhook {} for {}/{}: {} - {}", hook_id, owner, repo, status, error_text).into());
        }

        info!("✅ Successfully deleted webhook {} for {}/{}", hook_id, owner, repo);
        Ok(())
    }
}