// Valkyrie Integration Configuration
// Task 3.2: Unified Control Plane Integration

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for Valkyrie Protocol integration with RustCI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieIntegrationConfig {
    // Core integration settings
    pub valkyrie_enabled: bool,
    pub integration_mode: IntegrationMode,
    pub backward_compatibility_enabled: bool,
    pub performance_enhancement_enabled: bool,
    
    // Valkyrie engine configuration
    pub valkyrie_listen_address: String,
    pub max_connections: u32,
    #[serde(with = "duration_serde")]
    pub connection_timeout: Duration,
    pub enable_tls: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    
    // Runner adapter configuration
    pub max_concurrent_jobs: u32,
    #[serde(with = "duration_serde")]
    pub dispatch_timeout: Duration,
    pub queue_capacity: usize,
    pub intelligent_routing_enabled: bool,
    
    // Fallback configuration
    pub fallback_mode_enabled: bool,
    #[serde(with = "duration_serde")]
    pub fallback_timeout: Duration,
    pub fallback_retry_attempts: u32,
    
    // Health monitoring
    #[serde(with = "duration_serde")]
    pub health_check_interval: Duration,
    pub health_check_timeout: Duration,
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
    
    // Performance settings
    pub enable_zero_copy: bool,
    pub enable_simd_optimization: bool,
    pub memory_pool_size: usize,
    pub buffer_pool_sizes: Vec<usize>,
    
    // Observability
    pub metrics_enabled: bool,
    #[serde(with = "duration_serde")]
    pub metrics_interval: Duration,
    pub tracing_enabled: bool,
    pub logging_level: LogLevel,
    
    // Configuration management
    pub hot_reload_enabled: bool,
    pub config_validation_strict: bool,
    pub config_backup_enabled: bool,
    
    // Security settings
    pub authentication_required: bool,
    pub authorization_enabled: bool,
    pub audit_logging_enabled: bool,
    pub rate_limiting_enabled: bool,
    
    // Advanced features
    pub adaptive_routing_enabled: bool,
    pub machine_learning_enabled: bool,
    pub predictive_scaling_enabled: bool,
    pub chaos_engineering_enabled: bool,
}

/// Integration mode for Valkyrie Protocol
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegrationMode {
    /// Full Valkyrie integration with all features
    Full,
    /// Hybrid mode with selective feature usage
    Hybrid,
    /// Fallback mode using HTTP when Valkyrie unavailable
    Fallback,
    /// Compatibility mode for legacy systems
    Compatibility,
    /// Disabled - no Valkyrie integration
    Disabled,
}

/// Logging level configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Environment-specific configuration overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub environment: Environment,
    pub overrides: ValkyrieIntegrationConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

/// Feature flags for gradual rollout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_enhanced_api: bool,
    pub enable_performance_monitoring: bool,
    pub enable_advanced_routing: bool,
    pub enable_ml_optimization: bool,
    pub enable_zero_downtime_updates: bool,
}

/// Migration settings for upgrading from legacy systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub migration_enabled: bool,
    pub migration_phase: MigrationPhase,
    pub legacy_api_support_duration: Duration,
    pub data_migration_batch_size: u32,
    pub rollback_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MigrationPhase {
    Planning,
    Preparation,
    Execution,
    Validation,
    Completion,
    Rollback,
}

impl Default for ValkyrieIntegrationConfig {
    fn default() -> Self {
        Self {
            // Core integration settings
            valkyrie_enabled: false,
            integration_mode: IntegrationMode::Disabled,
            backward_compatibility_enabled: true,
            performance_enhancement_enabled: false,
            
            // Valkyrie engine configuration
            valkyrie_listen_address: "127.0.0.1:8080".to_string(),
            max_connections: 1000,
            connection_timeout: Duration::from_secs(30),
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            
            // Runner adapter configuration
            max_concurrent_jobs: 1000,
            dispatch_timeout: Duration::from_micros(100),
            queue_capacity: 10000,
            intelligent_routing_enabled: true,
            
            // Fallback configuration
            fallback_mode_enabled: true,
            fallback_timeout: Duration::from_millis(5000),
            fallback_retry_attempts: 3,
            
            // Health monitoring
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            failure_threshold: 3,
            recovery_threshold: 2,
            
            // Performance settings
            enable_zero_copy: true,
            enable_simd_optimization: true,
            memory_pool_size: 1024 * 1024 * 512, // 512MB
            buffer_pool_sizes: vec![1024, 4096, 16384, 65536], // Various buffer sizes
            
            // Observability
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            tracing_enabled: true,
            logging_level: LogLevel::Info,
            
            // Configuration management
            hot_reload_enabled: false,
            config_validation_strict: true,
            config_backup_enabled: true,
            
            // Security settings
            authentication_required: false,
            authorization_enabled: false,
            audit_logging_enabled: false,
            rate_limiting_enabled: false,
            
            // Advanced features
            adaptive_routing_enabled: false,
            machine_learning_enabled: false,
            predictive_scaling_enabled: false,
            chaos_engineering_enabled: false,
        }
    }
}

impl ValkyrieIntegrationConfig {
    /// Create configuration for development environment
    pub fn development() -> Self {
        Self {
            valkyrie_enabled: true,
            integration_mode: IntegrationMode::Full,
            performance_enhancement_enabled: true,
            logging_level: LogLevel::Debug,
            hot_reload_enabled: true,
            config_validation_strict: false,
            chaos_engineering_enabled: true,
            ..Default::default()
        }
    }
    
    /// Create configuration for testing environment
    pub fn testing() -> Self {
        Self {
            valkyrie_enabled: true,
            integration_mode: IntegrationMode::Full,
            performance_enhancement_enabled: true,
            logging_level: LogLevel::Debug,
            metrics_enabled: true,
            tracing_enabled: true,
            config_validation_strict: true,
            ..Default::default()
        }
    }
    
    /// Create configuration for staging environment
    pub fn staging() -> Self {
        Self {
            valkyrie_enabled: true,
            integration_mode: IntegrationMode::Hybrid,
            performance_enhancement_enabled: true,
            enable_tls: true,
            authentication_required: true,
            authorization_enabled: true,
            audit_logging_enabled: true,
            logging_level: LogLevel::Info,
            adaptive_routing_enabled: true,
            ..Default::default()
        }
    }
    
    /// Create configuration for production environment
    pub fn production() -> Self {
        Self {
            valkyrie_enabled: true,
            integration_mode: IntegrationMode::Full,
            performance_enhancement_enabled: true,
            enable_tls: true,
            authentication_required: true,
            authorization_enabled: true,
            audit_logging_enabled: true,
            rate_limiting_enabled: true,
            logging_level: LogLevel::Warn,
            hot_reload_enabled: false,
            config_validation_strict: true,
            config_backup_enabled: true,
            adaptive_routing_enabled: true,
            machine_learning_enabled: true,
            predictive_scaling_enabled: true,
            chaos_engineering_enabled: false,
            ..Default::default()
        }
    }
    
    /// Validate configuration consistency
    pub fn validate(&self) -> Result<(), String> {
        // Validate Valkyrie-specific settings
        if self.valkyrie_enabled {
            if self.valkyrie_listen_address.is_empty() {
                return Err("Valkyrie listen address cannot be empty when enabled".to_string());
            }
            
            if self.max_connections == 0 {
                return Err("Max connections must be greater than 0".to_string());
            }
            
            if self.max_concurrent_jobs == 0 {
                return Err("Max concurrent jobs must be greater than 0".to_string());
            }
            
            if self.dispatch_timeout.as_millis() == 0 {
                return Err("Dispatch timeout must be greater than 0".to_string());
            }
        }
        
        // Validate TLS settings
        if self.enable_tls {
            if self.tls_cert_path.is_none() || self.tls_key_path.is_none() {
                return Err("TLS certificate and key paths must be specified when TLS is enabled".to_string());
            }
        }
        
        // Validate fallback settings
        if self.fallback_mode_enabled {
            if self.fallback_timeout.as_millis() == 0 {
                return Err("Fallback timeout must be greater than 0".to_string());
            }
            
            if self.fallback_retry_attempts == 0 {
                return Err("Fallback retry attempts must be greater than 0".to_string());
            }
        }
        
        // Validate health monitoring settings
        if self.health_check_interval.as_millis() == 0 {
            return Err("Health check interval must be greater than 0".to_string());
        }
        
        if self.failure_threshold == 0 || self.recovery_threshold == 0 {
            return Err("Failure and recovery thresholds must be greater than 0".to_string());
        }
        
        // Validate performance settings
        if self.memory_pool_size == 0 {
            return Err("Memory pool size must be greater than 0".to_string());
        }
        
        if self.buffer_pool_sizes.is_empty() {
            return Err("Buffer pool sizes cannot be empty".to_string());
        }
        
        // Validate observability settings
        if self.metrics_enabled && self.metrics_interval.as_millis() == 0 {
            return Err("Metrics interval must be greater than 0 when metrics are enabled".to_string());
        }
        
        Ok(())
    }
    
    /// Apply environment-specific overrides
    pub fn apply_environment_overrides(&mut self, env_config: &EnvironmentConfig) {
        // Apply overrides based on environment
        match env_config.environment {
            Environment::Development => {
                self.logging_level = LogLevel::Debug;
                self.hot_reload_enabled = true;
                self.config_validation_strict = false;
            }
            Environment::Testing => {
                self.logging_level = LogLevel::Debug;
                self.metrics_enabled = true;
                self.tracing_enabled = true;
            }
            Environment::Staging => {
                self.enable_tls = true;
                self.authentication_required = true;
                self.audit_logging_enabled = true;
            }
            Environment::Production => {
                self.enable_tls = true;
                self.authentication_required = true;
                self.authorization_enabled = true;
                self.audit_logging_enabled = true;
                self.rate_limiting_enabled = true;
                self.logging_level = LogLevel::Warn;
                self.hot_reload_enabled = false;
            }
        }
        
        // Apply specific overrides
        // This would merge the override config with the current config
        // For now, we'll just update a few key fields as an example
        if env_config.overrides.valkyrie_enabled != self.valkyrie_enabled {
            self.valkyrie_enabled = env_config.overrides.valkyrie_enabled;
        }
    }
    
    /// Get configuration for specific integration mode
    pub fn for_integration_mode(mode: IntegrationMode) -> Self {
        let mut config = Self::default();
        config.integration_mode = mode.clone();
        
        match mode {
            IntegrationMode::Full => {
                config.valkyrie_enabled = true;
                config.performance_enhancement_enabled = true;
                config.intelligent_routing_enabled = true;
                config.adaptive_routing_enabled = true;
            }
            IntegrationMode::Hybrid => {
                config.valkyrie_enabled = true;
                config.performance_enhancement_enabled = true;
                config.fallback_mode_enabled = true;
            }
            IntegrationMode::Fallback => {
                config.valkyrie_enabled = true;
                config.fallback_mode_enabled = true;
                config.performance_enhancement_enabled = false;
            }
            IntegrationMode::Compatibility => {
                config.valkyrie_enabled = true;
                config.backward_compatibility_enabled = true;
                config.performance_enhancement_enabled = false;
            }
            IntegrationMode::Disabled => {
                config.valkyrie_enabled = false;
            }
        }
        
        config
    }
    
    /// Check if zero-downtime updates are supported
    pub fn supports_zero_downtime_updates(&self) -> bool {
        self.valkyrie_enabled && 
        self.hot_reload_enabled && 
        self.fallback_mode_enabled
    }
    
    /// Get performance optimization level
    pub fn get_performance_level(&self) -> PerformanceLevel {
        if !self.valkyrie_enabled {
            return PerformanceLevel::Disabled;
        }
        
        if self.performance_enhancement_enabled && 
           self.enable_zero_copy && 
           self.enable_simd_optimization &&
           self.adaptive_routing_enabled {
            PerformanceLevel::Maximum
        } else if self.performance_enhancement_enabled {
            PerformanceLevel::Enhanced
        } else {
            PerformanceLevel::Standard
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceLevel {
    Disabled,
    Standard,
    Enhanced,
    Maximum,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enable_enhanced_api: true,
            enable_performance_monitoring: true,
            enable_advanced_routing: false,
            enable_ml_optimization: false,
            enable_zero_downtime_updates: false,
        }
    }
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            migration_enabled: false,
            migration_phase: MigrationPhase::Planning,
            legacy_api_support_duration: Duration::from_secs(86400 * 30), // 30 days
            data_migration_batch_size: 1000,
            rollback_enabled: true,
        }
    }
}

// Custom serde module for Duration
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = ValkyrieIntegrationConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_development_config() {
        let config = ValkyrieIntegrationConfig::development();
        assert!(config.valkyrie_enabled);
        assert_eq!(config.integration_mode, IntegrationMode::Full);
        assert_eq!(config.logging_level, LogLevel::Debug);
        assert!(config.hot_reload_enabled);
    }

    #[test]
    fn test_production_config() {
        let config = ValkyrieIntegrationConfig::production();
        assert!(config.valkyrie_enabled);
        assert!(config.enable_tls);
        assert!(config.authentication_required);
        assert!(config.authorization_enabled);
        assert_eq!(config.logging_level, LogLevel::Warn);
        assert!(!config.hot_reload_enabled);
    }

    #[test]
    fn test_config_validation_errors() {
        let mut config = ValkyrieIntegrationConfig::default();
        config.valkyrie_enabled = true;
        config.valkyrie_listen_address = "".to_string();
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_level() {
        let config = ValkyrieIntegrationConfig::production();
        assert_eq!(config.get_performance_level(), PerformanceLevel::Maximum);
        
        let config = ValkyrieIntegrationConfig::default();
        assert_eq!(config.get_performance_level(), PerformanceLevel::Disabled);
    }

    #[test]
    fn test_zero_downtime_support() {
        let mut config = ValkyrieIntegrationConfig::default();
        config.valkyrie_enabled = true;
        config.hot_reload_enabled = true;
        config.fallback_mode_enabled = true;
        
        assert!(config.supports_zero_downtime_updates());
    }
}