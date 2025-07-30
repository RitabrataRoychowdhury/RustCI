//! Integration module for the modular CI Engine
//!
//! This module provides factory functions and integration utilities
//! to wire together all the components of the modular CI engine.

use std::path::PathBuf;
use std::sync::Arc;

use crate::ci::{
    connectors::ConnectorManager, executor::PipelineExecutor, workspace::WorkspaceManager,
};
use crate::core::{
    events::EventBus,
    sagas::{SagaOrchestrator, SagaPersistence},
    CorrelationTracker,
};
use crate::error::Result;
use crate::infrastructure::database::DatabaseManager;

use super::{
    CIEngineOrchestrator, ExecutionCoordinator, ExecutionMonitoring, ExecutionStrategyFactory,
    MetricsCollector, ParallelExecutionStrategy, PipelineExecutionSagaFactory, PipelineManager,
    ResourceLimits, SequentialExecutionStrategy,
};

/// Configuration for the CI Engine
#[derive(Debug, Clone)]
pub struct CIEngineConfig {
    pub max_concurrent_executions: usize,
    pub workspace_base_path: PathBuf,
    pub cache_directory: PathBuf,
    pub deployment_directory: PathBuf,
    pub resource_limits: ResourceLimits,
    pub metrics_history_size: usize,
}

impl Default for CIEngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 10,
            workspace_base_path: PathBuf::from("/tmp/ci-workspaces"),
            cache_directory: PathBuf::from("/tmp/ci-cache"),
            deployment_directory: PathBuf::from("/tmp/ci-deployments"),
            resource_limits: ResourceLimits::default(),
            metrics_history_size: 1000,
        }
    }
}

/// Factory for creating a fully configured CI Engine
#[allow(dead_code)]
pub struct CIEngineFactory {
    config: CIEngineConfig,
    database: Arc<DatabaseManager>,
    correlation_tracker: Arc<CorrelationTracker>,
    event_bus: Arc<EventBus>,
    saga_persistence: Arc<dyn SagaPersistence>,
}

impl CIEngineFactory {
    /// Create a new CI Engine factory
    pub fn new(
        config: CIEngineConfig,
        database: Arc<DatabaseManager>,
        correlation_tracker: Arc<CorrelationTracker>,
        event_bus: Arc<EventBus>,
        saga_persistence: Arc<dyn SagaPersistence>,
    ) -> Self {
        Self {
            config,
            database,
            correlation_tracker,
            event_bus,
            saga_persistence,
        }
    }

    /// Create a fully configured CI Engine Orchestrator
    pub async fn create_engine(&self) -> Result<Arc<CIEngineOrchestrator>> {
        // Create workspace manager
        let workspace_manager = Arc::new(WorkspaceManager::new(
            self.config.workspace_base_path.clone(),
        ));

        // Create connector manager
        let connector_manager = ConnectorManager::new();

        // Create pipeline executor
        let executor = Arc::new(PipelineExecutor::new(
            connector_manager,
            self.config.cache_directory.clone(),
            self.config.deployment_directory.clone(),
        ));

        // Create pipeline manager
        let pipeline_manager = Arc::new(PipelineManager::new(
            (*self.database).clone(),
            self.event_bus.clone(),
        ));

        // Create execution coordinator with resource limits
        let execution_coordinator = Arc::new(ExecutionCoordinator::with_resource_limits(
            self.correlation_tracker.clone(),
            self.config.max_concurrent_executions,
            self.config.resource_limits.clone(),
        ));

        // Create monitoring system
        let monitoring = Arc::new(ExecutionMonitoring::new(self.correlation_tracker.clone()));

        // Create metrics collector
        let metrics_collector = Arc::new(MetricsCollector::new());

        // Create and configure execution strategy factory
        let mut strategy_factory = ExecutionStrategyFactory::new();

        // Register sequential strategy
        let sequential_strategy =
            SequentialExecutionStrategy::new(executor.clone(), workspace_manager.clone());
        strategy_factory.register_strategy(Box::new(sequential_strategy));

        // Register parallel strategy
        let parallel_strategy = ParallelExecutionStrategy::new(
            executor.clone(),
            workspace_manager.clone(),
            4, // Max 4 parallel stages
        );
        strategy_factory.register_strategy(Box::new(parallel_strategy));

        // Create SAGA factory
        let saga_factory = Arc::new(PipelineExecutionSagaFactory::new(
            executor.clone(),
            workspace_manager.clone(),
            monitoring.clone(),
            metrics_collector.clone(),
        ));

        // Create SAGA orchestrator
        let saga_orchestrator = Arc::new(SagaOrchestrator::new(
            "pipeline-execution".to_string(),
            self.event_bus.clone(),
            self.correlation_tracker.clone(),
            self.saga_persistence.clone(),
        ));

        // Create the main orchestrator
        let orchestrator = Arc::new(CIEngineOrchestrator::new(
            pipeline_manager,
            execution_coordinator,
            self.event_bus.clone(),
            saga_orchestrator,
            saga_factory,
            monitoring,
        ));

        Ok(orchestrator)
    }

    /// Create individual components for testing or custom configurations
    pub async fn create_components(&self) -> Result<CIEngineComponents> {
        let workspace_manager = Arc::new(WorkspaceManager::new(
            self.config.workspace_base_path.clone(),
        ));

        let connector_manager = ConnectorManager::new();

        let executor = Arc::new(PipelineExecutor::new(
            connector_manager,
            self.config.cache_directory.clone(),
            self.config.deployment_directory.clone(),
        ));

        let pipeline_manager = Arc::new(PipelineManager::new(
            (*self.database).clone(),
            self.event_bus.clone(),
        ));

        let execution_coordinator = Arc::new(ExecutionCoordinator::with_resource_limits(
            self.correlation_tracker.clone(),
            self.config.max_concurrent_executions,
            self.config.resource_limits.clone(),
        ));

        let monitoring = Arc::new(ExecutionMonitoring::new(self.correlation_tracker.clone()));

        let metrics_collector = Arc::new(MetricsCollector::new());

        let saga_orchestrator = Arc::new(SagaOrchestrator::new(
            "pipeline-execution".to_string(),
            self.event_bus.clone(),
            self.correlation_tracker.clone(),
            self.saga_persistence.clone(),
        ));

        Ok(CIEngineComponents {
            workspace_manager,
            executor,
            pipeline_manager,
            execution_coordinator,
            monitoring,
            metrics_collector,
            saga_orchestrator,
        })
    }
}

/// Individual components of the CI Engine for testing or custom configurations
#[allow(dead_code)]
pub struct CIEngineComponents {
    pub workspace_manager: Arc<WorkspaceManager>,
    pub executor: Arc<PipelineExecutor>,
    pub pipeline_manager: Arc<PipelineManager>,
    pub execution_coordinator: Arc<ExecutionCoordinator>,
    pub monitoring: Arc<ExecutionMonitoring>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub saga_orchestrator: Arc<SagaOrchestrator>,
}

/// Builder for creating CI Engine with custom configuration
pub struct CIEngineBuilder {
    config: CIEngineConfig,
    database: Option<Arc<DatabaseManager>>,
    correlation_tracker: Option<Arc<CorrelationTracker>>,
    event_bus: Option<Arc<EventBus>>,
    saga_persistence: Option<Arc<dyn SagaPersistence>>,
}

impl CIEngineBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: CIEngineConfig::default(),
            database: None,
            correlation_tracker: None,
            event_bus: None,
            saga_persistence: None,
        }
    }

    /// Set the configuration
    pub fn with_config(mut self, config: CIEngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the database manager
    pub fn with_database(mut self, database: Arc<DatabaseManager>) -> Self {
        self.database = Some(database);
        self
    }

    /// Set the correlation tracker
    pub fn with_correlation_tracker(mut self, tracker: Arc<CorrelationTracker>) -> Self {
        self.correlation_tracker = Some(tracker);
        self
    }

    /// Set the event bus
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Set the SAGA persistence
    pub fn with_saga_persistence(mut self, persistence: Arc<dyn SagaPersistence>) -> Self {
        self.saga_persistence = Some(persistence);
        self
    }

    /// Set max concurrent executions
    pub fn with_max_concurrent_executions(mut self, max: usize) -> Self {
        self.config.max_concurrent_executions = max;
        self
    }

    /// Set workspace base path
    pub fn with_workspace_path(mut self, path: PathBuf) -> Self {
        self.config.workspace_base_path = path;
        self
    }

    /// Set resource limits
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.config.resource_limits = limits;
        self
    }

    /// Build the CI Engine
    pub async fn build(self) -> Result<Arc<CIEngineOrchestrator>> {
        let database = self.database.ok_or_else(|| {
            crate::error::AppError::ValidationError("Database manager is required".to_string())
        })?;

        let correlation_tracker = self.correlation_tracker.ok_or_else(|| {
            crate::error::AppError::ValidationError("Correlation tracker is required".to_string())
        })?;

        let event_bus = self.event_bus.ok_or_else(|| {
            crate::error::AppError::ValidationError("Event bus is required".to_string())
        })?;

        let saga_persistence = self.saga_persistence.ok_or_else(|| {
            crate::error::AppError::ValidationError("SAGA persistence is required".to_string())
        })?;

        let factory = CIEngineFactory::new(
            self.config,
            database,
            correlation_tracker,
            event_bus,
            saga_persistence,
        );

        factory.create_engine().await
    }
}

impl Default for CIEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sagas::{SagaExecution, SagaStatistics, SagaStatus};
    use uuid::Uuid;

    // Mock SAGA persistence for testing
    struct MockSagaPersistence;

    #[async_trait::async_trait]
    impl SagaPersistence for MockSagaPersistence {
        async fn save_execution(&self, _execution: &SagaExecution) -> Result<()> {
            Ok(())
        }

        async fn load_execution(&self, _saga_id: Uuid) -> Result<Option<SagaExecution>> {
            Ok(None)
        }

        async fn find_by_status(&self, _status: SagaStatus) -> Result<Vec<SagaExecution>> {
            Ok(Vec::new())
        }

        async fn find_by_correlation_id(
            &self,
            _correlation_id: Uuid,
        ) -> Result<Vec<SagaExecution>> {
            Ok(Vec::new())
        }

        async fn delete_execution(&self, _saga_id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn get_statistics(&self) -> Result<SagaStatistics> {
            Ok(SagaStatistics {
                total_executions: 0,
                completed_executions: 0,
                failed_executions: 0,
                compensated_executions: 0,
                average_duration_ms: 0.0,
                success_rate: 0.0,
            })
        }
    }

    #[tokio::test]
    async fn test_ci_engine_builder() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker.clone(), None));
        let database = Arc::new(
            DatabaseManager::new("mongodb://localhost:27017", "test")
                .await
                .unwrap(),
        );
        let saga_persistence = Arc::new(MockSagaPersistence);

        let result = CIEngineBuilder::new()
            .with_database(database)
            .with_correlation_tracker(correlation_tracker)
            .with_event_bus(event_bus)
            .with_saga_persistence(saga_persistence)
            .with_max_concurrent_executions(5)
            .build()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ci_engine_factory() {
        let correlation_tracker = Arc::new(CorrelationTracker::new());
        let event_bus = Arc::new(EventBus::new(correlation_tracker.clone(), None));
        let database = Arc::new(
            DatabaseManager::new("mongodb://localhost:27017", "test")
                .await
                .unwrap(),
        );
        let saga_persistence = Arc::new(MockSagaPersistence);

        let factory = CIEngineFactory::new(
            CIEngineConfig::default(),
            database,
            correlation_tracker,
            event_bus,
            saga_persistence,
        );

        let result = factory.create_engine().await;
        assert!(result.is_ok());

        let components_result = factory.create_components().await;
        assert!(components_result.is_ok());
    }

    #[test]
    fn test_ci_engine_config_default() {
        let config = CIEngineConfig::default();
        assert_eq!(config.max_concurrent_executions, 10);
        assert_eq!(
            config.workspace_base_path,
            PathBuf::from("/tmp/ci-workspaces")
        );
        assert_eq!(config.cache_directory, PathBuf::from("/tmp/ci-cache"));
        assert_eq!(
            config.deployment_directory,
            PathBuf::from("/tmp/ci-deployments")
        );
    }
}
