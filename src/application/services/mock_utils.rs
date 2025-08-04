use crate::{
    application::services::command::{Command, CommandResult},
    domain::entities::{
        GitHubBranch, GitHubCommit, GitHubPullRequest, GitHubTree, GitHubUser, PullRequestRequest,
    },
    error::{AppError, Result},
};
use chrono::Utc;
use uuid::Uuid;

/// Common mock utilities for testing
pub struct MockUtils;

impl MockUtils {
    /// Create a mock GitHub user for testing
    pub fn create_mock_github_user(id: i64, login: &str) -> GitHubUser {
        GitHubUser {
            id,
            login: login.to_string(),
            name: Some(format!("{} User", login)),
            email: Some(format!("{}@example.com", login)),
            avatar_url: format!("https://avatars.githubusercontent.com/u/{}", id),
            html_url: format!("https://github.com/{}", login),
            company: Some("Test Company".to_string()),
            blog: Some(format!("https://{}.dev", login)),
            location: Some("Test City".to_string()),
            bio: Some("Test bio".to_string()),
            public_repos: 10,
            public_gists: 5,
            followers: 100,
            following: 50,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Create a mock GitHub commit for testing
    pub fn create_mock_commit(sha: &str, message: &str) -> GitHubCommit {
        GitHubCommit {
            sha: sha.to_string(),
            url: format!("https://api.github.com/repos/test/test/git/commits/{}", sha),
            html_url: format!("https://github.com/test/test/commit/{}", sha),
            author: None,
            committer: None,
            message: message.to_string(),
            tree: GitHubTree {
                sha: format!("tree_{}", sha),
                url: format!(
                    "https://api.github.com/repos/test/test/git/trees/tree_{}",
                    sha
                ),
            },
            parents: vec![],
        }
    }

    /// Create a mock GitHub branch for testing
    pub fn create_mock_branch(name: &str, commit_sha: &str) -> GitHubBranch {
        GitHubBranch {
            name: name.to_string(),
            commit: Self::create_mock_commit(commit_sha, &format!("Commit for {}", name)),
            protected: name == "main",
        }
    }
}

/// Base mock command with common functionality
pub struct BaseMockCommand {
    pub description: String,
    pub should_fail: bool,
    pub can_undo: bool,
    pub command_id: Uuid,
}

impl BaseMockCommand {
    pub fn new(description: &str, should_fail: bool, can_undo: bool) -> Self {
        Self {
            description: description.to_string(),
            should_fail,
            can_undo,
            command_id: Uuid::new_v4(),
        }
    }

    pub fn check_should_fail(&self) -> Result<()> {
        if self.should_fail {
            Err(AppError::GitHubApiError(format!(
                "Mock command failed: {}",
                self.description
            )))
        } else {
            Ok(())
        }
    }
}

/// Mock command for creating branches
pub struct MockCreateBranchCommand {
    base: BaseMockCommand,
    branch_name: String,
}

impl MockCreateBranchCommand {
    pub fn new(branch_name: &str, should_fail: bool) -> Self {
        Self {
            base: BaseMockCommand::new(
                &format!("Create branch {}", branch_name),
                should_fail,
                true,
            ),
            branch_name: branch_name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Command for MockCreateBranchCommand {
    async fn execute(&self) -> Result<CommandResult> {
        self.base.check_should_fail()?;

        Ok(CommandResult::BranchCreated(MockUtils::create_mock_branch(
            &self.branch_name,
            "abc123",
        )))
    }

    async fn undo(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        self.base.description.clone()
    }

    fn command_id(&self) -> Uuid {
        self.base.command_id
    }

    fn can_undo(&self) -> bool {
        self.base.can_undo
    }
}

/// Mock command for creating files
pub struct MockCreateFileCommand {
    base: BaseMockCommand,
    file_path: String,
    content: String,
}

impl MockCreateFileCommand {
    pub fn new(file_path: &str, content: &str, should_fail: bool) -> Self {
        Self {
            base: BaseMockCommand::new(&format!("Create file {}", file_path), should_fail, false),
            file_path: file_path.to_string(),
            content: content.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Command for MockCreateFileCommand {
    async fn execute(&self) -> Result<CommandResult> {
        self.base.check_should_fail()?;

        Ok(CommandResult::FileCreated(MockUtils::create_mock_commit(
            "file123",
            &format!("Add {}", self.file_path),
        )))
    }

    async fn undo(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        self.base.description.clone()
    }

    fn command_id(&self) -> Uuid {
        self.base.command_id
    }

    fn can_undo(&self) -> bool {
        self.base.can_undo
    }
}

/// Mock command for creating pull requests
pub struct MockCreatePRCommand {
    base: BaseMockCommand,
    pr_request: PullRequestRequest,
}

impl MockCreatePRCommand {
    pub fn new(pr_request: PullRequestRequest, should_fail: bool) -> Self {
        Self {
            base: BaseMockCommand::new(
                &format!("Create PR: {}", pr_request.title),
                should_fail,
                false,
            ),
            pr_request,
        }
    }
}

#[async_trait::async_trait]
impl Command for MockCreatePRCommand {
    async fn execute(&self) -> Result<CommandResult> {
        self.base.check_should_fail()?;

        Ok(CommandResult::PullRequestCreated(Box::new(GitHubPullRequest {
            id: 123,
            number: 1,
            title: self.pr_request.title.clone(),
            body: self.pr_request.body.clone(),
            html_url: "https://github.com/test/test/pull/1".to_string(),
            state: "open".to_string(),
            draft: false,
            merged: false,
            mergeable: Some(true),
            head: MockUtils::create_mock_branch("test-branch", "abc123"),
            base: MockUtils::create_mock_branch("main", "main123"),
            user: MockUtils::create_mock_github_user(12345, "test-user"),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            merged_at: None,
            closed_at: None,
        })))
    }

    async fn undo(&self) -> Result<()> {
        Ok(())
    }

    fn description(&self) -> String {
        self.base.description.clone()
    }

    fn command_id(&self) -> Uuid {
        self.base.command_id
    }

    fn can_undo(&self) -> bool {
        self.base.can_undo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_utils_create_github_user() {
        let user = MockUtils::create_mock_github_user(12345, "testuser");
        assert_eq!(user.id, 12345);
        assert_eq!(user.login, "testuser");
        assert_eq!(user.email, Some("testuser@example.com".to_string()));
    }

    #[test]
    fn test_mock_utils_create_commit() {
        let commit = MockUtils::create_mock_commit("abc123", "Test commit");
        assert_eq!(commit.sha, "abc123");
        assert_eq!(commit.message, "Test commit");
    }

    #[test]
    fn test_mock_utils_create_branch() {
        let branch = MockUtils::create_mock_branch("feature", "def456");
        assert_eq!(branch.name, "feature");
        assert_eq!(branch.commit.sha, "def456");
        assert!(!branch.protected);
    }

    #[tokio::test]
    async fn test_mock_create_branch_command() {
        let command = MockCreateBranchCommand::new("test-branch", false);
        let result = command.execute().await;
        assert!(result.is_ok());

        let failing_command = MockCreateBranchCommand::new("test-branch", true);
        let result = failing_command.execute().await;
        assert!(result.is_err());
    }
}
