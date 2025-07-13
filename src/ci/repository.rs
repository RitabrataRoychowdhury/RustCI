use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs;
use tokio::process::Command;
use tracing::{info, debug, error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfig {
    pub url: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub access_token: Option<String>,
    pub ssh_key: Option<String>,
    pub clone_depth: Option<u32>,
    pub submodules: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneResult {
    pub repository_path: PathBuf,
    pub commit_hash: String,
    pub branch: String,
    pub clone_time: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
}

pub struct RepositoryManager;

impl RepositoryManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn clone_repository(
        &self,
        config: &RepositoryConfig,
        target_directory: &Path,
    ) -> Result<CloneResult> {
        info!("📥 Cloning repository: {}", config.url);

        // Ensure target directory exists
        fs::create_dir_all(target_directory).await
            .map_err(|e| AppError::InternalServerError(format!("Failed to create target directory: {}", e)))?;

        // Prepare git clone command
        let mut git_args = vec!["clone".to_string()];

        // Add clone depth if specified
        if let Some(depth) = config.clone_depth {
            git_args.push("--depth".to_string());
            git_args.push(depth.to_string());
        }

        // Add branch if specified
        if let Some(branch) = &config.branch {
            git_args.push("--branch".to_string());
            git_args.push(branch.clone());
        }

        // Add submodules if requested
        if config.submodules {
            git_args.push("--recurse-submodules".to_string());
        }

        // Prepare repository URL with authentication
        let repo_url = self.prepare_authenticated_url(&config.url, &config.access_token)?;
        git_args.push(repo_url);
        git_args.push(target_directory.to_string_lossy().to_string());

        debug!("🔧 Git clone command: git {}", git_args.join(" "));

        // Execute git clone
        let output = Command::new("git")
            .args(&git_args)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to execute git clone: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("❌ Git clone failed: {}", stderr);
            return Err(AppError::ExternalServiceError(format!("Git clone failed: {}", stderr)));
        }

        // Get commit information
        let commit_hash = self.get_current_commit(target_directory).await?;
        let branch = self.get_current_branch(target_directory).await?;

        // Calculate repository size
        let size_bytes = self.calculate_directory_size(target_directory).await?;

        let result = CloneResult {
            repository_path: target_directory.to_path_buf(),
            commit_hash,
            branch,
            clone_time: chrono::Utc::now(),
            size_bytes,
        };

        info!("✅ Repository cloned successfully: {} bytes", size_bytes);
        Ok(result)
    }

    pub async fn checkout_commit(
        &self,
        repository_path: &Path,
        commit_hash: &str,
    ) -> Result<()> {
        info!("🔄 Checking out commit: {}", commit_hash);

        let output = Command::new("git")
            .args(["checkout", commit_hash])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to execute git checkout: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::ExternalServiceError(format!("Git checkout failed: {}", stderr)));
        }

        info!("✅ Successfully checked out commit: {}", commit_hash);
        Ok(())
    }

    pub async fn pull_latest(&self, repository_path: &Path) -> Result<String> {
        info!("⬇️ Pulling latest changes");

        let output = Command::new("git")
            .args(["pull", "origin"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to execute git pull: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::ExternalServiceError(format!("Git pull failed: {}", stderr)));
        }

        // Get the new commit hash
        let commit_hash = self.get_current_commit(repository_path).await?;
        info!("✅ Successfully pulled latest changes, new commit: {}", commit_hash);
        
        Ok(commit_hash)
    }

    pub async fn get_repository_info(&self, repository_path: &Path) -> Result<RepositoryInfo> {
        let commit_hash = self.get_current_commit(repository_path).await?;
        let branch = self.get_current_branch(repository_path).await?;
        let remote_url = self.get_remote_url(repository_path).await?;
        let last_commit_message = self.get_last_commit_message(repository_path).await?;
        let last_commit_author = self.get_last_commit_author(repository_path).await?;
        let last_commit_date = self.get_last_commit_date(repository_path).await?;

        Ok(RepositoryInfo {
            commit_hash,
            branch,
            remote_url,
            last_commit_message,
            last_commit_author,
            last_commit_date,
        })
    }

    fn prepare_authenticated_url(&self, url: &str, access_token: &Option<String>) -> Result<String> {
        if let Some(token) = access_token {
            if url.starts_with("https://github.com/") {
                // For GitHub, use token authentication
                let authenticated_url = url.replace("https://github.com/", &format!("https://{}@github.com/", token));
                Ok(authenticated_url)
            } else if url.starts_with("https://gitlab.com/") {
                // For GitLab, use token authentication
                let authenticated_url = url.replace("https://gitlab.com/", &format!("https://oauth2:{}@gitlab.com/", token));
                Ok(authenticated_url)
            } else {
                // For other Git providers, try generic token auth
                Ok(url.replace("https://", &format!("https://{}@", token)))
            }
        } else {
            // No authentication, use URL as-is
            Ok(url.to_string())
        }
    }

    async fn get_current_commit(&self, repository_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to get commit hash: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::ExternalServiceError("Failed to get current commit".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_current_branch(&self, repository_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to get branch name: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::ExternalServiceError("Failed to get current branch".to_string()));
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            Ok("HEAD".to_string()) // Detached HEAD state
        } else {
            Ok(branch)
        }
    }

    async fn get_remote_url(&self, repository_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to get remote URL: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::ExternalServiceError("Failed to get remote URL".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_last_commit_message(&self, repository_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=format:%s"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to get commit message: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::ExternalServiceError("Failed to get commit message".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_last_commit_author(&self, repository_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=format:%an <%ae>"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to get commit author: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::ExternalServiceError("Failed to get commit author".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_last_commit_date(&self, repository_path: &Path) -> Result<chrono::DateTime<chrono::Utc>> {
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=format:%cI"])
            .current_dir(repository_path)
            .output()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to get commit date: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::ExternalServiceError("Failed to get commit date".to_string()));
        }

        let date_string = String::from_utf8_lossy(&output.stdout);
        let date_str = date_string.trim();
        chrono::DateTime::parse_from_rfc3339(date_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| AppError::InternalServerError(format!("Failed to parse commit date: {}", e)))
    }

    pub fn calculate_directory_size<'a>(
        &'a self,
        directory: &'a Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            let mut total_size = 0u64;

            let mut entries = fs::read_dir(directory).await
                .map_err(|e| AppError::InternalServerError(format!("Failed to read directory: {}", e)))?;

            while let Some(entry) = entries.next_entry().await
                .map_err(|e| AppError::InternalServerError(format!("Failed to read directory entry: {}", e)))? {
                    
                let metadata = entry.metadata().await
                    .map_err(|e| AppError::InternalServerError(format!("Failed to get file metadata: {}", e)))?;

                if metadata.is_file() {
                    total_size += metadata.len();
                } else if metadata.is_dir() {
                    // Recurse with `.await?`, boxed
                    total_size += self.calculate_directory_size(&entry.path()).await?;
                }
            }

            Ok(total_size)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub commit_hash: String,
    pub branch: String,
    pub remote_url: String,
    pub last_commit_message: String,
    pub last_commit_author: String,
    pub last_commit_date: chrono::DateTime<chrono::Utc>,
}