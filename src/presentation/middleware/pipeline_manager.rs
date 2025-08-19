use crate::{
    config::AppConfiguration,
    core::networking::security::{AuditAction, AuditEvent, AuditLogger, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::{sync::Arc, time::Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Comprehensive middleware pipeline manager
pub struct MiddlewarePipelineManager {
    config: Arc<AppConfiguration>,
    audit_logger: Option<Arc<dyn AuditLogger>>,
}

impl MiddlewarePipelineManager {
    pub fn new(config: Arc<AppConfiguration>, audit_logger: Option<Arc<dyn AuditLogger>>) -> Self {
        Self {
            config,
            audit_logger,
        }
    }

    /// Main middleware pipeline that orchestrates all middleware in the correct order
    pub async fn execute_pipeline(
        State(state): State<AppState>,
        req: Request,
        next: Next,
    ) -> Result<Response> {
        let start_time = Instant::now();
        let request_id = Uuid::new_v4();
        let method = req.method().clone();
        let uri = req.uri().clone();

        debug!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            "🔄 Starting comprehensive middleware pipeline"
        );

        // Create pipeline manager
        let pipeline_manager = Self::new(Arc::clone(&state.env), state.audit_logger.clone());

        // Execute the pipeline
        let response = pipeline_manager
            .process_request_through_pipeline(State(state), req, next)
            .await?;

        let duration = start_time.elapsed();
        info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = response.status().as_u16(),
            duration_ms = duration.as_millis(),
            "✅ Middleware pipeline completed"
        );

        Ok(response)
    }

    /// Process request through the complete middleware pipeline
    async fn process_request_through_pipeline(
        &self,
        state: State<AppState>,
        mut req: Request,
        next: Next,
    ) -> Result<Response> {
        let request_id = Uuid::new_v4();
        req.headers_mut()
            .insert("x-request-id", request_id.to_string().parse().unwrap());

        // Phase 1: Pre-processing and validation
        self.pre_processing_phase(&mut req).await?;

        // Phase 2: Security and authentication
        let security_context = self.security_phase(&state, &mut req).await?;

        // Phase 3: Authorization and RBAC
        if let Some(ref ctx) = security_context {
            self.authorization_phase(&state, &req, ctx).await?;
        }

        // Phase 4: Rate limiting and throttling
        self.rate_limiting_phase(&state, &req, security_context.as_ref())
            .await?;

        // Phase 5: Request processing
        let mut response = next.run(req).await;

        // Phase 6: Post-processing and response enhancement
        self.post_processing_phase(&mut response, &state.0.env)
            .await?;

        // Phase 7: Audit logging
        self.audit_logging_phase(&state, &response, security_context.as_ref())
            .await?;

        Ok(response)
    }

    /// Phase 1: Pre-processing and validation
    async fn pre_processing_phase(&self, req: &mut Request) -> Result<()> {
        debug!("🔍 Phase 1: Pre-processing and validation");

        // Validate request size
        if let Some(content_length) = req.headers().get("content-length") {
            if let Ok(length_str) = content_length.to_str() {
                if let Ok(length) = length_str.parse::<usize>() {
                    const MAX_REQUEST_SIZE: usize = 100 * 1024 * 1024; // 100MB
                    if length > MAX_REQUEST_SIZE {
                        return Err(AppError::ValidationError(format!(
                            "Request size {} exceeds maximum allowed size of {} bytes",
                            length, MAX_REQUEST_SIZE
                        )));
                    }
                }
            }
        }

        // Validate HTTP method
        let method = req.method();
        if !matches!(
            method,
            &axum::http::Method::GET
                | &axum::http::Method::POST
                | &axum::http::Method::PUT
                | &axum::http::Method::DELETE
                | &axum::http::Method::PATCH
                | &axum::http::Method::OPTIONS
                | &axum::http::Method::HEAD
        ) {
            return Err(AppError::ValidationError(format!(
                "HTTP method {} not allowed",
                method
            )));
        }

        // Validate headers for security threats
        self.validate_security_headers(req.headers())?;

        // Validate URL path
        let path = req.uri().path();
        if path.contains("..") || path.contains("//") || path.contains('\0') {
            return Err(AppError::ValidationError(
                "Invalid characters in URL path".to_string(),
            ));
        }

        debug!("✅ Pre-processing phase completed");
        Ok(())
    }

    /// Phase 2: Security and authentication
    async fn security_phase(
        &self,
        state: &State<AppState>,
        req: &mut Request,
    ) -> Result<Option<SecurityContext>> {
        debug!("🔐 Phase 2: Security and authentication");

        // Skip authentication for public endpoints
        if self.is_public_endpoint(req.uri().path()) {
            debug!("🔓 Skipping authentication for public endpoint");
            return Ok(None);
        }

        // Extract and verify JWT token
        let token = self
            .extract_token(req.headers())
            .ok_or_else(|| AppError::AuthError("Authentication required".to_string()))?;

        // Verify token
        let jwt_manager = crate::core::networking::security::JwtManager::new(
            state.env.security.jwt.secret.clone(),
            state.env.security.jwt.expires_in_seconds,
        );

        let claims = jwt_manager.verify_token(&token).await?;

        // Create security context
        let mut security_context = SecurityContext::from_claims(&claims)?;
        security_context.ip_address = self.extract_client_ip(req.headers());
        security_context.user_agent = self.extract_user_agent(req.headers());

        // Insert security context into request
        req.extensions_mut().insert(security_context.clone());

        debug!(
            user_id = %security_context.user_id,
            roles = ?security_context.roles,
            "✅ Authentication phase completed"
        );

        Ok(Some(security_context))
    }

    /// Phase 3: Authorization and RBAC
    async fn authorization_phase(
        &self,
        state: &State<AppState>,
        req: &Request,
        security_context: &SecurityContext,
    ) -> Result<()> {
        debug!("🛡️ Phase 3: Authorization and RBAC");

        // Check if RBAC is enabled
        if !state.env.security.rbac.enabled {
            debug!("RBAC disabled, skipping authorization");
            return Ok(());
        }

        // Get required permission for this endpoint
        if let Some(required_permission) =
            self.get_required_permission(req.method(), req.uri().path())
        {
            security_context
                .require_permission(&required_permission)
                .map_err(|e| {
                    warn!(
                        user_id = %security_context.user_id,
                        required_permission = ?required_permission,
                        user_permissions = ?security_context.permissions,
                        endpoint = req.uri().path(),
                        "❌ Authorization failed: {}", e
                    );
                    e
                })?;

            debug!(
                user_id = %security_context.user_id,
                permission = ?required_permission,
                "✅ Authorization check passed"
            );
        }

        debug!("✅ Authorization phase completed");
        Ok(())
    }

    /// Phase 4: Rate limiting and throttling
    async fn rate_limiting_phase(
        &self,
        state: &State<AppState>,
        req: &Request,
        security_context: Option<&SecurityContext>,
    ) -> Result<()> {
        debug!("⏱️ Phase 4: Rate limiting and throttling");

        let _client_ip = self
            .extract_client_ip(req.headers())
            .unwrap_or_else(|| "unknown".to_string());
        let _user_id = security_context.map(|ctx| ctx.user_id.to_string());

        // Create rate limiter from app config
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
        let _rate_limiter =
            crate::presentation::middleware::rate_limit::RateLimiter::new(rate_limit_config);

        // Check rate limits (simplified for now)
        if state.env.security.rate_limiting.enabled {
            // TODO: Implement proper rate limiting check
            debug!("Rate limiting enabled but not fully implemented");
        }

        debug!("✅ Rate limiting phase completed");
        Ok(())
    }

    /// Phase 6: Post-processing and response enhancement
    async fn post_processing_phase(
        &self,
        response: &mut Response,
        config: &AppConfiguration,
    ) -> Result<()> {
        debug!("🔧 Phase 6: Post-processing and response enhancement");

        // Add comprehensive security headers
        self.add_security_headers(response, config);

        // Add performance headers
        self.add_performance_headers(response);

        // Add custom headers
        self.add_custom_headers(response);

        debug!("✅ Post-processing phase completed");
        Ok(())
    }

    /// Phase 7: Audit logging
    async fn audit_logging_phase(
        &self,
        _state: &State<AppState>,
        response: &Response,
        security_context: Option<&SecurityContext>,
    ) -> Result<()> {
        debug!("📝 Phase 7: Audit logging");

        if let Some(audit_logger) = &self.audit_logger {
            // Create audit event
            let audit_event = AuditEvent::new(
                AuditAction::ViewPipeline, // This would be determined by the endpoint
                "http_request".to_string(),
                security_context.map(|ctx| ctx.user_id),
                security_context.map(|ctx| ctx.session_id.clone()),
            )
            .with_details(
                "status".to_string(),
                serde_json::Value::Number(response.status().as_u16().into()),
            );

            // Log asynchronously
            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    error!("Failed to log audit event: {}", e);
                }
            });
        }

        debug!("✅ Audit logging phase completed");
        Ok(())
    }

    /// Validate security headers
    fn validate_security_headers(&self, headers: &HeaderMap) -> Result<()> {
        // Check for header injection attacks
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                if value_str.contains('\n') || value_str.contains('\r') {
                    return Err(AppError::ValidationError(format!(
                        "Invalid characters in header {}",
                        name
                    )));
                }
            }
        }

        // Validate Host header
        if let Some(host) = headers.get("host") {
            if let Ok(host_str) = host.to_str() {
                if host_str.contains(' ') || host_str.contains('\t') {
                    return Err(AppError::ValidationError("Invalid Host header".to_string()));
                }
            }
        }

        Ok(())
    }

    /// Extract client IP from headers
    fn extract_client_ip(&self, headers: &HeaderMap) -> Option<String> {
        let ip_headers = [
            "cf-connecting-ip",
            "x-real-ip",
            "x-forwarded-for",
            "x-client-ip",
        ];

        for header_name in &ip_headers {
            if let Some(header_value) = headers.get(*header_name) {
                if let Ok(ip_str) = header_value.to_str() {
                    let ip = ip_str.split(',').next().unwrap_or(ip_str).trim();
                    if !ip.is_empty() && ip != "unknown" {
                        return Some(ip.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extract user agent from headers
    fn extract_user_agent(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Extract JWT token from headers
    fn extract_token(&self, headers: &HeaderMap) -> Option<String> {
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
    fn is_public_endpoint(&self, path: &str) -> bool {
        matches!(
            path,
            "/api/healthchecker"
                | "/api/sessions/oauth/github"
                | "/api/sessions/oauth/github/callback"
                | "/api/sessions/oauth/google"
                | "/api/sessions/oauth/google/callback"
                | "/metrics"
                | "/health"
                | "/favicon.ico"
        )
    }

    /// Get required permission for endpoint
    fn get_required_permission(
        &self,
        method: &axum::http::Method,
        path: &str,
    ) -> Option<crate::core::networking::security::Permission> {
        use crate::core::networking::security::Permission;

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

    /// Add comprehensive security headers
    fn add_security_headers(&self, response: &mut Response, config: &AppConfiguration) {
        let headers = response.headers_mut();

        // Basic security headers
        headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
        headers.insert("X-Frame-Options", "DENY".parse().unwrap());
        headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
        headers.insert(
            "Referrer-Policy",
            "strict-origin-when-cross-origin".parse().unwrap(),
        );
        headers.insert(
            "Permissions-Policy",
            "geolocation=(), microphone=(), camera=()".parse().unwrap(),
        );

        // HSTS (only in production)
        if std::env::var("ENVIRONMENT").unwrap_or_default() == "production" {
            headers.insert(
                "Strict-Transport-Security",
                "max-age=31536000; includeSubDomains; preload"
                    .parse()
                    .unwrap(),
            );
        }

        // Content Security Policy
        let csp = if config.features.enable_experimental_features {
            "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' https:; connect-src 'self' https:; frame-ancestors 'none'"
        } else {
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'"
        };
        headers.insert("Content-Security-Policy", csp.parse().unwrap());
    }

    /// Add performance headers
    fn add_performance_headers(&self, response: &mut Response) {
        let headers = response.headers_mut();

        // Cache control for API responses
        headers.insert(
            "Cache-Control",
            "no-cache, no-store, must-revalidate".parse().unwrap(),
        );
        headers.insert("Pragma", "no-cache".parse().unwrap());
        headers.insert("Expires", "0".parse().unwrap());

        // Response timing
        headers.insert(
            "X-Response-Time",
            chrono::Utc::now().to_rfc3339().parse().unwrap(),
        );
    }

    /// Add custom headers
    fn add_custom_headers(&self, response: &mut Response) {
        let headers = response.headers_mut();

        // Custom application headers
        headers.insert("X-Powered-By", "RustCI".parse().unwrap());
        headers.insert("X-API-Version", "v1".parse().unwrap());
        headers.insert("X-Request-ID", Uuid::new_v4().to_string().parse().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, Method};

    #[test]
    fn test_is_public_endpoint() {
        let config = Arc::new(AppConfiguration::default());
        let manager = MiddlewarePipelineManager::new(config, None);

        assert!(manager.is_public_endpoint("/api/healthchecker"));
        assert!(manager.is_public_endpoint("/api/sessions/oauth/github"));
        assert!(!manager.is_public_endpoint("/api/ci/pipelines"));
    }

    #[test]
    fn test_extract_client_ip() {
        let config = Arc::new(AppConfiguration::default());
        let manager = MiddlewarePipelineManager::new(config, None);

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = manager.extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_get_required_permission() {
        let config = Arc::new(AppConfiguration::default());
        let manager = MiddlewarePipelineManager::new(config, None);

        let perm = manager.get_required_permission(&Method::GET, "/api/ci/pipelines");
        assert!(matches!(
            perm,
            Some(crate::core::networking::security::Permission::ReadPipelines)
        ));

        let perm = manager.get_required_permission(&Method::POST, "/api/ci/pipelines");
        assert!(matches!(
            perm,
            Some(crate::core::networking::security::Permission::WritePipelines)
        ));

        let perm = manager.get_required_permission(&Method::GET, "/api/healthchecker");
        assert!(perm.is_none());
    }
}
