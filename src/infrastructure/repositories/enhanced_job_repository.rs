use crate::{
    domain::entities::runner::{Job, JobId, JobPriority, JobStatus, PipelineId, RunnerId},
    domain::repositories::runner::JobRepository,
    error::{AppError, Result},
    infrastructure::database::{ProductionDatabaseManager, ProductionDatabaseOperations},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{FindOptions, IndexOptions, InsertOneOptions, UpdateOptions},
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

/// Enhanced MongoDB job repository with production-grade optimizations
pub struct EnhancedMongoJobRepository {
    collection: Collection<JobDocument>,
    db_manager: Arc<ProductionDatabaseManager>,
    batch_size: usize,
    cache_ttl: Duration,
    performance_metrics: Arc<tokio::sync::RwLock<RepositoryMetrics>>,
}

/// Repository performance metrics
#[derive(Debug, Clone, Default)]
pub struct RepositoryMetrics {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub average_response_time_ms: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub batch_operations: u64,
}

/// MongoDB document representation with optimized field ordering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    
    // Hot fields - accessed most frequently
    pub job_id: String,
    pub status: JobStatus,
    pub priority: JobPriority,
    pub pipeline_id: String,
    pub runner_id: Option<String>,
    
    // Timing fields
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    
    // Job definition
    pub name: String,
    pub steps: Vec<JobStepDocument>,
    pub requirements: JobRequirementsDocument,
    pub timeout_seconds: u64,
    pub retry_policy: RetryPolicyDocument,
    pub metadata: HashMap<String, String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    
    // Execution tracking
    pub execution_log: Vec<LogEntryDocument>,
    pub artifacts: Vec<ArtifactReferenceDocument>,
    pub performance_metrics: Option<JobPerformanceMetricsDocument>,
    
    // Optimistic locking
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStepDocument {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    pub environment: HashMap<String, String>,
    pub timeout_seconds: Option<u64>,
    pub continue_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequirementsDocument {
    pub required_tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    pub min_cpu_cores: Option<u32>,
    pub min_memory_mb: Option<u32>,
    pub min_storage_gb: Option<u32>,
    pub runner_type: Option<String>,
    pub node_affinity: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicyDocument {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay_seconds: u64,
    pub max_delay_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryDocument {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub step_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactReferenceDocument {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPerformanceMetricsDocument {
    pub queue_time_microseconds: u64,
    pub routing_time_microseconds: u64,
    pub execution_time_microseconds: u64,
    pub total_time_microseconds: u64,
    pub memory_peak_mb: Option<u32>,
    pub cpu_usage_percent: Option<f64>,
}

impl EnhancedMongoJobRepository {
    /// Create a new enhanced job repository with production optimizations
    pub async fn new(
        database: &Database,
        db_manager: Arc<ProductionDatabaseManager>,
    ) -> Result<Self> {
        let collection = database.collection::<JobDocument>("jobs");
        
        let repo = Self {
            collection,
            db_manager,
            batch_size: 100, // Configurable batch size
            cache_ttl: Duration::from_secs(300), // 5 minute cache TTL
            performance_metrics: Arc::new(tokio::sync::RwLock::new(RepositoryMetrics::default())),
        };
        
        // Create optimized indexes
        repo.create_production_indexes().await?;
        
        info!("Enhanced MongoDB Job Repository initialized with production optimizations");
        Ok(repo)
    }

    /// Create production-grade indexes for optimal performance
    async fn create_production_indexes(&self) -> Result<()> {
        let indexes = vec![
            // Primary lookup index - compound index for hot path queries
            IndexModel::builder()
                .keys(doc! { 
                    "job_id": 1,
                    "status": 1,
                    "priority": -1
                })
                .options(IndexOptions::builder()
                    .unique(false)
                    .name("job_id_status_priority_idx".to_string())
                    .build())
                .build(),
            
            // Status-based queries with priority ordering
            IndexModel::builder()
                .keys(doc! { 
                    "status": 1,
                    "priority": -1,
                    "created_at": 1
                })
                .options(IndexOptions::builder()
                    .name("status_priority_created_idx".to_string())
                    .build())
                .build(),
            
            // Pipeline-based queries
            IndexModel::builder()
                .keys(doc! { 
                    "pipeline_id": 1,
                    "status": 1,
                    "created_at": -1
                })
                .options(IndexOptions::builder()
                    .name("pipeline_status_created_idx".to_string())
                    .build())
                .build(),
            
            // Runner-based queries
            IndexModel::builder()
                .keys(doc! { 
                    "runner_id": 1,
                    "status": 1,
                    "updated_at": -1
                })
                .options(IndexOptions::builder()
                    .name("runner_status_updated_idx".to_string())
                    .build())
                .build(),
            
            // Time-range queries for analytics
            IndexModel::builder()
                .keys(doc! { 
                    "created_at": 1,
                    "status": 1
                })
                .options(IndexOptions::builder()
                    .name("created_status_idx".to_string())
                    .build())
                .build(),
            
            // Queue optimization - partial index for active jobs only
            IndexModel::builder()
                .keys(doc! { 
                    "priority": -1,
                    "created_at": 1
                })
                .options(IndexOptions::builder()
                    .name("queue_priority_idx".to_string())
                    .partial_filter_expression(Some(doc! {
                        "status": { "$in": ["Queued", "Running"] }
                    }))
                    .build())
                .build(),
        ];

        self.db_manager.execute_with_retry(|| async {
            self.collection
                .create_indexes(indexes.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to create indexes: {}", e)))
        }).await?;

        debug!("Created production-grade indexes for job repository");
        Ok(())
    }

    /// Convert Job entity to optimized document format
    fn entity_to_document(&self, job: &Job) -> Result<JobDocument> {
        let steps = job.steps.iter().map(|step| JobStepDocument {
            name: step.name.clone(),
            command: step.command.clone(),
            args: step.args.clone(),
            working_directory: step.working_directory.clone(),
            environment: step.environment.clone(),
            timeout_seconds: step.timeout.map(|d| d.as_secs()),
            continue_on_error: step.continue_on_error,
        }).collect();

        let requirements = JobRequirementsDocument {
            required_tags: job.requirements.required_tags.clone(),
            excluded_tags: job.requirements.excluded_tags.clone(),
            min_cpu_cores: job.requirements.min_resources.as_ref().map(|r| r.min_cpu),
            min_memory_mb: job.requirements.min_resources.as_ref().map(|r| r.min_memory),
            min_storage_gb: job.requirements.min_resources.as_ref().and_then(|r| r.min_disk.map(|mb| mb / 1024)),
            runner_type: None, // TODO: Convert RunnerType to string
            node_affinity: job.requirements.node_affinity.clone(),
        };

        let retry_policy = RetryPolicyDocument {
            max_attempts: job.retry_policy.max_retries,
            backoff_multiplier: match &job.retry_policy.backoff_strategy {
                crate::domain::entities::runner::BackoffStrategy::Exponential { multiplier } => *multiplier,
                _ => 1.0,
            },
            initial_delay_seconds: job.retry_policy.retry_delay.as_secs(),
            max_delay_seconds: job.retry_policy.retry_delay.as_secs() * 10,
        };

        Ok(JobDocument {
            id: None,
            job_id: job.id.to_string(),
            status: JobStatus::Queued,
            priority: job.priority.clone(),
            pipeline_id: job.pipeline_id.to_string(),
            runner_id: None,
            created_at: job.created_at,
            started_at: None,
            completed_at: None,
            updated_at: Utc::now(),
            name: job.name.clone(),
            steps,
            requirements,
            timeout_seconds: job.timeout.as_secs(),
            retry_policy,
            metadata: job.metadata.clone(),
            scheduled_at: job.scheduled_at,
            execution_log: Vec::new(),
            artifacts: Vec::new(),
            performance_metrics: None,
            version: 1,
        })
    }

    /// Convert document back to Job entity with proper error handling
    fn document_to_entity(&self, doc: JobDocument) -> Result<Job> {
        let job_id = Uuid::parse_str(&doc.job_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid job ID format: {}", e)))?;
        
        let pipeline_id = Uuid::parse_str(&doc.pipeline_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid pipeline ID format: {}", e)))?;

        let steps = doc.steps.into_iter().map(|step| {
            crate::domain::entities::runner::JobStep {
                name: step.name,
                command: step.command,
                args: step.args,
                working_directory: step.working_directory,
                environment: step.environment,
                timeout: step.timeout_seconds.map(Duration::from_secs),
                continue_on_error: step.continue_on_error,
            }
        }).collect();

        let requirements = crate::domain::entities::runner::JobRequirements {
            required_tags: doc.requirements.required_tags,
            excluded_tags: doc.requirements.excluded_tags,
            min_resources: if doc.requirements.min_cpu_cores.is_some() || 
                             doc.requirements.min_memory_mb.is_some() || 
                             doc.requirements.min_storage_gb.is_some() {
                Some(crate::domain::entities::runner::ResourceRequirements {
                    min_cpu: doc.requirements.min_cpu_cores.unwrap_or(1000),
                    min_memory: doc.requirements.min_memory_mb.unwrap_or(512),
                    min_disk: doc.requirements.min_storage_gb.map(|gb| gb * 1024),
                })
            } else {
                None
            },
            runner_type: None, // TODO: Parse runner type from string
            node_affinity: doc.requirements.node_affinity,
        };

        let retry_policy = crate::domain::entities::runner::RetryPolicy {
            max_retries: doc.retry_policy.max_attempts,
            retry_delay: Duration::from_secs(doc.retry_policy.initial_delay_seconds),
            backoff_strategy: if doc.retry_policy.backoff_multiplier > 1.0 {
                crate::domain::entities::runner::BackoffStrategy::Exponential { 
                    multiplier: doc.retry_policy.backoff_multiplier 
                }
            } else {
                crate::domain::entities::runner::BackoffStrategy::Fixed
            },
        };

        Ok(Job {
            id: job_id,
            pipeline_id,
            name: doc.name,
            steps,
            requirements,
            priority: doc.priority,
            timeout: Duration::from_secs(doc.timeout_seconds),
            retry_policy,
            metadata: doc.metadata,
            created_at: doc.created_at,
            scheduled_at: doc.scheduled_at,
        })
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
        
        debug!("Repository operation '{}' completed in {:?}, success: {}", operation_type, duration, success);
    }

    /// High-performance bulk insert with batch processing
    pub async fn bulk_create_optimized(&self, jobs: &[Job]) -> Result<Vec<JobId>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }

        let start_time = Instant::now();
        let mut job_ids = Vec::with_capacity(jobs.len());
        
        // Process in batches to avoid memory issues
        for chunk in jobs.chunks(self.batch_size) {
            let documents: Result<Vec<JobDocument>> = chunk.iter()
                .map(|job| self.entity_to_document(job))
                .collect();
            
            let documents = documents?;
            
            let result = self.db_manager.execute_with_retry(|| async {
                self.collection
                    .insert_many(documents.clone(), None)
                    .await
                    .map_err(|e| AppError::DatabaseError(format!("Bulk insert failed: {}", e)))
            }).await;

            match result {
                Ok(_) => {
                    // Extract job IDs from the chunk
                    let chunk_ids: Vec<JobId> = chunk.iter().map(|job| job.id).collect();
                    job_ids.extend(chunk_ids);
                }
                Err(e) => {
                    self.record_operation("bulk_create", start_time, false).await;
                    return Err(e);
                }
            }
        }

        // Update metrics
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.batch_operations += 1;
        }
        
        self.record_operation("bulk_create", start_time, true).await;
        info!("Bulk created {} jobs in {} batches", jobs.len(), (jobs.len() + self.batch_size - 1) / self.batch_size);
        
        Ok(job_ids)
    }

    /// Optimized job queue retrieval with priority ordering and caching
    pub async fn find_next_jobs_optimized(&self, limit: usize) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        
        let options = FindOptions::builder()
            .sort(doc! { 
                "priority": -1,
                "created_at": 1
            })
            .limit(limit as i64)
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(doc! { "status": "Queued" }, options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find next jobs: {}", e)))?;

            let mut jobs = Vec::with_capacity(limit);
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_next_jobs", start_time, true).await;
                debug!("Found {} next jobs for processing", jobs.len());
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_next_jobs", start_time, false).await;
                Err(e)
            }
        }
    }

    /// Atomic job status update with optimistic locking and retry
    pub async fn atomic_status_update_enhanced(
        &self, 
        job_id: JobId, 
        old_status: JobStatus, 
        new_status: JobStatus
    ) -> Result<bool> {
        let start_time = Instant::now();
        
        let result = self.db_manager.execute_with_retry(|| async {
            let old_status_bson = mongodb::bson::to_bson(&old_status)
                .map_err(|e| AppError::DatabaseError(format!("Failed to serialize old status: {}", e)))?;
            let new_status_bson = mongodb::bson::to_bson(&new_status)
                .map_err(|e| AppError::DatabaseError(format!("Failed to serialize new status: {}", e)))?;
            
            let filter = doc! { 
                "job_id": job_id.to_string(),
                "status": old_status_bson
            };
            
            let mut update_doc = doc! { 
                "$set": { 
                    "status": new_status_bson,
                    "updated_at": mongodb::bson::DateTime::now(),
                },
                "$inc": { "version": 1 }
            };

            // Add timing fields based on status transition
            match new_status {
                JobStatus::Running => {
                    update_doc.get_document_mut("$set")
                        .map_err(|e| AppError::DatabaseError(format!("Failed to update document: {}", e)))?
                        .insert("started_at", mongodb::bson::DateTime::now());
                }
                JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled | JobStatus::TimedOut => {
                    update_doc.get_document_mut("$set")
                        .map_err(|e| AppError::DatabaseError(format!("Failed to update document: {}", e)))?
                        .insert("completed_at", mongodb::bson::DateTime::now());
                }
                _ => {}
            }

            let result = self.collection
                .update_one(filter, update_doc, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Atomic update failed: {}", e)))?;

            Ok(result.modified_count > 0)
        }).await;

        match result {
            Ok(updated) => {
                self.record_operation("atomic_status_update", start_time, true).await;
                debug!("Atomic status update for job {}: {:?} -> {:?}, success: {}", 
                       job_id, old_status, new_status, updated);
                Ok(updated)
            }
            Err(e) => {
                self.record_operation("atomic_status_update", start_time, false).await;
                Err(e)
            }
        }
    }

    /// Get repository performance metrics
    pub async fn get_performance_metrics(&self) -> RepositoryMetrics {
        self.performance_metrics.read().await.clone()
    }

    /// Reset performance metrics
    pub async fn reset_performance_metrics(&self) {
        let mut metrics = self.performance_metrics.write().await;
        *metrics = RepositoryMetrics::default();
        info!("Repository performance metrics reset");
    }
}

#[async_trait]
impl JobRepository for EnhancedMongoJobRepository {
    async fn create(&self, job: &Job) -> Result<Job> {
        let start_time = Instant::now();
        let document = self.entity_to_document(job)?;
        
        let result = self.db_manager.execute_with_retry(|| async {
            self.collection
                .insert_one(document.clone(), InsertOneOptions::builder().build())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to create job: {}", e)))
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("create", start_time, true).await;
                Ok(job.clone())
            }
            Err(e) => {
                self.record_operation("create", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_by_id(&self, job_id: JobId) -> Result<Option<Job>> {
        let start_time = Instant::now();
        let filter = doc! { "job_id": job_id.to_string() };
        
        let result = self.db_manager.execute_with_retry(|| async {
            self.collection
                .find_one(filter.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find job: {}", e)))
        }).await;

        match result {
            Ok(Some(document)) => {
                match self.document_to_entity(document) {
                    Ok(job) => {
                        self.record_operation("find_by_id", start_time, true).await;
                        Ok(Some(job))
                    }
                    Err(e) => {
                        self.record_operation("find_by_id", start_time, false).await;
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                self.record_operation("find_by_id", start_time, true).await;
                Ok(None)
            }
            Err(e) => {
                self.record_operation("find_by_id", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_all(&self) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(doc! {}, options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find all jobs: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_all", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_all", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_by_status(&self, status: JobStatus) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let status_bson = mongodb::bson::to_bson(&status)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize status: {}", e)))?;
        let filter = doc! { "status": status_bson };
        
        let options = FindOptions::builder()
            .sort(doc! { "priority": -1, "created_at": 1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by status: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_by_status", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_by_status", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_by_pipeline_id(&self, pipeline_id: PipelineId) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let filter = doc! { "pipeline_id": pipeline_id.to_string() };
        
        let options = FindOptions::builder()
            .sort(doc! { "created_at": 1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by pipeline: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_by_pipeline_id", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_by_pipeline_id", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_by_runner_id(&self, runner_id: RunnerId) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let filter = doc! { "runner_id": runner_id.to_string() };
        
        let options = FindOptions::builder()
            .sort(doc! { "updated_at": -1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by runner: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_by_runner_id", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_by_runner_id", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_queued(&self) -> Result<Vec<Job>> {
        self.find_by_status(JobStatus::Queued).await
    }

    async fn find_running(&self) -> Result<Vec<Job>> {
        self.find_by_status(JobStatus::Running).await
    }

    async fn find_by_priority(&self, priority: JobPriority) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let priority_bson = mongodb::bson::to_bson(&priority)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize priority: {}", e)))?;
        let filter = doc! { "priority": priority_bson };
        
        let options = FindOptions::builder()
            .sort(doc! { "created_at": 1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by priority: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_by_priority", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_by_priority", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_by_time_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let filter = doc! { 
            "created_at": { 
                "$gte": mongodb::bson::DateTime::from_system_time(start.into()),
                "$lte": mongodb::bson::DateTime::from_system_time(end.into())
            }
        };
        
        let options = FindOptions::builder()
            .sort(doc! { "created_at": 1 })
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by time range: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_by_time_range", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_by_time_range", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn update(&self, job: &Job) -> Result<Job> {
        let start_time = Instant::now();
        let filter = doc! { "job_id": job.id.to_string() };
        let document = self.entity_to_document(job)?;
        
        let update = doc! { 
            "$set": mongodb::bson::to_document(&document)
                .map_err(|e| AppError::DatabaseError(format!("Failed to serialize job: {}", e)))?,
            "$inc": { "version": 1 }
        };

        let result = self.db_manager.execute_with_retry(|| async {
            let result = self.collection
                .update_one(filter.clone(), update.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to update job: {}", e)))?;

            if result.matched_count == 0 {
                return Err(AppError::NotFound(format!("Job not found: {}", job.id)));
            }

            Ok(())
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("update", start_time, true).await;
                Ok(job.clone())
            }
            Err(e) => {
                self.record_operation("update", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn update_status(&self, job_id: JobId, status: JobStatus) -> Result<()> {
        // Use the enhanced atomic update method
        let updated = self.atomic_status_update_enhanced(job_id, JobStatus::Queued, status.clone()).await?
            || self.atomic_status_update_enhanced(job_id, JobStatus::Running, status.clone()).await?;

        if !updated {
            return Err(AppError::NotFound(format!("Job not found or status transition not allowed: {}", job_id)));
        }

        Ok(())
    }

    async fn delete(&self, job_id: JobId) -> Result<()> {
        let start_time = Instant::now();
        let filter = doc! { "job_id": job_id.to_string() };
        
        let result = self.db_manager.execute_with_retry(|| async {
            let result = self.collection
                .delete_one(filter.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to delete job: {}", e)))?;

            if result.deleted_count == 0 {
                return Err(AppError::NotFound(format!("Job not found: {}", job_id)));
            }

            Ok(())
        }).await;

        match result {
            Ok(_) => {
                self.record_operation("delete", start_time, true).await;
                Ok(())
            }
            Err(e) => {
                self.record_operation("delete", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn exists(&self, job_id: JobId) -> Result<bool> {
        let start_time = Instant::now();
        let filter = doc! { "job_id": job_id.to_string() };
        
        let result = self.db_manager.execute_with_retry(|| async {
            let count = self.collection
                .count_documents(filter.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to check job existence: {}", e)))?;

            Ok(count > 0)
        }).await;

        match result {
            Ok(exists) => {
                self.record_operation("exists", start_time, true).await;
                Ok(exists)
            }
            Err(e) => {
                self.record_operation("exists", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn count(&self) -> Result<u64> {
        let start_time = Instant::now();
        
        let result = self.db_manager.execute_with_retry(|| async {
            let count = self.collection
                .count_documents(doc! {}, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count jobs: {}", e)))?;

            Ok(count)
        }).await;

        match result {
            Ok(count) => {
                self.record_operation("count", start_time, true).await;
                Ok(count)
            }
            Err(e) => {
                self.record_operation("count", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn count_by_status(&self, status: JobStatus) -> Result<u64> {
        let start_time = Instant::now();
        let status_bson = mongodb::bson::to_bson(&status)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize status: {}", e)))?;
        let filter = doc! { "status": status_bson };
        
        let result = self.db_manager.execute_with_retry(|| async {
            let count = self.collection
                .count_documents(filter.clone(), None)
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to count jobs by status: {}", e)))?;

            Ok(count)
        }).await;

        match result {
            Ok(count) => {
                self.record_operation("count_by_status", start_time, true).await;
                Ok(count)
            }
            Err(e) => {
                self.record_operation("count_by_status", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(limit as i64)
            .skip(offset as u64)
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(doc! {}, options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs with pagination: {}", e)))?;

            let mut jobs = Vec::with_capacity(limit);
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_with_pagination", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_with_pagination", start_time, false).await;
                Err(e)
            }
        }
    }

    async fn find_next_job(&self) -> Result<Option<Job>> {
        let jobs = self.find_next_jobs_optimized(1).await?;
        Ok(jobs.into_iter().next())
    }

    async fn find_matching_jobs(&self, runner: &crate::domain::entities::runner::RunnerEntity) -> Result<Vec<Job>> {
        let start_time = Instant::now();
        
        // Build complex query to match jobs with runner capabilities
        let mut match_conditions = vec![
            doc! { "status": "Queued" }
        ];

        // Match required tags
        if !runner.tags.is_empty() {
            match_conditions.push(doc! {
                "$or": [
                    { "requirements.required_tags": { "$size": 0 } },
                    { "requirements.required_tags": { "$all": &runner.tags } }
                ]
            });
        }

        // Exclude jobs with excluded tags
        if !runner.tags.is_empty() {
            match_conditions.push(doc! {
                "requirements.excluded_tags": { "$nin": &runner.tags }
            });
        }

        let filter = doc! { "$and": match_conditions };
        
        let options = FindOptions::builder()
            .sort(doc! { "priority": -1, "created_at": 1 })
            .limit(100)
            .build();

        let result = self.db_manager.execute_with_retry(|| async {
            let mut cursor = self.collection
                .find(filter.clone(), options.clone())
                .await
                .map_err(|e| AppError::DatabaseError(format!("Failed to find matching jobs: {}", e)))?;

            let mut jobs = Vec::new();
            while cursor.advance().await
                .map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
                let document = cursor.deserialize_current()
                    .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
                jobs.push(self.document_to_entity(document)?);
            }

            Ok(jobs)
        }).await;

        match result {
            Ok(jobs) => {
                self.record_operation("find_matching_jobs", start_time, true).await;
                Ok(jobs)
            }
            Err(e) => {
                self.record_operation("find_matching_jobs", start_time, false).await;
                Err(e)
            }
        }
    }
}