use chrono::Utc;
use serde_json;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::core::networking::node_communication::{
    CompressionAlgorithm, CompressionInfo, EnhancedProtocolMessage, FlowControlFlags,
    FlowControlInfo, MessageCompressor, NodeMessage, PerformanceHints, PriorityMessageQueue,
    RetryCondition, RetryPolicy,
};
use crate::core::networking::valkyrie::message::MessageValidator;
use crate::core::networking::valkyrie::types::{
    CompressionPreference, DestinationType, MessagePriority, MessageType, ReliabilityLevel,
    ValkyrieMessage,
};
use crate::error::AppError;

/// Enhanced message processor that integrates Valkyrie messages with QoS and compression
pub struct EnhancedMessageProcessor {
    /// Message validator
    validator: MessageValidator,
    /// Priority queue for message processing
    message_queue: PriorityMessageQueue,
    /// Compression settings
    compression_enabled: bool,
    /// Default compression algorithm
    default_compression: CompressionAlgorithm,
    /// QoS policies
    qos_policies: HashMap<String, QoSPolicyConfig>,
    /// Performance metrics
    metrics: ProcessorMetrics,
}

/// QoS policy configuration
#[derive(Debug, Clone)]
pub struct QoSPolicyConfig {
    pub priority_mapping: HashMap<MessageType, MessagePriority>,
    pub compression_rules: HashMap<MessageType, CompressionPreference>,
    pub reliability_requirements: HashMap<MessageType, ReliabilityLevel>,
    pub retry_policies: HashMap<MessageType, RetryPolicy>,
    pub flow_control_enabled: bool,
}

/// Processor performance metrics
#[derive(Debug, Clone, Default)]
pub struct ProcessorMetrics {
    pub messages_processed: u64,
    pub messages_compressed: u64,
    pub messages_validated: u64,
    pub validation_failures: u64,
    pub compression_ratio_avg: f64,
    pub processing_time_avg_ms: f64,
    pub queue_depth_max: u32,
    pub throughput_messages_per_second: f64,
}

impl EnhancedMessageProcessor {
    /// Create a new enhanced message processor
    pub fn new() -> Self {
        Self {
            validator: MessageValidator::new(),
            message_queue: PriorityMessageQueue::new(),
            compression_enabled: true,
            default_compression: CompressionAlgorithm::Zstd,
            qos_policies: Self::default_qos_policies(),
            metrics: ProcessorMetrics::default(),
        }
    }

    /// Create default QoS policies
    fn default_qos_policies() -> HashMap<String, QoSPolicyConfig> {
        let mut policies = HashMap::new();

        // High-performance policy for critical messages
        let mut priority_mapping = HashMap::new();
        priority_mapping.insert(MessageType::Error, MessagePriority::Critical(10));
        priority_mapping.insert(
            MessageType::AlertNotification,
            MessagePriority::Critical(20),
        );
        priority_mapping.insert(MessageType::JobCancel, MessagePriority::High);
        priority_mapping.insert(MessageType::JobStart, MessagePriority::High);
        priority_mapping.insert(MessageType::Ping, MessagePriority::Normal);
        priority_mapping.insert(MessageType::MetricsReport, MessagePriority::Low);

        let mut compression_rules = HashMap::new();
        compression_rules.insert(MessageType::MetricsReport, CompressionPreference::HighRatio);
        compression_rules.insert(MessageType::LogEntry, CompressionPreference::Balanced);
        compression_rules.insert(MessageType::Ping, CompressionPreference::None);
        compression_rules.insert(MessageType::Error, CompressionPreference::Fast);

        let mut reliability_requirements = HashMap::new();
        reliability_requirements.insert(MessageType::JobStart, ReliabilityLevel::ExactlyOnce);
        reliability_requirements.insert(MessageType::JobComplete, ReliabilityLevel::AtLeastOnce);
        reliability_requirements.insert(MessageType::Error, ReliabilityLevel::AtLeastOnce);
        reliability_requirements.insert(MessageType::Ping, ReliabilityLevel::BestEffort);

        let mut retry_policies = HashMap::new();
        retry_policies.insert(
            MessageType::JobStart,
            RetryPolicy {
                max_attempts: 3,
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(5),
                backoff_multiplier: 2.0,
                jitter: true,
                retry_conditions: vec![
                    RetryCondition::NetworkError,
                    RetryCondition::Timeout,
                    RetryCondition::ServerError,
                ],
            },
        );

        policies.insert(
            "default".to_string(),
            QoSPolicyConfig {
                priority_mapping,
                compression_rules,
                reliability_requirements,
                retry_policies,
                flow_control_enabled: true,
            },
        );

        policies
    }

    /// Process a Valkyrie message with QoS and compression
    pub async fn process_message(
        &mut self,
        message: ValkyrieMessage,
        policy_name: Option<&str>,
    ) -> Result<EnhancedProtocolMessage, AppError> {
        let start_time = std::time::Instant::now();

        // Validate the message
        self.validator
            .validate(&message)
            .map_err(|e| AppError::ValidationError(e.to_string()))?;
        self.metrics.messages_validated += 1;

        // Get QoS policy
        let policy = self
            .qos_policies
            .get(policy_name.unwrap_or("default"))
            .cloned()
            .unwrap_or_else(|| self.qos_policies["default"].clone());

        // Determine message priority based on policy
        let priority = policy
            .priority_mapping
            .get(&message.header.message_type)
            .copied()
            .unwrap_or(message.header.priority);

        // Determine compression based on policy and message size
        let compression_preference = policy
            .compression_rules
            .get(&message.header.message_type)
            .copied()
            .unwrap_or(CompressionPreference::Auto);

        let compression_algorithm =
            self.select_compression_algorithm(compression_preference, message.estimated_size());

        // Serialize and potentially compress the message
        let serialized = serde_json::to_vec(&message)
            .map_err(|e| AppError::SerializationError(e.to_string()))?;

        let (compressed_data, compression_info) =
            if compression_algorithm != CompressionAlgorithm::None {
                let (data, info) = MessageCompressor::compress(&serialized, compression_algorithm)
                    .map_err(|e| AppError::CompressionError(e.to_string()))?;
                self.metrics.messages_compressed += 1;
                (data, info)
            } else {
                let original_size = serialized.len() as u64;
                (
                    serialized,
                    CompressionInfo {
                        algorithm: CompressionAlgorithm::None,
                        original_size,
                        compressed_size: original_size,
                        compression_ratio: 1.0,
                    },
                )
            };

        // Get reliability requirements
        let reliability = policy
            .reliability_requirements
            .get(&message.header.message_type)
            .copied()
            .unwrap_or(ReliabilityLevel::BestEffort);

        // Get retry policy
        let retry_policy = policy
            .retry_policies
            .get(&message.header.message_type)
            .cloned();

        // Create flow control information if enabled
        let flow_control = if policy.flow_control_enabled {
            Some(FlowControlInfo {
                window_size: 65536, // 64KB default window
                sequence_number: 0, // TODO: Implement proper sequence numbering
                acknowledgment_number: None,
                flow_control_flags: FlowControlFlags {
                    window_update: false,
                    end_stream: false,
                    priority_update: priority != MessagePriority::Normal,
                    reset_stream: false,
                },
            })
        } else {
            None
        };

        // Create performance hints
        let performance_hints = PerformanceHints {
            latency_sensitive: matches!(
                priority,
                MessagePriority::Critical(_) | MessagePriority::High
            ),
            bandwidth_intensive: compressed_data.len() > 1024 * 1024, // > 1MB
            cpu_intensive: compression_info.algorithm != CompressionAlgorithm::None,
            memory_intensive: false,
            cacheable: matches!(
                message.header.message_type,
                MessageType::MetricsReport | MessageType::StatusUpdate
            ),
            preferred_transport: None,
            batch_eligible: matches!(
                message.header.message_type,
                MessageType::LogEntry | MessageType::MetricsReport
            ),
        };

        // Create the enhanced protocol message
        let enhanced_message = EnhancedProtocolMessage {
            base: crate::core::networking::node_communication::ProtocolMessage {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                source: message.header.routing.source.parse().unwrap_or_default(),
                destination: match &message.header.routing.destination {
                    DestinationType::Unicast(endpoint) => {
                        Some(endpoint.parse().unwrap_or_default())
                    }
                    _ => None,
                },
                message: crate::core::networking::node_communication::MessagePayload::NodeMessage(
                    NodeMessage::MetricsUpdate {
                        node_id: message.header.routing.source.parse().unwrap_or_default(),
                        metrics: crate::core::networking::node_communication::RealtimeMetrics {
                            cpu_usage_percent: 0.0,
                            memory_usage_mb: 0,
                            disk_io_read_bps: 0,
                            disk_io_write_bps: 0,
                            network_rx_bps: 0,
                            network_tx_bps: 0,
                            active_connections: 0,
                            queue_depth: 0,
                            response_time_ms: 0.0,
                            error_rate: 0.0,
                            custom_metrics: HashMap::new(),
                        },
                        timestamp: Utc::now(),
                    },
                ),
                signature: None,
            },
            priority: crate::core::networking::node_communication::MessagePriority::from(priority),
            compression: compression_info,
            reliability,
            retry_policy,
            flow_control,
            performance_hints,
            integrity: None, // Add integrity metadata if needed
        };

        // Update metrics
        self.metrics.messages_processed += 1;
        let processing_time = start_time.elapsed().as_millis() as f64;
        self.metrics.processing_time_avg_ms = (self.metrics.processing_time_avg_ms
            * (self.metrics.messages_processed - 1) as f64
            + processing_time)
            / self.metrics.messages_processed as f64;

        Ok(enhanced_message)
    }

    /// Select appropriate compression algorithm based on preference and message size
    fn select_compression_algorithm(
        &self,
        preference: CompressionPreference,
        message_size: usize,
    ) -> CompressionAlgorithm {
        if !self.compression_enabled {
            return CompressionAlgorithm::None;
        }

        match preference {
            CompressionPreference::None => CompressionAlgorithm::None,
            CompressionPreference::Fast => CompressionAlgorithm::Lz4,
            CompressionPreference::Balanced => CompressionAlgorithm::Zstd,
            CompressionPreference::HighRatio => CompressionAlgorithm::Brotli,
            CompressionPreference::Auto => {
                if message_size < 1024 {
                    // Small messages - no compression overhead
                    CompressionAlgorithm::None
                } else if message_size < 64 * 1024 {
                    // Medium messages - fast compression
                    CompressionAlgorithm::Lz4
                } else {
                    // Large messages - balanced compression
                    CompressionAlgorithm::Zstd
                }
            }
        }
    }

    /// Enqueue a message for processing
    pub fn enqueue_message(&mut self, message: EnhancedProtocolMessage) {
        self.message_queue.enqueue(message);
        let queue_len = self.message_queue.len() as u32;
        if queue_len > self.metrics.queue_depth_max {
            self.metrics.queue_depth_max = queue_len;
        }
    }

    /// Dequeue the next message for processing
    pub fn dequeue_message(&mut self) -> Option<EnhancedProtocolMessage> {
        self.message_queue.dequeue()
    }

    /// Get processor metrics
    pub fn metrics(&self) -> &ProcessorMetrics {
        &self.metrics
    }

    /// Update QoS policy
    pub fn update_qos_policy(&mut self, name: String, policy: QoSPolicyConfig) {
        self.qos_policies.insert(name, policy);
    }

    /// Enable or disable compression
    pub fn set_compression_enabled(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }

    /// Set default compression algorithm
    pub fn set_default_compression(&mut self, algorithm: CompressionAlgorithm) {
        self.default_compression = algorithm;
    }
}

impl Default for EnhancedMessageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Message batch processor for high-throughput scenarios
pub struct MessageBatchProcessor {
    /// Individual message processor
    processor: EnhancedMessageProcessor,
    /// Batch size configuration
    batch_size: usize,
    /// Batch timeout
    batch_timeout: Duration,
    /// Current batch
    current_batch: Vec<ValkyrieMessage>,
    /// Batch start time
    batch_start_time: Option<std::time::Instant>,
}

impl MessageBatchProcessor {
    /// Create a new batch processor
    pub fn new(batch_size: usize, batch_timeout: Duration) -> Self {
        Self {
            processor: EnhancedMessageProcessor::new(),
            batch_size,
            batch_timeout,
            current_batch: Vec::new(),
            batch_start_time: None,
        }
    }

    /// Add message to current batch
    pub fn add_message(&mut self, message: ValkyrieMessage) -> Option<Vec<ValkyrieMessage>> {
        if self.current_batch.is_empty() {
            self.batch_start_time = Some(std::time::Instant::now());
        }

        self.current_batch.push(message);

        // Check if batch is ready for processing
        if self.should_process_batch() {
            self.flush_batch()
        } else {
            None
        }
    }

    /// Check if batch should be processed
    fn should_process_batch(&self) -> bool {
        // Process if batch is full
        if self.current_batch.len() >= self.batch_size {
            return true;
        }

        // Process if batch timeout exceeded
        if let Some(start_time) = self.batch_start_time {
            if start_time.elapsed() >= self.batch_timeout {
                return true;
            }
        }

        false
    }

    /// Flush current batch and return messages
    pub fn flush_batch(&mut self) -> Option<Vec<ValkyrieMessage>> {
        if self.current_batch.is_empty() {
            return None;
        }

        let batch = std::mem::take(&mut self.current_batch);
        self.batch_start_time = None;
        Some(batch)
    }

    /// Process a batch of messages
    pub async fn process_batch(
        &mut self,
        messages: Vec<ValkyrieMessage>,
        policy_name: Option<&str>,
    ) -> Result<Vec<EnhancedProtocolMessage>, AppError> {
        let mut processed_messages = Vec::with_capacity(messages.len());

        for message in messages {
            let processed = self.processor.process_message(message, policy_name).await?;
            processed_messages.push(processed);
        }

        Ok(processed_messages)
    }

    /// Get processor metrics
    pub fn metrics(&self) -> &ProcessorMetrics {
        self.processor.metrics()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::networking::valkyrie::message::MessagePayload;
    use crate::core::networking::valkyrie::StructuredPayload;

    #[tokio::test]
    async fn test_message_processing_with_compression() {
        let mut processor = EnhancedMessageProcessor::new();

        // Create a test message
        let message = ValkyrieMessage::new(
            MessageType::MetricsReport,
            "test-node".to_string(),
            DestinationType::Unicast("control-plane".to_string()),
            MessagePayload::Structured(StructuredPayload {
                payload_type: "metrics".to_string(),
                version: "1.0".to_string(),
                data: serde_json::json!({
                    "cpu_usage": 75.5,
                    "memory_usage": 1024,
                    "disk_usage": 50.0
                }),
                schema: None,
            }),
        )
        .with_priority(MessagePriority::Background);

        let result = processor.process_message(message, None).await;
        assert!(result.is_ok());

        let enhanced_message = result.unwrap();
        assert_eq!(enhanced_message.priority, MessagePriority::Background);
        assert!(enhanced_message.compression.algorithm != CompressionAlgorithm::None);
    }

    #[tokio::test]
    async fn test_priority_queue_ordering() {
        let mut queue = PriorityMessageQueue::new();

        // Create messages with different priorities
        let low_priority_msg = create_test_enhanced_message(MessagePriority::Background);
        let high_priority_msg = create_test_enhanced_message(MessagePriority::Critical(10));
        let normal_priority_msg = create_test_enhanced_message(MessagePriority::Normal);

        // Enqueue in random order
        queue.enqueue(low_priority_msg);
        queue.enqueue(normal_priority_msg);
        queue.enqueue(high_priority_msg);

        // Dequeue should return highest priority first
        let first = queue.dequeue().unwrap();
        assert_eq!(first.priority, MessagePriority::Critical(10));

        let second = queue.dequeue().unwrap();
        assert_eq!(second.priority, MessagePriority::Normal);

        let third = queue.dequeue().unwrap();
        assert_eq!(third.priority, MessagePriority::Background);
    }

    fn create_test_enhanced_message(priority: MessagePriority) -> EnhancedProtocolMessage {
        EnhancedProtocolMessage {
            base: crate::core::networking::node_communication::ProtocolMessage {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                source: Uuid::new_v4(),
                destination: None,
                message: crate::core::networking::node_communication::MessagePayload::NodeMessage(
                    NodeMessage::Heartbeat {
                        node_id: Uuid::new_v4(),
                        status: crate::core::networking::node_communication::NodeStatus::Ready,
                        resources: crate::core::networking::node_communication::NodeResources {
                            cpu_cores: 4,
                            memory_mb: 8192,
                            disk_gb: 100,
                            network_mbps: 1000,
                            available_cpu: 2.0,
                            available_memory_mb: 4096,
                            available_disk_gb: 50,
                        },
                        metrics: crate::core::networking::node_communication::NodeMetrics {
                            cpu_usage_percent: 50.0,
                            memory_usage_percent: 60.0,
                            disk_usage_percent: 40.0,
                            network_rx_mbps: 10.0,
                            network_tx_mbps: 5.0,
                            load_average: 1.5,
                            active_jobs: 2,
                            completed_jobs: 100,
                            failed_jobs: 5,
                            uptime_seconds: 86400,
                        },
                        timestamp: Utc::now(),
                    },
                ),
                signature: None,
            },
            priority,
            compression: CompressionInfo {
                algorithm: CompressionAlgorithm::None,
                original_size: 1024,
                compressed_size: 1024,
                compression_ratio: 1.0,
            },
            reliability: ReliabilityLevel::BestEffort,
            retry_policy: None,
            flow_control: None,
            performance_hints: PerformanceHints {
                latency_sensitive: false,
                bandwidth_intensive: false,
                cpu_intensive: false,
                memory_intensive: false,
                cacheable: false,
                preferred_transport: None,
                batch_eligible: false,
            },
            integrity: None,
        }
    }
}
