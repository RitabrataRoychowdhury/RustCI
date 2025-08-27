use crate::{
    domain::entities::runner::{RunnerEntity, RunnerId, RunnerStatus, RunnerType},
    error::{AppError, Result},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{FindOptions, IndexOptions},
    Collection, Database, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// MongoDB document representation of a runner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub runner_id: String,
    pub name: String,
    pub runner_type: RunnerTypeDocument,
    pub status: RunnerStatus,
    pub capacity: u32,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
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

/// Filter criteria for querying runners
#[derive(Debug, Default)]
pub struct RunnerFilter {
    pub status: Option<RunnerStatus>,
    pub runner_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub capacity_min: Option<u32>,
    pub limit: Option<i64>,
    pub skip: Option<u64>,
}

/// Repository trait for runner operations
#[async_trait]
pub trait RunnerRepository: Send + Sync {
    async fn store_runner(&self, runner: &RunnerEntity) -> Result<()>;
    async fn find_runner(&self, id: RunnerId) -> Result<Option<RunnerEntity>>;
    async fn list_runners(&self, filter: RunnerFilter) -> Result<Vec<RunnerEntity>>;
    async fn update_runner_status(&self, id: RunnerId, status: RunnerStatus) -> Result<()>;
    async fn update_runner_heartbeat(&self, id: RunnerId) -> Result<()>;
    async fn remove_runner(&self, id: RunnerId) -> Result<()>;
    async fn count_runners(&self, filter: RunnerFilter) -> Result<i64>;
}

/// MongoDB implementation of the runner repository
pub struct MongoRunnerRepository {
    collection: Collection<RunnerDocument>,
}

impl MongoRunnerRepository {
    pub async fn new(database: &Database) -> Result<Self> {
        let collection = database.collection::<RunnerDocument>("runners");
        
        // Create indexes for efficient querying
        let repo = Self { collection };
        repo.create_indexes().await?;
        
        Ok(repo)
    }

    async fn create_indexes(&self) -> Result<()> {
        let indexes = vec![
            IndexModel::builder()
                .keys(doc! { "runner_id": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            IndexModel::builder()
                .keys(doc! { "status": 1 })
                .build(),
            IndexModel::builder()
                .keys(doc! { "tags": 1 })
                .build(),
            IndexModel::builder()
                .keys(doc! { "last_heartbeat": 1 })
                .build(),
            IndexModel::builder()
                .keys(doc! { "created_at": 1 })
                .build(),
        ];

        self.collection
            .create_indexes(indexes, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to create indexes: {}", e)))?;

        Ok(())
    }

    fn entity_to_document(&self, runner: &RunnerEntity) -> RunnerDocument {
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
                        volumes: docker_config.volumes.iter().map(|v| format!("{}:{}", v.host_path, v.container_path)).collect(),
                        environment: docker_config.environment.clone(),
                    },
                }, *max_concurrent_jobs)
            }
            RunnerType::Kubernetes { namespace, resource_limits, max_concurrent_jobs } => {
                (RunnerTypeDocument::Kubernetes {
                    max_concurrent_jobs: *max_concurrent_jobs,
                    namespace: namespace.clone(),
                    service_account: None, // Not used in current entity
                }, *max_concurrent_jobs)
            }
        };

        RunnerDocument {
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
            version: 1, // Start with version 1
        }
    }

    fn document_to_entity(&self, doc: RunnerDocument) -> Result<RunnerEntity> {
        let runner_id = Uuid::parse_str(&doc.runner_id)
            .map_err(|e| AppError::DatabaseError(format!("Invalid runner ID: {}", e)))?;

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
                        cpu_limit: 1000, // Default values
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

    fn build_filter_document(&self, filter: &RunnerFilter) -> Document {
        let mut doc = Document::new();

        if let Some(status) = &filter.status {
            doc.insert("status", mongodb::bson::to_bson(status).unwrap());
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

        doc
    }
}

#[async_trait]
impl RunnerRepository for MongoRunnerRepository {
    async fn store_runner(&self, runner: &RunnerEntity) -> Result<()> {
        let document = self.entity_to_document(runner);
        
        self.collection
            .insert_one(document, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to store runner: {}", e)))?;

        Ok(())
    }

    async fn find_runner(&self, id: RunnerId) -> Result<Option<RunnerEntity>> {
        let filter = doc! { "runner_id": id.to_string() };
        
        let document = self.collection
            .find_one(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to find runner: {}", e)))?;

        match document {
            Some(doc) => Ok(Some(self.document_to_entity(doc)?)),
            None => Ok(None),
        }
    }

    async fn list_runners(&self, filter: RunnerFilter) -> Result<Vec<RunnerEntity>> {
        let filter_doc = self.build_filter_document(&filter);
        
        let mut options = FindOptions::default();
        if let Some(limit) = filter.limit {
            options.limit = Some(limit);
        }
        if let Some(skip) = filter.skip {
            options.skip = Some(skip);
        }
        options.sort = Some(doc! { "created_at": -1 });

        let mut cursor = self.collection
            .find(filter_doc, options)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to list runners: {}", e)))?;

        let mut runners = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::DatabaseError(format!("Cursor error: {}", e)))? {
            let document = cursor.deserialize_current()
                .map_err(|e| AppError::DatabaseError(format!("Failed to deserialize runner: {}", e)))?;
            runners.push(self.document_to_entity(document)?);
        }

        Ok(runners)
    }

    async fn update_runner_status(&self, id: RunnerId, status: RunnerStatus) -> Result<()> {
        let filter = doc! { "runner_id": id.to_string() };
        let update = doc! { 
            "$set": { 
                "status": mongodb::bson::to_bson(&status).unwrap(),
                "updated_at": mongodb::bson::DateTime::from_chrono(Utc::now())
            },
            "$inc": { "version": 1 }
        };

        let result = self.collection
            .update_one(filter, update, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update runner status: {}", e)))?;

        if result.matched_count == 0 {
            return Err(AppError::NotFound(format!("Runner not found: {}", id)));
        }

        Ok(())
    }

    async fn update_runner_heartbeat(&self, id: RunnerId) -> Result<()> {
        let filter = doc! { "runner_id": id.to_string() };
        let update = doc! { 
            "$set": { 
                "last_heartbeat": mongodb::bson::DateTime::from_chrono(Utc::now()),
                "updated_at": mongodb::bson::DateTime::from_chrono(Utc::now())
            },
            "$inc": { "version": 1 }
        };

        let result = self.collection
            .update_one(filter, update, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update runner heartbeat: {}", e)))?;

        if result.matched_count == 0 {
            return Err(AppError::NotFound(format!("Runner not found: {}", id)));
        }

        Ok(())
    }

    async fn remove_runner(&self, id: RunnerId) -> Result<()> {
        let filter = doc! { "runner_id": id.to_string() };
        
        let result = self.collection
            .delete_one(filter, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to remove runner: {}", e)))?;

        if result.deleted_count == 0 {
            return Err(AppError::NotFound(format!("Runner not found: {}", id)));
        }

        Ok(())
    }

    async fn count_runners(&self, filter: RunnerFilter) -> Result<i64> {
        let filter_doc = self.build_filter_document(&filter);
        
        let count = self.collection
            .count_documents(filter_doc, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to count runners: {}", e)))?;

        Ok(count as i64)
    }
}