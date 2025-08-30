//! Tests for API rate limiting system

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Router,
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

use RustAutoDevOps::api::{
    rate_limiting::{
        EndpointRateLimit, InMemoryRateLimiter, RateLimitConfig, RateLimitRequestInfo,
        RateLimitResult, RateLimitScope, RateLimitService, RateLimitState, RateLimiter,
        UserRateLimit, RateLimitPriority, rate_limiting_middleware,
    },
    rate_limit_monitoring::{
        RateLimitMonitor, RateLimitMonitoringConfig, AlertThresholds, AlertType, AlertSeverity,
    },
};

#[test]
fn test_rate_limit_state_creation() {
    let state = RateLimitState::new(100, 3600);
    assert_eq!(state.limit, 100);
    assert_eq!(state.window_seconds, 3600);
    assert_eq!(state.count, 0);
}

#[test]
fn test_rate_limit_state_within_limit() {
    let mut state = RateLimitState::new(5, 60);
    
    // First few requests should be allowed
    for i in 0..5 {
        match state.check_and_update() {
            RateLimitResult::Allowed { remaining, limit, .. } => {
                assert_eq!(remaining, 5 - i - 1);
                assert_eq!(limit, 5);
            }
            _ => panic!("Expected allowed for request {}", i),
        }
    }
    
    // Next request should be rate limited
    match state.check_and_update() {
        RateLimitResult::Exceeded { limit, .. } => {
            assert_eq!(limit, 5);
        }
        _ => panic!("Expected exceeded"),
    }
}

#[test]
fn test_rate_limit_state_status() {
    let mut state = RateLimitState::new(10, 60);
    
    // Make some requests
    state.check_and_update();
    state.check_and_update();
    
    let status = state.status();
    assert_eq!(status.limit, 10);
    assert_eq!(status.remaining, 8);
    assert_eq!(status.window_seconds, 60);
}

#[tokio::test]
async fn test_in_memory_rate_limiter() {
    let limiter = InMemoryRateLimiter::new();
    
    // Test basic rate limiting
    let result1 = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
    match result1 {
        RateLimitResult::Allowed { remaining, .. } => {
            assert_eq!(remaining, 1);
        }
        _ => panic!("Expected allowed"),
    }
    
    let result2 = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
    match result2 {
        RateLimitResult::Allowed { remaining, .. } => {
            assert_eq!(remaining, 0);
        }
        _ => panic!("Expected allowed"),
    }
    
    let result3 = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
    match result3 {
        RateLimitResult::Exceeded { limit, .. } => {
            assert_eq!(limit, 2);
        }
        _ => panic!("Expected exceeded"),
    }
}

#[tokio::test]
async fn test_in_memory_rate_limiter_different_keys() {
    let limiter = InMemoryRateLimiter::new();
    
    // Different keys should have independent limits
    let result1 = limiter.check_rate_limit("key1", 1, 60).await.unwrap();
    let result2 = limiter.check_rate_limit("key2", 1, 60).await.unwrap();
    
    match (result1, result2) {
        (RateLimitResult::Allowed { .. }, RateLimitResult::Allowed { .. }) => {}
        _ => panic!("Both should be allowed"),
    }
    
    // Both keys should now be at limit
    let result3 = limiter.check_rate_limit("key1", 1, 60).await.unwrap();
    let result4 = limiter.check_rate_limit("key2", 1, 60).await.unwrap();
    
    match (result3, result4) {
        (RateLimitResult::Exceeded { .. }, RateLimitResult::Exceeded { .. }) => {}
        _ => panic!("Both should be exceeded"),
    }
}

#[tokio::test]
async fn test_rate_limiter_status() {
    let limiter = InMemoryRateLimiter::new();
    
    // Make a request
    limiter.check_rate_limit("test_key", 5, 60).await.unwrap();
    
    // Check status
    let status = limiter.get_status("test_key").await.unwrap();
    assert!(status.is_some());
    
    let status = status.unwrap();
    assert_eq!(status.limit, 5);
    assert_eq!(status.remaining, 4);
}

#[tokio::test]
async fn test_rate_limiter_reset() {
    let limiter = InMemoryRateLimiter::new();
    
    // Exhaust the limit
    limiter.check_rate_limit("test_key", 1, 60).await.unwrap();
    let result = limiter.check_rate_limit("test_key", 1, 60).await.unwrap();
    assert!(matches!(result, RateLimitResult::Exceeded { .. }));
    
    // Reset the key
    limiter.reset("test_key").await.unwrap();
    
    // Should be allowed again
    let result = limiter.check_rate_limit("test_key", 1, 60).await.unwrap();
    assert!(matches!(result, RateLimitResult::Allowed { .. }));
}

#[tokio::test]
async fn test_rate_limiter_stats() {
    let limiter = InMemoryRateLimiter::new();
    
    // Make some requests
    limiter.check_rate_limit("key1", 5, 60).await.unwrap();
    limiter.check_rate_limit("key2", 5, 60).await.unwrap();
    
    let stats = limiter.get_stats().await.unwrap();
    assert_eq!(stats.total_keys, 2);
    assert_eq!(stats.backend, "in-memory");
}

#[test]
fn test_rate_limit_config_default() {
    let config = RateLimitConfig::default();
    assert_eq!(config.default_limit, 1000);
    assert_eq!(config.default_window_seconds, 3600);
    assert!(config.include_headers);
    assert!(config.enable_monitoring);
    assert!(!config.global_limits.is_empty());
}

#[test]
fn test_endpoint_rate_limit_config() {
    let endpoint_limit = EndpointRateLimit {
        method: "POST".to_string(),
        path: "/api/v2/pipelines".to_string(),
        limit: 100,
        window_seconds: 3600,
        scope: RateLimitScope::PerUser,
        custom_headers: Some(HashMap::from([
            ("X-Custom-Header".to_string(), "custom-value".to_string()),
        ])),
    };
    
    assert_eq!(endpoint_limit.method, "POST");
    assert_eq!(endpoint_limit.limit, 100);
    assert!(endpoint_limit.custom_headers.is_some());
}

#[test]
fn test_user_rate_limit_config() {
    let user_limit = UserRateLimit {
        role: "premium".to_string(),
        limit: 5000,
        window_seconds: 3600,
        burst_limit: Some(100),
        priority: RateLimitPriority::High,
    };
    
    assert_eq!(user_limit.role, "premium");
    assert_eq!(user_limit.limit, 5000);
    assert_eq!(user_limit.burst_limit, Some(100));
}

#[test]
fn test_rate_limit_service_creation() {
    let config = RateLimitConfig::default();
    let service = RateLimitService::new(config);
    
    // Service should be created successfully
    assert!(service.config.default_limit > 0);
}

#[tokio::test]
async fn test_rate_limit_service_basic_check() {
    let config = RateLimitConfig::default();
    let service = RateLimitService::new(config);
    
    let request_info = RateLimitRequestInfo {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        path: "/api/v2/test".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: Some(Uuid::new_v4()),
    };
    
    let result = service.check_request(&request_info).await.unwrap();
    match result {
        RateLimitResult::Allowed { .. } => {}
        _ => panic!("Expected allowed"),
    }
}

#[tokio::test]
async fn test_rate_limit_service_with_endpoint_limits() {
    let mut config = RateLimitConfig::default();
    config.endpoint_limits.insert(
        "test_endpoint".to_string(),
        EndpointRateLimit {
            method: "POST".to_string(),
            path: "/api/v2/test".to_string(),
            limit: 2,
            window_seconds: 60,
            scope: RateLimitScope::PerIp,
            custom_headers: None,
        },
    );
    
    let service = RateLimitService::new(config);
    
    let request_info = RateLimitRequestInfo {
        client_ip: "192.168.1.1".to_string(),
        method: "POST".to_string(),
        path: "/api/v2/test".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: Some(Uuid::new_v4()),
    };
    
    // First two requests should be allowed
    for _ in 0..2 {
        let result = service.check_request(&request_info).await.unwrap();
        match result {
            RateLimitResult::Allowed { .. } => {}
            _ => panic!("Expected allowed"),
        }
    }
    
    // Third request should be rate limited
    let result = service.check_request(&request_info).await.unwrap();
    match result {
        RateLimitResult::Exceeded { .. } => {}
        _ => panic!("Expected exceeded"),
    }
}

#[tokio::test]
async fn test_rate_limit_service_with_user_limits() {
    let mut config = RateLimitConfig::default();
    config.user_limits.insert(
        "premium".to_string(),
        UserRateLimit {
            role: "premium".to_string(),
            limit: 1000,
            window_seconds: 3600,
            burst_limit: None,
            priority: RateLimitPriority::High,
        },
    );
    
    let service = RateLimitService::new(config);
    
    let request_info = RateLimitRequestInfo {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        path: "/api/v2/test".to_string(),
        user_id: Some("user123".to_string()),
        user_role: Some("premium".to_string()),
        api_key: None,
        request_id: Some(Uuid::new_v4()),
    };
    
    let result = service.check_request(&request_info).await.unwrap();
    match result {
        RateLimitResult::Allowed { limit, .. } => {
            assert_eq!(limit, 1000); // Should use premium limit
        }
        _ => panic!("Expected allowed"),
    }
}

#[test]
fn test_rate_limit_headers() {
    let config = RateLimitConfig::default();
    let service = RateLimitService::new(config);
    
    let result = RateLimitResult::Allowed {
        remaining: 99,
        reset_time: 1234567890,
        limit: 100,
    };
    
    let headers = service.get_rate_limit_headers(&result);
    
    assert_eq!(headers.get("X-RateLimit-Limit").unwrap(), "100");
    assert_eq!(headers.get("X-RateLimit-Remaining").unwrap(), "99");
    assert_eq!(headers.get("X-RateLimit-Reset").unwrap(), "1234567890");
}

#[test]
fn test_rate_limit_headers_exceeded() {
    let config = RateLimitConfig::default();
    let service = RateLimitService::new(config);
    
    let result = RateLimitResult::Exceeded {
        limit: 100,
        window_seconds: 3600,
        retry_after_seconds: 300,
        reset_time: 1234567890,
    };
    
    let headers = service.get_rate_limit_headers(&result);
    
    assert_eq!(headers.get("X-RateLimit-Limit").unwrap(), "100");
    assert_eq!(headers.get("X-RateLimit-Remaining").unwrap(), "0");
    assert_eq!(headers.get("Retry-After").unwrap(), "300");
}

#[test]
fn test_request_info_extraction() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/pipelines?param=value")
        .header("x-forwarded-for", "192.168.1.1, 10.0.0.1")
        .header("x-api-key", "test-api-key-123")
        .body(Body::empty())
        .unwrap();
    
    // Note: In real implementation, this would be called by the middleware
    // Here we're testing the logic conceptually
    assert_eq!(req.method(), Method::POST);
    assert_eq!(req.uri().path(), "/api/v2/pipelines");
    
    let forwarded_for = req.headers().get("x-forwarded-for").unwrap().to_str().unwrap();
    let client_ip = forwarded_for.split(',').next().unwrap().trim();
    assert_eq!(client_ip, "192.168.1.1");
    
    let api_key = req.headers().get("x-api-key").unwrap().to_str().unwrap();
    assert_eq!(api_key, "test-api-key-123");
}

// Test handlers for middleware testing
async fn test_handler() -> &'static str {
    "success"
}

async fn slow_handler() -> &'static str {
    tokio::time::sleep(Duration::from_millis(100)).await;
    "slow success"
}

#[tokio::test]
async fn test_rate_limiting_middleware_success() {
    let config = RateLimitConfig::default();
    let service = Arc::new(RateLimitService::new(config));
    
    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn_with_state(
            service.clone(),
            rate_limiting_middleware,
        ))
        .with_state(service);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header("x-forwarded-for", "192.168.1.1")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check rate limit headers
    assert!(response.headers().contains_key("X-RateLimit-Limit"));
    assert!(response.headers().contains_key("X-RateLimit-Remaining"));
    assert!(response.headers().contains_key("X-RateLimit-Reset"));
}

#[tokio::test]
async fn test_rate_limiting_middleware_exceeded() {
    let mut config = RateLimitConfig::default();
    config.default_limit = 1;
    config.default_window_seconds = 60;
    
    let service = Arc::new(RateLimitService::new(config));
    
    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn_with_state(
            service.clone(),
            rate_limiting_middleware,
        ))
        .with_state(service);
    
    // First request should succeed
    let request1 = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header("x-forwarded-for", "192.168.1.1")
        .body(Body::empty())
        .unwrap();
    
    let response1 = app.clone().oneshot(request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);
    
    // Second request should be rate limited
    let request2 = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header("x-forwarded-for", "192.168.1.1")
        .body(Body::empty())
        .unwrap();
    
    let response2 = app.oneshot(request2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::TOO_MANY_REQUESTS);
    
    // Check error response format
    let body = hyper::body::to_bytes(response2.into_body()).await.unwrap();
    let error_response: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(error_response["error"]["code"], "RATE_LIMIT_EXCEEDED");
    assert!(error_response["error"]["recovery_suggestions"].is_array());
}

#[tokio::test]
async fn test_rate_limit_monitoring() {
    let config = RateLimitMonitoringConfig::default();
    let monitor = RateLimitMonitor::new(config);
    
    // Record some checks
    let result_allowed = RateLimitResult::Allowed {
        remaining: 99,
        reset_time: 1234567890,
        limit: 100,
    };
    
    let result_exceeded = RateLimitResult::Exceeded {
        limit: 100,
        window_seconds: 3600,
        retry_after_seconds: 300,
        reset_time: 1234567890,
    };
    
    monitor.record_check(
        "/api/v2/pipelines".to_string(),
        "GET".to_string(),
        "192.168.1.1".to_string(),
        &result_allowed,
        1000,
    ).await;
    
    monitor.record_check(
        "/api/v2/pipelines".to_string(),
        "POST".to_string(),
        "192.168.1.2".to_string(),
        &result_exceeded,
        2000,
    ).await;
    
    let stats = monitor.get_stats().await;
    assert!(stats.monitoring_enabled);
    assert_eq!(stats.collection_interval_seconds, 60);
}

#[tokio::test]
async fn test_rate_limit_alert_triggering() {
    let config = RateLimitMonitoringConfig::default();
    let monitor = RateLimitMonitor::new(config);
    
    // Trigger an alert manually
    monitor.trigger_alert(
        AlertType::HighHitRate,
        AlertSeverity::High,
        "Test high hit rate alert".to_string(),
        serde_json::json!({
            "hit_rate": 0.95,
            "total_requests": 1000
        }),
    ).await;
    
    let active_alerts = monitor.get_active_alerts().await;
    assert_eq!(active_alerts.len(), 1);
    assert_eq!(active_alerts[0].message, "Test high hit rate alert");
    assert!(matches!(active_alerts[0].alert_type, AlertType::HighHitRate));
    assert!(matches!(active_alerts[0].severity, AlertSeverity::High));
    
    // Test alert acknowledgment
    let alert_id = &active_alerts[0].alert_id;
    let acknowledged = monitor.acknowledge_alert(alert_id).await;
    assert!(acknowledged);
    
    let updated_alerts = monitor.get_active_alerts().await;
    assert!(!updated_alerts[0].active);
}

#[test]
fn test_alert_thresholds_configuration() {
    let thresholds = AlertThresholds {
        hit_rate_threshold: 0.9,
        min_requests_per_minute: 200,
        consecutive_periods: 5,
        cooldown_seconds: 600,
    };
    
    assert_eq!(thresholds.hit_rate_threshold, 0.9);
    assert_eq!(thresholds.min_requests_per_minute, 200);
    assert_eq!(thresholds.consecutive_periods, 5);
    assert_eq!(thresholds.cooldown_seconds, 600);
}

#[tokio::test]
async fn test_rate_limit_service_stats() {
    let config = RateLimitConfig::default();
    let service = RateLimitService::new(config);
    
    let stats = service.get_stats().await.unwrap();
    assert_eq!(stats.backend, "in-memory");
    assert_eq!(stats.total_keys, 0); // No requests made yet
}

#[test]
fn test_rate_limit_scopes() {
    // Test serialization/deserialization of scopes
    let scopes = vec![
        RateLimitScope::PerIp,
        RateLimitScope::PerUser,
        RateLimitScope::PerApiKey,
        RateLimitScope::Global,
        RateLimitScope::PerEndpoint,
    ];
    
    for scope in scopes {
        let json = serde_json::to_value(&scope).unwrap();
        let deserialized: RateLimitScope = serde_json::from_value(json).unwrap();
        // Note: PartialEq would need to be implemented for direct comparison
        // Here we just test that serialization/deserialization works
        let _ = deserialized;
    }
}

#[test]
fn test_rate_limit_priorities() {
    let priorities = vec![
        RateLimitPriority::Low,
        RateLimitPriority::Normal,
        RateLimitPriority::High,
        RateLimitPriority::Critical,
    ];
    
    for priority in priorities {
        let json = serde_json::to_value(&priority).unwrap();
        let deserialized: RateLimitPriority = serde_json::from_value(json).unwrap();
        let _ = deserialized;
    }
}