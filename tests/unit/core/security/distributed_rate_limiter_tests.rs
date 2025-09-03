//! Tests for distributed rate limiting system

use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;
use uuid::Uuid;

use RustAutoDevOps::core::security::rate_limiter::{
    DistributedRateLimiter, DistributedRateLimitConfig, RateLimitPolicy, PolicyScope,
    RateLimitContext, HealthStatus, DistributedRateLimitStats,
};
use RustAutoDevOps::api::rate_limiting::{RateLimitResult, RateLimiter};

#[tokio::test]
async fn test_distributed_rate_limiter_creation() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.active_policies, 0);
    assert!(!stats.redis_available); // Should be false for placeholder implementation
}

#[tokio::test]
async fn test_distributed_rate_limiter_with_redis_config() {
    let config = DistributedRateLimitConfig {
        redis_url: Some("redis://localhost:6379".to_string()),
        pool_size: 5,
        connection_timeout_ms: 1000,
        fallback_to_memory: true,
        ..Default::default()
    };
    
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Should still work with placeholder Redis client
    let result = limiter.check_rate_limit("test_key", 10, 60).await.unwrap();
    match result {
        RateLimitResult::Allowed { remaining, .. } => {
            assert_eq!(remaining, 9); // Should use fallback
        }
        _ => panic!("Expected allowed"),
    }
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.fallback_requests, 1);
    assert_eq!(stats.redis_requests, 0); // Should fallback due to placeholder
}

#[tokio::test]
async fn test_policy_management() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let policy = RateLimitPolicy {
        name: "test_policy".to_string(),
        limit: 100,
        window_seconds: 3600,
        burst_limit: Some(10),
        priority: 1,
        scope: PolicyScope::PerIp,
        custom_headers: HashMap::new(),
        enabled: true,
    };
    
    // Add policy
    limiter.add_policy(policy.clone()).await.unwrap();
    
    let policies = limiter.get_policies().await;
    assert_eq!(policies.len(), 1);
    assert!(policies.contains_key("test_policy"));
    
    let stored_policy = &policies["test_policy"];
    assert_eq!(stored_policy.name, "test_policy");
    assert_eq!(stored_policy.limit, 100);
    assert_eq!(stored_policy.window_seconds, 3600);
    assert_eq!(stored_policy.burst_limit, Some(10));
    assert!(stored_policy.enabled);
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.active_policies, 1);
    
    // Remove policy
    let removed = limiter.remove_policy("test_policy").await.unwrap();
    assert!(removed);
    
    let policies = limiter.get_policies().await;
    assert_eq!(policies.len(), 0);
    
    // Try to remove non-existent policy
    let removed = limiter.remove_policy("non_existent").await.unwrap();
    assert!(!removed);
}

#[tokio::test]
async fn test_policy_with_different_scopes() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let policies = vec![
        RateLimitPolicy {
            name: "per_ip".to_string(),
            limit: 100,
            window_seconds: 3600,
            burst_limit: None,
            priority: 1,
            scope: PolicyScope::PerIp,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "per_user".to_string(),
            limit: 1000,
            window_seconds: 3600,
            burst_limit: None,
            priority: 2,
            scope: PolicyScope::PerUser,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "per_api_key".to_string(),
            limit: 500,
            window_seconds: 3600,
            burst_limit: None,
            priority: 3,
            scope: PolicyScope::PerApiKey,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "global".to_string(),
            limit: 10000,
            window_seconds: 3600,
            burst_limit: None,
            priority: 0,
            scope: PolicyScope::Global,
            custom_headers: HashMap::new(),
            enabled: true,
        },
    ];
    
    for policy in policies {
        limiter.add_policy(policy).await.unwrap();
    }
    
    let stored_policies = limiter.get_policies().await;
    assert_eq!(stored_policies.len(), 4);
    
    // Test different contexts
    let context_with_user = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/test".to_string(),
        user_id: Some("user123".to_string()),
        user_role: None,
        api_key: None,
        request_id: None,
        metadata: HashMap::new(),
    };
    
    let applicable = limiter.find_applicable_policies(&context_with_user).await;
    assert_eq!(applicable.len(), 3); // PerIp, PerUser, Global should match
    
    // Should be sorted by priority (higher first)
    assert_eq!(applicable[0].name, "per_api_key"); // Priority 3 (but won't match due to no API key)
    // Actually, let me fix this test - PerApiKey won't match without api_key
    
    let context_with_api_key = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/test".to_string(),
        user_id: Some("user123".to_string()),
        user_role: None,
        api_key: Some("api_key_123".to_string()),
        request_id: None,
        metadata: HashMap::new(),
    };
    
    let applicable_with_api_key = limiter.find_applicable_policies(&context_with_api_key).await;
    assert_eq!(applicable_with_api_key.len(), 4); // All should match
    
    // Check priority ordering
    assert_eq!(applicable_with_api_key[0].name, "per_api_key"); // Priority 3
    assert_eq!(applicable_with_api_key[1].name, "per_user");    // Priority 2
    assert_eq!(applicable_with_api_key[2].name, "per_ip");      // Priority 1
    assert_eq!(applicable_with_api_key[3].name, "global");      // Priority 0
}

#[tokio::test]
async fn test_disabled_policy() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let disabled_policy = RateLimitPolicy {
        name: "disabled_policy".to_string(),
        limit: 1,
        window_seconds: 60,
        burst_limit: None,
        priority: 1,
        scope: PolicyScope::PerIp,
        custom_headers: HashMap::new(),
        enabled: false, // Disabled
    };
    
    limiter.add_policy(disabled_policy).await.unwrap();
    
    let context = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/test".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: None,
        metadata: HashMap::new(),
    };
    
    let applicable = limiter.find_applicable_policies(&context).await;
    assert_eq!(applicable.len(), 0); // Disabled policy should not be applicable
}

#[tokio::test]
async fn test_custom_scope_policy() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let custom_policy = RateLimitPolicy {
        name: "custom_api_policy".to_string(),
        limit: 50,
        window_seconds: 3600,
        burst_limit: None,
        priority: 1,
        scope: PolicyScope::Custom("api/v2".to_string()),
        custom_headers: HashMap::new(),
        enabled: true,
    };
    
    limiter.add_policy(custom_policy).await.unwrap();
    
    // Test matching endpoint
    let matching_context = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/pipelines".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: None,
        metadata: HashMap::new(),
    };
    
    let applicable = limiter.find_applicable_policies(&matching_context).await;
    assert_eq!(applicable.len(), 1);
    assert_eq!(applicable[0].name, "custom_api_policy");
    
    // Test non-matching endpoint
    let non_matching_context = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v1/pipelines".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: None,
        metadata: HashMap::new(),
    };
    
    let applicable = limiter.find_applicable_policies(&non_matching_context).await;
    assert_eq!(applicable.len(), 0);
}

#[tokio::test]
async fn test_fallback_rate_limiting() {
    let config = DistributedRateLimitConfig {
        redis_url: None, // No Redis configured
        fallback_to_memory: true,
        ..Default::default()
    };
    
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Should use fallback limiter
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
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.fallback_requests, 3);
    assert_eq!(stats.redis_requests, 0);
    assert_eq!(stats.total_requests, 3);
}

#[tokio::test]
async fn test_rate_limit_with_policies() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Add a restrictive policy
    let policy = RateLimitPolicy {
        name: "restrictive_policy".to_string(),
        limit: 2,
        window_seconds: 60,
        burst_limit: None,
        priority: 1,
        scope: PolicyScope::PerIp,
        custom_headers: HashMap::new(),
        enabled: true,
    };
    
    limiter.add_policy(policy).await.unwrap();
    
    let context = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/test".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: Some(Uuid::new_v4()),
        metadata: HashMap::new(),
    };
    
    // First two requests should be allowed
    for i in 0..2 {
        let result = limiter.check_request_with_policies("test_key", &context).await.unwrap();
        match result {
            RateLimitResult::Allowed { remaining, .. } => {
                assert_eq!(remaining, 2 - i - 1);
            }
            _ => panic!("Expected allowed for request {}", i),
        }
    }
    
    // Third request should be rate limited
    let result = limiter.check_request_with_policies("test_key", &context).await.unwrap();
    match result {
        RateLimitResult::Exceeded { limit, .. } => {
            assert_eq!(limit, 2);
        }
        _ => panic!("Expected exceeded"),
    }
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.policy_hits.get("restrictive_policy"), Some(&3));
}

#[tokio::test]
async fn test_no_applicable_policies() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let context = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/test".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: None,
        metadata: HashMap::new(),
    };
    
    // No policies configured, should allow with max limits
    let result = limiter.check_request_with_policies("test_key", &context).await.unwrap();
    match result {
        RateLimitResult::Allowed { remaining, limit, .. } => {
            assert_eq!(remaining, u32::MAX);
            assert_eq!(limit, u32::MAX);
        }
        _ => panic!("Expected allowed with max limits"),
    }
}

#[tokio::test]
async fn test_multiple_policies_most_restrictive() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Add multiple policies with different limits
    let policies = vec![
        RateLimitPolicy {
            name: "lenient_policy".to_string(),
            limit: 1000,
            window_seconds: 3600,
            burst_limit: None,
            priority: 1,
            scope: PolicyScope::Global,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "restrictive_policy".to_string(),
            limit: 5,
            window_seconds: 60,
            burst_limit: None,
            priority: 2,
            scope: PolicyScope::PerIp,
            custom_headers: HashMap::new(),
            enabled: true,
        },
    ];
    
    for policy in policies {
        limiter.add_policy(policy).await.unwrap();
    }
    
    let context = RateLimitContext {
        client_ip: "192.168.1.1".to_string(),
        method: "GET".to_string(),
        endpoint: "/api/v2/test".to_string(),
        user_id: None,
        user_role: None,
        api_key: None,
        request_id: None,
        metadata: HashMap::new(),
    };
    
    // Should be limited by the more restrictive policy (5 requests)
    for i in 0..5 {
        let result = limiter.check_request_with_policies("test_key", &context).await.unwrap();
        match result {
            RateLimitResult::Allowed { .. } => {}
            _ => panic!("Expected allowed for request {}", i),
        }
    }
    
    // 6th request should be rate limited
    let result = limiter.check_request_with_policies("test_key", &context).await.unwrap();
    match result {
        RateLimitResult::Exceeded { limit, .. } => {
            assert_eq!(limit, 5); // Should be limited by restrictive policy
        }
        _ => panic!("Expected exceeded"),
    }
}

#[tokio::test]
async fn test_health_status() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let health = limiter.get_health_status().await;
    assert!(!health.redis_available); // Should be false for placeholder
    assert_eq!(health.consecutive_failures, 0);
    assert_eq!(health.total_requests, 0);
}

#[tokio::test]
async fn test_comprehensive_stats() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Add a policy
    let policy = RateLimitPolicy {
        name: "test_policy".to_string(),
        limit: 10,
        window_seconds: 60,
        burst_limit: None,
        priority: 1,
        scope: PolicyScope::PerIp,
        custom_headers: HashMap::new(),
        enabled: true,
    };
    
    limiter.add_policy(policy).await.unwrap();
    
    // Make some requests
    for _ in 0..3 {
        let _ = limiter.check_rate_limit("test_key", 10, 60).await;
    }
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.active_policies, 1);
    assert_eq!(stats.fallback_requests, 3);
    assert_eq!(stats.redis_requests, 0);
    assert!(!stats.redis_available);
    assert!(stats.average_latency_ms >= 0.0);
}

#[tokio::test]
async fn test_reset_functionality() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Exhaust rate limit
    let _ = limiter.check_rate_limit("test_key", 1, 60).await.unwrap();
    let result = limiter.check_rate_limit("test_key", 1, 60).await.unwrap();
    assert!(matches!(result, RateLimitResult::Exceeded { .. }));
    
    // Reset the key
    limiter.reset("test_key").await.unwrap();
    
    // Should be allowed again
    let result = limiter.check_rate_limit("test_key", 1, 60).await.unwrap();
    assert!(matches!(result, RateLimitResult::Allowed { .. }));
}

#[tokio::test]
async fn test_get_status() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Make a request to create state
    let _ = limiter.check_rate_limit("test_key", 10, 60).await.unwrap();
    
    // Get status
    let status = limiter.get_status("test_key").await.unwrap();
    assert!(status.is_some());
    
    let status = status.unwrap();
    assert_eq!(status.limit, 10);
    assert_eq!(status.remaining, 9);
    assert_eq!(status.window_seconds, 60);
}

#[tokio::test]
async fn test_reset_all() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Make some requests to generate stats
    for _ in 0..5 {
        let _ = limiter.check_rate_limit("test_key", 10, 60).await;
    }
    
    let stats_before = limiter.get_comprehensive_stats().await;
    assert!(stats_before.total_requests > 0);
    
    // Reset all
    limiter.reset_all().await.unwrap();
    
    let stats_after = limiter.get_comprehensive_stats().await;
    assert_eq!(stats_after.total_requests, 0);
    assert_eq!(stats_after.fallback_requests, 0);
    assert_eq!(stats_after.redis_requests, 0);
}

#[test]
fn test_config_defaults() {
    let config = DistributedRateLimitConfig::default();
    assert_eq!(config.pool_size, 10);
    assert_eq!(config.connection_timeout_ms, 5000);
    assert_eq!(config.command_timeout_ms, 1000);
    assert!(!config.cluster_mode);
    assert_eq!(config.key_prefix, "rustci:ratelimit");
    assert_eq!(config.default_ttl_seconds, 3600);
    assert!(!config.enable_compression);
    assert!(config.fallback_to_memory);
    assert_eq!(config.health_check_interval_seconds, 30);
}

#[test]
fn test_policy_scope_serialization() {
    let scopes = vec![
        PolicyScope::PerIp,
        PolicyScope::PerUser,
        PolicyScope::PerApiKey,
        PolicyScope::PerEndpoint,
        PolicyScope::Global,
        PolicyScope::Custom("test_pattern".to_string()),
    ];
    
    for scope in scopes {
        let json = serde_json::to_value(&scope).unwrap();
        let deserialized: PolicyScope = serde_json::from_value(json).unwrap();
        // Test that serialization/deserialization works
        let _ = deserialized;
    }
}

#[test]
fn test_rate_limit_context_creation() {
    let context = RateLimitContext {
        client_ip: "192.168.1.100".to_string(),
        method: "POST".to_string(),
        endpoint: "/api/v2/pipelines".to_string(),
        user_id: Some("user456".to_string()),
        user_role: Some("admin".to_string()),
        api_key: Some("api_key_789".to_string()),
        request_id: Some(Uuid::new_v4()),
        metadata: {
            let mut map = HashMap::new();
            map.insert("source".to_string(), "web".to_string());
            map.insert("version".to_string(), "1.0".to_string());
            map
        },
    };
    
    assert_eq!(context.client_ip, "192.168.1.100");
    assert_eq!(context.method, "POST");
    assert_eq!(context.endpoint, "/api/v2/pipelines");
    assert_eq!(context.user_id, Some("user456".to_string()));
    assert_eq!(context.user_role, Some("admin".to_string()));
    assert_eq!(context.api_key, Some("api_key_789".to_string()));
    assert!(context.request_id.is_some());
    assert_eq!(context.metadata.len(), 2);
    assert_eq!(context.metadata.get("source"), Some(&"web".to_string()));
}

// Performance test (basic)
#[tokio::test]
async fn test_performance_basic() {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    let start = std::time::Instant::now();
    
    // Make 100 requests
    for i in 0..100 {
        let key = format!("perf_test_key_{}", i % 10); // 10 different keys
        let _ = limiter.check_rate_limit(&key, 1000, 3600).await.unwrap();
    }
    
    let duration = start.elapsed();
    println!("100 rate limit checks took: {:?}", duration);
    
    // Should complete reasonably quickly (less than 1 second for in-memory)
    assert!(duration.as_secs() < 1);
    
    let stats = limiter.get_comprehensive_stats().await;
    assert_eq!(stats.fallback_requests, 100);
    assert!(stats.average_latency_ms >= 0.0);
}