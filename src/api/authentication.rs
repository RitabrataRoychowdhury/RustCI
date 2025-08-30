//! API Authentication and Token Management System
//!
//! This module provides robust API authentication with JWT and refresh tokens,
//! token validation, renewal, and revocation capabilities, plus API key management
//! for service-to-service authentication.

use crate::api::errors::{ApiError, ApiErrorBuilder, ApiErrorCode, ApiErrorResponse, create_error_response};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

/// JWT authentication configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// JWT signing secret
    pub secret: String,
    /// Access token expiration time
    pub access_token_expiry: Duration,
    /// Refresh token expiration time
    pub refresh_token_expiry: Duration,
    /// JWT issuer
    pub issuer: String,
    /// JWT audience
    pub audience: String,
    /// Algorithm for signing
    pub algorithm: Algorithm,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "your-secret-key".to_string(), // Should be from environment
            access_token_expiry: Duration::hours(1),
            refresh_token_expiry: Duration::days(30),
            issuer: "rustci-api".to_string(),
            audience: "rustci-users".to_string(),
            algorithm: Algorithm::HS256,
        }
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user ID)
    pub sub: String,
    /// Issued at
    pub iat: i64,
    /// Expiration time
    pub exp: i64,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// JWT ID
    pub jti: String,
    /// Token type (access or refresh)
    pub token_type: TokenType,
    /// User roles
    pub roles: Vec<String>,
    /// User permissions
    pub permissions: Vec<String>,
    /// Session ID
    pub session_id: String,
}

/// Token type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TokenType {
    #[serde(rename = "access")]
    Access,
    #[serde(rename = "refresh")]
    Refresh,
}

/// Authentication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Access token
    pub access_token: String,
    /// Refresh token
    pub refresh_token: String,
    /// Token type (always "Bearer")
    pub token_type: String,
    /// Expires in seconds
    pub expires_in: i64,
    /// User information
    pub user: UserInfo,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID
    pub user_id: String,
    /// Email address
    pub email: String,
    /// Display name
    pub display_name: String,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// User roles
    pub roles: Vec<String>,
    /// User permissions
    pub permissions: Vec<String>,
}

/// Token validation result
#[derive(Debug, Clone)]
pub struct TokenValidation {
    /// Whether token is valid
    pub valid: bool,
    /// Claims if valid
    pub claims: Option<JwtClaims>,
    /// Validation error if invalid
    pub error: Option<String>,
}

/// API key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// API key ID
    pub key_id: String,
    /// API key value (hashed)
    pub key_hash: String,
    /// Key name/description
    pub name: String,
    /// Associated user/service ID
    pub owner_id: String,
    /// Permissions granted to this key
    pub permissions: Vec<String>,
    /// Key scopes (endpoints/resources accessible)
    pub scopes: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether key is active
    pub active: bool,
    /// Last used timestamp
    pub last_used_at: Option<DateTime<Utc>>,
    /// Usage count
    pub usage_count: u64,
}

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    /// Session ID
    pub session_id: String,
    /// User ID
    pub user_id: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
    /// Whether session is active
    pub active: bool,
    /// Session metadata
    pub metadata: HashMap<String, String>,
}

/// JWT token manager
pub struct JwtManager {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    revoked_tokens: Arc<RwLock<HashSet<String>>>,
    active_sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl JwtManager {
    /// Create new JWT manager
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.secret.as_ref());
        
        let mut validation = Validation::new(config.algorithm);
        validation.set_issuer(&[&config.issuer]);
        validation.set_audience(&[&config.audience]);
        
        Self {
            config,
            encoding_key,
            decoding_key,
            validation,
            revoked_tokens: Arc::new(RwLock::new(HashSet::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate access token
    pub async fn generate_access_token(&self, user: &UserInfo, session_id: String) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + self.config.access_token_expiry;
        
        let claims = JwtClaims {
            sub: user.user_id.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            jti: Uuid::new_v4().to_string(),
            token_type: TokenType::Access,
            roles: user.roles.clone(),
            permissions: user.permissions.clone(),
            session_id,
        };

        encode(&Header::new(self.config.algorithm), &claims, &self.encoding_key)
            .map_err(|e| AuthError::TokenGeneration(e.to_string()))
    }

    /// Generate refresh token
    pub async fn generate_refresh_token(&self, user: &UserInfo, session_id: String) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + self.config.refresh_token_expiry;
        
        let claims = JwtClaims {
            sub: user.user_id.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            jti: Uuid::new_v4().to_string(),
            token_type: TokenType::Refresh,
            roles: user.roles.clone(),
            permissions: user.permissions.clone(),
            session_id,
        };

        encode(&Header::new(self.config.algorithm), &claims, &self.encoding_key)
            .map_err(|e| AuthError::TokenGeneration(e.to_string()))
    }

    /// Validate token
    pub async fn validate_token(&self, token: &str) -> TokenValidation {
        // Check if token is revoked
        let revoked_tokens = self.revoked_tokens.read().await;
        if revoked_tokens.contains(token) {
            return TokenValidation {
                valid: false,
                claims: None,
                error: Some("Token has been revoked".to_string()),
            };
        }
        drop(revoked_tokens);

        // Decode and validate token
        match decode::<JwtClaims>(token, &self.decoding_key, &self.validation) {
            Ok(token_data) => {
                let claims = token_data.claims;
                
                // Check if session is still active
                let sessions = self.active_sessions.read().await;
                if let Some(session) = sessions.get(&claims.session_id) {
                    if !session.active || Utc::now() > session.expires_at {
                        return TokenValidation {
                            valid: false,
                            claims: None,
                            error: Some("Session has expired or is inactive".to_string()),
                        };
                    }
                } else {
                    return TokenValidation {
                        valid: false,
                        claims: None,
                        error: Some("Session not found".to_string()),
                    };
                }

                TokenValidation {
                    valid: true,
                    claims: Some(claims),
                    error: None,
                }
            }
            Err(e) => TokenValidation {
                valid: false,
                claims: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// Refresh access token using refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<AuthResponse, AuthError> {
        let validation = self.validate_token(refresh_token).await;
        
        if !validation.valid {
            return Err(AuthError::InvalidToken(
                validation.error.unwrap_or_else(|| "Invalid refresh token".to_string())
            ));
        }

        let claims = validation.claims.unwrap();
        
        if claims.token_type != TokenType::Refresh {
            return Err(AuthError::InvalidToken("Not a refresh token".to_string()));
        }

        // Get user info (in real implementation, this would come from database)
        let user = UserInfo {
            user_id: claims.sub.clone(),
            email: "user@example.com".to_string(), // Would be fetched from DB
            display_name: "User".to_string(),
            avatar_url: None,
            roles: claims.roles.clone(),
            permissions: claims.permissions.clone(),
        };

        // Generate new tokens
        let new_access_token = self.generate_access_token(&user, claims.session_id.clone()).await?;
        let new_refresh_token = self.generate_refresh_token(&user, claims.session_id).await?;

        Ok(AuthResponse {
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_expiry.num_seconds(),
            user,
        })
    }

    /// Revoke token
    pub async fn revoke_token(&self, token: &str) -> Result<(), AuthError> {
        let mut revoked_tokens = self.revoked_tokens.write().await;
        revoked_tokens.insert(token.to_string());
        Ok(())
    }

    /// Create session
    pub async fn create_session(&self, user_id: String, metadata: HashMap<String, String>) -> String {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        let session = Session {
            session_id: session_id.clone(),
            user_id,
            created_at: now,
            last_activity: now,
            expires_at: now + self.config.refresh_token_expiry,
            active: true,
            metadata,
        };

        let mut sessions = self.active_sessions.write().await;
        sessions.insert(session_id.clone(), session);
        
        session_id
    }

    /// Update session activity
    pub async fn update_session_activity(&self, session_id: &str) -> Result<(), AuthError> {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_activity = Utc::now();
            Ok(())
        } else {
            Err(AuthError::SessionNotFound)
        }
    }

    /// Revoke session
    pub async fn revoke_session(&self, session_id: &str) -> Result<(), AuthError> {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.active = false;
            Ok(())
        } else {
            Err(AuthError::SessionNotFound)
        }
    }

    /// Clean up expired sessions and tokens
    pub async fn cleanup_expired(&self) {
        let now = Utc::now();
        
        // Clean up expired sessions
        let mut sessions = self.active_sessions.write().await;
        sessions.retain(|_, session| session.expires_at > now);
        
        // In a real implementation, you'd also clean up revoked tokens periodically
        // based on their expiration times
    }
}

/// API key manager
#[derive(Debug)]
pub struct ApiKeyManager {
    api_keys: Arc<RwLock<HashMap<String, ApiKey>>>,
}

impl ApiKeyManager {
    /// Create new API key manager
    pub fn new() -> Self {
        Self {
            api_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate new API key
    pub async fn generate_api_key(
        &self,
        name: String,
        owner_id: String,
        permissions: Vec<String>,
        scopes: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(String, ApiKey), AuthError> {
        let key_id = Uuid::new_v4().to_string();
        let raw_key = format!("rustci_{}", Uuid::new_v4().to_string().replace('-', ""));
        let key_hash = self.hash_api_key(&raw_key);

        let api_key = ApiKey {
            key_id: key_id.clone(),
            key_hash,
            name,
            owner_id,
            permissions,
            scopes,
            created_at: Utc::now(),
            expires_at,
            active: true,
            last_used_at: None,
            usage_count: 0,
        };

        let mut keys = self.api_keys.write().await;
        keys.insert(key_id.clone(), api_key.clone());

        Ok((raw_key, api_key))
    }

    /// Validate API key
    pub async fn validate_api_key(&self, raw_key: &str) -> Result<ApiKey, AuthError> {
        let key_hash = self.hash_api_key(raw_key);
        let mut keys = self.api_keys.write().await;

        for api_key in keys.values_mut() {
            if api_key.key_hash == key_hash && api_key.active {
                // Check expiration
                if let Some(expires_at) = api_key.expires_at {
                    if Utc::now() > expires_at {
                        return Err(AuthError::ApiKeyExpired);
                    }
                }

                // Update usage statistics
                api_key.last_used_at = Some(Utc::now());
                api_key.usage_count += 1;

                return Ok(api_key.clone());
            }
        }

        Err(AuthError::InvalidApiKey)
    }

    /// Revoke API key
    pub async fn revoke_api_key(&self, key_id: &str) -> Result<(), AuthError> {
        let mut keys = self.api_keys.write().await;
        if let Some(api_key) = keys.get_mut(key_id) {
            api_key.active = false;
            Ok(())
        } else {
            Err(AuthError::ApiKeyNotFound)
        }
    }

    /// List API keys for owner
    pub async fn list_api_keys(&self, owner_id: &str) -> Vec<ApiKey> {
        let keys = self.api_keys.read().await;
        keys.values()
            .filter(|key| key.owner_id == owner_id)
            .cloned()
            .collect()
    }

    /// Hash API key for storage
    fn hash_api_key(&self, raw_key: &str) -> String {
        // In production, use a proper hashing algorithm like bcrypt or Argon2
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        raw_key.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

impl Default for ApiKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Authentication service combining JWT and API key management
pub struct AuthService {
    jwt_manager: JwtManager,
    api_key_manager: ApiKeyManager,
}

impl AuthService {
    /// Create new authentication service
    pub fn new(jwt_config: JwtConfig) -> Self {
        Self {
            jwt_manager: JwtManager::new(jwt_config),
            api_key_manager: ApiKeyManager::new(),
        }
    }

    /// Authenticate user and generate tokens
    pub async fn authenticate_user(&self, user: UserInfo, metadata: HashMap<String, String>) -> Result<AuthResponse, AuthError> {
        let session_id = self.jwt_manager.create_session(user.user_id.clone(), metadata).await;
        
        let access_token = self.jwt_manager.generate_access_token(&user, session_id.clone()).await?;
        let refresh_token = self.jwt_manager.generate_refresh_token(&user, session_id).await?;

        Ok(AuthResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_manager.config.access_token_expiry.num_seconds(),
            user,
        })
    }

    /// Validate JWT token
    pub async fn validate_jwt(&self, token: &str) -> TokenValidation {
        self.jwt_manager.validate_token(token).await
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> Result<ApiKey, AuthError> {
        self.api_key_manager.validate_api_key(api_key).await
    }

    /// Refresh tokens
    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<AuthResponse, AuthError> {
        self.jwt_manager.refresh_token(refresh_token).await
    }

    /// Logout user (revoke session)
    pub async fn logout(&self, session_id: &str) -> Result<(), AuthError> {
        self.jwt_manager.revoke_session(session_id).await
    }

    /// Generate API key
    pub async fn generate_api_key(
        &self,
        name: String,
        owner_id: String,
        permissions: Vec<String>,
        scopes: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(String, ApiKey), AuthError> {
        self.api_key_manager.generate_api_key(name, owner_id, permissions, scopes, expires_at).await
    }

    /// Cleanup expired sessions and tokens
    pub async fn cleanup_expired(&self) {
        self.jwt_manager.cleanup_expired().await;
    }
}

/// Authentication errors
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Token generation failed: {0}")]
    TokenGeneration(String),
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    #[error("Session not found")]
    SessionNotFound,
    #[error("Invalid API key")]
    InvalidApiKey,
    #[error("API key expired")]
    ApiKeyExpired,
    #[error("API key not found")]
    ApiKeyNotFound,
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
}

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiErrorResponse> {
    // Extract authorization header
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    // Extract API key header
    let api_key_header = req
        .headers()
        .get("x-api-key")
        .and_then(|h| h.to_str().ok());

    let request_id = req.extensions().get::<Uuid>().copied();

    // Try JWT authentication first
    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            let validation = auth_service.validate_jwt(token).await;
            
            if validation.valid {
                let claims = validation.claims.unwrap();
                
                // Update session activity
                if let Err(e) = auth_service.jwt_manager.update_session_activity(&claims.session_id).await {
                    warn!("Failed to update session activity: {}", e);
                }
                
                // Add user info to request extensions
                req.extensions_mut().insert(claims.sub.clone());
                req.extensions_mut().insert(claims.roles.clone());
                req.extensions_mut().insert(claims.permissions.clone());
                req.extensions_mut().insert(claims.session_id.clone());
                
                debug!(
                    user_id = %claims.sub,
                    session_id = %claims.session_id,
                    "JWT authentication successful"
                );
                
                return Ok(next.run(req).await);
            } else {
                let error = ApiErrorBuilder::new(ApiErrorCode::Unauthorized)
                    .message("Invalid or expired JWT token")
                    .correlation_id(request_id.unwrap_or_else(Uuid::new_v4))
                    .build();
                
                return Err(create_error_response(
                    error,
                    request_id,
                    Some(req.uri().path().to_string()),
                    Some(req.method().to_string()),
                ));
            }
        }
    }

    // Try API key authentication
    if let Some(api_key) = api_key_header {
        match auth_service.validate_api_key(api_key).await {
            Ok(key_info) => {
                // Add API key info to request extensions
                req.extensions_mut().insert(key_info.owner_id.clone());
                req.extensions_mut().insert(key_info.permissions.clone());
                req.extensions_mut().insert(key_info.scopes.clone());
                req.extensions_mut().insert(key_info.key_id.clone());
                
                debug!(
                    key_id = %key_info.key_id,
                    owner_id = %key_info.owner_id,
                    "API key authentication successful"
                );
                
                return Ok(next.run(req).await);
            }
            Err(e) => {
                let error = ApiErrorBuilder::new(ApiErrorCode::Unauthorized)
                    .message(format!("Invalid API key: {}", e))
                    .correlation_id(request_id.unwrap_or_else(Uuid::new_v4))
                    .build();
                
                return Err(create_error_response(
                    error,
                    request_id,
                    Some(req.uri().path().to_string()),
                    Some(req.method().to_string()),
                ));
            }
        }
    }

    // No valid authentication found
    let error = ApiErrorBuilder::new(ApiErrorCode::Unauthorized)
        .message("Authentication required. Provide either JWT token or API key.")
        .correlation_id(request_id.unwrap_or_else(Uuid::new_v4))
        .build();
    
    Err(create_error_response(
        error,
        request_id,
        Some(req.uri().path().to_string()),
        Some(req.method().to_string()),
    ))
}

/// Optional authentication middleware (allows unauthenticated requests)
pub async fn optional_auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    mut req: Request,
    next: Next,
) -> Response {
    // Try to authenticate, but don't fail if authentication is missing
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let api_key_header = req
        .headers()
        .get("x-api-key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // Try JWT authentication
    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            let validation = auth_service.validate_jwt(token).await;
            
            if validation.valid {
                let claims = validation.claims.unwrap();
                req.extensions_mut().insert(claims.sub.clone());
                req.extensions_mut().insert(claims.roles.clone());
                req.extensions_mut().insert(claims.permissions.clone());
            }
        }
    }

    // Try API key authentication
    if let Some(api_key) = api_key_header {
        if let Ok(key_info) = auth_service.validate_api_key(&api_key).await {
            req.extensions_mut().insert(key_info.owner_id.clone());
            req.extensions_mut().insert(key_info.permissions.clone());
            req.extensions_mut().insert(key_info.scopes.clone());
        }
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_manager_creation() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);
        
        // Manager should be created successfully
        assert_eq!(manager.config.issuer, "rustci-api");
        assert_eq!(manager.config.audience, "rustci-users");
    }

    #[tokio::test]
    async fn test_token_generation_and_validation() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);
        
        let user = UserInfo {
            user_id: "user123".to_string(),
            email: "user@example.com".to_string(),
            display_name: "Test User".to_string(),
            avatar_url: None,
            roles: vec!["user".to_string()],
            permissions: vec!["read".to_string()],
        };
        
        let session_id = manager.create_session(user.user_id.clone(), HashMap::new()).await;
        let token = manager.generate_access_token(&user, session_id).await.unwrap();
        
        let validation = manager.validate_token(&token).await;
        assert!(validation.valid);
        assert!(validation.claims.is_some());
        
        let claims = validation.claims.unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[tokio::test]
    async fn test_token_revocation() {
        let config = JwtConfig::default();
        let manager = JwtManager::new(config);
        
        let user = UserInfo {
            user_id: "user123".to_string(),
            email: "user@example.com".to_string(),
            display_name: "Test User".to_string(),
            avatar_url: None,
            roles: vec!["user".to_string()],
            permissions: vec!["read".to_string()],
        };
        
        let session_id = manager.create_session(user.user_id.clone(), HashMap::new()).await;
        let token = manager.generate_access_token(&user, session_id).await.unwrap();
        
        // Token should be valid initially
        let validation = manager.validate_token(&token).await;
        assert!(validation.valid);
        
        // Revoke token
        manager.revoke_token(&token).await.unwrap();
        
        // Token should now be invalid
        let validation = manager.validate_token(&token).await;
        assert!(!validation.valid);
        assert!(validation.error.unwrap().contains("revoked"));
    }

    #[tokio::test]
    async fn test_api_key_generation_and_validation() {
        let manager = ApiKeyManager::new();
        
        let (raw_key, api_key) = manager.generate_api_key(
            "Test Key".to_string(),
            "owner123".to_string(),
            vec!["read".to_string(), "write".to_string()],
            vec!["pipelines".to_string()],
            None,
        ).await.unwrap();
        
        assert!(raw_key.starts_with("rustci_"));
        assert_eq!(api_key.name, "Test Key");
        assert_eq!(api_key.owner_id, "owner123");
        assert!(api_key.active);
        
        // Validate the key
        let validated_key = manager.validate_api_key(&raw_key).await.unwrap();
        assert_eq!(validated_key.key_id, api_key.key_id);
        assert_eq!(validated_key.usage_count, 1); // Should increment on validation
    }

    #[tokio::test]
    async fn test_api_key_revocation() {
        let manager = ApiKeyManager::new();
        
        let (raw_key, api_key) = manager.generate_api_key(
            "Test Key".to_string(),
            "owner123".to_string(),
            vec!["read".to_string()],
            vec!["pipelines".to_string()],
            None,
        ).await.unwrap();
        
        // Key should be valid initially
        let result = manager.validate_api_key(&raw_key).await;
        assert!(result.is_ok());
        
        // Revoke key
        manager.revoke_api_key(&api_key.key_id).await.unwrap();
        
        // Key should now be invalid
        let result = manager.validate_api_key(&raw_key).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_auth_service_integration() {
        let config = JwtConfig::default();
        let service = AuthService::new(config);
        
        let user = UserInfo {
            user_id: "user123".to_string(),
            email: "user@example.com".to_string(),
            display_name: "Test User".to_string(),
            avatar_url: None,
            roles: vec!["user".to_string()],
            permissions: vec!["read".to_string()],
        };
        
        // Authenticate user
        let auth_response = service.authenticate_user(user, HashMap::new()).await.unwrap();
        
        assert!(!auth_response.access_token.is_empty());
        assert!(!auth_response.refresh_token.is_empty());
        assert_eq!(auth_response.token_type, "Bearer");
        
        // Validate access token
        let validation = service.validate_jwt(&auth_response.access_token).await;
        assert!(validation.valid);
        
        // Refresh tokens
        let refreshed = service.refresh_tokens(&auth_response.refresh_token).await.unwrap();
        assert!(!refreshed.access_token.is_empty());
        assert_ne!(refreshed.access_token, auth_response.access_token);
    }

    #[test]
    fn test_jwt_config_default() {
        let config = JwtConfig::default();
        assert_eq!(config.issuer, "rustci-api");
        assert_eq!(config.audience, "rustci-users");
        assert_eq!(config.algorithm, Algorithm::HS256);
        assert_eq!(config.access_token_expiry, Duration::hours(1));
        assert_eq!(config.refresh_token_expiry, Duration::days(30));
    }

    #[test]
    fn test_token_type_serialization() {
        let access_type = TokenType::Access;
        let refresh_type = TokenType::Refresh;
        
        let access_json = serde_json::to_value(&access_type).unwrap();
        let refresh_json = serde_json::to_value(&refresh_type).unwrap();
        
        assert_eq!(access_json, "access");
        assert_eq!(refresh_json, "refresh");
        
        let deserialized_access: TokenType = serde_json::from_value(access_json).unwrap();
        let deserialized_refresh: TokenType = serde_json::from_value(refresh_json).unwrap();
        
        assert_eq!(deserialized_access, TokenType::Access);
        assert_eq!(deserialized_refresh, TokenType::Refresh);
    }
}