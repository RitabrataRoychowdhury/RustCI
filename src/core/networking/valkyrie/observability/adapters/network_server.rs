// High-performance network server for plug-and-play observability adapters
// Enables external systems to connect directly to Valkyrie with 100μs performance

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::timeout;
use uuid::Uuid;

use super::{AdapterPerformanceMetrics, AdapterProtocol, HealthStatus, ObservabilityError};

/// High-performance network server for observability adapters
pub struct ObservabilityNetworkServer {
    /// Server bind address
    bind_address: String,
    /// Server port
    port: u16,
    /// Maximum concurrent connections
    max_connections: usize,
    /// Active connections
    connections: Arc<RwLock<HashMap<Uuid, ActiveConnection>>>,
    /// Connection handler
    connection_handler: Arc<ConnectionHandler>,
    /// Server state
    server_state: Arc<RwLock<ServerState>>,
    /// Performance metrics
    performance_metrics: Arc<RwLock<ServerPerformanceMetrics>>,
}

/// Active connection information
#[derive(Debug, Clone)]
pub struct ActiveConnection {
    /// Connection ID
    pub id: Uuid,
    /// Remote address
    pub remote_addr: SocketAddr,
    /// Connection start time
    pub connected_at: Instant,
    /// Connection type
    pub connection_type: ConnectionType,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Request count
    pub request_count: u64,
    /// Last activity
    pub last_activity: Instant,
}

/// Connection type enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionType {
    Prometheus,
    OpenTelemetry,
    Jaeger,
    Grafana,
    Custom(String),
    Unknown,
}

/// Server state
#[derive(Debug, Clone)]
pub struct ServerState {
    /// Server running status
    pub running: bool,
    /// Server start time
    pub start_time: Option<Instant>,
    /// Total connections handled
    pub total_connections: u64,
    /// Current active connections
    pub active_connections: usize,
    /// Total requests handled
    pub total_requests: u64,
    /// Total bytes transferred
    pub total_bytes: u64,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            running: false,
            start_time: None,
            total_connections: 0,
            active_connections: 0,
            total_requests: 0,
            total_bytes: 0,
        }
    }
}

/// Server performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPerformanceMetrics {
    /// Average request latency in microseconds
    pub avg_request_latency_us: f64,
    /// P95 request latency in microseconds
    pub p95_request_latency_us: f64,
    /// P99 request latency in microseconds
    pub p99_request_latency_us: f64,
    /// Requests per second
    pub requests_per_second: f64,
    /// Connections per second
    pub connections_per_second: f64,
    /// Bytes per second
    pub bytes_per_second: f64,
    /// Active connections
    pub active_connections: usize,
    /// Connection success rate
    pub connection_success_rate: f64,
    /// Error rate
    pub error_rate: f64,
}

/// Connection handler for processing requests
pub struct ConnectionHandler {
    /// Request handlers by protocol
    handlers: Arc<RwLock<HashMap<AdapterProtocol, Box<dyn ProtocolHandler>>>>,
    /// Performance tracker
    performance_tracker: Arc<RwLock<RequestPerformanceTracker>>,
}

/// Protocol handler trait for different observability protocols
#[async_trait::async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Handle incoming request
    async fn handle_request(
        &self,
        request: ProtocolRequest,
    ) -> Result<ProtocolResponse, ObservabilityError>;

    /// Get handler capabilities
    fn capabilities(&self) -> ProtocolCapabilities;
}

/// Protocol request
#[derive(Debug, Clone)]
pub struct ProtocolRequest {
    /// Request ID
    pub id: Uuid,
    /// Connection ID
    pub connection_id: Uuid,
    /// Request timestamp
    pub timestamp: Instant,
    /// Request method/operation
    pub method: String,
    /// Request path/endpoint
    pub path: String,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Vec<u8>,
    /// Query parameters
    pub query_params: HashMap<String, String>,
}

/// Protocol response
#[derive(Debug, Clone)]
pub struct ProtocolResponse {
    /// Response status code
    pub status_code: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Vec<u8>,
    /// Processing duration
    pub processing_duration: Duration,
}

/// Protocol capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCapabilities {
    /// Supported operations
    pub supported_operations: Vec<String>,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Maximum response size in bytes
    pub max_response_size: usize,
    /// Supports streaming
    pub supports_streaming: bool,
    /// Supports compression
    pub supports_compression: bool,
}

/// Request performance tracker
#[derive(Debug)]
pub struct RequestPerformanceTracker {
    /// Latency samples (ring buffer for 100μs performance tracking)
    latency_samples_us: Vec<u64>,
    /// Sample index
    sample_index: usize,
    /// Total requests
    total_requests: u64,
    /// Error count
    error_count: u64,
    /// Start time
    start_time: Instant,
}

impl RequestPerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            latency_samples_us: vec![0; 10000], // Keep last 10k samples for accurate percentiles
            sample_index: 0,
            total_requests: 0,
            error_count: 0,
            start_time: Instant::now(),
        }
    }

    /// Record request latency in microseconds
    pub fn record_request(&mut self, latency_us: u64, is_error: bool) {
        self.latency_samples_us[self.sample_index] = latency_us;
        self.sample_index = (self.sample_index + 1) % self.latency_samples_us.len();
        self.total_requests += 1;

        if is_error {
            self.error_count += 1;
        }
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> ServerPerformanceMetrics {
        let mut sorted_latencies = self.latency_samples_us.clone();
        sorted_latencies.sort_unstable();

        let avg_latency = if !sorted_latencies.is_empty() {
            sorted_latencies.iter().sum::<u64>() as f64 / sorted_latencies.len() as f64
        } else {
            0.0
        };

        let p95_index = (sorted_latencies.len() as f64 * 0.95) as usize;
        let p99_index = (sorted_latencies.len() as f64 * 0.99) as usize;

        let p95_latency = sorted_latencies.get(p95_index).copied().unwrap_or(0) as f64;
        let p99_latency = sorted_latencies.get(p99_index).copied().unwrap_or(0) as f64;

        let elapsed = self.start_time.elapsed().as_secs_f64();
        let requests_per_second = if elapsed > 0.0 {
            self.total_requests as f64 / elapsed
        } else {
            0.0
        };

        let error_rate = if self.total_requests > 0 {
            self.error_count as f64 / self.total_requests as f64
        } else {
            0.0
        };

        ServerPerformanceMetrics {
            avg_request_latency_us: avg_latency,
            p95_request_latency_us: p95_latency,
            p99_request_latency_us: p99_latency,
            requests_per_second,
            connections_per_second: 0.0, // Would be calculated separately
            bytes_per_second: 0.0,       // Would be calculated separately
            active_connections: 0,       // Would be provided by server
            connection_success_rate: 1.0 - error_rate,
            error_rate,
        }
    }
}

impl ObservabilityNetworkServer {
    /// Create new network server
    pub fn new(bind_address: String, port: u16, max_connections: usize) -> Self {
        Self {
            bind_address,
            port,
            max_connections,
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_handler: Arc::new(ConnectionHandler::new()),
            server_state: Arc::new(RwLock::new(ServerState::default())),
            performance_metrics: Arc::new(RwLock::new(ServerPerformanceMetrics {
                avg_request_latency_us: 0.0,
                p95_request_latency_us: 0.0,
                p99_request_latency_us: 0.0,
                requests_per_second: 0.0,
                connections_per_second: 0.0,
                bytes_per_second: 0.0,
                active_connections: 0,
                connection_success_rate: 1.0,
                error_rate: 0.0,
            })),
        }
    }

    /// Start the network server
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let bind_addr = format!("{}:{}", self.bind_address, self.port);
        let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
            ObservabilityError::Internal(format!("Failed to bind to {}: {}", bind_addr, e))
        })?;

        // Update server state
        {
            let mut state = self.server_state.write().await;
            state.running = true;
            state.start_time = Some(Instant::now());
        }

        // Spawn connection acceptor task
        let connections = self.connections.clone();
        let connection_handler = self.connection_handler.clone();
        let server_state = self.server_state.clone();
        let max_connections = self.max_connections;

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        // Check connection limit
                        let current_connections = {
                            let connections_guard = connections.read().await;
                            connections_guard.len()
                        };

                        if current_connections >= max_connections {
                            // Drop connection if at limit
                            continue;
                        }

                        // Create connection
                        let connection_id = Uuid::new_v4();
                        let active_connection = ActiveConnection {
                            id: connection_id,
                            remote_addr: addr,
                            connected_at: Instant::now(),
                            connection_type: ConnectionType::Unknown,
                            bytes_sent: 0,
                            bytes_received: 0,
                            request_count: 0,
                            last_activity: Instant::now(),
                        };

                        // Store connection
                        {
                            let mut connections_guard = connections.write().await;
                            connections_guard.insert(connection_id, active_connection);
                        }

                        // Update server state
                        {
                            let mut state = server_state.write().await;
                            state.total_connections += 1;
                            state.active_connections = current_connections + 1;
                        }

                        // Spawn connection handler
                        let connections_clone = connections.clone();
                        let handler_clone = connection_handler.clone();
                        let state_clone = server_state.clone();

                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(
                                stream,
                                connection_id,
                                connections_clone,
                                handler_clone,
                                state_clone,
                            )
                            .await
                            {
                                eprintln!("Connection handling error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the network server
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut state = self.server_state.write().await;
        state.running = false;

        // Close all active connections
        let mut connections = self.connections.write().await;
        connections.clear();

        Ok(())
    }

    /// Handle individual connection
    async fn handle_connection(
        stream: TcpStream,
        connection_id: Uuid,
        connections: Arc<RwLock<HashMap<Uuid, ActiveConnection>>>,
        handler: Arc<ConnectionHandler>,
        server_state: Arc<RwLock<ServerState>>,
    ) -> Result<(), ObservabilityError> {
        let mut buffer = vec![0; 8192]; // 8KB buffer for high performance

        loop {
            // Set timeout for 100μs target performance
            let read_result = timeout(Duration::from_micros(100), stream.readable()).await;

            match read_result {
                Ok(_) => {
                    // Connection is readable, process request
                    match stream.try_read(&mut buffer) {
                        Ok(0) => {
                            // Connection closed
                            break;
                        }
                        Ok(n) => {
                            let request_start = Instant::now();

                            // Parse request (simplified HTTP-like protocol)
                            let request_data = &buffer[..n];
                            let request = Self::parse_request(connection_id, request_data)?;

                            // Handle request
                            let response = handler.handle_request(request).await?;

                            // Send response
                            let response_data = Self::serialize_response(&response)?;
                            if let Err(e) = stream.try_write(&response_data) {
                                eprintln!("Failed to write response: {}", e);
                                break;
                            }

                            // Record performance (targeting 100μs)
                            let request_duration = request_start.elapsed();
                            let latency_us = request_duration.as_micros() as u64;

                            // Update connection stats
                            {
                                let mut connections_guard = connections.write().await;
                                if let Some(conn) = connections_guard.get_mut(&connection_id) {
                                    conn.bytes_received += n as u64;
                                    conn.bytes_sent += response_data.len() as u64;
                                    conn.request_count += 1;
                                    conn.last_activity = Instant::now();
                                }
                            }

                            // Update server stats
                            {
                                let mut state = server_state.write().await;
                                state.total_requests += 1;
                                state.total_bytes += n as u64 + response_data.len() as u64;
                            }

                            // Record performance metrics
                            handler.record_performance(latency_us, false).await;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available, continue
                            continue;
                        }
                        Err(e) => {
                            eprintln!("Failed to read from connection: {}", e);
                            break;
                        }
                    }
                }
                Err(_) => {
                    // Timeout - check if connection is still alive
                    continue;
                }
            }
        }

        // Clean up connection
        {
            let mut connections_guard = connections.write().await;
            connections_guard.remove(&connection_id);
        }

        {
            let mut state = server_state.write().await;
            state.active_connections = state.active_connections.saturating_sub(1);
        }

        Ok(())
    }

    /// Parse incoming request (simplified protocol)
    fn parse_request(
        connection_id: Uuid,
        data: &[u8],
    ) -> Result<ProtocolRequest, ObservabilityError> {
        // Simplified HTTP-like request parsing for maximum performance
        let request_str = String::from_utf8_lossy(data);
        let lines: Vec<&str> = request_str.lines().collect();

        if lines.is_empty() {
            return Err(ObservabilityError::Internal("Empty request".to_string()));
        }

        // Parse request line (METHOD PATH HTTP/1.1)
        let request_line_parts: Vec<&str> = lines[0].split_whitespace().collect();
        if request_line_parts.len() < 2 {
            return Err(ObservabilityError::Internal(
                "Invalid request line".to_string(),
            ));
        }

        let method = request_line_parts[0].to_string();
        let path_and_query = request_line_parts[1];

        // Split path and query parameters
        let (path, query_params) = if let Some(query_start) = path_and_query.find('?') {
            let path = path_and_query[..query_start].to_string();
            let query_str = &path_and_query[query_start + 1..];
            let params = Self::parse_query_params(query_str);
            (path, params)
        } else {
            (path_and_query.to_string(), HashMap::new())
        };

        // Parse headers
        let mut headers = HashMap::new();
        let mut body_start = 1;

        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                body_start = i + 1;
                break;
            }

            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_lowercase();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        // Extract body
        let body = if body_start < lines.len() {
            lines[body_start..].join("\n").into_bytes()
        } else {
            Vec::new()
        };

        Ok(ProtocolRequest {
            id: Uuid::new_v4(),
            connection_id,
            timestamp: Instant::now(),
            method,
            path,
            headers,
            body,
            query_params,
        })
    }

    /// Parse query parameters
    fn parse_query_params(query_str: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();

        for param in query_str.split('&') {
            if let Some(eq_pos) = param.find('=') {
                let key = param[..eq_pos].to_string();
                let value = param[eq_pos + 1..].to_string();
                params.insert(key, value);
            }
        }

        params
    }

    /// Serialize response to bytes
    fn serialize_response(response: &ProtocolResponse) -> Result<Vec<u8>, ObservabilityError> {
        let mut response_data = Vec::new();

        // Status line
        let status_line = format!("HTTP/1.1 {} OK\r\n", response.status_code);
        response_data.extend_from_slice(status_line.as_bytes());

        // Headers
        for (key, value) in &response.headers {
            let header_line = format!("{}: {}\r\n", key, value);
            response_data.extend_from_slice(header_line.as_bytes());
        }

        // Content-Length header
        let content_length = format!("Content-Length: {}\r\n", response.body.len());
        response_data.extend_from_slice(content_length.as_bytes());

        // End of headers
        response_data.extend_from_slice(b"\r\n");

        // Body
        response_data.extend_from_slice(&response.body);

        Ok(response_data)
    }

    /// Get server health status
    pub async fn health(&self) -> HealthStatus {
        let state = self.server_state.read().await;

        if state.running {
            let connections = self.connections.read().await;
            if connections.len() < self.max_connections {
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            }
        } else {
            HealthStatus::Unhealthy
        }
    }

    /// Get server performance metrics
    pub async fn performance_metrics(&self) -> AdapterPerformanceMetrics {
        let performance = self.performance_metrics.read().await;
        let state = self.server_state.read().await;

        AdapterPerformanceMetrics {
            avg_latency_us: performance.avg_request_latency_us,
            p95_latency_us: performance.p95_request_latency_us,
            p99_latency_us: performance.p99_request_latency_us,
            throughput_ops: performance.requests_per_second,
            memory_usage_bytes: 0,  // Would be calculated separately
            cpu_usage_percent: 0.0, // Would be calculated separately
            network_bytes_sent: state.total_bytes,
            network_bytes_received: state.total_bytes,
        }
    }
}

impl ConnectionHandler {
    /// Create new connection handler
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            performance_tracker: Arc::new(RwLock::new(RequestPerformanceTracker::new())),
        }
    }

    /// Register protocol handler
    pub async fn register_handler(
        &self,
        protocol: AdapterProtocol,
        handler: Box<dyn ProtocolHandler>,
    ) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(protocol, handler);
    }

    /// Handle incoming request
    pub async fn handle_request(
        &self,
        request: ProtocolRequest,
    ) -> Result<ProtocolResponse, ObservabilityError> {
        let start_time = Instant::now();

        // Determine protocol from request
        let protocol = self.detect_protocol(&request);

        // Get appropriate handler
        let handlers = self.handlers.read().await;
        let handler = handlers.get(&protocol);

        let response = if let Some(handler) = handler {
            handler.handle_request(request).await?
        } else {
            // Default response for unsupported protocols
            ProtocolResponse {
                status_code: 404,
                headers: HashMap::new(),
                body: b"Protocol not supported".to_vec(),
                processing_duration: start_time.elapsed(),
            }
        };

        Ok(response)
    }

    /// Detect protocol from request
    fn detect_protocol(&self, request: &ProtocolRequest) -> AdapterProtocol {
        // Detect protocol based on path, headers, or content
        if request.path.starts_with("/metrics") {
            AdapterProtocol::Http // Prometheus
        } else if request.path.starts_with("/v1/traces") || request.path.starts_with("/v1/metrics")
        {
            AdapterProtocol::Grpc // OpenTelemetry
        } else if request
            .headers
            .get("user-agent")
            .map_or(false, |ua| ua.contains("Jaeger"))
        {
            AdapterProtocol::Http // Jaeger
        } else if request.headers.get("authorization").is_some() {
            AdapterProtocol::Https // Grafana (typically uses auth)
        } else {
            AdapterProtocol::Http // Default
        }
    }

    /// Record performance metrics
    pub async fn record_performance(&self, latency_us: u64, is_error: bool) {
        let mut tracker = self.performance_tracker.write().await;
        tracker.record_request(latency_us, is_error);
    }
}

impl Default for RequestPerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}
