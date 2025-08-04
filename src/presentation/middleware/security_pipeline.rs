use crate::{
    config::{CorsConfig, RateLimitConfig},
    core::networking::security::{AuditAction, AuditEvent, Permission, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Comprehensive security middleware pipeline
pub struct SecurityMiddlewarePipeline {
    pub state: AppState,
}

impl SecurityMiddlewarePipeline {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Main security middleware that combines all security checks
    pub async fn security_middleware(
        State(state): State<AppState>,
        mut req: Request,
        next: Next,
    ) -> Result<Response> {
        let start_time = Instant::now();
        let method = req.method().clone();
        let uri = req.uri().clone();
        let request_id = Uuid::new_v4();

        // Extract client information
        let client_ip = Self::extract_client_ip(req.headers());
        let user_agent = Self::extract_user_agent(req.headers());

        debug!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            client_ip = ?client_ip,
            "🔐 Starting comprehensive security pipeline"
        );

        // 1. Rate limiting check
        if let Err(e) = Self::check_rate_limit(&state, &client_ip, &req).await {
            Self::log_security_event(
                &state,
                AuditAction::Login,
                "rate_limit_exceeded",
                None,
                client_ip.clone(),
                user_agent.clone(),
                Some(format!("Rate limit exceeded: {}", e)),
            )
            .await;
            return Err(e);
        }

        // 2. Request validation
        if let Err(e) = Self::validate_request(&req) {
            Self::log_security_event(
                &state,
                AuditAction::Login,
                "request_validation_failed",
                None,
                client_ip.clone(),
                user_agent.clone(),
                Some(format!("Request validation failed: {}", e)),
            )
            .await;
            return Err(e);
        }

        // 3. Authentication and authorization (skip for public endpoints)
        let security_context = if Self::is_public_endpoint(uri.path()) {
            debug!("🔓 Skipping auth for public endpoint: {}", uri.path());
            None
        } else {
            match Self::authenticate_and_authorize(&state, &req, &method, uri.path()).await {
                Ok(ctx) => {
                    req.extensions_mut().insert(ctx.clone());
                    Some(ctx)
                }
                Err(e) => {
                    Self::log_security_event(
                        &state,
                        AuditAction::Login,
                        "authentication_failed",
                        None,
                        client_ip.clone(),
                        user_agent.clone(),
                        Some(format!("Authentication failed: {}", e)),
                    )
                    .await;
                    return Err(e);
                }
            }
        };

        // 4. Process request
        let response = next.run(req).await;
        let duration = start_time.elapsed();

        // 5. Log successful request
        Self::log_security_event(
            &state,
            Self::method_to_audit_action(&method, uri.path()),
            Self::path_to_resource_type(uri.path()),
            security_context.as_ref().map(|ctx| ctx.user_id),
            client_ip,
            user_agent,
            None,
        )
        .await;

        info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = response.status().as_u16(),
            duration_ms = duration.as_millis(),
            user_id = ?security_context.as_ref().map(|ctx| ctx.user_id),
            "✅ Security pipeline completed successfully"
        );

        Ok(response)
    }

    /// Extract client IP from headers
    fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
        headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|h| h.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
    }

    /// Extract user agent from headers
    fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Check rate limits
    async fn check_rate_limit(
        state: &AppState,
        client_ip: &Option<String>,
        req: &Request,
    ) -> Result<()> {
        let rate_limit_config = crate::presentation::middleware::rate_limit::RateLimitConfig {
            max_requests: state.env.security.rate_limiting.requests_per_minute,
            window_duration: std::time::Duration::from_secs(60),
            enabled: state.env.security.rate_limiting.enabled,
            per_ip_limit: Some(state.env.security.rate_limiting.requests_per_minute),
            per_user_limit: if state.env.security.rate_limiting.enable_per_user_limits {
                Some(state.env.security.rate_limiting.requests_per_minute * 2)
            } else {
                None
            },
            per_api_key_limit: Some(state.env.security.rate_limiting.requests_per_minute * 5),
        };
        let _rate_limiter = crate::presentation::middleware::rate_limit::RateLimiter::new(rate_limit_config);

        let _ip = client_ip.as_deref().unwrap_or("unknown");

        // Extract user ID from existing security context if available
        let _user_id = req
            .extensions()
            .get::<SecurityContext>()
            .map(|ctx| ctx.user_id.to_string());

        // Check rate limits (simplified for now)
        if state.env.security.rate_limiting.enabled {
            // TODO: Implement proper rate limiting check
            debug!("Rate limiting enabled but not fully implemented");
        }
        Ok(())
    }

    /// Validate request format and size
    fn validate_request(req: &Request) -> Result<()> {
        // Check request size
        if let Some(content_length) = req.headers().get("content-length") {
            if let Ok(length_str) = content_length.to_str() {
                if let Ok(length) = length_str.parse::<usize>() {
                    const MAX_REQUEST_SIZE: usize = 50 * 1024 * 1024; // 50MB
                    if length > MAX_REQUEST_SIZE {
                        return Err(AppError::ValidationError(format!(
                            "Request size {} exceeds maximum allowed size of {} bytes",
                            length, MAX_REQUEST_SIZE
                        )));
                    }
                }
            }
        }

        // Validate content type for POST/PUT requests
        let method = req.method();
        if matches!(method, &axum::http::Method::POST | &axum::http::Method::PUT) {
            if let Some(content_type) = req.headers().get("content-type") {
                let content_type_str = content_type.to_str().unwrap_or("");
                if !content_type_str.starts_with("application/json")
                    && !content_type_str.starts_with("multipart/form-data")
                    && !content_type_str.starts_with("application/x-www-form-urlencoded")
                {
                    warn!(
                        content_type = content_type_str,
                        method = %method,
                        "⚠️ Potentially unsupported content type"
                    );
                }
            }
        }

        Ok(())
    }

    /// Authenticate and authorize request
    async fn authenticate_and_authorize(
        state: &AppState,
        req: &Request,
        method: &axum::http::Method,
        path: &str,
    ) -> Result<SecurityContext> {
        // Extract JWT token
        let token = Self::extract_token(req.headers())
            .ok_or_else(|| AppError::AuthError("Authentication required".to_string()))?;

        // Verify token
        let jwt_manager = crate::core::networking::security::JwtManager::new(
            state.env.security.jwt.secret.clone(),
            state.env.security.jwt.expires_in_seconds,
        );

        let claims = jwt_manager.verify_token(&token).await?;

        // Create security context
        let mut security_context = SecurityContext::from_claims(&claims)?;
        security_context.ip_address = Self::extract_client_ip(req.headers());
        security_context.user_agent = Self::extract_user_agent(req.headers());

        // Check endpoint-specific permissions
        if let Some(required_permission) = Self::get_required_permission(method, path) {
            security_context.require_permission(&required_permission)?;
        }

        Ok(security_context)
    }

    /// Extract JWT token from headers
    fn extract_token(headers: &HeaderMap) -> Option<String> {
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_value) = auth_header.to_str() {
                if let Some(token) = auth_value.strip_prefix("Bearer ") {
                    return Some(token.to_string());
                }
            }
        }
        None
    }

    /// Check if endpoint is public
    fn is_public_endpoint(path: &str) -> bool {
        matches!(
            path,
            "/api/healthchecker"
                | "/api/sessions/oauth/github"
                | "/api/sessions/oauth/github/callback"
                | "/api/sessions/oauth/google"
                | "/api/sessions/oauth/google/callback"
        )
    }

    /// Get required permission for endpoint
    fn get_required_permission(method: &axum::http::Method, path: &str) -> Option<Permission> {
        match (method.as_str(), path) {
            ("GET", path) if path.starts_with("/api/ci/pipelines") => {
                Some(Permission::ReadPipelines)
            }
            ("POST", path) if path.starts_with("/api/ci/pipelines") => {
                Some(Permission::WritePipelines)
            }
            ("PUT", path) if path.starts_with("/api/ci/pipelines") => {
                Some(Permission::WritePipelines)
            }
            ("DELETE", path) if path.starts_with("/api/ci/pipelines") => {
                Some(Permission::DeletePipelines)
            }
            ("POST", path) if path.contains("/execute") => Some(Permission::ExecutePipelines),
            ("GET", "/api/users") => Some(Permission::ManageUsers),
            ("POST", "/api/users") => Some(Permission::ManageUsers),
            ("PUT", path) if path.starts_with("/api/users/") => Some(Permission::ManageUsers),
            ("DELETE", path) if path.starts_with("/api/users/") => Some(Permission::ManageUsers),
            ("GET", "/api/audit") => Some(Permission::ViewAuditLogs),
            ("GET", "/api/system") => Some(Permission::ManageSystem),
            ("POST", "/api/system") => Some(Permission::ManageSystem),
            _ => None,
        }
    }

    /// Convert HTTP method and path to audit action
    fn method_to_audit_action(method: &axum::http::Method, path: &str) -> AuditAction {
        match (method.as_str(), path) {
            ("POST", path) if path.contains("/login") || path.contains("/oauth") => {
                AuditAction::Login
            }
            ("POST", path) if path.contains("/logout") => AuditAction::Logout,
            ("POST", path) if path.contains("/pipelines") => AuditAction::CreatePipeline,
            ("PUT", path) if path.contains("/pipelines") => AuditAction::UpdatePipeline,
            ("DELETE", path) if path.contains("/pipelines") => AuditAction::DeletePipeline,
            ("POST", path) if path.contains("/execute") => AuditAction::ExecutePipeline,
            ("GET", path) if path.contains("/pipelines") => AuditAction::ViewPipeline,
            ("POST", path) if path.contains("/users") => AuditAction::CreateUser,
            ("PUT", path) if path.contains("/users") => AuditAction::UpdateUser,
            ("DELETE", path) if path.contains("/users") => AuditAction::DeleteUser,
            ("POST", path) if path.contains("/upload") => AuditAction::FileUpload,
            ("GET", path) if path.contains("/download") => AuditAction::FileDownload,
            _ => AuditAction::ViewPipeline,
        }
    }

    /// Convert path to resource type
    fn path_to_resource_type(path: &str) -> &'static str {
        if path.contains("/pipelines") {
            "pipeline"
        } else if path.contains("/users") {
            "user"
        } else if path.contains("/system") {
            "system"
        } else if path.contains("/sessions") {
            "session"
        } else {
            "unknown"
        }
    }

    /// Log security event
    async fn log_security_event(
        state: &AppState,
        action: AuditAction,
        resource_type: &str,
        user_id: Option<Uuid>,
        client_ip: Option<String>,
        user_agent: Option<String>,
        error_message: Option<String>,
    ) {
        if let Some(audit_logger) = &state.audit_logger {
            let mut audit_event = AuditEvent::new(
                action,
                resource_type.to_string(),
                user_id,
                None, // session_id would come from security context
            )
            .with_client_info(client_ip, user_agent);

            if let Some(error) = error_message {
                audit_event = audit_event.with_error(error);
            }

            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    warn!("Failed to log audit event: {}", e);
                }
            });
        }
    }
}

/// Enhanced rate limiter with per-IP and per-user limits
pub struct RateLimiter {
    config: RateLimitConfig,
    ip_counts: Arc<tokio::sync::RwLock<HashMap<String, (u32, Instant)>>>,
    user_counts: Arc<tokio::sync::RwLock<HashMap<String, (u32, Instant)>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_counts: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            user_counts: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_rate_limit(&self, client_ip: &str, user_id: Option<&str>) -> Result<()> {
        // Check if IP is whitelisted
        if self.config.whitelist_ips.contains(&client_ip.to_string()) {
            return Ok(());
        }

        // Check IP-based rate limit
        self.check_ip_limit(client_ip).await?;

        // Check user-based rate limit if enabled and user is authenticated
        if self.config.enable_per_user_limits {
            if let Some(uid) = user_id {
                self.check_user_limit(uid).await?;
            }
        }

        Ok(())
    }

    async fn check_ip_limit(&self, client_ip: &str) -> Result<()> {
        let mut counts = self.ip_counts.write().await;
        let now = Instant::now();

        match counts.get_mut(client_ip) {
            Some((count, last_reset)) => {
                if now.duration_since(*last_reset).as_secs() >= 60 {
                    *count = 1;
                    *last_reset = now;
                } else {
                    *count += 1;
                    if *count > self.config.requests_per_minute {
                        return Err(AppError::RateLimitExceeded {
                            limit: self.config.requests_per_minute,
                            window: "1 minute".to_string(),
                        });
                    }
                }
            }
            None => {
                counts.insert(client_ip.to_string(), (1, now));
            }
        }

        Ok(())
    }

    async fn check_user_limit(&self, user_id: &str) -> Result<()> {
        let mut counts = self.user_counts.write().await;
        let now = Instant::now();
        let user_limit = self.config.requests_per_minute * 2; // Users get higher limits

        match counts.get_mut(user_id) {
            Some((count, last_reset)) => {
                if now.duration_since(*last_reset).as_secs() >= 60 {
                    *count = 1;
                    *last_reset = now;
                } else {
                    *count += 1;
                    if *count > user_limit {
                        return Err(AppError::RateLimitExceeded {
                            limit: user_limit,
                            window: "1 minute".to_string(),
                        });
                    }
                }
            }
            None => {
                counts.insert(user_id.to_string(), (1, now));
            }
        }

        Ok(())
    }
}

/// Security headers middleware
pub async fn security_headers_middleware(req: Request, next: Next) -> Result<Response> {
    let mut response = next.run(req).await;

    // Add security headers
    let headers = response.headers_mut();

    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert(
        "Strict-Transport-Security",
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    headers.insert(
        "Permissions-Policy",
        "geolocation=(), microphone=(), camera=()".parse().unwrap(),
    );

    debug!("🔒 Security headers added");
    Ok(response)
}

/// Create CORS middleware with configuration
pub fn create_cors_middleware(config: &CorsConfig) -> tower_http::cors::CorsLayer {
    let mut cors = tower_http::cors::CorsLayer::new()
        .allow_credentials(config.allow_credentials)
        .max_age(std::time::Duration::from_secs(
            config.max_age_seconds as u64,
        ));

    // Set allowed methods
    let methods: Vec<axum::http::Method> = config
        .allowed_methods
        .iter()
        .filter_map(|m| m.parse().ok())
        .collect();

    if !methods.is_empty() {
        cors = cors.allow_methods(methods);
    }

    // Set allowed headers
    let headers: Vec<axum::http::HeaderName> = config
        .allowed_headers
        .iter()
        .filter_map(|h| h.parse().ok())
        .collect();

    if !headers.is_empty() {
        cors = cors.allow_headers(headers);
    }

    // Set allowed origins
    if config.allowed_origins.len() == 1 && config.allowed_origins[0] == "*" {
        cors = cors.allow_origin(tower_http::cors::Any);
    } else {
        for origin in &config.allowed_origins {
            if let Ok(header_value) = origin.parse::<axum::http::HeaderValue>() {
                cors = cors.allow_origin(header_value);
            }
        }
    }

    debug!("🌐 CORS middleware configured");
    cors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_minute: 5,
            burst_size: 10,
            enable_per_user_limits: false,
            whitelist_ips: vec!["127.0.0.1".to_string()],
        };

        let limiter = RateLimiter::new(config);

        // Should allow requests under the limit
        assert!(limiter.check_rate_limit("192.168.1.1", None).await.is_ok());
        assert!(limiter.check_rate_limit("192.168.1.1", None).await.is_ok());

        // Whitelisted IP should always pass
        assert!(limiter.check_rate_limit("127.0.0.1", None).await.is_ok());
    }

    #[test]
    fn test_extract_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = SecurityMiddlewarePipeline::extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_is_public_endpoint() {
        assert!(SecurityMiddlewarePipeline::is_public_endpoint(
            "/api/healthchecker"
        ));
        assert!(SecurityMiddlewarePipeline::is_public_endpoint(
            "/api/sessions/oauth/github"
        ));
        assert!(!SecurityMiddlewarePipeline::is_public_endpoint(
            "/api/ci/pipelines"
        ));
    }

    #[test]
    fn test_get_required_permission() {
        let perm = SecurityMiddlewarePipeline::get_required_permission(
            &axum::http::Method::GET,
            "/api/ci/pipelines",
        );
        assert_eq!(perm, Some(Permission::ReadPipelines));

        let perm = SecurityMiddlewarePipeline::get_required_permission(
            &axum::http::Method::POST,
            "/api/ci/pipelines",
        );
        assert_eq!(perm, Some(Permission::WritePipelines));

        let perm = SecurityMiddlewarePipeline::get_required_permission(
            &axum::http::Method::GET,
            "/api/healthchecker",
        );
        assert_eq!(perm, None);
    }
}
