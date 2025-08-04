//! Event loop implementation inspired by libuv architecture
//!
//! This module provides an event demultiplexer and event loop for handling
//! asynchronous operations in the RustCI platform.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::Result;

/// Unique identifier for events
pub type EventId = Uuid;

/// Type alias for complex event handlers map
type EventHandlersMap = Arc<RwLock<HashMap<EventType, Vec<Arc<dyn EventHandler>>>>>;

/// Event types supported by the event loop
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    /// Job submission event
    JobSubmission,
    /// Job completion event
    JobCompletion,
    /// Runner heartbeat event
    RunnerHeartbeat,
    /// Runner health check event
    RunnerHealthCheck,
    /// Node failure event
    NodeFailure,
    /// Timer event
    Timer,
    /// I/O event
    IO,
    /// Signal event
    Signal,
    /// Custom event
    Custom(String),
}

/// Event data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier
    pub id: EventId,
    /// Event type
    pub event_type: EventType,
    /// Event payload
    pub payload: EventPayload,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event priority
    pub priority: EventPriority,
    /// Event metadata
    pub metadata: HashMap<String, String>,
}

/// Event payload containing event-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    /// Job submission payload
    JobSubmission {
        job_id: crate::domain::entities::JobId,
        runner_id: Option<crate::domain::entities::RunnerId>,
    },
    /// Job completion payload
    JobCompletion {
        job_id: crate::domain::entities::JobId,
        result: JobCompletionResult,
    },
    /// Runner heartbeat payload
    RunnerHeartbeat {
        runner_id: crate::domain::entities::RunnerId,
        status: crate::domain::entities::RunnerStatus,
    },
    /// Runner health check payload
    RunnerHealthCheck {
        runner_id: crate::domain::entities::RunnerId,
        health_status: crate::domain::entities::HealthStatus,
    },
    /// Node failure payload
    NodeFailure {
        node_id: crate::domain::entities::NodeId,
        reason: String,
    },
    /// Timer payload
    Timer {
        timer_id: String,
        callback_data: Option<String>,
    },
    /// I/O payload
    IO { operation: String, data: Vec<u8> },
    /// Signal payload
    Signal {
        signal_type: String,
        data: Option<String>,
    },
    /// Custom payload
    Custom { data: serde_json::Value },
}

/// Job completion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCompletionResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub output: String,
    pub error: Option<String>,
    pub duration: Duration,
}

/// Event priority levels
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum EventPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Event handler trait
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    async fn handle(&self, event: Event) -> Result<()>;

    /// Get the event types this handler can process
    fn event_types(&self) -> Vec<EventType>;

    /// Get handler name for logging
    fn name(&self) -> &str;
}

/// Timer callback function type
pub type TimerCallback = Box<dyn Fn() -> Result<()> + Send + Sync>;

/// Event demultiplexer for managing events and handlers
pub struct EventDemultiplexer {
    /// Event handlers by event type
    handlers: EventHandlersMap,
    /// Event queue
    event_queue: Arc<RwLock<Vec<Event>>>,
    /// Timer wheel for scheduled events
    timer_wheel: Arc<RwLock<HashMap<String, TimerEntry>>>,
    /// Event statistics
    stats: Arc<RwLock<EventStats>>,
}

/// Timer entry for scheduled callbacks
struct TimerEntry {
    id: String,
    scheduled_at: Instant,
    interval: Option<Duration>,
    callback: TimerCallback,
    metadata: HashMap<String, String>,
}

/// Event processing statistics
#[derive(Debug, Clone, Default)]
pub struct EventStats {
    pub total_events: u64,
    pub events_by_type: HashMap<EventType, u64>,
    pub successful_events: u64,
    pub failed_events: u64,
    pub avg_processing_time: Duration,
    pub last_processed: Option<DateTime<Utc>>,
}

impl EventDemultiplexer {
    /// Create a new event demultiplexer
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            event_queue: Arc::new(RwLock::new(Vec::new())),
            timer_wheel: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(EventStats::default())),
        }
    }

    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) -> Result<()> {
        let mut handlers = self.handlers.write().await;

        for event_type in handler.event_types() {
            handlers
                .entry(event_type.clone())
                .or_insert_with(Vec::new)
                .push(handler.clone());
        }

        info!("Registered event handler: {}", handler.name());
        Ok(())
    }

    /// Unregister an event handler
    pub async fn unregister_handler(&self, handler_name: &str) -> Result<()> {
        let mut handlers = self.handlers.write().await;

        for (_, handler_list) in handlers.iter_mut() {
            handler_list.retain(|h| h.name() != handler_name);
        }

        info!("Unregistered event handler: {}", handler_name);
        Ok(())
    }

    /// Submit an event for processing
    pub async fn submit_event(&self, event: Event) -> Result<()> {
        debug!("Submitting event: {:?}", event.event_type);

        let mut queue = self.event_queue.write().await;
        queue.push(event);

        // Sort by priority (highest first)
        queue.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());

        Ok(())
    }

    /// Poll for events and return them for processing
    pub async fn poll_events(&self) -> Vec<Event> {
        let mut queue = self.event_queue.write().await;
        let events = queue.drain(..).collect();
        events
    }

    /// Process a single event
    pub async fn process_event(&self, event: Event) -> Result<()> {
        let start_time = Instant::now();
        let event_type = event.event_type.clone();

        debug!("Processing event: {:?} (ID: {})", event_type, event.id);

        let handlers = self.handlers.read().await;
        let event_handlers = handlers.get(&event_type).cloned().unwrap_or_default();

        if event_handlers.is_empty() {
            warn!("No handlers registered for event type: {:?}", event_type);
            return Ok(());
        }

        let mut success = true;
        for handler in event_handlers {
            if let Err(e) = handler.handle(event.clone()).await {
                error!(
                    "Handler {} failed to process event {}: {}",
                    handler.name(),
                    event.id,
                    e
                );
                success = false;
            }
        }

        // Update statistics
        let processing_time = start_time.elapsed();
        let mut stats = self.stats.write().await;
        stats.total_events += 1;
        *stats.events_by_type.entry(event_type).or_insert(0) += 1;

        if success {
            stats.successful_events += 1;
        } else {
            stats.failed_events += 1;
        }

        // Update average processing time
        let total_time = stats.avg_processing_time.as_nanos() as u64 * (stats.total_events - 1)
            + processing_time.as_nanos() as u64;
        stats.avg_processing_time = Duration::from_nanos(total_time / stats.total_events);
        stats.last_processed = Some(Utc::now());

        debug!("Event processed in {:?}", processing_time);
        Ok(())
    }

    /// Schedule a timer callback
    pub async fn schedule_timer(
        &self,
        timer_id: String,
        duration: Duration,
        callback: TimerCallback,
    ) -> Result<()> {
        let scheduled_at = Instant::now() + duration;
        let entry = TimerEntry {
            id: timer_id.clone(),
            scheduled_at,
            interval: None,
            callback,
            metadata: HashMap::new(),
        };

        let mut timers = self.timer_wheel.write().await;
        timers.insert(timer_id.clone(), entry);

        debug!("Scheduled timer: {} for {:?}", timer_id, duration);
        Ok(())
    }

    /// Schedule a recurring timer
    pub async fn schedule_recurring_timer(
        &self,
        timer_id: String,
        interval: Duration,
        callback: TimerCallback,
    ) -> Result<()> {
        let scheduled_at = Instant::now() + interval;
        let entry = TimerEntry {
            id: timer_id.clone(),
            scheduled_at,
            interval: Some(interval),
            callback,
            metadata: HashMap::new(),
        };

        let mut timers = self.timer_wheel.write().await;
        timers.insert(timer_id.clone(), entry);

        debug!(
            "Scheduled recurring timer: {} with interval {:?}",
            timer_id, interval
        );
        Ok(())
    }

    /// Cancel a timer
    pub async fn cancel_timer(&self, timer_id: &str) -> Result<()> {
        let mut timers = self.timer_wheel.write().await;
        if timers.remove(timer_id).is_some() {
            debug!("Cancelled timer: {}", timer_id);
        }
        Ok(())
    }

    /// Process expired timers
    pub async fn process_timers(&self) -> Result<()> {
        let now = Instant::now();
        let mut timers = self.timer_wheel.write().await;
        let mut expired_timers = Vec::new();

        // Find expired timers
        for (id, entry) in timers.iter() {
            if entry.scheduled_at <= now {
                expired_timers.push(id.clone());
            }
        }

        // Process expired timers
        for timer_id in expired_timers {
            if let Some(mut entry) = timers.remove(&timer_id) {
                debug!("Processing expired timer: {}", timer_id);

                // Execute callback
                if let Err(e) = (entry.callback)() {
                    error!("Timer callback failed for {}: {}", timer_id, e);
                }

                // Reschedule if recurring
                if let Some(interval) = entry.interval {
                    entry.scheduled_at = now + interval;
                    timers.insert(timer_id, entry);
                }
            }
        }

        Ok(())
    }

    /// Get event processing statistics
    pub async fn get_stats(&self) -> EventStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        *stats = EventStats::default();
        info!("Event statistics reset");
        Ok(())
    }
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, payload: EventPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            payload,
            timestamp: Utc::now(),
            priority: EventPriority::Normal,
            metadata: HashMap::new(),
        }
    }

    /// Create a new event with priority
    pub fn with_priority(
        event_type: EventType,
        payload: EventPayload,
        priority: EventPriority,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            payload,
            timestamp: Utc::now(),
            priority,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the event
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl Default for EventDemultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

/// Event loop that integrates event demultiplexer with job queue and runner pool
pub struct EventLoop {
    /// Event demultiplexer for handling events
    demux: Arc<EventDemultiplexer>,
    /// Job queue for managing jobs
    job_queue: Arc<dyn crate::core::jobs::job_queue::JobQueue>,
    /// Runner pool for executing jobs
    runner_pool: Arc<RwLock<Vec<Arc<dyn crate::domain::entities::runner::Runner>>>>,
    /// Event loop statistics
    stats: Arc<RwLock<EventLoopStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

/// Event loop statistics
#[derive(Debug, Clone, Default)]
pub struct EventLoopStats {
    pub events_processed: u64,
    pub jobs_scheduled: u64,
    pub jobs_completed: u64,
    pub runner_heartbeats: u64,
    pub node_failures: u64,
    pub uptime: Duration,
    pub last_activity: Option<DateTime<Utc>>,
}

impl EventLoop {
    /// Create a new event loop
    pub fn new(
        demux: Arc<EventDemultiplexer>,
        job_queue: Arc<dyn crate::core::jobs::job_queue::JobQueue>,
    ) -> Self {
        Self {
            demux,
            job_queue,
            runner_pool: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(EventLoopStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Add a runner to the pool
    pub async fn add_runner(
        &self,
        runner: Arc<dyn crate::domain::entities::runner::Runner>,
    ) -> Result<()> {
        let mut pool = self.runner_pool.write().await;
        pool.push(runner);
        Ok(())
    }

    /// Remove a runner from the pool
    pub async fn remove_runner(&self, runner_id: crate::domain::entities::RunnerId) -> Result<()> {
        let mut pool = self.runner_pool.write().await;
        pool.retain(|r| r.get_metadata().id != runner_id);
        Ok(())
    }

    /// Get available runners
    pub async fn get_available_runners(
        &self,
    ) -> Result<Vec<Arc<dyn crate::domain::entities::runner::Runner>>> {
        let pool = self.runner_pool.read().await;
        let mut available = Vec::new();

        for runner in pool.iter() {
            if let Ok(capacity) = runner.get_capacity().await {
                if capacity.available_slots > 0 {
                    available.push(runner.clone());
                }
            }
        }

        Ok(available)
    }

    /// Run the event loop
    pub async fn run(&self) -> Result<()> {
        info!("Starting event loop");
        let start_time = Instant::now();

        // Register built-in event handlers
        self.register_builtin_handlers().await?;

        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            // Check for shutdown signal
            {
                let shutdown = self.shutdown.read().await;
                if *shutdown {
                    info!("Event loop shutting down");
                    break;
                }
            }

            // Process events
            let events = self.demux.poll_events().await;
            for event in events {
                self.process_event(event).await?;
            }

            // Process timers
            self.demux.process_timers().await?;

            // Schedule jobs to available runners
            self.schedule_jobs().await?;

            // Update statistics
            self.update_stats(start_time.elapsed()).await;
        }

        Ok(())
    }

    /// Process a single event
    async fn process_event(&self, event: Event) -> Result<()> {
        debug!("Processing event: {:?}", event.event_type);

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.events_processed += 1;
            stats.last_activity = Some(Utc::now());
        }

        // Process the event through the demultiplexer
        self.demux.process_event(event).await?;

        Ok(())
    }

    /// Schedule jobs to available runners
    async fn schedule_jobs(&self) -> Result<()> {
        let available_runners = self.get_available_runners().await?;

        if available_runners.is_empty() {
            return Ok(());
        }

        // Try to dequeue and schedule jobs
        for runner in available_runners {
            if let Ok(Some(job)) = self.job_queue.dequeue().await {
                if let Ok(true) = runner.can_handle_job(&job).await {
                    debug!(
                        "Scheduling job {} to runner {}",
                        job.id,
                        runner.get_metadata().id
                    );

                    // Execute job on runner
                    let runner_clone = runner.clone();
                    let job_clone = job.clone();
                    tokio::spawn(async move {
                        if let Err(e) = runner_clone.execute(job_clone).await {
                            error!("Job execution failed: {}", e);
                        }
                    });

                    // Update statistics
                    {
                        let mut stats = self.stats.write().await;
                        stats.jobs_scheduled += 1;
                    }
                } else {
                    // Put job back in queue if runner can't handle it
                    self.job_queue.enqueue(job).await?;
                }
            }
        }

        Ok(())
    }

    /// Register built-in event handlers
    async fn register_builtin_handlers(&self) -> Result<()> {
        // Register job completion handler
        let completion_handler = Arc::new(BuiltinJobCompletionHandler {
            stats: self.stats.clone(),
        });
        self.demux.register_handler(completion_handler).await?;

        // Register runner heartbeat handler
        let heartbeat_handler = Arc::new(BuiltinRunnerHeartbeatHandler {
            stats: self.stats.clone(),
        });
        self.demux.register_handler(heartbeat_handler).await?;

        // Register node failure handler
        let failure_handler = Arc::new(BuiltinNodeFailureHandler {
            stats: self.stats.clone(),
            runner_pool: self.runner_pool.clone(),
        });
        self.demux.register_handler(failure_handler).await?;

        Ok(())
    }

    /// Update event loop statistics
    async fn update_stats(&self, uptime: Duration) {
        let mut stats = self.stats.write().await;
        stats.uptime = uptime;
    }

    /// Get event loop statistics
    pub async fn get_stats(&self) -> EventLoopStats {
        self.stats.read().await.clone()
    }

    /// Shutdown the event loop
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down event loop");

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = true;
        }

        // Shutdown all runners
        let pool = self.runner_pool.read().await;
        for runner in pool.iter() {
            if let Err(e) = runner.shutdown().await {
                warn!(
                    "Failed to shutdown runner {}: {}",
                    runner.get_metadata().id,
                    e
                );
            }
        }

        Ok(())
    }
}

/// Built-in job completion event handler
struct BuiltinJobCompletionHandler {
    stats: Arc<RwLock<EventLoopStats>>,
}

#[async_trait]
impl EventHandler for BuiltinJobCompletionHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::JobCompletion { job_id, result } = event.payload {
            debug!("Handling job completion for: {}", job_id);

            let mut stats = self.stats.write().await;
            stats.jobs_completed += 1;

            if result.success {
                info!("Job {} completed successfully", job_id);
            } else {
                warn!("Job {} failed: {:?}", job_id, result.error);
            }
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::JobCompletion]
    }

    fn name(&self) -> &str {
        "builtin-job-completion-handler"
    }
}

/// Built-in runner heartbeat event handler
struct BuiltinRunnerHeartbeatHandler {
    stats: Arc<RwLock<EventLoopStats>>,
}

#[async_trait]
impl EventHandler for BuiltinRunnerHeartbeatHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::RunnerHeartbeat { runner_id, status } = event.payload {
            debug!(
                "Handling runner heartbeat for: {} (status: {:?})",
                runner_id, status
            );

            let mut stats = self.stats.write().await;
            stats.runner_heartbeats += 1;
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::RunnerHeartbeat]
    }

    fn name(&self) -> &str {
        "builtin-runner-heartbeat-handler"
    }
}

/// Built-in node failure event handler
struct BuiltinNodeFailureHandler {
    stats: Arc<RwLock<EventLoopStats>>,
    runner_pool: Arc<RwLock<Vec<Arc<dyn crate::domain::entities::runner::Runner>>>>,
}

#[async_trait]
impl EventHandler for BuiltinNodeFailureHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        if let EventPayload::NodeFailure { node_id, reason } = event.payload {
            warn!(
                "Handling node failure for: {} (reason: {})",
                node_id, reason
            );

            let mut stats = self.stats.write().await;
            stats.node_failures += 1;

            // Remove runners from failed node
            let mut pool = self.runner_pool.write().await;
            pool.retain(|_runner| {
                // In a real implementation, we'd check if the runner belongs to the failed node
                true
            });
        }

        Ok(())
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![EventType::NodeFailure]
    }

    fn name(&self) -> &str {
        "builtin-node-failure-handler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::time::sleep;

    struct TestEventHandler {
        name: String,
        event_types: Vec<EventType>,
        call_count: Arc<AtomicU32>,
    }

    impl TestEventHandler {
        fn new(name: String, event_types: Vec<EventType>) -> Self {
            Self {
                name,
                event_types,
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl EventHandler for TestEventHandler {
        async fn handle(&self, _event: Event) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn event_types(&self) -> Vec<EventType> {
            self.event_types.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_event_demultiplexer_creation() {
        let demux = EventDemultiplexer::new();
        let stats = demux.get_stats().await;
        assert_eq!(stats.total_events, 0);
    }

    #[tokio::test]
    async fn test_event_handler_registration() {
        let demux = EventDemultiplexer::new();
        let handler = Arc::new(TestEventHandler::new(
            "test-handler".to_string(),
            vec![EventType::JobSubmission],
        ));

        demux.register_handler(handler.clone()).await.unwrap();

        let event = Event::new(
            EventType::JobSubmission,
            EventPayload::JobSubmission {
                job_id: Uuid::new_v4(),
                runner_id: None,
            },
        );

        demux.process_event(event).await.unwrap();
        assert_eq!(handler.call_count(), 1);
    }

    #[tokio::test]
    async fn test_event_priority_ordering() {
        let demux = EventDemultiplexer::new();

        let low_event = Event::with_priority(
            EventType::JobSubmission,
            EventPayload::JobSubmission {
                job_id: Uuid::new_v4(),
                runner_id: None,
            },
            EventPriority::Low,
        );

        let high_event = Event::with_priority(
            EventType::JobSubmission,
            EventPayload::JobSubmission {
                job_id: Uuid::new_v4(),
                runner_id: None,
            },
            EventPriority::High,
        );

        demux.submit_event(low_event).await.unwrap();
        demux.submit_event(high_event.clone()).await.unwrap();

        let events = demux.poll_events().await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].priority, EventPriority::High);
        assert_eq!(events[1].priority, EventPriority::Low);
    }

    #[tokio::test]
    async fn test_timer_scheduling() {
        let demux = EventDemultiplexer::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let callback = Box::new(move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        demux
            .schedule_timer(
                "test-timer".to_string(),
                Duration::from_millis(10),
                callback,
            )
            .await
            .unwrap();

        // Wait for timer to expire
        sleep(Duration::from_millis(20)).await;
        demux.process_timers().await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_event_statistics() {
        let demux = EventDemultiplexer::new();
        let handler = Arc::new(TestEventHandler::new(
            "test-handler".to_string(),
            vec![EventType::JobSubmission],
        ));

        demux.register_handler(handler).await.unwrap();

        let event = Event::new(
            EventType::JobSubmission,
            EventPayload::JobSubmission {
                job_id: Uuid::new_v4(),
                runner_id: None,
            },
        );

        demux.process_event(event).await.unwrap();

        let stats = demux.get_stats().await;
        assert_eq!(stats.total_events, 1);
        assert_eq!(stats.successful_events, 1);
        assert_eq!(stats.failed_events, 0);
        assert!(stats.last_processed.is_some());
    }
}
