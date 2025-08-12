//! Rate Limiting Middleware
//!
//! This module implements rate limiting for API endpoints to prevent abuse
//! and ensure fair resource usage across users and nodes.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::{error::AppError, AppState};

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Enable rate limiting
    pub enabled: bool,
    /// Rate limit per IP
    pub per_ip_limit: Option<u32>,
    /// Rate limit per user
    pub per_user_limit: Option<u32>,
    /// Rate limit per API key
    pub per_api_key_limit: Option<u32>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 1000,
            window_duration: Duration::from_secs(3600), // 1 hour
            enabled: true,
            per_ip_limit: Some(100),
            per_user_limit: Some(500),
            per_api_key_limit: Some(1000),
        }
    }
}

/// Builder for RateLimitConfig using the Builder pattern
pub struct RateLimitConfigBuilder {
    max_requests: u32,
    window_duration: Duration,
    enabled: bool,
    per_ip_limit: Option<u32>,
    per_user_limit: Option<u32>,
    per_api_key_limit: Option<u32>,
}

impl RateLimitConfigBuilder {
    /// Create a new rate limit configuration builder
    pub fn new() -> Self {
        Self {
            max_requests: 1000,
            window_duration: Duration::from_secs(3600),
            enabled: true,
            per_ip_limit: Some(100),
            per_user_limit: Some(500),
            per_api_key_limit: Some(1000),
        }
    }
    
    /// Set maximum requests per window
    pub fn max_requests(mut self, max_requests: u32) -> Self {
        self.max_requests = max_requests;
        self
    }
    
    /// Set window duration
    pub fn window_duration(mut self, duration: Duration) -> Self {
        self.window_duration = duration;
        self
    }
    
    /// Enable or disable rate limiting
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    
    /// Set per-IP rate limit
    pub fn per_ip_limit(mut self, limit: Option<u32>) -> Self {
        self.per_ip_limit = limit;
        self
    }
    
    /// Set per-user rate limit
    pub fn per_user_limit(mut self, limit: Option<u32>) -> Self {
        self.per_user_limit = limit;
        self
    }
    
    /// Set per-API-key rate limit
    pub fn per_api_key_limit(mut self, limit: Option<u32>) -> Self {
        self.per_api_key_limit = limit;
        self
    }
    
    /// Configure for development environment (more permissive)
    pub fn for_development(mut self) -> Self {
        self.max_requests = 10000;
        self.per_ip_limit = Some(1000);
        self.per_user_limit = Some(5000);
        self.per_api_key_limit = Some(10000);
        self
    }
    
    /// Configure for production environment (more restrictive)
    pub fn for_production(mut self) -> Self {
        self.max_requests = 500;
        self.per_ip_limit = Some(50);
        self.per_user_limit = Some(200);
        self.per_api_key_limit = Some(500);
        self
    }
    
    /// Build the final configuration
    pub fn build(self) -> RateLimitConfig {
        RateLimitConfig {
            max_requests: self.max_requests,
            window_duration: self.window_duration,
            enabled: self.enabled,
            per_ip_limit: self.per_ip_limit,
            per_user_limit: self.per_user_limit,
            per_api_key_limit: self.per_api_key_limit,
        }
    }
}

impl Default for RateLimitConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate limit bucket for tracking requests
#[derive(Debug, Clone)]
struct RateLimitBucket {
    /// Number of requests in current window
    requests: u32,
    /// Window start time
    window_start: Instant,
    /// Window duration
    window_duration: Duration,
    /// Maximum requests allowed
    max_requests: u32,
}

impl RateLimitBucket {
    fn new(max_requests: u32, window_duration: Duration) -> Self {
        Self {
            requests: 0,
            window_start: Instant::now(),
            window_duration,
            max_requests,
        }
    }

    /// Check if request is allowed and increment counter
    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        
        // Reset window if expired
        if now.duration_since(self.window_start) >= self.window_duration {
            self.requests = 0;
            self.window_start = now;
        }

        // Check if under limit
        if self.requests < self.max_requests {
            self.requests += 1;
            true
        } else {
            false
        }
    }

    /// Get remaining requests in current window
    fn remaining(&self) -> u32 {
        let now = Instant::now();
        
        // If window expired, full limit is available
        if now.duration_since(self.window_start) >= self.window_duration {
            self.max_requests
        } else {
            self.max_requests.saturating_sub(self.requests)
        }
    }

    /// Get time until window reset
    fn reset_time(&self) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);
        
        if elapsed >= self.window_duration {
            Duration::from_secs(0)
        } else {
            self.window_duration - elapsed
        }
    }
}

/// Rate limiter state
pub struct RateLimiter {
    /// Rate limit buckets by IP address
    ip_buckets: Arc<RwLock<HashMap<IpAddr, RateLimitBucket>>>,
    /// Rate limit buckets by user ID
    user_buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
    /// Rate limit buckets by API key
    api_key_buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
    /// Configuration
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            ip_buckets: Arc::new(RwLock::new(HashMap::new())),
            user_buckets: Arc::new(RwLock::new(HashMap::new())),
            api_key_buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check rate limit for IP address
    async fn check_ip_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        if let Some(limit) = self.config.per_ip_limit {
            let mut buckets = self.ip_buckets.write().await;
            let bucket = buckets
                .entry(ip)
                .or_insert_with(|| RateLimitBucket::new(limit, self.config.window_duration));

            if !bucket.try_consume() {
                return Err(RateLimitError {
                    limit_type: "ip".to_string(),
                    identifier: ip.to_string(),
                    limit: limit,
                    remaining: bucket.remaining(),
                    reset_time: bucket.reset_time(),
                });
            }
        }
        Ok(())
    }

    /// Check rate limit for user
    async fn check_user_limit(&self, user_id: &str) -> Result<(), RateLimitError> {
        if let Some(limit) = self.config.per_user_limit {
            let mut buckets = self.user_buckets.write().await;
            let bucket = buckets
                .entry(user_id.to_string())
                .or_insert_with(|| RateLimitBucket::new(limit, self.config.window_duration));

            if !bucket.try_consume() {
                return Err(RateLimitError {
                    limit_type: "user".to_string(),
                    identifier: user_id.to_string(),
                    limit: limit,
                    remaining: bucket.remaining(),
                    reset_time: bucket.reset_time(),
                });
            }
        }
        Ok(())
    }

    /// Check rate limit for API key
    async fn check_api_key_limit(&self, api_key: &str) -> Result<(), RateLimitError> {
        if let Some(limit) = self.config.per_api_key_limit {
            let mut buckets = self.api_key_buckets.write().await;
            let bucket = buckets
                .entry(api_key.to_string())
                .or_insert_with(|| RateLimitBucket::new(limit, self.config.window_duration));

            if !bucket.try_consume() {
                return Err(RateLimitError {
                    limit_type: "api_key".to_string(),
                    identifier: api_key.to_string(),
                    limit: limit,
                    remaining: bucket.remaining(),
                    reset_time: bucket.reset_time(),
                });
            }
        }
        Ok(())
    }
}

/// Rate limit error
#[derive(Debug)]
pub struct RateLimitError {
    pub limit_type: String,
    pub identifier: String,
    pub limit: u32,
    pub remaining: u32,
    pub reset_time: Duration,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert("X-RateLimit-Limit", self.limit.to_string().parse().unwrap());
        headers.insert("X-RateLimit-Remaining", self.remaining.to_string().parse().unwrap());
        headers.insert("X-RateLimit-Reset", self.reset_time.as_secs().to_string().parse().unwrap());
        headers.insert("Retry-After", self.reset_time.as_secs().to_string().parse().unwrap());

        let body = serde_json::json!({
            "error": "Rate limit exceeded",
            "limit_type": self.limit_type,
            "limit": self.limit,
            "remaining": self.remaining,
            "reset_in_seconds": self.reset_time.as_secs()
        });

        (StatusCode::TOO_MANY_REQUESTS, headers, body.to_string()).into_response()
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Skip rate limiting if disabled
    if !state.env.security.rate_limiting.enabled {
        return Ok(next.run(req).await);
    }

    // Extract client IP
    let client_ip = extract_client_ip(&req);
    
    // Extract user ID from security context (if authenticated)
    let user_id = req.extensions().get::<crate::core::networking::security::SecurityContext>()
        .map(|ctx| ctx.user_id.to_string());

    // Extract API key from headers
    let api_key = req.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // Create rate limiter from app config
    let rate_limit_config = RateLimitConfig {
        max_requests: state.env.security.rate_limiting.requests_per_minute,
        window_duration: Duration::from_secs(60), // 1 minute window
        enabled: state.env.security.rate_limiting.enabled,
        per_ip_limit: Some(state.env.security.rate_limiting.requests_per_minute),
        per_user_limit: if state.env.security.rate_limiting.enable_per_user_limits {
            Some(state.env.security.rate_limiting.requests_per_minute * 2)
        } else {
            None
        },
        per_api_key_limit: Some(state.env.security.rate_limiting.requests_per_minute * 5),
    };
    let rate_limiter = RateLimiter::new(rate_limit_config);

    // Check IP rate limit
    if let Some(ip) = client_ip {
        if let Err(e) = rate_limiter.check_ip_limit(ip).await {
            warn!(
                ip = %ip,
                limit_type = %e.limit_type,
                "Rate limit exceeded for IP"
            );
            return Err(AppError::RateLimitExceededSimple(format!(
                "Rate limit exceeded for IP {}: {} requests per {} seconds",
                ip, e.limit, e.reset_time.as_secs()
            )));
        }
    }

    // Check user rate limit
    if let Some(user_id) = &user_id {
        if let Err(e) = rate_limiter.check_user_limit(user_id).await {
            warn!(
                user_id = %user_id,
                limit_type = %e.limit_type,
                "Rate limit exceeded for user"
            );
            return Err(AppError::RateLimitExceededSimple(format!(
                "Rate limit exceeded for user {}: {} requests per {} seconds",
                user_id, e.limit, e.reset_time.as_secs()
            )));
        }
    }

    // Check API key rate limit
    if let Some(api_key) = &api_key {
        if let Err(e) = rate_limiter.check_api_key_limit(api_key).await {
            warn!(
                api_key = %api_key,
                limit_type = %e.limit_type,
                "Rate limit exceeded for API key"
            );
            return Err(AppError::RateLimitExceededSimple(format!(
                "Rate limit exceeded for API key: {} requests per {} seconds",
                e.limit, e.reset_time.as_secs()
            )));
        }
    }

    debug!(
        ip = ?client_ip,
        user_id = ?user_id,
        api_key = ?api_key.as_ref().map(|k| &k[..8]),
        "Rate limit check passed"
    );

    Ok(next.run(req).await)
}

/// Extract client IP from request headers
fn extract_client_ip(req: &Request<Body>) -> Option<IpAddr> {
    // Check X-Forwarded-For header first
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Take the first IP in the chain
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // Check X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            if let Ok(ip) = real_ip_str.parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }

    // TODO: Extract from connection info if available
    None
}

/// Create rate limiting middleware with custom configuration
pub fn create_rate_limit_middleware(
    config: RateLimitConfig,
) -> impl Fn(State<AppState>, Request<Body>, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, AppError>> + Send>> + Clone {
    move |state: State<AppState>, req: Request<Body>, next: Next| {
        let _config = config.clone();
        Box::pin(async move {
            rate_limit_middleware(state, req, next).await
        })
    }
}