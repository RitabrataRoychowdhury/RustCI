//! HTTP Gateway for Valkyrie Protocol
//! 
//! Provides HTTP/HTTPS to Valkyrie protocol conversion with automatic
//! protocol negotiation and transparent fallback capabilities.

use super::{
    BridgeConfig, BridgeError, BridgeResult, HttpRequestContext, HttpResponseContext,
    HighPerformanceProcessor, PerformanceConfig, ZeroCopyBufferManager, BufferConfig
};
use crate::core::networking::valkyrie::{
    engine::ValkyrieEngine,
    message::{
        ValkyrieMessage, MessageHeader, MessageType, MessagePayload, MessageFlags, 
        MessagePriority, ProtocolInfo, ProtocolVersion, RoutingInfo, DestinationType,
        ServiceSelector, RoutingHints, LoadBalancingStrategy, ReliabilityLevel,
        CompressionInfo, CompressionPreference, CompressionAlgorithm
    },
};
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Router, Json, body::Body,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
    timeout::TimeoutLayer,
};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// HTTP Gateway that bridges HTTP/HTTPS requests to Valkyrie Protocol
pub struct HttpGateway {
    /// Gateway configuration
    config: BridgeConfig,
    /// Valkyrie engine instance
    valkyrie_engine: Arc<ValkyrieEngine>,
    /// Active HTTP sessions
    sessions: Arc<RwLock<HashMap<Uuid, HttpSession>>>,
    /// Gateway metrics
    metrics: Arc<GatewayMetrics>,
    /// High-performance processor for sub-millisecond responses
    performance_processor: Option<Arc<HighPerformanceProcessor>>,
    /// Zero-copy buffer manager
    buffer_manager: Arc<ZeroCopyBufferManager>,
}

/// HTTP session tracking
#[derive(Debug, Clone)]
pub struct HttpSession {
    /// Session ID
    pub id: Uuid,
    /// Client address
    pub client_addr: SocketAddr,
    /// Session start time
    pub start_time: Instant,
    /// Last activity time
    pub last_activity: Instant,
    /// Request count
    pub request_count: u64,
    /// Authentication status
    pub authenticated: bool,
    /// User ID if authenticated
    pub user_id: Option<String>,
}

/// Gateway metrics for monitoring
#[derive(Debug, Default)]
pub struct GatewayMetrics {
    /// Total requests processed
    pub total_requests: Arc<std::sync::atomic::AtomicU64>,
    /// Successful conversions
    pub successful_conversions: Arc<std::sync::atomic::AtomicU64>,
    /// Failed conversions
    pub failed_conversions: Arc<std::sync::atomic::AtomicU64>,
    /// Active sessions
    pub active_sessions: Arc<std::sync::atomic::AtomicU64>,
    /// Average response time
    pub avg_response_time_ms: Arc<std::sync::atomic::AtomicU64>,
}

/// HTTP request payload for conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestPayload {
    /// HTTP method
    pub method: String,
    /// Request path
    pub path: String,
    /// Query parameters
    pub query: HashMap<String, String>,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Option<serde_json::Value>,
    /// Content type
    pub content_type: Option<String>,
}

/// HTTP response payload from Valkyrie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponsePayload {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Option<serde_json::Value>,
    /// Content type
    pub content_type: Option<String>,
}

impl HttpGateway {
    /// Create a new HTTP Gateway
    pub async fn new(config: BridgeConfig, valkyrie_engine: Arc<ValkyrieEngine>) -> Result<Self, BridgeError> {
        // Initialize performance optimizations
        let buffer_config = BufferConfig {
            initial_size: 8192,
            max_size: 1024 * 1024,
            pool_size: 1000, // Larger pool for high throughput
            enable_simd: true,
            enable_mmap: true,
            ..Default::default()
        };
        
        let buffer_manager = Arc::new(ZeroCopyBufferManager::new(buffer_config));
        buffer_manager.initialize().await.map_err(|e| BridgeError::InternalError {
            message: format!("Failed to initialize buffer manager: {}", e),
        })?;

        // Initialize high-performance processor if enabled
        let performance_processor = if config.enable_performance_optimizations.unwrap_or(true) {
            let perf_config = PerformanceConfig {
                enable_zero_copy: true,
                enable_simd: true,
                request_queue_size: 50000, // Large queue for high throughput
                worker_threads: num_cpus::get() * 2, // More workers for better parallelism
                target_response_time_us: 200, // 200 microseconds target
                enable_prefetch: true,
            };
            
            let processor = HighPerformanceProcessor::new(perf_config).await.map_err(|e| BridgeError::InternalError {
                message: format!("Failed to initialize performance processor: {}", e),
            })?;
            
            // Start background workers
            processor.start_workers().await.map_err(|e| BridgeError::InternalError {
                message: format!("Failed to start performance workers: {}", e),
            })?;
            
            Some(Arc::new(processor))
        } else {
            None
        };

        Ok(Self {
            config,
            valkyrie_engine,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(GatewayMetrics::default()),
            performance_processor,
            buffer_manager,
        })
    }

    /// Start the HTTP gateway server
    pub async fn start(&self) -> BridgeResult<()> {
        info!("Starting HTTP Gateway on {}", self.config.http_listen_addr);

        let app = self.create_router().await?;
        
        let listener = tokio::net::TcpListener::bind(&self.config.http_listen_addr)
            .await
            .map_err(|e| BridgeError::ConfigurationError {
                parameter: "http_listen_addr".to_string(),
                message: format!("Failed to bind to address: {}", e),
            })?;

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| BridgeError::InternalError {
            message: format!("HTTP server error: {}", e),
        })?;

        Ok(())
    }

    /// Create the Axum router with all routes
    async fn create_router(&self) -> BridgeResult<Router> {
        let gateway = Arc::new(self.clone());

        let mut router = Router::new()
            .route("/health", get(health_check))
            .route("/metrics", get(gateway_metrics))
            .route("/api/v1/*path", any(handle_api_request))
            .route("/*path", any(handle_generic_request))
            .with_state(gateway.clone());

        // Add middleware layers
        if self.config.cors_enabled {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            router = router.layer(cors);
        }

        router = router
            .layer(TimeoutLayer::new(Duration::from_millis(
                self.config.request_timeout_ms,
            )))
            .layer(TraceLayer::new_for_http());

        Ok(router)
    }

    /// Convert HTTP request to Valkyrie message
    pub async fn http_to_valkyrie(
        &self,
        context: &HttpRequestContext,
        payload: HttpRequestPayload,
    ) -> BridgeResult<ValkyrieMessage> {
        debug!("Converting HTTP request to Valkyrie message: {}", context.request_id);

        let message_type = match payload.method.as_str() {
            "GET" => MessageType::JobRequest, // Map to appropriate Valkyrie message type
            "POST" => MessageType::JobRequest,
            "PUT" => MessageType::JobRequest,
            "DELETE" => MessageType::JobCancel,
            _ => return Err(BridgeError::HttpConversionFailed {
                details: format!("Unsupported HTTP method: {}", payload.method),
            }),
        };

        let header = MessageHeader {
            protocol_info: ProtocolInfo {
                magic: 0x56414C4B, // "VALK"
                version: ProtocolVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                extensions: vec![],
            },
            message_type,
            stream_id: 0, // HTTP bridge uses stream 0
            flags: MessageFlags {
                requires_ack: true,
                compressed: false,
                encrypted: false,
                fragmented: false,
                urgent: false,
                retransmission: false,
                binary_data: false,
                custom_flags: 0,
            },
            priority: MessagePriority::Normal,
            timestamp: chrono::Utc::now(),
            ttl: Some(Duration::from_millis(self.config.request_timeout_ms)),
            correlation_id: Some(context.request_id),
            routing: RoutingInfo {
                source: "http_gateway".to_string(),
                destination: DestinationType::Service(ServiceSelector {
                    service: "rustci".to_string(),
                    version: None,
                    tags: HashMap::new(),
                    preferred_instance: None,
                }),
                hints: RoutingHints {
                    preferred_transport: Some("tcp".to_string()),
                    latency_sensitive: true,
                    bandwidth_requirements: None,
                    reliability_level: ReliabilityLevel::AtLeastOnce,
                    cacheable: false,
                    compression_preference: CompressionPreference::None,
                },
                load_balancing: LoadBalancingStrategy::RoundRobin,
                path: vec![],
            },
            compression: CompressionInfo {
                algorithm: CompressionAlgorithm::None,
                original_size: None,
                compressed_size: None,
                level: None,
            },
            sequence_number: 0,
            ack_number: None,
        };

        let valkyrie_payload = MessagePayload::Json(serde_json::to_value(payload)
            .map_err(|e| BridgeError::HttpConversionFailed {
                details: format!("Failed to serialize payload: {}", e),
            })?);

        Ok(ValkyrieMessage {
            header,
            payload: valkyrie_payload,
            signature: None,
            trace_context: None,
        })
    }

    /// Convert Valkyrie message to HTTP response
    pub async fn valkyrie_to_http(
        &self,
        message: ValkyrieMessage,
    ) -> BridgeResult<HttpResponsePayload> {
        debug!("Converting Valkyrie message to HTTP response");

        let status = match message.header.message_type {
            MessageType::JobComplete => 200,
            MessageType::JobFailed => 500,
            MessageType::JobReject => 400,
            MessageType::Error => 500,
            _ => 200,
        };

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Valkyrie-Message-Type".to_string(), format!("{:?}", message.header.message_type));

        if let Some(correlation_id) = message.header.correlation_id {
            headers.insert("X-Correlation-ID".to_string(), correlation_id.to_string());
        }

        let body = match message.payload {
            MessagePayload::Json(value) => Some(value),
            MessagePayload::Binary(data) => {
                headers.insert("Content-Type".to_string(), "application/octet-stream".to_string());
                Some(serde_json::Value::String(base64ct::Base64::encode_string(data)))
            },
            MessagePayload::Text(text) => {
                headers.insert("Content-Type".to_string(), "text/plain".to_string());
                Some(serde_json::Value::String(text))
            },
            MessagePayload::Empty => None,
        };

        Ok(HttpResponsePayload {
            status,
            headers,
            body,
            content_type: Some("application/json".to_string()),
        })
    }

    /// Create HTTP session
    async fn create_session(&self, client_addr: SocketAddr) -> Uuid {
        let session_id = Uuid::new_v4();
        let session = HttpSession {
            id: session_id,
            client_addr,
            start_time: Instant::now(),
            last_activity: Instant::now(),
            request_count: 0,
            authenticated: false,
            user_id: None,
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session);
        
        self.metrics.active_sessions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        session_id
    }

    /// Update session activity
    async fn update_session(&self, session_id: Uuid) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.last_activity = Instant::now();
            session.request_count += 1;
        }
    }

    /// Process message through Valkyrie engine
    async fn process_valkyrie_message(&self, message: ValkyrieMessage) -> BridgeResult<ValkyrieMessage> {
        // For HTTP bridge, we simulate processing by creating a response message
        // In a full implementation, this would route through the actual engine
        let response_payload = match &message.payload {
            MessagePayload::Json(value) => {
                serde_json::json!({
                    "status": "success",
                    "message": "Request processed successfully via Valkyrie Protocol",
                    "original_payload": value,
                    "processed_at": chrono::Utc::now().to_rfc3339(),
                    "correlation_id": message.header.correlation_id
                })
            }
            _ => {
                serde_json::json!({
                    "status": "success",
                    "message": "Request processed successfully via Valkyrie Protocol",
                    "processed_at": chrono::Utc::now().to_rfc3339(),
                    "correlation_id": message.header.correlation_id
                })
            }
        };

        let response_message = ValkyrieMessage {
            header: MessageHeader {
                protocol_info: message.header.protocol_info.clone(),
                message_type: MessageType::JobComplete,
                stream_id: message.header.stream_id,
                flags: MessageFlags {
                    requires_ack: false,
                    compressed: false,
                    encrypted: false,
                    fragmented: false,
                    urgent: false,
                    retransmission: false,
                    binary_data: false,
                    custom_flags: 0,
                },
                priority: message.header.priority,
                timestamp: chrono::Utc::now(),
                ttl: Some(Duration::from_secs(30)),
                correlation_id: message.header.correlation_id,
                routing: RoutingInfo {
                    source: "valkyrie_engine".to_string(),
                    destination: DestinationType::Unicast("http_bridge".to_string()),
                    hints: message.header.routing.hints.clone(),
                    load_balancing: message.header.routing.load_balancing.clone(),
                    path: vec!["valkyrie_engine".to_string()],
                },
                compression: CompressionInfo {
                    algorithm: CompressionAlgorithm::None,
                    original_size: None,
                    compressed_size: None,
                    level: None,
                },
                sequence_number: message.header.sequence_number + 1,
                ack_number: Some(message.header.sequence_number),
            },
            payload: MessagePayload::Json(response_payload),
            signature: None,
            trace_context: message.trace_context,
        };

        Ok(response_message)
    }

    /// Clean up expired sessions
    pub async fn cleanup_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Instant::now();
        let session_timeout = Duration::from_secs(3600); // 1 hour

        sessions.retain(|_, session| {
            let expired = now.duration_since(session.last_activity) > session_timeout;
            if expired {
                self.metrics.active_sessions.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            !expired
        });
    }
}

impl Clone for HttpGateway {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            valkyrie_engine: self.valkyrie_engine.clone(),
            sessions: self.sessions.clone(),
            metrics: self.metrics.clone(),
            performance_processor: self.performance_processor.clone(),
            buffer_manager: self.buffer_manager.clone(),
        }
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "valkyrie-http-gateway",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Gateway metrics endpoint
async fn gateway_metrics(
    State(gateway): State<Arc<HttpGateway>>,
) -> impl IntoResponse {
    let metrics = &gateway.metrics;
    
    Json(serde_json::json!({
        "total_requests": metrics.total_requests.load(std::sync::atomic::Ordering::Relaxed),
        "successful_conversions": metrics.successful_conversions.load(std::sync::atomic::Ordering::Relaxed),
        "failed_conversions": metrics.failed_conversions.load(std::sync::atomic::Ordering::Relaxed),
        "active_sessions": metrics.active_sessions.load(std::sync::atomic::Ordering::Relaxed),
        "avg_response_time_ms": metrics.avg_response_time_ms.load(std::sync::atomic::Ordering::Relaxed),
    }))
}

/// Handle API requests
async fn handle_api_request(
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(gateway): State<Arc<HttpGateway>>,
    body: Body,
) -> impl IntoResponse {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4();
    
    gateway.metrics.total_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let context = HttpRequestContext {
        request_id,
        method: method.clone(),
        uri: uri.clone(),
        headers: headers.clone(),
        client_ip: Some(addr.ip().to_string()),
        user_agent: headers.get("user-agent").and_then(|v| v.to_str().ok()).map(String::from),
        auth_token: headers.get("authorization").and_then(|v| v.to_str().ok()).map(String::from),
        timestamp: chrono::Utc::now(),
    };

    // Create session
    let session_id = gateway.create_session(addr).await;
    gateway.update_session(session_id).await;

    // Convert body to bytes
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            gateway.metrics.failed_conversions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    // Parse request payload
    let payload = HttpRequestPayload {
        method: method.to_string(),
        path: uri.path().to_string(),
        query: uri.query()
            .map(|q| serde_urlencoded::from_str(q).unwrap_or_default())
            .unwrap_or_default(),
        headers: headers.iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect(),
        body: if body_bytes.is_empty() {
            None
        } else {
            serde_json::from_slice(&body_bytes).ok()
        },
        content_type: headers.get("content-type").and_then(|v| v.to_str().ok()).map(String::from),
    };

    // Convert to Valkyrie message
    let valkyrie_message = match gateway.http_to_valkyrie(&context, payload).await {
        Ok(msg) => msg,
        Err(e) => {
            error!("Failed to convert HTTP to Valkyrie: {}", e);
            gateway.metrics.failed_conversions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Conversion failed: {}", e)).into_response();
        }
    };

    // Process request with performance optimizations if available
    let response_payload = if let Some(ref processor) = gateway.performance_processor {
        // Use high-performance processor for sub-millisecond responses
        match processor.process_request(body_bytes.clone()).await {
            Ok(response_bytes) => {
                // Parse the response back to HttpResponsePayload
                match serde_json::from_slice::<serde_json::Value>(&response_bytes) {
                    Ok(json_response) => HttpResponsePayload {
                        status: 200,
                        headers: {
                            let mut h = HashMap::new();
                            h.insert("Content-Type".to_string(), "application/json".to_string());
                            h.insert("X-Valkyrie-Optimized".to_string(), "true".to_string());
                            h
                        },
                        body: Some(json_response),
                        content_type: Some("application/json".to_string()),
                    },
                    Err(_) => HttpResponsePayload {
                        status: 200,
                        headers: {
                            let mut h = HashMap::new();
                            h.insert("Content-Type".to_string(), "text/plain".to_string());
                            h.insert("X-Valkyrie-Optimized".to_string(), "true".to_string());
                            h
                        },
                        body: Some(serde_json::Value::String(String::from_utf8_lossy(&response_bytes).to_string())),
                        content_type: Some("text/plain".to_string()),
                    }
                }
            }
            Err(e) => {
                error!("High-performance processor failed: {}", e);
                gateway.metrics.failed_conversions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                
                HttpResponsePayload {
                    status: 500,
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                    body: Some(serde_json::json!({
                        "error": "High-performance processing failed",
                        "message": e.to_string(),
                        "request_id": request_id
                    })),
                    content_type: Some("application/json".to_string()),
                }
            }
        }
    } else {
        // Fallback to standard Valkyrie message processing
        match gateway.process_valkyrie_message(valkyrie_message).await {
            Ok(response_msg) => {
                match gateway.valkyrie_to_http(response_msg).await {
                    Ok(payload) => payload,
                    Err(e) => {
                        error!("Failed to convert Valkyrie response to HTTP: {}", e);
                        gateway.metrics.failed_conversions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Response conversion failed: {}", e)).into_response();
                    }
                }
            }
            Err(e) => {
                error!("Failed to process message through Valkyrie engine: {}", e);
                gateway.metrics.failed_conversions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                
                // Return error response
                HttpResponsePayload {
                    status: 500,
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                    body: Some(serde_json::json!({
                        "error": "Internal server error",
                        "message": "Failed to process request through Valkyrie Protocol",
                        "request_id": request_id
                    })),
                    content_type: Some("application/json".to_string()),
                }
            }
        }
    };

    gateway.metrics.successful_conversions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    
    let duration = start_time.elapsed();
    gateway.metrics.avg_response_time_ms.store(
        duration.as_millis() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );

    (StatusCode::OK, Json(response_payload)).into_response()
}

/// Handle generic requests (non-API)
async fn handle_generic_request(
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(gateway): State<Arc<HttpGateway>>,
) -> impl IntoResponse {
    info!("Generic request: {} {}", method, uri);
    
    (StatusCode::OK, Json(serde_json::json!({
        "message": "Valkyrie HTTP Gateway",
        "method": method.to_string(),
        "path": uri.path(),
        "client": addr.to_string()
    }))).into_response()
}