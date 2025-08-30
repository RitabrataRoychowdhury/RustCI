use std::sync::Arc;
use std::time::Duration;
use deployment::{
    CircuitBreaker, ProductionCircuitBreaker, CircuitBreakerConfig, CircuitBreakerState,
    GracefulDegradationManager, ProductionGracefulDegradationManager, DegradationConfig,
    DegradationMode, SystemHealthMonitor, ProductionSystemHealthMonitor, FallbackExecutor,
    ProductionFallbackExecutor, FallbackMechanism, FallbackType, FallbackConfig,
    SystemDegradationOrchestrator
};
use crate::error::{AppError, Result};

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout: Duration::from_secs(1),
        reset_timeout: Duration::from_millis(100),
        max_concurrent_requests: 5,
    };

    let breaker = ProductionCircuitBreaker::new(config);
    
    // Initially closed
    let stats = breaker.get_stats().await;
    assert_eq!(stats.state, CircuitBreakerState::Closed);
    assert_eq!(stats.failure_count, 0);

    // Simulate failures to trigger open state
    for i in 0..3 {
        let result = breaker.call(async { 
            Err::<(), AppError>(AppError::InternalServerError("test failure".to_string()))
        }).await;
        
        assert!(result.is_err());
        
        let stats = breaker.get_stats().await;
        if i < 2 {
            assert_eq!(stats.state, CircuitBreakerState::Closed);
            assert_eq!(stats.failure_count, i + 1);
        } else {
            assert_eq!(stats.state, CircuitBreakerState::Open);
            assert_eq!(stats.failure_count, 3);
        }
    }

    // Wait for reset timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Next call should transition to half-open
    let result = breaker.call(async { Ok::<(), AppError>(()) }).await;
    assert!(result.is_ok());
    
    let stats = breaker.get_stats().await;
    assert_eq!(stats.state, CircuitBreakerState::HalfOpen);
    assert_eq!(stats.success_count, 1);

    // One more success should close the circuit
    let result = breaker.call(async { Ok::<(), AppError>(()) }).await;
    assert!(result.is_ok());
    
    let stats = breaker.get_stats().await;
    assert_eq!(stats.state, CircuitBreakerState::Closed);
    assert_eq!(stats.failure_count, 0);
    assert_eq!(stats.success_count, 0); // Reset on transition to closed
}

#[tokio::test]
async fn test_circuit_breaker_timeout() {
    let config = CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 2,
        timeout: Duration::from_millis(50),
        reset_timeout: Duration::from_secs(5),
        max_concurrent_requests: 5,
    };

    let breaker = ProductionCircuitBreaker::new(config);
    
    // Operation that takes longer than timeout
    let result = breaker.call(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<(), AppError>(())
    }).await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::Timeout(_)));
    
    let stats = breaker.get_stats().await;
    assert_eq!(stats.failure_count, 1);
}

#[tokio::test]
async fn test_circuit_breaker_concurrent_requests() {
    let config = CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 2,
        timeout: Duration::from_secs(1),
        reset_timeout: Duration::from_millis(50),
        max_concurrent_requests: 2,
    };

    let breaker = Arc::new(ProductionCircuitBreaker::new(config));
    
    // Force to half-open state
    breaker.force_open().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start multiple concurrent requests
    let breaker1 = Arc::clone(&breaker);
    let breaker2 = Arc::clone(&breaker);
    let breaker3 = Arc::clone(&breaker);

    let task1 = tokio::spawn(async move {
        breaker1.call(async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), AppError>(())
        }).await
    });

    let task2 = tokio::spawn(async move {
        breaker2.call(async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), AppError>(())
        }).await
    });

    // Third request should be rejected due to concurrent limit
    tokio::time::sleep(Duration::from_millis(10)).await; // Let first two start
    let result3 = breaker3.call(async { Ok::<(), AppError>(()) }).await;
    
    assert!(result3.is_err());
    assert!(matches!(result3.unwrap_err(), AppError::ServiceUnavailable(_)));

    // Wait for tasks to complete
    let _ = task1.await;
    let _ = task2.await;
}

#[tokio::test]
async fn test_circuit_breaker_force_operations() {
    let config = CircuitBreakerConfig::default();
    let breaker = ProductionCircuitBreaker::new(config);
    
    // Force open
    breaker.force_open().await.unwrap();
    let stats = breaker.get_stats().await;
    assert_eq!(stats.state, CircuitBreakerState::Open);

    // Force close
    breaker.force_close().await.unwrap();
    let stats = breaker.get_stats().await;
    assert_eq!(stats.state, CircuitBreakerState::Closed);
    assert_eq!(stats.failure_count, 0);

    // Reset
    breaker.reset().await.unwrap();
    let stats = breaker.get_stats().await;
    assert_eq!(stats.state, CircuitBreakerState::Closed);
    assert_eq!(stats.total_requests, 0);
}

#[tokio::test]
async fn test_graceful_degradation_manager() {
    let manager = ProductionGracefulDegradationManager::new();
    
    let config = DegradationConfig {
        mode: DegradationMode::ReadOnly,
        fallback_endpoints: vec!["http://fallback.example.com".to_string()],
        cache_ttl: Duration::from_secs(300),
        retry_after: Duration::from_secs(30),
        health_check_interval: Duration::from_secs(10),
    };

    // Enable degradation
    manager.enable_degradation("test-service", config.clone()).await.unwrap();
    
    // Check status
    let status = manager.get_degradation_status("test-service").await.unwrap();
    assert!(status.is_some());
    let status = status.unwrap();
    assert!(matches!(status.mode, DegradationMode::ReadOnly));
    assert_eq!(status.fallback_endpoints.len(), 1);

    // Disable degradation
    manager.disable_degradation("test-service").await.unwrap();
    
    let status = manager.get_degradation_status("test-service").await.unwrap();
    assert!(status.is_none());
}

#[tokio::test]
async fn test_system_health_monitor() {
    let monitor = ProductionSystemHealthMonitor::new();
    
    // Register health check for a non-existent service (will fail)
    let result = monitor.register_health_check(
        "test-service",
        "http://localhost:9999/health".to_string(),
        Duration::from_secs(30),
    ).await;
    
    assert!(result.is_ok());
    
    // Get system health
    let health = monitor.get_system_health().await.unwrap();
    assert!(health.overall_health >= 0.0 && health.overall_health <= 1.0);
    assert_eq!(health.degraded_services.len(), 0); // No services checked yet
}

#[tokio::test]
async fn test_fallback_executor_static_response() {
    let executor = ProductionFallbackExecutor::new();
    
    let mechanism = FallbackMechanism {
        mechanism_type: FallbackType::StaticResponse,
        priority: 1,
        config: FallbackConfig {
            endpoint: None,
            timeout: Duration::from_secs(5),
            retry_count: 2,
            static_response: Some(serde_json::json!({
                "status": "fallback",
                "message": "Service temporarily unavailable",
                "data": null
            })),
            cache_key: None,
        },
    };

    // Register fallback
    executor.register_fallback("test-service", mechanism.clone()).await.unwrap();
    
    // Execute fallback
    let result = executor.execute_fallback("test-service", &mechanism).await.unwrap();
    assert_eq!(result["status"], "fallback");
    assert_eq!(result["message"], "Service temporarily unavailable");
    
    // Remove fallback
    executor.remove_fallback("test-service").await.unwrap();
    
    let mechanisms = executor.fallback_mechanisms.read().await;
    assert!(!mechanisms.contains_key("test-service"));
}

#[tokio::test]
async fn test_fallback_executor_priority_ordering() {
    let executor = ProductionFallbackExecutor::new();
    
    let high_priority = FallbackMechanism {
        mechanism_type: FallbackType::CachedResponse,
        priority: 1,
        config: FallbackConfig {
            endpoint: None,
            timeout: Duration::from_secs(5),
            retry_count: 2,
            static_response: None,
            cache_key: Some("high-priority".to_string()),
        },
    };

    let low_priority = FallbackMechanism {
        mechanism_type: FallbackType::StaticResponse,
        priority: 5,
        config: FallbackConfig {
            endpoint: None,
            timeout: Duration::from_secs(5),
            retry_count: 2,
            static_response: Some(serde_json::json!({"priority": "low"})),
            cache_key: None,
        },
    };

    // Register in reverse priority order
    executor.register_fallback("test-service", low_priority).await.unwrap();
    executor.register_fallback("test-service", high_priority).await.unwrap();
    
    // Check that mechanisms are sorted by priority
    let mechanisms = executor.fallback_mechanisms.read().await;
    let service_mechanisms = mechanisms.get("test-service").unwrap();
    assert_eq!(service_mechanisms.len(), 2);
    assert_eq!(service_mechanisms[0].priority, 1); // High priority first
    assert_eq!(service_mechanisms[1].priority, 5); // Low priority second
}

#[tokio::test]
async fn test_system_degradation_orchestrator() {
    let health_monitor = Arc::new(ProductionSystemHealthMonitor::new());
    let fallback_executor = Arc::new(ProductionFallbackExecutor::new());
    
    let orchestrator = SystemDegradationOrchestrator::new(health_monitor, fallback_executor);
    
    // Register a circuit breaker
    let circuit_breaker = Arc::new(ProductionCircuitBreaker::new(CircuitBreakerConfig::default()));
    orchestrator.register_circuit_breaker("test-service", circuit_breaker).await;
    
    // Set degradation threshold
    orchestrator.set_degradation_threshold("test-service", 0.8).await.unwrap();
    
    // Get system resilience status
    let status = orchestrator.get_system_resilience_status().await.unwrap();
    assert!(status.overall_health >= 0.0 && status.overall_health <= 1.0);
}

#[tokio::test]
async fn test_fallback_mechanism_types() {
    let executor = ProductionFallbackExecutor::new();
    
    // Test different fallback types
    let static_mechanism = FallbackMechanism {
        mechanism_type: FallbackType::StaticResponse,
        priority: 1,
        config: FallbackConfig {
            endpoint: None,
            timeout: Duration::from_secs(5),
            retry_count: 2,
            static_response: Some(serde_json::json!({"type": "static"})),
            cache_key: None,
        },
    };

    let queued_mechanism = FallbackMechanism {
        mechanism_type: FallbackType::QueuedRequest,
        priority: 2,
        config: FallbackConfig {
            endpoint: None,
            timeout: Duration::from_secs(5),
            retry_count: 2,
            static_response: None,
            cache_key: None,
        },
    };

    let reduced_mechanism = FallbackMechanism {
        mechanism_type: FallbackType::ReducedFunctionality,
        priority: 3,
        config: FallbackConfig {
            endpoint: None,
            timeout: Duration::from_secs(5),
            retry_count: 2,
            static_response: None,
            cache_key: None,
        },
    };

    // Test static response
    let result = executor.execute_fallback("test", &static_mechanism).await.unwrap();
    assert_eq!(result["type"], "static");

    // Test queued request
    let result = executor.execute_fallback("test", &queued_mechanism).await.unwrap();
    assert_eq!(result["status"], "queued");

    // Test reduced functionality
    let result = executor.execute_fallback("test", &reduced_mechanism).await.unwrap();
    assert_eq!(result["status"], "reduced_functionality");
}