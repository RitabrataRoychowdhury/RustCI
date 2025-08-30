use crate::{
    domain::entities::runner::{RunnerEntity, RunnerId, RunnerStatus, RunnerType},
    error::{AppError, Result},
    infrastructure::database::{ProductionDatabaseManager, ProductionDatabaseOperations},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{FindOptions, IndexOptions},
    Collection, Database, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced MongoDB runner repository with production optimizations
pub struct EnhancedMongoRunnerRepository {
    collection: Collection<RunnerDocument>,
    db_manager: Arc<ProductionDatabaseManager>,
    performance_metrics: Arc<tokio::sync::RwLock<RunnerRepositoryMetrics>>,
}

/// Runner repository performance metrics
#[derive(Debug, Clone, Default)]
pub struct RunnerRepositoryMetrics {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub average_response_time_ms: f64,
    pub heartbeat_updates: u64,
    pub status_updates: u64,
}

/// MongoDB document representation of a runner with optimized structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    
    // Hot fields - accessed most frequently
    pub runner_id: String,
    pub status: RunnerStatus,
    pub last_heartbeat: DateTime<Utc>,
    
    // Runner information
    pub name: String,
    pub runner_type: RunnerTypeDocument,
    pub capacity: u32,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
    
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Versioning for optimistic locking
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RunnerTypeDocument {
    Local {
        max_concurrent_jobs: u32,
        working_directory: String,
    },
    Docker {
        max_concurrent_jobs: u32,
        docker_config: DockerConfigDocument,
    },
    Kubernetes {
        max_concurrent_jobs: u32,
        namespace: String,
        service_account: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfigDocument {
    pub endpoint: String,
    pub default_image: String,
    pub network: Option<String>,
    pub volumes: Vec<String>,
    pub environment: HashMap<String, String>,
}

/// Enhanced filter criteria for querying runners
#[derive(Debug, Default)]
pub struct EnhancedRunnerFilter {
    pub status: Option<RunnerStatus>,
    pub runner_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub capacity_min: Option<u32>,
    pub capacity_max: Option<u32>,
    pub last_heartbeat_after: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub skip: Option<u64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<i32>, // 1 for ascending, -1 for descending
}

/// Enhanced repository trait with additional operations
#[async_trait]
pub trait EnhancedRunnerRepository: Send + Sync {
    async fn store_runner(&self, runner: &RunnerEntity) -> Result<()>;
    async fn find_runner(&self, id: RunnerId) -> Result<Option<RunnerEntity>>;
    async fn list_runners(&self, filter: EnhancedRunnerFilter) -> Result<Vec<RunnerEntity>>;
    async fn update_runner_status(&self, id: RunnerId, status: RunnerStatus) -> Result<()>;
    async fn update_runner_heartbeat(&self, id: RunnerId) -> Result<()>;
    async fn remove_runner(&self, id: RunnerId) -> Result<()>;
    async fn count_runners(&self, filter: EnhancedRunnerFilter) -> Result<i64>;
    async fn find_available_runners(&self, required_tags: Vec<String>) -> Result<Vec<RunnerEntity>>;
    async fn find_runners_by_capacity(&self, min_capacity: u32) -> Result<Vec<RunnerEntity>>;
    async fn cleanup_stale_runners(&self, stale_threshold: Duration) -> Result<u64>;
    async fn get_runner_statistics(&self) -> Result<RunnerStatistics>;
}

/// Runner statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStatistics {
    pub total_runners: u64,
    pub active_runners: u64,
    pub idle_runners: u64,
    pub offline_runners: u64,
    pub total_capacity: u64,
    pub used_capacity: u64,
    pub average_heartbeat_interval_ms: f64,
}

impl EnhancedMongoRunnerRepository {
    /// Create a new enhanced runner repository
    pub async fn new(
        database: &Database,
        db_manager: Arc<ProductionDatabaseManager>,
    ) -> Result<Self> {
        let collection = database.collection::<RunnerDocument>("runners");
        
        let repo = Self {
            collection,
            db_manager,
            performance_metrics: Arc::new(tokio::sync::RwLock::new(RunnerRepositoryMetrics::default())),
        };
        
        // Create optimized indexes
        repo.create_production_indexes().await?;
        
        info!("Enhanced MongoDB Runner Repository initialized with production optimizations");
        Ok(repo)
    }

    /// Create production-grade indexes for optimal performance
    async fn create_production_indexes(&self) -> Result<()> {
        let indexes = vec![
            // Primary lookup index
            IndexModel::builder()
                .keys(doc! { "runner_id": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            
            // Status-based queries with heartbeat ordering
            IndexModel::builder()
                .keys(doc! { 
                    "status": 1,
                    "last_heartbeat": -1
                })
                .options(IndexOptions::builder()
                    .name("status_heartbeat_idx".to_string())
                    .build())
                .build(),
            
            // Tag-based queries for job matching
            IndexModel::builder()
                .keys(doc! { "tags": 1 })
                .options(IndexOptions::builder()
                    .name("tags_idx".to_string())
                    .build())
                .build(),
            
            // Capacity-based queries
            IndexModel::builder()
                .keys(doc! { 
                    "capacity": 1,
                    "status": 1
                })
                .options(IndexOptions::builder()
                    .name("capacity_status_idx".to_string())
                    .build())
                .build(),
            
            // Heartbeat monitoring index
            IndexModel::builder()
                .keys(doc! { "last_heartbeat": 1 })
                .options(IndexOptions::builder()
                    .name("heartbeat_idx".to_string())
                    .build())
                .build(),
            
            // Runner type queries
            IndexModel::builder()
                .keys(doc! { 
                    "runner_type.type": 1,
                    "status": 1
                })
                .options(IndexOptions::builder()
                    .name("type_status_idx".to_string())
                    .build())
                .build(),
        ];

        self.db_manager.execute_with_retry(|| async {
            self.collection
                .create_indexes(indexes.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to create indexes: {}", e)))
        }).await?;

        debug!("Created production-grade indexes for runner repository");
        Ok(())
    }

    /// Convert RunnerEntity to optimized document format
    fn entity_to_document(&self, runner: &RunnerEntity) -> Result<RunnerDocument> {
        let (runner_type_doc, capacity) = match &runner.runner_type {
            RunnerType::Local { max_concurrent_jobs, working_directory } => {
                (RunnerTypeDocument::Local {
                    max_concurrent_jobs: *max_concurrent_jobs,
                    working_directory: working_directory.clone(),
                }, *max_concurrent_jobs)
            }
            RunnerType::Docker { max_concurrent_jobs, docker_config } => {
                (RunnerTypeDocument::Docker {
                    max_concurrent_jobs: *max_concurrent_jobs,
                    docker_config: DockerConfigDocument {
                        endpoint: docker_config.endpoint.clone(),
                        default_image: docker_config.default_image.clone(),
                        network: docker_config.network.clone(),
                        volumes: docker_config.volumes.iter()
                            .map(|v| format!("{}:{}", v.host_path, v.container_path))
                            .collect(),
                        environment: docker_config.environment.clone(),
                    },
                }, *max_concurrent_jobs)
            }
            RunnerType::Kubernetes { namespace, max_concurrent_jobs, .. } => {
                (RunnerTypeDocument::Kubernetes {
                    max_concurrent_jobs: *max_concurrent_jobs,
                    namespace: namespace.clone(),
                    service_account: None,
                }, *max_concurrent_jobs)
            }
        };

        Ok(RunnerDocument {
            id: None,
            runner_id: runner.id.to_string(),
            name: runner.name.clone(),
            runner_type: runner_type_doc,
            status: runner.status.clone(),
            capacity,
            tags: runner.tags.clone(),
            metadata: runner.metadata.clone(),
            created_at: runner.created_at,
            updated_at: Utc::now(),
            last_heartbeat: runner.last_heartbeat,
            version: 1,
        })
    }

    /// Convert document back to RunnerEntity with proper error handling
    fn document_to_entity(&self, doc: RunnerDocument) -> Result<RunnerEntity> {
        let runner_id = Uuid::parse_str(&doc.runner_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid runner ID format: {}", e)))?;

        let runner_type = match doc.runner_type {
            RunnerTypeDocument::Local { max_concurrent_jobs, working_directory } => {
                RunnerType::Local { max_concurrent_jobs, working_directory }
            }
            RunnerTypeDocument::Docker { max_concurrent_jobs, docker_config } => {
                let volumes = docker_config.volumes.into_iter().map(|v| {
                    let parts: Vec<&str> = v.split(':').collect();
                    crate::domain::entities::runner::VolumeMount {
                        host_path: parts.get(0).unwrap_or(&"").to_string(),
                        container_path: parts.get(1).unwrap_or(&"").to_string(),
                        read_only: false,
                    }
                }).collect();
                
                RunnerType::Docker {
                    max_concurrent_jobs,
                    docker_config: crate::domain::entities::runner::DockerConfig {
                        endpoint: docker_config.endpoint,
                        default_image: docker_config.default_image,
                        network: docker_config.network,
                        volumes,
                        environment: docker_config.environment,
                    },
                }
            }
            RunnerTypeDocument::Kubernetes { max_concurrent_jobs, namespace, .. } => {
                RunnerType::Kubernetes { 
                    namespace,
                    resource_limits: crate::domain::entities::runner::ResourceLimits {
                        cpu_limit: 1000,
                        memory_limit: 1024,
                        storage_limit: Some(10240),
                    },
                    max_concurrent_jobs,
                }
            }
        };

        let mut runner = RunnerEntity::new(doc.name, runner_type);
        runner.id = runner_id;
        runner.status = doc.status;
        runner.tags = doc.tags;
        runner.metadata = doc.metadata;
        runner.created_at = doc.created_at;
        runner.last_heartbeat = doc.last_heartbeat;

        Ok(runner)
    }

    /// Build filter document from enhanced filter criteria
    fn build_filter_document(&self, filter: &EnhancedRunnerFilter) -> Result<Document> {
        let mut doc = Document::new();

        if let Some(status) = &filter.status {
            let status_bson = mongodb::bson::to_bson(status)
                .map_err(|e| AppError::DatabaseError(format!("Failed to serialize status: {}", e)))?;
            doc.insert("status", status_bson);
        }

        if let Some(runner_type) = &filter.runner_type {
            doc.insert("runner_type.type", runner_type);
        }

        if let Some(tags) = &filter.tags {
            if !tags.is_empty() {
                doc.insert("tags", doc! { "$in": tags });
            }
        }

        if let Some(capacity_min) = filter.capacity_min {
            doc.insert("capacity", doc! { "$gte": capacity_min });
        }

        if let Some(capacity_max) = filter.capacity_max {
            if let Some(existing) = doc.get_document_mut("capacity").ok() {
                existing.insert("$lte", capacity_max);
            } else {
                doc.insert("capacity", doc! { "$lte": capacity_max });
            }
        }

        if let Some(heartbeat_after) = filter.last_heartbeat_after {
            doc.insert("last_heartbeat", doc! { 
                "$gte": mongodb::bson::DateTime::from_system_time(heartbeat_after.into())
            });
        }

        Ok(doc)
    }

    /// Record operation metrics for monitoring
    async fn record_operation(&self, operation_type: &str, start_time: Instant, success: bool) {
        let duration = start_time.elapsed();
        let mut metrics = self.performance_metrics.write().await;
        
        metrics.total_operations += 1;
        if success {
            metrics.successful_operations += 1;
        } else {
            metrics.failed_operations += 1;
        }
        
        // Update average response time
        let duration_ms = duration.as_millis() as f64;
        metrics.average_response_time_ms = (metrics.average_response_time_ms * (metrics.total_operations - 1) as f64 
            + duration_ms) / metrics.total_operations as f64;
        
        // Update specific operation counters
        match operation_type {
            "update_runner_heartbeat" => metrics.heartbeat_updates += 1,
            "update_runner_status" => metrics.status_updates += 1,
            _ => {}
        }
        
        debug!("Runner repository operation '{}' completed in {:?}, success: {}", 
               operation_type, duration, success);
    }

    /// Get repository performance metrics
    pub async fn get_performance_metrics(&self) -> RunnerRepositoryMetrics {
        self.performance_metrics.read().await.clone()
    }

    /// Reset performance metrics
    pub async fn reset_performance_metrics(&self) {
        let mut metrics = self.performance_metrics.write().await;
        *metrics = RunnerRepositoryMetrics::default();
        info!("Runner repository performance metrics reset");
    }
}

#[async_trait]
impl EnhancedRunnerRepository for EnhancedMongoRunnerRepository {
    async fn store_runner(&self, runner: &RunnerEntity) -> Result<()> {
        let start_time = Instant::now();
        let document = self.entity_to_document(runner)?;
        
        let result = self.db_manager.execute_with_retry(|| async {
            self.collection
                .insert_one(document.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to store runner: {}", e)))
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("store_runner", start_time, true).await;
                debug!("Stored runner: {}", runner.id);
                Ok(())
            }
            Err(e) => {
                self.record_operation("store_runner", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_runner(&self, id: RunnerId) -> Result<Option<RunnerEntity>> {
        let start_time = Instant::now();
        let filter = doc! { "runner_id": id.to_string() };
        
        let result = self.db_manager.execute_with_retry(|| async {
            self.collection
                .find_one(filter.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find runner: {}", e)))
        }).await;

        match result {
            Ok(Some(document)) => {
                match self.document_to_entity(document) {
                    Ok(runner) => {
                        self.record_operation("find_runner", start_time, true).await;
                        Ok(Some(runner))
                    }
                    Err(e) => {
                        self.record_operation("find_runner", start_time, false).await;
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                self.record_operation("find_runner", start_time, true).await;
                Ok(None)
            }
            Err(e) => {
                self.record_operation("find_runner", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn list_runners(&self, filter: EnhancedRunnerFilter) -> Result<Vec<RunnerEntity>> {
        let start_time = Instant::now();
        let filter_doc = self.build_filter_document(&filter)?;
        
        let mut options = FindOptions::default();
        if let Some(limit) = filter.limit {
            options.limit = Some(limit);
        }
        if let Some(skip) = filter.skip {
            options.skip = Some(skip);
        }
        
        // Set sort options
        let sort_doc = if let Some(sort_by) = &filter.sort_by {
            let order = filter.sort_order.unwrap_or(1);
            doc! { sort_by: order }
        } else {
            doc! { "created_at": -1 }
        };
        options.sort = Some(sort_doc);

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter_doc.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to list runners: {}", e)))?;

            let mut runners = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize runner: {}", e)))?;
                runners.push(self.document_to_entity(document)?);
            }

            Ok(runners)
        }).await;

        match result {
            Ok(runners) => {
                self.record_operation("list_runners", start_time, true).await;
                Ok(runners)
            }
            Err(e) => {
                self.record_operation("list_runners", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn update_runner_status(&self, id: RunnerId, status: RunnerStatus) -> Result<()> {
        let start_time = Instant::now();
        let filter = doc! { "runner_id": id.to_string() };
        let status_bson = mongodb::bson::to_bson(&status)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize status: {}", e)))?;
        
        let update = doc! { 
            "$set": { 
                "status": status_bson,
                "updated_at": mongodb::bson::DateTime::now()
            },
            "$inc": { "version": 1 }
        };

        let result = self.db_manager.execute_with_retry(|| async {
            let result = self.collection
                .update_one(filter.clone(), update.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to update runner status: {}", e)))?;

            if result.matched_count == 0 {
                return Err(AppError::NotFound(format!("Runner not found: {}", id)));
            }

            Ok(())
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("update_runner_status", start_time, true).await;
                debug!("Updated runner {} status to {:?}", id, status);
                Ok(())
            }
            Err(e) => {
                self.record_operation("update_runner_status", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn update_runner_heartbeat(&self, id: RunnerId) -> Result<()> {
        let start_time = Instant::now();
        let filter = doc! { "runner_id": id.to_string() };
        let update = doc! { 
            "$set": { 
                "last_heartbeat": mongodb::bson::DateTime::now(),
                "updated_at": mongodb::bson::DateTime::now()
            },
            "$inc": { "version": 1 }
        };

        let result = self.db_manager.execute_with_retry(|| async {
            let result = self.collection
                .update_one(filter.clone(), update.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to update runner heartbeat: {}", e)))?;

            if result.matched_count == 0 {
                return Err(AppError::NotFound(format!("Runner not found: {}", id)));
            }

            Ok(())
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("update_runner_heartbeat", start_time, true).await;
                Ok(())
            }
            Err(e) => {
                self.record_operation("update_runner_heartbeat", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn remove_runner(&self, id: RunnerId) -> Result<()> {
        let start_time = Instant::now();
        let filter = doc! { "runner_id": id.to_string() };
        
        let result = self.db_manager.execute_with_retry(|| async {
            let result = self.collection
                .delete_one(filter.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to remove runner: {}", e)))?;

            if result.deleted_count == 0 {
                return Err(AppError::NotFound(format!("Runner not found: {}", id)));
            }

            Ok(())
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("remove_runner", start_time, true).await;
                info!("Removed runner: {}", id);
                Ok(())
            }
            Err(e) => {
                self.record_operation("remove_runner", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn count_runners(&self, filter: EnhancedRunnerFilter) -> Result<i64> {
        let start_time = Instant::now();
        let filter_doc = self.build_filter_document(&filter)?;
        
        let result = self.db_manager.execute_with_retry(|| async {
            let count = self.collection
                .count_documents(filter_doc.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count runners: {}", e)))?;

            Ok(count as i64)
        }).await;

        match result {
            Ok(count) => {
                self.record_operation("count_runners", start_time, true).await;
                Ok(count)
            }
            Err(e) => {
                self.record_operation("count_runners", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_available_runners(&self, required_tags: Vec<String>) -> Result<Vec<RunnerEntity>> {
        let start_time = Instant::now();
        
        let mut filter_conditions = vec![
            doc! { "status": "Idle" }
        ];

        if !required_tags.is_empty() {
            filter_conditions.push(doc! {
                "tags": { "$all": required_tags }
            });
        }

        let filter = doc! { "$and": filter_conditions };
        let options = FindOptions::builder()
            .sort(doc! { "capacity": -1, "last_heartbeat": -1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find available runners: {}", e)))?;

            let mut runners = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize runner: {}", e)))?;
                runners.push(self.document_to_entity(document)?);
            }

            Ok(runners)
        }).await;

        match result {
            Ok(runners) => {
                self.record_operation("find_available_runners", start_time, true).await;
                debug!("Found {} available runners with required tags", runners.len());
                Ok(runners)
            }
            Err(e) => {
                self.record_operation("find_available_runners", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_runners_by_capacity(&self, min_capacity: u32) -> Result<Vec<RunnerEntity>> {
        let start_time = Instant::now();
        let filter = doc! { 
            "capacity": { "$gte": min_capacity },
            "status": { "$ne": "Offline" }
        };
        
        let options = FindOptions::builder()
            .sort(doc! { "capacity": -1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find runners by capacity: {}", e)))?;

            let mut runners = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize runner: {}", e)))?;
                runners.push(self.document_to_entity(document)?);
            }

            Ok(runners)
        }).await;

        match result {
            Ok(runners) => {
                self.record_operation("find_runners_by_capacity", start_time, true).await;
                Ok(runners)
            }
            Err(e) => {
                self.record_operation("find_runners_by_capacity", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn cleanup_stale_runners(&self, stale_threshold: Duration) -> Result<u64> {
        let start_time = Instant::now();
        let threshold_time = Utc::now() - chrono::Duration::from_std(stale_threshold)
            .map_err(|e| AppError::DatabaseError(format!("Invalid duration: {}", e)))?;
        
        let filter = doc! {
            "last_heartbeat": { 
                "$lt": mongodb::bson::DateTime::from_system_time(threshold_time.into())
            },
            "status": { "$ne": "Offline" }
        };

        let result = self.db_manager.execute_with_retry(|| async {
            // First, update stale runners to offline status
            let update = doc! {
                "$set": {
                    "status": "Offline",
                    "updated_at": mongodb::bson::DateTime::now()
                }
            };

            let result = self.collection
                .update_many(filter.clone(), update, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to cleanup stale runners: {}", e)))?;

            Ok(result.modified_count)
        }).await;

        match result {
            Ok(count) => {
                self.record_operation("cleanup_stale_runners", start_time, true).await;
                if count > 0 {
                    warn!("Marked {} stale runners as offline", count);
                }
                Ok(count)
            }
            Err(e) => {
                self.record_operation("cleanup_stale_runners", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn get_runner_statistics(&self) -> Result<RunnerStatistics> {
        let start_time = Instant::now();
        
        let result = self.db_manager.execute_with_retry(|| async {
            // Get total count
            let total_runners = self.collection
                .count_documents(doc! {}, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count total runners: {}", e)))?;

            // Get status counts
            let active_runners = self.collection
                .count_documents(doc! { "status": "Busy" }, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count active runners: {}", e)))?;

            let idle_runners = self.collection
                .count_documents(doc! { "status": "Idle" }, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count idle runners: {}", e)))?;

            let offline_runners = self.collection
                .count_documents(doc! { "status": "Offline" }, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count offline runners: {}", e)))?;

            // Calculate capacity statistics using aggregation
            let pipeline = vec![
                doc! {
                    "$group": {
                        "_id": null,
                        "total_capacity": { "$sum": "$capacity" },
                        "used_capacity": {
                            "$sum": {
                                "$cond": [
                                    { "$eq": ["$status", "Busy"] },
                                    "$capacity",
                                    0
                                ]
                            }
                        }
                    }
                }
            ];

            let mut cursor = self.collection
                .aggregate(pipeline, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to aggregate capacity: {}", e)))?;

            let (total_capacity, used_capacity) = if cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Aggregation cursor error: {}", e)))? {
                let doc = cursor.current();
                let total = doc.get_i64("total_capacity").unwrap_or(0) as u64;
                let used = doc.get_i64("used_capacity").unwrap_or(0) as u64;
                (total, used)
            } else {
                (0, 0)
            };

            Ok(RunnerStatistics {
                total_runners,
                active_runners,
                idle_runners,
                offline_runners,
                total_capacity,
                used_capacity,
                average_heartbeat_interval_ms: 30000.0, // Default 30 seconds
            })
        }).await;

        match result {
            Ok(stats) => {
                self.record_operation("get_runner_statistics", start_time, true).await;
                Ok(stats)
            }
            Err(e) => {
                self.record_operation("get_runner_statistics", start_time, false).await;
                Err(e)
            }
        }
    }
}