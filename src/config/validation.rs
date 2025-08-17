//! Configuration validation for Valkyrie Protocol
//! 
//! This module provides comprehensive validation for Valkyrie configuration
//! including JSON Schema validation, semantic validation, and testing tools.

use crate::config::valkyrie_config::ValkyrieConfig;
use crate::error::{AppError, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use tracing::{debug, info, warn};

/// Configuration validator
pub struct ConfigValidator {
    /// JSON Schema for validation
    schema: Option<Value>,
    /// Validation rules
    rules: Vec<Box<dyn ValidationRule>>,
}

/// Validation rule trait
pub trait ValidationRule: Send + Sync {
    /// Validate configuration
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>>;
    
    /// Get rule name
    fn name(&self) -> &str;
    
    /// Get rule description
    fn description(&self) -> &str;
}

/// Validation issue severity
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    /// Error - configuration is invalid
    Error,
    /// Warning - configuration may cause issues
    Warning,
    /// Info - informational message
    Info,
}

/// Validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Issue severity
    pub severity: ValidationSeverity,
    /// Configuration path where issue was found
    pub path: String,
    /// Issue message
    pub message: String,
    /// Suggested fix
    pub suggestion: Option<String>,
}

/// Validation result
#[derive(Debug)]
pub struct ValidationResult {
    /// Whether validation passed
    pub valid: bool,
    /// List of validation issues
    pub issues: Vec<ValidationIssue>,
    /// Validation summary
    pub summary: ValidationSummary,
}

/// Validation summary
#[derive(Debug)]
pub struct ValidationSummary {
    /// Number of errors
    pub error_count: usize,
    /// Number of warnings
    pub warning_count: usize,
    /// Number of info messages
    pub info_count: usize,
    /// Total issues
    pub total_issues: usize,
}

impl ConfigValidator {
    /// Create a new configuration validator
    pub fn new() -> Self {
        let mut validator = Self {
            schema: None,
            rules: Vec::new(),
        };
        
        // Add default validation rules
        validator.add_default_rules();
        
        validator
    }
    
    /// Validate configuration for a specific environment
    pub fn validate_for_environment(&self, config: &ValkyrieConfig, environment: &str) -> Result<ValidationResult> {
        info!("Validating configuration for environment: {}", environment);
        
        // Create a copy of the config with the specified environment
        let mut env_config = config.clone();
        env_config.global.environment = environment.to_string();
        
        // Apply environment-specific defaults if needed
        match environment {
            "development" => {
                if env_config.global.log_level.is_empty() {
                    env_config.global.log_level = "debug".to_string();
                }
            }
            "staging" => {
                if env_config.global.log_level.is_empty() {
                    env_config.global.log_level = "info".to_string();
                }
            }
            "production" => {
                if env_config.global.log_level.is_empty() {
                    env_config.global.log_level = "warn".to_string();
                }
            }
            _ => {}
        }
        
        // Validate the environment-specific config
        self.validate(&env_config)
    }
    
    /// Load JSON Schema from file
    pub fn load_schema<P: AsRef<Path>>(&mut self, schema_path: P) -> Result<()> {
        let schema_path = schema_path.as_ref();
        info!("Loading JSON Schema from: {}", schema_path.display());
        
        let schema_content = std::fs::read_to_string(schema_path)
            .map_err(|e| AppError::ConfigurationError(format!("Failed to read schema file: {}", e)))?;
        
        let schema: Value = serde_json::from_str(&schema_content)
            .map_err(|e| AppError::ConfigurationError(format!("Failed to parse schema: {}", e)))?;
        
        self.schema = Some(schema);
        debug!("JSON Schema loaded successfully");
        Ok(())
    }
    
    /// Add a validation rule
    pub fn add_rule(&mut self, rule: Box<dyn ValidationRule>) {
        debug!("Adding validation rule: {}", rule.name());
        self.rules.push(rule);
    }
    
    /// Validate configuration
    pub fn validate(&self, config: &ValkyrieConfig) -> Result<ValidationResult> {
        info!("Validating Valkyrie configuration");
        let mut all_issues = Vec::new();
        
        // JSON Schema validation
        if let Some(ref schema) = self.schema {
            match self.validate_against_schema(config, schema) {
                Ok(mut schema_issues) => all_issues.append(&mut schema_issues),
                Err(e) => {
                    warn!("Schema validation failed: {}", e);
                    all_issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        path: "root".to_string(),
                        message: format!("Schema validation failed: {}", e),
                        suggestion: Some("Check configuration format against schema".to_string()),
                    });
                }
            }
        }
        
        // Custom validation rules
        for rule in &self.rules {
            debug!("Running validation rule: {}", rule.name());
            match rule.validate(config) {
                Ok(mut rule_issues) => all_issues.append(&mut rule_issues),
                Err(e) => {
                    warn!("Validation rule '{}' failed: {}", rule.name(), e);
                    all_issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        path: "validation".to_string(),
                        message: format!("Rule '{}' failed: {}", rule.name(), e),
                        suggestion: None,
                    });
                }
            }
        }
        
        // Calculate summary
        let error_count = all_issues.iter().filter(|i| i.severity == ValidationSeverity::Error).count();
        let warning_count = all_issues.iter().filter(|i| i.severity == ValidationSeverity::Warning).count();
        let info_count = all_issues.iter().filter(|i| i.severity == ValidationSeverity::Info).count();
        
        let summary = ValidationSummary {
            error_count,
            warning_count,
            info_count,
            total_issues: all_issues.len(),
        };
        
        let valid = error_count == 0;
        
        info!("Validation completed: {} errors, {} warnings, {} info", 
              error_count, warning_count, info_count);
        
        Ok(ValidationResult {
            valid,
            issues: all_issues,
            summary,
        })
    }
    
    /// Validate configuration against JSON Schema
    fn validate_against_schema(&self, config: &ValkyrieConfig, _schema: &Value) -> Result<Vec<ValidationIssue>> {
        // This is a simplified implementation
        // In a real system, you'd use a JSON Schema validation library like `jsonschema`
        debug!("Performing JSON Schema validation");
        
        let mut issues = Vec::new();
        
        // Basic structure validation
        if config.global.environment.is_empty() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "global.environment".to_string(),
                message: "Environment must not be empty".to_string(),
                suggestion: Some("Set environment to 'development', 'staging', or 'production'".to_string()),
            });
        }
        
        if config.global.log_level.is_empty() {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "global.log_level".to_string(),
                message: "Log level must not be empty".to_string(),
                suggestion: Some("Set log_level to 'trace', 'debug', 'info', 'warn', or 'error'".to_string()),
            });
        }
        
        Ok(issues)
    }
    
    /// Add default validation rules
    fn add_default_rules(&mut self) {
        self.add_rule(Box::new(EnvironmentValidationRule));
        self.add_rule(Box::new(ServerValidationRule));
        self.add_rule(Box::new(ClientValidationRule));
        self.add_rule(Box::new(SecurityValidationRule));
        self.add_rule(Box::new(TransportValidationRule));
        self.add_rule(Box::new(PerformanceValidationRule));
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment validation rule
struct EnvironmentValidationRule;

impl ValidationRule for EnvironmentValidationRule {
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        
        // Validate environment
        let valid_environments = ["development", "staging", "production"];
        if !valid_environments.contains(&config.global.environment.as_str()) {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "global.environment".to_string(),
                message: format!("Invalid environment: {}", config.global.environment),
                suggestion: Some("Use 'development', 'staging', or 'production'".to_string()),
            });
        }
        
        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&config.global.log_level.as_str()) {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                path: "global.log_level".to_string(),
                message: format!("Invalid log level: {}", config.global.log_level),
                suggestion: Some("Use 'trace', 'debug', 'info', 'warn', or 'error'".to_string()),
            });
        }
        
        // Environment-specific warnings
        if config.global.environment == "production" {
            if config.global.log_level == "debug" || config.global.log_level == "trace" {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    path: "global.log_level".to_string(),
                    message: "Debug/trace logging in production may impact performance".to_string(),
                    suggestion: Some("Consider using 'info' or 'warn' log level in production".to_string()),
                });
            }
        }
        
        Ok(issues)
    }
    
    fn name(&self) -> &str {
        "environment"
    }
    
    fn description(&self) -> &str {
        "Validates environment and logging configuration"
    }
}

/// Server validation rule
struct ServerValidationRule;

impl ValidationRule for ServerValidationRule {
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        
        if let Some(ref server) = config.server {
            // Validate port range
            if server.port == 0 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "server.port".to_string(),
                    message: "Server port cannot be 0".to_string(),
                    suggestion: Some("Use a valid port number (1-65535)".to_string()),
                });
            }
            
            // Validate bind address
            if server.bind_address.is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "server.bind_address".to_string(),
                    message: "Server bind address cannot be empty".to_string(),
                    suggestion: Some("Use '0.0.0.0' for all interfaces or specific IP".to_string()),
                });
            }
            
            // TLS validation
            if server.enable_tls {
                if server.tls_cert_path.is_none() {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        path: "server.tls_cert_path".to_string(),
                        message: "TLS certificate path required when TLS is enabled".to_string(),
                        suggestion: Some("Provide path to TLS certificate file".to_string()),
                    });
                }
                
                if server.tls_key_path.is_none() {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        path: "server.tls_key_path".to_string(),
                        message: "TLS private key path required when TLS is enabled".to_string(),
                        suggestion: Some("Provide path to TLS private key file".to_string()),
                    });
                }
            }
            
            // Performance warnings
            if server.max_connections > 50000 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    path: "server.max_connections".to_string(),
                    message: "Very high connection limit may impact performance".to_string(),
                    suggestion: Some("Consider system limits and available resources".to_string()),
                });
            }
        }
        
        Ok(issues)
    }
    
    fn name(&self) -> &str {
        "server"
    }
    
    fn description(&self) -> &str {
        "Validates server configuration"
    }
}

/// Client validation rule
struct ClientValidationRule;

impl ValidationRule for ClientValidationRule {
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        
        if let Some(ref client) = config.client {
            // Validate endpoint
            if client.endpoint.is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "client.endpoint".to_string(),
                    message: "Client endpoint cannot be empty".to_string(),
                    suggestion: Some("Provide a valid server endpoint".to_string()),
                });
            }
            
            // Validate timeouts
            if client.connect_timeout_ms < 100 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    path: "client.connect_timeout_ms".to_string(),
                    message: "Very short connection timeout may cause failures".to_string(),
                    suggestion: Some("Consider using at least 1000ms".to_string()),
                });
            }
            
            if client.request_timeout_ms < 1000 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    path: "client.request_timeout_ms".to_string(),
                    message: "Very short request timeout may cause failures".to_string(),
                    suggestion: Some("Consider using at least 5000ms".to_string()),
                });
            }
            
            // Validate connection pool
            if client.enable_pooling && client.max_connections == 0 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "client.max_connections".to_string(),
                    message: "Max connections cannot be 0 when pooling is enabled".to_string(),
                    suggestion: Some("Set max_connections to at least 1".to_string()),
                });
            }
        }
        
        Ok(issues)
    }
    
    fn name(&self) -> &str {
        "client"
    }
    
    fn description(&self) -> &str {
        "Validates client configuration"
    }
}

/// Security validation rule
struct SecurityValidationRule;

impl ValidationRule for SecurityValidationRule {
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        
        if let Some(ref security) = config.security {
            // Production security warnings
            if config.global.environment == "production" {
                if security.enable_encryption == Some(false) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        path: "security.enable_encryption".to_string(),
                        message: "Encryption disabled in production environment".to_string(),
                        suggestion: Some("Enable encryption for production deployments".to_string()),
                    });
                }
                
                if security.enable_authentication == Some(false) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        path: "security.enable_authentication".to_string(),
                        message: "Authentication disabled in production environment".to_string(),
                        suggestion: Some("Enable authentication for production deployments".to_string()),
                    });
                }
            }
            
            // JWT validation
            if let Some(ref jwt) = security.jwt {
                if jwt.secret_key.len() < 32 {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        path: "security.jwt.secret_key".to_string(),
                        message: "JWT secret key too short".to_string(),
                        suggestion: Some("Use at least 32 characters for JWT secret".to_string()),
                    });
                }
                
                if jwt.secret_key.starts_with("${") && config.global.environment == "production" {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Info,
                        path: "security.jwt.secret_key".to_string(),
                        message: "JWT secret uses environment variable (recommended)".to_string(),
                        suggestion: None,
                    });
                }
            }
            
            // API Key validation
            if let Some(ref api_key) = security.api_key {
                if api_key.valid_keys.is_empty() {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Error,
                        path: "security.api_key.valid_keys".to_string(),
                        message: "No valid API keys configured".to_string(),
                        suggestion: Some("Add at least one valid API key".to_string()),
                    });
                }
                
                for (i, key) in api_key.valid_keys.iter().enumerate() {
                    if key.len() < 16 {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Warning,
                            path: format!("security.api_key.valid_keys[{}]", i),
                            message: "API key too short".to_string(),
                            suggestion: Some("Use at least 16 characters for API keys".to_string()),
                        });
                    }
                }
            }
        }
        
        Ok(issues)
    }
    
    fn name(&self) -> &str {
        "security"
    }
    
    fn description(&self) -> &str {
        "Validates security configuration"
    }
}

/// Transport validation rule
struct TransportValidationRule;

impl ValidationRule for TransportValidationRule {
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        
        if let Some(ref transport) = config.transport {
            // Validate enabled transports
            if transport.enabled_transports.is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "transport.enabled_transports".to_string(),
                    message: "No transports enabled".to_string(),
                    suggestion: Some("Enable at least one transport (tcp, quic, unix, websocket)".to_string()),
                });
            }
            
            // Validate default transport
            if !transport.enabled_transports.contains(&transport.default_transport) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    path: "transport.default_transport".to_string(),
                    message: "Default transport not in enabled transports list".to_string(),
                    suggestion: Some("Ensure default transport is enabled".to_string()),
                });
            }
            
            // Check for duplicate transports
            let mut seen = HashSet::new();
            for transport_name in &transport.enabled_transports {
                if !seen.insert(transport_name) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        path: "transport.enabled_transports".to_string(),
                        message: format!("Duplicate transport: {}", transport_name),
                        suggestion: Some("Remove duplicate transport entries".to_string()),
                    });
                }
            }
            
            // Unix socket validation
            if transport.enabled_transports.contains(&"unix".to_string()) {
                if let Some(ref unix) = transport.unix {
                    if unix.socket_path.is_empty() {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Error,
                            path: "transport.unix.socket_path".to_string(),
                            message: "Unix socket path cannot be empty".to_string(),
                            suggestion: Some("Provide a valid socket file path".to_string()),
                        });
                    }
                }
            }
        }
        
        Ok(issues)
    }
    
    fn name(&self) -> &str {
        "transport"
    }
    
    fn description(&self) -> &str {
        "Validates transport configuration"
    }
}

/// Performance validation rule
struct PerformanceValidationRule;

impl ValidationRule for PerformanceValidationRule {
    fn validate(&self, config: &ValkyrieConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        
        if let Some(ref performance) = config.performance {
            // Thread pool validation
            if let Some(ref thread_pool) = performance.thread_pool {
                if let Some(max_threads) = thread_pool.max_threads {
                    if max_threads > 10000 {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Warning,
                            path: "performance.thread_pool.max_threads".to_string(),
                            message: "Very high thread count may impact performance".to_string(),
                            suggestion: Some("Consider system resources and workload characteristics".to_string()),
                        });
                    }
                }
            }
            
            // Memory validation
            if let Some(ref memory) = performance.memory {
                if let Some(max_buffer_size) = memory.max_buffer_size {
                    if max_buffer_size > 100 * 1024 * 1024 { // 100MB
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Warning,
                            path: "performance.memory.max_buffer_size".to_string(),
                            message: "Very large buffer size may impact memory usage".to_string(),
                            suggestion: Some("Consider available memory and typical message sizes".to_string()),
                        });
                    }
                }
            }
        }
        
        Ok(issues)
    }
    
    fn name(&self) -> &str {
        "performance"
    }
    
    fn description(&self) -> &str {
        "Validates performance configuration"
    }
}

/// Environment-specific configuration loader
pub struct EnvironmentConfigLoader;

impl EnvironmentConfigLoader {
    /// Load configuration for a specific environment
    pub fn load_for_environment<P: AsRef<Path>>(config_path: P, environment: &str) -> Result<ValkyrieConfig> {
        let config_path = config_path.as_ref();
        info!("Loading configuration for environment: {}", environment);
        
        // Load base configuration
        let mut config = ValkyrieConfig::from_file(config_path)?;
        
        // Override environment
        config.global.environment = environment.to_string();
        
        // Apply environment-specific defaults
        match environment {
            "development" => {
                if config.global.log_level.is_empty() {
                    config.global.log_level = "debug".to_string();
                }
                if !config.global.enable_metrics {
                    config.global.enable_metrics = false;
                }
            }
            "staging" => {
                if config.global.log_level.is_empty() {
                    config.global.log_level = "info".to_string();
                }
                config.global.enable_metrics = true;
            }
            "production" => {
                if config.global.log_level.is_empty() {
                    config.global.log_level = "warn".to_string();
                }
                config.global.enable_metrics = true;
            }
            _ => {}
        }
        
        Ok(config)
    }
}

/// Configuration testing utilities
pub struct ConfigTester;

impl ConfigTester {
    /// Test configuration by attempting to load and validate it
    pub fn test_config_file<P: AsRef<Path>>(config_path: P) -> Result<ValidationResult> {
        let config_path = config_path.as_ref();
        info!("Testing configuration file: {}", config_path.display());
        
        // Load configuration
        let config = ValkyrieConfig::from_file(config_path)?;
        
        // Create validator and load schema
        let mut validator = ConfigValidator::new();
        
        // Try to load schema if it exists
        let schema_path = config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("schemas/valkyrie.json");
        
        if schema_path.exists() {
            validator.load_schema(&schema_path)?;
        }
        
        // Validate configuration
        validator.validate(&config)
    }
    
    /// Test configuration with a specific environment
    pub fn test_config_with_env<P: AsRef<Path>>(config_path: P, environment: &str) -> Result<ValidationResult> {
        let config_path = config_path.as_ref();
        info!("Testing configuration file: {} for environment: {}", config_path.display(), environment);
        
        // Load configuration
        let config = ValkyrieConfig::from_file(config_path)?;
        
        // Create validator and load schema
        let mut validator = ConfigValidator::new();
        
        // Try to load schema if it exists
        let schema_path = config_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("schemas/valkyrie.json");
        
        if schema_path.exists() {
            validator.load_schema(&schema_path)?;
        }
        
        // Validate configuration for specific environment
        validator.validate_for_environment(&config, environment)
    }
    
    /// Generate sample configurations for all environments
    pub fn generate_sample_configs<P: AsRef<Path>>(output_dir: P) -> Result<()> {
        let output_dir = output_dir.as_ref();
        info!("Generating sample configurations in: {}", output_dir.display());
        
        std::fs::create_dir_all(output_dir)
            .map_err(|e| AppError::ConfigurationError(format!("Failed to create output directory: {}", e)))?;
        
        let environments = ["development", "staging", "production"];
        
        for env in &environments {
            let mut config = ValkyrieConfig::default();
            config.global.environment = env.to_string();
            
            // Environment-specific settings
            match *env {
                "development" => {
                    config.global.log_level = "debug".to_string();
                    config.global.enable_metrics = false;
                }
                "staging" => {
                    config.global.log_level = "info".to_string();
                    config.global.enable_metrics = true;
                }
                "production" => {
                    config.global.log_level = "warn".to_string();
                    config.global.enable_metrics = true;
                }
                _ => {}
            }
            
            let filename = format!("valkyrie-{}.yaml", env);
            let output_path = output_dir.join(&filename);
            
            config.to_file(&output_path)?;
            info!("Generated sample configuration: {}", output_path.display());
        }
        
        Ok(())
    }
    
    /// Generate test configuration for different environments
    pub fn generate_test_configs() -> Result<()> {
        info!("Generating test configurations");
        
        let environments = ["development", "staging", "production"];
        
        for env in &environments {
            let mut config = ValkyrieConfig::default();
            config.global.environment = env.to_string();
            
            // Environment-specific settings
            match *env {
                "development" => {
                    config.global.log_level = "debug".to_string();
                    config.global.enable_metrics = false;
                }
                "staging" => {
                    config.global.log_level = "info".to_string();
                    config.global.enable_metrics = true;
                }
                "production" => {
                    config.global.log_level = "warn".to_string();
                    config.global.enable_metrics = true;
                }
                _ => {}
            }
            
            let filename = format!("config/test-{}.yaml", env);
            config.to_file(&filename)?;
            info!("Generated test configuration: {}", filename);
        }
        
        Ok(())
    }
}