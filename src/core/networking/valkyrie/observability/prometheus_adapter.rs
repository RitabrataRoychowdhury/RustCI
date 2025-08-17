// Optional Prometheus Adapter (Feature-Gated)
// Task 3.4: Pluggable Observability System

#[cfg(feature = "prometheus")]
use std::collections::HashMap;
#[cfg(feature = "prometheus")]
use std::sync::Arc;
#[cfg(feature = "prometheus")]
use std::time::{Duration, SystemTime};
#[cfg(feature = "prometheus")]
use async_trait::async_trait;
#[cfg(feature = "prometheus")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "prometheus")]
use dashmap::DashMap;

#[cfg(feature = "prometheus")]
use super::adapter_system::{MetricsAdapter, AdapterHealth, HealthStatus};

/// Prometheus adapter configuration
#[cfg(feature = "prometheus")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// Prometheus server endpoint
    pub endpoint: String,
    
    /// Push gateway endpoint (optional)
    pub push_gateway: Option<String>,
    
    /// Metrics prefix
    pub metrics_prefix: String,
    
    /// Push interval for push gateway
    pub push_interval: Duration,
    
    /// Enable/disable specific metric types
    pub enable_counters: bool,
    pub enable_gauges: bool,
    pub enable_histograms: bool,
    
    /// Histogram buckets
    pub histogram_buckets: Vec<f64>,
    
    /// Maximum labels per metric
    pub max_labels: usize,
    
    /// Connection timeout
    pub connection_timeout: Duration,
}

#[cfg(feature = "prometheus")]
impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            endpoint: "0.0.0.0:9090".to_string(),
            push_gateway: None,
            metrics_prefix: "rustci_valkyrie_".to_string(),
            push_interval: Duration::from_secs(15),
            enable_counters: true,
            enable_gauges: true,
            enable_histograms: true,
            histogram_buckets: vec![0.001, 0.01, 0.1, 1.0, 10.0, 100.0, 1000.0],
            max_labels: 10,
            connection_timeout: Duration::from_secs(5),
        }
    }
}

/// Prometheus metrics adapter (only available with "prometheus" feature)
#[cfg(feature = "prometheus")]
pub struct PrometheusAdapter {
    /// Prometheus registry
    registry: prometheus::Registry,
    
    /// Counter metrics cache
    counters: DashMap<String, prometheus::Counter>,
    
    /// Gauge metrics cache
    gauges: DashMap<String, prometheus::Gauge>,
    
    /// Histogram metrics cache
    histograms: DashMap<String, prometheus::Histogram>,
    
    /// Configuration
    config: PrometheusConfig,
    
    /// HTTP client for push gateway
    http_client: Option<reqwest::Client>,
    
    /// Adapter health tracking
    health: Arc<std::sync::Mutex<AdapterHealthTracker>>,
}

#[cfg(feature = "prometheus")]
#[derive(Debug)]
struct AdapterHealthTracker {
    last_successful_operation: Option<SystemTime>,
    error_count: u64,
    total_operations: u64,
}

#[cfg(feature = "prometheus")]
impl PrometheusAdapter {
    /// Create a new Prometheus adapter
    pub fn new(config: PrometheusConfig) -> Result<Self, PrometheusAdapterError> {
        let registry = prometheus::Registry::new();
        
        // Create HTTP client if push gateway is configured
        let http_client = if config.push_gateway.is_some() {
            Some(
                reqwest::Client::builder()
                    .timeout(config.connection_timeout)
                    .build()
                    .map_err(|e| PrometheusAdapterError::InitializationFailed(e.to_string()))?
            )
        } else {
            None
        };
        
        Ok(Self {
            registry,
            counters: DashMap::new(),
            gauges: DashMap::new(),
            histograms: DashMap::new(),
            config,
            http_client,
            health: Arc::new(std::sync::Mutex::new(AdapterHealthTracker {
                last_successful_operation: Some(SystemTime::now()),
                error_count: 0,
                total_operations: 0,
            })),
        })
    }
    
    /// Get or create a counter metric
    fn get_or_create_counter(&self, name: &str, labels: &[(&str, &str)]) -> Result<prometheus::Counter, PrometheusAdapterError> {
        let metric_name = self.build_metric_name(name);
        let cache_key = self.build_cache_key(&metric_name, labels);
        
        if let Some(counter) = self.counters.get(&cache_key) {
            return Ok(counter.clone());
        }
        
        // Create new counter
        let label_names: Vec<String> = labels.iter().map(|(k, _)| k.to_string()).collect();
        let counter = prometheus::CounterVec::new(
            prometheus::Opts::new(&metric_name, &format!("Counter metric: {}", name)),
            &label_names,
        )
        .map_err(|e| PrometheusAdapterError::MetricCreationFailed(e.to_string()))?;
        
        // Register with Prometheus registry
        self.registry.register(Box::new(counter.clone()))
            .map_err(|e| PrometheusAdapterError::RegistrationFailed(e.to_string()))?;
        
        // Get the specific counter for these label values
        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        let specific_counter = counter.with_label_values(&label_values)
            .map_err(|e| PrometheusAdapterError::LabelError(e.to_string()))?;
        
        // Cache it
        self.counters.insert(cache_key, specific_counter.clone());
        
        Ok(specific_counter)
    }
    
    /// Get or create a gauge metric
    fn get_or_create_gauge(&self, name: &str, labels: &[(&str, &str)]) -> Result<prometheus::Gauge, PrometheusAdapterError> {
        let metric_name = self.build_metric_name(name);
        let cache_key = self.build_cache_key(&metric_name, labels);
        
        if let Some(gauge) = self.gauges.get(&cache_key) {
            return Ok(gauge.clone());
        }
        
        // Create new gauge
        let label_names: Vec<String> = labels.iter().map(|(k, _)| k.to_string()).collect();
        let gauge = prometheus::GaugeVec::new(
            prometheus::Opts::new(&metric_name, &format!("Gauge metric: {}", name)),
            &label_names,
        )
        .map_err(|e| PrometheusAdapterError::MetricCreationFailed(e.to_string()))?;
        
        // Register with Prometheus registry
        self.registry.register(Box::new(gauge.clone()))
            .map_err(|e| PrometheusAdapterError::RegistrationFailed(e.to_string()))?;
        
        // Get the specific gauge for these label values
        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        let specific_gauge = gauge.with_label_values(&label_values)
            .map_err(|e| PrometheusAdapterError::LabelError(e.to_string()))?;
        
        // Cache it
        self.gauges.insert(cache_key, specific_gauge.clone());
        
        Ok(specific_gauge)
    }
    
    /// Get or create a histogram metric
    fn get_or_create_histogram(&self, name: &str, labels: &[(&str, &str)]) -> Result<prometheus::Histogram, PrometheusAdapterError> {
        let metric_name = self.build_metric_name(name);
        let cache_key = self.build_cache_key(&metric_name, labels);
        
        if let Some(histogram) = self.histograms.get(&cache_key) {
            return Ok(histogram.clone());
        }
        
        // Create new histogram
        let label_names: Vec<String> = labels.iter().map(|(k, _)| k.to_string()).collect();
        let histogram = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(&metric_name, &format!("Histogram metric: {}", name))
                .buckets(self.config.histogram_buckets.clone()),
            &label_names,
        )
        .map_err(|e| PrometheusAdapterError::MetricCreationFailed(e.to_string()))?;
        
        // Register with Prometheus registry
        self.registry.register(Box::new(histogram.clone()))
            .map_err(|e| PrometheusAdapterError::RegistrationFailed(e.to_string()))?;
        
        // Get the specific histogram for these label values
        let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
        let specific_histogram = histogram.with_label_values(&label_values)
            .map_err(|e| PrometheusAdapterError::LabelError(e.to_string()))?;
        
        // Cache it
        self.histograms.insert(cache_key, specific_histogram.clone());
        
        Ok(specific_histogram)
    }
    
    /// Build metric name with prefix
    fn build_metric_name(&self, name: &str) -> String {
        format!("{}{}", self.config.metrics_prefix, name)
    }
    
    /// Build cache key for metric with labels
    fn build_cache_key(&self, metric_name: &str, labels: &[(&str, &str)]) -> String {
        let mut key = metric_name.to_string();
        for (k, v) in labels {
            key.push_str(&format!("{}={}", k, v));
        }
        key
    }
    
    /// Update health tracking
    fn update_health(&self, success: bool) {
        if let Ok(mut health) = self.health.lock() {
            health.total_operations += 1;
            if success {
                health.last_successful_operation = Some(SystemTime::now());
            } else {
                health.error_count += 1;
            }
        }
    }
    
    /// Push metrics to push gateway if configured
    pub async fn push_metrics(&self) -> Result<(), PrometheusAdapterError> {
        if let (Some(push_gateway), Some(client)) = (&self.config.push_gateway, &self.http_client) {
            let metrics = self.registry.gather();
            let mut buffer = Vec::new();
            
            // Encode metrics in Prometheus format
            let encoder = prometheus::TextEncoder::new();
            encoder.encode(&metrics, &mut buffer)
                .map_err(|e| PrometheusAdapterError::EncodingFailed(e.to_string()))?;
            
            // Push to gateway
            let response = client
                .post(&format!("{}/metrics/job/rustci_valkyrie", push_gateway))
                .header("Content-Type", "text/plain")
                .body(buffer)
                .send()
                .await
                .map_err(|e| PrometheusAdapterError::PushFailed(e.to_string()))?;
            
            if !response.status().is_success() {
                return Err(PrometheusAdapterError::PushFailed(
                    format!("Push gateway returned status: {}", response.status())
                ));
            }
            
            self.update_health(true);
        }
        
        Ok(())
    }
}

#[cfg(feature = "prometheus")]
#[async_trait]
impl MetricsAdapter for PrometheusAdapter {
    async fn record_counter(&self, name: &str, value: u64, labels: &[(&str, &str)]) {
        if !self.config.enable_counters {
            return;
        }
        
        // Limit number of labels
        let limited_labels = if labels.len() > self.config.max_labels {
            &labels[..self.config.max_labels]
        } else {
            labels
        };
        
        match self.get_or_create_counter(name, limited_labels) {
            Ok(counter) => {
                counter.inc_by(value as f64);
                self.update_health(true);
            }
            Err(_) => {
                self.update_health(false);
            }
        }
    }
    
    async fn record_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if !self.config.enable_gauges {
            return;
        }
        
        let limited_labels = if labels.len() > self.config.max_labels {
            &labels[..self.config.max_labels]
        } else {
            labels
        };
        
        match self.get_or_create_gauge(name, limited_labels) {
            Ok(gauge) => {
                gauge.set(value);
                self.update_health(true);
            }
            Err(_) => {
                self.update_health(false);
            }
        }
    }
    
    async fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        if !self.config.enable_histograms {
            return;
        }
        
        let limited_labels = if labels.len() > self.config.max_labels {
            &labels[..self.config.max_labels]
        } else {
            labels
        };
        
        match self.get_or_create_histogram(name, limited_labels) {
            Ok(histogram) => {
                histogram.observe(value);
                self.update_health(true);
            }
            Err(_) => {
                self.update_health(false);
            }
        }
    }
    
    async fn record_timing(&self, name: &str, duration: Duration, labels: &[(&str, &str)]) {
        // Record timing as histogram in seconds
        self.record_histogram(
            &format!("{}_duration_seconds", name),
            duration.as_secs_f64(),
            labels
        ).await;
    }
    
    fn name(&self) -> &str {
        "prometheus"
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

/// Prometheus adapter errors
#[cfg(feature = "prometheus")]
#[derive(Debug, thiserror::Error)]
pub enum PrometheusAdapterError {
    #[error("Failed to initialize Prometheus adapter: {0}")]
    InitializationFailed(String),
    
    #[error("Failed to create metric: {0}")]
    MetricCreationFailed(String),
    
    #[error("Failed to register metric: {0}")]
    RegistrationFailed(String),
    
    #[error("Label error: {0}")]
    LabelError(String),
    
    #[error("Failed to encode metrics: {0}")]
    EncodingFailed(String),
    
    #[error("Failed to push metrics: {0}")]
    PushFailed(String),
}

/// Factory for creating Prometheus adapter (only available with feature)
#[cfg(feature = "prometheus")]
pub struct PrometheusAdapterFactory;

#[cfg(feature = "prometheus")]
impl PrometheusAdapterFactory {
    /// Create a Prometheus adapter with default configuration
    pub fn create_default() -> Result<Arc<dyn MetricsAdapter>, PrometheusAdapterError> {
        let config = PrometheusConfig::default();
        let adapter = PrometheusAdapter::new(config)?;
        Ok(Arc::new(adapter))
    }
    
    /// Create a Prometheus adapter with custom configuration
    pub fn create_with_config(config: PrometheusConfig) -> Result<Arc<dyn MetricsAdapter>, PrometheusAdapterError> {
        let adapter = PrometheusAdapter::new(config)?;
        Ok(Arc::new(adapter))
    }
    
    /// Create from environment variables
    pub fn create_from_env() -> Result<Arc<dyn MetricsAdapter>, PrometheusAdapterError> {
        let config = PrometheusConfig {
            endpoint: std::env::var("PROMETHEUS_ENDPOINT")
                .unwrap_or_else(|_| "0.0.0.0:9090".to_string()),
            push_gateway: std::env::var("PROMETHEUS_PUSH_GATEWAY").ok(),
            metrics_prefix: std::env::var("PROMETHEUS_METRICS_PREFIX")
                .unwrap_or_else(|_| "rustci_valkyrie_".to_string()),
            ..Default::default()
        };
        
        Self::create_with_config(config)
    }
}

// Stub implementations when prometheus feature is not enabled
#[cfg(not(feature = "prometheus"))]
pub struct PrometheusConfig;

#[cfg(not(feature = "prometheus"))]
impl Default for PrometheusConfig {
    fn default() -> Self {
        Self
    }
}

#[cfg(not(feature = "prometheus"))]
pub struct PrometheusAdapterFactory;

#[cfg(not(feature = "prometheus"))]
impl PrometheusAdapterFactory {
    pub fn create_default() -> Result<Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::MetricsAdapter>, Box<dyn std::error::Error>> {
        Err("Prometheus feature not enabled. Enable with --features prometheus".into())
    }
    
    pub fn create_with_config(_config: PrometheusConfig) -> Result<Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::MetricsAdapter>, Box<dyn std::error::Error>> {
        Err("Prometheus feature not enabled. Enable with --features prometheus".into())
    }
    
    pub fn create_from_env() -> Result<Arc<dyn crate::core::networking::valkyrie::observability::adapter_system::MetricsAdapter>, Box<dyn std::error::Error>> {
        Err("Prometheus feature not enabled. Enable with --features prometheus".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "prometheus")]
    #[tokio::test]
    async fn test_prometheus_adapter_creation() {
        let config = PrometheusConfig::default();
        let adapter = PrometheusAdapter::new(config);
        assert!(adapter.is_ok());
        
        let adapter = adapter.unwrap();
        assert_eq!(adapter.name(), "prometheus");
        
        let health = adapter.health_check().await;
        assert!(matches!(health.status, HealthStatus::Healthy));
    }
    
    #[cfg(feature = "prometheus")]
    #[tokio::test]
    async fn test_prometheus_metrics_recording() {
        let config = PrometheusConfig::default();
        let adapter = PrometheusAdapter::new(config).unwrap();
        
        // Test counter
        adapter.record_counter("test_counter", 1, &[("label", "value")]).await;
        
        // Test gauge
        adapter.record_gauge("test_gauge", 42.0, &[("label", "value")]).await;
        
        // Test histogram
        adapter.record_histogram("test_histogram", 1.5, &[("label", "value")]).await;
        
        // Test timing
        adapter.record_timing("test_timing", Duration::from_millis(100), &[("label", "value")]).await;
        
        let health = adapter.health_check().await;
        assert!(matches!(health.status, HealthStatus::Healthy));
        assert!(health.total_operations > 0);
    }
    
    #[cfg(not(feature = "prometheus"))]
    #[test]
    fn test_prometheus_feature_disabled() {
        let result = PrometheusAdapterFactory::create_default();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Prometheus feature not enabled"));
    }
    
    #[test]
    fn test_prometheus_config_default() {
        let _config = PrometheusConfig::default();
        // Should compile regardless of feature flag
    }
}