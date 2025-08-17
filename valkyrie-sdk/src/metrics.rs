//! Metrics collection for Valkyrie SDK

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// Metrics collector
pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, u64>>>,
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<f64>>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Increment a counter
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += value;
    }
    
    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }
    
    /// Record a histogram value
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        histograms.entry(name.to_string()).or_insert_with(Vec::new).push(value);
    }
    
    /// Get all metrics as a snapshot
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let counters = self.counters.read().await.clone();
        let gauges = self.gauges.read().await.clone();
        let histograms = self.histograms.read().await.clone();
        
        MetricsSnapshot {
            counters,
            gauges,
            histograms,
            timestamp: chrono::Utc::now(),
        }
    }
    
    /// Reset all metrics
    pub async fn reset(&self) {
        let mut counters = self.counters.write().await;
        let mut gauges = self.gauges.write().await;
        let mut histograms = self.histograms.write().await;
        
        counters.clear();
        gauges.clear();
        histograms.clear();
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Counter values
    pub counters: HashMap<String, u64>,
    /// Gauge values
    pub gauges: HashMap<String, f64>,
    /// Histogram values
    pub histograms: HashMap<String, Vec<f64>>,
    /// Timestamp when snapshot was taken
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MetricsSnapshot {
    /// Get counter value
    pub fn get_counter(&self, name: &str) -> Option<u64> {
        self.counters.get(name).copied()
    }
    
    /// Get gauge value
    pub fn get_gauge(&self, name: &str) -> Option<f64> {
        self.gauges.get(name).copied()
    }
    
    /// Get histogram statistics
    pub fn get_histogram_stats(&self, name: &str) -> Option<HistogramStats> {
        self.histograms.get(name).map(|values| {
            if values.is_empty() {
                return HistogramStats::default();
            }
            
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let count = sorted.len();
            let sum: f64 = sorted.iter().sum();
            let mean = sum / count as f64;
            let min = sorted[0];
            let max = sorted[count - 1];
            
            let p50_idx = (count as f64 * 0.5) as usize;
            let p95_idx = (count as f64 * 0.95) as usize;
            let p99_idx = (count as f64 * 0.99) as usize;
            
            HistogramStats {
                count: count as u64,
                sum,
                mean,
                min,
                max,
                p50: sorted[p50_idx.min(count - 1)],
                p95: sorted[p95_idx.min(count - 1)],
                p99: sorted[p99_idx.min(count - 1)],
            }
        })
    }
    
    /// Export metrics in Prometheus format
    pub fn to_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Counters
        for (name, value) in &self.counters {
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }
        
        // Gauges
        for (name, value) in &self.gauges {
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }
        
        // Histograms
        for (name, _values) in &self.histograms {
            if let Some(stats) = self.get_histogram_stats(name) {
                output.push_str(&format!("# TYPE {} histogram\n", name));
                output.push_str(&format!("{}_count {}\n", name, stats.count));
                output.push_str(&format!("{}_sum {}\n", name, stats.sum));
                output.push_str(&format!("{}_bucket{{le=\"0.5\"}} {}\n", name, stats.p50));
                output.push_str(&format!("{}_bucket{{le=\"0.95\"}} {}\n", name, stats.p95));
                output.push_str(&format!("{}_bucket{{le=\"0.99\"}} {}\n", name, stats.p99));
                output.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", name, stats.count));
            }
        }
        
        output
    }
}

/// Histogram statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramStats {
    /// Number of observations
    pub count: u64,
    /// Sum of all observations
    pub sum: f64,
    /// Mean value
    pub mean: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// 50th percentile
    pub p50: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
}

impl Default for HistogramStats {
    fn default() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            mean: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
        }
    }
}