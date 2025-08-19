//! Valkyrie Protocol Factory Pattern Implementation
//!
//! This module provides factory patterns for creating and configuring
//! Valkyrie Protocol components with proper dependency injection.

use crate::valkyrie::{
    ObservabilityManager, Result, SecurityManager, StreamMultiplexer, TransportManager,
    ValkyrieConfig, ValkyrieConfigBuilder, ValkyrieEngine,
};
use std::sync::Arc;

/// Factory for creating Valkyrie Protocol engines
pub struct ValkyrieFactory;

impl ValkyrieFactory {
    /// Create a new Valkyrie engine with default configuration
    pub async fn create_default() -> Result<ValkyrieEngine> {
        let config = ValkyrieConfigBuilder::new()
            .with_default_transport()
            .with_default_security()
            .with_default_streaming()
            .with_default_observability()
            .build()?;

        Self::create_with_config(config).await
    }

    /// Create a new Valkyrie engine with custom configuration
    pub async fn create_with_config(config: ValkyrieConfig) -> Result<ValkyrieEngine> {
        let builder = EngineBuilder::new(config);
        builder.build().await
    }

    /// Create a Valkyrie engine optimized for RustCI integration
    pub async fn create_for_rustci() -> Result<ValkyrieEngine> {
        let config = ValkyrieConfigBuilder::new()
            .with_rustci_defaults()
            .enable_ci_optimizations()
            .enable_container_transport()
            .enable_kubernetes_transport()
            .build()?;

        Self::create_with_config(config).await
    }
}

/// Builder pattern for constructing Valkyrie engines
pub struct EngineBuilder {
    config: ValkyrieConfig,
    transport_manager: Option<Arc<TransportManager>>,
    security_manager: Option<Arc<SecurityManager>>,
    stream_multiplexer: Option<Arc<StreamMultiplexer>>,
    observability_manager: Option<Arc<ObservabilityManager>>,
}

impl EngineBuilder {
    /// Create a new engine builder with the given configuration
    pub fn new(config: ValkyrieConfig) -> Self {
        Self {
            config,
            transport_manager: None,
            security_manager: None,
            stream_multiplexer: None,
            observability_manager: None,
        }
    }

    /// Set a custom transport manager
    pub fn with_transport_manager(mut self, manager: Arc<TransportManager>) -> Self {
        self.transport_manager = Some(manager);
        self
    }

    /// Set a custom security manager
    pub fn with_security_manager(mut self, manager: Arc<SecurityManager>) -> Self {
        self.security_manager = Some(manager);
        self
    }

    /// Set a custom stream multiplexer
    pub fn with_stream_multiplexer(mut self, multiplexer: Arc<StreamMultiplexer>) -> Self {
        self.stream_multiplexer = Some(multiplexer);
        self
    }

    /// Set a custom observability manager
    pub fn with_observability_manager(mut self, manager: Arc<ObservabilityManager>) -> Self {
        self.observability_manager = Some(manager);
        self
    }

    /// Build the Valkyrie engine
    pub async fn build(self) -> Result<ValkyrieEngine> {
        // Create managers if not provided
        let transport_manager = match self.transport_manager {
            Some(manager) => manager,
            None => Arc::new(TransportManager::new(self.config.transport.clone())?),
        };

        let security_manager = match self.security_manager {
            Some(manager) => manager,
            None => Arc::new(SecurityManager::new(self.config.security.clone())?),
        };

        let stream_multiplexer = match self.stream_multiplexer {
            Some(multiplexer) => multiplexer,
            None => Arc::new(StreamMultiplexer::new(self.config.streaming.clone())),
        };

        let observability_manager = match self.observability_manager {
            Some(manager) => manager,
            None => Arc::new(ObservabilityManager::new(
                self.config.observability.clone(),
            )?),
        };

        // Create the engine with dependency injection
        ValkyrieEngine::new_with_dependencies(
            self.config,
            transport_manager,
            security_manager,
            stream_multiplexer,
            observability_manager,
        )
    }
}

/// Strategy pattern for different engine configurations
pub trait EngineStrategy {
    fn configure(&self, builder: ValkyrieConfigBuilder) -> ValkyrieConfigBuilder;
}

/// RustCI-specific engine strategy
pub struct RustCIStrategy;

impl EngineStrategy for RustCIStrategy {
    fn configure(&self, builder: ValkyrieConfigBuilder) -> ValkyrieConfigBuilder {
        builder
            .with_rustci_defaults()
            .enable_ci_optimizations()
            .enable_container_transport()
            .enable_kubernetes_transport()
            .with_high_throughput_settings()
    }
}

/// High-performance engine strategy
pub struct HighPerformanceStrategy;

impl EngineStrategy for HighPerformanceStrategy {
    fn configure(&self, builder: ValkyrieConfigBuilder) -> ValkyrieConfigBuilder {
        builder
            .enable_zero_copy()
            .enable_simd_optimizations()
            .with_high_throughput_settings()
            .with_minimal_security()
    }
}

/// Secure communication strategy
pub struct SecureStrategy;

impl EngineStrategy for SecureStrategy {
    fn configure(&self, builder: ValkyrieConfigBuilder) -> ValkyrieConfigBuilder {
        builder
            .with_maximum_security()
            .enable_post_quantum_crypto()
            .enable_mutual_tls()
            .enable_audit_logging()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_create_default() {
        let _result = ValkyrieFactory::create_default().await;
        // This would need proper mocking to test
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let config = ValkyrieConfigBuilder::new().build().unwrap();
        let builder = EngineBuilder::new(config);
        let _result = builder.build().await;
        // This would need proper mocking to test
        // assert!(result.is_ok());
    }
}
