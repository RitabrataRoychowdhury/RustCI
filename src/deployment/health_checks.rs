use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub endpoint: String,
    pub timeout_seconds: u64,
    pub interval_seconds: u64,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub check_name: String,
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[async_trait]
pub trait HealthChecker: Send + Sync {
    async fn check_health(&self, check: &HealthCheck) -> Result<HealthCheckResult, HealthCheckError>;
    async fn check_all_health(&self, checks: &[HealthCheck]) -> Result<Vec<HealthCheckResult>, HealthCheckError>;
    async fn is_service_healthy(&self, service_name: &str) -> Result<bool, HealthCheckError>;
}

#[derive(Debug, thiserror::Error)]
pub enum HealthCheckError {
    #[error("Health check failed: {0}")]
    CheckFailed(String),
    
    #[error("Health check timeout: {0}")]
    Timeout(String),
    
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}

pub struct ProductionHealthChecker {
    client: reqwest::Client,
}

impl ProductionHealthChecker {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl HealthChecker for ProductionHealthChecker {
    async fn check_health(&self, check: &HealthCheck) -> Result<HealthCheckResult, HealthCheckError> {
        let start_time = std::time::Instant::now();
        
        println!("Performing health check: {}", check.name);
        
        let timeout = std::time::Duration::from_secs(check.timeout_seconds);
        
        match tokio::time::timeout(timeout, self.client.get(&check.endpoint).send()).await {
            Ok(Ok(response)) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                let status = if response.status().is_success() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                };
                
                Ok(HealthCheckResult {
                    check_name: check.name.clone(),
                    status,
                    response_time_ms: response_time,
                    message: Some(format!("HTTP {}", response.status())),
                    timestamp: Utc::now(),
                })
            }
            Ok(Err(e)) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                Ok(HealthCheckResult {
                    check_name: check.name.clone(),
                    status: HealthStatus::Unhealthy,
                    response_time_ms: response_time,
                    message: Some(format!("Request failed: {}", e)),
                    timestamp: Utc::now(),
                })
            }
            Err(_) => {
                Err(HealthCheckError::Timeout(format!("Health check timed out after {} seconds", check.timeout_seconds)))
            }
        }
    }

    async fn check_all_health(&self, checks: &[HealthCheck]) -> Result<Vec<HealthCheckResult>, HealthCheckError> {
        let mut results = Vec::new();
        
        for check in checks {
            match self.check_health(check).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    // Continue with other checks even if one fails
                    results.push(HealthCheckResult {
                        check_name: check.name.clone(),
                        status: HealthStatus::Unknown,
                        response_time_ms: 0,
                        message: Some(format!("Check error: {}", e)),
                        timestamp: Utc::now(),
                    });
                }
            }
        }
        
        Ok(results)
    }

    async fn is_service_healthy(&self, service_name: &str) -> Result<bool, HealthCheckError> {
        // This would typically check a service registry or health endpoint
        println!("Checking if service is healthy: {}", service_name);
        
        // For now, simulate a health check
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        // Simulate some services being unhealthy
        Ok(!service_name.contains("unhealthy"))
    }
}

pub struct MockHealthChecker {
    pub results: HashMap<String, HealthCheckResult>,
}

impl MockHealthChecker {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }
    
    pub fn set_result(&mut self, check_name: String, result: HealthCheckResult) {
        self.results.insert(check_name, result);
    }
}

#[async_trait]
impl HealthChecker for MockHealthChecker {
    async fn check_health(&self, check: &HealthCheck) -> Result<HealthCheckResult, HealthCheckError> {
        if let Some(result) = self.results.get(&check.name) {
            Ok(result.clone())
        } else {
            Ok(HealthCheckResult {
                check_name: check.name.clone(),
                status: HealthStatus::Healthy,
                response_time_ms: 100,
                message: Some("Mock health check".to_string()),
                timestamp: Utc::now(),
            })
        }
    }

    async fn check_all_health(&self, checks: &[HealthCheck]) -> Result<Vec<HealthCheckResult>, HealthCheckError> {
        let mut results = Vec::new();
        for check in checks {
            results.push(self.check_health(check).await?);
        }
        Ok(results)
    }

    async fn is_service_healthy(&self, service_name: &str) -> Result<bool, HealthCheckError> {
        Ok(!service_name.contains("unhealthy"))
    }
}