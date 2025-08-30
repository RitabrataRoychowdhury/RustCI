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

/// MongoDB document representation of a job with optimized field ordering for performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    
    // Hot fields - accessed most frequently, placed first for cache efficiency
    pub job_id: String,
    pub status: JobStatus,
    pub priority: JobPriority,
    pub pipeline_id: String,
    pub runner_id: Option<String>,
    
    // Timing fields - critical for performance monitoring
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    
    // Job definition fields
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

/// High-performance MongoDB implementation of the job repository
/// Optimized for microsecond-level operations with advanced indexing and caching
pub struct MongoJobRepository {
    collection: Collection<JobDocument>,
    db_manager: Arc<ProductionDatabaseManager>,
    // Pre-compiled queries for maximum performance
    find_by_status_query: Document,
    find_queued_query: Document,
    find_running_query: Document,
    // Performance optimization settings
    batch_size: usize,
    cache_ttl: Duration,
}

impl MongoJobRepository {
    pub async fn new(database: &Database, db_manager: Arc<ProductionDatabaseManager>) -> Result<Self> {
        let collection = database.collection::<JobDocument>("jobs");
        
        let repo = Self {
            collection,
            db_manager,
            // Pre-compile frequently used queries to avoid runtime compilation overhead
            find_by_status_query: doc! {},
            find_queued_query: doc! { "status": "Queued" },
            find_running_query: doc! { "status": "Running" },
            // Performance optimization settings
            batch_size: 100,
            cache_ttl: Duration::from_secs(300),
        };
        
        // Create optimized indexes for microsecond-level performance
        repo.create_performance_indexes().await?;
        
        info!("MongoDB Job Repository initialized with production optimizations");
        Ok(repo)
    }

    /// Creates highly optimized indexes for maximum query performance
    async fn create_performance_indexes(&self) -> Result<()> {
        let indexes = vec![
            // Primary lookup index - compound index for hot path queries
            IndexModel::builder()
                .keys(doc! { 
                    "job_id": 1,
                    "status": 1,
                    "priority": -1  // Descending for priority ordering
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
            
            // TTL index for completed jobs cleanup (optional)
            IndexModel::builder()
                .keys(doc! { "completed_at": 1 })
                .options(IndexOptions::builder()
                    .name("completed_ttl_idx".to_string())
                    .expire_after(Some(Duration::from_secs(30 * 24 * 60 * 60))) // 30 days
                    .partial_filter_expression(Some(doc! {
                        "status": { "$in": ["Success", "Failed", "Cancelled", "TimedOut"] }
                    }))
                    .build())
                .build(),
        ];

        self.collection
            .create_indexes(indexes, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to create performance indexes: {}", e)))?;

        Ok(())
    }

    /// Converts Job entity to optimized document format
    fn entity_to_document(&self, job: &Job) -> JobDocument {
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
            min_storage_gb: job.requirements.min_resources.as_ref().and_then(|r| r.min_disk.map(|mb| mb / 1024)), // Convert MB to GB
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
            max_delay_seconds: job.retry_policy.retry_delay.as_secs() * 10, // Reasonable max
        };

        JobDocument {
            id: None,
            job_id: job.id.to_string(),
            status: JobStatus::Queued, // Default status for new jobs
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
        }
    }

    /// Converts document back to Job entity
    fn document_to_entity(&self, doc: JobDocument) -> Result<Job> {
        let job_id = Uuid::parse_str(&doc.job_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid job ID: {}", e)))?;
        
        let pipeline_id = Uuid::parse_str(&doc.pipeline_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid pipeline ID: {}", e)))?;

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
                    min_cpu: doc.requirements.min_cpu_cores.unwrap_or(1000), // millicores
                    min_memory: doc.requirements.min_memory_mb.unwrap_or(512),
                    min_disk: doc.requirements.min_storage_gb.map(|gb| gb * 1024), // Convert GB to MB
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

    /// High-performance bulk insert for job batches with connection pooling
    pub async fn bulk_create(&self, jobs: &[Job]) -> Result<Vec<JobId>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }

        let start_time = Instant::now();
        let mut job_ids = Vec::with_capacity(jobs.len());
        
        // Process in batches to avoid memory issues and improve performance
        for chunk in jobs.chunks(self.batch_size) {
            let operation = || async {
                let documents: Vec<JobDocument> = chunk.iter()
                    .map(|job| self.entity_to_document(job))
                    .collect();

                self.collection
                    .insert_many(documents, None)
                    .await
                    .map_err(|e| AppError::DatabaseError(format!(
                        "Bulk job creation failed for batch of {} jobs: {}. Context: operation=bulk_create, batch_size={}", 
                        chunk.len(), e, chunk.len()
                    )))
            };

            self.db_manager.execute_with_retry(operation).await?;
            
            // Extract job IDs from this chunk
            let chunk_ids: Vec<JobId> = chunk.iter().map(|job| job.id).collect();
            job_ids.extend(chunk_ids);
        }
        
        let duration = start_time.elapsed();
        info!("Bulk created {} jobs in {} batches in {:?}", 
              jobs.len(), 
              (jobs.len() + self.batch_size - 1) / self.batch_size,
              duration);
        
        Ok(job_ids)
    }

    /// Optimized job queue retrieval with priority ordering
    pub async fn find_next_jobs(&self, limit: usize) -> Result<Vec<Job>> {
        let options = FindOptions::builder()
            .sort(doc! { 
                "priority": -1,  // Higher priority first
                "created_at": 1   // FIFO within same priority
            })
            .limit(limit as i64)
            .build();

        let mut cursor = self.collection
            .find(self.find_queued_query.clone(), options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find next jobs: {}", e)))?;

        let mut jobs = Vec::with_capacity(limit);
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    /// Atomic job status update with optimistic locking and retry logic
    pub async fn atomic_status_update(&self, job_id: JobId, old_status: JobStatus, new_status: JobStatus) -> Result<bool> {
        let start_time = Instant::now();
        
        let operation = || async {
            let old_status_bson = mongodb::bson::to_bson(&old_status)
                .map_err(|e| AppError::DatabaseError(format!(
                    "Failed to serialize old job status: {}. Context: operation=atomic_status_update, job_id={}, old_status={:?}", 
                    e, job_id, old_status
                )))?;
            let new_status_bson = mongodb::bson::to_bson(&new_status)
                .map_err(|e| AppError::DatabaseError(format!(
                    "Failed to serialize new job status: {}. Context: operation=atomic_status_update, job_id={}, new_status={:?}", 
                    e, job_id, new_status
                )))?;
            
            let filter = doc! { 
                "job_id": job_id.to_string(),
                "status": old_status_bson
            };
            
            let mut update_fields = doc! {
                "status": new_status_bson,
                "updated_at": mongodb::bson::DateTime::now(),
            };
            
            // Add timing fields based on status transition
            if new_status == JobStatus::Running {
                update_fields.insert("started_at", mongodb::bson::DateTime::now());
            }
            
            if matches!(new_status, JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled | JobStatus::TimedOut) {
                update_fields.insert("completed_at", mongodb::bson::DateTime::now());
            }
            
            let update = doc! { 
                "$set": update_fields,
                "$inc": { "version": 1 }
            };

            self.collection
                .update_one(filter, update, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!(
                    "Atomic status update failed: {}. Context: operation=atomic_status_update, job_id={}, old_status={:?}, new_status={:?}", 
                    e, job_id, old_status, new_status
                )))
        };

        let result = self.db_manager.execute_with_retry(operation).await?;
        
        let duration = start_time.elapsed();
        let success = result.modified_count > 0;
        
        if success {
            debug!("Atomic status update completed in {:?}: {} {:?} -> {:?}", 
                   duration, job_id, old_status, new_status);
        } else {
            warn!("Atomic status update failed - no matching document: {} {:?} -> {:?}", 
                  job_id, old_status, new_status);
        }

        Ok(success)
    }
}

#[async_trait]
impl JobRepository for MongoJobRepository {
    async fn create(&self, job: &Job) -> Result<Job> {
        let start_time = Instant::now();
        
        let operation = || async {
            let document = self.entity_to_document(job);
            let options = InsertOneOptions::builder().build();
            
            self.collection
                .insert_one(document, options)
                .await
                .map_err(|e| AppError::DatabaseError(format!(
                    "Failed to create job {}: {}. Context: operation=create, job_id={}, pipeline_id={}", 
                    job.id, e, job.id, job.pipeline_id
                )))
        };

        self.db_manager.execute_with_retry(operation).await?;
        
        let duration = start_time.elapsed();
        debug!("Job created successfully in {:?}: {}", duration, job.id);
        
        Ok(job.clone())
    }

    async fn find_by_id(&self, job_id: JobId) -> Result<Option<Job>> {
        let start_time = Instant::now();
        
        let operation = || async {
            let filter = doc! { "job_id": job_id.to_string() };
            
            self.collection
                .find_one(filter, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!(
                    "Failed to find job by ID: {}. Context: operation=find_by_id, job_id={}", 
                    e, job_id
                )))
        };

        let document = self.db_manager.execute_with_retry(operation).await?;
        
        let duration = start_time.elapsed();
        debug!("Job lookup completed in {:?}: {}", duration, job_id);

        match document {
            Some(doc) => {
                let job = self.document_to_entity(doc)
                    .map_err(|e| AppError::DatabaseError(format!(
                        "Failed to convert document to entity for job {}: {}", 
                        job_id, e
                    )))?;
                Ok(Some(job))
            },
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Job>> {
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .build();

        let mut cursor = self.collection
            .find(doc! {}, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find all jobs: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn find_by_status(&self, status: JobStatus) -> Result<Vec<Job>> {
        let status_bson = mongodb::bson::to_bson(&status)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize job status: {}", e)))?;
        let filter = doc! { "status": status_bson };
        
        let options = FindOptions::builder()
            .sort(doc! { "priority": -1, "created_at": 1 })
            .build();

        let mut cursor = self.collection
            .find(filter, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by status: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn find_by_pipeline_id(&self, pipeline_id: PipelineId) -> Result<Vec<Job>> {
        let filter = doc! { "pipeline_id": pipeline_id.to_string() };
        
        let options = FindOptions::builder()
            .sort(doc! { "created_at": 1 })
            .build();

        let mut cursor = self.collection
            .find(filter, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by pipeline: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn find_by_runner_id(&self, runner_id: RunnerId) -> Result<Vec<Job>> {
        let filter = doc! { "runner_id": runner_id.to_string() };
        
        let options = FindOptions::builder()
            .sort(doc! { "updated_at": -1 })
            .build();

        let mut cursor = self.collection
            .find(filter, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by runner: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn find_queued(&self) -> Result<Vec<Job>> {
        self.find_by_status(JobStatus::Queued).await
    }

    async fn find_running(&self) -> Result<Vec<Job>> {
        self.find_by_status(JobStatus::Running).await
    }

    async fn find_by_priority(&self, priority: JobPriority) -> Result<Vec<Job>> {
        let priority_bson = mongodb::bson::to_bson(&priority)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize job priority: {}", e)))?;
        let filter = doc! { "priority": priority_bson };
        
        let options = FindOptions::builder()
            .sort(doc! { "created_at": 1 })
            .build();

        let mut cursor = self.collection
            .find(filter, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by priority: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn find_by_time_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Job>> {
        let filter = doc! { 
            "created_at": { 
                "$gte": mongodb::bson::DateTime::from_system_time(start.into()),
                "$lte": mongodb::bson::DateTime::from_system_time(end.into())
            }
        };
        
        let options = FindOptions::builder()
            .sort(doc! { "created_at": 1 })
            .build();

        let mut cursor = self.collection
            .find(filter, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs by time range: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn update(&self, job: &Job) -> Result<Job> {
        let filter = doc! { "job_id": job.id.to_string() };
        let document = self.entity_to_document(job);
        
        let update = doc! { 
            "$set": mongodb::bson::to_document(&document)
                .map_err(|e| AppError::DatabaseError(format!("Failed to serialize job: {}", e)))?,
            "$inc": { "version": 1 }
        };

        let result = self.collection
            .update_one(filter, update, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update job: {}", e)))?;

        if result.matched_count == 0 {
            return Err(AppError::NotFound(format!("Job not found: {}", job.id)));
        }

        Ok(job.clone())
    }

    async fn update_status(&self, job_id: JobId, status: JobStatus) -> Result<()> {
        let start_time = Instant::now();
        
        let operation = || async {
            let filter = doc! { "job_id": job_id.to_string() };
            let status_bson = mongodb::bson::to_bson(&status)
                .map_err(|e| AppError::DatabaseError(format!(
                    "Failed to serialize job status: {}. Context: operation=update_status, job_id={}, status={:?}", 
                    e, job_id, status
                )))?;
            
            let mut update_fields = doc! {
                "status": status_bson,
                "updated_at": mongodb::bson::DateTime::now()
            };
            
            // Add timing fields based on status
            if status == JobStatus::Running {
                update_fields.insert("started_at", mongodb::bson::DateTime::now());
            }
            
            if matches!(status, JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled | JobStatus::TimedOut) {
                update_fields.insert("completed_at", mongodb::bson::DateTime::now());
            }
            
            let update = doc! { 
                "$set": update_fields,
                "$inc": { "version": 1 }
            };

            self.collection
                .update_one(filter, update, None)
                .await
                .map_err(|e| AppError::DatabaseError(format!(
                    "Failed to update job status: {}. Context: operation=update_status, job_id={}, status={:?}", 
                    e, job_id, status
                )))
        };

        let result = self.db_manager.execute_with_retry(operation).await?;
        
        let duration = start_time.elapsed();
        
        if result.matched_count == 0 {
            return Err(AppError::NotFound(format!(
                "Job not found for status update: {}. Context: operation=update_status, status={:?}", 
                job_id, status
            )));
        }

        debug!("Job status updated in {:?}: {} -> {:?}", duration, job_id, status);
        Ok(())
    }

    async fn delete(&self, job_id: JobId) -> Result<()> {
        let filter = doc! { "job_id": job_id.to_string() };
        
        let result = self.collection
            .delete_one(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to delete job: {}", e)))?;

        if result.deleted_count == 0 {
            return Err(AppError::NotFound(format!("Job not found: {}", job_id)));
        }

        Ok(())
    }

    async fn exists(&self, job_id: JobId) -> Result<bool> {
        let filter = doc! { "job_id": job_id.to_string() };
        
        let count = self.collection
            .count_documents(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to check job existence: {}", e)))?;

        Ok(count > 0)
    }

    async fn count(&self) -> Result<u64> {
        let count = self.collection
            .count_documents(doc! {}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to count jobs: {}", e)))?;

        Ok(count)
    }

    async fn count_by_status(&self, status: JobStatus) -> Result<u64> {
        let status_bson = mongodb::bson::to_bson(&status)
            .map_err(|e| AppError::DatabaseError(format!("Failed to serialize job status: {}", e)))?;
        let filter = doc! { "status": status_bson };
        
        let count = self.collection
            .count_documents(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to count jobs by status: {}", e)))?;

        Ok(count)
    }

    async fn find_with_pagination(&self, limit: usize, offset: usize) -> Result<Vec<Job>> {
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(limit as i64)
            .skip(offset as u64)
            .build();

        let mut cursor = self.collection
            .find(doc! {}, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find jobs with pagination: {}", e)))?;

        let mut jobs = Vec::with_capacity(limit);
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }

    async fn find_next_job(&self) -> Result<Option<Job>> {
        let options = FindOptions::builder()
            .sort(doc! { 
                "priority": -1,  // Higher priority first
                "created_at": 1   // FIFO within same priority
            })
            .limit(1)
            .build();

        let find_one_options = mongodb::options::FindOneOptions::builder()
            .sort(options.sort.clone())
            .build();
        
        let document = self.collection
            .find_one(self.find_queued_query.clone(), find_one_options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find next job: {}", e)))?;

        match document {
            Some(doc) => Ok(Some(self.document_to_entity(doc)?)),
            None => Ok(None),
        }
    }

    async fn find_matching_jobs(&self, runner: &crate::domain::entities::runner::RunnerEntity) -> Result<Vec<Job>> {
        // Build a complex query to match jobs with runner capabilities
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
            .limit(100) // Reasonable limit for matching jobs
            .build();

        let mut cursor = self.collection
            .find(filter, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find matching jobs: {}", e)))?;

        let mut jobs = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(self.document_to_entity(document)?);
        }

        Ok(jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::runner::{JobStep, JobRequirements, RetryPolicy};
    use std::collections::HashMap;
    use testcontainers::clients::Cli;
    use testcontainers::images::mongo::Mongo;

    async fn create_test_repository() -> MongoJobRepository {
        let docker = Cli::default();
        let mongo_container = docker.run(Mongo::default());
        let connection_string = format!("mongodb://localhost:{}", mongo_container.get_host_port_ipv4(27017));
        
        let client = mongodb::Client::with_uri_str(&connection_string).await.unwrap();
        let database = client.database("test_jobs");
        
        // Create a mock database manager for testing
        let db_config = crate::infrastructure::database::ProductionDatabaseConfig::default();
        let db_manager = Arc::new(
            crate::infrastructure::database::ProductionDatabaseManager::new(db_config)
                .await
                .unwrap()
        );
        
        MongoJobRepository::new(&database, db_manager).await.unwrap()
    }

    fn create_test_job() -> Job {
        Job {
            id: Uuid::new_v4(),
            pipeline_id: Uuid::new_v4(),
            name: "test-job".to_string(),
            steps: vec![
                JobStep {
                    name: "build".to_string(),
                    command: "cargo".to_string(),
                    args: vec!["build".to_string()],
                    working_directory: None,
                    environment: HashMap::new(),
                    timeout: Some(Duration::from_secs(300)),
                    continue_on_error: false,
                }
            ],
            requirements: JobRequirements::default(),
            priority: JobPriority::Normal,
            timeout: Duration::from_secs(3600),
            retry_policy: RetryPolicy {
                max_retries: 3,
                retry_delay: Duration::from_secs(1),
                backoff_strategy: crate::domain::entities::runner::BackoffStrategy::Exponential { multiplier: 2.0 },
            },
            metadata: HashMap::new(),
            created_at: Utc::now(),
            scheduled_at: None,
        }
    }

    #[tokio::test]
    async fn test_job_crud_operations() {
        let repo = create_test_repository().await;
        let job = create_test_job();
        
        // Create
        let created_job = repo.create(&job).await.unwrap();
        assert_eq!(created_job.id, job.id);
        
        // Read
        let found_job = repo.find_by_id(job.id).await.unwrap();
        assert!(found_job.is_some());
        assert_eq!(found_job.unwrap().id, job.id);
        
        // Update status
        repo.update_status(job.id, JobStatus::Running).await.unwrap();
        let updated_job = repo.find_by_id(job.id).await.unwrap().unwrap();
        // Note: Status comparison would need to be implemented in the actual entity
        
        // Delete
        repo.delete(job.id).await.unwrap();
        let deleted_job = repo.find_by_id(job.id).await.unwrap();
        assert!(deleted_job.is_none());
    }

    #[tokio::test]
    async fn test_job_queue_operations() {
        let repo = create_test_repository().await;
        
        // Create multiple jobs with different priorities
        let mut jobs = Vec::new();
        for i in 0..5 {
            let mut job = create_test_job();
            job.priority = if i % 2 == 0 { JobPriority::High } else { JobPriority::Normal };
            jobs.push(job);
        }
        
        // Bulk create
        let job_ids = repo.bulk_create(&jobs).await.unwrap();
        assert_eq!(job_ids.len(), 5);
        
        // Find next jobs (should be ordered by priority)
        let next_jobs = repo.find_next_jobs(3).await.unwrap();
        assert_eq!(next_jobs.len(), 3);
        
        // Verify priority ordering
        for i in 1..next_jobs.len() {
            assert!(next_jobs[i-1].priority >= next_jobs[i].priority);
        }
    }

    #[tokio::test]
    async fn test_atomic_status_update() {
        let repo = create_test_repository().await;
        let job = create_test_job();
        
        repo.create(&job).await.unwrap();
        
        // Successful atomic update
        let success = repo.atomic_status_update(job.id, JobStatus::Queued, JobStatus::Running).await.unwrap();
        assert!(success);
        
        // Failed atomic update (wrong old status)
        let failure = repo.atomic_status_update(job.id, JobStatus::Queued, JobStatus::Success).await.unwrap();
        assert!(!failure);
    }
}