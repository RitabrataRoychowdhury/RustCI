use crate::{
    core::security::{AuditAction, AuditEvent, Permission, Role, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced middleware pipeline with comprehensive security features
pub struct EnhancedMiddlewarePipeline {
    pub state: AppState,
}

impl EnhancedMiddlewarePipeline {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Comprehensive authentication and authorization middleware
    pub async fn auth_and_authz_middleware(
        State(state): State<AppState>,
        mut req: Request,
        next: Next,
    ) -> Result<Response> {
        let start_time = std::time::Instant::now();
        let method = req.method().clone();
        let uri = req.uri().clone();
        let request_id = Uuid::new_v4();

        // Extract client information for audit logging
        let client_ip = Self::extract_client_ip(req.headers());
        let user_agent = Self::extract_user_agent(req.headers());

        debug!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            client_ip = ?client_ip,
            "🔐 Starting authentication and authorization"
        );

        // Skip auth for health check and public endpoints
        if Self::is_public_endpoint(uri.path()) {
            debug!("🔓 Skipping auth for public endpoint: {}", uri.path());
            return Ok(next.run(req).await);
        }

        // Extract and verify JWT token
        let token = Self::extract_token(req.headers()).ok_or_else(|| {
            error!("❌ No authentication token provided for protected endpoint");
            AppError::AuthError("Authentication required".to_string())
        })?;

        // Verify token and create security context
        let jwt_manager = crate::core::security::JwtManager::new(
            state.env.security.jwt.secret.clone(),
            state.env.security.jwt.expires_in_seconds,
        );

        let claims = jwt_manager.verify_token(&token).await.map_err(|e| {
            error!("❌ Token verification failed: {}", e);

            // Log failed authentication attempt
            if let Some(audit_logger) = &state.audit_logger {
                let audit_event =
                    AuditEvent::new(AuditAction::Login, "authentication".to_string(), None, None)
                        .with_client_info(client_ip.clone(), user_agent.clone())
                        .with_error(format!("Token verification failed: {}", e));

                let audit_logger_clone = Arc::clone(audit_logger);
                tokio::spawn(async move {
                    if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                        warn!("Failed to log audit event: {}", e);
                    }
                });
            }

            AppError::AuthError("Invalid or expired token".to_string())
        })?;

        // Create security context
        let mut security_context = SecurityContext::from_claims(&claims).map_err(|e| {
            error!("❌ Failed to create security context: {}", e);
            AppError::AuthError("Invalid token claims".to_string())
        })?;

        // Add client information to security context
        security_context.ip_address = client_ip.clone();
        security_context.user_agent = user_agent.clone();

        // Check endpoint-specific permissions
        if let Some(required_permission) = Self::get_required_permission(&method, uri.path()) {
            security_context
                .require_permission(&required_permission)
                .map_err(|e| {
                    warn!(
                        user_id = %security_context.user_id,
                        required_permission = ?required_permission,
                        user_permissions = ?security_context.permissions,
                        endpoint = %uri.path(),
                        "❌ Permission denied: {}", e
                    );

                    // Log authorization failure
                    if let Some(audit_logger) = &state.audit_logger {
                        let audit_event = AuditEvent::new(
                            AuditAction::ViewPipeline, // or appropriate action
                            "authorization".to_string(),
                            Some(security_context.user_id),
                            Some(security_context.session_id.clone()),
                        )
                        .with_client_info(client_ip.clone(), user_agent.clone())
                        .with_error(format!("Permission denied: {}", e))
                        .with_details(
                            "endpoint".to_string(),
                            serde_json::Value::String(uri.path().to_string()),
                        );

                        let audit_logger_clone = Arc::clone(audit_logger);
                        tokio::spawn(async move {
                            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                                warn!("Failed to log audit event: {}", e);
                            }
                        });
                    }

                    e
                })?;
        }

        // Insert security context into request extensions
        req.extensions_mut().insert(security_context.clone());

        // Log successful authentication and authorization
        if let Some(audit_logger) = &state.audit_logger {
            let audit_event = AuditEvent::new(
                AuditAction::Login,
                "authentication".to_string(),
                Some(security_context.user_id),
                Some(security_context.session_id.clone()),
            )
            .with_client_info(client_ip, user_agent)
            .with_details(
                "endpoint".to_string(),
                serde_json::Value::String(uri.path().to_string()),
            );

            let audit_logger_clone = Arc::clone(audit_logger);
            tokio::spawn(async move {
                if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                    warn!("Failed to log audit event: {}", e);
                }
            });
        }

        let response = next.run(req).await;
        let duration = start_time.elapsed();

        info!(
            request_id = %request_id,
            user_id = %security_context.user_id,
            session_id = %security_context.session_id,
            method = %method,
            uri = %uri,
            status = response.status().as_u16(),
            duration_ms = duration.as_millis(),
            "✅ Authenticated request completed"
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

    /// Extract JWT token from headers or cookies
    fn extract_token(headers: &HeaderMap) -> Option<String> {
        // Try Authorization header first
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_value) = auth_header.to_str() {
                if let Some(token) = auth_value.strip_prefix("Bearer ") {
                    return Some(token.to_string());
                }
            }
        }

        // Try cookie (would need cookie parsing in real implementation)
        // For now, we'll just use the Authorization header
        None
    }

    /// Check if endpoint is public (doesn't require authentication)
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
            // Pipeline operations
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

            // User management
            ("GET", "/api/users") => Some(Permission::ManageUsers),
            ("POST", "/api/users") => Some(Permission::ManageUsers),
            ("PUT", path) if path.starts_with("/api/users/") => Some(Permission::ManageUsers),
            ("DELETE", path) if path.starts_with("/api/users/") => Some(Permission::ManageUsers),

            // Audit logs
            ("GET", "/api/audit") => Some(Permission::ViewAuditLogs),

            // System management
            ("GET", "/api/system") => Some(Permission::ManageSystem),
            ("POST", "/api/system") => Some(Permission::ManageSystem),

            _ => None, // No specific permission required
        }
    }

    /// Audit logging middleware for all requests
    pub async fn audit_middleware(
        State(state): State<AppState>,
        req: Request,
        next: Next,
    ) -> Result<Response> {
        let start_time = std::time::Instant::now();
        let method = req.method().clone();
        let uri = req.uri().clone();
        let client_ip = Self::extract_client_ip(req.headers());
        let user_agent = Self::extract_user_agent(req.headers());

        // Extract security context if available
        let security_context = req.extensions().get::<SecurityContext>().cloned();

        let response = next.run(req).await;
        let duration = start_time.elapsed();
        let status = response.status();

        // Log the request if audit logging is enabled
        if let Some(audit_logger) = &state.audit_logger {
            let action = Self::method_to_audit_action(&method, uri.path());
            let resource_type = Self::path_to_resource_type(uri.path());

            let mut audit_event = AuditEvent::new(
                action,
                resource_type,
                security_context.as_ref().map(|ctx| ctx.user_id),
                security_context.as_ref().map(|ctx| ctx.session_id.clone()),
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
                    warn!("Failed to log audit event: {}", e);
                }
            });
        }

        debug!(
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            duration_ms = duration.as_millis(),
            user_id = ?security_context.as_ref().map(|ctx| ctx.user_id),
            "📝 Request audited"
        );

        Ok(response)
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
            _ => AuditAction::ViewPipeline, // Default action
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

    /// Role-based access control middleware
    pub fn require_role_middleware(
        required_role: Role,
    ) -> impl Fn(
        Request,
        Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response>> + Send>>
           + Clone {
        move |req: Request, next: Next| {
            let required_role = required_role.clone();
            Box::pin(async move {
                // Extract security context from request extensions
                let security_context =
                    req.extensions().get::<SecurityContext>().ok_or_else(|| {
                        error!("❌ No security context found in request");
                        AppError::AuthError("Authentication required".to_string())
                    })?;

                // Check role
                if !security_context.has_role(&required_role) {
                    warn!(
                        user_id = %security_context.user_id,
                        required_role = ?required_role,
                        user_roles = ?security_context.roles,
                        "❌ Role check failed"
                    );
                    return Err(AppError::AuthError(format!(
                        "Insufficient privileges: {:?} role required",
                        required_role
                    )));
                }

                debug!(
                    user_id = %security_context.user_id,
                    role = ?required_role,
                    "✅ Role check passed"
                );

                Ok(next.run(req).await)
            })
        }
    }

    /// Permission-based access control middleware
    pub fn require_permission_middleware(
        required_permission: Permission,
    ) -> impl Fn(
        Request,
        Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response>> + Send>>
           + Clone {
        move |req: Request, next: Next| {
            let required_permission = required_permission.clone();
            Box::pin(async move {
                // Extract security context from request extensions
                let security_context =
                    req.extensions().get::<SecurityContext>().ok_or_else(|| {
                        error!("❌ No security context found in request");
                        AppError::AuthError("Authentication required".to_string())
                    })?;

                // Check permission
                security_context
                    .require_permission(&required_permission)
                    .map_err(|e| {
                        warn!(
                            user_id = %security_context.user_id,
                            required_permission = ?required_permission,
                            user_permissions = ?security_context.permissions,
                            "❌ Permission denied: {}", e
                        );
                        e
                    })?;

                debug!(
                    user_id = %security_context.user_id,
                    permission = ?required_permission,
                    "✅ Permission check passed"
                );

                Ok(next.run(req).await)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, Method};

    #[test]
    fn test_extract_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = EnhancedMiddlewarePipeline::extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_is_public_endpoint() {
        assert!(EnhancedMiddlewarePipeline::is_public_endpoint(
            "/api/healthchecker"
        ));
        assert!(EnhancedMiddlewarePipeline::is_public_endpoint(
            "/api/sessions/oauth/github"
        ));
        assert!(!EnhancedMiddlewarePipeline::is_public_endpoint(
            "/api/ci/pipelines"
        ));
    }

    #[test]
    fn test_get_required_permission() {
        let perm =
            EnhancedMiddlewarePipeline::get_required_permission(&Method::GET, "/api/ci/pipelines");
        assert_eq!(perm, Some(Permission::ReadPipelines));

        let perm =
            EnhancedMiddlewarePipeline::get_required_permission(&Method::POST, "/api/ci/pipelines");
        assert_eq!(perm, Some(Permission::WritePipelines));

        let perm =
            EnhancedMiddlewarePipeline::get_required_permission(&Method::GET, "/api/healthchecker");
        assert_eq!(perm, None);
    }

    #[test]
    fn test_method_to_audit_action() {
        let action =
            EnhancedMiddlewarePipeline::method_to_audit_action(&Method::POST, "/api/ci/pipelines");
        assert!(matches!(action, AuditAction::CreatePipeline));

        let action = EnhancedMiddlewarePipeline::method_to_audit_action(
            &Method::POST,
            "/api/sessions/oauth/github",
        );
        assert!(matches!(action, AuditAction::Login));
    }
}
