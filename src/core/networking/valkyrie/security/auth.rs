/// Multi-provider authentication system for the Valkyrie Protocol
/// 
/// This module provides pluggable authentication mechanisms including:
/// - JWT token authentication
/// - Mutual TLS authentication
/// - SPIFFE/SPIRE integration
/// - OAuth2 integration
/// - LDAP authentication
/// - Custom authentication providers

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::core::networking::node_communication::NodeId;
use crate::error::Result;
use super::{AuthMethod, CertificateManager};

/// Authentication provider trait
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Authenticate using the provided credentials
    async fn authenticate(&self, credentials: AuthCredentials) -> Result<AuthResult>;
    
    /// Validate an existing authentication token/session
    async fn validate(&self, token: &str) -> Result<AuthResult>;
    
    /// Revoke an authentication token/session
    async fn revoke(&self, token: &str) -> Result<()>;
    
    /// Get provider capabilities
    fn capabilities(&self) -> AuthCapabilities;
    
    /// Get provider metrics
    async fn metrics(&self) -> AuthProviderMetrics;
}

/// Authentication credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    pub method: AuthMethod,
    pub node_id: Option<NodeId>,
    pub source_ip: Option<String>,
    pub data: AuthCredentialData,
}

impl AuthCredentials {
    pub fn node_id(&self) -> Option<NodeId> {
        self.node_id.clone()
    }
    
    pub fn source_ip(&self) -> Option<String> {
        self.source_ip.clone()
    }
}

/// Authentication credential data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthCredentialData {
    JWT { token: String },
    MutualTLS { certificate: Vec<u8>, private_key: Vec<u8> },
    SPIFFE { svid: String },
    OAuth2 { access_token: String, refresh_token: Option<String> },
    LDAP { username: String, password: String },
    Custom { data: HashMap<String, String> },
}

/// Authentication result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub success: bool,
    pub subject: Option<AuthSubject>,
    pub token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub permissions: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Authenticated subject information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSubject {
    pub node_id: NodeId,
    pub identity: String,
    pub roles: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub source_ip: Option<String>,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
}

/// Authentication provider capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCapabilities {
    pub supports_token_refresh: bool,
    pub supports_revocation: bool,
    pub supports_multi_factor: bool,
    pub token_lifetime: Option<Duration>,
    pub max_concurrent_sessions: Option<u32>,
}

/// Authentication provider metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProviderMetrics {
    pub total_authentications: u64,
    pub successful_authentications: u64,
    pub failed_authentications: u64,
    pub active_sessions: u64,
    pub average_auth_time_ms: f64,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_enabled: bool,
    pub mtls_enabled: bool,
    pub spiffe_enabled: bool,
    pub oauth2_enabled: bool,
    pub ldap_enabled: bool,
    pub jwt: JwtConfig,
    pub spiffe: SpiffeConfig,
    pub oauth2: OAuth2Config,
    pub ldap: LdapConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_enabled: true,
            mtls_enabled: true,
            spiffe_enabled: false,
            oauth2_enabled: false,
            ldap_enabled: false,
            jwt: JwtConfig::default(),
            spiffe: SpiffeConfig::default(),
            oauth2: OAuth2Config::default(),
            ldap: LdapConfig::default(),
        }
    }
}

/// JWT authentication provider
pub struct JwtAuthProvider {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    active_tokens: Arc<RwLock<HashMap<String, JwtClaims>>>,
    metrics: Arc<RwLock<AuthProviderMetrics>>,
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub algorithm: String,
    pub issuer: String,
    pub audience: Option<String>,
    pub token_lifetime: Duration,
    pub refresh_token_lifetime: Duration,
    pub allow_refresh: bool,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "valkyrie-default-secret-change-in-production".to_string(),
            algorithm: "HS256".to_string(),
            issuer: "valkyrie-protocol".to_string(),
            audience: None,
            token_lifetime: Duration::from_secs(3600), // 1 hour
            refresh_token_lifetime: Duration::from_secs(7 * 24 * 3600), // 7 days
            allow_refresh: true,
        }
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String, // Subject (node_id)
    pub iss: String, // Issuer
    pub aud: Option<String>, // Audience
    pub exp: u64, // Expiration time
    pub iat: u64, // Issued at
    pub nbf: u64, // Not before
    pub jti: String, // JWT ID
    pub node_id: NodeId,
    pub node_type: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub attributes: HashMap<String, String>,
}

impl JwtAuthProvider {
    pub fn new(config: JwtConfig) -> Result<Self> {
        let algorithm = match config.algorithm.as_str() {
            "HS256" => Algorithm::HS256,
            "HS384" => Algorithm::HS384,
            "HS512" => Algorithm::HS512,
            "RS256" => Algorithm::RS256,
            "RS384" => Algorithm::RS384,
            "RS512" => Algorithm::RS512,
            _ => return Err(crate::error::AppError::SecurityError(
                format!("Unsupported JWT algorithm: {}", config.algorithm)
            ).into()),
        };

        let encoding_key = EncodingKey::from_secret(config.secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.secret.as_ref());
        
        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&[&config.issuer]);
        if let Some(ref audience) = config.audience {
            validation.set_audience(&[audience]);
        }

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            validation,
            active_tokens: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(AuthProviderMetrics {
                total_authentications: 0,
                successful_authentications: 0,
                failed_authentications: 0,
                active_sessions: 0,
                average_auth_time_ms: 0.0,
            })),
        })
    }

    pub async fn generate_token(&self, node_id: NodeId, node_type: String, roles: Vec<String>, permissions: Vec<String>) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = JwtClaims {
            sub: node_id.to_string(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            exp: now + self.config.token_lifetime.as_secs(),
            iat: now,
            nbf: now,
            jti: uuid::Uuid::new_v4().to_string(),
            node_id: node_id.clone(),
            node_type,
            roles,
            permissions,
            attributes: HashMap::new(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| crate::error::AppError::SecurityError(
                format!("Failed to generate JWT token: {}", e)
            ))?;

        // Store token for tracking
        self.active_tokens.write().await.insert(token.clone(), claims);

        Ok(token)
    }

    async fn verify_token(&self, token: &str) -> Result<JwtClaims> {
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| crate::error::AppError::SecurityError(
                format!("Invalid JWT token: {}", e)
            ))?;

        // Check if token is still active
        let active_tokens = self.active_tokens.read().await;
        if !active_tokens.contains_key(token) {
            return Err(crate::error::AppError::SecurityError(
                "Token has been revoked".to_string()
            ).into());
        }

        Ok(token_data.claims)
    }
}

#[async_trait]
impl AuthProvider for JwtAuthProvider {
    async fn authenticate(&self, credentials: AuthCredentials) -> Result<AuthResult> {
        let start_time = std::time::Instant::now();
        
        let result = match credentials.data {
            AuthCredentialData::JWT { token } => {
                match self.verify_token(&token).await {
                    Ok(claims) => {
                        let subject = AuthSubject {
                            node_id: claims.node_id.clone(),
                            identity: claims.sub.clone(),
                            roles: claims.roles.clone(),
                            attributes: claims.attributes.clone(),
                            source_ip: credentials.source_ip,
                            authenticated_at: chrono::Utc::now(),
                        };

                        AuthResult {
                            success: true,
                            subject: Some(subject),
                            token: Some(token),
                            expires_at: Some(chrono::DateTime::from_timestamp(claims.exp as i64, 0).unwrap_or_default()),
                            permissions: claims.permissions,
                            metadata: HashMap::new(),
                        }
                    }
                    Err(e) => AuthResult {
                        success: false,
                        subject: None,
                        token: None,
                        expires_at: None,
                        permissions: Vec::new(),
                        metadata: HashMap::from([
                            ("error".to_string(), e.to_string()),
                        ]),
                    }
                }
            }
            _ => AuthResult {
                success: false,
                subject: None,
                token: None,
                expires_at: None,
                permissions: Vec::new(),
                metadata: HashMap::from([
                    ("error".to_string(), "Invalid credential type for JWT provider".to_string()),
                ]),
            }
        };

        // Update metrics
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_authentications += 1;
        if result.success {
            metrics.successful_authentications += 1;
        } else {
            metrics.failed_authentications += 1;
        }
        metrics.average_auth_time_ms = (metrics.average_auth_time_ms * (metrics.total_authentications - 1) as f64 + duration.as_millis() as f64) / metrics.total_authentications as f64;

        Ok(result)
    }

    async fn validate(&self, token: &str) -> Result<AuthResult> {
        match self.verify_token(token).await {
            Ok(claims) => {
                let subject = AuthSubject {
                    node_id: claims.node_id.clone(),
                    identity: claims.sub.clone(),
                    roles: claims.roles.clone(),
                    attributes: claims.attributes.clone(),
                    source_ip: None,
                    authenticated_at: chrono::DateTime::from_timestamp(claims.iat as i64, 0).unwrap_or_default(),
                };

                Ok(AuthResult {
                    success: true,
                    subject: Some(subject),
                    token: Some(token.to_string()),
                    expires_at: Some(chrono::DateTime::from_timestamp(claims.exp as i64, 0).unwrap_or_default()),
                    permissions: claims.permissions,
                    metadata: HashMap::new(),
                })
            }
            Err(e) => Ok(AuthResult {
                success: false,
                subject: None,
                token: None,
                expires_at: None,
                permissions: Vec::new(),
                metadata: HashMap::from([
                    ("error".to_string(), e.to_string()),
                ]),
            })
        }
    }

    async fn revoke(&self, token: &str) -> Result<()> {
        self.active_tokens.write().await.remove(token);
        Ok(())
    }

    fn capabilities(&self) -> AuthCapabilities {
        AuthCapabilities {
            supports_token_refresh: self.config.allow_refresh,
            supports_revocation: true,
            supports_multi_factor: false,
            token_lifetime: Some(self.config.token_lifetime),
            max_concurrent_sessions: None,
        }
    }

    async fn metrics(&self) -> AuthProviderMetrics {
        let metrics = self.metrics.read().await;
        let active_sessions = self.active_tokens.read().await.len() as u64;
        
        AuthProviderMetrics {
            total_authentications: metrics.total_authentications,
            successful_authentications: metrics.successful_authentications,
            failed_authentications: metrics.failed_authentications,
            active_sessions,
            average_auth_time_ms: metrics.average_auth_time_ms,
        }
    }
}

/// Mutual TLS authentication provider
pub struct MtlsAuthProvider {
    cert_manager: Arc<CertificateManager>,
    metrics: Arc<RwLock<AuthProviderMetrics>>,
}

impl MtlsAuthProvider {
    pub fn new(cert_manager: Arc<CertificateManager>) -> Result<Self> {
        Ok(Self {
            cert_manager,
            metrics: Arc::new(RwLock::new(AuthProviderMetrics {
                total_authentications: 0,
                successful_authentications: 0,
                failed_authentications: 0,
                active_sessions: 0,
                average_auth_time_ms: 0.0,
            })),
        })
    }
}

#[async_trait]
impl AuthProvider for MtlsAuthProvider {
    async fn authenticate(&self, credentials: AuthCredentials) -> Result<AuthResult> {
        let start_time = std::time::Instant::now();
        
        let result = match credentials.data {
            AuthCredentialData::MutualTLS { certificate, .. } => {
                match self.cert_manager.verify_certificate(&certificate).await {
                    Ok(cert_info) => {
                        let subject = AuthSubject {
                            node_id: cert_info.subject_id.clone(),
                            identity: cert_info.common_name.clone(),
                            roles: cert_info.roles.clone(),
                            attributes: cert_info.attributes.clone(),
                            source_ip: credentials.source_ip,
                            authenticated_at: chrono::Utc::now(),
                        };

                        AuthResult {
                            success: true,
                            subject: Some(subject),
                            token: Some(cert_info.fingerprint),
                            expires_at: Some(cert_info.expires_at),
                            permissions: cert_info.permissions,
                            metadata: HashMap::new(),
                        }
                    }
                    Err(e) => AuthResult {
                        success: false,
                        subject: None,
                        token: None,
                        expires_at: None,
                        permissions: Vec::new(),
                        metadata: HashMap::from([
                            ("error".to_string(), e.to_string()),
                        ]),
                    }
                }
            }
            _ => AuthResult {
                success: false,
                subject: None,
                token: None,
                expires_at: None,
                permissions: Vec::new(),
                metadata: HashMap::from([
                    ("error".to_string(), "Invalid credential type for mTLS provider".to_string()),
                ]),
            }
        };

        // Update metrics
        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_authentications += 1;
        if result.success {
            metrics.successful_authentications += 1;
        } else {
            metrics.failed_authentications += 1;
        }
        metrics.average_auth_time_ms = (metrics.average_auth_time_ms * (metrics.total_authentications - 1) as f64 + duration.as_millis() as f64) / metrics.total_authentications as f64;

        Ok(result)
    }

    async fn validate(&self, token: &str) -> Result<AuthResult> {
        // For mTLS, the token is the certificate fingerprint
        match self.cert_manager.get_certificate_by_fingerprint(token).await {
            Ok(Some(cert_info)) => {
                if cert_info.expires_at > chrono::Utc::now() {
                    let subject = AuthSubject {
                        node_id: cert_info.subject_id.clone(),
                        identity: cert_info.common_name.clone(),
                        roles: cert_info.roles.clone(),
                        attributes: cert_info.attributes.clone(),
                        source_ip: None,
                        authenticated_at: cert_info.issued_at,
                    };

                    Ok(AuthResult {
                        success: true,
                        subject: Some(subject),
                        token: Some(token.to_string()),
                        expires_at: Some(cert_info.expires_at),
                        permissions: cert_info.permissions,
                        metadata: HashMap::new(),
                    })
                } else {
                    Ok(AuthResult {
                        success: false,
                        subject: None,
                        token: None,
                        expires_at: None,
                        permissions: Vec::new(),
                        metadata: HashMap::from([
                            ("error".to_string(), "Certificate has expired".to_string()),
                        ]),
                    })
                }
            }
            Ok(None) => Ok(AuthResult {
                success: false,
                subject: None,
                token: None,
                expires_at: None,
                permissions: Vec::new(),
                metadata: HashMap::from([
                    ("error".to_string(), "Certificate not found".to_string()),
                ]),
            }),
            Err(e) => Ok(AuthResult {
                success: false,
                subject: None,
                token: None,
                expires_at: None,
                permissions: Vec::new(),
                metadata: HashMap::from([
                    ("error".to_string(), e.to_string()),
                ]),
            })
        }
    }

    async fn revoke(&self, token: &str) -> Result<()> {
        // For mTLS, revocation means adding the certificate to the CRL
        self.cert_manager.revoke_certificate(token).await
    }

    fn capabilities(&self) -> AuthCapabilities {
        AuthCapabilities {
            supports_token_refresh: false,
            supports_revocation: true,
            supports_multi_factor: true,
            token_lifetime: None, // Certificates have their own expiration
            max_concurrent_sessions: None,
        }
    }

    async fn metrics(&self) -> AuthProviderMetrics {
        let metrics = self.metrics.read().await;
        let active_sessions = self.cert_manager.get_active_certificate_count().await;
        
        AuthProviderMetrics {
            total_authentications: metrics.total_authentications,
            successful_authentications: metrics.successful_authentications,
            failed_authentications: metrics.failed_authentications,
            active_sessions,
            average_auth_time_ms: metrics.average_auth_time_ms,
        }
    }
}

/// SPIFFE authentication provider
pub struct SpiffeAuthProvider {
    config: SpiffeConfig,
    metrics: Arc<RwLock<AuthProviderMetrics>>,
}

/// SPIFFE configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiffeConfig {
    pub trust_domain: String,
    pub workload_api_socket: String,
    pub svid_ttl: Duration,
}

impl Default for SpiffeConfig {
    fn default() -> Self {
        Self {
            trust_domain: "valkyrie.local".to_string(),
            workload_api_socket: "unix:///tmp/spire-agent/public/api.sock".to_string(),
            svid_ttl: Duration::from_secs(3600),
        }
    }
}

impl SpiffeAuthProvider {
    pub fn new(config: SpiffeConfig) -> Result<Self> {
        Ok(Self {
            config,
            metrics: Arc::new(RwLock::new(AuthProviderMetrics {
                total_authentications: 0,
                successful_authentications: 0,
                failed_authentications: 0,
                active_sessions: 0,
                average_auth_time_ms: 0.0,
            })),
        })
    }
}

#[async_trait]
impl AuthProvider for SpiffeAuthProvider {
    async fn authenticate(&self, _credentials: AuthCredentials) -> Result<AuthResult> {
        // SPIFFE implementation would go here
        // For now, return a placeholder implementation
        Ok(AuthResult {
            success: false,
            subject: None,
            token: None,
            expires_at: None,
            permissions: Vec::new(),
            metadata: HashMap::from([
                ("error".to_string(), "SPIFFE authentication not yet implemented".to_string()),
            ]),
        })
    }

    async fn validate(&self, _token: &str) -> Result<AuthResult> {
        Ok(AuthResult {
            success: false,
            subject: None,
            token: None,
            expires_at: None,
            permissions: Vec::new(),
            metadata: HashMap::from([
                ("error".to_string(), "SPIFFE validation not yet implemented".to_string()),
            ]),
        })
    }

    async fn revoke(&self, _token: &str) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> AuthCapabilities {
        AuthCapabilities {
            supports_token_refresh: true,
            supports_revocation: false,
            supports_multi_factor: false,
            token_lifetime: Some(self.config.svid_ttl),
            max_concurrent_sessions: None,
        }
    }

    async fn metrics(&self) -> AuthProviderMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }
}

/// OAuth2 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    pub authorization_url: String,
    pub token_url: String,
    pub redirect_url: String,
    pub scopes: Vec<String>,
}

impl Default for OAuth2Config {
    fn default() -> Self {
        Self {
            client_id: "valkyrie-client".to_string(),
            client_secret: "change-me".to_string(),
            authorization_url: "https://auth.example.com/oauth2/authorize".to_string(),
            token_url: "https://auth.example.com/oauth2/token".to_string(),
            redirect_url: "https://valkyrie.example.com/auth/callback".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
        }
    }
}

/// LDAP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapConfig {
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub user_base_dn: String,
    pub user_filter: String,
    pub group_base_dn: String,
    pub group_filter: String,
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            server_url: "ldap://localhost:389".to_string(),
            bind_dn: "cn=admin,dc=example,dc=com".to_string(),
            bind_password: "admin".to_string(),
            user_base_dn: "ou=users,dc=example,dc=com".to_string(),
            user_filter: "(uid={})".to_string(),
            group_base_dn: "ou=groups,dc=example,dc=com".to_string(),
            group_filter: "(member={})".to_string(),
        }
    }
}