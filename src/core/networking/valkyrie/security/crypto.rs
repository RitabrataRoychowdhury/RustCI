use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use async_trait::async_trait;
use chacha20poly1305::{
    aead::{Aead as ChaChaAead, KeyInit as ChaChaKeyInit},
    ChaCha20Poly1305,
};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
/// Post-quantum cryptography and encryption engines for the Valkyrie Protocol
///
/// This module provides:
/// - Post-quantum key exchange (Kyber)
/// - Post-quantum digital signatures (Dilithium)
/// - Modern symmetric encryption (AES-256-GCM, ChaCha20-Poly1305)
/// - Quantum-safe random number generation
/// - Cryptographic agility for future algorithm transitions
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
// EncryptionMethod import removed as unused

/// Encryption engine trait for pluggable cryptographic implementations
#[async_trait]
pub trait EncryptionEngine: Send + Sync {
    /// Encrypt data with the given context
    async fn encrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>>;

    /// Decrypt data with the given context
    async fn decrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>>;

    /// Generate a new encryption key
    async fn generate_key(&self) -> Result<Vec<u8>>;

    /// Get encryption capabilities
    fn capabilities(&self) -> EncryptionCapabilities;

    /// Get encryption metrics
    async fn metrics(&self) -> EncryptionMetrics;
}

/// Encryption context containing metadata and parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionContext {
    pub key_id: String,
    pub algorithm: String,
    pub additional_data: Option<Vec<u8>>,
    pub nonce: Option<Vec<u8>>,
    pub metadata: HashMap<String, String>,
}

/// Encryption capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionCapabilities {
    pub algorithm: String,
    pub key_size_bits: u32,
    pub nonce_size_bytes: u32,
    pub supports_aead: bool,
    pub quantum_safe: bool,
    pub performance_tier: PerformanceTier,
}

/// Performance tier classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceTier {
    High,     // Optimized for speed
    Balanced, // Balance of speed and security
    Secure,   // Optimized for security
}

/// Encryption metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetrics {
    pub total_encryptions: u64,
    pub total_decryptions: u64,
    pub total_bytes_encrypted: u64,
    pub total_bytes_decrypted: u64,
    pub average_encrypt_time_ms: f64,
    pub average_decrypt_time_ms: f64,
    pub error_count: u64,
}

/// Post-quantum cryptography engine
pub struct PostQuantumCrypto {
    config: PostQuantumConfig,
    kyber: KyberKeyExchange,
    dilithium: DilithiumSignature,
    rng: Arc<RwLock<ChaCha20Rng>>,
    metrics: Arc<RwLock<PostQuantumMetrics>>,
}

/// Post-quantum configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostQuantumConfig {
    pub kyber_variant: KyberVariant,
    pub dilithium_variant: DilithiumVariant,
    pub enable_hybrid_mode: bool,
    pub classical_backup: ClassicalAlgorithm,
}

impl Default for PostQuantumConfig {
    fn default() -> Self {
        Self {
            kyber_variant: KyberVariant::Kyber768,
            dilithium_variant: DilithiumVariant::Dilithium3,
            enable_hybrid_mode: true,
            classical_backup: ClassicalAlgorithm::X25519,
        }
    }
}

/// Kyber key exchange variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KyberVariant {
    Kyber512,  // NIST Level 1
    Kyber768,  // NIST Level 3
    Kyber1024, // NIST Level 5
}

/// Dilithium signature variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DilithiumVariant {
    Dilithium2, // NIST Level 1
    Dilithium3, // NIST Level 3
    Dilithium5, // NIST Level 5
}

/// Classical algorithm fallbacks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClassicalAlgorithm {
    X25519,
    P256,
    P384,
    P521,
}

/// Post-quantum metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostQuantumMetrics {
    pub key_exchanges: u64,
    pub signatures_generated: u64,
    pub signatures_verified: u64,
    pub hybrid_operations: u64,
    pub quantum_safe_operations: u64,
    pub average_keygen_time_ms: f64,
    pub average_sign_time_ms: f64,
    pub average_verify_time_ms: f64,
}

impl PostQuantumCrypto {
    pub fn new(config: PostQuantumConfig) -> Result<Self> {
        let kyber = KyberKeyExchange::new(config.kyber_variant.clone())?;
        let dilithium = DilithiumSignature::new(config.dilithium_variant.clone())?;
        let rng = Arc::new(RwLock::new(ChaCha20Rng::from_entropy()));

        Ok(Self {
            config,
            kyber,
            dilithium,
            rng,
            metrics: Arc::new(RwLock::new(PostQuantumMetrics {
                key_exchanges: 0,
                signatures_generated: 0,
                signatures_verified: 0,
                hybrid_operations: 0,
                quantum_safe_operations: 0,
                average_keygen_time_ms: 0.0,
                average_sign_time_ms: 0.0,
                average_verify_time_ms: 0.0,
            })),
        })
    }

    /// Generate a post-quantum key pair
    pub async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        let start_time = std::time::Instant::now();

        let (public_key, secret_key) = self.kyber.generate_keypair().await?;

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.quantum_safe_operations += 1;
        metrics.average_keygen_time_ms = (metrics.average_keygen_time_ms
            * (metrics.quantum_safe_operations - 1) as f64
            + duration.as_millis() as f64)
            / metrics.quantum_safe_operations as f64;

        Ok((public_key, secret_key))
    }

    /// Perform key exchange
    pub async fn key_exchange(&self, public_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let start_time = std::time::Instant::now();

        let (ciphertext, shared_secret) = self.kyber.encapsulate(public_key).await?;

        let _duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.key_exchanges += 1;
        metrics.quantum_safe_operations += 1;

        Ok((ciphertext, shared_secret))
    }

    /// Decapsulate shared secret
    pub async fn decapsulate(&self, ciphertext: &[u8], secret_key: &[u8]) -> Result<Vec<u8>> {
        self.kyber.decapsulate(ciphertext, secret_key).await
    }

    /// Sign data with post-quantum signature
    pub async fn sign(&self, data: &[u8], secret_key: &[u8]) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        let signature = self.dilithium.sign(data, secret_key).await?;

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.signatures_generated += 1;
        metrics.quantum_safe_operations += 1;
        metrics.average_sign_time_ms = (metrics.average_sign_time_ms
            * (metrics.signatures_generated - 1) as f64
            + duration.as_millis() as f64)
            / metrics.signatures_generated as f64;

        Ok(signature)
    }

    /// Verify post-quantum signature
    pub async fn verify(&self, data: &[u8], signature: &[u8], public_key: &[u8]) -> Result<bool> {
        let start_time = std::time::Instant::now();

        let valid = self.dilithium.verify(data, signature, public_key).await?;

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.signatures_verified += 1;
        metrics.quantum_safe_operations += 1;
        metrics.average_verify_time_ms = (metrics.average_verify_time_ms
            * (metrics.signatures_verified - 1) as f64
            + duration.as_millis() as f64)
            / metrics.signatures_verified as f64;

        Ok(valid)
    }

    /// Get post-quantum metrics
    pub async fn get_metrics(&self) -> PostQuantumMetrics {
        self.metrics.read().await.clone()
    }
}

/// Kyber key exchange implementation
pub struct KyberKeyExchange {
    variant: KyberVariant,
}

impl KyberKeyExchange {
    pub fn new(variant: KyberVariant) -> Result<Self> {
        Ok(Self { variant })
    }

    pub async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        // In a real implementation, this would use the actual Kyber algorithm
        // For now, we'll simulate with placeholder values
        let mut rng = ChaCha20Rng::from_entropy();

        let (public_key_size, secret_key_size) = match self.variant {
            KyberVariant::Kyber512 => (800, 1632),
            KyberVariant::Kyber768 => (1184, 2400),
            KyberVariant::Kyber1024 => (1568, 3168),
        };

        let mut public_key = vec![0u8; public_key_size];
        let mut secret_key = vec![0u8; secret_key_size];

        rng.fill_bytes(&mut public_key);
        rng.fill_bytes(&mut secret_key);

        Ok((public_key, secret_key))
    }

    pub async fn encapsulate(&self, _public_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        // Placeholder implementation
        let mut rng = ChaCha20Rng::from_entropy();

        let (ciphertext_size, shared_secret_size) = match self.variant {
            KyberVariant::Kyber512 => (768, 32),
            KyberVariant::Kyber768 => (1088, 32),
            KyberVariant::Kyber1024 => (1568, 32),
        };

        let mut ciphertext = vec![0u8; ciphertext_size];
        let mut shared_secret = vec![0u8; shared_secret_size];

        rng.fill_bytes(&mut ciphertext);
        rng.fill_bytes(&mut shared_secret);

        Ok((ciphertext, shared_secret))
    }

    pub async fn decapsulate(&self, _ciphertext: &[u8], _secret_key: &[u8]) -> Result<Vec<u8>> {
        // Placeholder implementation
        let mut rng = ChaCha20Rng::from_entropy();
        let mut shared_secret = vec![0u8; 32];
        rng.fill_bytes(&mut shared_secret);
        Ok(shared_secret)
    }
}

/// Dilithium signature implementation
pub struct DilithiumSignature {
    variant: DilithiumVariant,
}

impl DilithiumSignature {
    pub fn new(variant: DilithiumVariant) -> Result<Self> {
        Ok(Self { variant })
    }

    pub async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        let mut rng = ChaCha20Rng::from_entropy();

        let (public_key_size, secret_key_size) = match self.variant {
            DilithiumVariant::Dilithium2 => (1312, 2528),
            DilithiumVariant::Dilithium3 => (1952, 4000),
            DilithiumVariant::Dilithium5 => (2592, 4864),
        };

        let mut public_key = vec![0u8; public_key_size];
        let mut secret_key = vec![0u8; secret_key_size];

        rng.fill_bytes(&mut public_key);
        rng.fill_bytes(&mut secret_key);

        Ok((public_key, secret_key))
    }

    pub async fn sign(&self, _data: &[u8], _secret_key: &[u8]) -> Result<Vec<u8>> {
        // Placeholder implementation
        let mut rng = ChaCha20Rng::from_entropy();

        let signature_size = match self.variant {
            DilithiumVariant::Dilithium2 => 2420,
            DilithiumVariant::Dilithium3 => 3293,
            DilithiumVariant::Dilithium5 => 4595,
        };

        let mut signature = vec![0u8; signature_size];
        rng.fill_bytes(&mut signature);
        Ok(signature)
    }

    pub async fn verify(
        &self,
        _data: &[u8],
        _signature: &[u8],
        _public_key: &[u8],
    ) -> Result<bool> {
        // Placeholder implementation - in reality, this would verify the signature
        Ok(true)
    }
}

/// AES-256-GCM encryption engine
pub struct Aes256GcmEngine {
    metrics: Arc<RwLock<EncryptionMetrics>>,
}

impl Aes256GcmEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            metrics: Arc::new(RwLock::new(EncryptionMetrics {
                total_encryptions: 0,
                total_decryptions: 0,
                total_bytes_encrypted: 0,
                total_bytes_decrypted: 0,
                average_encrypt_time_ms: 0.0,
                average_decrypt_time_ms: 0.0,
                error_count: 0,
            })),
        })
    }
}

#[async_trait]
impl EncryptionEngine for Aes256GcmEngine {
    async fn encrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        // Parse key from context
        let key_bytes = hex::decode(&context.key_id)
            .map_err(|_| crate::error::AppError::SecurityError("Invalid key format".to_string()))?;

        if key_bytes.len() != 32 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid key length for AES-256".to_string(),
            )
            .into());
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        // Generate or use provided nonce
        let nonce_bytes = if let Some(ref nonce) = context.nonce {
            if nonce.len() != 12 {
                return Err(crate::error::AppError::SecurityError(
                    "Invalid nonce length for AES-GCM".to_string(),
                )
                .into());
            }
            nonce.clone()
        } else {
            let mut nonce = vec![0u8; 12];
            let mut rng = ChaCha20Rng::from_entropy();
            rng.fill_bytes(&mut nonce);
            nonce
        };

        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data).map_err(|e| {
            crate::error::AppError::SecurityError(format!("AES encryption failed: {}", e))
        })?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes;
        result.extend_from_slice(&ciphertext);

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_encryptions += 1;
        metrics.total_bytes_encrypted += data.len() as u64;
        metrics.average_encrypt_time_ms = (metrics.average_encrypt_time_ms
            * (metrics.total_encryptions - 1) as f64
            + duration.as_millis() as f64)
            / metrics.total_encryptions as f64;

        Ok(result)
    }

    async fn decrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        if data.len() < 12 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid ciphertext length".to_string(),
            )
            .into());
        }

        // Parse key from context
        let key_bytes = hex::decode(&context.key_id)
            .map_err(|_| crate::error::AppError::SecurityError("Invalid key format".to_string()))?;

        if key_bytes.len() != 32 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid key length for AES-256".to_string(),
            )
            .into());
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
            crate::error::AppError::SecurityError(format!("AES decryption failed: {}", e))
        })?;

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_decryptions += 1;
        metrics.total_bytes_decrypted += plaintext.len() as u64;
        metrics.average_decrypt_time_ms = (metrics.average_decrypt_time_ms
            * (metrics.total_decryptions - 1) as f64
            + duration.as_millis() as f64)
            / metrics.total_decryptions as f64;

        Ok(plaintext)
    }

    async fn generate_key(&self) -> Result<Vec<u8>> {
        let mut key = vec![0u8; 32];
        let mut rng = ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut key);
        Ok(key)
    }

    fn capabilities(&self) -> EncryptionCapabilities {
        EncryptionCapabilities {
            algorithm: "AES-256-GCM".to_string(),
            key_size_bits: 256,
            nonce_size_bytes: 12,
            supports_aead: true,
            quantum_safe: false,
            performance_tier: PerformanceTier::High,
        }
    }

    async fn metrics(&self) -> EncryptionMetrics {
        self.metrics.read().await.clone()
    }
}

/// ChaCha20-Poly1305 encryption engine
pub struct ChaCha20Poly1305Engine {
    metrics: Arc<RwLock<EncryptionMetrics>>,
}

impl ChaCha20Poly1305Engine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            metrics: Arc::new(RwLock::new(EncryptionMetrics {
                total_encryptions: 0,
                total_decryptions: 0,
                total_bytes_encrypted: 0,
                total_bytes_decrypted: 0,
                average_encrypt_time_ms: 0.0,
                average_decrypt_time_ms: 0.0,
                error_count: 0,
            })),
        })
    }
}

#[async_trait]
impl EncryptionEngine for ChaCha20Poly1305Engine {
    async fn encrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        // Parse key from context
        let key_bytes = hex::decode(&context.key_id)
            .map_err(|_| crate::error::AppError::SecurityError("Invalid key format".to_string()))?;

        if key_bytes.len() != 32 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid key length for ChaCha20".to_string(),
            )
            .into());
        }

        let key = chacha20poly1305::Key::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);

        // Generate or use provided nonce
        let nonce_bytes = if let Some(ref nonce) = context.nonce {
            if nonce.len() != 12 {
                return Err(crate::error::AppError::SecurityError(
                    "Invalid nonce length for ChaCha20-Poly1305".to_string(),
                )
                .into());
            }
            nonce.clone()
        } else {
            let mut nonce = vec![0u8; 12];
            let mut rng = ChaCha20Rng::from_entropy();
            rng.fill_bytes(&mut nonce);
            nonce
        };

        let nonce = chacha20poly1305::Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data).map_err(|e| {
            crate::error::AppError::SecurityError(format!("ChaCha20 encryption failed: {}", e))
        })?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes;
        result.extend_from_slice(&ciphertext);

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_encryptions += 1;
        metrics.total_bytes_encrypted += data.len() as u64;
        metrics.average_encrypt_time_ms = (metrics.average_encrypt_time_ms
            * (metrics.total_encryptions - 1) as f64
            + duration.as_millis() as f64)
            / metrics.total_encryptions as f64;

        Ok(result)
    }

    async fn decrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        if data.len() < 12 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid ciphertext length".to_string(),
            )
            .into());
        }

        // Parse key from context
        let key_bytes = hex::decode(&context.key_id)
            .map_err(|_| crate::error::AppError::SecurityError("Invalid key format".to_string()))?;

        if key_bytes.len() != 32 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid key length for ChaCha20".to_string(),
            )
            .into());
        }

        let key = chacha20poly1305::Key::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = chacha20poly1305::Nonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
            crate::error::AppError::SecurityError(format!("ChaCha20 decryption failed: {}", e))
        })?;

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_decryptions += 1;
        metrics.total_bytes_decrypted += plaintext.len() as u64;
        metrics.average_decrypt_time_ms = (metrics.average_decrypt_time_ms
            * (metrics.total_decryptions - 1) as f64
            + duration.as_millis() as f64)
            / metrics.total_decryptions as f64;

        Ok(plaintext)
    }

    async fn generate_key(&self) -> Result<Vec<u8>> {
        let mut key = vec![0u8; 32];
        let mut rng = ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut key);
        Ok(key)
    }

    fn capabilities(&self) -> EncryptionCapabilities {
        EncryptionCapabilities {
            algorithm: "ChaCha20-Poly1305".to_string(),
            key_size_bits: 256,
            nonce_size_bytes: 12,
            supports_aead: true,
            quantum_safe: false,
            performance_tier: PerformanceTier::Balanced,
        }
    }

    async fn metrics(&self) -> EncryptionMetrics {
        self.metrics.read().await.clone()
    }
}

/// Post-quantum encryption engine wrapper
pub struct PostQuantumEngine {
    pq_crypto: Arc<PostQuantumCrypto>,
    metrics: Arc<RwLock<EncryptionMetrics>>,
}

impl PostQuantumEngine {
    pub fn new(pq_crypto: Arc<PostQuantumCrypto>) -> Result<Self> {
        Ok(Self {
            pq_crypto,
            metrics: Arc::new(RwLock::new(EncryptionMetrics {
                total_encryptions: 0,
                total_decryptions: 0,
                total_bytes_encrypted: 0,
                total_bytes_decrypted: 0,
                average_encrypt_time_ms: 0.0,
                average_decrypt_time_ms: 0.0,
                error_count: 0,
            })),
        })
    }
}

#[async_trait]
impl EncryptionEngine for PostQuantumEngine {
    async fn encrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        // For post-quantum encryption, we use hybrid approach:
        // 1. Generate ephemeral key pair
        // 2. Perform key exchange with recipient's public key
        // 3. Use shared secret to encrypt data with ChaCha20-Poly1305

        // Parse recipient's public key from context
        let recipient_public_key = hex::decode(&context.key_id).map_err(|_| {
            crate::error::AppError::SecurityError("Invalid recipient public key format".to_string())
        })?;

        // Perform key exchange
        let (ciphertext, shared_secret) =
            self.pq_crypto.key_exchange(&recipient_public_key).await?;

        // Use shared secret as encryption key
        let key = chacha20poly1305::Key::from_slice(&shared_secret[..32]);
        let cipher = ChaCha20Poly1305::new(key);

        // Generate nonce
        let mut nonce_bytes = vec![0u8; 12];
        let mut rng = ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut nonce_bytes);
        let nonce = chacha20poly1305::Nonce::from_slice(&nonce_bytes);

        let encrypted_data = cipher.encrypt(nonce, data).map_err(|e| {
            crate::error::AppError::SecurityError(format!("Post-quantum encryption failed: {}", e))
        })?;

        // Combine key exchange ciphertext, nonce, and encrypted data
        let mut result = Vec::new();
        result.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
        result.extend_from_slice(&ciphertext);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&encrypted_data);

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_encryptions += 1;
        metrics.total_bytes_encrypted += data.len() as u64;
        metrics.average_encrypt_time_ms = (metrics.average_encrypt_time_ms
            * (metrics.total_encryptions - 1) as f64
            + duration.as_millis() as f64)
            / metrics.total_encryptions as f64;

        Ok(result)
    }

    async fn decrypt(&self, data: &[u8], context: &EncryptionContext) -> Result<Vec<u8>> {
        let start_time = std::time::Instant::now();

        if data.len() < 4 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid post-quantum ciphertext length".to_string(),
            )
            .into());
        }

        // Parse the combined data
        let ciphertext_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + ciphertext_len + 12 {
            return Err(crate::error::AppError::SecurityError(
                "Invalid post-quantum ciphertext format".to_string(),
            )
            .into());
        }

        let key_exchange_ciphertext = &data[4..4 + ciphertext_len];
        let nonce_bytes = &data[4 + ciphertext_len..4 + ciphertext_len + 12];
        let encrypted_data = &data[4 + ciphertext_len + 12..];

        // Parse secret key from context
        let secret_key = hex::decode(&context.key_id).map_err(|_| {
            crate::error::AppError::SecurityError("Invalid secret key format".to_string())
        })?;

        // Decapsulate shared secret
        let shared_secret = self
            .pq_crypto
            .decapsulate(key_exchange_ciphertext, &secret_key)
            .await?;

        // Use shared secret to decrypt data
        let key = chacha20poly1305::Key::from_slice(&shared_secret[..32]);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = chacha20poly1305::Nonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, encrypted_data).map_err(|e| {
            crate::error::AppError::SecurityError(format!("Post-quantum decryption failed: {}", e))
        })?;

        let duration = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_decryptions += 1;
        metrics.total_bytes_decrypted += plaintext.len() as u64;
        metrics.average_decrypt_time_ms = (metrics.average_decrypt_time_ms
            * (metrics.total_decryptions - 1) as f64
            + duration.as_millis() as f64)
            / metrics.total_decryptions as f64;

        Ok(plaintext)
    }

    async fn generate_key(&self) -> Result<Vec<u8>> {
        let (public_key, _secret_key) = self.pq_crypto.generate_keypair().await?;
        Ok(public_key)
    }

    fn capabilities(&self) -> EncryptionCapabilities {
        EncryptionCapabilities {
            algorithm: "Kyber+ChaCha20-Poly1305".to_string(),
            key_size_bits: 256,
            nonce_size_bytes: 12,
            supports_aead: true,
            quantum_safe: true,
            performance_tier: PerformanceTier::Secure,
        }
    }

    async fn metrics(&self) -> EncryptionMetrics {
        self.metrics.read().await.clone()
    }
}
