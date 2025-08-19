// Pluggable Observability Adapter System
// Task 3.4: Pluggable Observability System

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Core trait for metrics adapters - users can implement this for their monitoring systems
#[async_trait]
pub trait MetricsAdapter: Send + Sync {
    /// Record a counter metric (monotonically increasing value)
    async fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]);

    /// Record a gauge metric (current value that can go up or down)
    async fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Record a histogram metric (distribution of values)
    async fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Record a timing metric (duration measurement)
    async fn record_timing(&self, name: &str, duration: Duration, labels: &[(&str, &str)]);

    /// Get adapter name for identification
    fn name(&self) -> &str;

    /// Check if adapter is healthy and ready to receive metrics
    async fn health_check(&self) -> AdapterHealth;
}

/// Core trait for tracing adapters - distributed tracing support
#[async_trait]
pub trait TracingAdapter: Send + Sync {
    /// Start a new span with optional parent
    async fn start_span(&self, name: &str, parent: Option<SpanId>) -> SpanId;

    /// End a span
    async fn end_span(&self, span_id: SpanId);

    /// Add an event to a span
    async fn add_event(&self, span_id: SpanId, name: &str, attributes: &[(&str, &str)]);

    /// Add attributes to a span
    async fn add_attributes(&self, span_id: SpanId, attributes: &[(&str, &str)]);

    /// Set span status (success, error, etc.)
    async fn set_span_status(&self, span_id: SpanId, status: SpanStatus);

    /// Get adapter name
    fn name(&self) -> &str;

    /// Health check
    async fn health_check(&self) -> AdapterHealth;
}

/// Core trait for health monitoring adapters
#[async_trait]
pub trait HealthAdapter: Send + Sync {
    /// Report component health status
    async fn report_health(&self, component: &str, status: HealthStatus);

    /// Check dependencies health
    async fn check_dependencies(&self) -> Vec<DependencyHealth>;

    /// Record availability metric
    async fn record_availability(&self, component: &str, available: bool);

    /// Get adapter name
    fn name(&self) -> &str;

    /// Health check
    async fn health_check(&self) -> AdapterHealth;
}

/// Core trait for logging adapters - structured logging support
#[async_trait]
pub trait LoggingAdapter: Send + Sync {
    /// Log a message with level and structured fields
    async fn log(&self, level: LogLevel, message: &str, fields: &[(&str, &str)]);

    /// Log with correlation ID for distributed tracing
    async fn log_with_correlation(
        &self,
        level: LogLevel,
        message: &str,
        correlation_id: &str,
        fields: &[(&str, &str)],
    );

    /// Get adapter name
    fn name(&self) -> &str;

    /// Health check
    async fn health_check(&self) -> AdapterHealth;
}

/// Span identifier for distributed tracing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(pub Uuid);

impl SpanId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Span status for tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error { message: String },
    Cancelled,
    DeadlineExceeded,
    InvalidArgument,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    DataLoss,
    Unauthenticated,
}

/// Health status for components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unknown,
}

/// Dependency health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: SystemTime,
    pub response_time: Option<Duration>,
    pub error_count: u64,
}

/// Adapter health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealth {
    pub status: HealthStatus,
    pub last_successful_operation: Option<SystemTime>,
    pub error_count: u64,
    pub total_operations: u64,
}

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

/// Central observability manager that coordinates all adapters
pub struct ObservabilityManager {
    /// Registered metrics adapters
    metrics_adapters: Vec<Arc<dyn MetricsAdapter>>,

    /// Registered tracing adapters
    tracing_adapters: Vec<Arc<dyn TracingAdapter>>,

    /// Registered health adapters
    health_adapters: Vec<Arc<dyn HealthAdapter>>,

    /// Registered logging adapters
    logging_adapters: Vec<Arc<dyn LoggingAdapter>>,

    /// Configuration
    config: ObservabilityConfig,

    /// Circuit breakers for failing adapters
    circuit_breakers: HashMap<String, CircuitBreaker>,
}

/// Configuration for the observability system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable/disable metrics collection
    pub metrics_enabled: bool,

    /// Enable/disable tracing
    pub tracing_enabled: bool,

    /// Enable/disable health monitoring
    pub health_enabled: bool,

    /// Enable/disable logging
    pub logging_enabled: bool,

    /// Minimum log level
    pub log_level: LogLevel,

    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,

    /// Adapter-specific configurations
    pub adapter_configs: HashMap<String, serde_json::Value>,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,

    /// Time to wait before trying again
    pub timeout: Duration,

    /// Success threshold to close circuit
    pub success_threshold: u32,
}

/// Circuit breaker for adapter failure handling
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure: Option<SystemTime>,
    pub config: CircuitBreakerConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing, not trying
    HalfOpen, // Testing if service recovered
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            tracing_enabled: true,
            health_enabled: true,
            logging_enabled: true,
            log_level: LogLevel::Info,
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 5,
                timeout: Duration::from_secs(60),
                success_threshold: 3,
            },
            adapter_configs: HashMap::new(),
        }
    }
}

impl ObservabilityManager {
    /// Create a new observability manager with default configuration
    pub fn new() -> Self {
        Self::with_config(ObservabilityConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: ObservabilityConfig) -> Self {
        let mut manager = Self {
            metrics_adapters: Vec::new(),
            tracing_adapters: Vec::new(),
            health_adapters: Vec::new(),
            logging_adapters: Vec::new(),
            config,
            circuit_breakers: HashMap::new(),
        };

        // Always register the built-in no-op adapters as fallbacks
        manager.register_metrics_adapter(Arc::new(NoOpMetricsAdapter));
        manager.register_tracing_adapter(Arc::new(NoOpTracingAdapter));
        manager.register_health_adapter(Arc::new(NoOpHealthAdapter));
        manager.register_logging_adapter(Arc::new(BuiltInJsonLogger::new()));

        manager
    }

    /// Register a metrics adapter
    pub fn register_metrics_adapter(&mut self, adapter: Arc<dyn MetricsAdapter>) {
        let name = adapter.name().to_string();
        self.circuit_breakers.insert(
            name,
            CircuitBreaker::new(self.config.circuit_breaker.clone()),
        );
        self.metrics_adapters.push(adapter);
    }

    /// Register a tracing adapter
    pub fn register_tracing_adapter(&mut self, adapter: Arc<dyn TracingAdapter>) {
        let name = adapter.name().to_string();
        self.circuit_breakers.insert(
            name,
            CircuitBreaker::new(self.config.circuit_breaker.clone()),
        );
        self.tracing_adapters.push(adapter);
    }

    /// Register a health adapter
    pub fn register_health_adapter(&mut self, adapter: Arc<dyn HealthAdapter>) {
        let name = adapter.name().to_string();
        self.circuit_breakers.insert(
            name,
            CircuitBreaker::new(self.config.circuit_breaker.clone()),
        );
        self.health_adapters.push(adapter);
    }

    /// Register a logging adapter
    pub fn register_logging_adapter(&mut self, adapter: Arc<dyn LoggingAdapter>) {
        let name = adapter.name().to_string();
        self.circuit_breakers.insert(
            name,
            CircuitBreaker::new(self.config.circuit_breaker.clone()),
        );
        self.logging_adapters.push(adapter);
    }

    /// Record a counter metric across all registered adapters
    pub async fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]) {
        if !self.config.metrics_enabled {
            return;
        }

        for adapter in &self.metrics_adapters {
            if self.is_circuit_open(adapter.name()) {
                continue;
            }

            adapter.record_counter(name, value, labels).await;
        }
    }

    /// Record a gauge metric across all registered adapters
    pub async fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if !self.config.metrics_enabled {
            return;
        }

        for adapter in &self.metrics_adapters {
            if self.is_circuit_open(adapter.name()) {
                continue;
            }

            adapter.record_gauge(name, value, labels).await;
        }
    }

    /// Start a span across all tracing adapters
    pub async fn start_span(&self, name: &str, parent: Option<SpanId>) -> Vec<SpanId> {
        if !self.config.tracing_enabled {
            return Vec::new();
        }

        let mut span_ids = Vec::new();

        for adapter in &self.tracing_adapters {
            if self.is_circuit_open(adapter.name()) {
                continue;
            }

            let span_id = adapter.start_span(name, parent).await;
            span_ids.push(span_id);
        }

        span_ids
    }

    /// Log a message across all logging adapters
    pub async fn log(&self, level: LogLevel, message: &str, fields: &[(&str, &str)]) {
        if !self.config.logging_enabled || level < self.config.log_level {
            return;
        }

        for adapter in &self.logging_adapters {
            if self.is_circuit_open(adapter.name()) {
                continue;
            }

            adapter.log(level, message, fields).await;
        }
    }

    /// Check if circuit breaker is open for an adapter
    fn is_circuit_open(&self, adapter_name: &str) -> bool {
        self.circuit_breakers
            .get(adapter_name)
            .map(|cb| cb.state == CircuitBreakerState::Open)
            .unwrap_or(false)
    }

    /// Record successful operation for circuit breaker
    fn record_success(&self, adapter_name: &str) {
        // Implementation would update circuit breaker state
        // For now, this is a placeholder
    }

    /// Record failed operation for circuit breaker
    fn record_failure(&self, adapter_name: &str) {
        // Implementation would update circuit breaker state
        // For now, this is a placeholder
    }

    /// Get health status of all adapters
    pub async fn get_adapter_health(&self) -> HashMap<String, AdapterHealth> {
        let mut health_map = HashMap::new();

        // Check metrics adapters
        for adapter in &self.metrics_adapters {
            let health = adapter.health_check().await;
            health_map.insert(format!("metrics_{}", adapter.name()), health);
        }

        // Check tracing adapters
        for adapter in &self.tracing_adapters {
            let health = adapter.health_check().await;
            health_map.insert(format!("tracing_{}", adapter.name()), health);
        }

        // Check health adapters
        for adapter in &self.health_adapters {
            let health = adapter.health_check().await;
            health_map.insert(format!("health_{}", adapter.name()), health);
        }

        // Check logging adapters
        for adapter in &self.logging_adapters {
            let health = adapter.health_check().await;
            health_map.insert(format!("logging_{}", adapter.name()), health);
        }

        health_map
    }
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            config,
        }
    }
}

/// No-op metrics adapter (zero cost, always available)
pub struct NoOpMetricsAdapter;

#[async_trait]
impl MetricsAdapter for NoOpMetricsAdapter {
    async fn record_counter(&self, _name: &str, _value: u64, _labels: &[(&str, &str)]) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn record_gauge(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn record_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn record_timing(&self, _name: &str, _duration: Duration, _labels: &[(&str, &str)]) {
        // Zero cost - compiles to nothing in release mode
    }

    fn name(&self) -> &str {
        "noop"
    }

    async fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: HealthStatus::Healthy,
            last_successful_operation: Some(SystemTime::now()),
            error_count: 0,
            total_operations: 0,
        }
    }
}

/// No-op tracing adapter (zero cost, always available)
pub struct NoOpTracingAdapter;

#[async_trait]
impl TracingAdapter for NoOpTracingAdapter {
    async fn start_span(&self, _name: &str, _parent: Option<SpanId>) -> SpanId {
        SpanId::new() // Return a valid span ID but do nothing
    }

    async fn end_span(&self, _span_id: SpanId) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn add_event(&self, _span_id: SpanId, _name: &str, _attributes: &[(&str, &str)]) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn add_attributes(&self, _span_id: SpanId, _attributes: &[(&str, &str)]) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn set_span_status(&self, _span_id: SpanId, _status: SpanStatus) {
        // Zero cost - compiles to nothing in release mode
    }

    fn name(&self) -> &str {
        "noop"
    }

    async fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: HealthStatus::Healthy,
            last_successful_operation: Some(SystemTime::now()),
            error_count: 0,
            total_operations: 0,
        }
    }
}

/// No-op health adapter (zero cost, always available)
pub struct NoOpHealthAdapter;

#[async_trait]
impl HealthAdapter for NoOpHealthAdapter {
    async fn report_health(&self, _component: &str, _status: HealthStatus) {
        // Zero cost - compiles to nothing in release mode
    }

    async fn check_dependencies(&self) -> Vec<DependencyHealth> {
        Vec::new() // No dependencies to check
    }

    async fn record_availability(&self, _component: &str, _available: bool) {
        // Zero cost - compiles to nothing in release mode
    }

    fn name(&self) -> &str {
        "noop"
    }

    async fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: HealthStatus::Healthy,
            last_successful_operation: Some(SystemTime::now()),
            error_count: 0,
            total_operations: 0,
        }
    }
}

/// Built-in JSON logger (always available, no external dependencies)
pub struct BuiltInJsonLogger {
    min_level: LogLevel,
}

impl BuiltInJsonLogger {
    pub fn new() -> Self {
        Self {
            min_level: LogLevel::Info,
        }
    }

    pub fn with_level(min_level: LogLevel) -> Self {
        Self { min_level }
    }
}

#[async_trait]
impl LoggingAdapter for BuiltInJsonLogger {
    async fn log(&self, level: LogLevel, message: &str, fields: &[(&str, &str)]) {
        if level < self.min_level {
            return;
        }

        let mut log_entry = serde_json::json!({
            "timestamp": SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "level": format!("{:?}", level).to_uppercase(),
            "message": message,
            "component": "valkyrie"
        });

        // Add custom fields
        for (key, value) in fields {
            log_entry[key] = serde_json::Value::String(value.to_string());
        }

        // Print to stdout (can be redirected by users)
        println!("{}", log_entry);
    }

    async fn log_with_correlation(
        &self,
        level: LogLevel,
        message: &str,
        correlation_id: &str,
        fields: &[(&str, &str)],
    ) {
        let mut extended_fields = vec![("correlation_id", correlation_id)];
        extended_fields.extend_from_slice(fields);
        self.log(level, message, &extended_fields).await;
    }

    fn name(&self) -> &str {
        "builtin_json"
    }

    async fn health_check(&self) -> AdapterHealth {
        AdapterHealth {
            status: HealthStatus::Healthy,
            last_successful_operation: Some(SystemTime::now()),
            error_count: 0,
            total_operations: 0,
        }
    }
}

impl PartialOrd for LogLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_val = match self {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
            LogLevel::Fatal => 5,
        };

        let other_val = match other {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
            LogLevel::Fatal => 5,
        };

        self_val.cmp(&other_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability_manager_creation() {
        let manager = ObservabilityManager::new();

        // Should have default adapters registered
        assert!(!manager.metrics_adapters.is_empty());
        assert!(!manager.tracing_adapters.is_empty());
        assert!(!manager.health_adapters.is_empty());
        assert!(!manager.logging_adapters.is_empty());
    }

    #[tokio::test]
    async fn test_no_op_adapters() {
        let manager = ObservabilityManager::new();

        // These should work without any external dependencies
        manager
            .record_counter("test_counter", 1, &[("label", "value")])
            .await;
        manager
            .record_gauge("test_gauge", 1.0, &[("label", "value")])
            .await;

        let _spans = manager.start_span("test_span", None).await;

        manager
            .log(LogLevel::Info, "Test message", &[("field", "value")])
            .await;
    }

    #[tokio::test]
    async fn test_built_in_json_logger() {
        let logger = BuiltInJsonLogger::new();

        // Should log without errors
        logger
            .log(LogLevel::Info, "Test message", &[("key", "value")])
            .await;
        logger
            .log_with_correlation(LogLevel::Warn, "Test with correlation", "test-id", &[])
            .await;

        let health = logger.health_check().await;
        assert!(matches!(health.status, HealthStatus::Healthy));
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Fatal);
    }
}
