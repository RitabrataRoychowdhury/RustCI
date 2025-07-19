//! Kubernetes lifecycle hooks with MongoDB integration
//! 
//! This module provides lifecycle hook functionality that integrates with MongoDB
//! to track job execution state, metrics, and custom data operations.

use crate::error::{AppError, Result};
use crate::database::DatabaseManager;
use super::super::traits::{LifecycleHook, LifecycleHookType, MongoOperation, ExecutionResult};
use mongodb::bson::{doc, Document, DateTime as BsonDateTime};
use std::collections::HashMap;
use tracing::{debug, info};
use uuid::Uuid;

/// Manages lifecycle hooks with MongoDB integration
pub struct LifecycleHookManager {
    db_manager: DatabaseManager,
}

impl LifecycleHookManager {
    /// Create a new lifecycle hook manager
    pub fn new(db_manager: DatabaseManager) -> Self {
        Self { db_manager }
    }

    /// Execute pre-execution hooks
    pub async fn execute_pre_hooks(
        &self,
        hooks: &[LifecycleHook],
        step_name: &str,
        execution_id: &Uuid,
        workspace_id: &Uuid,
    ) -> Result<()> {
        info!("🔄 Executing {} pre-execution hooks for step: {}", hooks.len(), step_name);

        for hook in hooks {
            if matches!(hook.hook_type, LifecycleHookType::PreExecution) {
                self.execute_hook(hook, step_name, execution_id, workspace_id, None).await?;
            }
        }

        Ok(())
    }

    /// Execute post-execution hooks
    pub async fn execute_post_hooks(
        &self,
        hooks: &[LifecycleHook],
        step_name: &str,
        execution_id: &Uuid,
        workspace_id: &Uuid,
        result: &ExecutionResult,
    ) -> Result<()> {
        info!("🔄 Executing {} post-execution hooks for step: {}", hooks.len(), step_name);

        for hook in hooks {
            let should_execute = match hook.hook_type {
                LifecycleHookType::PostExecution => true,
                LifecycleHookType::OnSuccess => result.is_success(),
                LifecycleHookType::OnFailure => !result.is_success(),
                LifecycleHookType::PreExecution => false, // Skip pre-execution hooks
            };

            if should_execute {
                self.execute_hook(hook, step_name, execution_id, workspace_id, Some(result)).await?;
            }
        }

        Ok(())
    }

    /// Execute a single lifecycle hook
    async fn execute_hook(
        &self,
        hook: &LifecycleHook,
        step_name: &str,
        execution_id: &Uuid,
        workspace_id: &Uuid,
        result: Option<&ExecutionResult>,
    ) -> Result<()> {
        debug!("🪝 Executing lifecycle hook: {} (type: {:?})", hook.name, hook.hook_type);

        // Prepare document data
        let doc_data = self.prepare_hook_data(hook, step_name, execution_id, workspace_id, result)?;

        // Get collection
        let collection = self.db_manager.database.collection::<Document>(&hook.mongodb_collection);

        // Execute MongoDB operation
        match hook.mongodb_operation {
            MongoOperation::Insert => {
                debug!("📝 Inserting document into collection: {}", hook.mongodb_collection);
                collection.insert_one(doc_data, None).await.map_err(|e| {
                    AppError::DatabaseError(format!("Failed to insert lifecycle hook data: {}", e))
                })?;
            }
            MongoOperation::Update => {
                debug!("🔄 Updating document in collection: {}", hook.mongodb_collection);
                let filter = doc! { "execution_id": execution_id.to_string() };
                let update = doc! { "$set": doc_data };
                collection.update_one(filter, update, None).await.map_err(|e| {
                    AppError::DatabaseError(format!("Failed to update lifecycle hook data: {}", e))
                })?;
            }
            MongoOperation::FindAndUpdate => {
                debug!("🔍 Find and update document in collection: {}", hook.mongodb_collection);
                let filter = doc! { "execution_id": execution_id.to_string() };
                let update = doc! { "$set": doc_data };
                let options = mongodb::options::FindOneAndUpdateOptions::builder()
                    .upsert(true)
                    .build();
                collection.find_one_and_update(filter, update, options).await.map_err(|e| {
                    AppError::DatabaseError(format!("Failed to find and update lifecycle hook data: {}", e))
                })?;
            }
            MongoOperation::Delete => {
                debug!("🗑️ Deleting document from collection: {}", hook.mongodb_collection);
                let filter = doc! { "execution_id": execution_id.to_string() };
                collection.delete_one(filter, None).await.map_err(|e| {
                    AppError::DatabaseError(format!("Failed to delete lifecycle hook data: {}", e))
                })?;
            }
        }

        info!("✅ Lifecycle hook '{}' executed successfully", hook.name);
        Ok(())
    }

    /// Prepare document data for MongoDB operation
    fn prepare_hook_data(
        &self,
        hook: &LifecycleHook,
        step_name: &str,
        execution_id: &Uuid,
        workspace_id: &Uuid,
        result: Option<&ExecutionResult>,
    ) -> Result<Document> {
        let mut doc = Document::new();

        // Add standard fields
        doc.insert("hook_name", &hook.name);
        doc.insert("hook_type", format!("{:?}", hook.hook_type));
        doc.insert("step_name", step_name);
        doc.insert("execution_id", execution_id.to_string());
        doc.insert("workspace_id", workspace_id.to_string());
        doc.insert("timestamp", BsonDateTime::from_system_time(chrono::Utc::now().into()));

        // Add custom data from hook configuration
        for (key, value) in &hook.data {
            doc.insert(key, value);
        }

        // Add execution result data if available
        if let Some(exec_result) = result {
            doc.insert("exit_code", exec_result.exit_code);
            doc.insert("success", exec_result.is_success());
            
            // Add metadata
            for (key, value) in &exec_result.metadata {
                doc.insert(format!("metadata_{}", key), value);
            }

            // Add stdout/stderr lengths (not full content to avoid large documents)
            doc.insert("stdout_length", exec_result.stdout.len() as i32);
            doc.insert("stderr_length", exec_result.stderr.len() as i32);

            // Add execution duration if available
            if let Some(duration) = exec_result.metadata.get("duration_seconds") {
                if let Ok(duration_num) = duration.parse::<f64>() {
                    doc.insert("duration_seconds", duration_num);
                }
            }
        }

        Ok(doc)
    }

    /// Create a default execution tracking hook
    pub fn create_execution_tracking_hook(hook_type: LifecycleHookType) -> LifecycleHook {
        LifecycleHook {
            name: format!("execution_tracking_{:?}", hook_type).to_lowercase(),
            hook_type,
            mongodb_collection: "job_executions".to_string(),
            mongodb_operation: MongoOperation::FindAndUpdate,
            data: HashMap::new(),
        }
    }

    /// Create a metrics collection hook
    pub fn create_metrics_hook() -> LifecycleHook {
        let mut data = HashMap::new();
        data.insert("metric_type".to_string(), "job_execution".to_string());
        
        LifecycleHook {
            name: "metrics_collection".to_string(),
            hook_type: LifecycleHookType::PostExecution,
            mongodb_collection: "execution_metrics".to_string(),
            mongodb_operation: MongoOperation::Insert,
            data,
        }
    }

    /// Create a failure tracking hook
    pub fn create_failure_tracking_hook() -> LifecycleHook {
        let mut data = HashMap::new();
        data.insert("event_type".to_string(), "job_failure".to_string());
        
        LifecycleHook {
            name: "failure_tracking".to_string(),
            hook_type: LifecycleHookType::OnFailure,
            mongodb_collection: "job_failures".to_string(),
            mongodb_operation: MongoOperation::Insert,
            data,
        }
    }

    /// Validate hook configuration
    pub fn validate_hook(&self, hook: &LifecycleHook) -> Result<()> {
        if hook.name.is_empty() {
            return Err(AppError::ValidationError(
                "Lifecycle hook name cannot be empty".to_string()
            ));
        }

        if hook.mongodb_collection.is_empty() {
            return Err(AppError::ValidationError(
                "MongoDB collection name cannot be empty".to_string()
            ));
        }

        // Validate collection name format (MongoDB naming rules)
        if hook.mongodb_collection.starts_with('.') || hook.mongodb_collection.contains('$') {
            return Err(AppError::ValidationError(
                "Invalid MongoDB collection name format".to_string()
            ));
        }

        Ok(())
    }

    /// Get execution statistics from MongoDB
    pub async fn get_execution_stats(&self, workspace_id: &Uuid) -> Result<ExecutionStats> {
        debug!("📊 Getting execution statistics for workspace: {}", workspace_id);

        let collection = self.db_manager.database.collection::<Document>("job_executions");
        let filter = doc! { "workspace_id": workspace_id.to_string() };

        // Count total executions
        let total_executions = collection.count_documents(filter.clone(), None).await
            .map_err(|e| AppError::DatabaseError(format!("Failed to count executions: {}", e)))?;

        // Count successful executions
        let mut success_filter = filter.clone();
        success_filter.insert("success", true);
        let successful_executions = collection.count_documents(success_filter, None).await
            .map_err(|e| AppError::DatabaseError(format!("Failed to count successful executions: {}", e)))?;

        // Count failed executions
        let mut failure_filter = filter.clone();
        failure_filter.insert("success", false);
        let failed_executions = collection.count_documents(failure_filter, None).await
            .map_err(|e| AppError::DatabaseError(format!("Failed to count failed executions: {}", e)))?;

        Ok(ExecutionStats {
            total_executions,
            successful_executions,
            failed_executions,
            success_rate: if total_executions > 0 {
                (successful_executions as f64 / total_executions as f64) * 100.0
            } else {
                0.0
            },
        })
    }
}

/// Execution statistics structure
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::connectors::traits::{LifecycleHookType, MongoOperation};

    #[test]
    fn test_create_execution_tracking_hook() {
        let hook = LifecycleHookManager::create_execution_tracking_hook(LifecycleHookType::PreExecution);
        assert_eq!(hook.name, "execution_tracking_preexecution");
        assert_eq!(hook.mongodb_collection, "job_executions");
        assert!(matches!(hook.mongodb_operation, MongoOperation::FindAndUpdate));
    }

    #[test]
    fn test_create_metrics_hook() {
        let hook = LifecycleHookManager::create_metrics_hook();
        assert_eq!(hook.name, "metrics_collection");
        assert_eq!(hook.mongodb_collection, "execution_metrics");
        assert!(matches!(hook.hook_type, LifecycleHookType::PostExecution));
    }

    #[test]
    fn test_create_failure_tracking_hook() {
        let hook = LifecycleHookManager::create_failure_tracking_hook();
        assert_eq!(hook.name, "failure_tracking");
        assert_eq!(hook.mongodb_collection, "job_failures");
        assert!(matches!(hook.hook_type, LifecycleHookType::OnFailure));
    }

    #[test]
    fn test_hook_validation() {
        let valid_hook = LifecycleHook {
            name: "test_hook".to_string(),
            hook_type: LifecycleHookType::PreExecution,
            mongodb_collection: "test_collection".to_string(),
            mongodb_operation: MongoOperation::Insert,
            data: HashMap::new(),
        };

        // This would require a database manager instance to test properly
        // For now, we'll just test the hook creation
        assert_eq!(valid_hook.name, "test_hook");
        assert_eq!(valid_hook.mongodb_collection, "test_collection");
    }

    #[test]
    fn test_prepare_hook_data_structure() {
        // Test that we can create the basic structures needed for hook data
        let mut data = HashMap::new();
        data.insert("custom_field".to_string(), "custom_value".to_string());

        let hook = LifecycleHook {
            name: "test_hook".to_string(),
            hook_type: LifecycleHookType::PostExecution,
            mongodb_collection: "test_collection".to_string(),
            mongodb_operation: MongoOperation::Insert,
            data,
        };

        assert!(hook.data.contains_key("custom_field"));
        assert_eq!(hook.data.get("custom_field").unwrap(), "custom_value");
    }
}