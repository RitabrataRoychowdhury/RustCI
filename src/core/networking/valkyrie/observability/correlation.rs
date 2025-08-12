// Correlation ID tracking for distributed tracing

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::ObservabilityError;

/// Correlation ID for tracing requests across components
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(Uuid);

impl CorrelationId {
    /// Create a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create correlation ID from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn uuid(&self) -> Uuid {
        self.0
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for CorrelationId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Correlation tracker for managing correlation contexts
pub struct CorrelationTracker {
    /// Active correlation contexts
    contexts: Arc<RwLock<HashMap<CorrelationId, CorrelationContext>>>,
    /// Correlation statistics
    statistics: Arc<RwLock<CorrelationStatistics>>,
}

/// Correlation context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationContext {
    /// Correlation ID
    pub correlation_id: CorrelationId,
    /// Parent correlation ID (for nested operations)
    pub parent_id: Option<CorrelationId>,
    /// Context creation timestamp
    pub created_at: u64,
    /// Last activity timestamp
    pub last_activity: u64,
    /// Operation name or description
    pub operation: String,
    /// User or service identifier
    pub user_id: Option<String>,
    /// Session identifier
    pub session_id: Option<String>,
    /// Request identifier
    pub request_id: Option<String>,
    /// Additional context metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Trace spans associated with this correlation
    pub spans: Vec<TraceSpan>,
    /// Context status
    pub status: CorrelationStatus,
}

/// Correlation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrelationStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
}

/// Trace span for detailed operation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    /// Span identifier
    pub span_id: Uuid,
    /// Parent span ID
    pub parent_span_id: Option<Uuid>,
    /// Operation name
    pub operation_name: String,
    /// Component or service name
    pub component: String,
    /// Span start timestamp
    pub start_time: u64,
    /// Span end timestamp (None if still active)
    pub end_time: Option<u64>,
    /// Span duration in microseconds
    pub duration_us: Option<u64>,
    /// Span status
    pub status: SpanStatus,
    /// Span tags/labels
    pub tags: HashMap<String, String>,
    /// Span logs/events
    pub logs: Vec<SpanLog>,
}

/// Span status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error,
    Timeout,
    Cancelled,
}

/// Span log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    /// Log timestamp
    pub timestamp: u64,
    /// Log level
    pub level: String,
    /// Log message
    pub message: String,
    /// Additional fields
    pub fields: HashMap<String, serde_json::Value>,
}

/// Correlation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationStatistics {
    /// Total correlations created
    pub total_created: u64,
    /// Currently active correlations
    pub active_count: u64,
    /// Completed correlations
    pub completed_count: u64,
    /// Failed correlations
    pub failed_count: u64,
    /// Cancelled correlations
    pub cancelled_count: u64,
    /// Average correlation duration in seconds
    pub average_duration_seconds: f64,
    /// Longest correlation duration in seconds
    pub max_duration_seconds: f64,
    /// Statistics last updated
    pub last_updated: u64,
}

impl Default for CorrelationStatistics {
    fn default() -> Self {
        Self {
            total_created: 0,
            active_count: 0,
            completed_count: 0,
            failed_count: 0,
            cancelled_count: 0,
            average_duration_seconds: 0.0,
            max_duration_seconds: 0.0,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl CorrelationTracker {
    /// Create a new correlation tracker
    pub fn new() -> Self {
        Self {
            contexts: Arc::new(RwLock::new(HashMap::new())),
            statistics: Arc::new(RwLock::new(CorrelationStatistics::default())),
        }
    }

    /// Create a new correlation ID
    pub fn new_correlation_id(&self) -> CorrelationId {
        CorrelationId::new()
    }

    /// Start a new correlation context
    pub async fn start_correlation(
        &self,
        operation: &str,
        parent_id: Option<CorrelationId>,
    ) -> Result<CorrelationId, ObservabilityError> {
        let correlation_id = CorrelationId::new();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        let context = CorrelationContext {
            correlation_id,
            parent_id,
            created_at: timestamp,
            last_activity: timestamp,
            operation: operation.to_string(),
            user_id: None,
            session_id: None,
            request_id: None,
            metadata: HashMap::new(),
            spans: Vec::new(),
            status: CorrelationStatus::Active,
        };

        let mut contexts = self.contexts.write().await;
        contexts.insert(correlation_id, context);

        // Update statistics
        let mut stats = self.statistics.write().await;
        stats.total_created += 1;
        stats.active_count += 1;
        stats.last_updated = timestamp;

        Ok(correlation_id)
    }

    /// End a correlation context
    pub async fn end_correlation(
        &self,
        correlation_id: CorrelationId,
        status: CorrelationStatus,
    ) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        let mut contexts = self.contexts.write().await;
        if let Some(context) = contexts.get_mut(&correlation_id) {
            context.status = status;
            context.last_activity = timestamp;

            // Calculate duration
            let duration_seconds = (timestamp - context.created_at) as f64;

            // Update statistics
            let mut stats = self.statistics.write().await;
            stats.active_count = stats.active_count.saturating_sub(1);
            
            match status {
                CorrelationStatus::Completed => stats.completed_count += 1,
                CorrelationStatus::Failed => stats.failed_count += 1,
                CorrelationStatus::Cancelled => stats.cancelled_count += 1,
                CorrelationStatus::Active => {} // Should not happen
            }

            // Update duration statistics
            if duration_seconds > stats.max_duration_seconds {
                stats.max_duration_seconds = duration_seconds;
            }

            // Recalculate average duration (simplified)
            let total_completed = stats.completed_count + stats.failed_count + stats.cancelled_count;
            if total_completed > 0 {
                stats.average_duration_seconds = 
                    (stats.average_duration_seconds * (total_completed - 1) as f64 + duration_seconds) 
                    / total_completed as f64;
            }

            stats.last_updated = timestamp;
        }

        Ok(())
    }

    /// Get correlation context
    pub async fn get_context(&self, correlation_id: CorrelationId) -> Option<CorrelationContext> {
        let contexts = self.contexts.read().await;
        contexts.get(&correlation_id).cloned()
    }

    /// Update correlation context
    pub async fn update_context(
        &self,
        correlation_id: CorrelationId,
        updater: impl FnOnce(&mut CorrelationContext),
    ) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        let mut contexts = self.contexts.write().await;
        if let Some(context) = contexts.get_mut(&correlation_id) {
            updater(context);
            context.last_activity = timestamp;
        }

        Ok(())
    }

    /// Add metadata to correlation context
    pub async fn add_metadata(
        &self,
        correlation_id: CorrelationId,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), ObservabilityError> {
        self.update_context(correlation_id, |context| {
            context.metadata.insert(key.to_string(), value);
        }).await
    }

    /// Start a new trace span
    pub async fn start_span(
        &self,
        correlation_id: CorrelationId,
        operation_name: &str,
        component: &str,
        parent_span_id: Option<Uuid>,
    ) -> Result<Uuid, ObservabilityError> {
        let span_id = Uuid::new_v4();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        let span = TraceSpan {
            span_id,
            parent_span_id,
            operation_name: operation_name.to_string(),
            component: component.to_string(),
            start_time: timestamp,
            end_time: None,
            duration_us: None,
            status: SpanStatus::Ok,
            tags: HashMap::new(),
            logs: Vec::new(),
        };

        self.update_context(correlation_id, |context| {
            context.spans.push(span);
        }).await?;

        Ok(span_id)
    }

    /// End a trace span
    pub async fn end_span(
        &self,
        correlation_id: CorrelationId,
        span_id: Uuid,
        status: SpanStatus,
    ) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        self.update_context(correlation_id, |context| {
            if let Some(span) = context.spans.iter_mut().find(|s| s.span_id == span_id) {
                span.end_time = Some(timestamp);
                span.status = status;
                if let Some(start_time) = span.start_time.checked_sub(0) {
                    span.duration_us = Some((timestamp - start_time) * 1_000_000);
                }
            }
        }).await
    }

    /// Add span log
    pub async fn add_span_log(
        &self,
        correlation_id: CorrelationId,
        span_id: Uuid,
        level: &str,
        message: &str,
        fields: HashMap<String, serde_json::Value>,
    ) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        let log = SpanLog {
            timestamp,
            level: level.to_string(),
            message: message.to_string(),
            fields,
        };

        self.update_context(correlation_id, |context| {
            if let Some(span) = context.spans.iter_mut().find(|s| s.span_id == span_id) {
                span.logs.push(log);
            }
        }).await
    }

    /// Get all active correlations
    pub async fn get_active_correlations(&self) -> Vec<CorrelationContext> {
        let contexts = self.contexts.read().await;
        contexts.values()
            .filter(|ctx| ctx.status == CorrelationStatus::Active)
            .cloned()
            .collect()
    }

    /// Get correlation statistics
    pub async fn get_statistics(&self) -> CorrelationStatistics {
        let stats = self.statistics.read().await;
        stats.clone()
    }

    /// Clean up old completed correlations
    pub async fn cleanup_old_correlations(&self, max_age_seconds: u64) -> Result<usize, ObservabilityError> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Correlation(format!("Time error: {}", e)))?
            .as_secs();

        let mut contexts = self.contexts.write().await;
        let initial_count = contexts.len();

        contexts.retain(|_, context| {
            // Keep active correlations
            if context.status == CorrelationStatus::Active {
                return true;
            }

            // Remove old completed correlations
            current_time - context.last_activity < max_age_seconds
        });

        let removed_count = initial_count - contexts.len();
        Ok(removed_count)
    }

    /// Export correlation data for external tracing systems
    pub async fn export_traces(&self) -> Vec<CorrelationContext> {
        let contexts = self.contexts.read().await;
        contexts.values().cloned().collect()
    }
}

impl Default for CorrelationTracker {
    fn default() -> Self {
        Self::new()
    }
}