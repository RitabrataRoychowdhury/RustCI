//! HTTP/HTTPS Adapter Implementation
//!
//! Provides HTTP/HTTPS adapter with automatic protocol conversion,
//! REST API gateway functionality, and transparent protocol negotiation.

use async_trait::async_trait;
use reqwest::{Client, ClientBuilder, Method, Request, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::factory::AdapterBuilder;
use super::*;
use crate::error::{Result, ValkyrieError};

/// HTTP/HTTPS Adapter with REST API gateway functionality
pub struct HttpAdapter {
    /// Adapter ID
    id: AdapterId,
    /// HTTP client with connection pooling
    client: Client,
    /// Base URL for requests
    base_url: String,
    /// Adapter configuration
    config: AdapterConfig,
    /// Adapter capabilities
    capabilities: AdapterCapabilities,
    /// Performance metrics
    metrics: Arc<RwLock<AdapterMetrics>>,
    /// Health status
    health_status: Arc<RwLock<HealthStatus>>,
    /// Request/response translator
    translator: Arc<RestTranslator>,
}

/// REST API to Valkyrie message translator
pub struct RestTranslator {
    /// Message type mappings
    type_mappings: HashMap<String, AdapterMessageType>,
    /// Default headers
    default_headers: HashMap<String, String>,
}

/// HTTP adapter builder
pub struct HttpAdapterBuilder {
    /// Default configuration
    default_config: AdapterConfig,
}

impl HttpAdapter {
    /// Create new HTTP adapter
    pub async fn new(config: AdapterConfig) -> Result<Self> {
        let client = Self::build_client(&config)?;
        let base_url = config
            .custom
            .get("base_url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8080")
            .to_string();

        let capabilities = Self::create_capabilities();
        let translator = Arc::new(RestTranslator::new());

        Ok(Self {
            id: Uuid::new_v4(),
            client,
            base_url,
            config,
            capabilities,
            metrics: Arc::new(RwLock::new(AdapterMetrics::default())),
            health_status: Arc::new(RwLock::new(HealthStatus::Unknown)),
            translator,
        })
    }

    /// Build HTTP client with configuration
    fn build_client(config: &AdapterConfig) -> Result<Client> {
        let mut builder = ClientBuilder::new()
            .timeout(config.connection.timeout)
            .pool_max_idle_per_host(config.connection.pool_size.unwrap_or(10))
            .pool_idle_timeout(config.connection.keepalive);

        // Configure TLS if enabled
        if config.security.tls_enabled {
            builder = builder.use_rustls_tls();

            if let Some(cert_path) = &config.security.cert_path {
                // Add certificate configuration
                debug!("Configuring TLS with certificate: {}", cert_path);
            }
        }

        builder.build().map_err(|e| {
            ValkyrieError::AdapterInitializationFailed(format!("HTTP client build failed: {}", e))
        })
    }

    /// Create HTTP adapter capabilities
    fn create_capabilities() -> AdapterCapabilities {
        AdapterCapabilities {
            adapter_type: AdapterType::Http,
            max_connections: Some(1000),
            max_message_size: 10 * 1024 * 1024, // 10MB
            latency_profile: LatencyProfile {
                avg_latency: Duration::from_millis(50),
                p95_latency: Duration::from_millis(100),
                p99_latency: Duration::from_millis(200),
                max_latency: Duration::from_secs(30),
            },
            throughput_profile: ThroughputProfile {
                max_messages_per_sec: 10000,
                max_bytes_per_sec: 100 * 1024 * 1024, // 100MB/s
                burst_capacity: 50000,
            },
            reliability_features: ReliabilityFeatures {
                supports_retry: true,
                supports_circuit_breaker: true,
                supports_health_check: true,
                supports_failover: true,
                supports_load_balancing: true,
            },
            security_features: SecurityFeatures {
                supports_tls: true,
                supports_mtls: true,
                supports_auth: true,
                supports_authz: true,
                supports_encryption: true,
            },
            transport_features: TransportFeatures {
                supports_streaming: true,
                supports_multiplexing: false,
                supports_compression: true,
                supports_keepalive: true,
                supports_pooling: true,
            },
            supported_operations: vec![
                AdapterOperation::Send,
                AdapterOperation::Receive,
                AdapterOperation::Stream,
            ],
        }
    }

    /// Convert Valkyrie message to HTTP request
    async fn message_to_request(&self, message: &AdapterMessage) -> Result<Request> {
        let url = format!("{}/api/v1/messages", self.base_url);
        let method = match message.message_type {
            AdapterMessageType::Request => Method::POST,
            AdapterMessageType::Command => Method::PUT,
            AdapterMessageType::Data => Method::POST,
            _ => Method::POST,
        };

        let mut request_builder = self.client.request(method, &url);

        // Add headers
        request_builder = request_builder
            .header("Content-Type", "application/json")
            .header("X-Message-ID", message.id.to_string())
            .header("X-Message-Type", format!("{:?}", message.message_type))
            .header("X-Priority", (message.priority as u8).to_string());

        // Add authentication if configured
        if let Some(token) = &self.config.security.auth_token {
            request_builder = request_builder.bearer_auth(token);
        }

        // Add custom headers from metadata
        for (key, value) in &message.metadata {
            if key.starts_with("http-") {
                let header_name = &key[5..]; // Remove "http-" prefix
                request_builder = request_builder.header(header_name, value);
            }
        }

        // Convert payload to JSON
        let json_payload = self.translator.payload_to_json(&message.payload)?;
        request_builder = request_builder.json(&json_payload);

        request_builder.build().map_err(|e| {
            ValkyrieError::MessageConversionFailed(format!("Request build failed: {}", e))
        })
    }

    /// Convert HTTP response to Valkyrie message
    async fn response_to_message(
        &self,
        response: Response,
        original_id: Uuid,
    ) -> Result<AdapterMessage> {
        let status = response.status();
        let headers = response.headers().clone();

        // Extract metadata from headers
        let mut metadata = HashMap::new();
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                metadata.insert(format!("http-{}", name.as_str()), value_str.to_string());
            }
        }
        metadata.insert("http-status".to_string(), status.as_u16().to_string());

        // Get response body
        let body_bytes = response.bytes().await.map_err(|e| {
            ValkyrieError::MessageConversionFailed(format!("Response body read failed: {}", e))
        })?;

        let payload = self.translator.json_to_payload(&body_bytes)?;

        Ok(AdapterMessage {
            id: Uuid::new_v4(),
            message_type: AdapterMessageType::Response,
            payload,
            metadata,
            timestamp: chrono::Utc::now(),
            priority: MessagePriority::Normal,
            routing: RoutingInfo {
                source: Some(self.id),
                destination: DestinationType::Direct(original_id),
                hints: HashMap::new(),
            },
        })
    }

    /// Update adapter metrics
    async fn update_metrics(&self, operation: &str, latency: Duration, success: bool) {
        let mut metrics = self.metrics.write().await;

        if success {
            match operation {
                "send" => metrics.messages_sent += 1,
                "receive" => metrics.messages_received += 1,
                _ => {}
            }
        } else {
            // Update error rate calculation
            let total_ops = metrics.messages_sent + metrics.messages_received;
            if total_ops > 0 {
                metrics.error_rate = (metrics.error_rate * (total_ops - 1) as f64
                    + if success { 0.0 } else { 1.0 })
                    / total_ops as f64;
            }
        }

        // Update latency metrics (simplified)
        metrics.avg_latency = (metrics.avg_latency + latency) / 2;
        if latency > metrics.p95_latency {
            metrics.p95_latency = latency;
        }
        if latency > metrics.p99_latency {
            metrics.p99_latency = latency;
        }
    }
}

#[async_trait]
impl UniversalAdapter for HttpAdapter {
    async fn send(&self, message: AdapterMessage, qos: QoSParams) -> Result<SendResult> {
        let send_start = Instant::now();

        debug!(
            "Sending HTTP message: {} (type: {:?})",
            message.id, message.message_type
        );

        // Convert message to HTTP request
        let request = self.message_to_request(&message).await?;
        let message_size = message.payload.len();

        // Apply QoS parameters
        let timeout = qos.max_latency.min(Duration::from_secs(30));

        // Send request with timeout
        let response_result = tokio::time::timeout(timeout, self.client.execute(request)).await;

        let send_result = match response_result {
            Ok(Ok(response)) => {
                let status = response.status();
                let success = status.is_success();

                let response_data = if success {
                    Some(response.bytes().await.unwrap_or_default().to_vec())
                } else {
                    None
                };

                SendResult {
                    success,
                    latency: send_start.elapsed(),
                    bytes_sent: message_size,
                    error: if success {
                        None
                    } else {
                        Some(format!("HTTP {}", status))
                    },
                    response: response_data,
                }
            }
            Ok(Err(e)) => SendResult {
                success: false,
                latency: send_start.elapsed(),
                bytes_sent: message_size,
                error: Some(format!("Request failed: {}", e)),
                response: None,
            },
            Err(_) => SendResult {
                success: false,
                latency: timeout,
                bytes_sent: message_size,
                error: Some("Request timeout".to_string()),
                response: None,
            },
        };

        // Update metrics
        self.update_metrics("send", send_result.latency, send_result.success)
            .await;

        if send_result.success {
            debug!(
                "HTTP message sent successfully in {:?}",
                send_result.latency
            );
        } else {
            warn!("HTTP message send failed: {:?}", send_result.error);
        }

        Ok(send_result)
    }

    async fn receive(&self, timeout: Option<Duration>) -> Result<Option<AdapterMessage>> {
        // HTTP adapter typically doesn't receive unsolicited messages
        // This would be implemented for webhook or long-polling scenarios
        debug!("HTTP adapter receive not implemented for request/response pattern");
        Ok(None)
    }

    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> HealthStatus {
        let health_url = format!("{}/health", self.base_url);

        match self.client.get(&health_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let status = HealthStatus::Healthy;
                    *self.health_status.write().await = status.clone();
                    status
                } else {
                    let status = HealthStatus::Degraded {
                        reason: format!("HTTP {}", response.status()),
                    };
                    *self.health_status.write().await = status.clone();
                    status
                }
            }
            Err(e) => {
                let status = HealthStatus::Unhealthy {
                    reason: format!("Health check failed: {}", e),
                };
                *self.health_status.write().await = status.clone();
                status
            }
        }
    }

    async fn metrics(&self) -> AdapterMetrics {
        self.metrics.read().await.clone()
    }

    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing HTTP adapter for {}", self.base_url);

        // Perform initial health check
        let health = self.health_check().await;
        if matches!(health, HealthStatus::Unhealthy { .. }) {
            warn!(
                "HTTP adapter initialized but health check failed: {}",
                health
            );
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down HTTP adapter {}", self.id);
        *self.health_status.write().await = HealthStatus::Unhealthy {
            reason: "Adapter shutdown".to_string(),
        };
        Ok(())
    }

    async fn update_config(&mut self, config: &AdapterConfig) -> Result<()> {
        info!("Updating HTTP adapter configuration");
        self.config = config.clone();

        // Rebuild client with new configuration
        self.client = Self::build_client(config)?;

        // Update base URL if changed
        if let Some(new_url) = config.custom.get("base_url").and_then(|v| v.as_str()) {
            self.base_url = new_url.to_string();
        }

        Ok(())
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Http
    }

    fn adapter_id(&self) -> &AdapterId {
        &self.id
    }
}

impl RestTranslator {
    /// Create new REST translator
    pub fn new() -> Self {
        let mut type_mappings = HashMap::new();
        type_mappings.insert("POST".to_string(), AdapterMessageType::Request);
        type_mappings.insert("PUT".to_string(), AdapterMessageType::Command);
        type_mappings.insert("GET".to_string(), AdapterMessageType::Request);
        type_mappings.insert("DELETE".to_string(), AdapterMessageType::Command);

        let mut default_headers = HashMap::new();
        default_headers.insert(
            "User-Agent".to_string(),
            "Valkyrie-HTTP-Adapter/1.0".to_string(),
        );

        Self {
            type_mappings,
            default_headers,
        }
    }

    /// Convert payload bytes to JSON
    pub fn payload_to_json(&self, payload: &[u8]) -> Result<Value> {
        // Try to parse as JSON first
        if let Ok(json) = serde_json::from_slice::<Value>(payload) {
            Ok(json)
        } else {
            // Convert binary data to base64 JSON object
            let base64_data = base64::encode(payload);
            Ok(serde_json::json!({
                "data": base64_data,
                "encoding": "base64",
                "size": payload.len()
            }))
        }
    }

    /// Convert JSON response to payload bytes
    pub fn json_to_payload(&self, json_bytes: &[u8]) -> Result<Vec<u8>> {
        // Try to parse as JSON
        if let Ok(json) = serde_json::from_slice::<Value>(json_bytes) {
            // Check if it's a base64 encoded object
            if let Some(obj) = json.as_object() {
                if let (Some(data), Some(encoding)) = (obj.get("data"), obj.get("encoding")) {
                    if encoding.as_str() == Some("base64") {
                        if let Some(base64_str) = data.as_str() {
                            return base64::engine::general_purpose::STANDARD
                                .decode(base64_str)
                                .map_err(|e| {
                                    ValkyrieError::MessageConversionFailed(format!(
                                        "Base64 decode failed: {}",
                                        e
                                    ))
                                });
                        }
                    }
                }
            }

            // Return JSON as bytes
            Ok(json_bytes.to_vec())
        } else {
            // Return raw bytes
            Ok(json_bytes.to_vec())
        }
    }
}

impl HttpAdapterBuilder {
    /// Create new HTTP adapter builder
    pub fn new() -> Self {
        Self {
            default_config: AdapterConfig {
                adapter_type: AdapterType::Http,
                connection: ConnectionConfig::default(),
                security: SecurityConfig::default(),
                performance: PerformanceConfig::default(),
                custom: {
                    let mut custom = HashMap::new();
                    custom.insert(
                        "base_url".to_string(),
                        serde_json::Value::String("http://localhost:8080".to_string()),
                    );
                    custom
                },
            },
        }
    }
}

#[async_trait]
impl AdapterBuilder for HttpAdapterBuilder {
    async fn build(&self, config: &AdapterConfig) -> Result<Box<dyn UniversalAdapter>> {
        let adapter = HttpAdapter::new(config.clone()).await?;
        Ok(Box::new(adapter))
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Http
    }

    fn validate_config(&self, config: &AdapterConfig) -> Result<()> {
        if config.adapter_type != AdapterType::Http {
            return Err(ValkyrieError::InvalidConfiguration(
                "Adapter type must be HTTP".to_string(),
            ));
        }

        // Validate base URL
        if let Some(url) = config.custom.get("base_url") {
            if url.as_str().is_none() {
                return Err(ValkyrieError::InvalidConfiguration(
                    "base_url must be a string".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn default_config(&self) -> AdapterConfig {
        self.default_config.clone()
    }
}

impl Default for AdapterMetrics {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            avg_latency: Duration::from_millis(50),
            p95_latency: Duration::from_millis(100),
            p99_latency: Duration::from_millis(200),
            error_rate: 0.0,
            throughput: 0.0,
            active_connections: 0,
            last_health_check: chrono::Utc::now(),
        }
    }
}
