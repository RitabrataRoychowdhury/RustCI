use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
// tokio::sync imports removed as unused
use crate::core::networking::valkyrie::types::ValkyrieMessage;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod bandwidth;
pub mod classifier;
pub mod flow_control;
pub mod multiplexer;
pub mod priority;
pub mod router;

pub use flow_control::{FlowControlConfig, FlowControlManager};
pub use multiplexer::*;
pub use priority::*;

// Re-export QoS-aware streaming components
pub use bandwidth::{BandwidthAllocation, BandwidthAllocator};
pub use classifier::{ClassificationResult, MessageClassifier};
pub use router::{QoSStreamRouter, QueuedMessage, RoutingResult};

// Congestion control components are defined in this module and automatically available

/// Stream identifier type
pub type StreamId = Uuid;

/// QoS classes for message prioritization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QoSClass {
    Critical = 0,     // System-critical messages (heartbeats, health checks)
    System = 1,       // System management messages
    JobExecution = 2, // Job execution commands and responses
    DataTransfer = 3, // Large data transfers
    LogsMetrics = 4,  // Logs and metrics (lowest priority)
}

/// Stream priority levels for QoS
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StreamPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl Default for StreamPriority {
    fn default() -> Self {
        StreamPriority::Normal
    }
}

/// Stream types for different use cases
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamType {
    /// Control messages (highest priority)
    Control,
    /// Job execution data
    JobExecution,
    /// File transfer streams
    FileTransfer,
    /// Metrics and monitoring data
    Metrics,
    /// Log streaming
    Logs,
    /// General data transfer
    Data,
    /// Custom stream type
    Custom(String),
}

/// Stream state tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamState {
    /// Stream is being initialized
    Initializing,
    /// Stream is active and ready for data
    Active,
    /// Stream is paused (backpressure applied)
    Paused,
    /// Stream is being closed gracefully
    Closing,
    /// Stream is closed
    Closed,
    /// Stream encountered an error
    Error(String),
}

/// Stream configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Maximum buffer size for this stream
    pub max_buffer_size: usize,
    /// Flow control window size
    pub flow_window_size: u32,
    /// Stream timeout duration
    pub timeout: Duration,
    /// Enable compression for this stream
    pub compression_enabled: bool,
    /// Stream priority
    pub priority: StreamPriority,
    /// Custom stream parameters
    pub custom_params: HashMap<String, String>,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 64 * 1024, // 64KB default buffer
            flow_window_size: 65536,    // 64KB flow window
            timeout: Duration::from_secs(30),
            compression_enabled: false,
            priority: StreamPriority::Normal,
            custom_params: HashMap::new(),
        }
    }
}

/// Stream metrics for monitoring and optimization
#[derive(Debug, Clone, Default)]
pub struct StreamMetrics {
    /// Total bytes sent on this stream
    pub bytes_sent: u64,
    /// Total bytes received on this stream
    pub bytes_received: u64,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Stream creation timestamp
    pub created_at: Option<Instant>,
    /// Last activity timestamp
    pub last_activity: Option<Instant>,
    /// Number of flow control events
    pub flow_control_events: u64,
    /// Number of backpressure events
    pub backpressure_events: u64,
    /// Average message latency
    pub avg_latency: Duration,
    /// Current buffer utilization (0.0 to 1.0)
    pub buffer_utilization: f64,
}

/// Flow control window for managing data flow
#[derive(Debug, Clone)]
pub struct FlowWindow {
    /// Current window size
    pub size: u32,
    /// Available window space
    pub available: u32,
    /// Window update threshold
    pub update_threshold: u32,
    /// Last window update time
    pub last_update: Instant,
}

impl FlowWindow {
    pub fn new(size: u32) -> Self {
        Self {
            size,
            available: size,
            update_threshold: size / 2, // Update when 50% consumed
            last_update: Instant::now(),
        }
    }

    /// Consume window space
    pub fn consume(&mut self, amount: u32) -> bool {
        if self.available >= amount {
            self.available -= amount;
            true
        } else {
            false
        }
    }

    /// Release window space
    pub fn release(&mut self, amount: u32) {
        self.available = (self.available + amount).min(self.size);
        self.last_update = Instant::now();
    }

    /// Check if window update is needed
    pub fn needs_update(&self) -> bool {
        self.available <= self.update_threshold
    }
}

/// Congestion control state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CongestionState {
    /// Current congestion window size
    pub cwnd: u32,
    /// Slow start threshold
    pub ssthresh: u32,
    /// Round trip time estimate
    pub rtt: Duration,
    /// RTT variance
    pub rtt_var: Duration,
    /// Congestion control algorithm state
    pub algorithm_state: CongestionAlgorithm,
}

/// Congestion control algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CongestionAlgorithm {
    /// TCP Reno-like algorithm
    Reno {
        duplicate_acks: u32,
        fast_recovery: bool,
    },
    /// TCP Cubic algorithm
    Cubic {
        w_max: f64,
        k: f64,
        #[serde(skip)]
        epoch_start: Option<Instant>,
    },
    /// BBR (Bottleneck Bandwidth and RTT) algorithm
    Bbr {
        bandwidth: f64,
        #[serde(with = "humantime_serde")]
        min_rtt: Duration,
        pacing_rate: f64,
    },
}

/// Stream lifecycle events
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream was created
    Created {
        stream_id: StreamId,
        stream_type: StreamType,
        config: StreamConfig,
    },
    /// Stream state changed
    StateChanged {
        stream_id: StreamId,
        old_state: StreamState,
        new_state: StreamState,
    },
    /// Data was sent on stream
    DataSent { stream_id: StreamId, bytes: usize },
    /// Data was received on stream
    DataReceived { stream_id: StreamId, bytes: usize },
    /// Flow control event occurred
    FlowControl {
        stream_id: StreamId,
        event_type: FlowControlEvent,
    },
    /// Stream was closed
    Closed { stream_id: StreamId, reason: String },
    /// Stream error occurred
    Error { stream_id: StreamId, error: String },
}

/// Flow control event types
#[derive(Debug, Clone)]
pub enum FlowControlEvent {
    /// Window update sent
    WindowUpdate { new_size: u32 },
    /// Backpressure applied
    BackpressureApplied,
    /// Backpressure released
    BackpressureReleased,
    /// Congestion detected
    CongestionDetected,
    /// Congestion cleared
    CongestionCleared,
}

/// Stream event handler trait
pub trait StreamEventHandler: Send + Sync {
    fn handle_event(&self, event: StreamEvent);
}

/// Default stream event handler that logs events
pub struct DefaultStreamEventHandler;

impl StreamEventHandler for DefaultStreamEventHandler {
    fn handle_event(&self, event: StreamEvent) {
        match event {
            StreamEvent::Created {
                stream_id,
                stream_type,
                ..
            } => {
                tracing::info!("Stream created: {} (type: {:?})", stream_id, stream_type);
            }
            StreamEvent::StateChanged {
                stream_id,
                old_state,
                new_state,
            } => {
                tracing::debug!(
                    "Stream {} state: {:?} -> {:?}",
                    stream_id,
                    old_state,
                    new_state
                );
            }
            StreamEvent::DataSent { stream_id, bytes } => {
                tracing::trace!("Stream {} sent {} bytes", stream_id, bytes);
            }
            StreamEvent::DataReceived { stream_id, bytes } => {
                tracing::trace!("Stream {} received {} bytes", stream_id, bytes);
            }
            StreamEvent::FlowControl {
                stream_id,
                event_type,
            } => {
                tracing::debug!("Stream {} flow control: {:?}", stream_id, event_type);
            }
            StreamEvent::Closed { stream_id, reason } => {
                tracing::info!("Stream {} closed: {}", stream_id, reason);
            }
            StreamEvent::Error { stream_id, error } => {
                tracing::error!("Stream {} error: {}", stream_id, error);
            }
        }
    }
}

/// Congestion controller for managing network congestion
pub struct CongestionController {
    /// Congestion control algorithm
    algorithm: CongestionControlAlgorithm,
    /// Per-stream congestion state
    stream_states: Arc<RwLock<HashMap<StreamId, CongestionState>>>,
    /// Global congestion metrics
    global_metrics: Arc<Mutex<CongestionMetrics>>,
    /// Congestion detection threshold
    detection_threshold: f64,
    /// Recovery strategy
    recovery_strategy: RecoveryStrategy,
}

/// Congestion control algorithms
#[derive(Debug, Clone)]
pub enum CongestionControlAlgorithm {
    /// TCP Reno-like algorithm
    Reno,
    /// TCP Cubic algorithm
    Cubic,
    /// BBR (Bottleneck Bandwidth and RTT)
    Bbr,
    /// Custom algorithm
    Custom(String),
}

/// Global congestion metrics
#[derive(Debug, Clone, Default)]
pub struct CongestionMetrics {
    /// Total congestion events
    pub congestion_events: u64,
    /// Recovery events
    pub recovery_events: u64,
    /// Average congestion window size
    pub avg_cwnd: f64,
    /// Packet loss rate
    pub loss_rate: f64,
    /// Average RTT
    pub avg_rtt: Duration,
}

/// Recovery strategies for congestion events
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Exponential backoff
    ExponentialBackoff {
        initial_delay: Duration,
        max_delay: Duration,
        backoff_factor: f64,
    },
    /// Linear backoff
    LinearBackoff {
        step_size: Duration,
        max_delay: Duration,
    },
    /// Adaptive recovery based on network conditions
    Adaptive,
    /// Custom recovery logic
    Custom(String),
}

impl CongestionController {
    /// Create a new congestion controller
    pub fn new() -> Self {
        Self {
            algorithm: CongestionControlAlgorithm::Cubic,
            stream_states: Arc::new(RwLock::new(HashMap::new())),
            global_metrics: Arc::new(Mutex::new(CongestionMetrics::default())),
            detection_threshold: 1.5,
            recovery_strategy: RecoveryStrategy::Adaptive,
        }
    }

    /// Initialize congestion control for a stream
    pub fn initialize_stream(&self, stream_id: StreamId) -> Result<()> {
        let state = CongestionState {
            cwnd: 10,                           // Initial congestion window
            ssthresh: 65536,                    // Slow start threshold
            rtt: Duration::from_millis(100),    // Initial RTT estimate
            rtt_var: Duration::from_millis(50), // RTT variance
            algorithm_state: match self.algorithm {
                CongestionControlAlgorithm::Reno => CongestionAlgorithm::Reno {
                    duplicate_acks: 0,
                    fast_recovery: false,
                },
                CongestionControlAlgorithm::Cubic => CongestionAlgorithm::Cubic {
                    w_max: 0.0,
                    k: 0.0,
                    epoch_start: None,
                },
                CongestionControlAlgorithm::Bbr => CongestionAlgorithm::Bbr {
                    bandwidth: 1000000.0, // 1 Mbps initial estimate
                    min_rtt: Duration::from_millis(100),
                    pacing_rate: 1000000.0,
                },
                CongestionControlAlgorithm::Custom(_) => CongestionAlgorithm::Reno {
                    duplicate_acks: 0,
                    fast_recovery: false,
                },
            },
        };

        let mut states = self.stream_states.write().unwrap();
        states.insert(stream_id, state);

        tracing::debug!("Initialized congestion control for stream {}", stream_id);
        Ok(())
    }

    /// Handle congestion event for a stream
    pub fn handle_congestion(&self, stream_id: StreamId) -> Result<()> {
        let mut states = self.stream_states.write().unwrap();
        if let Some(state) = states.get_mut(&stream_id) {
            match &mut state.algorithm_state {
                CongestionAlgorithm::Reno { fast_recovery, .. } => {
                    if !*fast_recovery {
                        state.ssthresh = state.cwnd / 2;
                        state.cwnd = state.ssthresh;
                        *fast_recovery = true;
                    }
                }
                CongestionAlgorithm::Cubic {
                    w_max, epoch_start, ..
                } => {
                    *w_max = state.cwnd as f64;
                    state.cwnd = (state.cwnd as f64 * 0.7) as u32; // Beta = 0.7 for Cubic
                    *epoch_start = Some(Instant::now());
                }
                CongestionAlgorithm::Bbr { pacing_rate, .. } => {
                    *pacing_rate *= 0.8; // Reduce pacing rate
                }
            }

            // Update global metrics
            {
                let mut metrics = self.global_metrics.lock().unwrap();
                metrics.congestion_events += 1;
            }

            tracing::warn!("Handled congestion for stream {}", stream_id);
        }

        Ok(())
    }

    /// Update RTT measurement for a stream
    pub fn update_rtt(&self, stream_id: StreamId, rtt: Duration) -> Result<()> {
        let mut states = self.stream_states.write().unwrap();
        if let Some(state) = states.get_mut(&stream_id) {
            // Update RTT using exponential weighted moving average
            let alpha = 0.125; // Standard TCP alpha
            let new_rtt_ms = rtt.as_millis() as f64;
            let old_rtt_ms = state.rtt.as_millis() as f64;
            let updated_rtt_ms = (1.0 - alpha) * old_rtt_ms + alpha * new_rtt_ms;

            state.rtt = Duration::from_millis(updated_rtt_ms as u64);

            // Update RTT variance
            let rtt_var_ms = state.rtt_var.as_millis() as f64;
            let updated_var_ms =
                (1.0 - alpha) * rtt_var_ms + alpha * (new_rtt_ms - updated_rtt_ms).abs();
            state.rtt_var = Duration::from_millis(updated_var_ms as u64);

            // Update global metrics
            {
                let mut metrics = self.global_metrics.lock().unwrap();
                metrics.avg_rtt = state.rtt;
            }
        }

        Ok(())
    }

    /// Get congestion window size for a stream
    pub fn get_congestion_window(&self, stream_id: &StreamId) -> Option<u32> {
        let states = self.stream_states.read().unwrap();
        states.get(stream_id).map(|state| state.cwnd)
    }

    /// Get congestion metrics
    pub fn get_metrics(&self) -> CongestionMetrics {
        self.global_metrics.lock().unwrap().clone()
    }

    /// Remove congestion control for a stream
    pub fn remove_stream(&self, stream_id: &StreamId) {
        let mut states = self.stream_states.write().unwrap();
        states.remove(stream_id);
        tracing::debug!("Removed congestion control for stream {}", stream_id);
    }
}
