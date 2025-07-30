use crate::config::{enhanced_validation::*, AppConfiguration};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Comprehensive configuration validation engine
pub struct ConfigValidationEngine {
    validators: HashMap<String, Box<dyn ConfigValidator>>,
    validation_cache: Arc<RwLock<ValidationCache>>,
    environment: String,
}

#[derive(Debug, Clone)]
struct ValidationCache {
    results: HashMap<String, CachedValidationResult>,
}

#[derive(Debug, Clone)]
pub struct CachedValidationResult {
    result: ValidationReport,
    timestamp: chrono::DateTime<chrono::Utc>,
    config_hash: u64,
}

impl ConfigValidationEngine {
    pub fn new(environment: &str) -> Self {
        let mut engine = Self {
            validators: HashMap::new(),
            validation_cache: Arc::new(RwLock::new(ValidationCache {
                results: HashMap::new(),
            })),
            environment: environment.to_string(),
        };

        // Register default validators
        engine.register_default_validators();
        engine
    }

    fn register_default_validators(&mut self) {
        self.register_validator(
            "security",
            Box::new(SecurityConfigValidator::new(&self.environment)),
        );
        self.register_validator("database", Box::new(DatabaseConfigValidator));
        self.register_validator("server", Box::new(ServerConfigValidator));
        self.register_validator("observability", Box::new(ObservabilityConfigValidator));
        self.register_validator("ci", Box::new(CIConfigValidator));
        self.register_validator(
            "features",
            Box::new(FeatureFlagValidator::new(&self.environment)),
        );
        self.register_validator("external_services", Box::new(ExternalServicesValidator));
    }

    pub fn register_validator(&mut self, name: &str, validator: Box<dyn ConfigValidator>) {
        self.validators.insert(name.to_string(), validator);
    }

    /// Validate configuration with caching
    pub async fn validate(
        &self,
        config: &AppConfiguration,
    ) -> Result<ComprehensiveValidationReport> {
        let config_hash = self.calculate_config_hash(config);

        // Check cache first
        if let Some(cached) = self.check_cache(config_hash).await {
            debug!("Using cached validation result");
            return Ok(ComprehensiveValidationReport::from_cached(cached));
        }

        debug!("🔍 Starting comprehensive configuration validation");
        let start_time = std::time::Instant::now();

        let mut comprehensive_report = ComprehensiveValidationReport::new();
        let mut validation_futures = Vec::new();

        // Run all validators concurrently
        for (name, validator) in &self.validators {
            let validator_name = name.clone();
            let config_clone = config.clone();
            let validator_ref = validator.as_ref();

            let future = async move {
                let result = validator_ref.validate(&config_clone).await;
                (validator_name, result)
            };

            validation_futures.push(future);
        }

        // Collect results
        let results = futures::future::join_all(validation_futures).await;

        for (validator_name, result) in results {
            match result {
                Ok(report) => {
                    comprehensive_report.add_validator_report(validator_name, report);
                }
                Err(e) => {
                    error!("Validator '{}' failed: {}", validator_name, e);
                    comprehensive_report.add_validator_error(validator_name, e.to_string());
                }
            }
        }

        // Add environment-specific validations
        self.add_environment_validations(&mut comprehensive_report, config)
            .await;

        // Add cross-validator checks
        self.add_cross_validator_checks(&mut comprehensive_report, config)
            .await;

        let duration = start_time.elapsed();
        comprehensive_report.set_validation_duration(duration);

        // Cache the result
        self.cache_result(config_hash, &comprehensive_report).await;

        info!(
            duration_ms = duration.as_millis(),
            errors = comprehensive_report.total_errors(),
            warnings = comprehensive_report.total_warnings(),
            "✅ Comprehensive configuration validation completed"
        );

        Ok(comprehensive_report)
    }

    async fn add_environment_validations(
        &self,
        report: &mut ComprehensiveValidationReport,
        config: &AppConfiguration,
    ) {
        let mut env_report = ValidationReport::new();

        match self.environment.as_str() {
            "production" => {
                // Production-specific validations
                if config.observability.logging.level == "debug"
                    || config.observability.logging.level == "trace"
                {
                    env_report.add_warning(
                        "logging_level".to_string(),
                        "Debug/trace logging in production may impact performance".to_string(),
                    );
                }

                if config.features.enable_experimental_features {
                    env_report.add_error(
                        "experimental_features".to_string(),
                        "Experimental features should not be enabled in production".to_string(),
                    );
                }

                if !config.security.session.secure_cookies {
                    env_report.add_error(
                        "secure_cookies".to_string(),
                        "Secure cookies must be enabled in production".to_string(),
                    );
                }

                if config
                    .security
                    .cors
                    .allowed_origins
                    .contains(&"*".to_string())
                {
                    env_report.add_error(
                        "cors_wildcard".to_string(),
                        "Wildcard CORS origins not allowed in production".to_string(),
                    );
                }
            }
            "staging" => {
                // Staging-specific validations
                if config.features.enable_experimental_features {
                    env_report.add_warning(
                        "experimental_features".to_string(),
                        "Experimental features enabled in staging - ensure thorough testing"
                            .to_string(),
                    );
                }
            }
            "development" => {
                // Development-specific validations
                if !config.features.enable_hot_reload {
                    env_report.add_info(
                        "hot_reload".to_string(),
                        "Hot reload disabled in development environment".to_string(),
                    );
                }

                if config.security.session.secure_cookies {
                    env_report.add_info(
                        "secure_cookies".to_string(),
                        "Secure cookies enabled in development (may cause issues with HTTP)"
                            .to_string(),
                    );
                }
            }
            _ => {
                env_report.add_warning(
                    "unknown_environment".to_string(),
                    format!("Unknown environment: {}", self.environment),
                );
            }
        }

        report.add_validator_report("environment".to_string(), env_report);
    }

    async fn add_cross_validator_checks(
        &self,
        report: &mut ComprehensiveValidationReport,
        config: &AppConfiguration,
    ) {
        let mut cross_report = ValidationReport::new();

        // Check consistency between related configurations

        // Database and CI configuration consistency
        if config.ci.max_concurrent_pipelines > config.database.connection_pool_size * 2 {
            cross_report.add_warning(
                "db_ci_consistency".to_string(),
                "High concurrent pipelines may overwhelm database connection pool".to_string(),
            );
        }

        // Security and server configuration consistency
        if config.server.port < 1024 && self.environment != "development" {
            cross_report.add_warning(
                "privileged_port".to_string(),
                "Using privileged port may require elevated permissions".to_string(),
            );
        }

        // Observability and performance consistency
        if config.observability.metrics.enabled
            && config.observability.metrics.collection_interval_seconds < 10
        {
            cross_report.add_warning(
                "metrics_frequency".to_string(),
                "Very frequent metrics collection may impact performance".to_string(),
            );
        }

        // Rate limiting and server capacity consistency
        let total_rate_limit = config.security.rate_limiting.requests_per_minute * 60; // per hour
        let server_capacity_estimate = config.server.max_connections * 100; // rough estimate

        if total_rate_limit > server_capacity_estimate {
            cross_report.add_warning(
                "rate_limit_capacity".to_string(),
                "Rate limits may exceed estimated server capacity".to_string(),
            );
        }

        report.add_validator_report("cross_validation".to_string(), cross_report);
    }

    fn calculate_config_hash(&self, config: &AppConfiguration) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        // In a real implementation, you'd serialize the config and hash it
        // For now, we'll use a simple approach
        format!("{:?}", config).hash(&mut hasher);
        hasher.finish()
    }

    async fn check_cache(&self, config_hash: u64) -> Option<CachedValidationResult> {
        let cache = self.validation_cache.read().await;
        if let Some(cached) = cache.results.get("main") {
            if cached.config_hash == config_hash {
                let age = chrono::Utc::now().signed_duration_since(cached.timestamp);
                if age.num_minutes() < 5 {
                    // Cache for 5 minutes
                    return Some(cached.clone());
                }
            }
        }
        None
    }

    async fn cache_result(&self, config_hash: u64, report: &ComprehensiveValidationReport) {
        let mut cache = self.validation_cache.write().await;
        cache.results.insert(
            "main".to_string(),
            CachedValidationResult {
                result: report.summary_report(),
                timestamp: chrono::Utc::now(),
                config_hash,
            },
        );
    }
}

/// Comprehensive validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveValidationReport {
    pub validator_reports: HashMap<String, ValidationReport>,
    pub validation_duration: Option<std::time::Duration>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub environment: String,
}

impl Default for ComprehensiveValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ComprehensiveValidationReport {
    pub fn new() -> Self {
        Self {
            validator_reports: HashMap::new(),
            validation_duration: None,
            timestamp: chrono::Utc::now(),
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        }
    }

    pub fn from_cached(cached: CachedValidationResult) -> Self {
        let mut report = Self::new();
        report
            .validator_reports
            .insert("cached".to_string(), cached.result);
        report.timestamp = cached.timestamp;
        report
    }

    pub fn add_validator_report(&mut self, validator_name: String, report: ValidationReport) {
        self.validator_reports.insert(validator_name, report);
    }

    pub fn add_validator_error(&mut self, validator_name: String, error: String) {
        let mut error_report = ValidationReport::new();
        error_report.add_error("validation_error".to_string(), error);
        self.validator_reports.insert(validator_name, error_report);
    }

    pub fn set_validation_duration(&mut self, duration: std::time::Duration) {
        self.validation_duration = Some(duration);
    }

    pub fn is_valid(&self) -> bool {
        self.validator_reports
            .values()
            .all(|report| report.is_valid())
    }

    pub fn total_errors(&self) -> usize {
        self.validator_reports
            .values()
            .map(|report| report.errors.len())
            .sum()
    }

    pub fn total_warnings(&self) -> usize {
        self.validator_reports
            .values()
            .map(|report| report.warnings.len())
            .sum()
    }

    pub fn total_info(&self) -> usize {
        self.validator_reports
            .values()
            .map(|report| report.info.len())
            .sum()
    }

    pub fn summary(&self) -> String {
        format!(
            "Validation Summary: {} validators, {} errors, {} warnings, {} info messages",
            self.validator_reports.len(),
            self.total_errors(),
            self.total_warnings(),
            self.total_info()
        )
    }

    pub fn summary_report(&self) -> ValidationReport {
        let mut summary = ValidationReport::new();

        for (validator_name, report) in &self.validator_reports {
            for (error_key, error_msg) in &report.errors {
                summary.add_error(
                    format!("{}:{}", validator_name, error_key),
                    error_msg.clone(),
                );
            }
            for (warning_key, warning_msg) in &report.warnings {
                summary.add_warning(
                    format!("{}:{}", validator_name, warning_key),
                    warning_msg.clone(),
                );
            }
            for (info_key, info_msg) in &report.info {
                summary.add_info(format!("{}:{}", validator_name, info_key), info_msg.clone());
            }
            for success in &report.successes {
                summary.add_success(format!("{}:{}", validator_name, success));
            }
        }

        summary
    }
}

/// Enhanced configuration validator trait
#[async_trait::async_trait]
pub trait ConfigValidator: Send + Sync {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport>;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn priority(&self) -> u8 {
        100
    } // Lower number = higher priority
}

/// Security configuration validator
pub struct SecurityConfigValidator {
    environment: String,
}

impl SecurityConfigValidator {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ConfigValidator for SecurityConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let security_rule = SecurityValidationRule::new(&self.environment);
        security_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "security"
    }

    fn description(&self) -> &'static str {
        "Validates security configuration including JWT, encryption, OAuth, and RBAC"
    }

    fn priority(&self) -> u8 {
        10 // High priority
    }
}

/// Database configuration validator
pub struct DatabaseConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for DatabaseConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let db_rule = DatabaseValidationRule;
        db_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "database"
    }

    fn description(&self) -> &'static str {
        "Validates database configuration including connection settings and timeouts"
    }

    fn priority(&self) -> u8 {
        20 // High priority
    }
}

/// Server configuration validator
pub struct ServerConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for ServerConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let server_rule = ServerValidationRule;
        server_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "server"
    }

    fn description(&self) -> &'static str {
        "Validates server configuration including port, host, and connection limits"
    }
}

/// Observability configuration validator
pub struct ObservabilityConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for ObservabilityConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let obs_rule = ObservabilityValidationRule;
        obs_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "observability"
    }

    fn description(&self) -> &'static str {
        "Validates observability configuration including logging, metrics, and tracing"
    }
}

/// CI configuration validator
pub struct CIConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for CIConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let ci_rule = CIValidationRule;
        ci_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "ci"
    }

    fn description(&self) -> &'static str {
        "Validates CI configuration including pipelines and executors"
    }
}

/// Feature flag validator
pub struct FeatureFlagValidator {
    environment: String,
}

impl FeatureFlagValidator {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ConfigValidator for FeatureFlagValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Validate feature flag combinations
        if config.features.enable_experimental_features && self.environment == "production" {
            report.add_error(
                "experimental_in_production".to_string(),
                "Experimental features should not be enabled in production".to_string(),
            );
        }

        if config.features.enable_hot_reload && self.environment == "production" {
            report.add_warning(
                "hot_reload_in_production".to_string(),
                "Hot reload in production may impact performance".to_string(),
            );
        }

        if !config.features.enable_audit_logging && self.environment == "production" {
            report.add_error(
                "audit_logging_disabled".to_string(),
                "Audit logging should be enabled in production".to_string(),
            );
        }

        if !config.features.enable_metrics_collection {
            report.add_warning(
                "metrics_disabled".to_string(),
                "Metrics collection is disabled - monitoring will be limited".to_string(),
            );
        }

        // Check custom flags
        for (flag_name, flag_value) in &config.features.custom_flags {
            if flag_name.contains("debug") && *flag_value && self.environment == "production" {
                report.add_warning(
                    format!("debug_flag_{}", flag_name),
                    format!("Debug flag '{}' enabled in production", flag_name),
                );
            }
        }

        if report.errors.is_empty() && report.warnings.is_empty() {
            report.add_success("feature_flags".to_string());
        }

        Ok(report)
    }

    fn name(&self) -> &'static str {
        "features"
    }

    fn description(&self) -> &'static str {
        "Validates feature flag configuration and environment compatibility"
    }
}

/// External services validator
pub struct ExternalServicesValidator;

#[async_trait::async_trait]
impl ConfigValidator for ExternalServicesValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Validate GitHub service configuration
        let github_config = &config.external_services.github;
        if github_config.api_url.is_empty() {
            report.add_error(
                "github_api_url".to_string(),
                "GitHub API URL is required".to_string(),
            );
        } else if !github_config.api_url.starts_with("https://") {
            report.add_warning(
                "github_api_url_insecure".to_string(),
                "GitHub API URL should use HTTPS".to_string(),
            );
        }

        if github_config.timeout_seconds == 0 {
            report.add_warning(
                "github_timeout".to_string(),
                "GitHub API timeout is disabled".to_string(),
            );
        } else if github_config.timeout_seconds > 300 {
            report.add_warning(
                "github_timeout_long".to_string(),
                "GitHub API timeout is very long (> 5 minutes)".to_string(),
            );
        }

        // Validate Docker registry configuration
        let docker_config = &config.external_services.docker_registry;
        if docker_config.url.is_empty() {
            report.add_error(
                "docker_registry_url".to_string(),
                "Docker registry URL is required".to_string(),
            );
        }

        if docker_config.insecure
            && std::env::var("ENVIRONMENT").unwrap_or_default() == "production"
        {
            report.add_error(
                "docker_registry_insecure".to_string(),
                "Insecure Docker registry not allowed in production".to_string(),
            );
        }

        // Validate notification configuration
        let notification_config = &config.external_services.notification;
        if notification_config.enabled && notification_config.channels.is_empty() {
            report.add_warning(
                "notification_no_channels".to_string(),
                "Notifications enabled but no channels configured".to_string(),
            );
        }

        for channel in &notification_config.channels {
            if channel.name.is_empty() {
                report.add_error(
                    "notification_channel_name".to_string(),
                    "Notification channel name is required".to_string(),
                );
            }

            match channel.channel_type.as_str() {
                "email" => {
                    if !channel.config.contains_key("smtp_server") {
                        report.add_error(
                            format!("notification_channel_{}_smtp", channel.name),
                            "Email channel requires smtp_server configuration".to_string(),
                        );
                    }
                }
                "slack" => {
                    if !channel.config.contains_key("webhook_url") {
                        report.add_error(
                            format!("notification_channel_{}_webhook", channel.name),
                            "Slack channel requires webhook_url configuration".to_string(),
                        );
                    }
                }
                "webhook" => {
                    if !channel.config.contains_key("url") {
                        report.add_error(
                            format!("notification_channel_{}_url", channel.name),
                            "Webhook channel requires url configuration".to_string(),
                        );
                    }
                }
                _ => {
                    report.add_warning(
                        format!("notification_channel_{}_type", channel.name),
                        format!(
                            "Unknown notification channel type: {}",
                            channel.channel_type
                        ),
                    );
                }
            }
        }

        if report.errors.is_empty() {
            report.add_success("external_services".to_string());
        }

        Ok(report)
    }

    fn name(&self) -> &'static str {
        "external_services"
    }

    fn description(&self) -> &'static str {
        "Validates external service configurations including GitHub, Docker registry, and notifications"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_validation_engine() {
        let engine = ConfigValidationEngine::new("development");
        let config = AppConfiguration::default();

        let report = engine.validate(&config).await.unwrap();

        // Should have some validation results
        assert!(!report.validator_reports.is_empty());
    }

    #[tokio::test]
    async fn test_security_config_validator() {
        let validator = SecurityConfigValidator::new("production");
        let config = AppConfiguration::default();

        let report = validator.validate(&config).await.unwrap();

        // Should have errors due to default config in production
        assert!(!report.is_valid());
    }

    #[tokio::test]
    async fn test_feature_flag_validator() {
        let validator = FeatureFlagValidator::new("production");
        let mut config = AppConfiguration::default();
        config.features.enable_experimental_features = true;

        let report = validator.validate(&config).await.unwrap();

        // Should have error for experimental features in production
        assert!(!report.is_valid());
        assert!(report.errors.contains_key("experimental_in_production"));
    }
}
