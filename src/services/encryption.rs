use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use async_trait::async_trait;
use base64ct::{Base64, Encoding};
use std::sync::Arc;

use crate::error::{AppError, Result};

#[async_trait]
pub trait EncryptionService: Send + Sync {
    async fn encrypt(&self, plaintext: &str) -> Result<String>;
    async fn decrypt(&self, ciphertext: &str) -> Result<String>;
}

pub struct AesGcmEncryptionService {
    key: Key<Aes256Gcm>,
}

impl AesGcmEncryptionService {
    pub fn new(key_base64: &str) -> Result<Self> {
        let key_bytes = Base64::decode_vec(key_base64)
            .map_err(|e| AppError::EncryptionError(format!("Invalid key format: {}", e)))?;
        
        if key_bytes.len() != 32 {
            return Err(AppError::EncryptionError(
                "Key must be 32 bytes (256 bits) long".to_string(),
            ));
        }
        
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        
        Ok(Self { key: *key })
    }
    
    pub fn from_env() -> Result<Self> {
        let key_base64 = std::env::var("ENCRYPTION_KEY")
            .map_err(|_| AppError::EncryptionError("ENCRYPTION_KEY environment variable not set".to_string()))?;
        
        Self::new(&key_base64)
    }
}

#[async_trait]
impl EncryptionService for AesGcmEncryptionService {
    async fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(&self.key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| AppError::EncryptionError(format!("Encryption failed: {}", e)))?;
        
        // Combine nonce and ciphertext, then base64 encode
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        Ok(Base64::encode_string(&combined))
    }
    
    async fn decrypt(&self, ciphertext_base64: &str) -> Result<String> {
        let combined = Base64::decode_vec(ciphertext_base64)
            .map_err(|e| AppError::EncryptionError(format!("Invalid ciphertext format: {}", e)))?;
        
        if combined.len() < 12 {
            return Err(AppError::EncryptionError(
                "Ciphertext too short".to_string(),
            ));
        }
        
        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext_bytes) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let cipher = Aes256Gcm::new(&self.key);
        let plaintext = cipher
            .decrypt(nonce, ciphertext_bytes)
            .map_err(|e| AppError::EncryptionError(format!("Decryption failed: {}", e)))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| AppError::EncryptionError(format!("Invalid UTF-8 in decrypted data: {}", e)))
    }
}

impl AesGcmEncryptionService {
    pub fn generate_key() -> String {
        let key = Aes256Gcm::generate_key(OsRng);
        Base64::encode_string(key.as_slice())
    }
}

// Factory function for creating encryption service
pub fn create_encryption_service() -> Result<Arc<dyn EncryptionService>> {
    let service = AesGcmEncryptionService::from_env()?;
    Ok(Arc::new(service))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_encrypt_decrypt() {
        let key = AesGcmEncryptionService::generate_key();
        let service = AesGcmEncryptionService::new(&key).unwrap();
        
        let plaintext = "Hello, World! This is a secret message.";
        let encrypted = service.encrypt(plaintext).await.unwrap();
        let decrypted = service.decrypt(&encrypted).await.unwrap();
        
        assert_eq!(plaintext, decrypted);
    }
    
    #[tokio::test]
    async fn test_encrypt_different_each_time() {
        let key = AesGcmEncryptionService::generate_key();
        let service = AesGcmEncryptionService::new(&key).unwrap();
        
        let plaintext = "Same message";
        let encrypted1 = service.encrypt(plaintext).await.unwrap();
        let encrypted2 = service.encrypt(plaintext).await.unwrap();
        
        // Should be different due to random nonce
        assert_ne!(encrypted1, encrypted2);
        
        // But both should decrypt to the same plaintext
        let decrypted1 = service.decrypt(&encrypted1).await.unwrap();
        let decrypted2 = service.decrypt(&encrypted2).await.unwrap();
        
        assert_eq!(decrypted1, plaintext);
        assert_eq!(decrypted2, plaintext);
    }
    
    #[tokio::test]
    async fn test_invalid_key_length() {
        let short_key = Base64::encode_string(b"short");
        let result = AesGcmEncryptionService::new(&short_key);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::EncryptionError(_)));
    }
    
    #[tokio::test]
    async fn test_invalid_ciphertext() {
        let key = AesGcmEncryptionService::generate_key();
        let service = AesGcmEncryptionService::new(&key).unwrap();
        
        let result = service.decrypt("invalid_base64!").await;
        assert!(result.is_err());
        
        let result = service.decrypt("dGVzdA==").await; // "test" in base64, too short
        assert!(result.is_err());
    }
}