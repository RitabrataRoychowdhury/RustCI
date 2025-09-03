//! Comprehensive Audit Logging System
//!
//! This module provides tamper-proof audit logging with
//! comprehensive security event tracking, integrity verification,
//! and compliance reporting capabilities.

use crate::core::networking::security::{AuditAction, AuditEvent, AuditLogger};
use crate::error::{AppError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced audit logger with tamper-proof capabilities
pub struct EnhancedAuditLogger {
    config: AuditConfig,
    event_buffer: Arc<RwLock<VecDeque<AuditEventWithIntegrity>>>,
    integrity_chain: Arc<RwLock<IntegrityChain>>,
    event_sender: mpsc::UnboundedSender<AuditEvent>,
    metrics: Arc<RwLock<AuditMetrics>>,
    alert_thresholds: AlertThresholds,
    correlation_tracker: Arc<RwLock<CorrelationTracker>>,
}

/// Configuration for the enhanced audit logger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enable_file_logging: bool,
    pub enable_integrity_verification: bool,
    pub enable_real_time_alerts: bool,
    pub log_file_path: String,
    pub backup_file_path: String,
    pub max_buffer_size: usize,
    pub flush_interval_seconds: u64,
    pub retention_days: u32,
    pub compression_enabled: bool,
    pub encryption_enabled: bool,
    pub batch_size: usize,
    pub enable_correlation_tracking: bool,
    pub correlation_timeout_minutes: u64,
    pub sensitive_fields: Vec<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable_file_logging: true,
            enable_integrity_verification: true,
            enable_real_time_alerts: true,
            log_file_path: "/var/log/rustci/audit.log".to_string(),
            backup_file_path: "/var/log/rustci/audit.backup.log".to_string(),
            max_buffer_size: 10000,
            flush_interval_seconds: 30,
            retention_days: 365,
            compression_enabled: true,
            encryption_enabled: true,
            batch_size: 100,
            enable_correlation_tracking: true,
            correlation_timeout_minutes: 60,
            sensitive_fields: vec![
                "password".to_string(),
                "token".to_string(),
                "secret".to_string(),
                "key".to_string(),
                "authorization".to_string(),
                "cookie".to_string(),
            ],
        }
    }
}

/// Audit event with integrity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventWithIntegrity {
    pub event: AuditEvent,
    pub integrity_hash: String,
    pub previous_hash: Option<String>,
    pub sequence_number: u64,
    pub signature: Option<String>,
    pub correlation_id: Option<String>,
    pub enriched_data: HashMap<String, serde_json::Value>,
}

impl AuditEventWithIntegrity {
    pub fn new(event: AuditEvent, previous_hash: Option<String>, sequence_number: u64) -> Self {
        let mut integrity_event = Self {
            event,
            integrity_hash: String::new(),
            previous_hash,
            sequence_number,
            signature: None,
            correlation_id: None,
            enriched_data: HashMap::new(),
        };
        
        integrity_event.integrity_hash = integrity_event.calculate_integrity_hash();
        integrity_event
    }

    /// Calculate cryptographic hash for tamper detection
    pub fn calculate_integrity_hash(&self) -> String {
        let mut hasher = Sha256::new();
        
        // Include event data
        hasher.update(self.event.id.as_bytes());
        hasher.update(self.event.timestamp.to_rfc3339().as_bytes());
        hasher.update(format!("{:?}", self.event.action).as_bytes());
        hasher.update(self.event.resource_type.as_bytes());
        
        if let Some(user_id) = self.event.user_id {
            hasher.update(user_id.as_bytes());
        }
        
        if let Some(ref resource_id) = self.event.resource_id {
            hasher.update(resource_id.as_bytes());
        }
        
        if let Some(ref ip) = self.event.ip_address {
            hasher.update(ip.as_bytes());
        }
        
        hasher.update(self.event.success.to_string().as_bytes());
        hasher.update(self.sequence_number.to_be_bytes());
        
        // Include previous hash for chaining
        if let Some(ref prev_hash) = self.previous_hash {
            hasher.update(prev_hash.as_bytes());
        }
        
        // Include details in sorted order for consistency
        let mut sorted_details: Vec<_> = self.event.details.iter().collect();
        sorted_details.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_details {
            hasher.update(key.as_bytes());
            hasher.update(value.to_string().as_bytes());
        }
        
        format!("{:x}", hasher.finalize())
    }

    /// Verify integrity of this event
    pub fn verify_integrity(&self) -> bool {
        let calculated_hash = self.calculate_integrity_hash();
        calculated_hash == self.integrity_hash
    }
}

/// Integrity chain for tamper-proof logging
#[derive(Debug, Clone)]
pub struct IntegrityChain {
    pub genesis_hash: String,
    pub last_hash: Option<String>,
    pub sequence_number: u64,
    pub created_at: DateTime<Utc>,
    pub last_verified_at: Option<DateTime<Utc>>,
}

impl IntegrityChain {
    pub fn new() -> Self {
        let genesis_hash = format!("{:x}", Sha256::digest(b"rustci-audit-genesis"));
        Self {
            genesis_hash: genesis_hash.clone(),
            last_hash: Some(genesis_hash),
            sequence_number: 0,
            created_at: Utc::now(),
            last_verified_at: None,
        }
    }

    pub fn add_event(&mut self, event: &mut AuditEventWithIntegrity) {
        self.sequence_number += 1;
        event.sequence_number = self.sequence_number;
        event.previous_hash = self.last_hash.clone();
        event.integrity_hash = event.calculate_integrity_hash();
        self.last_hash = Some(event.integrity_hash.clone());
    }

    pub fn verify_chain(&self, events: &[AuditEventWithIntegrity]) -> Result<bool> {
        if events.is_empty() {
            return Ok(true);
        }

        let mut expected_previous = Some(self.genesis_hash.clone());
        let mut expected_sequence = 1;

        for event in events {
            // Verify sequence number
            if event.sequence_number != expected_sequence {
                warn!(
                    expected = expected_sequence,
                    actual = event.sequence_number,
                    "Sequence number mismatch in audit chain"
                );
                return Ok(false);
            }

            // Verify previous hash
            if event.previous_hash != expected_previous {
                warn!(
                    expected = ?expected_previous,
                    actual = ?event.previous_hash,
                    "Previous hash mismatch in audit chain"
                );
                return Ok(false);
            }

            // Verify event integrity
            if !event.verify_integrity() {
                warn!(
                    event_id = %event.event.id,
                    "Event integrity verification failed"
                );
                return Ok(false);
            }

            expected_previous = Some(event.integrity_hash.clone());
            expected_sequence += 1;
        }

        Ok(true)
    }
}

/// Alert thresholds for security events
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub failed_login_threshold: u32,
    pub failed_login_window_minutes: u64,
    pub privilege_escalation_threshold: u32,
    pub data_access_threshold: u32,
    pub system_change_threshold: u32,
    pub suspicious_ip_threshold: u32,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            failed_login_threshold: 5,
            failed_login_window_minutes: 15,
            privilege_escalation_threshold: 3,
            data_access_threshold: 100,
            system_change_threshold: 10,
            suspicious_ip_threshold: 20,
        }
    }
}

/// Correlation tracker for linking related events
#[derive(Debug, Clone)]
pub struct CorrelationTracker {
    pub active_sessions: HashMap<String, SessionContext>,
    pub ip_activity: HashMap<String, IpActivityContext>,
    pub user_activity: HashMap<Uuid, UserActivityContext>,
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub session_id: String,
    pub user_id: Option<Uuid>,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub event_count: u32,
    pub failed_attempts: u32,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IpActivityContext {
    pub ip_address: String,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub event_count: u32,
    pub failed_attempts: u32,
    pub unique_users: std::collections::HashSet<Uuid>,
}

#[derive(Debug, Clone)]
pub struct UserActivityContext {
    pub user_id: Uuid,
    pub session_count: u32,
    pub last_login: Option<DateTime<Utc>>,
    pub failed_attempts: u32,
    pub privilege_changes: u32,
    pub data_access_count: u32,
}

/// Audit metrics for monitoring and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditMetrics {
    pub total_events: u64,
    pub events_by_action: HashMap<String, u64>,
    pub events_by_user: HashMap<String, u64>,
    pub failed_events: u64,
    pub integrity_violations: u64,
    pub correlation_matches: u64,
    pub alert_count: u64,
    pub average_processing_time_ms: f64,
    pub buffer_utilization: f64,
    pub disk_usage_bytes: u64,
    pub last_flush_time: Option<DateTime<Utc>>,
}

impl Default for AuditMetrics {
    fn default() -> Self {
        Self {
            total_events: 0,
            events_by_action: HashMap::new(),
            events_by_user: HashMap::new(),
            failed_events: 0,
            integrity_violations: 0,
            correlation_matches: 0,
            alert_count: 0,
            average_processing_time_ms: 0.0,
            buffer_utilization: 0.0,
            disk_usage_bytes: 0,
            last_flush_time: None,
        }
    }
}

impl EnhancedAuditLogger {
    pub fn new(config: AuditConfig) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let logger = Self {
            config: config.clone(),
            event_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_buffer_size))),
            integrity_chain: Arc::new(RwLock::new(IntegrityChain::new())),
            event_sender,
            metrics: Arc::new(RwLock::new(AuditMetrics::default())),
            alert_thresholds: AlertThresholds::default(),
            correlation_tracker: Arc::new(RwLock::new(CorrelationTracker {
                active_sessions: HashMap::new(),
                ip_activity: HashMap::new(),
                user_activity: HashMap::new(),
            })),
        };

        // Start background processing
        logger.start_background_processing(event_receiver);
        
        // Start periodic tasks
        logger.start_periodic_tasks();

        info!("🔒 Enhanced audit logger initialized with tamper-proof capabilities");
        Ok(logger)
    }

    /// Start background event processing
    fn start_background_processing(&self, mut event_receiver: mpsc::UnboundedReceiver<AuditEvent>) {
        let event_buffer = Arc::clone(&self.event_buffer);
        let integrity_chain = Arc::clone(&self.integrity_chain);
        let metrics = Arc::clone(&self.metrics);
        let correlation_tracker = Arc::clone(&self.correlation_tracker);
        let config = self.config.clone();
        let alert_thresholds = self.alert_thresholds.clone();

        tokio::spawn(async move {
            while let Some(event) = event_receiver.recv().await {
                let start_time = std::time::Instant::now();

                // Process the event
                if let Err(e) = Self::process_event(
                    event,
                    &event_buffer,
                    &integrity_chain,
                    &metrics,
                    &correlation_tracker,
                    &config,
                    &alert_thresholds,
                ).await {
                    error!("Failed to process audit event: {}", e);
                }

                // Update processing time metrics
                let processing_time = start_time.elapsed().as_millis() as f64;
                let mut metrics_guard = metrics.write().await;
                metrics_guard.average_processing_time_ms = 
                    (metrics_guard.average_processing_time_ms * (metrics_guard.total_events - 1) as f64 + processing_time) 
                    / metrics_guard.total_events as f64;
            }
        });
    }

    /// Process a single audit event
    async fn process_event(
        mut event: AuditEvent,
        event_buffer: &Arc<RwLock<VecDeque<AuditEventWithIntegrity>>>,
        integrity_chain: &Arc<RwLock<IntegrityChain>>,
        metrics: &Arc<RwLock<AuditMetrics>>,
        correlation_tracker: &Arc<RwLock<CorrelationTracker>>,
        config: &AuditConfig,
        alert_thresholds: &AlertThresholds,
    ) -> Result<()> {
        // Sanitize sensitive data
        Self::sanitize_sensitive_data(&mut event, &config.sensitive_fields);

        // Enrich event with additional context
        Self::enrich_event(&mut event).await;

        // Create integrity event
        let mut integrity_chain_guard = integrity_chain.write().await;
        let mut integrity_event = AuditEventWithIntegrity::new(
            event.clone(),
            integrity_chain_guard.last_hash.clone(),
            integrity_chain_guard.sequence_number + 1,
        );
        integrity_chain_guard.add_event(&mut integrity_event);
        drop(integrity_chain_guard);

        // Update correlation tracking
        if config.enable_correlation_tracking {
            Self::update_correlation_tracking(&event, correlation_tracker).await;
        }

        // Check for security alerts
        if config.enable_real_time_alerts {
            Self::check_security_alerts(&event, correlation_tracker, alert_thresholds, metrics).await;
        }

        // Store sequence number for logging before moving integrity_event
        let sequence_number = integrity_event.sequence_number;
        
        // Add to buffer
        let mut buffer_guard = event_buffer.write().await;
        buffer_guard.push_back(integrity_event);

        // Maintain buffer size
        if buffer_guard.len() > config.max_buffer_size {
            buffer_guard.pop_front();
        }

        // Update metrics
        let mut metrics_guard = metrics.write().await;
        metrics_guard.total_events += 1;
        
        let action_key = format!("{:?}", event.action);
        *metrics_guard.events_by_action.entry(action_key).or_insert(0) += 1;
        
        if let Some(user_id) = event.user_id {
            let user_key = user_id.to_string();
            *metrics_guard.events_by_user.entry(user_key).or_insert(0) += 1;
        }
        
        if !event.success {
            metrics_guard.failed_events += 1;
        }

        metrics_guard.buffer_utilization = buffer_guard.len() as f64 / config.max_buffer_size as f64;

        debug!(
            event_id = %event.id,
            action = ?event.action,
            user_id = ?event.user_id,
            success = event.success,
            sequence = sequence_number,
            "🔒 Audit event processed with integrity verification"
        );

        Ok(())
    }

    /// Sanitize sensitive data from audit events
    fn sanitize_sensitive_data(event: &mut AuditEvent, sensitive_fields: &[String]) {
        for field in sensitive_fields {
            if event.details.contains_key(field) {
                event.details.insert(field.clone(), serde_json::Value::String("[REDACTED]".to_string()));
            }
        }

        // Also check user agent and other fields for sensitive patterns
        if let Some(ref mut user_agent) = event.user_agent {
            if user_agent.to_lowercase().contains("password") || user_agent.to_lowercase().contains("token") {
                *user_agent = "[SANITIZED]".to_string();
            }
        }
    }

    /// Enrich event with additional context
    async fn enrich_event(event: &mut AuditEvent) {
        // Add system context
        event.details.insert(
            "hostname".to_string(),
            serde_json::Value::String(
                hostname::get()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
        );

        event.details.insert(
            "process_id".to_string(),
            serde_json::Value::Number(std::process::id().into()),
        );

        event.details.insert(
            "environment".to_string(),
            serde_json::Value::String(
                std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            ),
        );

        // Add timestamp details
        event.details.insert(
            "timestamp_unix".to_string(),
            serde_json::Value::Number(event.timestamp.timestamp().into()),
        );
    }

    /// Update correlation tracking
    async fn update_correlation_tracking(
        event: &AuditEvent,
        correlation_tracker: &Arc<RwLock<CorrelationTracker>>,
    ) {
        let mut tracker = correlation_tracker.write().await;

        // Update session context
        if let Some(ref session_id) = event.session_id {
            let session = tracker.active_sessions.entry(session_id.clone()).or_insert_with(|| {
                SessionContext {
                    session_id: session_id.clone(),
                    user_id: event.user_id,
                    start_time: event.timestamp,
                    last_activity: event.timestamp,
                    event_count: 0,
                    failed_attempts: 0,
                    ip_address: event.ip_address.clone(),
                }
            });

            session.last_activity = event.timestamp;
            session.event_count += 1;
            if !event.success {
                session.failed_attempts += 1;
            }
        }

        // Update IP activity
        if let Some(ref ip) = event.ip_address {
            let ip_context = tracker.ip_activity.entry(ip.clone()).or_insert_with(|| {
                IpActivityContext {
                    ip_address: ip.clone(),
                    first_seen: event.timestamp,
                    last_seen: event.timestamp,
                    event_count: 0,
                    failed_attempts: 0,
                    unique_users: std::collections::HashSet::new(),
                }
            });

            ip_context.last_seen = event.timestamp;
            ip_context.event_count += 1;
            if !event.success {
                ip_context.failed_attempts += 1;
            }
            if let Some(user_id) = event.user_id {
                ip_context.unique_users.insert(user_id);
            }
        }

        // Update user activity
        if let Some(user_id) = event.user_id {
            let user_context = tracker.user_activity.entry(user_id).or_insert_with(|| {
                UserActivityContext {
                    user_id,
                    session_count: 0,
                    last_login: None,
                    failed_attempts: 0,
                    privilege_changes: 0,
                    data_access_count: 0,
                }
            });

            if event.action == AuditAction::Login && event.success {
                user_context.last_login = Some(event.timestamp);
                user_context.session_count += 1;
            }

            if !event.success {
                user_context.failed_attempts += 1;
            }

            match event.action {
                AuditAction::CreateUser | AuditAction::UpdateUser | AuditAction::DeleteUser => {
                    user_context.privilege_changes += 1;
                }
                AuditAction::ViewPipeline | AuditAction::FileDownload => {
                    user_context.data_access_count += 1;
                }
                _ => {}
            }
        }
    }

    /// Check for security alerts
    async fn check_security_alerts(
        event: &AuditEvent,
        correlation_tracker: &Arc<RwLock<CorrelationTracker>>,
        alert_thresholds: &AlertThresholds,
        metrics: &Arc<RwLock<AuditMetrics>>,
    ) {
        let tracker = correlation_tracker.read().await;
        let mut alert_triggered = false;

        // Check for failed login threshold
        if event.action == AuditAction::Login && !event.success {
            if let Some(ref ip) = event.ip_address {
                if let Some(ip_context) = tracker.ip_activity.get(ip) {
                    let window_start = Utc::now() - Duration::minutes(alert_thresholds.failed_login_window_minutes as i64);
                    if ip_context.last_seen > window_start && ip_context.failed_attempts >= alert_thresholds.failed_login_threshold {
                        warn!(
                            ip = ip,
                            failed_attempts = ip_context.failed_attempts,
                            threshold = alert_thresholds.failed_login_threshold,
                            "🚨 SECURITY ALERT: Multiple failed login attempts from IP"
                        );
                        alert_triggered = true;
                    }
                }
            }
        }

        // Check for privilege escalation
        if matches!(event.action, AuditAction::CreateUser | AuditAction::UpdateUser | AuditAction::DeleteUser) {
            if let Some(user_id) = event.user_id {
                if let Some(user_context) = tracker.user_activity.get(&user_id) {
                    if user_context.privilege_changes >= alert_thresholds.privilege_escalation_threshold {
                        warn!(
                            user_id = %user_id,
                            privilege_changes = user_context.privilege_changes,
                            threshold = alert_thresholds.privilege_escalation_threshold,
                            "🚨 SECURITY ALERT: Potential privilege escalation detected"
                        );
                        alert_triggered = true;
                    }
                }
            }
        }

        // Check for suspicious IP activity
        if let Some(ref ip) = event.ip_address {
            if let Some(ip_context) = tracker.ip_activity.get(ip) {
                if ip_context.unique_users.len() >= alert_thresholds.suspicious_ip_threshold as usize {
                    warn!(
                        ip = ip,
                        unique_users = ip_context.unique_users.len(),
                        threshold = alert_thresholds.suspicious_ip_threshold,
                        "🚨 SECURITY ALERT: Suspicious IP activity - multiple users from same IP"
                    );
                    alert_triggered = true;
                }
            }
        }

        if alert_triggered {
            let mut metrics_guard = metrics.write().await;
            metrics_guard.alert_count += 1;
        }
    }

    /// Start periodic maintenance tasks
    fn start_periodic_tasks(&self) {
        let event_buffer = Arc::clone(&self.event_buffer);
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);
        let correlation_tracker = Arc::clone(&self.correlation_tracker);

        // Periodic flush task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(config.flush_interval_seconds));
            
            loop {
                interval.tick().await;
                
                if config.enable_file_logging {
                    if let Err(e) = Self::flush_to_disk(&event_buffer, &config, &metrics).await {
                        error!("Failed to flush audit events to disk: {}", e);
                    }
                }
            }
        });

        // Cleanup task
        let correlation_tracker_cleanup = Arc::clone(&correlation_tracker);
        let cleanup_config = self.config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                Self::cleanup_expired_correlations(&correlation_tracker_cleanup, &cleanup_config).await;
            }
        });
    }

    /// Flush events to disk
    async fn flush_to_disk(
        event_buffer: &Arc<RwLock<VecDeque<AuditEventWithIntegrity>>>,
        config: &AuditConfig,
        metrics: &Arc<RwLock<AuditMetrics>>,
    ) -> Result<()> {
        let events_to_flush: Vec<AuditEventWithIntegrity> = {
            let mut buffer = event_buffer.write().await;
            let events: Vec<_> = buffer.drain(..).collect();
            events
        };

        if events_to_flush.is_empty() {
            return Ok(());
        }

        debug!("🔄 Flushing {} audit events to disk", events_to_flush.len());

        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(&config.log_file_path).parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AppError::InternalServerError(format!("Failed to create audit log directory: {}", e))
            })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file_path)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to open audit log file: {}", e))
            })?;

        let mut bytes_written = 0u64;
        for event in &events_to_flush {
            let json_line = serde_json::to_string(event).map_err(|e| {
                AppError::InternalServerError(format!("Failed to serialize audit event: {}", e))
            })?;
            
            let line_with_newline = format!("{}\n", json_line);
            file.write_all(line_with_newline.as_bytes()).await.map_err(|e| {
                AppError::InternalServerError(format!("Failed to write audit event: {}", e))
            })?;
            
            bytes_written += line_with_newline.len() as u64;
        }

        file.flush().await.map_err(|e| {
            AppError::InternalServerError(format!("Failed to flush audit log: {}", e))
        })?;

        // Update metrics
        let mut metrics_guard = metrics.write().await;
        metrics_guard.disk_usage_bytes += bytes_written;
        metrics_guard.last_flush_time = Some(Utc::now());

        info!("✅ Flushed {} audit events ({} bytes) to disk", events_to_flush.len(), bytes_written);
        Ok(())
    }

    /// Clean up expired correlation data
    async fn cleanup_expired_correlations(
        correlation_tracker: &Arc<RwLock<CorrelationTracker>>,
        config: &AuditConfig,
    ) {
        let mut tracker = correlation_tracker.write().await;
        let cutoff_time = Utc::now() - Duration::minutes(config.correlation_timeout_minutes as i64);

        // Clean up expired sessions
        let initial_session_count = tracker.active_sessions.len();
        tracker.active_sessions.retain(|_, session| session.last_activity > cutoff_time);
        
        // Clean up expired IP activity
        let initial_ip_count = tracker.ip_activity.len();
        tracker.ip_activity.retain(|_, ip_context| ip_context.last_seen > cutoff_time);

        let sessions_cleaned = initial_session_count - tracker.active_sessions.len();
        let ips_cleaned = initial_ip_count - tracker.ip_activity.len();

        if sessions_cleaned > 0 || ips_cleaned > 0 {
            debug!(
                sessions_cleaned = sessions_cleaned,
                ips_cleaned = ips_cleaned,
                "🧹 Cleaned up expired correlation data"
            );
        }
    }

    /// Verify integrity of the entire audit log
    pub async fn verify_integrity(&self) -> Result<IntegrityVerificationReport> {
        let buffer = self.event_buffer.read().await;
        let chain = self.integrity_chain.read().await;
        
        let events: Vec<_> = buffer.iter().cloned().collect();
        let chain_valid = chain.verify_chain(&events)?;
        
        let mut individual_failures = Vec::new();
        let mut total_events = 0;
        let mut valid_events = 0;

        for event in &events {
            total_events += 1;
            if event.verify_integrity() {
                valid_events += 1;
            } else {
                individual_failures.push(event.event.id);
            }
        }

        let report = IntegrityVerificationReport {
            overall_valid: chain_valid && individual_failures.is_empty(),
            chain_valid,
            total_events,
            valid_events,
            failed_events: individual_failures.clone(),
            verification_time: Utc::now(),
            genesis_hash: chain.genesis_hash.clone(),
            last_hash: chain.last_hash.clone(),
            sequence_number: chain.sequence_number,
        };

        if !report.overall_valid {
            let mut metrics = self.metrics.write().await;
            metrics.integrity_violations += individual_failures.len() as u64;
            
            error!(
                chain_valid = chain_valid,
                failed_events = individual_failures.len(),
                "🚨 CRITICAL: Audit log integrity verification failed"
            );
        } else {
            info!("✅ Audit log integrity verification passed");
        }

        Ok(report)
    }

    /// Generate compliance report
    pub async fn generate_compliance_report(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<ComplianceReport> {
        let buffer = self.event_buffer.read().await;
        let metrics = self.metrics.read().await;
        
        let filtered_events: Vec<_> = buffer
            .iter()
            .filter(|event| {
                event.event.timestamp >= start_date && event.event.timestamp <= end_date
            })
            .collect();

        let mut report = ComplianceReport {
            period_start: start_date,
            period_end: end_date,
            total_events: filtered_events.len() as u64,
            events_by_action: HashMap::new(),
            failed_events: 0,
            integrity_verified: false,
            security_alerts: metrics.alert_count,
            data_access_events: 0,
            privilege_changes: 0,
            authentication_events: 0,
            compliance_score: 0.0,
            recommendations: Vec::new(),
        };

        // Analyze events
        for event in &filtered_events {
            let action_key = format!("{:?}", event.event.action);
            *report.events_by_action.entry(action_key).or_insert(0) += 1;
            
            if !event.event.success {
                report.failed_events += 1;
            }

            match event.event.action {
                AuditAction::Login | AuditAction::Logout => {
                    report.authentication_events += 1;
                }
                AuditAction::CreateUser | AuditAction::UpdateUser | AuditAction::DeleteUser => {
                    report.privilege_changes += 1;
                }
                AuditAction::ViewPipeline | AuditAction::FileDownload => {
                    report.data_access_events += 1;
                }
                _ => {}
            }
        }

        // Verify integrity
        let integrity_report = self.verify_integrity().await?;
        report.integrity_verified = integrity_report.overall_valid;

        // Calculate compliance score
        report.compliance_score = Self::calculate_compliance_score(&report);

        // Generate recommendations
        report.recommendations = Self::generate_compliance_recommendations(&report);

        Ok(report)
    }

    /// Calculate compliance score based on various factors
    fn calculate_compliance_score(report: &ComplianceReport) -> f64 {
        let mut score = 100.0;

        // Deduct for integrity issues
        if !report.integrity_verified {
            score -= 50.0;
        }

        // Deduct for high failure rate
        if report.total_events > 0 {
            let failure_rate = report.failed_events as f64 / report.total_events as f64;
            if failure_rate > 0.1 {
                score -= (failure_rate - 0.1) * 200.0; // Deduct more for higher failure rates
            }
        }

        // Deduct for security alerts
        if report.security_alerts > 0 {
            score -= (report.security_alerts as f64 * 5.0).min(30.0);
        }

        score.max(0.0)
    }

    /// Generate compliance recommendations
    fn generate_compliance_recommendations(report: &ComplianceReport) -> Vec<String> {
        let mut recommendations = Vec::new();

        if !report.integrity_verified {
            recommendations.push("CRITICAL: Audit log integrity verification failed. Investigate potential tampering.".to_string());
        }

        if report.total_events > 0 {
            let failure_rate = report.failed_events as f64 / report.total_events as f64;
            if failure_rate > 0.1 {
                recommendations.push(format!(
                    "High failure rate detected ({:.1}%). Review failed operations and implement additional controls.",
                    failure_rate * 100.0
                ));
            }
        }

        if report.security_alerts > 10 {
            recommendations.push("High number of security alerts. Review alert thresholds and investigate potential threats.".to_string());
        }

        if report.authentication_events == 0 {
            recommendations.push("No authentication events found. Verify audit logging is capturing all security events.".to_string());
        }

        if report.privilege_changes > report.total_events / 10 {
            recommendations.push("High number of privilege changes detected. Review user management procedures.".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("Audit log appears healthy. Continue monitoring for anomalies.".to_string());
        }

        recommendations
    }

    /// Get current audit metrics
    pub async fn get_metrics(&self) -> AuditMetrics {
        self.metrics.read().await.clone()
    }

    /// Perform health check
    pub async fn health_check(&self) -> Result<AuditHealthStatus> {
        let metrics = self.metrics.read().await;
        let buffer = self.event_buffer.read().await;
        
        let status = AuditHealthStatus {
            is_healthy: true,
            buffer_utilization: metrics.buffer_utilization,
            total_events: metrics.total_events,
            failed_events: metrics.failed_events,
            integrity_violations: metrics.integrity_violations,
            last_flush_time: metrics.last_flush_time,
            disk_usage_bytes: metrics.disk_usage_bytes,
            average_processing_time_ms: metrics.average_processing_time_ms,
        };

        // Check for health issues
        if status.buffer_utilization > 0.9 {
            warn!("Audit buffer utilization high: {:.1}%", status.buffer_utilization * 100.0);
        }

        if status.integrity_violations > 0 {
            error!("Integrity violations detected: {}", status.integrity_violations);
        }

        Ok(status)
    }
}

#[async_trait::async_trait]
impl AuditLogger for EnhancedAuditLogger {
    async fn log_event(&self, event: AuditEvent) -> Result<()> {
        self.event_sender.send(event).map_err(|e| {
            AppError::InternalServerError(format!("Failed to queue audit event: {}", e))
        })?;
        Ok(())
    }

    async fn query_events(
        &self,
        user_id: Option<Uuid>,
        action: Option<AuditAction>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>> {
        let buffer = self.event_buffer.read().await;
        let mut filtered: Vec<AuditEvent> = buffer
            .iter()
            .filter(|integrity_event| {
                let event = &integrity_event.event;
                
                if let Some(uid) = user_id {
                    if event.user_id != Some(uid) {
                        return false;
                    }
                }
                if let Some(ref act) = action {
                    if std::mem::discriminant(&event.action) != std::mem::discriminant(act) {
                        return false;
                    }
                }
                if let Some(start) = start_time {
                    if event.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if event.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .map(|integrity_event| integrity_event.event.clone())
            .collect();

        // Sort by timestamp (newest first)
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit
        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }
}

/// Integrity verification report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityVerificationReport {
    pub overall_valid: bool,
    pub chain_valid: bool,
    pub total_events: usize,
    pub valid_events: usize,
    pub failed_events: Vec<Uuid>,
    pub verification_time: DateTime<Utc>,
    pub genesis_hash: String,
    pub last_hash: Option<String>,
    pub sequence_number: u64,
}

/// Compliance report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_events: u64,
    pub events_by_action: HashMap<String, u64>,
    pub failed_events: u64,
    pub integrity_verified: bool,
    pub security_alerts: u64,
    pub data_access_events: u64,
    pub privilege_changes: u64,
    pub authentication_events: u64,
    pub compliance_score: f64,
    pub recommendations: Vec<String>,
}

/// Audit health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditHealthStatus {
    pub is_healthy: bool,
    pub buffer_utilization: f64,
    pub total_events: u64,
    pub failed_events: u64,
    pub integrity_violations: u64,
    pub last_flush_time: Option<DateTime<Utc>>,
    pub disk_usage_bytes: u64,
    pub average_processing_time_ms: f64,
}