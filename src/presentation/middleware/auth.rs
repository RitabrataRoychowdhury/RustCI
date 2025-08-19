use crate::{
    core::networking::security::{AuditAction, AuditEvent, JwtManager, SecurityContext},
    error::{AppError, Result},
    AppState,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced authentication middleware with RBAC and audit logging
pub async fn enhanced_auth(
    cookie_jar: CookieJar,
    State(data): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response> {
    debug!("🔐 Enhanced authentication middleware triggered");

    // Extract client information for audit logging
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .or_else(|| req.headers().get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // Extract token from cookie or Authorization header
    let token = cookie_jar
        .get("token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            req.headers()
                .get(header::AUTHORIZATION)
                .and_then(|auth_header| auth_header.to_str().ok())
                .and_then(|auth_value| {
                    auth_value
                        .strip_prefix("Bearer ")
                        .map(|stripped| stripped.to_owned())
                })
        });

    let token = token.ok_or_else(|| {
        error!("❌ No authentication token provided");
        AppError::AuthError("You are not logged in, please provide token".to_string())
    })?;

    // Create JWT manager (in a real app, this would be injected via DI)
    let jwt_manager = JwtManager::new(
        data.env.security.jwt.secret.clone(),
        data.env.security.jwt.expires_in_seconds,
    );

    debug!("🔍 Verifying JWT token with enhanced security");
    let claims = jwt_manager.verify_token(&token).await.map_err(|e| {
        error!("❌ Token verification failed: {}", e);

        // Log failed authentication attempt
        if let Some(audit_logger) = data.audit_logger.as_ref() {
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

        AppError::AuthError("Invalid token".to_string())
    })?;

    // Create security context
    let security_context = SecurityContext::from_claims(&claims).map_err(|e| {
        error!("❌ Failed to create security context: {}", e);
        AppError::AuthError("Invalid token claims".to_string())
    })?;

    debug!(
        user_id = %security_context.user_id,
        roles = ?security_context.roles,
        permissions = ?security_context.permissions,
        "✅ Token verified with RBAC context"
    );

    // Insert security context into request extensions
    req.extensions_mut().insert(security_context.clone());

    // Log successful authentication
    if let Some(audit_logger) = data.audit_logger.as_ref() {
        let audit_event = AuditEvent::new(
            AuditAction::Login,
            "authentication".to_string(),
            Some(security_context.user_id),
            Some(security_context.session_id.clone()),
        )
        .with_client_info(client_ip, user_agent);

        let audit_logger_clone = Arc::clone(audit_logger);
        tokio::spawn(async move {
            if let Err(e) = audit_logger_clone.log_event(audit_event).await {
                warn!("Failed to log audit event: {}", e);
            }
        });
    }

    info!(
        user_id = %security_context.user_id,
        session_id = %security_context.session_id,
        "✅ User authenticated successfully with enhanced security"
    );

    Ok(next.run(req).await)
}

/// Legacy authentication middleware (for backward compatibility)
pub async fn auth(
    cookie_jar: CookieJar,
    State(data): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response> {
    debug!("🔐 Legacy authentication middleware triggered");

    let token = cookie_jar
        .get("token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            req.headers()
                .get(header::AUTHORIZATION)
                .and_then(|auth_header| auth_header.to_str().ok())
                .and_then(|auth_value| {
                    auth_value
                        .strip_prefix("Bearer ")
                        .map(|stripped| stripped.to_owned())
                })
        });

    let token = token.ok_or_else(|| {
        error!("❌ No authentication token provided");
        AppError::AuthError("You are not logged in, please provide token".to_string())
    })?;

    // Use legacy token verification
    let claims = crate::token::verify_jwt_token(data.env.security.jwt.secret.clone(), &token)
        .map_err(|e| {
            error!("❌ Token verification failed: {}", e);
            AppError::AuthError("Invalid token".to_string())
        })?;

    let user_id = Uuid::parse_str(&claims.claims.sub).map_err(|e| {
        error!("❌ Invalid user ID in token: {}", e);
        AppError::AuthError("Invalid token".to_string())
    })?;

    debug!("✅ Token verified for user ID: {}", user_id);

    // Insert user_id into request extensions for use in handlers
    req.extensions_mut().insert(user_id);

    info!("✅ User authenticated successfully: {}", user_id);

    Ok(next.run(req).await)
}

/// Permission-based authorization middleware
pub fn require_permission(
    permission: crate::core::networking::security::Permission,
) -> impl Fn(
    Request<Body>,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response>> + Send>>
       + Clone {
    move |req: Request<Body>, next: Next| {
        let permission = permission.clone();
        Box::pin(async move {
            // Extract security context from request extensions
            let security_context = req.extensions().get::<SecurityContext>().ok_or_else(|| {
                error!("❌ No security context found in request");
                AppError::AuthError("Authentication required".to_string())
            })?;

            // Check permission
            security_context
                .require_permission(&permission)
                .map_err(|e| {
                    warn!(
                        user_id = %security_context.user_id,
                        required_permission = ?permission,
                        user_permissions = ?security_context.permissions,
                        "❌ Permission denied: {}", e
                    );
                    e
                })?;

            debug!(
                user_id = %security_context.user_id,
                permission = ?permission,
                "✅ Permission check passed"
            );

            Ok(next.run(req).await)
        })
    }
}

/// Role-based authorization middleware
pub fn require_role(
    role: crate::core::networking::security::Role,
) -> impl Fn(
    Request<Body>,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response>> + Send>>
       + Clone {
    move |req: Request<Body>, next: Next| {
        let role = role.clone();
        Box::pin(async move {
            // Extract security context from request extensions
            let security_context = req.extensions().get::<SecurityContext>().ok_or_else(|| {
                error!("❌ No security context found in request");
                AppError::AuthError("Authentication required".to_string())
            })?;

            // Check role
            if !security_context.has_role(&role) {
                warn!(
                    user_id = %security_context.user_id,
                    required_role = ?role,
                    user_roles = ?security_context.roles,
                    "❌ Role check failed"
                );
                return Err(AppError::AuthError(format!(
                    "Insufficient privileges: {:?} role required",
                    role
                )));
            }

            debug!(
                user_id = %security_context.user_id,
                role = ?role,
                "✅ Role check passed"
            );

            Ok(next.run(req).await)
        })
    }
}

/// Create a middleware that combines authentication and authorization
/// Note: This is a simplified version - in practice, you'd need more complex composition
pub async fn auth_with_permission_middleware(
    _permission: crate::core::networking::security::Permission,
    cookie_jar: CookieJar,
    state: State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response> {
    // First authenticate
    let authenticated_req = enhanced_auth(cookie_jar, state, req, next).await?;

    // Note: In a real implementation, you'd need to properly compose these middlewares
    // This is a simplified version for demonstration
    Ok(authenticated_req)
}
