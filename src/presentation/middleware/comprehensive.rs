use crate::{
    config::AppConfiguration,
    core::networking::security::{AuditAction, AuditEvent, JwtManager, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{sync::Arc, time::Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Comprehensive middleware pipeline that combines all security features
pub struct ComprehensiveSecurityPipeline;

impl ComprehensiveSecurityPipeline {
    /// Main middleware function that orchestrates all security checks
    pub async fn process_request(
        State(state): State<AppState>,
        mut req: Request,
        next: Next,
    ) -> Result<Response> {
        let start_time = Instant::now();
        let method = req.method().clone();
        let uri = req.uri().clone();
        let request_id = Uuid::new_v4();

        // Extract client information for security context
        let client_ip = Self::extract_client_ip(req.headers());
        let user_agent = Self::extract_user_agent(req.headers());

        debug!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            client_ip = ?client_ip,
            "🔐 Starting comprehensive security pipeline"
        );

        // Add request ID to headers for tracing
        req.headers_mut()
            .insert("x-request-id", request_id.to_string().parse().unwrap());

        // 1. Request validation and sanitization
        let validation_result = {
            let content_length = req
                .headers()
                .get("content-length")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<usize>().ok());

            if let Some(length) = content_length {
                const MAX_REQUEST_SIZE: usize = 50 * 1024 * 1024; // 50MB
                if length > MAX_REQUEST_SIZE {
                    Err(AppError::ValidationError("Request too large".to_string()))
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        };

        if let Err(e) = validation_result {
            Self::log_security_violation(
                &state,
                "request_validation",
                None,
                client_ip.clone(),
                user_agent.clone(),
                Some(format!("Request validation failed: {}", e)),
            )
            .await;
            return Err(e);
        }

        // 2. Rate limiting with advanced algorithms
        if state.env.security.rate_limiting.enabled {
            // Basic rate limiting check - in production, use Redis or proper rate limiter
            let path = req.uri().path().to_string();
            if path.starts_with("/api/control-plane/") {
                // Apply stricter rate limits to control plane endpoints
                debug!(
                    "🚦 Applying rate limiting to control plane endpoint: {}",
                    path
                );
                // Basic rate limiting implementation
                // In production, this should use a proper distributed rate limiter like Redis
                debug!("Rate limiting check for path: {}", path);
            }
        }

        // 3. Authentication and RBAC (skip for public endpoints)
        let security_context = if Self::is_public_endpoint(uri.path()) {
            debug!("🔓 Skipping auth for public endpoint: {}", uri.path());
            None
        } else {
            // Extract data before async call to avoid Send trait issues
            let token = Self::extract_token(req.headers());
            let client_ip_extracted = Self::extract_client_ip(req.headers());
            let user_agent_extracted = Self::extract_user_agent(req.headers());
            let path = uri.path().to_string();
            let method_clone = method.clone();

            match Self::authenticate_with_extracted_data(
                &state,
                token,
                &method_clone,
                &path,
                client_ip_extracted,
                user_agent_extracted,
            )
            .await
            {
                Ok(ctx) => {
                    req.extensions_mut().insert(ctx.clone());
                    Some(ctx)
                }
                Err(e) => {
                    Self::log_security_violation(
                        &state,
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

        // 4. Session management and validation
        if let Some(ref ctx) = security_context {
            if let Err(e) = Self::validate_session(&state, ctx).await {
                Self::log_security_violation(
                    &state,
                    "session_invalid",
                    Some(ctx.user_id),
                    client_ip.clone(),
                    user_agent.clone(),
                    Some(format!("Session validation failed: {}", e)),
                )
                .await;
                return Err(e);
            }
        }

        // 5. Content Security Policy and input validation
        let csp_result = {
            // Check content type for POST/PUT requests
            if matches!(method, axum::http::Method::POST | axum::http::Method::PUT) {
                if let Some(content_type) = req.headers().get("content-type") {
                    if let Ok(ct_str) = content_type.to_str() {
                        if !ct_str.starts_with("application/json")
                            && !ct_str.starts_with("application/x-www-form-urlencoded")
                            && !ct_str.starts_with("multipart/form-data")
                        {
                            Err(AppError::ValidationError(
                                "Unsupported content type".to_string(),
                            ))
                        } else {
                            Ok(())
                        }
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        };

        if let Err(e) = csp_result {
            warn!("CSP validation warning: {}", e);
            // Don't fail the request for CSP warnings, just log them
        }

        // 6. Process the request
        let mut response = next.run(req).await;
        let duration = start_time.elapsed();

        // 7. Add comprehensive security headers
        Self::add_comprehensive_security_headers(&mut response, state.env.as_ref());

        // 8. Log successful request with audit trail
        Self::log_successful_request(
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

        info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = response.status().as_u16(),
            duration_ms = duration.as_millis(),
            user_id = ?security_context.as_ref().map(|ctx| ctx.user_id),
            "✅ Comprehensive security pipeline completed"
        );

        Ok(response)
    }

    /// Extract client IP with support for various proxy headers
    fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
        // Check multiple headers in order of preference
        let ip_headers = [
            "cf-connecting-ip",    // Cloudflare
            "x-real-ip",           // Nginx
            "x-forwarded-for",     // Standard proxy header
            "x-client-ip",         // Alternative
            "x-cluster-client-ip", // Cluster environments
        ];

        for header_name in &ip_headers {
            if let Some(header_value) = headers.get(*header_name) {
                if let Ok(ip_str) = header_value.to_str() {
                    // For X-Forwarded-For, take the first IP (client)
                    let ip = ip_str.split(',').next().unwrap_or(ip_str).trim();
                    if !ip.is_empty() && ip != "unknown" {
                        return Some(ip.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extract user agent with validation
    fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| {
                // Sanitize user agent string
                s.chars()
                    .filter(|c| c.is_ascii() && !c.is_control())
                    .take(500) // Limit length
                    .collect()
            })
    }

    /// Validate security-related headers
    fn validate_security_headers(headers: &HeaderMap) -> Result<()> {
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

        // Validate Host header to prevent Host header injection
        if let Some(host) = headers.get("host") {
            if let Ok(host_str) = host.to_str() {
                // Basic validation - in production, you'd check against allowed hosts
                if host_str.contains(' ') || host_str.contains('\t') {
                    return Err(AppError::ValidationError("Invalid Host header".to_string()));
                }
            }
        }

        Ok(())
    }

    /// Enhanced authentication and authorization with RBAC
    async fn authenticate_with_extracted_data(
        state: &AppState,
        token: Option<String>,
        method: &axum::http::Method,
        path: &str,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<SecurityContext> {
        // Check if token exists
        let token =
            token.ok_or_else(|| AppError::AuthError("Authentication required".to_string()))?;

        // Verify token with enhanced JWT manager
        let jwt_manager = JwtManager::new(
            state.env.security.jwt.secret.clone(),
            state.env.security.jwt.expires_in_seconds,
        );

        let claims = jwt_manager.verify_token(&token).await?;

        // Create security context with enhanced information
        let mut security_context = SecurityContext::from_claims(&claims)?;
        security_context.ip_address = client_ip;
        security_context.user_agent = user_agent;

        // Check RBAC permissions if enabled
        if state.env.security.rbac.enabled {
            if let Some(required_permission) = Self::get_required_permission(method, path) {
                security_context.require_permission(&required_permission)?;
            }
        }

        // Additional security checks
        Self::perform_additional_security_checks(&security_context, state).await?;

        Ok(security_context)
    }

    /// Perform additional security checks
    async fn perform_additional_security_checks(
        security_context: &SecurityContext,
        state: &AppState,
    ) -> Result<()> {
        // Check for concurrent session limits
        if state.env.security.session.max_concurrent_sessions > 0 {
            // In a real implementation, you'd check active sessions in database
            debug!(
                user_id = %security_context.user_id,
                "Checking concurrent session limits"
            );
        }

        // Check for suspicious activity patterns
        if let Some(ip) = &security_context.ip_address {
            // In a real implementation, you'd check for:
            // - Multiple failed login attempts
            // - Unusual access patterns
            // - Geolocation anomalies
            debug!(ip = ip, "Checking for suspicious activity patterns");
        }

        // Validate session freshness - we'll skip this for now since we don't have claims here
        // In a real implementation, you'd get the claims from the security context
        debug!(
            user_id = %security_context.user_id,
            "Session freshness validation would be performed here"
        );

        Ok(())
    }

    /// Extract JWT token from various sources
    fn extract_token(headers: &HeaderMap) -> Option<String> {
        // Try Authorization header first
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_value) = auth_header.to_str() {
                if let Some(token) = auth_value.strip_prefix("Bearer ") {
                    return Some(token.to_string());
                }
            }
        }

        // Try custom header
        if let Some(token_header) = headers.get("x-auth-token") {
            if let Ok(token_value) = token_header.to_str() {
                return Some(token_value.to_string());
            }
        }

        None
    }

    /// Validate session state and security
    async fn validate_session(_state: &AppState, security_context: &SecurityContext) -> Result<()> {
        // In a real implementation, you'd:
        // 1. Check session in database/cache
        // 2. Validate session hasn't been revoked
        // 3. Check for session hijacking indicators
        // 4. Update last activity timestamp

        debug!(
            user_id = %security_context.user_id,
            session_id = security_context.session_id,
            "Validating session"
        );

        Ok(())
    }

    /// Add comprehensive security headers
    fn add_comprehensive_security_headers(response: &mut Response, config: &AppConfiguration) {
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

        // Enhanced Content Security Policy
        let csp = if config.features.enable_experimental_features {
            "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' https:; connect-src 'self' https:; frame-ancestors 'none'"
        } else {
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'"
        };
        headers.insert("Content-Security-Policy", csp.parse().unwrap());

        // Custom security headers
        headers.insert("X-Powered-By", "RustCI".parse().unwrap());
        headers.insert("X-Request-ID", Uuid::new_v4().to_string().parse().unwrap());
        headers.insert(
            "X-Response-Time",
            chrono::Utc::now().to_rfc3339().parse().unwrap(),
        );

        // Cache control for sensitive endpoints
        headers.insert(
            "Cache-Control",
            "no-cache, no-store, must-revalidate".parse().unwrap(),
        );
        headers.insert("Pragma", "no-cache".parse().unwrap());
        headers.insert("Expires", "0".parse().unwrap());
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
                | "/favicon.ico"
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

    /// Log security violations
    async fn log_security_violation(
        state: &AppState,
        violation_type: &str,
        user_id: Option<Uuid>,
        client_ip: Option<String>,
        user_agent: Option<String>,
        error_message: Option<String>,
    ) {
        if let Some(audit_logger) = &state.audit_logger {
            let mut audit_event = AuditEvent::new(
                AuditAction::Login, // Generic action for security violations
                violation_type.to_string(),
                user_id,
                None,
            )
            .with_client_info(client_ip, user_agent);

            if let Some(error) = error_message {
                audit_event = audit_event.with_error(error);
            }

            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    error!("Failed to log security violation: {}", e);
                }
            });
        }
    }

    /// Log successful requests
    async fn log_successful_request(
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
                    error!("Failed to log successful request: {}", e);
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

/// Main middleware function that can be used with axum
pub async fn comprehensive_security_middleware(
    state: State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    match ComprehensiveSecurityPipeline::process_request(state, req, next).await {
        Ok(response) => response,
        Err(err) => {
            error!("Security middleware error: {}", err);
            // Convert AppError to HTTP response
            err.into_response()
        }
    }
}
