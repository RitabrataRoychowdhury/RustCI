//! Security layer for Valkyrie Protocol

pub mod encryption;
pub mod authentication;

use crate::Result;
use serde::{Deserialize, Serialize};

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable encryption
    pub enable_encryption: bool,
    /// Encryption algorithm
    pub encryption_algorithm: EncryptionAlgorithm,
    /// Enable authentication
    pub enable_authentication: bool,
    /// Authentication method
    pub authentication_method: AuthenticationMethod,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_encryption: true,
            encryption_algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            enable_authentication: true,
            authentication_method: AuthenticationMethod::JWT,
        }
    }
}

/// Supported encryption algorithms
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// ChaCha20-Poly1305 (recommended)
    ChaCha20Poly1305,
    /// AES-256-GCM
    Aes256Gcm,
    /// No encryption
    None,
}

/// Supported authentication methods
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthenticationMethod {
    /// JSON Web Tokens
    JWT,
    /// Mutual TLS
    MTLS,
    /// API Key
    ApiKey,
    /// No authentication
    None,
}

/// Security context for a connection
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// Whether the connection is encrypted
    pub encrypted: bool,
    /// Whether the connection is authenticated
    pub authenticated: bool,
    /// User identity (if authenticated)
    pub user_id: Option<String>,
    /// Security metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self {
            encrypted: false,
            authenticated: false,
            user_id: None,
            metadata: std::collections::HashMap::new(),
        }
    }
}

/// Security manager for handling encryption and authentication
pub struct SecurityManager {
    config: SecurityConfig,
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }
    
    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
    
    /// Encrypt data
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enable_encryption {
            return Ok(data.to_vec());
        }
        
        match self.config.encryption_algorithm {
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                encryption::chacha20_encrypt(data)
            }
            EncryptionAlgorithm::Aes256Gcm => {
                encryption::aes256_encrypt(data)
            }
            EncryptionAlgorithm::None => Ok(data.to_vec()),
        }
    }
    
    /// Decrypt data
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enable_encryption {
            return Ok(data.to_vec());
        }
        
        match self.config.encryption_algorithm {
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                encryption::chacha20_decrypt(data)
            }
            EncryptionAlgorithm::Aes256Gcm => {
                encryption::aes256_decrypt(data)
            }
            EncryptionAlgorithm::None => Ok(data.to_vec()),
        }
    }
    
    /// Authenticate a token
    pub fn authenticate(&self, token: &str) -> Result<SecurityContext> {
        if !self.config.enable_authentication {
            return Ok(SecurityContext::default());
        }
        
        match self.config.authentication_method {
            AuthenticationMethod::JWT => {
                authentication::validate_jwt(token)
            }
            AuthenticationMethod::MTLS => {
                // MTLS validation would happen at transport layer
                Ok(SecurityContext {
                    encrypted: true,
                    authenticated: true,
                    user_id: Some("mtls-user".to_string()),
                    metadata: std::collections::HashMap::new(),
                })
            }
            AuthenticationMethod::ApiKey => {
                authentication::validate_api_key(token)
            }
            AuthenticationMethod::None => Ok(SecurityContext::default()),
        }
    }
    
    /// Get security configuration
    pub fn config(&self) -> &SecurityConfig {
        &self.config
    }
}