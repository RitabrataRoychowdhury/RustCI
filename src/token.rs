use crate::error::AppError;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}

pub fn generate_jwt_token(
    user_id: Uuid,
    jwt_secret: String,
    expires_in: String,
) -> Result<String, AppError> {
    if jwt_secret.is_empty() {
        return Err(AppError::InternalServerError(
            "JWT secret cannot be empty".to_string(),
        ));
    }

    let now = Utc::now();
    let iat = now.timestamp().max(0) as usize;

    let exp = match expires_in.chars().last() {
        Some('m') => {
            let minutes = expires_in[..expires_in.len() - 1]
                .parse::<i64>()
                .map_err(|_| {
                    AppError::InternalServerError("Invalid JWT expiration format".to_string())
                })?;
            (now + Duration::minutes(minutes)).timestamp() as usize
        }
        Some('h') => {
            let hours = expires_in[..expires_in.len() - 1]
                .parse::<i64>()
                .map_err(|_| {
                    AppError::InternalServerError("Invalid JWT expiration format".to_string())
                })?;
            (now + Duration::hours(hours)).timestamp() as usize
        }
        Some('d') => {
            let days = expires_in[..expires_in.len() - 1]
                .parse::<i64>()
                .map_err(|_| {
                    AppError::InternalServerError("Invalid JWT expiration format".to_string())
                })?;
            (now + Duration::days(days)).timestamp() as usize
        }
        _ => {
            // If no suffix, treat as seconds (for backwards compatibility)
            let seconds = expires_in.parse::<i64>().map_err(|_| {
                AppError::InternalServerError("Invalid JWT expiration format".to_string())
            })?;
            (now + Duration::seconds(seconds)).timestamp() as usize
        }
    };

    let claims = TokenClaims {
        sub: user_id.to_string(),
        exp,
        iat,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    )
    .map_err(|e| AppError::InternalServerError(format!("Failed to generate JWT token: {}", e)))
}

pub fn verify_jwt_token(
    jwt_secret: String,
    token: &str,
) -> Result<TokenData<TokenClaims>, AppError> {
    let validation = Validation::default();

    decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    )
    .map_err(|e| AppError::AuthError(format!("Invalid token: {}", e)))
}
