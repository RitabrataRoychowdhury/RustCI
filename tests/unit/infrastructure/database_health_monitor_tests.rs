use rustci::{
    error::{AppError, Result},
    infrastructure::database::{
        DatabaseHealthMonitor, ProductionDatabaseHealthMonitor, DatabaseHealthConfig,
        AlertType, AlertSeverity,
    },
};
use chrono::Utc;
use mongodb::{Client, Database};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Mock database health monitor for testing
pub struct MockDatabaseHealthMonitor {
    pub is_healthy: bool,
    pub should_fail_health_check: bool,
    pub slow_query_count: usize,
    pub error_count: usize,
}

impl MockDatabaseHealthMonitor {
    pub fn new() -> Self {
        Self {
            is_healthy: true,
            should_fail_health_check: false,
            slow_query_count: 0,
            error_count: 0,
        }
    }

    pub fn set_unhealthy(&mut self) {
        self.is_healthy = false;
    }

    pub fn set_health_check_failure(&mut self, should_fail: bool) {
        self.should_fail_health_check = should_fail;
    }

    pub fn add_slow_queries(&mut self, count: usize) {
        self.slow_query_count += count;
    }

    pub fn add_errors(&mut self, count: usize) {
        self.error_count += count;
    }
}

#[async_trait::async_trait]
impl DatabaseHealthMonitor for MockDatabaseHealthMonitor {
    async fn start_monitoring(&self) -> Result<()> {
        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        Ok(())
    }

    async fn get_health_status(&self) -> Result<rustci::infrastructure::database::HealthMonitorStatus> {
        if self.should_fail_health_check {
            return Err(AppError::DatabaseError("Health check failed".to_string()));
        }

        let connection_status = rustci::infrastructure::database::ConnectionStatus {
            is_connected: self.is_healthy,
            active_connections: 5,
            max_connections: 100,
            connection_pool_utilization: 0.05,
            connection_errors: self.error_count as u64,
            last_connection_error: if self.error_count > 0 {
                Some("Connection timeout".to_string())
            } else {
                None
            },
        };

        let performance_metrics = rustci::infrastructure::database::PerformanceMetrics {
            average_response_time_ms: if self.is_healthy { 50.0 } else { 2500.0 },
            p95_response_time_ms: if self.is_healthy { 100.0 } else { 5000.0 },
            p99_response_time_ms: if self.is_healthy { 200.0 } else { 10000.0 },
            operations_per_second: if self.is_healthy { 1000.0 } else { 10.0 },
            slow_queries_count: self.slow_query_count as u64,
            total_operations: 10000,
        };

        let query_metrics = rustci::infrastructure::database::QueryMetrics {
            read_operations: 7000,
            write_operations: 2000,
            transaction_operations: 1000,
            failed_operations: self.error_count as u64,
            average_query_time_ms: performance_metrics.average_response_time_ms,
            slow_queries: Vec::new(),
        };

        let error_metrics = rustci::infrastructure::database::ErrorMetrics {
            total_errors: self.error_count as u64,
            error_rate: if self.error_count > 0 { 0.1 } else { 0.0 },
            connection_errors: self.error_count as u64,
            timeout_errors: 0,
            query_errors: 0,
            recent_errors: Vec::new(),
        };

        let resource_usage = rustci::infrastructure::database::ResourceUsage {
            memory_usage_mb: Some(512.0),
            cpu_usage_percent: Some(25.0),
            disk_usage_mb: Some(1024.0),
            network_io_mb: Some(100.0),
        };

        let mut alerts = Vec::new();
        
        if !self.is_healthy {
            alerts.push(rustci::infrastructure::database::HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::DatabaseUnresponsive,
                severity: AlertSeverity::Critical,
                message: "Database is not responding".to_string(),
                timestamp: Utc::now(),
                resolved: false,
                metadata: std::collections::HashMap::new(),
            });
        }

        if self.slow_query_count > 10 {
            alerts.push(rustci::infrastructure::database::HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::SlowQueries,
                severity: AlertSeverity::Medium,
                message: format!("High number of slow queries: {}", self.slow_query_count),
                timestamp: Utc::now(),
                resolved: false,
                metadata: std::collections::HashMap::new(),
            });
        }

        if self.error_count > 0 {
            alerts.push(rustci::infrastructure::database::HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::HighErrorRate,
                severity: AlertSeverity::High,
                message: format!("High error rate detected: {} errors", self.error_count),
                timestamp: Utc::now(),
                resolved: false,
                metadata: std::collections::HashMap::new(),
            });
        }

        Ok(rustci::infrastructure::database::HealthMonitorStatus {
            is_healthy: self.is_healthy,
            timestamp: Utc::now(),
            connection_status,
            performance_metrics,
            query_metrics,
            error_metrics,
            resource_usage,
            alerts,
        })
    }

    async fn get_health_history(&self) -> Result<Vec<rustci::infrastructure::database::HealthMonitorStatus>> {
        let status = self.get_health_status().await?;
        Ok(vec![status])
    }

    async fn perform_health_check(&self) -> Result<rustci::infrastructure::database::HealthMonitorStatus> {
        self.get_health_status().await
    }

    async fn get_slow_queries(&self, _limit: usize) -> Result<Vec<rustci::infrastructure::database::SlowQueryInfo>> {
        Ok(Vec::new())
    }

    async fn get_recent_errors(&self, _limit: usize) -> Result<Vec<rustci::infrastructure::database::ErrorInfo>> {
        Ok(Vec::new())
    }

    async fn clear_alerts(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_database_health_config_default() {
    let config = DatabaseHealthConfig::default();
    
    assert_eq!(config.health_check_interval_ms, 30000);
    assert_eq!(config.slow_query_threshold_ms, 1000);
    assert_eq!(config.connection_timeout_ms, 5000);
    assert_eq!(config.max_health_history, 100);
    assert_eq!(config.alert_threshold_error_rate, 0.05);
    assert_eq!(config.alert_threshold_response_time_ms, 2000.0);
    assert!(config.enable_slow_query_logging);
    assert!(config.enable_performance_metrics);
}

#[tokio::test]
async fn test_mock_health_monitor_healthy_status() {
    let monitor = MockDatabaseHealthMonitor::new();
    
    let status = monitor.get_health_status().await.unwrap();
    
    assert!(status.is_healthy);
    assert!(status.connection_status.is_connected);
    assert_eq!(status.connection_status.active_connections, 5);
    assert_eq!(status.connection_status.max_connections, 100);
    assert_eq!(status.performance_metrics.average_response_time_ms, 50.0);
    assert_eq!(status.performance_metrics.operations_per_second, 1000.0);
    assert_eq!(status.error_metrics.total_errors, 0);
    assert_eq!(status.error_metrics.error_rate, 0.0);
    assert!(status.alerts.is_empty());
}

#[tokio::test]
async fn test_mock_health_monitor_unhealthy_status() {
    let mut monitor = MockDatabaseHealthMonitor::new();
    monitor.set_unhealthy();
    monitor.add_errors(5);
    monitor.add_slow_queries(15);
    
    let status = monitor.get_health_status().await.unwrap();
    
    assert!(!status.is_healthy);
    assert!(!status.connection_status.is_connected);
    assert_eq!(status.connection_status.connection_errors, 5);
    assert_eq!(status.performance_metrics.average_response_time_ms, 2500.0);
    assert_eq!(status.performance_metrics.slow_queries_count, 15);
    assert_eq!(status.error_metrics.total_errors, 5);
    assert_eq!(status.error_metrics.error_rate, 0.1);
    assert_eq!(status.alerts.len(), 3); // Database unresponsive, slow queries, high error rate
}

#[tokio::test]
async fn test_mock_health_monitor_health_check_failure() {
    let mut monitor = MockDatabaseHealthMonitor::new();
    monitor.set_health_check_failure(true);
    
    let result = monitor.get_health_status().await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Health check failed"));
}

#[tokio::test]
async fn test_mock_health_monitor_alert_generation() {
    let mut monitor = MockDatabaseHealthMonitor::new();
    monitor.add_slow_queries(20);
    monitor.add_errors(10);
    
    let status = monitor.get_health_status().await.unwrap();
    
    assert_eq!(status.alerts.len(), 2);
    
    let slow_query_alert = status.alerts.iter()
        .find(|a| matches!(a.alert_type, AlertType::SlowQueries))
        .unwrap();
    assert_eq!(slow_query_alert.severity, AlertSeverity::Medium);
    assert!(slow_query_alert.message.contains("20"));
    
    let error_alert = status.alerts.iter()
        .find(|a| matches!(a.alert_type, AlertType::HighErrorRate))
        .unwrap();
    assert_eq!(error_alert.severity, AlertSeverity::High);
    assert!(error_alert.message.contains("10 errors"));
}

#[tokio::test]
async fn test_mock_health_monitor_operations() {
    let monitor = MockDatabaseHealthMonitor::new();
    
    // Test start monitoring
    assert!(monitor.start_monitoring().await.is_ok());
    
    // Test stop monitoring
    assert!(monitor.stop_monitoring().await.is_ok());
    
    // Test get health history
    let history = monitor.get_health_history().await.unwrap();
    assert_eq!(history.len(), 1);
    
    // Test perform health check
    let status = monitor.perform_health_check().await.unwrap();
    assert!(status.is_healthy);
    
    // Test get slow queries
    let slow_queries = monitor.get_slow_queries(10).await.unwrap();
    assert!(slow_queries.is_empty());
    
    // Test get recent errors
    let recent_errors = monitor.get_recent_errors(10).await.unwrap();
    assert!(recent_errors.is_empty());
    
    // Test clear alerts
    assert!(monitor.clear_alerts().await.is_ok());
}

#[tokio::test]
async fn test_performance_metrics_calculation() {
    let monitor = MockDatabaseHealthMonitor::new();
    let status = monitor.get_health_status().await.unwrap();
    
    // Verify performance metrics are reasonable
    assert!(status.performance_metrics.average_response_time_ms > 0.0);
    assert!(status.performance_metrics.p95_response_time_ms >= status.performance_metrics.average_response_time_ms);
    assert!(status.performance_metrics.p99_response_time_ms >= status.performance_metrics.p95_response_time_ms);
    assert!(status.performance_metrics.operations_per_second > 0.0);
    assert_eq!(status.performance_metrics.total_operations, 10000);
}

#[tokio::test]
async fn test_connection_status_details() {
    let mut monitor = MockDatabaseHealthMonitor::new();
    monitor.add_errors(3);
    
    let status = monitor.get_health_status().await.unwrap();
    
    assert_eq!(status.connection_status.active_connections, 5);
    assert_eq!(status.connection_status.max_connections, 100);
    assert_eq!(status.connection_status.connection_pool_utilization, 0.05);
    assert_eq!(status.connection_status.connection_errors, 3);
    assert!(status.connection_status.last_connection_error.is_some());
    assert_eq!(status.connection_status.last_connection_error.unwrap(), "Connection timeout");
}

#[tokio::test]
async fn test_query_metrics_details() {
    let monitor = MockDatabaseHealthMonitor::new();
    let status = monitor.get_health_status().await.unwrap();
    
    assert_eq!(status.query_metrics.read_operations, 7000);
    assert_eq!(status.query_metrics.write_operations, 2000);
    assert_eq!(status.query_metrics.transaction_operations, 1000);
    assert_eq!(status.query_metrics.failed_operations, 0);
    assert_eq!(status.query_metrics.average_query_time_ms, 50.0);
    assert!(status.query_metrics.slow_queries.is_empty());
}

#[tokio::test]
async fn test_resource_usage_metrics() {
    let monitor = MockDatabaseHealthMonitor::new();
    let status = monitor.get_health_status().await.unwrap();
    
    assert_eq!(status.resource_usage.memory_usage_mb, Some(512.0));
    assert_eq!(status.resource_usage.cpu_usage_percent, Some(25.0));
    assert_eq!(status.resource_usage.disk_usage_mb, Some(1024.0));
    assert_eq!(status.resource_usage.network_io_mb, Some(100.0));
}

#[tokio::test]
async fn test_alert_severity_levels() {
    let mut monitor = MockDatabaseHealthMonitor::new();
    monitor.set_unhealthy();
    monitor.add_slow_queries(15);
    monitor.add_errors(5);
    
    let status = monitor.get_health_status().await.unwrap();
    
    // Check that we have alerts of different severity levels
    let critical_alerts: Vec<_> = status.alerts.iter()
        .filter(|a| a.severity == AlertSeverity::Critical)
        .collect();
    let high_alerts: Vec<_> = status.alerts.iter()
        .filter(|a| a.severity == AlertSeverity::High)
        .collect();
    let medium_alerts: Vec<_> = status.alerts.iter()
        .filter(|a| a.severity == AlertSeverity::Medium)
        .collect();
    
    assert_eq!(critical_alerts.len(), 1); // Database unresponsive
    assert_eq!(high_alerts.len(), 1);     // High error rate
    assert_eq!(medium_alerts.len(), 1);   // Slow queries
}

// Integration test that would require a real MongoDB instance
// This test is disabled by default and can be enabled for integration testing
#[tokio::test]
#[ignore]
async fn test_production_health_monitor_integration() {
    // This test requires a running MongoDB instance
    let client = Client::with_uri_str("mongodb://localhost:27017")
        .await
        .expect("Failed to connect to MongoDB");
    
    let database = client.database("test_health_monitor");
    let config = DatabaseHealthConfig::default();
    
    let monitor = ProductionDatabaseHealthMonitor::new(client, database, config)
        .await
        .expect("Failed to create health monitor");
    
    // Test health check
    let status = monitor.perform_health_check().await.unwrap();
    assert!(status.is_healthy);
    assert!(status.connection_status.is_connected);
    
    // Test monitoring start/stop
    monitor.start_monitoring().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    monitor.stop_monitoring().await.unwrap();
    
    // Test health history
    let history = monitor.get_health_history().await.unwrap();
    assert!(!history.is_empty());
}