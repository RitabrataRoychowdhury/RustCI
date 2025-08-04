//! Pipeline Manager - Handles pipeline CRUD operations and validation
//!
//! This manager is responsible for pipeline lifecycle management,
//! validation, and persistence operations.

use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::ci::config::CIPipeline;
use crate::core::patterns::events::EventBus;
use crate::error::{AppError, Result};
use crate::infrastructure::database::DatabaseManager;

/// Pipeline Manager handles pipeline operations
pub struct PipelineManager {
    db: Arc<DatabaseManager>,
    event_bus: Arc<EventBus>,
}

impl PipelineManager {
    /// Create a new pipeline manager
    pub fn new(db: DatabaseManager, event_bus: Arc<EventBus>) -> Self {
        Self {
            db: Arc::new(db),
            event_bus,
        }
    }

    /// Create a new pipeline
    pub async fn create_pipeline(&self, mut pipeline: CIPipeline) -> Result<Uuid> {
        info!("Creating new pipeline: {}", pipeline.name);

        // Validate pipeline
        self.validate_pipeline(&pipeline).await?;

        // Generate ID if not provided
        let pipeline_id = pipeline.id.unwrap_or_else(Uuid::new_v4);
        pipeline.id = Some(pipeline_id);
        pipeline.created_at = Some(chrono::Utc::now());
        pipeline.updated_at = Some(chrono::Utc::now());

        // Store in database
        self.store_pipeline(&pipeline).await?;

        // Publish pipeline created event
        self.publish_pipeline_created_event(pipeline_id, &pipeline)
            .await?;

        info!(
            "Pipeline created successfully: {} (ID: {})",
            pipeline.name, pipeline_id
        );
        Ok(pipeline_id)
    }

    /// Get a pipeline by ID
    pub async fn get_pipeline(&self, pipeline_id: Uuid) -> Result<CIPipeline> {
        debug!("Retrieving pipeline: {}", pipeline_id);

        let pipeline = self.get_pipeline_from_db(pipeline_id).await?;

        debug!("Pipeline retrieved: {}", pipeline.name);
        Ok(pipeline)
    }

    /// Update an existing pipeline
    pub async fn update_pipeline(&self, mut pipeline: CIPipeline) -> Result<()> {
        let pipeline_id = pipeline.id.ok_or_else(|| {
            AppError::ValidationError("Pipeline ID is required for updates".to_string())
        })?;

        info!("Updating pipeline: {} (ID: {})", pipeline.name, pipeline_id);

        // Validate pipeline
        self.validate_pipeline(&pipeline).await?;

        // Update timestamp
        pipeline.updated_at = Some(chrono::Utc::now());

        // Update in database
        self.update_pipeline_in_db(&pipeline).await?;

        // Publish pipeline updated event
        self.publish_pipeline_updated_event(pipeline_id, &pipeline)
            .await?;

        info!("Pipeline updated successfully: {}", pipeline_id);
        Ok(())
    }

    /// Delete a pipeline
    pub async fn delete_pipeline(&self, pipeline_id: Uuid) -> Result<()> {
        info!("Deleting pipeline: {}", pipeline_id);

        // Check if pipeline exists
        let _pipeline = self.get_pipeline(pipeline_id).await?;

        // Delete from database
        self.delete_pipeline_from_db(pipeline_id).await?;

        // Publish pipeline deleted event
        self.publish_pipeline_deleted_event(pipeline_id).await?;

        info!("Pipeline deleted successfully: {}", pipeline_id);
        Ok(())
    }

    /// List all pipelines
    pub async fn list_pipelines(&self) -> Result<Vec<CIPipeline>> {
        debug!("Listing all pipelines");

        let pipelines = self.get_all_pipelines().await?;

        debug!("Retrieved {} pipelines", pipelines.len());
        Ok(pipelines)
    }

    /// Validate pipeline configuration
    pub async fn validate_pipeline(&self, pipeline: &CIPipeline) -> Result<()> {
        debug!("Validating pipeline: {}", pipeline.name);

        // Basic validation
        if pipeline.name.is_empty() {
            return Err(AppError::ValidationError(
                "Pipeline name cannot be empty".to_string(),
            ));
        }

        if pipeline.stages.is_empty() {
            return Err(AppError::ValidationError(
                "Pipeline must have at least one stage".to_string(),
            ));
        }

        // Validate each stage
        for (stage_index, stage) in pipeline.stages.iter().enumerate() {
            if stage.name.is_empty() {
                return Err(AppError::ValidationError(format!(
                    "Stage {} name cannot be empty",
                    stage_index
                )));
            }

            if stage.steps.is_empty() {
                return Err(AppError::ValidationError(format!(
                    "Stage '{}' must have at least one step",
                    stage.name
                )));
            }

            // Validate each step
            for (step_index, step) in stage.steps.iter().enumerate() {
                if step.name.is_empty() {
                    return Err(AppError::ValidationError(format!(
                        "Step {} in stage '{}' name cannot be empty",
                        step_index, stage.name
                    )));
                }

                // Validate step configuration based on type
                self.validate_step_config(step, &stage.name)?;
            }
        }

        // Check for duplicate stage names
        let mut stage_names = std::collections::HashSet::new();
        for stage in &pipeline.stages {
            if !stage_names.insert(&stage.name) {
                return Err(AppError::ValidationError(format!(
                    "Duplicate stage name: '{}'",
                    stage.name
                )));
            }
        }

        debug!("Pipeline validation successful: {}", pipeline.name);
        Ok(())
    }

    /// Validate step configuration
    fn validate_step_config(&self, step: &crate::ci::config::Step, stage_name: &str) -> Result<()> {
        use crate::ci::config::StepType;

        match step.step_type {
            StepType::Shell => {
                if step.config.command.is_none() && step.config.script.is_none() {
                    return Err(AppError::ValidationError(format!(
                        "Shell step '{}' in stage '{}' must have either command or script",
                        step.name, stage_name
                    )));
                }
            }
            StepType::Docker => {
                if step.config.image.is_none() {
                    return Err(AppError::ValidationError(format!(
                        "Docker step '{}' in stage '{}' must specify an image",
                        step.name, stage_name
                    )));
                }
            }
            StepType::Kubernetes => {
                if step.config.manifest.is_none() && step.config.command.is_none() {
                    return Err(AppError::ValidationError(format!(
                        "Kubernetes step '{}' in stage '{}' must have manifest or command",
                        step.name, stage_name
                    )));
                }
            }
            _ => {
                // Other step types have their own validation logic
                // This can be extended as needed
            }
        }

        Ok(())
    }

    /// Database operations
    async fn store_pipeline(&self, pipeline: &CIPipeline) -> Result<()> {
        let collection = self.db.database.collection::<CIPipeline>("ci_pipelines");

        collection
            .insert_one(pipeline, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to store pipeline: {}", e)))?;

        Ok(())
    }

    async fn get_pipeline_from_db(&self, pipeline_id: Uuid) -> Result<CIPipeline> {
        let collection = self.db.database.collection::<CIPipeline>("ci_pipelines");

        let pipeline = collection
            .find_one(
                mongodb::bson::doc! {"id": self.uuid_to_mongo_binary(&pipeline_id)},
                None,
            )
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to retrieve pipeline: {}", e)))?;

        pipeline.ok_or_else(|| AppError::NotFound("Pipeline not found".to_string()))
    }

    async fn update_pipeline_in_db(&self, pipeline: &CIPipeline) -> Result<()> {
        let collection = self.db.database.collection::<CIPipeline>("ci_pipelines");
        let pipeline_id = pipeline.id.unwrap();

        collection
            .replace_one(
                mongodb::bson::doc! {"id": self.uuid_to_mongo_binary(&pipeline_id)},
                pipeline,
                None,
            )
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to update pipeline: {}", e)))?;

        Ok(())
    }

    async fn delete_pipeline_from_db(&self, pipeline_id: Uuid) -> Result<()> {
        let collection = self.db.database.collection::<CIPipeline>("ci_pipelines");

        collection
            .delete_one(
                mongodb::bson::doc! {"id": self.uuid_to_mongo_binary(&pipeline_id)},
                None,
            )
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to delete pipeline: {}", e)))?;

        Ok(())
    }

    async fn get_all_pipelines(&self) -> Result<Vec<CIPipeline>> {
        let collection = self.db.database.collection::<CIPipeline>("ci_pipelines");

        let mut cursor = collection
            .find(mongodb::bson::doc! {}, None)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to retrieve pipelines: {}", e)))?;

        let mut pipelines = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Failed to iterate pipelines: {}", e)))?
        {
            let pipeline = cursor.deserialize_current().map_err(|e| {
                AppError::DatabaseError(format!("Failed to deserialize pipeline: {}", e))
            })?;
            pipelines.push(pipeline);
        }

        Ok(pipelines)
    }

    /// Helper function to convert UUID to MongoDB binary
    fn uuid_to_mongo_binary(&self, uuid: &Uuid) -> mongodb::bson::Binary {
        mongodb::bson::Binary {
            subtype: mongodb::bson::spec::BinarySubtype::Generic,
            bytes: uuid.as_bytes().to_vec(),
        }
    }

    /// Event publishing methods
    async fn publish_pipeline_created_event(
        &self,
        pipeline_id: Uuid,
        pipeline: &CIPipeline,
    ) -> Result<()> {
        use crate::core::patterns::events::PipelineStartedEvent;

        let event = PipelineStartedEvent {
            pipeline_id,
            execution_id: Uuid::new_v4(), // This would be the actual execution ID in real usage
            triggered_by: "system".to_string(),
            correlation_id: Uuid::new_v4(),
            occurred_at: chrono::Utc::now(),
            metadata: {
                let mut metadata = std::collections::HashMap::new();
                metadata.insert("event_type".to_string(), "pipeline_created".to_string());
                metadata.insert("pipeline_name".to_string(), pipeline.name.clone());
                metadata
            },
        };

        self.event_bus.publish(event).await
    }

    async fn publish_pipeline_updated_event(
        &self,
        pipeline_id: Uuid,
        pipeline: &CIPipeline,
    ) -> Result<()> {
        use crate::core::patterns::events::PipelineStartedEvent;

        let event = PipelineStartedEvent {
            pipeline_id,
            execution_id: Uuid::new_v4(),
            triggered_by: "system".to_string(),
            correlation_id: Uuid::new_v4(),
            occurred_at: chrono::Utc::now(),
            metadata: {
                let mut metadata = std::collections::HashMap::new();
                metadata.insert("event_type".to_string(), "pipeline_updated".to_string());
                metadata.insert("pipeline_name".to_string(), pipeline.name.clone());
                metadata
            },
        };

        self.event_bus.publish(event).await
    }

    async fn publish_pipeline_deleted_event(&self, pipeline_id: Uuid) -> Result<()> {
        use crate::core::patterns::events::PipelineStartedEvent;

        let event = PipelineStartedEvent {
            pipeline_id,
            execution_id: Uuid::new_v4(),
            triggered_by: "system".to_string(),
            correlation_id: Uuid::new_v4(),
            occurred_at: chrono::Utc::now(),
            metadata: {
                let mut metadata = std::collections::HashMap::new();
                metadata.insert("event_type".to_string(), "pipeline_deleted".to_string());
                metadata
            },
        };

        self.event_bus.publish(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci::config::{Stage, Step, StepConfig, StepType};
    use crate::core::patterns::correlation::CorrelationTracker;
    use std::collections::HashMap;

    fn create_test_pipeline() -> CIPipeline {
        CIPipeline {
            mongo_id: None,
            id: None,
            name: "test-pipeline".to_string(),
            description: Some("Test pipeline".to_string()),
            triggers: vec![], // ✅ required
            stages: vec![Stage {
                name: "build".to_string(),
                condition: None,         // ✅ required
                steps: vec![Step {
                    name: "compile".to_string(),
                    step_type: StepType::Shell,
                    config: StepConfig {
                        command: Some("echo 'Building...'".to_string()),
                        ..Default::default()
                    },
                    condition: None,     // ✅ required in Step
                    continue_on_error: Some(false),
                    timeout: None,       // ✅ required in Step
                }],
                parallel: Some(false),
                timeout: Some(600),      // ✅ required
                retry_count: Some(0),    // ✅ required
                environment: None,
            }],
            environment: HashMap::new(),
            timeout: Some(3600),           // ✅ required
            retry_count: Some(0),          // ✅ required
            notifications: None,           // ✅ required
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }
    

    #[tokio::test]
    async fn test_pipeline_validation() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        let db = DatabaseManager::new(
            "mongodb://localhost:27017",
            "test",
        )
        .await
        .unwrap();    
        let manager = PipelineManager::new(db, event_bus);

        let pipeline = create_test_pipeline();
        let result = manager.validate_pipeline(&pipeline).await;
        assert!(result.is_ok());

        // Test invalid pipeline
        let mut invalid_pipeline = create_test_pipeline();
        invalid_pipeline.name = "".to_string();
        let result = manager.validate_pipeline(&invalid_pipeline).await;
        assert!(result.is_err());
    }

    #[tokio::test] // ✅ since we’re using async
async fn test_step_validation() {
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker, None));

    // ✅ Borrow &str instead of String
    let db = DatabaseManager::new(
        "mongodb://localhost:27017",
        "test",
    )
    .await
    .unwrap();

    let manager = PipelineManager::new(db, event_bus);

    // ✅ Test valid shell step
    let step = Step {
        name: "test-step".to_string(),
        step_type: StepType::Shell,
        config: StepConfig {
            command: Some("echo 'test'".to_string()),
            ..Default::default()
        },
        condition: None,        // ✅ required now
        continue_on_error: Some(false),
        timeout: Some(30),      // ✅ required now
    };

    let result = manager.validate_step_config(&step, "test-stage");
    assert!(result.is_ok());

    // ✅ Test invalid shell step (no command or script)
    let invalid_step = Step {
        name: "test-step".to_string(),
        step_type: StepType::Shell,
        config: StepConfig::default(),
        condition: None,        // ✅ required now
        continue_on_error: Some(false),
        timeout: Some(30),      // ✅ required now
    };

    let result = manager.validate_step_config(&invalid_step, "test-stage");
    assert!(result.is_err());
}
}
