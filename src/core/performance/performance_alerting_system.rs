use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use super::{
    PerformanceMetrics, MetricsIssueSeverity as IssueSeverity, AlertManager, AlertRule, AlertCondition, 
    ComparisonOperator, NotificationChannel, SelfHealingManager, SelfHealingConfig,
    PerformanceSnapshot, DetectedIssue, HealingAction
};

/// Comprehensive performance alerting and self-healing system
pub struct PerformanceAlertingSystem {
    config: PerformanceAlertingConfig,
    alert_manager: Arc<dyn AlertManager>,
    self_healing_manager: Arc<SelfHealingManager>,
    alert_rules: Arc<RwLock<HashMap<String, PerformanceAlertRule>>>,
    escalation_policies: Arc<RwLock<HashMap<String, EscalationPolicy>>>,
    metrics: Arc<PerformanceAlertingMetrics>,
    alert_history: Arc<RwLock<Vec<AlertEvent>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlertingConfig {
    pub enable_alerting: bool,
    pub enable_self_healing: bool,
    pub alert_evaluation_interval: Duration,
    pub max_alert_history: usize,
    pub default_escalation_timeout: Duration,
    pub performance_thresholds: PerformanceThresholds,
    pub notification_settings: NotificationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    pub cpu_usage_warning: f64,
    pub cpu_usage_critical: f64,
    pub memory_usage_warning: f64,
    pub memory_usage_critical: f64,
    pub response_time_warning_ms: f64,
    pub response_time_critical_ms: f64,
    pub error_rate_warning: f64,
    pub error_rate_critical: f64,
    pub disk_usage_warning: f64,
    pub disk_usage_critical: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub enable_email: bool,
    pub enable_slack: bool,
    pub enable_pagerduty: bool,
    pub enable_webhooks: bool,
    pub email_addresses: Vec<String>,
    pub slack_webhook_url: Option<String>,
    pub slack_channel: Option<String>,
    pub pagerduty_integration_key: Option<String>,
    pub webhook_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlertRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub metric_path: String,
    pub threshold: f64,
    pub comparison: ComparisonOperator,
    pub severity: IssueSeverity,
    pub duration: Duration,
    pub cooldown: Duration,
    pub auto_resolve: bool,
    pub enable_self_healing: bool,
    pub healing_actions: Vec<HealingAction>,
    pub escalation_policy_id: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationPolicy {
    pub id: String,
    pub name: String,
    pub steps: Vec<EscalationStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationStep {
    pub delay: Duration,
    pub notification_channels: Vec<NotificationChannel>,
    pub auto_escalate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: Uuid,
    pub rule_id: String,
    pub event_type: AlertEventType,
    pub timestamp: u64,
    pub severity: IssueSeverity,
    pub message: String,
    pub current_value: f64,
    pub threshold: f64,
    pub healing_attempted: bool,
    pub healing_successful: Option<bool>,
    pub escalation_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertEventType {
    Triggered,
    Resolved,
    Escalated,
    Acknowledged,
    HealingAttempted,
    HealingSucceeded,
    HealingFailed,
}

#[derive(Debug, Default)]
pub struct PerformanceAlertingMetrics {
    pub total_alerts_triggered: AtomicU64,
    pub total_alerts_resolved: AtomicU64,
    pub total_escalations: AtomicU64,
    pub total_healing_attempts: AtomicU64,
    pub successful_healings: AtomicU64,
    pub failed_healings: AtomicU64,
    pub false_positive_alerts: AtomicU64,
    pub average_resolution_time_ms: AtomicU64,
}

impl Default for PerformanceAlertingConfig {
    fn default() -> Self {
        Self {
            enable_alerting: true,
            enable_self_healing: true,
            alert_evaluation_interval: Duration::from_secs(30),
            max_alert_history: 10000,
            default_escalation_timeout: Duration::from_secs(900), // 15 minutes
            performance_thresholds: PerformanceThresholds::default(),
            notification_settings: NotificationSettings::default(),
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            cpu_usage_warning: 70.0,
            cpu_usage_critical: 85.0,
            memory_usage_warning: 75.0,
            memory_usage_critical: 90.0,
            response_time_warning_ms: 1000.0,
            response_time_critical_ms: 3000.0,
            error_rate_warning: 5.0,
            error_rate_critical: 10.0,
            disk_usage_warning: 80.0,
            disk_usage_critical: 95.0,
        }
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enable_email: false,
            enable_slack: false,
            enable_pagerduty: false,
            enable_webhooks: false,
            email_addresses: vec![],
            slack_webhook_url: None,
            slack_channel: None,
            pagerduty_integration_key: None,
            webhook_urls: vec![],
        }
    }
}

impl PerformanceAlertingSystem {
    pub fn new(
        config: PerformanceAlertingConfig,
        alert_manager: Arc<dyn AlertManager>,
        self_healing_manager: Arc<SelfHealingManager>,
    ) -> Self {
        Self {
            config,
            alert_manager,
            self_healing_manager,
            alert_rules: Arc::new(RwLock::new(HashMap::new())),
            escalation_policies: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(PerformanceAlertingMetrics::default()),
            alert_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if !self.config.enable_alerting {
            tracing::info!("Performance alerting is disabled");
            return Ok(());
        }

        tracing::info!("Starting performance alerting system");

        // Initialize default alert rules
        self.initialize_default_alert_rules().await?;

        // Initialize default escalation policies
        self.initialize_default_escalation_policies().await?;

        // Start self-healing if enabled
        if self.config.enable_self_healing {
            self.self_healing_manager.start().await?;
        }

        // Start alert evaluation loop
        self.start_alert_evaluation_loop().await;

        Ok(())
    }

    async fn initialize_default_alert_rules(&self) -> Result<()> {
        let thresholds = &self.config.performance_thresholds;
        
        let default_rules = vec![
            PerformanceAlertRule {
                id: "cpu_usage_warning".to_string(),
                name: "High CPU Usage Warning".to_string(),
                description: "CPU usage is above warning threshold".to_string(),
                metric_path: "cpu_usage".to_string(),
                threshold: thresholds.cpu_usage_warning,
                comparison: ComparisonOperator::GreaterThan,
                severity: IssueSeverity::Medium,
                duration: Duration::from_secs(300),
                cooldown: Duration::from_secs(600),
                auto_resolve: true,
                enable_self_healing: true,
                healing_actions: vec![
                    HealingAction::ScaleUp { target_instances: 1 },
                    HealingAction::ReduceLoad { percentage: 10 },
                ],
                escalation_policy_id: Some("default_escalation".to_string()),
                enabled: true,
            },
            PerformanceAlertRule {
                id: "cpu_usage_critical".to_string(),
                name: "Critical CPU Usage".to_string(),
                description: "CPU usage is critically high".to_string(),
                metric_path: "cpu_usage".to_string(),
                threshold: thresholds.cpu_usage_critical,
                comparison: ComparisonOperator::GreaterThan,
                severity: IssueSeverity::Critical,
                duration: Duration::from_secs(120),
                cooldown: Duration::from_secs(300),
                auto_resolve: true,
                enable_self_healing: true,
                healing_actions: vec![
                    HealingAction::ScaleUp { target_instances: 2 },
                    HealingAction::ReduceLoad { percentage: 25 },
                    HealingAction::EnableCircuitBreaker { service: "api".to_string() },
                ],
                escalation_policy_id: Some("critical_escalation".to_string()),
                enabled: true,
            },
            PerformanceAlertRule {
                id: "memory_usage_warning".to_string(),
                name: "High Memory Usage Warning".to_string(),
                description: "Memory usage is above warning threshold".to_string(),
                metric_path: "memory_usage.usage_percentage".to_string(),
                threshold: thresholds.memory_usage_warning,
                comparison: ComparisonOperator::GreaterThan,
                severity: IssueSeverity::Medium,
                duration: Duration::from_secs(180),
                cooldown: Duration::from_secs(600),
                auto_resolve: true,
                enable_self_healing: true,
                healing_actions: vec![
                    HealingAction::GarbageCollect,
                    HealingAction::ClearCache { cache_name: "all".to_string() },
                ],
                escalation_policy_id: Some("default_escalation".to_string()),
                enabled: true,
            },
            PerformanceAlertRule {
                id: "memory_usage_critical".to_string(),
                name: "Critical Memory Usage".to_string(),
                description: "Memory usage is critically high".to_string(),
                metric_path: "memory_usage.usage_percentage".to_string(),
                threshold: thresholds.memory_usage_critical,
                comparison: ComparisonOperator::GreaterThan,
                severity: IssueSeverity::Critical,
                duration: Duration::from_secs(60),
                cooldown: Duration::from_secs(300),
                auto_resolve: true,
                enable_self_healing: true,
                healing_actions: vec![
                    HealingAction::GarbageCollect,
                    HealingAction::ClearCache { cache_name: "all".to_string() },
                    HealingAction::ScaleUp { target_instances: 1 },
                ],
                escalation_policy_id: Some("critical_escalation".to_string()),
                enabled: true,
            },
            PerformanceAlertRule {
                id: "response_time_warning".to_string(),
                name: "Slow Response Time Warning".to_string(),
                description: "Response times are above warning threshold".to_string(),
                metric_path: "request_metrics.average_response_time_ms".to_string(),
                threshold: thresholds.response_time_warning_ms,
                comparison: ComparisonOperator::GreaterThan,
                severity: IssueSeverity::Medium,
                duration: Duration::from_secs(240),
                cooldown: Duration::from_secs(600),
                auto_resolve: true,
                enable_self_healing: true,
                healing_actions: vec![
                    HealingAction::OptimizeDatabase,
                    HealingAction::ClearCache { cache_name: "query".to_string() },
                ],
                escalation_policy_id: Some("default_escalation".to_string()),
                enabled: true,
            },
            PerformanceAlertRule {
                id: "response_time_critical".to_string(),
                name: "Critical Response Time".to_string(),
                description: "Response times are critically slow".to_string(),
                metric_path: "request_metrics.average_response_time_ms".to_string(),
                threshold: thresholds.response_time_critical_ms,
                comparison: ComparisonOperator::GreaterThan,
                severity: IssueSeverity::Critical,
                duration: Duration::from_secs(120),
                cooldown: Duration::from_secs(300),
                auto_resolve: true,
                enable_self_healing: true,
                healing_actions: vec![
                    HealingAction::OptimizeDatabase,
                    HealingAction::ScaleUp { target_instances: 2 },
                    HealingAction::EnableCircuitBreaker { service: "database".to_string() },
                ],
                escalation_policy_id: Some("critical_escalation".to_string()),
                enabled: true,
            },
        ];

        let mut rules = self.alert_rules.write().await;
        for rule in default_rules {
            rules.insert(rule.id.clone(), rule);
        }

        Ok(())
    }

    async fn initialize_default_escalation_policies(&self) -> Result<()> {
        let notification_channels = self.build_notification_channels();

        let default_policies = vec![
            EscalationPolicy {
                id: "default_escalation".to_string(),
                name: "Default Escalation Policy".to_string(),
                steps: vec![
                    EscalationStep {
                        delay: Duration::from_secs(0),
                        notification_channels: vec![
                            NotificationChannel::Log { level: "warn".to_string() }
                        ],
                        auto_escalate: true,
                    },
                    EscalationStep {
                        delay: Duration::from_secs(600), // 10 minutes
                        notification_channels: notification_channels.clone(),
                        auto_escalate: true,
                    },
                    EscalationStep {
                        delay: Duration::from_secs(1800), // 30 minutes
                        notification_channels: notification_channels.clone(),
                        auto_escalate: false,
                    },
                ],
            },
            EscalationPolicy {
                id: "critical_escalation".to_string(),
                name: "Critical Issue Escalation Policy".to_string(),
                steps: vec![
                    EscalationStep {
                        delay: Duration::from_secs(0),
                        notification_channels: notification_channels.clone(),
                        auto_escalate: true,
                    },
                    EscalationStep {
                        delay: Duration::from_secs(300), // 5 minutes
                        notification_channels: notification_channels.clone(),
                        auto_escalate: true,
                    },
                    EscalationStep {
                        delay: Duration::from_secs(900), // 15 minutes
                        notification_channels: notification_channels,
                        auto_escalate: false,
                    },
                ],
            },
        ];

        let mut policies = self.escalation_policies.write().await;
        for policy in default_policies {
            policies.insert(policy.id.clone(), policy);
        }

        Ok(())
    }

    fn build_notification_channels(&self) -> Vec<NotificationChannel> {
        let mut channels = vec![
            NotificationChannel::Log { level: "error".to_string() }
        ];

        let settings = &self.config.notification_settings;

        if settings.enable_email && !settings.email_addresses.is_empty() {
            channels.push(NotificationChannel::Email {
                addresses: settings.email_addresses.clone()
            });
        }

        if settings.enable_slack {
            if let (Some(webhook_url), Some(channel)) = (&settings.slack_webhook_url, &settings.slack_channel) {
                channels.push(NotificationChannel::Slack {
                    webhook_url: webhook_url.clone(),
                    channel: channel.clone(),
                });
            }
        }

        if settings.enable_pagerduty {
            if let Some(integration_key) = &settings.pagerduty_integration_key {
                channels.push(NotificationChannel::PagerDuty {
                    integration_key: integration_key.clone(),
                });
            }
        }

        if settings.enable_webhooks {
            for url in &settings.webhook_urls {
                channels.push(NotificationChannel::Webhook {
                    url: url.clone(),
                    headers: HashMap::new(),
                });
            }
        }

        channels
    }

    async fn start_alert_evaluation_loop(&self) {
        let alert_rules = Arc::clone(&self.alert_rules);
        let alert_manager = Arc::clone(&self.alert_manager);
        let self_healing_manager = Arc::clone(&self.self_healing_manager);
        let escalation_policies = Arc::clone(&self.escalation_policies);
        let metrics = Arc::clone(&self.metrics);
        let alert_history = Arc::clone(&self.alert_history);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.alert_evaluation_interval);

            loop {
                interval.tick().await;

                // Collect current performance metrics
                if let Ok(current_metrics) = Self::collect_current_metrics().await {
                    // Evaluate all alert rules
                    let rules = alert_rules.read().await;
                    
                    for rule in rules.values() {
                        if !rule.enabled {
                            continue;
                        }

                        if let Err(e) = Self::evaluate_alert_rule(
                            rule,
                            &current_metrics,
                            &alert_manager,
                            &self_healing_manager,
                            &escalation_policies,
                            &metrics,
                            &alert_history,
                            &config,
                        ).await {
                            tracing::error!("Failed to evaluate alert rule {}: {}", rule.id, e);
                        }
                    }
                }

                // Cleanup old alert history
                Self::cleanup_alert_history(&alert_history, config.max_alert_history).await;
            }
        });
    }

    async fn collect_current_metrics() -> Result<PerformanceMetrics> {
        // In a real implementation, this would collect actual system metrics
        // For now, we'll create a placeholder
        Ok(PerformanceMetrics {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            correlation_id: uuid::Uuid::new_v4(),
            cpu_usage: 45.0,
            memory_usage: crate::core::performance::metrics::MemoryMetrics {
                total_mb: 8192,
                used_mb: 4096,
                available_mb: 4096,
                usage_percentage: 50.0,
                heap_size_mb: 2048,
                gc_count: 10,
            },
            disk_usage: crate::core::performance::metrics::DiskMetrics {
                total_gb: 1000,
                used_gb: 500,
                available_gb: 500,
                usage_percentage: 50.0,
                read_iops: 1000,
                write_iops: 500,
                read_throughput_mbps: 100.0,
                write_throughput_mbps: 50.0,
            },
            network_io: crate::core::performance::metrics::NetworkMetrics {
                bytes_sent: 1024000,
                bytes_received: 2048000,
                packets_sent: 1000,
                packets_received: 1500,
                connections_active: 150,
                connections_idle: 50,
                bandwidth_utilization: 60.0,
            },
            database_metrics: crate::core::performance::metrics::DatabaseMetrics {
                connections_active: 25,
                connections_idle: 75,
                connections_max: 100,
                query_count: 10000,
                slow_query_count: 2,
                average_query_time_ms: 50.0,
                transaction_count: 5000,
                deadlock_count: 0,
                cache_hit_ratio: 0.95,
            },
            request_metrics: crate::core::performance::metrics::RequestMetrics {
                total_requests: 10000,
                successful_requests: 9850,
                failed_requests: 150,
                average_response_time_ms: 200.0,
                p95_response_time_ms: 400.0,
                p99_response_time_ms: 800.0,
                requests_per_second: 500.0,
                concurrent_requests: 50,
            },
            custom_metrics: HashMap::new(),
        })
    }

    async fn evaluate_alert_rule(
        rule: &PerformanceAlertRule,
        metrics: &PerformanceMetrics,
        alert_manager: &Arc<dyn AlertManager>,
        self_healing_manager: &Arc<SelfHealingManager>,
        escalation_policies: &Arc<RwLock<HashMap<String, EscalationPolicy>>>,
        system_metrics: &Arc<PerformanceAlertingMetrics>,
        alert_history: &Arc<RwLock<Vec<AlertEvent>>>,
        config: &PerformanceAlertingConfig,
    ) -> Result<()> {
        // Extract metric value
        let current_value = Self::extract_metric_value(metrics, &rule.metric_path)?;

        // Check if threshold is exceeded
        let threshold_exceeded = match rule.comparison {
            ComparisonOperator::GreaterThan => current_value > rule.threshold,
            ComparisonOperator::LessThan => current_value < rule.threshold,
            ComparisonOperator::GreaterThanOrEqual => current_value >= rule.threshold,
            ComparisonOperator::LessThanOrEqual => current_value <= rule.threshold,
            ComparisonOperator::Equal => (current_value - rule.threshold).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (current_value - rule.threshold).abs() >= f64::EPSILON,
        };

        if threshold_exceeded {
            system_metrics.total_alerts_triggered.fetch_add(1, Ordering::Relaxed);

            // Create alert event
            let alert_event = AlertEvent {
                id: Uuid::new_v4(),
                rule_id: rule.id.clone(),
                event_type: AlertEventType::Triggered,
                timestamp: metrics.timestamp,
                severity: rule.severity.clone(),
                message: format!("{}: {} {} {}", rule.description, current_value, 
                               Self::comparison_operator_symbol(&rule.comparison), rule.threshold),
                current_value,
                threshold: rule.threshold,
                healing_attempted: false,
                healing_successful: None,
                escalation_level: 1,
            };

            // Store alert event
            {
                let mut history = alert_history.write().await;
                history.push(alert_event.clone());
            }

            // Attempt self-healing if enabled
            if config.enable_self_healing && rule.enable_self_healing && !rule.healing_actions.is_empty() {
                system_metrics.total_healing_attempts.fetch_add(1, Ordering::Relaxed);

                let detected_issue = DetectedIssue {
                    issue_id: Uuid::new_v4(),
                    rule_id: rule.id.clone(),
                    description: rule.description.clone(),
                    severity: Self::convert_severity(&rule.severity),
                    detected_at: Instant::now(),
                    current_value,
                    threshold: rule.threshold,
                    suggested_actions: rule.healing_actions.clone(),
                };

                // Record healing attempt
                {
                    let mut history = alert_history.write().await;
                    history.push(AlertEvent {
                        id: Uuid::new_v4(),
                        rule_id: rule.id.clone(),
                        event_type: AlertEventType::HealingAttempted,
                        timestamp: metrics.timestamp,
                        severity: rule.severity.clone(),
                        message: "Attempting automatic healing".to_string(),
                        current_value,
                        threshold: rule.threshold,
                        healing_attempted: true,
                        healing_successful: None,
                        escalation_level: 1,
                    });
                }

                // Note: In a real implementation, we would integrate with the self-healing manager
                // For now, we'll simulate the healing attempt
                tracing::info!("Would attempt self-healing for rule {} with actions: {:?}", 
                             rule.id, rule.healing_actions);
            }

            // Send alert notification
            let alert_rule = AlertRule {
                id: Uuid::new_v4(),
                name: rule.name.clone(),
                description: rule.description.clone(),
                condition: AlertCondition::Threshold {
                    metric_path: rule.metric_path.clone(),
                    operator: rule.comparison.clone(),
                    value: rule.threshold,
                    duration: Some(rule.duration),
                },
                severity: rule.severity.clone(),
                notification_channels: vec![
                    NotificationChannel::Log { level: "warn".to_string() }
                ],
                cooldown_duration: rule.cooldown,
                auto_resolve: rule.auto_resolve,
                enabled: rule.enabled,
                created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                updated_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            };

            alert_manager.trigger_alert(alert_rule, current_value).await?;
        }

        Ok(())
    }

    fn extract_metric_value(metrics: &PerformanceMetrics, path: &str) -> Result<f64> {
        match path {
            "cpu_usage" => Ok(metrics.cpu_usage),
            "memory_usage.usage_percentage" => Ok(metrics.memory_usage.usage_percentage),
            "memory_usage.used_mb" => Ok(metrics.memory_usage.used_mb as f64),
            "disk_usage.usage_percentage" => Ok(metrics.disk_usage.usage_percentage),
            "request_metrics.average_response_time_ms" => Ok(metrics.request_metrics.average_response_time_ms),
            "request_metrics.p95_response_time_ms" => Ok(metrics.request_metrics.p95_response_time_ms),
            "request_metrics.p99_response_time_ms" => Ok(metrics.request_metrics.p99_response_time_ms),
            "request_metrics.requests_per_second" => Ok(metrics.request_metrics.requests_per_second),
            "request_metrics.error_rate_percentage" => {
                let error_rate = if metrics.request_metrics.total_requests > 0 {
                    (metrics.request_metrics.failed_requests as f64 / metrics.request_metrics.total_requests as f64) * 100.0
                } else {
                    0.0
                };
                Ok(error_rate)
            },
            "database_metrics.average_query_time_ms" => Ok(metrics.database_metrics.average_query_time_ms),
            "database_metrics.connections_active" => Ok(metrics.database_metrics.connections_active as f64),
            _ => {
                if let Some(value) = metrics.custom_metrics.get(path) {
                    Ok(*value)
                } else {
                    Err(AppError::ValidationError(format!("Unknown metric path: {}", path)))
                }
            }
        }
    }

    fn comparison_operator_symbol(op: &ComparisonOperator) -> &'static str {
        match op {
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::GreaterThanOrEqual => ">=",
            ComparisonOperator::LessThanOrEqual => "<=",
            ComparisonOperator::Equal => "==",
            ComparisonOperator::NotEqual => "!=",
        }
    }

    fn convert_severity(severity: &IssueSeverity) -> crate::core::performance::self_healing::IssueSeverity {
        match severity {
            IssueSeverity::Low => crate::core::performance::self_healing::IssueSeverity::Low,
            IssueSeverity::Medium => crate::core::performance::self_healing::IssueSeverity::Medium,
            IssueSeverity::High => crate::core::performance::self_healing::IssueSeverity::High,
            IssueSeverity::Critical => crate::core::performance::self_healing::IssueSeverity::Critical,
            IssueSeverity::Info => crate::core::performance::self_healing::IssueSeverity::Low,
        }
    }

    async fn cleanup_alert_history(
        alert_history: &Arc<RwLock<Vec<AlertEvent>>>,
        max_history: usize,
    ) {
        let mut history = alert_history.write().await;
        
        if history.len() > max_history {
            let excess = history.len() - max_history;
            history.drain(0..excess);
        }
    }

    pub async fn add_alert_rule(&self, rule: PerformanceAlertRule) -> Result<()> {
        let mut rules = self.alert_rules.write().await;
        rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    pub async fn remove_alert_rule(&self, rule_id: &str) -> Result<()> {
        let mut rules = self.alert_rules.write().await;
        rules.remove(rule_id);
        Ok(())
    }

    pub async fn get_alert_rules(&self) -> Vec<PerformanceAlertRule> {
        let rules = self.alert_rules.read().await;
        rules.values().cloned().collect()
    }

    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<AlertEvent> {
        let history = self.alert_history.read().await;
        
        if let Some(limit) = limit {
            history.iter().rev().take(limit).cloned().collect()
        } else {
            history.clone()
        }
    }

    pub async fn get_metrics(&self) -> PerformanceAlertingMetrics {
        PerformanceAlertingMetrics {
            total_alerts_triggered: AtomicU64::new(self.metrics.total_alerts_triggered.load(Ordering::Relaxed)),
            total_alerts_resolved: AtomicU64::new(self.metrics.total_alerts_resolved.load(Ordering::Relaxed)),
            total_escalations: AtomicU64::new(self.metrics.total_escalations.load(Ordering::Relaxed)),
            total_healing_attempts: AtomicU64::new(self.metrics.total_healing_attempts.load(Ordering::Relaxed)),
            successful_healings: AtomicU64::new(self.metrics.successful_healings.load(Ordering::Relaxed)),
            failed_healings: AtomicU64::new(self.metrics.failed_healings.load(Ordering::Relaxed)),
            false_positive_alerts: AtomicU64::new(self.metrics.false_positive_alerts.load(Ordering::Relaxed)),
            average_resolution_time_ms: AtomicU64::new(self.metrics.average_resolution_time_ms.load(Ordering::Relaxed)),
        }
    }

    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping performance alerting system");

        if self.config.enable_self_healing {
            self.self_healing_manager.stop().await?;
        }

        tracing::info!("Performance alerting system stopped");
        Ok(())
    }
}