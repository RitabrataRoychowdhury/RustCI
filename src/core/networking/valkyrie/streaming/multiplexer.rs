use super::*;
use crate::core::networking::valkyrie::types::ValkyrieMessage;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::interval;
use uuid::Uuid;

/// Advanced stream multiplexer with QoS and flow control
pub struct StreamMultiplexer {
    /// Active streams registry
    streams: Arc<RwLock<HashMap<StreamId, Arc<Stream>>>>,
    /// Stream allocation strategy
    allocation_strategy: StreamAllocationStrategy,
    /// Flow control manager
    flow_control: Arc<FlowControlManager>,
    /// Priority scheduler
    priority_scheduler: Arc<PriorityScheduler>,
    /// Congestion controller
    congestion_controller: Arc<CongestionController>,
    /// Stream metrics collector
    metrics: Arc<StreamMetricsCollector>,
    /// Event handler for stream lifecycle events
    event_handler: Arc<dyn StreamEventHandler>,
    /// Global multiplexer configuration
    config: MultiplexerConfig,
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

/// Stream allocation strategies
#[derive(Debug, Clone)]
pub enum StreamAllocationStrategy {
    /// Round-robin allocation
    RoundRobin,
    /// Least-loaded allocation
    LeastLoaded,
    /// Priority-based allocation
    PriorityBased,
    /// Custom allocation logic
    Custom(String),
}

/// Multiplexer configuration
#[derive(Debug, Clone)]
pub struct MultiplexerConfig {
    /// Maximum number of concurrent streams
    pub max_streams: usize,
    /// Default stream configuration
    pub default_stream_config: StreamConfig,
    /// Flow control enabled
    pub flow_control_enabled: bool,
    /// Congestion control enabled
    pub congestion_control_enabled: bool,
    /// Stream cleanup interval
    pub cleanup_interval: Duration,
    /// Stream timeout
    pub stream_timeout: Duration,
    /// Enable stream metrics collection
    pub metrics_enabled: bool,
}

impl Default for MultiplexerConfig {
    fn default() -> Self {
        Self {
            max_streams: 1000,
            default_stream_config: StreamConfig::default(),
            flow_control_enabled: true,
            congestion_control_enabled: true,
            cleanup_interval: Duration::from_secs(30),
            stream_timeout: Duration::from_secs(300), // 5 minutes
            metrics_enabled: true,
        }
    }
}

/// Individual stream with advanced capabilities
pub struct Stream {
    /// Stream identifier
    pub id: StreamId,
    /// Stream type and purpose
    pub stream_type: StreamType,
    /// Current stream state
    pub state: Arc<RwLock<StreamState>>,
    /// Stream priority for scheduling
    pub priority: StreamPriority,
    /// Flow control window
    pub flow_window: Arc<Mutex<FlowWindow>>,
    /// Stream-specific metrics
    pub metrics: Arc<Mutex<StreamMetrics>>,
    /// Message queue for this stream
    pub message_queue: Arc<Mutex<VecDeque<ValkyrieMessage>>>,
    /// Stream configuration
    pub config: StreamConfig,
    /// Stream creation time
    pub created_at: Instant,
    /// Last activity time
    pub last_activity: Arc<Mutex<Instant>>,
    /// Stream sender channel
    pub sender: Arc<Mutex<Option<mpsc::UnboundedSender<ValkyrieMessage>>>>,
    /// Stream receiver channel
    pub receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<ValkyrieMessage>>>>,
    /// Backpressure semaphore
    pub backpressure_semaphore: Arc<Semaphore>,
}

impl Stream {
    pub fn new(
        id: StreamId,
        stream_type: StreamType,
        priority: StreamPriority,
        config: StreamConfig,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let flow_window = FlowWindow::new(config.flow_window_size);
        let now = Instant::now();
        
        let mut metrics = StreamMetrics::default();
        metrics.created_at = Some(now);
        metrics.last_activity = Some(now);

        Self {
            id,
            stream_type,
            state: Arc::new(RwLock::new(StreamState::Initializing)),
            priority,
            flow_window: Arc::new(Mutex::new(flow_window)),
            metrics: Arc::new(Mutex::new(metrics)),
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
            created_at: now,
            last_activity: Arc::new(Mutex::new(now)),
            sender: Arc::new(Mutex::new(Some(sender))),
            receiver: Arc::new(Mutex::new(Some(receiver))),
            backpressure_semaphore: Arc::new(Semaphore::new(1000)), // Allow 1000 pending messages
        }
    }

    /// Send a message on this stream
    pub async fn send_message(&self, message: ValkyrieMessage) -> Result<()> {
        // Check if stream is active
        {
            let state = self.state.read().unwrap();
            match *state {
                StreamState::Active => {},
                StreamState::Paused => {
                    // Wait for backpressure to be released
                    let _permit = self.backpressure_semaphore.acquire().await
                        .map_err(|e| AppError::InternalError {
                            component: "stream".to_string(),
                            message: format!("Failed to acquire backpressure permit: {}", e),
                        })?;
                },
                _ => {
                    return Err(AppError::StreamError {
                        stream_id: self.id,
                        error_type: format!("Stream not ready for sending: {:?}", *state),
                    });
                }
            }
        }

        // Check flow control
        let message_size = message.payload.len() as u32;
        {
            let mut flow_window = self.flow_window.lock().unwrap();
            if !flow_window.consume(message_size) {
                return Err(AppError::FlowControlViolation {
                    stream_id: self.id,
                    details: format!("Insufficient flow window: need {}, available {}", 
                                   message_size, flow_window.available),
                });
            }
        }

        // Send message
        {
            let sender_guard = self.sender.lock().unwrap();
            if let Some(sender) = sender_guard.as_ref() {
                sender.send(message).map_err(|e| AppError::StreamError {
                    stream_id: self.id,
                    error_type: format!("Failed to send message: {}", e),
                })?;
            } else {
                return Err(AppError::StreamError {
                    stream_id: self.id,
                    error_type: "Stream sender not available".to_string(),
                });
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.bytes_sent += message_size as u64;
            metrics.messages_sent += 1;
            metrics.last_activity = Some(Instant::now());
        }

        // Update last activity
        {
            let mut last_activity = self.last_activity.lock().unwrap();
            *last_activity = Instant::now();
        }

        Ok(())
    }

    /// Receive a message from this stream
    pub async fn receive_message(&self) -> Result<Option<ValkyrieMessage>> {
        let mut receiver_guard = self.receiver.lock().unwrap();
        if let Some(receiver) = receiver_guard.as_mut() {
            match receiver.try_recv() {
                Ok(message) => {
                    // Update metrics
                    {
                        let mut metrics = self.metrics.lock().unwrap();
                        metrics.bytes_received += message.payload.len() as u64;
                        metrics.messages_received += 1;
                        metrics.last_activity = Some(Instant::now());
                    }

                    // Update last activity
                    {
                        let mut last_activity = self.last_activity.lock().unwrap();
                        *last_activity = Instant::now();
                    }

                    Ok(Some(message))
                },
                Err(mpsc::error::TryRecvError::Empty) => Ok(None),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    Err(AppError::StreamError {
                        stream_id: self.id,
                        error_type: "Stream receiver disconnected".to_string(),
                    })
                }
            }
        } else {
            Err(AppError::StreamError {
                stream_id: self.id,
                error_type: "Stream receiver not available".to_string(),
            })
        }
    }

    /// Update stream state
    pub fn update_state(&self, new_state: StreamState) {
        let old_state = {
            let mut state = self.state.write().unwrap();
            let old = state.clone();
            *state = new_state.clone();
            old
        };

        tracing::debug!("Stream {} state changed: {:?} -> {:?}", self.id, old_state, new_state);
    }

    /// Check if stream is active
    pub fn is_active(&self) -> bool {
        let state = self.state.read().unwrap();
        matches!(*state, StreamState::Active | StreamState::Paused)
    }

    /// Check if stream has timed out
    pub fn is_timed_out(&self) -> bool {
        let last_activity = self.last_activity.lock().unwrap();
        last_activity.elapsed() > self.config.timeout
    }

    /// Get current stream metrics
    pub fn get_metrics(&self) -> StreamMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Close the stream gracefully
    pub async fn close(&self, reason: String) -> Result<()> {
        self.update_state(StreamState::Closing);

        // Close sender
        {
            let mut sender_guard = self.sender.lock().unwrap();
            *sender_guard = None;
        }

        // Close receiver
        {
            let mut receiver_guard = self.receiver.lock().unwrap();
            *receiver_guard = None;
        }

        self.update_state(StreamState::Closed);
        tracing::info!("Stream {} closed: {}", self.id, reason);

        Ok(())
    }
}

impl StreamMultiplexer {
    /// Create a new stream multiplexer
    pub fn new(
        config: MultiplexerConfig,
        event_handler: Arc<dyn StreamEventHandler>,
    ) -> Self {
        let flow_control = Arc::new(FlowControlManager::new(config.clone()));
        let priority_scheduler = Arc::new(PriorityScheduler::new());
        let congestion_controller = Arc::new(CongestionController::new());
        let metrics = Arc::new(StreamMetricsCollector::new());

        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            allocation_strategy: StreamAllocationStrategy::PriorityBased,
            flow_control,
            priority_scheduler,
            congestion_controller,
            metrics,
            event_handler,
            config,
            shutdown_tx: None,
            task_handles: Vec::new(),
        }
    }

    /// Start the multiplexer background tasks
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Start cleanup task
        let streams_clone = Arc::clone(&self.streams);
        let config_clone = self.config.clone();
        let cleanup_handle = tokio::spawn(async move {
            let mut cleanup_interval = interval(config_clone.cleanup_interval);
            
            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        Self::cleanup_streams(&streams_clone, &config_clone).await;
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!("Stream multiplexer cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        self.task_handles.push(cleanup_handle);

        tracing::info!("Stream multiplexer started");
        Ok(())
    }

    /// Stop the multiplexer and cleanup resources
    pub async fn stop(&mut self) -> Result<()> {
        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        // Wait for background tasks to complete
        for handle in self.task_handles.drain(..) {
            let _ = handle.await;
        }

        // Close all active streams
        let streams = self.streams.read().unwrap();
        for stream in streams.values() {
            let _ = stream.close("Multiplexer shutdown".to_string()).await;
        }

        tracing::info!("Stream multiplexer stopped");
        Ok(())
    }

    /// Create a new stream
    pub async fn create_stream(
        &self,
        stream_type: StreamType,
        priority: StreamPriority,
        config: Option<StreamConfig>,
    ) -> Result<StreamId> {
        // Check stream limit
        {
            let streams = self.streams.read().unwrap();
            if streams.len() >= self.config.max_streams {
                return Err(AppError::ResourceExhausted(
                    format!("Maximum streams ({}) exceeded", self.config.max_streams)
                ));
            }
        }

        let stream_id = Uuid::new_v4();
        let stream_config = config.unwrap_or_else(|| self.config.default_stream_config.clone());
        
        let stream = Arc::new(Stream::new(stream_id, stream_type.clone(), priority, stream_config.clone()));
        stream.update_state(StreamState::Active);

        // Add to streams registry
        {
            let mut streams = self.streams.write().unwrap();
            streams.insert(stream_id, stream);
        }

        // Emit stream created event
        self.event_handler.handle_event(StreamEvent::Created {
            stream_id,
            stream_type,
            config: stream_config,
        });

        tracing::info!("Created stream {} with priority {:?}", stream_id, priority);
        Ok(stream_id)
    }

    /// Get a stream by ID
    pub fn get_stream(&self, stream_id: &StreamId) -> Option<Arc<Stream>> {
        let streams = self.streams.read().unwrap();
        streams.get(stream_id).cloned()
    }

    /// Send a message on a specific stream
    pub async fn send_message(
        &self,
        stream_id: StreamId,
        message: ValkyrieMessage,
    ) -> Result<()> {
        let stream = self.get_stream(&stream_id)
            .ok_or_else(|| AppError::StreamError {
                stream_id,
                error_type: "Stream not found".to_string(),
            })?;

        let message_size = message.payload.len();
        stream.send_message(message).await?;

        // Emit data sent event
        self.event_handler.handle_event(StreamEvent::DataSent {
            stream_id,
            bytes: message_size,
        });

        Ok(())
    }

    /// Receive a message from a specific stream
    pub async fn receive_message(&self, stream_id: StreamId) -> Result<Option<ValkyrieMessage>> {
        let stream = self.get_stream(&stream_id)
            .ok_or_else(|| AppError::StreamError {
                stream_id,
                error_type: "Stream not found".to_string(),
            })?;

        let message = stream.receive_message().await?;

        if let Some(ref msg) = message {
            // Emit data received event
            self.event_handler.handle_event(StreamEvent::DataReceived {
                stream_id,
                bytes: msg.payload.len(),
            });
        }

        Ok(message)
    }

    /// Close a stream
    pub async fn close_stream(&self, stream_id: StreamId, reason: String) -> Result<()> {
        let stream = self.get_stream(&stream_id)
            .ok_or_else(|| AppError::StreamError {
                stream_id,
                error_type: "Stream not found".to_string(),
            })?;

        stream.close(reason.clone()).await?;

        // Remove from streams registry
        {
            let mut streams = self.streams.write().unwrap();
            streams.remove(&stream_id);
        }

        // Emit stream closed event
        self.event_handler.handle_event(StreamEvent::Closed {
            stream_id,
            reason,
        });

        Ok(())
    }

    /// Get all active streams
    pub fn get_active_streams(&self) -> Vec<StreamId> {
        let streams = self.streams.read().unwrap();
        streams.keys().cloned().collect()
    }

    /// Get stream metrics
    pub fn get_stream_metrics(&self, stream_id: &StreamId) -> Option<StreamMetrics> {
        self.get_stream(stream_id).map(|stream| stream.get_metrics())
    }

    /// Get multiplexer statistics
    pub fn get_statistics(&self) -> MultiplexerStatistics {
        let streams = self.streams.read().unwrap();
        let total_streams = streams.len();
        
        let mut active_streams = 0;
        let mut paused_streams = 0;
        let mut total_bytes_sent = 0;
        let mut total_bytes_received = 0;

        for stream in streams.values() {
            let state = stream.state.read().unwrap();
            match *state {
                StreamState::Active => active_streams += 1,
                StreamState::Paused => paused_streams += 1,
                _ => {}
            }

            let metrics = stream.get_metrics();
            total_bytes_sent += metrics.bytes_sent;
            total_bytes_received += metrics.bytes_received;
        }

        MultiplexerStatistics {
            total_streams,
            active_streams,
            paused_streams,
            total_bytes_sent,
            total_bytes_received,
        }
    }

    /// Cleanup expired streams
    async fn cleanup_streams(
        streams: &Arc<RwLock<HashMap<StreamId, Arc<Stream>>>>,
        _config: &MultiplexerConfig,
    ) {
        let mut expired_streams = Vec::new();

        // Identify expired streams
        {
            let streams_guard = streams.read().unwrap();
            for (stream_id, stream) in streams_guard.iter() {
                if stream.is_timed_out() {
                    expired_streams.push(*stream_id);
                }
            }
        }

        // Close expired streams
        for stream_id in expired_streams {
            if let Some(stream) = {
                let streams_guard = streams.read().unwrap();
                streams_guard.get(&stream_id).cloned()
            } {
                let _ = stream.close("Stream timeout".to_string()).await;
                
                // Remove from registry
                let mut streams_guard = streams.write().unwrap();
                streams_guard.remove(&stream_id);
                
                tracing::info!("Cleaned up expired stream: {}", stream_id);
            }
        }
    }
}

/// Multiplexer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiplexerStatistics {
    pub total_streams: usize,
    pub active_streams: usize,
    pub paused_streams: usize,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
}

/// Stream metrics collector
pub struct StreamMetricsCollector {
    /// Global metrics
    global_metrics: Arc<Mutex<HashMap<String, u64>>>,
}

impl StreamMetricsCollector {
    pub fn new() -> Self {
        Self {
            global_metrics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn record_metric(&self, name: String, value: u64) {
        let mut metrics = self.global_metrics.lock().unwrap();
        *metrics.entry(name).or_insert(0) += value;
    }

    pub fn get_metrics(&self) -> HashMap<String, u64> {
        self.global_metrics.lock().unwrap().clone()
    }
}