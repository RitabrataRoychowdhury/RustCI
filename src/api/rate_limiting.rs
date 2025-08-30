//! Advanced API Rate Limiting System
//!
//! This module provides comprehensive rate limiting with Redis backend support,
//! per-endpoint and per-user policies, and intelligent retry guidance.

use crate::api::errors::{rate_limit_error, ApiError, ApiErrorCode, ApiErrorResponse, create_error_response};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Default rate limit (requests per window)
    pub default_limit: u32,
    /// Default time window in seconds
    pub default_window_seconds: u32,
    /// Per-endpoint rate limits
    pub endpoint_limits: HashMap<String, EndpointRateLimit>,
    /// Per-user rate limits
    pub user_limits: HashMap<String, UserRateLimit>,
    /// Global rate limits
    pub global_limits: Vec<GlobalRateLimit>,
    /// Redis connection string (optional)
    pub redis_url: Option<String>,
    /// Enable rate limit headers
    pub include_headers: bool,
    /// Enable rate limit monitoring
    pub enable_monitoring: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            default_limit: 1000,
            default_window_seconds: 3600, // 1 hour
            endpoint_limits: HashMap::new(),
            user_limits: HashMap::new(),
            global_limits: vec![
                GlobalRateLimit {
                    name: "burst".to_string(),
                    limit: 100,
                    window_seconds: 60, // 1 minute
                    scope: RateLimitScope::Global,
                },
            ],
            redis_url: None,
            include_headers: true,
            enable_monitoring: true,
        }
    }
}

/// Endpoint-specific rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointRateLimit {
    /// HTTP method
    pub method: String,
    /// Endpoint path pattern
    pub path: String,
    /// Rate limit (requests per window)
    pub limit: u32,
    /// Time window in seconds
    pub window_seconds: u32,
    /// Rate limit scope
    pub scope: RateLimitScope,
    /// Custom headers to include
    pub custom_headers: Option<HashMap<String, String>>,
}

/// User-specific rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRateLimit {
    /// User role or tier
    pub role: String,
    /// Rate limit (requests per window)
    pub limit: u32,
    /// Time window in seconds
    pub window_seconds: u32,
    /// Burst allowance
    pub burst_limit: Option<u32>,
    /// Priority level
    pub priority: RateLimitPriority,
}

/// Global rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalRateLimit {
    /// Rate limit name
    pub name: String,
    /// Rate limit (requests per window)
    pub limit: u32,
    /// Time window in seconds
    pub window_seconds: u32,
    /// Rate limit scope
    pub scope: RateLimitScope,
}

/// Rate limit scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitScope {
    /// Per IP address
    PerIp,
    /// Per authenticated user
    PerUser,
    /// Per API key
    PerApiKey,
    /// Global (across all requests)
    Global,
    /// Per endpoint
    PerEndpoint,
}

/// Rate limit priority
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Rate limit state for tracking usage
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// Current request count
    pub count: u32,
    /// Window start time
    pub window_start: u64,
    /// Window duration in seconds
    pub window_seconds: u32,
    /// Rate limit
    pub limit: u32,
    /// Last request time
    pub last_request: u64,
}

impl RateLimitState {
    /// Create new rate limit state
    pub fn new(limit: u32, window_seconds: u32) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            count: 0,
            window_start: now,
            window_seconds,
            limit,
            last_request: now,
        }
    }

    /// Check if request is allowed and update state
    pub fn check_and_update(&mut self) -> RateLimitResult {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check if we need to reset the window
        if now >= self.window_start + self.window_seconds as u64 {
            self.count = 0;
            self.window_start = now;
        }

        // Check if limit is exceeded
        if self.count >= self.limit {
            let reset_time = self.window_start + self.window_seconds as u64;
            let retry_after = reset_time.saturating_sub(now);

            return RateLimitResult::Exceeded {
                limit: self.limit,
                window_seconds: self.window_seconds,
                retry_after_seconds: retry_after as u32,
                reset_time,
            };
        }

        // Allow request and increment counter
        self.count += 1;
        self.last_request = now;

        RateLimitResult::Allowed {
            remaining: self.limit - self.count,
            reset_time: self.window_start + self.window_seconds as u64,
            limit: self.limit,
        }
    }

    /// Get current status without updating
    pub fn status(&self) -> RateLimitStatus {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let reset_time = self.window_start + self.window_seconds as u64;
        let remaining = if now >= reset_time {
            self.limit // Window has reset
        } else {
            self.limit.saturating_sub(self.count)
        };

        RateLimitStatus {
            limit: self.limit,
            remaining,
            reset_time,
            window_seconds: self.window_seconds,
        }
    }
}

/// Rate limit check result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed {
        remaining: u32,
        reset_time: u64,
        limit: u32,
    },
    /// Rate limit exceeded
    Exceeded {
        limit: u32,
        window_seconds: u32,
        retry_after_seconds: u32,
        reset_time: u64,
    },
}

/// Rate limit status information
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitStatus {
    /// Rate limit
    pub limit: u32,
    /// Remaining requests
    pub remaining: u32,
    /// Reset time (Unix timestamp)
    pub reset_time: u64,
    /// Window duration in seconds
    pub window_seconds: u32,
}

/// Rate limiter trait for different backends
#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync {
    /// Check rate limit for a key
    async fn check_rate_limit(&self, key: &str, limit: u32, window_seconds: u32) -> Result<RateLimitResult, RateLimitError>;
    
    /// Get current status for a key
    async fn get_status(&self, key: &str) -> Result<Option<RateLimitStatus>, RateLimitError>;
    
    /// Reset rate limit for a key
    async fn reset(&self, key: &str) -> Result<(), RateLimitError>;
    
    /// Get rate limit statistics
    async fn get_stats(&self) -> Result<RateLimitStats, RateLimitError>;
}

/// In-memory rate limiter implementation
#[derive(Debug)]
pub struct InMemoryRateLimiter {
    states: Arc<RwLock<HashMap<String, RateLimitState>>>,
}

impl InMemoryRateLimiter {
    /// Create new in-memory rate limiter
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn check_rate_limit(&self, key: &str, limit: u32, window_seconds: u32) -> Result<RateLimitResult, RateLimitError> {
        let mut states = self.states.write().await;
        
        let state = states
            .entry(key.to_string())
            .or_insert_with(|| RateLimitState::new(limit, window_seconds));
        
        // Update limits if they've changed
        if state.limit != limit || state.window_seconds != window_seconds {
            state.limit = limit;
            state.window_seconds = window_seconds;
        }
        
        Ok(state.check_and_update())
    }

    async fn get_status(&self, key: &str) -> Result<Option<RateLimitStatus>, RateLimitError> {
        let states = self.states.read().await;
        Ok(states.get(key).map(|state| state.status()))
    }

    async fn reset(&self, key: &str) -> Result<(), RateLimitError> {
        let mut states = self.states.write().await;
        states.remove(key);
        Ok(())
    }

    async fn get_stats(&self) -> Result<RateLimitStats, RateLimitError> {
        let states = self.states.read().await;
        let total_keys = states.len();
        let active_limits = states.values().filter(|s| s.count > 0).count();
        
        Ok(RateLimitStats {
            total_keys,
            active_limits,
            backend: "in-memory".to_string(),
        })
    }
}

/// Redis-based rate limiter implementation
#[derive(Debug)]
pub struct RedisRateLimiter {
    // This would contain Redis client
    // For now, we'll use a placeholder
    _redis_url: String,
}

impl RedisRateLimiter {
    /// Create new Redis rate limiter
    pub fn new(redis_url: String) -> Self {
        Self {
            _redis_url: redis_url,
        }
    }
}

#[async_trait::async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn check_rate_limit(&self, _key: &str, _limit: u32, _window_seconds: u32) -> Result<RateLimitResult, RateLimitError> {
        // TODO: Implement Redis-based rate limiting
        // This would use Redis commands like INCR, EXPIRE, etc.
        Err(RateLimitError::BackendError("Redis implementation not yet available".to_string()))
    }

    async fn get_status(&self, _key: &str) -> Result<Option<RateLimitStatus>, RateLimitError> {
        Err(RateLimitError::BackendError("Redis implementation not yet available".to_string()))
    }

    async fn reset(&self, _key: &str) -> Result<(), RateLimitError> {
        Err(RateLimitError::BackendError("Redis implementation not yet available".to_string()))
    }

    async fn get_stats(&self) -> Result<RateLimitStats, RateLimitError> {
        Ok(RateLimitStats {
            total_keys: 0,
            active_limits: 0,
            backend: "redis".to_string(),
        })
    }
}

/// Rate limit error types
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Backend error: {0}")]
    BackendError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Key error: {0}")]
    KeyError(String),
}

/// Rate limit statistics
#[derive(Debug, Serialize)]
pub struct RateLimitStats {
    /// Total number of tracked keys
    pub total_keys: usize,
    /// Number of keys with active limits
    pub active_limits: usize,
    /// Backend type
    pub backend: String,
}

/// Rate limiting service
pub struct RateLimitService {
    config: RateLimitConfig,
    limiter: Arc<dyn RateLimiter>,
}

impl RateLimitService {
    /// Create new rate limiting service
    pub fn new(config: RateLimitConfig) -> Self {
        let limiter: Arc<dyn RateLimiter> = if let Some(redis_url) = &config.redis_url {
            Arc::new(RedisRateLimiter::new(redis_url.clone()))
        } else {
            Arc::new(InMemoryRateLimiter::new())
        };

        Self { config, limiter }
    }

    /// Check rate limit for a request
    pub async fn check_request(&self, request_info: &RateLimitRequestInfo) -> Result<RateLimitResult, RateLimitError> {
        // Determine applicable rate limits
        let limits = self.get_applicable_limits(request_info);
        
        // Check each limit (most restrictive wins)
        let mut most_restrictive: Option<RateLimitResult> = None;
        
        for (key, limit, window) in limits {
            let result = self.limiter.check_rate_limit(&key, limit, window).await?;
            
            match (&result, &most_restrictive) {
                (RateLimitResult::Exceeded { .. }, _) => {
                    // If any limit is exceeded, return immediately
                    return Ok(result);
                }
                (RateLimitResult::Allowed { remaining, .. }, Some(RateLimitResult::Allowed { remaining: prev_remaining, .. })) => {
                    // Keep the most restrictive (lowest remaining)
                    if remaining < prev_remaining {
                        most_restrictive = Some(result);
                    }
                }
                (RateLimitResult::Allowed { .. }, None) => {
                    most_restrictive = Some(result);
                }
                _ => {}
            }
        }
        
        most_restrictive.ok_or_else(|| RateLimitError::ConfigError("No applicable rate limits found".to_string()))
    }

    /// Get applicable rate limits for a request
    fn get_applicable_limits(&self, request_info: &RateLimitRequestInfo) -> Vec<(String, u32, u32)> {
        let mut limits = Vec::new();
        
        // Global limits
        for global_limit in &self.config.global_limits {
            let key = match global_limit.scope {
                RateLimitScope::Global => "global".to_string(),
                RateLimitScope::PerIp => format!("ip:{}", request_info.client_ip),
                RateLimitScope::PerUser => {
                    if let Some(user_id) = &request_info.user_id {
                        format!("user:{}", user_id)
                    } else {
                        continue;
                    }
                }
                RateLimitScope::PerApiKey => {
                    if let Some(api_key) = &request_info.api_key {
                        format!("apikey:{}", api_key)
                    } else {
                        continue;
                    }
                }
                RateLimitScope::PerEndpoint => {
                    format!("endpoint:{}:{}", request_info.method, request_info.path)
                }
            };
            
            limits.push((key, global_limit.limit, global_limit.window_seconds));
        }
        
        // Endpoint-specific limits
        for endpoint_limit in self.config.endpoint_limits.values() {
            if endpoint_limit.method == request_info.method && 
               self.path_matches(&endpoint_limit.path, &request_info.path) {
                let key = match endpoint_limit.scope {
                    RateLimitScope::PerIp => format!("endpoint:{}:{}:ip:{}", endpoint_limit.method, endpoint_limit.path, request_info.client_ip),
                    RateLimitScope::PerUser => {
                        if let Some(user_id) = &request_info.user_id {
                            format!("endpoint:{}:{}:user:{}", endpoint_limit.method, endpoint_limit.path, user_id)
                        } else {
                            continue;
                        }
                    }
                    RateLimitScope::PerEndpoint => format!("endpoint:{}:{}", endpoint_limit.method, endpoint_limit.path),
                    _ => continue,
                };
                
                limits.push((key, endpoint_limit.limit, endpoint_limit.window_seconds));
            }
        }
        
        // User-specific limits
        if let Some(user_role) = &request_info.user_role {
            if let Some(user_limit) = self.config.user_limits.get(user_role) {
                let key = format!("user_role:{}:{}", user_role, request_info.user_id.as_deref().unwrap_or("anonymous"));
                limits.push((key, user_limit.limit, user_limit.window_seconds));
            }
        }
        
        // Default limit if no specific limits apply
        if limits.is_empty() {
            let key = format!("default:ip:{}", request_info.client_ip);
            limits.push((key, self.config.default_limit, self.config.default_window_seconds));
        }
        
        limits
    }

    /// Check if path matches pattern (simple implementation)
    fn path_matches(&self, pattern: &str, path: &str) -> bool {
        // Simple exact match for now
        // Could be enhanced with wildcards, regex, etc.
        pattern == path || pattern == "*"
    }

    /// Get rate limit headers for response
    pub fn get_rate_limit_headers(&self, result: &RateLimitResult) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        if !self.config.include_headers {
            return headers;
        }
        
        match result {
            RateLimitResult::Allowed { remaining, reset_time, limit } => {
                headers.insert("X-RateLimit-Limit", HeaderValue::from(*limit));
                headers.insert("X-RateLimit-Remaining", HeaderValue::from(*remaining));
                headers.insert("X-RateLimit-Reset", HeaderValue::from(*reset_time));
            }
            RateLimitResult::Exceeded { limit, retry_after_seconds, reset_time, .. } => {
                headers.insert("X-RateLimit-Limit", HeaderValue::from(*limit));
                headers.insert("X-RateLimit-Remaining", HeaderValue::from(0u32));
                headers.insert("X-RateLimit-Reset", HeaderValue::from(*reset_time));
                headers.insert("Retry-After", HeaderValue::from(*retry_after_seconds));
            }
        }
        
        headers
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> Result<RateLimitStats, RateLimitError> {
        self.limiter.get_stats().await
    }
}

/// Request information for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitRequestInfo {
    /// Client IP address
    pub client_ip: String,
    /// HTTP method
    pub method: String,
    /// Request path
    pub path: String,
    /// User ID (if authenticated)
    pub user_id: Option<String>,
    /// User role (if authenticated)
    pub user_role: Option<String>,
    /// API key (if provided)
    pub api_key: Option<String>,
    /// Request ID for correlation
    pub request_id: Option<Uuid>,
}

/// Rate limiting middleware
pub async fn rate_limiting_middleware(
    State(rate_limit_service): State<Arc<RateLimitService>>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiErrorResponse> {
    // Extract request information
    let request_info = extract_request_info(&req);
    
    debug!(
        client_ip = %request_info.client_ip,
        method = %request_info.method,
        path = %request_info.path,
        user_id = ?request_info.user_id,
        "Checking rate limit"
    );
    
    // Check rate limit
    let result = rate_limit_service
        .check_request(&request_info)
        .await
        .map_err(|e| {
            let error = ApiError {
                code: ApiErrorCode::InternalServerError.to_string(),
                message: "Rate limiting service error".to_string(),
                details: Some(serde_json::json!({"error": e.to_string()})),
                recovery_suggestions: vec![],
                documentation_url: None,
                correlation_id: request_info.request_id,
            };
            create_error_response(error, request_info.request_id, Some(request_info.path.clone()), Some(request_info.method.clone()))
        })?;
    
    // Handle rate limit result
    match result {
        RateLimitResult::Exceeded { limit, window_seconds, retry_after_seconds, .. } => {
            warn!(
                client_ip = %request_info.client_ip,
                method = %request_info.method,
                path = %request_info.path,
                limit = limit,
                window_seconds = window_seconds,
                retry_after = retry_after_seconds,
                "Rate limit exceeded"
            );
            
            let error = rate_limit_error(limit, window_seconds, retry_after_seconds);
            let error_response = create_error_response(
                error,
                request_info.request_id,
                Some(request_info.path),
                Some(request_info.method),
            );
            
            Err(error_response)
        }
        RateLimitResult::Allowed { .. } => {
            // Add rate limit info to request for handlers to use
            req.extensions_mut().insert(result.clone());
            
            // Continue with request
            let mut response = next.run(req).await;
            
            // Add rate limit headers to response
            let rate_limit_headers = rate_limit_service.get_rate_limit_headers(&result);
            response.headers_mut().extend(rate_limit_headers);
            
            Ok(response)
        }
    }
}

/// Extract request information for rate limiting
fn extract_request_info(req: &Request) -> RateLimitRequestInfo {
    // Extract client IP (simplified - in production, consider X-Forwarded-For, etc.)
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .unwrap_or("unknown")
        .trim()
        .to_string();
    
    // Extract method and path
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    
    // Extract user information from extensions (set by auth middleware)
    let user_id = req.extensions().get::<Uuid>().map(|id| id.to_string());
    let user_role = req.extensions().get::<String>().cloned(); // Assuming role is stored as String
    
    // Extract API key from headers
    let api_key = req
        .headers()
        .get("x-api-key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    
    // Extract request ID
    let request_id = req.extensions().get::<Uuid>().copied();
    
    RateLimitRequestInfo {
        client_ip,
        method,
        path,
        user_id,
        user_role,
        api_key,
        request_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_state_creation() {
        let state = RateLimitState::new(100, 3600);
        assert_eq!(state.limit, 100);
        assert_eq!(state.window_seconds, 3600);
        assert_eq!(state.count, 0);
    }

    #[test]
    fn test_rate_limit_state_check_and_update() {
        let mut state = RateLimitState::new(2, 60);
        
        // First request should be allowed
        match state.check_and_update() {
            RateLimitResult::Allowed { remaining, .. } => {
                assert_eq!(remaining, 1);
            }
            _ => panic!("Expected allowed"),
        }
        
        // Second request should be allowed
        match state.check_and_update() {
            RateLimitResult::Allowed { remaining, .. } => {
                assert_eq!(remaining, 0);
            }
            _ => panic!("Expected allowed"),
        }
        
        // Third request should be exceeded
        match state.check_and_update() {
            RateLimitResult::Exceeded { limit, .. } => {
                assert_eq!(limit, 2);
            }
            _ => panic!("Expected exceeded"),
        }
    }

    #[tokio::test]
    async fn test_in_memory_rate_limiter() {
        let limiter = InMemoryRateLimiter::new();
        
        // First request should be allowed
        let result = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
        match result {
            RateLimitResult::Allowed { remaining, .. } => {
                assert_eq!(remaining, 1);
            }
            _ => panic!("Expected allowed"),
        }
        
        // Second request should be allowed
        let result = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
        match result {
            RateLimitResult::Allowed { remaining, .. } => {
                assert_eq!(remaining, 0);
            }
            _ => panic!("Expected allowed"),
        }
        
        // Third request should be exceeded
        let result = limiter.check_rate_limit("test_key", 2, 60).await.unwrap();
        match result {
            RateLimitResult::Exceeded { .. } => {}
            _ => panic!("Expected exceeded"),
        }
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.default_limit, 1000);
        assert_eq!(config.default_window_seconds, 3600);
        assert!(config.include_headers);
        assert!(config.enable_monitoring);
    }

    #[test]
    fn test_request_info_extraction() {
        use axum::body::Body;
        use axum::http::{Method, Request};
        
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v2/pipelines")
            .header("x-forwarded-for", "192.168.1.1, 10.0.0.1")
            .header("x-api-key", "test-api-key")
            .body(Body::empty())
            .unwrap();
        
        let info = extract_request_info(&req);
        assert_eq!(info.client_ip, "192.168.1.1");
        assert_eq!(info.method, "GET");
        assert_eq!(info.path, "/api/v2/pipelines");
        assert_eq!(info.api_key, Some("test-api-key".to_string()));
    }

    #[test]
    fn test_rate_limit_service_creation() {
        let config = RateLimitConfig::default();
        let service = RateLimitService::new(config);
        
        // Service should be created successfully
        assert!(service.config.default_limit > 0);
    }

    #[tokio::test]
    async fn test_rate_limit_service_stats() {
        let config = RateLimitConfig::default();
        let service = RateLimitService::new(config);
        
        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats.backend, "in-memory");
    }
}