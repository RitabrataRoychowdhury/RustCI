//! Security type definitions for Valkyrie Protocol
//!
//! This module provides the security-related types and enums used throughout
//! the Valkyrie Protocol for authentication, authorization, and auditing.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Feature flags for enabling/disabling various system features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable experimental features
    pub experimental: bool,
    /// Enable RustCI integration features
    pub rustci_integration: bool,
    /// Enable container transport
    pub container_transport: bool,
    /// Enable Kubernetes transport
    pub kubernetes_transport: bool,
    /// Enable Valkyrie routing
    pub valkyrie_routing: bool,
    /// Enable advanced security features
    pub advanced_security: bool,
    /// Enable performance monitoring
    pub performance_monitoring: bool,
    /// Enable experimental features
    pub experimental_features: bool,
    /// Custom feature flags
    pub custom: std::collections::HashMap<String, bool>,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            experimental: false,
            rustci_integration: true,
            container_transport: false,
            kubernetes_transport: false,
            valkyrie_routing: true,
            advanced_security: true,
            performance_monitoring: true,
            experimental_features: false,
            custom: std::collections::HashMap::new(),
        }
    }
}

/// Authentication methods supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// No authentication (for testing/development)
    None,
    /// JWT token-based authentication
    Token,
    /// JWT token authentication
    JWT,
    /// OAuth2 authentication
    OAuth2,
    /// API key authentication
    ApiKey,
    /// Certificate-based authentication
    Certificate,
    /// Mutual TLS authentication
    MutualTls,
    /// Custom authentication method
    Custom(String),
}

impl Default for AuthMethod {
    fn default() -> Self {
        AuthMethod::JWT
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    /// Supported authentication methods
    pub methods: Vec<AuthMethod>,
    /// Token expiry duration
    pub token_expiry: Duration,
    /// Session timeout duration
    pub session_timeout: Duration,
    /// Whether mutual authentication is required
    pub require_mutual_auth: bool,
    /// Whether multi-factor authentication is required
    pub multi_factor_required: bool,
}

impl Default for AuthenticationConfig {
    fn default() -> Self {
        Self {
            methods: vec![AuthMethod::JWT],
            token_expiry: Duration::from_secs(3600), // 1 hour
            session_timeout: Duration::from_secs(3600), // 1 hour
            require_mutual_auth: false,
            multi_factor_required: false,
        }
    }
}

/// Supported cipher suites for encryption
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CipherSuite {
    /// AES-256-GCM encryption
    Aes256Gcm,
    /// AES-128-GCM encryption
    Aes128Gcm,
    /// ChaCha20-Poly1305 encryption
    ChaCha20Poly1305,
}

impl Default for CipherSuite {
    fn default() -> Self {
        CipherSuite::Aes256Gcm
    }
}

/// TLS version specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsVersion {
    /// TLS 1.2
    V1_2,
    /// TLS 1.3
    V1_3,
}

impl Default for TlsVersion {
    fn default() -> Self {
        TlsVersion::V1_3
    }
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Supported cipher suites
    pub cipher_suites: Vec<CipherSuite>,
    /// Primary cipher suite to use
    pub cipher_suite: CipherSuite,
    /// Key rotation interval
    pub key_rotation_interval: Duration,
    /// TLS version to use
    pub tls_version: TlsVersion,
    /// Whether forward secrecy is enabled
    pub forward_secrecy: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            cipher_suites: vec![CipherSuite::Aes256Gcm, CipherSuite::ChaCha20Poly1305],
            cipher_suite: CipherSuite::Aes256Gcm,
            key_rotation_interval: Duration::from_secs(86400), // 24 hours
            tls_version: TlsVersion::V1_3,
            forward_secrecy: true,
        }
    }
}

/// Policy engine types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyEngineType {
    /// Simple RBAC-based policy engine
    Simple,
    /// Advanced attribute-based access control
    ABAC,
    /// Custom policy engine
    Custom(String),
}

impl Default for PolicyEngineType {
    fn default() -> Self {
        PolicyEngineType::Simple
    }
}

/// Audit levels for logging
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLevel {
    /// No auditing
    None,
    /// Basic auditing (authentication, authorization)
    Basic,
    /// Detailed auditing (all operations)
    Detailed,
    /// Full auditing (including data access)
    Full,
}

impl Default for AuditLevel {
    fn default() -> Self {
        AuditLevel::Basic
    }
}

/// Authorization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationConfig {
    /// Whether RBAC is enabled
    pub rbac_enabled: bool,
    /// Whether role-based access control is enabled
    pub enable_rbac: bool,
    /// Policy engine type
    pub policy_engine: PolicyEngineType,
    /// Audit level
    pub audit_level: AuditLevel,
    /// Default permissions for new users
    pub default_permissions: Vec<String>,
    /// Cache TTL for authorization decisions
    pub cache_ttl: Duration,
}

impl Default for AuthorizationConfig {
    fn default() -> Self {
        Self {
            rbac_enabled: true,
            enable_rbac: true,
            policy_engine: PolicyEngineType::Simple,
            audit_level: AuditLevel::Basic,
            default_permissions: vec!["read".to_string()],
            cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Types of audit events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEvent {
    /// Authentication events
    Authentication,
    /// Authorization events
    Authorization,
    /// Data access events
    DataAccess,
    /// Configuration change events
    ConfigChange,
    /// System events
    SystemEvent,
    /// Connection events
    Connection,
    /// User management events
    UserManagement,
    /// Pipeline events
    Pipeline,
    /// Job events
    Job,
}

impl Default for AuditEvent {
    fn default() -> Self {
        AuditEvent::Authentication
    }
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Whether auditing is enabled
    pub enabled: bool,
    /// Types of events to audit
    pub events: Vec<AuditEvent>,
    /// Retention period for audit logs
    pub retention_period: Duration,
    /// Retention period in days
    pub retention_days: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            events: vec![
                AuditEvent::Authentication,
                AuditEvent::Authorization,
                AuditEvent::Connection,
            ],
            retention_period: Duration::from_secs(2592000), // 30 days
            retention_days: 30,
        }
    }
}