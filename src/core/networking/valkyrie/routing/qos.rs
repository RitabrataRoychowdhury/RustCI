// QoS-Aware Routing Implementation
// Task 3.1.4: QoS Integration and Traffic Management

use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};

/// QoS-aware router for priority-based routing
pub struct QoSRouter {
    priority_manager: Arc<PriorityManager>,
    sla_enforcer: Arc<SLAEnforcer>,
    traffic_shaper: Arc<TrafficShaper>,
    bandwidth_manager: Arc<BandwidthManager>,
    qos_scheduler: Arc<QoSScheduler>,
}

/// Priority management for different message classes
pub struct PriorityManager {
    priority_queues: Arc<RwLock<HashMap<MessagePriority, PriorityQueue<QueuedMessage>>>>,
    scheduler: Arc<QoSScheduler>,
    preemption_handler: Arc<PreemptionHandler>,
    priority_weights: HashMap<MessagePriority, f64>,
}

/// Queued message with metadata
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub message_id: Uuid,
    pub routing_context: RoutingContext,
    pub queued_at: Instant,
    pub deadline: Option<Instant>,
    pub retry_count: u32,
    pub priority_boost: f64,
}

/// Priority queue implementation
pub struct PriorityQueue<T> {
    items: VecDeque<T>,
    max_size: usize,
    dropped_items: AtomicU64,
}

/// SLA enforcement and monitoring
pub struct SLAEnforcer {
    sla_definitions: Arc<RwLock<HashMap<ServiceId, SLADefinition>>>,
    violation_detector: Arc<ViolationDetector>,
    remediation_engine: Arc<RemediationEngine>,
    sla_metrics: Arc<SLAMetrics>,
}

/// SLA definition for a service
#[derive(Debug, Clone)]
pub struct SLADefinition {
    pub service_id: ServiceId,
    pub max_latency: Duration,
    pub min_throughput: u64,
    pub max_error_rate: f64,
    pub availability_target: f64,
    pub penalties: Vec<SLAPenalty>,
    pub measurement_window: Duration,
    pub grace_period: Duration,
}

/// SLA penalty for violations
#[derive(Debug, Clone)]
pub struct SLAPenalty {
    pub violation_type: SLAViolationType,
    pub penalty_type: PenaltyType,
    pub severity: PenaltySeverity,
    pub duration: Duration,
}

/// Types of SLA violations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SLAViolationType {
    LatencyExceeded,
    ThroughputBelowThreshold,
    ErrorRateExceeded,
    AvailabilityBelowTarget,
    CustomViolation(u32),
}

/// Types of penalties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenaltyType {
    PriorityReduction,
    BandwidthReduction,
    RouteRestriction,
    ServiceSuspension,
    CustomPenalty(u32),
}

/// Penalty severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PenaltySeverity {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Traffic shaping for bandwidth management
pub struct TrafficShaper {
    bandwidth_allocations: Arc<RwLock<HashMap<MessagePriority, BandwidthAllocation>>>,
    token_buckets: Arc<RwLock<HashMap<MessagePriority, TokenBucket>>>,
    congestion_controller: Arc<CongestionController>,
    shaping_policies: Arc<RwLock<Vec<ShapingPolicy>>>,
}

/// Bandwidth allocation per priority
#[derive(Debug)]
pub struct BandwidthAllocation {
    pub priority: MessagePriority,
    pub guaranteed_bandwidth: u64,
    pub max_bandwidth: u64,
    pub burst_allowance: u64,
    pub current_usage: AtomicU64,
    pub allocation_window: Duration,
}

impl Clone for BandwidthAllocation {
    fn clone(&self) -> Self {
        Self {
            priority: self.priority,
            guaranteed_bandwidth: self.guaranteed_bandwidth,
            max_bandwidth: self.max_bandwidth,
            burst_allowance: self.burst_allowance,
            current_usage: AtomicU64::new(
                self.current_usage
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            allocation_window: self.allocation_window,
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    pub capacity: u64,
    pub tokens: AtomicU64,
    pub refill_rate: u64,
    pub last_refill: Arc<RwLock<Instant>>,
}

/// Congestion control mechanism
pub struct CongestionController {
    congestion_state: Arc<RwLock<CongestionState>>,
    congestion_detector: Arc<CongestionDetector>,
    backpressure_manager: Arc<BackpressureManager>,
}

/// Current congestion state
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CongestionState {
    Normal,
    Mild,
    Moderate,
    Severe,
    Critical,
}

/// Bandwidth management across the system
pub struct BandwidthManager {
    total_bandwidth: u64,
    reserved_bandwidth: Arc<RwLock<HashMap<ServiceId, u64>>>,
    dynamic_allocations: Arc<RwLock<HashMap<MessagePriority, u64>>>,
    utilization_monitor: Arc<UtilizationMonitor>,
}

/// QoS scheduling algorithms
pub struct QoSScheduler {
    scheduling_algorithm: SchedulingAlgorithm,
    scheduler_state: Arc<RwLock<SchedulerState>>,
    fairness_controller: Arc<FairnessController>,
}

/// Available scheduling algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingAlgorithm {
    StrictPriority,
    WeightedFairQueuing,
    DeficitRoundRobin,
    ClassBasedQueuing,
    HierarchicalFairService,
}

/// Scheduler internal state
#[derive(Debug, Clone, Default)]
pub struct SchedulerState {
    pub queue_weights: HashMap<MessagePriority, f64>,
    pub deficit_counters: HashMap<MessagePriority, u64>,
    pub service_credits: HashMap<MessagePriority, u64>,
    pub last_service_time: HashMap<MessagePriority, Instant>,
}

/// Fairness control to prevent starvation
pub struct FairnessController {
    starvation_detector: Arc<StarvationDetector>,
    fairness_metrics: Arc<FairnessMetrics>,
    anti_starvation_policies: Arc<RwLock<Vec<AntiStarvationPolicy>>>,
}

/// Traffic shaping policy
#[derive(Debug, Clone)]
pub struct ShapingPolicy {
    pub name: String,
    pub conditions: Vec<ShapingCondition>,
    pub actions: Vec<ShapingAction>,
    pub priority: u8,
    pub enabled: bool,
}

/// Condition for applying shaping
#[derive(Debug, Clone)]
pub enum ShapingCondition {
    PriorityEquals(MessagePriority),
    SourceNode(NodeId),
    DestinationNode(NodeId),
    ServiceId(ServiceId),
    BandwidthUsageAbove(f64),
    LatencyAbove(Duration),
    ErrorRateAbove(f64),
    TimeWindow(Duration, Duration), // start, end
    Custom(String),                 // Custom condition expression
}

/// Action to take when shaping
#[derive(Debug, Clone)]
pub enum ShapingAction {
    LimitBandwidth(u64),
    DelayMessages(Duration),
    DropMessages(f64), // Drop probability
    RerouteMessages(Vec<NodeId>),
    ChangePriority(MessagePriority),
    ApplyBackpressure,
    Custom(String), // Custom action
}

impl QoSRouter {
    pub fn new(total_bandwidth: u64, scheduling_algorithm: SchedulingAlgorithm) -> Self {
        Self {
            priority_manager: Arc::new(PriorityManager::new()),
            sla_enforcer: Arc::new(SLAEnforcer::new()),
            traffic_shaper: Arc::new(TrafficShaper::new()),
            bandwidth_manager: Arc::new(BandwidthManager::new(total_bandwidth)),
            qos_scheduler: Arc::new(QoSScheduler::new(scheduling_algorithm)),
        }
    }

    /// Route message with QoS considerations
    pub async fn route_with_qos(
        &self,
        context: &RoutingContext,
        routing_engine: &AlgorithmManager,
        topology: &NetworkTopology,
    ) -> Result<Route, RoutingError> {
        // Check SLA requirements
        self.sla_enforcer.check_sla_compliance(context).await?;

        // Apply traffic shaping
        self.traffic_shaper.apply_shaping(context).await?;

        // Reserve bandwidth if needed
        if let Some(min_bandwidth) = context.qos_requirements.min_bandwidth {
            self.bandwidth_manager
                .reserve_bandwidth(
                    &context.message_id,
                    min_bandwidth,
                    context.qos_requirements.priority,
                )
                .await?;
        }

        // Calculate QoS-aware route
        let mut qos_context = context.clone();
        self.enhance_context_with_qos(&mut qos_context).await;

        let route = routing_engine
            .calculate_route(&qos_context, topology)
            .await?;

        // Validate route meets QoS requirements
        self.validate_route_qos(&route, &context.qos_requirements)
            .await?;

        // Schedule message based on priority
        self.qos_scheduler.schedule_message(context).await?;

        Ok(route)
    }

    /// Add SLA definition for a service
    pub async fn add_sla_definition(&self, sla: SLADefinition) {
        self.sla_enforcer.add_sla_definition(sla).await;
    }

    /// Add traffic shaping policy
    pub async fn add_shaping_policy(&self, policy: ShapingPolicy) {
        self.traffic_shaper.add_policy(policy).await;
    }

    /// Get QoS metrics
    pub async fn get_qos_metrics(&self) -> QoSMetricsSnapshot {
        QoSMetricsSnapshot {
            priority_queue_depths: self.priority_manager.get_queue_depths().await,
            sla_violations: self.sla_enforcer.get_violation_count().await,
            bandwidth_utilization: self.bandwidth_manager.get_utilization().await,
            congestion_state: self.traffic_shaper.get_congestion_state().await,
            scheduling_fairness: self.qos_scheduler.get_fairness_metrics().await,
            timestamp: Instant::now(),
        }
    }

    // Private helper methods
    async fn enhance_context_with_qos(&self, context: &mut RoutingContext) {
        // Add QoS-specific routing hints
        let priority_weight = self
            .priority_manager
            .get_priority_weight(context.qos_requirements.priority);

        if let Some(latency_weight) = context.routing_hints.latency_weight {
            context.routing_hints.latency_weight = Some(latency_weight * priority_weight);
        } else {
            context.routing_hints.latency_weight = Some(priority_weight);
        }

        // Adjust cost weight based on priority
        if let Some(cost_weight) = context.routing_hints.cost_weight {
            context.routing_hints.cost_weight = Some(cost_weight / priority_weight);
        } else {
            context.routing_hints.cost_weight = Some(1.0 / priority_weight);
        }
    }

    async fn validate_route_qos(
        &self,
        route: &Route,
        qos_requirements: &QoSRequirements,
    ) -> Result<(), RoutingError> {
        // Check latency requirement
        if let Some(max_latency) = qos_requirements.max_latency {
            if route.estimated_latency > max_latency {
                return Err(RoutingError::QoSNotSatisfiable {
                    requirements: qos_requirements.clone(),
                });
            }
        }

        // Check bandwidth requirement
        if let Some(min_bandwidth) = qos_requirements.min_bandwidth {
            if route.bandwidth_available < min_bandwidth {
                return Err(RoutingError::QoSNotSatisfiable {
                    requirements: qos_requirements.clone(),
                });
            }
        }

        // Check reliability requirement
        if let Some(min_reliability) = qos_requirements.reliability_threshold {
            if route.reliability_score < min_reliability {
                return Err(RoutingError::QoSNotSatisfiable {
                    requirements: qos_requirements.clone(),
                });
            }
        }

        Ok(())
    }
}

impl PriorityManager {
    pub fn new() -> Self {
        let mut priority_weights = HashMap::new();
        priority_weights.insert(MessagePriority::Critical, 4.0);
        priority_weights.insert(MessagePriority::High, 3.0);
        priority_weights.insert(MessagePriority::Normal, 2.0);
        priority_weights.insert(MessagePriority::Low, 1.0);
        priority_weights.insert(MessagePriority::Background, 0.5);

        Self {
            priority_queues: Arc::new(RwLock::new(HashMap::new())),
            scheduler: Arc::new(QoSScheduler::new(SchedulingAlgorithm::WeightedFairQueuing)),
            preemption_handler: Arc::new(PreemptionHandler::new()),
            priority_weights,
        }
    }

    pub fn get_priority_weight(&self, priority: MessagePriority) -> f64 {
        self.priority_weights.get(&priority).copied().unwrap_or(1.0)
    }

    pub async fn get_queue_depths(&self) -> HashMap<MessagePriority, usize> {
        let queues = self.priority_queues.read().await;
        queues
            .iter()
            .map(|(priority, queue)| (*priority, queue.len()))
            .collect()
    }

    pub async fn enqueue_message(&self, message: QueuedMessage) -> Result<(), QoSError> {
        let priority = message.routing_context.qos_requirements.priority;
        let mut queues = self.priority_queues.write().await;

        let queue = queues.entry(priority).or_insert_with(|| {
            PriorityQueue::new(1000) // Default max size
        });

        queue.enqueue(message)
    }

    pub async fn dequeue_message(&self, priority: MessagePriority) -> Option<QueuedMessage> {
        let mut queues = self.priority_queues.write().await;
        queues.get_mut(&priority)?.dequeue()
    }
}

impl<T> PriorityQueue<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            items: VecDeque::new(),
            max_size,
            dropped_items: AtomicU64::new(0),
        }
    }

    pub fn enqueue(&mut self, item: T) -> Result<(), QoSError> {
        if self.items.len() >= self.max_size {
            self.dropped_items.fetch_add(1, Ordering::Relaxed);
            return Err(QoSError::QueueFull);
        }

        self.items.push_back(item);
        Ok(())
    }

    pub fn dequeue(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl SLAEnforcer {
    pub fn new() -> Self {
        Self {
            sla_definitions: Arc::new(RwLock::new(HashMap::new())),
            violation_detector: Arc::new(ViolationDetector::new()),
            remediation_engine: Arc::new(RemediationEngine::new()),
            sla_metrics: Arc::new(SLAMetrics::new()),
        }
    }

    pub async fn add_sla_definition(&self, sla: SLADefinition) {
        let mut definitions = self.sla_definitions.write().await;
        definitions.insert(sla.service_id, sla);
    }

    pub async fn check_sla_compliance(&self, context: &RoutingContext) -> Result<(), RoutingError> {
        // Implementation would check if the routing context meets SLA requirements
        // For now, we'll just return Ok
        Ok(())
    }

    pub async fn get_violation_count(&self) -> u64 {
        self.sla_metrics.total_violations.load(Ordering::Relaxed)
    }
}

impl TrafficShaper {
    pub fn new() -> Self {
        Self {
            bandwidth_allocations: Arc::new(RwLock::new(HashMap::new())),
            token_buckets: Arc::new(RwLock::new(HashMap::new())),
            congestion_controller: Arc::new(CongestionController::new()),
            shaping_policies: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn apply_shaping(&self, context: &RoutingContext) -> Result<(), RoutingError> {
        let priority = context.qos_requirements.priority;

        // Check token bucket
        let token_buckets = self.token_buckets.read().await;
        if let Some(bucket) = token_buckets.get(&priority) {
            if !bucket.consume_token().await {
                return Err(RoutingError::QoSNotSatisfiable {
                    requirements: context.qos_requirements.clone(),
                });
            }
        }

        // Apply shaping policies
        let policies = self.shaping_policies.read().await;
        for policy in policies.iter() {
            if policy.enabled && self.policy_matches(policy, context).await {
                self.apply_policy_actions(policy, context).await?;
            }
        }

        Ok(())
    }

    pub async fn add_policy(&self, policy: ShapingPolicy) {
        let mut policies = self.shaping_policies.write().await;
        policies.push(policy);
        policies.sort_by_key(|p| std::cmp::Reverse(p.priority));
    }

    pub async fn get_congestion_state(&self) -> CongestionState {
        *self.congestion_controller.congestion_state.read().await
    }

    async fn policy_matches(&self, policy: &ShapingPolicy, context: &RoutingContext) -> bool {
        for condition in &policy.conditions {
            if !self.evaluate_condition(condition, context).await {
                return false;
            }
        }
        true
    }

    async fn evaluate_condition(
        &self,
        condition: &ShapingCondition,
        context: &RoutingContext,
    ) -> bool {
        match condition {
            ShapingCondition::PriorityEquals(priority) => {
                context.qos_requirements.priority == *priority
            }
            ShapingCondition::SourceNode(node_id) => context.source == *node_id,
            ShapingCondition::DestinationNode(node_id) => context.destination == *node_id,
            _ => true, // Simplified for other conditions
        }
    }

    async fn apply_policy_actions(
        &self,
        policy: &ShapingPolicy,
        context: &RoutingContext,
    ) -> Result<(), RoutingError> {
        for action in &policy.actions {
            match action {
                ShapingAction::DelayMessages(duration) => {
                    tokio::time::sleep(*duration).await;
                }
                ShapingAction::ApplyBackpressure => {
                    self.congestion_controller.apply_backpressure().await;
                }
                _ => {
                    // Handle other actions
                }
            }
        }
        Ok(())
    }
}

impl TokenBucket {
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: AtomicU64::new(capacity),
            refill_rate,
            last_refill: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub async fn consume_token(&self) -> bool {
        self.refill_tokens().await;

        let current_tokens = self.tokens.load(Ordering::Relaxed);
        if current_tokens > 0 {
            self.tokens.fetch_sub(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    async fn refill_tokens(&self) {
        let now = Instant::now();
        let mut last_refill = self.last_refill.write().await;
        let elapsed = now.duration_since(*last_refill);

        if elapsed >= Duration::from_secs(1) {
            let tokens_to_add = (elapsed.as_secs() * self.refill_rate).min(self.capacity);
            let current_tokens = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current_tokens + tokens_to_add).min(self.capacity);

            self.tokens.store(new_tokens, Ordering::Relaxed);
            *last_refill = now;
        }
    }
}

impl BandwidthManager {
    pub fn new(total_bandwidth: u64) -> Self {
        Self {
            total_bandwidth,
            reserved_bandwidth: Arc::new(RwLock::new(HashMap::new())),
            dynamic_allocations: Arc::new(RwLock::new(HashMap::new())),
            utilization_monitor: Arc::new(UtilizationMonitor::new()),
        }
    }

    pub async fn reserve_bandwidth(
        &self,
        message_id: &Uuid,
        bandwidth: u64,
        priority: MessagePriority,
    ) -> Result<(), RoutingError> {
        let mut reserved = self.reserved_bandwidth.write().await;
        let total_reserved: u64 = reserved.values().sum();

        if total_reserved + bandwidth > self.total_bandwidth {
            return Err(RoutingError::QoSNotSatisfiable {
                requirements: QoSRequirements {
                    min_bandwidth: Some(bandwidth),
                    priority,
                    ..Default::default()
                },
            });
        }

        reserved.insert(*message_id, bandwidth);
        Ok(())
    }

    pub async fn get_utilization(&self) -> f64 {
        let reserved = self.reserved_bandwidth.read().await;
        let total_reserved: u64 = reserved.values().sum();
        total_reserved as f64 / self.total_bandwidth as f64
    }
}

impl QoSScheduler {
    pub fn new(algorithm: SchedulingAlgorithm) -> Self {
        Self {
            scheduling_algorithm: algorithm,
            scheduler_state: Arc::new(RwLock::new(SchedulerState::default())),
            fairness_controller: Arc::new(FairnessController::new()),
        }
    }

    pub async fn schedule_message(&self, context: &RoutingContext) -> Result<(), RoutingError> {
        match self.scheduling_algorithm {
            SchedulingAlgorithm::StrictPriority => self.schedule_strict_priority(context).await,
            SchedulingAlgorithm::WeightedFairQueuing => {
                self.schedule_weighted_fair_queuing(context).await
            }
            _ => Ok(()), // Other algorithms not implemented
        }
    }

    pub async fn get_fairness_metrics(&self) -> FairnessMetrics {
        self.fairness_controller.fairness_metrics.as_ref().clone()
    }

    async fn schedule_strict_priority(&self, context: &RoutingContext) -> Result<(), RoutingError> {
        // Strict priority scheduling - higher priority messages go first
        Ok(())
    }

    async fn schedule_weighted_fair_queuing(
        &self,
        context: &RoutingContext,
    ) -> Result<(), RoutingError> {
        // Weighted fair queuing implementation
        let mut state = self.scheduler_state.write().await;
        let priority = context.qos_requirements.priority;

        // Update service credits based on priority weight
        let weight = match priority {
            MessagePriority::Critical => 4.0,
            MessagePriority::High => 3.0,
            MessagePriority::Normal => 2.0,
            MessagePriority::Low => 1.0,
            MessagePriority::Background => 0.5,
        };

        state.queue_weights.insert(priority, weight);
        state.last_service_time.insert(priority, Instant::now());

        Ok(())
    }
}

/// QoS-related errors
#[derive(Debug, thiserror::Error)]
pub enum QoSError {
    #[error("Queue is full")]
    QueueFull,

    #[error("SLA violation: {violation:?}")]
    SLAViolation { violation: SLAViolationType },

    #[error("Bandwidth not available: requested {requested}, available {available}")]
    BandwidthUnavailable { requested: u64, available: u64 },

    #[error("Traffic shaping applied: {reason}")]
    TrafficShaped { reason: String },

    #[error("Congestion detected: {state:?}")]
    CongestionDetected { state: CongestionState },
}

/// QoS metrics snapshot
#[derive(Debug, Clone)]
pub struct QoSMetricsSnapshot {
    pub priority_queue_depths: HashMap<MessagePriority, usize>,
    pub sla_violations: u64,
    pub bandwidth_utilization: f64,
    pub congestion_state: CongestionState,
    pub scheduling_fairness: FairnessMetrics,
    pub timestamp: Instant,
}

// Placeholder implementations for supporting types
pub struct PreemptionHandler;
impl PreemptionHandler {
    pub fn new() -> Self {
        Self
    }
}

pub struct ViolationDetector;
impl ViolationDetector {
    pub fn new() -> Self {
        Self
    }
}

pub struct RemediationEngine;
impl RemediationEngine {
    pub fn new() -> Self {
        Self
    }
}

pub struct SLAMetrics {
    pub total_violations: AtomicU64,
}
impl SLAMetrics {
    pub fn new() -> Self {
        Self {
            total_violations: AtomicU64::new(0),
        }
    }
}

pub struct CongestionDetector;
impl CongestionDetector {
    pub fn new() -> Self {
        Self
    }
}

pub struct BackpressureManager;
impl BackpressureManager {
    pub fn new() -> Self {
        Self
    }
}

impl CongestionController {
    pub fn new() -> Self {
        Self {
            congestion_state: Arc::new(RwLock::new(CongestionState::Normal)),
            congestion_detector: Arc::new(CongestionDetector::new()),
            backpressure_manager: Arc::new(BackpressureManager::new()),
        }
    }

    pub async fn apply_backpressure(&self) {
        // Implementation would apply backpressure mechanisms
    }
}

pub struct UtilizationMonitor;
impl UtilizationMonitor {
    pub fn new() -> Self {
        Self
    }
}

pub struct StarvationDetector;
impl StarvationDetector {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone)]
pub struct FairnessMetrics {
    pub queue_service_ratios: HashMap<MessagePriority, f64>,
    pub starvation_events: u64,
    pub fairness_index: f64,
}

impl FairnessController {
    pub fn new() -> Self {
        Self {
            starvation_detector: Arc::new(StarvationDetector::new()),
            fairness_metrics: Arc::new(FairnessMetrics {
                queue_service_ratios: HashMap::new(),
                starvation_events: 0,
                fairness_index: 1.0,
            }),
            anti_starvation_policies: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

pub struct AntiStarvationPolicy;

impl Default for CongestionState {
    fn default() -> Self {
        Self::Normal
    }
}
