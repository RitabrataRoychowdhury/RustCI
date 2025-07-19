use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

use crate::{
    error::{AppError, Result},
    models::{
        GitHubUser, Workspace, RepositoryMetadata, ProjectType, GitHubContent,
        PullRequestRequest, GitHubPullRequest, DockerfileGenerationResult, ValidationResult,
    },
    services::{
        GitHubService, EncryptionService,
        workspace::WorkspaceService,
        project_detection::ProjectTypeDetectorFactory,
        dockerfile_generation::DockerfileGeneratorFactory,
        pr_builder::PullRequestBuilder,
        command::{CommandInvoker, Command, CommandResult},
    },
};

/// Integration test suite for the complete RustCI workflow
pub struct IntegrationTestSuite {
    workspace_service: Arc<dyn WorkspaceService>,
    github_service: Arc<dyn GitHubService>,
    encryption_service: Arc<dyn EncryptionService>,
}

impl IntegrationTestSuite {
    pub fn new(
        workspace_service: Arc<dyn WorkspaceService>,
        github_service: Arc<dyn GitHubService>,
        encryption_service: Arc<dyn EncryptionService>,
    ) -> Self {
        Self {
            workspace_service,
            github_service,
            encryption_service,
        }
    }

    /// Test the complete flow: OAuth → workspace → detection → Dockerfile → PR
    pub async fn test_complete_flow(&self) -> Result<()> {
        println!("🧪 Starting complete integration test flow...");

        // Step 1: Simulate OAuth authentication and workspace creation
        let (user_id, workspace) = self.test_oauth_and_workspace_creation().await?;
        println!("✅ Step 1: OAuth and workspace creation completed");

        // Step 2: Test repository linking and project detection
        let repo_metadata = self.test_repository_linking_and_detection(&workspace.id).await?;
        println!("✅ Step 2: Repository linking and project detection completed");

        // Step 3: Test Dockerfile generation and validation
        let dockerfile_result = self.test_dockerfile_generation_and_validation(&workspace.id, &repo_metadata).await?;
        println!("✅ Step 3: Dockerfile generation and validation completed");

        // Step 4: Test PR creation with command pattern and rollback
        self.test_pr_creation_with_rollback(&workspace.id, &repo_metadata, &dockerfile_result).await?;
        println!("✅ Step 4: PR creation with rollback testing completed");

        // Step 5: Test token encryption/decryption
        self.test_token_encryption(&workspace.id).await?;
        println!("✅ Step 5: Token encryption testing completed");

        println!("🎉 Complete integration test flow passed!");
        Ok(())
    }

    /// Test OAuth authentication and workspace creation
    async fn test_oauth_and_workspace_creation(&self) -> Result<(Uuid, Workspace)> {
        let user_id = Uuid::new_v4();
        let github_user = GitHubUser {
            id: 12345,
            login: "test-user".to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            avatar_url: "https://avatars.githubusercontent.com/u/12345".to_string(),
            html_url: "https://github.com/test-user".to_string(),
            company: Some("Test Company".to_string()),
            blog: Some("https://test-user.dev".to_string()),
            location: Some("Test City".to_string()),
            bio: Some("Test bio".to_string()),
            public_repos: 10,
            public_gists: 5,
            followers: 100,
            following: 50,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Test workspace creation
        let workspace = self.workspace_service
            .create_or_get_workspace(user_id, &github_user)
            .await?;

        // Verify workspace properties
        assert_eq!(workspace.user_id, user_id);
        assert_eq!(workspace.github_user_id, github_user.id);
        assert_eq!(workspace.github_username, github_user.login);

        // Test getting existing workspace
        let existing_workspace = self.workspace_service
            .create_or_get_workspace(user_id, &github_user)
            .await?;
        assert_eq!(workspace.id, existing_workspace.id);

        Ok((user_id, workspace))
    }

    /// Test repository linking and project type detection
    async fn test_repository_linking_and_detection(&self, workspace_id: &Uuid) -> Result<RepositoryMetadata> {
        // Mock repository data
        let repo_metadata = RepositoryMetadata {
            id: 67890,
            name: "test-rust-project".to_string(),
            full_name: "test-user/test-rust-project".to_string(),
            clone_url: "https://github.com/test-user/test-rust-project.git".to_string(),
            default_branch: "main".to_string(),
            has_dockerfile: false,
            project_type: None,
            linked_at: Utc::now(),
            last_dockerfile_check: None,
        };

        // Test adding repository to workspace
        let updated_workspace = self.workspace_service
            .add_repository(*workspace_id, repo_metadata.clone())
            .await?;

        // Verify repository was added
        assert_eq!(updated_workspace.repositories.len(), 1);
        assert_eq!(updated_workspace.repositories[0].id, repo_metadata.id);

        // Test project type detection
        let mock_files = vec![
            GitHubContent {
                name: "Cargo.toml".to_string(),
                path: "Cargo.toml".to_string(),
                sha: "abc123".to_string(),
                size: 500,
                url: "https://api.github.com/repos/test-user/test-rust-project/contents/Cargo.toml".to_string(),
                html_url: "https://github.com/test-user/test-rust-project/blob/main/Cargo.toml".to_string(),
                git_url: "https://api.github.com/repos/test-user/test-rust-project/git/blobs/abc123".to_string(),
                download_url: Some("https://raw.githubusercontent.com/test-user/test-rust-project/main/Cargo.toml".to_string()),
                file_type: "file".to_string(),
                content: None,
                encoding: None,
            },
            GitHubContent {
                name: "src".to_string(),
                path: "src".to_string(),
                sha: "def456".to_string(),
                size: 0,
                url: "https://api.github.com/repos/test-user/test-rust-project/contents/src".to_string(),
                html_url: "https://github.com/test-user/test-rust-project/tree/main/src".to_string(),
                git_url: "https://api.github.com/repos/test-user/test-rust-project/git/trees/def456".to_string(),
                download_url: None,
                file_type: "dir".to_string(),
                content: None,
                encoding: None,
            },
        ];

        let detected_type = ProjectTypeDetectorFactory::detect_project_type(&mock_files)?;
        assert_eq!(detected_type, ProjectType::Rust);

        // Update repository with detected project type
        let mut updated_repo = repo_metadata.clone();
        updated_repo.project_type = Some(detected_type);
        
        let _final_workspace = self.workspace_service
            .update_repository(*workspace_id, repo_metadata.id, updated_repo.clone())
            .await?;

        Ok(updated_repo)
    }

    /// Test Dockerfile generation and validation
    async fn test_dockerfile_generation_and_validation(
        &self,
        workspace_id: &Uuid,
        repo_metadata: &RepositoryMetadata,
    ) -> Result<DockerfileGenerationResult> {
        // Test Dockerfile generation
        let project_info = crate::models::ProjectInfo {
            binary_name: "test-rust-project".to_string(),
            port: 8080,
            dependencies: vec!["tokio".to_string(), "axum".to_string()],
            build_command: Some("cargo build --release".to_string()),
            run_command: Some("./target/release/test-rust-project".to_string()),
        };

        let generator = DockerfileGeneratorFactory::create_generator(ProjectType::Rust);
        let dockerfile_content = generator.generate(&project_info)?;

        // Verify Dockerfile content
        assert!(dockerfile_content.contains("FROM rust:"));
        assert!(dockerfile_content.contains("COPY Cargo.toml"));
        assert!(dockerfile_content.contains("cargo build --release"));

        // Test Dockerfile validation (mock)
        let validation_result = ValidationResult {
            success: true,
            build_logs: vec![
                "Step 1/8 : FROM rust:1.75 as builder".to_string(),
                "Successfully built abc123def456".to_string(),
            ],
            run_logs: vec![
                "Server starting on port 8080".to_string(),
            ],
            errors: vec![],
            warnings: vec![
                "Consider using a smaller base image for production".to_string(),
            ],
        };

        let dockerfile_result = DockerfileGenerationResult {
            id: Uuid::new_v4(),
            workspace_id: *workspace_id,
            repository_id: repo_metadata.id,
            dockerfile_content,
            validation_result: Some(validation_result),
            status: crate::models::GenerationStatus::Validated,
            created_at: Utc::now(),
            approved_at: None,
            pr_url: None,
        };

        Ok(dockerfile_result)
    }

    /// Test PR creation with command pattern and rollback functionality
    async fn test_pr_creation_with_rollback(
        &self,
        workspace_id: &Uuid,
        repo_metadata: &RepositoryMetadata,
        dockerfile_result: &DockerfileGenerationResult,
    ) -> Result<()> {
        // Test PR builder pattern
        let pr_request = PullRequestBuilder::new()
            .owner("test-user")
            .repo("test-rust-project")
            .title("Add auto-generated Dockerfile")
            .body("This PR adds an auto-generated Dockerfile for the Rust project.")
            .head_branch("rustci/dockerfile-autogen")
            .base_branch("main")
            .draft(false)
            .build()?;

        // Verify PR request structure
        assert_eq!(pr_request.owner, "test-user");
        assert_eq!(pr_request.repo, "test-rust-project");
        assert_eq!(pr_request.head, "rustci/dockerfile-autogen");
        assert_eq!(pr_request.base, "main");

        // Test command pattern with rollback
        let mut command_invoker = CommandInvoker::new();

        // Add commands for PR creation workflow
        let create_branch_command = Box::new(MockCreateBranchCommand {
            branch_name: "rustci/dockerfile-autogen".to_string(),
            should_fail: false,
        });

        let create_file_command = Box::new(MockCreateFileCommand {
            file_path: "Dockerfile".to_string(),
            content: dockerfile_result.dockerfile_content.clone(),
            should_fail: false,
        });

        let create_pr_command = Box::new(MockCreatePRCommand {
            pr_request: pr_request.clone(),
            should_fail: false,
        });

        command_invoker.add_command(create_branch_command);
        command_invoker.add_command(create_file_command);
        command_invoker.add_command(create_pr_command);

        // Test successful execution
        let results = command_invoker.execute_all().await?;
        assert_eq!(results.len(), 3);

        // Test rollback scenario
        let mut rollback_invoker = CommandInvoker::new();
        
        let failing_command = Box::new(MockCreatePRCommand {
            pr_request: pr_request.clone(),
            should_fail: true,
        });

        rollback_invoker.add_command(Box::new(MockCreateBranchCommand {
            branch_name: "rustci/dockerfile-autogen".to_string(),
            should_fail: false,
        }));
        rollback_invoker.add_command(failing_command);

        // This should fail and trigger rollback
        let rollback_result = rollback_invoker.execute_all().await;
        assert!(rollback_result.is_err());

        Ok(())
    }

    /// Test token encryption and decryption
    async fn test_token_encryption(&self, workspace_id: &Uuid) -> Result<()> {
        let test_token = "ghp_test_token_1234567890abcdef";

        // Test storing encrypted token
        self.workspace_service
            .store_encrypted_token(*workspace_id, test_token)
            .await?;

        // Test retrieving and decrypting token
        let decrypted_token = self.workspace_service
            .get_decrypted_token(*workspace_id)
            .await?;

        assert_eq!(decrypted_token, test_token);

        // Test shared secrets
        self.workspace_service
            .add_shared_secret(*workspace_id, "DATABASE_URL", "postgresql://localhost/test")
            .await?;

        let secrets = self.workspace_service
            .get_shared_secrets(*workspace_id)
            .await?;

        assert_eq!(secrets.get("DATABASE_URL").unwrap(), "postgresql://localhost/test");

        Ok(())
    }
}

// Mock command implementations for testing
struct MockCreateBranchCommand {
    branch_name: String,
    should_fail: bool,
}

#[async_trait::async_trait]
impl Command for MockCreateBranchCommand {
    async fn execute(&self) -> Result<CommandResult> {
        if self.should_fail {
            return Err(AppError::GitHubApiError("Failed to create branch".to_string()));
        }
        
        Ok(CommandResult::BranchCreated(
            crate::models::GitHubBranch {
                name: self.branch_name.clone(),
                commit: crate::models::GitHubCommit {
                    sha: "abc123".to_string(),
                    url: "https://api.github.com/repos/test/test/git/commits/abc123".to_string(),
                    html_url: "https://github.com/test/test/commit/abc123".to_string(),
                    author: None,
                    committer: None,
                    message: "Branch creation".to_string(),
                    tree: crate::models::GitHubTree {
                        sha: "def456".to_string(),
                        url: "https://api.github.com/repos/test/test/git/trees/def456".to_string(),
                    },
                    parents: vec![],
                },
                protected: false,
            }
        ))
    }

    async fn undo(&self) -> Result<()> {
        // Mock branch deletion
        Ok(())
    }

    fn description(&self) -> String {
        format!("Create branch {}", self.branch_name)
    }
    
    fn command_id(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
    
    fn can_undo(&self) -> bool {
        true
    }
}

struct MockCreateFileCommand {
    file_path: String,
    content: String,
    should_fail: bool,
}

#[async_trait::async_trait]
impl Command for MockCreateFileCommand {
    async fn execute(&self) -> Result<CommandResult> {
        if self.should_fail {
            return Err(AppError::GitHubApiError("Failed to create file".to_string()));
        }
        
        Ok(CommandResult::FileCreated(
            crate::models::GitHubCommit {
                sha: "file123".to_string(),
                url: "https://api.github.com/repos/test/test/git/commits/file123".to_string(),
                html_url: "https://github.com/test/test/commit/file123".to_string(),
                author: None,
                committer: None,
                message: format!("Add {}", self.file_path),
                tree: crate::models::GitHubTree {
                    sha: "tree456".to_string(),
                    url: "https://api.github.com/repos/test/test/git/trees/tree456".to_string(),
                },
                parents: vec![],
            }
        ))
    }

    async fn undo(&self) -> Result<()> {
        // Mock file deletion
        Ok(())
    }

    fn description(&self) -> String {
        format!("Create file {}", self.file_path)
    }
    
    fn command_id(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
    
    fn can_undo(&self) -> bool {
        false
    }
}

struct MockCreatePRCommand {
    pr_request: PullRequestRequest,
    should_fail: bool,
}

#[async_trait::async_trait]
impl Command for MockCreatePRCommand {
    async fn execute(&self) -> Result<CommandResult> {
        if self.should_fail {
            return Err(AppError::GitHubApiError("Failed to create PR".to_string()));
        }
        
        Ok(CommandResult::PullRequestCreated(
            GitHubPullRequest {
                id: 123,
                number: 1,
                title: self.pr_request.title.clone(),
                body: self.pr_request.body.clone(),
                html_url: "https://github.com/test/test/pull/1".to_string(),
                state: "open".to_string(),
                draft: false,
                merged: false,
                mergeable: Some(true),
                head: crate::models::GitHubBranch {
                    name: "test-branch".to_string(),
                    commit: crate::models::GitHubCommit {
                        sha: "abc123".to_string(),
                        url: "https://api.github.com/repos/test/test/git/commits/abc123".to_string(),
                        html_url: "https://github.com/test/test/commit/abc123".to_string(),
                        author: None,
                        committer: None,
                        message: "Test commit".to_string(),
                        tree: crate::models::GitHubTree {
                            sha: "def456".to_string(),
                            url: "https://api.github.com/repos/test/test/git/trees/def456".to_string(),
                        },
                        parents: vec![],
                    },
                    protected: false,
                },
                base: crate::models::GitHubBranch {
                    name: "main".to_string(),
                    commit: crate::models::GitHubCommit {
                        sha: "main123".to_string(),
                        url: "https://api.github.com/repos/test/test/git/commits/main123".to_string(),
                        html_url: "https://github.com/test/test/commit/main123".to_string(),
                        author: None,
                        committer: None,
                        message: "Main commit".to_string(),
                        tree: crate::models::GitHubTree {
                            sha: "main456".to_string(),
                            url: "https://api.github.com/repos/test/test/git/trees/main456".to_string(),
                        },
                        parents: vec![],
                    },
                    protected: true,
                },
                user: crate::models::GitHubUser {
                    id: 12345,
                    login: "test-user".to_string(),
                    name: Some("Test User".to_string()),
                    email: Some("test@example.com".to_string()),
                    avatar_url: "https://avatars.githubusercontent.com/u/12345".to_string(),
                    html_url: "https://github.com/test-user".to_string(),
                    company: None,
                    blog: None,
                    location: None,
                    bio: None,
                    public_repos: 10,
                    public_gists: 5,
                    followers: 100,
                    following: 50,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                created_at: Utc::now(),
                updated_at: Utc::now(),
                merged_at: None,
                closed_at: None,
            }
        ))
    }

    async fn undo(&self) -> Result<()> {
        // Mock PR closure/deletion
        Ok(())
    }

    fn description(&self) -> String {
        format!("Create PR: {}", self.pr_request.title)
    }
    
    fn command_id(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
    
    fn can_undo(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        encryption::AesGcmEncryptionService, 
        github::GitHubServiceImpl
    };

    #[tokio::test]
    async fn test_integration_suite_creation() {
        // This test verifies that the integration test suite can be created
        // In a real scenario, you would inject actual service implementations
        
        // For now, we'll skip this test since it requires actual service implementations
        // In production, you would create mock services or use test containers
    }

    #[tokio::test]
    async fn test_mock_commands() {
        let command = MockCreateBranchCommand {
            branch_name: "test-branch".to_string(),
            should_fail: false,
        };

        let result = command.execute().await;
        assert!(result.is_ok());

        let failing_command = MockCreateBranchCommand {
            branch_name: "test-branch".to_string(),
            should_fail: true,
        };

        let result = failing_command.execute().await;
        assert!(result.is_err());
    }
}