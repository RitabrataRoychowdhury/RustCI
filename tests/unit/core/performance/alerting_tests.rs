use rustci::core::performance::alerting::*;
use rustci::core::performance::metrics::*;
use std::collections::HashMap;
use std::time::Duration;
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test_alert_manager_creation() {
    let alert_manager = ProductionAlertManager::new(1000);
    assert!(alert_manager.initialize_default_rules().await.is_ok());
}

#[tokio::test]
async fn test_alert_rule_registration() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Test Rule".to_string(),
        description: "Test alert rule".to_string(),
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
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    let rule_id = rule.id;
    assert!(alert_manager.register_alert_rule(rule).await.is_ok());
    assert!(alert_manager.remove_alert_rule(&rule_id).await.is_ok());
}

#[tokio::test]
async fn test_threshold_condition_evaluation() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "CPU Alert".to_string(),
        description: "High CPU usage".to_string(),
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
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    // Test with high CPU usage - should trigger alert
    let mut high_cpu_metrics = PerformanceMetrics::new();
    high_cpu_metrics.cpu_usage = 90.0;

    let notifications = alert_manager.evaluate_alerts(&high_cpu_metrics).await.unwrap();
    assert!(!notifications.is_empty());
    assert_eq!(notifications[0].alert_type, AlertNotificationType::Triggered);

    // Test with normal CPU usage - should resolve alert if auto_resolve is true
    let mut normal_cpu_metrics = PerformanceMetrics::new();
    normal_cpu_metrics.cpu_usage = 50.0;

    let notifications = alert_manager.evaluate_alerts(&normal_cpu_metrics).await.unwrap();
    // Should have a resolution notification
    let resolved_notifications: Vec<_> = notifications.iter()
        .filter(|n| n.alert_type == AlertNotificationType::Resolved)
        .collect();
    assert!(!resolved_notifications.is_empty());
}

#[tokio::test]
async fn test_memory_threshold_evaluation() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Memory Alert".to_string(),
        description: "High memory usage".to_string(),
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
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    let mut high_memory_metrics = PerformanceMetrics::new();
    high_memory_metrics.memory_usage.usage_percentage = 90.0;

    let notifications = alert_manager.evaluate_alerts(&high_memory_metrics).await.unwrap();
    assert!(!notifications.is_empty());
    assert_eq!(notifications[0].alert_type, AlertNotificationType::Triggered);
}

#[tokio::test]
async fn test_response_time_threshold_evaluation() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Response Time Alert".to_string(),
        description: "High response time".to_string(),
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
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    let mut slow_response_metrics = PerformanceMetrics::new();
    slow_response_metrics.request_metrics.average_response_time_ms = 1500.0;

    let notifications = alert_manager.evaluate_alerts(&slow_response_metrics).await.unwrap();
    assert!(!notifications.is_empty());
    assert_eq!(notifications[0].alert_type, AlertNotificationType::Triggered);
}

#[tokio::test]
async fn test_composite_condition_and() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Composite AND Alert".to_string(),
        description: "High CPU and memory".to_string(),
        condition: AlertCondition::Composite {
            conditions: vec![
                AlertCondition::Threshold {
                    metric_path: "cpu_usage".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 80.0,
                    duration: None,
                },
                AlertCondition::Threshold {
                    metric_path: "memory_usage.usage_percentage".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 85.0,
                    duration: None,
                },
            ],
            operator: LogicalOperator::And,
        },
        severity: IssueSeverity::Critical,
        notification_channels: vec![
            NotificationChannel::Log { level: "error".to_string() },
        ],
        cooldown_duration: Duration::from_secs(600),
        auto_resolve: true,
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    // Test with both conditions met
    let mut high_both_metrics = PerformanceMetrics::new();
    high_both_metrics.cpu_usage = 90.0;
    high_both_metrics.memory_usage.usage_percentage = 90.0;

    let notifications = alert_manager.evaluate_alerts(&high_both_metrics).await.unwrap();
    assert!(!notifications.is_empty());

    // Test with only one condition met
    let mut high_cpu_only_metrics = PerformanceMetrics::new();
    high_cpu_only_metrics.cpu_usage = 90.0;
    high_cpu_only_metrics.memory_usage.usage_percentage = 50.0;

    let notifications = alert_manager.evaluate_alerts(&high_cpu_only_metrics).await.unwrap();
    // Should not trigger since AND requires both conditions
    let triggered_notifications: Vec<_> = notifications.iter()
        .filter(|n| n.alert_type == AlertNotificationType::Triggered)
        .collect();
    assert!(triggered_notifications.is_empty());
}

#[tokio::test]
async fn test_composite_condition_or() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Composite OR Alert".to_string(),
        description: "High CPU or memory".to_string(),
        condition: AlertCondition::Composite {
            conditions: vec![
                AlertCondition::Threshold {
                    metric_path: "cpu_usage".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 80.0,
                    duration: None,
                },
                AlertCondition::Threshold {
                    metric_path: "memory_usage.usage_percentage".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 85.0,
                    duration: None,
                },
            ],
            operator: LogicalOperator::Or,
        },
        severity: IssueSeverity::High,
        notification_channels: vec![
            NotificationChannel::Log { level: "warn".to_string() },
        ],
        cooldown_duration: Duration::from_secs(600),
        auto_resolve: true,
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    // Test with only CPU condition met
    let mut high_cpu_only_metrics = PerformanceMetrics::new();
    high_cpu_only_metrics.cpu_usage = 90.0;
    high_cpu_only_metrics.memory_usage.usage_percentage = 50.0;

    let notifications = alert_manager.evaluate_alerts(&high_cpu_only_metrics).await.unwrap();
    assert!(!notifications.is_empty());
}

#[tokio::test]
async fn test_custom_metric_threshold() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Custom Metric Alert".to_string(),
        description: "Custom metric threshold".to_string(),
        condition: AlertCondition::Threshold {
            metric_path: "custom_latency".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: 100.0,
            duration: None,
        },
        severity: IssueSeverity::Medium,
        notification_channels: vec![
            NotificationChannel::Log { level: "info".to_string() },
        ],
        cooldown_duration: Duration::from_secs(300),
        auto_resolve: true,
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    let mut custom_metrics = PerformanceMetrics::new();
    custom_metrics.add_custom_metric("custom_latency".to_string(), 150.0);

    let notifications = alert_manager.evaluate_alerts(&custom_metrics).await.unwrap();
    assert!(!notifications.is_empty());
}

#[tokio::test]
async fn test_disabled_rule_not_evaluated() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Disabled Rule".to_string(),
        description: "This rule is disabled".to_string(),
        condition: AlertCondition::Threshold {
            metric_path: "cpu_usage".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: 80.0,
            duration: None,
        },
        severity: IssueSeverity::High,
        notification_channels: vec![
            NotificationChannel::Log { level: "warn".to_string() },
        ],
        cooldown_duration: Duration::from_secs(600),
        auto_resolve: true,
        enabled: false, // Disabled
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    let mut high_cpu_metrics = PerformanceMetrics::new();
    high_cpu_metrics.cpu_usage = 90.0;

    let notifications = alert_manager.evaluate_alerts(&high_cpu_metrics).await.unwrap();
    assert!(notifications.is_empty());
}

#[tokio::test]
async fn test_alert_acknowledgment() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    let rule = AlertRule {
        id: Uuid::new_v4(),
        name: "Test Alert".to_string(),
        description: "Test alert for acknowledgment".to_string(),
        condition: AlertCondition::Threshold {
            metric_path: "cpu_usage".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: 80.0,
            duration: None,
        },
        severity: IssueSeverity::High,
        notification_channels: vec![
            NotificationChannel::Log { level: "warn".to_string() },
        ],
        cooldown_duration: Duration::from_secs(600),
        auto_resolve: false, // Don't auto-resolve
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule).await.is_ok());

    // Trigger alert
    let mut high_cpu_metrics = PerformanceMetrics::new();
    high_cpu_metrics.cpu_usage = 90.0;

    let notifications = alert_manager.evaluate_alerts(&high_cpu_metrics).await.unwrap();
    assert!(!notifications.is_empty());

    // Get active alerts
    let active_alerts = alert_manager.get_active_alerts().await.unwrap();
    assert!(!active_alerts.is_empty());

    let alert_id = active_alerts[0].id;
    assert!(!active_alerts[0].acknowledged);

    // Acknowledge the alert
    assert!(alert_manager.acknowledge_alert(&alert_id, "test_user").await.is_ok());

    // Check that alert is acknowledged
    let active_alerts = alert_manager.get_active_alerts().await.unwrap();
    let acknowledged_alert = active_alerts.iter().find(|a| a.id == alert_id).unwrap();
    assert!(acknowledged_alert.acknowledged);
    assert_eq!(acknowledged_alert.acknowledged_by, Some("test_user".to_string()));
}

#[tokio::test]
async fn test_notification_channels() {
    let notification = AlertNotification {
        id: Uuid::new_v4(),
        rule_id: Uuid::new_v4(),
        alert_type: AlertNotificationType::Triggered,
        severity: IssueSeverity::High,
        title: "Test Alert".to_string(),
        message: "Test alert message".to_string(),
        metrics_snapshot: PerformanceMetrics::new(),
        suggested_actions: vec!["Take action".to_string()],
        notification_channels: vec![
            NotificationChannel::Log { level: "warn".to_string() },
            NotificationChannel::Email { addresses: vec!["test@example.com".to_string()] },
            NotificationChannel::Slack { 
                webhook_url: "https://hooks.slack.com/test".to_string(),
                channel: "#alerts".to_string(),
            },
        ],
        created_at: 1234567890,
        escalation_level: 1,
    };

    let alert_manager = ProductionAlertManager::new(1000);
    assert!(alert_manager.send_alert(notification).await.is_ok());
}

#[tokio::test]
async fn test_comparison_operators() {
    let alert_manager = ProductionAlertManager::new(1000);

    // Test GreaterThan
    let rule_gt = AlertRule {
        id: Uuid::new_v4(),
        name: "GT Test".to_string(),
        description: "Greater than test".to_string(),
        condition: AlertCondition::Threshold {
            metric_path: "cpu_usage".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: 80.0,
            duration: None,
        },
        severity: IssueSeverity::Medium,
        notification_channels: vec![NotificationChannel::Log { level: "info".to_string() }],
        cooldown_duration: Duration::from_secs(300),
        auto_resolve: true,
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule_gt).await.is_ok());

    let mut metrics = PerformanceMetrics::new();
    metrics.cpu_usage = 85.0; // Should trigger

    let notifications = alert_manager.evaluate_alerts(&metrics).await.unwrap();
    assert!(!notifications.is_empty());

    // Test LessThan
    let rule_lt = AlertRule {
        id: Uuid::new_v4(),
        name: "LT Test".to_string(),
        description: "Less than test".to_string(),
        condition: AlertCondition::Threshold {
            metric_path: "cpu_usage".to_string(),
            operator: ComparisonOperator::LessThan,
            value: 20.0,
            duration: None,
        },
        severity: IssueSeverity::Low,
        notification_channels: vec![NotificationChannel::Log { level: "info".to_string() }],
        cooldown_duration: Duration::from_secs(300),
        auto_resolve: true,
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(rule_lt).await.is_ok());

    metrics.cpu_usage = 10.0; // Should trigger LT rule

    let notifications = alert_manager.evaluate_alerts(&metrics).await.unwrap();
    let triggered_notifications: Vec<_> = notifications.iter()
        .filter(|n| n.alert_type == AlertNotificationType::Triggered)
        .collect();
    assert!(!triggered_notifications.is_empty());
}

#[tokio::test]
async fn test_alert_suggested_actions() {
    let alert_manager = ProductionAlertManager::new(1000);
    
    // Test CPU alert suggestions
    let cpu_rule = AlertRule {
        id: Uuid::new_v4(),
        name: "CPU Alert".to_string(),
        description: "High CPU usage".to_string(),
        condition: AlertCondition::Threshold {
            metric_path: "cpu_usage".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: 80.0,
            duration: None,
        },
        severity: IssueSeverity::High,
        notification_channels: vec![NotificationChannel::Log { level: "warn".to_string() }],
        cooldown_duration: Duration::from_secs(600),
        auto_resolve: true,
        enabled: true,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert!(alert_manager.register_alert_rule(cpu_rule).await.is_ok());

    let mut high_cpu_metrics = PerformanceMetrics::new();
    high_cpu_metrics.cpu_usage = 90.0;

    let notifications = alert_manager.evaluate_alerts(&high_cpu_metrics).await.unwrap();
    assert!(!notifications.is_empty());
    
    let notification = &notifications[0];
    assert!(!notification.suggested_actions.is_empty());
    assert!(notification.suggested_actions.iter().any(|action| 
        action.contains("CPU-intensive") || action.contains("scaling")));
}