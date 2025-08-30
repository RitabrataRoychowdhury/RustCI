use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use super::{
    ProductionLoadBalancer, LoadBalancer, LoadBalancerConfig, ServiceEndpoint, LoadBalancerHealthStatus as HealthStatus,
    RoutingContext, ProductionAutoScaler, AutoScalerConfig, ScalingMetrics, ScalingEvent,
    InstanceProvider, AutoScalerMetricsCollector
};
use super::auto_scaler::ScalingAction;

/// Configuration for the integrated auto-scaling load balancer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScalingLoadBalancerConfig {
    pub load_balancer: LoadBalancerConfig,
    pub auto_scaler: AutoScalerConfig,
    pub integration_interval: Duration,
    pub health_check_enabled: bool,
    pub metrics_collection_interval: Duration,
    pub scale_decision_cooldown: Duration,
}

impl Default for AutoScalingLoadBalancerConfig {
    fn default() -> Self {
        Self {
            load_balancer: LoadBalancerConfig::default(),
            auto_scaler: AutoScalerConfig::default(),
            integration_interval: Duration::from_secs(30),
            health_check_enabled: true,
            metrics_collection_interval: Duration::from_secs(10),
            scale_decision_cooldown: Duration::from_secs(60),
        }
    }
}

/// Statistics for the integrated system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScalingLoadBalancerStats {
    pub load_balancer_stats: super::LoadBalancerStats,
    pub auto_scaler_stats: super::AutoScalerStats,
    pub integration_stats: IntegrationStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStats {
    pub total_scaling_events: u64,
    pub successful_integrations: u64,
    pub failed_integrations: u64,
    pub average_response_time: Duration,
    #[serde(skip)]
    pub last_integration: Option<Instant>,
}

impl Default for IntegrationStats {
    fn default() -> Self {
        Self {
            total_scaling_events: 0,
            successful_integrations: 0,
            failed_integrations: 0,
            average_response_time: Duration::from_millis(0),
            last_integration: None,
        }
    }
}

/// Integrated auto-scaling load balancer that combines load balancing with auto-scaling
pub struct AutoScalingLoadBalancer<P, M>
where
    P: InstanceProvider,
    M: AutoScalerMetricsCollector,
{
    config: AutoScalingLoadBalancerConfig,
    load_balancer: Arc<ProductionLoadBalancer>,
    auto_scaler: Arc<ProductionAutoScaler<P, M>>,
    integration_stats: Arc<RwLock<IntegrationStats>>,
    last_scale_decision: Arc<RwLock<Option<Instant>>>,
}

impl<P, M> AutoScalingLoadBalancer<P, M>
where
    P: InstanceProvider + 'static,
    M: AutoScalerMetricsCollector + 'static,
{
    pub fn new(
        config: AutoScalingLoadBalancerConfig,
        instance_provider: P,
        metrics_collector: M,
    ) -> Self {
        let load_balancer = Arc::new(ProductionLoadBalancer::new(config.load_balancer.clone()));
        let auto_scaler = Arc::new(ProductionAutoScaler::new(
            config.auto_scaler.clone(),
            instance_provider,
            metrics_collector,
        ));

        Self {
            config,
            load_balancer,
            auto_scaler,
            integration_stats: Arc::new(RwLock::new(IntegrationStats::default())),
            last_scale_decision: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the integrated system
    pub async fn start(&self) -> Result<(), AppError> {
        // Start auto scaler
        self.auto_scaler.start().await?;
        
        // Start load balancer health checks
        self.load_balancer.start_health_checks().await;
        
        // Start integration loop
        self.start_integration_loop().await;
        
        Ok(())
    }

    /// Start the integration loop that synchronizes auto scaler and load balancer
    async fn start_integration_loop(&self) {
        let auto_scaler = Arc::clone(&self.auto_scaler);
        let load_balancer = Arc::clone(&self.load_balancer);
        let integration_stats = Arc::clone(&self.integration_stats);
        let last_scale_decision = Arc::clone(&self.last_scale_decision);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.integration_interval);

            loop {
                interval.tick().await;

                let integration_start = Instant::now();
                let mut integration_successful = true;

                // Check if we should make scaling decisions
                let should_scale = {
                    let last_decision = last_scale_decision.read().await;
                    last_decision.map_or(true, |t| {
                        integration_start.duration_since(t) > config.scale_decision_cooldown
                    })
                };

                if should_scale {
                    // Get recent scaling events from auto scaler
                    let recent_events = auto_scaler.get_scaling_events(5).await;
                    
                    for event in &recent_events {
                        match &event.action {
                            ScalingAction::ScaleUp { target_instances, .. } => {
                                if let Err(e) = Self::handle_scale_up(
                                    &load_balancer,
                                    &auto_scaler,
                                    *target_instances,
                                ).await {
                                    eprintln!("Failed to handle scale up: {}", e);
                                    integration_successful = false;
                                }
                            }
                            ScalingAction::ScaleDown { target_instances, .. } => {
                                if let Err(e) = Self::handle_scale_down(
                                    &load_balancer,
                                    *target_instances,
                                ).await {
                                    eprintln!("Failed to handle scale down: {}", e);
                                    integration_successful = false;
                                }
                            }
                            ScalingAction::NoAction { .. } => {
                                // No action needed
                            }
                        }
                    }

                    // Update last scale decision time
                    if !recent_events.is_empty() {
                        let mut last_decision = last_scale_decision.write().await;
                        *last_decision = Some(integration_start);
                    }
                }

                // Sync health status from load balancer to auto scaler
                if let Err(e) = Self::sync_health_status(&load_balancer).await {
                    eprintln!("Failed to sync health status: {}", e);
                    integration_successful = false;
                }

                // Update integration stats
                let integration_duration = integration_start.elapsed();
                let mut stats = integration_stats.write().await;
                stats.last_integration = Some(integration_start);
                
                if integration_successful {
                    stats.successful_integrations += 1;
                } else {
                    stats.failed_integrations += 1;
                }

                // Update average response time
                let total_integrations = stats.successful_integrations + stats.failed_integrations;
                if total_integrations > 0 {
                    let current_avg = stats.average_response_time.as_millis() as u64;
                    let new_avg = (current_avg * (total_integrations - 1) + integration_duration.as_millis() as u64) / total_integrations;
                    stats.average_response_time = Duration::from_millis(new_avg);
                }
            }
        });
    }

    /// Handle scale up by adding new endpoints to load balancer
    async fn handle_scale_up<IP, MC>(
        load_balancer: &Arc<ProductionLoadBalancer>,
        auto_scaler: &Arc<ProductionAutoScaler<IP, MC>>,
        target_instances: u32,
    ) -> Result<(), AppError>
    where
        IP: InstanceProvider + 'static,
        MC: AutoScalerMetricsCollector + 'static,
    {
        // Get current stats to determine how many instances to add
        let current_stats = auto_scaler.get_stats().await;
        let instances_to_add = target_instances.saturating_sub(current_stats.current_instances);

        // Force scale the auto scaler to the target
        auto_scaler.force_scale(target_instances).await?;

        // Note: In a real implementation, we would get the actual new instances
        // from the auto scaler and add them to the load balancer
        // For now, we'll create mock endpoints
        for i in 0..instances_to_add {
            let endpoint = ServiceEndpoint::new(
                "127.0.0.1".to_string(),
                8080 + i as u16,
            );
            load_balancer.add_endpoint(endpoint).await?;
        }

        Ok(())
    }

    /// Handle scale down by removing endpoints from load balancer
    async fn handle_scale_down(
        load_balancer: &Arc<ProductionLoadBalancer>,
        target_instances: u32,
    ) -> Result<(), AppError> {
        let current_endpoints = load_balancer.get_healthy_endpoints().await;
        let instances_to_remove = current_endpoints.len().saturating_sub(target_instances as usize);

        // Remove least utilized endpoints first
        let mut endpoints_to_remove = current_endpoints;
        endpoints_to_remove.sort_by(|a, b| {
            a.connection_utilization().partial_cmp(&b.connection_utilization())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for endpoint in endpoints_to_remove.iter().take(instances_to_remove) {
            load_balancer.remove_endpoint(endpoint.id).await?;
        }

        Ok(())
    }

    /// Sync health status between components
    async fn sync_health_status(
        load_balancer: &Arc<ProductionLoadBalancer>,
    ) -> Result<(), AppError> {
        let endpoints = load_balancer.get_healthy_endpoints().await;
        
        // In a real implementation, we would update the auto scaler's
        // instance provider with the health status from the load balancer
        // For now, we'll just log the sync
        tracing::debug!("Synced health status for {} endpoints", endpoints.len());
        
        Ok(())
    }

    /// Select an endpoint using the load balancer
    pub async fn select_endpoint(&self, context: &RoutingContext) -> Result<ServiceEndpoint, AppError> {
        self.load_balancer.select_endpoint(context).await
    }

    /// Add an endpoint to the load balancer
    pub async fn add_endpoint(&self, endpoint: ServiceEndpoint) -> Result<(), AppError> {
        self.load_balancer.add_endpoint(endpoint).await
    }

    /// Remove an endpoint from the load balancer
    pub async fn remove_endpoint(&self, endpoint_id: Uuid) -> Result<(), AppError> {
        self.load_balancer.remove_endpoint(endpoint_id).await
    }

    /// Update endpoint health status
    pub async fn update_endpoint_health(&self, endpoint_id: Uuid, status: HealthStatus) -> Result<(), AppError> {
        self.load_balancer.update_endpoint_health(endpoint_id, status).await
    }

    /// Record request start
    pub async fn record_request_start(&self, endpoint_id: Uuid) -> Result<(), AppError> {
        self.load_balancer.record_request_start(endpoint_id).await
    }

    /// Record request end
    pub async fn record_request_end(&self, endpoint_id: Uuid, success: bool, response_time: Duration) -> Result<(), AppError> {
        self.load_balancer.record_request_end(endpoint_id, success, response_time).await
    }

    /// Get comprehensive stats
    pub async fn get_stats(&self) -> AutoScalingLoadBalancerStats {
        let load_balancer_stats = self.load_balancer.get_stats().await;
        let auto_scaler_stats = self.auto_scaler.get_stats().await;
        let integration_stats = self.integration_stats.read().await.clone();

        AutoScalingLoadBalancerStats {
            load_balancer_stats,
            auto_scaler_stats,
            integration_stats,
        }
    }

    /// Force scale to a specific number of instances
    pub async fn force_scale(&self, target_instances: u32) -> Result<(), AppError> {
        self.auto_scaler.force_scale(target_instances).await
    }

    /// Get recent scaling events
    pub async fn get_scaling_events(&self, limit: usize) -> Vec<ScalingEvent> {
        self.auto_scaler.get_scaling_events(limit).await
    }

    /// Get healthy endpoints
    pub async fn get_healthy_endpoints(&self) -> Vec<ServiceEndpoint> {
        self.load_balancer.get_healthy_endpoints().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::performance::{MockInstanceProvider, MockMetricsCollector};

    #[tokio::test]
    async fn test_auto_scaling_load_balancer_creation() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
        
        let stats = aslb.get_stats().await;
        assert_eq!(stats.load_balancer_stats.total_endpoints, 0);
        assert_eq!(stats.auto_scaler_stats.current_instances, 0);
    }

    #[tokio::test]
    async fn test_endpoint_management() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
        
        // Add endpoint
        let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let endpoint_id = endpoint.id;
        
        aslb.add_endpoint(endpoint).await.unwrap();
        
        let stats = aslb.get_stats().await;
        assert_eq!(stats.load_balancer_stats.total_endpoints, 1);
        
        // Remove endpoint
        aslb.remove_endpoint(endpoint_id).await.unwrap();
        
        let stats = aslb.get_stats().await;
        assert_eq!(stats.load_balancer_stats.total_endpoints, 0);
    }

    #[tokio::test]
    async fn test_request_lifecycle() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
        
        let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let endpoint_id = endpoint.id;
        
        aslb.add_endpoint(endpoint).await.unwrap();
        
        // Record request lifecycle
        aslb.record_request_start(endpoint_id).await.unwrap();
        aslb.record_request_end(endpoint_id, true, Duration::from_millis(100)).await.unwrap();
        
        let stats = aslb.get_stats().await;
        assert_eq!(stats.load_balancer_stats.total_requests, 1);
        assert_eq!(stats.load_balancer_stats.successful_requests, 1);
    }

    #[tokio::test]
    async fn test_health_status_update() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
        
        let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let endpoint_id = endpoint.id;
        
        aslb.add_endpoint(endpoint).await.unwrap();
        
        // Update health status
        aslb.update_endpoint_health(endpoint_id, HealthStatus::Healthy).await.unwrap();
        
        let healthy_endpoints = aslb.get_healthy_endpoints().await;
        assert_eq!(healthy_endpoints.len(), 1);
        assert_eq!(healthy_endpoints[0].health_status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_force_scaling() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider.clone(), collector);
        
        // Force scale to 5 instances
        aslb.force_scale(5).await.unwrap();
        
        // Check that instances were created
        let created = provider.created_instances.read().await;
        assert!(created.len() >= 5);
    }

    #[tokio::test]
    async fn test_endpoint_selection() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
        
        // Add healthy endpoint
        let mut endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        endpoint.health_status = HealthStatus::Healthy;
        
        aslb.add_endpoint(endpoint.clone()).await.unwrap();
        
        let context = RoutingContext::new("/test".to_string(), "GET".to_string());
        let selected = aslb.select_endpoint(&context).await.unwrap();
        
        assert_eq!(selected.id, endpoint.id);
    }

    #[tokio::test]
    async fn test_integration_stats() {
        let config = AutoScalingLoadBalancerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let aslb = AutoScalingLoadBalancer::new(config, provider, collector);
        
        let stats = aslb.get_stats().await;
        assert_eq!(stats.integration_stats.total_scaling_events, 0);
        assert_eq!(stats.integration_stats.successful_integrations, 0);
        assert_eq!(stats.integration_stats.failed_integrations, 0);
    }
}