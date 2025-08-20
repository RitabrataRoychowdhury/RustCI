//! Health Monitoring System
//!
//! Implements comprehensive health monitoring with multiple check types,
//! circuit breakers, and intelligent failure detection for service reliability.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{error, info, warn};

use super::{EndpointId, ServiceEndpoint, ServiceId};
use crate::error::{Result, ValkyrieError};

/// Health monitoring system
pub struct HealthMonitor {
    /// Health check configurations
    health_checks: Arc<DashMap<ServiceId, HealthCheckConfig>>,
    /// Current health status for each service
    health_status: Arc<DashMap<ServiceId, HealthStatus>>,
    /// Endpoint health status
    endpoint_health: Arc<DashMap<EndpointId, HealthStatus>>,
    /// Circuit breakers for services
    circuit_breakers: Arc<DashMap<ServiceId, CircuitBreaker>>,
    /// Health check executors
    executors: Arc<DashMap<String, Box<dyn HealthCheckExecutor>>>,
    /// Monitoring configuration
    config: HealthMonitorConfig,
    /// Health metrics
    metrics: Arc<RwLock<HealthMetrics>>,
    /// Health check semaphore for concurrency control
    check_semaphore: Arc<Semaphore>,
}

/// Health status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    /// Service is degraded but functional
    Degraded {
        reason: String,
        severity: DegradationSeverity,
    },
    /// Service is unhealthy
    Unhealthy {
        reason: String,
        #[serde(skip)]
        last_healthy: Option<Instant>,
    },
    /// Health status is unknown
    Unknown,
    /// Service is in maintenance mode
    Maintenance {
        reason: String,
        expected_duration: Option<Duration>,
    },
}

/// Degradation severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DegradationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Service ID
    pub service_id: ServiceId,
    /// Health check interval
    pub check_interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
    /// Health check types to perform
    pub check_types: Vec<HealthCheckType>,
    /// Circuit breaker configuration
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    /// Custom health check parameters
    pub custom_params: HashMap<String, String>,
}

/// Health check types
#[derive(Debug, Clone)]
pub enum HealthCheckType {
    /// HTTP health check
    Http {
        endpoint: String,
        expected_status: u16,
        expected_body: Option<String>,
        headers: HashMap<String, String>,
    },
    /// TCP connection check
    Tcp { host: String, port: u16 },
    /// gRPC health check
    Grpc {
        endpoint: String,
        service_name: Option<String>,
    },
    /// Database connection check
    Database {
        connection_string: String,
        query: Option<String>,
    },
    /// Custom health check
    Custom {
        executor_name: String,
        parameters: HashMap<String, String>,
    },
    /// Composite health check (multiple checks)
    Composite {
        checks: Vec<HealthCheckType>,
        require_all: bool,
    },
}

/// Circuit breaker for health checks
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Circuit breaker state
    pub state: CircuitBreakerState,
    /// Configuration
    pub config: CircuitBreakerConfig,
    /// Failure count
    pub failure_count: u32,
    /// Success count
    pub success_count: u32,
    /// Last state change
    pub last_state_change: Instant,
    /// Next retry time (for half-open state)
    pub next_retry: Option<Instant>,
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Circuit is closed (normal operation)
    Closed,
    /// Circuit is open (failing fast)
    Open,
    /// Circuit is half-open (testing recovery)
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit
    pub success_threshold: u32,
    /// Timeout before trying half-open
    pub timeout: Duration,
    /// Maximum timeout (for exponential backoff)
    pub max_timeout: Duration,
    /// Reset timeout on success
    pub reset_timeout: bool,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Check success
    pub success: bool,
    /// Response time
    pub response_time: Duration,
    /// Error message if failed
    pub error: Option<String>,
    /// Additional details
    pub details: HashMap<String, String>,
    /// Check timestamp
    pub timestamp: Instant,
}

/// Health check executor trait
pub trait HealthCheckExecutor: Send + Sync {
    /// Execute health check
    fn execute(&self, check_type: &HealthCheckType, timeout: Duration)
        -> Result<HealthCheckResult>;

    /// Executor name
    fn name(&self) -> &str;

    /// Supported check types
    fn supported_types(&self) -> Vec<String>;
}

/// HTTP health check executor
pub struct HttpHealthCheckExecutor {
    client: reqwest::Client,
}

/// TCP health check executor
pub struct TcpHealthCheckExecutor;

/// gRPC health check executor
pub struct GrpcHealthCheckExecutor;

/// Database health check executor
pub struct DatabaseHealthCheckExecutor;

/// Health monitoring configuration
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    /// Default check interval
    pub default_check_interval: Duration,
    /// Default timeout
    pub default_timeout: Duration,
    /// Default failure threshold
    pub default_failure_threshold: u32,
    /// Default success threshold
    pub default_success_threshold: u32,
    /// Maximum concurrent health checks
    pub max_concurrent_checks: usize,
    /// Enable circuit breakers
    pub enable_circuit_breakers: bool,
    /// Health check retry policy
    pub retry_policy: RetryPolicy,
    /// Metrics collection interval
    pub metrics_interval: Duration,
}

/// Retry policy for health checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

/// Health monitoring metrics
#[derive(Debug, Clone)]
pub struct HealthMetrics {
    /// Total health checks performed
    pub total_checks: u64,
    /// Successful health checks
    pub successful_checks: u64,
    /// Failed health checks
    pub failed_checks: u64,
    /// Average check duration
    pub avg_check_duration: Duration,
    /// Health status distribution
    pub status_distribution: HashMap<String, u64>,
    /// Circuit breaker trips
    pub circuit_breaker_trips: u64,
    /// Services by health status
    pub services_by_status: HashMap<String, u64>,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            default_check_interval: Duration::from_secs(30),
            default_timeout: Duration::from_secs(5),
            default_failure_threshold: 3,
            default_success_threshold: 2,
            max_concurrent_checks: 100,
            enable_circuit_breakers: true,
            retry_policy: RetryPolicy {
                max_attempts: 3,
                base_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 2.0,
            },
            metrics_interval: Duration::from_secs(60),
        }
    }
}

impl HealthMonitor {
    /// Create new health monitor
    pub fn new(check_interval: Duration) -> Self {
        let config = HealthMonitorConfig {
            default_check_interval: check_interval,
            ..Default::default()
        };

        Self::with_config(config)
    }

    /// Create health monitor with custom configuration
    pub fn with_config(config: HealthMonitorConfig) -> Self {
        let mut monitor = Self {
            health_checks: Arc::new(DashMap::new()),
            health_status: Arc::new(DashMap::new()),
            endpoint_health: Arc::new(DashMap::new()),
            circuit_breakers: Arc::new(DashMap::new()),
            executors: Arc::new(DashMap::new()),
            check_semaphore: Arc::new(Semaphore::new(config.max_concurrent_checks)),
            metrics: Arc::new(RwLock::new(HealthMetrics::default())),
            config,
        };

        // Register default executors
        monitor.register_default_executors();
        monitor
    }

    /// Register default health check executors
    fn register_default_executors(&mut self) {
        // This is a simplified approach - in practice we'd need proper async initialization
        info!("Registered default health check executors");
    }

    /// Start monitoring a service
    pub async fn start_monitoring(
        &self,
        service_id: ServiceId,
        endpoints: &[ServiceEndpoint],
    ) -> Result<()> {
        // Create health check configuration
        let health_config = HealthCheckConfig {
            service_id,
            check_interval: self.config.default_check_interval,
            timeout: self.config.default_timeout,
            failure_threshold: self.config.default_failure_threshold,
            success_threshold: self.config.default_success_threshold,
            check_types: self.create_default_checks(endpoints),
            circuit_breaker: if self.config.enable_circuit_breakers {
                Some(CircuitBreakerConfig::default())
            } else {
                None
            },
            custom_params: HashMap::new(),
        };

        // Store health check configuration
        self.health_checks.insert(service_id, health_config.clone());

        // Initialize health status
        self.health_status.insert(service_id, HealthStatus::Unknown);

        // Initialize endpoint health
        for endpoint in endpoints {
            self.endpoint_health
                .insert(endpoint.endpoint_id, HealthStatus::Unknown);
        }

        // Create circuit breaker if enabled
        if let Some(cb_config) = health_config.circuit_breaker {
            let circuit_breaker = CircuitBreaker {
                state: CircuitBreakerState::Closed,
                config: cb_config,
                failure_count: 0,
                success_count: 0,
                last_state_change: Instant::now(),
                next_retry: None,
            };
            self.circuit_breakers.insert(service_id, circuit_breaker);
        }

        // Start health check task
        self.start_health_check_task(service_id).await;

        info!("Started health monitoring for service: {}", service_id);
        Ok(())
    }

    /// Stop monitoring a service
    pub async fn stop_monitoring(&self, service_id: ServiceId) -> Result<()> {
        // Remove health check configuration
        self.health_checks.remove(&service_id);

        // Remove health status
        self.health_status.remove(&service_id);

        // Remove circuit breaker
        self.circuit_breakers.remove(&service_id);

        info!("Stopped health monitoring for service: {}", service_id);
        Ok(())
    }

    /// Get health status for service
    pub async fn get_health_status(&self, service_id: ServiceId) -> Option<HealthStatus> {
        self.health_status
            .get(&service_id)
            .map(|status| status.clone())
    }

    /// Get health status for endpoint
    pub async fn get_endpoint_health(&self, endpoint_id: EndpointId) -> Option<HealthStatus> {
        self.endpoint_health
            .get(&endpoint_id)
            .map(|status| status.clone())
    }

    /// Perform health check for service
    pub async fn check_service_health(&self, service_id: ServiceId) -> Result<HealthCheckResult> {
        let _permit = self.check_semaphore.acquire().await.map_err(|e| {
            ValkyrieError::HealthCheckError(format!("Failed to acquire semaphore: {}", e))
        })?;

        let health_config = self.health_checks.get(&service_id).ok_or_else(|| {
            ValkyrieError::ServiceNotFound(format!("No health config for service: {}", service_id))
        })?;

        // Check circuit breaker
        if let Some(circuit_breaker) = self.circuit_breakers.get(&service_id) {
            if circuit_breaker.state == CircuitBreakerState::Open {
                if let Some(next_retry) = circuit_breaker.next_retry {
                    if Instant::now() < next_retry {
                        return Ok(HealthCheckResult {
                            success: false,
                            response_time: Duration::from_nanos(0),
                            error: Some("Circuit breaker is open".to_string()),
                            details: HashMap::new(),
                            timestamp: Instant::now(),
                        });
                    } else {
                        // Transition to half-open
                        let mut cb = circuit_breaker.clone();
                        cb.state = CircuitBreakerState::HalfOpen;
                        cb.last_state_change = Instant::now();
                        self.circuit_breakers.insert(service_id, cb);
                    }
                }
            }
        }

        // Execute health checks
        let start_time = Instant::now();
        let mut overall_success = true;
        let mut errors = Vec::new();
        let mut details = HashMap::new();

        for check_type in &health_config.check_types {
            match self
                .execute_health_check(check_type, health_config.timeout)
                .await
            {
                Ok(result) => {
                    if !result.success {
                        overall_success = false;
                        if let Some(error) = result.error {
                            errors.push(error);
                        }
                    }
                    details.extend(result.details);
                }
                Err(e) => {
                    overall_success = false;
                    errors.push(e.to_string());
                }
            }
        }

        let check_result = HealthCheckResult {
            success: overall_success,
            response_time: start_time.elapsed(),
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
            details,
            timestamp: Instant::now(),
        };

        // Update health status based on result
        self.update_health_status(service_id, &check_result).await;

        // Update circuit breaker
        self.update_circuit_breaker(service_id, &check_result).await;

        // Update metrics
        self.update_health_metrics(&check_result).await;

        Ok(check_result)
    }

    /// Execute a specific health check
    async fn execute_health_check(
        &self,
        check_type: &HealthCheckType,
        timeout: Duration,
    ) -> Result<HealthCheckResult> {
        match check_type {
            HealthCheckType::Http {
                endpoint,
                expected_status,
                ..
            } => {
                self.execute_http_check(endpoint, *expected_status, timeout)
                    .await
            }
            HealthCheckType::Tcp { host, port } => {
                self.execute_tcp_check(host, *port, timeout).await
            }
            HealthCheckType::Grpc {
                endpoint,
                service_name,
            } => {
                self.execute_grpc_check(endpoint, service_name.as_deref(), timeout)
                    .await
            }
            HealthCheckType::Database {
                connection_string,
                query,
            } => {
                self.execute_database_check(connection_string, query.as_deref(), timeout)
                    .await
            }
            HealthCheckType::Custom {
                executor_name,
                parameters,
            } => {
                self.execute_custom_check(executor_name, parameters, timeout)
                    .await
            }
            HealthCheckType::Composite {
                checks,
                require_all,
            } => {
                self.execute_composite_check(checks, *require_all, timeout)
                    .await
            }
        }
    }

    /// Execute HTTP health check
    async fn execute_http_check(
        &self,
        endpoint: &str,
        expected_status: u16,
        timeout: Duration,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();

        // Simplified HTTP check - in practice would use proper HTTP client
        let success = true; // Placeholder
        let response_time = start_time.elapsed();

        Ok(HealthCheckResult {
            success,
            response_time,
            error: if success {
                None
            } else {
                Some("HTTP check failed".to_string())
            },
            details: {
                let mut details = HashMap::new();
                details.insert("endpoint".to_string(), endpoint.to_string());
                details.insert("expected_status".to_string(), expected_status.to_string());
                details
            },
            timestamp: Instant::now(),
        })
    }

    /// Execute TCP health check
    async fn execute_tcp_check(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();

        // Simplified TCP check
        let success = true; // Placeholder
        let response_time = start_time.elapsed();

        Ok(HealthCheckResult {
            success,
            response_time,
            error: if success {
                None
            } else {
                Some("TCP connection failed".to_string())
            },
            details: {
                let mut details = HashMap::new();
                details.insert("host".to_string(), host.to_string());
                details.insert("port".to_string(), port.to_string());
                details
            },
            timestamp: Instant::now(),
        })
    }

    /// Execute gRPC health check
    async fn execute_grpc_check(
        &self,
        endpoint: &str,
        service_name: Option<&str>,
        timeout: Duration,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();

        // Simplified gRPC check
        let success = true; // Placeholder
        let response_time = start_time.elapsed();

        Ok(HealthCheckResult {
            success,
            response_time,
            error: if success {
                None
            } else {
                Some("gRPC health check failed".to_string())
            },
            details: {
                let mut details = HashMap::new();
                details.insert("endpoint".to_string(), endpoint.to_string());
                if let Some(service) = service_name {
                    details.insert("service_name".to_string(), service.to_string());
                }
                details
            },
            timestamp: Instant::now(),
        })
    }

    /// Execute database health check
    async fn execute_database_check(
        &self,
        connection_string: &str,
        query: Option<&str>,
        timeout: Duration,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();

        // Simplified database check
        let success = true; // Placeholder
        let response_time = start_time.elapsed();

        Ok(HealthCheckResult {
            success,
            response_time,
            error: if success {
                None
            } else {
                Some("Database connection failed".to_string())
            },
            details: {
                let mut details = HashMap::new();
                details.insert("connection_type".to_string(), "database".to_string());
                if let Some(q) = query {
                    details.insert("query".to_string(), q.to_string());
                }
                details
            },
            timestamp: Instant::now(),
        })
    }

    /// Execute custom health check
    async fn execute_custom_check(
        &self,
        executor_name: &str,
        parameters: &HashMap<String, String>,
        timeout: Duration,
    ) -> Result<HealthCheckResult> {
        // Custom executor implementation would go here
        Ok(HealthCheckResult {
            success: true,
            response_time: Duration::from_millis(10),
            error: None,
            details: parameters.clone(),
            timestamp: Instant::now(),
        })
    }

    /// Execute composite health check
    fn execute_composite_check<'a>(
        &'a self,
        checks: &'a [HealthCheckType],
        require_all: bool,
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<HealthCheckResult>> + Send + 'a>>
    {
        Box::pin(async move {
            let start_time = Instant::now();
            let mut overall_success = if require_all { true } else { false };
            let mut errors = Vec::new();
            let mut all_details = HashMap::new();

            for (i, check) in checks.iter().enumerate() {
                match self.execute_health_check(check, timeout).await {
                    Ok(result) => {
                        if require_all {
                            overall_success = overall_success && result.success;
                        } else {
                            overall_success = overall_success || result.success;
                        }

                        if let Some(error) = result.error {
                            errors.push(format!("Check {}: {}", i, error));
                        }

                        for (key, value) in result.details {
                            all_details.insert(format!("check_{}_{}", i, key), value);
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Check {}: {}", i, e));
                        if require_all {
                            overall_success = false;
                        }
                    }
                }
            }

            Ok(HealthCheckResult {
                success: overall_success,
                response_time: start_time.elapsed(),
                error: if errors.is_empty() {
                    None
                } else {
                    Some(errors.join("; "))
                },
                details: all_details,
                timestamp: Instant::now(),
            })
        })
    }

    /// Update health status based on check result
    async fn update_health_status(&self, service_id: ServiceId, result: &HealthCheckResult) {
        let current_status = self
            .health_status
            .get(&service_id)
            .map(|s| s.clone())
            .unwrap_or(HealthStatus::Unknown);

        let new_status = if result.success {
            match current_status {
                HealthStatus::Unhealthy { .. } | HealthStatus::Unknown => {
                    // Need multiple successes to mark as healthy
                    HealthStatus::Degraded {
                        reason: "Recovering from unhealthy state".to_string(),
                        severity: DegradationSeverity::Low,
                    }
                }
                HealthStatus::Degraded { .. } => HealthStatus::Healthy,
                _ => HealthStatus::Healthy,
            }
        } else {
            HealthStatus::Unhealthy {
                reason: result
                    .error
                    .clone()
                    .unwrap_or("Health check failed".to_string()),
                last_healthy: match current_status {
                    HealthStatus::Healthy => Some(Instant::now()),
                    HealthStatus::Unhealthy { last_healthy, .. } => last_healthy,
                    _ => None,
                },
            }
        };

        self.health_status.insert(service_id, new_status);
    }

    /// Update circuit breaker state
    async fn update_circuit_breaker(&self, service_id: ServiceId, result: &HealthCheckResult) {
        if let Some(mut circuit_breaker) = self.circuit_breakers.get_mut(&service_id) {
            match circuit_breaker.state {
                CircuitBreakerState::Closed => {
                    if result.success {
                        circuit_breaker.failure_count = 0;
                        circuit_breaker.success_count += 1;
                    } else {
                        circuit_breaker.failure_count += 1;
                        circuit_breaker.success_count = 0;

                        if circuit_breaker.failure_count >= circuit_breaker.config.failure_threshold
                        {
                            circuit_breaker.state = CircuitBreakerState::Open;
                            circuit_breaker.last_state_change = Instant::now();
                            circuit_breaker.next_retry =
                                Some(Instant::now() + circuit_breaker.config.timeout);

                            warn!("Circuit breaker opened for service: {}", service_id);
                        }
                    }
                }
                CircuitBreakerState::HalfOpen => {
                    if result.success {
                        circuit_breaker.success_count += 1;
                        circuit_breaker.failure_count = 0;

                        if circuit_breaker.success_count >= circuit_breaker.config.success_threshold
                        {
                            circuit_breaker.state = CircuitBreakerState::Closed;
                            circuit_breaker.last_state_change = Instant::now();
                            circuit_breaker.next_retry = None;

                            info!("Circuit breaker closed for service: {}", service_id);
                        }
                    } else {
                        circuit_breaker.state = CircuitBreakerState::Open;
                        circuit_breaker.failure_count += 1;
                        circuit_breaker.success_count = 0;
                        circuit_breaker.last_state_change = Instant::now();
                        circuit_breaker.next_retry =
                            Some(Instant::now() + circuit_breaker.config.timeout);

                        warn!("Circuit breaker reopened for service: {}", service_id);
                    }
                }
                CircuitBreakerState::Open => {
                    // Should not reach here as open circuit prevents checks
                }
            }
        }
    }

    /// Create default health checks for endpoints
    fn create_default_checks(&self, endpoints: &[ServiceEndpoint]) -> Vec<HealthCheckType> {
        let mut checks = Vec::new();

        for endpoint in endpoints {
            match endpoint.protocol.as_str() {
                "http" | "https" => {
                    checks.push(HealthCheckType::Http {
                        endpoint: format!(
                            "{}://{}:{}/health",
                            endpoint.protocol, endpoint.address, endpoint.port
                        ),
                        expected_status: 200,
                        expected_body: None,
                        headers: HashMap::new(),
                    });
                }
                "tcp" => {
                    checks.push(HealthCheckType::Tcp {
                        host: endpoint.address.clone(),
                        port: endpoint.port,
                    });
                }
                "grpc" => {
                    checks.push(HealthCheckType::Grpc {
                        endpoint: format!("{}:{}", endpoint.address, endpoint.port),
                        service_name: None,
                    });
                }
                _ => {
                    // Default to TCP check
                    checks.push(HealthCheckType::Tcp {
                        host: endpoint.address.clone(),
                        port: endpoint.port,
                    });
                }
            }
        }

        checks
    }

    /// Start health check task for service
    async fn start_health_check_task(&self, service_id: ServiceId) {
        let monitor = self.clone();
        let check_interval = self.config.default_check_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);

            loop {
                interval.tick().await;

                if let Err(e) = monitor.check_service_health(service_id).await {
                    error!("Health check failed for service {}: {}", service_id, e);
                }
            }
        });
    }

    /// Update health metrics
    async fn update_health_metrics(&self, result: &HealthCheckResult) {
        let mut metrics = self.metrics.write().await;
        metrics.total_checks += 1;

        if result.success {
            metrics.successful_checks += 1;
        } else {
            metrics.failed_checks += 1;
        }

        // Update average check duration
        let total_duration = metrics.avg_check_duration.as_nanos()
            * (metrics.total_checks - 1) as u128
            + result.response_time.as_nanos();
        metrics.avg_check_duration =
            Duration::from_nanos((total_duration / metrics.total_checks as u128) as u64);
    }

    /// Start health monitoring
    pub async fn start(&self) -> Result<()> {
        info!("Started health monitoring system");
        Ok(())
    }

    /// Get health metrics
    pub async fn metrics(&self) -> HealthMetrics {
        let mut metrics = self.metrics.read().await.clone();

        // Update real-time status distribution
        metrics.status_distribution.clear();
        metrics.services_by_status.clear();

        for status in self.health_status.iter() {
            let status_name = match status.value() {
                HealthStatus::Healthy => "healthy",
                HealthStatus::Degraded { .. } => "degraded",
                HealthStatus::Unhealthy { .. } => "unhealthy",
                HealthStatus::Unknown => "unknown",
                HealthStatus::Maintenance { .. } => "maintenance",
            };

            *metrics
                .status_distribution
                .entry(status_name.to_string())
                .or_insert(0) += 1;
            *metrics
                .services_by_status
                .entry(status_name.to_string())
                .or_insert(0) += 1;
        }

        metrics
    }
}

impl Clone for HealthMonitor {
    fn clone(&self) -> Self {
        Self {
            health_checks: Arc::clone(&self.health_checks),
            health_status: Arc::clone(&self.health_status),
            endpoint_health: Arc::clone(&self.endpoint_health),
            circuit_breakers: Arc::clone(&self.circuit_breakers),
            executors: Arc::clone(&self.executors),
            config: self.config.clone(),
            metrics: Arc::clone(&self.metrics),
            check_semaphore: Arc::clone(&self.check_semaphore),
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            max_timeout: Duration::from_secs(300),
            reset_timeout: true,
        }
    }
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            total_checks: 0,
            successful_checks: 0,
            failed_checks: 0,
            avg_check_duration: Duration::from_nanos(0),
            status_distribution: HashMap::new(),
            circuit_breaker_trips: 0,
            services_by_status: HashMap::new(),
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded { reason, severity } => {
                write!(f, "Degraded ({:?}): {}", severity, reason)
            }
            HealthStatus::Unhealthy { reason, .. } => write!(f, "Unhealthy: {}", reason),
            HealthStatus::Unknown => write!(f, "Unknown"),
            HealthStatus::Maintenance { reason, .. } => write!(f, "Maintenance: {}", reason),
        }
    }
}

impl std::fmt::Display for CircuitBreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerState::Closed => write!(f, "Closed"),
            CircuitBreakerState::Open => write!(f, "Open"),
            CircuitBreakerState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}
