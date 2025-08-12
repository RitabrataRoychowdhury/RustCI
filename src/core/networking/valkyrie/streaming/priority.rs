use super::*;
use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::cmp::Ordering;
// tokio::sync imports removed as unused
// tokio::time imports removed as unused

/// Priority-based scheduler for stream multiplexing
pub struct PriorityScheduler {
    /// Priority queues for different stream priorities
    priority_queues: Arc<RwLock<HashMap<StreamPriority, Arc<Mutex<VecDeque<ScheduledMessage>>>>>>,
    /// Scheduling algorithm
    algorithm: SchedulingAlgorithm,
    /// Scheduler configuration
    config: SchedulerConfig,
    /// Scheduling metrics
    metrics: Arc<Mutex<SchedulerMetrics>>,
    /// Active scheduling tasks
    active_tasks: Arc<Mutex<HashMap<StreamId, SchedulingTask>>>,
    /// Bandwidth allocator
    bandwidth_allocator: Arc<BandwidthAllocator>,
    /// QoS manager
    qos_manager: Arc<QosManager>,
}

/// Scheduling algorithms
#[derive(Debug, Clone)]
pub enum SchedulingAlgorithm {
    /// Strict priority scheduling
    StrictPriority,
    /// Weighted fair queuing
    WeightedFairQueuing {
        weights: HashMap<StreamPriority, f64>,
    },
    /// Deficit round robin
    DeficitRoundRobin {
        quantum: HashMap<StreamPriority, u32>,
    },
    /// Hierarchical token bucket
    HierarchicalTokenBucket {
        rates: HashMap<StreamPriority, f64>,
        bursts: HashMap<StreamPriority, u32>,
    },
    /// Custom scheduling logic
    Custom {
        name: String,
        parameters: HashMap<String, f64>,
    },
}

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum queue size per priority
    pub max_queue_size: usize,
    /// Scheduling interval
    pub scheduling_interval: Duration,
    /// Enable preemption
    pub preemption_enabled: bool,
    /// Starvation prevention enabled
    pub starvation_prevention: bool,
    /// Aging factor for preventing starvation
    pub aging_factor: f64,
    /// Maximum scheduling latency
    pub max_scheduling_latency: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
            scheduling_interval: Duration::from_millis(1),
            preemption_enabled: true,
            starvation_prevention: true,
            aging_factor: 0.1,
            max_scheduling_latency: Duration::from_millis(10),
        }
    }
}

/// Scheduled message with priority and timing information
#[derive(Debug, Clone)]
pub struct ScheduledMessage {
    /// Stream identifier
    pub stream_id: StreamId,
    /// Message to be scheduled
    pub message: ValkyrieMessage,
    /// Message priority
    pub priority: StreamPriority,
    /// Effective priority (may be adjusted by aging)
    pub effective_priority: f64,
    /// Timestamp when message was queued
    pub queued_at: Instant,
    /// Deadline for message delivery
    pub deadline: Option<Instant>,
    /// Message size in bytes
    pub size: usize,
    /// QoS requirements
    pub qos_requirements: QosRequirements,
}

impl PartialEq for ScheduledMessage {
    fn eq(&self, other: &Self) -> bool {
        self.effective_priority == other.effective_priority
    }
}

impl Eq for ScheduledMessage {}

impl PartialOrd for ScheduledMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher effective priority comes first (reverse order for max heap)
        other.effective_priority.partial_cmp(&self.effective_priority)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.queued_at.cmp(&other.queued_at)) // FIFO for same priority
    }
}

/// QoS requirements for messages
#[derive(Debug, Clone, Default)]
pub struct QosRequirements {
    /// Maximum acceptable latency
    pub max_latency: Option<Duration>,
    /// Minimum bandwidth requirement
    pub min_bandwidth: Option<f64>,
    /// Reliability requirement (0.0 to 1.0)
    pub reliability: f64,
    /// Jitter tolerance
    pub jitter_tolerance: Option<Duration>,
    /// Custom QoS parameters
    pub custom_params: HashMap<String, f64>,
}

/// Scheduling task for tracking active operations
#[derive(Debug, Clone)]
pub struct SchedulingTask {
    /// Task identifier
    pub task_id: Uuid,
    /// Stream being scheduled
    pub stream_id: StreamId,
    /// Task priority
    pub priority: StreamPriority,
    /// Task start time
    pub started_at: Instant,
    /// Estimated completion time
    pub estimated_completion: Option<Instant>,
    /// Task state
    pub state: TaskState,
}

/// Task states
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Queued,
    Running,
    Preempted,
    Completed,
    Failed,
}

/// Scheduler metrics
#[derive(Debug, Clone, Default)]
pub struct SchedulerMetrics {
    /// Total messages scheduled
    pub messages_scheduled: u64,
    /// Messages by priority
    pub messages_by_priority: HashMap<StreamPriority, u64>,
    /// Average scheduling latency
    pub avg_scheduling_latency: Duration,
    /// Queue utilization by priority
    pub queue_utilization: HashMap<StreamPriority, f64>,
    /// Preemption events
    pub preemption_events: u64,
    /// Starvation prevention activations
    pub starvation_preventions: u64,
    /// Deadline misses
    pub deadline_misses: u64,
}

/// Bandwidth allocator for QoS management
pub struct BandwidthAllocator {
    /// Total available bandwidth
    total_bandwidth: f64,
    /// Allocated bandwidth per priority
    allocations: Arc<RwLock<HashMap<StreamPriority, f64>>>,
    /// Current usage per priority
    current_usage: Arc<RwLock<HashMap<StreamPriority, f64>>>,
    /// Allocation algorithm
    algorithm: BandwidthAllocationAlgorithm,
}

/// Bandwidth allocation algorithms
#[derive(Debug, Clone)]
pub enum BandwidthAllocationAlgorithm {
    /// Fixed allocation per priority
    Fixed(HashMap<StreamPriority, f64>),
    /// Proportional allocation based on demand
    Proportional,
    /// Max-min fair allocation
    MaxMinFair,
    /// Weighted proportional fair
    WeightedProportionalFair(HashMap<StreamPriority, f64>),
}

/// QoS manager for quality of service enforcement
pub struct QosManager {
    /// QoS policies
    policies: Arc<RwLock<HashMap<StreamPriority, QosPolicy>>>,
    /// QoS metrics
    metrics: Arc<Mutex<QosMetrics>>,
    /// SLA monitor
    sla_monitor: Arc<SlaMonitor>,
}

/// QoS policy definition
#[derive(Debug, Clone)]
pub struct QosPolicy {
    /// Maximum latency allowed
    pub max_latency: Duration,
    /// Minimum bandwidth guarantee
    pub min_bandwidth: f64,
    /// Maximum bandwidth limit
    pub max_bandwidth: Option<f64>,
    /// Packet loss tolerance
    pub loss_tolerance: f64,
    /// Jitter tolerance
    pub jitter_tolerance: Duration,
    /// Priority boost for SLA violations
    pub sla_boost: f64,
}

/// QoS metrics
#[derive(Debug, Clone, Default)]
pub struct QosMetrics {
    /// Latency percentiles by priority
    pub latency_percentiles: HashMap<StreamPriority, LatencyPercentiles>,
    /// Bandwidth utilization by priority
    pub bandwidth_utilization: HashMap<StreamPriority, f64>,
    /// SLA violations by priority
    pub sla_violations: HashMap<StreamPriority, u64>,
    /// QoS score by priority (0.0 to 1.0)
    pub qos_scores: HashMap<StreamPriority, f64>,
}

/// Latency percentiles
#[derive(Debug, Clone, Default)]
pub struct LatencyPercentiles {
    pub p50: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p99_9: Duration,
}

/// SLA monitor for tracking service level agreements
pub struct SlaMonitor {
    /// SLA definitions
    slas: Arc<RwLock<HashMap<StreamPriority, ServiceLevelAgreement>>>,
    /// SLA violations
    violations: Arc<Mutex<Vec<SlaViolation>>>,
    /// Monitoring interval
    monitoring_interval: Duration,
}

/// Service Level Agreement definition
#[derive(Debug, Clone)]
pub struct ServiceLevelAgreement {
    /// SLA identifier
    pub id: String,
    /// Target latency
    pub target_latency: Duration,
    /// Target availability
    pub target_availability: f64,
    /// Target throughput
    pub target_throughput: f64,
    /// Measurement window
    pub measurement_window: Duration,
    /// Violation threshold
    pub violation_threshold: f64,
}

/// SLA violation record
#[derive(Debug, Clone)]
pub struct SlaViolation {
    /// Violation timestamp
    pub timestamp: Instant,
    /// Stream priority affected
    pub priority: StreamPriority,
    /// Violation type
    pub violation_type: SlaViolationType,
    /// Severity level
    pub severity: ViolationSeverity,
    /// Measured value
    pub measured_value: f64,
    /// Expected value
    pub expected_value: f64,
}

/// SLA violation types
#[derive(Debug, Clone)]
pub enum SlaViolationType {
    LatencyViolation,
    AvailabilityViolation,
    ThroughputViolation,
    CustomViolation(String),
}

/// Violation severity levels
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl PriorityScheduler {
    /// Create a new priority scheduler
    pub fn new() -> Self {
        let mut priority_queues = HashMap::new();
        
        // Initialize queues for each priority level
        for priority in [
            StreamPriority::Critical,
            StreamPriority::High,
            StreamPriority::Normal,
            StreamPriority::Low,
            StreamPriority::Background,
        ] {
            priority_queues.insert(priority, Arc::new(Mutex::new(VecDeque::new())));
        }

        let algorithm = SchedulingAlgorithm::WeightedFairQueuing {
            weights: [
                (StreamPriority::Critical, 1.0),
                (StreamPriority::High, 0.8),
                (StreamPriority::Normal, 0.6),
                (StreamPriority::Low, 0.4),
                (StreamPriority::Background, 0.2),
            ].iter().cloned().collect(),
        };

        Self {
            priority_queues: Arc::new(RwLock::new(priority_queues)),
            algorithm,
            config: SchedulerConfig::default(),
            metrics: Arc::new(Mutex::new(SchedulerMetrics::default())),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            bandwidth_allocator: Arc::new(BandwidthAllocator::new(1_000_000.0)), // 1 Mbps default
            qos_manager: Arc::new(QosManager::new()),
        }
    }

    /// Schedule a message for transmission
    pub async fn schedule_message(
        &self,
        stream_id: StreamId,
        message: ValkyrieMessage,
        priority: StreamPriority,
        qos_requirements: QosRequirements,
    ) -> Result<()> {
        let scheduled_message = ScheduledMessage {
            stream_id,
            message: message.clone(),
            priority,
            effective_priority: priority as u8 as f64,
            queued_at: Instant::now(),
            deadline: qos_requirements.max_latency.map(|d| Instant::now() + d),
            size: message.payload.len(),
            qos_requirements,
        };

        // Check queue capacity
        let queues = self.priority_queues.read().unwrap();
        if let Some(queue) = queues.get(&priority) {
            let mut queue_guard = queue.lock().unwrap();
            if queue_guard.len() >= self.config.max_queue_size {
                return Err(AppError::ResourceExhausted(
                    format!("Priority queue {:?}: Queue size limit ({}) exceeded", 
                           priority, self.config.max_queue_size)
                ));
            }
            
            queue_guard.push_back(scheduled_message);
        } else {
            return Err(AppError::InternalError {
                component: "scheduler".to_string(),
                message: format!("Priority queue not found: {:?}", priority),
            });
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.messages_scheduled += 1;
            *metrics.messages_by_priority.entry(priority).or_insert(0) += 1;
        }

        tracing::debug!("Scheduled message for stream {} with priority {:?}", stream_id, priority);
        Ok(())
    }

    /// Get next message to transmit based on scheduling algorithm
    pub async fn get_next_message(&self) -> Option<ScheduledMessage> {
        match &self.algorithm {
            SchedulingAlgorithm::StrictPriority => self.get_next_strict_priority().await,
            SchedulingAlgorithm::WeightedFairQueuing { weights } => {
                self.get_next_weighted_fair(weights).await
            },
            SchedulingAlgorithm::DeficitRoundRobin { quantum } => {
                self.get_next_deficit_round_robin(quantum).await
            },
            SchedulingAlgorithm::HierarchicalTokenBucket { rates, bursts } => {
                self.get_next_token_bucket(rates, bursts).await
            },
            SchedulingAlgorithm::Custom { name, parameters } => {
                self.get_next_custom(name, parameters).await
            },
        }
    }

    /// Strict priority scheduling implementation
    async fn get_next_strict_priority(&self) -> Option<ScheduledMessage> {
        let priorities = [
            StreamPriority::Critical,
            StreamPriority::High,
            StreamPriority::Normal,
            StreamPriority::Low,
            StreamPriority::Background,
        ];

        let queues = self.priority_queues.read().unwrap();
        
        for priority in priorities {
            if let Some(queue) = queues.get(&priority) {
                let mut queue_guard = queue.lock().unwrap();
                if let Some(mut message) = queue_guard.pop_front() {
                    // Apply aging to prevent starvation
                    if self.config.starvation_prevention {
                        let age_factor = message.queued_at.elapsed().as_secs_f64() * self.config.aging_factor;
                        message.effective_priority += age_factor;
                    }
                    
                    return Some(message);
                }
            }
        }

        None
    }

    /// Weighted fair queuing implementation
    async fn get_next_weighted_fair(&self, weights: &HashMap<StreamPriority, f64>) -> Option<ScheduledMessage> {
        let queues = self.priority_queues.read().unwrap();
        let mut candidates = Vec::new();

        // Collect candidates from all non-empty queues
        for (priority, queue) in queues.iter() {
            let queue_guard = queue.lock().unwrap();
            if let Some(message) = queue_guard.front() {
                let weight = weights.get(priority).copied().unwrap_or(1.0);
                let age_factor = if self.config.starvation_prevention {
                    message.queued_at.elapsed().as_secs_f64() * self.config.aging_factor
                } else {
                    0.0
                };
                
                let effective_weight = weight + age_factor;
                candidates.push((*priority, effective_weight));
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Select based on weighted probability
        let total_weight: f64 = candidates.iter().map(|(_, w)| w).sum();
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos().hash(&mut hasher);
        let mut random_value = (hasher.finish() as f64 / u64::MAX as f64) * total_weight;
        
        for (priority, weight) in candidates {
            random_value -= weight;
            if random_value <= 0.0 {
                if let Some(queue) = queues.get(&priority) {
                    let mut queue_guard = queue.lock().unwrap();
                    return queue_guard.pop_front();
                }
            }
        }

        None
    }

    /// Deficit round robin implementation
    async fn get_next_deficit_round_robin(&self, _quantum: &HashMap<StreamPriority, u32>) -> Option<ScheduledMessage> {
        // Simplified DRR implementation
        // In a full implementation, we would maintain deficit counters per queue
        self.get_next_strict_priority().await
    }

    /// Hierarchical token bucket implementation
    async fn get_next_token_bucket(
        &self,
        _rates: &HashMap<StreamPriority, f64>,
        _bursts: &HashMap<StreamPriority, u32>,
    ) -> Option<ScheduledMessage> {
        // Simplified token bucket implementation
        // In a full implementation, we would maintain token buckets per priority
        self.get_next_strict_priority().await
    }

    /// Custom scheduling implementation
    async fn get_next_custom(
        &self,
        _name: &str,
        _parameters: &HashMap<String, f64>,
    ) -> Option<ScheduledMessage> {
        // Custom scheduling logic would be implemented here
        // For now, fall back to strict priority
        self.get_next_strict_priority().await
    }

    /// Update scheduling algorithm
    pub fn update_algorithm(&mut self, algorithm: SchedulingAlgorithm) {
        self.algorithm = algorithm;
        tracing::info!("Updated scheduling algorithm");
    }

    /// Get scheduler metrics
    pub fn get_metrics(&self) -> SchedulerMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Get queue statistics
    pub fn get_queue_statistics(&self) -> HashMap<StreamPriority, QueueStatistics> {
        let queues = self.priority_queues.read().unwrap();
        let mut stats = HashMap::new();

        for (priority, queue) in queues.iter() {
            let queue_guard = queue.lock().unwrap();
            let queue_size = queue_guard.len();
            let total_bytes: usize = queue_guard.iter().map(|m| m.size).sum();
            
            let oldest_message_age = queue_guard.front()
                .map(|m| m.queued_at.elapsed())
                .unwrap_or_default();

            stats.insert(*priority, QueueStatistics {
                queue_size,
                total_bytes,
                oldest_message_age,
                utilization: queue_size as f64 / self.config.max_queue_size as f64,
            });
        }

        stats
    }

    /// Apply backpressure to specific priority
    pub async fn apply_backpressure(&self, priority: StreamPriority) -> Result<()> {
        // Implementation would pause scheduling for this priority
        tracing::warn!("Applied backpressure to priority {:?}", priority);
        Ok(())
    }

    /// Release backpressure from specific priority
    pub async fn release_backpressure(&self, priority: StreamPriority) -> Result<()> {
        // Implementation would resume scheduling for this priority
        tracing::info!("Released backpressure from priority {:?}", priority);
        Ok(())
    }
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStatistics {
    pub queue_size: usize,
    pub total_bytes: usize,
    pub oldest_message_age: Duration,
    pub utilization: f64,
}

impl BandwidthAllocator {
    /// Create a new bandwidth allocator
    pub fn new(total_bandwidth: f64) -> Self {
        Self {
            total_bandwidth,
            allocations: Arc::new(RwLock::new(HashMap::new())),
            current_usage: Arc::new(RwLock::new(HashMap::new())),
            algorithm: BandwidthAllocationAlgorithm::Proportional,
        }
    }

    /// Allocate bandwidth for a priority level
    pub fn allocate_bandwidth(&self, priority: StreamPriority, bandwidth: f64) -> Result<()> {
        let mut allocations = self.allocations.write().unwrap();
        let total_allocated: f64 = allocations.values().sum();
        
        if total_allocated + bandwidth > self.total_bandwidth {
            return Err(AppError::ResourceExhausted(
                format!("Insufficient bandwidth: requested {}, available {}", 
                       bandwidth, self.total_bandwidth - total_allocated)
            ));
        }

        allocations.insert(priority, bandwidth);
        tracing::debug!("Allocated {} bandwidth to priority {:?}", bandwidth, priority);
        Ok(())
    }

    /// Check if bandwidth is available for a priority
    pub fn is_bandwidth_available(&self, priority: StreamPriority, required: f64) -> bool {
        let allocations = self.allocations.read().unwrap();
        let current_usage = self.current_usage.read().unwrap();
        
        let allocated = allocations.get(&priority).copied().unwrap_or(0.0);
        let used = current_usage.get(&priority).copied().unwrap_or(0.0);
        
        used + required <= allocated
    }

    /// Update bandwidth usage
    pub fn update_usage(&self, priority: StreamPriority, usage: f64) {
        let mut current_usage = self.current_usage.write().unwrap();
        current_usage.insert(priority, usage);
    }
}

impl QosManager {
    /// Create a new QoS manager
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(QosMetrics::default())),
            sla_monitor: Arc::new(SlaMonitor::new()),
        }
    }

    /// Set QoS policy for a priority level
    pub fn set_policy(&self, priority: StreamPriority, policy: QosPolicy) {
        let mut policies = self.policies.write().unwrap();
        policies.insert(priority, policy);
        tracing::info!("Set QoS policy for priority {:?}", priority);
    }

    /// Check if message meets QoS requirements
    pub fn check_qos(&self, message: &ScheduledMessage) -> bool {
        let policies = self.policies.read().unwrap();
        if let Some(_policy) = policies.get(&message.priority) {
            // Check latency requirement
            if let Some(max_latency) = message.qos_requirements.max_latency {
                if message.queued_at.elapsed() > max_latency {
                    return false;
                }
            }
            
            // Additional QoS checks would be implemented here
            true
        } else {
            true // No policy means no restrictions
        }
    }

    /// Record QoS metrics
    pub fn record_metrics(&self, priority: StreamPriority, latency: Duration, bandwidth: f64) {
        let mut metrics = self.metrics.lock().unwrap();
        
        // Update bandwidth utilization
        metrics.bandwidth_utilization.insert(priority, bandwidth);
        
        // Update latency percentiles (simplified)
        let percentiles = metrics.latency_percentiles.entry(priority).or_default();
        percentiles.p50 = latency; // Simplified - would need proper percentile calculation
        
        tracing::trace!("Recorded QoS metrics for priority {:?}: latency={:?}, bandwidth={}", 
                       priority, latency, bandwidth);
    }
}

impl SlaMonitor {
    /// Create a new SLA monitor
    pub fn new() -> Self {
        Self {
            slas: Arc::new(RwLock::new(HashMap::new())),
            violations: Arc::new(Mutex::new(Vec::new())),
            monitoring_interval: Duration::from_secs(60),
        }
    }

    /// Add SLA definition
    pub fn add_sla(&self, priority: StreamPriority, sla: ServiceLevelAgreement) {
        let mut slas = self.slas.write().unwrap();
        slas.insert(priority, sla);
        tracing::info!("Added SLA for priority {:?}", priority);
    }

    /// Check for SLA violations
    pub fn check_violations(&self, priority: StreamPriority, latency: Duration, throughput: f64) {
        let slas = self.slas.read().unwrap();
        if let Some(sla) = slas.get(&priority) {
            let mut violations = self.violations.lock().unwrap();
            
            // Check latency SLA
            if latency > sla.target_latency {
                violations.push(SlaViolation {
                    timestamp: Instant::now(),
                    priority,
                    violation_type: SlaViolationType::LatencyViolation,
                    severity: ViolationSeverity::Medium,
                    measured_value: latency.as_millis() as f64,
                    expected_value: sla.target_latency.as_millis() as f64,
                });
            }
            
            // Check throughput SLA
            if throughput < sla.target_throughput {
                violations.push(SlaViolation {
                    timestamp: Instant::now(),
                    priority,
                    violation_type: SlaViolationType::ThroughputViolation,
                    severity: ViolationSeverity::Medium,
                    measured_value: throughput,
                    expected_value: sla.target_throughput,
                });
            }
        }
    }

    /// Get recent violations
    pub fn get_violations(&self) -> Vec<SlaViolation> {
        self.violations.lock().unwrap().clone()
    }
}