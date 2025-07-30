use crate::error::{AppError, Result};
use base64ct::Encoding;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Configuration validation framework
pub struct ConfigValidator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigValidator {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn validate<T>(&self, config: &T) -> Result<ValidationReport>
    where
        T: Serialize,
    {
        let config_value = serde_json::to_value(config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize config: {}", e)))?;

        let mut report = ValidationReport::new();

        for rule in &self.rules {
            match rule.validate(&config_value) {
                Ok(()) => {
                    report.add_success(rule.name().to_string());
                }
                Err(e) => {
                    report.add_error(rule.name().to_string(), e.to_string());
                }
            }
        }

        debug!(
            total_rules = self.rules.len(),
            passed = report.successes.len(),
            failed = report.errors.len(),
            "🔍 Configuration validation completed"
        );

        Ok(report)
    }
}

/// Validation rule trait
pub trait ValidationRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, config: &serde_json::Value) -> Result<()>;
    fn description(&self) -> &'static str {
        "No description provided"
    }
}

/// Validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub successes: Vec<String>,
    pub errors: HashMap<String, String>,
    pub warnings: HashMap<String, String>,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            successes: Vec::new(),
            errors: HashMap::new(),
            warnings: HashMap::new(),
        }
    }

    pub fn add_success(&mut self, rule_name: String) {
        self.successes.push(rule_name);
    }

    pub fn add_error(&mut self, rule_name: String, error: String) {
        self.errors.insert(rule_name, error);
    }

    pub fn add_warning(&mut self, rule_name: String, warning: String) {
        self.warnings.insert(rule_name, warning);
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Required field validation rule
pub struct RequiredFieldRule {
    field_path: String,
    field_name: &'static str,
}

impl RequiredFieldRule {
    pub fn new(field_path: &str, field_name: &'static str) -> Self {
        Self {
            field_path: field_path.to_string(),
            field_name,
        }
    }
}

impl ValidationRule for RequiredFieldRule {
    fn name(&self) -> &'static str {
        self.field_name
    }

    fn validate(&self, config: &serde_json::Value) -> Result<()> {
        let value = self.get_nested_value(config, &self.field_path)?;

        match value {
            serde_json::Value::String(s) if s.is_empty() => Err(AppError::ConfigError(format!(
                "{} cannot be empty",
                self.field_name
            ))),
            serde_json::Value::Null => Err(AppError::ConfigError(format!(
                "{} is required",
                self.field_name
            ))),
            _ => Ok(()),
        }
    }

    fn description(&self) -> &'static str {
        "Validates that required fields are present and not empty"
    }
}

impl RequiredFieldRule {
    fn get_nested_value<'a>(
        &self,
        config: &'a serde_json::Value,
        path: &str,
    ) -> Result<&'a serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = config;

        for part in parts {
            current = current.get(part).ok_or_else(|| {
                AppError::ConfigError(format!("Field '{}' not found in config", path))
            })?;
        }

        Ok(current)
    }
}

/// Port range validation rule
pub struct PortRangeRule {
    field_path: String,
}

impl PortRangeRule {
    pub fn new(field_path: &str) -> Self {
        Self {
            field_path: field_path.to_string(),
        }
    }
}

impl ValidationRule for PortRangeRule {
    fn name(&self) -> &'static str {
        "port_range"
    }

    fn validate(&self, config: &serde_json::Value) -> Result<()> {
        let value = config
            .pointer(&format!("/{}", self.field_path.replace('.', "/")))
            .ok_or_else(|| {
                AppError::ConfigError(format!("Port field '{}' not found", self.field_path))
            })?;

        if let Some(port) = value.as_u64() {
            if port == 0 || port > 65535 {
                return Err(AppError::ConfigError(format!(
                    "Port {} is out of valid range (1-65535)",
                    port
                )));
            }
        } else {
            return Err(AppError::ConfigError(format!(
                "Port field '{}' must be a number",
                self.field_path
            )));
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Validates that port numbers are within valid range (1-65535)"
    }
}

/// URL format validation rule
pub struct UrlFormatRule {
    field_path: String,
    schemes: Vec<String>,
}

impl UrlFormatRule {
    pub fn new(field_path: &str, schemes: Vec<&str>) -> Self {
        Self {
            field_path: field_path.to_string(),
            schemes: schemes.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl ValidationRule for UrlFormatRule {
    fn name(&self) -> &'static str {
        "url_format"
    }

    fn validate(&self, config: &serde_json::Value) -> Result<()> {
        let value = config
            .pointer(&format!("/{}", self.field_path.replace('.', "/")))
            .ok_or_else(|| {
                AppError::ConfigError(format!("URL field '{}' not found", self.field_path))
            })?;

        if let Some(url_str) = value.as_str() {
            if url_str.is_empty() {
                return Ok(()); // Allow empty URLs for optional fields
            }

            let url = url::Url::parse(url_str)
                .map_err(|e| AppError::ConfigError(format!("Invalid URL '{}': {}", url_str, e)))?;

            if !self.schemes.is_empty() && !self.schemes.contains(&url.scheme().to_string()) {
                return Err(AppError::ConfigError(format!(
                    "URL scheme '{}' not allowed. Allowed schemes: {:?}",
                    url.scheme(),
                    self.schemes
                )));
            }
        } else {
            return Err(AppError::ConfigError(format!(
                "URL field '{}' must be a string",
                self.field_path
            )));
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Validates URL format and allowed schemes"
    }
}

/// Security validation rule for production environments
pub struct SecurityRule {
    environment: String,
}

impl SecurityRule {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

impl ValidationRule for SecurityRule {
    fn name(&self) -> &'static str {
        "security"
    }

    fn validate(&self, config: &serde_json::Value) -> Result<()> {
        if self.environment != "production" {
            return Ok(());
        }

        // Check JWT secret strength
        if let Some(jwt_secret) = config.pointer("/security/jwt/secret") {
            if let Some(secret_str) = jwt_secret.as_str() {
                if secret_str.len() < 32 {
                    return Err(AppError::ConfigError(
                        "JWT secret must be at least 32 characters in production".to_string(),
                    ));
                }
                if secret_str.contains("example") || secret_str == "your-secret-key" {
                    return Err(AppError::ConfigError(
                        "JWT secret must not contain default/example values in production"
                            .to_string(),
                    ));
                }
            }
        }

        // Check encryption key
        if let Some(encryption_key) = config.pointer("/security/encryption/key") {
            if let Some(key_str) = encryption_key.as_str() {
                if key_str.is_empty() {
                    return Err(AppError::ConfigError(
                        "Encryption key is required in production".to_string(),
                    ));
                }

                // Validate base64 format and length
                match base64ct::Base64::decode_vec(key_str) {
                    Ok(key_bytes) => {
                        if key_bytes.len() != 32 {
                            return Err(AppError::ConfigError(
                                "Encryption key must be 32 bytes (base64 encoded) in production"
                                    .to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        return Err(AppError::ConfigError(
                            "Encryption key must be valid base64 in production".to_string(),
                        ));
                    }
                }
            }
        }

        // Check database SSL
        if let Some(enable_ssl) = config.pointer("/database/enable_ssl") {
            if let Some(ssl_bool) = enable_ssl.as_bool() {
                if !ssl_bool {
                    warn!("⚠️ Database SSL is disabled in production environment");
                }
            }
        }

        // Check CORS configuration
        if let Some(allowed_origins) = config.pointer("/security/cors/allowed_origins") {
            if let Some(origins_array) = allowed_origins.as_array() {
                for origin in origins_array {
                    if let Some(origin_str) = origin.as_str() {
                        if origin_str == "*" {
                            return Err(AppError::ConfigError(
                                "CORS wildcard (*) is not allowed in production".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Validates security configuration for production environments"
    }
}

/// Resource limits validation rule
pub struct ResourceLimitsRule;

impl ValidationRule for ResourceLimitsRule {
    fn name(&self) -> &'static str {
        "resource_limits"
    }

    fn validate(&self, config: &serde_json::Value) -> Result<()> {
        // Check server configuration
        if let Some(max_connections) = config.pointer("/server/max_connections") {
            if let Some(connections) = max_connections.as_u64() {
                if connections > 10000 {
                    warn!(
                        "⚠️ Max connections ({}) is very high, consider system limits",
                        connections
                    );
                }
            }
        }

        // Check database pool size
        if let Some(pool_size) = config.pointer("/database/connection_pool_size") {
            if let Some(size) = pool_size.as_u64() {
                if size > 100 {
                    warn!("⚠️ Database connection pool size ({}) is very high", size);
                }
                if size == 0 {
                    return Err(AppError::ConfigError(
                        "Database connection pool size must be greater than 0".to_string(),
                    ));
                }
            }
        }

        // Check CI resource limits
        if let Some(executors) = config.pointer("/ci/executors") {
            if let Some(executors_obj) = executors.as_object() {
                for (executor_name, executor_config) in executors_obj {
                    if let Some(resource_limits) = executor_config.pointer("/resource_limits") {
                        self.validate_executor_limits(executor_name, resource_limits)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Validates resource limits and constraints"
    }
}

impl ResourceLimitsRule {
    fn validate_executor_limits(
        &self,
        executor_name: &str,
        limits: &serde_json::Value,
    ) -> Result<()> {
        if let Some(cpu_cores) = limits.pointer("/cpu_cores") {
            if let Some(cores) = cpu_cores.as_f64() {
                if cores <= 0.0 || cores > 64.0 {
                    return Err(AppError::ConfigError(format!(
                        "Executor '{}' CPU cores must be between 0 and 64",
                        executor_name
                    )));
                }
            }
        }

        if let Some(memory_mb) = limits.pointer("/memory_mb") {
            if let Some(memory) = memory_mb.as_u64() {
                if memory == 0 || memory > 128 * 1024 {
                    // 128GB
                    return Err(AppError::ConfigError(format!(
                        "Executor '{}' memory must be between 1MB and 128GB",
                        executor_name
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Create default validator with common rules
pub fn create_default_validator(environment: &str) -> ConfigValidator {
    ConfigValidator::new()
        .add_rule(Box::new(RequiredFieldRule::new(
            "security.jwt.secret",
            "JWT Secret",
        )))
        .add_rule(Box::new(RequiredFieldRule::new(
            "database.mongodb_uri",
            "MongoDB URI",
        )))
        .add_rule(Box::new(PortRangeRule::new("server.port")))
        .add_rule(Box::new(UrlFormatRule::new(
            "database.mongodb_uri",
            vec!["mongodb", "mongodb+srv"],
        )))
        .add_rule(Box::new(SecurityRule::new(environment)))
        .add_rule(Box::new(ResourceLimitsRule))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_required_field_rule() {
        let rule = RequiredFieldRule::new("test.field", "Test Field");

        // Valid config
        let valid_config = json!({
            "test": {
                "field": "value"
            }
        });
        assert!(rule.validate(&valid_config).is_ok());

        // Missing field
        let invalid_config = json!({
            "test": {}
        });
        assert!(rule.validate(&invalid_config).is_err());

        // Empty field
        let empty_config = json!({
            "test": {
                "field": ""
            }
        });
        assert!(rule.validate(&empty_config).is_err());
    }

    #[test]
    fn test_port_range_rule() {
        let rule = PortRangeRule::new("server.port");

        // Valid port
        let valid_config = json!({
            "server": {
                "port": 8080
            }
        });
        assert!(rule.validate(&valid_config).is_ok());

        // Invalid port (too high)
        let invalid_config = json!({
            "server": {
                "port": 70000
            }
        });
        assert!(rule.validate(&invalid_config).is_err());

        // Invalid port (zero)
        let zero_config = json!({
            "server": {
                "port": 0
            }
        });
        assert!(rule.validate(&zero_config).is_err());
    }

    #[test]
    fn test_url_format_rule() {
        let rule = UrlFormatRule::new("database.url", vec!["mongodb", "mongodb+srv"]);

        // Valid URL
        let valid_config = json!({
            "database": {
                "url": "mongodb://localhost:27017"
            }
        });
        assert!(rule.validate(&valid_config).is_ok());

        // Invalid scheme
        let invalid_config = json!({
            "database": {
                "url": "http://localhost:27017"
            }
        });
        assert!(rule.validate(&invalid_config).is_err());

        // Invalid URL format
        let malformed_config = json!({
            "database": {
                "url": "not-a-url"
            }
        });
        assert!(rule.validate(&malformed_config).is_err());
    }

    #[test]
    fn test_validation_report() {
        let mut report = ValidationReport::new();

        report.add_success("rule1".to_string());
        report.add_error("rule2".to_string(), "Error message".to_string());
        report.add_warning("rule3".to_string(), "Warning message".to_string());

        assert!(!report.is_valid());
        assert!(report.has_warnings());
        assert_eq!(report.successes.len(), 1);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.warnings.len(), 1);
    }
}
