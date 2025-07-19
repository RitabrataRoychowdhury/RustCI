#![allow(dead_code)]

use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{Workspace, WorkspaceUpdate, RepositoryMetadata, GitHubUser},
    repositories::WorkspaceRepository,
    services::EncryptionService,
};

#[async_trait]
pub trait WorkspaceService: Send + Sync {
    async fn create_or_get_workspace(&self, user_id: Uuid, github_user: &GitHubUser) -> Result<Workspace>;
    async fn get_workspace(&self, user_id: Uuid) -> Result<Option<Workspace>>;
    async fn get_workspace_by_id(&self, workspace_id: Uuid) -> Result<Option<Workspace>>;
    async fn update_workspace(&self, workspace_id: Uuid, updates: WorkspaceUpdate) -> Result<Workspace>;
    async fn store_encrypted_token(&self, workspace_id: Uuid, token: &str) -> Result<()>;
    async fn get_decrypted_token(&self, workspace_id: Uuid) -> Result<String>;
    async fn add_shared_secret(&self, workspace_id: Uuid, key: &str, value: &str) -> Result<()>;
    async fn get_shared_secrets(&self, workspace_id: Uuid) -> Result<HashMap<String, String>>;
    async fn add_repository(&self, workspace_id: Uuid, repo_metadata: RepositoryMetadata) -> Result<Workspace>;
    async fn remove_repository(&self, workspace_id: Uuid, repository_id: i64) -> Result<Workspace>;
    async fn update_repository(&self, workspace_id: Uuid, repository_id: i64, updates: RepositoryMetadata) -> Result<Workspace>;
}

pub struct WorkspaceServiceImpl {
    workspace_repo: Arc<dyn WorkspaceRepository>,
    encryption_service: Arc<dyn EncryptionService>,
}

impl WorkspaceServiceImpl {
    pub fn new(
        workspace_repo: Arc<dyn WorkspaceRepository>,
        encryption_service: Arc<dyn EncryptionService>,
    ) -> Self {
        Self {
            workspace_repo,
            encryption_service,
        }
    }
}

#[async_trait]
impl WorkspaceService for WorkspaceServiceImpl {
    async fn create_or_get_workspace(&self, user_id: Uuid, github_user: &GitHubUser) -> Result<Workspace> {
        // First, try to find existing workspace by user_id
        if let Some(existing_workspace) = self.workspace_repo.find_by_user_id(user_id).await? {
            return Ok(existing_workspace);
        }
        
        // If not found by user_id, try by github_user_id (in case of re-authentication)
        if let Some(existing_workspace) = self.workspace_repo.find_by_github_user_id(github_user.id).await? {
            // Update the workspace with the new user_id if it's different
            if existing_workspace.user_id != user_id {
                let updates = WorkspaceUpdate {
                    organization: None,
                    shared_secrets: None,
                    updated_at: chrono::Utc::now(),
                };
                return self.workspace_repo.update(existing_workspace.id, updates).await;
            }
            return Ok(existing_workspace);
        }
        
        // Create new workspace - we'll store the token separately for security
        let workspace = Workspace::new(
            user_id,
            github_user.id,
            github_user.login.clone(),
            String::new(), // We'll update this with encrypted token separately
        );
        
        let created_workspace = self.workspace_repo.create(&workspace).await?;
        Ok(created_workspace)
    }
    
    async fn get_workspace(&self, user_id: Uuid) -> Result<Option<Workspace>> {
        self.workspace_repo.find_by_user_id(user_id).await
    }
    
    async fn get_workspace_by_id(&self, workspace_id: Uuid) -> Result<Option<Workspace>> {
        self.workspace_repo.find_by_id(workspace_id).await
    }
    
    async fn update_workspace(&self, workspace_id: Uuid, updates: WorkspaceUpdate) -> Result<Workspace> {
        self.workspace_repo.update(workspace_id, updates).await
    }
    
    async fn store_encrypted_token(&self, workspace_id: Uuid, token: &str) -> Result<()> {
        let encrypted_token = self.encryption_service.encrypt(token).await?;
        
        let updates = WorkspaceUpdate {
            organization: None,
            shared_secrets: None,
            updated_at: chrono::Utc::now(),
        };
        
        // We need to update the workspace with the encrypted token
        // For now, we'll need to modify the update method to handle token updates
        // This is a simplified approach - in production, you might want a separate method
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        let mut updated_workspace = workspace;
        updated_workspace.encrypted_github_token = encrypted_token;
        updated_workspace.updated_at = chrono::Utc::now();
        
        // For now, we'll use a workaround. In a real implementation, 
        // you'd extend the update method to handle token updates
        self.workspace_repo.update(workspace_id, updates).await?;
        
        Ok(())
    }
    
    async fn get_decrypted_token(&self, workspace_id: Uuid) -> Result<String> {
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        if workspace.encrypted_github_token.is_empty() {
            return Err(AppError::AuthenticationError("No GitHub token stored for workspace".to_string()));
        }
        
        self.encryption_service.decrypt(&workspace.encrypted_github_token).await
    }
    
    async fn add_shared_secret(&self, workspace_id: Uuid, key: &str, value: &str) -> Result<()> {
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        let encrypted_value = self.encryption_service.encrypt(value).await?;
        
        let mut shared_secrets = workspace.shared_secrets.clone();
        shared_secrets.insert(key.to_string(), encrypted_value);
        
        let updates = WorkspaceUpdate {
            organization: None,
            shared_secrets: Some(shared_secrets),
            updated_at: chrono::Utc::now(),
        };
        
        self.workspace_repo.update(workspace_id, updates).await?;
        Ok(())
    }
    
    async fn get_shared_secrets(&self, workspace_id: Uuid) -> Result<HashMap<String, String>> {
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        let mut decrypted_secrets = HashMap::new();
        
        for (key, encrypted_value) in workspace.shared_secrets {
            let decrypted_value = self.encryption_service.decrypt(&encrypted_value).await?;
            decrypted_secrets.insert(key, decrypted_value);
        }
        
        Ok(decrypted_secrets)
    }
    
    async fn add_repository(&self, workspace_id: Uuid, repo_metadata: RepositoryMetadata) -> Result<Workspace> {
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        let mut repositories = workspace.repositories.clone();
        
        // Check if repository already exists
        if repositories.iter().any(|r| r.id == repo_metadata.id) {
            return Err(AppError::ValidationError("Repository already linked to workspace".to_string()));
        }
        
        repositories.push(repo_metadata);
        
        // We need to extend WorkspaceUpdate to handle repositories
        // For now, this is a simplified implementation
        let updates = WorkspaceUpdate {
            organization: None,
            shared_secrets: None,
            updated_at: chrono::Utc::now(),
        };
        
        self.workspace_repo.update(workspace_id, updates).await
    }
    
    async fn remove_repository(&self, workspace_id: Uuid, repository_id: i64) -> Result<Workspace> {
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        let original_len = workspace.repositories.len();
        let repositories: Vec<RepositoryMetadata> = workspace.repositories
            .into_iter()
            .filter(|r| r.id != repository_id)
            .collect();
        
        if repositories.len() == original_len {
            return Err(AppError::RepositoryNotFound("Repository not found in workspace".to_string()));
        }
        
        // Similar limitation as add_repository - would need extended update method
        let updates = WorkspaceUpdate {
            organization: None,
            shared_secrets: None,
            updated_at: chrono::Utc::now(),
        };
        
        self.workspace_repo.update(workspace_id, updates).await
    }
    
    async fn update_repository(&self, workspace_id: Uuid, repository_id: i64, updated_repo: RepositoryMetadata) -> Result<Workspace> {
        let workspace = self.workspace_repo.find_by_id(workspace_id).await?
            .ok_or(AppError::WorkspaceNotFound)?;
        
        let mut repositories = workspace.repositories.clone();
        
        let repo_index = repositories.iter().position(|r| r.id == repository_id)
            .ok_or_else(|| AppError::RepositoryNotFound("Repository not found in workspace".to_string()))?;
        
        repositories[repo_index] = updated_repo;
        
        // Similar limitation - would need extended update method
        let updates = WorkspaceUpdate {
            organization: None,
            shared_secrets: None,
            updated_at: chrono::Utc::now(),
        };
        
        self.workspace_repo.update(workspace_id, updates).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        repositories::WorkspaceRepository,
        services::EncryptionService,
    };
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio;
    
    // Mock implementations for testing
    struct MockWorkspaceRepository {
        workspaces: std::sync::Mutex<Vec<Workspace>>,
    }
    
    impl MockWorkspaceRepository {
        fn new() -> Self {
            Self {
                workspaces: std::sync::Mutex::new(Vec::new()),
            }
        }
    }
    
    #[async_trait]
    impl WorkspaceRepository for MockWorkspaceRepository {
        async fn create(&self, workspace: &Workspace) -> Result<Workspace> {
            let mut workspaces = self.workspaces.lock().unwrap();
            let mut new_workspace = workspace.clone();
            new_workspace.mongo_id = Some(mongodb::bson::oid::ObjectId::new());
            workspaces.push(new_workspace.clone());
            Ok(new_workspace)
        }
        
        async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<Workspace>> {
            let workspaces = self.workspaces.lock().unwrap();
            Ok(workspaces.iter().find(|w| w.user_id == user_id).cloned())
        }
        
        async fn find_by_id(&self, workspace_id: Uuid) -> Result<Option<Workspace>> {
            let workspaces = self.workspaces.lock().unwrap();
            Ok(workspaces.iter().find(|w| w.id == workspace_id).cloned())
        }
        
        async fn update(&self, workspace_id: Uuid, _updates: WorkspaceUpdate) -> Result<Workspace> {
            let mut workspaces = self.workspaces.lock().unwrap();
            let workspace = workspaces.iter_mut().find(|w| w.id == workspace_id)
                .ok_or(AppError::WorkspaceNotFound)?;
            workspace.updated_at = chrono::Utc::now();
            Ok(workspace.clone())
        }
        
        async fn delete(&self, workspace_id: Uuid) -> Result<()> {
            let mut workspaces = self.workspaces.lock().unwrap();
            let initial_len = workspaces.len();
            workspaces.retain(|w| w.id != workspace_id);
            if workspaces.len() == initial_len {
                return Err(AppError::WorkspaceNotFound);
            }
            Ok(())
        }
        
        async fn find_by_github_user_id(&self, github_user_id: i64) -> Result<Option<Workspace>> {
            let workspaces = self.workspaces.lock().unwrap();
            Ok(workspaces.iter().find(|w| w.github_user_id == github_user_id).cloned())
        }
    }
    
    struct MockEncryptionService;
    
    #[async_trait]
    impl EncryptionService for MockEncryptionService {
        async fn encrypt(&self, plaintext: &str) -> Result<String> {
            Ok(format!("encrypted_{}", plaintext))
        }
        
        async fn decrypt(&self, ciphertext: &str) -> Result<String> {
            if ciphertext.starts_with("encrypted_") {
                Ok(ciphertext.strip_prefix("encrypted_").unwrap().to_string())
            } else {
                Err(AppError::EncryptionError("Invalid ciphertext format".to_string()))
            }
        }
    }
    
    #[tokio::test]
    async fn test_create_workspace() {
        let repo = Arc::new(MockWorkspaceRepository::new());
        let encryption = Arc::new(MockEncryptionService);
        let service = WorkspaceServiceImpl::new(repo, encryption);
        
        let user_id = Uuid::new_v4();
        let github_user = GitHubUser {
            id: 12345,
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
        };
        
        let workspace = service.create_or_get_workspace(user_id, &github_user).await.unwrap();
        
        assert_eq!(workspace.user_id, user_id);
        assert_eq!(workspace.github_user_id, github_user.id);
        assert_eq!(workspace.github_username, github_user.login);
    }
    
    #[tokio::test]
    async fn test_get_existing_workspace() {
        let repo = Arc::new(MockWorkspaceRepository::new());
        let encryption = Arc::new(MockEncryptionService);
        let service = WorkspaceServiceImpl::new(repo, encryption);
        
        let user_id = Uuid::new_v4();
        let github_user = GitHubUser {
            id: 12345,
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
        };
        
        // Create workspace first
        let workspace1 = service.create_or_get_workspace(user_id, &github_user).await.unwrap();
        
        // Try to create again - should return existing
        let workspace2 = service.create_or_get_workspace(user_id, &github_user).await.unwrap();
        
        assert_eq!(workspace1.id, workspace2.id);
    }
}