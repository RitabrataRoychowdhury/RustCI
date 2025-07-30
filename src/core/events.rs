//! Event system implementation using Observer pattern
//!
//! This module provides the foundation for event-driven architecture
//! with proper correlation tracking and error handling.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Type alias for complex event handlers map
type EventHandlerMap = Arc<RwLock<HashMap<String, Vec<Arc<dyn EventHandlerWrapper>>>>>;

/// Core trait that all domain events must implement
pub trait DomainEvent: Send + Sync + std::fmt::Debug + Serialize {
    /// Get the event type identifier
    fn event_type(&self) -> &'static str;

    /// Get the aggregate ID this event relates to
    fn aggregate_id(&self) -> Uuid;

    /// Get when this event occurred
    fn occurred_at(&self) -> DateTime<Utc>;

    /// Get the correlation ID for tracing
    fn correlation_id(&self) -> Uuid;

    /// Get event version for schema evolution
    fn version(&self) -> u32 {
        1
    }

    /// Get event metadata
    fn metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

/// Handler for domain events
#[async_trait]
pub trait EventHandler<T: DomainEvent>: Send + Sync {
    /// Handle the event
    async fn handle(&self, event: &T) -> Result<()>;

    /// Get handler name for logging and debugging
    fn handler_name(&self) -> &'static str;

    /// Check if this handler can process the event
    fn can_handle(&self, event: &T) -> bool {
        let _ = event;
        true
    }
}

/// Enhanced event bus with retry mechanisms and handler registration
pub struct EventBus {
    correlation_tracker: Arc<crate::core::CorrelationTracker>,
    handlers: EventHandlerMap,
    retry_queue: Arc<Mutex<VecDeque<RetryableEvent>>>,
    event_sender: mpsc::UnboundedSender<EventWrapper>,
    processor_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    config: EventBusConfig,
}

/// Configuration for event bus
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    pub max_retry_attempts: u32,
    pub initial_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub enable_dead_letter_queue: bool,
    pub batch_size: usize,
    pub processing_interval_ms: u64,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 3,
            initial_retry_delay_ms: 100,
            max_retry_delay_ms: 30000, // 30 seconds
            backoff_multiplier: 2.0,
            enable_dead_letter_queue: true,
            batch_size: 100,
            processing_interval_ms: 1000,
        }
    }
}

/// Wrapper for events with metadata
#[derive(Debug, Clone)]
struct EventWrapper {
    event_type: String,
    event_data: serde_json::Value,
    correlation_id: Uuid,
    aggregate_id: Uuid,
    occurred_at: DateTime<Utc>,
    version: u32,
    metadata: HashMap<String, String>,
    retry_count: u32,
}

/// Retryable event for failed processing
#[derive(Debug, Clone)]
struct RetryableEvent {
    event: EventWrapper,
    next_retry_at: DateTime<Utc>,
    last_error: String,
}

/// Wrapper trait for event handlers to support dynamic dispatch
#[async_trait]
trait EventHandlerWrapper: Send + Sync {
    async fn handle_event(&self, event: &EventWrapper) -> Result<()>;
    fn handler_name(&self) -> &'static str;
    fn can_handle(&self, event_type: &str) -> bool;
}

/// Concrete wrapper implementation
struct ConcreteEventHandlerWrapper<T: DomainEvent + for<'de> Deserialize<'de>> {
    handler: Arc<dyn EventHandler<T>>,
    _phantom: std::marker::PhantomData<T>,
}

#[async_trait]
impl<T: DomainEvent + for<'de> Deserialize<'de>> EventHandlerWrapper
    for ConcreteEventHandlerWrapper<T>
{
    async fn handle_event(&self, event: &EventWrapper) -> Result<()> {
        // Deserialize event data
        let domain_event: T = serde_json::from_value(event.event_data.clone()).map_err(|e| {
            AppError::ValidationError(format!("Failed to deserialize event: {}", e))
        })?;

        self.handler.handle(&domain_event).await
    }

    fn handler_name(&self) -> &'static str {
        self.handler.handler_name()
    }

    fn can_handle(&self, event_type: &str) -> bool {
        // In a real implementation, you'd check if this handler can process the event type
        std::any::type_name::<T>().contains(event_type)
    }
}

impl EventBus {
    /// Create a new event bus
    pub fn new(
        correlation_tracker: Arc<crate::core::CorrelationTracker>,
        config: Option<EventBusConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let bus = Self {
            correlation_tracker,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            retry_queue: Arc::new(Mutex::new(VecDeque::new())),
            event_sender,
            processor_handle: Arc::new(Mutex::new(None)),
            config,
        };

        // Start background processing
        bus.start_background_processing(event_receiver);

        bus
    }

    /// Start background event processing
    fn start_background_processing(
        &self,
        mut event_receiver: mpsc::UnboundedReceiver<EventWrapper>,
    ) {
        let handlers = Arc::clone(&self.handlers);
        let retry_queue = Arc::clone(&self.retry_queue);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_millis(config.processing_interval_ms));

            loop {
                tokio::select! {
                    // Process incoming events
                    Some(event) = event_receiver.recv() => {
                        Self::process_event(&handlers, &retry_queue, event, &config).await;
                    }

                    // Process retry queue
                    _ = interval.tick() => {
                        Self::process_retry_queue(&handlers, &retry_queue, &config).await;
                    }
                }
            }
        });

        // Store handle for cleanup
        let processor_handle = Arc::clone(&self.processor_handle);
        tokio::spawn(async move {
            *processor_handle.lock().await = Some(handle);
        });
    }

    /// Process a single event
    async fn process_event(
        handlers: &EventHandlerMap,
        retry_queue: &Arc<Mutex<VecDeque<RetryableEvent>>>,
        event: EventWrapper,
        config: &EventBusConfig,
    ) {
        let handlers_guard = handlers.read().await;

        if let Some(event_handlers) = handlers_guard.get(&event.event_type) {
            for handler in event_handlers {
                if handler.can_handle(&event.event_type) {
                    match handler.handle_event(&event).await {
                        Ok(()) => {
                            debug!(
                                event_type = event.event_type,
                                handler = handler.handler_name(),
                                correlation_id = %event.correlation_id,
                                "Event processed successfully"
                            );
                        }
                        Err(e) => {
                            warn!(
                                event_type = event.event_type,
                                handler = handler.handler_name(),
                                correlation_id = %event.correlation_id,
                                error = %e,
                                "Event processing failed"
                            );

                            // Add to retry queue if retries are enabled
                            if event.retry_count < config.max_retry_attempts {
                                let retry_delay = std::cmp::min(
                                    config.initial_retry_delay_ms
                                        * (config.backoff_multiplier.powi(event.retry_count as i32)
                                            as u64),
                                    config.max_retry_delay_ms,
                                );

                                let mut retry_event = event.clone();
                                retry_event.retry_count += 1;

                                let retryable = RetryableEvent {
                                    event: retry_event,
                                    next_retry_at: Utc::now()
                                        + chrono::Duration::milliseconds(retry_delay as i64),
                                    last_error: e.to_string(),
                                };

                                let mut queue = retry_queue.lock().await;
                                queue.push_back(retryable);
                            } else if config.enable_dead_letter_queue {
                                error!(
                                    event_type = event.event_type,
                                    correlation_id = %event.correlation_id,
                                    retry_count = event.retry_count,
                                    error = %e,
                                    "Event moved to dead letter queue after max retries"
                                );
                                // In a real implementation, you'd send to dead letter queue
                            }
                        }
                    }
                }
            }
        } else {
            debug!(
                event_type = event.event_type,
                "No handlers registered for event type"
            );
        }
    }

    /// Process retry queue
    async fn process_retry_queue(
        handlers: &EventHandlerMap,
        retry_queue: &Arc<Mutex<VecDeque<RetryableEvent>>>,
        config: &EventBusConfig,
    ) {
        let now = Utc::now();
        let mut events_to_retry = Vec::new();

        // Get events ready for retry
        {
            let mut queue = retry_queue.lock().await;
            while let Some(retryable) = queue.front() {
                if retryable.next_retry_at <= now {
                    events_to_retry.push(queue.pop_front().unwrap());
                } else {
                    break;
                }
            }
        }

        // Process retry events
        for retryable in events_to_retry {
            Self::process_event(handlers, retry_queue, retryable.event, config).await;
        }
    }

    /// Register an event handler
    pub async fn register_handler<T: DomainEvent + for<'de> Deserialize<'de> + 'static>(
        &self,
        handler: Arc<dyn EventHandler<T>>,
    ) -> Result<()> {
        let event_type = std::any::type_name::<T>().to_string();
        let wrapper = Arc::new(ConcreteEventHandlerWrapper {
            handler,
            _phantom: std::marker::PhantomData,
        });

        let mut handlers = self.handlers.write().await;
        handlers
            .entry(event_type.clone())
            .or_insert_with(Vec::new)
            .push(wrapper);

        info!("Registered event handler for: {}", event_type);
        Ok(())
    }

    /// Publish an event
    pub async fn publish<T: DomainEvent + Serialize>(&self, event: T) -> Result<()> {
        let event_type = event.event_type();

        // Set correlation context
        self.correlation_tracker
            .set_correlation_id(event.correlation_id())
            .await;

        info!(
            correlation_id = %event.correlation_id(),
            event_type = event_type,
            aggregate_id = %event.aggregate_id(),
            "Publishing event"
        );

        // Serialize event
        let event_data = serde_json::to_value(&event)
            .map_err(|e| AppError::ValidationError(format!("Failed to serialize event: {}", e)))?;

        let event_wrapper = EventWrapper {
            event_type: event_type.to_string(),
            event_data,
            correlation_id: event.correlation_id(),
            aggregate_id: event.aggregate_id(),
            occurred_at: event.occurred_at(),
            version: event.version(),
            metadata: event.metadata(),
            retry_count: 0,
        };

        // Send to processing queue
        self.event_sender.send(event_wrapper).map_err(|_| {
            AppError::InternalServerError("Failed to queue event for processing".to_string())
        })?;

        Ok(())
    }

    /// Stop event processing
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping event bus");

        if let Some(handle) = self.processor_handle.lock().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Get event bus statistics
    pub async fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        let handlers = self.handlers.read().await;
        stats.insert("registered_handlers".to_string(), handlers.len());

        let retry_queue = self.retry_queue.lock().await;
        stats.insert("retry_queue_size".to_string(), retry_queue.len());

        stats
    }

    /// Get retry queue status
    pub async fn get_retry_queue_status(&self) -> Vec<RetryQueueStatus> {
        let retry_queue = self.retry_queue.lock().await;

        retry_queue
            .iter()
            .map(|retryable| RetryQueueStatus {
                event_type: retryable.event.event_type.clone(),
                correlation_id: retryable.event.correlation_id,
                retry_count: retryable.event.retry_count,
                next_retry_at: retryable.next_retry_at,
                last_error: retryable.last_error.clone(),
            })
            .collect()
    }

    /// Clear retry queue (useful for testing)
    pub async fn clear_retry_queue(&self) {
        let mut retry_queue = self.retry_queue.lock().await;
        retry_queue.clear();
        info!("Cleared event retry queue");
    }
}

/// Status information for retry queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryQueueStatus {
    pub event_type: String,
    pub correlation_id: Uuid,
    pub retry_count: u32,
    pub next_retry_at: DateTime<Utc>,
    pub last_error: String,
}

/// Common domain events for the CI system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStartedEvent {
    pub pipeline_id: Uuid,
    pub execution_id: Uuid,
    pub triggered_by: String,
    pub correlation_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl DomainEvent for PipelineStartedEvent {
    fn event_type(&self) -> &'static str {
        "pipeline.started"
    }

    fn aggregate_id(&self) -> Uuid {
        self.pipeline_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn metadata(&self) -> HashMap<String, String> {
        self.metadata.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCompletedEvent {
    pub pipeline_id: Uuid,
    pub execution_id: Uuid,
    pub status: String,
    pub duration_ms: u64,
    pub correlation_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl DomainEvent for PipelineCompletedEvent {
    fn event_type(&self) -> &'static str {
        "pipeline.completed"
    }

    fn aggregate_id(&self) -> Uuid {
        self.pipeline_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    fn metadata(&self) -> HashMap<String, String> {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestEventHandler {
        name: &'static str,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventHandler<PipelineStartedEvent> for TestEventHandler {
        async fn handle(&self, _event: &PipelineStartedEvent) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn handler_name(&self) -> &'static str {
            self.name
        }
    }

    #[tokio::test]
    async fn test_event_bus_publish() {
        let correlation_tracker = Arc::new(crate::core::CorrelationTracker::new());
        let event_bus = EventBus::new(correlation_tracker, None);

        let event = PipelineStartedEvent {
            pipeline_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            triggered_by: "test".to_string(),
            correlation_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            metadata: HashMap::new(),
        };

        let result = event_bus.publish(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_event_bus_stats() {
        let correlation_tracker = Arc::new(crate::core::CorrelationTracker::new());
        let event_bus = EventBus::new(correlation_tracker, None);

        let stats = event_bus.get_stats().await;
        assert!(stats.len() > 0);
    }
}
