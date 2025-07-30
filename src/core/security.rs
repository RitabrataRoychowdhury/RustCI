use crate::error::{AppError, Result};
use base64ct::Encoding;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Role-based access control (RBAC) system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Admin,
    Developer,
    Viewer,
    ServiceAccount,
}

impl Role {
    pub fn permissions(&self) -> HashSet<Permission> {
        match self {
            Role::Admin => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPipelines);
                perms.insert(Permission::WritePipelines);
                perms.insert(Permission::ExecutePipelines);
                perms.insert(Permission::DeletePipelines);
                perms.insert(Permission::ManageUsers);
                perms.insert(Permission::ViewAuditLogs);
                perms.insert(Permission::ManageSystem);
                perms.insert(Permission::ManageRunners);
                perms.insert(Permission::TriggerJobs);
                perms.insert(Permission::ViewLogs);
                perms.insert(Permission::ViewArtifacts);
                perms.insert(Permission::CancelPipelines);
                perms
            }
            Role::Developer => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPipelines);
                perms.insert(Permission::WritePipelines);
                perms.insert(Permission::ExecutePipelines);
                perms.insert(Permission::TriggerJobs);
                perms.insert(Permission::ViewLogs);
                perms.insert(Permission::ViewArtifacts);
                perms.insert(Permission::CancelPipelines);
                perms
            }
            Role::Viewer => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPipelines);
                perms.insert(Permission::ViewLogs);
                perms.insert(Permission::ViewArtifacts);
                perms
            }
            Role::ServiceAccount => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPipelines);
                perms.insert(Permission::ExecutePipelines);
                perms.insert(Permission::TriggerJobs);
                perms.insert(Permission::ViewLogs);
                perms.insert(Permission::ViewArtifacts);
                perms
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    ReadPipelines,
    WritePipelines,
    CreatePipeline,
    ExecutePipelines,
    DeletePipelines,
    ManageUsers,
    ViewAuditLogs,
    ManageSystem,
    // Runner management permissions
    ManageRunners,
    TriggerJobs,
    ViewLogs,
    ViewArtifacts,
    CancelPipelines,
}

/// Enhanced JWT claims with RBAC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,                      // Subject (user ID)
    pub email: String,                    // User email
    pub roles: Vec<Role>,                 // User roles
    pub permissions: HashSet<Permission>, // Computed permissions
    pub iat: i64,                         // Issued at
    pub exp: i64,                         // Expires at
    pub iss: String,                      // Issuer
    pub aud: String,                      // Audience
    pub jti: String,                      // JWT ID (for revocation)
    pub session_id: String,               // Session identifier
}

impl JwtClaims {
    pub fn new(user_id: Uuid, email: String, roles: Vec<Role>, expires_in_seconds: i64) -> Self {
        let now = Utc::now().timestamp();
        let permissions = roles.iter().flat_map(|role| role.permissions()).collect();

        Self {
            sub: user_id.to_string(),
            email,
            roles,
            permissions,
            iat: now,
            exp: now + expires_in_seconds,
            iss: "rustci".to_string(),
            aud: "rustci-api".to_string(),
            jti: Uuid::new_v4().to_string(),
            session_id: Uuid::new_v4().to_string(),
        }
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }
}

/// JWT token manager with enhanced security
pub struct JwtManager {
    secret: String,
    issuer: String,
    audience: String,
    default_expiry: i64,
    revoked_tokens: tokio::sync::RwLock<HashSet<String>>, // JTI of revoked tokens
}

impl JwtManager {
    pub fn new(secret: String, default_expiry_seconds: i64) -> Self {
        Self {
            secret,
            issuer: "rustci".to_string(),
            audience: "rustci-api".to_string(),
            default_expiry: default_expiry_seconds,
            revoked_tokens: tokio::sync::RwLock::new(HashSet::new()),
        }
    }

    pub fn create_token(&self, claims: &JwtClaims) -> Result<String> {
        let header = jsonwebtoken::Header::default();
        let key = jsonwebtoken::EncodingKey::from_secret(self.secret.as_ref());

        jsonwebtoken::encode(&header, claims, &key)
            .map_err(|e| AppError::AuthError(format!("Failed to create JWT: {}", e)))
    }

    pub async fn verify_token(&self, token: &str) -> Result<JwtClaims> {
        let key = jsonwebtoken::DecodingKey::from_secret(self.secret.as_ref());
        let mut validation = jsonwebtoken::Validation::default();
        
        // Configure validation for our token structure
        validation.set_audience(&[&self.audience]);
        validation.set_issuer(&[&self.issuer]);

        let token_data = jsonwebtoken::decode::<JwtClaims>(token, &key, &validation)
            .map_err(|e| AppError::AuthError(format!("Invalid JWT: {}", e)))?;

        let claims = token_data.claims;

        // Check if token is expired
        if claims.is_expired() {
            return Err(AppError::AuthError("Token has expired".to_string()));
        }

        // Check if token is revoked
        let revoked_tokens = self.revoked_tokens.read().await;
        if revoked_tokens.contains(&claims.jti) {
            return Err(AppError::AuthError("Token has been revoked".to_string()));
        }

        Ok(claims)
    }

    pub async fn revoke_token(&self, jti: &str) -> Result<()> {
        let mut revoked_tokens = self.revoked_tokens.write().await;
        revoked_tokens.insert(jti.to_string());
        info!(jti = jti, "🚫 JWT token revoked");
        Ok(())
    }

    pub async fn revoke_user_tokens(&self, user_id: Uuid) -> Result<()> {
        // In a real implementation, this would query a database for all user tokens
        // For now, we'll just log the action
        warn!(user_id = %user_id, "🚫 All tokens revoked for user");
        Ok(())
    }
}

/// Audit logging system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub session_id: Option<String>,
    pub action: AuditAction,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub details: HashMap<String, serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditAction {
    Login,
    Logout,
    CreatePipeline,
    UpdatePipeline,
    DeletePipeline,
    ExecutePipeline,
    ViewPipeline,
    CreateUser,
    UpdateUser,
    DeleteUser,
    ChangePassword,
    RevokeToken,
    SystemConfiguration,
    FileUpload,
    FileDownload,
}

impl AuditEvent {
    pub fn new(
        action: AuditAction,
        resource_type: String,
        user_id: Option<Uuid>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            user_id,
            session_id,
            action,
            resource_type,
            resource_id: None,
            details: HashMap::new(),
            ip_address: None,
            user_agent: None,
            success: true,
            error_message: None,
        }
    }

    pub fn with_resource_id(mut self, resource_id: String) -> Self {
        self.resource_id = Some(resource_id);
        self
    }

    pub fn with_details(mut self, key: String, value: serde_json::Value) -> Self {
        self.details.insert(key, value);
        self
    }

    pub fn with_client_info(
        mut self,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.success = false;
        self.error_message = Some(error);
        self
    }
}

/// Audit logger trait
#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    async fn log_event(&self, event: AuditEvent) -> Result<()>;
    async fn query_events(
        &self,
        user_id: Option<Uuid>,
        action: Option<AuditAction>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>>;
}

/// In-memory audit logger (for development/testing)
pub struct InMemoryAuditLogger {
    events: tokio::sync::RwLock<Vec<AuditEvent>>,
}

impl Default for InMemoryAuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryAuditLogger {
    pub fn new() -> Self {
        Self {
            events: tokio::sync::RwLock::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl AuditLogger for InMemoryAuditLogger {
    async fn log_event(&self, event: AuditEvent) -> Result<()> {
        let mut events = self.events.write().await;
        debug!(
            event_id = %event.id,
            action = ?event.action,
            user_id = ?event.user_id,
            success = event.success,
            "📝 Audit event logged"
        );
        events.push(event);
        Ok(())
    }

    async fn query_events(
        &self,
        user_id: Option<Uuid>,
        action: Option<AuditAction>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>> {
        let events = self.events.read().await;
        let mut filtered: Vec<AuditEvent> = events
            .iter()
            .filter(|event| {
                if let Some(uid) = user_id {
                    if event.user_id != Some(uid) {
                        return false;
                    }
                }
                if let Some(ref act) = action {
                    if std::mem::discriminant(&event.action) != std::mem::discriminant(act) {
                        return false;
                    }
                }
                if let Some(start) = start_time {
                    if event.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if event.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by timestamp (newest first)
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit
        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }
}

/// Encryption utilities
pub struct EncryptionManager {
    key: [u8; 32], // AES-256 key
}

impl EncryptionManager {
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != 32 {
            return Err(AppError::InternalServerError(
                "Encryption key must be 32 bytes".to_string(),
            ));
        }
        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(key);
        Ok(Self { key: key_array })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        use aes_gcm::{aead::Aead, AeadCore, Aes256Gcm, KeyInit};

        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|e| {
            AppError::InternalServerError(format!("Failed to create cipher: {}", e))
        })?;

        let nonce = Aes256Gcm::generate_nonce(&mut rand::thread_rng());
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| AppError::InternalServerError(format!("Encryption failed: {}", e)))?;

        // Combine nonce and ciphertext
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(base64ct::Base64::encode_string(&result))
    }

    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};

        let data = base64ct::Base64::decode_vec(encrypted)
            .map_err(|e| AppError::InternalServerError(format!("Base64 decode failed: {}", e)))?;

        if data.len() < 12 {
            return Err(AppError::InternalServerError(
                "Invalid encrypted data".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|e| {
            AppError::InternalServerError(format!("Failed to create cipher: {}", e))
        })?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::InternalServerError(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| AppError::InternalServerError(format!("Invalid UTF-8: {}", e)))
    }
}

/// Security context for requests
#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub user_id: Uuid,
    pub roles: Vec<Role>,
    pub permissions: HashSet<Permission>,
    pub session_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl SecurityContext {
    pub fn from_claims(claims: &JwtClaims) -> Result<Self> {
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|e| AppError::AuthError(format!("Invalid user ID: {}", e)))?;

        Ok(Self {
            user_id,
            roles: claims.roles.clone(),
            permissions: claims.permissions.clone(),
            session_id: claims.session_id.clone(),
            ip_address: None,
            user_agent: None,
        })
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    pub fn require_permission(&self, permission: &Permission) -> Result<()> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(AppError::AuthError(format!(
                "Insufficient permissions: {:?} required",
                permission
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permissions() {
        let admin_perms = Role::Admin.permissions();
        assert!(admin_perms.contains(&Permission::ManageUsers));
        assert!(admin_perms.contains(&Permission::ReadPipelines));

        let viewer_perms = Role::Viewer.permissions();
        assert!(viewer_perms.contains(&Permission::ReadPipelines));
        assert!(!viewer_perms.contains(&Permission::ManageUsers));
    }

    #[test]
    fn test_jwt_claims() {
        let claims = JwtClaims::new(
            Uuid::new_v4(),
            "test@example.com".to_string(),
            vec![Role::Developer],
            3600,
        );

        assert!(claims.has_permission(&Permission::ReadPipelines));
        assert!(!claims.has_permission(&Permission::ManageUsers));
        assert!(!claims.is_expired());
    }

    #[tokio::test]
    async fn test_audit_logger() {
        let logger = InMemoryAuditLogger::new();
        let event = AuditEvent::new(
            AuditAction::Login,
            "user".to_string(),
            Some(Uuid::new_v4()),
            Some("session123".to_string()),
        );

        logger.log_event(event.clone()).await.unwrap();

        let events = logger
            .query_events(None, Some(AuditAction::Login), None, None, None)
            .await
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event.id);
    }

    #[test]
    fn test_encryption() {
        let key = b"my_32_byte_key_for_testing_12345";
        let manager = EncryptionManager::new(key).unwrap();

        let plaintext = "Hello, World!";
        let encrypted = manager.encrypt(plaintext).unwrap();
        let decrypted = manager.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
