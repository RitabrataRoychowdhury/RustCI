//! Distributed Rate Limiting System
//!
//! This module provides distributed rate limiting with Redis backend
//! and configurable policies per endpoint and user.
//!
//! This is currently a placeholder implementation that will be replaced
//! by Yggdrasil storage backend for ultra-low latency operations.

use crate::api::rate_limiting::{
    RateLimitResult, RateLimitError, RateLimitStats, RateLimitStatus, RateLimiter
};

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, warn, error, info};
use uuid::Uuid;

/// Configuration for distributed rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedRateLimitConfig {
    /// Redis connection string for distributed backend
    pub redis_url: Option<String>,
    /// Connection pool size
    pub pool_size: u32,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
    /// Command timeout in milliseconds
    pub command_timeout_ms: u64,
    /// Enable Redis cluster mode
    pub cluster_mode: bool,
    /// Redis key prefix for rate limiting
    pub key_prefix: String,
    /// Default TTL for rate limit keys in seconds
    pub default_ttl_seconds: u64,
    /// Enable compression for Redis values
    pub enable_compression: bool,
    /// Fallback to in-memory when Redis is unavailable
    pub fallback_to_memory: bool,
    /// Health check interval in seconds
    pub health_check_interval_seconds: u64,
}

impl Default for DistributedRateLimitConfig {
    fn default() -> Self {
        Self {
            redis_url: None,
            pool_size: 10,
            connection_timeout_ms: 5000,
            command_timeout_ms: 1000,
            cluster_mode: false,
            key_prefix: "rustci:ratelimit".to_string(),
            default_ttl_seconds: 3600,
            enable_compression: false,
            fallback_to_memory: true,
            health_check_interval_seconds: 30,
        }
    }
}

/// Policy configuration for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitPolicy {
    /// Policy name/identifier
    pub name: String,
    /// Rate limit (requests per window)
    pub limit: u32,
    /// Time window in seconds
    pub window_seconds: u32,
    /// Burst allowance (optional)
    pub burst_limit: Option<u32>,
    /// Policy priority (higher number = higher priority)
    pub priority: u32,
    /// Policy scope
    pub scope: PolicyScope,
    /// Custom headers to include in responses
    pub custom_headers: HashMap<String, String>,
    /// Whether this policy is enabled
    pub enabled: bool,
}

/// Scope for rate limiting policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyScope {
    /// Per IP address
    PerIp,
    /// Per authenticated user
    PerUser,
    /// Per API key
    PerApiKey,
    /// Per endpoint
    PerEndpoint,
    /// Global across all requests
    Global,
    /// Custom scope with pattern
    Custom(String),
}

/// Distributed rate limiter implementation
pub struct DistributedRateLimiter {
    config: DistributedRateLimitConfig,
    policies: Arc<RwLock<HashMap<String, RateLimitPolicy>>>,
    // Placeholder for Redis client - will be replaced by Yggdrasil
    redis_client: Option<Arc<dyn RedisClient>>,
    // Fallback in-memory limiter
    fallback_limiter: Arc<dyn RateLimiter>,
    // Health status
    health_status: Arc<RwLock<HealthStatus>>,
    // Statistics
    stats: Arc<RwLock<DistributedRateLimitStats>>,
}

/// Health status for the distributed rate limiter
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub redis_available: bool,
    pub last_health_check: SystemTime,
    pub consecutive_failures: u32,
    pub total_requests: u64,
    pub redis_requests: u64,
    pub fallback_requests: u64,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            redis_available: false,
            last_health_check: SystemTime::now(),
            consecutive_failures: 0,
            total_requests: 0,
            redis_requests: 0,
            fallback_requests: 0,
        }
    }
}

/// Statistics for distributed rate limiting
#[derive(Debug, Clone, Serialize)]
pub struct DistributedRateLimitStats {
    pub total_requests: u64,
    pub redis_requests: u64,
    pub fallback_requests: u64,
    pub redis_errors: u64,
    pub policy_hits: HashMap<String, u64>,
    pub average_latency_ms: f64,
    pub redis_available: bool,
    pub active_policies: usize,
}

impl Default for DistributedRateLimitStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            redis_requests: 0,
            fallback_requests: 0,
            redis_errors: 0,
            policy_hits: HashMap::new(),
            average_latency_ms: 0.0,
            redis_available: false,
            active_policies: 0,
        }
    }
}

/// Trait for Redis client abstraction (placeholder for future Yggdrasil integration)
#[async_trait::async_trait]
pub trait RedisClient: Send + Sync {
    /// Execute rate limit check with Lua script
    async fn check_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_seconds: u32,
    ) -> Result<RateLimitResult, RateLimitError>;
    
    /// Get current rate limit status
    async fn get_status(&self, key: &str) -> Result<Option<RateLimitStatus>, RateLimitError>;
    
    /// Reset rate limit for a key
    async fn reset(&self, key: &str) -> Result<(), RateLimitError>;
    
    /// Health check
    async fn health_check(&self) -> Result<(), RateLimitError>;
    
    /// Get connection statistics
    async fn get_connection_stats(&self) -> Result<ConnectionStats, RateLimitError>;
}

/// Connection statistics for Redis client
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionStats {
    pub active_connections: u32,
    pub total_connections: u32,
    pub failed_connections: u32,
    pub average_response_time_ms: f64,
}

/// Placeholder Redis client implementation
/// TODO: Replace with actual Redis implementation using deadpool-redis
pub struct PlaceholderRedisClient {
    config: DistributedRateLimitConfig,
}

impl PlaceholderRedisClient {
    pub fn new(config: DistributedRateLimitConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl RedisClient for PlaceholderRedisClient {
    async fn check_rate_limit(
        &self,
        _key: &str,
        _limit: u32,
        _window_seconds: u32,
    ) -> Result<RateLimitResult, RateLimitError> {
        // TODO: Implement actual Redis-based rate limiting with Lua scripts
        // This will be replaced by Yggdrasil storage backend
        warn!("Using placeholder Redis client - rate limiting will fall back to in-memory");
        Err(RateLimitError::BackendError(
            "Redis client not implemented - using fallback".to_string()
        ))
    }
    
    async fn get_status(&self, _key: &str) -> Result<Option<RateLimitStatus>, RateLimitError> {
        Err(RateLimitError::BackendError(
            "Redis client not implemented".to_string()
        ))
    }
    
    async fn reset(&self, _key: &str) -> Result<(), RateLimitError> {
        Err(RateLimitError::BackendError(
            "Redis client not implemented".to_string()
        ))
    }
    
    async fn health_check(&self) -> Result<(), RateLimitError> {
        // Simulate health check failure for placeholder
        Err(RateLimitError::BackendError(
            "Placeholder Redis client - health check failed".to_string()
        ))
    }
    
    async fn get_connection_stats(&self) -> Result<ConnectionStats, RateLimitError> {
        Ok(ConnectionStats {
            active_connections: 0,
            total_connections: 0,
            failed_connections: 1,
            average_response_time_ms: 0.0,
        })
    }
}

impl DistributedRateLimiter {
    /// Create new distributed rate limiter
    pub async fn new(config: DistributedRateLimitConfig) -> Result<Self, RateLimitError> {
        info!("Initializing distributed rate limiter with config: {:?}", config);
        
        // Create Redis client (placeholder for now)
        let redis_client = if config.redis_url.is_some() {
            Some(Arc::new(PlaceholderRedisClient::new(config.clone())) as Arc<dyn RedisClient>)
        } else {
            None
        };
        
        // Create fallback in-memory limiter
        let fallback_limiter = Arc::new(crate::api::rate_limiting::InMemoryRateLimiter::new());
        
        let limiter = Self {
            config,
            policies: Arc::new(RwLock::new(HashMap::new())),
            redis_client,
            fallback_limiter,
            health_status: Arc::new(RwLock::new(HealthStatus::default())),
            stats: Arc::new(RwLock::new(DistributedRateLimitStats::default())),
        };
        
        // Start health check task
        limiter.start_health_check_task().await;
        
        Ok(limiter)
    }
    
    /// Add or update a rate limiting policy
    pub async fn add_policy(&self, policy: RateLimitPolicy) -> Result<(), RateLimitError> {
        info!("Adding rate limiting policy: {}", policy.name);
        
        if !policy.enabled {
            warn!("Policy {} is disabled", policy.name);
        }
        
        let mut policies = self.policies.write().await;
        policies.insert(policy.name.clone(), policy);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.active_policies = policies.len();
        
        Ok(())
    }
    
    /// Remove a rate limiting policy
    pub async fn remove_policy(&self, policy_name: &str) -> Result<bool, RateLimitError> {
        info!("Removing rate limiting policy: {}", policy_name);
        
        let mut policies = self.policies.write().await;
        let removed = policies.remove(policy_name).is_some();
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.active_policies = policies.len();
        
        Ok(removed)
    }
    
    /// Get all configured policies
    pub async fn get_policies(&self) -> HashMap<String, RateLimitPolicy> {
        self.policies.read().await.clone()
    }
    
    /// Check rate limit for a request with policy evaluation
    pub async fn check_request_with_policies(
        &self,
        key: &str,
        context: &RateLimitContext,
    ) -> Result<RateLimitResult, RateLimitError> {
        let start_time = SystemTime::now();
        
        // Update total requests
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }
        
        // Find applicable policies
        let applicable_policies = self.find_applicable_policies(context).await;
        
        if applicable_policies.is_empty() {
            debug!("No applicable policies found for key: {}", key);
            return Ok(RateLimitResult::Allowed {
                remaining: u32::MAX,
                reset_time: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() + 3600,
                limit: u32::MAX,
            });
        }
        
        // Check each policy (most restrictive wins)
        let mut most_restrictive: Option<RateLimitResult> = None;
        
        for policy in applicable_policies {
            let policy_key = format!("{}:{}:{}", self.config.key_prefix, policy.name, key);
            
            let result = self.check_rate_limit_internal(
                &policy_key,
                policy.limit,
                policy.window_seconds,
            ).await?;
            
            // Update policy hit statistics
            {
                let mut stats = self.stats.write().await;
                *stats.policy_hits.entry(policy.name.clone()).or_insert(0) += 1;
            }
            
            match (&result, &most_restrictive) {
                (RateLimitResult::Exceeded { .. }, _) => {
                    // If any policy is exceeded, return immediately
                    self.update_latency_stats(start_time).await;
                    return Ok(result);
                }
                (RateLimitResult::Allowed { remaining, .. }, Some(RateLimitResult::Allowed { remaining: prev_remaining, .. })) => {
                    // Keep the most restrictive (lowest remaining)
                    if remaining < prev_remaining {
                        most_restrictive = Some(result);
                    }
                }
                (RateLimitResult::Allowed { .. }, Some(RateLimitResult::Exceeded { .. })) => {
                    // Previous result was exceeded, keep it
                    // most_restrictive remains unchanged
                }
                (RateLimitResult::Allowed { .. }, None) => {
                    most_restrictive = Some(result);
                }
            }
        }
        
        self.update_latency_stats(start_time).await;
        
        most_restrictive.ok_or_else(|| {
            RateLimitError::ConfigError("No applicable rate limits processed".to_string())
        })
    }
    
    /// Internal rate limit check with fallback logic
    async fn check_rate_limit_internal(
        &self,
        key: &str,
        limit: u32,
        window_seconds: u32,
    ) -> Result<RateLimitResult, RateLimitError> {
        // Try Redis first if available
        if let Some(redis_client) = &self.redis_client {
            let health_status = self.health_status.read().await;
            if health_status.redis_available {
                drop(health_status);
                
                match redis_client.check_rate_limit(key, limit, window_seconds).await {
                    Ok(result) => {
                        // Update Redis request count
                        let mut stats = self.stats.write().await;
                        stats.redis_requests += 1;
                        return Ok(result);
                    }
                    Err(e) => {
                        error!("Redis rate limit check failed: {}", e);
                        
                        // Update error count and health status
                        {
                            let mut stats = self.stats.write().await;
                            stats.redis_errors += 1;
                        }
                        
                        {
                            let mut health_status = self.health_status.write().await;
                            health_status.consecutive_failures += 1;
                            if health_status.consecutive_failures >= 3 {
                                health_status.redis_available = false;
                                warn!("Marking Redis as unavailable after {} consecutive failures", 
                                      health_status.consecutive_failures);
                            }
                        }
                        
                        // Fall through to fallback if enabled
                        if !self.config.fallback_to_memory {
                            return Err(e);
                        }
                    }
                }
            }
        }
        
        // Use fallback in-memory limiter
        debug!("Using fallback in-memory rate limiter for key: {}", key);
        
        let result = self.fallback_limiter.check_rate_limit(key, limit, window_seconds).await?;
        
        // Update fallback request count
        let mut stats = self.stats.write().await;
        stats.fallback_requests += 1;
        
        Ok(result)
    }
    
    /// Find applicable policies for a request context
    async fn find_applicable_policies(&self, context: &RateLimitContext) -> Vec<RateLimitPolicy> {
        let policies = self.policies.read().await;
        let mut applicable = Vec::new();
        
        for policy in policies.values() {
            if !policy.enabled {
                continue;
            }
            
            let matches = match &policy.scope {
                PolicyScope::PerIp => true, // Always applicable for IP-based limiting
                PolicyScope::PerUser => context.user_id.is_some(),
                PolicyScope::PerApiKey => context.api_key.is_some(),
                PolicyScope::PerEndpoint => true, // Always applicable for endpoint-based limiting
                PolicyScope::Global => true, // Always applicable for global limiting
                PolicyScope::Custom(pattern) => {
                    // Simple pattern matching - could be enhanced with regex
                    context.endpoint.contains(pattern) || context.method.contains(pattern)
                }
            };
            
            if matches {
                applicable.push(policy.clone());
            }
        }
        
        // Sort by priority (higher priority first)
        applicable.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        applicable
    }
    
    /// Update latency statistics
    async fn update_latency_stats(&self, start_time: SystemTime) {
        if let Ok(duration) = start_time.elapsed() {
            let latency_ms = duration.as_millis() as f64;
            
            let mut stats = self.stats.write().await;
            // Simple moving average (could be enhanced with more sophisticated metrics)
            stats.average_latency_ms = (stats.average_latency_ms * 0.9) + (latency_ms * 0.1);
        }
    }
    
    /// Start background health check task
    async fn start_health_check_task(&self) {
        if let Some(redis_client) = &self.redis_client {
            let redis_client = redis_client.clone();
            let health_status = self.health_status.clone();
            let stats = self.stats.clone();
            let interval = Duration::from_secs(self.config.health_check_interval_seconds);
            
            tokio::spawn(async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    match redis_client.health_check().await {
                        Ok(()) => {
                            let mut health = health_status.write().await;
                            health.redis_available = true;
                            health.consecutive_failures = 0;
                            health.last_health_check = SystemTime::now();
                            
                            let mut stats = stats.write().await;
                            stats.redis_available = true;
                            
                            debug!("Redis health check passed");
                        }
                        Err(e) => {
                            let mut health = health_status.write().await;
                            health.consecutive_failures += 1;
                            health.last_health_check = SystemTime::now();
                            
                            if health.consecutive_failures >= 3 {
                                health.redis_available = false;
                                
                                let mut stats = stats.write().await;
                                stats.redis_available = false;
                                
                                warn!("Redis health check failed: {}", e);
                            }
                        }
                    }
                }
            });
        }
    }
    
    /// Get comprehensive statistics
    pub async fn get_comprehensive_stats(&self) -> DistributedRateLimitStats {
        let mut stats = self.stats.read().await.clone();
        
        // Update active policies count
        let policies = self.policies.read().await;
        stats.active_policies = policies.len();
        
        stats
    }
    
    /// Get health status
    pub async fn get_health_status(&self) -> HealthStatus {
        self.health_status.read().await.clone()
    }
    
    /// Reset all rate limits (for testing/admin purposes)
    pub async fn reset_all(&self) -> Result<(), RateLimitError> {
        warn!("Resetting all rate limits - this should only be used for testing or emergency situations");
        
        // Reset fallback limiter
        // Note: InMemoryRateLimiter doesn't have a reset_all method, so we'd need to implement it
        // For now, we'll just log the action
        
        if let Some(redis_client) = &self.redis_client {
            // TODO: Implement Redis FLUSHDB or pattern-based deletion
            debug!("Would reset Redis rate limit keys with pattern: {}:*", self.config.key_prefix);
        }
        
        // Reset statistics
        {
            let mut stats = self.stats.write().await;
            *stats = DistributedRateLimitStats::default();
        }
        
        Ok(())
    }
}

/// Context for rate limiting decisions
#[derive(Debug, Clone)]
pub struct RateLimitContext {
    /// Client IP address
    pub client_ip: String,
    /// HTTP method
    pub method: String,
    /// Request endpoint/path
    pub endpoint: String,
    /// User ID (if authenticated)
    pub user_id: Option<String>,
    /// User role (if authenticated)
    pub user_role: Option<String>,
    /// API key (if provided)
    pub api_key: Option<String>,
    /// Request ID for correlation
    pub request_id: Option<Uuid>,
    /// Additional context data
    pub metadata: HashMap<String, String>,
}

#[async_trait::async_trait]
impl RateLimiter for DistributedRateLimiter {
    async fn check_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_seconds: u32,
    ) -> Result<RateLimitResult, RateLimitError> {
        self.check_rate_limit_internal(key, limit, window_seconds).await
    }
    
    async fn get_status(&self, key: &str) -> Result<Option<RateLimitStatus>, RateLimitError> {
        // Try Redis first, then fallback
        if let Some(redis_client) = &self.redis_client {
            let health_status = self.health_status.read().await;
            if health_status.redis_available {
                drop(health_status);
                
                match redis_client.get_status(key).await {
                    Ok(status) => return Ok(status),
                    Err(_) => {
                        // Fall through to fallback
                    }
                }
            }
        }
        
        self.fallback_limiter.get_status(key).await
    }
    
    async fn reset(&self, key: &str) -> Result<(), RateLimitError> {
        // Reset in both Redis and fallback
        if let Some(redis_client) = &self.redis_client {
            let _ = redis_client.reset(key).await; // Ignore errors
        }
        
        self.fallback_limiter.reset(key).await
    }
    
    async fn get_stats(&self) -> Result<RateLimitStats, RateLimitError> {
        let distributed_stats = self.get_comprehensive_stats().await;
        
        Ok(RateLimitStats {
            total_keys: distributed_stats.total_requests as usize,
            active_limits: distributed_stats.active_policies,
            backend: if distributed_stats.redis_available {
                "distributed-redis".to_string()
            } else {
                "distributed-fallback".to_string()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_distributed_rate_limiter_creation() {
        let config = DistributedRateLimitConfig::default();
        let limiter = DistributedRateLimiter::new(config).await.unwrap();
        
        let stats = limiter.get_comprehensive_stats().await;
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.active_policies, 0);
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
        
        // Remove policy
        let removed = limiter.remove_policy("test_policy").await.unwrap();
        assert!(removed);
        
        let policies = limiter.get_policies().await;
        assert_eq!(policies.len(), 0);
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
        let result = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
        match result {
            RateLimitResult::Allowed { remaining, .. } => {
                assert_eq!(remaining, 1);
            }
            _ => panic!("Expected allowed"),
        }
        
        let stats = limiter.get_comprehensive_stats().await;
        assert_eq!(stats.fallback_requests, 1);
        assert_eq!(stats.redis_requests, 0);
    }
    
    #[tokio::test]
    async fn test_rate_limit_context() {
        let context = RateLimitContext {
            client_ip: "192.168.1.1".to_string(),
            method: "GET".to_string(),
            endpoint: "/api/v2/pipelines".to_string(),
            user_id: Some("user123".to_string()),
            user_role: Some("premium".to_string()),
            api_key: None,
            request_id: Some(Uuid::new_v4()),
            metadata: HashMap::new(),
        };
        
        assert_eq!(context.client_ip, "192.168.1.1");
        assert_eq!(context.method, "GET");
        assert_eq!(context.endpoint, "/api/v2/pipelines");
        assert!(context.user_id.is_some());
    }
    
    #[tokio::test]
    async fn test_policy_scope_matching() {
        let config = DistributedRateLimitConfig::default();
        let limiter = DistributedRateLimiter::new(config).await.unwrap();
        
        // Add policies with different scopes
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
        ];
        
        for policy in policies {
            limiter.add_policy(policy).await.unwrap();
        }
        
        let context = RateLimitContext {
            client_ip: "192.168.1.1".to_string(),
            method: "GET".to_string(),
            endpoint: "/api/v2/test".to_string(),
            user_id: Some("user123".to_string()),
            user_role: None,
            api_key: None,
            request_id: None,
            metadata: HashMap::new(),
        };
        
        let applicable = limiter.find_applicable_policies(&context).await;
        assert_eq!(applicable.len(), 2); // Both PerIp and PerUser should match
        
        // Should be sorted by priority (higher first)
        assert_eq!(applicable[0].name, "per_user"); // Priority 2
        assert_eq!(applicable[1].name, "per_ip");   // Priority 1
    }
    
    #[test]
    fn test_config_defaults() {
        let config = DistributedRateLimitConfig::default();
        assert_eq!(config.pool_size, 10);
        assert_eq!(config.connection_timeout_ms, 5000);
        assert_eq!(config.key_prefix, "rustci:ratelimit");
        assert!(config.fallback_to_memory);
    }
}