use crate::{
    error::{AppError, Result},
    token::verify_jwt_token,
    AppState,
};
use axum::{
    body::Body, // ✅ Added: Needed for `Request<Body>`
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{info, error, debug};
use uuid::Uuid;

pub async fn auth(
    cookie_jar: CookieJar,
    State(data): State<AppState>,
    mut req: Request<Body>, // ✅ CHANGED: Specify Body to match `Next`
    next: Next,             // ✅ Already fixed in previous step
) -> Result<Response> {
    debug!("🔐 Authentication middleware triggered");
    
    let token = cookie_jar
        .get("token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            req.headers()
                .get(header::AUTHORIZATION)
                .and_then(|auth_header| auth_header.to_str().ok())
                .and_then(|auth_value| {
                    if auth_value.starts_with("Bearer ") {
                        Some(auth_value[7..].to_owned())
                    } else {
                        None
                    }
                })
        });

    let token = token.ok_or_else(|| {
        error!("❌ No authentication token provided");
        AppError::AuthError("You are not logged in, please provide token".to_string())
    })?;

    debug!("🔍 Verifying JWT token");
    let claims = verify_jwt_token(data.env.jwt_secret.clone(), &token)
        .map_err(|e| {
            error!("❌ Token verification failed: {}", e);
            AppError::AuthError("Invalid token".to_string())
        })?;

    let user_id = Uuid::parse_str(&claims.claims.sub)
        .map_err(|e| {
            error!("❌ Invalid user ID in token: {}", e);
            AppError::AuthError("Invalid token".to_string())
        })?;

    debug!("✅ Token verified for user ID: {}", user_id);
    
    // Insert user_id into request extensions for use in handlers
    req.extensions_mut().insert(user_id);
    
    info!("✅ User authenticated successfully: {}", user_id);
    
    Ok(next.run(req).await) // ✅ Will now compile correctly
}
