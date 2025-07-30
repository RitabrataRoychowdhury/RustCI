#![allow(dead_code)]

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

use crate::{
    domain::entities::{
        GitHubBranch, GitHubCommit, GitHubContent, GitHubPullRequest, GitHubRepo, GitHubUser,
        PullRequestRequest,
    },
    error::{AppError, Result},
};

#[async_trait]
pub trait GitHubService: Send + Sync {
    async fn get_user_info(&self, access_token: &str) -> Result<GitHubUser>;
    async fn get_user_repositories(&self, access_token: &str) -> Result<Vec<GitHubRepo>>;
    async fn get_repository_contents(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        path: &str,
    ) -> Result<Vec<GitHubContent>>;
    async fn check_dockerfile_exists(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<bool>;
    async fn create_branch(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        branch_name: &str,
        from_branch: &str,
    ) -> Result<GitHubBranch>;
    async fn create_file(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        path: &str,
        content: &str,
        message: &str,
        branch: &str,
    ) -> Result<GitHubCommit>;
    async fn create_pull_request(
        &self,
        access_token: &str,
        pr_request: &PullRequestRequest,
    ) -> Result<GitHubPullRequest>;
    async fn delete_branch(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        branch_name: &str,
    ) -> Result<()>;
}

pub struct GitHubServiceImpl {
    http_client: Arc<Client>,
    base_url: String,
}

impl GitHubServiceImpl {
    pub fn new(http_client: Arc<Client>) -> Self {
        Self {
            http_client,
            base_url: "https://api.github.com".to_string(),
        }
    }

    fn create_headers(&self, access_token: &str) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", access_token).parse().unwrap(),
        );
        headers.insert(reqwest::header::USER_AGENT, "RustCI/1.0".parse().unwrap());
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github.v3+json".parse().unwrap(),
        );
        headers
    }

    async fn handle_github_response<T>(&self, response: reqwest::Response) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::GitHubApiError(format!(
                "GitHub API error ({}): {}",
                status, error_text
            )));
        }

        response.json::<T>().await.map_err(|e| {
            AppError::GitHubApiError(format!("Failed to parse GitHub response: {}", e))
        })
    }
}

#[async_trait]
impl GitHubService for GitHubServiceImpl {
    async fn get_user_info(&self, access_token: &str) -> Result<GitHubUser> {
        let url = format!("{}/user", self.base_url);
        let headers = self.create_headers(access_token);

        let response = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::GitHubApiError(format!("Failed to get user info: {}", e)))?;

        self.handle_github_response(response).await
    }

    async fn get_user_repositories(&self, access_token: &str) -> Result<Vec<GitHubRepo>> {
        let url = format!("{}/user/repos", self.base_url);
        let headers = self.create_headers(access_token);

        let response = self
            .http_client
            .get(&url)
            .headers(headers)
            .query(&[
                ("sort", "updated"),
                ("direction", "desc"),
                ("per_page", "100"),
                ("type", "all"),
            ])
            .send()
            .await
            .map_err(|e| AppError::GitHubApiError(format!("Failed to get repositories: {}", e)))?;

        self.handle_github_response(response).await
    }

    async fn get_repository_contents(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        path: &str,
    ) -> Result<Vec<GitHubContent>> {
        let url = format!(
            "{}/repos/{}/{}/contents/{}",
            self.base_url, owner, repo, path
        );
        let headers = self.create_headers(access_token);

        let response = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                AppError::GitHubApiError(format!("Failed to get repository contents: {}", e))
            })?;

        // Handle both single file and directory responses
        if response.status().is_success() {
            let response_text = response
                .text()
                .await
                .map_err(|e| AppError::GitHubApiError(format!("Failed to read response: {}", e)))?;

            // Try to parse as array first (directory), then as single object (file)
            if let Ok(contents) = serde_json::from_str::<Vec<GitHubContent>>(&response_text) {
                Ok(contents)
            } else if let Ok(content) = serde_json::from_str::<GitHubContent>(&response_text) {
                Ok(vec![content])
            } else {
                Err(AppError::GitHubApiError(
                    "Failed to parse repository contents".to_string(),
                ))
            }
        } else {
            let error_text = response.text().await.map_err(|e| {
                AppError::GitHubApiError(format!("Failed to read error response: {}", e))
            })?;
            Err(AppError::GitHubApiError(format!(
                "GitHub API error: {}",
                error_text
            )))
        }
    }

    async fn check_dockerfile_exists(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<bool> {
        let url = format!(
            "{}/repos/{}/{}/contents/Dockerfile",
            self.base_url, owner, repo
        );
        let headers = self.create_headers(access_token);

        let response = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::GitHubApiError(format!("Failed to check Dockerfile: {}", e)))?;

        Ok(response.status().is_success())
    }

    async fn create_branch(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        branch_name: &str,
        from_branch: &str,
    ) -> Result<GitHubBranch> {
        // First, get the SHA of the from_branch
        let ref_url = format!(
            "{}/repos/{}/{}/git/ref/heads/{}",
            self.base_url, owner, repo, from_branch
        );
        let headers = self.create_headers(access_token);

        let ref_response = self
            .http_client
            .get(&ref_url)
            .headers(headers.clone())
            .send()
            .await
            .map_err(|e| {
                AppError::GitHubApiError(format!("Failed to get branch reference: {}", e))
            })?;

        let ref_data: serde_json::Value = self.handle_github_response(ref_response).await?;
        let sha = ref_data["object"]["sha"]
            .as_str()
            .ok_or_else(|| AppError::GitHubApiError("Failed to get branch SHA".to_string()))?;

        // Create new branch
        let create_ref_url = format!("{}/repos/{}/{}/git/refs", self.base_url, owner, repo);
        let create_body = json!({
            "ref": format!("refs/heads/{}", branch_name),
            "sha": sha
        });

        let create_response = self
            .http_client
            .post(&create_ref_url)
            .headers(headers)
            .json(&create_body)
            .send()
            .await
            .map_err(|e| AppError::GitHubApiError(format!("Failed to create branch: {}", e)))?;

        let _ref_result: serde_json::Value = self.handle_github_response(create_response).await?;

        // Return branch info (simplified)
        Ok(GitHubBranch {
            name: branch_name.to_string(),
            commit: GitHubCommit {
                sha: sha.to_string(),
                url: format!(
                    "{}/repos/{}/{}/git/commits/{}",
                    self.base_url, owner, repo, sha
                ),
                html_url: format!("https://github.com/{}/{}/commit/{}", owner, repo, sha),
                author: None,
                committer: None,
                message: "Branch creation".to_string(),
                tree: crate::domain::entities::GitHubTree {
                    sha: sha.to_string(),
                    url: format!(
                        "{}/repos/{}/{}/git/trees/{}",
                        self.base_url, owner, repo, sha
                    ),
                },
                parents: vec![],
            },
            protected: false,
        })
    }

    async fn create_file(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        path: &str,
        content: &str,
        message: &str,
        branch: &str,
    ) -> Result<GitHubCommit> {
        let url = format!(
            "{}/repos/{}/{}/contents/{}",
            self.base_url, owner, repo, path
        );
        let headers = self.create_headers(access_token);

        // Encode content to base64
        use base64ct::Encoding;
        let encoded_content = base64ct::Base64::encode_string(content.as_bytes());

        let body = json!({
            "message": message,
            "content": encoded_content,
            "branch": branch
        });

        let response = self
            .http_client
            .put(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::GitHubApiError(format!("Failed to create file: {}", e)))?;

        let result: serde_json::Value = self.handle_github_response(response).await?;

        // Extract commit info from response
        let commit_data = &result["commit"];
        Ok(GitHubCommit {
            sha: commit_data["sha"].as_str().unwrap_or("").to_string(),
            url: commit_data["url"].as_str().unwrap_or("").to_string(),
            html_url: commit_data["html_url"].as_str().unwrap_or("").to_string(),
            author: None,
            committer: None,
            message: message.to_string(),
            tree: crate::domain::entities::GitHubTree {
                sha: commit_data["tree"]["sha"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                url: commit_data["tree"]["url"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            },
            parents: vec![],
        })
    }

    async fn create_pull_request(
        &self,
        access_token: &str,
        pr_request: &PullRequestRequest,
    ) -> Result<GitHubPullRequest> {
        let url = format!(
            "{}/repos/{}/{}/pulls",
            self.base_url, pr_request.owner, pr_request.repo
        );
        let headers = self.create_headers(access_token);

        let body = json!({
            "title": pr_request.title,
            "body": pr_request.body,
            "head": pr_request.head,
            "base": pr_request.base,
            "draft": pr_request.draft
        });

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AppError::GitHubApiError(format!("Failed to create pull request: {}", e))
            })?;

        self.handle_github_response(response).await
    }

    async fn delete_branch(
        &self,
        access_token: &str,
        owner: &str,
        repo: &str,
        branch_name: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/git/refs/heads/{}",
            self.base_url, owner, repo, branch_name
        );
        let headers = self.create_headers(access_token);

        let response = self
            .http_client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::GitHubApiError(format!("Failed to delete branch: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::GitHubApiError(format!(
                "Failed to delete branch ({}): {}",
                status, error_text
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_github_service_creation() {
        let client = Arc::new(Client::new());
        let service = GitHubServiceImpl::new(client);

        assert_eq!(service.base_url, "https://api.github.com");
    }

    #[tokio::test]
    async fn test_create_headers() {
        let client = Arc::new(Client::new());
        let service = GitHubServiceImpl::new(client);
        let token = "test_token";

        let headers = service.create_headers(token);

        assert!(headers.contains_key(reqwest::header::AUTHORIZATION));
        assert!(headers.contains_key(reqwest::header::USER_AGENT));
        assert!(headers.contains_key(reqwest::header::ACCEPT));
    }

    // Note: Integration tests would require actual GitHub API access
    // These would be run separately with real tokens in a test environment
}
