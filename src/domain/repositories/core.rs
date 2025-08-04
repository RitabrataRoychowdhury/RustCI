//! Core repository implementations using MongoDB

use async_trait::async_trait;
use mongodb::{bson::doc, Collection, Database};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
use tracing::debug;
use uuid::Uuid;

use crate::core::infrastructure::traits::{Repository, UnitOfWork};
use crate::error::{AppError, Result};

/// MongoDB repository implementation
pub struct MongoRepository<T> {
    collection: Collection<T>,
    _phantom: PhantomData<T>,
}

impl<T> MongoRepository<T>
where
    T: Serialize + DeserializeOwned + Unpin + Send + Sync,
{
    pub fn new(database: &Database, collection_name: &str) -> Self {
        Self {
            collection: database.collection(collection_name),
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<T> Repository<T, Uuid> for MongoRepository<T>
where
    T: Serialize + DeserializeOwned + Unpin + Send + Sync + 'static,
{
    async fn find_by_id(&self, id: Uuid) -> Result<Option<T>> {
        debug!("Finding entity by ID: {}", id);
        let filter = doc! { "id": id.to_string() };
        match self.collection.find_one(filter, None).await {
            Ok(result) => Ok(result),
            Err(e) => Err(AppError::DatabaseError(format!("Find by ID failed: {}", e))),
        }
    }

    async fn find_all(&self) -> Result<Vec<T>> {
        debug!("Finding all entities");
        let mut cursor = self
            .collection
            .find(doc! {}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Find all failed: {}", e)))?;

        let mut results = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Cursor advance failed: {}", e)))?
        {
            let entity = cursor
                .deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Deserialization failed: {}", e)))?;
            results.push(entity);
        }
        Ok(results)
    }
    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<T>> {
        debug!(
            "Finding entities with pagination: limit={}, offset={}",
            limit, offset
        );

        let options = mongodb::options::FindOptions::builder()
            .limit(limit as i64)
            .skip(offset as u64)
            .build();

        let mut cursor = self
            .collection
            .find(doc! {}, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Paginated find failed: {}", e)))?;

        let mut results = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Cursor advance failed: {}", e)))?
        {
            let entity = cursor
                .deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Deserialization failed: {}", e)))?;
            results.push(entity);
        }
        Ok(results)
    }

    async fn save(&self, entity: &T) -> Result<T> {
        debug!("Saving entity");
        // For simplicity, we'll use replace_one with upsert
        // In practice, you'd want to handle insert vs update differently
        let serialized = mongodb::bson::to_document(entity)
            .map_err(|e| AppError::DatabaseError(format!("Serialization failed: {}", e)))?;

        let filter = doc! { "id": serialized.get("id") };
        let options = mongodb::options::ReplaceOptions::builder()
            .upsert(true)
            .build();

        self.collection
            .replace_one(filter, entity, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Save failed: {}", e)))?;

        // Return a clone of the entity (in practice, you might want to return the saved version)
        Ok(mongodb::bson::from_document(serialized)
            .map_err(|e| AppError::DatabaseError(format!("Deserialization failed: {}", e)))?)
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        debug!("Deleting entity by ID: {}", id);
        let filter = doc! { "id": id.to_string() };
        self.collection
            .delete_one(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Delete failed: {}", e)))?;
        Ok(())
    }

    async fn exists(&self, id: Uuid) -> Result<bool> {
        debug!("Checking if entity exists: {}", id);
        let filter = doc! { "id": id.to_string() };
        let count = self
            .collection
            .count_documents(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Exists check failed: {}", e)))?;
        Ok(count > 0)
    }

    async fn count(&self) -> Result<u64> {
        debug!("Counting all entities");
        self.collection
            .count_documents(doc! {}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Count failed: {}", e)))
    }

    async fn find_by_criteria(&self, criteria: HashMap<String, String>) -> Result<Vec<T>> {
        debug!("Finding entities by criteria: {:?}", criteria);

        let mut filter = doc! {};
        for (key, value) in criteria {
            filter.insert(key, value);
        }

        let mut cursor = self
            .collection
            .find(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Criteria find failed: {}", e)))?;

        let mut results = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Cursor advance failed: {}", e)))?
        {
            let entity = cursor
                .deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Deserialization failed: {}", e)))?;
            results.push(entity);
        }
        Ok(results)
    }
}

/// Unit of Work implementation for MongoDB
pub struct MongoUnitOfWork {
    database: Database,
    repositories: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
    transaction_active: bool,
}

impl MongoUnitOfWork {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            repositories: HashMap::new(),
            transaction_active: false,
        }
    }
}

#[async_trait]
impl UnitOfWork for MongoUnitOfWork {
    async fn begin_transaction(&mut self) -> Result<()> {
        debug!("Beginning transaction");
        // MongoDB transactions require replica sets or sharded clusters
        // For simplicity, we'll just mark the transaction as active
        self.transaction_active = true;
        Ok(())
    }

    async fn commit(&mut self) -> Result<()> {
        debug!("Committing transaction");
        if !self.transaction_active {
            return Err(AppError::InternalServerError(
                "No active transaction".to_string(),
            ));
        }
        self.transaction_active = false;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        debug!("Rolling back transaction");
        if !self.transaction_active {
            return Err(AppError::InternalServerError(
                "No active transaction".to_string(),
            ));
        }
        self.transaction_active = false;
        Ok(())
    }

    fn is_transaction_active(&self) -> bool {
        self.transaction_active
    }

    fn get_repository<T, ID>(
        &self,
        _repo_type: &str,
    ) -> Result<std::sync::Arc<dyn Repository<T, ID>>> {
        // This is a simplified implementation
        // In practice, you'd want proper type-safe repository management
        Err(AppError::InternalServerError(
            "Repository retrieval not fully implemented".to_string(),
        ))
    }
}
