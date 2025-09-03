//! Distributed Rate Limiting Example
//!
//! This example demonstrates the distributed rate limiting system with Redis backend support,
//! configurable policies per endpoint and user, and rate limit headers with client guidance.
//!
//! This is currently a placeholder implementation that will be replaced by Yggdrasil storage
//! backend for ultra-low latency operations.

use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;
use uuid::Uuid;

use RustAutoDevOps::core::security::rate_limiter::{
    DistributedRateLimiter, DistributedRateLimitConfig, RateLimitPolicy, PolicyScope,
    RateLimitContext,
};
use RustAutoDevOps::api::rate_limiting::{RateLimitResult, RateLimiter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 Distributed Rate Limiting Example");
    println!("=====================================");
    
    // Example 1: Basic distributed rate limiter setup
    println!("\n📋 Example 1: Basic Setup");
    basic_setup_example().await?;
    
    // Example 2: Policy management
    println!("\n📋 Example 2: Policy Management");
    policy_management_example().await?;
    
    // Example 3: Rate limiting with different scopes
    println!("\n📋 Example 3: Different Policy Scopes");
    policy_scopes_example().await?;
    
    // Example 4: Fallback behavior
    println!("\n📋 Example 4: Fallback Behavior");
    fallback_behavior_example().await?;
    
    // Example 5: Performance demonstration
    println!("\n📋 Example 5: Performance Test");
    performance_example().await?;
    
    println!("\n✅ All examples completed successfully!");
    println!("\n📝 Note: This is a placeholder implementation.");
    println!("   In production, this will be replaced by Yggdrasil storage backend");
    println!("   for ultra-low latency (<20µs P99) distributed rate limiting.");
    
    Ok(())
}

async fn basic_setup_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration for distributed rate limiter
    let config = DistributedRateLimitConfig {
        redis_url: Some("redis://localhost:6379".to_string()),
        pool_size: 10,
        connection_timeout_ms: 5000,
        fallback_to_memory: true,
        key_prefix: "example:ratelimit".to_string(),
        ..Default::default()
    };
    
    println!("   Creating distributed rate limiter with config:");
    println!("   - Redis URL: {:?}", config.redis_url);
    println!("   - Pool size: {}", config.pool_size);
    println!("   - Fallback enabled: {}", config.fallback_to_memory);
    
    let limiter = DistributedRateLimiter::new(config).await?;
    
    // Test basic rate limiting
    println!("   Testing basic rate limiting (limit: 3 requests per 60 seconds):");
    
    for i in 1..=5 {
        let result = limiter.check_rate_limit("example_key", 3, 60).await?;
        match result {
            RateLimitResult::Allowed { remaining, limit, .. } => {
                println!("   ✅ Request {}: Allowed (remaining: {}/{})", i, remaining, limit);
            }
            RateLimitResult::Exceeded { limit, retry_after_seconds, .. } => {
                println!("   ❌ Request {}: Rate limited (limit: {}, retry after: {}s)", 
                         i, limit, retry_after_seconds);
            }
        }
    }
    
    let stats = limiter.get_comprehensive_stats().await;
    println!("   📊 Stats: {} total requests, {} fallback requests", 
             stats.total_requests, stats.fallback_requests);
    
    Ok(())
}

async fn policy_management_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await?;
    
    println!("   Adding rate limiting policies:");
    
    // Add different types of policies
    let policies = vec![
        RateLimitPolicy {
            name: "api_endpoints".to_string(),
            limit: 100,
            window_seconds: 3600,
            burst_limit: Some(10),
            priority: 2,
            scope: PolicyScope::PerEndpoint,
            custom_headers: {
                let mut headers = HashMap::new();
                headers.insert("X-RateLimit-Policy".to_string(), "api_endpoints".to_string());
                headers
            },
            enabled: true,
        },
        RateLimitPolicy {
            name: "premium_users".to_string(),
            limit: 1000,
            window_seconds: 3600,
            burst_limit: Some(50),
            priority: 3,
            scope: PolicyScope::PerUser,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "global_limit".to_string(),
            limit: 10000,
            window_seconds: 3600,
            burst_limit: None,
            priority: 1,
            scope: PolicyScope::Global,
            custom_headers: HashMap::new(),
            enabled: true,
        },
    ];
    
    for policy in policies {
        limiter.add_policy(policy.clone()).await?;
        println!("   ✅ Added policy: {} (limit: {}, scope: {:?})", 
                 policy.name, policy.limit, policy.scope);
    }
    
    let stored_policies = limiter.get_policies().await;
    println!("   📋 Total policies configured: {}", stored_policies.len());
    
    // Test policy removal
    let removed = limiter.remove_policy("global_limit").await?;
    println!("   🗑️  Removed 'global_limit' policy: {}", removed);
    
    let updated_policies = limiter.get_policies().await;
    println!("   📋 Policies after removal: {}", updated_policies.len());
    
    Ok(())
}

async fn policy_scopes_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await?;
    
    // Add policies with different scopes
    let policies = vec![
        RateLimitPolicy {
            name: "per_ip_strict".to_string(),
            limit: 5,
            window_seconds: 60,
            burst_limit: None,
            priority: 3,
            scope: PolicyScope::PerIp,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "per_user_generous".to_string(),
            limit: 50,
            window_seconds: 60,
            burst_limit: Some(10),
            priority: 2,
            scope: PolicyScope::PerUser,
            custom_headers: HashMap::new(),
            enabled: true,
        },
        RateLimitPolicy {
            name: "api_v2_custom".to_string(),
            limit: 20,
            window_seconds: 60,
            burst_limit: None,
            priority: 4,
            scope: PolicyScope::Custom("api/v2".to_string()),
            custom_headers: HashMap::new(),
            enabled: true,
        },
    ];
    
    for policy in policies {
        limiter.add_policy(policy.clone()).await?;
        println!("   ✅ Added {} policy: {} requests/{}s", 
                 match policy.scope {
                     PolicyScope::PerIp => "Per-IP",
                     PolicyScope::PerUser => "Per-User",
                     PolicyScope::Custom(_) => "Custom",
                     _ => "Other",
                 },
                 policy.limit, policy.window_seconds);
    }
    
    // Test different contexts
    let contexts = vec![
        RateLimitContext {
            client_ip: "192.168.1.100".to_string(),
            method: "GET".to_string(),
            endpoint: "/api/v2/pipelines".to_string(),
            user_id: Some("user123".to_string()),
            user_role: Some("premium".to_string()),
            api_key: None,
            request_id: Some(Uuid::new_v4()),
            metadata: HashMap::new(),
        },
        RateLimitContext {
            client_ip: "192.168.1.101".to_string(),
            method: "POST".to_string(),
            endpoint: "/api/v1/jobs".to_string(),
            user_id: None,
            user_role: None,
            api_key: Some("api_key_456".to_string()),
            request_id: Some(Uuid::new_v4()),
            metadata: HashMap::new(),
        },
    ];
    
    for (i, context) in contexts.iter().enumerate() {
        println!("   🧪 Testing context {} (IP: {}, endpoint: {})", 
                 i + 1, context.client_ip, context.endpoint);
        
        let result = limiter.check_request_with_policies("test_key", context).await?;
        match result {
            RateLimitResult::Allowed { remaining, limit, .. } => {
                println!("      ✅ Allowed (remaining: {}/{})", remaining, limit);
            }
            RateLimitResult::Exceeded { limit, retry_after_seconds, .. } => {
                println!("      ❌ Rate limited (limit: {}, retry after: {}s)", 
                         limit, retry_after_seconds);
            }
        }
    }
    
    Ok(())
}

async fn fallback_behavior_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("   Testing fallback behavior (Redis unavailable):");
    
    // Create config without Redis (will use in-memory fallback)
    let config = DistributedRateLimitConfig {
        redis_url: None,
        fallback_to_memory: true,
        ..Default::default()
    };
    
    let limiter = DistributedRateLimiter::new(config).await?;
    
    // Test fallback rate limiting
    println!("   Making requests with fallback limiter:");
    
    for i in 1..=3 {
        let result = limiter.check_rate_limit("fallback_key", 2, 60).await?;
        match result {
            RateLimitResult::Allowed { remaining, .. } => {
                println!("      ✅ Request {}: Allowed (remaining: {})", i, remaining);
            }
            RateLimitResult::Exceeded { limit, .. } => {
                println!("      ❌ Request {}: Rate limited (limit: {})", i, limit);
            }
        }
    }
    
    let health = limiter.get_health_status().await;
    println!("   📊 Health status: Redis available: {}, fallback requests: {}", 
             health.redis_available, health.fallback_requests);
    
    Ok(())
}

async fn performance_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await?;
    
    println!("   Running performance test (100 requests):");
    
    let start = std::time::Instant::now();
    let mut allowed_count = 0;
    let mut exceeded_count = 0;
    
    for i in 0..100 {
        let key = format!("perf_key_{}", i % 10); // 10 different keys
        let result = limiter.check_rate_limit(&key, 50, 3600).await?;
        
        match result {
            RateLimitResult::Allowed { .. } => allowed_count += 1,
            RateLimitResult::Exceeded { .. } => exceeded_count += 1,
        }
    }
    
    let duration = start.elapsed();
    let stats = limiter.get_comprehensive_stats().await;
    
    println!("   📊 Performance Results:");
    println!("      - Duration: {:?}", duration);
    println!("      - Requests/second: {:.2}", 100.0 / duration.as_secs_f64());
    println!("      - Allowed: {}, Exceeded: {}", allowed_count, exceeded_count);
    println!("      - Average latency: {:.2}ms", stats.average_latency_ms);
    println!("      - Backend: {}", if stats.redis_available { "Redis" } else { "In-memory" });
    
    Ok(())
}

/// Example of integrating with HTTP middleware
#[allow(dead_code)]
async fn middleware_integration_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("   HTTP Middleware Integration Example:");
    
    let config = DistributedRateLimitConfig::default();
    let limiter = DistributedRateLimiter::new(config).await?;
    
    // Add API-specific policies
    let api_policy = RateLimitPolicy {
        name: "api_v2_endpoints".to_string(),
        limit: 100,
        window_seconds: 3600,
        burst_limit: Some(20),
        priority: 1,
        scope: PolicyScope::PerEndpoint,
        custom_headers: {
            let mut headers = HashMap::new();
            headers.insert("X-RateLimit-Policy".to_string(), "api_v2".to_string());
            headers.insert("X-RateLimit-Scope".to_string(), "endpoint".to_string());
            headers
        },
        enabled: true,
    };
    
    limiter.add_policy(api_policy).await?;
    
    // Simulate HTTP request processing
    let request_context = RateLimitContext {
        client_ip: "203.0.113.1".to_string(),
        method: "POST".to_string(),
        endpoint: "/api/v2/pipelines".to_string(),
        user_id: Some("authenticated_user".to_string()),
        user_role: Some("standard".to_string()),
        api_key: None,
        request_id: Some(Uuid::new_v4()),
        metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("user_agent".to_string(), "MyApp/1.0".to_string());
            metadata.insert("source".to_string(), "web".to_string());
            metadata
        },
    };
    
    println!("      Processing request: {} {}", 
             request_context.method, request_context.endpoint);
    
    let result = limiter.check_request_with_policies("middleware_key", &request_context).await?;
    
    match result {
        RateLimitResult::Allowed { remaining, limit, reset_time } => {
            println!("      ✅ Request allowed");
            println!("         X-RateLimit-Limit: {}", limit);
            println!("         X-RateLimit-Remaining: {}", remaining);
            println!("         X-RateLimit-Reset: {}", reset_time);
        }
        RateLimitResult::Exceeded { limit, retry_after_seconds, reset_time, .. } => {
            println!("      ❌ Request rate limited");
            println!("         X-RateLimit-Limit: {}", limit);
            println!("         X-RateLimit-Remaining: 0");
            println!("         X-RateLimit-Reset: {}", reset_time);
            println!("         Retry-After: {}", retry_after_seconds);
        }
    }
    
    Ok(())
}

/// Example of Redis configuration for production
#[allow(dead_code)]
fn production_redis_config_example() {
    println!("   Production Redis Configuration Example:");
    
    let _production_config = DistributedRateLimitConfig {
        redis_url: Some("redis://redis-cluster.example.com:6379".to_string()),
        pool_size: 20,
        connection_timeout_ms: 3000,
        command_timeout_ms: 1000,
        cluster_mode: true,
        key_prefix: "rustci:prod:ratelimit".to_string(),
        default_ttl_seconds: 7200, // 2 hours
        enable_compression: true,
        fallback_to_memory: true,
        health_check_interval_seconds: 30,
    };
    
    println!("      - Redis cluster mode enabled");
    println!("      - Connection pool size: 20");
    println!("      - Compression enabled for bandwidth efficiency");
    println!("      - Fallback to in-memory when Redis unavailable");
    println!("      - Health checks every 30 seconds");
    println!("      - 2-hour TTL for rate limit keys");
}