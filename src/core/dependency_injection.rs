use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Service lifetime management
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ServiceLifetime {
    /// Single instance shared across the application
    Singleton,
    /// New instance per request/scope
    Transient,
    /// Single instance per scope (e.g., per HTTP request)
    Scoped,
}

/// Service registration information
pub struct ServiceRegistration {
    pub service_type: TypeId,
    pub lifetime: ServiceLifetime,
    pub factory: Box<dyn ServiceFactory>,
    pub dependencies: Vec<TypeId>,
}

/// Factory trait for creating service instances
#[async_trait]
pub trait ServiceFactory: Send + Sync {
    async fn create(&self, container: &ServiceContainer) -> Result<Box<dyn Any + Send + Sync>>;
    fn service_name(&self) -> &'static str;
    fn clone_factory(&self) -> Box<dyn ServiceFactory>;
}

/// Dependency injection container
pub struct ServiceContainer {
    registrations: RwLock<HashMap<TypeId, ServiceRegistration>>,
    singletons: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    scoped_instances: RwLock<HashMap<(Uuid, TypeId), Weak<dyn Any + Send + Sync>>>,
    circular_detection: RwLock<Vec<TypeId>>,
}

impl ServiceContainer {
    pub fn new() -> Self {
        Self {
            registrations: RwLock::new(HashMap::new()),
            singletons: RwLock::new(HashMap::new()),
            scoped_instances: RwLock::new(HashMap::new()),
            circular_detection: RwLock::new(Vec::new()),
        }
    }

    /// Register a service with the container
    pub fn register<T: 'static + Send + Sync>(
        &self,
        lifetime: ServiceLifetime,
        factory: Box<dyn ServiceFactory>,
        dependencies: Vec<TypeId>,
    ) -> Result<()> {
        let service_type = TypeId::of::<T>();

        debug!(
            "Registering service: {} with lifetime: {:?}",
            factory.service_name(),
            lifetime
        );

        let registration = ServiceRegistration {
            service_type,
            lifetime,
            factory,
            dependencies,
        };

        self.registrations
            .write()
            .map_err(|_| AppError::InternalServerError("Failed to acquire write lock".to_string()))?
            .insert(service_type, registration);

        Ok(())
    }

    /// Resolve a service instance
    pub async fn resolve<T: 'static + Send + Sync>(&self) -> Result<Arc<T>> {
        let service_type = TypeId::of::<T>();
        self.resolve_internal(service_type, None).await
    }

    /// Resolve a service instance within a scope
    pub async fn resolve_scoped<T: 'static + Send + Sync>(&self, scope_id: Uuid) -> Result<Arc<T>> {
        let service_type = TypeId::of::<T>();
        self.resolve_internal(service_type, Some(scope_id)).await
    }

    async fn resolve_internal<T: 'static + Send + Sync>(
        &self,
        service_type: TypeId,
        scope_id: Option<Uuid>,
    ) -> Result<Arc<T>> {
        // Check for circular dependencies
        {
            let mut detection = self.circular_detection.write().map_err(|_| {
                AppError::InternalServerError(
                    "Failed to acquire circular detection lock".to_string(),
                )
            })?;

            if detection.contains(&service_type) {
                let cycle: Vec<String> = detection.iter().map(|id| format!("{:?}", id)).collect();
                return Err(AppError::DependencyNotFound {
                    service: format!("Circular dependency detected: {:?}", cycle),
                });
            }
            detection.push(service_type);
        }

        let result = self
            .resolve_service_internal::<T>(service_type, scope_id)
            .await;

        // Clean up circular detection
        {
            let mut detection = self.circular_detection.write().map_err(|_| {
                AppError::InternalServerError(
                    "Failed to acquire circular detection lock".to_string(),
                )
            })?;
            detection.retain(|&id| id != service_type);
        }

        result
    }

    async fn resolve_service_internal<T: 'static + Send + Sync>(
        &self,
        service_type: TypeId,
        scope_id: Option<Uuid>,
    ) -> Result<Arc<T>> {
        let registration = {
            let registrations = self.registrations.read().map_err(|_| {
                AppError::InternalServerError("Failed to acquire read lock".to_string())
            })?;

            let reg =
                registrations
                    .get(&service_type)
                    .ok_or_else(|| AppError::DependencyNotFound {
                        service: format!("Service not registered: {:?}", service_type),
                    })?;

            ServiceRegistration {
                service_type: reg.service_type,
                lifetime: reg.lifetime.clone(),
                factory: reg.factory.clone_factory(),
                dependencies: reg.dependencies.clone(),
            }
        };

        match registration.lifetime {
            ServiceLifetime::Singleton => {
                self.resolve_singleton::<T>(service_type, &registration)
                    .await
            }
            ServiceLifetime::Transient => self.resolve_transient::<T>(&registration).await,
            ServiceLifetime::Scoped => {
                let scope_id = scope_id.ok_or_else(|| AppError::DependencyNotFound {
                    service: "Scope ID required for scoped service".to_string(),
                })?;
                self.resolve_scoped_internal::<T>(service_type, scope_id, &registration)
                    .await
            }
        }
    }

    async fn resolve_singleton<T: 'static + Send + Sync>(
        &self,
        service_type: TypeId,
        registration: &ServiceRegistration,
    ) -> Result<Arc<T>> {
        // Check if singleton already exists
        {
            let singletons = self.singletons.read().map_err(|_| {
                AppError::InternalServerError("Failed to acquire singleton read lock".to_string())
            })?;

            if let Some(instance) = singletons.get(&service_type) {
                return instance.clone().downcast::<T>().map_err(|_| {
                    AppError::InternalServerError(
                        "Failed to downcast singleton service".to_string(),
                    )
                });
            }
        }

        // Create new singleton instance
        let instance = registration.factory.create(self).await?;
        let arc_instance: Arc<dyn std::any::Any + Send + Sync> = Arc::from(instance);

        // Store singleton
        {
            let mut singletons = self.singletons.write().map_err(|_| {
                AppError::InternalServerError("Failed to acquire singleton write lock".to_string())
            })?;
            singletons.insert(service_type, arc_instance.clone());
        }

        arc_instance.downcast::<T>().map_err(|_| {
            AppError::InternalServerError("Failed to downcast singleton service".to_string())
        })
    }

    async fn resolve_transient<T: 'static + Send + Sync>(
        &self,
        registration: &ServiceRegistration,
    ) -> Result<Arc<T>> {
        let instance = registration.factory.create(self).await?;
        let arc_instance: Arc<dyn std::any::Any + Send + Sync> = Arc::from(instance);
        arc_instance.downcast::<T>().map_err(|_| {
            AppError::InternalServerError("Failed to downcast transient service".to_string())
        })
    }

    async fn resolve_scoped_internal<T: 'static + Send + Sync>(
        &self,
        service_type: TypeId,
        scope_id: Uuid,
        registration: &ServiceRegistration,
    ) -> Result<Arc<T>> {
        let scope_key = (scope_id, service_type);

        // Check if scoped instance exists and is still valid
        {
            let scoped = self.scoped_instances.read().map_err(|_| {
                AppError::InternalServerError("Failed to acquire scoped read lock".to_string())
            })?;

            if let Some(weak_instance) = scoped.get(&scope_key) {
                if let Some(instance) = weak_instance.upgrade() {
                    return instance.downcast::<T>().map_err(|_| {
                        AppError::InternalServerError(
                            "Failed to downcast scoped service".to_string(),
                        )
                    });
                }
            }
        }

        // Create new scoped instance
        let instance = registration.factory.create(self).await?;
        let arc_instance = Arc::from(instance);

        // Store weak reference
        {
            let mut scoped = self.scoped_instances.write().map_err(|_| {
                AppError::InternalServerError("Failed to acquire scoped write lock".to_string())
            })?;
            scoped.insert(scope_key, Arc::downgrade(&arc_instance));
        }

        arc_instance.downcast::<T>().map_err(|_| {
            AppError::InternalServerError("Failed to downcast scoped service".to_string())
        })
    }

    /// Clean up expired scoped instances
    pub fn cleanup_scoped_instances(&self) -> Result<()> {
        let mut scoped = self.scoped_instances.write().map_err(|_| {
            AppError::InternalServerError("Failed to acquire scoped write lock".to_string())
        })?;

        let initial_count = scoped.len();
        scoped.retain(|_, weak_ref| weak_ref.strong_count() > 0);
        let cleaned_count = initial_count - scoped.len();

        if cleaned_count > 0 {
            debug!(
                "Cleaned up {} expired scoped service instances",
                cleaned_count
            );
        }

        Ok(())
    }

    /// Get service registration information
    pub fn get_registration_info(&self, service_type: TypeId) -> Result<Option<ServiceInfo>> {
        let registrations = self.registrations.read().map_err(|_| {
            AppError::InternalServerError("Failed to acquire read lock".to_string())
        })?;

        if let Some(registration) = registrations.get(&service_type) {
            Ok(Some(ServiceInfo {
                service_name: registration.factory.service_name().to_string(),
                lifetime: registration.lifetime.clone(),
                dependencies_count: registration.dependencies.len(),
            }))
        } else {
            Ok(None)
        }
    }

    /// List all registered services
    pub fn list_services(&self) -> Result<Vec<ServiceInfo>> {
        let registrations = self.registrations.read().map_err(|_| {
            AppError::InternalServerError("Failed to acquire read lock".to_string())
        })?;

        let services = registrations
            .values()
            .map(|reg| ServiceInfo {
                service_name: reg.factory.service_name().to_string(),
                lifetime: reg.lifetime.clone(),
                dependencies_count: reg.dependencies.len(),
            })
            .collect();

        Ok(services)
    }
}

impl Default for ServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Service information for diagnostics
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub service_name: String,
    pub lifetime: ServiceLifetime,
    pub dependencies_count: usize,
}

/// Service scope for managing scoped service lifetimes
pub struct ServiceScope {
    pub scope_id: Uuid,
    container: Arc<ServiceContainer>,
    created_at: Instant,
}

impl ServiceScope {
    pub fn new(container: Arc<ServiceContainer>) -> Self {
        Self {
            scope_id: Uuid::new_v4(),
            container,
            created_at: Instant::now(),
        }
    }

    pub async fn resolve<T: 'static + Send + Sync>(&self) -> Result<Arc<T>> {
        self.container.resolve_scoped::<T>(self.scope_id).await
    }

    pub fn duration(&self) -> Duration {
        self.created_at.elapsed()
    }
}

impl Drop for ServiceScope {
    fn drop(&mut self) {
        debug!(
            "Service scope {} dropped after {:?}",
            self.scope_id,
            self.duration()
        );

        // Clean up scoped instances when scope is dropped
        if let Err(e) = self.container.cleanup_scoped_instances() {
            warn!("Failed to cleanup scoped instances: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestService {
        id: usize,
    }

    struct TestServiceFactory {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ServiceFactory for TestServiceFactory {
        async fn create(
            &self,
            _container: &ServiceContainer,
        ) -> Result<Box<dyn Any + Send + Sync>> {
            let id = self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(TestService { id }))
        }

        fn service_name(&self) -> &'static str {
            "TestService"
        }

        fn clone_factory(&self) -> Box<dyn ServiceFactory> {
            Box::new(TestServiceFactory {
                counter: self.counter.clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_singleton_lifetime() {
        let container = ServiceContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));

        container
            .register::<TestService>(
                ServiceLifetime::Singleton,
                Box::new(TestServiceFactory {
                    counter: counter.clone(),
                }),
                vec![],
            )
            .unwrap();

        let instance1 = container.resolve::<TestService>().await.unwrap();
        let instance2 = container.resolve::<TestService>().await.unwrap();

        assert_eq!(instance1.id, instance2.id);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_transient_lifetime() {
        let container = ServiceContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));

        container
            .register::<TestService>(
                ServiceLifetime::Transient,
                Box::new(TestServiceFactory {
                    counter: counter.clone(),
                }),
                vec![],
            )
            .unwrap();

        let instance1 = container.resolve::<TestService>().await.unwrap();
        let instance2 = container.resolve::<TestService>().await.unwrap();

        assert_ne!(instance1.id, instance2.id);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_scoped_lifetime() {
        let container = Arc::new(ServiceContainer::new());
        let counter = Arc::new(AtomicUsize::new(0));

        container
            .register::<TestService>(
                ServiceLifetime::Scoped,
                Box::new(TestServiceFactory {
                    counter: counter.clone(),
                }),
                vec![],
            )
            .unwrap();

        let scope1 = ServiceScope::new(container.clone());
        let scope2 = ServiceScope::new(container.clone());

        let instance1a = scope1.resolve::<TestService>().await.unwrap();
        let instance1b = scope1.resolve::<TestService>().await.unwrap();
        let instance2 = scope2.resolve::<TestService>().await.unwrap();

        assert_eq!(instance1a.id, instance1b.id); // Same within scope
        assert_ne!(instance1a.id, instance2.id); // Different across scopes
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
