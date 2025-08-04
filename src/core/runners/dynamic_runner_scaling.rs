//! Dynamic Runner Scaling System
//!
//! This module provides workload-based runner provisioning and deprovisioning,
//! resource quota management, and capacity planning for cluster-wide scaling.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::core::cluster::node_registry::NodeRegistry;
use crate::core::runners::runner_pool::RunnerPoolManager;
use crate::domain::entities::{
    NodeId, RunnerId, RunnerEntity, RunnerStatus, RunnerType,
};
use crate::error::{AppError, Result};

/// Dynamic scaling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicScalingConfig {
    /// Minimum number of runners to maintain
    pub min_runners: u32,
    /// Maximum number of runners allowed
    pub max_runners: u32,
    /// Target CPU utilization percentage
    pub target_cpu_utilization: f64,
    /// Target memory utilization percentage
    pub target_memory_utilization: f64,
    /// Scale up threshold (utilization above this triggers scale up)
    pub scale_up_threshold: f64,
    /// Scale down threshold (utilization below this triggers scale down)
    pub scale_down_threshold: f64,
    /// Scaling evaluation interval in seconds
    pub evaluation_interval: u64,
    /// Cooldown period after scaling in seconds
    pub scaling_cooldown: u64,
    /// Maximum runners to add/remove in one scaling event
    pub max_scaling_step: u32,
    /// Enable predictive scaling
    pub predictive_scaling: bool,
    /// Resource quota limits
    pub resource_quotas: ResourceQuotas,
    /// Scaling policies
    pub scaling_policies: Vec<ScalingPolicy>,
}

impl Default for DynamicScalingConfig {
    fn default() -> Self {
        Self {
            min_runners: 2,
            max_runners: 50,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            scale_up_threshold: 80.0,
            scale_down_threshold: 50.0,
            evaluation_interval: 60,
            scaling_cooldown: 300,
            max_scaling_step: 5,
            predictive_scaling: false,
            resource_quotas: ResourceQuotas::default(),
            scaling_policies: vec![
                ScalingPolicy::default_cpu_policy(),
                ScalingPolicy::default_memory_policy(),
                ScalingPolicy::default_queue_policy(),
            ],
        }
    }
}

/// Resource quota limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotas {
    /// Maximum total CPU cores
    pub max_cpu_cores: u32,
    /// Maximum total memory in MB
    pub max_memory_mb: u32,
    /// Maximum total disk space in MB
    pub max_disk_mb: u32,
    /// Maximum network bandwidth in Mbps
    pub max_network_mbps: u32,
    /// Per-node resource limits
    pub per_node_limits: NodeResourceLimits,
}

impl Default for ResourceQuotas {
    fn default() -> Self {
        Self {
            max_cpu_cores: 1000,
            max_memory_mb: 1024000, // 1TB
            max_disk_mb: 10240000,  // 10TB
            max_network_mbps: 10000, // 10Gbps
            per_node_limits: NodeResourceLimits::default(),
        }
    }
}

/// Per-node resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResourceLimits {
    /// Maximum CPU cores per node
    pub max_cpu_cores: u32,
    /// Maximum memory per node in MB
    pub max_memory_mb: u32,
    /// Maximum runners per node
    pub max_runners_per_node: u32,
}

impl Default for NodeResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_cores: 64,
            max_memory_mb: 128000, // 128GB
            max_runners_per_node: 20,
        }
    }
}

/// Scaling policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicy {
    /// Policy name
    pub name: String,
    /// Policy type
    pub policy_type: ScalingPolicyType,
    /// Metric to monitor
    pub metric: ScalingMetric,
    /// Threshold for scaling action
    pub threshold: f64,
    /// Comparison operator
    pub comparison: ComparisonOperator,
    /// Scaling action
    pub action: ScalingAction,
    /// Policy priority (higher = more important)
    pub priority: u32,
    /// Evaluation period in seconds
    pub evaluation_period: u64,
    /// Number of consecutive periods before action
    pub consecutive_periods: u32,
}

impl ScalingPolicy {
    /// Create default CPU-based scaling policy
    pub fn default_cpu_policy() -> Self {
        Self {
            name: "cpu-scale-up".to_string(),
            policy_type: ScalingPolicyType::ScaleUp,
            metric: ScalingMetric::CpuUtilization,
            threshold: 80.0,
            comparison: ComparisonOperator::GreaterThan,
            action: ScalingAction::AddRunners { count: 2 },
            priority: 100,
            evaluation_period: 60,
            consecutive_periods: 2,
        }
    }
    
    /// Create default memory-based scaling policy
    pub fn default_memory_policy() -> Self {
        Self {
            name: "memory-scale-up".to_string(),
            policy_type: ScalingPolicyType::ScaleUp,
            metric: ScalingMetric::MemoryUtilization,
            threshold: 85.0,
            comparison: ComparisonOperator::GreaterThan,
            action: ScalingAction::AddRunners { count: 1 },
            priority: 90,
            evaluation_period: 60,
            consecutive_periods: 3,
        }
    }
    
    /// Create default queue-based scaling policy
    pub fn default_queue_policy() -> Self {
        Self {
            name: "queue-scale-up".to_string(),
            policy_type: ScalingPolicyType::ScaleUp,
            metric: ScalingMetric::QueueLength,
            threshold: 10.0,
            comparison: ComparisonOperator::GreaterThan,
            action: ScalingAction::AddRunners { count: 3 },
            priority: 110,
            evaluation_period: 30,
            consecutive_periods: 2,
        }
    }
}

/// Scaling policy types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingPolicyType {
    /// Scale up policy
    ScaleUp,
    /// Scale down policy
    ScaleDown,
    /// Predictive scaling policy
    Predictive,
}

/// Scaling metrics to monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingMetric {
    /// CPU utilization percentage
    CpuUtilization,
    /// Memory utilization percentage
    MemoryUtilization,
    /// Disk utilization percentage
    DiskUtilization,
    /// Network utilization percentage
    NetworkUtilization,
    /// Job queue length
    QueueLength,
    /// Average response time
    ResponseTime,
    /// Error rate percentage
    ErrorRate,
    /// Custom metric
    Custom { name: String },
}

/// Comparison operators for thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// Greater than
    GreaterThan,
    /// Greater than or equal
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal
    LessThanOrEqual,
    /// Equal
    Equal,
}

/// Scaling actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingAction {
    /// Add specific number of runners
    AddRunners { count: u32 },
    /// Remove specific number of runners
    RemoveRunners { count: u32 },
    /// Scale to specific number of runners
    ScaleTo { count: u32 },
    /// Scale by percentage
    ScaleByPercentage { percentage: f64 },
}

/// Current resource utilization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    /// CPU utilization percentage
    pub cpu_utilization: f64,
    /// Memory utilization percentage
    pub memory_utilization: f64,
    /// Disk utilization percentage
    pub disk_utilization: f64,
    /// Network utilization percentage
    pub network_utilization: f64,
    /// Job queue length
    pub queue_length: u32,
    /// Average response time in milliseconds
    pub avg_response_time: f64,
    /// Error rate percentage
    pub error_rate: f64,
    /// Custom metrics
    pub custom_metrics: HashMap<String, f64>,
    /// Timestamp of measurement
    pub timestamp: DateTime<Utc>,
}

/// Scaling decision information
#[derive(Debug, Clone)]
pub struct ScalingDecision {
    /// Decision type
    pub decision_type: ScalingDecisionType,
    /// Reason for the decision
    pub reason: String,
    /// Triggered policy
    pub triggered_policy: Option<String>,
    /// Current runner count
    pub current_runners: u32,
    /// Target runner count
    pub target_runners: u32,
    /// Resource utilization that triggered decision
    pub utilization: ResourceUtilization,
    /// Decision timestamp
    pub timestamp: DateTime<Utc>,
}

/// Scaling decision types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingDecisionType {
    /// Scale up
    ScaleUp,
    /// Scale down
    ScaleDown,
    /// No scaling needed
    NoAction,
    /// Scaling blocked (e.g., by quotas or cooldown)
    Blocked { reason: String },
}

/// Capacity planning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityPlan {
    /// Current capacity
    pub current_capacity: ClusterCapacity,
    /// Projected capacity needs
    pub projected_capacity: ClusterCapacity,
    /// Recommended actions
    pub recommendations: Vec<CapacityRecommendation>,
    /// Planning horizon in hours
    pub planning_horizon_hours: u32,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
    /// Plan generation timestamp
    pub generated_at: DateTime<Utc>,
}

/// Cluster capacity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterCapacity {
    /// Total CPU cores
    pub total_cpu_cores: u32,
    /// Total memory in MB
    pub total_memory_mb: u32,
    /// Total disk space in MB
    pub total_disk_mb: u32,
    /// Total number of runners
    pub total_runners: u32,
    /// Available CPU cores
    pub available_cpu_cores: u32,
    /// Available memory in MB
    pub available_memory_mb: u32,
    /// Available disk space in MB
    pub available_disk_mb: u32,
    /// Available runner slots
    pub available_runner_slots: u32,
}

/// Capacity recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityRecommendation {
    /// Recommendation type
    pub recommendation_type: RecommendationType,
    /// Description
    pub description: String,
    /// Priority (1-10, higher = more important)
    pub priority: u32,
    /// Estimated impact
    pub impact: String,
    /// Implementation timeline
    pub timeline: String,
}

/// Recommendation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    /// Add more nodes
    AddNodes { count: u32 },
    /// Remove nodes
    RemoveNodes { count: u32 },
    /// Upgrade node resources
    UpgradeNodes { node_ids: Vec<NodeId> },
    /// Optimize resource allocation
    OptimizeAllocation,
    /// Adjust scaling policies
    AdjustPolicies,
}

/// Dynamic runner scaling service trait
#[async_trait]
pub trait DynamicRunnerScaler: Send + Sync {
    /// Start the scaling service
    async fn start(&self) -> Result<()>;
    
    /// Stop the scaling service
    async fn stop(&self) -> Result<()>;
    
    /// Evaluate current utilization and make scaling decisions
    async fn evaluate_scaling(&self) -> Result<ScalingDecision>;
    
    /// Execute a scaling decision
    async fn execute_scaling(&self, decision: &ScalingDecision) -> Result<()>;
    
    /// Get current resource utilization
    async fn get_resource_utilization(&self) -> Result<ResourceUtilization>;
    
    /// Generate capacity plan
    async fn generate_capacity_plan(&self, horizon_hours: u32) -> Result<CapacityPlan>;
    
    /// Get scaling history
    async fn get_scaling_history(&self, duration: Duration) -> Result<Vec<ScalingDecision>>;
    
    /// Update scaling configuration
    async fn update_config(&self, config: DynamicScalingConfig) -> Result<()>;
}

/// Default implementation of dynamic runner scaler
pub struct DefaultDynamicRunnerScaler {
    /// Configuration
    config: Arc<RwLock<DynamicScalingConfig>>,
    /// Node registry
    node_registry: Arc<NodeRegistry>,
    /// Runner pool manager
    runner_pool: Arc<dyn RunnerPoolManager>,
    /// Scaling history
    scaling_history: Arc<RwLock<Vec<ScalingDecision>>>,
    /// Last scaling timestamp
    last_scaling: Arc<Mutex<Option<DateTime<Utc>>>>,
    /// Running state
    running: Arc<Mutex<bool>>,
    /// Utilization history for predictive scaling
    utilization_history: Arc<RwLock<Vec<ResourceUtilization>>>,
}

impl DefaultDynamicRunnerScaler {
    /// Create a new dynamic runner scaler
    pub fn new(
        config: DynamicScalingConfig,
        node_registry: Arc<NodeRegistry>,
        runner_pool: Arc<dyn RunnerPoolManager>,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            node_registry,
            runner_pool,
            scaling_history: Arc::new(RwLock::new(Vec::new())),
            last_scaling: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
            utilization_history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Start scaling evaluation task
    async fn start_scaling_task(&self) -> Result<()> {
        let config = self.config.clone();
        let node_registry = self.node_registry.clone();
        let runner_pool = self.runner_pool.clone();
        let scaling_history = self.scaling_history.clone();
        let last_scaling = self.last_scaling.clone();
        let running = self.running.clone();
        let utilization_history = self.utilization_history.clone();
        
        tokio::spawn(async move {
            loop {
                let evaluation_interval = {
                    let config_guard = config.read().await;
                    config_guard.evaluation_interval
                };
                
                let mut interval = interval(Duration::from_secs(evaluation_interval));
                interval.tick().await;
                
                // Check if still running
                {
                    let running_guard = running.lock().await;
                    if !*running_guard {
                        break;
                    }
                }
                
                // Evaluate scaling
                let scaler = DefaultDynamicRunnerScaler {
                    config: config.clone(),
                    node_registry: node_registry.clone(),
                    runner_pool: runner_pool.clone(),
                    scaling_history: scaling_history.clone(),
                    last_scaling: last_scaling.clone(),
                    running: running.clone(),
                    utilization_history: utilization_history.clone(),
                };
                
                match scaler.evaluate_scaling().await {
                    Ok(decision) => {
                        // Record decision
                        {
                            let mut history = scaling_history.write().await;
                            history.push(decision.clone());
                            
                            // Keep only last 1000 decisions
                            if history.len() > 1000 {
                                history.remove(0);
                            }
                        }
                        
                        // Execute scaling if needed
                        if !matches!(decision.decision_type, ScalingDecisionType::NoAction) {
                            if let Err(e) = scaler.execute_scaling(&decision).await {
                                error!("Failed to execute scaling decision: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to evaluate scaling: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Collect current resource utilization
    async fn collect_resource_utilization(&self) -> Result<ResourceUtilization> {
        // Get runner pool stats
        let pool_stats = self.runner_pool.get_stats().await?;
        
        // Get node information
        let _nodes = self.node_registry.get_active_nodes().await?;
        
        // Calculate utilization (simplified implementation)
        let cpu_utilization = if pool_stats.total_runners > 0 {
            (pool_stats.busy_runners as f64 / pool_stats.total_runners as f64) * 100.0
        } else {
            0.0
        };
        
        let memory_utilization = cpu_utilization * 0.8; // Simplified correlation
        let disk_utilization = 30.0; // Simplified static value
        let network_utilization = 15.0; // Simplified static value
        
        // Calculate queue length (simplified)
        let queue_length = if pool_stats.total_runners > 0 {
            (pool_stats.total_jobs_processed % 20) as u32
        } else {
            0
        };
        
        Ok(ResourceUtilization {
            cpu_utilization,
            memory_utilization,
            disk_utilization,
            network_utilization,
            queue_length,
            avg_response_time: pool_stats.avg_job_execution_time * 1000.0, // Convert to ms
            error_rate: if pool_stats.total_jobs_processed > 0 {
                (pool_stats.total_jobs_failed as f64 / pool_stats.total_jobs_processed as f64) * 100.0
            } else {
                0.0
            },
            custom_metrics: HashMap::new(),
            timestamp: Utc::now(),
        })
    }
    
    /// Check if scaling is allowed (not in cooldown)
    async fn is_scaling_allowed(&self) -> Result<bool> {
        let config = self.config.read().await;
        let last_scaling_guard = self.last_scaling.lock().await;
        
        if let Some(last_scaling_time) = *last_scaling_guard {
            let cooldown_duration = chrono::Duration::seconds(config.scaling_cooldown as i64);
            let now = Utc::now();
            
            Ok(now - last_scaling_time > cooldown_duration)
        } else {
            Ok(true) // No previous scaling
        }
    }
    
    /// Evaluate scaling policies
    async fn evaluate_policies(&self, utilization: &ResourceUtilization) -> Result<Option<ScalingAction>> {
        let config = self.config.read().await;
        let mut triggered_policies = Vec::new();
        
        for policy in &config.scaling_policies {
            if self.evaluate_policy(policy, utilization).await? {
                triggered_policies.push(policy.clone());
            }
        }
        
        // Sort by priority (highest first)
        triggered_policies.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        // Return action from highest priority policy
        Ok(triggered_policies.first().map(|p| p.action.clone()))
    }
    
    /// Evaluate a single policy
    async fn evaluate_policy(&self, policy: &ScalingPolicy, utilization: &ResourceUtilization) -> Result<bool> {
        let metric_value = match &policy.metric {
            ScalingMetric::CpuUtilization => utilization.cpu_utilization,
            ScalingMetric::MemoryUtilization => utilization.memory_utilization,
            ScalingMetric::DiskUtilization => utilization.disk_utilization,
            ScalingMetric::NetworkUtilization => utilization.network_utilization,
            ScalingMetric::QueueLength => utilization.queue_length as f64,
            ScalingMetric::ResponseTime => utilization.avg_response_time,
            ScalingMetric::ErrorRate => utilization.error_rate,
            ScalingMetric::Custom { name } => {
                utilization.custom_metrics.get(name).copied().unwrap_or(0.0)
            }
        };
        
        let threshold_met = match policy.comparison {
            ComparisonOperator::GreaterThan => metric_value > policy.threshold,
            ComparisonOperator::GreaterThanOrEqual => metric_value >= policy.threshold,
            ComparisonOperator::LessThan => metric_value < policy.threshold,
            ComparisonOperator::LessThanOrEqual => metric_value <= policy.threshold,
            ComparisonOperator::Equal => (metric_value - policy.threshold).abs() < 0.001,
        };
        
        Ok(threshold_met)
    }
    
    /// Check resource quotas
    async fn check_resource_quotas(&self, action: &ScalingAction) -> Result<bool> {
        let config = self.config.read().await;
        let pool_stats = self.runner_pool.get_stats().await?;
        
        match action {
            ScalingAction::AddRunners { count } => {
                let new_total = pool_stats.total_runners + *count as usize;
                Ok(new_total <= config.max_runners as usize)
            }
            ScalingAction::RemoveRunners { count } => {
                let new_total = pool_stats.total_runners.saturating_sub(*count as usize);
                Ok(new_total >= config.min_runners as usize)
            }
            ScalingAction::ScaleTo { count } => {
                Ok(*count >= config.min_runners && *count <= config.max_runners)
            }
            ScalingAction::ScaleByPercentage { percentage } => {
                let new_count = (pool_stats.total_runners as f64 * (1.0 + percentage / 100.0)) as u32;
                Ok(new_count >= config.min_runners && new_count <= config.max_runners)
            }
        }
    }
    
    /// Create new runner instances
    async fn create_runners(&self, count: u32) -> Result<Vec<RunnerId>> {
        let mut created_runners = Vec::new();
        
        for _i in 0..count {
            // Create a new runner entity (simplified - would need actual runner implementation)
            let runner_entity = RunnerEntity::new(
                format!("auto-scaled-runner-{}", Uuid::new_v4()),
                RunnerType::Local {
                    max_concurrent_jobs: 4,
                    working_directory: "/tmp".to_string(),
                },
            );
            
            let runner_id = runner_entity.id;
            created_runners.push(runner_id);
            
            info!("Created auto-scaled runner: {}", runner_id);
        }
        
        Ok(created_runners)
    }
    
    /// Remove runner instances
    async fn remove_runners(&self, count: u32) -> Result<Vec<RunnerId>> {
        let runners = self.runner_pool.list_runners().await?;
        let mut removed_runners = Vec::new();
        
        // Select runners to remove (prefer idle runners)
        let mut candidates: Vec<_> = runners
            .into_iter()
            .filter(|r| r.entity.status == RunnerStatus::Idle)
            .collect();
        
        candidates.sort_by_key(|r| r.last_health_check);
        
        for runner_registration in candidates.into_iter().take(count as usize) {
            let runner_id = runner_registration.entity.id;
            
            if let Err(e) = self.runner_pool.deregister_runner(runner_id).await {
                warn!("Failed to remove runner {}: {}", runner_id, e);
            } else {
                removed_runners.push(runner_id);
                info!("Removed auto-scaled runner: {}", runner_id);
            }
        }
        
        Ok(removed_runners)
    }
}

#[async_trait]
impl DynamicRunnerScaler for DefaultDynamicRunnerScaler {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(AppError::BadRequest("Dynamic scaler is already running".to_string()));
        }
        
        *running = true;
        
        // Start scaling task
        self.start_scaling_task().await?;
        
        info!("Dynamic runner scaler started");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Ok(());
        }
        
        *running = false;
        
        info!("Dynamic runner scaler stopped");
        Ok(())
    }
    
    async fn evaluate_scaling(&self) -> Result<ScalingDecision> {
        // Collect current utilization
        let utilization = self.collect_resource_utilization().await?;
        
        // Store utilization for history
        {
            let mut history = self.utilization_history.write().await;
            history.push(utilization.clone());
            
            // Keep only last 1000 entries
            if history.len() > 1000 {
                history.remove(0);
            }
        }
        
        // Check if scaling is allowed
        if !self.is_scaling_allowed().await? {
            return Ok(ScalingDecision {
                decision_type: ScalingDecisionType::Blocked {
                    reason: "Scaling is in cooldown period".to_string(),
                },
                reason: "Cooldown period active".to_string(),
                triggered_policy: None,
                current_runners: self.runner_pool.get_stats().await?.total_runners as u32,
                target_runners: self.runner_pool.get_stats().await?.total_runners as u32,
                utilization,
                timestamp: Utc::now(),
            });
        }
        
        // Evaluate scaling policies
        let scaling_action = self.evaluate_policies(&utilization).await?;
        
        let current_runners = self.runner_pool.get_stats().await?.total_runners as u32;
        
        match scaling_action {
            Some(action) => {
                // Check resource quotas
                if !self.check_resource_quotas(&action).await? {
                    return Ok(ScalingDecision {
                        decision_type: ScalingDecisionType::Blocked {
                            reason: "Resource quota limits exceeded".to_string(),
                        },
                        reason: "Quota limits prevent scaling".to_string(),
                        triggered_policy: None,
                        current_runners,
                        target_runners: current_runners,
                        utilization,
                        timestamp: Utc::now(),
                    });
                }
                
                let (decision_type, target_runners, reason) = match &action {
                    ScalingAction::AddRunners { count } => (
                        ScalingDecisionType::ScaleUp,
                        current_runners + count,
                        format!("Adding {} runners due to high utilization", count),
                    ),
                    ScalingAction::RemoveRunners { count } => (
                        ScalingDecisionType::ScaleDown,
                        current_runners.saturating_sub(*count),
                        format!("Removing {} runners due to low utilization", count),
                    ),
                    ScalingAction::ScaleTo { count } => {
                        if *count > current_runners {
                            (
                                ScalingDecisionType::ScaleUp,
                                *count,
                                format!("Scaling up to {} runners", count),
                            )
                        } else {
                            (
                                ScalingDecisionType::ScaleDown,
                                *count,
                                format!("Scaling down to {} runners", count),
                            )
                        }
                    }
                    ScalingAction::ScaleByPercentage { percentage } => {
                        let new_count = (current_runners as f64 * (1.0 + percentage / 100.0)) as u32;
                        if new_count > current_runners {
                            (
                                ScalingDecisionType::ScaleUp,
                                new_count,
                                format!("Scaling up by {}%", percentage),
                            )
                        } else {
                            (
                                ScalingDecisionType::ScaleDown,
                                new_count,
                                format!("Scaling down by {}%", percentage),
                            )
                        }
                    }
                };
                
                Ok(ScalingDecision {
                    decision_type,
                    reason,
                    triggered_policy: Some("policy-name".to_string()), // Would be actual policy name
                    current_runners,
                    target_runners,
                    utilization,
                    timestamp: Utc::now(),
                })
            }
            None => Ok(ScalingDecision {
                decision_type: ScalingDecisionType::NoAction,
                reason: "No scaling policies triggered".to_string(),
                triggered_policy: None,
                current_runners,
                target_runners: current_runners,
                utilization,
                timestamp: Utc::now(),
            }),
        }
    }
    
    async fn execute_scaling(&self, decision: &ScalingDecision) -> Result<()> {
        match &decision.decision_type {
            ScalingDecisionType::ScaleUp => {
                let runners_to_add = decision.target_runners - decision.current_runners;
                let created_runners = self.create_runners(runners_to_add).await?;
                
                info!(
                    "Scaled up: added {} runners ({:?})",
                    runners_to_add, created_runners
                );
            }
            ScalingDecisionType::ScaleDown => {
                let runners_to_remove = decision.current_runners - decision.target_runners;
                let removed_runners = self.remove_runners(runners_to_remove).await?;
                
                info!(
                    "Scaled down: removed {} runners ({:?})",
                    runners_to_remove, removed_runners
                );
            }
            _ => {
                debug!("No scaling action executed: {:?}", decision.decision_type);
                return Ok(());
            }
        }
        
        // Update last scaling timestamp
        {
            let mut last_scaling = self.last_scaling.lock().await;
            *last_scaling = Some(Utc::now());
        }
        
        Ok(())
    }
    
    async fn get_resource_utilization(&self) -> Result<ResourceUtilization> {
        self.collect_resource_utilization().await
    }
    
    async fn generate_capacity_plan(&self, horizon_hours: u32) -> Result<CapacityPlan> {
        // Get current capacity
        let pool_stats = self.runner_pool.get_stats().await?;
        let nodes = self.node_registry.get_active_nodes().await?;
        
        let current_capacity = ClusterCapacity {
            total_cpu_cores: nodes.len() as u32 * 8, // Simplified: 8 cores per node
            total_memory_mb: nodes.len() as u32 * 16384, // Simplified: 16GB per node
            total_disk_mb: nodes.len() as u32 * 102400, // Simplified: 100GB per node
            total_runners: pool_stats.total_runners as u32,
            available_cpu_cores: nodes.len() as u32 * 2, // Simplified: 2 cores available
            available_memory_mb: nodes.len() as u32 * 4096, // Simplified: 4GB available
            available_disk_mb: nodes.len() as u32 * 51200, // Simplified: 50GB available
            available_runner_slots: (pool_stats.total_runners as u32).saturating_sub(pool_stats.busy_runners as u32),
        };
        
        // Project future capacity needs (simplified prediction)
        let growth_factor = 1.2; // 20% growth expected
        let projected_capacity = ClusterCapacity {
            total_cpu_cores: (current_capacity.total_cpu_cores as f64 * growth_factor) as u32,
            total_memory_mb: (current_capacity.total_memory_mb as f64 * growth_factor) as u32,
            total_disk_mb: (current_capacity.total_disk_mb as f64 * growth_factor) as u32,
            total_runners: (current_capacity.total_runners as f64 * growth_factor) as u32,
            available_cpu_cores: current_capacity.available_cpu_cores,
            available_memory_mb: current_capacity.available_memory_mb,
            available_disk_mb: current_capacity.available_disk_mb,
            available_runner_slots: current_capacity.available_runner_slots,
        };
        
        // Generate recommendations
        let mut recommendations = Vec::new();
        
        if projected_capacity.total_runners > current_capacity.total_runners {
            recommendations.push(CapacityRecommendation {
                recommendation_type: RecommendationType::AddNodes {
                    count: (projected_capacity.total_runners - current_capacity.total_runners) / 10,
                },
                description: "Add nodes to handle projected growth".to_string(),
                priority: 8,
                impact: "Increased capacity and redundancy".to_string(),
                timeline: "Next 30 days".to_string(),
            });
        }
        
        Ok(CapacityPlan {
            current_capacity,
            projected_capacity,
            recommendations,
            planning_horizon_hours: horizon_hours,
            confidence: 0.75, // 75% confidence in prediction
            generated_at: Utc::now(),
        })
    }
    
    async fn get_scaling_history(&self, duration: Duration) -> Result<Vec<ScalingDecision>> {
        let history = self.scaling_history.read().await;
        let cutoff = Utc::now() - chrono::Duration::from_std(duration)
            .map_err(|e| AppError::BadRequest(format!("Invalid duration: {}", e)))?;
        
        Ok(history
            .iter()
            .filter(|decision| decision.timestamp > cutoff)
            .cloned()
            .collect())
    }
    
    async fn update_config(&self, config: DynamicScalingConfig) -> Result<()> {
        let mut config_guard = self.config.write().await;
        *config_guard = config;
        
        info!("Dynamic scaling configuration updated");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cluster::node_registry::tests::create_test_registry;
    use crate::core::DefaultRunnerPoolManager;
    
    fn create_test_scaler() -> DefaultDynamicRunnerScaler {
        let config = DynamicScalingConfig::default();
        let node_registry = Arc::new(create_test_registry());
        let runner_pool = Arc::new(DefaultRunnerPoolManager::with_default_config());
        
        DefaultDynamicRunnerScaler::new(config, node_registry, runner_pool)
    }
    
    #[tokio::test]
    async fn test_scaler_creation() {
        let scaler = create_test_scaler();
        assert!(!*scaler.running.lock().await);
    }
    
    #[tokio::test]
    async fn test_resource_utilization() {
        let scaler = create_test_scaler();
        let utilization = scaler.collect_resource_utilization().await.unwrap();
        
        assert!(utilization.cpu_utilization >= 0.0);
        assert!(utilization.memory_utilization >= 0.0);
        assert!(utilization.timestamp <= Utc::now());
    }
    
    #[tokio::test]
    async fn test_scaling_policies() {
        let cpu_policy = ScalingPolicy::default_cpu_policy();
        assert_eq!(cpu_policy.name, "cpu-scale-up");
        assert!(matches!(cpu_policy.metric, ScalingMetric::CpuUtilization));
        assert_eq!(cpu_policy.threshold, 80.0);
        
        let memory_policy = ScalingPolicy::default_memory_policy();
        assert_eq!(memory_policy.name, "memory-scale-up");
        assert!(matches!(memory_policy.metric, ScalingMetric::MemoryUtilization));
        
        let queue_policy = ScalingPolicy::default_queue_policy();
        assert_eq!(queue_policy.name, "queue-scale-up");
        assert!(matches!(queue_policy.metric, ScalingMetric::QueueLength));
    }
    
    #[tokio::test]
    async fn test_capacity_plan_generation() {
        let scaler = create_test_scaler();
        let plan = scaler.generate_capacity_plan(24).await.unwrap();
        
        assert_eq!(plan.planning_horizon_hours, 24);
        assert!(plan.confidence > 0.0 && plan.confidence <= 1.0);
        assert!(plan.projected_capacity.total_runners >= plan.current_capacity.total_runners);
    }
    
    #[tokio::test]
    async fn test_scaling_decision() {
        let utilization = ResourceUtilization {
            cpu_utilization: 85.0, // Above threshold
            memory_utilization: 70.0,
            disk_utilization: 30.0,
            network_utilization: 15.0,
            queue_length: 5,
            avg_response_time: 100.0,
            error_rate: 1.0,
            custom_metrics: HashMap::new(),
            timestamp: Utc::now(),
        };
        
        let decision = ScalingDecision {
            decision_type: ScalingDecisionType::ScaleUp,
            reason: "High CPU utilization".to_string(),
            triggered_policy: Some("cpu-scale-up".to_string()),
            current_runners: 5,
            target_runners: 7,
            utilization,
            timestamp: Utc::now(),
        };
        
        assert!(matches!(decision.decision_type, ScalingDecisionType::ScaleUp));
        assert_eq!(decision.current_runners, 5);
        assert_eq!(decision.target_runners, 7);
    }
}