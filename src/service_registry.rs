use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use crate::core::{
    infrastructure::{
        dependency_injection::{ServiceContainer, ServiceFactory, ServiceLifetime, ServiceScope},
        resilience::{BulkheadConfig, BulkheadManager},
        service_decorators::{CircuitBreakerConfig, RetryConfig, Service, ServiceDecorator},
    },
};
use crate::error::{AppError, Result};

/// Enhanced service information stored in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub status: ServiceStatus,
    pub endpoint: String,
    pub health_check_url: Option<String>,
    pub metadata: HashMap<String, String>,
    pub registered_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
    pub service_type: String,
    pub lifetime: ServiceLifetime,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum ServiceStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
    #[default]
    Unknown,
}

/// Enhanced service registry with dependency injection integration
pub struct ServiceRegistry {
    services: Arc<RwLock<HashMap<Uuid, ServiceInfo>>>,
    name_index: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    container: Arc<ServiceContainer>,
    bulkhead_manager: Arc<BulkheadManager>,
    service_factories: Arc<RwLock<HashMap<String, Box<dyn ServiceFactory>>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        let container = Arc::new(ServiceContainer::new());
        let bulkhead_manager = Arc::new(BulkheadManager::new());

        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            container,
            bulkhead_manager,
            service_factories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a service factory with the DI container
    pub async fn register_service_factory<T: 'static + Send + Sync + Default>(
        &self,
        name: String,
        lifetime: ServiceLifetime,
        factory: Box<dyn ServiceFactory>,
        dependencies: Vec<String>,
    ) -> Result<()> {
        // Register with DI container
        let type_dependencies: Vec<TypeId> = dependencies
            .iter()
            .map(|_| TypeId::of::<T>()) // Simplified - in real implementation, you'd map names to TypeIds
            .collect();

        // Create a simple factory for DI container registration
        let _factory_name = factory.service_name().to_string();
        let simple_factory = Box::new(ExampleServiceFactory::<T>::new());

        self.container
            .register::<T>(lifetime.clone(), simple_factory, type_dependencies)
            .map_err(|e| AppError::DependencyNotFound {
                service: format!("Failed to register service factory: {}", e),
            })?;

        // Store factory reference
        {
            let mut factories = self.service_factories.write().await;
            factories.insert(name.clone(), factory);
        }

        // Create bulkhead pool for the service
        let bulkhead_config = BulkheadConfig::default();
        self.bulkhead_manager
            .create_pool(name.clone(), bulkhead_config)
            .await?;

        info!(
            "Registered service factory: {} with lifetime: {:?}",
            name, lifetime
        );
        Ok(())
    }

    /// Register a new service instance
    pub async fn register_service(
        &self,
        name: String,
        version: String,
        endpoint: String,
        health_check_url: Option<String>,
        metadata: HashMap<String, String>,
        service_type: String,
        lifetime: ServiceLifetime,
        dependencies: Vec<String>,
    ) -> Result<Uuid> {
        let service_id = Uuid::new_v4();
        let service_info = ServiceInfo {
            id: service_id,
            name: name.clone(),
            version,
            status: ServiceStatus::Starting,
            endpoint,
            health_check_url,
            metadata,
            registered_at: chrono::Utc::now(),
            last_heartbeat: None,
            service_type,
            lifetime,
            dependencies,
        };

        // Add to services map
        {
            let mut services = self.services.write().await;
            services.insert(service_id, service_info);
        }

        // Update name index
        {
            let mut name_index = self.name_index.write().await;
            name_index
                .entry(name.clone())
                .or_insert_with(Vec::new)
                .push(service_id);
        }

        info!("Registered service: {} ({})", name, service_id);
        Ok(service_id)
    }

    /// Resolve a service instance using dependency injection
    pub async fn resolve_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>> {
        self.container
            .resolve::<T>()
            .await
            .map_err(|e| AppError::DependencyNotFound {
                service: format!("Failed to resolve service: {}", e),
            })
    }

    /// Resolve a service instance within a scope
    pub async fn resolve_service_scoped<T: 'static + Send + Sync>(
        &self,
        scope: &ServiceScope,
    ) -> Result<Arc<T>> {
        scope
            .resolve::<T>()
            .await
            .map_err(|e| AppError::DependencyNotFound {
                service: format!("Failed to resolve scoped service: {}", e),
            })
    }

    /// Create a decorated service with middleware
    pub async fn create_decorated_service<T: Service + 'static>(
        &self,
        service: Arc<T>,
        enable_logging: bool,
        retry_config: Option<RetryConfig>,
        circuit_breaker_config: Option<CircuitBreakerConfig>,
        enable_metrics: bool,
    ) -> ServiceDecorator<T> {
        let service_name = T::service_name(&*service).to_string();
        let mut decorator = ServiceDecorator::new(service);

        if enable_logging {
            decorator = decorator.with_logging();
        }

        if let Some(config) = retry_config {
            decorator = decorator.with_retry(config);
        }

        if let Some(config) = circuit_breaker_config {
            decorator = decorator.with_circuit_breaker(config);
        }

        if enable_metrics {
            decorator = decorator.with_metrics(service_name);
        }

        decorator
    }

    /// Execute operation in bulkhead pool
    pub async fn execute_in_bulkhead<F, T, Fut>(
        &self,
        service_name: &str,
        context: &crate::core::infrastructure::service_decorators::ServiceContext,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        self.bulkhead_manager
            .execute_in_pool(service_name, context, operation)
            .await
    }

    /// Update service status
    pub async fn update_service_status(
        &self,
        service_id: Uuid,
        status: ServiceStatus,
    ) -> Result<()> {
        let mut services = self.services.write().await;

        if let Some(service) = services.get_mut(&service_id) {
            service.status = status.clone();
            service.last_heartbeat = Some(chrono::Utc::now());
            info!("Updated service {} status to {:?}", service_id, status);
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Service not found: {}",
                service_id
            )))
        }
    }

    /// Get service by ID
    pub async fn get_service(&self, service_id: Uuid) -> Result<ServiceInfo> {
        let services = self.services.read().await;
        services
            .get(&service_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Service not found: {}", service_id)))
    }

    /// Get services by name
    pub async fn get_services_by_name(&self, name: &str) -> Result<Vec<ServiceInfo>> {
        let name_index = self.name_index.read().await;
        let services = self.services.read().await;

        if let Some(service_ids) = name_index.get(name) {
            let mut result = Vec::new();
            for &service_id in service_ids {
                if let Some(service) = services.get(&service_id) {
                    result.push(service.clone());
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    /// List all services
    pub async fn list_services(&self) -> Result<Vec<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services.values().cloned().collect())
    }

    /// Remove service from registry
    pub async fn deregister_service(&self, service_id: Uuid) -> Result<()> {
        let service_name = {
            let services = self.services.read().await;
            services.get(&service_id).map(|s| s.name.clone())
        };

        if let Some(name) = service_name {
            // Remove from services map
            {
                let mut services = self.services.write().await;
                services.remove(&service_id);
            }

            // Update name index
            {
                let mut name_index = self.name_index.write().await;
                if let Some(service_ids) = name_index.get_mut(&name) {
                    service_ids.retain(|&id| id != service_id);
                    if service_ids.is_empty() {
                        name_index.remove(&name);
                    }
                }
            }

            info!("Deregistered service: {} ({})", name, service_id);
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Service not found: {}",
                service_id
            )))
        }
    }

    /// Get service statistics including DI container stats
    pub async fn get_statistics(&self) -> ServiceRegistryStats {
        let services = self.services.read().await;
        let total_services = services.len();

        let mut status_counts = HashMap::new();
        let mut lifetime_counts = HashMap::new();

        for service in services.values() {
            *status_counts.entry(service.status.clone()).or_insert(0) += 1;
            *lifetime_counts.entry(service.lifetime.clone()).or_insert(0) += 1;
        }

        // Get DI container stats
        let di_services = self.container.list_services().unwrap_or_default();
        let total_di_services = di_services.len();

        // Get bulkhead stats
        let bulkhead_pools = self.bulkhead_manager.list_pools().await;
        let mut bulkhead_stats = HashMap::new();

        for pool_name in &bulkhead_pools {
            if let Ok(stats) = self.bulkhead_manager.get_pool_stats(pool_name).await {
                bulkhead_stats.insert(pool_name.clone(), stats);
            }
        }

        ServiceRegistryStats {
            total_services,
            status_counts,
            lifetime_counts,
            total_di_services,
            bulkhead_pools: bulkhead_pools.len(),
            bulkhead_stats,
        }
    }

    /// Perform health check on all services
    pub async fn health_check_all(&self) -> Result<Vec<ServiceHealthResult>> {
        let services = self.services.read().await;
        let mut health_statuses = Vec::new();

        for service in services.values() {
            let health_status = if let Some(health_url) = &service.health_check_url {
                match self.check_service_health(health_url).await {
                    Ok(healthy) => {
                        if healthy {
                            ServiceHealthStatus::Healthy
                        } else {
                            ServiceHealthStatus::Unhealthy
                        }
                    }
                    Err(_) => ServiceHealthStatus::Unreachable,
                }
            } else {
                ServiceHealthStatus::Unknown
            };

            health_statuses.push(ServiceHealthResult {
                service_id: service.id,
                service_name: service.name.clone(),
                status: health_status,
                checked_at: chrono::Utc::now(),
            });
        }

        Ok(health_statuses)
    }

    /// Create a new service scope
    pub fn create_scope(&self) -> ServiceScope {
        ServiceScope::new(self.container.clone())
    }

    /// Clean up expired scoped instances
    pub async fn cleanup_scoped_instances(&self) -> Result<()> {
        self.container.cleanup_scoped_instances()
    }

    /// Get dependency injection container
    pub fn get_container(&self) -> Arc<ServiceContainer> {
        self.container.clone()
    }

    /// Get bulkhead manager
    pub fn get_bulkhead_manager(&self) -> Arc<BulkheadManager> {
        self.bulkhead_manager.clone()
    }

    async fn check_service_health(&self, health_url: &str) -> Result<bool> {
        // Simple HTTP health check
        match reqwest::get(health_url).await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                warn!("Health check failed for {}: {}", health_url, e);
                Ok(false)
            }
        }
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceRegistryStats {
    pub total_services: usize,
    pub status_counts: HashMap<ServiceStatus, usize>,
    pub lifetime_counts: HashMap<ServiceLifetime, usize>,
    pub total_di_services: usize,
    pub bulkhead_pools: usize,
    pub bulkhead_stats: HashMap<String, crate::core::infrastructure::resilience::BulkheadStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceHealthResult {
    pub service_id: Uuid,
    pub service_name: String,
    pub status: ServiceHealthStatus,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ServiceHealthStatus {
    Healthy,
    Unhealthy,
    Unreachable,
    Unknown,
}

/// Example service factory implementation
#[derive(Default)]
pub struct ExampleServiceFactory<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ExampleServiceFactory<T> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T: Send + Sync + Default + 'static> ServiceFactory for ExampleServiceFactory<T> {
    async fn create(
        &self,
        _container: &ServiceContainer,
    ) -> Result<Box<dyn std::any::Any + Send + Sync>> {
        Ok(Box::new(T::default()))
    }

    fn service_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn clone_factory(&self) -> Box<dyn ServiceFactory> {
        Box::new(ExampleServiceFactory::<T>::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Service;
    use crate::core::infrastructure::service_decorators::ServiceContext;

    struct TestService {
        name: String,
    }

    impl TestService {
        fn new(name: String) -> Self {
            Self { name }
        }
    }

    impl Default for TestService {
        fn default() -> Self {
            Self::new("test".to_string())
        }
    }

    #[async_trait]
    impl Service for TestService {
        fn service_name(&self) -> &'static str {
            "TestService"
        }
    }

    #[tokio::test]
    async fn test_enhanced_service_registration() {
        let registry = ServiceRegistry::new();

        let service_id = registry
            .register_service(
                "test-service".to_string(),
                "1.0.0".to_string(),
                "http://localhost:8080".to_string(),
                Some("http://localhost:8080/health".to_string()),
                HashMap::new(),
                "TestService".to_string(),
                ServiceLifetime::Singleton,
                vec![],
            )
            .await
            .unwrap();

        let service = registry.get_service(service_id).await.unwrap();
        assert_eq!(service.name, "test-service");
        assert_eq!(service.version, "1.0.0");
        assert_eq!(service.status, ServiceStatus::Starting);
        assert_eq!(service.lifetime, ServiceLifetime::Singleton);
    }

    #[tokio::test]
    async fn test_service_factory_registration() {
        let registry = ServiceRegistry::new();

        let factory = Box::new(ExampleServiceFactory::<TestService>::new());

        registry
            .register_service_factory::<TestService>(
                "test-service".to_string(),
                ServiceLifetime::Singleton,
                factory,
                vec![],
            )
            .await
            .unwrap();

        // Test resolution
        let service = registry.resolve_service::<TestService>().await.unwrap();
        assert_eq!(service.service_name(), "TestService");
    }

    #[tokio::test]
    async fn test_decorated_service_creation() {
        let registry = ServiceRegistry::new();
        let service = Arc::new(TestService::new("decorated".to_string()));

        let decorated = registry
            .create_decorated_service(
                service,
                true, // enable logging
                Some(RetryConfig::default()),
                Some(CircuitBreakerConfig::default()),
                true, // enable metrics
            )
            .await;

        assert_eq!(decorated.service_name(), "TestService");
    }

    #[tokio::test]
    async fn test_bulkhead_execution() {
        let registry = ServiceRegistry::new();

        // Create bulkhead pool
        registry
            .bulkhead_manager
            .create_pool("test-pool".to_string(), BulkheadConfig::default())
            .await
            .unwrap();

        let context = ServiceContext::new("test_operation".to_string());
        let result = registry
            .execute_in_bulkhead("test-pool", &context, || async {
                Ok("success".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_service_scope() {
        let registry = ServiceRegistry::new();
        let factory = Box::new(ExampleServiceFactory::<TestService>::new());

        registry
            .register_service_factory::<TestService>(
                "scoped-service".to_string(),
                ServiceLifetime::Scoped,
                factory,
                vec![],
            )
            .await
            .unwrap();

        let scope = registry.create_scope();
        let service = registry
            .resolve_service_scoped::<TestService>(&scope)
            .await
            .unwrap();
        assert_eq!(service.service_name(), "TestService");
    }
}
