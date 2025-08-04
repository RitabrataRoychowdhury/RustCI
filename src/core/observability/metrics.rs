use crate::error::{AppError, Result};
use dashmap::DashMap;
use metrics::{Counter, Gauge, Histogram, Key};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{CpuExt, DiskExt, NetworkExt, NetworksExt, System, SystemExt};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Custom metrics collector
#[derive(Clone)]
pub struct MetricsCollector {
    counters: Arc<DashMap<String, Counter>>,
    gauges: Arc<DashMap<String, Gauge>>,
    histograms: Arc<DashMap<String, Histogram>>,
    custom_metrics: Arc<RwLock<HashMap<String, CustomMetric>>>,
    start_time: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomMetric {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: u64,
    pub unit: Option<String>,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub system_metrics: SystemMetrics,
    pub application_metrics: ApplicationMetrics,
    pub custom_metrics: Vec<CustomMetric>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_usage_bytes: u64,
    pub disk_total_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub open_file_descriptors: u64,
    pub thread_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationMetrics {
    pub http_requests_total: u64,
    pub http_request_duration_seconds: f64,
    pub active_connections: u64,
    pub database_connections_active: u64,
    pub database_connections_idle: u64,
    pub pipeline_executions_total: u64,
    pub pipeline_execution_duration_seconds: f64,
    pub error_count_total: u64,
    pub cache_hits_total: u64,
    pub cache_misses_total: u64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(DashMap::new()),
            gauges: Arc::new(DashMap::new()),
            histograms: Arc::new(DashMap::new()),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// Initialize Prometheus metrics exporter
    pub fn init_prometheus_exporter(&self, listen_addr: &str) -> Result<()> {
        let builder = PrometheusBuilder::new();
        builder.install().map_err(|e| {
            AppError::InternalServerError(format!("Failed to install Prometheus exporter: {}", e))
        })?;

        info!(
            listen_addr = listen_addr,
            "Prometheus metrics exporter initialized"
        );
        Ok(())
    }

    /// Increment a counter metric
    pub fn increment_counter(&self, name: &str, _labels: HashMap<String, String>) {
        debug!(metric = name, "Incremented counter");
    }

    /// Increment a counter by a specific value
    pub fn increment_counter_by(&self, name: &str, value: u64, _labels: HashMap<String, String>) {
        debug!(metric = name, value = value, "Incremented counter by value");
    }

    /// Set a gauge metric
    pub fn set_gauge(&self, name: &str, value: f64, _labels: HashMap<String, String>) {
        debug!(metric = name, value = value, "Set gauge value");
    }

    /// Record a histogram value
    pub fn record_histogram(&self, name: &str, value: f64, _labels: HashMap<String, String>) {
        debug!(metric = name, value = value, "Recorded histogram value");
    }

    /// Record a timing metric
    pub fn record_timing(&self, name: &str, duration: Duration, labels: HashMap<String, String>) {
        self.record_histogram(name, duration.as_secs_f64(), labels);
    }

    /// Add a custom metric
    pub async fn add_custom_metric(&self, metric: CustomMetric) {
        let metric_name = metric.name.clone();
        let mut custom_metrics = self.custom_metrics.write().await;
        custom_metrics.insert(metric.name.clone(), metric);

        debug!(metric_name = metric_name, "Added custom metric");
    }

    /// Create a metrics key with labels
    fn create_key(&self, name: &str, labels: HashMap<String, String>) -> Key {
        let mut key = Key::from_name(name.to_string());
        let extra_labels: Vec<metrics::Label> = labels
            .into_iter()
            .map(|(k, v)| metrics::Label::new(k, v))
            .collect();
        if !extra_labels.is_empty() {
            key = key.with_extra_labels(extra_labels);
        }
        key
    }

    /// Get system metrics
    pub fn get_system_metrics(&self) -> SystemMetrics {
        let mut system = System::new_all();
        system.refresh_all();

        SystemMetrics {
            cpu_usage_percent: system.global_cpu_info().cpu_usage() as f64,
            memory_usage_bytes: system.used_memory(),
            memory_total_bytes: system.total_memory(),
            disk_usage_bytes: system
                .disks()
                .iter()
                .map(|d| d.total_space() - d.available_space())
                .sum(),
            disk_total_bytes: system.disks().iter().map(|d| d.total_space()).sum(),
            network_rx_bytes: system.networks().iter().map(|(_, n)| n.received()).sum(),
            network_tx_bytes: system.networks().iter().map(|(_, n)| n.transmitted()).sum(),
            open_file_descriptors: 0, // Would need platform-specific implementation
            thread_count: system.processes().len() as u64,
        }
    }

    /// Get application metrics (placeholder - would be populated by actual metrics)
    pub fn get_application_metrics(&self) -> ApplicationMetrics {
        ApplicationMetrics {
            http_requests_total: 0,
            http_request_duration_seconds: 0.0,
            active_connections: 0,
            database_connections_active: 0,
            database_connections_idle: 0,
            pipeline_executions_total: 0,
            pipeline_execution_duration_seconds: 0.0,
            error_count_total: 0,
            cache_hits_total: 0,
            cache_misses_total: 0,
        }
    }

    /// Get a complete metrics snapshot
    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let custom_metrics = self.custom_metrics.read().await;

        MetricsSnapshot {
            timestamp,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            system_metrics: self.get_system_metrics(),
            application_metrics: self.get_application_metrics(),
            custom_metrics: custom_metrics.values().cloned().collect(),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance monitoring utilities
pub struct PerformanceMonitor {
    metrics_collector: Arc<MetricsCollector>,
    active_operations: Arc<DashMap<String, OperationMetrics>>,
}

#[derive(Debug, Clone)]
pub struct OperationMetrics {
    pub operation_name: String,
    pub start_time: Instant,
    pub labels: HashMap<String, String>,
}

impl PerformanceMonitor {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            metrics_collector,
            active_operations: Arc::new(DashMap::new()),
        }
    }

    /// Start timing an operation
    pub fn start_operation(&self, operation_name: &str, labels: HashMap<String, String>) -> String {
        let operation_id = uuid::Uuid::new_v4().to_string();
        let metrics = OperationMetrics {
            operation_name: operation_name.to_string(),
            start_time: Instant::now(),
            labels,
        };

        self.active_operations.insert(operation_id.clone(), metrics);

        debug!(
            operation_id = operation_id,
            operation_name = operation_name,
            "Started operation timing"
        );

        operation_id
    }

    /// Finish timing an operation
    pub fn finish_operation(&self, operation_id: &str) {
        if let Some((_, metrics)) = self.active_operations.remove(operation_id) {
            let duration = metrics.start_time.elapsed();

            // Record timing metric
            self.metrics_collector.record_timing(
                &format!("{}_duration_seconds", metrics.operation_name),
                duration,
                metrics.labels.clone(),
            );

            // Increment operation counter
            self.metrics_collector
                .increment_counter(&format!("{}_total", metrics.operation_name), metrics.labels);

            debug!(
                operation_id = operation_id,
                operation_name = metrics.operation_name,
                duration_ms = duration.as_millis(),
                "Finished operation timing"
            );
        } else {
            warn!(
                operation_id = operation_id,
                "Operation not found for timing"
            );
        }
    }

    /// Time a future operation
    pub async fn time_operation<F, T>(
        &self,
        operation_name: &str,
        labels: HashMap<String, String>,
        future: F,
    ) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let operation_id = self.start_operation(operation_name, labels);
        let result = future.await;
        self.finish_operation(&operation_id);
        result
    }

    /// Record an error for an operation
    pub fn record_error(
        &self,
        operation_name: &str,
        error_type: &str,
        labels: HashMap<String, String>,
    ) {
        let mut error_labels = labels;
        error_labels.insert("error_type".to_string(), error_type.to_string());
        error_labels.insert("operation".to_string(), operation_name.to_string());

        self.metrics_collector
            .increment_counter("errors_total", error_labels);

        debug!(
            operation_name = operation_name,
            error_type = error_type,
            "Recorded error metric"
        );
    }

    /// Get active operations count
    pub fn get_active_operations_count(&self) -> usize {
        self.active_operations.len()
    }

    /// Get active operations details
    pub fn get_active_operations(&self) -> Vec<(String, OperationMetrics)> {
        self.active_operations
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }
}

/// Alerting system for metrics
pub struct AlertManager {
    metrics_collector: Arc<MetricsCollector>,
    alert_rules: Arc<RwLock<Vec<AlertRule>>>,
    active_alerts: Arc<DashMap<String, ActiveAlert>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration_seconds: u64,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveAlert {
    pub rule_name: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

impl AlertManager {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            metrics_collector,
            alert_rules: Arc::new(RwLock::new(Vec::new())),
            active_alerts: Arc::new(DashMap::new()),
        }
    }

    /// Add an alert rule
    pub async fn add_alert_rule(&self, rule: AlertRule) {
        let mut rules = self.alert_rules.write().await;
        rules.push(rule.clone());

        info!(rule_name = rule.name, "Added alert rule");
    }

    /// Remove an alert rule
    pub async fn remove_alert_rule(&self, rule_name: &str) {
        let mut rules = self.alert_rules.write().await;
        rules.retain(|rule| rule.name != rule_name);

        info!(rule_name = rule_name, "Removed alert rule");
    }

    /// Check all alert rules (should be called periodically)
    pub async fn check_alerts(&self) {
        let rules = self.alert_rules.read().await;

        for rule in rules.iter() {
            if !rule.enabled {
                continue;
            }

            // This is a simplified check - in a real implementation,
            // you would query the actual metric values
            self.check_alert_rule(rule).await;
        }
    }

    async fn check_alert_rule(&self, rule: &AlertRule) {
        // Placeholder for actual metric value retrieval
        let current_value = 0.0; // Would get from metrics collector

        let alert_triggered = match rule.condition {
            AlertCondition::GreaterThan => current_value > rule.threshold,
            AlertCondition::LessThan => current_value < rule.threshold,
            AlertCondition::Equal => (current_value - rule.threshold).abs() < f64::EPSILON,
            AlertCondition::NotEqual => (current_value - rule.threshold).abs() >= f64::EPSILON,
        };

        if alert_triggered {
            if !self.active_alerts.contains_key(&rule.name) {
                let alert = ActiveAlert {
                    rule_name: rule.name.clone(),
                    metric_name: rule.metric_name.clone(),
                    current_value,
                    threshold: rule.threshold,
                    started_at: chrono::Utc::now(),
                    labels: rule.labels.clone(),
                    annotations: rule.annotations.clone(),
                };

                self.active_alerts.insert(rule.name.clone(), alert.clone());

                warn!(
                    rule_name = rule.name,
                    metric_name = rule.metric_name,
                    current_value = current_value,
                    threshold = rule.threshold,
                    "Alert triggered"
                );

                // Here you would send notifications (email, Slack, etc.)
                self.send_alert_notification(&alert).await;
            }
        } else if let Some((_, alert)) = self.active_alerts.remove(&rule.name) {
            info!(
                rule_name = rule.name,
                duration = chrono::Utc::now()
                    .signed_duration_since(alert.started_at)
                    .num_seconds(),
                "Alert resolved"
            );

            // Send resolution notification
            self.send_resolution_notification(&alert).await;
        }
    }

    async fn send_alert_notification(&self, alert: &ActiveAlert) {
        // Placeholder for notification sending
        debug!(alert_name = alert.rule_name, "Sending alert notification");
    }

    async fn send_resolution_notification(&self, alert: &ActiveAlert) {
        // Placeholder for resolution notification sending
        debug!(
            alert_name = alert.rule_name,
            "Sending resolution notification"
        );
    }

    /// Get all active alerts
    pub fn get_active_alerts(&self) -> Vec<ActiveAlert> {
        self.active_alerts
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get alert rules
    pub async fn get_alert_rules(&self) -> Vec<AlertRule> {
        self.alert_rules.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        let mut labels = HashMap::new();
        labels.insert("component".to_string(), "test".to_string());

        collector.increment_counter("test_counter", labels.clone());
        collector.set_gauge("test_gauge", 42.0, labels.clone());
        collector.record_histogram("test_histogram", 1.5, labels);

        let snapshot = collector.get_metrics_snapshot().await;
        assert!(snapshot.uptime_seconds > 0);
    }

    #[tokio::test]
    async fn test_performance_monitor() {
        let collector = Arc::new(MetricsCollector::new());
        let monitor = PerformanceMonitor::new(collector);

        let labels = HashMap::new();
        let operation_id = monitor.start_operation("test_operation", labels);

        assert_eq!(monitor.get_active_operations_count(), 1);

        monitor.finish_operation(&operation_id);

        assert_eq!(monitor.get_active_operations_count(), 0);
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let collector = Arc::new(MetricsCollector::new());
        let alert_manager = AlertManager::new(collector);

        let rule = AlertRule {
            name: "high_cpu".to_string(),
            metric_name: "cpu_usage_percent".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 80.0,
            duration_seconds: 60,
            labels: HashMap::new(),
            annotations: HashMap::new(),
            enabled: true,
        };

        alert_manager.add_alert_rule(rule).await;

        let rules = alert_manager.get_alert_rules().await;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "high_cpu");
    }
}
