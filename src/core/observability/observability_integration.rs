use crate::error::Result;
use axum::Router;
use std::sync::Arc;
use tracing::{info, instrument};

use crate::core::cluster::{
    control_plane_health::{
        ControlPlaneHealth, ControlPlaneHealthResponse, DatabaseHealthCheck,
        JobSchedulerHealthCheck, NodeRegistryHealthCheck,
    },
    control_plane_metrics::{ControlPlaneMetrics, MetricsSnapshot},
    control_plane_observability::{
        ControlPlaneObservability, ObservabilityConfig, ObservabilityStatus,
    },
};

use super::{distributed_tracing::DistributedTracing, structured_logging::StructuredLogging};

/// Comprehensive observability integration service
pub struct ObservabilityIntegration {
    observability: Arc<ControlPlaneObservability>,
    health: Arc<ControlPlaneHealth>,
    metrics: Arc<ControlPlaneMetrics>,
    tracing: Arc<DistributedTracing>,
    logging: Arc<StructuredLogging>,
}

impl ObservabilityIntegration {
    /// Initialize comprehensive observability integration
    #[instrument]
    pub async fn new(
        database: Option<Arc<mongodb::Database>>,
        active_jobs: Option<Arc<tokio::sync::RwLock<usize>>>,
        queue_length: Option<Arc<tokio::sync::RwLock<usize>>>,
        healthy_nodes: Option<Arc<tokio::sync::RwLock<usize>>>,
        total_nodes: Option<Arc<tokio::sync::RwLock<usize>>>,
    ) -> Result<Self> {
        info!("Initializing comprehensive observability integration");

        // Create observability configuration
        let observability_config = ObservabilityConfig::default();

        // Initialize main observability service
        let observability = Arc::new(ControlPlaneObservability::new(observability_config).await?);

        // Get individual components
        let health = observability.health_monitor();
        let metrics = observability.metrics_collector();
        let tracing = observability.distributed_tracing();
        let logging = observability.structured_logging();

        // Add health checks if components are provided
        if let Some(db) = database {
            health
                .add_check(Box::new(DatabaseHealthCheck::new(db)))
                .await;
        }

        if let (Some(active), Some(queue)) = (active_jobs, queue_length) {
            health
                .add_check(Box::new(JobSchedulerHealthCheck::new(active, queue)))
                .await;
        }

        if let (Some(healthy), Some(total)) = (healthy_nodes, total_nodes) {
            health
                .add_check(Box::new(NodeRegistryHealthCheck::new(healthy, total)))
                .await;
        }

        // Start background monitoring
        health.start_monitoring().await?;

        info!("Observability integration initialized successfully");

        Ok(Self {
            observability,
            health,
            metrics,
            tracing,
            logging,
        })
    }

    /// Get the main observability service
    pub fn observability(&self) -> Arc<ControlPlaneObservability> {
        Arc::clone(&self.observability)
    }

    /// Get health monitor
    pub fn health(&self) -> Arc<ControlPlaneHealth> {
        Arc::clone(&self.health)
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<ControlPlaneMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Get distributed tracing
    pub fn tracing(&self) -> Arc<DistributedTracing> {
        Arc::clone(&self.tracing)
    }

    /// Get structured logging
    pub fn logging(&self) -> Arc<StructuredLogging> {
        Arc::clone(&self.logging)
    }

    /// Create comprehensive HTTP router for all observability endpoints
    pub fn create_router(self: Arc<Self>) -> Router {
        let observability_router = self.observability.clone().create_router();
        let health_router = self.health.clone().create_router();

        // Create a simple router that combines the sub-routers
        Router::new()
            // Main observability endpoints
            .nest("/observability", observability_router)
            // Health endpoints
            .nest("/", health_router)
    }

    /// Get comprehensive integration status
    #[instrument(skip(self))]
    pub async fn get_integration_status(&self) -> IntegrationStatus {
        let observability_status = self.observability.get_observability_status().await;
        let health_response = self.health.check_health().await;
        let metrics_snapshot = self.metrics.get_metrics_snapshot().await;
        let active_traces = self.tracing.get_active_traces_count().await;
        let logging_stats = self.logging.get_statistics().await;

        IntegrationStatus {
            observability_status,
            health_response,
            metrics_snapshot,
            active_traces,
            logging_stats,
            integration_uptime_seconds: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Comprehensive integration status
#[derive(Debug, serde::Serialize)]
pub struct IntegrationStatus {
    pub observability_status: ObservabilityStatus,
    pub health_response: ControlPlaneHealthResponse,
    pub metrics_snapshot: MetricsSnapshot,
    pub active_traces: usize,
    pub logging_stats: super::structured_logging::LoggingStatistics,
    pub integration_uptime_seconds: u64,
}

// HTTP handlers can be added here if needed

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_observability_integration_creation() {
        let integration = ObservabilityIntegration::new(
            None,
            Some(Arc::new(RwLock::new(0))),
            Some(Arc::new(RwLock::new(0))),
            Some(Arc::new(RwLock::new(0))),
            Some(Arc::new(RwLock::new(0))),
        )
        .await;

        assert!(integration.is_ok());

        let integration = integration.unwrap();
        let status = integration.get_integration_status().await;

        assert!(!status.observability_status.service_name.is_empty());
        assert!(status.integration_uptime_seconds > 0);
    }

    #[tokio::test]
    async fn test_router_creation() {
        let integration = Arc::new(
            ObservabilityIntegration::new(None, None, None, None, None)
                .await
                .unwrap(),
        );

        let router = integration.create_router();
        // Router creation should not panic
        assert!(true);
    }
}
