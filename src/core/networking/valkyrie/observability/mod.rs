// Valkyrie Protocol Observability Module
// Self-contained observability without external dependencies

pub mod metrics;
pub mod logging;
pub mod health;
pub mod dashboard;
pub mod correlation;
pub mod compatibility;
pub mod external;
pub mod adapters;

use std::sync::Arc;
use std::collections::HashMap;
// tokio::sync::RwLock import removed as unused
use serde::{Serialize, Deserialize};
// uuid::Uuid import removed as unused

pub use metrics::{MetricsCollector, MetricValue, MetricType};
pub use logging::{StructuredLogger, LogLevel, LogEntry};
pub use health::{HealthMonitor, HealthStatus, HealthCheck};
pub use dashboard::{MetricsDashboard, DashboardConfig};
pub use correlation::{CorrelationId, CorrelationTracker};
pub use compatibility::{
    LegacyObservabilityAdapter, CompatibilityConfig, LegacyMetricType, LegacyLogLevel,
    LegacyHealthStatus, LegacyDashboardData, CompatibilityInfo,
    ProtocolVersionNegotiator, FeatureDetector
};
pub use external::{
    ExternalObservabilityIntegration, ExternalObservabilityConfig, ExternalObservabilityStatus,
    PrometheusIntegration, OpenTelemetryIntegration, JaegerIntegration, GrafanaIntegration,
    IntegrationStatus, TraceSpan, SpanLog
};

/// Main observability manager for the Valkyrie Protocol
pub struct ObservabilityManager {
    /// Built-in metrics collection
    metrics_collector: Arc<MetricsCollector>,
    /// Structured logging with correlation IDs
    logger: Arc<StructuredLogger>,
    /// Health monitoring system
    health_monitor: Arc<HealthMonitor>,
    /// Internal metrics dashboard
    dashboard: Arc<MetricsDashboard>,
    /// Correlation tracking
    correlation_tracker: Arc<CorrelationTracker>,
    /// External integrations
    external_integration: Option<Arc<external::ExternalObservabilityIntegration>>,
    /// Plug-and-play adapter registry
    adapter_registry: Arc<adapters::ObservabilityAdapterRegistry>,
    /// Configuration
    config: ObservabilityConfig,
}

/// Configuration for observability features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Enable structured logging
    pub logging_enabled: bool,
    /// Enable health monitoring
    pub health_enabled: bool,
    /// Enable internal dashboard
    pub dashboard_enabled: bool,
    /// Metrics retention period in seconds
    pub metrics_retention_seconds: u64,
    /// Log retention period in seconds
    pub log_retention_seconds: u64,
    /// Health check interval in seconds
    pub health_check_interval_seconds: u64,
    /// Dashboard refresh interval in seconds
    pub dashboard_refresh_seconds: u64,
    /// External observability configuration
    pub external_config: Option<external::ExternalObservabilityConfig>,
    /// Adapter registry configuration
    pub adapter_registry_config: Option<adapters::AdapterRegistryConfig>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            logging_enabled: true,
            health_enabled: true,
            dashboard_enabled: true,
            metrics_retention_seconds: 3600, // 1 hour
            log_retention_seconds: 86400,    // 24 hours
            health_check_interval_seconds: 30,
            dashboard_refresh_seconds: 5,
            external_config: None,
            adapter_registry_config: None,
        }
    }
}

impl ObservabilityManager {
    /// Create a new observability manager
    pub fn new(config: ObservabilityConfig) -> Self {
        let metrics_collector = Arc::new(MetricsCollector::new(config.metrics_retention_seconds));
        let logger = Arc::new(StructuredLogger::new(config.log_retention_seconds));
        let health_monitor = Arc::new(HealthMonitor::new(config.health_check_interval_seconds));
        let dashboard = Arc::new(MetricsDashboard::new(
            metrics_collector.clone(),
            health_monitor.clone(),
            config.dashboard_refresh_seconds,
        ));
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        
        let external_integration = if let Some(ref external_config) = config.external_config {
            Some(Arc::new(external::ExternalObservabilityIntegration::new(external_config.clone())))
        } else {
            None
        };

        let adapter_registry_config = config.adapter_registry_config.clone()
            .unwrap_or_else(|| adapters::AdapterRegistryConfig::default());
        let adapter_registry = Arc::new(adapters::ObservabilityAdapterRegistry::new(adapter_registry_config));

        Self {
            metrics_collector,
            logger,
            health_monitor,
            dashboard,
            correlation_tracker,
            adapter_registry,
            external_integration,
            config,
        }
    }

    /// Start the observability manager
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        if self.config.metrics_enabled {
            self.metrics_collector.start().await?;
        }

        if self.config.logging_enabled {
            self.logger.start().await?;
        }

        if self.config.health_enabled {
            self.health_monitor.start().await?;
        }

        if self.config.dashboard_enabled {
            self.dashboard.start().await?;
        }

        if let Some(ref external) = self.external_integration {
            external.start().await?;
        }

        Ok(())
    }

    /// Stop the observability manager
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        if let Some(ref external) = self.external_integration {
            external.stop().await?;
        }

        if self.config.dashboard_enabled {
            self.dashboard.stop().await?;
        }

        if self.config.health_enabled {
            self.health_monitor.stop().await?;
        }

        if self.config.logging_enabled {
            self.logger.stop().await?;
        }

        if self.config.metrics_enabled {
            self.metrics_collector.stop().await?;
        }

        Ok(())
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics_collector.clone()
    }

    /// Get structured logger
    pub fn logger(&self) -> Arc<StructuredLogger> {
        self.logger.clone()
    }

    /// Get health monitor
    pub fn health(&self) -> Arc<HealthMonitor> {
        self.health_monitor.clone()
    }

    /// Get dashboard
    pub fn dashboard(&self) -> Arc<MetricsDashboard> {
        self.dashboard.clone()
    }

    /// Get correlation tracker
    pub fn correlation(&self) -> Arc<CorrelationTracker> {
        self.correlation_tracker.clone()
    }

    /// Get external integrations
    pub fn external(&self) -> Option<Arc<external::ExternalObservabilityIntegration>> {
        self.external_integration.clone()
    }

    /// Create a new correlation ID
    pub fn new_correlation_id(&self) -> CorrelationId {
        self.correlation_tracker.new_correlation_id()
    }

    /// Record a metric with correlation
    pub async fn record_metric_with_correlation(
        &self,
        correlation_id: CorrelationId,
        name: &str,
        value: MetricValue,
        labels: HashMap<String, String>,
    ) -> Result<(), ObservabilityError> {
        if !self.config.metrics_enabled {
            return Ok(());
        }

        let mut enhanced_labels = labels;
        enhanced_labels.insert("correlation_id".to_string(), correlation_id.to_string());

        self.metrics_collector.record(name, value, enhanced_labels).await
    }

    /// Log with correlation
    pub async fn log_with_correlation(
        &self,
        correlation_id: CorrelationId,
        level: LogLevel,
        message: &str,
        context: HashMap<String, serde_json::Value>,
    ) -> Result<(), ObservabilityError> {
        if !self.config.logging_enabled {
            return Ok(());
        }

        let mut enhanced_context = context;
        enhanced_context.insert(
            "correlation_id".to_string(),
            serde_json::Value::String(correlation_id.to_string()),
        );

        self.logger.log(level, message, enhanced_context).await
    }

    /// Get observability status
    pub async fn status(&self) -> ObservabilityStatus {
        ObservabilityStatus {
            metrics_enabled: self.config.metrics_enabled,
            logging_enabled: self.config.logging_enabled,
            health_enabled: self.config.health_enabled,
            dashboard_enabled: self.config.dashboard_enabled,
            metrics_count: if self.config.metrics_enabled {
                self.metrics_collector.count().await
            } else {
                0
            },
            log_count: if self.config.logging_enabled {
                self.logger.count().await
            } else {
                0
            },
            health_status: if self.config.health_enabled {
                self.health_monitor.overall_status().await
            } else {
                HealthStatus::Unknown
            },
        }
    }
}

/// Observability status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityStatus {
    pub metrics_enabled: bool,
    pub logging_enabled: bool,
    pub health_enabled: bool,
    pub dashboard_enabled: bool,
    pub metrics_count: usize,
    pub log_count: usize,
    pub health_status: HealthStatus,
}

/// Observability errors
#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    #[error("Metrics error: {0}")]
    Metrics(String),
    
    #[error("Logging error: {0}")]
    Logging(String),
    
    #[error("Health monitoring error: {0}")]
    Health(String),
    
    #[error("Dashboard error: {0}")]
    Dashboard(String),
    
    #[error("Correlation tracking error: {0}")]
    Correlation(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}