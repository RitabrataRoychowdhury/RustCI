//! MongoDB implementation of SAGA persistence
//!
//! This module provides MongoDB-based persistence for SAGA executions
//! with proper indexing, querying, and performance optimization.

use async_trait::async_trait;
use mongodb::{
    bson::doc,
    options::{FindOptions, IndexOptions},
    Collection, IndexModel,
};
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::core::sagas::{SagaExecution, SagaPersistence, SagaStatistics, SagaStatus};
use crate::domain::repositories::advanced::AdvancedDatabaseManager;
use crate::error::{AppError, Result};

/// MongoDB implementation of SAGA persistence
pub struct MongoSagaPersistence {
    collection: Collection<SagaExecution>,
    db_manager: Arc<AdvancedDatabaseManager>,
}

impl MongoSagaPersistence {
    /// Create a new MongoDB SAGA persistence
    pub async fn new(
        db_manager: Arc<AdvancedDatabaseManager>,
        collection_name: Option<&str>,
    ) -> Result<Self> {
        let database = db_manager.get_database(false).await?;
        let collection_name = collection_name.unwrap_or("saga_executions");
        let collection = database.collection(collection_name);

        let persistence = Self {
            collection,
            db_manager,
        };

        // Create indexes for performance
        persistence.create_indexes().await?;

        Ok(persistence)
    }

    /// Create necessary indexes for SAGA collections
    async fn create_indexes(&self) -> Result<()> {
        info!("Creating SAGA persistence indexes...");

        let indexes = vec![
            // Index on saga_id (primary key)
            IndexModel::builder()
                .keys(doc! {"saga_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            // Index on correlation_id for tracing
            IndexModel::builder()
                .keys(doc! {"correlation_id": 1})
                .build(),
            // Index on status for querying by status
            IndexModel::builder().keys(doc! {"status": 1}).build(),
            // Compound index on status and started_at for time-based queries
            IndexModel::builder()
                .keys(doc! {"status": 1, "started_at": -1})
                .build(),
            // Index on saga_name for querying by saga type
            IndexModel::builder().keys(doc! {"saga_name": 1}).build(),
            // TTL index for automatic cleanup of old completed sagas (30 days)
            IndexModel::builder()
                .keys(doc! {"completed_at": 1})
                .options(
                    IndexOptions::builder()
                        .expire_after(std::time::Duration::from_secs(30 * 24 * 60 * 60))
                        .partial_filter_expression(doc! {
                            "status": {"$in": ["Completed", "Compensated", "Failed"]}
                        })
                        .build(),
                )
                .build(),
        ];

        self.collection
            .create_indexes(indexes, None)
            .await
            .map_err(|e| {
                AppError::DatabaseError(format!("Failed to create SAGA indexes: {}", e))
            })?;

        info!("✅ SAGA persistence indexes created successfully");
        Ok(())
    }
}

#[async_trait]
impl SagaPersistence for MongoSagaPersistence {
    async fn save_execution(&self, execution: &SagaExecution) -> Result<()> {
        debug!(
            saga_id = %execution.saga_id,
            status = ?execution.status,
            "Saving SAGA execution"
        );

        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();
        let execution = execution.clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();
                let execution = execution.clone();

                Box::pin(async move {
                    // Use replace_one with upsert to handle both insert and update
                    let filter = doc! {"saga_id": execution.saga_id.to_string()};
                    let options = mongodb::options::ReplaceOptions::builder()
                        .upsert(true)
                        .build();

                    collection
                        .replace_one(filter, &execution, options)
                        .await
                        .map_err(|e| {
                            AppError::DatabaseError(format!("Failed to save SAGA execution: {}", e))
                        })?;

                    Ok(())
                })
            })
            .await
    }

    async fn load_execution(&self, saga_id: Uuid) -> Result<Option<SagaExecution>> {
        debug!(saga_id = %saga_id, "Loading SAGA execution");

        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();

                Box::pin(async move {
                    let filter = doc! {"saga_id": saga_id.to_string()};
                    collection.find_one(filter, None).await.map_err(|e| {
                        AppError::DatabaseError(format!("Failed to load SAGA execution: {}", e))
                    })
                })
            })
            .await
    }

    async fn find_by_status(&self, status: SagaStatus) -> Result<Vec<SagaExecution>> {
        debug!(status = ?status, "Finding SAGA executions by status");

        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();

        db_manager.execute_with_retry(move || {
            let collection = collection.clone();
            let status = status.clone();

            Box::pin(async move {
                let filter = doc! {"status": mongodb::bson::to_bson(&status)
                    .map_err(|e| AppError::DatabaseError(format!("Failed to serialize status: {}", e)))?};

                let options = FindOptions::builder()
                    .sort(doc! {"started_at": -1})
                    .limit(1000) // Limit to prevent memory issues
                    .build();

                let mut cursor = collection.find(filter, options).await
                    .map_err(|e| AppError::DatabaseError(format!("Failed to query SAGA executions: {}", e)))?;

                let mut executions = Vec::new();
                while cursor.advance().await.map_err(|e|
                    AppError::DatabaseError(format!("Cursor advance failed: {}", e)))? {
                    let execution = cursor.deserialize_current().map_err(|e|
                        AppError::DatabaseError(format!("Deserialization failed: {}", e)))?;
                    executions.push(execution);
                }

                Ok(executions)
            })
        }).await
    }

    async fn find_by_correlation_id(&self, correlation_id: Uuid) -> Result<Vec<SagaExecution>> {
        debug!(correlation_id = %correlation_id, "Finding SAGA executions by correlation ID");

        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();

                Box::pin(async move {
                    let filter = doc! {"correlation_id": correlation_id.to_string()};

                    let options = FindOptions::builder().sort(doc! {"started_at": -1}).build();

                    let mut cursor = collection.find(filter, options).await.map_err(|e| {
                        AppError::DatabaseError(format!("Failed to query SAGA executions: {}", e))
                    })?;

                    let mut executions = Vec::new();
                    while cursor.advance().await.map_err(|e| {
                        AppError::DatabaseError(format!("Cursor advance failed: {}", e))
                    })? {
                        let execution = cursor.deserialize_current().map_err(|e| {
                            AppError::DatabaseError(format!("Deserialization failed: {}", e))
                        })?;
                        executions.push(execution);
                    }

                    Ok(executions)
                })
            })
            .await
    }

    async fn delete_execution(&self, saga_id: Uuid) -> Result<()> {
        debug!(saga_id = %saga_id, "Deleting SAGA execution");

        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();

                Box::pin(async move {
                    let filter = doc! {"saga_id": saga_id.to_string()};
                    collection.delete_one(filter, None).await.map_err(|e| {
                        AppError::DatabaseError(format!("Failed to delete SAGA execution: {}", e))
                    })?;

                    Ok(())
                })
            })
            .await
    }

    async fn get_statistics(&self) -> Result<SagaStatistics> {
        debug!("Calculating SAGA statistics");

        let db_manager = self.db_manager.clone();
        let collection = self.collection.clone();

        db_manager
            .execute_with_retry(move || {
                let collection = collection.clone();

                Box::pin(async move {
                    // Use aggregation pipeline for efficient statistics calculation
                    let pipeline = vec![doc! {
                        "$group": {
                            "_id": null,
                            "total": {"$sum": 1},
                            "completed": {
                                "$sum": {
                                    "$cond": [
                                        {"$eq": ["$status", "Completed"]},
                                        1,
                                        0
                                    ]
                                }
                            },
                            "failed": {
                                "$sum": {
                                    "$cond": [
                                        {"$eq": ["$status", "Failed"]},
                                        1,
                                        0
                                    ]
                                }
                            },
                            "compensated": {
                                "$sum": {
                                    "$cond": [
                                        {"$eq": ["$status", "Compensated"]},
                                        1,
                                        0
                                    ]
                                }
                            },
                            "avg_duration": {
                                "$avg": {
                                    "$cond": [
                                        {"$ne": ["$completed_at", null]},
                                        {
                                            "$subtract": [
                                                "$completed_at",
                                                "$started_at"
                                            ]
                                        },
                                        null
                                    ]
                                }
                            }
                        }
                    }];

                    let mut cursor = collection.aggregate(pipeline, None).await.map_err(|e| {
                        AppError::DatabaseError(format!("Failed to calculate statistics: {}", e))
                    })?;

                    if cursor.advance().await.map_err(|e| {
                        AppError::DatabaseError(format!("Cursor advance failed: {}", e))
                    })? {
                        let doc = cursor.deserialize_current().map_err(|e| {
                            AppError::DatabaseError(format!("Deserialization failed: {}", e))
                        })?;

                        let total = doc.get_i64("total").unwrap_or(0) as u64;
                        let completed = doc.get_i64("completed").unwrap_or(0) as u64;
                        let failed = doc.get_i64("failed").unwrap_or(0) as u64;
                        let compensated = doc.get_i64("compensated").unwrap_or(0) as u64;
                        let avg_duration_ms = doc.get_f64("avg_duration").unwrap_or(0.0);

                        let success_rate = if total > 0 {
                            completed as f64 / total as f64
                        } else {
                            0.0
                        };

                        Ok(SagaStatistics {
                            total_executions: total,
                            completed_executions: completed,
                            failed_executions: failed,
                            compensated_executions: compensated,
                            average_duration_ms: avg_duration_ms,
                            success_rate,
                        })
                    } else {
                        // No data found
                        Ok(SagaStatistics {
                            total_executions: 0,
                            completed_executions: 0,
                            failed_executions: 0,
                            compensated_executions: 0,
                            average_duration_ms: 0.0,
                            success_rate: 0.0,
                        })
                    }
                })
            })
            .await
    }
}

/// SAGA cleanup service for managing old executions
#[allow(dead_code)]
pub struct SagaCleanupService {
    persistence: Arc<dyn SagaPersistence>,
    cleanup_config: SagaCleanupConfig,
}

#[derive(Debug, Clone)]
pub struct SagaCleanupConfig {
    pub retention_days: u32,
    pub batch_size: usize,
    pub cleanup_interval: std::time::Duration,
}

impl Default for SagaCleanupConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            batch_size: 100,
            cleanup_interval: std::time::Duration::from_secs(24 * 60 * 60), // Daily
        }
    }
}

impl SagaCleanupService {
    /// Create a new SAGA cleanup service
    pub fn new(persistence: Arc<dyn SagaPersistence>, config: SagaCleanupConfig) -> Self {
        Self {
            persistence,
            cleanup_config: config,
        }
    }

    /// Start the cleanup service
    pub async fn start(&self) {
        let persistence = self.persistence.clone();
        let config = self.cleanup_config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.cleanup_interval);

            loop {
                interval.tick().await;

                if let Err(e) = Self::cleanup_old_executions(&persistence, &config).await {
                    error!("SAGA cleanup failed: {}", e);
                } else {
                    debug!("SAGA cleanup completed successfully");
                }
            }
        });
    }

    /// Cleanup old SAGA executions
    async fn cleanup_old_executions(
        persistence: &Arc<dyn SagaPersistence>,
        config: &SagaCleanupConfig,
    ) -> Result<()> {
        info!("Starting SAGA cleanup process...");

        // Find completed sagas older than retention period
        let completed_sagas = persistence.find_by_status(SagaStatus::Completed).await?;
        let compensated_sagas = persistence.find_by_status(SagaStatus::Compensated).await?;
        let failed_sagas = persistence.find_by_status(SagaStatus::Failed).await?;

        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(config.retention_days as i64);
        let mut deleted_count = 0;

        for saga in completed_sagas
            .into_iter()
            .chain(compensated_sagas)
            .chain(failed_sagas)
        {
            if let Some(completed_at) = saga.completed_at {
                if completed_at < cutoff_date {
                    if let Err(e) = persistence.delete_execution(saga.saga_id).await {
                        error!("Failed to delete SAGA {}: {}", saga.saga_id, e);
                    } else {
                        deleted_count += 1;
                    }
                }
            }

            // Process in batches to avoid overwhelming the database
            if deleted_count % config.batch_size == 0 && deleted_count > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        if deleted_count > 0 {
            info!(
                "✅ SAGA cleanup completed: {} executions deleted",
                deleted_count
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sagas::{SagaContext, SagaExecution, SagaStatus};
    use chrono::Utc;

    #[test]
    fn test_saga_cleanup_config() {
        let config = SagaCleanupConfig::default();
        assert_eq!(config.retention_days, 30);
        assert_eq!(config.batch_size, 100);
        assert!(config.cleanup_interval > std::time::Duration::from_secs(0));
    }

    #[test]
    fn test_saga_execution_serialization() {
        let context = SagaContext::new(Uuid::new_v4(), Uuid::new_v4());
        let execution = SagaExecution {
            saga_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            saga_name: "test-saga".to_string(),
            status: SagaStatus::Completed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            current_step: 2,
            total_steps: 3,
            step_results: Vec::new(),
            context,
            error: None,
        };

        // Test that the execution can be serialized/deserialized
        let serialized = mongodb::bson::to_bson(&execution).unwrap();
        assert!(serialized.as_document().is_some());
    }
}
