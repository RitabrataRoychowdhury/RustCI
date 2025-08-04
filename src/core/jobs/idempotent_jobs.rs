//! Idempotent Job Lifecycle System
//!
//! This module implements idempotent job execution to support retries across
//! node failover scenarios, ensuring jobs can be safely retried without
//! causing duplicate side effects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::core::patterns::events::{DomainEvent, EventBus};
use crate::domain::entities::{JobId, NodeId, RunnerId};
use crate::error::Result;

/// Idempotency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyConfig {
    /// Enable idempotency tracking
    pub enabled: bool,
    /// Idempotency key expiration time in seconds
    pub key_expiration: u64,
    /// Maximum retry attempts for a job
    pub max_retry_attempts: u32,
    /// Retry backoff strategy
    pub retry_backoff: RetryBackoffStrategy,
    /// Enable duplicate detection
    pub duplicate_detection: bool,
    /// Cleanup interval for expired keys in seconds
    pub cleanup_interval: u64,
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key_expiration: 3600, // 1 hour
            max_retry_attempts: 3,
            retry_backoff: RetryBackoffStrategy::Exponential { base: 2.0, max_delay: 300 },
            duplicate_detection: true,
            cleanup_interval: 300, // 5 minutes
        }
    }
}

/// Retry backoff strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryBackoffStrategy {
    /// Fixed delay between retries
    Fixed { delay: u64 },
    /// Linear backoff (delay * attempt)
    Linear { base_delay: u64, max_delay: u64 },
    /// Exponential backoff (base^attempt)
    Exponential { base: f64, max_delay: u64 },
    /// Custom backoff with specific delays
    Custom { delays: Vec<u64> },
}

/// Job execution state for idempotency tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobExecutionState {
    /// Job is pending execution
    Pending,
    /// Job is currently running
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed,
    /// Job was cancelled
    Cancelled,
    /// Job is being retried
    Retrying,
}

/// Idempotency key for job execution
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey {
    /// Job ID
    pub job_id: JobId,
    /// Job configuration hash (for detecting changes)
    pub config_hash: String,
    /// Execution attempt number
    pub attempt: u32,
}

impl IdempotencyKey {
    /// Create a new idempotency key
    pub fn new(job_id: JobId, config_hash: String, attempt: u32) -> Self {
        Self {
            job_id,
            config_hash,
            attempt,
        }
    }


}

impl std::fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.job_id, self.config_hash, self.attempt)
    }
}

/// Job execution record for idempotency tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecutionRecord {
    /// Idempotency key
    pub key: IdempotencyKey,
    /// Current execution state
    pub state: JobExecutionState,
    /// Node where job is/was executing
    pub node_id: Option<NodeId>,
    /// Runner executing the job
    pub runner_id: Option<RunnerId>,
    /// Job started timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Job completed timestamp
    pub completed_at: Option<DateTime<Utc>>,
    /// Job result (if completed)
    pub result: Option<JobResult>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Last retry timestamp
    pub last_retry_at: Option<DateTime<Utc>>,
    /// Next retry timestamp
    pub next_retry_at: Option<DateTime<Utc>>,
    /// Record creation timestamp
    pub created_at: DateTime<Utc>,
    /// Record expiration timestamp
    pub expires_at: DateTime<Utc>,
    /// Job configuration for comparison
    pub job_config: serde_json::Value,
}

/// Job execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    /// Whether job succeeded
    pub success: bool,
    /// Exit code (if applicable)
    pub exit_code: Option<i32>,
    /// Output data
    pub output: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Resource usage metrics
    pub resource_usage: Option<serde_json::Value>,
}

/// Idempotency events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdempotencyEvent {
    /// Job execution started
    JobExecutionStarted {
        key: IdempotencyKey,
        node_id: NodeId,
        runner_id: RunnerId,
        timestamp: DateTime<Utc>,
    },
    /// Job execution completed
    JobExecutionCompleted {
        key: IdempotencyKey,
        result: JobResult,
        timestamp: DateTime<Utc>,
    },
    /// Job execution failed
    JobExecutionFailed {
        key: IdempotencyKey,
        error: String,
        retry_scheduled: bool,
        timestamp: DateTime<Utc>,
    },
    /// Job retry scheduled
    JobRetryScheduled {
        key: IdempotencyKey,
        retry_count: u32,
        next_retry_at: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },
    /// Duplicate job execution detected
    DuplicateJobDetected {
        key: IdempotencyKey,
        existing_state: JobExecutionState,
        timestamp: DateTime<Utc>,
    },
    /// Job execution record expired
    JobRecordExpired {
        key: IdempotencyKey,
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent for IdempotencyEvent {
    fn event_type(&self) -> &'static str {
        match self {
            IdempotencyEvent::JobExecutionStarted { .. } => "idempotency.job_execution_started",
            IdempotencyEvent::JobExecutionCompleted { .. } => "idempotency.job_execution_completed",
            IdempotencyEvent::JobExecutionFailed { .. } => "idempotency.job_execution_failed",
            IdempotencyEvent::JobRetryScheduled { .. } => "idempotency.job_retry_scheduled",
            IdempotencyEvent::DuplicateJobDetected { .. } => "idempotency.duplicate_job_detected",
            IdempotencyEvent::JobRecordExpired { .. } => "idempotency.job_record_expired",
        }
    }

    fn aggregate_id(&self) -> Uuid {
        match self {
            IdempotencyEvent::JobExecutionStarted { key, .. }
            | IdempotencyEvent::JobExecutionCompleted { key, .. }
            | IdempotencyEvent::JobExecutionFailed { key, .. }
            | IdempotencyEvent::JobRetryScheduled { key, .. }
            | IdempotencyEvent::DuplicateJobDetected { key, .. }
            | IdempotencyEvent::JobRecordExpired { key, .. } => key.job_id,
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            IdempotencyEvent::JobExecutionStarted { timestamp, .. }
            | IdempotencyEvent::JobExecutionCompleted { timestamp, .. }
            | IdempotencyEvent::JobExecutionFailed { timestamp, .. }
            | IdempotencyEvent::JobRetryScheduled { timestamp, .. }
            | IdempotencyEvent::DuplicateJobDetected { timestamp, .. }
            | IdempotencyEvent::JobRecordExpired { timestamp, .. } => *timestamp,
        }
    }

    fn correlation_id(&self) -> Uuid {
        self.aggregate_id()
    }
}

/// Idempotent Job Manager
pub struct IdempotentJobManager {
    /// Configuration
    config: IdempotencyConfig,
    /// Event bus for publishing events
    event_bus: Arc<EventBus>,
    /// Job execution records
    execution_records: Arc<RwLock<HashMap<IdempotencyKey, JobExecutionRecord>>>,
    /// Event channel for internal events
    event_sender: mpsc::UnboundedSender<IdempotencyEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<IdempotencyEvent>>>>,
    /// Cleanup timer handle
    cleanup_timer: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Retry scheduler timer handle
    retry_timer: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl IdempotentJobManager {
    /// Create a new idempotent job manager
    pub fn new(config: IdempotencyConfig, event_bus: Arc<EventBus>) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            config,
            event_bus,
            execution_records: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            cleanup_timer: Arc::new(RwLock::new(None)),
            retry_timer: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the idempotent job manager
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Idempotent job manager disabled");
            return Ok(());
        }

        info!("Starting idempotent job manager");

        // Start event processing
        self.start_event_processing().await;

        // Start cleanup timer
        self.start_cleanup_timer().await;

        // Start retry scheduler
        self.start_retry_scheduler().await;

        info!("Idempotent job manager started");
        Ok(())
    }

    /// Stop the idempotent job manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping idempotent job manager");

        // Stop timers
        self.stop_cleanup_timer().await;
        self.stop_retry_scheduler().await;

        info!("Idempotent job manager stopped");
        Ok(())
    }

    /// Check if a job execution is idempotent (safe to retry)
    pub async fn is_job_idempotent(&self, key: &IdempotencyKey) -> Result<bool> {
        if !self.config.enabled {
            return Ok(true); // Allow all jobs if idempotency is disabled
        }

        let records = self.execution_records.read().await;
        
        if let Some(record) = records.get(key) {
            // Check if record has expired
            if Utc::now() > record.expires_at {
                return Ok(true); // Expired record, safe to retry
            }

            match record.state {
                JobExecutionState::Pending | JobExecutionState::Running => {
                    // Job is already running, not safe to retry
                    Ok(false)
                }
                JobExecutionState::Completed => {
                    // Job completed successfully, return cached result
                    Ok(false)
                }
                JobExecutionState::Failed | JobExecutionState::Cancelled => {
                    // Job failed or was cancelled, safe to retry if under limit
                    Ok(record.retry_count < self.config.max_retry_attempts)
                }
                JobExecutionState::Retrying => {
                    // Job is being retried, not safe to start another retry
                    Ok(false)
                }
            }
        } else {
            // No existing record, safe to execute
            Ok(true)
        }
    }

    /// Start job execution with idempotency tracking
    pub async fn start_job_execution(
        &self,
        key: IdempotencyKey,
        node_id: NodeId,
        runner_id: RunnerId,
        job_config: serde_json::Value,
    ) -> Result<JobExecutionResult> {
        if !self.config.enabled {
            return Ok(JobExecutionResult::Proceed);
        }

        let mut records = self.execution_records.write().await;

        // Check for existing record
        if let Some(existing_record) = records.get(&key) {
            // Check if record has expired
            if Utc::now() > existing_record.expires_at {
                // Remove expired record
                records.remove(&key);
            } else {
                // Handle based on current state
                let existing_record_clone = existing_record.clone();
                let existing_state = existing_record.state.clone();
                let existing_result = existing_record.result.clone();
                
                return match existing_state {
                    JobExecutionState::Pending | JobExecutionState::Running => {
                        // Job is already running
                        if self.config.duplicate_detection {
                            let event = IdempotencyEvent::DuplicateJobDetected {
                                key: key.clone(),
                                existing_state: existing_state.clone(),
                                timestamp: Utc::now(),
                            };
                            drop(records); // Release lock before async call
                            self.publish_event(event).await?;
                        }
                        Ok(JobExecutionResult::Duplicate(existing_record_clone))
                    }
                    JobExecutionState::Completed => {
                        // Return cached result
                        if let Some(result) = existing_result {
                            Ok(JobExecutionResult::Cached(result))
                        } else {
                            Ok(JobExecutionResult::Proceed)
                        }
                    }
                    JobExecutionState::Failed | JobExecutionState::Cancelled => {
                        // Check if we can retry
                        if existing_record.retry_count >= self.config.max_retry_attempts {
                            Ok(JobExecutionResult::MaxRetriesExceeded(existing_record.clone()))
                        } else {
                            // Update record for retry
                            let mut updated_record = existing_record.clone();
                            updated_record.state = JobExecutionState::Running;
                            updated_record.node_id = Some(node_id);
                            updated_record.runner_id = Some(runner_id);
                            updated_record.started_at = Some(Utc::now());
                            updated_record.retry_count += 1;
                            updated_record.last_retry_at = Some(Utc::now());
                            
                            records.insert(key.clone(), updated_record);
                            let event_key = key.clone();
                            drop(records); // Release lock before async call
                            
                            let event = IdempotencyEvent::JobExecutionStarted {
                                key: event_key,
                                node_id,
                                runner_id,
                                timestamp: Utc::now(),
                            };
                            self.publish_event(event).await?;
                            
                            Ok(JobExecutionResult::Retry)
                        }
                    }
                    JobExecutionState::Retrying => {
                        // Job is already being retried
                        Ok(JobExecutionResult::Duplicate(existing_record.clone()))
                    }
                };
            }
        }

        // Create new execution record
        let record = JobExecutionRecord {
            key: key.clone(),
            state: JobExecutionState::Running,
            node_id: Some(node_id),
            runner_id: Some(runner_id),
            started_at: Some(Utc::now()),
            completed_at: None,
            result: None,
            retry_count: 0,
            last_retry_at: None,
            next_retry_at: None,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(self.config.key_expiration as i64),
            job_config,
        };

        let event_key = key.clone();
        records.insert(key, record);
        drop(records); // Release lock before async call

        // Publish event
        let event = IdempotencyEvent::JobExecutionStarted {
            key: event_key,
            node_id,
            runner_id,
            timestamp: Utc::now(),
        };
        self.publish_event(event).await?;

        Ok(JobExecutionResult::Proceed)
    }

    /// Complete job execution
    pub async fn complete_job_execution(
        &self,
        key: IdempotencyKey,
        result: JobResult,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut records = self.execution_records.write().await;
        
        if let Some(record) = records.get_mut(&key) {
            record.state = if result.success {
                JobExecutionState::Completed
            } else {
                JobExecutionState::Failed
            };
            record.completed_at = Some(Utc::now());
            record.result = Some(result.clone());

            // Schedule retry if failed and under limit
            let should_retry = !result.success && record.retry_count < self.config.max_retry_attempts;
            let retry_event = if should_retry {
                let next_retry_delay = self.calculate_retry_delay(record.retry_count);
                record.next_retry_at = Some(Utc::now() + chrono::Duration::seconds(next_retry_delay as i64));
                record.state = JobExecutionState::Retrying;

                Some(IdempotencyEvent::JobRetryScheduled {
                    key: key.clone(),
                    retry_count: record.retry_count,
                    next_retry_at: record.next_retry_at.unwrap(),
                    timestamp: Utc::now(),
                })
            } else {
                None
            };

            // Prepare data for events before dropping records
            let event_key = key.clone();
            let event_result = result.clone();
            let retry_scheduled = should_retry;

            drop(records); // Release lock before async call

            if let Some(event) = retry_event {
                self.publish_event(event).await?;
            }

            // Publish completion event
            let event = if event_result.success {
                IdempotencyEvent::JobExecutionCompleted {
                    key: event_key,
                    result: event_result,
                    timestamp: Utc::now(),
                }
            } else {
                IdempotencyEvent::JobExecutionFailed {
                    key: event_key,
                    error: event_result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    retry_scheduled,
                    timestamp: Utc::now(),
                }
            };
            self.publish_event(event).await?;
        }

        Ok(())
    }

    /// Cancel job execution
    pub async fn cancel_job_execution(&self, key: IdempotencyKey) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut records = self.execution_records.write().await;
        
        if let Some(record) = records.get_mut(&key) {
            record.state = JobExecutionState::Cancelled;
            record.completed_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Get job execution record
    pub async fn get_execution_record(&self, key: &IdempotencyKey) -> Option<JobExecutionRecord> {
        let records = self.execution_records.read().await;
        records.get(key).cloned()
    }

    /// Get all execution records for a job
    pub async fn get_job_execution_history(&self, job_id: JobId) -> Vec<JobExecutionRecord> {
        let records = self.execution_records.read().await;
        records
            .values()
            .filter(|record| record.key.job_id == job_id)
            .cloned()
            .collect()
    }

    /// Get jobs ready for retry
    pub async fn get_jobs_ready_for_retry(&self) -> Vec<IdempotencyKey> {
        let records = self.execution_records.read().await;
        let now = Utc::now();
        
        records
            .values()
            .filter(|record| {
                record.state == JobExecutionState::Retrying &&
                record.next_retry_at.is_some_and(|retry_at| now >= retry_at)
            })
            .map(|record| record.key.clone())
            .collect()
    }

    /// Calculate retry delay based on backoff strategy
    fn calculate_retry_delay(&self, retry_count: u32) -> u64 {
        match &self.config.retry_backoff {
            RetryBackoffStrategy::Fixed { delay } => *delay,
            RetryBackoffStrategy::Linear { base_delay, max_delay } => {
                std::cmp::min(base_delay * (retry_count as u64 + 1), *max_delay)
            }
            RetryBackoffStrategy::Exponential { base, max_delay } => {
                (base.powi(retry_count as i32) as u64).min(*max_delay)
            }
            RetryBackoffStrategy::Custom { delays } => {
                delays.get(retry_count as usize).copied().unwrap_or_else(|| {
                    delays.last().copied().unwrap_or(60) // Default to 60 seconds
                })
            }
        }
    }

    /// Start cleanup timer for expired records
    async fn start_cleanup_timer(&self) {
        let execution_records = self.execution_records.clone();
        let event_sender = self.event_sender.clone();
        let cleanup_interval = self.config.cleanup_interval;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(cleanup_interval));

            loop {
                interval.tick().await;

                let expired_keys = {
                    let mut records = execution_records.write().await;
                    let now = Utc::now();
                    let mut expired_keys = Vec::new();

                    records.retain(|key, record| {
                        if now > record.expires_at {
                            expired_keys.push(key.clone());
                            false
                        } else {
                            true
                        }
                    });

                    expired_keys
                };

                // Publish expiration events
                for key in expired_keys {
                    let event = IdempotencyEvent::JobRecordExpired {
                        key,
                        timestamp: Utc::now(),
                    };
                    if event_sender.send(event).is_err() {
                        warn!("Failed to send job record expired event");
                    }
                }
            }
        });

        let mut timer = self.cleanup_timer.write().await;
        *timer = Some(handle);
    }

    /// Stop cleanup timer
    async fn stop_cleanup_timer(&self) {
        let mut timer = self.cleanup_timer.write().await;
        if let Some(handle) = timer.take() {
            handle.abort();
        }
    }

    /// Start retry scheduler
    async fn start_retry_scheduler(&self) {
        let execution_records = self.execution_records.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10)); // Check every 10 seconds

            loop {
                interval.tick().await;

                let ready_for_retry = {
                    let records = execution_records.read().await;
                    let now = Utc::now();
                    
                    records
                        .values()
                        .filter(|record| {
                            record.state == JobExecutionState::Retrying &&
                            record.next_retry_at.is_some_and(|retry_at| now >= retry_at)
                        })
                        .map(|record| record.key.clone())
                        .collect::<Vec<_>>()
                };

                for key in ready_for_retry {
                    debug!(
                        job_id = %key.job_id,
                        attempt = key.attempt,
                        "Job ready for retry"
                    );
                    // In a real implementation, this would trigger job rescheduling
                }
            }
        });

        let mut timer = self.retry_timer.write().await;
        *timer = Some(handle);
    }

    /// Stop retry scheduler
    async fn stop_retry_scheduler(&self) {
        let mut timer = self.retry_timer.write().await;
        if let Some(handle) = timer.take() {
            handle.abort();
        }
    }

    /// Publish an event
    async fn publish_event(&self, event: IdempotencyEvent) -> Result<()> {
        // Send to internal channel
        if self.event_sender.send(event.clone()).is_err() {
            warn!("Failed to send idempotency event to internal channel");
        }

        // Publish to event bus
        self.event_bus.publish(event).await?;

        Ok(())
    }

    /// Start event processing task
    async fn start_event_processing(&self) {
        let event_receiver = self.event_receiver.clone();

        tokio::spawn(async move {
            let receiver = {
                let mut guard = event_receiver.write().await;
                guard.take()
            };

            if let Some(mut rx) = receiver {
                while let Some(event) = rx.recv().await {
                    Self::process_idempotency_event(event).await;
                }
            }
        });
    }

    /// Process idempotency events
    async fn process_idempotency_event(event: IdempotencyEvent) {
        match event {
            IdempotencyEvent::JobExecutionStarted { key, node_id, runner_id, timestamp } => {
                info!(
                    job_id = %key.job_id,
                    attempt = key.attempt,
                    node_id = %node_id,
                    runner_id = %runner_id,
                    timestamp = %timestamp,
                    "Job execution started"
                );
            }
            IdempotencyEvent::JobExecutionCompleted { key, result, timestamp } => {
                info!(
                    job_id = %key.job_id,
                    attempt = key.attempt,
                    success = result.success,
                    duration_ms = result.duration_ms,
                    timestamp = %timestamp,
                    "Job execution completed"
                );
            }
            IdempotencyEvent::JobExecutionFailed { key, error, retry_scheduled, timestamp } => {
                warn!(
                    job_id = %key.job_id,
                    attempt = key.attempt,
                    error = %error,
                    retry_scheduled = retry_scheduled,
                    timestamp = %timestamp,
                    "Job execution failed"
                );
            }
            IdempotencyEvent::DuplicateJobDetected { key, existing_state, timestamp } => {
                warn!(
                    job_id = %key.job_id,
                    attempt = key.attempt,
                    existing_state = ?existing_state,
                    timestamp = %timestamp,
                    "Duplicate job execution detected"
                );
            }
            _ => {
                debug!(event = ?event, "Idempotency event processed");
            }
        }
    }
}

/// Result of starting job execution
#[derive(Debug, Clone)]
pub enum JobExecutionResult {
    /// Job should proceed with execution
    Proceed,
    /// Job is a duplicate of an existing execution
    Duplicate(JobExecutionRecord),
    /// Job result is cached from previous execution
    Cached(JobResult),
    /// Job is being retried
    Retry,
    /// Maximum retry attempts exceeded
    MaxRetriesExceeded(JobExecutionRecord),
}

/// Utility functions for creating idempotency keys
pub mod key_utils {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    /// Create idempotency key from job configuration
    pub fn create_key_from_config(
        job_id: JobId,
        job_config: &serde_json::Value,
        attempt: u32,
    ) -> IdempotencyKey {
        let config_hash = hash_config(job_config);
        IdempotencyKey::new(job_id, config_hash, attempt)
    }

    /// Hash job configuration for idempotency
    pub fn hash_config(config: &serde_json::Value) -> String {
        let mut hasher = DefaultHasher::new();
        config.to_string().hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patterns::correlation::CorrelationTracker;

    #[tokio::test]
    async fn test_idempotent_job_manager_creation() {
        let config = IdempotencyConfig::default();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let job_manager = IdempotentJobManager::new(config, event_bus);
        
        // Test basic functionality
        let key = IdempotencyKey::new(Uuid::new_v4(), "test-hash".to_string(), 1);
        let is_idempotent = job_manager.is_job_idempotent(&key).await.unwrap();
        assert!(is_idempotent);
    }

    #[tokio::test]
    async fn test_job_execution_lifecycle() {
        let config = IdempotencyConfig::default();
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let job_manager = IdempotentJobManager::new(config, event_bus);
        job_manager.start().await.unwrap();
        
        let job_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        let runner_id = Uuid::new_v4();
        let key = IdempotencyKey::new(job_id, "test-hash".to_string(), 1);
        let job_config = serde_json::json!({"command": "echo hello"});
        
        // Start job execution
        let result = job_manager.start_job_execution(
            key.clone(),
            node_id,
            runner_id,
            job_config,
        ).await.unwrap();
        
        assert!(matches!(result, JobExecutionResult::Proceed));
        
        // Try to start same job again (should be duplicate)
        let result = job_manager.start_job_execution(
            key.clone(),
            node_id,
            runner_id,
            serde_json::json!({"command": "echo hello"}),
        ).await.unwrap();
        
        assert!(matches!(result, JobExecutionResult::Duplicate(_)));
        
        // Complete the job
        let job_result = JobResult {
            success: true,
            exit_code: Some(0),
            output: Some("hello".to_string()),
            error: None,
            duration_ms: 100,
            resource_usage: None,
        };
        
        job_manager.complete_job_execution(key.clone(), job_result.clone()).await.unwrap();
        
        // Try to start same job again (should return cached result)
        let result = job_manager.start_job_execution(
            key.clone(),
            node_id,
            runner_id,
            serde_json::json!({"command": "echo hello"}),
        ).await.unwrap();
        
        assert!(matches!(result, JobExecutionResult::Cached(_)));
        
        job_manager.stop().await.unwrap();
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = IdempotencyConfig {
            retry_backoff: RetryBackoffStrategy::Exponential { base: 2.0, max_delay: 300 },
            ..Default::default()
        };
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker, None));
        
        let job_manager = IdempotentJobManager::new(config, event_bus);
        
        assert_eq!(job_manager.calculate_retry_delay(0), 1);
        assert_eq!(job_manager.calculate_retry_delay(1), 2);
        assert_eq!(job_manager.calculate_retry_delay(2), 4);
        assert_eq!(job_manager.calculate_retry_delay(3), 8);
        assert_eq!(job_manager.calculate_retry_delay(10), 300); // Max delay
    }

    #[test]
    fn test_idempotency_key_creation() {
        let job_id = Uuid::new_v4();
        let config = serde_json::json!({"command": "echo test"});
        
        let key1 = key_utils::create_key_from_config(job_id, &config, 1);
        let key2 = key_utils::create_key_from_config(job_id, &config, 1);
        
        assert_eq!(key1, key2);
        
        let key3 = key_utils::create_key_from_config(job_id, &config, 2);
        assert_ne!(key1, key3);
        
        let different_config = serde_json::json!({"command": "echo different"});
        let key4 = key_utils::create_key_from_config(job_id, &different_config, 1);
        assert_ne!(key1, key4);
    }
}