use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::core::performance::{
    PerformanceAlertingSystem, PerformanceAlertingConfig, PerformanceThresholds,
    NotificationSettings, PerformanceAlertRule, EscalationPolicy, EscalationStep,
    AlertEvent, AlertEventType, PerformanceAlertingMetrics, IssueSeverity,
    ComparisonOperator, NotificationChannel, HealingAction, SelfHealingManager,
    SelfHealingConfig, ProductionAlertManager, PerformanceMetrics
};
use crate::error::AppError;

// Mock AlertManager for testing
struct MockAlertManager {
    triggered_alerts: Arc<RwLock<Vec<(String, f64)>>>,
}

impl MockAlertManager {
    fn new() -> Self {
        Self {
            triggered_alerts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn get_triggered_alerts(&self) -> Vec<(String, f64)> {
        let alerts = self.triggered_alerts.read().await;
        alerts.clone()
    }
}

#[async_trait::async_trait]
impl crate::core::performance::AlertManager for MockAlertManager {
    async fn register_alert_rule(&self, _rule: crate::core::performance::AlertRule) -> Result<(), AppError> {
        Ok(())
    }

    async fn remove_alert_rule(&self, _rule_id: &Uuid) -> Result<(), AppError> {
        Ok(())
    }

    async fn evaluate_alerts(&self, _metrics: &PerformanceMetrics) -> Result<Vec<crate::core::performance::AlertNotification>, AppError> {
        Ok(vec![])
    }

    async fn send_alert(&self, _notification: crate::core::performance::AlertNotification) -> Result<(), AppError> {
        Ok(())
    }

    async fn acknowledge_alert(&self, _alert_id: &Uuid, _acknowledged_by: &str) -> Result<(), AppError> {
        Ok(())
    }

    async fn get_active_alerts(&self) -> Result<Vec<crate::core::performance::ActiveAlert>, AppError> {
        Ok(vec![])
    }

    async fn trigger_alert(&self, rule: crate::core::performance::AlertRule, current_value: f64) -> Result<(), AppError> {
        let mut alerts = self.triggered_alerts.write().await;
        alerts.push((rule.name, current_value));
        Ok(())
    }
}

fn create_test_config() -> PerformanceAlertingConfig {
    PerformanceAlertingConfig {
        enable_alerting: true,
        enable_self_healing: true,
        alert_evaluation_interval: Duration::from_millis(100),
        max_alert_history: 1000,
        default_escalation_timeout: Duration::from_secs(60),
        performance_thresholds: PerformanceThresholds {
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
        },
        notification_settings: NotificationSettings {
            enable_email: true,
            enable_slack: false,
            enable_pagerduty: false,
            enable_webhooks: false,
            email_addresses: vec!["admin@example.com".to_string()],
            slack_webhook_url: None,
            slack_channel: None,
            pagerduty_integration_key: None,
            webhook_urls: vec![],
        },
    }
}

#[tokio::test]
async fn test_performance_alerting_system_creation() {
    let config = create_test_config();
    let alert_manager = Arc::new(MockAlertManager::new());
    let self_healing_config = SelfHealingConfig::default();
    let self_healing_manager = Arc::new(SelfHealingManager::new(
        self_healing_config,
        Arc::new(ProductionAlertManager::new(1000))
    ));

    let alerting_system = PerformanceAlertingSystem::new(
        config,
        alert_manager,
        self_healing_manager,
    );

    // Test that the system was created successfully
    let metrics = alerting_system.get_metrics().await;
    assert_eq!(metrics.total_alerts_triggered.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_default_alert_rules_initialization() {
    let config = create_test_config();
    let alert_manager = Arc::new(MockAlertManager::new());
    let self_healing_config = SelfHealingConfig::default();
    let self_healing_manager = Arc::new(SelfHealingManager::new(
        self_healing_config,
        Arc::new(ProductionAlertManager::new(1000))
    ));

    let alerting_system = PerformanceAlertingSystem::new(
        config,
        alert_manager,
        self_healing_manager,
    );

    // Start the system to initialize default rules
    alerting_system.start().await.expect("Failed to start alerting system");

    // Check that default alert rules were created
    let rules = alerting_system.get_alert_rules().await;
    assert!(!rules.is_empty());

    // Verify specific rules exist
    let rule_ids: Vec<String> = rules.iter().map(|r| r.id.clone()).collect();
    assert!(rule_ids.contains(&"cpu_usage_warning".to_string()));
    assert!(rule_ids.contains(&"cpu_usage_critical".to_string()));
    assert!(rule_ids.contains(&"memory_usage_warning".to_string()));
    assert!(rule_ids.contains(&"memory_usage_critical".to_string()));
    assert!(rule_ids.contains(&"response_time_warning".to_string()));
    assert!(rule_ids.contains(&"response_time_critical".to_string()));

    alerting_system.stop().await.expect("Failed to stop alerting system");
}

#[tokio::test]
async fn test_alert_rule_management() {
    let config = create_test_config();
    let alert_manager = Arc::new(MockAlertManager::new());
    let self_healing_config = SelfHealingConfig::default();
    let self_healing_manager = Arc::new(SelfHealingManager::new(
        self_healing_config,
        Arc::new(ProductionAlertManager::new(1000))
    ));

    let alerting_system = PerformanceAlertingSystem::new(
        config,
        alert_manager,
        self_healing_manager,
    );

    // Add a custom alert rule
    let custom_rule = PerformanceAlertRule {
        id: "custom_test_rule".to_string(),
        name: "Custom Test Rule".to_string(),
        description: "Test rule for unit testing".to_string(),
        metric_path: "custom_metric".to_string(),
        threshold: 100.0,
        comparison: ComparisonOperator::GreaterThan,
        severity: IssueSeverity::Medium,
        duration: Duration::from_secs(60),
        cooldown: Duration::from_secs(300),
        auto_resolve: true,
        enable_self_healing: false,
        healing_actions: vec![],
        escalation_policy_id: None,
        enabled: true,
    };

    alerting_system.add_alert_rule(custom_rule.clone()).await.expect("Failed to add alert rule");

    // Verify the rule was added
    let rules = alerting_system.get_alert_rules().await;
    let added_rule = rules.iter().find(|r| r.id == "custom_test_rule");
    assert!(added_rule.is_some());
    assert_eq!(added_rule.unwrap().name, "Custom Test Rule");

    // Remove the rule
    alerting_system.remove_alert_rule("custom_test_rule").await.expect("Failed to remove alert rule");

    // Verify the rule was removed
    let rules = alerting_system.get_alert_rules().await;
    let removed_rule = rules.iter().find(|r| r.id == "custom_test_rule");
    assert!(removed_rule.is_none());
}

#[tokio::test]
async fn test_performance_thresholds() {
    let thresholds = PerformanceThresholds::default();

    // Test default values
    assert_eq!(thresholds.cpu_usage_warning, 70.0);
    assert_eq!(thresholds.cpu_usage_critical, 85.0);
    assert_eq!(thresholds.memory_usage_warning, 75.0);
    assert_eq!(thresholds.memory_usage_critical, 90.0);
    assert_eq!(thresholds.response_time_warning_ms, 1000.0);
    assert_eq!(thresholds.response_time_critical_ms, 3000.0);
    assert_eq!(thresholds.error_rate_warning, 5.0);
    assert_eq!(thresholds.error_rate_critical, 10.0);
    assert_eq!(thresholds.disk_usage_warning, 80.0);
    assert_eq!(thresholds.disk_usage_critical, 95.0);
}

#[tokio::test]
async fn test_notification_settings() {
    let settings = NotificationSettings::default();

    // Test default values
    assert!(!settings.enable_email);
    assert!(!settings.enable_slack);
    assert!(!settings.enable_pagerduty);
    assert!(!settings.enable_webhooks);
    assert!(settings.email_addresses.is_empty());
    assert!(settings.slack_webhook_url.is_none());
    assert!(settings.slack_channel.is_none());
    assert!(settings.pagerduty_integration_key.is_none());
    assert!(settings.webhook_urls.is_empty());
}

#[tokio::test]
async fn test_escalation_policy() {
    let policy = EscalationPolicy {
        id: "test_policy".to_string(),
        name: "Test Escalation Policy".to_string(),
        steps: vec![
            EscalationStep {
                delay: Duration::from_secs(0),
                notification_channels: vec![
                    NotificationChannel::Log { level: "warn".to_string() }
                ],
                auto_escalate: true,
            },
            EscalationStep {
                delay: Duration::from_secs(300),
                notification_channels: vec![
                    NotificationChannel::Email { 
                        addresses: vec!["admin@example.com".to_string()] 
                    }
                ],
                auto_escalate: false,
            },
        ],
    };

    assert_eq!(policy.id, "test_policy");
    assert_eq!(policy.steps.len(), 2);
    assert_eq!(policy.steps[0].delay, Duration::from_secs(0));
    assert!(policy.steps[0].auto_escalate);
    assert_eq!(policy.steps[1].delay, Duration::from_secs(300));
    assert!(!policy.steps[1].auto_escalate);
}

#[tokio::test]
async fn test_alert_event_creation() {
    let event = AlertEvent {
        id: Uuid::new_v4(),
        rule_id: "test_rule".to_string(),
        event_type: AlertEventType::Triggered,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        severity: IssueSeverity::High,
        message: "Test alert message".to_string(),
        current_value: 85.0,
        threshold: 80.0,
        healing_attempted: false,
        healing_successful: None,
        escalation_level: 1,
    };

    assert_eq!(event.rule_id, "test_rule");
    assert!(matches!(event.event_type, AlertEventType::Triggered));
    assert!(matches!(event.severity, IssueSeverity::High));
    assert_eq!(event.current_value, 85.0);
    assert_eq!(event.threshold, 80.0);
    assert!(!event.healing_attempted);
    assert!(event.healing_successful.is_none());
    assert_eq!(event.escalation_level, 1);
}

#[tokio::test]
async fn test_performance_alerting_metrics() {
    let metrics = PerformanceAlertingMetrics::default();

    // Test default values
    assert_eq!(metrics.total_alerts_triggered.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.total_alerts_resolved.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.total_escalations.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.total_healing_attempts.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.successful_healings.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.failed_healings.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.false_positive_alerts.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.average_resolution_time_ms.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_healing_actions_configuration() {
    let rule = PerformanceAlertRule {
        id: "test_healing_rule".to_string(),
        name: "Test Healing Rule".to_string(),
        description: "Rule with healing actions".to_string(),
        metric_path: "cpu_usage".to_string(),
        threshold: 80.0,
        comparison: ComparisonOperator::GreaterThan,
        severity: IssueSeverity::High,
        duration: Duration::from_secs(120),
        cooldown: Duration::from_secs(300),
        auto_resolve: true,
        enable_self_healing: true,
        healing_actions: vec![
            HealingAction::ScaleUp { target_instances: 2 },
            HealingAction::ReduceLoad { percentage: 20 },
            HealingAction::EnableCircuitBreaker { service: "api".to_string() },
        ],
        escalation_policy_id: Some("critical_escalation".to_string()),
        enabled: true,
    };

    assert!(rule.enable_self_healing);
    assert_eq!(rule.healing_actions.len(), 3);
    assert!(matches!(rule.healing_actions[0], HealingAction::ScaleUp { target_instances: 2 }));
    assert!(matches!(rule.healing_actions[1], HealingAction::ReduceLoad { percentage: 20 }));
    assert!(matches!(rule.healing_actions[2], HealingAction::EnableCircuitBreaker { .. }));
    assert_eq!(rule.escalation_policy_id, Some("critical_escalation".to_string()));
}

#[tokio::test]
async fn test_alert_history_management() {
    let config = create_test_config();
    let alert_manager = Arc::new(MockAlertManager::new());
    let self_healing_config = SelfHealingConfig::default();
    let self_healing_manager = Arc::new(SelfHealingManager::new(
        self_healing_config,
        Arc::new(ProductionAlertManager::new(1000))
    ));

    let alerting_system = PerformanceAlertingSystem::new(
        config,
        alert_manager,
        self_healing_manager,
    );

    // Initially, history should be empty
    let history = alerting_system.get_alert_history(None).await;
    assert!(history.is_empty());

    // Test with limit
    let limited_history = alerting_system.get_alert_history(Some(5)).await;
    assert!(limited_history.is_empty());
}

#[tokio::test]
async fn test_system_lifecycle() {
    let config = create_test_config();
    let alert_manager = Arc::new(MockAlertManager::new());
    let self_healing_config = SelfHealingConfig::default();
    let self_healing_manager = Arc::new(SelfHealingManager::new(
        self_healing_config,
        Arc::new(ProductionAlertManager::new(1000))
    ));

    let alerting_system = PerformanceAlertingSystem::new(
        config,
        alert_manager,
        self_healing_manager,
    );

    // Start the system
    alerting_system.start().await.expect("Failed to start alerting system");

    // Verify system is running by checking that default rules were initialized
    let rules = alerting_system.get_alert_rules().await;
    assert!(!rules.is_empty());

    // Stop the system
    alerting_system.stop().await.expect("Failed to stop alerting system");

    // System should still have the rules after stopping
    let rules_after_stop = alerting_system.get_alert_rules().await;
    assert_eq!(rules.len(), rules_after_stop.len());
}