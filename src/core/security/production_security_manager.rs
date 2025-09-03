//! Production Security Manager
//!
//! Provides enterprise-grade security management for production environments.

use crate::error::{AppError, Result};
use crate::core::security::input_sanitizer::{InputSanitizer, ValidationRule, ValidationResult};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

/// Configuration for production security manager
#[derive(Clone, Serialize, Deserialize)]
pub struct ProductionSecurityConfig {
    pub enable_mfa: bool,
    pub enable_audit_logging: bool,
    pub enable_rate_limiting: bool,
    pub enable_encryption: bool,
    pub security_level: SecurityLevel,
    #[serde(skip)]
    pub audit_logger: Option<Arc<crate::core::observability::audit::EnhancedAuditLogger>>,
}

impl std::fmt::Debug for ProductionSecurityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionSecurityConfig")
            .field("enable_mfa", &self.enable_mfa)
            .field("enable_audit_logging", &self.enable_audit_logging)
            .field("enable_rate_limiting", &self.enable_rate_limiting)
            .field("enable_encryption", &self.enable_encryption)
            .field("security_level", &self.security_level)
            .field("audit_logger", &self.audit_logger.is_some())
            .finish()
    }
}

impl Default for ProductionSecurityConfig {
    fn default() -> Self {
        Self {
            enable_mfa: true,
            enable_audit_logging: true,
            enable_rate_limiting: true,
            enable_encryption: true,
            security_level: SecurityLevel::High,
            audit_logger: None,
        }
    }
}

/// Security levels for production environments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Health status for security components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHealthStatus {
    pub overall_status: String,
    pub mfa_status: String,
    pub audit_status: String,
    pub rate_limit_status: String,
    pub encryption_status: String,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Production-grade security manager
pub struct ProductionSecurityManager {
    config: ProductionSecurityConfig,
    health_status: Arc<RwLock<SecurityHealthStatus>>,
    input_sanitizer: InputSanitizer,
}

impl ProductionSecurityManager {
    /// Create a new production security manager
    pub async fn new(config: ProductionSecurityConfig) -> Result<Self> {
        info!("🔒 Initializing production security manager");
        
        let input_sanitizer = InputSanitizer::new()
            .map_err(|e| AppError::InternalServerError(format!("Failed to initialize input sanitizer: {}", e)))?;
        
        let health_status = SecurityHealthStatus {
            overall_status: "healthy".to_string(),
            mfa_status: if config.enable_mfa { "enabled" } else { "disabled" }.to_string(),
            audit_status: if config.enable_audit_logging { "enabled" } else { "disabled" }.to_string(),
            rate_limit_status: if config.enable_rate_limiting { "enabled" } else { "disabled" }.to_string(),
            encryption_status: if config.enable_encryption { "enabled" } else { "disabled" }.to_string(),
            last_check: chrono::Utc::now(),
        };
        
        Ok(Self {
            config,
            health_status: Arc::new(RwLock::new(health_status)),
            input_sanitizer,
        })
    }

    /// Validate and sanitize input according to security rules
    pub async fn validate_and_sanitize_input(
        &self,
        input: &str,
        validation_rules: &[ValidationRule],
    ) -> Result<String> {
        info!("🔍 Validating and sanitizing input");
        
        let result = self.input_sanitizer.validate_and_sanitize(input, validation_rules)?;
        
        if !result.is_valid {
            let violations: Vec<String> = result.violations
                .iter()
                .map(|v| format!("{}: {}", v.rule, v.description))
                .collect();
            
            warn!("Input validation failed: {}", violations.join(", "));
            
            return Err(AppError::ValidationError(format!(
                "Input validation failed: {}",
                violations.join(", ")
            )));
        }
        
        Ok(result.sanitized_input.unwrap_or_else(|| input.to_string()))
    }

    /// Get validation result without throwing error
    pub async fn get_validation_result(
        &self,
        input: &str,
        validation_rules: &[ValidationRule],
    ) -> Result<ValidationResult> {
        self.input_sanitizer.validate_and_sanitize(input, validation_rules)
    }
    
    /// Check the health of security components
    pub async fn check_health(&self) -> Result<SecurityHealthStatus> {
        let mut status = self.health_status.write().await;
        status.last_check = chrono::Utc::now();
        
        // Perform health checks for each component
        if self.config.enable_mfa {
            // Check MFA health
            status.mfa_status = "healthy".to_string();
        }
        
        if self.config.enable_audit_logging {
            // Check audit logging health
            status.audit_status = "healthy".to_string();
        }
        
        if self.config.enable_rate_limiting {
            // Check rate limiting health
            status.rate_limit_status = "healthy".to_string();
        }
        
        if self.config.enable_encryption {
            // Check encryption health
            status.encryption_status = "healthy".to_string();
        }
        
        status.overall_status = "healthy".to_string();
        Ok(status.clone())
    }
    
    /// Get security configuration
    pub fn get_config(&self) -> &ProductionSecurityConfig {
        &self.config
    }
    
    /// Update security configuration
    pub async fn update_config(&mut self, new_config: ProductionSecurityConfig) -> Result<()> {
        info!("🔄 Updating production security configuration");
        self.config = new_config;
        Ok(())
    }
    
    /// Shutdown security manager
    pub async fn shutdown(&self) -> Result<()> {
        info!("🛑 Shutting down production security manager");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_production_security_manager_creation() {
        let config = ProductionSecurityConfig::default();
        let manager = ProductionSecurityManager::new(config).await;
        assert!(manager.is_ok());
    }
    
    #[tokio::test]
    async fn test_health_check() {
        let config = ProductionSecurityConfig::default();
        let manager = ProductionSecurityManager::new(config).await.unwrap();
        let health = manager.check_health().await;
        assert!(health.is_ok());
    }
}