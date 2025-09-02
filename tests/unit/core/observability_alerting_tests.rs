use rustci::core::observability::alerting::{
    AlertingSystem, AlertingConfig, AlertManager, NotificationDispatcher,
    EscalationEngine, MonitoringIntegrations, Alert, AlertSeverity, AlertStatus,
    AlertRule, AlertCondition, ComparisonOperator, AggregationType,
    NotificationRequest, NotificationPriority, EscalationPolicy, EscalationStep,
    MonitoringDashboard, DashboardPanel, PanelType, PanelPosition, PanelSize,
    SlackNotificationChannel, EmailNotificationChannel, EmailConfig,
    PrometheusIntegration, NotificationChannelType, MonitoringIntegrationType,
    MetricsQuery, TimeRange, AlertAction, AlertHistoryEntry,
};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[tokio::test]
async fn test_alerting_system_initialization() {
    let config = AlertingConfig {
        enable_notifications: true,
        enable_escalation: true,
        max_alerts_per_minute: 50,
        alert_retention_duration: Duration::from_secs(3600),
        ..AlertingConfig::default()
    };

    let alerting_system = AlertingSystem::new(config);
    
    // Test initial state
    let active_alerts = alerting_system.get_active_alerts().await;
    assert!(active_alerts.is_empty());
    
    let alert_history = alerting_system.get_alert_history(Duration::from_secs(3600)).await;
    assert!(alert_history.is_empty());
}

#[tokio::test]
async fn test_alert_creation_and_management() {
    let config = AlertingConfig::default();
    let alert_manager = AlertManager::new(config);

    // Create an alert
    let alert = Alert {
        alert_id: "test_alert_001".to_string(),
        rule_id: "cpu_high".to_string(),
        severity: AlertSeverity::Critical,
        status: AlertStatus::Firing,
        title: "High CPU Usage".to_string(),
        description: "CPU usage is above 90% for more than 5 minutes".to_string(),
        source_component: "server-01".to_string(),
        labels: {
            let mut labels = HashMap::new();
            labels.insert("instance".to_string(), "server-01".to_string());
            labels.insert("job".to_string(), "node-exporter".to_string());
            labels
        },
        annotations: {
            let mut annotations = HashMap::new();
            annotations.insert("summary".to_string(), "High CPU usage detected".to_string());
            annotations.insert("runbook_url".to_string(), "https://runbooks.example.com/cpu-high".to_string());
            annotations
        },
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        resolved_at: None,
        acknowledged_at: None,
        acknowledged_by: None,
        escalation_level: 0,
        notification_count: 0,
        related_alerts: Vec::new(),
        metrics: None,
    };

    // Create the alert
    let alert_id = alert_manager.create_alert(alert).await;
    assert!(alert_id.is_ok());
    let alert_id = alert_id.unwrap();
    assert_eq!(alert_id, "test_alert_001");

    // Verify alert is active
    let active_alerts = alert_manager.get_active_alerts().await;
    assert_eq!(active_alerts.len(), 1);
    assert_eq!(active_alerts[0].alert_id, "test_alert_001");
    assert_eq!(active_alerts[0].status, AlertStatus::Firing);

    // Acknowledge the alert
    let ack_result = alert_manager.acknowledge_alert(&alert_id, "admin").await;
    assert!(ack_result.is_ok());

    // Verify acknowledgment
    let active_alerts = alert_manager.get_active_alerts().await;
    assert_eq!(active_alerts[0].status, AlertStatus::Acknowledged);
    assert_eq!(active_alerts[0].acknowledged_by, Some("admin".to_string()));
    assert!(active_alerts[0].acknowledged_at.is_some());

    // Resolve the alert
    let resolve_result = alert_manager.resolve_alert(&alert_id, Some("admin")).await;
    assert!(resolve_result.is_ok());

    // Verify resolution
    let active_alerts = alert_manager.get_active_alerts().await;
    assert_eq!(active_alerts[0].status, AlertStatus::Resolved);
    assert!(active_alerts[0].resolved_at.is_some());

    // Check alert history
    let history = alert_manager.get_alert_history(Duration::from_secs(3600)).await;
    assert!(history.len() >= 3); // Created, Acknowledged, Resolved
    
    let created_entry = history.iter().find(|e| matches!(e.action, AlertAction::Created));
    assert!(created_entry.is_some());
    
    let ack_entry = history.iter().find(|e| matches!(e.action, AlertAction::Acknowledged));
    assert!(ack_entry.is_some());
    
    let resolved_entry = history.iter().find(|e| matches!(e.action, AlertAction::Resolved));
    assert!(resolved_entry.is_some());
}

#[tokio::test]
async fn test_notification_channels() {
    let config = AlertingConfig::default();
    let dispatcher = NotificationDispatcher::new(config);

    // Add Slack notification channel
    let slack_channel = Box::new(SlackNotificationChannel::new(
        "slack-alerts".to_string(),
        "https://hooks.slack.com/services/test/webhook".to_string(),
    ));
    
    let add_result = dispatcher.add_channel(slack_channel).await;
    assert!(add_result.is_ok());

    // Add Email notification channel
    let email_config = EmailConfig {
        smtp_server: "smtp.example.com".to_string(),
        smtp_port: 587,
        username: "alerts@example.com".to_string(),
        password: "password123".to_string(),
        from_address: "alerts@example.com".to_string(),
    };
    
    let email_channel = Box::new(EmailNotificationChannel::new(
        "email-alerts".to_string(),
        email_config,
    ));
    
    let add_result = dispatcher.add_channel(email_channel).await;
    assert!(add_result.is_ok());

    // Create notification request
    let alert = Alert {
        alert_id: "test_notification_alert".to_string(),
        rule_id: "memory_high".to_string(),
        severity: AlertSeverity::Warning,
        status: AlertStatus::Firing,
        title: "High Memory Usage".to_string(),
        description: "Memory usage is above 85%".to_string(),
        source_component: "server-02".to_string(),
        labels: HashMap::new(),
        annotations: HashMap::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        resolved_at: None,
        acknowledged_at: None,
        acknowledged_by: None,
        escalation_level: 0,
        notification_count: 0,
        related_alerts: Vec::new(),
        metrics: None,
    };

    let notification_request = NotificationRequest {
        request_id: Uuid::new_v4().to_string(),
        alert: alert.clone(),
        channel_name: "slack-alerts".to_string(),
        template: None,
        priority: NotificationPriority::High,
        retry_count: 0,
        created_at: SystemTime::now(),
    };

    // Send Slack notification
    let send_result = dispatcher.send_notification(notification_request).await;
    assert!(send_result.is_ok());

    // Send Email notification
    let email_notification_request = NotificationRequest {
        request_id: Uuid::new_v4().to_string(),
        alert,
        channel_name: "email-alerts".to_string(),
        template: None,
        priority: NotificationPriority::Normal,
        retry_count: 0,
        created_at: SystemTime::now(),
    };

    let send_result = dispatcher.send_notification(email_notification_request).await;
    assert!(send_result.is_ok());
}

#[tokio::test]
async fn test_escalation_policies() {
    let config = AlertingConfig::default();
    let escalation_engine = EscalationEngine::new(config);

    // Create escalation policy
    let escalation_policy = EscalationPolicy {
        policy_id: "critical_escalation".to_string(),
        name: "Critical Alert Escalation".to_string(),
        description: "Escalation policy for critical alerts".to_string(),
        enabled: true,
        escalation_steps: vec![
            EscalationStep {
                step_number: 1,
                delay: Duration::from_secs(300), // 5 minutes
                notification_channels: vec!["slack-alerts".to_string()],
                required_acknowledgment: false,
                auto_resolve_timeout: None,
            },
            EscalationStep {
                step_number: 2,
                delay: Duration::from_secs(900), // 15 minutes
                notification_channels: vec!["email-alerts".to_string(), "pager-duty".to_string()],
                required_acknowledgment: true,
                auto_resolve_timeout: Some(Duration::from_secs(3600)),
            },
            EscalationStep {
                step_number: 3,
                delay: Duration::from_secs(1800), // 30 minutes
                notification_channels: vec!["phone-alerts".to_string()],
                required_acknowledgment: true,
                auto_resolve_timeout: None,
            },
        ],
        repeat_escalation: false,
        max_escalation_level: 3,
    };

    let add_result = escalation_engine.add_policy(escalation_policy).await;
    assert!(add_result.is_ok());
}

#[tokio::test]
async fn test_monitoring_integrations() {
    let config = AlertingConfig::default();
    let monitoring = MonitoringIntegrations::new(config);

    // Add Prometheus integration
    let prometheus_integration = Box::new(PrometheusIntegration::new(
        "prometheus-main".to_string(),
        "http://prometheus:9090".to_string(),
    ));
    
    let add_result = monitoring.add_integration(prometheus_integration).await;
    assert!(add_result.is_ok());

    // Create monitoring dashboard
    let dashboard = MonitoringDashboard {
        dashboard_id: "system-overview".to_string(),
        title: "System Overview Dashboard".to_string(),
        description: "Overview of system health and performance".to_string(),
        tags: vec!["system".to_string(), "overview".to_string()],
        panels: vec![
            DashboardPanel {
                panel_id: "cpu-usage".to_string(),
                title: "CPU Usage".to_string(),
                panel_type: PanelType::Graph,
                queries: vec![
                    MetricsQuery {
                        query_id: "cpu_query".to_string(),
                        expression: "100 - (avg(irate(node_cpu_seconds_total{mode=\"idle\"}[5m])) * 100)".to_string(),
                        legend: Some("CPU Usage %".to_string()),
                        time_range: TimeRange {
                            start: SystemTime::now() - Duration::from_secs(3600),
                            end: SystemTime::now(),
                        },
                        step: Duration::from_secs(15),
                    }
                ],
                position: PanelPosition { x: 0, y: 0 },
                size: PanelSize { width: 12, height: 8 },
                thresholds: Vec::new(),
            },
            DashboardPanel {
                panel_id: "memory-usage".to_string(),
                title: "Memory Usage".to_string(),
                panel_type: PanelType::Graph,
                queries: vec![
                    MetricsQuery {
                        query_id: "memory_query".to_string(),
                        expression: "(1 - (node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)) * 100".to_string(),
                        legend: Some("Memory Usage %".to_string()),
                        time_range: TimeRange {
                            start: SystemTime::now() - Duration::from_secs(3600),
                            end: SystemTime::now(),
                        },
                        step: Duration::from_secs(15),
                    }
                ],
                position: PanelPosition { x: 0, y: 8 },
                size: PanelSize { width: 12, height: 8 },
                thresholds: Vec::new(),
            },
        ],
        variables: HashMap::new(),
        refresh_interval: Duration::from_secs(30),
        time_range: TimeRange {
            start: SystemTime::now() - Duration::from_secs(3600),
            end: SystemTime::now(),
        },
    };

    let dashboard_result = monitoring.create_dashboard(dashboard).await;
    assert!(dashboard_result.is_ok());
    assert_eq!(dashboard_result.unwrap(), "system-overview");
}

#[tokio::test]
async fn test_alert_rules() {
    // Test alert rule structure
    let alert_rule = AlertRule {
        rule_id: "high_cpu_usage".to_string(),
        name: "High CPU Usage Alert".to_string(),
        description: "Alert when CPU usage exceeds 90% for 5 minutes".to_string(),
        enabled: true,
        condition: AlertCondition {
            metric_name: "cpu_usage_percent".to_string(),
            operator: ComparisonOperator::GreaterThan,
            threshold: 90.0,
            aggregation: AggregationType::Average,
            time_window: Duration::from_secs(300), // 5 minutes
            filters: {
                let mut filters = HashMap::new();
                filters.insert("job".to_string(), "node-exporter".to_string());
                filters
            },
        },
        severity: AlertSeverity::Critical,
        labels: {
            let mut labels = HashMap::new();
            labels.insert("team".to_string(), "infrastructure".to_string());
            labels.insert("service".to_string(), "monitoring".to_string());
            labels
        },
        annotations: {
            let mut annotations = HashMap::new();
            annotations.insert("summary".to_string(), "High CPU usage detected on {{ $labels.instance }}".to_string());
            annotations.insert("description".to_string(), "CPU usage is {{ $value }}% on {{ $labels.instance }}".to_string());
            annotations
        },
        evaluation_interval: Duration::from_secs(60),
        for_duration: Duration::from_secs(300),
        escalation_policy: Some("critical_escalation".to_string()),
        notification_channels: vec!["slack-alerts".to_string(), "email-alerts".to_string()],
        auto_resolve: true,
    };

    // Verify rule structure
    assert_eq!(alert_rule.rule_id, "high_cpu_usage");
    assert!(alert_rule.enabled);
    assert_eq!(alert_rule.severity, AlertSeverity::Critical);
    assert_eq!(alert_rule.condition.threshold, 90.0);
    assert!(matches!(alert_rule.condition.operator, ComparisonOperator::GreaterThan));
    assert!(matches!(alert_rule.condition.aggregation, AggregationType::Average));
    assert!(!alert_rule.notification_channels.is_empty());
}

#[tokio::test]
async fn test_comprehensive_alerting_workflow() {
    let config = AlertingConfig {
        enable_notifications: true,
        enable_escalation: true,
        max_alerts_per_minute: 100,
        ..AlertingConfig::default()
    };

    let alerting_system = AlertingSystem::new(config);

    // Add notification channels
    let slack_channel = Box::new(SlackNotificationChannel::new(
        "ops-alerts".to_string(),
        "https://hooks.slack.com/services/ops/webhook".to_string(),
    ));
    alerting_system.add_notification_channel(slack_channel).await.unwrap();

    let email_config = EmailConfig {
        smtp_server: "smtp.company.com".to_string(),
        smtp_port: 587,
        username: "alerts@company.com".to_string(),
        password: "secure_password".to_string(),
        from_address: "noreply@company.com".to_string(),
    };
    let email_channel = Box::new(EmailNotificationChannel::new(
        "ops-email".to_string(),
        email_config,
    ));
    alerting_system.add_notification_channel(email_channel).await.unwrap();

    // Add escalation policy
    let escalation_policy = EscalationPolicy {
        policy_id: "ops_escalation".to_string(),
        name: "Operations Team Escalation".to_string(),
        description: "Standard escalation for operations alerts".to_string(),
        enabled: true,
        escalation_steps: vec![
            EscalationStep {
                step_number: 1,
                delay: Duration::from_secs(0), // Immediate
                notification_channels: vec!["ops-alerts".to_string()],
                required_acknowledgment: false,
                auto_resolve_timeout: None,
            },
            EscalationStep {
                step_number: 2,
                delay: Duration::from_secs(600), // 10 minutes
                notification_channels: vec!["ops-email".to_string()],
                required_acknowledgment: true,
                auto_resolve_timeout: Some(Duration::from_secs(1800)),
            },
        ],
        repeat_escalation: false,
        max_escalation_level: 2,
    };
    alerting_system.add_escalation_policy(escalation_policy).await.unwrap();

    // Add monitoring integration
    let prometheus_integration = Box::new(PrometheusIntegration::new(
        "main-prometheus".to_string(),
        "http://prometheus.company.com:9090".to_string(),
    ));
    alerting_system.add_monitoring_integration(prometheus_integration).await.unwrap();

    // Create and send alert
    let alert = Alert {
        alert_id: "workflow_test_alert".to_string(),
        rule_id: "disk_space_low".to_string(),
        severity: AlertSeverity::Warning,
        status: AlertStatus::Firing,
        title: "Low Disk Space".to_string(),
        description: "Disk space is below 10% on /var partition".to_string(),
        source_component: "web-server-01".to_string(),
        labels: {
            let mut labels = HashMap::new();
            labels.insert("instance".to_string(), "web-server-01".to_string());
            labels.insert("mountpoint".to_string(), "/var".to_string());
            labels
        },
        annotations: {
            let mut annotations = HashMap::new();
            annotations.insert("summary".to_string(), "Disk space critically low".to_string());
            annotations
        },
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        resolved_at: None,
        acknowledged_at: None,
        acknowledged_by: None,
        escalation_level: 0,
        notification_count: 0,
        related_alerts: Vec::new(),
        metrics: None,
    };

    // Create the alert
    let alert_id = alerting_system.create_alert(alert).await.unwrap();
    assert_eq!(alert_id, "workflow_test_alert");

    // Send notification
    let notification_request = NotificationRequest {
        request_id: Uuid::new_v4().to_string(),
        alert: alerting_system.get_active_alerts().await[0].clone(),
        channel_name: "ops-alerts".to_string(),
        template: None,
        priority: NotificationPriority::High,
        retry_count: 0,
        created_at: SystemTime::now(),
    };

    let send_result = alerting_system.send_notification(notification_request).await;
    assert!(send_result.is_ok());

    // Acknowledge alert
    let ack_result = alerting_system.acknowledge_alert(&alert_id, "ops_engineer").await;
    assert!(ack_result.is_ok());

    // Resolve alert
    let resolve_result = alerting_system.resolve_alert(&alert_id, Some("ops_engineer")).await;
    assert!(resolve_result.is_ok());

    // Verify final state
    let active_alerts = alerting_system.get_active_alerts().await;
    assert_eq!(active_alerts.len(), 1);
    assert_eq!(active_alerts[0].status, AlertStatus::Resolved);
    assert_eq!(active_alerts[0].acknowledged_by, Some("ops_engineer".to_string()));

    // Check history
    let history = alerting_system.get_alert_history(Duration::from_secs(3600)).await;
    assert!(history.len() >= 3); // Created, Acknowledged, Resolved
}

#[tokio::test]
async fn test_notification_channel_types() {
    // Test different notification channel types
    let slack_channel = SlackNotificationChannel::new(
        "test-slack".to_string(),
        "https://hooks.slack.com/test".to_string(),
    );
    assert_eq!(slack_channel.get_channel_type(), NotificationChannelType::Slack);
    assert_eq!(slack_channel.get_name(), "test-slack");
    assert!(slack_channel.is_enabled());

    let email_config = EmailConfig {
        smtp_server: "smtp.test.com".to_string(),
        smtp_port: 587,
        username: "test@test.com".to_string(),
        password: "password".to_string(),
        from_address: "noreply@test.com".to_string(),
    };
    let email_channel = EmailNotificationChannel::new(
        "test-email".to_string(),
        email_config,
    );
    assert_eq!(email_channel.get_channel_type(), NotificationChannelType::Email);
    assert_eq!(email_channel.get_name(), "test-email");
    assert!(email_channel.is_enabled());
}

#[tokio::test]
async fn test_monitoring_integration_types() {
    let prometheus_integration = PrometheusIntegration::new(
        "test-prometheus".to_string(),
        "http://localhost:9090".to_string(),
    );
    
    assert_eq!(prometheus_integration.get_integration_type(), MonitoringIntegrationType::Prometheus);
    assert_eq!(prometheus_integration.get_name(), "test-prometheus");

    // Test metrics query
    let query = MetricsQuery {
        query_id: "test_query".to_string(),
        expression: "up".to_string(),
        legend: Some("Instance Status".to_string()),
        time_range: TimeRange {
            start: SystemTime::now() - Duration::from_secs(300),
            end: SystemTime::now(),
        },
        step: Duration::from_secs(15),
    };

    let metrics_result = prometheus_integration.get_metrics(&query).await;
    assert!(metrics_result.is_ok());
    
    let result = metrics_result.unwrap();
    assert_eq!(result.query_id, "test_query");
    assert!(!result.data_points.is_empty());
    assert!(result.execution_time > Duration::from_millis(0));
}

#[tokio::test]
async fn test_alert_severity_ordering() {
    // Test that alert severities are properly ordered
    let severities = vec![
        AlertSeverity::Info,
        AlertSeverity::Warning,
        AlertSeverity::Critical,
        AlertSeverity::Emergency,
    ];

    for i in 0..severities.len() - 1 {
        assert!(severities[i] < severities[i + 1]);
    }

    // Test specific comparisons
    assert!(AlertSeverity::Info < AlertSeverity::Warning);
    assert!(AlertSeverity::Warning < AlertSeverity::Critical);
    assert!(AlertSeverity::Critical < AlertSeverity::Emergency);
}

#[tokio::test]
async fn test_notification_priority_ordering() {
    // Test that notification priorities are properly ordered
    let priorities = vec![
        NotificationPriority::Low,
        NotificationPriority::Normal,
        NotificationPriority::High,
        NotificationPriority::Critical,
    ];

    for i in 0..priorities.len() - 1 {
        assert!(priorities[i] < priorities[i + 1]);
    }

    // Test specific comparisons
    assert!(NotificationPriority::Low < NotificationPriority::Normal);
    assert!(NotificationPriority::Normal < NotificationPriority::High);
    assert!(NotificationPriority::High < NotificationPriority::Critical);
}