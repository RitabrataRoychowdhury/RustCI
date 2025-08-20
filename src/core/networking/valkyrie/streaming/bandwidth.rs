//! Bandwidth Allocation and Management System
//!
//! Implements intelligent bandwidth allocation with QoS guarantees, adaptive
//! throttling, and fair resource distribution across different message classes.

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{error, info};

use super::QoSClass;
use crate::core::networking::valkyrie::adapters::*;
use crate::error::{Result, ValkyrieError};

/// Bandwidth allocator with QoS-aware resource management
pub struct BandwidthAllocator {
    /// Total available bandwidth (bytes/sec)
    total_bandwidth: Arc<RwLock<u64>>,
    /// Bandwidth allocations by QoS class
    class_allocations: Arc<DashMap<QoSClass, ClassBandwidthAllocation>>,
    /// Active stream allocations
    stream_allocations: Arc<DashMap<StreamId, StreamBandwidthAllocation>>,
    /// Bandwidth usage tracking
    usage_tracker: Arc<BandwidthUsageTracker>,
    /// Throttling manager
    throttling_manager: Arc<ThrottlingManager>,
    /// Configuration
    config: BandwidthConfig,
    /// Performance metrics
    metrics: Arc<RwLock<BandwidthMetrics>>,
}

/// Bandwidth allocation for QoS class
#[derive(Debug, Clone)]
pub struct ClassBandwidthAllocation {
    /// QoS class
    pub qos_class: QoSClass,
    /// Guaranteed bandwidth (bytes/sec)
    pub guaranteed_bandwidth: u64,
    /// Maximum bandwidth (bytes/sec)
    pub max_bandwidth: u64,
    /// Current usage (bytes/sec)
    pub current_usage: Arc<RwLock<u64>>,
    /// Priority weight (0.0 - 1.0)
    pub priority_weight: f64,
    /// Burst allowance (bytes)
    pub burst_allowance: u64,
    /// Current burst usage
    pub burst_usage: Arc<RwLock<u64>>,
    /// Last allocation update
    pub last_updated: Instant,
}

/// Bandwidth allocation for individual stream
#[derive(Debug, Clone)]
pub struct StreamBandwidthAllocation {
    /// Stream ID
    pub stream_id: StreamId,
    /// QoS class
    pub qos_class: QoSClass,
    /// Allocated bandwidth (bytes/sec)
    pub allocated_bandwidth: u64,
    /// Current usage (bytes/sec)
    pub current_usage: Arc<RwLock<u64>>,
    /// Usage history for adaptive allocation
    pub usage_history: Arc<RwLock<Vec<UsageDataPoint>>>,
    /// Throttling state
    pub throttling_state: Arc<RwLock<ThrottlingState>>,
    /// Created timestamp
    pub created_at: Instant,
    /// Last activity
    pub last_activity: Instant,
}

/// Bandwidth usage tracker
pub struct BandwidthUsageTracker {
    /// Usage samples by QoS class
    class_usage: Arc<DashMap<QoSClass, Arc<RwLock<Vec<UsageDataPoint>>>>>,
    /// Global usage samples
    global_usage: Arc<RwLock<Vec<UsageDataPoint>>>,
    /// Sampling interval
    sampling_interval: Duration,
    /// Maximum samples to keep
    max_samples: usize,
}

/// Throttling manager for congestion control
pub struct ThrottlingManager {
    /// Throttling policies by QoS class
    policies: Arc<DashMap<QoSClass, ThrottlingPolicy>>,
    /// Active throttling sessions
    active_throttling: Arc<DashMap<StreamId, ThrottlingSession>>,
    /// Congestion detection
    congestion_detector: Arc<CongestionDetector>,
}

/// Usage data point for tracking
#[derive(Debug, Clone)]
pub struct UsageDataPoint {
    /// Timestamp
    pub timestamp: Instant,
    /// Bytes transferred
    pub bytes: u64,
    /// Duration
    pub duration: Duration,
    /// Calculated rate (bytes/sec)
    pub rate: f64,
}

/// Throttling state
#[derive(Debug, Clone)]
pub struct ThrottlingState {
    /// Is throttling active
    pub active: bool,
    /// Throttling rate (0.0 - 1.0, where 1.0 = no throttling)
    pub rate: f64,
    /// Throttling reason
    pub reason: ThrottlingReason,
    /// Started timestamp
    pub started_at: Option<Instant>,
    /// Expected duration
    pub expected_duration: Option<Duration>,
}

/// Throttling policy
#[derive(Debug, Clone)]
pub struct ThrottlingPolicy {
    /// QoS class
    pub qos_class: QoSClass,
    /// Congestion threshold (0.0 - 1.0)
    pub congestion_threshold: f64,
    /// Throttling rate when congested
    pub throttling_rate: f64,
    /// Minimum throttling rate
    pub min_throttling_rate: f64,
    /// Recovery rate (how fast to recover from throttling)
    pub recovery_rate: f64,
    /// Burst tolerance
    pub burst_tolerance: Duration,
}

/// Active throttling session
#[derive(Debug, Clone)]
pub struct ThrottlingSession {
    /// Stream ID
    pub stream_id: StreamId,
    /// Current throttling rate
    pub current_rate: f64,
    /// Target throttling rate
    pub target_rate: f64,
    /// Session start time
    pub started_at: Instant,
    /// Last adjustment time
    pub last_adjusted: Instant,
    /// Adjustment history
    pub adjustment_history: Vec<ThrottlingAdjustment>,
}

/// Throttling adjustment record
#[derive(Debug, Clone)]
pub struct ThrottlingAdjustment {
    /// Timestamp
    pub timestamp: Instant,
    /// Old rate
    pub old_rate: f64,
    /// New rate
    pub new_rate: f64,
    /// Reason for adjustment
    pub reason: String,
}

/// Congestion detector
pub struct CongestionDetector {
    /// Congestion thresholds
    thresholds: CongestionThresholds,
    /// Detection window
    detection_window: Duration,
    /// Current congestion state
    congestion_state: Arc<RwLock<CongestionState>>,
}

/// Congestion thresholds
#[derive(Debug, Clone)]
pub struct CongestionThresholds {
    /// Bandwidth utilization threshold
    pub bandwidth_threshold: f64,
    /// Queue depth threshold
    pub queue_threshold: usize,
    /// Latency threshold
    pub latency_threshold: Duration,
    /// Error rate threshold
    pub error_rate_threshold: f64,
}

/// Congestion state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CongestionState {
    /// Is congested
    pub congested: bool,
    /// Congestion level (0.0 - 1.0)
    pub level: f64,
    /// Congestion factors
    pub factors: Vec<CongestionFactor>,
    /// Detection timestamp (UNIX timestamp in milliseconds)
    pub detected_at: Option<u64>,
    /// Expected recovery time
    pub expected_recovery: Option<Duration>,
}

/// Congestion factor
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CongestionFactor {
    /// Factor type
    pub factor_type: CongestionFactorType,
    /// Severity (0.0 - 1.0)
    pub severity: f64,
    /// Description
    pub description: String,
}

/// Congestion factor types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CongestionFactorType {
    BandwidthUtilization,
    QueueDepth,
    Latency,
    ErrorRate,
    ResourceExhaustion,
}

/// Throttling reasons
#[derive(Debug, Clone)]
pub enum ThrottlingReason {
    Congestion,
    QoSViolation,
    ResourceLimit,
    FairShare,
    Manual,
}

/// Bandwidth configuration
#[derive(Debug, Clone)]
pub struct BandwidthConfig {
    /// Total bandwidth limit (bytes/sec)
    pub total_bandwidth: u64,
    /// QoS class bandwidth ratios
    pub class_ratios: HashMap<QoSClass, f64>,
    /// Enable adaptive allocation
    pub adaptive_allocation: bool,
    /// Enable burst allowance
    pub burst_allowance: bool,
    /// Burst multiplier
    pub burst_multiplier: f64,
    /// Usage tracking interval
    pub tracking_interval: Duration,
    /// Throttling enabled
    pub throttling_enabled: bool,
    /// Congestion detection enabled
    pub congestion_detection: bool,
}

/// Bandwidth metrics
#[derive(Debug, Clone)]
pub struct BandwidthMetrics {
    /// Total bandwidth utilization
    pub total_utilization: f64,
    /// Utilization by QoS class
    pub class_utilization: HashMap<QoSClass, f64>,
    /// Active streams count
    pub active_streams: usize,
    /// Throttling events
    pub throttling_events: u64,
    /// Congestion events
    pub congestion_events: u64,
    /// Average allocation latency
    pub avg_allocation_latency: Duration,
    /// Bandwidth efficiency
    pub efficiency: f64,
}

/// Stream ID type
pub type StreamId = uuid::Uuid;

/// Bandwidth allocation result
#[derive(Debug, Clone)]
pub struct BandwidthAllocation {
    /// Stream ID
    pub stream_id: StreamId,
    /// Allocated bandwidth (bytes/sec)
    pub allocated_bandwidth: u64,
    /// QoS class
    pub qos_class: QoSClass,
    /// Allocation success
    pub success: bool,
    /// Allocation latency
    pub allocation_latency: Duration,
    /// Throttling applied
    pub throttling_applied: bool,
    /// Expected duration
    pub expected_duration: Option<Duration>,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        let mut class_ratios = HashMap::new();
        class_ratios.insert(QoSClass::Critical, 0.3); // 30% for critical
        class_ratios.insert(QoSClass::System, 0.25); // 25% for system
        class_ratios.insert(QoSClass::JobExecution, 0.25); // 25% for job execution
        class_ratios.insert(QoSClass::DataTransfer, 0.15); // 15% for data transfer
        class_ratios.insert(QoSClass::LogsMetrics, 0.05); // 5% for logs/metrics

        Self {
            total_bandwidth: 1_000_000_000, // 1 Gbps
            class_ratios,
            adaptive_allocation: true,
            burst_allowance: true,
            burst_multiplier: 2.0,
            tracking_interval: Duration::from_secs(1),
            throttling_enabled: true,
            congestion_detection: true,
        }
    }
}

impl BandwidthAllocator {
    /// Create new bandwidth allocator
    pub fn new() -> Self {
        Self::with_config(BandwidthConfig::default())
    }

    /// Create bandwidth allocator with custom configuration
    pub fn with_config(config: BandwidthConfig) -> Self {
        let allocator = Self {
            total_bandwidth: Arc::new(RwLock::new(config.total_bandwidth)),
            class_allocations: Arc::new(DashMap::new()),
            stream_allocations: Arc::new(DashMap::new()),
            usage_tracker: Arc::new(BandwidthUsageTracker::new(config.tracking_interval)),
            throttling_manager: Arc::new(ThrottlingManager::new()),
            metrics: Arc::new(RwLock::new(BandwidthMetrics::default())),
            config: config.clone(),
        };

        // Initialize class allocations
        allocator.initialize_class_allocations();
        allocator
    }

    /// Initialize bandwidth allocations for QoS classes
    fn initialize_class_allocations(&self) {
        for (qos_class, ratio) in &self.config.class_ratios {
            let guaranteed_bandwidth = (self.config.total_bandwidth as f64 * ratio) as u64;
            let max_bandwidth = if self.config.burst_allowance {
                (guaranteed_bandwidth as f64 * self.config.burst_multiplier) as u64
            } else {
                guaranteed_bandwidth
            };

            let allocation = ClassBandwidthAllocation {
                qos_class: *qos_class,
                guaranteed_bandwidth,
                max_bandwidth,
                current_usage: Arc::new(RwLock::new(0)),
                priority_weight: *ratio,
                burst_allowance: max_bandwidth - guaranteed_bandwidth,
                burst_usage: Arc::new(RwLock::new(0)),
                last_updated: Instant::now(),
            };

            self.class_allocations.insert(*qos_class, allocation);
        }
    }

    /// Allocate bandwidth for stream
    pub async fn allocate_bandwidth(
        &self,
        stream_id: StreamId,
        qos_class: QoSClass,
        requested_bandwidth: u64,
        qos_params: &QoSParams,
    ) -> Result<BandwidthAllocation> {
        let start_time = Instant::now();

        // Check if class allocation exists
        let class_allocation = self.class_allocations.get(&qos_class).ok_or_else(|| {
            ValkyrieError::InvalidQoSClass(format!("Unknown QoS class: {:?}", qos_class))
        })?;

        // Calculate available bandwidth for this class
        let available_bandwidth = self
            .calculate_available_bandwidth(&class_allocation)
            .await?;

        // Determine actual allocation
        let allocated_bandwidth = std::cmp::min(requested_bandwidth, available_bandwidth);

        // Check for congestion and apply throttling if needed
        let (final_bandwidth, throttling_applied) = if self.config.throttling_enabled {
            self.apply_throttling(stream_id, qos_class, allocated_bandwidth)
                .await?
        } else {
            (allocated_bandwidth, false)
        };

        // Create stream allocation
        let stream_allocation = StreamBandwidthAllocation {
            stream_id,
            qos_class,
            allocated_bandwidth: final_bandwidth,
            current_usage: Arc::new(RwLock::new(0)),
            usage_history: Arc::new(RwLock::new(Vec::new())),
            throttling_state: Arc::new(RwLock::new(ThrottlingState::default())),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };

        // Store allocation
        self.stream_allocations.insert(stream_id, stream_allocation);

        // Update class usage
        self.update_class_usage(&class_allocation, final_bandwidth)
            .await;

        // Update metrics
        self.update_allocation_metrics(start_time.elapsed()).await;

        Ok(BandwidthAllocation {
            stream_id,
            allocated_bandwidth: final_bandwidth,
            qos_class,
            success: final_bandwidth > 0,
            allocation_latency: start_time.elapsed(),
            throttling_applied,
            expected_duration: Some(qos_params.max_latency),
        })
    }

    /// Calculate available bandwidth for QoS class
    async fn calculate_available_bandwidth(
        &self,
        class_allocation: &ClassBandwidthAllocation,
    ) -> Result<u64> {
        let current_usage = *class_allocation.current_usage.read().await;
        let available = if current_usage < class_allocation.guaranteed_bandwidth {
            class_allocation.guaranteed_bandwidth - current_usage
        } else if current_usage < class_allocation.max_bandwidth {
            class_allocation.max_bandwidth - current_usage
        } else {
            0
        };

        Ok(available)
    }

    /// Apply throttling based on congestion and policies
    async fn apply_throttling(
        &self,
        stream_id: StreamId,
        qos_class: QoSClass,
        requested_bandwidth: u64,
    ) -> Result<(u64, bool)> {
        // Check for congestion
        let congestion_state = self
            .throttling_manager
            .congestion_detector
            .congestion_state
            .read()
            .await;

        if !congestion_state.congested {
            return Ok((requested_bandwidth, false));
        }

        // Get throttling policy for QoS class
        if let Some(policy) = self.throttling_manager.policies.get(&qos_class) {
            let throttling_rate = self
                .calculate_throttling_rate(&policy, &congestion_state)
                .await;
            let throttled_bandwidth = (requested_bandwidth as f64 * throttling_rate) as u64;

            // Create or update throttling session
            self.create_throttling_session(stream_id, throttling_rate)
                .await;

            Ok((throttled_bandwidth, true))
        } else {
            Ok((requested_bandwidth, false))
        }
    }

    /// Calculate throttling rate based on congestion
    async fn calculate_throttling_rate(
        &self,
        policy: &ThrottlingPolicy,
        congestion_state: &CongestionState,
    ) -> f64 {
        let base_rate = policy.throttling_rate;
        let congestion_factor = congestion_state.level;

        // Adjust throttling rate based on congestion severity
        let adjusted_rate = base_rate * (1.0 - congestion_factor * 0.5);

        // Ensure minimum throttling rate
        adjusted_rate.max(policy.min_throttling_rate)
    }

    /// Create throttling session
    async fn create_throttling_session(&self, stream_id: StreamId, throttling_rate: f64) {
        let session = ThrottlingSession {
            stream_id,
            current_rate: throttling_rate,
            target_rate: throttling_rate,
            started_at: Instant::now(),
            last_adjusted: Instant::now(),
            adjustment_history: Vec::new(),
        };

        self.throttling_manager
            .active_throttling
            .insert(stream_id, session);
    }

    /// Update class bandwidth usage
    async fn update_class_usage(
        &self,
        class_allocation: &ClassBandwidthAllocation,
        additional_usage: u64,
    ) {
        let mut current_usage = class_allocation.current_usage.write().await;
        *current_usage += additional_usage;
    }

    /// Release bandwidth allocation
    pub async fn release_bandwidth(&self, stream_id: StreamId) -> Result<()> {
        if let Some((_, stream_allocation)) = self.stream_allocations.remove(&stream_id) {
            // Update class usage
            if let Some(class_allocation) = self.class_allocations.get(&stream_allocation.qos_class)
            {
                let mut current_usage = class_allocation.current_usage.write().await;
                *current_usage =
                    current_usage.saturating_sub(stream_allocation.allocated_bandwidth);
            }

            // Remove throttling session if exists
            self.throttling_manager.active_throttling.remove(&stream_id);

            info!("Released bandwidth allocation for stream {}", stream_id);
        }

        Ok(())
    }

    /// Update bandwidth usage for stream
    pub async fn update_usage(
        &self,
        stream_id: StreamId,
        bytes_transferred: u64,
        duration: Duration,
    ) -> Result<()> {
        if let Some(stream_allocation) = self.stream_allocations.get(&stream_id) {
            // Update current usage
            let rate = bytes_transferred as f64 / duration.as_secs_f64();
            *stream_allocation.current_usage.write().await = rate as u64;

            // Add to usage history
            let usage_point = UsageDataPoint {
                timestamp: Instant::now(),
                bytes: bytes_transferred,
                duration,
                rate,
            };

            let mut history = stream_allocation.usage_history.write().await;
            history.push(usage_point);

            // Keep only recent history
            if history.len() > 100 {
                history.remove(0);
            }

            // Update last activity
            let mut allocation = stream_allocation.clone();
            allocation.last_activity = Instant::now();
            self.stream_allocations.insert(stream_id, allocation);

            // Track usage globally
            self.usage_tracker
                .record_usage(stream_allocation.qos_class, bytes_transferred, duration)
                .await;
        }

        Ok(())
    }

    /// Get bandwidth metrics
    pub async fn metrics(&self) -> BandwidthMetrics {
        let mut metrics = self.metrics.read().await.clone();

        // Update real-time metrics
        metrics.active_streams = self.stream_allocations.len();

        // Calculate total utilization
        let total_bandwidth = *self.total_bandwidth.read().await;
        let mut total_used = 0u64;

        for class_allocation in self.class_allocations.iter() {
            let usage = *class_allocation.current_usage.read().await;
            total_used += usage;
            metrics.class_utilization.insert(
                class_allocation.qos_class,
                usage as f64 / class_allocation.guaranteed_bandwidth as f64,
            );
        }

        metrics.total_utilization = total_used as f64 / total_bandwidth as f64;
        metrics.efficiency = self.calculate_efficiency().await;

        metrics
    }

    /// Calculate bandwidth efficiency
    async fn calculate_efficiency(&self) -> f64 {
        // Simplified efficiency calculation
        let total_allocated = self
            .stream_allocations
            .iter()
            .map(|entry| entry.allocated_bandwidth)
            .sum::<u64>();

        let total_used = self
            .stream_allocations
            .iter()
            .map(|entry| {
                // This would need async access in practice
                1000u64 // Placeholder
            })
            .sum::<u64>();

        if total_allocated > 0 {
            total_used as f64 / total_allocated as f64
        } else {
            0.0
        }
    }

    /// Update allocation metrics
    async fn update_allocation_metrics(&self, latency: Duration) {
        let mut metrics = self.metrics.write().await;

        // Update average allocation latency
        let total_allocations = metrics.active_streams + 1;
        let total_latency = metrics.avg_allocation_latency.as_nanos()
            * (total_allocations - 1) as u128
            + latency.as_nanos();
        metrics.avg_allocation_latency =
            Duration::from_nanos((total_latency / total_allocations as u128) as u64);
    }

    /// Perform adaptive bandwidth reallocation
    pub async fn adaptive_reallocation(&self) -> Result<()> {
        if !self.config.adaptive_allocation {
            return Ok(());
        }

        // Analyze usage patterns
        let usage_analysis = self.usage_tracker.analyze_usage_patterns().await;

        // Reallocate based on analysis
        for (qos_class, analysis) in usage_analysis {
            if let Some(mut class_allocation) = self.class_allocations.get_mut(&qos_class) {
                // Adjust allocation based on usage trends
                if analysis.trend > 1.2 {
                    // Increase allocation for growing usage
                    class_allocation.guaranteed_bandwidth =
                        (class_allocation.guaranteed_bandwidth as f64 * 1.1) as u64;
                } else if analysis.trend < 0.8 {
                    // Decrease allocation for declining usage
                    class_allocation.guaranteed_bandwidth =
                        (class_allocation.guaranteed_bandwidth as f64 * 0.9) as u64;
                }

                class_allocation.last_updated = Instant::now();
            }
        }

        info!("Performed adaptive bandwidth reallocation");
        Ok(())
    }
}

impl BandwidthUsageTracker {
    /// Create new usage tracker
    pub fn new(sampling_interval: Duration) -> Self {
        Self {
            class_usage: Arc::new(DashMap::new()),
            global_usage: Arc::new(RwLock::new(Vec::new())),
            sampling_interval,
            max_samples: 1000,
        }
    }

    /// Record bandwidth usage
    pub async fn record_usage(&self, qos_class: QoSClass, bytes: u64, duration: Duration) {
        let usage_point = UsageDataPoint {
            timestamp: Instant::now(),
            bytes,
            duration,
            rate: bytes as f64 / duration.as_secs_f64(),
        };

        // Record for specific class
        let class_samples = self
            .class_usage
            .entry(qos_class)
            .or_insert_with(|| Arc::new(RwLock::new(Vec::new())));

        let mut samples = class_samples.write().await;
        samples.push(usage_point.clone());

        if samples.len() > self.max_samples {
            samples.remove(0);
        }

        // Record globally
        let mut global_samples = self.global_usage.write().await;
        global_samples.push(usage_point);

        if global_samples.len() > self.max_samples {
            global_samples.remove(0);
        }
    }

    /// Analyze usage patterns
    pub async fn analyze_usage_patterns(&self) -> HashMap<QoSClass, UsageAnalysis> {
        let mut analysis = HashMap::new();

        for entry in self.class_usage.iter() {
            let qos_class = *entry.key();
            let samples = entry.value().read().await;

            if samples.len() < 2 {
                continue;
            }

            let total_bytes: u64 = samples.iter().map(|s| s.bytes).sum();
            let avg_rate = samples.iter().map(|s| s.rate).sum::<f64>() / samples.len() as f64;

            // Calculate trend (simplified linear regression)
            let trend = if samples.len() > 10 {
                let recent_avg = samples.iter().rev().take(5).map(|s| s.rate).sum::<f64>() / 5.0;
                let older_avg = samples
                    .iter()
                    .rev()
                    .skip(5)
                    .take(5)
                    .map(|s| s.rate)
                    .sum::<f64>()
                    / 5.0;
                if older_avg > 0.0 {
                    recent_avg / older_avg
                } else {
                    1.0
                }
            } else {
                1.0
            };

            analysis.insert(
                qos_class,
                UsageAnalysis {
                    total_bytes,
                    avg_rate,
                    trend,
                    sample_count: samples.len(),
                },
            );
        }

        analysis
    }
}

/// Usage analysis result
#[derive(Debug, Clone)]
pub struct UsageAnalysis {
    /// Total bytes transferred
    pub total_bytes: u64,
    /// Average transfer rate
    pub avg_rate: f64,
    /// Usage trend (1.0 = stable, >1.0 = increasing, <1.0 = decreasing)
    pub trend: f64,
    /// Number of samples
    pub sample_count: usize,
}

impl ThrottlingManager {
    /// Create new throttling manager
    pub fn new() -> Self {
        let mut policies = DashMap::new();

        // Initialize default throttling policies
        for qos_class in [
            QoSClass::Critical,
            QoSClass::System,
            QoSClass::JobExecution,
            QoSClass::DataTransfer,
            QoSClass::LogsMetrics,
        ] {
            let policy = ThrottlingPolicy {
                qos_class,
                congestion_threshold: match qos_class {
                    QoSClass::Critical => 0.9,     // High threshold for critical
                    QoSClass::System => 0.8,       // Medium-high for system
                    QoSClass::JobExecution => 0.7, // Medium for job execution
                    QoSClass::DataTransfer => 0.6, // Lower for data transfer
                    QoSClass::LogsMetrics => 0.5,  // Lowest for logs/metrics
                },
                throttling_rate: match qos_class {
                    QoSClass::Critical => 0.9,     // Light throttling for critical
                    QoSClass::System => 0.8,       // Medium throttling for system
                    QoSClass::JobExecution => 0.7, // More throttling for jobs
                    QoSClass::DataTransfer => 0.5, // Heavy throttling for data
                    QoSClass::LogsMetrics => 0.3,  // Heaviest for logs/metrics
                },
                min_throttling_rate: 0.1,
                recovery_rate: 0.05,
                burst_tolerance: Duration::from_secs(5),
            };

            policies.insert(qos_class, policy);
        }

        Self {
            policies: Arc::new(policies),
            active_throttling: Arc::new(DashMap::new()),
            congestion_detector: Arc::new(CongestionDetector::new()),
        }
    }
}

impl CongestionDetector {
    /// Create new congestion detector
    pub fn new() -> Self {
        Self {
            thresholds: CongestionThresholds {
                bandwidth_threshold: 0.8,
                queue_threshold: 1000,
                latency_threshold: Duration::from_millis(100),
                error_rate_threshold: 0.05,
            },
            detection_window: Duration::from_secs(30),
            congestion_state: Arc::new(RwLock::new(CongestionState::default())),
        }
    }

    /// Detect congestion based on metrics
    pub async fn detect_congestion(&self, metrics: &BandwidthMetrics) -> Result<bool> {
        let mut factors = Vec::new();

        // Check bandwidth utilization
        if metrics.total_utilization > self.thresholds.bandwidth_threshold {
            factors.push(CongestionFactor {
                factor_type: CongestionFactorType::BandwidthUtilization,
                severity: (metrics.total_utilization - self.thresholds.bandwidth_threshold)
                    / (1.0 - self.thresholds.bandwidth_threshold),
                description: format!(
                    "Bandwidth utilization: {:.1}%",
                    metrics.total_utilization * 100.0
                ),
            });
        }

        // Update congestion state
        let congested = !factors.is_empty();
        let level = factors.iter().map(|f| f.severity).fold(0.0, f64::max);

        let mut state = self.congestion_state.write().await;
        state.congested = congested;
        state.level = level;
        state.factors = factors;

        if congested && state.detected_at.is_none() {
            state.detected_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
        } else if !congested {
            state.detected_at = None;
        }

        Ok(congested)
    }
}

impl Default for ThrottlingState {
    fn default() -> Self {
        Self {
            active: false,
            rate: 1.0,
            reason: ThrottlingReason::Manual,
            started_at: None,
            expected_duration: None,
        }
    }
}

impl Default for CongestionState {
    fn default() -> Self {
        Self {
            congested: false,
            level: 0.0,
            factors: Vec::new(),
            detected_at: None,
            expected_recovery: None,
        }
    }
}

impl Default for BandwidthMetrics {
    fn default() -> Self {
        Self {
            total_utilization: 0.0,
            class_utilization: HashMap::new(),
            active_streams: 0,
            throttling_events: 0,
            congestion_events: 0,
            avg_allocation_latency: Duration::from_nanos(0),
            efficiency: 0.0,
        }
    }
}

impl std::fmt::Display for ThrottlingReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThrottlingReason::Congestion => write!(f, "Congestion"),
            ThrottlingReason::QoSViolation => write!(f, "QoS Violation"),
            ThrottlingReason::ResourceLimit => write!(f, "Resource Limit"),
            ThrottlingReason::FairShare => write!(f, "Fair Share"),
            ThrottlingReason::Manual => write!(f, "Manual"),
        }
    }
}
