use crate::{
    config::RateLimitConfig,
    core::security::{AuditAction, AuditEvent, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Enhanced rate limiter with sliding window and multiple limit types
pub struct RateLimiter {
    config: RateLimitConfig,
    // IP-based rate limiting
    ip_windows: Arc<tokio::sync::RwLock<HashMap<String, SlidingWindow>>>,
    // User-based rate limiting
    user_windows: Arc<tokio::sync::RwLock<HashMap<String, SlidingWindow>>>,
    // Endpoint-based rate limiting
    endpoint_windows: Arc<tokio::sync::RwLock<HashMap<String, SlidingWindow>>>,
}

/// Sliding window for rate limiting
#[derive(Debug, Clone)]
struct SlidingWindow {
    requests: Vec<Instant>,
    window_size: std::time::Duration,
    max_requests: u32,
}

impl SlidingWindow {
    fn new(window_size: std::time::Duration, max_requests: u32) -> Self {
        Self {
            requests: Vec::new(),
            window_size,
            max_requests,
        }
    }

    fn can_make_request(&mut self) -> bool {
        let now = Instant::now();

        // Remove old requests outside the window
        self.requests
            .retain(|&request_time| now.duration_since(request_time) < self.window_size);

        // Check if we can make a new request
        if self.requests.len() < self.max_requests as usize {
            self.requests.push(now);
            true
        } else {
            false
        }
    }

    fn requests_in_window(&self) -> usize {
        let now = Instant::now();
        self.requests
            .iter()
            .filter(|&&request_time| now.duration_since(request_time) < self.window_size)
            .count()
    }

    fn time_until_next_request(&self) -> Option<std::time::Duration> {
        if self.requests.len() < self.max_requests as usize {
            return None;
        }

        let now = Instant::now();
        self.requests
            .iter()
            .filter_map(|&request_time| {
                let elapsed = now.duration_since(request_time);
                if elapsed < self.window_size {
                    Some(self.window_size - elapsed)
                } else {
                    None
                }
            })
            .min()
    }
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_windows: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            user_windows: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            endpoint_windows: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Check all rate limits for a request
    pub async fn check_rate_limits(
        &self,
        client_ip: &str,
        user_id: Option<&str>,
        endpoint: &str,
    ) -> Result<()> {
        // Check if IP is whitelisted
        if self.config.whitelist_ips.contains(&client_ip.to_string()) {
            debug!(
                client_ip = client_ip,
                "✅ IP whitelisted, skipping rate limits"
            );
            return Ok(());
        }

        // Check IP-based rate limit
        self.check_ip_rate_limit(client_ip).await?;

        // Check user-based rate limit if enabled and user is authenticated
        if self.config.enable_per_user_limits {
            if let Some(uid) = user_id {
                self.check_user_rate_limit(uid).await?;
            }
        }

        // Check endpoint-specific rate limits
        self.check_endpoint_rate_limit(endpoint).await?;

        Ok(())
    }

    async fn check_ip_rate_limit(&self, client_ip: &str) -> Result<()> {
        let mut windows = self.ip_windows.write().await;
        let window = windows.entry(client_ip.to_string()).or_insert_with(|| {
            SlidingWindow::new(
                std::time::Duration::from_secs(60),
                self.config.requests_per_minute,
            )
        });

        if !window.can_make_request() {
            let retry_after = window
                .time_until_next_request()
                .unwrap_or(std::time::Duration::from_secs(60));

            warn!(
                client_ip = client_ip,
                requests_in_window = window.requests_in_window(),
                limit = self.config.requests_per_minute,
                retry_after_secs = retry_after.as_secs(),
                "🚫 IP rate limit exceeded"
            );

            return Err(AppError::RateLimitExceeded {
                limit: self.config.requests_per_minute,
                window: "1 minute".to_string(),
            });
        }

        debug!(
            client_ip = client_ip,
            requests_in_window = window.requests_in_window(),
            limit = self.config.requests_per_minute,
            "✅ IP rate limit check passed"
        );

        Ok(())
    }

    async fn check_user_rate_limit(&self, user_id: &str) -> Result<()> {
        let user_limit = self.config.requests_per_minute * 2; // Users get higher limits
        let mut windows = self.user_windows.write().await;
        let window = windows
            .entry(user_id.to_string())
            .or_insert_with(|| SlidingWindow::new(std::time::Duration::from_secs(60), user_limit));

        if !window.can_make_request() {
            let retry_after = window
                .time_until_next_request()
                .unwrap_or(std::time::Duration::from_secs(60));

            warn!(
                user_id = user_id,
                requests_in_window = window.requests_in_window(),
                limit = user_limit,
                retry_after_secs = retry_after.as_secs(),
                "🚫 User rate limit exceeded"
            );

            return Err(AppError::RateLimitExceeded {
                limit: user_limit,
                window: "1 minute".to_string(),
            });
        }

        debug!(
            user_id = user_id,
            requests_in_window = window.requests_in_window(),
            limit = user_limit,
            "✅ User rate limit check passed"
        );

        Ok(())
    }

    async fn check_endpoint_rate_limit(&self, endpoint: &str) -> Result<()> {
        // Different endpoints have different limits
        let endpoint_limit = self.get_endpoint_limit(endpoint);
        if endpoint_limit == 0 {
            return Ok(()); // No limit for this endpoint
        }

        let mut windows = self.endpoint_windows.write().await;
        let window = windows.entry(endpoint.to_string()).or_insert_with(|| {
            SlidingWindow::new(std::time::Duration::from_secs(60), endpoint_limit)
        });

        if !window.can_make_request() {
            let retry_after = window
                .time_until_next_request()
                .unwrap_or(std::time::Duration::from_secs(60));

            warn!(
                endpoint = endpoint,
                requests_in_window = window.requests_in_window(),
                limit = endpoint_limit,
                retry_after_secs = retry_after.as_secs(),
                "🚫 Endpoint rate limit exceeded"
            );

            return Err(AppError::RateLimitExceeded {
                limit: endpoint_limit,
                window: "1 minute".to_string(),
            });
        }

        debug!(
            endpoint = endpoint,
            requests_in_window = window.requests_in_window(),
            limit = endpoint_limit,
            "✅ Endpoint rate limit check passed"
        );

        Ok(())
    }

    fn get_endpoint_limit(&self, endpoint: &str) -> u32 {
        match endpoint {
            // High-frequency endpoints get higher limits
            path if path.starts_with("/api/healthchecker") => 0, // No limit
            path if path.starts_with("/api/ci/pipelines") && path.contains("/status") => {
                self.config.requests_per_minute * 3
            }

            // Authentication endpoints get moderate limits
            path if path.contains("/oauth") || path.contains("/login") => {
                self.config.requests_per_minute / 2
            }

            // Pipeline execution gets lower limits (resource intensive)
            path if path.contains("/execute") => self.config.requests_per_minute / 4,

            // File upload gets very low limits
            path if path.contains("/upload") => self.config.requests_per_minute / 8,

            // Default limit for other endpoints
            _ => self.config.requests_per_minute,
        }
    }

    /// Clean up old windows periodically
    pub async fn cleanup_old_windows(&self) {
        let cleanup_threshold = std::time::Duration::from_secs(300); // 5 minutes
        let now = Instant::now();

        // Clean IP windows
        {
            let mut windows = self.ip_windows.write().await;
            windows.retain(|_, window| {
                window
                    .requests
                    .iter()
                    .any(|&request_time| now.duration_since(request_time) < cleanup_threshold)
            });
        }

        // Clean user windows
        {
            let mut windows = self.user_windows.write().await;
            windows.retain(|_, window| {
                window
                    .requests
                    .iter()
                    .any(|&request_time| now.duration_since(request_time) < cleanup_threshold)
            });
        }

        // Clean endpoint windows
        {
            let mut windows = self.endpoint_windows.write().await;
            windows.retain(|_, window| {
                window
                    .requests
                    .iter()
                    .any(|&request_time| now.duration_since(request_time) < cleanup_threshold)
            });
        }

        debug!("🧹 Rate limiter cleanup completed");
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response> {
    let start_time = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();

    // Extract client IP
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .or_else(|| req.headers().get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Extract user ID from security context if available
    let user_id = req
        .extensions()
        .get::<SecurityContext>()
        .map(|ctx| ctx.user_id.to_string());

    // Create rate limiter
    let rate_limiter = RateLimiter::new(state.env.security.rate_limiting.clone());

    // Check rate limits
    if let Err(e) = rate_limiter
        .check_rate_limits(&client_ip, user_id.as_deref(), path)
        .await
    {
        // Log rate limit violation
        if let Some(audit_logger) = &state.audit_logger {
            let audit_event = AuditEvent::new(
                AuditAction::Login, // Generic action for rate limit violations
                "rate_limit".to_string(),
                user_id.as_ref().and_then(|id| id.parse::<Uuid>().ok()),
                None,
            )
            .with_client_info(Some(client_ip.clone()), None)
            .with_error(format!("Rate limit exceeded: {}", e))
            .with_details(
                "endpoint".to_string(),
                serde_json::Value::String(path.to_string()),
            )
            .with_details(
                "method".to_string(),
                serde_json::Value::String(method.to_string()),
            );

            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    error!("Failed to log rate limit audit event: {}", e);
                }
            });
        }

        error!(
            client_ip = client_ip,
            user_id = ?user_id,
            method = %method,
            path = path,
            error = %e,
            "🚫 Rate limit exceeded"
        );

        return Err(e);
    }

    let response = next.run(req).await;
    let duration = start_time.elapsed();

    debug!(
        client_ip = client_ip,
        user_id = ?user_id,
        method = %method,
        path = path,
        status = response.status().as_u16(),
        duration_ms = duration.as_millis(),
        "✅ Rate limit check passed"
    );

    Ok(response)
}

/// Background task to clean up old rate limit windows
pub async fn start_rate_limit_cleanup_task(rate_limiter: Arc<RateLimiter>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

    loop {
        interval.tick().await;
        rate_limiter.cleanup_old_windows().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliding_window() {
        let mut window = SlidingWindow::new(std::time::Duration::from_secs(60), 5);

        // Should allow requests under the limit
        assert!(window.can_make_request());
        assert!(window.can_make_request());
        assert!(window.can_make_request());
        assert!(window.can_make_request());
        assert!(window.can_make_request());

        // Should reject the 6th request
        assert!(!window.can_make_request());

        // Check requests in window
        assert_eq!(window.requests_in_window(), 5);
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let config = RateLimitConfig {
            requests_per_minute: 5,
            burst_size: 10,
            enable_per_user_limits: true,
            whitelist_ips: vec!["127.0.0.1".to_string()],
        };

        let limiter = RateLimiter::new(config);

        // Should allow requests under the limit
        assert!(limiter
            .check_rate_limits("192.168.1.1", None, "/api/test")
            .await
            .is_ok());
        assert!(limiter
            .check_rate_limits("192.168.1.1", None, "/api/test")
            .await
            .is_ok());

        // Whitelisted IP should always pass
        assert!(limiter
            .check_rate_limits("127.0.0.1", None, "/api/test")
            .await
            .is_ok());
    }

    #[test]
    fn test_endpoint_limits() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 10,
            enable_per_user_limits: false,
            whitelist_ips: vec![],
        };

        let limiter = RateLimiter::new(config);

        // Health check should have no limit
        assert_eq!(limiter.get_endpoint_limit("/api/healthchecker"), 0);

        // Execute endpoints should have lower limits
        assert_eq!(limiter.get_endpoint_limit("/api/ci/pipelines/execute"), 15);

        // Upload endpoints should have very low limits
        assert_eq!(limiter.get_endpoint_limit("/api/upload"), 7);

        // Default endpoints should have normal limits
        assert_eq!(limiter.get_endpoint_limit("/api/other"), 60);
    }
}
