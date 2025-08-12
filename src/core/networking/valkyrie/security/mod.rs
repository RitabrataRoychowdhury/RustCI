/// Valkyrie Protocol Security Framework
/// 
/// This module provides enterprise-grade security features including:
/// - Post-quantum cryptography
/// - Multi-provider authentication
/// - Role-based access control (RBAC)
/// - Intrusion detection system
/// - Security audit logging
/// - Certificate management

pub mod auth;
pub mod crypto;
pub mod audit;
pub mod rbac;
pub mod ids;
pub mod cert_manager;

pub use auth::*;
pub use crypto::*;
pub use audit::*;
pub use rbac::*;
pub use ids::*;
pub use cert_manager::*;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// NodeId import removed as unused
use crate::error::Result;

/// Security policy engine for centralized policy management
pub struct SecurityPolicyEngine {
    policies: Arc<RwLock<HashMap<String, SecurityPolicy>>>,
}

impl SecurityPolicyEngine {
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_policy(&self, name: String, policy: SecurityPolicy) {
        self.policies.write().await.insert(name, policy);
    }

    pub async fn get_policy(&self, name: &str) -> Option<SecurityPolicy> {
        self.policies.read().await.get(name).cloned()
    }
}

/// Security policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicyConfig {
    pub default_policy: String,
    pub strict_mode: bool,
    pub audit_all_access: bool,
}

impl Default for SecurityPolicyConfig {
    fn default() -> Self {
        Self {
            default_policy: "default".to_string(),
            strict_mode: false,
            audit_all_access: true,
        }
    }
}

/// Security policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub name: String,
    pub description: String,
    pub rules: Vec<SecurityRule>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub active: bool,
}

/// Security rule within a policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRule {
    pub name: String,
    pub condition: String,
    pub action: SecurityAction,
    pub priority: u32,
}

/// Security actions that can be taken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityAction {
    Allow,
    Deny,
    Log,
    Alert,
    Block,
}

/// Comprehensive security manager for the Valkyrie Protocol
pub struct SecurityManager {
    /// Authentication providers registry
    auth_providers: HashMap<AuthMethod, Box<dyn AuthProvider>>,
    /// Encryption engines registry
    encryption_engines: HashMap<EncryptionMethod, Box<dyn EncryptionEngine>>,
    /// Certificate manager
    cert_manager: Arc<CertificateManager>,
    /// Security policy engine
    policy_engine: Arc<SecurityPolicyEngine>,
    /// Intrusion detection system
    ids: Arc<IntrusionDetectionSystem>,
    /// Security audit logger
    audit_logger: Arc<SecurityAuditLogger>,
    /// RBAC manager
    rbac_manager: Arc<RbacManager>,
    /// Post-quantum crypto engine
    pq_crypto: Arc<PostQuantumCrypto>,
}

impl SecurityManager {
    /// Create a new security manager with default configuration
    pub fn new(config: SecurityConfig) -> Result<Self> {
        let cert_manager = Arc::new(CertificateManager::new(config.certificate.clone())?);
        let policy_engine = Arc::new(SecurityPolicyEngine::new());
        let ids = Arc::new(IntrusionDetectionSystem::new(config.ids.clone())?);
        let audit_logger = Arc::new(SecurityAuditLogger::new(config.audit.clone())?);
        let rbac_manager = Arc::new(RbacManager::new(config.rbac.clone())?);
        let pq_crypto = Arc::new(PostQuantumCrypto::new(config.post_quantum.clone())?);

        let mut auth_providers: HashMap<AuthMethod, Box<dyn AuthProvider>> = HashMap::new();
        let mut encryption_engines: HashMap<EncryptionMethod, Box<dyn EncryptionEngine>> = HashMap::new();

        // Initialize default authentication providers
        if config.auth.jwt_enabled {
            auth_providers.insert(AuthMethod::JWT, Box::new(JwtAuthProvider::new(config.auth.jwt.clone())?));
        }
        if config.auth.mtls_enabled {
            auth_providers.insert(AuthMethod::MutualTLS, Box::new(MtlsAuthProvider::new(cert_manager.clone())?));
        }
        if config.auth.spiffe_enabled {
            auth_providers.insert(AuthMethod::SPIFFE, Box::new(SpiffeAuthProvider::new(config.auth.spiffe.clone())?));
        }

        // Initialize encryption engines
        encryption_engines.insert(EncryptionMethod::AES256GCM, Box::new(Aes256GcmEngine::new()?));
        encryption_engines.insert(EncryptionMethod::ChaCha20Poly1305, Box::new(ChaCha20Poly1305Engine::new()?));
        encryption_engines.insert(EncryptionMethod::PostQuantum, Box::new(PostQuantumEngine::new(pq_crypto.clone())?));

        Ok(Self {
            auth_providers,
            encryption_engines,
            cert_manager,
            policy_engine,
            ids,
            audit_logger,
            rbac_manager,
            pq_crypto,
        })
    }

    /// Authenticate a node using the specified method
    pub async fn authenticate(&self, method: AuthMethod, credentials: AuthCredentials) -> Result<AuthResult> {
        let provider = self.auth_providers.get(&method)
            .ok_or_else(|| crate::error::AppError::SecurityError(format!("Authentication method {:?} not supported", method)))?;

        let start_time = std::time::Instant::now();
        let result = provider.authenticate(credentials.clone()).await;
        let duration = start_time.elapsed();

        // Log authentication attempt
        let event_type = match &result {
            Ok(_) => SecurityEventType::AuthenticationSuccess,
            Err(_) => SecurityEventType::AuthenticationFailure,
        };

        let mut audit_entry = SecurityAuditEntry::new(
            event_type,
            credentials.node_id(),
            credentials.source_ip().unwrap_or_default(),
            match &result {
                Ok(_) => SecuritySeverity::Info,
                Err(_) => SecuritySeverity::Warning,
            },
        );
        audit_entry.method = Some(method);
        audit_entry.details = HashMap::from([
            ("duration_ms".to_string(), duration.as_millis().to_string()),
            ("method".to_string(), format!("{:?}", method)),
        ]);
        
        self.audit_logger.log_event(audit_entry).await?;

        // Check for suspicious activity
        if let Err(ref error) = result {
            self.ids.report_authentication_failure(
                credentials.node_id(),
                credentials.source_ip().unwrap_or_default(),
                method,
                error.to_string(),
            ).await?;
        }

        result
    }

    /// Authorize an operation using RBAC
    pub async fn authorize(&self, subject: &AuthSubject, resource: &str, action: &str) -> Result<bool> {
        let authorized = self.rbac_manager.check_permission(subject, resource, action).await?;

        // Log authorization attempt
        let mut audit_entry = SecurityAuditEntry::new(
            if authorized {
                SecurityEventType::AuthorizationSuccess
            } else {
                SecurityEventType::AuthorizationFailure
            },
            Some(subject.node_id.clone()),
            subject.source_ip.clone().unwrap_or_default(),
            if authorized {
                SecuritySeverity::Info
            } else {
                SecuritySeverity::Warning
            },
        );
        audit_entry.details = HashMap::from([
            ("resource".to_string(), resource.to_string()),
            ("action".to_string(), action.to_string()),
            ("subject".to_string(), subject.identity.clone()),
        ]);
        
        self.audit_logger.log_event(audit_entry).await?;

        if !authorized {
            self.ids.report_authorization_failure(
                &subject.node_id,
                subject.source_ip.as_deref().unwrap_or_default(),
                resource,
                action,
            ).await?;
        }

        Ok(authorized)
    }

    /// Encrypt data using the specified method
    pub async fn encrypt(&self, method: EncryptionMethod, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let engine = self.encryption_engines.get(&method)
            .ok_or_else(|| crate::error::AppError::SecurityError(format!("Encryption method {:?} not supported", method)))?;

        engine.encrypt(data, context).await
    }

    /// Decrypt data using the specified method
    pub async fn decrypt(&self, method: EncryptionMethod, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let engine = self.encryption_engines.get(&method)
            .ok_or_else(|| crate::error::AppError::SecurityError(format!("Encryption method {:?} not supported", method)))?;

        engine.decrypt(data, context).await
    }

    /// Get security metrics
    pub async fn get_metrics(&self) -> SecurityMetrics {
        SecurityMetrics {
            authentication_attempts: self.audit_logger.count_events_by_type(SecurityEventType::AuthenticationSuccess).await +
                                   self.audit_logger.count_events_by_type(SecurityEventType::AuthenticationFailure).await,
            authentication_failures: self.audit_logger.count_events_by_type(SecurityEventType::AuthenticationFailure).await,
            authorization_failures: self.audit_logger.count_events_by_type(SecurityEventType::AuthorizationFailure).await,
            intrusion_attempts: self.ids.get_threat_count().await,
            active_certificates: self.cert_manager.get_active_certificate_count().await,
            certificate_expiring_soon: self.cert_manager.get_expiring_certificate_count(Duration::from_secs(30 * 24 * 3600)).await, // 30 days
        }
    }

    /// Perform security health check
    pub async fn health_check(&self) -> SecurityHealthStatus {
        let mut issues = Vec::new();

        // Check certificate health
        if let Err(e) = self.cert_manager.health_check().await {
            issues.push(format!("Certificate manager: {}", e));
        }

        // Check IDS health
        if let Err(e) = self.ids.health_check().await {
            issues.push(format!("Intrusion detection: {}", e));
        }

        // Check audit logger health
        if let Err(e) = self.audit_logger.health_check().await {
            issues.push(format!("Audit logger: {}", e));
        }

        SecurityHealthStatus {
            healthy: issues.is_empty(),
            issues,
            last_check: chrono::Utc::now(),
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub auth: AuthConfig,
    pub certificate: CertificateConfig,
    pub policy: SecurityPolicyConfig,
    pub ids: IdsConfig,
    pub audit: AuditConfig,
    pub rbac: RbacConfig,
    pub post_quantum: PostQuantumConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            auth: AuthConfig::default(),
            certificate: CertificateConfig::default(),
            policy: SecurityPolicyConfig::default(),
            ids: IdsConfig::default(),
            audit: AuditConfig::default(),
            rbac: RbacConfig::default(),
            post_quantum: PostQuantumConfig::default(),
        }
    }
}

/// Security metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetrics {
    pub authentication_attempts: u64,
    pub authentication_failures: u64,
    pub authorization_failures: u64,
    pub intrusion_attempts: u64,
    pub active_certificates: u64,
    pub certificate_expiring_soon: u64,
}

/// Security health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHealthStatus {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Authentication methods supported by the security framework
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMethod {
    JWT,
    MutualTLS,
    SPIFFE,
    OAuth2,
    LDAP,
    Custom(u16),
}

/// Encryption methods supported by the security framework
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EncryptionMethod {
    AES256GCM,
    ChaCha20Poly1305,
    PostQuantum,
    Custom(u16),
}

/// Security event types for audit logging
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecurityEventType {
    AuthenticationSuccess,
    AuthenticationFailure,
    AuthorizationSuccess,
    AuthorizationFailure,
    TokenGenerated,
    TokenRevoked,
    CertificateIssued,
    CertificateRevoked,
    CertificateExpired,
    ConnectionEstablished,
    ConnectionClosed,
    IntrusionDetected,
    ThreatMitigated,
    PolicyViolation,
    AuditLogTampered,
    SystemCompromised,
}

/// Security event severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecuritySeverity {
    Info,
    Warning,
    Error,
    Critical,
}