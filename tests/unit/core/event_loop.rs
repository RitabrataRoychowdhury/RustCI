//! Unit tests for the event loop implementation

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use rustci::core::event_loop::{
    Event, EventDemultiplexer, EventHandler, EventPayload, EventPriority, EventType,
};
use rustci::error::Result;

/// Test event handler that counts invocations
struct CountingEventHandler {
    name: String,
    event_types: Vec<EventType>,
    call_count: Arc<AtomicU32>,
    should_fail: bool,
}

impl CountingEventHandler {
    fn new(name: String, event_types: Vec<EventType>) -> Self {
        Self {
            name,
            event_types,
            call_count: Arc::new(AtomicU32::new(0)),
            should_fail: false,
        }
    }

    fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl EventHandler for CountingEventHandler {
    async fn handle(&self, _event: Event) -> Result<()> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail {
            return Err(rustci::error::AppError::InternalServerError(
                "Handler intentionally failed".to_string(),
            ));
        }

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
    assert_eq!(stats.successful_events, 0);
    assert_eq!(stats.failed_events, 0);
    assert!(stats.last_processed.is_none());
}

#[tokio::test]
async fn test_event_handler_registration() {
    let demux = EventDemultiplexer::new();
    let handler = Arc::new(CountingEventHandler::new(
        "test-handler".to_string(),
        vec![EventType::JobSubmission],
    ));

    // Register handler
    demux.register_handler(handler.clone()).await.unwrap();

    // Create and process event
    let event = Event::new(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
    );

    demux.process_event(event).await.unwrap();

    // Verify handler was called
    assert_eq!(handler.call_count(), 1);
}

#[tokio::test]
async fn test_event_handler_unregistration() {
    let demux = EventDemultiplexer::new();
    let handler = Arc::new(CountingEventHandler::new(
        "test-handler".to_string(),
        vec![EventType::JobSubmission],
    ));

    // Register and then unregister handler
    demux.register_handler(handler.clone()).await.unwrap();
    demux.unregister_handler("test-handler").await.unwrap();

    // Create and process event
    let event = Event::new(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
    );

    demux.process_event(event).await.unwrap();

    // Handler should not have been called
    assert_eq!(handler.call_count(), 0);
}

#[tokio::test]
async fn test_multiple_event_handlers() {
    let demux = EventDemultiplexer::new();

    let handler1 = Arc::new(CountingEventHandler::new(
        "handler1".to_string(),
        vec![EventType::JobSubmission],
    ));
    let handler2 = Arc::new(CountingEventHandler::new(
        "handler2".to_string(),
        vec![EventType::JobSubmission],
    ));

    // Register both handlers
    demux.register_handler(handler1.clone()).await.unwrap();
    demux.register_handler(handler2.clone()).await.unwrap();

    // Create and process event
    let event = Event::new(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
    );

    demux.process_event(event).await.unwrap();

    // Both handlers should have been called
    assert_eq!(handler1.call_count(), 1);
    assert_eq!(handler2.call_count(), 1);
}

#[tokio::test]
async fn test_event_priority_ordering() {
    let demux = EventDemultiplexer::new();

    // Submit events in reverse priority order
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

    let critical_event = Event::with_priority(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
        EventPriority::Critical,
    );

    demux.submit_event(low_event).await.unwrap();
    demux.submit_event(high_event.clone()).await.unwrap();
    demux.submit_event(critical_event.clone()).await.unwrap();

    // Poll events - should be in priority order
    let events = demux.poll_events().await;
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].priority, EventPriority::Critical);
    assert_eq!(events[1].priority, EventPriority::High);
    assert_eq!(events[2].priority, EventPriority::Low);
}

#[tokio::test]
async fn test_event_submission_and_polling() {
    let demux = EventDemultiplexer::new();

    let event1 = Event::new(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
    );
    let event2 = Event::new(
        EventType::JobCompletion,
        EventPayload::JobCompletion {
            job_id: Uuid::new_v4(),
            result: rustci::core::event_loop::JobCompletionResult {
                success: true,
                exit_code: Some(0),
                output: "test output".to_string(),
                error: None,
                duration: Duration::from_secs(1),
            },
        },
    );

    // Submit events
    demux.submit_event(event1.clone()).await.unwrap();
    demux.submit_event(event2.clone()).await.unwrap();

    // Poll events
    let events = demux.poll_events().await;
    assert_eq!(events.len(), 2);

    // Queue should be empty after polling
    let empty_events = demux.poll_events().await;
    assert!(empty_events.is_empty());
}

#[tokio::test]
async fn test_event_handler_failure() {
    let demux = EventDemultiplexer::new();
    let handler = Arc::new(
        CountingEventHandler::new(
            "failing-handler".to_string(),
            vec![EventType::JobSubmission],
        )
        .with_failure(),
    );

    demux.register_handler(handler.clone()).await.unwrap();

    let event = Event::new(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
    );

    // Process event - should not fail even if handler fails
    demux.process_event(event).await.unwrap();

    // Handler should have been called
    assert_eq!(handler.call_count(), 1);

    // Statistics should reflect the failure
    let stats = demux.get_stats().await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.successful_events, 0);
    assert_eq!(stats.failed_events, 1);
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

    // Schedule timer
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

    // Process timers
    demux.process_timers().await.unwrap();

    // Callback should have been called
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_recurring_timer() {
    let demux = EventDemultiplexer::new();
    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();

    let callback = Box::new(move || {
        call_count_clone.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });

    // Schedule recurring timer
    demux
        .schedule_recurring_timer(
            "recurring-timer".to_string(),
            Duration::from_millis(10),
            callback,
        )
        .await
        .unwrap();

    // Wait and process timers multiple times
    for _ in 0..3 {
        sleep(Duration::from_millis(15)).await;
        demux.process_timers().await.unwrap();
    }

    // Callback should have been called multiple times
    let count = call_count.load(Ordering::SeqCst);
    assert!(count >= 2, "Expected at least 2 calls, got {}", count);
}

#[tokio::test]
async fn test_timer_cancellation() {
    let demux = EventDemultiplexer::new();
    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();

    let callback = Box::new(move || {
        call_count_clone.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });

    // Schedule timer
    demux
        .schedule_timer(
            "cancel-timer".to_string(),
            Duration::from_millis(20),
            callback,
        )
        .await
        .unwrap();

    // Cancel timer before it expires
    demux.cancel_timer("cancel-timer").await.unwrap();

    // Wait and process timers
    sleep(Duration::from_millis(30)).await;
    demux.process_timers().await.unwrap();

    // Callback should not have been called
    assert_eq!(call_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_event_statistics() {
    let demux = EventDemultiplexer::new();
    let handler = Arc::new(CountingEventHandler::new(
        "stats-handler".to_string(),
        vec![EventType::JobSubmission, EventType::JobCompletion],
    ));

    demux.register_handler(handler).await.unwrap();

    // Process multiple events
    for i in 0..5 {
        let event = Event::new(
            if i % 2 == 0 {
                EventType::JobSubmission
            } else {
                EventType::JobCompletion
            },
            if i % 2 == 0 {
                EventPayload::JobSubmission {
                    job_id: Uuid::new_v4(),
                    runner_id: None,
                }
            } else {
                EventPayload::JobCompletion {
                    job_id: Uuid::new_v4(),
                    result: rustci::core::event_loop::JobCompletionResult {
                        success: true,
                        exit_code: Some(0),
                        output: "test".to_string(),
                        error: None,
                        duration: Duration::from_millis(100),
                    },
                }
            },
        );

        demux.process_event(event).await.unwrap();
    }

    // Check statistics
    let stats = demux.get_stats().await;
    assert_eq!(stats.total_events, 5);
    assert_eq!(stats.successful_events, 5);
    assert_eq!(stats.failed_events, 0);
    assert!(stats.last_processed.is_some());
    assert_eq!(stats.events_by_type.len(), 2);
}

#[tokio::test]
async fn test_event_metadata() {
    let demux = EventDemultiplexer::new();

    let event = Event::new(
        EventType::Custom("test".to_string()),
        EventPayload::Custom {
            data: serde_json::json!({"test": "value"}),
        },
    )
    .with_metadata("source".to_string(), "test".to_string())
    .with_metadata("version".to_string(), "1.0".to_string());

    assert_eq!(event.metadata.get("source"), Some(&"test".to_string()));
    assert_eq!(event.metadata.get("version"), Some(&"1.0".to_string()));
}

#[tokio::test]
async fn test_statistics_reset() {
    let demux = EventDemultiplexer::new();
    let handler = Arc::new(CountingEventHandler::new(
        "reset-handler".to_string(),
        vec![EventType::JobSubmission],
    ));

    demux.register_handler(handler).await.unwrap();

    // Process an event
    let event = Event::new(
        EventType::JobSubmission,
        EventPayload::JobSubmission {
            job_id: Uuid::new_v4(),
            runner_id: None,
        },
    );

    demux.process_event(event).await.unwrap();

    // Verify statistics
    let stats = demux.get_stats().await;
    assert_eq!(stats.total_events, 1);

    // Reset statistics
    demux.reset_stats().await.unwrap();

    // Verify reset
    let stats = demux.get_stats().await;
    assert_eq!(stats.total_events, 0);
    assert_eq!(stats.successful_events, 0);
    assert_eq!(stats.failed_events, 0);
    assert!(stats.last_processed.is_none());
}
