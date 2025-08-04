use crate::core::networking::security::{AuditAction, AuditEvent, AuditLogger};
use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced audit logging system with buffering and filtering
pub struct EnhancedAuditLogger {
    buffer: Arc<RwLock<Vec<AuditEvent>>>,
    config: AuditConfig,
    event_sender: mpsc::UnboundedSender<AuditEvent>,
    filters: Vec<Box<dyn AuditFilter>>,
    enrichers: Vec<Box<dyn AuditEnricher>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub buffer_size: usize,
    pub flush_interval_seconds: u64,
    pub enable_real_time_alerts: bool,
    pub retention_days: u32,
    pub sensitive_fields: Vec<String>,
    pub log_levels: AuditLogLevels,
    pub export_formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogLevels {
    pub authentication: String,
    pub authorization: String,
    pub data_access: String,
    pub system_changes: String,
    pub security_events: String,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            flush_interval_seconds: 60,
            enable_real_time_alerts: true,
            retention_days: 90,
            sensitive_fields: vec![
                "password".to_string(),
                "token".to_string(),
                "secret".to_string(),
                "key".to_string(),
                "authorization".to_string(),
            ],
            log_levels: AuditLogLevels {
                authentication: "info".to_string(),
                authorization: "warn".to_string(),
                data_access: "debug".to_string(),
                system_changes: "info".to_string(),
                security_events: "error".to_string(),
            },
            export_formats: vec!["json".to_string()],
        }
    }
}

impl EnhancedAuditLogger {
    pub fn new(config: AuditConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let buffer = Arc::new(RwLock::new(Vec::with_capacity(config.buffer_size)));

        let mut logger = Self {
            buffer: Arc::clone(&buffer),
            config: config.clone(),
            event_sender,
            filters: Vec::new(),
            enrichers: Vec::new(),
        };

        // Add default filters and enrichers
        logger.add_default_filters();
        logger.add_default_enrichers();

        // Start background processing
        logger.start_background_processing(event_receiver);

        logger
    }

    fn add_default_filters(&mut self) {
        self.filters.push(Box::new(SensitiveDataFilter::new(
            self.config.sensitive_fields.clone(),
        )));
        self.filters.push(Box::new(DuplicateEventFilter::new()));
        self.filters.push(Box::new(SecurityEventFilter::new()));
    }

    fn add_default_enrichers(&mut self) {
        self.enrichers.push(Box::new(TimestampEnricher));
        self.enrichers.push(Box::new(ContextEnricher));
        self.enrichers.push(Box::new(GeoLocationEnricher));
    }

    pub fn add_filter(&mut self, filter: Box<dyn AuditFilter>) {
        self.filters.push(filter);
    }

    pub fn add_enricher(&mut self, enricher: Box<dyn AuditEnricher>) {
        self.enrichers.push(enricher);
    }

    fn start_background_processing(&self, mut event_receiver: mpsc::UnboundedReceiver<AuditEvent>) {
        let buffer = Arc::clone(&self.buffer);
        let config = self.config.clone();
        let filters = self
            .filters
            .iter()
            .map(|f| f.clone_box())
            .collect::<Vec<_>>();
        let enrichers = self
            .enrichers
            .iter()
            .map(|e| e.clone_box())
            .collect::<Vec<_>>();

        tokio::spawn(async move {
            let mut flush_interval = tokio::time::interval(std::time::Duration::from_secs(
                config.flush_interval_seconds,
            ));

            loop {
                tokio::select! {
                    // Process incoming events
                    Some(mut event) = event_receiver.recv() => {
                        // Apply enrichers
                        for enricher in &enrichers {
                            if let Err(e) = enricher.enrich(&mut event).await {
                                error!("Failed to enrich audit event: {}", e);
                            }
                        }

                        // Apply filters
                        let mut should_log = true;
                        for filter in &filters {
                            match filter.should_log(&event).await {
                                Ok(false) => {
                                    should_log = false;
                                    break;
                                }
                                Err(e) => {
                                    error!("Audit filter error: {}", e);
                                }
                                _ => {}
                            }
                        }

                        if should_log {
                            let mut buffer_guard = buffer.write().await;
                            buffer_guard.push(event.clone());

                            // Check if buffer is full
                            if buffer_guard.len() >= config.buffer_size {
                                Self::flush_buffer(&mut buffer_guard, &config).await;
                            }

                            // Real-time alerts for critical events
                            if config.enable_real_time_alerts {
                                Self::check_for_alerts(&event).await;
                            }
                        }
                    }

                    // Periodic flush
                    _ = flush_interval.tick() => {
                        let mut buffer_guard = buffer.write().await;
                        if !buffer_guard.is_empty() {
                            Self::flush_buffer(&mut buffer_guard, &config).await;
                        }
                    }
                }
            }
        });
    }

    async fn flush_buffer(buffer: &mut Vec<AuditEvent>, _config: &AuditConfig) {
        if buffer.is_empty() {
            return;
        }

        debug!("🔄 Flushing {} audit events", buffer.len());

        // In a real implementation, you would:
        // 1. Write to persistent storage (database, file, etc.)
        // 2. Send to external SIEM systems
        // 3. Export in configured formats

        for event in buffer.iter() {
            // Log to structured logging system
            match event.action {
                AuditAction::Login | AuditAction::Logout => {
                    info!(
                        event_id = %event.id,
                        user_id = ?event.user_id,
                        action = ?event.action,
                        success = event.success,
                        ip_address = ?event.ip_address,
                        "🔐 Authentication event"
                    );
                }
                AuditAction::CreatePipeline
                | AuditAction::UpdatePipeline
                | AuditAction::DeletePipeline
                | AuditAction::ExecutePipeline => {
                    info!(
                        event_id = %event.id,
                        user_id = ?event.user_id,
                        action = ?event.action,
                        resource_type = event.resource_type,
                        resource_id = ?event.resource_id,
                        success = event.success,
                        "🔧 Pipeline event"
                    );
                }
                AuditAction::CreateUser | AuditAction::UpdateUser | AuditAction::DeleteUser => {
                    warn!(
                        event_id = %event.id,
                        user_id = ?event.user_id,
                        action = ?event.action,
                        resource_id = ?event.resource_id,
                        success = event.success,
                        "👤 User management event"
                    );
                }
                _ => {
                    debug!(
                        event_id = %event.id,
                        user_id = ?event.user_id,
                        action = ?event.action,
                        success = event.success,
                        "📝 General audit event"
                    );
                }
            }
        }

        buffer.clear();
        debug!("✅ Audit buffer flushed");
    }

    async fn check_for_alerts(event: &AuditEvent) {
        // Check for security-critical events that need immediate attention
        let critical_actions = [
            AuditAction::DeleteUser,
            AuditAction::SystemConfiguration,
            AuditAction::RevokeToken,
        ];

        if critical_actions.contains(&event.action) || !event.success {
            warn!(
                event_id = %event.id,
                action = ?event.action,
                user_id = ?event.user_id,
                success = event.success,
                error = ?event.error_message,
                "🚨 Critical security event detected"
            );

            // In a real implementation, you would:
            // 1. Send alerts to security team
            // 2. Trigger automated responses
            // 3. Update threat intelligence systems
        }

        // Check for suspicious patterns
        if let Some(ip) = &event.ip_address {
            // In a real implementation, check for:
            // - Multiple failed attempts from same IP
            // - Unusual geographic locations
            // - Known malicious IPs
            debug!(ip = ip, "Checking IP for suspicious patterns");
        }
    }
}

#[async_trait::async_trait]
impl AuditLogger for EnhancedAuditLogger {
    async fn log_event(&self, event: AuditEvent) -> Result<()> {
        if let Err(e) = self.event_sender.send(event) {
            error!("Failed to send audit event to processing queue: {}", e);
            return Err(AppError::InternalServerError(
                "Failed to log audit event".to_string(),
            ));
        }
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
        let buffer = self.buffer.read().await;
        let mut filtered: Vec<AuditEvent> = buffer
            .iter()
            .filter(|event| {
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
            .cloned()
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

/// Trait for audit event filters
#[async_trait::async_trait]
pub trait AuditFilter: Send + Sync {
    async fn should_log(&self, event: &AuditEvent) -> Result<bool>;
    fn clone_box(&self) -> Box<dyn AuditFilter>;
}

/// Trait for audit event enrichers
#[async_trait::async_trait]
pub trait AuditEnricher: Send + Sync {
    async fn enrich(&self, event: &mut AuditEvent) -> Result<()>;
    fn clone_box(&self) -> Box<dyn AuditEnricher>;
}

/// Filter to remove sensitive data from audit logs
pub struct SensitiveDataFilter {
    sensitive_fields: Vec<String>,
}

impl SensitiveDataFilter {
    pub fn new(sensitive_fields: Vec<String>) -> Self {
        Self { sensitive_fields }
    }
}

#[async_trait::async_trait]
impl AuditFilter for SensitiveDataFilter {
    async fn should_log(&self, event: &AuditEvent) -> Result<bool> {
        // Always log, but sanitize sensitive data
        // This would modify the event in place in a real implementation
        for field in &self.sensitive_fields {
            if event.details.contains_key(field) {
                debug!("Sanitizing sensitive field: {}", field);
                // In a real implementation, you'd modify the event
            }
        }
        Ok(true)
    }

    fn clone_box(&self) -> Box<dyn AuditFilter> {
        Box::new(Self {
            sensitive_fields: self.sensitive_fields.clone(),
        })
    }
}

/// Filter to prevent duplicate events
pub struct DuplicateEventFilter {
    recent_events: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl Default for DuplicateEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl DuplicateEventFilter {
    pub fn new() -> Self {
        Self {
            recent_events: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn event_key(event: &AuditEvent) -> String {
        format!(
            "{}:{:?}:{}:{}",
            event.user_id.map(|u| u.to_string()).unwrap_or_default(),
            event.action,
            event.resource_type,
            event.resource_id.as_deref().unwrap_or_default()
        )
    }
}

#[async_trait::async_trait]
impl AuditFilter for DuplicateEventFilter {
    async fn should_log(&self, event: &AuditEvent) -> Result<bool> {
        let key = Self::event_key(event);
        let mut recent = self.recent_events.write().await;

        // Check if we've seen this event recently (within 5 minutes)
        if let Some(last_seen) = recent.get(&key) {
            let time_diff = event.timestamp.signed_duration_since(*last_seen);
            if time_diff.num_minutes() < 5 {
                debug!("Filtering duplicate audit event: {}", key);
                return Ok(false);
            }
        }

        recent.insert(key, event.timestamp);

        // Clean up old entries
        let cutoff = Utc::now() - chrono::Duration::minutes(10);
        recent.retain(|_, timestamp| *timestamp > cutoff);

        Ok(true)
    }

    fn clone_box(&self) -> Box<dyn AuditFilter> {
        Box::new(Self {
            recent_events: Arc::clone(&self.recent_events),
        })
    }
}

/// Filter for security-critical events
pub struct SecurityEventFilter;

impl Default for SecurityEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityEventFilter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl AuditFilter for SecurityEventFilter {
    async fn should_log(&self, event: &AuditEvent) -> Result<bool> {
        // Always log security events, but add additional context
        let security_actions = [
            AuditAction::Login,
            AuditAction::Logout,
            AuditAction::RevokeToken,
            AuditAction::ChangePassword,
            AuditAction::SystemConfiguration,
        ];

        if security_actions.contains(&event.action) || !event.success {
            debug!("Security event detected: {:?}", event.action);
        }

        Ok(true)
    }

    fn clone_box(&self) -> Box<dyn AuditFilter> {
        Box::new(Self)
    }
}

/// Enricher to add timestamp information
pub struct TimestampEnricher;

#[async_trait::async_trait]
impl AuditEnricher for TimestampEnricher {
    async fn enrich(&self, event: &mut AuditEvent) -> Result<()> {
        // Add additional timestamp details
        event.details.insert(
            "timestamp_iso".to_string(),
            serde_json::Value::String(event.timestamp.to_rfc3339()),
        );
        event.details.insert(
            "timestamp_unix".to_string(),
            serde_json::Value::Number(event.timestamp.timestamp().into()),
        );
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn AuditEnricher> {
        Box::new(Self)
    }
}

/// Enricher to add context information
pub struct ContextEnricher;

#[async_trait::async_trait]
impl AuditEnricher for ContextEnricher {
    async fn enrich(&self, event: &mut AuditEvent) -> Result<()> {
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

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn AuditEnricher> {
        Box::new(Self)
    }
}

/// Enricher to add geolocation information
pub struct GeoLocationEnricher;

#[async_trait::async_trait]
impl AuditEnricher for GeoLocationEnricher {
    async fn enrich(&self, event: &mut AuditEvent) -> Result<()> {
        if let Some(ip) = &event.ip_address {
            // In a real implementation, you would:
            // 1. Query a geolocation service
            // 2. Add country, city, ISP information
            // 3. Check against threat intelligence feeds

            debug!("Would enrich event with geolocation for IP: {}", ip);

            // Mock geolocation data
            if ip != "127.0.0.1" && ip != "localhost" {
                event.details.insert(
                    "geo_country".to_string(),
                    serde_json::Value::String("Unknown".to_string()),
                );
                event.details.insert(
                    "geo_city".to_string(),
                    serde_json::Value::String("Unknown".to_string()),
                );
            }
        }
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn AuditEnricher> {
        Box::new(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::networking::security::AuditEvent;

    #[tokio::test]
    async fn test_enhanced_audit_logger() {
        let config = AuditConfig::default();
        let logger = EnhancedAuditLogger::new(config);

        let event = AuditEvent::new(
            AuditAction::Login,
            "user".to_string(),
            Some(Uuid::new_v4()),
            Some("session123".to_string()),
        );

        assert!(logger.log_event(event).await.is_ok());
    }

    #[tokio::test]
    async fn test_sensitive_data_filter() {
        let filter = SensitiveDataFilter::new(vec!["password".to_string()]);
        let event = AuditEvent::new(
            AuditAction::Login,
            "user".to_string(),
            Some(Uuid::new_v4()),
            None,
        );

        assert!(filter.should_log(&event).await.unwrap());
    }

    #[tokio::test]
    async fn test_duplicate_event_filter() {
        let filter = DuplicateEventFilter::new();
        let event = AuditEvent::new(
            AuditAction::Login,
            "user".to_string(),
            Some(Uuid::new_v4()),
            None,
        );

        // First event should be logged
        assert!(filter.should_log(&event).await.unwrap());

        // Duplicate event should be filtered
        assert!(!filter.should_log(&event).await.unwrap());
    }
}
