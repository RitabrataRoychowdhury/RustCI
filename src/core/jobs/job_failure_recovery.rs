//! Job Failure Recovery System
//!
//! This module provides comprehensive failure recovery mechanisms for jobs
//! in a distributed environment, including automatic rescheduling and
//! failure pattern analysis.

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration as ChronoDuration, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::core::jobs::distributed_job_scheduler::{
    DistributedJobScheduler, ReschedulingReason,
};
use crate::core::patterns::events::{EventBus, DomainEvent, EventHandler};
use crate::domain::entities::{JobId, JobResult, NodeId};
use crate::error::Result;

/// Failure recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecoveryConfig {
    /// Enable automatic failure recovery
    pub enabled: bool,
    /// Maximum recovery attempts per job
    pub max_recovery_attempts: u32,
    /// Recovery attempt delay in seconds
    pub recovery_delay: u64,
    /// Node failure detection threshold (consecutive failures)
    pub node_failure_threshold: u32,
    /// Enable failure pattern analysis
    pub enable_pattern_analysis: bool,
    /// Failure pattern analysis window in hours
    pub pattern_analysis_window: u64,
    /// Minimum failures for pattern detection
    pub min_failures_for_pattern: u32,
}

impl Default for FailureRecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_recovery_attempts: 3,
            recovery_delay: 60,
            node_failure_threshold: 3,
            enable_pattern_analysis: true,
            pattern_analysis_window: 24,
            min_failures_for_pattern: 5,
        }
    }
}

/// Job failure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobFailure {
    /// Job ID
    pub job_id: JobId,
    /// Node where failure occurred
    pub node_id: NodeId,
    /// Failure timestamp
    pub failed_at: DateTime<Utc>,
    /// Failure reason
    pub reason: FailureReason,
    /// Error message
    pub error_message: String,
    /// Recovery attempt number
    pub recovery_attempt: u32,
    /// Job result if available
    pub job_result: Option<JobResult>,
}

/// Failure reasons categorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureReason {
    /// Node became unavailable
    NodeUnavailable,
    /// Resource exhaustion
    ResourceExhaustion,
    /// Job timeout
    JobTimeout,
    /// Job execution error
    ExecutionError,
    /// Network connectivity issues
    NetworkError,
    /// Unknown failure
    Unknown,
}

/// Failure pattern detected in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Pattern ID
    pub id: String,
    /// Pattern type
    pub pattern_type: FailurePatternType,
    /// Affected nodes
    pub affected_nodes: Vec<NodeId>,
    /// Failure count in pattern
    pub failure_count: u32,
    /// Pattern detection timestamp
    pub detected_at: DateTime<Utc>,
    /// Pattern confidence score (0-100)
    pub confidence: f64,
    /// Recommended actions
    pub recommended_actions: Vec<RecoveryAction>,
}

/// Types of failure patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailurePatternType {
    /// Single node experiencing repeated failures
    NodeSpecific,
    /// Cascading failures across multiple nodes
    Cascading,
    /// Resource-related failures
    ResourceBased,
    /// Time-based failure pattern
    Temporal,
    /// Job-type specific failures
    JobTypeSpecific,
}

/// Recovery actions that can be taken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Reschedule job to different node
    RescheduleJob { job_id: JobId, target_node: Option<NodeId> },
    /// Mark node for maintenance
    MarkNodeMaintenance { node_id: NodeId },
    /// Scale up cluster resources
    ScaleUpResources { resource_type: String },
    /// Adjust job priorities
    AdjustJobPriorities { priority_adjustment: i32 },
    /// Send alert to administrators
    SendAlert { message: String, severity: AlertSeverity },
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Failure recovery statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecoveryStats {
    /// Total failures detected
    pub total_failures: u64,
    /// Total recovery attempts
    pub total_recovery_attempts: u64,
    /// Successful recoveries
    pub successful_recoveries: u64,
    /// Failed recoveries
    pub failed_recoveries: u64,
    /// Recovery success rate
    pub recovery_success_rate: f64,
    /// Failures by reason
    pub failures_by_reason: HashMap<String, u64>,
    /// Failures by node
    pub failures_by_node: HashMap<NodeId, u64>,
    /// Patterns detected
    pub patterns_detected: u32,
    /// Average recovery time in seconds
    pub avg_recovery_time: f64,
}

/// Job failure recovery system
pub struct JobFailureRecovery {
    /// Configuration
    config: FailureRecoveryConfig,
    /// Distributed job scheduler for rescheduling
    scheduler: Arc<DistributedJobScheduler>,
    /// Event bus for listening to failures
    event_bus: Arc<EventBus>,
    /// Failure history
    failure_history: Arc<RwLock<Vec<JobFailure>>>,
    /// Detected patterns
    detected_patterns: Arc<RwLock<Vec<FailurePattern>>>,
    /// Recovery attempts tracking
    recovery_attempts: Arc<RwLock<HashMap<JobId, u32>>>,
    /// Node failure counters
    node_failure_counters: Arc<RwLock<HashMap<NodeId, u32>>>,
    /// Statistics
    stats: Arc<RwLock<FailureRecoveryStats>>,
    /// Pattern analysis state
    pattern_analysis_state: Arc<Mutex<PatternAnalysisState>>,
}

/// State for pattern analysis
#[derive(Debug)]
struct PatternAnalysisState {
    /// Last analysis timestamp
    last_analysis: DateTime<Utc>,
    /// Analysis in progress
    analysis_in_progress: bool,
    /// Temporary pattern candidates
    pattern_candidates: Vec<FailurePattern>,
}

impl JobFailureRecovery {
    /// Create a new failure recovery system
    pub fn new(
        config: FailureRecoveryConfig,
        scheduler: Arc<DistributedJobScheduler>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            config,
            scheduler,
            event_bus,
            failure_history: Arc::new(RwLock::new(Vec::new())),
            detected_patterns: Arc::new(RwLock::new(Vec::new())),
            recovery_attempts: Arc::new(RwLock::new(HashMap::new())),
            node_failure_counters: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(FailureRecoveryStats::default())),
            pattern_analysis_state: Arc::new(Mutex::new(PatternAnalysisState {
                last_analysis: Utc::now(),
                analysis_in_progress: false,
                pattern_candidates: Vec::new(),
            })),
        }
    }

    /// Start the failure recovery system
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Failure recovery system is disabled");
            return Ok(());
        }

        info!("Starting job failure recovery system");

        // Register event handlers
        self.register_event_handlers().await?;

        // Start pattern analysis task
        if self.config.enable_pattern_analysis {
            self.start_pattern_analysis_task().await;
        }

        // Start cleanup task
        self.start_cleanup_task().await;

        info!("Job failure recovery system started");
        Ok(())
    }

    /// Handle a job failure
    pub async fn handle_job_failure(
        &self,
        job_id: JobId,
        node_id: NodeId,
        reason: FailureReason,
        error_message: String,
        job_result: Option<JobResult>,
    ) -> Result<()> {
        info!("Handling job failure: {} on node {} - {:?}", job_id, node_id, reason);

        // Create failure record
        let failure = JobFailure {
            job_id,
            node_id,
            failed_at: Utc::now(),
            reason: reason.clone(),
            error_message: error_message.clone(),
            recovery_attempt: self.get_recovery_attempt_count(job_id).await,
            job_result,
        };

        // Store failure in history
        {
            let mut history = self.failure_history.write().await;
            history.push(failure.clone());
            
            // Keep only recent failures (last 1000)
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        // Update node failure counter
        {
            let mut counters = self.node_failure_counters.write().await;
            let count = counters.entry(node_id).or_insert(0);
            *count += 1;
        }

        // Update statistics
        self.update_failure_stats(&reason, node_id).await;

        // Attempt recovery if enabled and within limits
        if self.should_attempt_recovery(job_id).await {
            self.attempt_job_recovery(failure).await?;
        } else {
            warn!("Recovery not attempted for job {} - limits exceeded", job_id);
        }

        // Check for node failure threshold
        self.check_node_failure_threshold(node_id).await?;

        Ok(())
    }

    /// Check if recovery should be attempted for a job
    async fn should_attempt_recovery(&self, job_id: JobId) -> bool {
        let attempts = self.get_recovery_attempt_count(job_id).await;
        attempts < self.config.max_recovery_attempts
    }

    /// Get the current recovery attempt count for a job
    async fn get_recovery_attempt_count(&self, job_id: JobId) -> u32 {
        let attempts = self.recovery_attempts.read().await;
        attempts.get(&job_id).copied().unwrap_or(0)
    }

    /// Attempt to recover a failed job
    async fn attempt_job_recovery(&self, failure: JobFailure) -> Result<()> {
        let job_id = failure.job_id;
        
        // Increment recovery attempt counter
        {
            let mut attempts = self.recovery_attempts.write().await;
            let count = attempts.entry(job_id).or_insert(0);
            *count += 1;
        }

        // Wait for recovery delay
        if self.config.recovery_delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(self.config.recovery_delay)).await;
        }

        // Determine rescheduling reason based on failure reason
        let reschedule_reason = match failure.reason {
            FailureReason::NodeUnavailable => ReschedulingReason::NodeUnavailable,
            FailureReason::ResourceExhaustion => ReschedulingReason::ResourceConstraints,
            FailureReason::NetworkError => ReschedulingReason::NodeFailure,
            _ => ReschedulingReason::LoadBalancing,
        };

        // Attempt to reschedule the job
        match self.scheduler.reschedule_job(job_id, reschedule_reason).await {
            Ok(placement) => {
                info!("Successfully rescheduled job {} to node {}", job_id, placement.node_id);
                
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.total_recovery_attempts += 1;
                    stats.successful_recoveries += 1;
                    stats.recovery_success_rate = 
                        (stats.successful_recoveries as f64 / stats.total_recovery_attempts as f64) * 100.0;
                }
            }
            Err(e) => {
                error!("Failed to reschedule job {}: {}", job_id, e);
                
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.total_recovery_attempts += 1;
                    stats.failed_recoveries += 1;
                    stats.recovery_success_rate = 
                        (stats.successful_recoveries as f64 / stats.total_recovery_attempts as f64) * 100.0;
                }
                
                return Err(e);
            }
        }

        Ok(())
    }

    /// Check if a node has exceeded the failure threshold
    async fn check_node_failure_threshold(&self, node_id: NodeId) -> Result<()> {
        let failure_count = {
            let counters = self.node_failure_counters.read().await;
            counters.get(&node_id).copied().unwrap_or(0)
        };

        if failure_count >= self.config.node_failure_threshold {
            warn!("Node {} has exceeded failure threshold ({} failures)", node_id, failure_count);
            
            // Create recovery action
            let action = RecoveryAction::MarkNodeMaintenance { node_id };
            self.execute_recovery_action(action).await?;
            
            // Reset counter after taking action
            {
                let mut counters = self.node_failure_counters.write().await;
                counters.insert(node_id, 0);
            }
        }

        Ok(())
    }

    /// Execute a recovery action
    async fn execute_recovery_action(&self, action: RecoveryAction) -> Result<()> {
        match action {
            RecoveryAction::RescheduleJob { job_id, target_node: _ } => {
                // This would typically be handled by the main recovery flow
                info!("Recovery action: Reschedule job {}", job_id);
            }
            RecoveryAction::MarkNodeMaintenance { node_id } => {
                warn!("Recovery action: Mark node {} for maintenance", node_id);
                // In a real implementation, this would update the node status
            }
            RecoveryAction::ScaleUpResources { resource_type } => {
                info!("Recovery action: Scale up {} resources", resource_type);
                // This would trigger auto-scaling if available
            }
            RecoveryAction::AdjustJobPriorities { priority_adjustment } => {
                info!("Recovery action: Adjust job priorities by {}", priority_adjustment);
                // This would modify job scheduling priorities
            }
            RecoveryAction::SendAlert { message, severity } => {
                match severity {
                    AlertSeverity::Critical => error!("CRITICAL ALERT: {}", message),
                    AlertSeverity::High => error!("HIGH ALERT: {}", message),
                    AlertSeverity::Medium => warn!("MEDIUM ALERT: {}", message),
                    AlertSeverity::Low => info!("LOW ALERT: {}", message),
                }
            }
        }

        Ok(())
    }

    /// Update failure statistics
    async fn update_failure_stats(&self, reason: &FailureReason, node_id: NodeId) {
        let mut stats = self.stats.write().await;
        
        stats.total_failures += 1;
        
        // Update failures by reason
        let reason_str = format!("{:?}", reason);
        *stats.failures_by_reason.entry(reason_str).or_insert(0) += 1;
        
        // Update failures by node
        *stats.failures_by_node.entry(node_id).or_insert(0) += 1;
    }

    /// Register event handlers for failure detection
    async fn register_event_handlers(&self) -> Result<()> {
        // Create event handler for job failures
        let failure_recovery = Arc::new(self.clone());
        let handler = Arc::new(JobFailureEventHandler::new(failure_recovery));
        
        // Register handler with event bus
        self.event_bus.register_handler(handler).await?;
        
        Ok(())
    }

    /// Start pattern analysis background task
    async fn start_pattern_analysis_task(&self) {
        let failure_history = self.failure_history.clone();
        let detected_patterns = self.detected_patterns.clone();
        let pattern_analysis_state = self.pattern_analysis_state.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(3600) // Run every hour
            );

            loop {
                interval.tick().await;

                // Check if analysis should run
                let should_analyze = {
                    let state = pattern_analysis_state.lock().await;
                    if state.analysis_in_progress {
                        false
                    } else {
                        let time_since_last = Utc::now() - state.last_analysis;
                        time_since_last.num_hours() >= 1
                    }
                };

                if should_analyze {
                    if let Err(e) = Self::analyze_failure_patterns(
                        &failure_history,
                        &detected_patterns,
                        &pattern_analysis_state,
                        &stats,
                        &config,
                    ).await {
                        error!("Failed to analyze failure patterns: {}", e);
                    }
                }
            }
        });
    }

    /// Analyze failure patterns
    async fn analyze_failure_patterns(
        failure_history: &Arc<RwLock<Vec<JobFailure>>>,
        detected_patterns: &Arc<RwLock<Vec<FailurePattern>>>,
        pattern_analysis_state: &Arc<Mutex<PatternAnalysisState>>,
        stats: &Arc<RwLock<FailureRecoveryStats>>,
        config: &FailureRecoveryConfig,
    ) -> Result<()> {
        // Mark analysis as in progress
        {
            let mut state = pattern_analysis_state.lock().await;
            state.analysis_in_progress = true;
        }

        debug!("Starting failure pattern analysis");

        // Get recent failures
        let recent_failures = {
            let history = failure_history.read().await;
            let cutoff = Utc::now() - ChronoDuration::hours(config.pattern_analysis_window as i64);
            history
                .iter()
                .filter(|f| f.failed_at > cutoff)
                .cloned()
                .collect::<Vec<_>>()
        };

        if recent_failures.len() < config.min_failures_for_pattern as usize {
            debug!("Not enough failures for pattern analysis");
            
            // Mark analysis as complete
            {
                let mut state = pattern_analysis_state.lock().await;
                state.analysis_in_progress = false;
                state.last_analysis = Utc::now();
            }
            
            return Ok(());
        }

        // Analyze for different pattern types
        let mut new_patterns = Vec::new();

        // Node-specific patterns
        new_patterns.extend(Self::detect_node_specific_patterns(&recent_failures, config));

        // Resource-based patterns
        new_patterns.extend(Self::detect_resource_patterns(&recent_failures, config));

        // Temporal patterns
        new_patterns.extend(Self::detect_temporal_patterns(&recent_failures, config));

        // Store detected patterns
        if !new_patterns.is_empty() {
            let mut patterns = detected_patterns.write().await;
            patterns.extend(new_patterns.clone());
            
            // Keep only recent patterns (last 100)
            if patterns.len() > 100 {
                let excess = patterns.len() - 100;
                patterns.drain(0..excess);
            }

            // Update statistics
            {
                let mut stats_guard = stats.write().await;
                stats_guard.patterns_detected += new_patterns.len() as u32;
            }

            info!("Detected {} new failure patterns", new_patterns.len());
        }

        // Mark analysis as complete
        {
            let mut state = pattern_analysis_state.lock().await;
            state.analysis_in_progress = false;
            state.last_analysis = Utc::now();
        }

        Ok(())
    }

    /// Detect node-specific failure patterns
    fn detect_node_specific_patterns(
        failures: &[JobFailure],
        config: &FailureRecoveryConfig,
    ) -> Vec<FailurePattern> {
        let mut patterns = Vec::new();
        let mut node_failure_counts: HashMap<NodeId, u32> = HashMap::new();

        // Count failures per node
        for failure in failures {
            *node_failure_counts.entry(failure.node_id).or_insert(0) += 1;
        }

        // Detect nodes with excessive failures
        for (node_id, count) in node_failure_counts {
            if count >= config.min_failures_for_pattern {
                let pattern = FailurePattern {
                    id: format!("node-specific-{}", node_id),
                    pattern_type: FailurePatternType::NodeSpecific,
                    affected_nodes: vec![node_id],
                    failure_count: count,
                    detected_at: Utc::now(),
                    confidence: (count as f64 / failures.len() as f64) * 100.0,
                    recommended_actions: vec![
                        RecoveryAction::MarkNodeMaintenance { node_id },
                        RecoveryAction::SendAlert {
                            message: format!("Node {} experiencing repeated failures", node_id),
                            severity: AlertSeverity::High,
                        },
                    ],
                };
                patterns.push(pattern);
            }
        }

        patterns
    }

    /// Detect resource-based failure patterns
    fn detect_resource_patterns(
        failures: &[JobFailure],
        config: &FailureRecoveryConfig,
    ) -> Vec<FailurePattern> {
        let mut patterns = Vec::new();
        
        let resource_failures = failures
            .iter()
            .filter(|f| f.reason == FailureReason::ResourceExhaustion)
            .count();

        if resource_failures >= config.min_failures_for_pattern as usize {
            let affected_nodes: Vec<NodeId> = failures
                .iter()
                .filter(|f| f.reason == FailureReason::ResourceExhaustion)
                .map(|f| f.node_id)
                .collect();

            let pattern = FailurePattern {
                id: "resource-exhaustion".to_string(),
                pattern_type: FailurePatternType::ResourceBased,
                affected_nodes,
                failure_count: resource_failures as u32,
                detected_at: Utc::now(),
                confidence: (resource_failures as f64 / failures.len() as f64) * 100.0,
                recommended_actions: vec![
                    RecoveryAction::ScaleUpResources {
                        resource_type: "compute".to_string(),
                    },
                    RecoveryAction::SendAlert {
                        message: "Resource exhaustion pattern detected".to_string(),
                        severity: AlertSeverity::Medium,
                    },
                ],
            };
            patterns.push(pattern);
        }

        patterns
    }

    /// Detect temporal failure patterns
    fn detect_temporal_patterns(
        failures: &[JobFailure],
        config: &FailureRecoveryConfig,
    ) -> Vec<FailurePattern> {
        let mut patterns = Vec::new();

        // Group failures by hour of day
        let mut hourly_failures: HashMap<u32, u32> = HashMap::new();
        for failure in failures {
            let hour = failure.failed_at.hour();
            *hourly_failures.entry(hour).or_insert(0) += 1;
        }

        // Find hours with excessive failures
        for (hour, count) in hourly_failures {
            if count >= config.min_failures_for_pattern {
                let affected_nodes: Vec<NodeId> = failures
                    .iter()
                    .filter(|f| f.failed_at.hour() == hour)
                    .map(|f| f.node_id)
                    .collect();

                let pattern = FailurePattern {
                    id: format!("temporal-hour-{}", hour),
                    pattern_type: FailurePatternType::Temporal,
                    affected_nodes,
                    failure_count: count,
                    detected_at: Utc::now(),
                    confidence: (count as f64 / failures.len() as f64) * 100.0,
                    recommended_actions: vec![
                        RecoveryAction::AdjustJobPriorities {
                            priority_adjustment: -1,
                        },
                        RecoveryAction::SendAlert {
                            message: format!("Temporal failure pattern detected at hour {}", hour),
                            severity: AlertSeverity::Low,
                        },
                    ],
                };
                patterns.push(pattern);
            }
        }

        patterns
    }

    /// Start cleanup background task
    async fn start_cleanup_task(&self) {
        let recovery_attempts = self.recovery_attempts.clone();
        let node_failure_counters = self.node_failure_counters.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(3600) // Run every hour
            );

            loop {
                interval.tick().await;

                // Clean up old recovery attempts (older than 24 hours)
                // This would typically be based on job completion status
                {
                    let mut attempts = recovery_attempts.write().await;
                    // In a real implementation, we'd check job status and clean up completed jobs
                    if attempts.len() > 1000 {
                        attempts.clear(); // Simple cleanup for now
                    }
                }

                // Reset node failure counters periodically
                {
                    let mut counters = node_failure_counters.write().await;
                    counters.clear();
                }

                debug!("Performed failure recovery cleanup");
            }
        });
    }

    /// Get failure recovery statistics
    pub async fn get_stats(&self) -> FailureRecoveryStats {
        self.stats.read().await.clone()
    }

    /// Get detected failure patterns
    pub async fn get_detected_patterns(&self) -> Vec<FailurePattern> {
        self.detected_patterns.read().await.clone()
    }

    /// Get recent failure history
    pub async fn get_failure_history(&self, limit: Option<usize>) -> Vec<JobFailure> {
        let history = self.failure_history.read().await;
        let limit = limit.unwrap_or(100);
        
        if history.len() <= limit {
            history.clone()
        } else {
            history[history.len() - limit..].to_vec()
        }
    }
}

impl Clone for JobFailureRecovery {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            scheduler: self.scheduler.clone(),
            event_bus: self.event_bus.clone(),
            failure_history: self.failure_history.clone(),
            detected_patterns: self.detected_patterns.clone(),
            recovery_attempts: self.recovery_attempts.clone(),
            node_failure_counters: self.node_failure_counters.clone(),
            stats: self.stats.clone(),
            pattern_analysis_state: self.pattern_analysis_state.clone(),
        }
    }
}

impl Default for FailureRecoveryStats {
    fn default() -> Self {
        Self {
            total_failures: 0,
            total_recovery_attempts: 0,
            successful_recoveries: 0,
            failed_recoveries: 0,
            recovery_success_rate: 100.0,
            failures_by_reason: HashMap::new(),
            failures_by_node: HashMap::new(),
            patterns_detected: 0,
            avg_recovery_time: 0.0,
        }
    }
}

/// Job failure event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobFailureEvent {
    pub job_id: JobId,
    pub node_id: NodeId,
    pub failure_reason: FailureReason,
    pub error_message: String,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: uuid::Uuid,
}

impl DomainEvent for JobFailureEvent {
    fn event_type(&self) -> &'static str {
        "job_failure"
    }

    fn aggregate_id(&self) -> uuid::Uuid {
        self.job_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn correlation_id(&self) -> uuid::Uuid {
        self.correlation_id
    }
}

/// Event handler for job failures
pub struct JobFailureEventHandler {
    failure_recovery: Arc<JobFailureRecovery>,
}

impl JobFailureEventHandler {
    pub fn new(failure_recovery: Arc<JobFailureRecovery>) -> Self {
        Self { failure_recovery }
    }
}

#[async_trait]
impl EventHandler<JobFailureEvent> for JobFailureEventHandler {
    async fn handle(&self, event: &JobFailureEvent) -> Result<()> {
        if let Err(e) = self.failure_recovery
            .handle_job_failure(
                event.job_id,
                event.node_id,
                event.failure_reason.clone(),
                event.error_message.clone(),
                None,
            )
            .await
        {
            error!("Failed to handle job failure event: {}", e);
        }

        Ok(())
    }

    fn handler_name(&self) -> &'static str {
        "JobFailureEventHandler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_reason_categorization() {
        assert_eq!(FailureReason::NodeUnavailable, FailureReason::NodeUnavailable);
        assert_ne!(FailureReason::NodeUnavailable, FailureReason::ResourceExhaustion);
    }

    #[test]
    fn test_failure_pattern_types() {
        assert_eq!(FailurePatternType::NodeSpecific, FailurePatternType::NodeSpecific);
        assert_ne!(FailurePatternType::NodeSpecific, FailurePatternType::Cascading);
    }

    #[test]
    fn test_alert_severity_levels() {
        assert_eq!(AlertSeverity::Critical, AlertSeverity::Critical);
        assert_ne!(AlertSeverity::Critical, AlertSeverity::Low);
    }

    #[tokio::test]
    async fn test_failure_recovery_config() {
        let config = FailureRecoveryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_recovery_attempts, 3);
        assert_eq!(config.recovery_delay, 60);
    }
}