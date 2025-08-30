use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::AppError;
use crate::core::performance::load_balancer::ServiceEndpoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScalerConfig {
    pub min_instances: u32,
    pub max_instances: u32,
    pub target_cpu_utilization: f64,
    pub target_memory_utilization: f64,
    pub target_response_time: Duration,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
    pub scale_up_cooldown: Duration,
    pub scale_down_cooldown: Duration,
    pub evaluation_period: Duration,
    pub metrics_window: Duration,
}

impl Default for AutoScalerConfig {
    fn default() -> Self {
        Self {
            min_instances: 2,
            max_instances: 20,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            target_response_time: Duration::from_millis(500),
            scale_up_threshold: 80.0,
            scale_down_threshold: 50.0,
            scale_up_cooldown: Duration::from_secs(300), // 5 minutes
            scale_down_cooldown: Duration::from_secs(600), // 10 minutes
            evaluation_period: Duration::from_secs(60), // 1 minute
            metrics_window: Duration::from_secs(300), // 5 minutes
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingMetrics {
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub average_response_time: Duration,
    pub request_rate: f64,
    pub error_rate: f64,
    pub active_connections: u32,
    pub queue_length: u32,
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
}

impl ScalingMetrics {
    pub fn new() -> Self {
        Self {
            cpu_utilization: 0.0,
            memory_utilization: 0.0,
            average_response_time: Duration::from_millis(0),
            request_rate: 0.0,
            error_rate: 0.0,
            active_connections: 0,
            queue_length: 0,
            timestamp: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingAction {
    ScaleUp { target_instances: u32, reason: String },
    ScaleDown { target_instances: u32, reason: String },
    NoAction { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingEvent {
    pub id: Uuid,
    pub action: ScalingAction,
    pub current_instances: u32,
    pub metrics: ScalingMetrics,
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScalerStats {
    pub current_instances: u32,
    pub target_instances: u32,
    pub total_scale_ups: u64,
    pub total_scale_downs: u64,
    pub last_scaling_action: Option<ScalingEvent>,
    pub average_utilization: f64,
    pub scaling_efficiency: f64,
}

pub trait InstanceProvider: Send + Sync {
    fn create_instance(&self) -> impl std::future::Future<Output = Result<ServiceEndpoint, AppError>> + Send;
    fn terminate_instance(&self, instance_id: Uuid) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
    fn get_instance_metrics(&self, instance_id: Uuid) -> impl std::future::Future<Output = Result<ScalingMetrics, AppError>> + Send;
    fn list_instances(&self) -> impl std::future::Future<Output = Result<Vec<ServiceEndpoint>, AppError>> + Send;
}

pub trait MetricsCollector: Send + Sync {
    fn collect_metrics(&self) -> impl std::future::Future<Output = Result<ScalingMetrics, AppError>> + Send;
    fn get_historical_metrics(&self, window: Duration) -> impl std::future::Future<Output = Result<Vec<ScalingMetrics>, AppError>> + Send;
}

pub struct ProductionAutoScaler<P, M>
where
    P: InstanceProvider,
    M: MetricsCollector,
{
    config: AutoScalerConfig,
    instance_provider: Arc<P>,
    metrics_collector: Arc<M>,
    instances: Arc<RwLock<HashMap<Uuid, ServiceEndpoint>>>,
    metrics_history: Arc<RwLock<Vec<ScalingMetrics>>>,
    scaling_events: Arc<RwLock<Vec<ScalingEvent>>>,
    stats: Arc<RwLock<AutoScalerStats>>,
    last_scale_up: Arc<RwLock<Option<Instant>>>,
    last_scale_down: Arc<RwLock<Option<Instant>>>,
}

impl<P, M> ProductionAutoScaler<P, M>
where
    P: InstanceProvider + 'static,
    M: MetricsCollector + 'static,
{
    pub fn new(config: AutoScalerConfig, instance_provider: P, metrics_collector: M) -> Self {
        Self {
            config,
            instance_provider: Arc::new(instance_provider),
            metrics_collector: Arc::new(metrics_collector),
            instances: Arc::new(RwLock::new(HashMap::new())),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            scaling_events: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(AutoScalerStats {
                current_instances: 0,
                target_instances: 0,
                total_scale_ups: 0,
                total_scale_downs: 0,
                last_scaling_action: None,
                average_utilization: 0.0,
                scaling_efficiency: 0.0,
            })),
            last_scale_up: Arc::new(RwLock::new(None)),
            last_scale_down: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<(), AppError> {
        // Initialize with minimum instances
        self.ensure_min_instances().await?;
        
        // Start monitoring loop
        self.start_monitoring_loop().await;
        
        Ok(())
    }

    async fn start_monitoring_loop(&self) {
        let config = self.config.clone();
        let instance_provider = Arc::clone(&self.instance_provider);
        let metrics_collector = Arc::clone(&self.metrics_collector);
        let instances = Arc::clone(&self.instances);
        let metrics_history = Arc::clone(&self.metrics_history);
        let scaling_events = Arc::clone(&self.scaling_events);
        let stats = Arc::clone(&self.stats);
        let last_scale_up = Arc::clone(&self.last_scale_up);
        let last_scale_down = Arc::clone(&self.last_scale_down);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.evaluation_period);

            loop {
                interval.tick().await;

                // Collect current metrics
                if let Ok(current_metrics) = metrics_collector.collect_metrics().await {
                    // Store metrics
                    {
                        let mut history = metrics_history.write().await;
                        history.push(current_metrics.clone());
                        
                        // Keep only recent metrics
                        let cutoff = Instant::now() - config.metrics_window;
                        history.retain(|m| m.timestamp > cutoff);
                    }

                    // Evaluate scaling decision
                    let current_instances = {
                        let instances = instances.read().await;
                        instances.len() as u32
                    };

                    let scaling_decision = Self::evaluate_scaling_decision(
                        &config,
                        &current_metrics,
                        current_instances,
                        &last_scale_up,
                        &last_scale_down,
                    ).await;

                    // Execute scaling action
                    match scaling_decision.action {
                        ScalingAction::ScaleUp { target_instances, .. } => {
                            if let Err(e) = Self::scale_up(
                                &instance_provider,
                                &instances,
                                target_instances,
                                current_instances,
                            ).await {
                                eprintln!("Failed to scale up: {}", e);
                            } else {
                                let mut last_up = last_scale_up.write().await;
                                *last_up = Some(Instant::now());
                                
                                let mut stats_guard = stats.write().await;
                                stats_guard.total_scale_ups += 1;
                            }
                        }
                        ScalingAction::ScaleDown { target_instances, .. } => {
                            if let Err(e) = Self::scale_down(
                                &instance_provider,
                                &instances,
                                target_instances,
                                current_instances,
                            ).await {
                                eprintln!("Failed to scale down: {}", e);
                            } else {
                                let mut last_down = last_scale_down.write().await;
                                *last_down = Some(Instant::now());
                                
                                let mut stats_guard = stats.write().await;
                                stats_guard.total_scale_downs += 1;
                            }
                        }
                        ScalingAction::NoAction { .. } => {
                            // No action needed
                        }
                    }

                    // Record scaling event
                    {
                        let mut events = scaling_events.write().await;
                        events.push(scaling_decision.clone());
                        
                        // Keep only recent events
                        if events.len() > 1000 {
                            events.drain(0..500);
                        }
                    }

                    // Update stats
                    {
                        let mut stats_guard = stats.write().await;
                        stats_guard.current_instances = current_instances;
                        stats_guard.last_scaling_action = Some(scaling_decision);
                        stats_guard.average_utilization = current_metrics.cpu_utilization;
                    }
                }
            }
        });
    }

    async fn evaluate_scaling_decision(
        config: &AutoScalerConfig,
        metrics: &ScalingMetrics,
        current_instances: u32,
        last_scale_up: &Arc<RwLock<Option<Instant>>>,
        last_scale_down: &Arc<RwLock<Option<Instant>>>,
    ) -> ScalingEvent {
        let now = Instant::now();
        
        // Check cooldown periods
        let can_scale_up = {
            let last_up = last_scale_up.read().await;
            last_up.map_or(true, |t| now.duration_since(t) > config.scale_up_cooldown)
        };
        
        let can_scale_down = {
            let last_down = last_scale_down.read().await;
            last_down.map_or(true, |t| now.duration_since(t) > config.scale_down_cooldown)
        };

        // Calculate scaling factors
        let cpu_factor = metrics.cpu_utilization / config.target_cpu_utilization;
        let memory_factor = metrics.memory_utilization / config.target_memory_utilization;
        let response_time_factor = metrics.average_response_time.as_millis() as f64 / config.target_response_time.as_millis() as f64;
        
        // Use the highest factor as the primary scaling indicator
        let scaling_factor = cpu_factor.max(memory_factor).max(response_time_factor);
        
        let action = if scaling_factor > (config.scale_up_threshold / 100.0) && can_scale_up && current_instances < config.max_instances {
            let target_instances = ((current_instances as f64 * scaling_factor).ceil() as u32)
                .min(config.max_instances)
                .max(current_instances + 1);
            
            ScalingAction::ScaleUp {
                target_instances,
                reason: format!(
                    "High utilization detected: CPU={:.1}%, Memory={:.1}%, ResponseTime={}ms",
                    metrics.cpu_utilization,
                    metrics.memory_utilization,
                    metrics.average_response_time.as_millis()
                ),
            }
        } else if scaling_factor < (config.scale_down_threshold / 100.0) && can_scale_down && current_instances > config.min_instances {
            let target_instances = ((current_instances as f64 * scaling_factor).floor() as u32)
                .max(config.min_instances)
                .min(current_instances - 1);
            
            ScalingAction::ScaleDown {
                target_instances,
                reason: format!(
                    "Low utilization detected: CPU={:.1}%, Memory={:.1}%, ResponseTime={}ms",
                    metrics.cpu_utilization,
                    metrics.memory_utilization,
                    metrics.average_response_time.as_millis()
                ),
            }
        } else {
            let reason = if !can_scale_up && scaling_factor > (config.scale_up_threshold / 100.0) {
                "Scale up needed but in cooldown period".to_string()
            } else if !can_scale_down && scaling_factor < (config.scale_down_threshold / 100.0) {
                "Scale down possible but in cooldown period".to_string()
            } else if current_instances >= config.max_instances && scaling_factor > (config.scale_up_threshold / 100.0) {
                "Scale up needed but at maximum instances".to_string()
            } else if current_instances <= config.min_instances && scaling_factor < (config.scale_down_threshold / 100.0) {
                "Scale down possible but at minimum instances".to_string()
            } else {
                format!("Utilization within target range: {:.1}%", scaling_factor * 100.0)
            };
            
            ScalingAction::NoAction { reason }
        };

        ScalingEvent {
            id: Uuid::new_v4(),
            action,
            current_instances,
            metrics: metrics.clone(),
            timestamp: now,
        }
    }

    async fn scale_up(
        instance_provider: &Arc<P>,
        instances: &Arc<RwLock<HashMap<Uuid, ServiceEndpoint>>>,
        target_instances: u32,
        current_instances: u32,
    ) -> Result<(), AppError> {
        let instances_to_create = target_instances - current_instances;
        
        for _ in 0..instances_to_create {
            match instance_provider.create_instance().await {
                Ok(endpoint) => {
                    let mut instances_guard = instances.write().await;
                    instances_guard.insert(endpoint.id, endpoint);
                }
                Err(e) => {
                    eprintln!("Failed to create instance: {}", e);
                    // Continue trying to create other instances
                }
            }
        }
        
        Ok(())
    }

    async fn scale_down(
        instance_provider: &Arc<P>,
        instances: &Arc<RwLock<HashMap<Uuid, ServiceEndpoint>>>,
        target_instances: u32,
        current_instances: u32,
    ) -> Result<(), AppError> {
        let instances_to_remove = current_instances - target_instances;
        
        // Select instances to terminate (prefer least utilized)
        let instances_to_terminate = {
            let instances_guard = instances.read().await;
            let mut instance_list: Vec<_> = instances_guard.values().collect();
            
            // Sort by utilization (ascending) to terminate least utilized first
            instance_list.sort_by(|a, b| {
                a.connection_utilization().partial_cmp(&b.connection_utilization())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            
            instance_list.into_iter()
                .take(instances_to_remove as usize)
                .map(|e| e.id)
                .collect::<Vec<_>>()
        };
        
        for instance_id in instances_to_terminate {
            match instance_provider.terminate_instance(instance_id).await {
                Ok(()) => {
                    let mut instances_guard = instances.write().await;
                    instances_guard.remove(&instance_id);
                }
                Err(e) => {
                    eprintln!("Failed to terminate instance {}: {}", instance_id, e);
                    // Continue trying to terminate other instances
                }
            }
        }
        
        Ok(())
    }

    async fn ensure_min_instances(&self) -> Result<(), AppError> {
        let current_instances = {
            let instances = self.instances.read().await;
            instances.len() as u32
        };
        
        if current_instances < self.config.min_instances {
            let instances_to_create = self.config.min_instances - current_instances;
            
            for _ in 0..instances_to_create {
                match self.instance_provider.create_instance().await {
                    Ok(endpoint) => {
                        let mut instances = self.instances.write().await;
                        instances.insert(endpoint.id, endpoint);
                    }
                    Err(e) => {
                        eprintln!("Failed to create initial instance: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }

    pub async fn get_stats(&self) -> AutoScalerStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    pub async fn get_scaling_events(&self, limit: usize) -> Vec<ScalingEvent> {
        let events = self.scaling_events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    pub async fn force_scale(&self, target_instances: u32) -> Result<(), AppError> {
        let current_instances = {
            let instances = self.instances.read().await;
            instances.len() as u32
        };

        if target_instances > current_instances {
            Self::scale_up(
                &self.instance_provider,
                &self.instances,
                target_instances,
                current_instances,
            ).await?;
        } else if target_instances < current_instances {
            Self::scale_down(
                &self.instance_provider,
                &self.instances,
                target_instances,
                current_instances,
            ).await?;
        }

        Ok(())
    }
}

// Mock implementations for testing
#[derive(Clone)]
pub struct MockInstanceProvider {
    pub created_instances: Arc<RwLock<Vec<ServiceEndpoint>>>,
    pub terminated_instances: Arc<RwLock<Vec<Uuid>>>,
}

impl MockInstanceProvider {
    pub fn new() -> Self {
        Self {
            created_instances: Arc::new(RwLock::new(Vec::new())),
            terminated_instances: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl InstanceProvider for MockInstanceProvider {
    async fn create_instance(&self) -> Result<ServiceEndpoint, AppError> {
        let endpoint = ServiceEndpoint::new("127.0.0.1".to_string(), 8080);
        let mut created = self.created_instances.write().await;
        created.push(endpoint.clone());
        Ok(endpoint)
    }

    async fn terminate_instance(&self, instance_id: Uuid) -> Result<(), AppError> {
        let mut terminated = self.terminated_instances.write().await;
        terminated.push(instance_id);
        Ok(())
    }

    async fn get_instance_metrics(&self, _instance_id: Uuid) -> Result<ScalingMetrics, AppError> {
        Ok(ScalingMetrics::new())
    }

    async fn list_instances(&self) -> Result<Vec<ServiceEndpoint>, AppError> {
        let created = self.created_instances.read().await;
        Ok(created.clone())
    }
}

#[derive(Clone)]
pub struct MockMetricsCollector {
    pub metrics: Arc<RwLock<ScalingMetrics>>,
}

impl MockMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(ScalingMetrics::new())),
        }
    }

    pub async fn set_metrics(&self, metrics: ScalingMetrics) {
        let mut current = self.metrics.write().await;
        *current = metrics;
    }
}

impl MetricsCollector for MockMetricsCollector {
    async fn collect_metrics(&self) -> Result<ScalingMetrics, AppError> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    async fn get_historical_metrics(&self, _window: Duration) -> Result<Vec<ScalingMetrics>, AppError> {
        let metrics = self.metrics.read().await;
        Ok(vec![metrics.clone()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auto_scaler_initialization() {
        let config = AutoScalerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        let scaler = ProductionAutoScaler::new(config, provider, collector);
        scaler.start().await.unwrap();
        
        let stats = scaler.get_stats().await;
        assert!(stats.current_instances >= 2); // Should create min instances
    }

    #[tokio::test]
    async fn test_scale_up_decision() {
        let config = AutoScalerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        // Set high utilization metrics
        let mut high_metrics = ScalingMetrics::new();
        high_metrics.cpu_utilization = 90.0;
        high_metrics.memory_utilization = 85.0;
        collector.set_metrics(high_metrics.clone()).await;
        
        let last_scale_up = Arc::new(RwLock::new(None));
        let last_scale_down = Arc::new(RwLock::new(None));
        
        let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
            &config,
            &high_metrics,
            2,
            &last_scale_up,
            &last_scale_down,
        ).await;
        
        match decision.action {
            ScalingAction::ScaleUp { target_instances, .. } => {
                assert!(target_instances > 2);
            }
            _ => panic!("Expected scale up decision"),
        }
    }

    #[tokio::test]
    async fn test_scale_down_decision() {
        let config = AutoScalerConfig::default();
        let provider = MockInstanceProvider::new();
        let collector = MockMetricsCollector::new();
        
        // Set low utilization metrics
        let mut low_metrics = ScalingMetrics::new();
        low_metrics.cpu_utilization = 30.0;
        low_metrics.memory_utilization = 25.0;
        collector.set_metrics(low_metrics.clone()).await;
        
        let last_scale_up = Arc::new(RwLock::new(None));
        let last_scale_down = Arc::new(RwLock::new(None));
        
        let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
            &config,
            &low_metrics,
            5, // More than minimum
            &last_scale_up,
            &last_scale_down,
        ).await;
        
        match decision.action {
            ScalingAction::ScaleDown { target_instances, .. } => {
                assert!(target_instances < 5);
                assert!(target_instances >= config.min_instances);
            }
            _ => panic!("Expected scale down decision"),
        }
    }

    #[tokio::test]
    async fn test_cooldown_periods() {
        let config = AutoScalerConfig::default();
        
        // Set high utilization metrics
        let mut high_metrics = ScalingMetrics::new();
        high_metrics.cpu_utilization = 90.0;
        
        // Set recent scale up time
        let last_scale_up = Arc::new(RwLock::new(Some(Instant::now())));
        let last_scale_down = Arc::new(RwLock::new(None));
        
        let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
            &config,
            &high_metrics,
            2,
            &last_scale_up,
            &last_scale_down,
        ).await;
        
        match decision.action {
            ScalingAction::NoAction { reason } => {
                assert!(reason.contains("cooldown"));
            }
            _ => panic!("Expected no action due to cooldown"),
        }
    }

    #[tokio::test]
    async fn test_min_max_instance_limits() {
        let mut config = AutoScalerConfig::default();
        config.min_instances = 2;
        config.max_instances = 5;
        
        // Test max instances limit
        let mut high_metrics = ScalingMetrics::new();
        high_metrics.cpu_utilization = 95.0;
        
        let last_scale_up = Arc::new(RwLock::new(None));
        let last_scale_down = Arc::new(RwLock::new(None));
        
        let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
            &config,
            &high_metrics,
            5, // At max instances
            &last_scale_up,
            &last_scale_down,
        ).await;
        
        match decision.action {
            ScalingAction::NoAction { reason } => {
                assert!(reason.contains("maximum"));
            }
            _ => panic!("Expected no action due to max instances limit"),
        }
        
        // Test min instances limit
        let mut low_metrics = ScalingMetrics::new();
        low_metrics.cpu_utilization = 20.0;
        
        let decision = ProductionAutoScaler::<MockInstanceProvider, MockMetricsCollector>::evaluate_scaling_decision(
            &config,
            &low_metrics,
            2, // At min instances
            &last_scale_up,
            &last_scale_down,
        ).await;
        
        match decision.action {
            ScalingAction::NoAction { reason } => {
                assert!(reason.contains("minimum"));
            }
            _ => panic!("Expected no action due to min instances limit"),
        }
    }
}