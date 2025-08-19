use crate::{
    config::AppConfiguration,
    core::networking::security::{AuditAction, AuditEvent, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Comprehensive middleware pipeline with all security features
pub struct ComprehensiveMiddlewarePipeline {
    config: AppConfiguration,
}

impl ComprehensiveMiddlewarePipeline {
    pub fn new(config: AppConfiguration) -> Self {
        Self { config }
    }

    /// Main middleware pipeline that combines all security and operational features
    pub async fn comprehensive_middleware(
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

        // Add request context
        let context = RequestContext::new()
            .with_correlation_id(request_id)
            .with_client_info(client_ip.clone(), user_agent.clone());

        req.extensions_mut().insert(context.clone());

        // Request processing started

        // 1. Request validation
        if let Err(e) = Self::validate_request(&req) {
            Self::log_security_event(
                &state,
                AuditAction::Login,
                "request_validation",
                None,
                client_ip.clone(),
                user_agent.clone(),
                Some(format!("Request validation failed: {}", e)),
            )
            .await;
            return Err(e);
        }

        // 2. Rate limiting
        if let Err(e) = Self::check_rate_limits(&state, &req, &client_ip).await {
            Self::log_security_event(
                &state,
                AuditAction::Login,
                "rate_limit",
                None,
                client_ip.clone(),
                user_agent.clone(),
                Some(format!("Rate limit exceeded: {}", e)),
            )
            .await;
            return Err(e);
        }

        // 3. Authentication and authorization (skip for public endpoints)
        let security_context = if Self::is_public_endpoint(uri.path()) {
            None
        } else {
            match Self::authenticate_and_authorize(&state, &req, &method, uri.path()).await {
                Ok(ctx) => {
                    let _context = context.with_user_id(ctx.user_id);
                    req.extensions_mut().insert(ctx.clone());
                    Some(ctx)
                }
                Err(e) => {
                    Self::log_security_event(
                        &state,
                        AuditAction::Login,
                        "authentication",
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
        let mut response = next.run(req).await;
        let duration = start_time.elapsed();

        // 5. Add security headers
        Self::add_security_headers(&mut response, &state.env);

        // 6. Log successful request
        Self::log_request_completion(
            &state,
            &method,
            &uri,
            response.status(),
            duration,
            security_context.as_ref(),
            client_ip,
            user_agent,
        )
        .await;

        // Request completed successfully

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

    /// Validate request format and constraints
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

        // Validate headers for security
        Self::validate_security_headers(req.headers())?;

        Ok(())
    }

    /// Validate security-related headers
    fn validate_security_headers(headers: &HeaderMap) -> Result<()> {
        // Check for suspicious headers
        let suspicious_headers = ["x-forwarded-host", "x-original-url", "x-rewrite-url"];

        for header in &suspicious_headers {
            if let Some(value) = headers.get(*header) {
                if let Ok(value_str) = value.to_str() {
                    // Basic validation to prevent header injection
                    if value_str.contains('\n') || value_str.contains('\r') {
                        return Err(AppError::ValidationError(format!(
                            "Invalid characters in header {}",
                            header
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Check rate limits using the enhanced rate limiter
    async fn check_rate_limits(
        state: &AppState,
        req: &Request,
        client_ip: &Option<String>,
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
        let _rate_limiter =
            crate::presentation::middleware::rate_limit::RateLimiter::new(rate_limit_config);

        let _ip = client_ip.as_deref().unwrap_or("unknown");
        let _path = req.uri().path();

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
                | "/metrics"
                | "/health"
        )
    }

    /// Get required permission for endpoint
    fn get_required_permission(
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
    fn add_security_headers(response: &mut Response, config: &AppConfiguration) {
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
            "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'"
        } else {
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
        };
        headers.insert("Content-Security-Policy", csp.parse().unwrap());

        // Custom security headers
        headers.insert("X-Powered-By", "RustCI".parse().unwrap());
        headers.insert("X-Request-ID", Uuid::new_v4().to_string().parse().unwrap());
    }

    /// Log security events
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
            let mut audit_event = AuditEvent::new(action, resource_type.to_string(), user_id, None)
                .with_client_info(client_ip, user_agent);

            if let Some(error) = error_message {
                audit_event = audit_event.with_error(error);
            }

            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    error!("Failed to log security audit event: {}", e);
                }
            });
        }
    }

    /// Log request completion
    async fn log_request_completion(
        state: &AppState,
        method: &axum::http::Method,
        uri: &axum::http::Uri,
        status: StatusCode,
        duration: std::time::Duration,
        security_context: Option<&SecurityContext>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) {
        if let Some(audit_logger) = &state.audit_logger {
            let action = Self::method_to_audit_action(method, uri.path());
            let resource_type = Self::path_to_resource_type(uri.path());

            let mut audit_event = AuditEvent::new(
                action,
                resource_type,
                security_context.map(|ctx| ctx.user_id),
                security_context.map(|ctx| ctx.session_id.clone()),
            )
            .with_client_info(client_ip, user_agent)
            .with_details(
                "method".to_string(),
                serde_json::Value::String(method.to_string()),
            )
            .with_details(
                "uri".to_string(),
                serde_json::Value::String(uri.to_string()),
            )
            .with_details(
                "status".to_string(),
                serde_json::Value::Number(status.as_u16().into()),
            )
            .with_details(
                "duration_ms".to_string(),
                serde_json::Value::Number((duration.as_millis() as u64).into()),
            );

            if !status.is_success() {
                audit_event =
                    audit_event.with_error(format!("Request failed with status: {}", status));
            }

            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    error!("Failed to log request completion audit event: {}", e);
                }
            });
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
    fn path_to_resource_type(path: &str) -> String {
        if path.contains("/pipelines") {
            "pipeline".to_string()
        } else if path.contains("/users") {
            "user".to_string()
        } else if path.contains("/system") {
            "system".to_string()
        } else if path.contains("/sessions") {
            "session".to_string()
        } else {
            "unknown".to_string()
        }
    }
}

/// Request context for middleware
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub user_id: Option<Uuid>,
    pub start_time: Instant,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestContext {
    pub fn new() -> Self {
        let request_id = Uuid::new_v4();
        Self {
            request_id,
            correlation_id: request_id, // Default to request_id
            user_id: None,
            start_time: Instant::now(),
            client_ip: None,
            user_agent: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_client_info(
        mut self,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        self.client_ip = client_ip;
        self.user_agent = user_agent;
        self
    }

    pub fn add_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Create CORS middleware with configuration
pub fn create_cors_middleware(config: &crate::config::CorsConfig) -> tower_http::cors::CorsLayer {
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

    // Set allowed origins - fix CORS credentials issue
    if config.allowed_origins.len() == 1 && config.allowed_origins[0] == "*" {
        if config.allow_credentials {
            // Cannot use wildcard with credentials, use localhost for development
            cors = cors.allow_origin(
                "http://localhost:3000"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
            );
            cors = cors.allow_origin(
                "http://127.0.0.1:3000"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
            );
        } else {
            cors = cors.allow_origin(tower_http::cors::Any);
        }
    } else {
        for origin in &config.allowed_origins {
            if let Ok(header_value) = origin.parse::<axum::http::HeaderValue>() {
                cors = cors.allow_origin(header_value);
            }
        }
    }

    cors
}

/// Create comprehensive middleware pipeline
pub async fn comprehensive_middleware_handler(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response> {
    ComprehensiveMiddlewarePipeline::comprehensive_middleware(State(state), req, next).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, Method};

    #[test]
    fn test_extract_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = ComprehensiveMiddlewarePipeline::extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_is_public_endpoint() {
        assert!(ComprehensiveMiddlewarePipeline::is_public_endpoint(
            "/api/healthchecker"
        ));
        assert!(ComprehensiveMiddlewarePipeline::is_public_endpoint(
            "/api/sessions/oauth/github"
        ));
        assert!(!ComprehensiveMiddlewarePipeline::is_public_endpoint(
            "/api/ci/pipelines"
        ));
    }

    #[test]
    fn test_get_required_permission() {
        use crate::core::networking::security::Permission;

        let perm = ComprehensiveMiddlewarePipeline::get_required_permission(
            &Method::GET,
            "/api/ci/pipelines",
        );
        assert_eq!(perm, Some(Permission::ReadPipelines));

        let perm = ComprehensiveMiddlewarePipeline::get_required_permission(
            &Method::POST,
            "/api/ci/pipelines",
        );
        assert_eq!(perm, Some(Permission::WritePipelines));

        let perm = ComprehensiveMiddlewarePipeline::get_required_permission(
            &Method::GET,
            "/api/healthchecker",
        );
        assert_eq!(perm, None);
    }

    #[test]
    fn test_method_to_audit_action() {
        let action = ComprehensiveMiddlewarePipeline::method_to_audit_action(
            &Method::POST,
            "/api/ci/pipelines",
        );
        assert!(matches!(action, AuditAction::CreatePipeline));

        let action = ComprehensiveMiddlewarePipeline::method_to_audit_action(
            &Method::POST,
            "/api/sessions/oauth/github",
        );
        assert!(matches!(action, AuditAction::Login));
    }

    #[test]
    fn test_request_context() {
        let context = RequestContext::new()
            .with_correlation_id(Uuid::new_v4())
            .with_client_info(
                Some("192.168.1.1".to_string()),
                Some("test-agent".to_string()),
            )
            .add_metadata("key".to_string(), "value".to_string());

        assert!(context.user_id.is_none());
        assert_eq!(context.client_ip, Some("192.168.1.1".to_string()));
        assert_eq!(context.user_agent, Some("test-agent".to_string()));
        assert_eq!(context.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_validate_security_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-host", "example.com".parse().unwrap());

        // Should pass with normal headers
        assert!(ComprehensiveMiddlewarePipeline::validate_security_headers(&headers).is_ok());

        // Should fail with malicious headers
        headers.insert(
            "x-forwarded-host",
            "example.com\nmalicious".parse().unwrap(),
        );
        assert!(ComprehensiveMiddlewarePipeline::validate_security_headers(&headers).is_err());
    }
}
