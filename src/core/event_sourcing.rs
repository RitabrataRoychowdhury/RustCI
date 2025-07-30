//! Event Sourcing implementation with projections, retries, and schema evolution
//!
//! This module provides a comprehensive event sourcing system that supports:
//! - Event store with persistence
//! - Event projections for read models
//! - Schema evolution and versioning
//! - Retry mechanisms for event processing
//! - Snapshot support for performance

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Event store entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStoreEntry {
    pub event_id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub event_type: String,
    pub event_version: u32,
    pub sequence_number: u64,
    pub event_data: serde_json::Value,
    pub metadata: HashMap<String, String>,
    pub correlation_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub schema_version: u32,
}

/// Event stream for a specific aggregate
#[derive(Debug, Clone)]
pub struct EventStream {
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub events: Vec<EventStoreEntry>,
    pub current_version: u64,
}

/// Snapshot for aggregate state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateSnapshot {
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub version: u64,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Event store interface
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append events to the store
    async fn append_events(
        &self,
        aggregate_id: Uuid,
        expected_version: Option<u64>,
        events: Vec<EventStoreEntry>,
    ) -> Result<u64>;

    /// Get events for an aggregate
    async fn get_events(
        &self,
        aggregate_id: Uuid,
        from_version: Option<u64>,
    ) -> Result<EventStream>;

    /// Get all events of a specific type
    async fn get_events_by_type(
        &self,
        event_type: &str,
        from_timestamp: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<EventStoreEntry>>;

    /// Save snapshot
    async fn save_snapshot(&self, snapshot: AggregateSnapshot) -> Result<()>;

    /// Get latest snapshot
    async fn get_snapshot(&self, aggregate_id: Uuid) -> Result<Option<AggregateSnapshot>>;

    /// Get event statistics
    async fn get_stats(&self) -> Result<EventStoreStats>;
}

/// Event store statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStoreStats {
    pub total_events: u64,
    pub total_aggregates: u64,
    pub events_by_type: HashMap<String, u64>,
    pub oldest_event: Option<DateTime<Utc>>,
    pub newest_event: Option<DateTime<Utc>>,
}

/// In-memory event store implementation
pub struct InMemoryEventStore {
    events: Arc<RwLock<HashMap<Uuid, EventStream>>>,
    snapshots: Arc<RwLock<HashMap<Uuid, AggregateSnapshot>>>,
    global_events: Arc<RwLock<Vec<EventStoreEntry>>>,
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            global_events: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append_events(
        &self,
        aggregate_id: Uuid,
        expected_version: Option<u64>,
        events: Vec<EventStoreEntry>,
    ) -> Result<u64> {
        let mut events_map = self.events.write().await;
        let mut global_events = self.global_events.write().await;

        let stream = events_map
            .entry(aggregate_id)
            .or_insert_with(|| EventStream {
                aggregate_id,
                aggregate_type: events
                    .first()
                    .map(|e| e.aggregate_type.clone())
                    .unwrap_or_default(),
                events: Vec::new(),
                current_version: 0,
            });

        // Check expected version for optimistic concurrency control
        if let Some(expected) = expected_version {
            if stream.current_version != expected {
                return Err(AppError::ConcurrencyError(format!(
                    "Expected version {} but current version is {}",
                    expected, stream.current_version
                )));
            }
        }

        // Append events with sequence numbers
        for mut event in events {
            stream.current_version += 1;
            event.sequence_number = stream.current_version;
            stream.events.push(event.clone());
            global_events.push(event);
        }

        info!(
            aggregate_id = %aggregate_id,
            new_version = stream.current_version,
            events_count = stream.events.len(),
            "Events appended to aggregate"
        );

        Ok(stream.current_version)
    }

    async fn get_events(
        &self,
        aggregate_id: Uuid,
        from_version: Option<u64>,
    ) -> Result<EventStream> {
        let events_map = self.events.read().await;

        if let Some(stream) = events_map.get(&aggregate_id) {
            let filtered_events = if let Some(from) = from_version {
                stream
                    .events
                    .iter()
                    .filter(|e| e.sequence_number > from)
                    .cloned()
                    .collect()
            } else {
                stream.events.clone()
            };

            Ok(EventStream {
                aggregate_id: stream.aggregate_id,
                aggregate_type: stream.aggregate_type.clone(),
                events: filtered_events,
                current_version: stream.current_version,
            })
        } else {
            Ok(EventStream {
                aggregate_id,
                aggregate_type: String::new(),
                events: Vec::new(),
                current_version: 0,
            })
        }
    }

    async fn get_events_by_type(
        &self,
        event_type: &str,
        from_timestamp: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<EventStoreEntry>> {
        let global_events = self.global_events.read().await;

        let mut filtered: Vec<EventStoreEntry> = global_events
            .iter()
            .filter(|e| {
                e.event_type == event_type && from_timestamp.is_none_or(|ts| e.timestamp >= ts)
            })
            .cloned()
            .collect();

        // Sort by timestamp
        filtered.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    async fn save_snapshot(&self, snapshot: AggregateSnapshot) -> Result<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot.aggregate_id, snapshot.clone());

        debug!(
            aggregate_id = %snapshot.aggregate_id,
            version = snapshot.version,
            "Snapshot saved"
        );

        Ok(())
    }

    async fn get_snapshot(&self, aggregate_id: Uuid) -> Result<Option<AggregateSnapshot>> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.get(&aggregate_id).cloned())
    }

    async fn get_stats(&self) -> Result<EventStoreStats> {
        let events_map = self.events.read().await;
        let global_events = self.global_events.read().await;

        let mut events_by_type = HashMap::new();
        let mut oldest_event = None;
        let mut newest_event = None;

        for event in global_events.iter() {
            *events_by_type.entry(event.event_type.clone()).or_insert(0) += 1;

            if oldest_event.is_none() || event.timestamp < oldest_event.unwrap() {
                oldest_event = Some(event.timestamp);
            }

            if newest_event.is_none() || event.timestamp > newest_event.unwrap() {
                newest_event = Some(event.timestamp);
            }
        }

        Ok(EventStoreStats {
            total_events: global_events.len() as u64,
            total_aggregates: events_map.len() as u64,
            events_by_type,
            oldest_event,
            newest_event,
        })
    }
}

/// Event projection interface
#[async_trait]
pub trait EventProjection: Send + Sync {
    /// Get projection name
    fn projection_name(&self) -> &'static str;

    /// Handle an event and update the projection
    async fn handle_event(&self, event: &EventStoreEntry) -> Result<()>;

    /// Get projection state
    async fn get_state(&self) -> Result<serde_json::Value>;

    /// Reset projection state
    async fn reset(&self) -> Result<()>;

    /// Check if this projection handles the given event type
    fn handles_event_type(&self, event_type: &str) -> bool;
}

/// Projection manager that coordinates multiple projections
pub struct ProjectionManager {
    projections: Arc<RwLock<Vec<Arc<dyn EventProjection>>>>,
    event_store: Arc<dyn EventStore>,
    projection_states: Arc<RwLock<HashMap<String, ProjectionState>>>,
    retry_queue: Arc<Mutex<VecDeque<RetryableProjectionTask>>>,
    processor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Projection state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionState {
    pub projection_name: String,
    pub last_processed_event: Option<Uuid>,
    pub last_processed_timestamp: Option<DateTime<Utc>>,
    pub processed_events_count: u64,
    pub error_count: u64,
    pub last_error: Option<String>,
    pub status: ProjectionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectionStatus {
    Running,
    Stopped,
    Error,
    Rebuilding,
}

/// Retryable projection task
#[derive(Debug, Clone)]
struct RetryableProjectionTask {
    projection_name: String,
    event: EventStoreEntry,
    retry_count: u32,
    max_retries: u32,
    next_retry_at: DateTime<Utc>,
}

impl ProjectionManager {
    pub fn new(event_store: Arc<dyn EventStore>) -> Self {
        Self {
            projections: Arc::new(RwLock::new(Vec::new())),
            event_store,
            projection_states: Arc::new(RwLock::new(HashMap::new())),
            retry_queue: Arc::new(Mutex::new(VecDeque::new())),
            processor_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Register a projection
    pub async fn register_projection(&self, projection: Arc<dyn EventProjection>) -> Result<()> {
        let projection_name = projection.projection_name();

        // Initialize projection state
        let state = ProjectionState {
            projection_name: projection_name.to_string(),
            last_processed_event: None,
            last_processed_timestamp: None,
            processed_events_count: 0,
            error_count: 0,
            last_error: None,
            status: ProjectionStatus::Stopped,
        };

        {
            let mut projections = self.projections.write().await;
            projections.push(projection);
        }

        {
            let mut states = self.projection_states.write().await;
            states.insert(projection_name.to_string(), state);
        }

        info!("Registered projection: {}", projection_name);
        Ok(())
    }

    /// Start projection processing
    pub async fn start(&self) -> Result<()> {
        info!("Starting projection manager");

        let projections = Arc::clone(&self.projections);
        let event_store = Arc::clone(&self.event_store);
        let projection_states = Arc::clone(&self.projection_states);
        let retry_queue = Arc::clone(&self.retry_queue);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                // Process retry queue
                Self::process_retry_queue(&projections, &projection_states, &retry_queue).await;

                // Process new events
                Self::process_new_events(
                    &projections,
                    &event_store,
                    &projection_states,
                    &retry_queue,
                )
                .await;
            }
        });

        *self.processor_handle.lock().await = Some(handle);
        Ok(())
    }

    /// Stop projection processing
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping projection manager");

        if let Some(handle) = self.processor_handle.lock().await.take() {
            handle.abort();
        }

        // Update all projection states to stopped
        let mut states = self.projection_states.write().await;
        for state in states.values_mut() {
            state.status = ProjectionStatus::Stopped;
        }

        Ok(())
    }

    /// Process retry queue
    async fn process_retry_queue(
        projections: &Arc<RwLock<Vec<Arc<dyn EventProjection>>>>,
        projection_states: &Arc<RwLock<HashMap<String, ProjectionState>>>,
        retry_queue: &Arc<Mutex<VecDeque<RetryableProjectionTask>>>,
    ) {
        let now = Utc::now();
        let mut tasks_to_retry = Vec::new();

        // Get tasks ready for retry
        {
            let mut queue = retry_queue.lock().await;
            while let Some(task) = queue.front() {
                if task.next_retry_at <= now {
                    tasks_to_retry.push(queue.pop_front().unwrap());
                } else {
                    break;
                }
            }
        }

        // Process retry tasks
        for task in tasks_to_retry {
            let projections_guard = projections.read().await;

            if let Some(projection) = projections_guard
                .iter()
                .find(|p| p.projection_name() == task.projection_name)
            {
                match projection.handle_event(&task.event).await {
                    Ok(()) => {
                        // Success - update state
                        let mut states = projection_states.write().await;
                        if let Some(state) = states.get_mut(&task.projection_name) {
                            state.processed_events_count += 1;
                            state.last_processed_event = Some(task.event.event_id);
                            state.last_processed_timestamp = Some(task.event.timestamp);
                            state.status = ProjectionStatus::Running;
                        }

                        debug!(
                            projection = task.projection_name,
                            event_id = %task.event.event_id,
                            retry_count = task.retry_count,
                            "Projection retry succeeded"
                        );
                    }
                    Err(e) => {
                        // Failed again
                        if task.retry_count < task.max_retries {
                            let mut new_task = task.clone();
                            new_task.retry_count += 1;
                            new_task.next_retry_at = now
                                + Duration::from_secs(
                                    (2_u64.pow(new_task.retry_count) * 5).min(300), // Max 5 minutes
                                );

                            let mut queue = retry_queue.lock().await;
                            queue.push_back(new_task);

                            warn!(
                                projection = task.projection_name,
                                event_id = %task.event.event_id,
                                retry_count = task.retry_count,
                                error = %e,
                                "Projection retry failed, will retry again"
                            );
                        } else {
                            // Max retries exceeded
                            let mut states = projection_states.write().await;
                            if let Some(state) = states.get_mut(&task.projection_name) {
                                state.error_count += 1;
                                state.last_error = Some(e.to_string());
                                state.status = ProjectionStatus::Error;
                            }

                            error!(
                                projection = task.projection_name,
                                event_id = %task.event.event_id,
                                retry_count = task.retry_count,
                                error = %e,
                                "Projection retry failed permanently"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Process new events
    async fn process_new_events(
        projections: &Arc<RwLock<Vec<Arc<dyn EventProjection>>>>,
        event_store: &Arc<dyn EventStore>,
        projection_states: &Arc<RwLock<HashMap<String, ProjectionState>>>,
        retry_queue: &Arc<Mutex<VecDeque<RetryableProjectionTask>>>,
    ) {
        let projections_guard = projections.read().await;

        for projection in projections_guard.iter() {
            let projection_name = projection.projection_name();

            // Get last processed timestamp
            let last_timestamp = {
                let states = projection_states.read().await;
                states
                    .get(projection_name)
                    .and_then(|s| s.last_processed_timestamp)
            };

            // Get new events
            match event_store
                .get_events_by_type("*", last_timestamp, Some(100))
                .await
            {
                Ok(events) => {
                    for event in events {
                        if projection.handles_event_type(&event.event_type) {
                            match projection.handle_event(&event).await {
                                Ok(()) => {
                                    // Update state
                                    let mut states = projection_states.write().await;
                                    if let Some(state) = states.get_mut(projection_name) {
                                        state.processed_events_count += 1;
                                        state.last_processed_event = Some(event.event_id);
                                        state.last_processed_timestamp = Some(event.timestamp);
                                        state.status = ProjectionStatus::Running;
                                    }
                                }
                                Err(e) => {
                                    // Add to retry queue
                                    let retry_task = RetryableProjectionTask {
                                        projection_name: projection_name.to_string(),
                                        event: event.clone(),
                                        retry_count: 0,
                                        max_retries: 3,
                                        next_retry_at: Utc::now() + Duration::from_secs(5),
                                    };

                                    let mut queue = retry_queue.lock().await;
                                    queue.push_back(retry_task);

                                    warn!(
                                        projection = projection_name,
                                        event_id = %event.event_id,
                                        error = %e,
                                        "Projection failed, added to retry queue"
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        projection = projection_name,
                        error = %e,
                        "Failed to get events for projection"
                    );
                }
            }
        }
    }

    /// Get projection states
    pub async fn get_projection_states(&self) -> HashMap<String, ProjectionState> {
        self.projection_states.read().await.clone()
    }

    /// Rebuild a projection
    pub async fn rebuild_projection(&self, projection_name: &str) -> Result<()> {
        let projections = self.projections.read().await;

        if let Some(projection) = projections
            .iter()
            .find(|p| p.projection_name() == projection_name)
        {
            // Update status to rebuilding
            {
                let mut states = self.projection_states.write().await;
                if let Some(state) = states.get_mut(projection_name) {
                    state.status = ProjectionStatus::Rebuilding;
                }
            }

            // Reset projection
            projection.reset().await?;

            // Process all events
            let all_events = self.event_store.get_events_by_type("*", None, None).await?;

            for event in all_events {
                if projection.handles_event_type(&event.event_type) {
                    projection.handle_event(&event).await?;
                }
            }

            // Update status to running
            {
                let mut states = self.projection_states.write().await;
                if let Some(state) = states.get_mut(projection_name) {
                    state.status = ProjectionStatus::Running;
                    state.processed_events_count = 0; // Reset counter
                    state.error_count = 0;
                    state.last_error = None;
                }
            }

            info!("Rebuilt projection: {}", projection_name);
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Projection not found: {}",
                projection_name
            )))
        }
    }
}

/// Schema evolution support
pub struct SchemaEvolutionManager {
    migrators: HashMap<String, Vec<Box<dyn EventMigrator>>>,
}

/// Event migrator for schema evolution
#[async_trait]
pub trait EventMigrator: Send + Sync {
    /// Get the source schema version this migrator handles
    fn source_version(&self) -> u32;

    /// Get the target schema version this migrator produces
    fn to_version(&self) -> u32;

    /// Migrate event data from old schema to new schema
    async fn migrate(&self, event_data: serde_json::Value) -> Result<serde_json::Value>;
}

impl Default for SchemaEvolutionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaEvolutionManager {
    pub fn new() -> Self {
        Self {
            migrators: HashMap::new(),
        }
    }

    /// Register an event migrator
    pub fn register_migrator(&mut self, event_type: String, migrator: Box<dyn EventMigrator>) {
        self.migrators
            .entry(event_type)
            .or_default()
            .push(migrator);
    }

    /// Migrate event to latest schema version
    pub async fn migrate_event(
        &self,
        mut event: EventStoreEntry,
        target_version: u32,
    ) -> Result<EventStoreEntry> {
        if event.schema_version >= target_version {
            return Ok(event); // Already at target version or newer
        }

        if let Some(migrators) = self.migrators.get(&event.event_type) {
            // Sort migrators by version
            let mut sorted_migrators: Vec<_> = migrators.iter().collect();
            sorted_migrators.sort_by_key(|m| m.source_version());

            // Apply migrations in sequence
            for migrator in sorted_migrators {
                if migrator.source_version() == event.schema_version
                    && migrator.to_version() <= target_version
                {
                    event.event_data = migrator.migrate(event.event_data).await?;
                    event.schema_version = migrator.to_version();

                    debug!(
                        event_id = %event.event_id,
                        from_version = migrator.source_version(),
                        to_version = migrator.to_version(),
                        "Migrated event schema"
                    );
                }
            }
        }

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_event_store() {
        let store = InMemoryEventStore::new();
        let aggregate_id = Uuid::new_v4();

        let event = EventStoreEntry {
            event_id: Uuid::new_v4(),
            aggregate_id,
            aggregate_type: "TestAggregate".to_string(),
            event_type: "TestEvent".to_string(),
            event_version: 1,
            sequence_number: 0, // Will be set by store
            event_data: serde_json::json!({"test": "data"}),
            metadata: HashMap::new(),
            correlation_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            schema_version: 1,
        };

        let version = store
            .append_events(aggregate_id, None, vec![event])
            .await
            .unwrap();
        assert_eq!(version, 1);

        let stream = store.get_events(aggregate_id, None).await.unwrap();
        assert_eq!(stream.events.len(), 1);
        assert_eq!(stream.current_version, 1);
    }

    #[tokio::test]
    async fn test_projection_manager() {
        let store = Arc::new(InMemoryEventStore::new());
        let manager = ProjectionManager::new(store);

        // Test basic functionality
        let states = manager.get_projection_states().await;
        assert!(states.is_empty());
    }

    #[tokio::test]
    async fn test_schema_evolution() {
        let mut manager = SchemaEvolutionManager::new();

        // Test with no migrators
        let event = EventStoreEntry {
            event_id: Uuid::new_v4(),
            aggregate_id: Uuid::new_v4(),
            aggregate_type: "Test".to_string(),
            event_type: "TestEvent".to_string(),
            event_version: 1,
            sequence_number: 1,
            event_data: serde_json::json!({"old": "data"}),
            metadata: HashMap::new(),
            correlation_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            schema_version: 1,
        };

        let migrated = manager.migrate_event(event.clone(), 2).await.unwrap();
        assert_eq!(migrated.schema_version, 1); // No migration occurred
    }
}
