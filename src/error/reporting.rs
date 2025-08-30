use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};
use super::context::{ErrorContext, ErrorResponse, ErrorSeverity};

/// Error pattern for detecting recurring issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    /// Pattern identifier
    pub pattern_id: Uuid,
    /// Error type pattern
    pub error_type: String,
    /// Component pattern
    pub component_pattern: String,
    /// Operation pattern
    pub operation_pattern: String,
    /// Number of occurrences
    pub occurrence_count: u64,
    /// First occurrence timestamp
    pub first_occurrence: DateTime<Utc>,
    /// Last occurrence timestamp
    pub last_occurrence: DateTime<Utc>,
    /// Average time between occurrences (in seconds)
    pub average_interval_seconds: f64,
    /// Severity level
    pub severity: ErrorSeverity,
    /// Whether this pattern is currently active
    pub is_active: bool,
}

/// Error correlation data for tracking related errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCorrelation {
    /// Primary correlation ID
    pub correlation_id: Uuid,
    /// Related correlation IDs
    pub related_correlation_ids: Vec<Uuid>,
    /// Request ID if available
    pub request_id: Option<String>,
    /// User ID if available
    pub user_id: Option<Uuid>,
    /// Session ID if available
    pub session_id: Option<String>,
    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,
    /// Span ID for distributed tracing
    pub span_id: Option<String>,
}

/// Error metrics for aggregation and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    /// Total error count
    pub total_errors: u64,
    /// Errors by type
    pub errors_by_type: HashMap<String, u64>,
    /// Errors by component
    pub errors_by_component: HashMap<String, u64>,
    /// Errors by severity
    pub errors_by_severity: HashMap<String, u64>,
    /// Error rate (errors per minute)
    pub error_rate: f64,
    /// Time window for metrics (in seconds)
    pub time_window_seconds: u64,
    /// Timestamp of metrics calculation
    pub calculated_at: DateTime<Utc>,
}

/// Error report containing comprehensive error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    /// Report identifier
    pub report_id: Uuid,
    /// Error response
    pub error_response: ErrorResponse,
    /// Error correlation data
    pub correlation: ErrorCorrelation,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp when report was created
    pub created_at: DateTime<Utc>,
    /// Whether this error has been acknowledged
    pub acknowledged: bool,
    /// Acknowledgment timestamp
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// Acknowledgment user
    pub acknowledged_by: Option<String>,
}

/// Configuration for error reporting service
#[derive(Debug, Clone)]
pub struct ErrorReportingConfig {
    /// Maximum number of error reports to keep in memory
    pub max_reports_in_memory: usize,
    /// Time window for error pattern detection (in seconds)
    pub pattern_detection_window_seconds: u64,
    /// Minimum occurrences to consider as a pattern
    pub min_pattern_occurrences: u64,
    /// Maximum time between occurrences for pattern detection (in seconds)
    pub max_pattern_interval_seconds: u64,
    /// Whether to enable real-time alerting
    pub enable_real_time_alerting: bool,
    /// Alert thresholds by severity
    pub alert_thresholds: HashMap<ErrorSeverity, u64>,
    /// Whether to enable error correlation
    pub enable_error_correlation: bool,
    /// Maximum correlation chain length
    pub max_correlation_chain_length: usize,
}

impl Default for ErrorReportingConfig {
    fn default() -> Self {
        let mut alert_thresholds = HashMap::new();
        alert_thresholds.insert(ErrorSeverity::Critical, 1);
        alert_thresholds.insert(ErrorSeverity::High, 5);
        alert_thresholds.insert(ErrorSeverity::Medium, 10);
        alert_thresholds.insert(ErrorSeverity::Low, 50);
        alert_thresholds.insert(ErrorSeverity::Info, 100);

        Self {
            max_reports_in_memory: 10000,
            pattern_detection_window_seconds: 3600, // 1 hour
            min_pattern_occurrences: 3,
            max_pattern_interval_seconds: 1800, // 30 minutes
            enable_real_time_alerting: true,
            alert_thresholds,
            enable_error_correlation: true,
            max_correlation_chain_length: 10,
        }
    }
}

/// Trait for error reporting service
#[async_trait]
pub trait ErrorReportingService: Send + Sync {
    /// Report an error with context
    async fn report_error(&self, error_response: ErrorResponse) -> Result<Uuid>;
    
    /// Get error metrics for a time window
    async fn get_error_metrics(&self, time_window_seconds: u64) -> Result<ErrorMetrics>;
    
    /// Detect error patterns
    async fn detect_error_patterns(&self) -> Result<Vec<ErrorPattern>>;
    
    /// Get error correlation data
    async fn get_error_correlation(&self, correlation_id: Uuid) -> Result<Option<ErrorCorrelation>>;
    
    /// Acknowledge an error report
    async fn acknowledge_error(&self, report_id: Uuid, acknowledged_by: String) -> Result<()>;
    
    /// Get recent error reports
    async fn get_recent_reports(&self, limit: usize) -> Result<Vec<ErrorReport>>;
    
    /// Get error reports by pattern
    async fn get_reports_by_pattern(&self, pattern_id: Uuid) -> Result<Vec<ErrorReport>>;
}

/// Default implementation of error reporting service
#[derive(Debug)]
pub struct DefaultErrorReportingService {
    /// Configuration
    config: ErrorReportingConfig,
    /// Error reports storage
    reports: Arc<RwLock<Vec<ErrorReport>>>,
    /// Error patterns storage
    patterns: Arc<RwLock<HashMap<String, ErrorPattern>>>,
    /// Error correlations storage
    correlations: Arc<RwLock<HashMap<Uuid, ErrorCorrelation>>>,
    /// Error metrics cache
    metrics_cache: Arc<RwLock<Option<(ErrorMetrics, DateTime<Utc>)>>>,
}

impl DefaultErrorReportingService {
    /// Create a new error reporting service
    pub fn new() -> Self {
        Self::with_config(ErrorReportingConfig::default())
    }

    /// Create a new error reporting service with custom configuration
    pub fn with_config(config: ErrorReportingConfig) -> Self {
        Self {
            config,
            reports: Arc::new(RwLock::new(Vec::new())),
            patterns: Arc::new(RwLock::new(HashMap::new())),
            correlations: Arc::new(RwLock::new(HashMap::new())),
            metrics_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Create error correlation data from context
    fn create_correlation(&self, context: &ErrorContext) -> ErrorCorrelation {
        ErrorCorrelation {
            correlation_id: context.correlation_id,
            related_correlation_ids: context.parent_correlation_id.into_iter().collect(),
            request_id: context.request_id.clone(),
            user_id: context.user_id,
            session_id: None, // Could be extracted from context if available
            trace_id: None,   // Could be extracted from tracing context
            span_id: None,    // Could be extracted from tracing context
        }
    }

    /// Generate pattern key for error pattern detection
    fn generate_pattern_key(&self, error_response: &ErrorResponse) -> String {
        format!(
            "{}:{}:{}",
            error_response.error_type,
            error_response.context.component,
            error_response.context.operation
        )
    }

    /// Update error patterns
    async fn update_patterns(&self, error_response: &ErrorResponse) -> Result<()> {
        let pattern_key = self.generate_pattern_key(error_response);
        let mut patterns = self.patterns.write().await;
        
        let now = Utc::now();
        
        if let Some(pattern) = patterns.get_mut(&pattern_key) {
            // Update existing pattern
            pattern.occurrence_count += 1;
            pattern.last_occurrence = now;
            
            // Calculate average interval
            let total_duration = pattern.last_occurrence
                .signed_duration_since(pattern.first_occurrence)
                .num_seconds() as f64;
            pattern.average_interval_seconds = total_duration / (pattern.occurrence_count - 1) as f64;
            
            // Check if pattern should be considered active
            let time_since_last = now
                .signed_duration_since(pattern.last_occurrence)
                .num_seconds() as u64;
            pattern.is_active = time_since_last <= self.config.max_pattern_interval_seconds;
            
            info!(
                pattern_id = %pattern.pattern_id,
                pattern_key = %pattern_key,
                occurrence_count = pattern.occurrence_count,
                "Updated error pattern"
            );
        } else {
            // Create new pattern
            let pattern = ErrorPattern {
                pattern_id: Uuid::new_v4(),
                error_type: error_response.error_type.clone(),
                component_pattern: error_response.context.component.clone(),
                operation_pattern: error_response.context.operation.clone(),
                occurrence_count: 1,
                first_occurrence: now,
                last_occurrence: now,
                average_interval_seconds: 0.0,
                severity: error_response.severity.clone(),
                is_active: true,
            };
            
            info!(
                pattern_id = %pattern.pattern_id,
                pattern_key = %pattern_key,
                "Created new error pattern"
            );
            
            patterns.insert(pattern_key, pattern);
        }
        
        Ok(())
    }

    /// Check if error should trigger an alert
    async fn should_trigger_alert(&self, error_response: &ErrorResponse) -> bool {
        if !self.config.enable_real_time_alerting {
            return false;
        }
        
        if let Some(threshold) = self.config.alert_thresholds.get(&error_response.severity) {
            let pattern_key = self.generate_pattern_key(error_response);
            let patterns = self.patterns.read().await;
            
            if let Some(pattern) = patterns.get(&pattern_key) {
                return pattern.occurrence_count >= *threshold;
            }
        }
        
        false
    }

    /// Trigger alert for error
    async fn trigger_alert(&self, error_response: &ErrorResponse, pattern: &ErrorPattern) {
        warn!(
            error_id = %error_response.error_id,
            pattern_id = %pattern.pattern_id,
            severity = ?error_response.severity,
            occurrence_count = pattern.occurrence_count,
            error_type = %error_response.error_type,
            component = %error_response.context.component,
            operation = %error_response.context.operation,
            "Error alert triggered"
        );
        
        // In a real implementation, this would send alerts to external systems
        // like PagerDuty, Slack, email, etc.
    }

    /// Clean up old reports to maintain memory limits
    async fn cleanup_old_reports(&self) -> Result<()> {
        let mut reports = self.reports.write().await;
        
        if reports.len() > self.config.max_reports_in_memory {
            let excess = reports.len() - self.config.max_reports_in_memory;
            reports.drain(0..excess);
            
            info!(
                removed_reports = excess,
                remaining_reports = reports.len(),
                "Cleaned up old error reports"
            );
        }
        
        Ok(())
    }

    /// Calculate error metrics
    async fn calculate_metrics(&self, time_window_seconds: u64) -> Result<ErrorMetrics> {
        let reports = self.reports.read().await;
        let now = Utc::now();
        let cutoff_time = now - chrono::Duration::seconds(time_window_seconds as i64);
        
        let recent_reports: Vec<_> = reports
            .iter()
            .filter(|report| report.created_at >= cutoff_time)
            .collect();
        
        let total_errors = recent_reports.len() as u64;
        
        let mut errors_by_type = HashMap::new();
        let mut errors_by_component = HashMap::new();
        let mut errors_by_severity = HashMap::new();
        
        for report in &recent_reports {
            *errors_by_type.entry(report.error_response.error_type.clone()).or_insert(0) += 1;
            *errors_by_component.entry(report.error_response.context.component.clone()).or_insert(0) += 1;
            
            let severity_key = format!("{:?}", report.error_response.severity);
            *errors_by_severity.entry(severity_key).or_insert(0) += 1;
        }
        
        let error_rate = if time_window_seconds > 0 {
            (total_errors as f64) / (time_window_seconds as f64 / 60.0) // errors per minute
        } else {
            0.0
        };
        
        Ok(ErrorMetrics {
            total_errors,
            errors_by_type,
            errors_by_component,
            errors_by_severity,
            error_rate,
            time_window_seconds,
            calculated_at: now,
        })
    }
}

#[async_trait]
impl ErrorReportingService for DefaultErrorReportingService {
    async fn report_error(&self, error_response: ErrorResponse) -> Result<Uuid> {
        let report_id = Uuid::new_v4();
        let correlation = self.create_correlation(&error_response.context);
        
        // Update patterns
        self.update_patterns(&error_response).await?;
        
        // Check for alerts
        if self.should_trigger_alert(&error_response).await {
            let pattern_key = self.generate_pattern_key(&error_response);
            let patterns = self.patterns.read().await;
            if let Some(pattern) = patterns.get(&pattern_key) {
                self.trigger_alert(&error_response, pattern).await;
            }
        }
        
        // Store correlation
        if self.config.enable_error_correlation {
            let mut correlations = self.correlations.write().await;
            correlations.insert(correlation.correlation_id, correlation.clone());
        }
        
        // Create error report
        let report = ErrorReport {
            report_id,
            error_response: error_response.clone(),
            correlation: correlation.clone(),
            metadata: HashMap::new(),
            created_at: Utc::now(),
            acknowledged: false,
            acknowledged_at: None,
            acknowledged_by: None,
        };
        
        // Store report
        let mut reports = self.reports.write().await;
        reports.push(report.clone());
        
        // Clean up old reports
        drop(reports);
        self.cleanup_old_reports().await?;
        
        // Invalidate metrics cache
        let mut cache = self.metrics_cache.write().await;
        *cache = None;
        
        info!(
            report_id = %report_id,
            correlation_id = %correlation.correlation_id,
            error_type = %report.error_response.error_type,
            "Error reported"
        );
        
        Ok(report_id)
    }

    async fn get_error_metrics(&self, time_window_seconds: u64) -> Result<ErrorMetrics> {
        // Check cache first
        let cache = self.metrics_cache.read().await;
        if let Some((cached_metrics, cached_at)) = cache.as_ref() {
            let cache_age = Utc::now().signed_duration_since(*cached_at).num_seconds();
            if cache_age < 60 && cached_metrics.time_window_seconds == time_window_seconds {
                return Ok(cached_metrics.clone());
            }
        }
        drop(cache);
        
        // Calculate new metrics
        let metrics = self.calculate_metrics(time_window_seconds).await?;
        
        // Update cache
        let mut cache = self.metrics_cache.write().await;
        *cache = Some((metrics.clone(), Utc::now()));
        
        Ok(metrics)
    }

    async fn detect_error_patterns(&self) -> Result<Vec<ErrorPattern>> {
        let patterns = self.patterns.read().await;
        let mut result = Vec::new();
        
        for pattern in patterns.values() {
            if pattern.occurrence_count >= self.config.min_pattern_occurrences {
                result.push(pattern.clone());
            }
        }
        
        // Sort by occurrence count (descending)
        result.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));
        
        Ok(result)
    }

    async fn get_error_correlation(&self, correlation_id: Uuid) -> Result<Option<ErrorCorrelation>> {
        let correlations = self.correlations.read().await;
        Ok(correlations.get(&correlation_id).cloned())
    }

    async fn acknowledge_error(&self, report_id: Uuid, acknowledged_by: String) -> Result<()> {
        let mut reports = self.reports.write().await;
        
        for report in reports.iter_mut() {
            if report.report_id == report_id {
                report.acknowledged = true;
                report.acknowledged_at = Some(Utc::now());
                report.acknowledged_by = Some(acknowledged_by.clone());
                
                info!(
                    report_id = %report_id,
                    acknowledged_by = %acknowledged_by,
                    "Error report acknowledged"
                );
                
                return Ok(());
            }
        }
        
        Err(AppError::NotFound(format!("Error report not found: {}", report_id)))
    }

    async fn get_recent_reports(&self, limit: usize) -> Result<Vec<ErrorReport>> {
        let reports = self.reports.read().await;
        let mut recent_reports: Vec<_> = reports.iter().cloned().collect();
        
        // Sort by creation time (most recent first)
        recent_reports.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        // Limit results
        recent_reports.truncate(limit);
        
        Ok(recent_reports)
    }

    async fn get_reports_by_pattern(&self, pattern_id: Uuid) -> Result<Vec<ErrorReport>> {
        let reports = self.reports.read().await;
        let patterns = self.patterns.read().await;
        
        // Find the pattern
        let target_pattern = patterns
            .values()
            .find(|p| p.pattern_id == pattern_id)
            .ok_or_else(|| AppError::NotFound(format!("Pattern not found: {}", pattern_id)))?;
        
        // Find matching reports
        let matching_reports: Vec<_> = reports
            .iter()
            .filter(|report| {
                report.error_response.error_type == target_pattern.error_type
                    && report.error_response.context.component == target_pattern.component_pattern
                    && report.error_response.context.operation == target_pattern.operation_pattern
            })
            .cloned()
            .collect();
        
        Ok(matching_reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_error_reporting_service_creation() {
        let service = DefaultErrorReportingService::new();
        assert!(service.config.enable_real_time_alerting);
    }

    #[tokio::test]
    async fn test_error_report_creation() {
        let service = DefaultErrorReportingService::new();
        let context = ErrorContext::new("test_component", "test_operation");
        let error_response = crate::error::ErrorResponse::new(
            "TestError",
            "Test error message",
            context,
            ErrorSeverity::Medium,
        );
        
        let report_id = service.report_error(error_response).await.unwrap();
        assert!(!report_id.is_nil());
        
        let reports = service.get_recent_reports(10).await.unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].report_id, report_id);
    }

    #[tokio::test]
    async fn test_error_pattern_detection() {
        let service = DefaultErrorReportingService::new();
        let context = ErrorContext::new("test_component", "test_operation");
        
        // Report the same error multiple times
        for _ in 0..5 {
            let error_response = crate::error::ErrorResponse::new(
                "TestError",
                "Test error message",
                context.clone(),
                ErrorSeverity::Medium,
            );
            service.report_error(error_response).await.unwrap();
        }
        
        let patterns = service.detect_error_patterns().await.unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].occurrence_count, 5);
        assert_eq!(patterns[0].error_type, "TestError");
    }

    #[tokio::test]
    async fn test_error_metrics_calculation() {
        let service = DefaultErrorReportingService::new();
        let context = ErrorContext::new("test_component", "test_operation");
        
        // Report different types of errors
        for i in 0..3 {
            let error_response = crate::error::ErrorResponse::new(
                format!("TestError{}", i),
                "Test error message",
                context.clone(),
                ErrorSeverity::Medium,
            );
            service.report_error(error_response).await.unwrap();
        }
        
        let metrics = service.get_error_metrics(3600).await.unwrap();
        assert_eq!(metrics.total_errors, 3);
        assert_eq!(metrics.errors_by_type.len(), 3);
        assert!(metrics.error_rate > 0.0);
    }

    #[tokio::test]
    async fn test_error_acknowledgment() {
        let service = DefaultErrorReportingService::new();
        let context = ErrorContext::new("test_component", "test_operation");
        let error_response = crate::error::ErrorResponse::new(
            "TestError",
            "Test error message",
            context,
            ErrorSeverity::Medium,
        );
        
        let report_id = service.report_error(error_response).await.unwrap();
        
        service.acknowledge_error(report_id, "test_user".to_string()).await.unwrap();
        
        let reports = service.get_recent_reports(10).await.unwrap();
        assert!(reports[0].acknowledged);
        assert_eq!(reports[0].acknowledged_by, Some("test_user".to_string()));
    }

    #[tokio::test]
    async fn test_error_correlation() {
        let service = DefaultErrorReportingService::new();
        let context = ErrorContext::new("test_component", "test_operation")
            .with_request_id("req-123");
        
        let error_response = crate::error::ErrorResponse::new(
            "TestError",
            "Test error message",
            context.clone(),
            ErrorSeverity::Medium,
        );
        
        service.report_error(error_response).await.unwrap();
        
        let correlation = service.get_error_correlation(context.correlation_id).await.unwrap();
        assert!(correlation.is_some());
        assert_eq!(correlation.unwrap().request_id, Some("req-123".to_string()));
    }
}