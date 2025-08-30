use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use super::{PerformanceMetrics, AlertManager, AlertRule, MetricsIssueSeverity};
use super::alerting::AlertCondition;
use chrono;

/// Self-healing performance management system
pub struct SelfHealingManager {
    config: SelfHealingConfig,
    issue_detector: Arc<PerformanceIssueDetector>,
    healing_engine: Arc<HealingEngine>,
    alert_manager: Arc<dyn AlertManager>,
    metrics: Arc<SelfHealingMetrics>,
    active_healers: Arc<RwLock<HashMap<String, ActiveHealer>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfHealingConfig {
    pub enable_auto_healing: bool,
    pub detection_interval: Duration,
    pub healing_timeout: Duration,
    pub max_concurrent_healers: usize,
    pub escalation_threshold: u32,
    pub cooldown_period: Duration,
    pub enable_predictive_healing: bool,
    pub metrics_retention: Duration,
}

impl Default for SelfHealingConfig {
    fn default() -> Self {
        Self {
            enable_auto_healing: true,
            detection_interval: Duration::from_secs(30),
            healing_timeout: Duration::from_secs(300),
            max_concurrent_healers: 10,
            escalation_threshold: 3,
            cooldown_period: Duration::from_secs(600),
            enable_predictive_healing: true,
            metrics_retention: Duration::from_secs(3600),
        }
    }
}

/// Performance issue detector that identifies problems automatically
pub struct PerformanceIssueDetector {
    config: SelfHealingConfig,
    metrics_history: Arc<RwLock<Vec<PerformanceSnapshot>>>,
    issue_patterns: Arc<RwLock<HashMap<String, IssuePattern>>>,
    detection_rules: Vec<DetectionRule>,
}

#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub timestamp: Instant,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub response_time: Duration,
    pub error_rate: f64,
    pub throughput: f64,
    pub active_connections: u32,
    pub queue_depth: u32,
}

#[derive(Debug, Clone)]
pub struct IssuePattern {
    pub pattern_id: String,
    pub description: String,
    pub detection_count: u32,
    pub last_detected: Instant,
    pub severity: IssueSeverity,
    pub healing_actions: Vec<HealingAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct DetectionRule {
    pub rule_id: String,
    pub name: String,
    pub condition: DetectionCondition,
    pub threshold: f64,
    pub duration: Duration,
    pub severity: IssueSeverity,
    pub healing_actions: Vec<HealingAction>,
}

#[derive(Debug, Clone)]
pub enum DetectionCondition {
    CpuUsageHigh,
    MemoryUsageHigh,
    ResponseTimeSlow,
    ErrorRateHigh,
    ThroughputLow,
    QueueDepthHigh,
    ConnectionsHigh,
}
// Healing engine that executes remediation actions
pub struct HealingEngine {
    config: SelfHealingConfig,
    healing_strategies: HashMap<String, Box<dyn HealingStrategy>>,
    execution_history: Arc<RwLock<Vec<HealingExecution>>>,
}

#[derive(Debug, Clone)]
pub struct HealingExecution {
    pub execution_id: Uuid,
    pub issue_id: String,
    pub action: HealingAction,
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
    pub success: Option<bool>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealingAction {
    RestartService { service_name: String },
    ScaleUp { target_instances: u32 },
    ScaleDown { target_instances: u32 },
    ClearCache { cache_name: String },
    RestartConnections,
    ReduceLoad { percentage: u32 },
    EnableCircuitBreaker { service: String },
    FlushQueues,
    GarbageCollect,
    OptimizeDatabase,
    Custom { action_name: String, parameters: HashMap<String, String> },
}

#[async_trait::async_trait]
pub trait HealingStrategy: Send + Sync {
    fn can_handle(&self, action: &HealingAction) -> bool;
    async fn execute(&self, action: HealingAction) -> Result<HealingResult, AppError>;
    fn estimate_duration(&self, action: &HealingAction) -> Duration;
    fn get_strategy_name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct HealingResult {
    pub success: bool,
    pub message: String,
    pub metrics_improvement: Option<MetricsImprovement>,
    pub side_effects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MetricsImprovement {
    pub cpu_improvement: f64,
    pub memory_improvement: f64,
    pub response_time_improvement: Duration,
    pub error_rate_improvement: f64,
}

/// Active healer tracking
#[derive(Debug, Clone)]
pub struct ActiveHealer {
    pub healer_id: Uuid,
    pub issue_id: String,
    pub action: HealingAction,
    pub started_at: Instant,
    pub estimated_completion: Instant,
}

/// Self-healing metrics
#[derive(Debug, Default)]
pub struct SelfHealingMetrics {
    pub total_issues_detected: AtomicU64,
    pub total_healing_attempts: AtomicU64,
    pub successful_healings: AtomicU64,
    pub failed_healings: AtomicU64,
    pub average_healing_time: AtomicU64, // microseconds
    pub escalations: AtomicU64,
    pub false_positives: AtomicU64,
    pub current_active_healers: AtomicU32,
}

impl PerformanceIssueDetector {
    pub fn new(config: SelfHealingConfig) -> Self {
        let detection_rules = vec![
            DetectionRule {
                rule_id: "cpu_high".to_string(),
                name: "High CPU Usage".to_string(),
                condition: DetectionCondition::CpuUsageHigh,
                threshold: 80.0,
                duration: Duration::from_secs(60),
                severity: IssueSeverity::High,
                healing_actions: vec![
                    HealingAction::ScaleUp { target_instances: 2 },
                    HealingAction::ReduceLoad { percentage: 20 },
                ],
            },
            DetectionRule {
                rule_id: "memory_high".to_string(),
                name: "High Memory Usage".to_string(),
                condition: DetectionCondition::MemoryUsageHigh,
                threshold: 85.0,
                duration: Duration::from_secs(30),
                severity: IssueSeverity::High,
                healing_actions: vec![
                    HealingAction::GarbageCollect,
                    HealingAction::ClearCache { cache_name: "all".to_string() },
                ],
            },
            DetectionRule {
                rule_id: "response_slow".to_string(),
                name: "Slow Response Times".to_string(),
                condition: DetectionCondition::ResponseTimeSlow,
                threshold: 5000.0, // 5 seconds in milliseconds
                duration: Duration::from_secs(120),
                severity: IssueSeverity::Medium,
                healing_actions: vec![
                    HealingAction::OptimizeDatabase,
                    HealingAction::ScaleUp { target_instances: 1 },
                ],
            },
        ];

        Self {
            config,
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            issue_patterns: Arc::new(RwLock::new(HashMap::new())),
            detection_rules,
        }
    }

    pub async fn detect_issues(&self, current_metrics: PerformanceSnapshot) -> Vec<DetectedIssue> {
        let mut detected_issues = Vec::new();
        
        // Store current metrics
        {
            let mut history = self.metrics_history.write().await;
            history.push(current_metrics.clone());
            
            // Keep only recent metrics
            let cutoff = current_metrics.timestamp - self.config.metrics_retention;
            history.retain(|snapshot| snapshot.timestamp > cutoff);
        }

        // Check each detection rule
        for rule in &self.detection_rules {
            if let Some(issue) = self.check_rule(rule, &current_metrics).await {
                detected_issues.push(issue);
            }
        }

        detected_issues
    }

    async fn check_rule(&self, rule: &DetectionRule, current: &PerformanceSnapshot) -> Option<DetectedIssue> {
        let value = match rule.condition {
            DetectionCondition::CpuUsageHigh => current.cpu_usage,
            DetectionCondition::MemoryUsageHigh => current.memory_usage,
            DetectionCondition::ResponseTimeSlow => current.response_time.as_millis() as f64,
            DetectionCondition::ErrorRateHigh => current.error_rate,
            DetectionCondition::ThroughputLow => current.throughput,
            DetectionCondition::QueueDepthHigh => current.queue_depth as f64,
            DetectionCondition::ConnectionsHigh => current.active_connections as f64,
        };

        if value > rule.threshold {
            // Check if condition persists for the required duration
            if self.condition_persists(&rule.condition, rule.threshold, rule.duration).await {
                return Some(DetectedIssue {
                    issue_id: Uuid::new_v4(),
                    rule_id: rule.rule_id.clone(),
                    description: format!("{}: {} > {}", rule.name, value, rule.threshold),
                    severity: rule.severity.clone(),
                    detected_at: current.timestamp,
                    current_value: value,
                    threshold: rule.threshold,
                    suggested_actions: rule.healing_actions.clone(),
                });
            }
        }

        None
    }

    async fn condition_persists(&self, condition: &DetectionCondition, threshold: f64, duration: Duration) -> bool {
        let history = self.metrics_history.read().await;
        let cutoff = Instant::now() - duration;
        
        let recent_snapshots: Vec<_> = history
            .iter()
            .filter(|snapshot| snapshot.timestamp > cutoff)
            .collect();

        if recent_snapshots.is_empty() {
            return false;
        }

        // Check if condition was true for the entire duration
        recent_snapshots.iter().all(|snapshot| {
            let value = match condition {
                DetectionCondition::CpuUsageHigh => snapshot.cpu_usage,
                DetectionCondition::MemoryUsageHigh => snapshot.memory_usage,
                DetectionCondition::ResponseTimeSlow => snapshot.response_time.as_millis() as f64,
                DetectionCondition::ErrorRateHigh => snapshot.error_rate,
                DetectionCondition::ThroughputLow => snapshot.throughput,
                DetectionCondition::QueueDepthHigh => snapshot.queue_depth as f64,
                DetectionCondition::ConnectionsHigh => snapshot.active_connections as f64,
            };
            value > threshold
        })
    }
}

#[derive(Debug, Clone)]
pub struct DetectedIssue {
    pub issue_id: Uuid,
    pub rule_id: String,
    pub description: String,
    pub severity: IssueSeverity,
    pub detected_at: Instant,
    pub current_value: f64,
    pub threshold: f64,
    pub suggested_actions: Vec<HealingAction>,
}impl
 HealingEngine {
    pub fn new(config: SelfHealingConfig) -> Self {
        let mut healing_strategies: HashMap<String, Box<dyn HealingStrategy>> = HashMap::new();
        
        // Register built-in healing strategies
        healing_strategies.insert("scaling".to_string(), Box::new(ScalingStrategy::new()));
        healing_strategies.insert("cache".to_string(), Box::new(CacheStrategy::new()));
        healing_strategies.insert("connection".to_string(), Box::new(ConnectionStrategy::new()));
        healing_strategies.insert("load".to_string(), Box::new(LoadStrategy::new()));
        healing_strategies.insert("system".to_string(), Box::new(SystemStrategy::new()));

        Self {
            config,
            healing_strategies,
            execution_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn execute_healing(&self, issue: DetectedIssue) -> Result<HealingResult, AppError> {
        let execution_id = Uuid::new_v4();
        let started_at = Instant::now();

        // Record execution start
        {
            let mut history = self.execution_history.write().await;
            history.push(HealingExecution {
                execution_id,
                issue_id: issue.issue_id.to_string(),
                action: issue.suggested_actions[0].clone(), // Use first suggested action
                started_at,
                completed_at: None,
                success: None,
                error_message: None,
            });
        }

        // Find appropriate strategy
        let action = &issue.suggested_actions[0];
        let strategy = self.find_strategy(action)?;

        // Execute healing action
        let result = tokio::time::timeout(
            self.config.healing_timeout,
            strategy.execute(action.clone())
        ).await;

        let healing_result = match result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => HealingResult {
                success: false,
                message: format!("Healing failed: {}", e),
                metrics_improvement: None,
                side_effects: vec![],
            },
            Err(_) => HealingResult {
                success: false,
                message: "Healing timed out".to_string(),
                metrics_improvement: None,
                side_effects: vec!["timeout".to_string()],
            },
        };

        // Update execution history
        {
            let mut history = self.execution_history.write().await;
            if let Some(execution) = history.iter_mut().find(|e| e.execution_id == execution_id) {
                execution.completed_at = Some(Instant::now());
                execution.success = Some(healing_result.success);
                if !healing_result.success {
                    execution.error_message = Some(healing_result.message.clone());
                }
            }
        }

        Ok(healing_result)
    }

    fn find_strategy(&self, action: &HealingAction) -> Result<&Box<dyn HealingStrategy>, AppError> {
        for strategy in self.healing_strategies.values() {
            if strategy.can_handle(action) {
                return Ok(strategy);
            }
        }
        Err(AppError::Internal(format!("No strategy found for action: {:?}", action)))
    }
}

// Built-in healing strategies
struct ScalingStrategy;
struct CacheStrategy;
struct ConnectionStrategy;
struct LoadStrategy;
struct SystemStrategy;

impl ScalingStrategy {
    fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl HealingStrategy for ScalingStrategy {
    fn can_handle(&self, action: &HealingAction) -> bool {
        matches!(action, HealingAction::ScaleUp { .. } | HealingAction::ScaleDown { .. })
    }

    async fn execute(&self, action: HealingAction) -> Result<HealingResult, AppError> {
        match action {
            HealingAction::ScaleUp { target_instances } => {
                // Simulate scaling up
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(HealingResult {
                    success: true,
                    message: format!("Scaled up to {} instances", target_instances),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 20.0,
                        memory_improvement: 10.0,
                        response_time_improvement: Duration::from_millis(500),
                        error_rate_improvement: 5.0,
                    }),
                    side_effects: vec!["increased_cost".to_string()],
                })
            }
            HealingAction::ScaleDown { target_instances } => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(HealingResult {
                    success: true,
                    message: format!("Scaled down to {} instances", target_instances),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: -5.0,
                        memory_improvement: -5.0,
                        response_time_improvement: Duration::from_millis(100),
                        error_rate_improvement: 0.0,
                    }),
                    side_effects: vec!["reduced_capacity".to_string()],
                })
            }
            _ => Err(AppError::Internal("Invalid action for scaling strategy".to_string())),
        }
    }

    fn estimate_duration(&self, _action: &HealingAction) -> Duration {
        Duration::from_secs(30)
    }

    fn get_strategy_name(&self) -> &str {
        "scaling"
    }
}

impl CacheStrategy {
    fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl HealingStrategy for CacheStrategy {
    fn can_handle(&self, action: &HealingAction) -> bool {
        matches!(action, HealingAction::ClearCache { .. })
    }

    async fn execute(&self, action: HealingAction) -> Result<HealingResult, AppError> {
        match action {
            HealingAction::ClearCache { cache_name } => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(HealingResult {
                    success: true,
                    message: format!("Cleared cache: {}", cache_name),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 5.0,
                        memory_improvement: 30.0,
                        response_time_improvement: Duration::from_millis(100),
                        error_rate_improvement: 2.0,
                    }),
                    side_effects: vec!["temporary_performance_degradation".to_string()],
                })
            }
            _ => Err(AppError::Internal("Invalid action for cache strategy".to_string())),
        }
    }

    fn estimate_duration(&self, _action: &HealingAction) -> Duration {
        Duration::from_secs(5)
    }

    fn get_strategy_name(&self) -> &str {
        "cache"
    }
}

impl ConnectionStrategy {
    fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl HealingStrategy for ConnectionStrategy {
    fn can_handle(&self, action: &HealingAction) -> bool {
        matches!(action, HealingAction::RestartConnections)
    }

    async fn execute(&self, action: HealingAction) -> Result<HealingResult, AppError> {
        match action {
            HealingAction::RestartConnections => {
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok(HealingResult {
                    success: true,
                    message: "Restarted connections".to_string(),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 10.0,
                        memory_improvement: 15.0,
                        response_time_improvement: Duration::from_millis(300),
                        error_rate_improvement: 10.0,
                    }),
                    side_effects: vec!["brief_service_interruption".to_string()],
                })
            }
            _ => Err(AppError::Internal("Invalid action for connection strategy".to_string())),
        }
    }

    fn estimate_duration(&self, _action: &HealingAction) -> Duration {
        Duration::from_secs(10)
    }

    fn get_strategy_name(&self) -> &str {
        "connection"
    }
}

impl LoadStrategy {
    fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl HealingStrategy for LoadStrategy {
    fn can_handle(&self, action: &HealingAction) -> bool {
        matches!(action, HealingAction::ReduceLoad { .. } | HealingAction::EnableCircuitBreaker { .. })
    }

    async fn execute(&self, action: HealingAction) -> Result<HealingResult, AppError> {
        match action {
            HealingAction::ReduceLoad { percentage } => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(HealingResult {
                    success: true,
                    message: format!("Reduced load by {}%", percentage),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: percentage as f64 * 0.8,
                        memory_improvement: percentage as f64 * 0.5,
                        response_time_improvement: Duration::from_millis(percentage as u64 * 10),
                        error_rate_improvement: percentage as f64 * 0.3,
                    }),
                    side_effects: vec!["reduced_throughput".to_string()],
                })
            }
            HealingAction::EnableCircuitBreaker { service } => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(HealingResult {
                    success: true,
                    message: format!("Enabled circuit breaker for {}", service),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 15.0,
                        memory_improvement: 10.0,
                        response_time_improvement: Duration::from_millis(200),
                        error_rate_improvement: 25.0,
                    }),
                    side_effects: vec!["service_degradation".to_string()],
                })
            }
            _ => Err(AppError::Internal("Invalid action for load strategy".to_string())),
        }
    }

    fn estimate_duration(&self, _action: &HealingAction) -> Duration {
        Duration::from_secs(2)
    }

    fn get_strategy_name(&self) -> &str {
        "load"
    }
}

impl SystemStrategy {
    fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl HealingStrategy for SystemStrategy {
    fn can_handle(&self, action: &HealingAction) -> bool {
        matches!(action, 
            HealingAction::GarbageCollect | 
            HealingAction::FlushQueues | 
            HealingAction::OptimizeDatabase |
            HealingAction::RestartService { .. }
        )
    }

    async fn execute(&self, action: HealingAction) -> Result<HealingResult, AppError> {
        match action {
            HealingAction::GarbageCollect => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(HealingResult {
                    success: true,
                    message: "Performed garbage collection".to_string(),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 5.0,
                        memory_improvement: 25.0,
                        response_time_improvement: Duration::from_millis(50),
                        error_rate_improvement: 1.0,
                    }),
                    side_effects: vec!["brief_pause".to_string()],
                })
            }
            HealingAction::FlushQueues => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(HealingResult {
                    success: true,
                    message: "Flushed queues".to_string(),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 10.0,
                        memory_improvement: 20.0,
                        response_time_improvement: Duration::from_millis(200),
                        error_rate_improvement: 5.0,
                    }),
                    side_effects: vec!["data_loss_risk".to_string()],
                })
            }
            HealingAction::OptimizeDatabase => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(HealingResult {
                    success: true,
                    message: "Optimized database".to_string(),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 15.0,
                        memory_improvement: 10.0,
                        response_time_improvement: Duration::from_millis(1000),
                        error_rate_improvement: 3.0,
                    }),
                    side_effects: vec!["temporary_slowdown".to_string()],
                })
            }
            HealingAction::RestartService { service_name } => {
                tokio::time::sleep(Duration::from_millis(1000)).await;
                Ok(HealingResult {
                    success: true,
                    message: format!("Restarted service: {}", service_name),
                    metrics_improvement: Some(MetricsImprovement {
                        cpu_improvement: 30.0,
                        memory_improvement: 40.0,
                        response_time_improvement: Duration::from_millis(2000),
                        error_rate_improvement: 50.0,
                    }),
                    side_effects: vec!["service_downtime".to_string()],
                })
            }
            _ => Err(AppError::Internal("Invalid action for system strategy".to_string())),
        }
    }

    fn estimate_duration(&self, action: &HealingAction) -> Duration {
        match action {
            HealingAction::GarbageCollect => Duration::from_secs(5),
            HealingAction::FlushQueues => Duration::from_secs(2),
            HealingAction::OptimizeDatabase => Duration::from_secs(30),
            HealingAction::RestartService { .. } => Duration::from_secs(60),
            _ => Duration::from_secs(10),
        }
    }

    fn get_strategy_name(&self) -> &str {
        "system"
    }
}

impl SelfHealingManager {
    pub fn new(config: SelfHealingConfig, alert_manager: Arc<dyn AlertManager>) -> Self {
        let issue_detector = Arc::new(PerformanceIssueDetector::new(config.clone()));
        let healing_engine = Arc::new(HealingEngine::new(config.clone()));
        let metrics = Arc::new(SelfHealingMetrics::default());

        Self {
            config,
            issue_detector,
            healing_engine,
            alert_manager,
            metrics,
            active_healers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<(), AppError> {
        if !self.config.enable_auto_healing {
            tracing::info!("Self-healing is disabled");
            return Ok(());
        }

        tracing::info!("Starting self-healing manager");
        
        // Start monitoring loop
        self.start_monitoring_loop().await;
        
        Ok(())
    }

    async fn start_monitoring_loop(&self) {
        let issue_detector = Arc::clone(&self.issue_detector);
        let healing_engine = Arc::clone(&self.healing_engine);
        let alert_manager = Arc::clone(&self.alert_manager);
        let metrics = Arc::clone(&self.metrics);
        let active_healers = Arc::clone(&self.active_healers);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.detection_interval);

            loop {
                interval.tick().await;

                // Collect current performance metrics
                let current_metrics = Self::collect_performance_metrics().await;

                // Detect issues
                let detected_issues = issue_detector.detect_issues(current_metrics).await;

                for issue in detected_issues {
                    metrics.total_issues_detected.fetch_add(1, Ordering::Relaxed);

                    // Check if we're already healing this type of issue
                    let should_heal = {
                        let healers = active_healers.read().await;
                        let active_count = healers.len();
                        
                        active_count < config.max_concurrent_healers &&
                        !healers.values().any(|h| h.issue_id == issue.rule_id)
                    };

                    if should_heal {
                        // Start healing process
                        let healing_engine_clone = Arc::clone(&healing_engine);
                        let alert_manager_clone = Arc::clone(&alert_manager);
                        let metrics_clone = Arc::clone(&metrics);
                        let active_healers_clone = Arc::clone(&active_healers);
                        let config_clone = config.clone();

                        tokio::spawn(async move {
                            Self::handle_issue(
                                issue,
                                healing_engine_clone,
                                alert_manager_clone,
                                metrics_clone,
                                active_healers_clone,
                                config_clone,
                            ).await;
                        });
                    } else {
                        // Send alert for unhandled issue
                        let alert_rule = AlertRule {
                            id: Uuid::new_v4(),
                            name: format!("Self-healing capacity exceeded: {}", issue.description),
                            condition: AlertCondition::Threshold {
                                metric_path: "issue_severity".to_string(),
                                operator: crate::core::performance::alerting::ComparisonOperator::GreaterThanOrEqual,
                                value: 1.0,
                                duration: Some(Duration::from_secs(60)),
                            },
                            description: format!("Self-healing capacity exceeded for issue: {}", issue.description),
                            severity: match issue.severity {
                                IssueSeverity::Critical => crate::core::performance::metrics::IssueSeverity::Critical,
                                IssueSeverity::High => crate::core::performance::metrics::IssueSeverity::High,
                                IssueSeverity::Medium => crate::core::performance::metrics::IssueSeverity::Medium,
                                IssueSeverity::Low => crate::core::performance::metrics::IssueSeverity::Low,
                            },
                            notification_channels: vec![],
                            cooldown_duration: Duration::from_secs(300),
                            auto_resolve: true,
                            enabled: true,
                            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                            updated_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                        };

                        if let Err(e) = alert_manager.trigger_alert(alert_rule, 1.0).await {
                            tracing::error!("Failed to send alert: {}", e);
                        }
                    }
                }

                // Cleanup completed healers
                Self::cleanup_completed_healers(&active_healers).await;
            }
        });
    }

    async fn handle_issue(
        issue: DetectedIssue,
        healing_engine: Arc<HealingEngine>,
        alert_manager: Arc<dyn AlertManager>,
        metrics: Arc<SelfHealingMetrics>,
        active_healers: Arc<RwLock<HashMap<String, ActiveHealer>>>,
        config: SelfHealingConfig,
    ) {
        let healer_id = Uuid::new_v4();
        let estimated_completion = Instant::now() + Duration::from_secs(60); // Default estimate

        // Register active healer
        {
            let mut healers = active_healers.write().await;
            healers.insert(issue.rule_id.clone(), ActiveHealer {
                healer_id,
                issue_id: issue.issue_id.to_string(),
                action: issue.suggested_actions[0].clone(),
                started_at: Instant::now(),
                estimated_completion,
            });
        }

        metrics.total_healing_attempts.fetch_add(1, Ordering::Relaxed);
        metrics.current_active_healers.fetch_add(1, Ordering::Relaxed);

        let start_time = Instant::now();
        
        // Attempt healing
        match healing_engine.execute_healing(issue.clone()).await {
            Ok(result) => {
                let healing_duration = start_time.elapsed();
                
                if result.success {
                    metrics.successful_healings.fetch_add(1, Ordering::Relaxed);
                    
                    // Update average healing time
                    let current_avg = metrics.average_healing_time.load(Ordering::Relaxed);
                    let new_avg = if current_avg == 0 {
                        healing_duration.as_micros() as u64
                    } else {
                        (current_avg + healing_duration.as_micros() as u64) / 2
                    };
                    metrics.average_healing_time.store(new_avg, Ordering::Relaxed);

                    tracing::info!(
                        "Successfully healed issue {} in {:?}: {}",
                        issue.rule_id,
                        healing_duration,
                        result.message
                    );

                    // Send success notification
                    let alert_rule = crate::core::performance::alerting::AlertRule {
                        id: Uuid::new_v4(),
                        name: format!("Self-healing Success: {}", issue.description),
                        description: format!("Successfully resolved: {}", result.message),
                        condition: crate::core::performance::alerting::AlertCondition::Threshold {
                            metric_path: "self_healing.success".to_string(),
                            operator: crate::core::performance::alerting::ComparisonOperator::GreaterThan,
                            value: 0.0,
                            duration: None,
                        },
                        severity: crate::core::performance::alerting::IssueSeverity::Low,
                        notification_channels: vec![
                            crate::core::performance::alerting::NotificationChannel::Log { 
                                level: "info".to_string() 
                            }
                        ],
                        cooldown_duration: Duration::from_secs(0),
                        auto_resolve: false,
                        enabled: true,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        updated_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    if let Err(e) = alert_manager.trigger_alert(alert_rule, 1.0).await {
                        tracing::error!("Failed to send healing success alert: {}", e);
                    }
                } else {
                    metrics.failed_healings.fetch_add(1, Ordering::Relaxed);
                    
                    tracing::error!(
                        "Failed to heal issue {}: {}",
                        issue.rule_id,
                        result.message
                    );

                    // Check if we should escalate
                    if Self::should_escalate(&issue, &config).await {
                        metrics.escalations.fetch_add(1, Ordering::Relaxed);
                        
                        let escalation_alert = crate::core::performance::alerting::AlertRule {
                            id: Uuid::new_v4(),
                            name: format!("Self-healing Escalation: {}", issue.description),
                            description: format!("Failed to resolve automatically: {}", result.message),
                            condition: crate::core::performance::alerting::AlertCondition::Threshold {
                                metric_path: "self_healing.escalation".to_string(),
                                operator: crate::core::performance::alerting::ComparisonOperator::GreaterThan,
                                value: 0.0,
                                duration: None,
                            },
                            severity: crate::core::performance::alerting::IssueSeverity::High,
                            notification_channels: vec![
                                crate::core::performance::alerting::NotificationChannel::Log { 
                                    level: "error".to_string() 
                                }
                            ],
                            cooldown_duration: Duration::from_secs(0),
                            auto_resolve: false,
                            enabled: true,
                            created_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            updated_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        };

                        if let Err(e) = alert_manager.trigger_alert(escalation_alert, 1.0).await {
                            tracing::error!("Failed to send escalation alert: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                metrics.failed_healings.fetch_add(1, Ordering::Relaxed);
                
                tracing::error!("Healing execution failed for issue {}: {}", issue.rule_id, e);

                // Send failure alert
                let failure_alert = crate::core::performance::alerting::AlertRule {
                    id: Uuid::new_v4(),
                    name: format!("Self-healing Failure: {}", issue.description),
                    description: format!("Healing execution failed: {}", e),
                    condition: crate::core::performance::alerting::AlertCondition::Threshold {
                        metric_path: "self_healing.failure".to_string(),
                        operator: crate::core::performance::alerting::ComparisonOperator::GreaterThan,
                        value: 0.0,
                        duration: None,
                    },
                    severity: crate::core::performance::alerting::IssueSeverity::Critical,
                    notification_channels: vec![
                        crate::core::performance::alerting::NotificationChannel::Log { 
                            level: "error".to_string() 
                        }
                    ],
                    cooldown_duration: Duration::from_secs(0),
                    auto_resolve: false,
                    enabled: true,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    updated_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };

                if let Err(e) = alert_manager.trigger_alert(failure_alert, 1.0).await {
                    tracing::error!("Failed to send healing failure alert: {}", e);
                }
            }
        }

        // Remove from active healers
        {
            let mut healers = active_healers.write().await;
            healers.remove(&issue.rule_id);
        }
        
        metrics.current_active_healers.fetch_sub(1, Ordering::Relaxed);
    }

    async fn should_escalate(issue: &DetectedIssue, config: &SelfHealingConfig) -> bool {
        // Escalate critical issues immediately
        if matches!(issue.severity, IssueSeverity::Critical) {
            return true;
        }

        // Escalate if we've exceeded the threshold
        // In a real implementation, this would track failure counts per issue type
        false
    }

    async fn cleanup_completed_healers(active_healers: &Arc<RwLock<HashMap<String, ActiveHealer>>>) {
        let mut healers = active_healers.write().await;
        let now = Instant::now();
        
        healers.retain(|_, healer| {
            // Remove healers that have exceeded their estimated completion time by a significant margin
            now < healer.estimated_completion + Duration::from_secs(300)
        });
    }

    async fn collect_performance_metrics() -> PerformanceSnapshot {
        // In a real implementation, this would collect actual system metrics
        // For now, we'll simulate some metrics
        PerformanceSnapshot {
            timestamp: Instant::now(),
            cpu_usage: 45.0,
            memory_usage: 60.0,
            response_time: Duration::from_millis(250),
            error_rate: 2.0,
            throughput: 1000.0,
            active_connections: 150,
            queue_depth: 10,
        }
    }

    pub async fn get_metrics(&self) -> SelfHealingMetrics {
        SelfHealingMetrics {
            total_issues_detected: AtomicU64::new(self.metrics.total_issues_detected.load(Ordering::Relaxed)),
            total_healing_attempts: AtomicU64::new(self.metrics.total_healing_attempts.load(Ordering::Relaxed)),
            successful_healings: AtomicU64::new(self.metrics.successful_healings.load(Ordering::Relaxed)),
            failed_healings: AtomicU64::new(self.metrics.failed_healings.load(Ordering::Relaxed)),
            average_healing_time: AtomicU64::new(self.metrics.average_healing_time.load(Ordering::Relaxed)),
            escalations: AtomicU64::new(self.metrics.escalations.load(Ordering::Relaxed)),
            false_positives: AtomicU64::new(self.metrics.false_positives.load(Ordering::Relaxed)),
            current_active_healers: AtomicU32::new(self.metrics.current_active_healers.load(Ordering::Relaxed)),
        }
    }

    pub async fn get_active_healers(&self) -> Vec<ActiveHealer> {
        let healers = self.active_healers.read().await;
        healers.values().cloned().collect()
    }

    pub async fn stop(&self) -> Result<(), AppError> {
        tracing::info!("Stopping self-healing manager");
        
        // Wait for active healers to complete or timeout
        let timeout = Duration::from_secs(30);
        let start = Instant::now();
        
        while start.elapsed() < timeout {
            let active_count = {
                let healers = self.active_healers.read().await;
                healers.len()
            };
            
            if active_count == 0 {
                break;
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Force cleanup any remaining healers
        {
            let mut healers = self.active_healers.write().await;
            healers.clear();
        }
        
        tracing::info!("Self-healing manager stopped");
        Ok(())
    }
}
