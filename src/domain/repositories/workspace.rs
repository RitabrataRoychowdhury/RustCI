#![allow(dead_code)]

use async_trait::async_trait;
use mongodb::{bson::doc, Collection, Database};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    domain::entities::{Workspace, WorkspaceUpdate},
    error::{AppError, Result},
};

#[async_trait]
pub trait WorkspaceRepository: Send + Sync {
    async fn create(&self, workspace: &Workspace) -> Result<Workspace>;
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<Workspace>>;
    async fn find_by_id(&self, workspace_id: Uuid) -> Result<Option<Workspace>>;
    async fn update(&self, workspace_id: Uuid, updates: WorkspaceUpdate) -> Result<Workspace>;
    async fn delete(&self, workspace_id: Uuid) -> Result<()>;
    async fn find_by_github_user_id(&self, github_user_id: i64) -> Result<Option<Workspace>>;
}

pub struct MongoWorkspaceRepository {
    collection: Collection<Workspace>,
}

impl MongoWorkspaceRepository {
    pub fn new(database: Arc<Database>) -> Self {
        let collection = database.collection::<Workspace>("workspaces");
        Self { collection }
    }

    pub async fn create_indexes(&self) -> Result<()> {
        use mongodb::bson::doc;
        use mongodb::IndexModel;

        let indexes = vec![
            IndexModel::builder().keys(doc! { "user_id": 1 }).build(),
            IndexModel::builder()
                .keys(doc! { "github_user_id": 1 })
                .build(),
            IndexModel::builder().keys(doc! { "id": 1 }).build(),
        ];

        self.collection
            .create_indexes(indexes, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to create indexes: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl WorkspaceRepository for MongoWorkspaceRepository {
    async fn create(&self, workspace: &Workspace) -> Result<Workspace> {
        let mut workspace_to_insert = workspace.clone();

        let insert_result = self
            .collection
            .insert_one(&workspace_to_insert, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to create workspace: {}", e)))?;

        workspace_to_insert.mongo_id = Some(insert_result.inserted_id.as_object_id().unwrap());

        Ok(workspace_to_insert)
    }

    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<Workspace>> {
        let filter = doc! { "user_id": user_id.to_string() };

        let workspace = self.collection.find_one(filter, None).await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to find workspace by user_id: {}", e))
        })?;

        Ok(workspace)
    }

    async fn find_by_id(&self, workspace_id: Uuid) -> Result<Option<Workspace>> {
        let filter = doc! { "id": workspace_id.to_string() };

        let workspace = self.collection.find_one(filter, None).await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to find workspace by id: {}", e))
        })?;

        Ok(workspace)
    }

    async fn update(&self, workspace_id: Uuid, updates: WorkspaceUpdate) -> Result<Workspace> {
        let filter = doc! { "id": workspace_id.to_string() };

        let mut update_doc = doc! {
            "$set": {
                "updated_at": mongodb::bson::DateTime::from_system_time(updates.updated_at.into())
            }
        };

        if let Some(organization) = &updates.organization {
            update_doc
                .get_document_mut("$set")
                .unwrap()
                .insert("organization", organization);
        }

        if let Some(shared_secrets) = &updates.shared_secrets {
            let bson_secrets = mongodb::bson::to_bson(shared_secrets).map_err(|e| {
                AppError::DatabaseError(format!("Failed to serialize shared_secrets: {}", e))
            })?;
            update_doc
                .get_document_mut("$set")
                .unwrap()
                .insert("shared_secrets", bson_secrets);
        }

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build();

        let updated_workspace = self
            .collection
            .find_one_and_update(filter, update_doc, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update workspace: {}", e)))?
            .ok_or(AppError::WorkspaceNotFound)?;

        Ok(updated_workspace)
    }

    async fn delete(&self, workspace_id: Uuid) -> Result<()> {
        let filter = doc! { "id": workspace_id.to_string() };

        let delete_result = self
            .collection
            .delete_one(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to delete workspace: {}", e)))?;

        if delete_result.deleted_count == 0 {
            return Err(AppError::WorkspaceNotFound);
        }

        Ok(())
    }

    async fn find_by_github_user_id(&self, github_user_id: i64) -> Result<Option<Workspace>> {
        let filter = doc! { "github_user_id": github_user_id };

        let workspace = self.collection.find_one(filter, None).await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to find workspace by github_user_id: {}", e))
        })?;

        Ok(workspace)
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::models::Workspace;
    // use chrono::Utc;
    // use std::collections::HashMap;

    // Note: These tests would require a test MongoDB instance
    // For now, they serve as documentation of expected behavior

    #[tokio::test]
    #[ignore] // Ignore by default since it requires MongoDB
    async fn test_create_workspace() {
        // This test would create a test database connection
        // and test the create functionality
    }

    #[tokio::test]
    #[ignore]
    async fn test_find_by_user_id() {
        // Test finding workspace by user ID
    }

    #[tokio::test]
    #[ignore]
    async fn test_update_workspace() {
        // Test updating workspace with new data
    }
}
