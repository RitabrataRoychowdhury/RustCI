use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

use crate::storage::{OperationMetrics, StoreStats};

/// Metrics collector for storage operations
pub struct StorageMetricsCollector {
    operation_counts: Arc<RwLock<HashMap<String, AtomicU64>>>,
    latency_histograms: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    error_counts: Arc<RwLock<HashMap<String, AtomicU64>>>,
    start_time: Instant,
}

impl StorageMetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            operation_counts: Arc::new(RwLock::new(HashMap::new())),
            latency_histograms: Arc::new(RwLock::new(HashMap::new())),
            error_counts: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// Record an operation metric
    pub async fn record_operation(&self, metric: OperationMetrics) {
        let operation_key = format!("{}_{}", metric.operation_type, if metric.success { "success" } else { "error" });
        
        // Update operation count
        {
            let mut counts = self.operation_counts.write().await;
            let counter = counts.entry(operation_key.clone()).or_insert_with(|| AtomicU64::new(0));
            counter.fetch_add(1, Ordering::Relaxed);
        }

        // Record latency if operation completed
        if let Some(duration) = metric.duration {
            let mut histograms = self.latency_histograms.write().await;
            let histogram = histograms.entry(metric.operation_type.clone()).or_insert_with(Vec::new);
            histogram.push(duration);
            
            // Keep only recent measurements (last 1000)
            if histogram.len() > 1000 {
                histogram.drain(0..histogram.len() - 1000);
            }
        }

        // Record error if operation failed
        if !metric.success {
            let mut error_counts = self.error_counts.write().await;
            let error_key = metric.error.as_deref().unwrap_or("unknown_error");
            let counter = error_counts.entry(error_key.to_string()).or_insert_with(|| AtomicU64::new(0));
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get aggregated statistics
    pub async fn get_stats(&self) -> StoreStats {
        let mut stats = StoreStats::default();
        
        // Calculate total operations
        let counts = self.operation_counts.read().await;
        for (key, counter) in counts.iter() {
            let count = counter.load(Ordering::Relaxed);
            stats.total_operations += count;
            
            if key.ends_with("_success") {
                stats.successful_operations += count;
            } else if key.ends_with("_error") {
                stats.failed_operations += count;
            }
        }

        // Calculate latency percentiles
        let histograms = self.latency_histograms.read().await;
        let mut all_latencies = Vec::new();
        
        for histogram in histograms.values() {
            all_latencies.extend(histogram.iter().cloned());
        }

        if !all_latencies.is_empty() {
            all_latencies.sort();
            
            let total_latency: Duration = all_latencies.iter().sum();
            stats.average_latency_ms = total_latency.as_millis() as f64 / all_latencies.len() as f64;
            
            let p95_index = (all_latencies.len() as f64 * 0.95) as usize;
            let p99_index = (all_latencies.len() as f64 * 0.99) as usize;
            
            if p95_index < all_latencies.len() {
                stats.p95_latency_ms = all_latencies[p95_index].as_millis() as f64;
            }
            
            if p99_index < all_latencies.len() {
                stats.p99_latency_ms = all_latencies[p99_index].as_millis() as f64;
            }
        }

        stats.last_updated = SystemTime::now();
        stats
    }

    /// Get operation counts by type
    pub async fn get_operation_counts(&self) -> HashMap<String, u64> {
        let counts = self.operation_counts.read().await;
        counts.iter()
            .map(|(key, counter)| (key.clone(), counter.load(Ordering::Relaxed)))
            .collect()
    }

    /// Get error counts by type
    pub async fn get_error_counts(&self) -> HashMap<String, u64> {
        let error_counts = self.error_counts.read().await;
        error_counts.iter()
            .map(|(key, counter)| (key.clone(), counter.load(Ordering::Relaxed)))
            .collect()
    }

    /// Get latency histogram for an operation type
    pub async fn get_latency_histogram(&self, operation_type: &str) -> Vec<Duration> {
        let histograms = self.latency_histograms.read().await;
        histograms.get(operation_type).cloned().unwrap_or_default()
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut counts = self.operation_counts.write().await;
        counts.clear();
        
        let mut histograms = self.latency_histograms.write().await;
        histograms.clear();
        
        let mut error_counts = self.error_counts.write().await;
        error_counts.clear();
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for StorageMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics aggregator for multiple storage backends
pub struct MultiBackendMetricsAggregator {
    backend_collectors: Arc<RwLock<HashMap<String, Arc<StorageMetricsCollector>>>>,
}

impl MultiBackendMetricsAggregator {
    /// Create a new aggregator
    pub fn new() -> Self {
        Self {
            backend_collectors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a backend collector
    pub async fn add_backend(&self, backend_name: String, collector: Arc<StorageMetricsCollector>) {
        let mut collectors = self.backend_collectors.write().await;
        collectors.insert(backend_name, collector);
    }

    /// Remove a backend collector
    pub async fn remove_backend(&self, backend_name: &str) {
        let mut collectors = self.backend_collectors.write().await;
        collectors.remove(backend_name);
    }

    /// Get aggregated stats across all backends
    pub async fn get_aggregated_stats(&self) -> HashMap<String, StoreStats> {
        let collectors = self.backend_collectors.read().await;
        let mut stats = HashMap::new();

        for (backend_name, collector) in collectors.iter() {
            let backend_stats = collector.get_stats().await;
            stats.insert(backend_name.clone(), backend_stats);
        }

        stats
    }

    /// Get total stats across all backends
    pub async fn get_total_stats(&self) -> StoreStats {
        let collectors = self.backend_collectors.read().await;
        let mut total_stats = StoreStats::default();

        for collector in collectors.values() {
            let stats = collector.get_stats().await;
            total_stats.total_operations += stats.total_operations;
            total_stats.successful_operations += stats.successful_operations;
            total_stats.failed_operations += stats.failed_operations;
            total_stats.connections_active += stats.connections_active;
            total_stats.connections_idle += stats.connections_idle;
            total_stats.memory_usage_bytes += stats.memory_usage_bytes;
        }

        // Calculate weighted average latency
        let mut total_weighted_latency = 0.0;
        let mut total_operations = 0;

        for collector in collectors.values() {
            let stats = collector.get_stats().await;
            if stats.total_operations > 0 {
                total_weighted_latency += stats.average_latency_ms * stats.total_operations as f64;
                total_operations += stats.total_operations;
            }
        }

        if total_operations > 0 {
            total_stats.average_latency_ms = total_weighted_latency / total_operations as f64;
        }

        total_stats.last_updated = SystemTime::now();
        total_stats
    }
}

impl Default for MultiBackendMetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = StorageMetricsCollector::new();

        // Record some operations
        let metric1 = OperationMetrics::new("get".to_string())
            .complete_success(100);
        collector.record_operation(metric1).await;

        let metric2 = OperationMetrics::new("set".to_string())
            .complete_error("timeout".to_string());
        collector.record_operation(metric2).await;

        // Get stats
        let stats = collector.get_stats().await;
        assert_eq!(stats.total_operations, 2);
        assert_eq!(stats.successful_operations, 1);
        assert_eq!(stats.failed_operations, 1);

        // Get operation counts
        let counts = collector.get_operation_counts().await;
        assert!(counts.contains_key("get_success"));
        assert!(counts.contains_key("set_error"));

        // Get error counts
        let error_counts = collector.get_error_counts().await;
        assert!(error_counts.contains_key("timeout"));
    }

    #[tokio::test]
    async fn test_multi_backend_aggregator() {
        let aggregator = MultiBackendMetricsAggregator::new();

        // Add backend collectors
        let redis_collector = Arc::new(StorageMetricsCollector::new());
        let valkey_collector = Arc::new(StorageMetricsCollector::new());

        aggregator.add_backend("redis".to_string(), redis_collector.clone()).await;
        aggregator.add_backend("valkey".to_string(), valkey_collector.clone()).await;

        // Record operations
        let metric = OperationMetrics::new("get".to_string()).complete_success(50);
        redis_collector.record_operation(metric).await;

        let metric = OperationMetrics::new("set".to_string()).complete_success(75);
        valkey_collector.record_operation(metric).await;

        // Get aggregated stats
        let stats = aggregator.get_aggregated_stats().await;
        assert_eq!(stats.len(), 2);
        assert!(stats.contains_key("redis"));
        assert!(stats.contains_key("valkey"));

        // Get total stats
        let total_stats = aggregator.get_total_stats().await;
        assert_eq!(total_stats.total_operations, 2);
        assert_eq!(total_stats.successful_operations, 2);
    }
}