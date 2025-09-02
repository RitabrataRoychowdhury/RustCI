use async_trait::async_trait;
use std::time::Duration;

use crate::storage::{HealthStatus, OperationMetrics, StoreError, StoreResult, StoreStats};

/// Core storage adapter trait that all backends must implement
#[async_trait]
pub trait StoreAdapter: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &str) -> StoreResult<Option<Vec<u8>>>;
    
    /// Set a value with optional TTL
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> StoreResult<()>;
    
    /// Try to consume tokens from a rate limiter bucket
    /// Returns true if tokens were successfully consumed
    async fn try_consume(&self, key: &str, amount: u32) -> StoreResult<bool>;
    
    /// Delete a key
    async fn delete(&self, key: &str) -> StoreResult<bool>;
    
    /// Check if a key exists
    async fn exists(&self, key: &str) -> StoreResult<bool>;
    
    /// Get TTL for a key
    async fn ttl(&self, key: &str) -> StoreResult<Option<Duration>>;
    
    /// Increment a counter atomically
    async fn increment(&self, key: &str, amount: i64, ttl: Option<Duration>) -> StoreResult<i64>;
    
    /// Health check for the backend
    async fn health_check(&self) -> StoreResult<HealthStatus>;
    
    /// Get backend statistics
    async fn get_stats(&self) -> StoreResult<StoreStats>;
    
    /// Get the backend name/type
    fn backend_name(&self) -> &'static str;
    
    /// Check if the backend supports a specific feature
    fn supports_feature(&self, feature: StoreFeature) -> bool;
    
    /// Flush all data (for testing/development)
    async fn flush_all(&self) -> StoreResult<()>;
}

/// Features that storage backends may or may not support
#[derive(Debug, Clone, PartialEq)]
pub enum StoreFeature {
    /// TTL (Time To Live) support
    Ttl,
    /// Atomic operations
    AtomicOperations,
    /// Transactions
    Transactions,
    /// Pub/Sub messaging
    PubSub,
    /// Lua scripting
    Scripting,
    /// Clustering
    Clustering,
    /// Persistence
    Persistence,
    /// Replication
    Replication,
    /// Compression
    Compression,
    /// Encryption
    Encryption,
}

/// Trait for observability and metrics collection
#[async_trait]
pub trait StoreObserver: Send + Sync {
    /// Record an operation metric
    async fn record_operation(&self, metric: OperationMetrics);
    
    /// Record a health check result
    async fn record_health_check(&self, backend: &str, status: HealthStatus);
    
    /// Record an error
    async fn record_error(&self, backend: &str, error: &StoreError);
    
    /// Get aggregated metrics
    async fn get_metrics(&self, backend: &str) -> StoreResult<StoreStats>;
}

/// Trait for notification adapters
#[async_trait]
pub trait NotificationAdapter: Send + Sync {
    /// Send a notification
    async fn send_notification(&self, message: NotificationMessage) -> StoreResult<()>;
    
    /// Get adapter name
    fn adapter_name(&self) -> &'static str;
    
    /// Check if adapter is configured and ready
    async fn is_ready(&self) -> bool;
}

/// Notification message structure
#[derive(Debug, Clone)]
pub struct NotificationMessage {
    pub title: String,
    pub body: String,
    pub severity: NotificationSeverity,
    pub tags: Vec<String>,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Trait for telemetry exporters
#[async_trait]
pub trait TelemetryExporter: Send + Sync {
    /// Export metrics
    async fn export_metrics(&self, metrics: Vec<TelemetryMetric>) -> StoreResult<()>;
    
    /// Export traces
    async fn export_traces(&self, traces: Vec<TelemetryTrace>) -> StoreResult<()>;
    
    /// Get exporter name
    fn exporter_name(&self) -> &'static str;
}

/// Telemetry metric
#[derive(Debug, Clone)]
pub struct TelemetryMetric {
    pub name: String,
    pub value: f64,
    pub labels: std::collections::HashMap<String, String>,
    pub timestamp: std::time::SystemTime,
}

/// Telemetry trace
#[derive(Debug, Clone)]
pub struct TelemetryTrace {
    pub trace_id: String,
    pub span_id: String,
    pub operation_name: String,
    pub start_time: std::time::SystemTime,
    pub duration: Duration,
    pub tags: std::collections::HashMap<String, String>,
}