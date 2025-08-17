//! Service registry for Valkyrie Protocol

use crate::{Result, ValkyrieError};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};


/// Service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Unique service identifier
    pub id: String,
    /// Service name
    pub name: String,
    /// Service version
    pub version: String,
    /// Service endpoint address
    pub endpoint: String,
    /// Service metadata
    pub metadata: HashMap<String, String>,
    /// Service health status
    pub health_status: HealthStatus,
    /// Registration timestamp
    pub registered_at: chrono::DateTime<chrono::Utc>,
    /// Last health check timestamp
    pub last_health_check: chrono::DateTime<chrono::Utc>,
}

/// Health status of a service
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Service is healthy and available
    Healthy,
    /// Service is unhealthy but may recover
    Unhealthy,
    /// Service is in warning state
    Warning,
    /// Service health is unknown
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Service registry for managing service discovery
pub struct ServiceRegistry {
    services: Arc<RwLock<HashMap<String, ServiceInfo>>>,
    health_check_interval_ms: u64,
}

impl ServiceRegistry {
    /// Create a new service registry
    pub fn new() -> Self {
        Self::with_health_check_interval(30000) // 30 seconds default
    }
    
    /// Create a new service registry with custom health check interval
    pub fn with_health_check_interval(interval_ms: u64) -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            health_check_interval_ms: interval_ms,
        }
    }
    
    /// Register a new service
    pub async fn register_service(&self, mut service: ServiceInfo) -> Result<()> {
        service.registered_at = chrono::Utc::now();
        service.last_health_check = chrono::Utc::now();
        
        info!("Registering service: {} ({})", service.name, service.id);
        
        let mut services = self.services.write().await;
        services.insert(service.id.clone(), service);
        
        Ok(())
    }
    
    /// Unregister a service
    pub async fn unregister_service<S: AsRef<str>>(&self, service_id: S) -> Result<()> {
        let service_id = service_id.as_ref();
        info!("Unregistering service: {}", service_id);
        
        let mut services = self.services.write().await;
        if services.remove(service_id).is_some() {
            Ok(())
        } else {
            Err(ValkyrieError::service_unavailable(format!("Service {} not found", service_id)))
        }
    }
    
    /// Get service information by ID
    pub async fn get_service<S: AsRef<str>>(&self, service_id: S) -> Option<ServiceInfo> {
        let services = self.services.read().await;
        services.get(service_id.as_ref()).cloned()
    }
    
    /// Get all services with a specific name
    pub async fn get_services_by_name<S: AsRef<str>>(&self, name: S) -> Vec<ServiceInfo> {
        let name = name.as_ref();
        let services = self.services.read().await;
        
        services.values()
            .filter(|service| service.name == name)
            .cloned()
            .collect()
    }
    
    /// Get all healthy services
    pub async fn get_healthy_services(&self) -> Vec<ServiceInfo> {
        let services = self.services.read().await;
        
        services.values()
            .filter(|service| service.health_status == HealthStatus::Healthy)
            .cloned()
            .collect()
    }
    
    /// Get all services
    pub async fn get_all_services(&self) -> Vec<ServiceInfo> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }
    
    /// Update service health status
    pub async fn update_health_status<S: AsRef<str>>(&self, service_id: S, status: HealthStatus) -> Result<()> {
        let service_id = service_id.as_ref();
        debug!("Updating health status for service {}: {:?}", service_id, status);
        
        let mut services = self.services.write().await;
        if let Some(service) = services.get_mut(service_id) {
            service.health_status = status;
            service.last_health_check = chrono::Utc::now();
            Ok(())
        } else {
            Err(ValkyrieError::service_unavailable(format!("Service {} not found", service_id)))
        }
    }
    
    /// Find services by metadata
    pub async fn find_services_by_metadata(&self, key: &str, value: &str) -> Vec<ServiceInfo> {
        let services = self.services.read().await;
        
        services.values()
            .filter(|service| {
                service.metadata.get(key).map(|v| v == value).unwrap_or(false)
            })
            .cloned()
            .collect()
    }
    
    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let services = self.services.read().await;
        
        let mut healthy = 0;
        let mut unhealthy = 0;
        let mut warning = 0;
        let mut unknown = 0;
        
        for service in services.values() {
            match service.health_status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Unhealthy => unhealthy += 1,
                HealthStatus::Warning => warning += 1,
                HealthStatus::Unknown => unknown += 1,
            }
        }
        
        RegistryStats {
            total_services: services.len(),
            healthy_services: healthy,
            unhealthy_services: unhealthy,
            warning_services: warning,
            unknown_services: unknown,
        }
    }
    
    /// Start periodic health checks
    pub async fn start_health_checks(&self) -> Result<()> {
        info!("Starting periodic health checks (interval: {}ms)", self.health_check_interval_ms);
        
        let services = Arc::clone(&self.services);
        let interval = self.health_check_interval_ms;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(interval));
            
            loop {
                interval.tick().await;
                
                let service_ids: Vec<String> = {
                    let services_guard = services.read().await;
                    services_guard.keys().cloned().collect()
                };
                
                for service_id in service_ids {
                    // TODO: Implement actual health check logic
                    // For now, just update the timestamp
                    let mut services_guard = services.write().await;
                    if let Some(service) = services_guard.get_mut(&service_id) {
                        service.last_health_check = chrono::Utc::now();
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Clean up stale services
    pub async fn cleanup_stale_services(&self, max_age_ms: u64) -> Result<usize> {
        debug!("Cleaning up stale services (max age: {}ms)", max_age_ms);
        
        let now = chrono::Utc::now();
        let max_age = chrono::Duration::milliseconds(max_age_ms as i64);
        
        let stale_ids: Vec<String> = {
            let services = self.services.read().await;
            services.iter()
                .filter(|(_, service)| {
                    now.signed_duration_since(service.last_health_check) > max_age
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        
        let count = stale_ids.len();
        for id in stale_ids {
            self.unregister_service(&id).await?;
            warn!("Removed stale service: {}", id);
        }
        
        debug!("Cleaned up {} stale services", count);
        Ok(count)
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    /// Total number of registered services
    pub total_services: usize,
    /// Number of healthy services
    pub healthy_services: usize,
    /// Number of unhealthy services
    pub unhealthy_services: usize,
    /// Number of services in warning state
    pub warning_services: usize,
    /// Number of services with unknown health
    pub unknown_services: usize,
}