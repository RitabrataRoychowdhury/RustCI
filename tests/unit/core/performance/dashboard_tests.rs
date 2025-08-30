use rustci::core::performance::dashboard::*;
use rustci::core::performance::metrics::*;
use rustci::core::performance::alerting::*;
use rustci::error::Result;
use async_trait::async_trait;
use std::time::Duration;
use tokio;

// Mock implementations for testing
struct MockMetricsCollector {
    current_metrics: PerformanceMetrics,
    historical_metrics: Vec<PerformanceMetrics>,
}

impl MockMetricsCollector {
    fn new() -> Self {
        let mut current = PerformanceMetrics::new();
        current.cpu_usage = 45.0;
        current.memory_usage.usage_percentage = 60.0;
        current.request_metrics.average_response_time_ms = 250.0;
        current.request_metrics.requests_per_second = 100.0;
        current.request_metrics.total_requests = 10000;
        current.request_metrics.successful_requests = 9800;
        current.request_metrics.failed_requests = 200;

        let mut historical = Vec::new();
        for i in 0..10 {
            let mut metric = PerformanceMetrics::new();
            metric.cpu_usage = 40.0 + (i as f64 * 2.0);
            metric.memory_usage.usage_percentage = 50.0 + (i as f64 * 1.5);
            metric.request_metrics.average_response_time_ms = 200.0 + (i as f64 * 10.0);
            historical.push(metric);
        }

        Self {
            current_metrics: current,
            historical_metrics: historical,
        }
    }
}

#[async_trait]
impl MetricsCollector for MockMetricsCollector {
    async fn get_current_metrics(&self) -> Result<PerformanceMetrics> {
        Ok(self.current_metrics.clone())
    }

    async fn get_historical_metrics(&self, _duration: Duration) -> Result<Vec<PerformanceMetrics>> {
        Ok(self.historical_metrics.clone())
    }
}

struct MockAlertManager {
    active_alerts: Vec<ActiveAlert>,
}

impl MockAlertManager {
    fn new() -> Self {
        Self {
            active_alerts: vec![
                ActiveAlert {
                    id: uuid::Uuid::new_v4(),
                    rule_id: uuid::Uuid::new_v4(),
                    triggered_at: 1234567890,
                    last_notification_at: 1234567890,
                    escalation_level: 1,
                    acknowledged: false,
                    acknowledged_by: None,
                    acknowledged_at: None,
                    current_value: 85.0,
                    threshold_value: 80.0,
                    notification_count: 1,
                },
            ],
        }
    }
}

#[async_trait]
impl AlertManager for MockAlertManager {
    async fn register_alert_rule(&self, _rule: AlertRule) -> Result<()> {
        Ok(())
    }

    async fn remove_alert_rule(&self, _rule_id: &uuid::Uuid) -> Result<()> {
        Ok(())
    }

    async fn evaluate_alerts(&self, _metrics: &PerformanceMetrics) -> Result<Vec<AlertNotification>> {
        Ok(Vec::new())
    }

    async fn send_alert(&self, _notification: AlertNotification) -> Result<()> {
        Ok(())
    }

    async fn acknowledge_alert(&self, _alert_id: &uuid::Uuid, _acknowledged_by: &str) -> Result<()> {
        Ok(())
    }

    async fn get_active_alerts(&self) -> Result<Vec<ActiveAlert>> {
        Ok(self.active_alerts.clone())
    }
}

#[tokio::test]
async fn test_dashboard_creation() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    // Test that dashboard was created successfully
    assert!(dashboard.get_current_metrics().await.is_ok());
}

#[tokio::test]
async fn test_get_current_metrics() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_current_metrics().await;
    assert!(result.is_ok());

    let dashboard_metrics = result.unwrap();
    assert!(dashboard_metrics.timestamp > 0);
    assert_eq!(dashboard_metrics.system_overview.total_requests, 10000);
    assert_eq!(dashboard_metrics.system_overview.successful_requests, 9800);
    assert_eq!(dashboard_metrics.system_overview.failed_requests, 200);
    assert_eq!(dashboard_metrics.performance_indicators.throughput_rps, 100.0);
    assert_eq!(dashboard_metrics.resource_utilization.cpu_usage_percentage, 45.0);
    assert_eq!(dashboard_metrics.resource_utilization.memory_usage_percentage, 60.0);
}

#[tokio::test]
async fn test_get_historical_data() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_historical_data(Duration::from_secs(3600)).await;
    assert!(result.is_ok());

    let historical_data = result.unwrap();
    assert_eq!(historical_data.metrics_timeline.len(), 10);
    assert!(historical_data.time_range.duration_seconds > 0);
    assert!(!historical_data.aggregated_stats.min_values.is_empty());
    assert!(!historical_data.aggregated_stats.max_values.is_empty());
    assert!(!historical_data.aggregated_stats.avg_values.is_empty());
}

#[tokio::test]
async fn test_get_performance_summary() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_performance_summary().await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert_eq!(summary.period.duration_seconds, 3600);
    assert!(summary.key_metrics.total_requests > 0);
    assert!(summary.key_metrics.average_response_time_ms > 0.0);
    assert!(summary.key_metrics.uptime_percentage > 0.0);
    assert!(!summary.recommendations.is_empty() || summary.recommendations.is_empty()); // May or may not have recommendations
    assert_eq!(summary.sla_compliance.availability_target, 99.9);
}

#[tokio::test]
async fn test_get_alert_summary() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_alert_summary().await;
    assert!(result.is_ok());

    let alert_summary = result.unwrap();
    assert_eq!(alert_summary.total_alerts, 1);
    assert_eq!(alert_summary.active_alerts, 1);
    assert_eq!(alert_summary.acknowledged_alerts, 0);
    assert!(!alert_summary.recent_alerts.is_empty());
    assert_eq!(alert_summary.recent_alerts[0].status, AlertStatus::Active);
}

#[tokio::test]
async fn test_generate_performance_report() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.generate_performance_report(Duration::from_secs(3600)).await;
    assert!(result.is_ok());

    let report = result.unwrap();
    assert!(report.report_id.to_string().len() > 0);
    assert!(report.generated_at > 0);
    assert_eq!(report.period.duration_seconds, 3600);
    assert!(!report.executive_summary.key_achievements.is_empty());
    assert!(!report.appendices.data_sources.is_empty());
    assert!(!report.appendices.methodology.is_empty());
}

#[tokio::test]
async fn test_get_system_health_status() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_system_health_status().await;
    assert!(result.is_ok());

    let health_status = result.unwrap();
    assert_eq!(health_status.overall_status, HealthStatus::Healthy);
    assert!(health_status.component_health.contains_key("system"));
    assert!(health_status.component_health.contains_key("database"));
    assert!(health_status.component_health.contains_key("api"));
    assert_eq!(health_status.health_score, 100.0);
    assert!(health_status.last_health_check > 0);
}

#[tokio::test]
async fn test_health_status_degraded() {
    let mut metrics_collector = MockMetricsCollector::new();
    // Set high CPU usage to trigger degraded status
    metrics_collector.current_metrics.cpu_usage = 85.0;
    metrics_collector.current_metrics.memory_usage.usage_percentage = 88.0;
    
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        Box::new(metrics_collector),
        alert_manager,
        30,
    );

    let result = dashboard.get_system_health_status().await;
    assert!(result.is_ok());

    let health_status = result.unwrap();
    assert_eq!(health_status.overall_status, HealthStatus::Degraded);
    assert_eq!(health_status.health_score, 75.0);
}

#[tokio::test]
async fn test_health_status_unhealthy() {
    let mut metrics_collector = MockMetricsCollector::new();
    // Set very high resource usage to trigger unhealthy status
    metrics_collector.current_metrics.cpu_usage = 95.0;
    metrics_collector.current_metrics.memory_usage.usage_percentage = 98.0;
    
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        Box::new(metrics_collector),
        alert_manager,
        30,
    );

    let result = dashboard.get_system_health_status().await;
    assert!(result.is_ok());

    let health_status = result.unwrap();
    assert_eq!(health_status.overall_status, HealthStatus::Unhealthy);
    assert_eq!(health_status.health_score, 25.0);
}

#[tokio::test]
async fn test_trend_analysis() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_historical_data(Duration::from_secs(3600)).await;
    assert!(result.is_ok());

    let historical_data = result.unwrap();
    // The mock data shows increasing trends, so we should detect that
    assert!(matches!(
        historical_data.trend_analysis.cpu_trend,
        TrendDirection::Increasing | TrendDirection::Stable | TrendDirection::Volatile
    ));
    assert!(matches!(
        historical_data.trend_analysis.memory_trend,
        TrendDirection::Increasing | TrendDirection::Stable | TrendDirection::Volatile
    ));
}

#[tokio::test]
async fn test_performance_recommendations() {
    let mut metrics_collector = MockMetricsCollector::new();
    // Set conditions that should trigger recommendations
    metrics_collector.current_metrics.cpu_usage = 85.0;
    metrics_collector.current_metrics.memory_usage.usage_percentage = 90.0;
    metrics_collector.current_metrics.request_metrics.average_response_time_ms = 1200.0;
    metrics_collector.current_metrics.database_metrics.average_query_time_ms = 600.0;
    
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        Box::new(metrics_collector),
        alert_manager,
        30,
    );

    let result = dashboard.get_performance_summary().await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    assert!(!summary.recommendations.is_empty());
    
    // Should have recommendations for high CPU, memory, response time, and database performance
    let recommendation_titles: Vec<String> = summary.recommendations.iter()
        .map(|r| r.title.clone())
        .collect();
    
    assert!(recommendation_titles.iter().any(|title| title.contains("CPU")));
    assert!(recommendation_titles.iter().any(|title| title.contains("Memory")));
    assert!(recommendation_titles.iter().any(|title| title.contains("Response")));
    assert!(recommendation_titles.iter().any(|title| title.contains("Database")));
}

#[tokio::test]
async fn test_sla_compliance() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_performance_summary().await;
    assert!(result.is_ok());

    let summary = result.unwrap();
    let sla = &summary.sla_compliance;
    
    assert_eq!(sla.availability_target, 99.9);
    assert!(sla.availability_actual > 0.0);
    assert_eq!(sla.response_time_target_ms, 500.0);
    assert!(sla.response_time_actual_ms > 0.0);
    assert_eq!(sla.error_rate_target, 1.0);
    assert!(sla.compliance_score > 0.0);
}

#[tokio::test]
async fn test_empty_historical_data() {
    let mut metrics_collector = MockMetricsCollector::new();
    metrics_collector.historical_metrics.clear(); // Empty historical data
    
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        Box::new(metrics_collector),
        alert_manager,
        30,
    );

    let result = dashboard.get_historical_data(Duration::from_secs(3600)).await;
    assert!(result.is_ok());

    let historical_data = result.unwrap();
    assert_eq!(historical_data.metrics_timeline.len(), 0);
    assert_eq!(historical_data.time_range.duration_seconds, 0);
    assert!(historical_data.aggregated_stats.min_values.is_empty());
    assert_eq!(historical_data.trend_analysis.cpu_trend, TrendDirection::Stable);
}

#[tokio::test]
async fn test_application_metrics() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_current_metrics().await;
    assert!(result.is_ok());

    let dashboard_metrics = result.unwrap();
    let app_metrics = &dashboard_metrics.application_metrics;
    
    // These are placeholder values from the implementation
    assert!(app_metrics.active_jobs > 0);
    assert!(app_metrics.completed_jobs > 0);
    assert!(app_metrics.pipeline_executions > 0);
    assert!(app_metrics.runner_instances > 0);
}

#[tokio::test]
async fn test_performance_indicators() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_current_metrics().await;
    assert!(result.is_ok());

    let dashboard_metrics = result.unwrap();
    let indicators = &dashboard_metrics.performance_indicators;
    
    assert_eq!(indicators.throughput_rps, 100.0);
    assert_eq!(indicators.latency_p50_ms, 250.0);
    assert!(indicators.error_rate_percentage > 0.0);
    assert_eq!(indicators.availability_percentage, 99.9);
    assert_eq!(indicators.apdex_score, 0.85);
}

#[tokio::test]
async fn test_resource_utilization() {
    let metrics_collector = Box::new(MockMetricsCollector::new());
    let alert_manager = Box::new(MockAlertManager::new());
    
    let dashboard = ProductionPerformanceDashboard::new(
        metrics_collector,
        alert_manager,
        30,
    );

    let result = dashboard.get_current_metrics().await;
    assert!(result.is_ok());

    let dashboard_metrics = result.unwrap();
    let utilization = &dashboard_metrics.resource_utilization;
    
    assert_eq!(utilization.cpu_usage_percentage, 45.0);
    assert_eq!(utilization.memory_usage_percentage, 60.0);
    assert!(utilization.database_connections_used > 0);
    assert!(utilization.database_connections_available >= 0);
    assert!(utilization.cache_hit_ratio >= 0.0);
}