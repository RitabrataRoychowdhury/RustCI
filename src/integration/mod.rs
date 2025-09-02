//! System Integration Module
//!
//! This module provides comprehensive integration and wiring of all production-grade
//! components, including dependency injection, lifecycle management, and system-wide
//! coordination.

pub mod component_registry;
pub mod dependency_injection;
pub mod lifecycle_manager;
pub mod system_coordinator;

pub use component_registry::ComponentRegistry;
pub use dependency_injection::DependencyInjector;
pub use lifecycle_manager::LifecycleManager;
pub use system_coordinator::SystemCoordinator;

use crate::error::Result;
use std::sync::Arc;

/// Production-grade application state with all enhanced components
#[derive(Clone)]
pub struct ProductionAppState {
    // Core infrastructure
    pub config_manager: Arc<crate::config::ProductionConfigManager>,
    pub database_manager: Arc<crate::infrastructure::database::ProductionDatabaseManager>,
    pub error_handler: Arc<dyn crate::error::handler::ProductionErrorHandler>,
    
    // Security components
    pub security_manager: Arc<crate::core::security::ProductionSecurityManager>,
    pub mfa_provider: Arc<crate::core::security::MultiFactorAuthProvider>,
    pub audit_logger: Arc<crate::core::observability::audit::EnhancedAuditLogger>,
    
    // Performance components
    pub performance_monitor: Arc<dyn crate::core::performance::PerformanceMonitor>,
    pub resource_manager: Arc<dyn crate::deployment::ResourceManager>,
    pub cache_manager: Arc<crate::core::performance::cache_manager::MultiTierCacheManager<String, String>>,
    pub load_balancer: Arc<dyn crate::core::performance::load_balancer::LoadBalancer>,
    
    // Storage components
    pub storage_factory: Arc<crate::storage::factory::StoreFactory>,
    pub hybrid_store_manager: Arc<crate::storage::factory::HybridStoreManager>,
    
    // Testing components
    pub test_suite: Arc<crate::testing::production_test_suite::ProductionTestSuite>,
    pub integration_test_manager: Arc<crate::testing::integration_test_manager::IntegrationTestManager>,
    
    // Deployment components
    pub blue_green_manager: Arc<dyn crate::deployment::BlueGreenDeploymentManager>,
    pub circuit_breaker: Arc<dyn crate::deployment::CircuitBreaker>,
    pub auto_scaler: Arc<crate::deployment::ProductionAutoScaler>,
    
    // Observability components
    pub structured_logging: Arc<crate::core::observability::structured_logging::StructuredLogging>,
    pub distributed_tracing: Arc<crate::core::observability::distributed_tracing::DistributedTracing>,
    pub prometheus_metrics: Arc<crate::core::observability::prometheus_metrics::PrometheusMetrics>,
    pub health_checks: Arc<crate::core::observability::health_checks::HealthCheckSystem>,
    pub alerting: Arc<crate::core::observability::alerting::AlertingSystem>,
    
    // API components
    pub api_versioning: Arc<crate::api::versioning::ApiVersionManager>,
    pub rate_limiter: Arc<crate::api::rate_limiting::RateLimitService>,
    pub response_optimizer: Arc<crate::api::response_optimization::ResponseOptimizationService>,
    
    // Legacy compatibility
    pub legacy_state: Arc<crate::AppState>,
}

impl ProductionAppState {
    /// Create a new production application state with all components initialized
    pub async fn new(config: crate::config::AppConfiguration) -> Result<Self> {
        let system_coordinator = SystemCoordinator::new();
        system_coordinator.initialize_all_components(config).await
    }
    
    /// Get the legacy app state for backward compatibility
    pub fn legacy(&self) -> Arc<crate::AppState> {
        self.legacy_state.clone()
    }
    
    /// Perform health check on all components
    pub async fn health_check(&self) -> SystemHealthReport {
        SystemCoordinator::perform_system_health_check(self).await
    }
    
    /// Gracefully shutdown all components
    pub async fn shutdown(&self) -> Result<()> {
        SystemCoordinator::graceful_shutdown(self).await
    }
}

/// System-wide health report
#[derive(Debug, Clone)]
pub struct SystemHealthReport {
    pub overall_status: SystemHealthStatus,
    pub component_statuses: std::collections::HashMap<String, ComponentHealthStatus>,
    pub performance_metrics: SystemPerformanceMetrics,
    pub error_summary: SystemErrorSummary,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SystemHealthStatus {
    Healthy,
    Degraded,
    Critical,
    Offline,
}

#[derive(Debug, Clone)]
pub struct ComponentHealthStatus {
    pub name: String,
    pub status: SystemHealthStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub error_count: u64,
    pub performance_score: f64,
    pub details: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SystemPerformanceMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_throughput: f64,
    pub request_latency_p99: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone)]
pub struct SystemErrorSummary {
    pub total_errors: u64,
    pub critical_errors: u64,
    pub error_rate_per_minute: f64,
    pub top_error_types: Vec<(String, u64)>,
}