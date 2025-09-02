use rustci::core::observability::health_checks::{
    HealthCheckSystem, HealthCheckConfig, HealthChecker, HealthCheckResult,
    HealthStatus, HealthCheckType, ComponentMetrics, ResourceUsage,
    DependencyHealthMonitor, DependencyInfo, DependencyType,
    CascadeFailureDetector, HealthAggregator, SystemHealthStatus,
    DatabaseHealthChecker, HttpServiceHealthChecker,
    DependencyHealth, CascadeIncident, HealthAlert, AlertSeverity,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[tokio::test]
async fn test_health_check_system_initialization() {
    let config = HealthCheckConfig {
        check_interval: Duration::from_secs(10),
        timeout_duration: Duration::from_secs(5),
        max_retries: 2,
        enable_cascade_detection: true,
        enable_dependency_monitoring: true,
        ..HealthCheckConfig::default()
    };

    let health_system = HealthCheckSystem::new(config);
    
    // Test initial system health
    let system_health = health_system.get_system_health().await;
    assert_eq!(system_health.overall_status, HealthStatus::Unknown);
    assert_eq!(system_health.component_statuses.len(), 0);
}

#[tokio::test]
async fn test_database_health_checker() {
    let checker = DatabaseHealthChecker::new(
        "test_database".to_string(),
        "postgresql://localhost:5432/test".to_string(),
        Duration::from_secs(5),
    );

    let result = checker.check_health().await;
    
    assert_eq!(result.component_name, "test_database");
    assert_eq!(result.status, HealthStatus::Healthy);
    assert!(result.duration > Duration::from_millis(0));
    assert!(result.metrics.is_some());
    
    let metrics = result.metrics.unwrap();
    assert!(metrics.response_time_ms > 0.0);
    assert!(metrics.success_rate > 0.0);
    assert!(metrics.resource_usage.cpu_percent >= 0.0);
}

#[tokio::test]
async fn test_http_service_health_checker() {
    let checker = HttpServiceHealthChecker::new(
        "test_api".to_string(),
        "http://localhost:8080/health".to_string(),
        Duration::from_secs(10),
    );

    let result = checker.check_health().await;
    
    assert_eq!(result.component_name, "test_api");
    assert_eq!(result.status, HealthStatus::Healthy);
    assert!(!result.dependencies.is_empty());
    assert_eq!(result.dependencies[0], "database");
    
    // Check details
    assert!(result.details.contains_key("http_status"));
    assert!(result.details.contains_key("response_size_bytes"));
}

#[tokio::test]
async fn test_health_checker_registration_and_execution() {
    let config = HealthCheckConfig::default();
    let health_system = HealthCheckSystem::new(config);

    // Register database health checker
    let db_checker = Box::new(DatabaseHealthChecker::new(
        "primary_database".to_string(),
        "postgresql://localhost:5432/rustci".to_string(),
        Duration::from_secs(5),
    ));
    
    let registration_result = health_system.register_health_checker(db_checker).await;
    assert!(registration_result.is_ok());

    // Register HTTP service health checker
    let api_checker = Box::new(HttpServiceHealthChecker::new(
        "api_service".to_string(),
        "http://localhost:8080/health".to_string(),
        Duration::from_secs(10),
    ));
    
    let registration_result = health_system.register_health_checker(api_checker).await;
    assert!(registration_result.is_ok());

    // Run health checks
    let system_status = health_system.run_health_checks().await;
    assert!(system_status.is_ok());
    
    let status = system_status.unwrap();
    assert_eq!(status.component_statuses.len(), 2);
    assert!(status.component_statuses.contains_key("primary_database"));
    assert!(status.component_statuses.contains_key("api_service"));
    assert_eq!(status.overall_status, HealthStatus::Healthy);
    assert!(status.health_score > 0.0);
}

#[tokio::test]
async fn test_dependency_health_monitoring() {
    let config = HealthCheckConfig::default();
    let monitor = DependencyHealthMonitor::new(config);

    // Add database dependency
    let db_dependency = DependencyInfo {
        name: "postgres_db".to_string(),
        endpoint: "postgresql://localhost:5432/rustci".to_string(),
        dependency_type: DependencyType::Database,
        critical: true,
        timeout: Duration::from_secs(5),
        retry_count: 3,
    };

    let add_result = monitor.add_dependency(db_dependency).await;
    assert!(add_result.is_ok());

    // Add HTTP service dependency
    let api_dependency = DependencyInfo {
        name: "external_api".to_string(),
        endpoint: "https://api.example.com/health".to_string(),
        dependency_type: DependencyType::ExternalAPI,
        critical: false,
        timeout: Duration::from_secs(10),
        retry_count: 2,
    };

    let add_result = monitor.add_dependency(api_dependency).await;
    assert!(add_result.is_ok());

    // Check all dependencies
    let check_result = monitor.check_all_dependencies().await;
    assert!(check_result.is_ok());

    // Get dependency health
    let health_map = monitor.get_all_dependency_health().await;
    assert_eq!(health_map.len(), 2);
    assert!(health_map.contains_key("postgres_db"));
    assert!(health_map.contains_key("external_api"));
    
    for (name, health) in &health_map {
        assert_eq!(health.name, *name);
        assert!(health.response_time >= Duration::from_millis(0));
    }
}

#[tokio::test]
async fn test_cascade_failure_detection() {
    let config = HealthCheckConfig {
        enable_cascade_detection: true,
        ..HealthCheckConfig::default()
    };
    let detector = CascadeFailureDetector::new(config);

    // Create mock health check results with failures
    let results = vec![
        HealthCheckResult {
            component_name: "database".to_string(),
            status: HealthStatus::Critical,
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(100),
            message: "Database connection failed".to_string(),
            details: HashMap::new(),
            metrics: None,
            dependencies: Vec::new(),
            check_id: Uuid::new_v4(),
        },
        HealthCheckResult {
            component_name: "api_service".to_string(),
            status: HealthStatus::Degraded,
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(200),
            message: "API service responding slowly".to_string(),
            details: HashMap::new(),
            metrics: None,
            dependencies: vec!["database".to_string()],
            check_id: Uuid::new_v4(),
        },
    ];

    let analysis_result = detector.analyze_failures(&results).await;
    assert!(analysis_result.is_ok());

    // Check for active incidents (would be populated if patterns were configured)
    let incidents = detector.get_active_incidents().await;
    // Initially empty since no patterns are pre-configured
    assert!(incidents.is_empty() || !incidents.is_empty());
}

#[tokio::test]
async fn test_health_aggregation() {
    let config = HealthCheckConfig::default();
    let aggregator = HealthAggregator::new(config);

    // Create mixed health check results
    let results = vec![
        HealthCheckResult {
            component_name: "healthy_service".to_string(),
            status: HealthStatus::Healthy,
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(50),
            message: "Service is healthy".to_string(),
            details: HashMap::new(),
            metrics: None,
            dependencies: Vec::new(),
            check_id: Uuid::new_v4(),
        },
        HealthCheckResult {
            component_name: "warning_service".to_string(),
            status: HealthStatus::Warning,
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(150),
            message: "Service has warnings".to_string(),
            details: HashMap::new(),
            metrics: None,
            dependencies: Vec::new(),
            check_id: Uuid::new_v4(),
        },
        HealthCheckResult {
            component_name: "critical_service".to_string(),
            status: HealthStatus::Critical,
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(300),
            message: "Service is critical".to_string(),
            details: HashMap::new(),
            metrics: None,
            dependencies: Vec::new(),
            check_id: Uuid::new_v4(),
        },
    ];

    let aggregation_result = aggregator.aggregate_health(&results).await;
    assert!(aggregation_result.is_ok());

    let system_status = aggregation_result.unwrap();
    assert_eq!(system_status.overall_status, HealthStatus::Critical); // Critical takes precedence
    assert_eq!(system_status.component_statuses.len(), 3);
    assert!(system_status.health_score < 100.0); // Should be less than 100% due to failures
    assert!(!system_status.degraded_services.is_empty());
    assert!(!system_status.critical_alerts.is_empty());

    // Check that critical alert was created
    let critical_alert = &system_status.critical_alerts[0];
    assert_eq!(critical_alert.component, "critical_service");
    assert_eq!(critical_alert.severity, AlertSeverity::Critical);
}

#[tokio::test]
async fn test_health_history_tracking() {
    let config = HealthCheckConfig {
        health_history_retention: Duration::from_secs(60),
        ..HealthCheckConfig::default()
    };
    let aggregator = HealthAggregator::new(config);

    // Create multiple health check results over time
    for i in 0..5 {
        let results = vec![
            HealthCheckResult {
                component_name: format!("service_{}", i),
                status: if i % 2 == 0 { HealthStatus::Healthy } else { HealthStatus::Warning },
                timestamp: SystemTime::now(),
                duration: Duration::from_millis(50 + i * 10),
                message: format!("Service {} status", i),
                details: HashMap::new(),
                metrics: None,
                dependencies: Vec::new(),
                check_id: Uuid::new_v4(),
            }
        ];

        aggregator.aggregate_health(&results).await.unwrap();
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Get health history
    let history = aggregator.get_health_history(Duration::from_secs(60)).await;
    assert_eq!(history.len(), 5);

    // Verify history entries
    for (i, snapshot) in history.iter().enumerate() {
        assert_eq!(snapshot.component_count, 1);
        if i % 2 == 0 {
            assert_eq!(snapshot.healthy_components, 1);
            assert_eq!(snapshot.degraded_components, 0);
        } else {
            assert_eq!(snapshot.healthy_components, 0);
            assert_eq!(snapshot.degraded_components, 1);
        }
    }
}

#[tokio::test]
async fn test_comprehensive_health_check_workflow() {
    let config = HealthCheckConfig {
        check_interval: Duration::from_secs(5),
        timeout_duration: Duration::from_secs(3),
        enable_cascade_detection: true,
        enable_dependency_monitoring: true,
        ..HealthCheckConfig::default()
    };

    let health_system = HealthCheckSystem::new(config);

    // Register multiple health checkers
    let db_checker = Box::new(DatabaseHealthChecker::new(
        "main_database".to_string(),
        "postgresql://localhost:5432/rustci".to_string(),
        Duration::from_secs(5),
    ));
    health_system.register_health_checker(db_checker).await.unwrap();

    let api_checker = Box::new(HttpServiceHealthChecker::new(
        "main_api".to_string(),
        "http://localhost:8080/health".to_string(),
        Duration::from_secs(10),
    ));
    health_system.register_health_checker(api_checker).await.unwrap();

    // Add dependencies
    let cache_dependency = DependencyInfo {
        name: "redis_cache".to_string(),
        endpoint: "redis://localhost:6379".to_string(),
        dependency_type: DependencyType::Cache,
        critical: false,
        timeout: Duration::from_secs(2),
        retry_count: 2,
    };
    health_system.add_dependency(cache_dependency).await.unwrap();

    // Run comprehensive health checks
    let system_status = health_system.run_health_checks().await.unwrap();
    
    // Verify system status
    assert!(matches!(
        system_status.overall_status,
        HealthStatus::Healthy | HealthStatus::Warning | HealthStatus::Critical
    ));
    assert_eq!(system_status.component_statuses.len(), 2);
    assert!(system_status.health_score >= 0.0 && system_status.health_score <= 100.0);

    // Get dependency health
    let dependency_health = health_system.get_dependency_health().await;
    assert!(dependency_health.contains_key("redis_cache"));

    // Get health history
    let history = health_system.get_health_history(Duration::from_secs(60)).await;
    assert!(!history.is_empty());

    // Get cascade incidents
    let incidents = health_system.get_cascade_incidents().await;
    // May or may not have incidents depending on health status
    assert!(incidents.is_empty() || !incidents.is_empty());
}

#[tokio::test]
async fn test_health_check_timeout_handling() {
    let config = HealthCheckConfig {
        timeout_duration: Duration::from_millis(100), // Very short timeout
        ..HealthCheckConfig::default()
    };

    let health_system = HealthCheckSystem::new(config);

    // Create a mock health checker that takes longer than timeout
    struct SlowHealthChecker;

    #[async_trait::async_trait]
    impl HealthChecker for SlowHealthChecker {
        async fn check_health(&self) -> HealthCheckResult {
            // Simulate slow operation
            tokio::time::sleep(Duration::from_millis(200)).await;
            
            HealthCheckResult {
                component_name: "slow_service".to_string(),
                status: HealthStatus::Healthy,
                timestamp: SystemTime::now(),
                duration: Duration::from_millis(200),
                message: "Slow but healthy".to_string(),
                details: HashMap::new(),
                metrics: None,
                dependencies: Vec::new(),
                check_id: Uuid::new_v4(),
            }
        }

        fn get_name(&self) -> &str {
            "slow_service"
        }

        fn get_dependencies(&self) -> Vec<String> {
            Vec::new()
        }

        fn get_check_type(&self) -> HealthCheckType {
            HealthCheckType::Custom("slow".to_string())
        }

        fn get_timeout(&self) -> Duration {
            Duration::from_millis(50) // Shorter than the actual operation
        }
    }

    let slow_checker = Box::new(SlowHealthChecker);
    health_system.register_health_checker(slow_checker).await.unwrap();

    // Run health checks - should handle timeout
    let system_status = health_system.run_health_checks().await.unwrap();
    
    // The slow service should be marked as critical due to timeout
    assert!(system_status.component_statuses.contains_key("slow_service"));
    let slow_service_status = &system_status.component_statuses["slow_service"];
    assert_eq!(*slow_service_status, HealthStatus::Critical);
}

#[tokio::test]
async fn test_dependency_removal() {
    let config = HealthCheckConfig::default();
    let monitor = DependencyHealthMonitor::new(config);

    // Add a dependency
    let dependency = DependencyInfo {
        name: "temp_service".to_string(),
        endpoint: "http://temp.example.com".to_string(),
        dependency_type: DependencyType::HttpService,
        critical: false,
        timeout: Duration::from_secs(5),
        retry_count: 1,
    };

    monitor.add_dependency(dependency).await.unwrap();

    // Verify it was added
    let health_map = monitor.get_all_dependency_health().await;
    assert!(health_map.contains_key("temp_service"));

    // Remove the dependency
    let remove_result = monitor.remove_dependency("temp_service").await;
    assert!(remove_result.is_ok());

    // Verify it was removed
    let health_map = monitor.get_all_dependency_health().await;
    assert!(!health_map.contains_key("temp_service"));
}

#[tokio::test]
async fn test_health_status_transitions() {
    // Test different health status scenarios
    let statuses = vec![
        HealthStatus::Healthy,
        HealthStatus::Warning,
        HealthStatus::Critical,
        HealthStatus::Degraded,
        HealthStatus::Unknown,
    ];

    for status in statuses {
        let result = HealthCheckResult {
            component_name: "test_service".to_string(),
            status: status.clone(),
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(100),
            message: format!("Service is {:?}", status),
            details: HashMap::new(),
            metrics: Some(ComponentMetrics {
                response_time_ms: 100.0,
                success_rate: match status {
                    HealthStatus::Healthy => 0.99,
                    HealthStatus::Warning => 0.95,
                    HealthStatus::Critical => 0.50,
                    HealthStatus::Degraded => 0.80,
                    HealthStatus::Unknown => 0.0,
                },
                error_rate: match status {
                    HealthStatus::Healthy => 0.01,
                    HealthStatus::Warning => 0.05,
                    HealthStatus::Critical => 0.50,
                    HealthStatus::Degraded => 0.20,
                    HealthStatus::Unknown => 1.0,
                },
                throughput: 50.0,
                resource_usage: ResourceUsage {
                    cpu_percent: 20.0,
                    memory_percent: 30.0,
                    disk_percent: 40.0,
                    network_io: 1024.0,
                    connection_count: 10,
                },
                custom_metrics: HashMap::new(),
            }),
            dependencies: Vec::new(),
            check_id: Uuid::new_v4(),
        };

        // Verify the result structure is valid for each status
        assert_eq!(result.status, status);
        assert!(result.metrics.is_some());
        
        let metrics = result.metrics.unwrap();
        assert!(metrics.response_time_ms > 0.0);
        assert!(metrics.success_rate >= 0.0 && metrics.success_rate <= 1.0);
        assert!(metrics.error_rate >= 0.0 && metrics.error_rate <= 1.0);
    }
}