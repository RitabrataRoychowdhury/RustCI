//! Git connector implementations
//! 
//! This module contains connector implementations for Git-based services:
//! - GitHub (GitHub Actions, GitHub API)
//! - GitLab (GitLab CI/CD, GitLab API)

pub mod github;
pub mod gitlab;

// Re-export connector implementations
pub use github::GitHubConnector;
pub use gitlab::GitLabConnector;