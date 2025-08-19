// Built-in metrics collection without external dependencies

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;

use super::ObservabilityError;

/// Built-in metrics collector
pub struct MetricsCollector {
    /// Metrics storage
    metrics: Arc<RwLock<HashMap<String, MetricSeries>>>,
    /// Retention period in seconds
    retention_seconds: u64,
    /// Background cleanup task handle
    cleanup_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary { sum: f64, count: u64 },
}

/// Metric type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// Time series data for a metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSeries {
    pub name: String,
    pub metric_type: MetricType,
    pub data_points: Vec<MetricDataPoint>,
    pub labels: HashMap<String, String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Individual metric data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    pub timestamp: u64,
    pub value: MetricValue,
}

/// Metric query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricQuery {
    pub name: Option<String>,
    pub labels: HashMap<String, String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub limit: Option<usize>,
}

/// Metric aggregation functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationFunction {
    Sum,
    Average,
    Min,
    Max,
    Count,
    P50,
    P95,
    P99,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(retention_seconds: u64) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            retention_seconds,
            cleanup_handle: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the metrics collector
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        *running = true;
        drop(running);

        // Start background cleanup task
        let metrics = self.metrics.clone();
        let retention_seconds = self.retention_seconds;
        let running_flag = self.running.clone();

        let handle = tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60)); // Cleanup every minute

            while *running_flag.read().await {
                cleanup_interval.tick().await;

                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let mut metrics_guard = metrics.write().await;
                metrics_guard.retain(|_, series| {
                    // Remove old data points
                    series
                        .data_points
                        .retain(|point| current_time - point.timestamp < retention_seconds);

                    // Keep series if it has recent data points
                    !series.data_points.is_empty()
                });
            }
        });

        *self.cleanup_handle.write().await = Some(handle);

        Ok(())
    }

    /// Stop the metrics collector
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);

        if let Some(handle) = self.cleanup_handle.write().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Record a metric value
    pub async fn record(
        &self,
        name: &str,
        value: MetricValue,
        labels: HashMap<String, String>,
    ) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ObservabilityError::Metrics(format!("Time error: {}", e)))?
            .as_secs();

        let metric_type = match &value {
            MetricValue::Counter(_) => MetricType::Counter,
            MetricValue::Gauge(_) => MetricType::Gauge,
            MetricValue::Histogram(_) => MetricType::Histogram,
            MetricValue::Summary { .. } => MetricType::Summary,
        };

        let data_point = MetricDataPoint { timestamp, value };
        let metric_key = format!("{}:{}", name, self.labels_to_key(&labels));

        let mut metrics = self.metrics.write().await;

        match metrics.get_mut(&metric_key) {
            Some(series) => {
                series.data_points.push(data_point);
                series.updated_at = timestamp;
            }
            None => {
                let series = MetricSeries {
                    name: name.to_string(),
                    metric_type,
                    data_points: vec![data_point],
                    labels,
                    created_at: timestamp,
                    updated_at: timestamp,
                };
                metrics.insert(metric_key, series);
            }
        }

        Ok(())
    }

    /// Increment a counter metric
    pub async fn increment_counter(
        &self,
        name: &str,
        labels: HashMap<String, String>,
    ) -> Result<(), ObservabilityError> {
        self.record(name, MetricValue::Counter(1), labels).await
    }

    /// Set a gauge metric
    pub async fn set_gauge(
        &self,
        name: &str,
        value: f64,
        labels: HashMap<String, String>,
    ) -> Result<(), ObservabilityError> {
        self.record(name, MetricValue::Gauge(value), labels).await
    }

    /// Record histogram values
    pub async fn record_histogram(
        &self,
        name: &str,
        values: Vec<f64>,
        labels: HashMap<String, String>,
    ) -> Result<(), ObservabilityError> {
        self.record(name, MetricValue::Histogram(values), labels)
            .await
    }

    /// Record summary metric
    pub async fn record_summary(
        &self,
        name: &str,
        sum: f64,
        count: u64,
        labels: HashMap<String, String>,
    ) -> Result<(), ObservabilityError> {
        self.record(name, MetricValue::Summary { sum, count }, labels)
            .await
    }

    /// Query metrics
    pub async fn query(&self, query: MetricQuery) -> Result<Vec<MetricSeries>, ObservabilityError> {
        let metrics = self.metrics.read().await;
        let mut results = Vec::new();

        for (_, series) in metrics.iter() {
            // Filter by name
            if let Some(ref name) = query.name {
                if &series.name != name {
                    continue;
                }
            }

            // Filter by labels
            let mut label_match = true;
            for (key, value) in &query.labels {
                if series.labels.get(key) != Some(value) {
                    label_match = false;
                    break;
                }
            }
            if !label_match {
                continue;
            }

            // Filter by time range
            let mut filtered_series = series.clone();
            if query.start_time.is_some() || query.end_time.is_some() {
                filtered_series.data_points.retain(|point| {
                    if let Some(start) = query.start_time {
                        if point.timestamp < start {
                            return false;
                        }
                    }
                    if let Some(end) = query.end_time {
                        if point.timestamp > end {
                            return false;
                        }
                    }
                    true
                });
            }

            // Apply limit
            if let Some(limit) = query.limit {
                filtered_series.data_points.truncate(limit);
            }

            results.push(filtered_series);
        }

        Ok(results)
    }

    /// Get all metric names
    pub async fn metric_names(&self) -> Vec<String> {
        let metrics = self.metrics.read().await;
        metrics.values().map(|s| s.name.clone()).collect()
    }

    /// Get metric count
    pub async fn count(&self) -> usize {
        let metrics = self.metrics.read().await;
        metrics.len()
    }

    /// Aggregate metric values
    pub async fn aggregate(
        &self,
        query: MetricQuery,
        function: AggregationFunction,
    ) -> Result<f64, ObservabilityError> {
        let series_list = self.query(query).await?;
        let mut all_values = Vec::new();

        for series in series_list {
            for point in series.data_points {
                match point.value {
                    MetricValue::Counter(v) => all_values.push(v as f64),
                    MetricValue::Gauge(v) => all_values.push(v),
                    MetricValue::Histogram(ref values) => all_values.extend(values),
                    MetricValue::Summary { sum, count } => {
                        if count > 0 {
                            all_values.push(sum / count as f64);
                        }
                    }
                }
            }
        }

        if all_values.is_empty() {
            return Ok(0.0);
        }

        let result = match function {
            AggregationFunction::Sum => all_values.iter().sum(),
            AggregationFunction::Average => {
                all_values.iter().sum::<f64>() / all_values.len() as f64
            }
            AggregationFunction::Min => all_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            AggregationFunction::Max => all_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
            AggregationFunction::Count => all_values.len() as f64,
            AggregationFunction::P50 => self.percentile(&mut all_values, 0.5),
            AggregationFunction::P95 => self.percentile(&mut all_values, 0.95),
            AggregationFunction::P99 => self.percentile(&mut all_values, 0.99),
        };

        Ok(result)
    }

    /// Calculate percentile
    fn percentile(&self, values: &mut [f64], percentile: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = (percentile * (values.len() - 1) as f64) as usize;
        values[index.min(values.len() - 1)]
    }

    /// Convert labels to a consistent key
    fn labels_to_key(&self, labels: &HashMap<String, String>) -> String {
        let mut pairs: Vec<_> = labels.iter().collect();
        pairs.sort_by_key(|(k, _)| *k);
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Get metrics summary
    pub async fn summary(&self) -> MetricsSummary {
        let metrics = self.metrics.read().await;
        let total_metrics = metrics.len();
        let mut type_counts = HashMap::new();
        let mut total_data_points = 0;

        for series in metrics.values() {
            *type_counts.entry(series.metric_type).or_insert(0) += 1;
            total_data_points += series.data_points.len();
        }

        MetricsSummary {
            total_metrics,
            total_data_points,
            type_counts,
            retention_seconds: self.retention_seconds,
        }
    }
}

/// Metrics summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_metrics: usize,
    pub total_data_points: usize,
    pub type_counts: HashMap<MetricType, usize>,
    pub retention_seconds: u64,
}
