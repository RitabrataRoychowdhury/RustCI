use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};
use crate::deployment::circuit_breaker::{CircuitBreaker, DegradationMode, DegradationConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthStatus {
    pub service_name: String,
    pub is_healthy: bool,
    pub last_check: SystemTime,
    pub consecutive_failures: u32,
    pub response_time: Option<Duration>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackMechanism {
    pub mechanism_type: FallbackType,
    pub priority: u32,
    pub config: FallbackConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallbackType {
    CachedResponse,
    AlternativeService,
    StaticResponse,
    QueuedRequest,
    ReducedFunctionality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    pub endpoint: Option<String>,
    pub timeout: Duration,
    pub retry_count: u32,
    pub static_response: Option<serde_json::Value>,
    pub cache_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDegradationState {
    pub overall_health: f64,
    pub degraded_services: Vec<String>,
    pub active_fallbacks: HashMap<String, FallbackType>,
    pub last_updated: SystemTime,
}

#[async_trait::async_trait]
pub trait SystemHealthMonitor: Send + Sync {
    async fn check_service_health(&self, service: &str) -> Result<ServiceHealthStatus>;
    async fn get_system_health(&self) -> Result<SystemDegradationState>;
    async fn register_health_check(&self, service: &str, endpoint: String, interval: Duration) -> Result<()>;
    async fn start_monitoring(&self) -> Result<()>;
    async fn stop_monitoring(&self) -> Result<()>;
}

#[async_trait::async_trait]
pub trait FallbackExecutor: Send + Sync {
    async fn execute_fallback(&self, service: &str, mechanism: &FallbackMechanism) -> Result<serde_json::Value>;
    async fn register_fallback(&self, service: &str, mechanism: FallbackMechanism) -> Result<()>;
    async fn remove_fallback(&self, service: &str) -> Result<()>;
}

pub struct ProductionSystemHealthMonitor {
    health_checks: Arc<RwLock<HashMap<String, (String, Duration)>>>, // service -> (endpoint, interval)
    health_status: Arc<RwLock<HashMap<String, ServiceHealthStatus>>>,
    monitoring_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
    client: reqwest::Client,
}

impl ProductionSystemHealthMonitor {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            health_checks: Arc::new(RwLock::new(HashMap::new())),
            health_status: Arc::new(RwLock::new(HashMap::new())),
            monitoring_tasks: Arc::new(RwLock::new(HashMap::new())),
            client,
        }
    }

    async fn perform_health_check(&self, service: &str, endpoint: &str) -> ServiceHealthStatus {
        let start_time = SystemTime::now();
        
        match self.client.get(endpoint).send().await {
            Ok(response) => {
                let response_time = start_time.elapsed().ok();
                let is_healthy = response.status().is_success();
                
                ServiceHealthStatus {
                    service_name: service.to_string(),
                    is_healthy,
                    last_check: SystemTime::now(),
                    consecutive_failures: if is_healthy { 0 } else { 1 },
                    response_time,
                    error_message: if is_healthy { 
                        None 
                    } else { 
                        Some(format!("HTTP {}", response.status())) 
                    },
                }
            }
            Err(e) => {
                ServiceHealthStatus {
                    service_name: service.to_string(),
                    is_healthy: false,
                    last_check: SystemTime::now(),
                    consecutive_failures: 1,
                    response_time: None,
                    error_message: Some(e.to_string()),
                }
            }
        }
    }

    async fn start_service_monitoring(&self, service: String, endpoint: String, interval: Duration) {
        let health_status = Arc::clone(&self.health_status);
        let client = self.client.clone();
        
        let task = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                let start_time = SystemTime::now();
                let health_check_result = match client.get(&endpoint).send().await {
                    Ok(response) => {
                        let response_time = start_time.elapsed().ok();
                        let is_healthy = response.status().is_success();
                        
                        ServiceHealthStatus {
                            service_name: service.clone(),
                            is_healthy,
                            last_check: SystemTime::now(),
                            consecutive_failures: if is_healthy { 0 } else { 1 },
                            response_time,
                            error_message: if is_healthy { 
                                None 
                            } else { 
                                Some(format!("HTTP {}", response.status())) 
                            },
                        }
                    }
                    Err(e) => {
                        ServiceHealthStatus {
                            service_name: service.clone(),
                            is_healthy: false,
                            last_check: SystemTime::now(),
                            consecutive_failures: 1,
                            response_time: None,
                            error_message: Some(e.to_string()),
                        }
                    }
                };

                // Update consecutive failures
                let mut status_map = health_status.write().await;
                if let Some(existing_status) = status_map.get(&service) {
                    let consecutive_failures = if health_check_result.is_healthy {
                        0
                    } else {
                        existing_status.consecutive_failures + 1
                    };
                    
                    let updated_status = ServiceHealthStatus {
                        consecutive_failures,
                        ..health_check_result
                    };
                    
                    status_map.insert(service.clone(), updated_status);
                } else {
                    status_map.insert(service.clone(), health_check_result);
                }
            }
        });

        let mut tasks = self.monitoring_tasks.write().await;
        if let Some(old_task) = tasks.insert(service, task) {
            old_task.abort();
        }
    }
}

#[async_trait::async_trait]
impl SystemHealthMonitor for ProductionSystemHealthMonitor {
    async fn check_service_health(&self, service: &str) -> Result<ServiceHealthStatus> {
        let health_checks = self.health_checks.read().await;
        
        if let Some((endpoint, _)) = health_checks.get(service) {
            let status = self.perform_health_check(service, endpoint).await;
            
            // Update stored status
            {
                let mut status_map = self.health_status.write().await;
                status_map.insert(service.to_string(), status.clone());
            }
            
            Ok(status)
        } else {
            Err(AppError::NotFound(format!("No health check configured for service: {}", service)))
        }
    }

    async fn get_system_health(&self) -> Result<SystemDegradationState> {
        let status_map = self.health_status.read().await;
        
        let total_services = status_map.len() as f64;
        let healthy_services = status_map.values()
            .filter(|status| status.is_healthy)
            .count() as f64;
        
        let overall_health = if total_services > 0.0 {
            healthy_services / total_services
        } else {
            1.0
        };

        let degraded_services: Vec<String> = status_map.values()
            .filter(|status| !status.is_healthy)
            .map(|status| status.service_name.clone())
            .collect();

        // For now, we'll use a simple mapping of degraded services to fallback types
        let active_fallbacks: HashMap<String, FallbackType> = degraded_services.iter()
            .map(|service| (service.clone(), FallbackType::CachedResponse))
            .collect();

        Ok(SystemDegradationState {
            overall_health,
            degraded_services,
            active_fallbacks,
            last_updated: SystemTime::now(),
        })
    }

    async fn register_health_check(&self, service: &str, endpoint: String, interval: Duration) -> Result<()> {
        {
            let mut health_checks = self.health_checks.write().await;
            health_checks.insert(service.to_string(), (endpoint.clone(), interval));
        }

        // Start monitoring for this service
        let endpoint_clone = endpoint.clone();
        self.start_service_monitoring(service.to_string(), endpoint, interval).await;
        
        log::info!("Registered health check for service: {} at endpoint: {}", service, endpoint_clone);
        Ok(())
    }

    async fn start_monitoring(&self) -> Result<()> {
        let health_checks = self.health_checks.read().await;
        
        for (service, (endpoint, interval)) in health_checks.iter() {
            self.start_service_monitoring(service.clone(), endpoint.clone(), *interval).await;
        }
        
        log::info!("Started health monitoring for {} services", health_checks.len());
        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        let mut tasks = self.monitoring_tasks.write().await;
        
        for (service, task) in tasks.drain() {
            task.abort();
            log::info!("Stopped monitoring for service: {}", service);
        }
        
        Ok(())
    }
}

impl Default for ProductionSystemHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ProductionFallbackExecutor {
    fallback_mechanisms: Arc<RwLock<HashMap<String, Vec<FallbackMechanism>>>>,
    response_cache: Arc<RwLock<HashMap<String, (serde_json::Value, SystemTime)>>>,
    client: reqwest::Client,
}

impl ProductionFallbackExecutor {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            fallback_mechanisms: Arc::new(RwLock::new(HashMap::new())),
            response_cache: Arc::new(RwLock::new(HashMap::new())),
            client,
        }
    }

    async fn execute_cached_response(&self, service: &str, config: &FallbackConfig) -> Result<serde_json::Value> {
        let cache = self.response_cache.read().await;
        let cache_key = config.cache_key.as_ref().unwrap_or(&service.to_string());
        
        if let Some((cached_response, timestamp)) = cache.get(cache_key) {
            // Check if cache is still valid (using a default TTL of 5 minutes)
            let cache_ttl = Duration::from_secs(300);
            if timestamp.elapsed().unwrap_or(Duration::MAX) < cache_ttl {
                return Ok(cached_response.clone());
            }
        }
        
        Err(AppError::NotFound(format!("No valid cached response for service: {}", service)))
    }

    async fn execute_alternative_service(&self, config: &FallbackConfig) -> Result<serde_json::Value> {
        if let Some(endpoint) = &config.endpoint {
            let mut attempts = 0;
            
            while attempts <= config.retry_count {
                match tokio::time::timeout(config.timeout, self.client.get(endpoint).send()).await {
                    Ok(Ok(response)) => {
                        if response.status().is_success() {
                            let json = response.json::<serde_json::Value>().await
                                .map_err(|e| AppError::SerializationError(e.to_string()))?;
                            return Ok(json);
                        }
                    }
                    Ok(Err(e)) => {
                        log::warn!("Alternative service request failed (attempt {}): {}", attempts + 1, e);
                    }
                    Err(_) => {
                        log::warn!("Alternative service request timed out (attempt {})", attempts + 1);
                    }
                }
                
                attempts += 1;
                if attempts <= config.retry_count {
                    tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                }
            }
        }
        
        Err(AppError::ServiceUnavailable("Alternative service unavailable".to_string()))
    }

    async fn execute_static_response(&self, config: &FallbackConfig) -> Result<serde_json::Value> {
        if let Some(static_response) = &config.static_response {
            Ok(static_response.clone())
        } else {
            Ok(serde_json::json!({
                "status": "fallback",
                "message": "Service temporarily unavailable"
            }))
        }
    }

    async fn cache_response(&self, service: &str, response: &serde_json::Value) {
        let mut cache = self.response_cache.write().await;
        cache.insert(service.to_string(), (response.clone(), SystemTime::now()));
    }
}

#[async_trait::async_trait]
impl FallbackExecutor for ProductionFallbackExecutor {
    async fn execute_fallback(&self, service: &str, mechanism: &FallbackMechanism) -> Result<serde_json::Value> {
        log::info!("Executing fallback for service: {} with mechanism: {:?}", service, mechanism.mechanism_type);
        
        let result = match mechanism.mechanism_type {
            FallbackType::CachedResponse => {
                self.execute_cached_response(service, &mechanism.config).await
            }
            FallbackType::AlternativeService => {
                self.execute_alternative_service(&mechanism.config).await
            }
            FallbackType::StaticResponse => {
                self.execute_static_response(&mechanism.config).await
            }
            FallbackType::QueuedRequest => {
                // For queued requests, we'd typically store the request for later processing
                // and return an acknowledgment
                Ok(serde_json::json!({
                    "status": "queued",
                    "message": "Request queued for processing when service is available"
                }))
            }
            FallbackType::ReducedFunctionality => {
                Ok(serde_json::json!({
                    "status": "reduced_functionality",
                    "message": "Service operating with reduced functionality"
                }))
            }
        };

        // Cache successful responses for future use
        if let Ok(ref response) = result {
            self.cache_response(service, response).await;
        }

        result
    }

    async fn register_fallback(&self, service: &str, mechanism: FallbackMechanism) -> Result<()> {
        let mut mechanisms = self.fallback_mechanisms.write().await;
        
        let service_mechanisms = mechanisms.entry(service.to_string()).or_insert_with(Vec::new);
        service_mechanisms.push(mechanism);
        
        // Sort by priority (lower number = higher priority)
        service_mechanisms.sort_by_key(|m| m.priority);
        
        log::info!("Registered fallback mechanism for service: {}", service);
        Ok(())
    }

    async fn remove_fallback(&self, service: &str) -> Result<()> {
        let mut mechanisms = self.fallback_mechanisms.write().await;
        mechanisms.remove(service);
        
        log::info!("Removed fallback mechanisms for service: {}", service);
        Ok(())
    }
}

impl Default for ProductionFallbackExecutor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SystemDegradationOrchestrator {
    health_monitor: Arc<dyn SystemHealthMonitor>,
    fallback_executor: Arc<dyn FallbackExecutor>,
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<dyn CircuitBreaker>>>>,
    degradation_thresholds: Arc<RwLock<HashMap<String, f64>>>,
}

impl SystemDegradationOrchestrator {
    pub fn new(
        health_monitor: Arc<dyn SystemHealthMonitor>,
        fallback_executor: Arc<dyn FallbackExecutor>,
    ) -> Self {
        Self {
            health_monitor,
            fallback_executor,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            degradation_thresholds: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_circuit_breaker(&self, service: &str, circuit_breaker: Arc<dyn CircuitBreaker>) {
        let mut breakers = self.circuit_breakers.write().await;
        breakers.insert(service.to_string(), circuit_breaker);
    }

    pub async fn set_degradation_threshold(&self, service: &str, threshold: f64) -> Result<()> {
        let mut thresholds = self.degradation_thresholds.write().await;
        thresholds.insert(service.to_string(), threshold);
        Ok(())
    }

    pub async fn execute_with_resilience<F, T>(&self, service: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>> + Send,
        T: Send + serde::Serialize,
    {
        // Try to execute through circuit breaker first
        let circuit_breaker = {
            let breakers = self.circuit_breakers.read().await;
            breakers.get(service).cloned()
        };

        if let Some(breaker) = circuit_breaker {
            match breaker.call(operation).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::warn!("Circuit breaker execution failed for {}: {}", service, e);
                    // Continue to fallback logic
                }
            }
        } else {
            // No circuit breaker, try direct execution
            match operation.await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::warn!("Direct execution failed for {}: {}", service, e);
                    // Continue to fallback logic
                }
            }
        }

        // Execute fallback mechanisms
        let mechanisms = {
            let fallback_mechanisms = self.fallback_executor.fallback_mechanisms.read().await;
            fallback_mechanisms.get(service).cloned().unwrap_or_default()
        };

        for mechanism in mechanisms {
            match self.fallback_executor.execute_fallback(service, &mechanism).await {
                Ok(fallback_response) => {
                    log::info!("Fallback successful for service: {} using mechanism: {:?}", 
                              service, mechanism.mechanism_type);
                    
                    // This is a limitation - we can't convert JSON back to T
                    // In a real implementation, you'd need proper type handling
                    return Err(AppError::ServiceUnavailable(
                        format!("Service {} failed, fallback returned: {}", service, fallback_response)
                    ));
                }
                Err(e) => {
                    log::warn!("Fallback mechanism failed for {}: {}", service, e);
                    continue;
                }
            }
        }

        Err(AppError::ServiceUnavailable(
            format!("All resilience mechanisms failed for service: {}", service)
        ))
    }

    pub async fn get_system_resilience_status(&self) -> Result<SystemDegradationState> {
        self.health_monitor.get_system_health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_health_monitor() {
        let monitor = ProductionSystemHealthMonitor::new();
        
        // Register a health check (this will fail since there's no actual service)
        let result = monitor.register_health_check(
            "test-service",
            "http://localhost:9999/health".to_string(),
            Duration::from_secs(30),
        ).await;
        
        assert!(result.is_ok());
        
        // Check system health
        let health = monitor.get_system_health().await.unwrap();
        assert!(health.overall_health >= 0.0 && health.overall_health <= 1.0);
    }

    #[tokio::test]
    async fn test_fallback_executor() {
        let executor = ProductionFallbackExecutor::new();
        
        let mechanism = FallbackMechanism {
            mechanism_type: FallbackType::StaticResponse,
            priority: 1,
            config: FallbackConfig {
                endpoint: None,
                timeout: Duration::from_secs(5),
                retry_count: 2,
                static_response: Some(serde_json::json!({"status": "ok", "data": "fallback"})),
                cache_key: None,
            },
        };

        executor.register_fallback("test-service", mechanism).await.unwrap();
        
        let mechanisms = executor.fallback_mechanisms.read().await;
        let service_mechanisms = mechanisms.get("test-service").unwrap();
        assert_eq!(service_mechanisms.len(), 1);
        assert_eq!(service_mechanisms[0].priority, 1);
    }

    #[tokio::test]
    async fn test_static_response_fallback() {
        let executor = ProductionFallbackExecutor::new();
        
        let mechanism = FallbackMechanism {
            mechanism_type: FallbackType::StaticResponse,
            priority: 1,
            config: FallbackConfig {
                endpoint: None,
                timeout: Duration::from_secs(5),
                retry_count: 2,
                static_response: Some(serde_json::json!({"message": "service unavailable"})),
                cache_key: None,
            },
        };

        let result = executor.execute_fallback("test-service", &mechanism).await.unwrap();
        assert_eq!(result["message"], "service unavailable");
    }
}