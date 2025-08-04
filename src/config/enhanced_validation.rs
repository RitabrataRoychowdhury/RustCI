use crate::config::AppConfiguration;
use crate::error::Result;
use base64ct::Encoding;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error};

/// Enhanced configuration validation with security focus
pub struct EnhancedConfigValidator {
    environment: String,
    validation_rules: Vec<Box<dyn ConfigValidationRule>>,
}

impl EnhancedConfigValidator {
    pub fn new(environment: &str) -> Self {
        let mut validator = Self {
            environment: environment.to_string(),
            validation_rules: Vec::new(),
        };

        // Add default validation rules
        validator.add_default_rules();
        validator
    }

    fn add_default_rules(&mut self) {
        self.validation_rules
            .push(Box::new(SecurityValidationRule::new(&self.environment)));
        self.validation_rules.push(Box::new(DatabaseValidationRule));
        self.validation_rules.push(Box::new(ServerValidationRule));
        self.validation_rules
            .push(Box::new(RateLimitValidationRule));
        self.validation_rules
            .push(Box::new(CorsValidationRule::new(&self.environment)));
        self.validation_rules
            .push(Box::new(ObservabilityValidationRule));
        self.validation_rules.push(Box::new(CIValidationRule));
    }

    pub fn add_rule(&mut self, rule: Box<dyn ConfigValidationRule>) {
        self.validation_rules.push(rule);
    }

    pub fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        debug!("🔍 Starting enhanced configuration validation");

        for rule in &self.validation_rules {
            match rule.validate(config) {
                Ok(rule_report) => {
                    report.merge(rule_report);
                }
                Err(e) => {
                    report.add_error(rule.name().to_string(), e.to_string());
                    error!("Validation rule '{}' failed: {}", rule.name(), e);
                }
            }
        }

        // Add environment-specific warnings
        self.add_environment_warnings(&mut report, config);

        debug!(
            total_rules = self.validation_rules.len(),
            errors = report.errors.len(),
            warnings = report.warnings.len(),
            "✅ Enhanced configuration validation completed"
        );

        Ok(report)
    }

    fn add_environment_warnings(&self, report: &mut ValidationReport, config: &AppConfiguration) {
        match self.environment.as_str() {
            "production" => {
                if config.observability.logging.level == "debug"
                    || config.observability.logging.level == "trace"
                {
                    report.add_warning(
                        "logging".to_string(),
                        "Debug/trace logging enabled in production".to_string(),
                    );
                }

                if config.features.enable_experimental_features {
                    report.add_warning(
                        "features".to_string(),
                        "Experimental features enabled in production".to_string(),
                    );
                }
            }
            "development" => {
                if !config.features.enable_hot_reload {
                    report.add_info(
                        "features".to_string(),
                        "Hot reload disabled in development".to_string(),
                    );
                }
            }
            _ => {}
        }
    }
}

/// Enhanced validation report with different severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub errors: HashMap<String, String>,
    pub warnings: HashMap<String, String>,
    pub info: HashMap<String, String>,
    pub successes: Vec<String>,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
            warnings: HashMap::new(),
            info: HashMap::new(),
            successes: Vec::new(),
        }
    }

    pub fn add_error(&mut self, rule_name: String, error: String) {
        self.errors.insert(rule_name, error);
    }

    pub fn add_warning(&mut self, rule_name: String, warning: String) {
        self.warnings.insert(rule_name, warning);
    }

    pub fn add_info(&mut self, rule_name: String, info: String) {
        self.info.insert(rule_name, info);
    }

    pub fn add_success(&mut self, rule_name: String) {
        self.successes.push(rule_name);
    }

    pub fn merge(&mut self, other: ValidationReport) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.info.extend(other.info);
        self.successes.extend(other.successes);
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "Validation: {} errors, {} warnings, {} info, {} passed",
            self.errors.len(),
            self.warnings.len(),
            self.info.len(),
            self.successes.len()
        )
    }
}

/// Trait for configuration validation rules
pub trait ConfigValidationRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport>;
    fn description(&self) -> &'static str {
        "No description provided"
    }
}

/// Security-focused validation rule
pub struct SecurityValidationRule {
    environment: String,
}

impl SecurityValidationRule {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

impl ConfigValidationRule for SecurityValidationRule {
    fn name(&self) -> &'static str {
        "security"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // JWT secret validation
        let jwt_secret = &config.security.jwt.secret;
        if jwt_secret.is_empty() {
            report.add_error(
                "jwt_secret".to_string(),
                "JWT secret is required".to_string(),
            );
        } else if self.environment == "production" {
            if jwt_secret.len() < 32 {
                report.add_error(
                    "jwt_secret".to_string(),
                    "JWT secret must be at least 32 characters in production".to_string(),
                );
            }
            if jwt_secret.contains("example") || jwt_secret == "your-secret-key" {
                report.add_error(
                    "jwt_secret".to_string(),
                    "JWT secret must not contain default values in production".to_string(),
                );
            }
        }

        // Encryption key validation
        let encryption_key = &config.security.encryption.key;
        if !encryption_key.is_empty() {
            match base64ct::Base64::decode_vec(encryption_key) {
                Ok(key_bytes) => {
                    if key_bytes.len() != 32 {
                        report.add_error(
                            "encryption_key".to_string(),
                            "Encryption key must be 32 bytes (base64 encoded)".to_string(),
                        );
                    } else {
                        report.add_success("encryption_key".to_string());
                    }
                }
                Err(_) => {
                    report.add_error(
                        "encryption_key".to_string(),
                        "Encryption key must be valid base64".to_string(),
                    );
                }
            }
        } else if self.environment == "production" {
            report.add_error(
                "encryption_key".to_string(),
                "Encryption key is required in production".to_string(),
            );
        }

        // OAuth configuration
        let github_config = &config.security.oauth.github;
        if github_config.client_id.is_empty() || github_config.client_secret.is_empty() {
            if self.environment == "production" {
                report.add_error(
                    "oauth".to_string(),
                    "GitHub OAuth credentials are required in production".to_string(),
                );
            } else {
                report.add_warning(
                    "oauth".to_string(),
                    "GitHub OAuth credentials not configured".to_string(),
                );
            }
        } else {
            report.add_success("oauth".to_string());
        }

        // RBAC configuration
        if config.security.rbac.enabled {
            let valid_roles = ["Admin", "Developer", "Viewer", "ServiceAccount"];
            if !valid_roles.contains(&config.security.rbac.default_role.as_str()) {
                report.add_error(
                    "rbac_default_role".to_string(),
                    format!(
                        "Invalid default role: {}",
                        config.security.rbac.default_role
                    ),
                );
            }
            report.add_success("rbac".to_string());
        }

        // Session configuration
        if config.security.session.timeout_minutes == 0 {
            report.add_error(
                "session_timeout".to_string(),
                "Session timeout must be greater than 0".to_string(),
            );
        }
        if config.security.session.max_concurrent_sessions == 0 {
            report.add_warning(
                "session_concurrent".to_string(),
                "Unlimited concurrent sessions allowed".to_string(),
            );
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates security configuration including JWT, encryption, OAuth, RBAC, and session settings"
    }
}

/// Database configuration validation
pub struct DatabaseValidationRule;

impl ConfigValidationRule for DatabaseValidationRule {
    fn name(&self) -> &'static str {
        "database"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // MongoDB URI validation
        if config.database.mongodb_uri.is_empty() {
            report.add_error(
                "mongodb_uri".to_string(),
                "MongoDB URI is required".to_string(),
            );
        } else {
            // Basic URI format validation
            if !config.database.mongodb_uri.starts_with("mongodb://")
                && !config.database.mongodb_uri.starts_with("mongodb+srv://")
            {
                report.add_error(
                    "mongodb_uri".to_string(),
                    "MongoDB URI must start with mongodb:// or mongodb+srv://".to_string(),
                );
            } else {
                report.add_success("mongodb_uri".to_string());
            }
        }

        // Database name validation
        if config.database.database_name.is_empty() {
            report.add_error(
                "database_name".to_string(),
                "Database name is required".to_string(),
            );
        } else if config.database.database_name.len() > 64 {
            report.add_error(
                "database_name".to_string(),
                "Database name too long (max 64 characters)".to_string(),
            );
        } else {
            report.add_success("database_name".to_string());
        }

        // Connection pool validation
        if config.database.connection_pool_size == 0 {
            report.add_error(
                "connection_pool_size".to_string(),
                "Connection pool size must be greater than 0".to_string(),
            );
        } else if config.database.connection_pool_size > 100 {
            report.add_warning(
                "connection_pool_size".to_string(),
                "Large connection pool size may impact performance".to_string(),
            );
        } else {
            report.add_success("connection_pool_size".to_string());
        }

        // Timeout validation
        if config.database.connection_timeout_seconds == 0 {
            report.add_warning(
                "connection_timeout".to_string(),
                "Connection timeout is 0 (no timeout)".to_string(),
            );
        }
        if config.database.query_timeout_seconds == 0 {
            report.add_warning(
                "query_timeout".to_string(),
                "Query timeout is 0 (no timeout)".to_string(),
            );
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates database configuration including URI, connection pool, and timeout settings"
    }
}

/// Server configuration validation
pub struct ServerValidationRule;

impl ConfigValidationRule for ServerValidationRule {
    fn name(&self) -> &'static str {
        "server"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Port validation
        if config.server.port == 0 {
            report.add_error(
                "port".to_string(),
                "Server port must be between 1 and 65535".to_string(),
            );
        } else if config.server.port < 1024 {
            report.add_warning(
                "port".to_string(),
                "Using privileged port (< 1024) may require elevated permissions".to_string(),
            );
        } else {
            report.add_success("port".to_string());
        }

        // Host validation
        if config.server.host.is_empty() {
            report.add_error("host".to_string(), "Server host is required".to_string());
        } else {
            report.add_success("host".to_string());
        }

        // Connection limits
        if config.server.max_connections == 0 {
            report.add_warning(
                "max_connections".to_string(),
                "Unlimited connections allowed".to_string(),
            );
        } else if config.server.max_connections > 10000 {
            report.add_warning(
                "max_connections".to_string(),
                "Very high connection limit may impact performance".to_string(),
            );
        }

        // Timeout validation
        if config.server.request_timeout_seconds == 0 {
            report.add_warning(
                "request_timeout".to_string(),
                "Request timeout is disabled".to_string(),
            );
        } else if config.server.request_timeout_seconds > 300 {
            report.add_warning(
                "request_timeout".to_string(),
                "Very long request timeout (> 5 minutes)".to_string(),
            );
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates server configuration including port, host, and connection settings"
    }
}

/// Rate limiting configuration validation
pub struct RateLimitValidationRule;

impl ConfigValidationRule for RateLimitValidationRule {
    fn name(&self) -> &'static str {
        "rate_limit"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        let rate_config = &config.security.rate_limiting;

        if rate_config.requests_per_minute == 0 {
            report.add_warning(
                "requests_per_minute".to_string(),
                "Rate limiting disabled (0 requests per minute)".to_string(),
            );
        } else if rate_config.requests_per_minute > 10000 {
            report.add_warning(
                "requests_per_minute".to_string(),
                "Very high rate limit may not be effective".to_string(),
            );
        } else {
            report.add_success("requests_per_minute".to_string());
        }

        if rate_config.burst_size > rate_config.requests_per_minute * 2 {
            report.add_warning(
                "burst_size".to_string(),
                "Burst size is much larger than rate limit".to_string(),
            );
        }

        if rate_config.whitelist_ips.is_empty() {
            report.add_info(
                "whitelist_ips".to_string(),
                "No IP addresses whitelisted".to_string(),
            );
        } else {
            // Validate IP addresses
            for ip in &rate_config.whitelist_ips {
                if ip.parse::<std::net::IpAddr>().is_err() && ip != "localhost" && ip != "127.0.0.1"
                {
                    report.add_warning(
                        "whitelist_ips".to_string(),
                        format!("Invalid IP address in whitelist: {}", ip),
                    );
                }
            }
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates rate limiting configuration including limits and whitelist"
    }
}

/// CORS configuration validation
pub struct CorsValidationRule {
    environment: String,
}

impl CorsValidationRule {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

impl ConfigValidationRule for CorsValidationRule {
    fn name(&self) -> &'static str {
        "cors"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        let cors_config = &config.security.cors;

        // Check for wildcard origins in production
        if self.environment == "production"
            && cors_config.allowed_origins.contains(&"*".to_string())
        {
            report.add_error(
                "cors_origins".to_string(),
                "Wildcard CORS origins not allowed in production".to_string(),
            );
        }

        // Validate origin URLs
        for origin in &cors_config.allowed_origins {
            if origin != "*" && !origin.starts_with("http://") && !origin.starts_with("https://") {
                report.add_warning(
                    "cors_origins".to_string(),
                    format!("Invalid origin format: {}", origin),
                );
            }
        }

        // Check credentials with wildcard
        if cors_config.allow_credentials && cors_config.allowed_origins.contains(&"*".to_string()) {
            report.add_error(
                "cors_credentials".to_string(),
                "Cannot allow credentials with wildcard origins".to_string(),
            );
        }

        // Validate max age
        if cors_config.max_age_seconds > 86400 {
            report.add_warning(
                "cors_max_age".to_string(),
                "CORS max age is very long (> 24 hours)".to_string(),
            );
        }

        if report.errors.is_empty() {
            report.add_success("cors".to_string());
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates CORS configuration for security compliance"
    }
}

/// Observability configuration validation
pub struct ObservabilityValidationRule;

impl ConfigValidationRule for ObservabilityValidationRule {
    fn name(&self) -> &'static str {
        "observability"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        let obs_config = &config.observability;

        // Log level validation
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&obs_config.logging.level.to_lowercase().as_str()) {
            report.add_error(
                "log_level".to_string(),
                format!("Invalid log level: {}", obs_config.logging.level),
            );
        } else {
            report.add_success("log_level".to_string());
        }

        // Log format validation
        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&obs_config.logging.format.as_str()) {
            report.add_error(
                "log_format".to_string(),
                format!("Invalid log format: {}", obs_config.logging.format),
            );
        }

        // Metrics validation
        if obs_config.metrics.enabled {
            if obs_config.metrics.collection_interval_seconds == 0 {
                report.add_warning(
                    "metrics_interval".to_string(),
                    "Metrics collection interval is 0".to_string(),
                );
            }
            if obs_config.metrics.endpoint.is_empty() {
                report.add_error(
                    "metrics_endpoint".to_string(),
                    "Metrics endpoint is required when metrics are enabled".to_string(),
                );
            }
        }

        // Tracing validation
        if obs_config.tracing.enabled {
            if obs_config.tracing.sample_rate < 0.0 || obs_config.tracing.sample_rate > 1.0 {
                report.add_error(
                    "tracing_sample_rate".to_string(),
                    "Tracing sample rate must be between 0.0 and 1.0".to_string(),
                );
            }
            if obs_config.tracing.service_name.is_empty() {
                report.add_error(
                    "tracing_service_name".to_string(),
                    "Service name is required when tracing is enabled".to_string(),
                );
            }
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates observability configuration including logging, metrics, and tracing"
    }
}

/// CI configuration validation with tiered pipeline support
pub struct CIValidationRule;

impl ConfigValidationRule for CIValidationRule {
    fn name(&self) -> &'static str {
        "ci"
    }

    fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        let ci_config = &config.ci;

        // Pipeline limits
        if ci_config.max_concurrent_pipelines == 0 {
            report.add_warning(
                "max_concurrent_pipelines".to_string(),
                "Unlimited concurrent pipelines allowed".to_string(),
            );
        } else if ci_config.max_concurrent_pipelines > 100 {
            report.add_warning(
                "max_concurrent_pipelines".to_string(),
                "Very high concurrent pipeline limit".to_string(),
            );
        }

        // Timeout validation
        if ci_config.pipeline_timeout_seconds == 0 {
            report.add_warning(
                "pipeline_timeout".to_string(),
                "Pipeline timeout is disabled".to_string(),
            );
        } else if ci_config.pipeline_timeout_seconds > 86400 {
            report.add_warning(
                "pipeline_timeout".to_string(),
                "Very long pipeline timeout (> 24 hours)".to_string(),
            );
        }

        // Default executor validation
        if !ci_config
            .executors
            .contains_key(&ci_config.default_executor)
        {
            report.add_error(
                "default_executor".to_string(),
                format!(
                    "Default executor '{}' not found in executors",
                    ci_config.default_executor
                ),
            );
        }

        // Executor validation
        for (name, executor) in &ci_config.executors {
            if executor.max_concurrent_jobs == 0 {
                report.add_warning(
                    format!("executor_{}_concurrent_jobs", name),
                    "Unlimited concurrent jobs allowed".to_string(),
                );
            }

            // Resource limits validation
            let limits = &executor.resource_limits;
            if let Some(cpu) = limits.cpu_cores {
                if cpu <= 0.0 || cpu > 64.0 {
                    report.add_warning(
                        format!("executor_{}_cpu", name),
                        "CPU cores should be between 0 and 64".to_string(),
                    );
                }
            }
            if let Some(memory) = limits.memory_mb {
                if memory == 0 || memory > 128 * 1024 {
                    report.add_warning(
                        format!("executor_{}_memory", name),
                        "Memory should be between 1MB and 128GB".to_string(),
                    );
                }
            }
        }

        if report.errors.is_empty() {
            report.add_success("ci".to_string());
        }

        Ok(report)
    }

    fn description(&self) -> &'static str {
        "Validates CI configuration including pipelines, executors, and resource limits with tiered pipeline support"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_validation_rule() {
        let rule = SecurityValidationRule::new("production");
        let mut config = AppConfiguration::default();

        // Test with default config (should fail in production)
        let report = rule.validate(&config).unwrap();
        assert!(!report.is_valid());

        // Fix the config
        config.security.jwt.secret = "a_very_secure_secret_key_that_is_long_enough".to_string();
        config.security.encryption.key = base64ct::Base64::encode_string(&[0u8; 32]);

        let report = rule.validate(&config).unwrap();
        assert!(report.errors.len() < 3); // Should have fewer errors now
    }

    #[test]
    fn test_database_validation_rule() {
        let rule = DatabaseValidationRule;
        let mut config = AppConfiguration::default();

        // Test with valid config
        config.database.mongodb_uri = "mongodb://localhost:27017".to_string();
        config.database.database_name = "test_db".to_string();

        let report = rule.validate(&config).unwrap();
        assert!(report.is_valid());
    }

    #[test]
    fn test_enhanced_config_validator() {
        let validator = EnhancedConfigValidator::new("development");
        let config = AppConfiguration::default();

        let report = validator.validate(&config).unwrap();

        // Should have some errors due to default config
        assert!(!report.is_valid());
        assert!(!report.errors.is_empty());
    }
}
