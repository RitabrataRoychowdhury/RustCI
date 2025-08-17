//! QoS-Aware Stream Router Implementation
//! 
//! Implements intelligent message routing with QoS guarantees, priority-based
//! classification, and adaptive flow control for optimal performance.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use dashmap::DashMap;
use tracing::{debug, info, warn, error};

use super::classifier::MessageClassifier;
use super::QoSClass;
use super::flow_control::FlowControlManager;
use super::bandwidth::{BandwidthAllocator, BandwidthAllocation};
use crate::core::networking::valkyrie::adapters::*;
use crate::error::{Result, ValkyrieError};

/// QoS-Aware Stream Router with intelligent message routing
pub struct QoSStreamRouter {
    /// Message classifier for QoS determination
    classifier: Arc<MessageClassifier>,
    /// Flow control manager
    flow_control: Arc<FlowControlManager>,
    /// Bandwidth allocator
    bandwidth_allocator: Arc<BandwidthAllocator>,
    /// Priority queues for different QoS classes
    priority_queues: Arc<DashMap<QoSClass, Arc<RwLock<VecDeque<QueuedMessage>>>>>,
    /// Route table for message destinations
    route_table: Arc<RwLock<HashMap<String, RouteEntry>>>,
    /// Active streams tracking
    active_streams: Arc<DashMap<StreamId, StreamInfo>>,
    /// Router configuration
    config: QoSRouterConfig,
    /// Performance metrics
    metrics: Arc<RwLock<RouterMetrics>>,
    /// Congestion control semaphore
    congestion_semaphore: Arc<Semaphore>,
}

// QoSClass is imported from classifier module

/// Queued message with routing information
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    /// Original message
    pub message: AdapterMessage,
    /// QoS parameters
    pub qos: QoSParams,
    /// QoS class determined by classifier
    pub qos_class: QoSClass,
    /// Routing destination
    pub destination: RouteDestination,
    /// Enqueue timestamp
    pub enqueued_at: Instant,
    /// Retry count
    pub retry_count: u32,
    /// Deadline for delivery
    pub deadline: Option<Instant>,
}

/// Route entry in the routing table
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// Destination adapter ID
    pub adapter_id: AdapterId,
    /// Route priority (lower = higher priority)
    pub priority: u8,
    /// Route weight for load balancing
    pub weight: u32,
    /// Route health status
    pub health: RouteHealth,
    /// Performance metrics
    pub metrics: RouteMetrics,
    /// Last updated timestamp
    pub last_updated: Instant,
}

/// Route destination types
#[derive(Debug, Clone)]
pub enum RouteDestination {
    /// Direct route to specific adapter
    Direct(AdapterId),
    /// Load balanced across multiple adapters
    LoadBalanced(Vec<AdapterId>),
    /// Service-based routing
    Service(String),
    /// Broadcast to all adapters
    Broadcast,
}

/// Stream information for active streams
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Stream ID
    pub stream_id: StreamId,
    /// QoS class
    pub qos_class: QoSClass,
    /// Source adapter
    pub source: AdapterId,
    /// Destination adapter(s)
    pub destinations: Vec<AdapterId>,
    /// Bandwidth allocation
    pub bandwidth_allocation: BandwidthAllocation,
    /// Stream metrics
    pub metrics: StreamMetrics,
    /// Created timestamp
    pub created_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
}

/// Stream ID type
pub type StreamId = uuid::Uuid;

/// Route health status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Route performance metrics
#[derive(Debug, Clone)]
pub struct RouteMetrics {
    /// Messages routed
    pub messages_routed: u64,
    /// Average latency
    pub avg_latency: Duration,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Bandwidth utilization
    pub bandwidth_utilization: f64,
    /// Last measurement time
    pub last_measured: Instant,
}

/// Stream performance metrics
#[derive(Debug, Clone)]
pub struct StreamMetrics {
    /// Messages sent
    pub messages_sent: u64,
    /// Messages received
    pub messages_received: u64,
    /// Bytes transferred
    pub bytes_transferred: u64,
    /// Average latency
    pub avg_latency: Duration,
    /// Throughput (messages/sec)
    pub throughput: f64,
    /// Error count
    pub error_count: u64,
}

/// Router performance metrics
#[derive(Debug, Clone)]
pub struct RouterMetrics {
    /// Total messages routed
    pub total_messages: u64,
    /// Messages by QoS class
    pub messages_by_class: HashMap<QoSClass, u64>,
    /// Average routing latency
    pub avg_routing_latency: Duration,
    /// Queue depths by class
    pub queue_depths: HashMap<QoSClass, usize>,
    /// Congestion events
    pub congestion_events: u64,
    /// Bandwidth utilization
    pub bandwidth_utilization: f64,
}

/// Classification metrics
#[derive(Debug, Clone)]
pub struct ClassificationMetrics {
    /// Total classifications
    pub total_classifications: u64,
    /// Classifications by class
    pub by_class: HashMap<QoSClass, u64>,
    /// Average classification time
    pub avg_classification_time: Duration,
    /// Misclassification rate
    pub misclassification_rate: f64,
}

/// Classification rule for QoS determination
#[derive(Debug, Clone)]
pub struct ClassificationRule {
    /// Rule name
    pub name: String,
    /// Rule condition
    pub condition: ClassificationCondition,
    /// Target QoS class
    pub target_class: QoSClass,
    /// Rule priority (lower = higher priority)
    pub priority: u8,
    /// Rule enabled
    pub enabled: bool,
}

/// Classification condition
#[derive(Debug, Clone)]
pub enum ClassificationCondition {
    /// Message type condition
    MessageType(AdapterMessageType),
    /// Priority condition
    Priority(MessagePriority),
    /// Metadata condition
    Metadata { key: String, value: String },
    /// Payload size condition
    PayloadSize { min: Option<usize>, max: Option<usize> },
    /// Source adapter condition
    SourceAdapter(AdapterId),
    /// Custom condition function
    Custom(String), // Function name for custom logic
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct QoSRouterConfig {
    /// Maximum queue size per QoS class
    pub max_queue_size: usize,
    /// Queue processing interval
    pub processing_interval: Duration,
    /// Congestion threshold (0.0 - 1.0)
    pub congestion_threshold: f64,
    /// Maximum concurrent streams
    pub max_concurrent_streams: usize,
    /// Route health check interval
    pub health_check_interval: Duration,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Enable adaptive routing
    pub adaptive_routing: bool,
    /// Enable load balancing
    pub load_balancing: bool,
}

impl Default for QoSRouterConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            processing_interval: Duration::from_millis(10),
            congestion_threshold: 0.8,
            max_concurrent_streams: 1000,
            health_check_interval: Duration::from_secs(30),
            metrics_interval: Duration::from_secs(10),
            adaptive_routing: true,
            load_balancing: true,
        }
    }
}

impl QoSStreamRouter {
    /// Create new QoS stream router
    pub fn new(config: QoSRouterConfig) -> Self {
        let priority_queues = Arc::new(DashMap::new());
        
        // Initialize priority queues for each QoS class
        for qos_class in [QoSClass::Critical, QoSClass::System, QoSClass::JobExecution, 
                         QoSClass::DataTransfer, QoSClass::LogsMetrics] {
            priority_queues.insert(qos_class, Arc::new(RwLock::new(VecDeque::new())));
        }

        Self {
            classifier: Arc::new(super::classifier::MessageClassifier::new()),
            flow_control: Arc::new(FlowControlManager::new()),
            bandwidth_allocator: Arc::new(BandwidthAllocator::new()),
            priority_queues,
            route_table: Arc::new(RwLock::new(HashMap::new())),
            active_streams: Arc::new(DashMap::new()),
            congestion_semaphore: Arc::new(Semaphore::new(config.max_concurrent_streams)),
            metrics: Arc::new(RwLock::new(RouterMetrics::default())),
            config,
        }
    }

    /// Route message with QoS guarantees
    pub async fn route_message(
        &self,
        message: AdapterMessage,
        qos: QoSParams,
        destination: RouteDestination,
    ) -> Result<RoutingResult> {
        let start_time = Instant::now();
        
        // Classify message for QoS
        let qos_class = self.classifier.classify(&message, &qos).await?;
        
        // Create queued message
        let queued_message = QueuedMessage {
            message: message.clone(),
            qos: qos.clone(),
            qos_class,
            destination: destination.clone(),
            enqueued_at: Instant::now(),
            retry_count: 0,
            deadline: Some(Instant::now() + qos.max_latency),
        };

        // Check congestion and apply flow control
        if self.is_congested().await? {
            self.apply_congestion_control(&queued_message).await?;
        }

        // Enqueue message based on priority
        self.enqueue_message(queued_message).await?;

        // Update metrics
        self.update_routing_metrics(qos_class, start_time.elapsed()).await;

        Ok(RoutingResult {
            success: true,
            qos_class,
            routing_latency: start_time.elapsed(),
            queue_position: self.get_queue_position(qos_class).await?,
        })
    }

    /// Process priority queues
    pub async fn process_queues(&self) -> Result<()> {
        // Process queues in priority order
        for qos_class in [QoSClass::Critical, QoSClass::System, QoSClass::JobExecution, 
                         QoSClass::DataTransfer, QoSClass::LogsMetrics] {
            self.process_queue(qos_class).await?;
        }
        Ok(())
    }

    /// Process specific queue
    async fn process_queue(&self, qos_class: QoSClass) -> Result<()> {
        if let Some(queue_ref) = self.priority_queues.get(&qos_class) {
            let mut queue = queue_ref.write().await;
            
            while let Some(queued_message) = queue.pop_front() {
                // Check deadline
                if let Some(deadline) = queued_message.deadline {
                    if Instant::now() > deadline {
                        warn!("Message deadline exceeded for QoS class {:?}", qos_class);
                        continue;
                    }
                }

                // Route the message
                match self.route_to_destination(&queued_message).await {
                    Ok(_) => {
                        debug!("Successfully routed message for QoS class {:?}", qos_class);
                    }
                    Err(e) => {
                        error!("Failed to route message: {}", e);
                        // Implement retry logic if needed
                        if queued_message.retry_count < 3 {
                            let mut retry_message = queued_message.clone();
                            retry_message.retry_count += 1;
                            queue.push_back(retry_message);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Route message to specific destination
    async fn route_to_destination(&self, queued_message: &QueuedMessage) -> Result<()> {
        match &queued_message.destination {
            RouteDestination::Direct(adapter_id) => {
                self.send_to_adapter(*adapter_id, &queued_message.message, &queued_message.qos).await
            }
            RouteDestination::LoadBalanced(adapter_ids) => {
                let selected_adapter = self.select_best_adapter(adapter_ids).await?;
                self.send_to_adapter(selected_adapter, &queued_message.message, &queued_message.qos).await
            }
            RouteDestination::Service(service_name) => {
                let adapter_id = self.resolve_service(service_name).await?;
                self.send_to_adapter(adapter_id, &queued_message.message, &queued_message.qos).await
            }
            RouteDestination::Broadcast => {
                self.broadcast_message(&queued_message.message, &queued_message.qos).await
            }
        }
    }

    /// Send message to specific adapter
    async fn send_to_adapter(
        &self,
        adapter_id: AdapterId,
        message: &AdapterMessage,
        qos: &QoSParams,
    ) -> Result<()> {
        // Implementation would integrate with adapter factory
        // For now, return success
        info!("Routing message to adapter {}", adapter_id);
        Ok(())
    }

    /// Select best adapter from list based on performance metrics
    async fn select_best_adapter(&self, adapter_ids: &[AdapterId]) -> Result<AdapterId> {
        let route_table = self.route_table.read().await;
        
        let mut best_adapter = adapter_ids[0];
        let mut best_score = 0.0;

        for &adapter_id in adapter_ids {
            if let Some(route_entry) = route_table.get(&adapter_id.to_string()) {
                let score = self.calculate_adapter_score(route_entry);
                if score > best_score {
                    best_score = score;
                    best_adapter = adapter_id;
                }
            }
        }

        Ok(best_adapter)
    }

    /// Calculate adapter performance score
    fn calculate_adapter_score(&self, route_entry: &RouteEntry) -> f64 {
        let health_score = match route_entry.health {
            RouteHealth::Healthy => 1.0,
            RouteHealth::Degraded { .. } => 0.5,
            RouteHealth::Unhealthy { .. } => 0.0,
        };

        let latency_score = 1.0 / (route_entry.metrics.avg_latency.as_millis() as f64 + 1.0);
        let success_score = route_entry.metrics.success_rate;
        let utilization_score = 1.0 - route_entry.metrics.bandwidth_utilization;

        // Weighted score calculation
        (health_score * 0.4) + (latency_score * 0.3) + (success_score * 0.2) + (utilization_score * 0.1)
    }

    /// Resolve service name to adapter ID
    async fn resolve_service(&self, service_name: &str) -> Result<AdapterId> {
        // Implementation would integrate with service registry
        // For now, return a dummy adapter ID
        Ok(uuid::Uuid::new_v4())
    }

    /// Broadcast message to all adapters
    async fn broadcast_message(&self, message: &AdapterMessage, qos: &QoSParams) -> Result<()> {
        // Implementation would broadcast to all registered adapters
        info!("Broadcasting message to all adapters");
        Ok(())
    }

    /// Check if router is congested
    async fn is_congested(&self) -> Result<bool> {
        let total_queue_size: usize = self.priority_queues
            .iter()
            .map(|entry| {
                // This is a simplified check - in practice we'd use try_read
                1 // Placeholder
            })
            .sum();

        let congestion_ratio = total_queue_size as f64 / 
            (self.config.max_queue_size * self.priority_queues.len()) as f64;

        Ok(congestion_ratio > self.config.congestion_threshold)
    }

    /// Apply congestion control
    async fn apply_congestion_control(&self, message: &QueuedMessage) -> Result<()> {
        // Acquire congestion control permit
        let _permit = self.congestion_semaphore.acquire().await
            .map_err(|e| ValkyrieError::CongestionControl(format!("Failed to acquire permit: {}", e)))?;

        // Apply flow control based on QoS class
        self.flow_control.apply_flow_control(message.qos_class, &message.qos).await?;

        Ok(())
    }

    /// Enqueue message in appropriate priority queue
    async fn enqueue_message(&self, message: QueuedMessage) -> Result<()> {
        if let Some(queue_ref) = self.priority_queues.get(&message.qos_class) {
            let mut queue = queue_ref.write().await;
            
            if queue.len() >= self.config.max_queue_size {
                return Err(ValkyrieError::QueueFull(format!(
                    "Queue full for QoS class {:?}", message.qos_class
                )));
            }
            
            queue.push_back(message);
            Ok(())
        } else {
            Err(ValkyrieError::InvalidQoSClass(format!(
                "Unknown QoS class: {:?}", message.qos_class
            )))
        }
    }

    /// Get queue position for QoS class
    async fn get_queue_position(&self, qos_class: QoSClass) -> Result<usize> {
        if let Some(queue_ref) = self.priority_queues.get(&qos_class) {
            let queue = queue_ref.read().await;
            Ok(queue.len())
        } else {
            Ok(0)
        }
    }

    /// Update routing metrics
    async fn update_routing_metrics(&self, qos_class: QoSClass, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_messages += 1;
        *metrics.messages_by_class.entry(qos_class).or_insert(0) += 1;
        
        // Update average latency (simplified)
        metrics.avg_routing_latency = Duration::from_nanos(
            (metrics.avg_routing_latency.as_nanos() + latency.as_nanos()) / 2
        );
    }

    /// Get router metrics
    pub async fn metrics(&self) -> RouterMetrics {
        self.metrics.read().await.clone()
    }
}

/// Routing result
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// Success status
    pub success: bool,
    /// Determined QoS class
    pub qos_class: QoSClass,
    /// Routing latency
    pub routing_latency: Duration,
    /// Queue position
    pub queue_position: usize,
}

impl MessageClassifier {
    /// Create new message classifier
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            default_class: QoSClass::DataTransfer,
            classification_metrics: Arc::new(RwLock::new(ClassificationMetrics::default())),
        }
    }

    /// Classify message for QoS
    pub async fn classify(&self, message: &AdapterMessage, qos: &QoSParams) -> Result<QoSClass> {
        let start_time = Instant::now();
        
        // Apply classification rules in priority order
        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            
            if self.matches_condition(&rule.condition, message, qos) {
                self.update_classification_metrics(rule.target_class, start_time.elapsed()).await;
                return Ok(rule.target_class);
            }
        }

        // Use default class if no rules match
        self.update_classification_metrics(self.default_class, start_time.elapsed()).await;
        Ok(self.default_class)
    }

    /// Check if message matches classification condition
    fn matches_condition(
        &self,
        condition: &ClassificationCondition,
        message: &AdapterMessage,
        qos: &QoSParams,
    ) -> bool {
        match condition {
            ClassificationCondition::MessageType(msg_type) => {
                message.message_type == *msg_type
            }
            ClassificationCondition::Priority(priority) => {
                message.priority == *priority
            }
            ClassificationCondition::Metadata { key, value } => {
                message.metadata.get(key).map_or(false, |v| v == value)
            }
            ClassificationCondition::PayloadSize { min, max } => {
                let size = message.payload.len();
                min.map_or(true, |m| size >= m) && max.map_or(true, |m| size <= m)
            }
            ClassificationCondition::SourceAdapter(adapter_id) => {
                // Would need source adapter info in message
                false // Placeholder
            }
            ClassificationCondition::Custom(_function_name) => {
                // Would execute custom classification function
                false // Placeholder
            }
        }
    }

    /// Default classification rules
    fn default_rules() -> Vec<ClassificationRule> {
        vec![
            ClassificationRule {
                name: "Critical Health Checks".to_string(),
                condition: ClassificationCondition::MessageType(AdapterMessageType::HealthCheck),
                target_class: QoSClass::Critical,
                priority: 0,
                enabled: true,
            },
            ClassificationRule {
                name: "High Priority Messages".to_string(),
                condition: ClassificationCondition::Priority(MessagePriority::Critical),
                target_class: QoSClass::System,
                priority: 1,
                enabled: true,
            },
            ClassificationRule {
                name: "Job Execution".to_string(),
                condition: ClassificationCondition::MessageType(AdapterMessageType::Request),
                target_class: QoSClass::JobExecution,
                priority: 2,
                enabled: true,
            },
            ClassificationRule {
                name: "Large Data Transfers".to_string(),
                condition: ClassificationCondition::PayloadSize { 
                    min: Some(1024 * 1024), // 1MB
                    max: None 
                },
                target_class: QoSClass::DataTransfer,
                priority: 3,
                enabled: true,
            },
        ]
    }

    /// Update classification metrics
    async fn update_classification_metrics(&self, qos_class: QoSClass, latency: Duration) {
        let mut metrics = self.classification_metrics.write().await;
        metrics.total_classifications += 1;
        *metrics.by_class.entry(qos_class).or_insert(0) += 1;
        
        // Update average classification time
        metrics.avg_classification_time = Duration::from_nanos(
            (metrics.avg_classification_time.as_nanos() + latency.as_nanos()) / 2
        );
    }
}

impl Default for RouterMetrics {
    fn default() -> Self {
        Self {
            total_messages: 0,
            messages_by_class: HashMap::new(),
            avg_routing_latency: Duration::from_nanos(0),
            queue_depths: HashMap::new(),
            congestion_events: 0,
            bandwidth_utilization: 0.0,
        }
    }
}

impl Default for ClassificationMetrics {
    fn default() -> Self {
        Self {
            total_classifications: 0,
            by_class: HashMap::new(),
            avg_classification_time: Duration::from_nanos(0),
            misclassification_rate: 0.0,
        }
    }
}

impl Default for StreamMetrics {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            bytes_transferred: 0,
            avg_latency: Duration::from_nanos(0),
            throughput: 0.0,
            error_count: 0,
        }
    }
}

impl std::fmt::Display for QoSClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QoSClass::Critical => write!(f, "Critical"),
            QoSClass::System => write!(f, "System"),
            QoSClass::JobExecution => write!(f, "JobExecution"),
            QoSClass::DataTransfer => write!(f, "DataTransfer"),
            QoSClass::LogsMetrics => write!(f, "LogsMetrics"),
        }
    }
}

impl std::fmt::Display for RouteHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteHealth::Healthy => write!(f, "Healthy"),
            RouteHealth::Degraded { reason } => write!(f, "Degraded: {}", reason),
            RouteHealth::Unhealthy { reason } => write!(f, "Unhealthy: {}", reason),
        }
    }
}