use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingMetrics {
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub request_rate: f64,
    pub response_time: Duration,
    pub error_rate: f64,
    pub queue_depth: u32,
    pub active_connections: u32,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicy {
    pub policy_id: Uuid,
    pub service_name: String,
    pub min_instances: u32,
    pub max_instances: u32,
    pub target_cpu_utilization: f64,
    pub target_memory_utilization: f64,
    pub target_response_time: Duration,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
    pub scale_up_cooldown: Duration,
    pub scale_down_cooldown: Duration,
    pub scale_up_step: u32,
    pub scale_down_step: u32,
}

impl Default for ScalingPolicy {
    fn default() -> Self {
        Self {
            policy_id: Uuid::new_v4(),
            service_name: String::new(),
            min_instances: 1,
            max_instances: 10,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            target_response_time: Duration::from_millis(500),
            scale_up_threshold: 80.0,
            scale_down_threshold: 30.0,
            scale_up_cooldown: Duration::from_secs(300),
            scale_down_cooldown: Duration::from_secs(600),
            scale_up_step: 1,
            scale_down_step: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingAction {
    ScaleUp { instances: u32, reason: String },
    ScaleDown { instances: u32, reason: String },
    NoAction { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingEvent {
    pub event_id: Uuid,
    pub service_name: String,
    pub action: ScalingAction,
    pub current_instances: u32,
    pub target_instances: u32,
    pub metrics: ScalingMetrics,
    pub timestamp: SystemTime,
    pub executed: bool,
    pub execution_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub cpu_cores: f64,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub network_bandwidth_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub service_name: String,
    pub current_instances: u32,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub storage_usage: f64,
    pub network_usage: f64,
    pub cost_per_hour: f64,
    pub efficiency_score: f64,
    pub last_updated: SystemTime,
}

#[async_trait::async_trait]
pub trait MetricsCollector: Send + Sync {
    async fn collect_metrics(&self, service: &str) -> Result<ScalingMetrics>;
    async fn get_current_instance_count(&self, service: &str) -> Result<u32>;
    async fn get_resource_usage(&self, service: &str) -> Result<ResourceUsage>;
}

#[async_trait::async_trait]
pub trait InstanceManager: Send + Sync {
    async fn scale_up(&self, service: &str, instances: u32) -> Result<u32>;
    async fn scale_down(&self, service: &str, instances: u32) -> Result<u32>;
    async fn get_instance_count(&self, service: &str) -> Result<u32>;
    async fn get_instance_health(&self, service: &str) -> Result<Vec<InstanceHealth>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceHealth {
    pub instance_id: String,
    pub is_healthy: bool,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub last_health_check: SystemTime,
}

#[async_trait::async_trait]
pub trait CostOptimizer: Send + Sync {
    async fn calculate_cost(&self, service: &str, instances: u32) -> Result<f64>;
    async fn optimize_resource_allocation(&self, service: &str) -> Result<ResourceQuota>;
    async fn get_cost_recommendations(&self, service: &str) -> Result<Vec<CostRecommendation>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecommendation {
    pub recommendation_type: CostRecommendationType,
    pub description: String,
    pub potential_savings: f64,
    pub implementation_effort: ImplementationEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostRecommendationType {
    RightSizing,
    ScheduledScaling,
    SpotInstances,
    ReservedInstances,
    ResourceConsolidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
}

pub struct ProductionAutoScaler {
    policies: Arc<RwLock<HashMap<String, ScalingPolicy>>>,
    scaling_events: Arc<RwLock<Vec<ScalingEvent>>>,
    last_scaling_actions: Arc<RwLock<HashMap<String, SystemTime>>>,
    metrics_collector: Arc<dyn MetricsCollector>,
    instance_manager: Arc<dyn InstanceManager>,
    cost_optimizer: Arc<dyn CostOptimizer>,
}

impl ProductionAutoScaler {
    pub fn new(
        metrics_collector: Arc<dyn MetricsCollector>,
        instance_manager: Arc<dyn InstanceManager>,
        cost_optimizer: Arc<dyn CostOptimizer>,
    ) -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            scaling_events: Arc::new(RwLock::new(Vec::new())),
            last_scaling_actions: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
            instance_manager,
            cost_optimizer,
        }
    }

    pub async fn register_scaling_policy(&self, policy: ScalingPolicy) -> Result<()> {
        let mut policies = self.policies.write().await;
        policies.insert(policy.service_name.clone(), policy);
        log::info!("Registered scaling policy for service: {}", policy.service_name);
        Ok(())
    }

    pub async fn remove_scaling_policy(&self, service: &str) -> Result<()> {
        let mut policies = self.policies.write().await;
        policies.remove(service);
        log::info!("Removed scaling policy for service: {}", service);
        Ok(())
    }

    pub async fn evaluate_scaling(&self, service: &str) -> Result<ScalingAction> {
        let policy = {
            let policies = self.policies.read().await;
            policies.get(service).cloned()
                .ok_or_else(|| AppError::NotFound(format!("No scaling policy found for service: {}", service)))?
        };

        let metrics = self.metrics_collector.collect_metrics(service).await?;
        let current_instances = self.instance_manager.get_instance_count(service).await?;

        // Check cooldown periods
        let last_action_time = {
            let last_actions = self.last_scaling_actions.read().await;
            last_actions.get(service).copied()
        };

        if let Some(last_time) = last_action_time {
            let elapsed = last_time.elapsed().unwrap_or(Duration::MAX);
            
            // Check if we're still in cooldown
            if elapsed < policy.scale_up_cooldown && elapsed < policy.scale_down_cooldown {
                return Ok(ScalingAction::NoAction {
                    reason: format!("In cooldown period, {} seconds remaining", 
                                  policy.scale_up_cooldown.as_secs().saturating_sub(elapsed.as_secs()))
                });
            }
        }

        // Evaluate scaling decision based on multiple metrics
        let cpu_pressure = metrics.cpu_utilization;
        let memory_pressure = metrics.memory_utilization;
        let response_time_pressure = if metrics.response_time > policy.target_response_time {
            (metrics.response_time.as_millis() as f64) / (policy.target_response_time.as_millis() as f64) * 100.0
        } else {
            0.0
        };

        let overall_pressure = (cpu_pressure + memory_pressure + response_time_pressure) / 3.0;

        // Scale up decision
        if overall_pressure > policy.scale_up_threshold && current_instances < policy.max_instances {
            let scale_up_instances = policy.scale_up_step.min(policy.max_instances - current_instances);
            return Ok(ScalingAction::ScaleUp {
                instances: scale_up_instances,
                reason: format!(
                    "High resource pressure: CPU {:.1}%, Memory {:.1}%, Response time {:.1}ms",
                    cpu_pressure, memory_pressure, metrics.response_time.as_millis()
                ),
            });
        }

        // Scale down decision
        if overall_pressure < policy.scale_down_threshold && current_instances > policy.min_instances {
            let scale_down_instances = policy.scale_down_step.min(current_instances - policy.min_instances);
            return Ok(ScalingAction::ScaleDown {
                instances: scale_down_instances,
                reason: format!(
                    "Low resource pressure: CPU {:.1}%, Memory {:.1}%, Response time {:.1}ms",
                    cpu_pressure, memory_pressure, metrics.response_time.as_millis()
                ),
            });
        }

        Ok(ScalingAction::NoAction {
            reason: format!(
                "Resource pressure within acceptable range: {:.1}% (threshold: {:.1}%-{:.1}%)",
                overall_pressure, policy.scale_down_threshold, policy.scale_up_threshold
            ),
        })
    }

    pub async fn execute_scaling(&self, service: &str, action: ScalingAction) -> Result<ScalingEvent> {
        let metrics = self.metrics_collector.collect_metrics(service).await?;
        let current_instances = self.instance_manager.get_instance_count(service).await?;

        let event = ScalingEvent {
            event_id: Uuid::new_v4(),
            service_name: service.to_string(),
            action: action.clone(),
            current_instances,
            target_instances: current_instances,
            metrics,
            timestamp: SystemTime::now(),
            executed: false,
            execution_result: None,
        };

        let mut updated_event = event.clone();

        match action {
            ScalingAction::ScaleUp { instances, reason } => {
                log::info!("Scaling up service {} by {} instances: {}", service, instances, reason);
                
                match self.instance_manager.scale_up(service, instances).await {
                    Ok(new_count) => {
                        updated_event.target_instances = new_count;
                        updated_event.executed = true;
                        updated_event.execution_result = Some(format!("Successfully scaled up to {} instances", new_count));
                        
                        // Update last scaling action time
                        {
                            let mut last_actions = self.last_scaling_actions.write().await;
                            last_actions.insert(service.to_string(), SystemTime::now());
                        }
                    }
                    Err(e) => {
                        updated_event.execution_result = Some(format!("Scale up failed: {}", e));
                        log::error!("Failed to scale up service {}: {}", service, e);
                    }
                }
            }
            ScalingAction::ScaleDown { instances, reason } => {
                log::info!("Scaling down service {} by {} instances: {}", service, instances, reason);
                
                match self.instance_manager.scale_down(service, instances).await {
                    Ok(new_count) => {
                        updated_event.target_instances = new_count;
                        updated_event.executed = true;
                        updated_event.execution_result = Some(format!("Successfully scaled down to {} instances", new_count));
                        
                        // Update last scaling action time
                        {
                            let mut last_actions = self.last_scaling_actions.write().await;
                            last_actions.insert(service.to_string(), SystemTime::now());
                        }
                    }
                    Err(e) => {
                        updated_event.execution_result = Some(format!("Scale down failed: {}", e));
                        log::error!("Failed to scale down service {}: {}", service, e);
                    }
                }
            }
            ScalingAction::NoAction { reason } => {
                log::debug!("No scaling action for service {}: {}", service, reason);
                updated_event.executed = true;
                updated_event.execution_result = Some(reason);
            }
        }

        // Store the scaling event
        {
            let mut events = self.scaling_events.write().await;
            events.push(updated_event.clone());
            
            // Keep only the last 1000 events
            if events.len() > 1000 {
                events.drain(0..events.len() - 1000);
            }
        }

        Ok(updated_event)
    }

    pub async fn get_scaling_history(&self, service: Option<&str>) -> Result<Vec<ScalingEvent>> {
        let events = self.scaling_events.read().await;
        
        let filtered_events: Vec<ScalingEvent> = events.iter()
            .filter(|event| {
                service.map_or(true, |s| event.service_name == s)
            })
            .cloned()
            .collect();

        Ok(filtered_events)
    }

    pub async fn get_resource_efficiency(&self, service: &str) -> Result<f64> {
        let usage = self.metrics_collector.get_resource_usage(service).await?;
        Ok(usage.efficiency_score)
    }

    pub async fn optimize_costs(&self, service: &str) -> Result<Vec<CostRecommendation>> {
        self.cost_optimizer.get_cost_recommendations(service).await
    }

    pub async fn start_auto_scaling(&self, service: &str, interval: Duration) -> Result<()> {
        let service_name = service.to_string();
        let auto_scaler = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                match auto_scaler.evaluate_scaling(&service_name).await {
                    Ok(action) => {
                        if !matches!(action, ScalingAction::NoAction { .. }) {
                            if let Err(e) = auto_scaler.execute_scaling(&service_name, action).await {
                                log::error!("Failed to execute scaling for {}: {}", service_name, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to evaluate scaling for {}: {}", service_name, e);
                    }
                }
            }
        });

        log::info!("Started auto-scaling for service: {} with interval: {:?}", service, interval);
        Ok(())
    }
}

impl Clone for ProductionAutoScaler {
    fn clone(&self) -> Self {
        Self {
            policies: Arc::clone(&self.policies),
            scaling_events: Arc::clone(&self.scaling_events),
            last_scaling_actions: Arc::clone(&self.last_scaling_actions),
            metrics_collector: Arc::clone(&self.metrics_collector),
            instance_manager: Arc::clone(&self.instance_manager),
            cost_optimizer: Arc::clone(&self.cost_optimizer),
        }
    }
}

// Mock implementations for testing
pub struct MockMetricsCollector {
    metrics: Arc<RwLock<HashMap<String, ScalingMetrics>>>,
    instance_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl MockMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            instance_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_metrics(&self, service: &str, metrics: ScalingMetrics) {
        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(service.to_string(), metrics);
    }

    pub async fn set_instance_count(&self, service: &str, count: u32) {
        let mut counts = self.instance_counts.write().await;
        counts.insert(service.to_string(), count);
    }
}

#[async_trait::async_trait]
impl MetricsCollector for MockMetricsCollector {
    async fn collect_metrics(&self, service: &str) -> Result<ScalingMetrics> {
        let metrics = self.metrics.read().await;
        metrics.get(service)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("No metrics found for service: {}", service)))
    }

    async fn get_current_instance_count(&self, service: &str) -> Result<u32> {
        let counts = self.instance_counts.read().await;
        Ok(counts.get(service).copied().unwrap_or(1))
    }

    async fn get_resource_usage(&self, service: &str) -> Result<ResourceUsage> {
        Ok(ResourceUsage {
            service_name: service.to_string(),
            current_instances: self.get_current_instance_count(service).await?,
            cpu_usage: 50.0,
            memory_usage: 60.0,
            storage_usage: 30.0,
            network_usage: 40.0,
            cost_per_hour: 10.0,
            efficiency_score: 0.75,
            last_updated: SystemTime::now(),
        })
    }
}

impl Default for MockMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockInstanceManager {
    instance_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl MockInstanceManager {
    pub fn new() -> Self {
        Self {
            instance_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl InstanceManager for MockInstanceManager {
    async fn scale_up(&self, service: &str, instances: u32) -> Result<u32> {
        let mut counts = self.instance_counts.write().await;
        let current = counts.get(service).copied().unwrap_or(1);
        let new_count = current + instances;
        counts.insert(service.to_string(), new_count);
        Ok(new_count)
    }

    async fn scale_down(&self, service: &str, instances: u32) -> Result<u32> {
        let mut counts = self.instance_counts.write().await;
        let current = counts.get(service).copied().unwrap_or(1);
        let new_count = current.saturating_sub(instances).max(1);
        counts.insert(service.to_string(), new_count);
        Ok(new_count)
    }

    async fn get_instance_count(&self, service: &str) -> Result<u32> {
        let counts = self.instance_counts.read().await;
        Ok(counts.get(service).copied().unwrap_or(1))
    }

    async fn get_instance_health(&self, service: &str) -> Result<Vec<InstanceHealth>> {
        let count = self.get_instance_count(service).await?;
        let mut health_status = Vec::new();
        
        for i in 0..count {
            health_status.push(InstanceHealth {
                instance_id: format!("{}-instance-{}", service, i),
                is_healthy: true,
                cpu_usage: 50.0,
                memory_usage: 60.0,
                last_health_check: SystemTime::now(),
            });
        }
        
        Ok(health_status)
    }
}

impl Default for MockInstanceManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockCostOptimizer;

impl MockCostOptimizer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CostOptimizer for MockCostOptimizer {
    async fn calculate_cost(&self, _service: &str, instances: u32) -> Result<f64> {
        Ok(instances as f64 * 10.0) // $10 per instance per hour
    }

    async fn optimize_resource_allocation(&self, _service: &str) -> Result<ResourceQuota> {
        Ok(ResourceQuota {
            cpu_cores: 2.0,
            memory_gb: 4.0,
            storage_gb: 20.0,
            network_bandwidth_mbps: 100.0,
        })
    }

    async fn get_cost_recommendations(&self, _service: &str) -> Result<Vec<CostRecommendation>> {
        Ok(vec![
            CostRecommendation {
                recommendation_type: CostRecommendationType::RightSizing,
                description: "Consider reducing instance size during low traffic periods".to_string(),
                potential_savings: 25.0,
                implementation_effort: ImplementationEffort::Medium,
            },
            CostRecommendation {
                recommendation_type: CostRecommendationType::ScheduledScaling,
                description: "Implement scheduled scaling based on traffic patterns".to_string(),
                potential_savings: 40.0,
                implementation_effort: ImplementationEffort::Low,
            },
        ])
    }
}

impl Default for MockCostOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auto_scaler_scale_up() {
        let metrics_collector = Arc::new(MockMetricsCollector::new());
        let instance_manager = Arc::new(MockInstanceManager::new());
        let cost_optimizer = Arc::new(MockCostOptimizer::new());
        
        let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager.clone(), cost_optimizer);

        // Set up high resource usage
        let high_usage_metrics = ScalingMetrics {
            cpu_utilization: 90.0,
            memory_utilization: 85.0,
            request_rate: 1000.0,
            response_time: Duration::from_millis(800),
            error_rate: 0.01,
            queue_depth: 50,
            active_connections: 200,
            timestamp: SystemTime::now(),
        };

        metrics_collector.set_metrics("test-service", high_usage_metrics).await;
        metrics_collector.set_instance_count("test-service", 2).await;

        // Register scaling policy
        let policy = ScalingPolicy {
            service_name: "test-service".to_string(),
            min_instances: 1,
            max_instances: 5,
            scale_up_threshold: 80.0,
            scale_down_threshold: 30.0,
            ..Default::default()
        };

        auto_scaler.register_scaling_policy(policy).await.unwrap();

        // Evaluate scaling
        let action = auto_scaler.evaluate_scaling("test-service").await.unwrap();
        
        match action {
            ScalingAction::ScaleUp { instances, .. } => {
                assert_eq!(instances, 1);
            }
            _ => panic!("Expected scale up action"),
        }

        // Execute scaling
        let event = auto_scaler.execute_scaling("test-service", action).await.unwrap();
        assert!(event.executed);
        assert_eq!(event.target_instances, 3);
    }

    #[tokio::test]
    async fn test_auto_scaler_scale_down() {
        let metrics_collector = Arc::new(MockMetricsCollector::new());
        let instance_manager = Arc::new(MockInstanceManager::new());
        let cost_optimizer = Arc::new(MockCostOptimizer::new());
        
        let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager.clone(), cost_optimizer);

        // Set up low resource usage
        let low_usage_metrics = ScalingMetrics {
            cpu_utilization: 20.0,
            memory_utilization: 25.0,
            request_rate: 50.0,
            response_time: Duration::from_millis(100),
            error_rate: 0.001,
            queue_depth: 2,
            active_connections: 10,
            timestamp: SystemTime::now(),
        };

        metrics_collector.set_metrics("test-service", low_usage_metrics).await;
        metrics_collector.set_instance_count("test-service", 3).await;

        // Register scaling policy
        let policy = ScalingPolicy {
            service_name: "test-service".to_string(),
            min_instances: 1,
            max_instances: 5,
            scale_up_threshold: 80.0,
            scale_down_threshold: 30.0,
            ..Default::default()
        };

        auto_scaler.register_scaling_policy(policy).await.unwrap();

        // Evaluate scaling
        let action = auto_scaler.evaluate_scaling("test-service").await.unwrap();
        
        match action {
            ScalingAction::ScaleDown { instances, .. } => {
                assert_eq!(instances, 1);
            }
            _ => panic!("Expected scale down action"),
        }
    }

    #[tokio::test]
    async fn test_auto_scaler_no_action() {
        let metrics_collector = Arc::new(MockMetricsCollector::new());
        let instance_manager = Arc::new(MockInstanceManager::new());
        let cost_optimizer = Arc::new(MockCostOptimizer::new());
        
        let auto_scaler = ProductionAutoScaler::new(metrics_collector.clone(), instance_manager, cost_optimizer);

        // Set up moderate resource usage
        let moderate_usage_metrics = ScalingMetrics {
            cpu_utilization: 50.0,
            memory_utilization: 55.0,
            request_rate: 200.0,
            response_time: Duration::from_millis(300),
            error_rate: 0.005,
            queue_depth: 10,
            active_connections: 50,
            timestamp: SystemTime::now(),
        };

        metrics_collector.set_metrics("test-service", moderate_usage_metrics).await;
        metrics_collector.set_instance_count("test-service", 2).await;

        // Register scaling policy
        let policy = ScalingPolicy {
            service_name: "test-service".to_string(),
            scale_up_threshold: 80.0,
            scale_down_threshold: 30.0,
            ..Default::default()
        };

        auto_scaler.register_scaling_policy(policy).await.unwrap();

        // Evaluate scaling
        let action = auto_scaler.evaluate_scaling("test-service").await.unwrap();
        
        match action {
            ScalingAction::NoAction { .. } => {
                // Expected behavior
            }
            _ => panic!("Expected no action"),
        }
    }

    #[tokio::test]
    async fn test_cost_optimization() {
        let metrics_collector = Arc::new(MockMetricsCollector::new());
        let instance_manager = Arc::new(MockInstanceManager::new());
        let cost_optimizer = Arc::new(MockCostOptimizer::new());
        
        let auto_scaler = ProductionAutoScaler::new(metrics_collector, instance_manager, cost_optimizer);

        let recommendations = auto_scaler.optimize_costs("test-service").await.unwrap();
        assert!(!recommendations.is_empty());
        
        let first_recommendation = &recommendations[0];
        assert!(matches!(first_recommendation.recommendation_type, CostRecommendationType::RightSizing));
        assert!(first_recommendation.potential_savings > 0.0);
    }
}