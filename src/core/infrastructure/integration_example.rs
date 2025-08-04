//! Integration example demonstrating event-driven command enhancements
//!
//! This module shows how all the event sourcing, command queuing, batching,
//! authorization, and projection features work together in a complete system.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::core::{
    patterns::{
        commands::{
            Command, CommandDispatcher, CommandDispatcherConfig, CommandHandler, CommandPriority,
        },
        event_sourcing::{EventStore, EventStoreEntry, InMemoryEventStore, ProjectionManager},
        events::{DomainEvent, EventBus, EventBusConfig, EventHandler},
        projections::{PipelineHistoryProjection, PipelineStatsProjection},
        correlation::CorrelationTracker,
    },
    networking::security::{Permission, SecurityContext},
};
use crate::error::Result;

/// Example command for creating a pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePipelineCommand {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub stages: Vec<String>,
    pub user_id: Uuid,
}

#[async_trait::async_trait]
impl Command for CreatePipelineCommand {
    type Result = Uuid;

    async fn execute(&self) -> Result<Self::Result> {
        // Simulate pipeline creation
        let pipeline_id = Uuid::new_v4();

        // In a real implementation, this would:
        // 1. Validate the pipeline configuration
        // 2. Store the pipeline in the database
        // 3. Publish domain events

        tracing::info!(
            pipeline_id = %pipeline_id,
            name = self.name,
            user_id = %self.user_id,
            "Pipeline created successfully"
        );

        Ok(pipeline_id)
    }

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn command_name(&self) -> &'static str {
        "CreatePipeline"
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::CreatePipeline]
    }

    fn priority(&self) -> CommandPriority {
        CommandPriority::Normal
    }

    fn can_batch(&self) -> bool {
        true
    }

    fn batch_key(&self) -> Option<String> {
        Some("pipeline_creation".to_string())
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("pipeline_name".to_string(), self.name.clone());
        metadata.insert("user_id".to_string(), self.user_id.to_string());
        metadata.insert("stage_count".to_string(), self.stages.len().to_string());
        metadata
    }
}

/// Command handler for CreatePipelineCommand
pub struct CreatePipelineHandler {
    event_bus: Arc<EventBus>,
    event_store: Arc<dyn EventStore>,
}

impl CreatePipelineHandler {
    pub fn new(event_bus: Arc<EventBus>, event_store: Arc<dyn EventStore>) -> Self {
        Self {
            event_bus,
            event_store,
        }
    }
}

#[async_trait::async_trait]
impl CommandHandler<CreatePipelineCommand> for CreatePipelineHandler {
    async fn handle(&self, command: CreatePipelineCommand) -> Result<Uuid> {
        // Execute the command
        let pipeline_id = command.execute().await?;

        // Create and publish domain events
        let pipeline_created_event = PipelineCreatedEvent {
            pipeline_id,
            name: command.name.clone(),
            description: command.description.clone(),
            stages: command.stages.clone(),
            created_by: command.user_id,
            correlation_id: command.correlation_id,
            occurred_at: Utc::now(),
            metadata: HashMap::new(),
        };

        // Store event in event store
        let event_entry = EventStoreEntry {
            event_id: Uuid::new_v4(),
            aggregate_id: pipeline_id,
            aggregate_type: "Pipeline".to_string(),
            event_type: pipeline_created_event.event_type().to_string(),
            event_version: 1,
            sequence_number: 0, // Will be set by store
            event_data: serde_json::to_value(&pipeline_created_event)?,
            metadata: pipeline_created_event.metadata(),
            correlation_id: pipeline_created_event.correlation_id(),
            timestamp: pipeline_created_event.occurred_at(),
            schema_version: 1,
        };

        self.event_store
            .append_events(pipeline_id, None, vec![event_entry])
            .await?;

        // Publish event to event bus for real-time processing
        self.event_bus.publish(pipeline_created_event).await?;

        Ok(pipeline_id)
    }

    fn handler_name(&self) -> &'static str {
        "CreatePipelineHandler"
    }
}

/// Domain event for pipeline creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCreatedEvent {
    pub pipeline_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub stages: Vec<String>,
    pub created_by: Uuid,
    pub correlation_id: Uuid,
    pub occurred_at: chrono::DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl DomainEvent for PipelineCreatedEvent {
    fn event_type(&self) -> &'static str {
        "pipeline.created"
    }

    fn aggregate_id(&self) -> Uuid {
        self.pipeline_id
    }

    fn occurred_at(&self) -> chrono::DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn metadata(&self) -> HashMap<String, String> {
        self.metadata.clone()
    }
}

/// Event handler for pipeline created events
pub struct PipelineCreatedEventHandler {
    notification_service: Arc<NotificationService>,
}

impl PipelineCreatedEventHandler {
    pub fn new(notification_service: Arc<NotificationService>) -> Self {
        Self {
            notification_service,
        }
    }
}

#[async_trait::async_trait]
impl EventHandler<PipelineCreatedEvent> for PipelineCreatedEventHandler {
    async fn handle(&self, event: &PipelineCreatedEvent) -> Result<()> {
        // Send notification about pipeline creation
        self.notification_service
            .send_pipeline_created_notification(event.pipeline_id, &event.name, event.created_by)
            .await?;

        tracing::info!(
            pipeline_id = %event.pipeline_id,
            name = event.name,
            created_by = %event.created_by,
            "Sent pipeline creation notification"
        );

        Ok(())
    }

    fn handler_name(&self) -> &'static str {
        "PipelineCreatedEventHandler"
    }
}

/// Mock notification service
pub struct NotificationService;

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self
    }

    pub async fn send_pipeline_created_notification(
        &self,
        pipeline_id: Uuid,
        name: &str,
        user_id: Uuid,
    ) -> Result<()> {
        // In a real implementation, this would send emails, Slack messages, etc.
        tracing::info!(
            pipeline_id = %pipeline_id,
            name = name,
            user_id = %user_id,
            "📧 Pipeline creation notification sent"
        );
        Ok(())
    }
}

/// Complete integration example
pub struct EventDrivenSystem {
    pub command_dispatcher: Arc<CommandDispatcher>,
    pub event_bus: Arc<EventBus>,
    pub event_store: Arc<dyn EventStore>,
    pub projection_manager: Arc<ProjectionManager>,
    pub correlation_tracker: Arc<CorrelationTracker>,
}

impl EventDrivenSystem {
    /// Create a new event-driven system with all components
    pub async fn new() -> Result<Self> {
        let correlation_tracker = Arc::new(CorrelationTracker::new());

        // Create event store
        let event_store: Arc<dyn EventStore> = Arc::new(InMemoryEventStore::new());

        // Create event bus with retry configuration
        let event_bus_config = EventBusConfig {
            max_retry_attempts: 3,
            initial_retry_delay_ms: 100,
            max_retry_delay_ms: 5000,
            backoff_multiplier: 2.0,
            enable_dead_letter_queue: true,
            batch_size: 50,
            processing_interval_ms: 1000,
        };
        let event_bus = Arc::new(EventBus::new(
            Arc::clone(&correlation_tracker),
            Some(event_bus_config),
        ));

        // Create command dispatcher with queuing and batching
        let command_config = CommandDispatcherConfig {
            max_queue_size: 10000,
            batch_size: 100,
            batch_timeout_ms: 1000,
            worker_count: 4,
            enable_authorization: true,
            enable_batching: true,
            enable_retries: true,
        };
        let command_dispatcher = Arc::new(CommandDispatcher::new(
            Arc::clone(&correlation_tracker),
            Some(command_config),
        ));

        // Create projection manager
        let projection_manager = Arc::new(ProjectionManager::new(Arc::clone(&event_store)));

        // Register projections
        let stats_projection = Arc::new(PipelineStatsProjection::new());
        projection_manager
            .register_projection(stats_projection)
            .await?;

        let history_projection = Arc::new(PipelineHistoryProjection::new(Some(1000)));
        projection_manager
            .register_projection(history_projection)
            .await?;

        // Register command handlers
        let notification_service = Arc::new(NotificationService::new());
        let create_pipeline_handler = Box::new(CreatePipelineHandler::new(
            Arc::clone(&event_bus),
            Arc::clone(&event_store),
        ));
        command_dispatcher
            .register_handler(create_pipeline_handler)
            .await?;

        // Register event handlers
        let pipeline_created_handler =
            Arc::new(PipelineCreatedEventHandler::new(notification_service));
        event_bus.register_handler(pipeline_created_handler).await?;

        // Start background processing
        command_dispatcher.start().await?;
        projection_manager.start().await?;

        Ok(Self {
            command_dispatcher,
            event_bus,
            event_store,
            projection_manager,
            correlation_tracker,
        })
    }

    /// Demonstrate the complete workflow
    pub async fn demonstrate_workflow(&self) -> Result<()> {
        tracing::info!("🚀 Starting event-driven system demonstration");

        // Create a security context for authorization
        let security_context = SecurityContext {
            user_id: Uuid::new_v4(),
            permissions: vec![Permission::CreatePipeline].into_iter().collect(),
            roles: vec![],
            session_id: "demo-session".to_string(),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("Demo Client".to_string()),
        };

        // Create and queue multiple pipeline creation commands
        for i in 1..=5 {
            let command = CreatePipelineCommand {
                command_id: Uuid::new_v4(),
                correlation_id: Uuid::new_v4(),
                name: format!("Demo Pipeline {}", i),
                description: Some(format!("This is demo pipeline number {}", i)),
                stages: vec![
                    "build".to_string(),
                    "test".to_string(),
                    "deploy".to_string(),
                ],
                user_id: security_context.user_id,
            };

            // Queue command for asynchronous processing
            self.command_dispatcher
                .queue_command(command, Some(security_context.clone()))
                .await?;
        }

        // Wait a bit for processing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Force process any remaining batches
        let batch_results = self.command_dispatcher.force_process_batches().await?;
        tracing::info!("📊 Processed {} batches", batch_results.len());

        // Get command execution statistics
        let command_stats = self.command_dispatcher.get_execution_stats().await;
        tracing::info!("📈 Command execution stats: {:?}", command_stats);

        // Get event bus statistics
        let event_stats = self.event_bus.get_stats().await;
        tracing::info!("📡 Event bus stats: {:?}", event_stats);

        // Get projection states
        let projection_states = self.projection_manager.get_projection_states().await;
        tracing::info!("🔄 Projection states: {:?}", projection_states);

        // Get event store statistics
        let store_stats = self.event_store.get_stats().await?;
        tracing::info!("💾 Event store stats: {:?}", store_stats);

        tracing::info!("✅ Event-driven system demonstration completed");
        Ok(())
    }

    /// Shutdown the system gracefully
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("🛑 Shutting down event-driven system");

        self.command_dispatcher.stop().await?;
        self.event_bus.stop().await?;
        self.projection_manager.stop().await?;

        tracing::info!("✅ Event-driven system shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_driven_system_integration() {
        // Initialize tracing for test output
        tracing_subscriber::fmt::init();

        // Create the system
        let system = EventDrivenSystem::new()
            .await
            .expect("Failed to create system");

        // Run the demonstration
        system
            .demonstrate_workflow()
            .await
            .expect("Demonstration failed");

        // Shutdown
        system.shutdown().await.expect("Shutdown failed");
    }

    #[tokio::test]
    async fn test_command_authorization() {
        let system = EventDrivenSystem::new()
            .await
            .expect("Failed to create system");

        // Create command without required permissions
        let command = CreatePipelineCommand {
            command_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            name: "Unauthorized Pipeline".to_string(),
            description: None,
            stages: vec!["build".to_string()],
            user_id: Uuid::new_v4(),
        };

        let security_context = SecurityContext {
            user_id: Uuid::new_v4(),
            permissions: std::collections::HashSet::new(), // No permissions
            roles: vec![],
            session_id: "test-session".to_string(),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("Test Client".to_string()),
        };

        // This should fail due to missing permissions
        let result = system
            .command_dispatcher
            .queue_command(command, Some(security_context))
            .await;
        assert!(result.is_err());

        system.shutdown().await.expect("Shutdown failed");
    }
}
