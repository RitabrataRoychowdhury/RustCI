// Backward compatibility layer for Valkyrie observability

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

use super::{ObservabilityManager, ObservabilityError, LogLevel};

/// Legacy observability interface for backward compatibility
pub struct LegacyObservabilityAdapter {
    /// Valkyrie observability manager
    valkyrie_manager: Arc<ObservabilityManager>,
    /// Compatibility configuration
    config: CompatibilityConfig,
}

/// Compatibility configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityConfig {
    /// Enable legacy API compatibility
    pub legacy_api_enabled: bool,
    /// Enable automatic metric conversion
    pub auto_metric_conversion: bool,
    /// Enable legacy log format support
    pub legacy_log_format: bool,
    /// Deprecation warnings enabled
    pub deprecation_warnings: bool,
    /// Version compatibility level
    pub compatibility_version: String,
}

impl Default for CompatibilityConfig {
    fn default() -> Self {
        Self {
            legacy_api_enabled: true,
            auto_metric_conversion: true,
            legacy_log_format: true,
            deprecation_warnings: true,
            compatibility_version: "1.0.0".to_string(),
        }
    }
}

/// Legacy metric types for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegacyMetricType {
    Counter,
    Gauge,
    Histogram,
    Timer,
    Meter,
}

/// Legacy log levels for backward compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LegacyObservabilityAdapter {
    /// Create a new legacy observability adapter
    pub fn new(valkyrie_manager: Arc<ObservabilityManager>, config: CompatibilityConfig) -> Self {
        Self {
            valkyrie_manager,
            config,
        }
    }

    /// Create with default configuration
    pub fn with_default_config(valkyrie_manager: Arc<ObservabilityManager>) -> Self {
        Self::new(valkyrie_manager, CompatibilityConfig::default())
    }

    // Legacy Metrics API
    
    /// Record a legacy counter metric
    pub async fn record_counter(&self, name: &str, value: u64, tags: Option<HashMap<String, String>>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("record_counter", "Use metrics().increment_counter() instead").await;
        }

        let labels = tags.unwrap_or_default();
        let metrics = self.valkyrie_manager.metrics();
        
        for _ in 0..value {
            metrics.increment_counter(name, labels.clone()).await?;
        }
        
        Ok(())
    }

    /// Record a legacy gauge metric
    pub async fn record_gauge(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("record_gauge", "Use metrics().set_gauge() instead").await;
        }

        let labels = tags.unwrap_or_default();
        let metrics = self.valkyrie_manager.metrics();
        metrics.set_gauge(name, value, labels).await
    }

    /// Record a legacy histogram metric
    pub async fn record_histogram(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("record_histogram", "Use metrics().record_histogram() instead").await;
        }

        let labels = tags.unwrap_or_default();
        let metrics = self.valkyrie_manager.metrics();
        metrics.record_histogram(name, vec![value], labels).await
    }

    /// Record a legacy timer metric
    pub async fn record_timer(&self, name: &str, duration_ms: f64, tags: Option<HashMap<String, String>>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("record_timer", "Use metrics().record_histogram() with duration values instead").await;
        }

        let labels = tags.unwrap_or_default();
        let metrics = self.valkyrie_manager.metrics();
        metrics.record_histogram(name, vec![duration_ms], labels).await
    }

    /// Record a legacy meter metric
    pub async fn record_meter(&self, name: &str, rate: f64, tags: Option<HashMap<String, String>>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("record_meter", "Use metrics().set_gauge() for rate metrics instead").await;
        }

        let labels = tags.unwrap_or_default();
        let metrics = self.valkyrie_manager.metrics();
        metrics.set_gauge(name, rate, labels).await
    }

    // Legacy Logging API

    /// Log with legacy format
    pub async fn log(&self, level: LegacyLogLevel, message: &str, fields: Option<HashMap<String, String>>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("log", "Use logger().log() with structured context instead").await;
        }

        let valkyrie_level = self.convert_log_level(level);
        let logger = self.valkyrie_manager.logger();
        
        let context = if let Some(fields) = fields {
            fields.into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect()
        } else {
            HashMap::new()
        };

        logger.log(valkyrie_level, message, context).await
    }

    /// Legacy log methods
    pub async fn log_trace(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LegacyLogLevel::Trace, message, None).await
    }

    pub async fn log_debug(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LegacyLogLevel::Debug, message, None).await
    }

    pub async fn log_info(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LegacyLogLevel::Info, message, None).await
    }

    pub async fn log_warn(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LegacyLogLevel::Warn, message, None).await
    }

    pub async fn log_error(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LegacyLogLevel::Error, message, None).await
    }

    pub async fn log_fatal(&self, message: &str) -> Result<(), ObservabilityError> {
        self.log(LegacyLogLevel::Fatal, message, None).await
    }

    // Legacy Health Check API

    /// Register a legacy health check
    pub async fn register_health_check(&self, name: &str, _check_fn: Box<dyn Fn() -> bool + Send + Sync>) -> Result<(), ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("register_health_check", "Use health().register_check() with HealthCheck struct instead").await;
        }

        // Convert legacy health check to Valkyrie format
        let health_check = super::health::HealthCheck {
            id: name.to_string(),
            name: name.to_string(),
            description: format!("Legacy health check: {}", name),
            check_type: super::health::HealthCheckType::Custom {
                function_name: name.to_string(),
            },
            config: super::health::HealthCheckConfig {
                parameters: HashMap::new(),
                environment: HashMap::new(),
                working_directory: None,
            },
            interval_seconds: 30,
            timeout_seconds: 5,
            failure_threshold: 3,
            success_threshold: 2,
            enabled: true,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let health = self.valkyrie_manager.health();
        health.register_check(health_check).await
    }

    /// Get legacy health status
    pub async fn get_health_status(&self) -> Result<LegacyHealthStatus, ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("get_health_status", "Use health().summary() instead").await;
        }

        let health = self.valkyrie_manager.health();
        let summary = health.summary().await;
        
        Ok(LegacyHealthStatus {
            overall_status: match summary.overall_status {
                super::health::HealthStatus::Healthy => "healthy".to_string(),
                super::health::HealthStatus::Degraded => "degraded".to_string(),
                super::health::HealthStatus::Unhealthy => "unhealthy".to_string(),
                super::health::HealthStatus::Unknown => "unknown".to_string(),
            },
            total_checks: summary.total_checks,
            healthy_checks: summary.healthy_checks,
            failed_checks: summary.unhealthy_checks,
        })
    }

    // Legacy Dashboard API

    /// Get legacy dashboard data
    pub async fn get_dashboard_data(&self) -> Result<LegacyDashboardData, ObservabilityError> {
        if self.config.deprecation_warnings {
            self.log_deprecation_warning("get_dashboard_data", "Use dashboard().get_data() instead").await;
        }

        let dashboard = self.valkyrie_manager.dashboard();
        let data = dashboard.get_data().await;
        
        Ok(LegacyDashboardData {
            title: data.metadata.title,
            uptime: data.metadata.uptime_seconds,
            metrics_count: data.metrics_summary.total_metrics,
            log_count: data.metrics_summary.total_data_points, // Approximation
            health_status: match data.health_status.overall_status {
                super::health::HealthStatus::Healthy => "healthy".to_string(),
                super::health::HealthStatus::Degraded => "degraded".to_string(),
                super::health::HealthStatus::Unhealthy => "unhealthy".to_string(),
                super::health::HealthStatus::Unknown => "unknown".to_string(),
            },
        })
    }

    // Utility methods

    /// Convert legacy log level to Valkyrie log level
    fn convert_log_level(&self, level: LegacyLogLevel) -> LogLevel {
        match level {
            LegacyLogLevel::Trace => LogLevel::Trace,
            LegacyLogLevel::Debug => LogLevel::Debug,
            LegacyLogLevel::Info => LogLevel::Info,
            LegacyLogLevel::Warn => LogLevel::Warn,
            LegacyLogLevel::Error => LogLevel::Error,
            LegacyLogLevel::Fatal => LogLevel::Fatal,
        }
    }

    /// Log deprecation warning
    async fn log_deprecation_warning(&self, method: &str, suggestion: &str) {
        let logger = self.valkyrie_manager.logger();
        let mut context = HashMap::new();
        context.insert("deprecated_method".to_string(), serde_json::Value::String(method.to_string()));
        context.insert("suggestion".to_string(), serde_json::Value::String(suggestion.to_string()));
        context.insert("compatibility_version".to_string(), serde_json::Value::String(self.config.compatibility_version.clone()));
        
        let _ = logger.log(
            LogLevel::Warn,
            &format!("DEPRECATED: {} is deprecated. {}", method, suggestion),
            context,
        ).await;
    }

    /// Check if feature is supported in compatibility mode
    pub fn is_feature_supported(&self, feature: &str) -> bool {
        match feature {
            "legacy_metrics" => self.config.auto_metric_conversion,
            "legacy_logging" => self.config.legacy_log_format,
            "legacy_health" => true, // Always supported
            "legacy_dashboard" => true, // Always supported
            _ => false,
        }
    }

    /// Get compatibility information
    pub fn get_compatibility_info(&self) -> CompatibilityInfo {
        CompatibilityInfo {
            version: self.config.compatibility_version.clone(),
            legacy_api_enabled: self.config.legacy_api_enabled,
            supported_features: vec![
                "legacy_metrics".to_string(),
                "legacy_logging".to_string(),
                "legacy_health".to_string(),
                "legacy_dashboard".to_string(),
            ],
            deprecation_warnings: self.config.deprecation_warnings,
        }
    }
}

/// Legacy health status for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyHealthStatus {
    pub overall_status: String,
    pub total_checks: usize,
    pub healthy_checks: usize,
    pub failed_checks: usize,
}

/// Legacy dashboard data for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyDashboardData {
    pub title: String,
    pub uptime: u64,
    pub metrics_count: usize,
    pub log_count: usize,
    pub health_status: String,
}

/// Compatibility information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    pub version: String,
    pub legacy_api_enabled: bool,
    pub supported_features: Vec<String>,
    pub deprecation_warnings: bool,
}

/// Protocol version negotiation
pub struct ProtocolVersionNegotiator {
    /// Supported protocol versions
    supported_versions: Vec<String>,
    /// Current protocol version
    current_version: String,
}

impl ProtocolVersionNegotiator {
    /// Create a new protocol version negotiator
    pub fn new() -> Self {
        Self {
            supported_versions: vec![
                "1.0.0".to_string(),
                "0.9.0".to_string(),
                "0.8.0".to_string(),
            ],
            current_version: "1.0.0".to_string(),
        }
    }

    /// Negotiate protocol version with client
    pub fn negotiate_version(&self, client_versions: &[String]) -> Option<String> {
        // Find the highest mutually supported version
        for version in &self.supported_versions {
            if client_versions.contains(version) {
                return Some(version.clone());
            }
        }
        None
    }

    /// Check if version is supported
    pub fn is_version_supported(&self, version: &str) -> bool {
        self.supported_versions.contains(&version.to_string())
    }

    /// Get supported versions
    pub fn get_supported_versions(&self) -> &[String] {
        &self.supported_versions
    }

    /// Get current version
    pub fn get_current_version(&self) -> &str {
        &self.current_version
    }
}

impl Default for ProtocolVersionNegotiator {
    fn default() -> Self {
        Self::new()
    }
}

/// Feature detection for graceful degradation
pub struct FeatureDetector {
    /// Available features
    available_features: HashMap<String, bool>,
}

impl FeatureDetector {
    /// Create a new feature detector
    pub fn new() -> Self {
        let mut available_features = HashMap::new();
        
        // Core observability features
        available_features.insert("metrics".to_string(), true);
        available_features.insert("logging".to_string(), true);
        available_features.insert("health".to_string(), true);
        available_features.insert("dashboard".to_string(), true);
        available_features.insert("correlation".to_string(), true);
        
        // Advanced features
        available_features.insert("distributed_tracing".to_string(), true);
        available_features.insert("custom_metrics".to_string(), true);
        available_features.insert("structured_logging".to_string(), true);
        available_features.insert("health_checks".to_string(), true);
        available_features.insert("html_dashboard".to_string(), true);
        
        // Legacy compatibility features
        available_features.insert("legacy_metrics_api".to_string(), true);
        available_features.insert("legacy_logging_api".to_string(), true);
        available_features.insert("legacy_health_api".to_string(), true);
        
        Self {
            available_features,
        }
    }

    /// Check if feature is available
    pub fn is_feature_available(&self, feature: &str) -> bool {
        self.available_features.get(feature).copied().unwrap_or(false)
    }

    /// Get all available features
    pub fn get_available_features(&self) -> Vec<String> {
        self.available_features
            .iter()
            .filter_map(|(feature, &available)| {
                if available {
                    Some(feature.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Enable or disable a feature
    pub fn set_feature_availability(&mut self, feature: &str, available: bool) {
        self.available_features.insert(feature.to_string(), available);
    }
}

impl Default for FeatureDetector {
    fn default() -> Self {
        Self::new()
    }
}