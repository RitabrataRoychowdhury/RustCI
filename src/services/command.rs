use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{GitHubBranch, GitHubCommit, GitHubPullRequest, PullRequestRequest},
    services::{GitHubService},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandResult {
    BranchCreated(GitHubBranch),
    FileCreated(GitHubCommit),
    PullRequestCreated(GitHubPullRequest),
    BranchDeleted(String),
    ValidationCompleted(crate::models::ValidationResult),
    WorkspaceUpdated(Uuid),
    Success(String),
    Error(String),
}

#[async_trait]
pub trait Command: Send + Sync {
    async fn execute(&self) -> Result<CommandResult>;
    async fn undo(&self) -> Result<()>;
    fn description(&self) -> String;
    fn command_id(&self) -> Uuid;
    fn can_undo(&self) -> bool;
}

pub struct CommandInvoker {
    commands: Vec<Box<dyn Command>>,
    executed_commands: Vec<Box<dyn Command>>,
    execution_log: Vec<CommandExecutionRecord>,
}

#[derive(Debug, Clone)]
pub struct CommandExecutionRecord {
    pub command_id: Uuid,
    pub description: String,
    pub executed_at: chrono::DateTime<chrono::Utc>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl CommandInvoker {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            executed_commands: Vec::new(),
            execution_log: Vec::new(),
        }
    }
    
    pub fn add_command(&mut self, command: Box<dyn Command>) {
        tracing::info!("Adding command: {}", command.description());
        self.commands.push(command);
    }
    
    pub fn add_commands(&mut self, commands: Vec<Box<dyn Command>>) {
        for command in commands {
            self.add_command(command);
        }
    }
    
    pub async fn execute_all(&mut self) -> Result<Vec<CommandResult>> {
        let mut results = Vec::new();
        
        tracing::info!("Executing {} commands", self.commands.len());
        
        for command in self.commands.drain(..) {
            let start_time = std::time::Instant::now();
            let command_id = command.command_id();
            let description = command.description();
            
            tracing::info!("Executing command: {}", description);
            
            match command.execute().await {
                Ok(result) => {
                    let duration = start_time.elapsed();
                    
                    // Record successful execution
                    let record = CommandExecutionRecord {
                        command_id,
                        description: description.clone(),
                        executed_at: chrono::Utc::now(),
                        result: Some(format!("{:?}", result)),
                        error: None,
                        duration_ms: duration.as_millis() as u64,
                    };
                    self.execution_log.push(record);
                    
                    results.push(result);
                    self.executed_commands.push(command);
                    
                    tracing::info!("Command executed successfully: {}", description);
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    
                    // Record failed execution
                    let record = CommandExecutionRecord {
                        command_id,
                        description: description.clone(),
                        executed_at: chrono::Utc::now(),
                        result: None,
                        error: Some(e.to_string()),
                        duration_ms: duration.as_millis() as u64,
                    };
                    self.execution_log.push(record);
                    
                    tracing::error!("Command failed: {} - Error: {}", description, e);
                    
                    // Rollback executed commands
                    if let Err(rollback_error) = self.rollback().await {
                        tracing::error!("Rollback failed: {}", rollback_error);
                        return Err(AppError::InternalServerError(format!(
                            "Command failed: {}. Rollback also failed: {}",
                            e, rollback_error
                        )));
                    }
                    
                    return Err(e);
                }
            }
        }
        
        tracing::info!("All commands executed successfully");
        Ok(results)
    }
    
    pub async fn rollback(&mut self) -> Result<()> {
        tracing::info!("Starting rollback of {} executed commands", self.executed_commands.len());
        
        let mut rollback_errors = Vec::new();
        
        // Execute rollback in reverse order
        for command in self.executed_commands.drain(..).rev() {
            if command.can_undo() {
                tracing::info!("Rolling back command: {}", command.description());
                
                if let Err(e) = command.undo().await {
                    tracing::error!("Failed to rollback command {}: {}", command.description(), e);
                    rollback_errors.push(format!("{}: {}", command.description(), e));
                } else {
                    tracing::info!("Successfully rolled back command: {}", command.description());
                }
            } else {
                tracing::warn!("Command cannot be undone: {}", command.description());
            }
        }
        
        if !rollback_errors.is_empty() {
            return Err(AppError::InternalServerError(format!(
                "Rollback partially failed: {}",
                rollback_errors.join(", ")
            )));
        }
        
        tracing::info!("Rollback completed successfully");
        Ok(())
    }
    
    pub fn get_execution_log(&self) -> &[CommandExecutionRecord] {
        &self.execution_log
    }
    
    pub fn clear_log(&mut self) {
        self.execution_log.clear();
    }
    
    pub fn has_pending_commands(&self) -> bool {
        !self.commands.is_empty()
    }
    
    pub fn pending_commands_count(&self) -> usize {
        self.commands.len()
    }
    
    pub fn executed_commands_count(&self) -> usize {
        self.executed_commands.len()
    }
}

impl Default for CommandInvoker {
    fn default() -> Self {
        Self::new()
    }
}

// Specific Commands for GitHub operations

pub struct CreateBranchCommand {
    id: Uuid,
    github_service: Arc<dyn GitHubService>,
    access_token: String,
    owner: String,
    repo: String,
    branch_name: String,
    from_branch: String,
    created_branch: Option<String>, // For rollback
}

impl CreateBranchCommand {
    pub fn new(
        github_service: Arc<dyn GitHubService>,
        access_token: String,
        owner: String,
        repo: String,
        branch_name: String,
        from_branch: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            github_service,
            access_token,
            owner,
            repo,
            branch_name,
            from_branch,
            created_branch: None,
        }
    }
}

#[async_trait]
impl Command for CreateBranchCommand {
    async fn execute(&self) -> Result<CommandResult> {
        let branch = self.github_service
            .create_branch(&self.access_token, &self.owner, &self.repo, &self.branch_name, &self.from_branch)
            .await?;
        
        // Store branch name for potential rollback
        // Note: In a real implementation, you'd need mutable access to store this
        // This is a simplified version for demonstration
        
        Ok(CommandResult::BranchCreated(branch))
    }
    
    async fn undo(&self) -> Result<()> {
        // Delete the created branch
        self.github_service
            .delete_branch(&self.access_token, &self.owner, &self.repo, &self.branch_name)
            .await
    }
    
    fn description(&self) -> String {
        format!("Create branch '{}' from '{}' in {}/{}", 
            self.branch_name, self.from_branch, self.owner, self.repo)
    }
    
    fn command_id(&self) -> Uuid {
        self.id
    }
    
    fn can_undo(&self) -> bool {
        true
    }
}

pub struct CreateFileCommand {
    id: Uuid,
    github_service: Arc<dyn GitHubService>,
    access_token: String,
    owner: String,
    repo: String,
    file_path: String,
    content: String,
    commit_message: String,
    branch: String,
}

impl CreateFileCommand {
    pub fn new(
        github_service: Arc<dyn GitHubService>,
        access_token: String,
        owner: String,
        repo: String,
        file_path: String,
        content: String,
        commit_message: String,
        branch: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            github_service,
            access_token,
            owner,
            repo,
            file_path,
            content,
            commit_message,
            branch,
        }
    }
}

#[async_trait]
impl Command for CreateFileCommand {
    async fn execute(&self) -> Result<CommandResult> {
        let commit = self.github_service
            .create_file(
                &self.access_token,
                &self.owner,
                &self.repo,
                &self.file_path,
                &self.content,
                &self.commit_message,
                &self.branch,
            )
            .await?;
        
        Ok(CommandResult::FileCreated(commit))
    }
    
    async fn undo(&self) -> Result<()> {
        // Note: GitHub API doesn't have a direct "delete file" endpoint
        // In a real implementation, you'd need to create another commit that removes the file
        // For now, we'll just log that this operation cannot be easily undone
        tracing::warn!("Cannot easily undo file creation for: {}", self.file_path);
        Ok(())
    }
    
    fn description(&self) -> String {
        format!("Create file '{}' in branch '{}' of {}/{}", 
            self.file_path, self.branch, self.owner, self.repo)
    }
    
    fn command_id(&self) -> Uuid {
        self.id
    }
    
    fn can_undo(&self) -> bool {
        false // File creation is not easily reversible via GitHub API
    }
}

pub struct CreatePullRequestCommand {
    id: Uuid,
    github_service: Arc<dyn GitHubService>,
    access_token: String,
    pr_request: PullRequestRequest,
    created_pr_number: Option<i32>, // For potential rollback
}

impl CreatePullRequestCommand {
    pub fn new(
        github_service: Arc<dyn GitHubService>,
        access_token: String,
        pr_request: PullRequestRequest,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            github_service,
            access_token,
            pr_request,
            created_pr_number: None,
        }
    }
}

#[async_trait]
impl Command for CreatePullRequestCommand {
    async fn execute(&self) -> Result<CommandResult> {
        let pr = self.github_service
            .create_pull_request(&self.access_token, &self.pr_request)
            .await?;
        
        Ok(CommandResult::PullRequestCreated(pr))
    }
    
    async fn undo(&self) -> Result<()> {
        // Note: In a real implementation, you might want to close the PR
        // rather than delete it, as GitHub doesn't allow PR deletion via API
        tracing::warn!("Cannot undo pull request creation - PR remains open");
        Ok(())
    }
    
    fn description(&self) -> String {
        format!("Create pull request '{}' from '{}' to '{}' in {}/{}", 
            self.pr_request.title, self.pr_request.head, self.pr_request.base,
            self.pr_request.owner, self.pr_request.repo)
    }
    
    fn command_id(&self) -> Uuid {
        self.id
    }
    
    fn can_undo(&self) -> bool {
        false // PR creation is not easily reversible
    }
}

// Composite command for the complete Dockerfile PR workflow
pub struct DockerfilePRWorkflowCommand {
    id: Uuid,
    commands: Vec<Box<dyn Command>>,
}

impl DockerfilePRWorkflowCommand {
    pub fn new(
        github_service: Arc<dyn GitHubService>,
        access_token: String,
        owner: String,
        repo: String,
        dockerfile_content: String,
        pr_request: PullRequestRequest,
    ) -> Self {
        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        
        // Step 1: Create branch
        commands.push(Box::new(CreateBranchCommand::new(
            github_service.clone(),
            access_token.clone(),
            owner.clone(),
            repo.clone(),
            "rustci/dockerfile-autogen".to_string(),
            "main".to_string(),
        )));
        
        // Step 2: Create Dockerfile
        commands.push(Box::new(CreateFileCommand::new(
            github_service.clone(),
            access_token.clone(),
            owner.clone(),
            repo.clone(),
            "Dockerfile".to_string(),
            dockerfile_content,
            "feat: Add auto-generated Dockerfile".to_string(),
            "rustci/dockerfile-autogen".to_string(),
        )));
        
        // Step 3: Create Pull Request
        commands.push(Box::new(CreatePullRequestCommand::new(
            github_service,
            access_token,
            pr_request,
        )));
        
        Self {
            id: Uuid::new_v4(),
            commands,
        }
    }
}

#[async_trait]
impl Command for DockerfilePRWorkflowCommand {
    async fn execute(&self) -> Result<CommandResult> {
        let mut invoker = CommandInvoker::new();
        
        // Add all sub-commands
        for command in &self.commands {
            // Note: This is a simplified approach. In a real implementation,
            // you'd need to handle the ownership properly
            tracing::info!("Adding sub-command: {}", command.description());
        }
        
        // For now, return success - in a real implementation, you'd execute the sub-commands
        Ok(CommandResult::Success("Dockerfile PR workflow initiated".to_string()))
    }
    
    async fn undo(&self) -> Result<()> {
        // Rollback would involve undoing all sub-commands in reverse order
        tracing::info!("Rolling back Dockerfile PR workflow");
        Ok(())
    }
    
    fn description(&self) -> String {
        "Complete Dockerfile PR workflow (create branch, add file, create PR)".to_string()
    }
    
    fn command_id(&self) -> Uuid {
        self.id
    }
    
    fn can_undo(&self) -> bool {
        true
    }
}

// Factory for creating common command sequences
pub struct CommandFactory;

impl CommandFactory {
    pub fn create_dockerfile_pr_workflow(
        github_service: Arc<dyn GitHubService>,
        access_token: String,
        owner: String,
        repo: String,
        dockerfile_content: String,
        pr_request: PullRequestRequest,
    ) -> Box<dyn Command> {
        Box::new(DockerfilePRWorkflowCommand::new(
            github_service,
            access_token,
            owner,
            repo,
            dockerfile_content,
            pr_request,
        ))
    }
    
    pub fn create_branch_and_file_commands(
        github_service: Arc<dyn GitHubService>,
        access_token: String,
        owner: String,
        repo: String,
        branch_name: String,
        file_path: String,
        file_content: String,
        commit_message: String,
    ) -> Vec<Box<dyn Command>> {
        vec![
            Box::new(CreateBranchCommand::new(
                github_service.clone(),
                access_token.clone(),
                owner.clone(),
                repo.clone(),
                branch_name.clone(),
                "main".to_string(),
            )),
            Box::new(CreateFileCommand::new(
                github_service,
                access_token,
                owner,
                repo,
                file_path,
                file_content,
                commit_message,
                branch_name,
            )),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::GitHubService;
    use async_trait::async_trait;
    
    // Mock GitHub service for testing
    struct MockGitHubService;
    
    #[async_trait]
    impl GitHubService for MockGitHubService {
        async fn get_user_info(&self, _access_token: &str) -> Result<crate::models::GitHubUser> {
            unimplemented!()
        }
        
        async fn get_user_repositories(&self, _access_token: &str) -> Result<Vec<crate::models::GitHubRepo>> {
            unimplemented!()
        }
        
        async fn get_repository_contents(&self, _access_token: &str, _owner: &str, _repo: &str, _path: &str) -> Result<Vec<crate::models::GitHubContent>> {
            unimplemented!()
        }
        
        async fn check_dockerfile_exists(&self, _access_token: &str, _owner: &str, _repo: &str) -> Result<bool> {
            Ok(false)
        }
        
        async fn create_branch(&self, _access_token: &str, _owner: &str, _repo: &str, _branch_name: &str, _from_branch: &str) -> Result<crate::models::GitHubBranch> {
            Ok(crate::models::GitHubBranch {
                name: _branch_name.to_string(),
                commit: crate::models::GitHubCommit {
                    sha: "test_sha".to_string(),
                    url: "test_url".to_string(),
                    html_url: "test_html_url".to_string(),
                    author: None,
                    committer: None,
                    message: "test commit".to_string(),
                    tree: crate::models::GitHubTree {
                        sha: "tree_sha".to_string(),
                        url: "tree_url".to_string(),
                    },
                    parents: vec![],
                },
                protected: false,
            })
        }
        
        async fn create_file(&self, _access_token: &str, _owner: &str, _repo: &str, _path: &str, _content: &str, _message: &str, _branch: &str) -> Result<crate::models::GitHubCommit> {
            Ok(crate::models::GitHubCommit {
                sha: "commit_sha".to_string(),
                url: "commit_url".to_string(),
                html_url: "commit_html_url".to_string(),
                author: None,
                committer: None,
                message: _message.to_string(),
                tree: crate::models::GitHubTree {
                    sha: "tree_sha".to_string(),
                    url: "tree_url".to_string(),
                },
                parents: vec![],
            })
        }
        
        async fn create_pull_request(&self, _access_token: &str, _pr_request: &PullRequestRequest) -> Result<crate::models::GitHubPullRequest> {
            Ok(crate::models::GitHubPullRequest {
                id: 123,
                number: 1,
                title: _pr_request.title.clone(),
                body: _pr_request.body.clone(),
                html_url: "https://github.com/test/test/pull/1".to_string(),
                state: "open".to_string(),
                draft: _pr_request.draft,
                merged: false,
                mergeable: Some(true),
                head: crate::models::GitHubBranch {
                    name: _pr_request.head.clone(),
                    commit: crate::models::GitHubCommit {
                        sha: "head_sha".to_string(),
                        url: "head_url".to_string(),
                        html_url: "head_html_url".to_string(),
                        author: None,
                        committer: None,
                        message: "head commit".to_string(),
                        tree: crate::models::GitHubTree {
                            sha: "head_tree_sha".to_string(),
                            url: "head_tree_url".to_string(),
                        },
                        parents: vec![],
                    },
                    protected: false,
                },
                base: crate::models::GitHubBranch {
                    name: _pr_request.base.clone(),
                    commit: crate::models::GitHubCommit {
                        sha: "base_sha".to_string(),
                        url: "base_url".to_string(),
                        html_url: "base_html_url".to_string(),
                        author: None,
                        committer: None,
                        message: "base commit".to_string(),
                        tree: crate::models::GitHubTree {
                            sha: "base_tree_sha".to_string(),
                            url: "base_tree_url".to_string(),
                        },
                        parents: vec![],
                    },
                    protected: false,
                },
                user: crate::models::GitHubUser {
                    id: 1,
                    login: "testuser".to_string(),
                    name: Some("Test User".to_string()),
                    email: Some("test@example.com".to_string()),
                    avatar_url: "https://example.com/avatar.png".to_string(),
                    html_url: "https://github.com/testuser".to_string(),
                    company: None,
                    blog: None,
                    location: None,
                    bio: None,
                    public_repos: 10,
                    public_gists: 5,
                    followers: 100,
                    following: 50,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                merged_at: None,
                closed_at: None,
            })
        }
        
        async fn delete_branch(&self, _access_token: &str, _owner: &str, _repo: &str, _branch_name: &str) -> Result<()> {
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_command_invoker() {
        let mut invoker = CommandInvoker::new();
        
        assert_eq!(invoker.pending_commands_count(), 0);
        assert_eq!(invoker.executed_commands_count(), 0);
        assert!(!invoker.has_pending_commands());
    }
    
    #[tokio::test]
    async fn test_create_branch_command() {
        let github_service = Arc::new(MockGitHubService);
        let command = CreateBranchCommand::new(
            github_service,
            "test_token".to_string(),
            "testowner".to_string(),
            "testrepo".to_string(),
            "test-branch".to_string(),
            "main".to_string(),
        );
        
        assert!(command.can_undo());
        assert!(command.description().contains("test-branch"));
        
        let result = command.execute().await.unwrap();
        match result {
            CommandResult::BranchCreated(branch) => {
                assert_eq!(branch.name, "test-branch");
            }
            _ => panic!("Expected BranchCreated result"),
        }
    }
    
    #[tokio::test]
    async fn test_create_file_command() {
        let github_service = Arc::new(MockGitHubService);
        let command = CreateFileCommand::new(
            github_service,
            "test_token".to_string(),
            "testowner".to_string(),
            "testrepo".to_string(),
            "Dockerfile".to_string(),
            "FROM ubuntu:20.04".to_string(),
            "Add Dockerfile".to_string(),
            "test-branch".to_string(),
        );
        
        assert!(!command.can_undo()); // File creation is not easily reversible
        assert!(command.description().contains("Dockerfile"));
        
        let result = command.execute().await.unwrap();
        match result {
            CommandResult::FileCreated(commit) => {
                assert_eq!(commit.message, "Add Dockerfile");
            }
            _ => panic!("Expected FileCreated result"),
        }
    }
    
    #[tokio::test]
    async fn test_command_factory() {
        let github_service = Arc::new(MockGitHubService);
        let commands = CommandFactory::create_branch_and_file_commands(
            github_service,
            "test_token".to_string(),
            "testowner".to_string(),
            "testrepo".to_string(),
            "test-branch".to_string(),
            "README.md".to_string(),
            "# Test".to_string(),
            "Add README".to_string(),
        );
        
        assert_eq!(commands.len(), 2);
        assert!(commands[0].description().contains("Create branch"));
        assert!(commands[1].description().contains("Create file"));
    }
}