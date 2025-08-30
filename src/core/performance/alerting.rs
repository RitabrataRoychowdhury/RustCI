use crate::core::performance::metrics::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

// Re-export IssueSeverity from metrics for consistency
pub use crate::core::performance::metrics::IssueSeverity;

#[async_trait]
pub trait AlertManager: Send + Sync {
    async fn register_alert_rule(&self, rule: AlertRule) -> Result<()>;
    async fn remove_alert_rule(&self, rule_id: &Uuid) -> Result<()>;
    async fn evaluate_alerts(&self, metrics: &PerformanceMetrics) -> Result<Vec<AlertNotification>>;
    async fn send_alert(&self, notification: AlertNotification) -> Result<()>;
    async fn acknowledge_alert(&self, alert_id: &Uuid, acknowledged_by: &str) -> Result<()>;
    async fn get_active_alerts(&self) -> Result<Vec<ActiveAlert>>;
    async fn trigger_alert(&self, rule: AlertRule, current_value: f64) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub condition: AlertCondition,
    pub severity: IssueSeverity,
    pub notification_channels: Vec<NotificationChannel>,
    pub cooldown_duration: Duration,
    pub auto_resolve: bool,
    pub enabled: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    Threshold {
        metric_path: String,
        operator: ComparisonOperator,
        value: f64,
        duration: Option<Duration>,
    },
    Composite {
        conditions: Vec<AlertCondition>,
        operator: LogicalOperator,
    },
    RateOfChange {
        metric_path: String,
        change_threshold: f64,
        time_window: Duration,
    },
    Anomaly {
        metric_path: String,
        sensitivity: f64,
        baseline_duration: Duration,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalOperator {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email { addresses: Vec<String> },
    Slack { webhook_url: String, channel: String },
    PagerDuty { integration_key: String },
    Webhook { url: String, headers: HashMap<String, String> },
    Log { level: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertNotification {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub alert_type: AlertNotificationType,
    pub severity: IssueSeverity,
    pub title: String,
    pub message: String,
    pub metrics_snapshot: PerformanceMetrics,
    pub suggested_actions: Vec<String>,
    pub notification_channels: Vec<NotificationChannel>,
    pub created_at: u64,
    pub escalation_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertNotificationType {
    Triggered,
    Resolved,
    Escalated,
    Acknowledged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAlert {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub triggered_at: u64,
    pub last_notification_at: u64,
    pub escalation_level: u32,
    pub acknowledged: bool,
    pub acknowledged_by: Option<String>,
    pub acknowledged_at: Option<u64>,
    pub current_value: f64,
    pub threshold_value: f64,
    pub notification_count: u32,
}

pub struct ProductionAlertManager {
    alert_rules: Arc<RwLock<HashMap<Uuid, AlertRule>>>,
    active_alerts: Arc<RwLock<HashMap<Uuid, ActiveAlert>>>,
    notification_history: Arc<RwLock<Vec<AlertNotification>>>,
    max_history_size: usize,
}

impl ProductionAlertManager {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            alert_rules: Arc::new(RwLock::new(HashMap::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            notification_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
        }
    }

    pub async fn initialize_default_rules(&self) -> Result<()> {
        let default_rules = vec![
            AlertRule {
                id: Uuid::new_v4(),
                name: "High CPU Usage".to_string(),
                description: "Alert when CPU usage exceeds 80%".to_string(),
                condition: AlertCondition::Threshold {
                    metric_path: "cpu_usage".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 80.0,
                    duration: Some(Duration::from_secs(300)),
                },
                severity: IssueSeverity::High,
                notification_channels: vec![
                    NotificationChannel::Log { level: "warn".to_string() },
                ],
                cooldown_duration: Duration::from_secs(600),
                auto_resolve: true,
                enabled: true,
                created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                updated_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            },
            AlertRule {
                id: Uuid::new_v4(),
                name: "High Memory Usage".to_string(),
                description: "Alert when memory usage exceeds 85%".to_string(),
                condition: AlertCondition::Threshold {
                    metric_path: "memory_usage.usage_percentage".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 85.0,
                    duration: Some(Duration::from_secs(180)),
                },
                severity: IssueSeverity::High,
                notification_channels: vec![
                    NotificationChannel::Log { level: "warn".to_string() },
                ],
                cooldown_duration: Duration::from_secs(600),
                auto_resolve: true,
                enabled: true,
                created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                updated_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            },
            AlertRule {
                id: Uuid::new_v4(),
                name: "High Response Time".to_string(),
                description: "Alert when average response time exceeds 1000ms".to_string(),
                condition: AlertCondition::Threshold {
                    metric_path: "request_metrics.average_response_time_ms".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 1000.0,
                    duration: Some(Duration::from_secs(120)),
                },
                severity: IssueSeverity::Medium,
                notification_channels: vec![
                    NotificationChannel::Log { level: "warn".to_string() },
                ],
                cooldown_duration: Duration::from_secs(300),
                auto_resolve: true,
                enabled: true,
                created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                updated_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            },
        ];

        for rule in default_rules {
            self.register_alert_rule(rule).await?;
        }

        Ok(())
    }

    async fn evaluate_condition(&self, condition: &AlertCondition, metrics: &PerformanceMetrics) -> Result<bool> {
        match condition {
            AlertCondition::Threshold { metric_path, operator, value, .. } => {
                let metric_value = self.extract_metric_value(metrics, metric_path)?;
                Ok(self.compare_values(metric_value, *value, operator))
            }
            AlertCondition::Composite { conditions, operator } => {
                let results: Result<Vec<bool>> = futures::future::try_join_all(
                    conditions.iter().map(|c| self.evaluate_condition(c, metrics))
                ).await;
                
                match results {
                    Ok(results) => Ok(self.apply_logical_operator(&results, operator)),
                    Err(e) => Err(e),
                }
            }
            AlertCondition::RateOfChange { metric_path, change_threshold, .. } => {
                // For now, return false as we need historical data for rate of change
                // In a full implementation, this would compare current vs historical values
                let _metric_value = self.extract_metric_value(metrics, metric_path)?;
                Ok(false)
            }
            AlertCondition::Anomaly { metric_path, .. } => {
                // For now, return false as anomaly detection requires baseline data
                // In a full implementation, this would use statistical analysis
                let _metric_value = self.extract_metric_value(metrics, metric_path)?;
                Ok(false)
            }
        }
    }

    fn extract_metric_value(&self, metrics: &PerformanceMetrics, path: &str) -> Result<f64> {
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

    fn compare_values(&self, actual: f64, threshold: f64, operator: &ComparisonOperator) -> bool {
        match operator {
            ComparisonOperator::GreaterThan => actual > threshold,
            ComparisonOperator::LessThan => actual < threshold,
            ComparisonOperator::GreaterThanOrEqual => actual >= threshold,
            ComparisonOperator::LessThanOrEqual => actual <= threshold,
            ComparisonOperator::Equal => (actual - threshold).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (actual - threshold).abs() >= f64::EPSILON,
        }
    }

    fn apply_logical_operator(&self, results: &[bool], operator: &LogicalOperator) -> bool {
        match operator {
            LogicalOperator::And => results.iter().all(|&r| r),
            LogicalOperator::Or => results.iter().any(|&r| r),
            LogicalOperator::Not => results.len() == 1 && !results[0],
        }
    }

    async fn create_notification(&self, rule: &AlertRule, metrics: &PerformanceMetrics, current_value: f64) -> AlertNotification {
        let threshold_value = match &rule.condition {
            AlertCondition::Threshold { value, .. } => *value,
            _ => 0.0,
        };

        AlertNotification {
            id: Uuid::new_v4(),
            rule_id: rule.id,
            alert_type: AlertNotificationType::Triggered,
            severity: rule.severity.clone(),
            title: format!("Alert: {}", rule.name),
            message: format!("{} - Current value: {:.2}, Threshold: {:.2}", 
                           rule.description, current_value, threshold_value),
            metrics_snapshot: metrics.clone(),
            suggested_actions: self.get_suggested_actions(&rule.condition),
            notification_channels: rule.notification_channels.clone(),
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            escalation_level: 1,
        }
    }

    fn get_suggested_actions(&self, condition: &AlertCondition) -> Vec<String> {
        match condition {
            AlertCondition::Threshold { metric_path, .. } => {
                match metric_path.as_str() {
                    "cpu_usage" => vec![
                        "Check for CPU-intensive processes".to_string(),
                        "Consider scaling out application instances".to_string(),
                        "Review recent code changes for performance issues".to_string(),
                    ],
                    "memory_usage.usage_percentage" => vec![
                        "Check for memory leaks".to_string(),
                        "Review memory-intensive operations".to_string(),
                        "Consider increasing available memory".to_string(),
                    ],
                    "request_metrics.average_response_time_ms" => vec![
                        "Check database query performance".to_string(),
                        "Review API endpoint performance".to_string(),
                        "Consider implementing caching".to_string(),
                    ],
                    _ => vec!["Review system performance and resource usage".to_string()],
                }
            }
            _ => vec!["Review system performance and resource usage".to_string()],
        }
    }

    async fn store_notification(&self, notification: AlertNotification) -> Result<()> {
        let mut history = self.notification_history.write().await;
        
        if history.len() >= self.max_history_size {
            history.remove(0);
        }
        
        history.push(notification);
        Ok(())
    }
}

#[async_trait]
impl AlertManager for ProductionAlertManager {
    async fn register_alert_rule(&self, rule: AlertRule) -> Result<()> {
        let mut rules = self.alert_rules.write().await;
        rules.insert(rule.id, rule);
        Ok(())
    }

    async fn remove_alert_rule(&self, rule_id: &Uuid) -> Result<()> {
        let mut rules = self.alert_rules.write().await;
        rules.remove(rule_id);
        
        // Also remove any active alerts for this rule
        let mut active_alerts = self.active_alerts.write().await;
        active_alerts.retain(|_, alert| alert.rule_id != *rule_id);
        
        Ok(())
    }

    async fn evaluate_alerts(&self, metrics: &PerformanceMetrics) -> Result<Vec<AlertNotification>> {
        let mut notifications = Vec::new();
        let rules = self.alert_rules.read().await;
        let mut active_alerts = self.active_alerts.write().await;

        for rule in rules.values() {
            if !rule.enabled {
                continue;
            }

            let condition_met = self.evaluate_condition(&rule.condition, metrics).await?;
            let current_value = match &rule.condition {
                AlertCondition::Threshold { metric_path, .. } => {
                    self.extract_metric_value(metrics, metric_path)?
                }
                _ => 0.0,
            };

            if condition_met {
                // Check if we already have an active alert for this rule
                let existing_alert = active_alerts.values().find(|a| a.rule_id == rule.id);
                
                if existing_alert.is_none() {
                    // Create new active alert
                    let alert_id = Uuid::new_v4();
                    let active_alert = ActiveAlert {
                        id: alert_id,
                        rule_id: rule.id,
                        triggered_at: metrics.timestamp,
                        last_notification_at: metrics.timestamp,
                        escalation_level: 1,
                        acknowledged: false,
                        acknowledged_by: None,
                        acknowledged_at: None,
                        current_value,
                        threshold_value: match &rule.condition {
                            AlertCondition::Threshold { value, .. } => *value,
                            _ => 0.0,
                        },
                        notification_count: 1,
                    };

                    active_alerts.insert(alert_id, active_alert);
                    
                    // Create notification
                    let notification = self.create_notification(rule, metrics, current_value).await;
                    notifications.push(notification);
                }
            } else {
                // Check if we should resolve any active alerts for this rule
                if let Some(alert) = active_alerts.values().find(|a| a.rule_id == rule.id).cloned() {
                    if rule.auto_resolve {
                        active_alerts.remove(&alert.id);
                        
                        // Create resolution notification
                        let notification = AlertNotification {
                            id: Uuid::new_v4(),
                            rule_id: rule.id,
                            alert_type: AlertNotificationType::Resolved,
                            severity: rule.severity.clone(),
                            title: format!("Resolved: {}", rule.name),
                            message: format!("{} - Alert condition no longer met", rule.description),
                            metrics_snapshot: metrics.clone(),
                            suggested_actions: vec![],
                            notification_channels: rule.notification_channels.clone(),
                            created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                            escalation_level: alert.escalation_level,
                        };
                        notifications.push(notification);
                    }
                }
            }
        }

        Ok(notifications)
    }

    async fn send_alert(&self, notification: AlertNotification) -> Result<()> {
        // Store the notification
        self.store_notification(notification.clone()).await?;

        // Send to each notification channel
        for channel in &notification.notification_channels {
            match channel {
                NotificationChannel::Log { level } => {
                    match level.as_str() {
                        "error" => error!("ALERT: {} - {}", notification.title, notification.message),
                        "warn" => warn!("ALERT: {} - {}", notification.title, notification.message),
                        "info" => info!("ALERT: {} - {}", notification.title, notification.message),
                        _ => info!("ALERT: {} - {}", notification.title, notification.message),
                    }
                }
                NotificationChannel::Email { addresses } => {
                    // In a real implementation, this would send emails
                    info!("Would send email alert to {:?}: {}", addresses, notification.title);
                }
                NotificationChannel::Slack { webhook_url, channel } => {
                    // In a real implementation, this would send to Slack
                    info!("Would send Slack alert to {} ({}): {}", channel, webhook_url, notification.title);
                }
                NotificationChannel::PagerDuty { integration_key } => {
                    // In a real implementation, this would send to PagerDuty
                    info!("Would send PagerDuty alert ({}): {}", integration_key, notification.title);
                }
                NotificationChannel::Webhook { url, headers: _ } => {
                    // In a real implementation, this would send HTTP webhook
                    info!("Would send webhook alert to {}: {}", url, notification.title);
                }
            }
        }

        Ok(())
    }

    async fn acknowledge_alert(&self, alert_id: &Uuid, acknowledged_by: &str) -> Result<()> {
        let mut active_alerts = self.active_alerts.write().await;
        
        if let Some(alert) = active_alerts.get_mut(alert_id) {
            alert.acknowledged = true;
            alert.acknowledged_by = Some(acknowledged_by.to_string());
            alert.acknowledged_at = Some(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs());
            
            info!("Alert {} acknowledged by {}", alert_id, acknowledged_by);
        }

        Ok(())
    }

    async fn get_active_alerts(&self) -> Result<Vec<ActiveAlert>> {
        let active_alerts = self.active_alerts.read().await;
        Ok(active_alerts.values().cloned().collect())
    }

    async fn trigger_alert(&self, rule: AlertRule, current_value: f64) -> Result<()> {
        // Create a dummy metrics object for the notification
        let metrics = PerformanceMetrics {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            correlation_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: crate::core::performance::metrics::MemoryMetrics {
                total_mb: 0,
                used_mb: 0,
                available_mb: 0,
                usage_percentage: 0.0,
                heap_size_mb: 0,
                gc_count: 0,
            },
            disk_usage: crate::core::performance::metrics::DiskMetrics {
                total_gb: 0,
                used_gb: 0,
                available_gb: 0,
                usage_percentage: 0.0,
                read_iops: 0,
                write_iops: 0,
                read_throughput_mbps: 0.0,
                write_throughput_mbps: 0.0,
            },
            network_io: crate::core::performance::metrics::NetworkMetrics {
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                connections_active: 0,
                connections_idle: 0,
                bandwidth_utilization: 0.0,
            },
            database_metrics: crate::core::performance::metrics::DatabaseMetrics {
                connections_active: 0,
                connections_idle: 0,
                connections_max: 0,
                query_count: 0,
                slow_query_count: 0,
                average_query_time_ms: 0.0,
                transaction_count: 0,
                deadlock_count: 0,
                cache_hit_ratio: 0.0,
            },
            request_metrics: crate::core::performance::metrics::RequestMetrics {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                average_response_time_ms: 0.0,
                p95_response_time_ms: 0.0,
                p99_response_time_ms: 0.0,
                requests_per_second: 0.0,
                concurrent_requests: 0,
            },
            custom_metrics: std::collections::HashMap::new(),
        };

        let notification = self.create_notification(&rule, &metrics, current_value).await;
        self.send_alert(notification).await
    }
}