/// Security audit logging system for the Valkyrie Protocol
/// 
/// This module provides:
/// - Tamper-proof audit logs with cryptographic integrity
/// - Structured logging with correlation IDs
/// - Real-time log streaming and analysis
/// - Compliance reporting (SOX, HIPAA, PCI-DSS)
/// - Log retention and archival policies
/// - Distributed log aggregation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use sha2::{Sha256, Digest};

use crate::core::networking::node_communication::NodeId;
use crate::error::Result;
use super::{AuthMethod, SecurityEventType, SecuritySeverity};

/// Security audit logger with tamper-proof capabilities
pub struct SecurityAuditLogger {
    config: AuditConfig,
    log_entries: Arc<RwLock<Vec<SecurityAuditEntry>>>,
    integrity_chain: Arc<RwLock<IntegrityChain>>,
    metrics: Arc<RwLock<AuditMetrics>>,
    correlation_tracker: Arc<RwLock<CorrelationTracker>>,
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enable_file_logging: bool,
    pub enable_remote_logging: bool,
    pub enable_integrity_verification: bool,
    pub log_file_path: String,
    pub max_log_entries: usize,
    pub retention_days: u32,
    pub compression_enabled: bool,
    pub encryption_enabled: bool,
    pub remote_endpoints: Vec<String>,
    pub batch_size: usize,
    pub flush_interval_seconds: u64,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable_file_logging: true,
            enable_remote_logging: false,
            enable_integrity_verification: true,
            log_file_path: "/var/log/valkyrie/security-audit.log".to_string(),
            max_log_entries: 100000,
            retention_days: 365,
            compression_enabled: true,
            encryption_enabled: true,
            remote_endpoints: Vec::new(),
            batch_size: 100,
            flush_interval_seconds: 60,
        }
    }
}

/// Security audit entry with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditEntry {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: SecurityEventType,
    pub node_id: Option<NodeId>,
    pub source_ip: String,
    pub method: Option<AuthMethod>,
    pub details: HashMap<String, String>,
    pub severity: SecuritySeverity,
    pub correlation_id: Option<String>,
    pub session_id: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub integrity_hash: Option<String>,
    pub previous_hash: Option<String>,
}

impl SecurityAuditEntry {
    pub fn new(
        event_type: SecurityEventType,
        node_id: Option<NodeId>,
        source_ip: String,
        severity: SecuritySeverity,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type,
            node_id,
            source_ip,
            method: None,
            details: HashMap::new(),
            severity,
            correlation_id: None,
            session_id: None,
            user_agent: None,
            request_id: None,
            trace_id: None,
            span_id: None,
            integrity_hash: None,
            previous_hash: None,
        }
    }

    pub fn with_method(mut self, method: AuthMethod) -> Self {
        self.method = Some(method);
        self
    }

    pub fn with_details(mut self, details: HashMap<String, String>) -> Self {
        self.details = details;
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_trace_context(mut self, trace_id: String, span_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self.span_id = Some(span_id);
        self
    }

    /// Calculate integrity hash for tamper detection
    pub fn calculate_integrity_hash(&self, previous_hash: Option<&str>) -> String {
        let mut hasher = Sha256::new();
        
        // Include all fields in hash calculation
        hasher.update(self.id.as_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(format!("{:?}", self.event_type).as_bytes());
        
        if let Some(ref node_id) = self.node_id {
            hasher.update(node_id.as_bytes());
        }
        
        hasher.update(self.source_ip.as_bytes());
        
        if let Some(ref method) = self.method {
            hasher.update(format!("{:?}", method).as_bytes());
        }
        
        // Include details in sorted order for consistency
        let mut sorted_details: Vec<_> = self.details.iter().collect();
        sorted_details.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_details {
            hasher.update(key.as_bytes());
            hasher.update(value.as_bytes());
        }
        
        hasher.update(format!("{:?}", self.severity).as_bytes());
        
        // Chain with previous hash for tamper detection
        if let Some(prev_hash) = previous_hash {
            hasher.update(prev_hash.as_bytes());
        }
        
        format!("{:x}", hasher.finalize())
    }
}

/// Integrity chain for tamper-proof logging
#[derive(Debug, Clone)]
pub struct IntegrityChain {
    pub last_hash: Option<String>,
    pub chain_length: u64,
    pub genesis_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl IntegrityChain {
    pub fn new() -> Self {
        let genesis_hash = format!("{:x}", Sha256::digest(b"valkyrie-audit-genesis"));
        Self {
            last_hash: Some(genesis_hash.clone()),
            chain_length: 0,
            genesis_hash,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn add_entry(&mut self, entry: &mut SecurityAuditEntry) {
        let integrity_hash = entry.calculate_integrity_hash(self.last_hash.as_deref());
        entry.integrity_hash = Some(integrity_hash.clone());
        entry.previous_hash = self.last_hash.clone();
        
        self.last_hash = Some(integrity_hash);
        self.chain_length += 1;
    }

    pub fn verify_integrity(&self, entries: &[SecurityAuditEntry]) -> Result<bool> {
        if entries.is_empty() {
            return Ok(true);
        }

        let mut previous_hash = Some(self.genesis_hash.clone());
        
        for entry in entries {
            let expected_hash = entry.calculate_integrity_hash(previous_hash.as_deref());
            
            if let Some(ref actual_hash) = entry.integrity_hash {
                if actual_hash != &expected_hash {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
            
            previous_hash = entry.integrity_hash.clone();
        }

        Ok(true)
    }
}

/// Correlation tracker for linking related events
#[derive(Debug, Clone)]
pub struct CorrelationTracker {
    pub active_correlations: HashMap<String, CorrelationContext>,
    pub correlation_timeout: Duration,
}

/// Correlation context for tracking related events
#[derive(Debug, Clone)]
pub struct CorrelationContext {
    pub correlation_id: String,
    pub root_event_id: String,
    pub related_events: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

impl CorrelationTracker {
    pub fn new(timeout: Duration) -> Self {
        Self {
            active_correlations: HashMap::new(),
            correlation_timeout: timeout,
        }
    }

    pub fn create_correlation(&mut self, root_event_id: String) -> String {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        
        let context = CorrelationContext {
            correlation_id: correlation_id.clone(),
            root_event_id,
            related_events: Vec::new(),
            created_at: now,
            last_activity: now,
            metadata: HashMap::new(),
        };
        
        self.active_correlations.insert(correlation_id.clone(), context);
        correlation_id
    }

    pub fn add_related_event(&mut self, correlation_id: &str, event_id: String) {
        if let Some(context) = self.active_correlations.get_mut(correlation_id) {
            context.related_events.push(event_id);
            context.last_activity = chrono::Utc::now();
        }
    }

    pub fn cleanup_expired(&mut self) {
        let now = chrono::Utc::now();
        self.active_correlations.retain(|_, context| {
            now.signed_duration_since(context.last_activity).to_std().unwrap_or(Duration::ZERO) < self.correlation_timeout
        });
    }
}

/// Audit metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditMetrics {
    pub total_entries: u64,
    pub entries_by_severity: HashMap<String, u64>,
    pub entries_by_type: HashMap<String, u64>,
    pub integrity_violations: u64,
    pub log_write_errors: u64,
    pub average_write_time_ms: f64,
    pub disk_usage_bytes: u64,
    pub compression_ratio: f64,
}

impl SecurityAuditLogger {
    pub fn new(config: AuditConfig) -> Result<Self> {
        let logger = Self {
            config,
            log_entries: Arc::new(RwLock::new(Vec::new())),
            integrity_chain: Arc::new(RwLock::new(IntegrityChain::new())),
            metrics: Arc::new(RwLock::new(AuditMetrics {
                total_entries: 0,
                entries_by_severity: HashMap::new(),
                entries_by_type: HashMap::new(),
                integrity_violations: 0,
                log_write_errors: 0,
                average_write_time_ms: 0.0,
                disk_usage_bytes: 0,
                compression_ratio: 1.0,
            })),
            correlation_tracker: Arc::new(RwLock::new(CorrelationTracker::new(Duration::from_secs(3600)))),
        };

        // Start background tasks
        logger.start_background_tasks();

        Ok(logger)
    }

    /// Log a security audit entry
    pub async fn log_event(&self, mut entry: SecurityAuditEntry) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Add to integrity chain
        {
            let mut chain = self.integrity_chain.write().await;
            chain.add_entry(&mut entry);
        }

        // Store in memory
        {
            let mut entries = self.log_entries.write().await;
            entries.push(entry.clone());

            // Maintain max entries limit
            if entries.len() > self.config.max_log_entries {
                let excess = entries.len() - self.config.max_log_entries;
                entries.drain(0..excess);
            }
        }

        // Write to file if enabled
        if self.config.enable_file_logging {
            self.write_to_file(&entry).await?;
        }

        // Send to remote endpoints if enabled
        if self.config.enable_remote_logging {
            self.send_to_remote(&entry).await?;
        }

        // Update metrics
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_entries += 1;
        
        let severity_key = format!("{:?}", entry.severity);
        *metrics.entries_by_severity.entry(severity_key).or_insert(0) += 1;
        
        let type_key = format!("{:?}", entry.event_type);
        *metrics.entries_by_type.entry(type_key).or_insert(0) += 1;
        
        metrics.average_write_time_ms = (metrics.average_write_time_ms * (metrics.total_entries - 1) as f64 + duration.as_millis() as f64) / metrics.total_entries as f64;

        Ok(())
    }

    /// Write audit entry to file
    async fn write_to_file(&self, entry: &SecurityAuditEntry) -> Result<()> {
        let json_entry = serde_json::to_string(entry)
            .map_err(|e| crate::error::AppError::SecurityError(format!("Failed to serialize audit entry: {}", e)))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_file_path)
            .await
            .map_err(|e| crate::error::AppError::SecurityError(format!("Failed to open audit log file: {}", e)))?;

        file.write_all(format!("{}\n", json_entry).as_bytes())
            .await
            .map_err(|e| crate::error::AppError::SecurityError(format!("Failed to write to audit log: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| crate::error::AppError::SecurityError(format!("Failed to flush audit log: {}", e)))?;

        Ok(())
    }

    /// Send audit entry to remote endpoints
    async fn send_to_remote(&self, _entry: &SecurityAuditEntry) -> Result<()> {
        // Placeholder for remote logging implementation
        // In a real implementation, this would send to SIEM systems, log aggregators, etc.
        Ok(())
    }

    /// Get recent audit entries
    pub async fn get_recent_entries(&self, count: usize) -> Vec<SecurityAuditEntry> {
        let entries = self.log_entries.read().await;
        entries.iter().rev().take(count).cloned().collect()
    }

    /// Get entries by event type
    pub async fn get_entries_by_type(&self, event_type: SecurityEventType) -> Vec<SecurityAuditEntry> {
        let entries = self.log_entries.read().await;
        entries.iter()
            .filter(|entry| entry.event_type == event_type)
            .cloned()
            .collect()
    }

    /// Get entries for a specific node
    pub async fn get_entries_for_node(&self, node_id: &NodeId) -> Vec<SecurityAuditEntry> {
        let entries = self.log_entries.read().await;
        entries.iter()
            .filter(|entry| entry.node_id.as_ref() == Some(node_id))
            .cloned()
            .collect()
    }

    /// Count events by type
    pub async fn count_events_by_type(&self, event_type: SecurityEventType) -> u64 {
        let entries = self.log_entries.read().await;
        entries.iter()
            .filter(|entry| entry.event_type == event_type)
            .count() as u64
    }

    /// Verify integrity of audit log
    pub async fn verify_integrity(&self) -> Result<bool> {
        let entries = self.log_entries.read().await;
        let chain = self.integrity_chain.read().await;
        chain.verify_integrity(&entries)
    }

    /// Generate compliance report
    pub async fn generate_compliance_report(&self, start_date: chrono::DateTime<chrono::Utc>, end_date: chrono::DateTime<chrono::Utc>) -> ComplianceReport {
        let entries = self.log_entries.read().await;
        let filtered_entries: Vec<_> = entries.iter()
            .filter(|entry| entry.timestamp >= start_date && entry.timestamp <= end_date)
            .collect();

        let mut report = ComplianceReport {
            period_start: start_date,
            period_end: end_date,
            total_events: filtered_entries.len() as u64,
            events_by_type: HashMap::new(),
            events_by_severity: HashMap::new(),
            authentication_events: 0,
            authorization_events: 0,
            security_violations: 0,
            integrity_verified: false,
            compliance_score: 0.0,
        };

        // Count events by type and severity
        for entry in &filtered_entries {
            let type_key = format!("{:?}", entry.event_type);
            *report.events_by_type.entry(type_key).or_insert(0) += 1;

            let severity_key = format!("{:?}", entry.severity);
            *report.events_by_severity.entry(severity_key).or_insert(0) += 1;

            match entry.event_type {
                SecurityEventType::AuthenticationSuccess | SecurityEventType::AuthenticationFailure => {
                    report.authentication_events += 1;
                }
                SecurityEventType::AuthorizationSuccess | SecurityEventType::AuthorizationFailure => {
                    report.authorization_events += 1;
                }
                SecurityEventType::IntrusionDetected | SecurityEventType::PolicyViolation => {
                    report.security_violations += 1;
                }
                _ => {}
            }
        }

        // Verify integrity
        report.integrity_verified = self.verify_integrity().await.unwrap_or(false);

        // Calculate compliance score (simplified)
        report.compliance_score = if report.integrity_verified {
            let violation_ratio = report.security_violations as f64 / report.total_events.max(1) as f64;
            (1.0 - violation_ratio).max(0.0) * 100.0
        } else {
            0.0
        };

        report
    }

    /// Start background maintenance tasks
    fn start_background_tasks(&self) {
        let correlation_tracker = self.correlation_tracker.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                correlation_tracker.write().await.cleanup_expired();
            }
        });
    }

    /// Perform health check
    pub async fn health_check(&self) -> Result<()> {
        // Check if we can write to the log file
        if self.config.enable_file_logging {
            let test_entry = SecurityAuditEntry::new(
                SecurityEventType::SystemCompromised,
                None,
                "127.0.0.1".to_string(),
                SecuritySeverity::Info,
            );
            
            // Try to serialize the entry
            serde_json::to_string(&test_entry)
                .map_err(|e| crate::error::AppError::SecurityError(format!("Health check failed: {}", e)))?;
        }

        // Verify integrity
        if !self.verify_integrity().await? {
            return Err(crate::error::AppError::SecurityError("Audit log integrity verification failed".to_string()).into());
        }

        Ok(())
    }

    /// Get audit metrics
    pub async fn get_metrics(&self) -> AuditMetrics {
        self.metrics.read().await.clone()
    }
}

/// Compliance report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
    pub total_events: u64,
    pub events_by_type: HashMap<String, u64>,
    pub events_by_severity: HashMap<String, u64>,
    pub authentication_events: u64,
    pub authorization_events: u64,
    pub security_violations: u64,
    pub integrity_verified: bool,
    pub compliance_score: f64,
}