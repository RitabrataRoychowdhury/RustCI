use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub reset_timeout: Duration,
    pub max_concurrent_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(30),
            reset_timeout: Duration::from_secs(60),
            max_concurrent_requests: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure_time: Option<SystemTime>,
    pub last_success_time: Option<SystemTime>,
    pub total_requests: u64,
    pub failed_requests: u64,
    pub concurrent_requests: u32,
}

impl Default for CircuitBreakerStats {
    fn default() -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_success_time: None,
            total_requests: 0,
            failed_requests: 0,
            concurrent_requests: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DegradationMode {
    None,
    ReadOnly,
    CacheOnly,
    FallbackService,
    MinimalFunctionality,
    MaintenanceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationConfig {
    pub mode: DegradationMode,
    pub fallback_endpoints: Vec<String>,
    pub cache_ttl: Duration,
    pub retry_after: Duration,
    pub health_check_interval: Duration,
}

impl Default for DegradationConfig {
    fn default() -> Self {
        Self {
            mode: DegradationMode::None,
            fallback_endpoints: Vec::new(),
            cache_ttl: Duration::from_secs(300),
            retry_after: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(10),
        }
    }
}

#[async_trait::async_trait]
pub trait CircuitBreaker: Send + Sync {
    async fn call<F, T, E>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, E>> + Send,
        E: Into<AppError>;
    
    async fn get_stats(&self) -> CircuitBreakerStats;
    async fn reset(&self) -> Result<()>;
    async fn force_open(&self) -> Result<()>;
    async fn force_close(&self) -> Result<()>;
}

pub struct ProductionCircuitBreaker {
    config: CircuitBreakerConfig,
    stats: Arc<RwLock<CircuitBreakerStats>>,
    last_state_change: Arc<RwLock<Instant>>,
}

impl ProductionCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(CircuitBreakerStats::default())),
            last_state_change: Arc::new(RwLock::new(Instant::now())),
        }
    }

    async fn should_allow_request(&self) -> bool {
        let stats = self.stats.read().await;
        
        match stats.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                let last_change = *self.last_state_change.read().await;
                last_change.elapsed() >= self.config.reset_timeout
            }
            CircuitBreakerState::HalfOpen => {
                stats.concurrent_requests < self.config.max_concurrent_requests
            }
        }
    }

    async fn record_success(&self) {
        let mut stats = self.stats.write().await;
        stats.success_count += 1;
        stats.total_requests += 1;
        stats.last_success_time = Some(SystemTime::now());

        match stats.state {
            CircuitBreakerState::HalfOpen => {
                if stats.success_count >= self.config.success_threshold {
                    stats.state = CircuitBreakerState::Closed;
                    stats.failure_count = 0;
                    stats.success_count = 0;
                    *self.last_state_change.write().await = Instant::now();
                    log::info!("Circuit breaker transitioned to CLOSED state");
                }
            }
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                stats.failure_count = 0;
            }
            _ => {}
        }

        if stats.concurrent_requests > 0 {
            stats.concurrent_requests -= 1;
        }
    }

    async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        stats.failure_count += 1;
        stats.total_requests += 1;
        stats.failed_requests += 1;
        stats.last_failure_time = Some(SystemTime::now());

        match stats.state {
            CircuitBreakerState::Closed => {
                if stats.failure_count >= self.config.failure_threshold {
                    stats.state = CircuitBreakerState::Open;
                    *self.last_state_change.write().await = Instant::now();
                    log::warn!("Circuit breaker transitioned to OPEN state after {} failures", stats.failure_count);
                }
            }
            CircuitBreakerState::HalfOpen => {
                stats.state = CircuitBreakerState::Open;
                stats.success_count = 0;
                *self.last_state_change.write().await = Instant::now();
                log::warn!("Circuit breaker transitioned back to OPEN state from HALF_OPEN");
            }
            _ => {}
        }

        if stats.concurrent_requests > 0 {
            stats.concurrent_requests -= 1;
        }
    }

    async fn transition_to_half_open(&self) {
        let mut stats = self.stats.write().await;
        if stats.state == CircuitBreakerState::Open {
            stats.state = CircuitBreakerState::HalfOpen;
            stats.failure_count = 0;
            stats.success_count = 0;
            *self.last_state_change.write().await = Instant::now();
            log::info!("Circuit breaker transitioned to HALF_OPEN state");
        }
    }
}

#[async_trait::async_trait]
impl CircuitBreaker for ProductionCircuitBreaker {
    async fn call<F, T, E>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, E>> + Send,
        T: Send,
        E: Into<AppError> + Send,
    {
        if !self.should_allow_request().await {
            let stats = self.stats.read().await;
            if stats.state == CircuitBreakerState::Open {
                self.transition_to_half_open().await;
                if !self.should_allow_request().await {
                    return Err(AppError::ServiceUnavailable(
                        "Circuit breaker is OPEN - service unavailable".to_string()
                    ));
                }
            } else {
                return Err(AppError::ServiceUnavailable(
                    "Circuit breaker - too many concurrent requests".to_string()
                ));
            }
        }

        // Increment concurrent requests
        {
            let mut stats = self.stats.write().await;
            stats.concurrent_requests += 1;
        }

        let start_time = Instant::now();
        let result = tokio::time::timeout(self.config.timeout, operation).await;

        match result {
            Ok(Ok(value)) => {
                self.record_success().await;
                Ok(value)
            }
            Ok(Err(e)) => {
                self.record_failure().await;
                Err(e.into())
            }
            Err(_) => {
                self.record_failure().await;
                Err(AppError::Timeout(format!(
                    "Operation timed out after {:?}",
                    self.config.timeout
                )))
            }
        }
    }

    async fn get_stats(&self) -> CircuitBreakerStats {
        self.stats.read().await.clone()
    }

    async fn reset(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        *stats = CircuitBreakerStats::default();
        *self.last_state_change.write().await = Instant::now();
        log::info!("Circuit breaker reset to initial state");
        Ok(())
    }

    async fn force_open(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        stats.state = CircuitBreakerState::Open;
        *self.last_state_change.write().await = Instant::now();
        log::warn!("Circuit breaker forced to OPEN state");
        Ok(())
    }

    async fn force_close(&self) -> Result<()> {
        let mut stats = self.stats.write().await;
        stats.state = CircuitBreakerState::Closed;
        stats.failure_count = 0;
        stats.success_count = 0;
        *self.last_state_change.write().await = Instant::now();
        log::info!("Circuit breaker forced to CLOSED state");
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait GracefulDegradationManager: Send + Sync {
    async fn enable_degradation(&self, service: &str, config: DegradationConfig) -> Result<()>;
    async fn disable_degradation(&self, service: &str) -> Result<()>;
    async fn get_degradation_status(&self, service: &str) -> Result<Option<DegradationConfig>>;
    async fn execute_with_fallback<F, T>(&self, service: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>> + Send,
        T: Send;
}

pub struct ProductionGracefulDegradationManager {
    degradation_configs: Arc<RwLock<HashMap<String, DegradationConfig>>>,
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<dyn CircuitBreaker>>>>,
    fallback_cache: Arc<RwLock<HashMap<String, (serde_json::Value, SystemTime)>>>,
}

impl ProductionGracefulDegradationManager {
    pub fn new() -> Self {
        Self {
            degradation_configs: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            fallback_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_circuit_breaker(&self, service: &str, circuit_breaker: Arc<dyn CircuitBreaker>) {
        let mut breakers = self.circuit_breakers.write().await;
        breakers.insert(service.to_string(), circuit_breaker);
    }

    async fn get_cached_response(&self, service: &str) -> Option<serde_json::Value> {
        let cache = self.fallback_cache.read().await;
        if let Some((value, timestamp)) = cache.get(service) {
            let configs = self.degradation_configs.read().await;
            if let Some(config) = configs.get(service) {
                if timestamp.elapsed().unwrap_or(Duration::MAX) < config.cache_ttl {
                    return Some(value.clone());
                }
            }
        }
        None
    }

    async fn cache_response(&self, service: &str, response: &serde_json::Value) {
        let mut cache = self.fallback_cache.write().await;
        cache.insert(service.to_string(), (response.clone(), SystemTime::now()));
    }

    async fn execute_fallback(&self, service: &str, config: &DegradationConfig) -> Result<serde_json::Value> {
        match &config.mode {
            DegradationMode::CacheOnly => {
                if let Some(cached) = self.get_cached_response(service).await {
                    Ok(cached)
                } else {
                    Err(AppError::ServiceUnavailable(
                        format!("Service {} unavailable and no cached response", service)
                    ))
                }
            }
            DegradationMode::FallbackService => {
                for endpoint in &config.fallback_endpoints {
                    match self.call_fallback_endpoint(endpoint).await {
                        Ok(response) => {
                            self.cache_response(service, &response).await;
                            return Ok(response);
                        }
                        Err(e) => {
                            log::warn!("Fallback endpoint {} failed: {}", endpoint, e);
                            continue;
                        }
                    }
                }
                Err(AppError::ServiceUnavailable(
                    format!("All fallback endpoints for {} failed", service)
                ))
            }
            DegradationMode::ReadOnly => {
                Ok(serde_json::json!({
                    "status": "read_only",
                    "message": "Service is in read-only mode"
                }))
            }
            DegradationMode::MinimalFunctionality => {
                Ok(serde_json::json!({
                    "status": "minimal",
                    "message": "Service is operating with minimal functionality"
                }))
            }
            DegradationMode::MaintenanceMode => {
                Err(AppError::ServiceUnavailable(
                    format!("Service {} is in maintenance mode", service)
                ))
            }
            DegradationMode::None => {
                Err(AppError::ServiceUnavailable(
                    format!("Service {} is unavailable", service)
                ))
            }
        }
    }

    async fn call_fallback_endpoint(&self, endpoint: &str) -> Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let response = client.get(endpoint)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::NetworkError(format!("Fallback request failed: {}", e)))?;

        if response.status().is_success() {
            let json = response.json::<serde_json::Value>().await
                .map_err(|e| AppError::SerializationError(format!("Failed to parse fallback response: {}", e)))?;
            Ok(json)
        } else {
            Err(AppError::NetworkError(format!(
                "Fallback endpoint returned status: {}", response.status()
            )))
        }
    }
}

#[async_trait::async_trait]
impl GracefulDegradationManager for ProductionGracefulDegradationManager {
    async fn enable_degradation(&self, service: &str, config: DegradationConfig) -> Result<()> {
        let mut configs = self.degradation_configs.write().await;
        configs.insert(service.to_string(), config);
        log::info!("Enabled graceful degradation for service: {}", service);
        Ok(())
    }

    async fn disable_degradation(&self, service: &str) -> Result<()> {
        let mut configs = self.degradation_configs.write().await;
        configs.remove(service);
        log::info!("Disabled graceful degradation for service: {}", service);
        Ok(())
    }

    async fn get_degradation_status(&self, service: &str) -> Result<Option<DegradationConfig>> {
        let configs = self.degradation_configs.read().await;
        Ok(configs.get(service).cloned())
    }

    async fn execute_with_fallback<F, T>(&self, service: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        // First try to execute through circuit breaker if available
        let circuit_breaker = {
            let breakers = self.circuit_breakers.read().await;
            breakers.get(service).cloned()
        };

        if let Some(breaker) = circuit_breaker {
            match breaker.call(operation).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::warn!("Circuit breaker failed for service {}: {}", service, e);
                    // Continue to fallback logic
                }
            }
        } else {
            // No circuit breaker, try operation directly
            match operation.await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::warn!("Direct operation failed for service {}: {}", service, e);
                    // Continue to fallback logic
                }
            }
        }

        // Check if degradation is configured
        let degradation_config = {
            let configs = self.degradation_configs.read().await;
            configs.get(service).cloned()
        };

        if let Some(config) = degradation_config {
            // This is a simplified fallback - in a real implementation,
            // you'd need to handle type conversion properly
            let fallback_result = self.execute_fallback(service, &config).await?;
            
            // For this example, we'll return an error since we can't convert JSON to T
            // In a real implementation, you'd need proper type handling
            Err(AppError::ServiceUnavailable(
                format!("Service {} failed and fallback returned: {}", service, fallback_result)
            ))
        } else {
            Err(AppError::ServiceUnavailable(
                format!("Service {} is unavailable and no degradation configured", service)
            ))
        }
    }
}

impl Default for ProductionGracefulDegradationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(1),
            reset_timeout: Duration::from_secs(5),
            max_concurrent_requests: 5,
        };

        let breaker = ProductionCircuitBreaker::new(config);
        
        // Initially closed
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, CircuitBreakerState::Closed);

        // Simulate failures
        for i in 0..3 {
            let result = breaker.call(async { 
                Err::<(), AppError>(AppError::InternalServerError("test failure".to_string()))
            }).await;
            assert!(result.is_err());
            
            let stats = breaker.get_stats().await;
            if i < 2 {
                assert_eq!(stats.state, CircuitBreakerState::Closed);
            } else {
                assert_eq!(stats.state, CircuitBreakerState::Open);
            }
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_to_closed() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_secs(1),
            reset_timeout: Duration::from_millis(100),
            max_concurrent_requests: 5,
        };

        let breaker = ProductionCircuitBreaker::new(config);
        
        // Force to open state
        breaker.force_open().await.unwrap();
        
        // Wait for reset timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should transition to half-open and then closed with successes
        for _ in 0..2 {
            let result = breaker.call(async { Ok::<(), AppError>(()) }).await;
            assert!(result.is_ok());
        }
        
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_graceful_degradation_manager() {
        let manager = ProductionGracefulDegradationManager::new();
        
        let config = DegradationConfig {
            mode: DegradationMode::ReadOnly,
            fallback_endpoints: vec![],
            cache_ttl: Duration::from_secs(300),
            retry_after: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(10),
        };

        // Enable degradation
        manager.enable_degradation("test-service", config.clone()).await.unwrap();
        
        // Check status
        let status = manager.get_degradation_status("test-service").await.unwrap();
        assert!(status.is_some());
        assert!(matches!(status.unwrap().mode, DegradationMode::ReadOnly));

        // Disable degradation
        manager.disable_degradation("test-service").await.unwrap();
        
        let status = manager.get_degradation_status("test-service").await.unwrap();
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_circuit_breaker_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            reset_timeout: Duration::from_secs(5),
            max_concurrent_requests: 5,
        };

        let breaker = ProductionCircuitBreaker::new(config);
        
        let result = breaker.call(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok::<(), AppError>(())
        }).await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::Timeout(_)));
    }
}