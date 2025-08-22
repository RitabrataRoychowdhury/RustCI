//! Real-time Metrics Collector for Performance Monitoring
//!
//! This module provides comprehensive real-time metrics collection with
//! performance trend analysis and predictive insights for Valkyrie Protocol.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::unified_benchmark_engine::BenchmarkError;

/// Real-time metrics collector with trend analysis
pub struct RealTimeMetricsCollector {
    config: MetricsConfig,
    metrics_store: Arc<RwLock<MetricsStore>>,
    collection_handle: Option<tokio::task::JoinHandle<()>>,
    trend_analyzer: Arc<TrendAnalyzer>,
    alert_manager: Arc<AlertManager>,
}

/// Configuration for metrics collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Collection interval in milliseconds
    pub collection_interval_ms: u64,
    /// Maximum number of data points to retain
    pub max_data_points: usize,
    /// Enable trend analysis
    pub enable_trend_analysis: bool,
    /// Enable predictive insights
    pub enable_predictive_insights: bool,
    /// Metrics to collect
    pub collected_metrics: Vec<MetricDefinition>,
    /// Alert thresholds
    pub alert_thresholds: HashMap<String, AlertThreshold>,
}

/// Definition of a metric to collect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub name: String,
    pub metric_type: MetricType,
    pub collection_method: CollectionMethod,
    pub aggregation_window: Duration,
    pub retention_period: Duration,
}

/// Type of metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Timer,
}

/// Method for collecting the metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CollectionMethod {
    SystemStats,
    ProcessStats,
    NetworkStats,
    CustomFunction(String),
}

/// Alert threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThreshold {
    pub warning_threshold: f64,
    pub critical_threshold: f64,
    pub comparison: ThresholdComparison,
    pub duration: Duration,
}

/// Threshold comparison type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThresholdComparison {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

/// Metrics storage with time-series data
pub struct MetricsStore {
    data_points: HashMap<String, Vec<DataPoint>>,
    aggregated_metrics: HashMap<String, AggregatedMetric>,
    last_cleanup: Instant,
}

/// Individual data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub tags: HashMap<String, String>,
}

/// Aggregated metric with statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    pub name: String,
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub percentiles: HashMap<String, f64>,
    pub last_updated: DateTime<Utc>,
}

/// Trend analyzer for predictive insights
pub struct TrendAnalyzer {
    config: TrendAnalysisConfig,
}

/// Configuration for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysisConfig {
    pub analysis_window_size: usize,
    pub prediction_horizon_minutes: u32,
    pub trend_detection_sensitivity: f64,
    pub seasonal_analysis_enabled: bool,
}

/// Trend analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysisResult {
    pub metric_name: String,
    pub trend_direction: TrendDirection,
    pub trend_strength: f64,
    pub prediction: Option<MetricPrediction>,
    pub anomalies: Vec<AnomalyDetection>,
    pub seasonal_patterns: Vec<SeasonalPattern>,
}

/// Trend direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

/// Metric prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPrediction {
    pub predicted_value: f64,
    pub confidence_interval: (f64, f64),
    pub confidence_level: f64,
    pub prediction_time: DateTime<Utc>,
}

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetection {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub expected_value: f64,
    pub deviation_score: f64,
    pub anomaly_type: AnomalyType,
}

/// Type of anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    Spike,
    Drop,
    Drift,
    Outlier,
}

/// Seasonal pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalPattern {
    pub pattern_name: String,
    pub period_minutes: u32,
    pub amplitude: f64,
    pub phase_offset: f64,
    pub confidence: f64,
}

/// Alert manager for threshold monitoring
pub struct AlertManager {
    config: AlertConfig,
    active_alerts: Arc<RwLock<HashMap<String, ActiveAlert>>>,
    notification_sender: mpsc::UnboundedSender<AlertNotification>,
}

/// Alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub enable_alerts: bool,
    pub notification_channels: Vec<NotificationChannel>,
    pub alert_cooldown_minutes: u32,
    pub escalation_rules: Vec<EscalationRule>,
}

/// Notification channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Console,
    Email { recipients: Vec<String> },
    Slack { webhook_url: String },
    Webhook { url: String, headers: HashMap<String, String> },
}

/// Escalation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    pub severity: AlertSeverity,
    pub duration_minutes: u32,
    pub escalation_channels: Vec<NotificationChannel>,
}

/// Active alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAlert {
    pub alert_id: String,
    pub metric_name: String,
    pub severity: AlertSeverity,
    pub threshold_value: f64,
    pub current_value: f64,
    pub started_at: DateTime<Utc>,
    pub last_triggered: DateTime<Utc>,
    pub trigger_count: u32,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertNotification {
    pub alert: ActiveAlert,
    pub notification_type: NotificationType,
    pub channels: Vec<NotificationChannel>,
}

/// Notification type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    Triggered,
    Resolved,
    Escalated,
}

/// Real-time performance dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDashboard {
    pub timestamp: DateTime<Utc>,
    pub overall_health: HealthStatus,
    pub key_metrics: HashMap<String, f64>,
    pub trend_summaries: Vec<TrendSummary>,
    pub active_alerts: Vec<ActiveAlert>,
    pub predictions: Vec<MetricPrediction>,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// Trend summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendSummary {
    pub metric_name: String,
    pub current_value: f64,
    pub trend_direction: TrendDirection,
    pub change_percentage: f64,
    pub time_window: String,
}

impl RealTimeMetricsCollector {
    /// Create a new real-time metrics collector
    pub fn new() -> Self {
        let config = MetricsConfig::default();
        let metrics_store = Arc::new(RwLock::new(MetricsStore::new()));
        let trend_analyzer = Arc::new(TrendAnalyzer::new(TrendAnalysisConfig::default()));
        let alert_manager = Arc::new(AlertManager::new(AlertConfig::default()));

        Self {
            config,
            metrics_store,
            collection_handle: None,
            trend_analyzer,
            alert_manager,
        }
    }

    /// Start metrics collection
    pub async fn start_collection(&mut self) -> Result<(), BenchmarkError> {
        if self.collection_handle.is_some() {
            return Ok(()); // Already running
        }

        println!("📊 Starting real-time metrics collection...");

        let metrics_store = Arc::clone(&self.metrics_store);
        let trend_analyzer = Arc::clone(&self.trend_analyzer);
        let alert_manager = Arc::clone(&self.alert_manager);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.collection_interval_ms));
            
            loop {
                interval.tick().await;
                
                // Collect metrics
                if let Err(e) = Self::collect_metrics_iteration(&metrics_store, &config).await {
                    eprintln!("Error collecting metrics: {}", e);
                }

                // Perform trend analysis
                if config.enable_trend_analysis {
                    if let Err(e) = Self::perform_trend_analysis(&metrics_store, &trend_analyzer).await {
                        eprintln!("Error in trend analysis: {}", e);
                    }
                }

                // Check alert thresholds
                if let Err(e) = Self::check_alert_thresholds(&metrics_store, &alert_manager, &config).await {
                    eprintln!("Error checking alerts: {}", e);
                }

                // Cleanup old data
                if let Err(e) = Self::cleanup_old_data(&metrics_store, &config).await {
                    eprintln!("Error cleaning up data: {}", e);
                }
            }
        });

        self.collection_handle = Some(handle);
        Ok(())
    }

    /// Stop metrics collection
    pub async fn stop_collection(&mut self) -> Result<(), BenchmarkError> {
        if let Some(handle) = self.collection_handle.take() {
            handle.abort();
            println!("📊 Stopped real-time metrics collection");
        }
        Ok(())
    }

    /// Get current performance dashboard
    pub async fn get_performance_dashboard(&self) -> Result<PerformanceDashboard, BenchmarkError> {
        let store = self.metrics_store.read().await;
        let active_alerts = self.alert_manager.get_active_alerts().await;

        // Calculate overall health
        let overall_health = if active_alerts.iter().any(|a| matches!(a.severity, AlertSeverity::Critical)) {
            HealthStatus::Critical
        } else if active_alerts.iter().any(|a| matches!(a.severity, AlertSeverity::Warning)) {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        // Get key metrics
        let mut key_metrics = HashMap::new();
        for (name, aggregated) in &store.aggregated_metrics {
            key_metrics.insert(name.clone(), aggregated.mean);
        }

        // Generate trend summaries
        let mut trend_summaries = Vec::new();
        for (name, data_points) in &store.data_points {
            if let Some(summary) = self.generate_trend_summary(name, data_points) {
                trend_summaries.push(summary);
            }
        }

        // Get predictions if enabled
        let predictions = if self.config.enable_predictive_insights {
            self.generate_predictions(&store).await
        } else {
            Vec::new()
        };

        Ok(PerformanceDashboard {
            timestamp: Utc::now(),
            overall_health,
            key_metrics,
            trend_summaries,
            active_alerts,
            predictions,
        })
    }

    /// Collect metrics for one iteration
    async fn collect_metrics_iteration(
        metrics_store: &Arc<RwLock<MetricsStore>>,
        config: &MetricsConfig,
    ) -> Result<(), BenchmarkError> {
        let mut store = metrics_store.write().await;
        let timestamp = Utc::now();

        for metric_def in &config.collected_metrics {
            let value = Self::collect_metric_value(metric_def).await?;
            
            let data_point = DataPoint {
                timestamp,
                value,
                tags: HashMap::new(),
            };

            store.add_data_point(metric_def.name.clone(), data_point);
            store.update_aggregated_metric(&metric_def.name);
        }

        Ok(())
    }

    /// Collect value for a specific metric
    async fn collect_metric_value(metric_def: &MetricDefinition) -> Result<f64, BenchmarkError> {
        match &metric_def.collection_method {
            CollectionMethod::SystemStats => {
                // Collect system statistics
                match metric_def.name.as_str() {
                    "cpu_usage_percent" => Ok(Self::get_cpu_usage().await),
                    "memory_usage_percent" => Ok(Self::get_memory_usage().await),
                    "disk_usage_percent" => Ok(Self::get_disk_usage().await),
                    _ => Ok(0.0),
                }
            }
            CollectionMethod::ProcessStats => {
                // Collect process statistics
                match metric_def.name.as_str() {
                    "process_cpu_percent" => Ok(Self::get_process_cpu().await),
                    "process_memory_mb" => Ok(Self::get_process_memory().await),
                    "process_threads" => Ok(Self::get_process_threads().await),
                    _ => Ok(0.0),
                }
            }
            CollectionMethod::NetworkStats => {
                // Collect network statistics
                match metric_def.name.as_str() {
                    "network_bytes_sent" => Ok(Self::get_network_bytes_sent().await),
                    "network_bytes_received" => Ok(Self::get_network_bytes_received().await),
                    "active_connections" => Ok(Self::get_active_connections().await),
                    _ => Ok(0.0),
                }
            }
            CollectionMethod::CustomFunction(_) => {
                // Custom metric collection
                Ok(0.0) // Placeholder
            }
        }
    }

    /// Perform trend analysis
    async fn perform_trend_analysis(
        metrics_store: &Arc<RwLock<MetricsStore>>,
        trend_analyzer: &Arc<TrendAnalyzer>,
    ) -> Result<(), BenchmarkError> {
        let store = metrics_store.read().await;
        
        for (metric_name, data_points) in &store.data_points {
            if data_points.len() >= trend_analyzer.config.analysis_window_size {
                let _analysis = trend_analyzer.analyze_trend(metric_name, data_points).await?;
                // Store or process analysis results
            }
        }

        Ok(())
    }

    /// Check alert thresholds
    async fn check_alert_thresholds(
        metrics_store: &Arc<RwLock<MetricsStore>>,
        alert_manager: &Arc<AlertManager>,
        config: &MetricsConfig,
    ) -> Result<(), BenchmarkError> {
        let store = metrics_store.read().await;
        
        for (metric_name, threshold) in &config.alert_thresholds {
            if let Some(aggregated) = store.aggregated_metrics.get(metric_name) {
                alert_manager.check_threshold(metric_name, aggregated.mean, threshold).await?;
            }
        }

        Ok(())
    }

    /// Cleanup old data
    async fn cleanup_old_data(
        metrics_store: &Arc<RwLock<MetricsStore>>,
        config: &MetricsConfig,
    ) -> Result<(), BenchmarkError> {
        let mut store = metrics_store.write().await;
        store.cleanup_old_data(config.max_data_points);
        Ok(())
    }

    /// Generate trend summary for a metric
    fn generate_trend_summary(&self, name: &str, data_points: &[DataPoint]) -> Option<TrendSummary> {
        if data_points.len() < 2 {
            return None;
        }

        let current_value = data_points.last()?.value;
        let previous_value = data_points[data_points.len() - 2].value;
        
        let change_percentage = if previous_value != 0.0 {
            ((current_value - previous_value) / previous_value) * 100.0
        } else {
            0.0
        };

        let trend_direction = if change_percentage > 1.0 {
            TrendDirection::Increasing
        } else if change_percentage < -1.0 {
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        };

        Some(TrendSummary {
            metric_name: name.to_string(),
            current_value,
            trend_direction,
            change_percentage,
            time_window: "1m".to_string(),
        })
    }

    /// Generate predictions for metrics
    async fn generate_predictions(&self, _store: &MetricsStore) -> Vec<MetricPrediction> {
        // Placeholder implementation
        Vec::new()
    }

    // System metric collection methods (simplified implementations)
    async fn get_cpu_usage() -> f64 { 25.0 }
    async fn get_memory_usage() -> f64 { 60.0 }
    async fn get_disk_usage() -> f64 { 45.0 }
    async fn get_process_cpu() -> f64 { 15.0 }
    async fn get_process_memory() -> f64 { 256.0 }
    async fn get_process_threads() -> f64 { 8.0 }
    async fn get_network_bytes_sent() -> f64 { 1024000.0 }
    async fn get_network_bytes_received() -> f64 { 2048000.0 }
    async fn get_active_connections() -> f64 { 150.0 }
}

impl MetricsStore {
    pub fn new() -> Self {
        Self {
            data_points: HashMap::new(),
            aggregated_metrics: HashMap::new(),
            last_cleanup: Instant::now(),
        }
    }

    pub fn add_data_point(&mut self, metric_name: String, data_point: DataPoint) {
        self.data_points
            .entry(metric_name)
            .or_insert_with(Vec::new)
            .push(data_point);
    }

    pub fn update_aggregated_metric(&mut self, metric_name: &str) {
        if let Some(data_points) = self.data_points.get(metric_name) {
            if data_points.is_empty() {
                return;
            }

            let values: Vec<f64> = data_points.iter().map(|dp| dp.value).collect();
            let count = values.len() as u64;
            let sum: f64 = values.iter().sum();
            let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let mean = sum / count as f64;
            
            let variance = values.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / count as f64;
            let std_dev = variance.sqrt();

            // Calculate percentiles
            let mut sorted_values = values.clone();
            sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let mut percentiles = HashMap::new();
            percentiles.insert("p50".to_string(), Self::percentile(&sorted_values, 0.5));
            percentiles.insert("p95".to_string(), Self::percentile(&sorted_values, 0.95));
            percentiles.insert("p99".to_string(), Self::percentile(&sorted_values, 0.99));

            let aggregated = AggregatedMetric {
                name: metric_name.to_string(),
                count,
                sum,
                min,
                max,
                mean,
                std_dev,
                percentiles,
                last_updated: Utc::now(),
            };

            self.aggregated_metrics.insert(metric_name.to_string(), aggregated);
        }
    }

    pub fn cleanup_old_data(&mut self, max_data_points: usize) {
        for data_points in self.data_points.values_mut() {
            if data_points.len() > max_data_points {
                let excess = data_points.len() - max_data_points;
                data_points.drain(0..excess);
            }
        }
        self.last_cleanup = Instant::now();
    }

    fn percentile(sorted_values: &[f64], percentile: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        
        let index = (percentile * (sorted_values.len() - 1) as f64) as usize;
        sorted_values[index.min(sorted_values.len() - 1)]
    }
}

impl TrendAnalyzer {
    pub fn new(config: TrendAnalysisConfig) -> Self {
        Self { config }
    }

    pub async fn analyze_trend(&self, metric_name: &str, data_points: &[DataPoint]) -> Result<TrendAnalysisResult, BenchmarkError> {
        let values: Vec<f64> = data_points.iter().map(|dp| dp.value).collect();
        
        // Analyze trend direction and strength
        let (trend_direction, trend_strength) = self.calculate_trend(&values);
        
        // Generate prediction if enabled
        let prediction = if self.config.prediction_horizon_minutes > 0 {
            self.generate_prediction(&values)
        } else {
            None
        };

        // Detect anomalies
        let anomalies = self.detect_anomalies(data_points);

        // Detect seasonal patterns if enabled
        let seasonal_patterns = if self.config.seasonal_analysis_enabled {
            self.detect_seasonal_patterns(&values)
        } else {
            Vec::new()
        };

        Ok(TrendAnalysisResult {
            metric_name: metric_name.to_string(),
            trend_direction,
            trend_strength,
            prediction,
            anomalies,
            seasonal_patterns,
        })
    }

    fn calculate_trend(&self, values: &[f64]) -> (TrendDirection, f64) {
        if values.len() < 2 {
            return (TrendDirection::Stable, 0.0);
        }

        // Simple linear regression to determine trend
        let n = values.len() as f64;
        let x_values: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
        
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(values.iter()).map(|(x, y)| x * y).sum();
        let sum_x_squared: f64 = x_values.iter().map(|x| x * x).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x_squared - sum_x * sum_x);
        let strength = slope.abs();

        let direction = if slope > self.config.trend_detection_sensitivity {
            TrendDirection::Increasing
        } else if slope < -self.config.trend_detection_sensitivity {
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        };

        (direction, strength)
    }

    fn generate_prediction(&self, values: &[f64]) -> Option<MetricPrediction> {
        if values.len() < 3 {
            return None;
        }

        // Simple linear extrapolation
        let recent_values = &values[values.len().saturating_sub(10)..];
        let (_, slope) = self.calculate_trend(recent_values);
        
        let last_value = *values.last()?;
        let predicted_value = last_value + (slope * self.config.prediction_horizon_minutes as f64);
        
        // Calculate confidence interval (simplified)
        let std_dev = self.calculate_std_dev(recent_values);
        let margin = std_dev * 1.96; // 95% confidence interval
        
        Some(MetricPrediction {
            predicted_value,
            confidence_interval: (predicted_value - margin, predicted_value + margin),
            confidence_level: 0.95,
            prediction_time: Utc::now() + chrono::Duration::minutes(self.config.prediction_horizon_minutes as i64),
        })
    }

    fn detect_anomalies(&self, data_points: &[DataPoint]) -> Vec<AnomalyDetection> {
        // Simplified anomaly detection using z-score
        let values: Vec<f64> = data_points.iter().map(|dp| dp.value).collect();
        
        if values.len() < 10 {
            return Vec::new();
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let std_dev = self.calculate_std_dev(&values);
        
        let mut anomalies = Vec::new();
        
        for (i, data_point) in data_points.iter().enumerate() {
            let z_score = (data_point.value - mean) / std_dev;
            
            if z_score.abs() > 3.0 { // 3-sigma rule
                let anomaly_type = if z_score > 0.0 {
                    AnomalyType::Spike
                } else {
                    AnomalyType::Drop
                };
                
                anomalies.push(AnomalyDetection {
                    timestamp: data_point.timestamp,
                    value: data_point.value,
                    expected_value: mean,
                    deviation_score: z_score.abs(),
                    anomaly_type,
                });
            }
        }
        
        anomalies
    }

    fn detect_seasonal_patterns(&self, _values: &[f64]) -> Vec<SeasonalPattern> {
        // Placeholder for seasonal pattern detection
        Vec::new()
    }

    fn calculate_std_dev(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (values.len() - 1) as f64;
        
        variance.sqrt()
    }
}

impl AlertManager {
    pub fn new(config: AlertConfig) -> Self {
        let (notification_sender, _notification_receiver) = mpsc::unbounded_channel();
        
        Self {
            config,
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            notification_sender,
        }
    }

    pub async fn check_threshold(&self, metric_name: &str, current_value: f64, threshold: &AlertThreshold) -> Result<(), BenchmarkError> {
        let is_threshold_exceeded = match threshold.comparison {
            ThresholdComparison::GreaterThan => current_value > threshold.critical_threshold,
            ThresholdComparison::LessThan => current_value < threshold.critical_threshold,
            ThresholdComparison::Equal => (current_value - threshold.critical_threshold).abs() < f64::EPSILON,
            ThresholdComparison::NotEqual => (current_value - threshold.critical_threshold).abs() > f64::EPSILON,
        };

        if is_threshold_exceeded {
            self.trigger_alert(metric_name, current_value, threshold, AlertSeverity::Critical).await?;
        } else {
            // Check warning threshold
            let is_warning_exceeded = match threshold.comparison {
                ThresholdComparison::GreaterThan => current_value > threshold.warning_threshold,
                ThresholdComparison::LessThan => current_value < threshold.warning_threshold,
                ThresholdComparison::Equal => (current_value - threshold.warning_threshold).abs() < f64::EPSILON,
                ThresholdComparison::NotEqual => (current_value - threshold.warning_threshold).abs() > f64::EPSILON,
            };

            if is_warning_exceeded {
                self.trigger_alert(metric_name, current_value, threshold, AlertSeverity::Warning).await?;
            } else {
                // Resolve any existing alerts for this metric
                self.resolve_alert(metric_name).await?;
            }
        }

        Ok(())
    }

    async fn trigger_alert(&self, metric_name: &str, current_value: f64, threshold: &AlertThreshold, severity: AlertSeverity) -> Result<(), BenchmarkError> {
        let alert_id = format!("{}_{}", metric_name, Utc::now().timestamp());
        
        let alert = ActiveAlert {
            alert_id: alert_id.clone(),
            metric_name: metric_name.to_string(),
            severity: severity.clone(),
            threshold_value: match severity {
                AlertSeverity::Critical => threshold.critical_threshold,
                AlertSeverity::Warning => threshold.warning_threshold,
                AlertSeverity::Info => threshold.warning_threshold,
            },
            current_value,
            started_at: Utc::now(),
            last_triggered: Utc::now(),
            trigger_count: 1,
        };

        let mut active_alerts = self.active_alerts.write().await;
        
        if let Some(existing_alert) = active_alerts.get_mut(metric_name) {
            existing_alert.last_triggered = Utc::now();
            existing_alert.trigger_count += 1;
            existing_alert.current_value = current_value;
        } else {
            active_alerts.insert(metric_name.to_string(), alert.clone());
            
            // Send notification
            let notification = AlertNotification {
                alert,
                notification_type: NotificationType::Triggered,
                channels: self.config.notification_channels.clone(),
            };
            
            let _ = self.notification_sender.send(notification);
        }

        Ok(())
    }

    async fn resolve_alert(&self, metric_name: &str) -> Result<(), BenchmarkError> {
        let mut active_alerts = self.active_alerts.write().await;
        
        if let Some(alert) = active_alerts.remove(metric_name) {
            let notification = AlertNotification {
                alert,
                notification_type: NotificationType::Resolved,
                channels: self.config.notification_channels.clone(),
            };
            
            let _ = self.notification_sender.send(notification);
        }

        Ok(())
    }

    pub async fn get_active_alerts(&self) -> Vec<ActiveAlert> {
        let active_alerts = self.active_alerts.read().await;
        active_alerts.values().cloned().collect()
    }
}

// Default implementations
impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval_ms: 1000, // 1 second
            max_data_points: 3600, // 1 hour of data at 1s intervals
            enable_trend_analysis: true,
            enable_predictive_insights: true,
            collected_metrics: vec![
                MetricDefinition {
                    name: "cpu_usage_percent".to_string(),
                    metric_type: MetricType::Gauge,
                    collection_method: CollectionMethod::SystemStats,
                    aggregation_window: Duration::from_secs(60),
                    retention_period: Duration::from_secs(3600),
                },
                MetricDefinition {
                    name: "memory_usage_percent".to_string(),
                    metric_type: MetricType::Gauge,
                    collection_method: CollectionMethod::SystemStats,
                    aggregation_window: Duration::from_secs(60),
                    retention_period: Duration::from_secs(3600),
                },
                MetricDefinition {
                    name: "active_connections".to_string(),
                    metric_type: MetricType::Gauge,
                    collection_method: CollectionMethod::NetworkStats,
                    aggregation_window: Duration::from_secs(30),
                    retention_period: Duration::from_secs(3600),
                },
            ],
            alert_thresholds: {
                let mut thresholds = HashMap::new();
                thresholds.insert("cpu_usage_percent".to_string(), AlertThreshold {
                    warning_threshold: 70.0,
                    critical_threshold: 90.0,
                    comparison: ThresholdComparison::GreaterThan,
                    duration: Duration::from_secs(300),
                });
                thresholds.insert("memory_usage_percent".to_string(), AlertThreshold {
                    warning_threshold: 80.0,
                    critical_threshold: 95.0,
                    comparison: ThresholdComparison::GreaterThan,
                    duration: Duration::from_secs(300),
                });
                thresholds
            },
        }
    }
}

impl Default for TrendAnalysisConfig {
    fn default() -> Self {
        Self {
            analysis_window_size: 60, // 1 minute of data
            prediction_horizon_minutes: 15,
            trend_detection_sensitivity: 0.1,
            seasonal_analysis_enabled: false,
        }
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enable_alerts: true,
            notification_channels: vec![NotificationChannel::Console],
            alert_cooldown_minutes: 5,
            escalation_rules: Vec::new(),
        }
    }
}