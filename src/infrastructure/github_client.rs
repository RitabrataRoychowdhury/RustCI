use reqwest::Client;
use serde_json::Value;

use crate::dto::{GitHubEmail, GitHubUser};

pub struct GitHubClient<'a> {
    client: &'a Client,
}

impl<'a> GitHubClient<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Get user information from GitHub API
    pub async fn get_user(&self, access_token: &str) -> Result<GitHubUser, Box<dyn std::error::Error>> {
        let response = self
            .client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to get user info: {}", error_text).into());
        }

        let user: GitHubUser = response.json().await?;
        Ok(user)
    }

    /// Get user's primary email from GitHub API
    pub async fn get_user_primary_email(&self, access_token: &str) -> Result<String, Box<dyn std::error::Error>> {
        let response = self
            .client
            .get("https://api.github.com/user/emails")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to get user emails: {}", error_text).into());
        }

        let emails: Vec<GitHubEmail> = response.json().await?;
        
        // Find primary email or first verified email
        let primary_email = emails
            .iter()
            .find(|email| email.primary && email.verified)
            .or_else(|| emails.iter().find(|email| email.verified))
            .map(|email| email.email.clone())
            .ok_or("No verified email found")?;

        Ok(primary_email)
    }

    /// Get user's repositories
    pub async fn get_user_repos(&self, access_token: &str, per_page: Option<u32>) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
        let per_page = per_page.unwrap_or(30).min(100); // GitHub API limits to 100 per page
        
        let url = format!("https://api.github.com/user/repos?per_page={}&sort=updated", per_page);
        
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to get user repositories: {}", error_text).into());
        }

        let repos: Vec<Value> = response.json().await?;
        Ok(repos)
    }

    /// Get repository information
    pub async fn get_repo(&self, access_token: &str, owner: &str, repo: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
        
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to get repository info: {}", error_text).into());
        }

        let repo_info: Value = response.json().await?;
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
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let webhook_payload = serde_json::json!({
            "name": "web",
            "active": true,
            "events": events,
            "config": {
                "url": webhook_url,
                "content_type": "json",
                "insecure_ssl": "0"
            }
        });

        let url = format!("https://api.github.com/repos/{}/{}/hooks", owner, repo);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "DevOps-CI/1.0")
            .json(&webhook_payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to create webhook: {}", error_text).into());
        }

        let webhook: Value = response.json().await?;
        Ok(webhook)
    }
}