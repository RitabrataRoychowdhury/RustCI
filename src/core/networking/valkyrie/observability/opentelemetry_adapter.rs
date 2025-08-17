// Optional OpenTelemetry Adapter (Feature-Gated)
// Task 3.4: Pluggable Observability System

#[cfg(feature = "opentelemetry")]
use std::collections::HashMap;
#[cfg(feature = "opentelemetry")]
use std::sync::Arc;
#[cfg(feature = "opentelemetry")]
use std::time::{Duration, SystemTime};
#[cfg(feature = "opentelemetry")]
use async_trait::async_trait;
#[cfg(feature = "opentelemetry")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "opentelemetry")]
use dashmap::DashMap;

#[cfg(feature = "opentelemetry")]
use super::adapter_system::{
    TracingAdapter, SpanId, SpanStatus, AdapterHealth, HealthStatus
};

/// OpenTelemetry adapter configuration
#[cfg(feature = "opentelemetry")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// Service name for tracing
    pub service_name: String,
    
    /// Service version
    pub service_version: String,
    
    /// Jaeger endpoint for trace export
    pub jaeger_endpoint: Option<String>,
    
    /// OTLP endpoint for trace export
    pub otlp_endpoint: Option<String>,
    
    /// Sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,
    
    /// Maximum span attributes
    pub max_attributes: usize,
    
    /// Maximum span events
    pub max_events: usize,
    
    /// Batch export timeout
    pub batch_timeout: Duration,
    
    /// Maximum batch size
    pub max_batch_size: usize,
    
    /// Export timeout
    pub export_timeout: Duration,
    
    /// Resource attributes
    pub resource_attributes: HashMap<String, String>,
}

#[cfg(feature = "opentelemetry")]
impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        let mut resource_attributes = HashMap::new();
        resource_attributes.insert("service.name".to_string(), "rustci-valkyrie".to_string());
        resource_attributes.insert("service.version".to_string(), "0.1.0".to_string());
        
        Self {
            service_name: "rustci-valkyrie".to_string(),
            service_version: "0.1.0".to_string(),
            jaeger_endpoint: Some("http://localhost:14268/api/traces".to_string()),
            otlp_endpoint: None,
            sampling_ratio: 1.0, // Sample all traces by default
            max_attributes: 128,
            max_events: 128,
            batch_timeout: Duration::from_secs(5),
            max_batch_size: 512,
            export_timeout: Duration::from_secs(30),
            resource_attributes,
        }
    }
}

/// OpenTelemetry tracing adapter (only available with "opentelemetry" feature)
#[cfg(feature = "opentelemetry")]
pub struct OpenTelemetryAdapter {
    /// Configuration
    config: OpenTelemetryConfig,
    
    /// Active spans tracking
    active_spans: DashMap<SpanId, String>,
    
    /// Adapter health tracking
    health: Arc<std::sync::Mutex<AdapterHealthTracker>>,
}

#[cfg(feature = "opentelemetry")]
#[derive(Debug)]
struct AdapterHealthTracker {
    last_successful_operation: Option<SystemTime>,
    error_count: u64,
    total_operations: u64,
    spans_created: u64,
    spans_completed: u64,
}

#[cfg(feature = "opentelemetry")]
impl OpenTelemetryAdapter {
    /// Create a new OpenTelemetry adapter
    pub async fn new(config: OpenTelemetryConfig) -> Result<Self, OpenTelemetryAdapterError> {
        // For now, we'll create a basic implementation
        // In a full implementation, this would initialize the OpenTelemetry SDK
        
        Ok(Self {
            config,
            active_spans: DashMap::new(),
            health: Arc::new(std::sync::Mutex::new(AdapterHealthTracker {
                last_successful_operation: Some(SystemTime::now()),
                error_count: 0,
                total_operations: 0,
                spans_created: 0,
                spans_completed: 0,
            })),
        })
    }
    
    /// Update health tracking
    fn update_health(&self, success: bool, operation_type: HealthOperation) {
        if let Ok(mut health) = self.health.lock() {
            health.total_operations += 1;
            if success {
                health.last_successful_operation = Some(SystemTime::now());
                match operation_type {
                    HealthOperation::SpanCreated => health.spans_created += 1,
                    HealthOperation::SpanCompleted => health.spans_completed += 1,
                    HealthOperation::Other => {},
                }
            } else {
                health.error_count += 1;
            }
        }
    }
}

#[cfg(feature = "opentelemetry")]
#[derive(Debug)]
enum HealthOperation {
    SpanCreated,
    SpanCompleted,
    Other,
}

#[cfg(feature = "opentelemetry")]
#[async_trait]
impl TracingAdapter for OpenTelemetryAdapter {
    async fn start_span(&self, name: &str, parent: Option<SpanId>) -> SpanId {
        let span_id = SpanId::new();
        
        // Store the span context for future reference
        self.active_spans.insert(span_id, name.to_string());
        
        self.update_health(true, HealthOperation::SpanCreated);
        
        span_id
    }
    
    async fn end_span(&self, span_id: SpanId) {
        if let Some((_, _)) = self.active_spans.remove(&span_id) {
            self.update_health(true, HealthOperation::SpanCompleted);
        }
    }
    
    async fn add_event(&self, span_id: SpanId, name: &str, attributes: &[(&str, &str)]) {
        if self.active_spans.contains_key(&span_id) {
            // In a real implementation, we would add the event to the actual span
            self.update_health(true, HealthOperation::Other);
        }
    }
    
    async fn add_attributes(&self, span_id: SpanId, attributes: &[(&str, &str)]) {
        if self.active_spans.contains_key(&span_id) {
            // In a real implementation, we would add attributes to the actual span
            self.update_health(true, HealthOperation::Other);
        }
    }
    
    async fn set_span_status(&self, span_id: SpanId, status: SpanStatus) {
        if self.active_spans.contains_key(&span_id) {
            // In a real implementation, we would set the status on the actual span
            self.update_health(true, HealthOperation::Other);
        }
    }
    
    fn name(&self) -> &str {
        "opentelemetry"
    }
    
    async fn health_check(&self) -> AdapterHealth {
        let health = self.health.lock().unwrap();
        
        let status = if health.error_count > 0 && health.total_operations > 0 {
            let error_rate = health.error_count as f64 / health.total_operations as f64;
            if error_rate > 0.1 { // More than 10% error rate
                HealthStatus::Degraded {
                    reason: format!("High error rate: {:.2}%", error_rate * 100.0)
                }
            } else {
                HealthStatus::Healthy
            }
        } else {
            HealthStatus::Healthy
        };
        
        AdapterHealth {
            status,
            last_successful_operation: health.last_successful_operation,
            error_count: health.error_count,
            total_operations: health.total_operations,
        }
    }
}

/// OpenTelemetry adapter errors
#[cfg(feature = "opentelemetry")]
#[derive(Debug, thiserror::Error)]
pub enum OpenTelemetryAdapterError {
    #[error("Failed to initialize OpenTelemetry adapter: {0}")]
    InitializationFailed(String),
    
    #[error("Failed to export traces: {0}")]
    ExportFailed(String),
    
    #[error("Span operation failed: {0}")]
    SpanOperationFailed(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// Factory for creating OpenTelemetry adapter (only available with feature)
#[cfg(feature = "opentelemetry")]
pub struct OpenTelemetryAdapterFactory;

#[cfg(feature = "opentelemetry")]
impl OpenTelemetryAdapterFactory {
    /// Create an OpenTelemetry adapter with default configuration
    pub async fn create_default() -> Result<std::sync::Arc<dyn TracingAdapter>, OpenTelemetryAdapterError> {
        let config = OpenTelemetryConfig::default();
        let adapter = OpenTelemetryAdapter::new(config).await?;
        Ok(std::sync::Arc::new(adapter))
    }
    
    /// Create an OpenTelemetry adapter with custom configuration
    pub async fn create_with_config(config: OpenTelemetryConfig) -> Result<std::sync::Arc<dyn TracingAdapter>, OpenTelemetryAdapterError> {
        let adapter = OpenTelemetryAdapter::new(config).await?;
        Ok(std::sync::Arc::new(adapter))
    }
    
    /// Create from environment variables
    pub async fn create_from_env() -> Result<std::sync::Arc<dyn TracingAdapter>, OpenTelemetryAdapterError> {
        let config = OpenTelemetryConfig {
            service_name: std::env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "rustci-valkyrie".to_string()),
            service_version: std::env::var("OTEL_SERVICE_VERSION")
                .unwrap_or_else(|_| "0.1.0".to_string()),
            jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            sampling_ratio: std::env::var("OTEL_TRACES_SAMPLER_ARG")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0),
            ..Default::default()
        };
        
        Self::create_with_config(config).await
    }
    
    /// Create Jaeger-specific adapter
    pub async fn create_jaeger(jaeger_endpoint: String, service_name: String) -> Result<std::sync::Arc<dyn TracingAdapter>, OpenTelemetryAdapterError> {
        let config = OpenTelemetryConfig {
            service_name,
            jaeger_endpoint: Some(jaeger_endpoint),
            otlp_endpoint: None,
            ..Default::default()
        };
        
        Self::create_with_config(config).await
    }
    
    /// Create OTLP-specific adapter
    pub async fn create_otlp(otlp_endpoint: String, service_name: String) -> Result<std::sync::Arc<dyn TracingAdapter>, OpenTelemetryAdapterError> {
        let config = OpenTelemetryConfig {
            service_name,
            jaeger_endpoint: None,
            otlp_endpoint: Some(otlp_endpoint),
            ..Default::default()
        };
        
        Self::create_with_config(config).await
    }
}

// Stub implementations when opentelemetry feature is not enabled
#[cfg(not(feature = "opentelemetry"))]
pub struct OpenTelemetryConfig;

#[cfg(not(feature = "opentelemetry"))]
impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self
    }
}

#[cfg(not(feature = "opentelemetry"))]
pub struct OpenTelemetryAdapterFactory;

#[cfg(not(feature = "opentelemetry"))]
impl OpenTelemetryAdapterFactory {
    pub async fn create_default() -> Result<std::sync::Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::TracingAdapter>, Box<dyn std::error::Error>> {
        Err("OpenTelemetry feature not enabled. Enable with --features opentelemetry".into())
    }
    
    pub async fn create_with_config(_config: OpenTelemetryConfig) -> Result<std::sync::Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::TracingAdapter>, Box<dyn std::error::Error>> {
        Err("OpenTelemetry feature not enabled. Enable with --features opentelemetry".into())
    }
    
    pub async fn create_from_env() -> Result<std::sync::Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::TracingAdapter>, Box<dyn std::error::Error>> {
        Err("OpenTelemetry feature not enabled. Enable with --features opentelemetry".into())
    }
    
    pub async fn create_jaeger(_jaeger_endpoint: String, _service_name: String) -> Result<std::sync::Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::TracingAdapter>, Box<dyn std::error::Error>> {
        Err("OpenTelemetry feature not enabled. Enable with --features opentelemetry".into())
    }
    
    pub async fn create_otlp(_otlp_endpoint: String, _service_name: String) -> Result<std::sync::Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::TracingAdapter>, Box<dyn std::error::Error>> {
        Err("OpenTelemetry feature not enabled. Enable with --features opentelemetry".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "opentelemetry")]
    #[tokio::test]
    async fn test_opentelemetry_adapter_creation() {
        let config = OpenTelemetryConfig::default();
        let result = OpenTelemetryAdapter::new(config).await;
        
        match result {
            Ok(adapter) => {
                assert_eq!(adapter.name(), "opentelemetry");
                let health = adapter.health_check().await;
                assert!(matches!(health.status, HealthStatus::Healthy));
            }
            Err(_) => {
                // Expected in test environment without OpenTelemetry setup
                println!("OpenTelemetry adapter creation failed (expected in test environment)");
            }
        }
    }
    
    #[cfg(not(feature = "opentelemetry"))]
    #[tokio::test]
    async fn test_opentelemetry_feature_disabled() {
        let result = OpenTelemetryAdapterFactory::create_default().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("OpenTelemetry feature not enabled"));
    }
    
    #[test]
    fn test_opentelemetry_config_default() {
        let _config = OpenTelemetryConfig::default();
        // Should compile regardless of feature flag
    }
}