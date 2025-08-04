use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use async_trait::async_trait;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;


use crate::core::networking::node_communication::{NodeId, ProtocolMessage, ProtocolError};
use crate::core::networking::transport::{Transport, Connection, TransportConfig, TransportEndpoint, ConnectionMetadata, TransportMetrics};
use crate::core::networking::node_communication::{MessagePayload, NodeMessage};
use crate::error::Result;

/// Secure transport wrapper that adds authentication and encryption
pub struct SecureTransport {
    inner_transport: Box<dyn Transport>,
    auth_manager: Arc<AuthenticationManager>,
    encryption_manager: Arc<EncryptionManager>,
}

impl SecureTransport {
    pub fn new(
        transport: Box<dyn Transport>,
        auth_manager: Arc<AuthenticationManager>,
        encryption_manager: Arc<EncryptionManager>,
    ) -> Self {
        Self {
            inner_transport: transport,
            auth_manager,
            encryption_manager,
        }
    }
}

#[async_trait]
impl Transport for SecureTransport {
    async fn listen(&self, config: &TransportConfig) -> Result<()> {
        self.inner_transport.listen(config).await
    }
    
    async fn connect(&self, endpoint: &TransportEndpoint) -> Result<Box<dyn Connection>> {
        let inner_connection = self.inner_transport.connect(endpoint).await?;
        
        let secure_connection = SecureConnection::new(
            inner_connection,
            self.auth_manager.clone(),
            self.encryption_manager.clone(),
        );
        
        Ok(Box::new(secure_connection))
    }
    
    async fn shutdown(&self) -> Result<()> {
        self.inner_transport.shutdown().await
    }
    
    async fn get_metrics(&self) -> TransportMetrics {
        self.inner_transport.get_metrics().await
    }
}

/// Secure connection wrapper
pub struct SecureConnection {
    inner_connection: Box<dyn Connection>,
    auth_manager: Arc<AuthenticationManager>,
    encryption_manager: Arc<EncryptionManager>,
    authenticated: bool,
    node_id: Option<NodeId>,
}

impl SecureConnection {
    pub fn new(
        connection: Box<dyn Connection>,
        auth_manager: Arc<AuthenticationManager>,
        encryption_manager: Arc<EncryptionManager>,
    ) -> Self {
        Self {
            inner_connection: connection,
            auth_manager,
            encryption_manager,
            authenticated: false,
            node_id: None,
        }
    }
    
    async fn authenticate(&mut self, token: &str) -> Result<NodeId> {
        let claims = self.auth_manager.verify_token(token).await?;
        self.authenticated = true;
        self.node_id = Some(claims.node_id);
        Ok(claims.node_id)
    }
}

#[async_trait]
impl Connection for SecureConnection {
    async fn send(&mut self, message: &ProtocolMessage) -> Result<()> {
        // Encrypt message if encryption is enabled
        let encrypted_message = self.encryption_manager.encrypt_message(message).await?;
        self.inner_connection.send(&encrypted_message).await
    }
    
    async fn receive(&mut self) -> Result<ProtocolMessage> {
        let encrypted_message = self.inner_connection.receive().await?;
        
        // Decrypt message if encryption is enabled
        let message = self.encryption_manager.decrypt_message(&encrypted_message).await?;
        
        // Verify authentication for non-registration messages
        if !self.authenticated {
            match &message.message {
                MessagePayload::NodeMessage(
                    NodeMessage::RegisterNode { auth_token, .. }
                ) => {
                    self.authenticate(auth_token).await?;
                }
                _ => {
                    return Err(ProtocolError::AuthenticationFailed {
                        reason: "Connection not authenticated".to_string(),
                    }.into());
                }
            }
        }
        
        Ok(message)
    }
    
    async fn close(&mut self) -> Result<()> {
        self.inner_connection.close().await
    }
    
    fn is_connected(&self) -> bool {
        self.inner_connection.is_connected()
    }
    
    fn metadata(&self) -> ConnectionMetadata {
        self.inner_connection.metadata()
    }
}

/// JWT claims for node authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeClaims {
    pub node_id: NodeId,
    pub node_type: String,
    pub capabilities: Vec<String>,
    pub exp: u64, // Expiration time
    pub iat: u64, // Issued at
    pub iss: String, // Issuer
}

/// Authentication manager for JWT tokens
pub struct AuthenticationManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    token_expiry: Duration,
    active_tokens: Arc<RwLock<HashMap<String, NodeClaims>>>,
}

impl AuthenticationManager {
    pub fn new(secret: &str, token_expiry: Duration) -> Self {
        let encoding_key = EncodingKey::from_secret(secret.as_ref());
        let decoding_key = DecodingKey::from_secret(secret.as_ref());
        
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["rustci-control-plane"]);
        
        Self {
            encoding_key,
            decoding_key,
            validation,
            token_expiry,
            active_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Generate a JWT token for a node
    pub async fn generate_token(&self, node_id: NodeId, node_type: String, capabilities: Vec<String>) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let claims = NodeClaims {
            node_id,
            node_type,
            capabilities,
            exp: now + self.token_expiry.as_secs(),
            iat: now,
            iss: "rustci-control-plane".to_string(),
        };
        
        let token = encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ProtocolError::AuthenticationFailed {
                reason: format!("Failed to generate token: {}", e),
            })?;
        
        // Store token for tracking
        self.active_tokens.write().await.insert(token.clone(), claims);
        
        Ok(token)
    }
    
    /// Verify a JWT token and return claims
    pub async fn verify_token(&self, token: &str) -> Result<NodeClaims> {
        let token_data = decode::<NodeClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| ProtocolError::AuthenticationFailed {
                reason: format!("Invalid token: {}", e),
            })?;
        
        // Check if token is still active
        let active_tokens = self.active_tokens.read().await;
        if !active_tokens.contains_key(token) {
            return Err(ProtocolError::AuthenticationFailed {
                reason: "Token has been revoked".to_string(),
            }.into());
        }
        
        Ok(token_data.claims)
    }
    
    /// Revoke a token
    pub async fn revoke_token(&self, token: &str) -> Result<()> {
        self.active_tokens.write().await.remove(token);
        Ok(())
    }
    
    /// Clean up expired tokens
    pub async fn cleanup_expired_tokens(&self) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut active_tokens = self.active_tokens.write().await;
        active_tokens.retain(|_, claims| claims.exp > now);
        
        Ok(())
    }
}

/// Encryption manager for message encryption
pub struct EncryptionManager {
    enabled: bool,
    key: Vec<u8>,
}

impl EncryptionManager {
    pub fn new(enabled: bool, key: Option<Vec<u8>>) -> Self {
        Self {
            enabled,
            key: key.unwrap_or_else(|| b"default-encryption-key-32-bytes!".to_vec()),
        }
    }
    
    /// Encrypt a protocol message
    pub async fn encrypt_message(&self, message: &ProtocolMessage) -> Result<ProtocolMessage> {
        if !self.enabled {
            return Ok(message.clone());
        }
        
        // In a real implementation, we would use proper encryption like AES-GCM
        // For now, this is a placeholder that just returns the original message
        Ok(message.clone())
    }
    
    /// Decrypt a protocol message
    pub async fn decrypt_message(&self, message: &ProtocolMessage) -> Result<ProtocolMessage> {
        if !self.enabled {
            return Ok(message.clone());
        }
        
        // In a real implementation, we would decrypt the message
        // For now, this is a placeholder that just returns the original message
        Ok(message.clone())
    }
}

/// TLS configuration for secure transport
#[derive(Debug, Clone)]
pub struct TlsConfiguration {
    pub cert_path: String,
    pub key_path: String,
    pub ca_cert_path: Option<String>,
    pub verify_client: bool,
    pub cipher_suites: Vec<String>,
}

impl TlsConfiguration {
    pub fn new(cert_path: String, key_path: String) -> Self {
        Self {
            cert_path,
            key_path,
            ca_cert_path: None,
            verify_client: false,
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                "TLS_AES_128_GCM_SHA256".to_string(),
            ],
        }
    }
    
    pub fn with_client_verification(mut self, ca_cert_path: String) -> Self {
        self.ca_cert_path = Some(ca_cert_path);
        self.verify_client = true;
        self
    }
    
    pub fn with_cipher_suites(mut self, cipher_suites: Vec<String>) -> Self {
        self.cipher_suites = cipher_suites;
        self
    }
}

/// Security policy for node communication
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub require_tls: bool,
    pub require_mutual_auth: bool,
    pub token_expiry: Duration,
    pub max_failed_attempts: u32,
    pub lockout_duration: Duration,
    pub allowed_cipher_suites: Vec<String>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            require_tls: true,
            require_mutual_auth: false,
            token_expiry: Duration::from_secs(3600), // 1 hour
            max_failed_attempts: 3,
            lockout_duration: Duration::from_secs(300), // 5 minutes
            allowed_cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
            ],
        }
    }
}

/// Security audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: SecurityEventType,
    pub node_id: Option<NodeId>,
    pub source_ip: String,
    pub details: HashMap<String, String>,
    pub severity: SecuritySeverity,
}

/// Types of security events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEventType {
    AuthenticationSuccess,
    AuthenticationFailure,
    TokenGenerated,
    TokenRevoked,
    ConnectionEstablished,
    ConnectionClosed,
    UnauthorizedAccess,
    SuspiciousActivity,
}

/// Security event severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecuritySeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Security audit logger
pub struct SecurityAuditor {
    entries: Arc<RwLock<Vec<SecurityAuditEntry>>>,
    max_entries: usize,
}

impl SecurityAuditor {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            max_entries,
        }
    }
    
    pub async fn log_event(
        &self,
        event_type: SecurityEventType,
        node_id: Option<NodeId>,
        source_ip: String,
        details: HashMap<String, String>,
        severity: SecuritySeverity,
    ) {
        let entry = SecurityAuditEntry {
            timestamp: chrono::Utc::now(),
            event_type,
            node_id,
            source_ip,
            details,
            severity,
        };
        
        let mut entries = self.entries.write().await;
        entries.push(entry);
        
        // Keep only the most recent entries
        if entries.len() > self.max_entries {
            let excess = entries.len() - self.max_entries;
            entries.drain(0..excess);
        }
    }
    
    pub async fn get_recent_events(&self, count: usize) -> Vec<SecurityAuditEntry> {
        let entries = self.entries.read().await;
        entries.iter().rev().take(count).cloned().collect()
    }
    
    pub async fn get_events_for_node(&self, node_id: NodeId) -> Vec<SecurityAuditEntry> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|entry| entry.node_id == Some(node_id))
            .cloned()
            .collect()
    }
}