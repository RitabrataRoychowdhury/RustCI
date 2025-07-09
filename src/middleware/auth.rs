use crate::{
    error::{AppError, Result},
    token::verify_jwt_token,
    AppState,
};
use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use uuid::Uuid;

pub async fn auth<B>(
    cookie_jar: CookieJar,
    State(data): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response> {
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
        AppError::AuthError("You are not logged in, please provide token".to_string())
    })?;

    let claims = verify_jwt_token(data.env.jwt_secret.clone(), &token)
        .map_err(|_| AppError::AuthError("Invalid token".to_string()))?;

    let user_id = Uuid::parse_str(&claims.claims.sub)
        .map_err(|_| AppError::AuthError("Invalid token".to_string()))?;

    // In a real application, you would fetch the user from the database here
    // For now, we'll just pass the user_id in the request extensions
    req.extensions_mut().insert(user_id);
    Ok(next.run(req).await)
}