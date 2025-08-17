// HTTP Fallback System for Valkyrie Runner Adapter
// Part of Task 3.1: High-Performance Runner Adapter

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use reqwest::{Client, Method, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::valkyrie_adapter::{
    ValkyrieJob, JobId, RunnerId, ValkyrieAdapterConfig, ValkyrieAdapterError,
    JobStatus, JobResult, ResourceUsage, JobPerformanceMetrics,
};
use super::selection::RunnerSelection;

/// HTTP fallback system for when Valkyrie is unavailable
pub struct HttpFallbackSystem {
    // HTTP client for runner communication
    http_client: Client,
    
    // Runner endpoint registry
    runner_endpoints: Arc<RwLock<HashMap<RunnerId, HttpRunnerEndpoint>>>,
    
    // Fallback configuration
    config: HttpFallbackConfig,
    
    // Circuit breaker for failing runners
    circuit_breakers: Arc<RwLock<HashMap<RunnerId, CircuitBreaker>>>,
    
    // Metrics tracking
    metrics: Arc<HttpFallbackMetrics>,
    
    // Request/response tracking
    active_requests: Arc<RwLock<HashMap<JobId, HttpJobRequest>>>,
}

/// Configuration for HTTP fallback system
#[derive(Debug, Clone)]
pub struct HttpFallbackConfig {
    pub request_timeout: Duration,
    pub connection_timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout: Duration,
    pub max_concurrent_requests: u32,
    pub enable_compression: bool,
    pub enable_keep_alive: bool,
    pub user_agent: String,
}

impl Default for HttpFallbackConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            connection_timeout: Duration::from_secs(10),
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(60),
            max_concurrent_requests: 100,
            enable_compression: true,
            enable_keep_alive: true,
            user_agent: "RustCI-Valkyrie-Fallback/1.0".to_string(),
        }
    }
}

/// HTTP runner endpoint information
#[derive(Debug, Clone)]
pub struct HttpRunnerEndpoint {
    pub runner_id: RunnerId,
    pub base_url: String,
    pub api_version: String,
    pub authentication: Option<HttpAuthentication>,
    pub capabilities: HttpRunnerCapabilities,
    pub last_seen: Instant,
    pub response_time_history: Vec<Duration>,
}

/// HTTP authentication methods
#[derive(Debug, Clone)]
pub enum HttpAuthentication {
    Bearer(String),
    Basic { username: String, password: String },
    ApiKey { header: String, value: String },
    Custom(HashMap<String, String>),
}

/// HTTP runner capabilities
#[derive(Debug, Clone, Default)]
pub struct HttpRunnerCapabilities {
    pub supports_streaming: bool,
    pub supports_websockets: bool,
    pub supports_chunked_upload: bool,
    pub max_payload_size: Option<u64>,
    pub supported_content_types: Vec<String>,
}

/// Circuit breaker for failing runners
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub runner_id: RunnerId,
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub last_failure: Option<Instant>,
    pub last_success: Option<Instant>,
    pub next_attempt: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing, requests blocked
    HalfOpen, // Testing if service recovered
}

impl CircuitBreaker {
    pub fn new(runner_id: RunnerId) -> Self {
        Self {
            runner_id,
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            last_failure: None,
            last_success: None,
            next_attempt: None,
        }
    }
    
    pub fn can_attempt(&self, config: &HttpFallbackConfig) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(next_attempt) = self.next_attempt {
                    Instant::now() >= next_attempt
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }
    
    pub fn record_success(&mut self) {
        self.state = CircuitBreakerState::Closed;
        self.failure_count = 0;
        self.last_success = Some(Instant::now());
        self.next_attempt = None;
    }
    
    pub fn record_failure(&mut self, config: &HttpFallbackConfig) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());
        
        if self.failure_count >= config.circuit_breaker_threshold {
            self.state = CircuitBreakerState::Open;
            self.next_attempt = Some(Instant::now() + config.circuit_breaker_timeout);
        }
    }
    
    pub fn transition_to_half_open(&mut self) {
        if self.state == CircuitBreakerState::Open {
            self.state = CircuitBreakerState::HalfOpen;
        }
    }
}

/// HTTP job request tracking
#[derive(Debug, Clone)]
pub struct HttpJobRequest {
    pub job_id: JobId,
    pub runner_id: RunnerId,
    pub started_at: Instant,
    pub request_url: String,
    pub retry_count: u32,
    pub last_response_status: Option<u16>,
}

/// HTTP fallback metrics
#[derive(Debug, Default)]
pub struct HttpFallbackMetrics {
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub timeout_requests: AtomicU64,
    pub circuit_breaker_trips: AtomicU64,
    pub average_response_time: AtomicU64, // microseconds
    pub active_requests: AtomicU32,
    pub fallback_usage_rate: AtomicU64, // per 1000 requests
}

/// HTTP API request/response types
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpJobSubmissionRequest {
    pub job_id: String,
    pub job_type: String,
    pub priority: u8,
    pub payload: Value,
    pub requirements: HttpJobRequirements,
    pub metadata: HashMap<String, String>,
    pub timeout: Option<u64>, // seconds
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpJobRequirements {
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u32>,
    pub storage_gb: Option<u32>,
    pub gpu_count: Option<u32>,
    pub max_execution_time: Option<u64>, // seconds
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpJobSubmissionResponse {
    pub job_id: String,
    pub status: String,
    pub estimated_start_time: Option<String>,
    pub estimated_completion_time: Option<String>,
    pub runner_info: Option<HttpRunnerInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpRunnerInfo {
    pub runner_id: String,
    pub endpoint: String,
    pub capabilities: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpJobStatusResponse {
    pub job_id: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub result: Option<HttpJobResult>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpJobResult {
    pub exit_code: i32,
    pub output: String,
    pub artifacts: Vec<String>,
    pub execution_time: u64, // seconds
    pub resource_usage: HttpResourceUsage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpResourceUsage {
    pub cpu_time: u64, // seconds
    pub memory_peak: u64, // bytes
    pub storage_used: u64, // bytes
    pub network_bytes: u64,
}

/// Dispatch result from HTTP fallback
#[derive(Debug, Clone)]
pub struct HttpDispatchResult {
    pub success: bool,
    pub latency: Duration,
    pub response_status: Option<u16>,
    pub response_size: Option<u64>,
    pub retry_count: u32,
}

impl HttpFallbackSystem {
    /// Create a new HTTP fallback system
    pub fn new(config: HttpFallbackConfig) -> Self {
        let mut client_builder = Client::builder()
            .timeout(config.request_timeout)
            .connect_timeout(config.connection_timeout)
            .user_agent(&config.user_agent);
            
        // Add compression if available
        #[cfg(feature = "gzip")]
        {
            client_builder = client_builder.gzip(config.enable_compression);
        }
        
        // Add keep-alive if enabled
        if config.enable_keep_alive {
            client_builder = client_builder.tcp_keepalive(Some(Duration::from_secs(60)));
        }
        
        let http_client = client_builder
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            http_client,
            runner_endpoints: Arc::new(RwLock::new(HashMap::new())),
            config,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(HttpFallbackMetrics::default()),
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Dispatch a job via HTTP fallback
    pub async fn dispatch_job(
        &self,
        job: &ValkyrieJob,
        runner_selection: &RunnerSelection,
    ) -> Result<HttpDispatchResult, ValkyrieAdapterError> {
        let start_time = Instant::now();
        
        // Check circuit breaker
        if !self.can_attempt_request(runner_selection.runner_id).await {
            return Err(ValkyrieAdapterError::HttpFallbackFailed(
                "Circuit breaker open".to_string()
            ));
        }
        
        // Get runner endpoint
        let endpoint = self.get_runner_endpoint(runner_selection.runner_id).await?;
        
        // Create HTTP request
        let request = self.create_job_submission_request(job).await?;
        
        // Track active request
        let job_request = HttpJobRequest {
            job_id: job.id,
            runner_id: runner_selection.runner_id,
            started_at: start_time,
            request_url: format!("{}/jobs", endpoint.base_url),
            retry_count: 0,
            last_response_status: None,
        };
        
        {
            let mut active_requests = self.active_requests.write().await;
            active_requests.insert(job.id, job_request.clone());
        }
        
        // Attempt request with retries
        let result = self.attempt_request_with_retries(job_request, request).await;
        
        // Remove from active requests
        {
            let mut active_requests = self.active_requests.write().await;
            active_requests.remove(&job.id);
        }
        
        // Update circuit breaker
        match &result {
            Ok(_) => self.record_success(runner_selection.runner_id).await,
            Err(_) => self.record_failure(runner_selection.runner_id).await,
        }
        
        // Update metrics
        let latency = start_time.elapsed();
        self.update_metrics(&result, latency).await;
        
        result
    }
    
    /// Check if we can attempt a request to a runner
    async fn can_attempt_request(&self, runner_id: RunnerId) -> bool {
        let circuit_breakers = self.circuit_breakers.read().await;
        
        if let Some(circuit_breaker) = circuit_breakers.get(&runner_id) {
            circuit_breaker.can_attempt(&self.config)
        } else {
            true // No circuit breaker means we can attempt
        }
    }
    
    /// Get runner endpoint information
    async fn get_runner_endpoint(
        &self,
        runner_id: RunnerId,
    ) -> Result<HttpRunnerEndpoint, ValkyrieAdapterError> {
        let endpoints = self.runner_endpoints.read().await;
        
        endpoints.get(&runner_id)
            .cloned()
            .ok_or_else(|| ValkyrieAdapterError::RunnerNotFound { runner_id })
    }
    
    /// Create HTTP job submission request
    async fn create_job_submission_request(
        &self,
        job: &ValkyrieJob,
    ) -> Result<HttpJobSubmissionRequest, ValkyrieAdapterError> {
        let payload = serde_json::to_value(&job.payload)
            .map_err(ValkyrieAdapterError::Serialization)?;
        
        Ok(HttpJobSubmissionRequest {
            job_id: job.id.to_string(),
            job_type: format!("{:?}", job.job_type),
            priority: job.priority as u8,
            payload,
            requirements: HttpJobRequirements {
                cpu_cores: job.requirements.cpu_cores,
                memory_mb: job.requirements.memory_mb,
                storage_gb: job.requirements.storage_gb,
                gpu_count: job.requirements.gpu_count,
                max_execution_time: job.requirements.max_execution_time
                    .map(|d| d.as_secs()),
                required_capabilities: job.requirements.required_capabilities.clone(),
            },
            metadata: job.metadata.tags.clone(),
            timeout: job.deadline.map(|d| d.duration_since(SystemTime::now()).unwrap_or_default().as_secs()),
        })
    }
    
    /// Attempt request with retries
    async fn attempt_request_with_retries(
        &self,
        mut job_request: HttpJobRequest,
        request: HttpJobSubmissionRequest,
    ) -> Result<HttpDispatchResult, ValkyrieAdapterError> {
        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            job_request.retry_count = attempt;
            
            match self.send_http_request(&job_request, &request).await {
                Ok(result) => {
                    return Ok(HttpDispatchResult {
                        success: true,
                        latency: job_request.started_at.elapsed(),
                        response_status: Some(result.status),
                        response_size: Some(result.content_length),
                        retry_count: attempt,
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    // Don't retry on certain errors
                    if let Some(ValkyrieAdapterError::HttpFallbackFailed(msg)) = &last_error {
                        if msg.contains("404") || msg.contains("401") || msg.contains("403") {
                            break;
                        }
                    }
                    
                    // Wait before retry (except on last attempt)
                    if attempt < self.config.max_retries {
                        tokio::time::sleep(self.config.retry_delay * (attempt + 1)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            ValkyrieAdapterError::HttpFallbackFailed("Max retries exceeded".to_string())
        }))
    }
    
    /// Send HTTP request to runner
    async fn send_http_request(
        &self,
        job_request: &HttpJobRequest,
        request: &HttpJobSubmissionRequest,
    ) -> Result<HttpResponseInfo, ValkyrieAdapterError> {
        let endpoint = self.get_runner_endpoint(job_request.runner_id).await?;
        
        let mut http_request = self.http_client
            .request(Method::POST, &job_request.request_url)
            .json(request);
        
        // Add authentication if configured
        if let Some(auth) = &endpoint.authentication {
            http_request = self.add_authentication(http_request, auth);
        }
        
        // Send request
        let response = http_request
            .send()
            .await
            .map_err(|e| ValkyrieAdapterError::HttpFallbackFailed(e.to_string()))?;
        
        let status = response.status().as_u16();
        let content_length = response.content_length().unwrap_or(0);
        
        if response.status().is_success() {
            let _response_body: HttpJobSubmissionResponse = response
                .json()
                .await
                .map_err(|e| ValkyrieAdapterError::HttpFallbackFailed(e.to_string()))?;
            
            Ok(HttpResponseInfo {
                status,
                content_length,
            })
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            
            Err(ValkyrieAdapterError::HttpFallbackFailed(
                format!("HTTP {} - {}", status, error_text)
            ))
        }
    }
    
    /// Add authentication to HTTP request
    fn add_authentication(
        &self,
        mut request: reqwest::RequestBuilder,
        auth: &HttpAuthentication,
    ) -> reqwest::RequestBuilder {
        match auth {
            HttpAuthentication::Bearer(token) => {
                request = request.bearer_auth(token);
            }
            HttpAuthentication::Basic { username, password } => {
                request = request.basic_auth(username, Some(password));
            }
            HttpAuthentication::ApiKey { header, value } => {
                request = request.header(header, value);
            }
            HttpAuthentication::Custom(headers) => {
                for (key, value) in headers {
                    request = request.header(key, value);
                }
            }
        }
        request
    }
    
    /// Record successful request
    async fn record_success(&self, runner_id: RunnerId) {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        
        if let Some(circuit_breaker) = circuit_breakers.get_mut(&runner_id) {
            circuit_breaker.record_success();
        }
    }
    
    /// Record failed request
    async fn record_failure(&self, runner_id: RunnerId) {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        
        let circuit_breaker = circuit_breakers
            .entry(runner_id)
            .or_insert_with(|| CircuitBreaker::new(runner_id));
        
        circuit_breaker.record_failure(&self.config);
        
        if circuit_breaker.state == CircuitBreakerState::Open {
            self.metrics.circuit_breaker_trips.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Update metrics
    async fn update_metrics(
        &self,
        result: &Result<HttpDispatchResult, ValkyrieAdapterError>,
        latency: Duration,
    ) {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
        
        match result {
            Ok(_) => {
                self.metrics.successful_requests.fetch_add(1, Ordering::Relaxed);
            }
            Err(ValkyrieAdapterError::DispatchTimeout) => {
                self.metrics.timeout_requests.fetch_add(1, Ordering::Relaxed);
                self.metrics.failed_requests.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                self.metrics.failed_requests.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        // Update average response time
        let latency_micros = latency.as_micros() as u64;
        let current_avg = self.metrics.average_response_time.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            latency_micros
        } else {
            (current_avg * 9 + latency_micros) / 10
        };
        self.metrics.average_response_time.store(new_avg, Ordering::Relaxed);
    }
    
    /// Register HTTP runner endpoint
    pub async fn register_runner_endpoint(
        &self,
        runner_id: RunnerId,
        base_url: String,
        api_version: String,
        authentication: Option<HttpAuthentication>,
        capabilities: HttpRunnerCapabilities,
    ) -> Result<(), ValkyrieAdapterError> {
        let endpoint = HttpRunnerEndpoint {
            runner_id,
            base_url,
            api_version,
            authentication,
            capabilities,
            last_seen: Instant::now(),
            response_time_history: Vec::new(),
        };
        
        let mut endpoints = self.runner_endpoints.write().await;
        endpoints.insert(runner_id, endpoint);
        
        Ok(())
    }
    
    /// Get job status via HTTP
    pub async fn get_job_status(
        &self,
        job_id: JobId,
        runner_id: RunnerId,
    ) -> Result<JobStatus, ValkyrieAdapterError> {
        let endpoint = self.get_runner_endpoint(runner_id).await?;
        
        let mut request = self.http_client
            .get(&format!("{}/jobs/{}", endpoint.base_url, job_id));
        
        if let Some(auth) = &endpoint.authentication {
            request = self.add_authentication(request, auth);
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| ValkyrieAdapterError::HttpFallbackFailed(e.to_string()))?;
        
        if response.status().is_success() {
            let status_response: HttpJobStatusResponse = response
                .json()
                .await
                .map_err(|e| ValkyrieAdapterError::HttpFallbackFailed(e.to_string()))?;
            
            self.convert_http_status_to_job_status(status_response)
        } else {
            Err(ValkyrieAdapterError::HttpFallbackFailed(
                format!("Failed to get job status: {}", response.status())
            ))
        }
    }
    
    /// Convert HTTP status response to JobStatus
    fn convert_http_status_to_job_status(
        &self,
        response: HttpJobStatusResponse,
    ) -> Result<JobStatus, ValkyrieAdapterError> {
        let job_id = Uuid::parse_str(&response.job_id)
            .map_err(|e| ValkyrieAdapterError::Internal(e.to_string()))?;
        
        match response.status.as_str() {
            "queued" => Ok(JobStatus::Queued { queued_at: Instant::now() }),
            "running" => {
                let runner_id = Uuid::new_v4(); // Would need to parse from response
                Ok(JobStatus::Running {
                    runner_id,
                    started_at: Instant::now(),
                })
            }
            "completed" => {
                if let Some(http_result) = response.result {
                    let result = JobResult {
                        exit_code: http_result.exit_code,
                        output: http_result.output,
                        artifacts: http_result.artifacts,
                        execution_time: Duration::from_secs(http_result.execution_time),
                        resource_usage: ResourceUsage {
                            cpu_time: Duration::from_secs(http_result.resource_usage.cpu_time),
                            memory_peak: http_result.resource_usage.memory_peak,
                            storage_used: http_result.resource_usage.storage_used,
                            network_bytes_sent: http_result.resource_usage.network_bytes / 2,
                            network_bytes_received: http_result.resource_usage.network_bytes / 2,
                        },
                        performance_metrics: JobPerformanceMetrics::default(),
                    };
                    Ok(JobStatus::Completed {
                        result,
                        completed_at: Instant::now(),
                    })
                } else {
                    Err(ValkyrieAdapterError::Internal(
                        "Completed job missing result".to_string()
                    ))
                }
            }
            "failed" => Ok(JobStatus::Failed {
                error: response.error.unwrap_or_else(|| "Unknown error".to_string()),
                failed_at: Instant::now(),
            }),
            "cancelled" => Ok(JobStatus::Cancelled {
                cancelled_at: Instant::now(),
            }),
            _ => Err(ValkyrieAdapterError::Internal(
                format!("Unknown job status: {}", response.status)
            )),
        }
    }
    
    /// Get fallback metrics
    pub fn get_metrics(&self) -> HttpFallbackMetricsSnapshot {
        HttpFallbackMetricsSnapshot {
            total_requests: self.metrics.total_requests.load(Ordering::Relaxed),
            successful_requests: self.metrics.successful_requests.load(Ordering::Relaxed),
            failed_requests: self.metrics.failed_requests.load(Ordering::Relaxed),
            timeout_requests: self.metrics.timeout_requests.load(Ordering::Relaxed),
            circuit_breaker_trips: self.metrics.circuit_breaker_trips.load(Ordering::Relaxed),
            average_response_time: Duration::from_micros(
                self.metrics.average_response_time.load(Ordering::Relaxed)
            ),
            active_requests: self.metrics.active_requests.load(Ordering::Relaxed),
            success_rate: {
                let total = self.metrics.total_requests.load(Ordering::Relaxed);
                let successful = self.metrics.successful_requests.load(Ordering::Relaxed);
                if total > 0 {
                    successful as f64 / total as f64
                } else {
                    0.0
                }
            },
        }
    }
}

/// HTTP response information
#[derive(Debug, Clone)]
struct HttpResponseInfo {
    pub status: u16,
    pub content_length: u64,
}

/// HTTP fallback metrics snapshot
#[derive(Debug, Clone)]
pub struct HttpFallbackMetricsSnapshot {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub timeout_requests: u64,
    pub circuit_breaker_trips: u64,
    pub average_response_time: Duration,
    pub active_requests: u32,
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_circuit_breaker_states() {
        let config = HttpFallbackConfig::default();
        let mut circuit_breaker = CircuitBreaker::new(Uuid::new_v4());
        
        // Initially closed
        assert_eq!(circuit_breaker.state, CircuitBreakerState::Closed);
        assert!(circuit_breaker.can_attempt(&config));
        
        // Record failures
        for _ in 0..config.circuit_breaker_threshold {
            circuit_breaker.record_failure(&config);
        }
        
        // Should be open now
        assert_eq!(circuit_breaker.state, CircuitBreakerState::Open);
        assert!(!circuit_breaker.can_attempt(&config));
        
        // Record success should close it
        circuit_breaker.record_success();
        assert_eq!(circuit_breaker.state, CircuitBreakerState::Closed);
        assert!(circuit_breaker.can_attempt(&config));
    }
    
    #[tokio::test]
    async fn test_http_fallback_system_creation() {
        let config = HttpFallbackConfig::default();
        let fallback_system = HttpFallbackSystem::new(config);
        
        // Should be able to create without errors
        assert_eq!(fallback_system.runner_endpoints.read().await.len(), 0);
        assert_eq!(fallback_system.circuit_breakers.read().await.len(), 0);
    }
}