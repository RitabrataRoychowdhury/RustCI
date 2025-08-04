use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::core::infrastructure::service_decorators::ServiceContext;
use crate::error::{AppError, Result};

/// Bulkhead pattern for resource isolation
pub struct BulkheadManager {
    pools: Arc<RwLock<HashMap<String, Arc<ResourcePool>>>>,
}

impl BulkheadManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_pool(&self, name: String, config: BulkheadConfig) -> Result<()> {
        let pool = Arc::new(ResourcePool::new(config));
        let mut pools = self.pools.write().await;
        pools.insert(name, pool);
        Ok(())
    }

    pub async fn get_pool(&self, name: &str) -> Result<Arc<ResourcePool>> {
        let pools = self.pools.read().await;
        pools
            .get(name)
            .cloned()
            .ok_or_else(|| AppError::DependencyNotFound {
                service: format!("Bulkhead pool not found: {}", name),
            })
    }

    pub async fn execute_in_pool<F, T, Fut>(
        &self,
        pool_name: &str,
        context: &ServiceContext,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let pool = self.get_pool(pool_name).await?;
        pool.execute(context, operation).await
    }

    pub async fn get_pool_stats(&self, pool_name: &str) -> Result<BulkheadStats> {
        let pool = self.get_pool(pool_name).await?;
        Ok(pool.get_stats().await)
    }

    pub async fn list_pools(&self) -> Vec<String> {
        let pools = self.pools.read().await;
        pools.keys().cloned().collect()
    }
}

impl Default for BulkheadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Bulkhead configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkheadConfig {
    pub max_concurrent_requests: usize,
    pub queue_size: usize,
    pub timeout: Duration,
    pub rejection_threshold: f64, // 0.0 to 1.0
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            queue_size: 100,
            timeout: Duration::from_secs(30),
            rejection_threshold: 0.8,
        }
    }
}

/// Resource pool for bulkhead pattern
pub struct ResourcePool {
    config: BulkheadConfig,
    semaphore: Arc<Semaphore>,
    stats: Arc<RwLock<BulkheadStats>>,
}

impl ResourcePool {
    pub fn new(config: BulkheadConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));
        let stats = Arc::new(RwLock::new(BulkheadStats::default()));

        Self {
            config,
            semaphore,
            stats,
        }
    }

    pub async fn execute<F, T, Fut>(&self, context: &ServiceContext, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if we should reject the request based on current load
        let current_stats = self.get_stats().await;
        let utilization =
            current_stats.active_requests as f64 / self.config.max_concurrent_requests as f64;

        if utilization >= self.config.rejection_threshold {
            self.increment_rejected().await;
            warn!(
                correlation_id = %context.correlation_id,
                operation = %context.operation,
                utilization = utilization,
                threshold = self.config.rejection_threshold,
                "Request rejected due to high utilization"
            );
            return Err(AppError::ExternalServiceError(
                "Resource pool at capacity, request rejected".to_string(),
            ));
        }

        // Try to acquire permit with timeout
        let permit = tokio::time::timeout(self.config.timeout, self.semaphore.acquire())
            .await
            .map_err(|_| {
                AppError::ExternalServiceError(format!(
                    "Timeout waiting for resource pool permit after {:?}",
                    self.config.timeout
                ))
            })?
            .map_err(|_| AppError::InternalServerError("Semaphore closed".to_string()))?;

        self.increment_active().await;
        let start_time = Instant::now();

        let result = operation().await;
        let duration = start_time.elapsed();

        self.decrement_active().await;

        match &result {
            Ok(_) => {
                self.increment_completed(duration).await;
                debug!(
                    correlation_id = %context.correlation_id,
                    operation = %context.operation,
                    duration_ms = duration.as_millis(),
                    "Operation completed successfully in resource pool"
                );
            }
            Err(error) => {
                self.increment_failed(duration).await;
                warn!(
                    correlation_id = %context.correlation_id,
                    operation = %context.operation,
                    duration_ms = duration.as_millis(),
                    error = %error,
                    "Operation failed in resource pool"
                );
            }
        }

        drop(permit);
        result
    }

    pub async fn get_stats(&self) -> BulkheadStats {
        *self.stats.read().await
    }

    async fn increment_active(&self) {
        let mut stats = self.stats.write().await;
        stats.active_requests += 1;
        stats.total_requests += 1;
    }

    async fn decrement_active(&self) {
        let mut stats = self.stats.write().await;
        stats.active_requests = stats.active_requests.saturating_sub(1);
    }

    async fn increment_completed(&self, duration: Duration) {
        let mut stats = self.stats.write().await;
        stats.completed_requests += 1;
        stats.total_duration += duration;
        stats.update_average_duration();
    }

    async fn increment_failed(&self, duration: Duration) {
        let mut stats = self.stats.write().await;
        stats.failed_requests += 1;
        stats.total_duration += duration;
        stats.update_average_duration();
    }

    async fn increment_rejected(&self) {
        let mut stats = self.stats.write().await;
        stats.rejected_requests += 1;
    }
}

/// Bulkhead statistics
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct BulkheadStats {
    pub active_requests: usize,
    pub total_requests: usize,
    pub completed_requests: usize,
    pub failed_requests: usize,
    pub rejected_requests: usize,
    pub total_duration: Duration,
    pub average_duration: Duration,
}

impl BulkheadStats {
    fn update_average_duration(&mut self) {
        let total_processed = self.completed_requests + self.failed_requests;
        if total_processed > 0 {
            self.average_duration = self.total_duration / total_processed as u32;
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total_processed = self.completed_requests + self.failed_requests;
        if total_processed == 0 {
            0.0
        } else {
            self.completed_requests as f64 / total_processed as f64
        }
    }

    pub fn rejection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.rejected_requests as f64 / self.total_requests as f64
        }
    }
}

/// Backoff strategy for retry operations
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Fixed(Duration),
    Linear {
        base: Duration,
        increment: Duration,
    },
    Exponential {
        base: Duration,
        multiplier: f64,
        max: Duration,
    },
    ExponentialWithJitter {
        base: Duration,
        multiplier: f64,
        max: Duration,
        jitter_factor: f64,
    },
}

impl BackoffStrategy {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        match self {
            BackoffStrategy::Fixed(duration) => *duration,
            BackoffStrategy::Linear { base, increment } => *base + *increment * attempt,
            BackoffStrategy::Exponential {
                base,
                multiplier,
                max,
            } => {
                let delay = base.as_millis() as f64 * multiplier.powi(attempt as i32);
                Duration::from_millis((delay as u64).min(max.as_millis() as u64))
            }
            BackoffStrategy::ExponentialWithJitter {
                base,
                multiplier,
                max,
                jitter_factor,
            } => {
                let base_delay = base.as_millis() as f64 * multiplier.powi(attempt as i32);
                let jitter = fastrand::f64() * jitter_factor;
                let delay = base_delay * (1.0 + jitter);
                Duration::from_millis((delay as u64).min(max.as_millis() as u64))
            }
        }
    }
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::ExponentialWithJitter {
            base: Duration::from_millis(100),
            multiplier: 2.0,
            max: Duration::from_secs(30),
            jitter_factor: 0.1,
        }
    }
}

/// Advanced retry manager with different backoff strategies
pub struct RetryManager {
    strategy: BackoffStrategy,
    max_attempts: u32,
    retry_conditions: Vec<Box<dyn RetryCondition>>,
}

impl RetryManager {
    pub fn new(strategy: BackoffStrategy, max_attempts: u32) -> Self {
        Self {
            strategy,
            max_attempts,
            retry_conditions: Vec::new(),
        }
    }

    pub fn with_condition(mut self, condition: Box<dyn RetryCondition>) -> Self {
        self.retry_conditions.push(condition);
        self
    }

    pub fn with_transient_errors(self) -> Self {
        self.with_condition(Box::new(TransientErrorCondition))
    }

    pub fn with_timeout_errors(self) -> Self {
        self.with_condition(Box::new(TimeoutErrorCondition))
    }

    pub async fn execute_with_retry<F, T, Fut>(
        &self,
        context: &ServiceContext,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..self.max_attempts {
            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!(
                            correlation_id = %context.correlation_id,
                            operation = %context.operation,
                            attempt = attempt + 1,
                            "Operation succeeded after retry"
                        );
                    }
                    return Ok(result);
                }
                Err(error) => {
                    let should_retry = self.should_retry(&error, attempt);
                    last_error = Some(error);

                    if should_retry && attempt < self.max_attempts - 1 {
                        let delay = self.strategy.calculate_delay(attempt);
                        warn!(
                            correlation_id = %context.correlation_id,
                            operation = %context.operation,
                            attempt = attempt + 1,
                            delay_ms = delay.as_millis(),
                            error = %last_error.as_ref().unwrap(),
                            "Operation failed, retrying with backoff"
                        );
                        sleep(delay).await;
                    } else {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AppError::InternalServerError("Retry operation failed without error".to_string())
        }))
    }

    fn should_retry(&self, error: &AppError, attempt: u32) -> bool {
        if attempt >= self.max_attempts - 1 {
            return false;
        }

        if self.retry_conditions.is_empty() {
            return true; // Retry all errors if no conditions specified
        }

        self.retry_conditions
            .iter()
            .any(|condition| condition.should_retry(error))
    }
}

/// Trait for determining if an error should trigger a retry
pub trait RetryCondition: Send + Sync {
    fn should_retry(&self, error: &AppError) -> bool;
}

/// Retry condition for transient errors
pub struct TransientErrorCondition;

impl RetryCondition for TransientErrorCondition {
    fn should_retry(&self, error: &AppError) -> bool {
        matches!(
            error,
            AppError::ExternalServiceError(_)
                | AppError::DatabaseError(_)
                | AppError::InternalServerError(_)
        )
    }
}

/// Retry condition for timeout errors
pub struct TimeoutErrorCondition;

impl RetryCondition for TimeoutErrorCondition {
    fn should_retry(&self, error: &AppError) -> bool {
        error.to_string().to_lowercase().contains("timeout")
    }
}

/// Service mesh configuration for inter-service communication
#[derive(Debug, Clone, Default)]
pub struct ServiceMeshConfig {
    pub service_discovery: ServiceDiscoveryConfig,
    pub load_balancing: LoadBalancingConfig,
    pub health_check: HealthCheckConfig,
    pub security: SecurityConfig,
}



#[derive(Debug, Clone)]
pub struct ServiceDiscoveryConfig {
    pub registry_url: Option<String>,
    pub refresh_interval: Duration,
    pub cache_ttl: Duration,
}

impl Default for ServiceDiscoveryConfig {
    fn default() -> Self {
        Self {
            registry_url: None,
            refresh_interval: Duration::from_secs(30),
            cache_ttl: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadBalancingConfig {
    pub strategy: LoadBalancingStrategy,
    pub health_check_enabled: bool,
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::RoundRobin,
            health_check_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    Random,
}

#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub timeout: Duration,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            unhealthy_threshold: 3,
            healthy_threshold: 2,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    pub tls_enabled: bool,
    pub mutual_tls: bool,
    pub certificate_path: Option<String>,
    pub key_path: Option<String>,
}



/// Service mesh manager for handling inter-service communication
pub struct ServiceMeshManager {
    config: ServiceMeshConfig,
    service_registry: Arc<RwLock<HashMap<String, Vec<ServiceEndpoint>>>>,
    health_checker: Arc<HealthChecker>,
}

impl ServiceMeshManager {
    pub fn new(config: ServiceMeshConfig) -> Self {
        let health_checker = Arc::new(HealthChecker::new(config.health_check.clone()));

        Self {
            config,
            service_registry: Arc::new(RwLock::new(HashMap::new())),
            health_checker,
        }
    }

    pub async fn register_service(
        &self,
        service_name: String,
        endpoints: Vec<ServiceEndpoint>,
    ) -> Result<()> {
        let mut registry = self.service_registry.write().await;
        registry.insert(service_name.clone(), endpoints);

        info!(
            "Registered service: {} with {} endpoints",
            service_name,
            registry.get(&service_name).unwrap().len()
        );
        Ok(())
    }

    pub async fn discover_service(&self, service_name: &str) -> Result<Vec<ServiceEndpoint>> {
        let registry = self.service_registry.read().await;
        let endpoints = registry.get(service_name).cloned().unwrap_or_default();

        if self.config.load_balancing.health_check_enabled {
            let healthy_endpoints = self
                .health_checker
                .filter_healthy_endpoints(endpoints)
                .await;
            Ok(healthy_endpoints)
        } else {
            Ok(endpoints)
        }
    }

    pub async fn select_endpoint(&self, service_name: &str) -> Result<ServiceEndpoint> {
        let endpoints = self.discover_service(service_name).await?;

        if endpoints.is_empty() {
            return Err(AppError::ExternalServiceError(format!(
                "No healthy endpoints available for service: {}",
                service_name
            )));
        }

        let selected = match self.config.load_balancing.strategy {
            LoadBalancingStrategy::RoundRobin => {
                // Simple round-robin implementation
                let index = fastrand::usize(0..endpoints.len());
                endpoints[index].clone()
            }
            LoadBalancingStrategy::Random => {
                let index = fastrand::usize(0..endpoints.len());
                endpoints[index].clone()
            }
            LoadBalancingStrategy::LeastConnections => {
                // For simplicity, just return the first endpoint
                // In a real implementation, you'd track connection counts
                endpoints[0].clone()
            }
            LoadBalancingStrategy::WeightedRoundRobin => {
                // For simplicity, just return the first endpoint
                // In a real implementation, you'd consider weights
                endpoints[0].clone()
            }
        };

        debug!(
            "Selected endpoint: {}:{} for service: {}",
            selected.host, selected.port, service_name
        );
        Ok(selected)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceEndpoint {
    pub host: String,
    pub port: u16,
    pub weight: u32,
    pub metadata: HashMap<String, String>,
}

impl ServiceEndpoint {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            weight: 1,
            metadata: HashMap::new(),
        }
    }

    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Health checker for service endpoints
pub struct HealthChecker {
    config: HealthCheckConfig,
    endpoint_health: Arc<RwLock<HashMap<String, EndpointHealth>>>,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            endpoint_health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn filter_healthy_endpoints(
        &self,
        endpoints: Vec<ServiceEndpoint>,
    ) -> Vec<ServiceEndpoint> {
        if !self.config.enabled {
            return endpoints;
        }

        let health_map = self.endpoint_health.read().await;
        endpoints
            .into_iter()
            .filter(|endpoint| {
                let key = format!("{}:{}", endpoint.host, endpoint.port);
                health_map
                    .get(&key)
                    .map(|health| health.is_healthy)
                    .unwrap_or(true) // Assume healthy if not checked yet
            })
            .collect()
    }

    pub async fn check_endpoint_health(&self, endpoint: &ServiceEndpoint) -> bool {
        // Simplified health check - in a real implementation, you'd make HTTP requests
        let key = format!("{}:{}", endpoint.host, endpoint.port);
        let is_healthy = true; // Placeholder - implement actual health check logic

        let mut health_map = self.endpoint_health.write().await;
        let health = health_map.entry(key).or_insert_with(|| EndpointHealth {
            is_healthy: true,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_check: Instant::now(),
        });

        if is_healthy {
            health.consecutive_successes += 1;
            health.consecutive_failures = 0;
            health.is_healthy = health.consecutive_successes >= self.config.healthy_threshold;
        } else {
            health.consecutive_failures += 1;
            health.consecutive_successes = 0;
            health.is_healthy = health.consecutive_failures < self.config.unhealthy_threshold;
        }

        health.last_check = Instant::now();
        health.is_healthy
    }
}

#[derive(Debug, Clone)]
struct EndpointHealth {
    is_healthy: bool,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_check: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bulkhead_manager() {
        let manager = BulkheadManager::new();
        let config = BulkheadConfig {
            max_concurrent_requests: 2,
            queue_size: 10,
            timeout: Duration::from_secs(1),
            rejection_threshold: 0.8,
        };

        manager
            .create_pool("test_pool".to_string(), config)
            .await
            .unwrap();

        let context = ServiceContext::new("test_operation".to_string());
        let result = manager
            .execute_in_pool("test_pool", &context, || async {
                Ok("success".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_backoff_strategies() {
        let fixed = BackoffStrategy::Fixed(Duration::from_millis(100));
        assert_eq!(fixed.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(fixed.calculate_delay(5), Duration::from_millis(100));

        let exponential = BackoffStrategy::Exponential {
            base: Duration::from_millis(100),
            multiplier: 2.0,
            max: Duration::from_secs(10),
        };
        assert_eq!(exponential.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(exponential.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(exponential.calculate_delay(2), Duration::from_millis(400));
    }

    #[tokio::test]
    async fn test_retry_manager() {
        let strategy = BackoffStrategy::Fixed(Duration::from_millis(10));
        let retry_manager = RetryManager::new(strategy, 3).with_transient_errors();

        let context = ServiceContext::new("test_operation".to_string());
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let result = retry_manager
            .execute_with_retry(&context, || {
                let count = call_count.clone();
                async move {
                    let current = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err(AppError::ExternalServiceError(
                            "Transient error".to_string(),
                        ))
                    } else {
                        Ok("success".to_string())
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_service_mesh_manager() {
        let config = ServiceMeshConfig::default();
        let manager = ServiceMeshManager::new(config);

        let endpoints = vec![
            ServiceEndpoint::new("localhost".to_string(), 8080),
            ServiceEndpoint::new("localhost".to_string(), 8081),
        ];

        manager
            .register_service("test_service".to_string(), endpoints)
            .await
            .unwrap();

        let discovered = manager.discover_service("test_service").await.unwrap();
        assert_eq!(discovered.len(), 2);

        let selected = manager.select_endpoint("test_service").await.unwrap();
        assert!(selected.port == 8080 || selected.port == 8081);
    }
}
