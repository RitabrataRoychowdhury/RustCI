//! Adapter Selection Strategies
//!
//! Implements the Strategy pattern for intelligent adapter selection based on
//! performance metrics, load conditions, and custom business rules.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::factory::{
    AdapterInfo, AdapterRequirements, PerformanceRequirements, ReliabilityRequirements,
};
use super::*;
use crate::error::{Result, ValkyrieError};

/// Strategy Pattern for Intelligent Adapter Selection
#[async_trait]
pub trait AdapterSelectionStrategy: Send + Sync {
    /// Select optimal adapter type based on requirements and current conditions
    async fn select_adapter_type(
        &self,
        requirements: &AdapterRequirements,
        available_adapters: &[AdapterInfo],
    ) -> Result<AdapterType>;

    /// Get strategy name for logging and metrics
    fn strategy_name(&self) -> &str;

    /// Get strategy priority (higher = more preferred)
    fn priority(&self) -> u8;

    /// Validate if strategy can handle requirements
    fn can_handle(&self, requirements: &AdapterRequirements) -> bool;
}

/// Performance-based adapter selection strategy
pub struct PerformanceBasedStrategy {
    /// Performance metrics collector
    metrics: Arc<PerformanceMetrics>,
    /// Latency weight in selection algorithm
    latency_weight: f64,
    /// Throughput weight in selection algorithm
    throughput_weight: f64,
    /// Reliability weight in selection algorithm
    reliability_weight: f64,
    /// Memory usage weight
    memory_weight: f64,
}

/// Reliability-focused selection strategy
pub struct ReliabilityBasedStrategy {
    /// Minimum availability threshold
    min_availability: f64,
    /// Maximum error rate threshold
    max_error_rate: f64,
    /// Failover capability weight
    failover_weight: f64,
    /// Health check weight
    health_weight: f64,
}

/// Latency-optimized selection strategy
pub struct LatencyOptimizedStrategy {
    /// Maximum acceptable latency
    max_latency: Duration,
    /// P99 latency weight
    p99_weight: f64,
    /// Average latency weight
    avg_weight: f64,
    /// Jitter tolerance
    jitter_tolerance: f64,
}

/// Throughput-optimized selection strategy
pub struct ThroughputOptimizedStrategy {
    /// Minimum required throughput
    min_throughput: u64,
    /// Burst capacity weight
    burst_weight: f64,
    /// Sustained throughput weight
    sustained_weight: f64,
    /// Connection pooling weight
    pooling_weight: f64,
}

/// Custom selection strategy with user-defined logic
pub struct CustomSelectionStrategy {
    /// Strategy name
    name: String,
    /// Selection function
    selector:
        Box<dyn Fn(&AdapterRequirements, &[AdapterInfo]) -> Result<AdapterType> + Send + Sync>,
    /// Priority level
    priority: u8,
}

/// Performance metrics for adapter selection
pub struct PerformanceMetrics {
    /// Adapter performance data
    adapter_metrics: Arc<tokio::sync::RwLock<HashMap<AdapterType, AdapterPerformanceData>>>,
    /// Metrics update interval
    update_interval: Duration,
}

/// Performance data for a specific adapter type
#[derive(Debug, Clone)]
pub struct AdapterPerformanceData {
    /// Average latency over time window
    pub avg_latency: Duration,
    /// P95 latency
    pub p95_latency: Duration,
    /// P99 latency
    pub p99_latency: Duration,
    /// Current throughput (messages/sec)
    pub current_throughput: f64,
    /// Peak throughput
    pub peak_throughput: f64,
    /// Error rate (0.0 - 1.0)
    pub error_rate: f64,
    /// Availability (0.0 - 1.0)
    pub availability: f64,
    /// Memory usage (bytes)
    pub memory_usage: u64,
    /// CPU usage (0.0 - 1.0)
    pub cpu_usage: f64,
    /// Active connections
    pub active_connections: usize,
    /// Last updated timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl PerformanceBasedStrategy {
    /// Create new performance-based strategy
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(PerformanceMetrics::new()),
            latency_weight: 0.3,
            throughput_weight: 0.3,
            reliability_weight: 0.3,
            memory_weight: 0.1,
        }
    }

    /// Create with custom weights
    pub fn with_weights(
        latency_weight: f64,
        throughput_weight: f64,
        reliability_weight: f64,
        memory_weight: f64,
    ) -> Self {
        Self {
            metrics: Arc::new(PerformanceMetrics::new()),
            latency_weight,
            throughput_weight,
            reliability_weight,
            memory_weight,
        }
    }

    /// Calculate latency score (higher is better)
    fn calculate_latency_score(
        &self,
        perf_data: &AdapterPerformanceData,
        requirements: &PerformanceRequirements,
    ) -> f64 {
        let max_acceptable = requirements.max_latency.as_nanos() as f64;
        let actual = perf_data.p99_latency.as_nanos() as f64;

        if actual > max_acceptable {
            0.0 // Unacceptable latency
        } else {
            1.0 - (actual / max_acceptable)
        }
    }

    /// Calculate throughput score (higher is better)
    fn calculate_throughput_score(
        &self,
        perf_data: &AdapterPerformanceData,
        requirements: &PerformanceRequirements,
    ) -> f64 {
        let min_required = requirements.min_throughput as f64;
        let actual = perf_data.current_throughput;

        if actual < min_required {
            0.0 // Insufficient throughput
        } else {
            (actual / (min_required * 2.0)).min(1.0) // Cap at 1.0
        }
    }

    /// Calculate reliability score (higher is better)
    fn calculate_reliability_score(
        &self,
        perf_data: &AdapterPerformanceData,
        requirements: &ReliabilityRequirements,
    ) -> f64 {
        let availability_score = if perf_data.availability >= requirements.min_availability {
            perf_data.availability
        } else {
            0.0
        };

        let error_score = if perf_data.error_rate <= requirements.max_error_rate {
            1.0 - perf_data.error_rate
        } else {
            0.0
        };

        (availability_score + error_score) / 2.0
    }

    /// Calculate memory score (higher is better, lower memory usage)
    fn calculate_memory_score(
        &self,
        perf_data: &AdapterPerformanceData,
        requirements: &PerformanceRequirements,
    ) -> f64 {
        if let Some(max_memory) = requirements.max_memory {
            let usage_ratio = perf_data.memory_usage as f64 / max_memory as f64;
            if usage_ratio > 1.0 {
                0.0 // Exceeds memory limit
            } else {
                1.0 - usage_ratio
            }
        } else {
            1.0 // No memory constraint
        }
    }
}

#[async_trait]
impl AdapterSelectionStrategy for PerformanceBasedStrategy {
    async fn select_adapter_type(
        &self,
        requirements: &AdapterRequirements,
        available_adapters: &[AdapterInfo],
    ) -> Result<AdapterType> {
        let mut best_adapter = None;
        let mut best_score = 0.0;

        debug!("Evaluating {} available adapters", available_adapters.len());

        for adapter_info in available_adapters {
            // Get real-time performance metrics
            let perf_metrics = self
                .metrics
                .get_adapter_metrics(adapter_info.adapter_type)
                .await?;

            // Calculate weighted scores
            let latency_score =
                self.calculate_latency_score(&perf_metrics, &requirements.performance);
            let throughput_score =
                self.calculate_throughput_score(&perf_metrics, &requirements.performance);
            let reliability_score =
                self.calculate_reliability_score(&perf_metrics, &requirements.reliability);
            let memory_score =
                self.calculate_memory_score(&perf_metrics, &requirements.performance);

            // Calculate total weighted score
            let total_score = (latency_score * self.latency_weight)
                + (throughput_score * self.throughput_weight)
                + (reliability_score * self.reliability_weight)
                + (memory_score * self.memory_weight);

            debug!("Adapter {} scores: latency={:.2}, throughput={:.2}, reliability={:.2}, memory={:.2}, total={:.2}",
                   adapter_info.adapter_type, latency_score, throughput_score, reliability_score, memory_score, total_score);

            if total_score > best_score {
                best_score = total_score;
                best_adapter = Some(adapter_info.adapter_type);
            }
        }

        match best_adapter {
            Some(adapter_type) => {
                info!(
                    "Selected adapter {} with score {:.2}",
                    adapter_type, best_score
                );
                Ok(adapter_type)
            }
            None => {
                warn!("No suitable adapter found for requirements");
                Err(ValkyrieError::NoSuitableAdapter)
            }
        }
    }

    fn strategy_name(&self) -> &str {
        "performance-based"
    }

    fn priority(&self) -> u8 {
        100 // High priority for performance-critical scenarios
    }

    fn can_handle(&self, _requirements: &AdapterRequirements) -> bool {
        true // Can handle any requirements
    }
}

impl ReliabilityBasedStrategy {
    /// Create new reliability-based strategy
    pub fn new() -> Self {
        Self {
            min_availability: 0.99,
            max_error_rate: 0.01,
            failover_weight: 0.4,
            health_weight: 0.6,
        }
    }
}

#[async_trait]
impl AdapterSelectionStrategy for ReliabilityBasedStrategy {
    async fn select_adapter_type(
        &self,
        requirements: &AdapterRequirements,
        available_adapters: &[AdapterInfo],
    ) -> Result<AdapterType> {
        let mut best_adapter = None;
        let mut best_reliability = 0.0;

        for adapter_info in available_adapters {
            let capabilities = &adapter_info.capabilities;

            // Calculate reliability score based on features
            let mut reliability_score = 0.0;

            if capabilities.reliability_features.supports_failover {
                reliability_score += self.failover_weight;
            }

            if capabilities.reliability_features.supports_health_check {
                reliability_score += self.health_weight;
            }

            if capabilities.reliability_features.supports_circuit_breaker {
                reliability_score += 0.2;
            }

            if capabilities.reliability_features.supports_retry {
                reliability_score += 0.1;
            }

            if reliability_score > best_reliability {
                best_reliability = reliability_score;
                best_adapter = Some(adapter_info.adapter_type);
            }
        }

        best_adapter.ok_or(ValkyrieError::NoSuitableAdapter)
    }

    fn strategy_name(&self) -> &str {
        "reliability-based"
    }

    fn priority(&self) -> u8 {
        90 // High priority for reliability-critical scenarios
    }

    fn can_handle(&self, requirements: &AdapterRequirements) -> bool {
        requirements.reliability.min_availability >= self.min_availability
    }
}

impl PerformanceMetrics {
    /// Create new performance metrics collector
    pub fn new() -> Self {
        Self {
            adapter_metrics: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            update_interval: Duration::from_secs(10),
        }
    }

    /// Get performance metrics for adapter type
    pub async fn get_adapter_metrics(
        &self,
        adapter_type: AdapterType,
    ) -> Result<AdapterPerformanceData> {
        let metrics = self.adapter_metrics.read().await;

        metrics
            .get(&adapter_type)
            .cloned()
            .or_else(|| Some(AdapterPerformanceData::default_for_type(adapter_type)))
            .ok_or(ValkyrieError::MetricsNotAvailable(adapter_type.to_string()))
    }

    /// Update metrics for adapter type
    pub async fn update_metrics(&self, adapter_type: AdapterType, data: AdapterPerformanceData) {
        let mut metrics = self.adapter_metrics.write().await;
        metrics.insert(adapter_type, data);
    }
}

impl AdapterPerformanceData {
    /// Create default performance data for adapter type
    pub fn default_for_type(adapter_type: AdapterType) -> Self {
        match adapter_type {
            AdapterType::Http => Self {
                avg_latency: Duration::from_millis(50),
                p95_latency: Duration::from_millis(100),
                p99_latency: Duration::from_millis(200),
                current_throughput: 1000.0,
                peak_throughput: 5000.0,
                error_rate: 0.01,
                availability: 0.99,
                memory_usage: 100 * 1024 * 1024, // 100MB
                cpu_usage: 0.1,
                active_connections: 50,
                last_updated: chrono::Utc::now(),
            },
            AdapterType::QuicTcp => Self {
                avg_latency: Duration::from_micros(500),
                p95_latency: Duration::from_millis(2),
                p99_latency: Duration::from_millis(5),
                current_throughput: 50000.0,
                peak_throughput: 100000.0,
                error_rate: 0.001,
                availability: 0.999,
                memory_usage: 50 * 1024 * 1024, // 50MB
                cpu_usage: 0.05,
                active_connections: 1000,
                last_updated: chrono::Utc::now(),
            },
            _ => Self {
                avg_latency: Duration::from_millis(10),
                p95_latency: Duration::from_millis(20),
                p99_latency: Duration::from_millis(50),
                current_throughput: 10000.0,
                peak_throughput: 50000.0,
                error_rate: 0.005,
                availability: 0.995,
                memory_usage: 75 * 1024 * 1024, // 75MB
                cpu_usage: 0.08,
                active_connections: 200,
                last_updated: chrono::Utc::now(),
            },
        }
    }
}
