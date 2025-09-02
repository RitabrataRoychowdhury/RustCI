//! Dependency Injection
//!
//! Provides dependency injection capabilities for managing component
//! dependencies and ensuring proper initialization order.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::error::{AppError, Result};

/// Dependency injection container
pub struct DependencyInjector {
    services: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    factories: Arc<RwLock<HashMap<TypeId, Box<dyn ServiceFactory + Send + Sync>>>>,
    singletons: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
}

/// Factory trait for creating services
pub trait ServiceFactory {
    fn create(&self) -> Result<Box<dyn Any + Send + Sync>>;
}

/// Service registration trait
pub trait ServiceRegistration<T> {
    fn register_singleton(&mut self, instance: Arc<T>) -> Result<()>;
    fn register_transient<F>(&mut self, factory: F) -> Result<()>
    where
        F: Fn() -> Result<T> + Send + Sync + 'static,
        T: Send + Sync + 'static;
    fn resolve(&self) -> Result<Arc<T>>;
}

impl DependencyInjector {
    /// Create a new dependency injector
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            factories: Arc::new(RwLock::new(HashMap::new())),
            singletons: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a singleton service
    pub async fn register_singleton<T>(&self, instance: Arc<T>) -> Result<()>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let mut singletons = self.singletons.write().await;
        singletons.insert(type_id, instance);
        Ok(())
    }
    
    /// Register a transient service with factory
    pub async fn register_transient<T, F>(&self, factory: F) -> Result<()>
    where
        T: Send + Sync + 'static,
        F: Fn() -> Result<T> + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let factory_wrapper = TransientFactory::new(factory);
        let mut factories = self.factories.write().await;
        factories.insert(type_id, Box::new(factory_wrapper));
        Ok(())
    }
    
    /// Resolve a service instance
    pub async fn resolve<T>(&self) -> Result<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        
        // Check singletons first
        {
            let singletons = self.singletons.read().await;
            if let Some(instance) = singletons.get(&type_id) {
                return instance
                    .clone()
                    .downcast::<T>()
                    .map_err(|_| AppError::DependencyInjection(
                        "Failed to downcast singleton service".to_string()
                    ));
            }
        }
        
        // Check factories
        {
            let factories = self.factories.read().await;
            if let Some(factory) = factories.get(&type_id) {
                let instance = factory.create()?;
                let arc_instance = Arc::new(
                    *instance
                        .downcast::<T>()
                        .map_err(|_| AppError::DependencyInjection(
                            "Failed to downcast factory service".to_string()
                        ))?
                );
                return Ok(arc_instance);
            }
        }
        
        Err(AppError::DependencyInjection(
            format!("Service not registered: {}", std::any::type_name::<T>())
        ))
    }
    
    /// Check if a service is registered
    pub async fn is_registered<T>(&self) -> bool
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        
        let singletons = self.singletons.read().await;
        if singletons.contains_key(&type_id) {
            return true;
        }
        
        let factories = self.factories.read().await;
        factories.contains_key(&type_id)
    }
    
    /// Get all registered service types
    pub async fn get_registered_types(&self) -> Vec<String> {
        let mut types = Vec::new();
        
        {
            let singletons = self.singletons.read().await;
            for type_id in singletons.keys() {
                types.push(format!("Singleton: {:?}", type_id));
            }
        }
        
        {
            let factories = self.factories.read().await;
            for type_id in factories.keys() {
                types.push(format!("Transient: {:?}", type_id));
            }
        }
        
        types
    }
    
    /// Clear all registrations
    pub async fn clear(&self) {
        {
            let mut singletons = self.singletons.write().await;
            singletons.clear();
        }
        
        {
            let mut factories = self.factories.write().await;
            factories.clear();
        }
        
        {
            let mut services = self.services.write().await;
            services.clear();
        }
    }
    
    /// Get registration statistics
    pub async fn get_statistics(&self) -> DependencyStatistics {
        let singletons_count = {
            let singletons = self.singletons.read().await;
            singletons.len()
        };
        
        let factories_count = {
            let factories = self.factories.read().await;
            factories.len()
        };
        
        DependencyStatistics {
            singleton_count: singletons_count,
            factory_count: factories_count,
            total_registrations: singletons_count + factories_count,
        }
    }
}

/// Transient factory wrapper
struct TransientFactory<T, F>
where
    T: Send + Sync + 'static,
    F: Fn() -> Result<T> + Send + Sync + 'static,
{
    factory: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> TransientFactory<T, F>
where
    T: Send + Sync + 'static,
    F: Fn() -> Result<T> + Send + Sync + 'static,
{
    fn new(factory: F) -> Self {
        Self {
            factory,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> ServiceFactory for TransientFactory<T, F>
where
    T: Send + Sync + 'static,
    F: Fn() -> Result<T> + Send + Sync + 'static,
{
    fn create(&self) -> Result<Box<dyn Any + Send + Sync>> {
        let instance = (self.factory)()?;
        Ok(Box::new(instance))
    }
}

/// Dependency injection statistics
#[derive(Debug, Clone)]
pub struct DependencyStatistics {
    pub singleton_count: usize,
    pub factory_count: usize,
    pub total_registrations: usize,
}

/// Dependency injection builder for fluent API
pub struct DependencyBuilder {
    injector: DependencyInjector,
}

impl DependencyBuilder {
    /// Create a new dependency builder
    pub fn new() -> Self {
        Self {
            injector: DependencyInjector::new(),
        }
    }
    
    /// Add a singleton service
    pub async fn add_singleton<T>(self, instance: Arc<T>) -> Result<Self>
    where
        T: Send + Sync + 'static,
    {
        self.injector.register_singleton(instance).await?;
        Ok(self)
    }
    
    /// Add a transient service
    pub async fn add_transient<T, F>(self, factory: F) -> Result<Self>
    where
        T: Send + Sync + 'static,
        F: Fn() -> Result<T> + Send + Sync + 'static,
    {
        self.injector.register_transient(factory).await?;
        Ok(self)
    }
    
    /// Build the dependency injector
    pub fn build(self) -> DependencyInjector {
        self.injector
    }
}

impl Default for DependencyInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DependencyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Macro for easier service registration
#[macro_export]
macro_rules! register_service {
    ($injector:expr, singleton, $service:expr) => {
        $injector.register_singleton(std::sync::Arc::new($service)).await?
    };
    ($injector:expr, transient, $factory:expr) => {
        $injector.register_transient($factory).await?
    };
}

/// Macro for easier service resolution
#[macro_export]
macro_rules! resolve_service {
    ($injector:expr, $service_type:ty) => {
        $injector.resolve::<$service_type>().await?
    };
}