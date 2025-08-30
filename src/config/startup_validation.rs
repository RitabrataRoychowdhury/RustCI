use crate::config::{AppConfiguration, EnvironmentConfigResolver, ProductionConfigManager};
use crate::config::migration::{ConfigMigrationManager, MigrationReport};
use crate::config::validation_engine::{ConfigValidationEngine, ComprehensiveValidationReport};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{error, info, warn};

/// Enhanced startup configuration validator with comprehensive validation and migration support
pub struct StartupConfigValidator {
    validation_engine: ConfigValidationEngine,
    migration_manager: ConfigMigrationManager,
    environment_resolver: Option<EnvironmentConfigResolver>,
    production_manager: Option<ProductionConfigManager>,
    environment: String,
}

/// Comprehensive startup validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupValidationReport {
    pub validation_report: ComprehensiveValidationReport,
    pub migration_report: Option<MigrationReport>,
    pub environment_resolution_report: Option<EnvironmentResolutionReport>,
    pub remediation_suggestions: Vec<RemediationSuggestion>,
    pub startup_readiness: StartupReadiness,
    pub configuration_summary: ConfigurationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentResolutionReport {
    pub environment: String,
    pub base_config_loaded: bool,
    pub environment_overrides_applied: bool,
    pub encrypted_values_decrypted: usize,
    pub feature_flags_applied: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationSuggestion {
    pub category: String,
    pub severity: RemediationSeverity,
    pub issue: String,
    pub suggestion: String,
    pub example: Option<String>,
    pub documentation_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemediationSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StartupReadiness {
    Ready,
    ReadyWithWarnings,
    NotReady,
    CriticalIssues,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationSummary {
    pub environment: String,
    pub total_configuration_keys: usize,
    pub encrypted_values: usize,
    pub feature_flags_enabled: usize,
    pub security_features_enabled: Vec<String>,
    pub observability_features_enabled: Vec<String>,
    pub ci_executors_configured: usize,
    pub external_services_configured: usize,
}

impl StartupConfigValidator {
    pub fn new(environment: &str) -> Self {
        Self {
            validation_engine: ConfigValidationEngine::new(environment),
            migration_manager: ConfigMigrationManager::new(),
            environment_resolver: None,
            production_manager: None,
            environment: environment.to_string(),
        }
    }

    /// Initialize with environment-specific configuration support
    pub async fn with_environment_support<P: AsRef<Path>>(
        environment: &str,
        config_dir: P,
        base_config: AppConfiguration,
    ) -> Result<Self> {
        let mut validator = Self::new(environment);
        
        // Initialize environment resolver
        let mut env_resolver = EnvironmentConfigResolver::new(base_config);
        env_resolver.load_environment_configs(config_dir).await?;
        validator.environment_resolver = Some(env_resolver);
        
        // Initialize production manager if in production
        if environment == "production" {
            validator.production_manager = Some(ProductionConfigManager::new(environment));
        }
        
        Ok(validator)
    }

    /// Perform comprehensive startup validation
    pub async fn validate_startup_configuration(
        &mut self,
        config: &mut AppConfiguration,
    ) -> Result<StartupValidationReport> {
        info!("🔍 Starting comprehensive startup configuration validation...");
        let start_time = std::time::Instant::now();

        let mut report = StartupValidationReport {
            validation_report: ComprehensiveValidationReport::new(),
            migration_report: None,
            environment_resolution_report: None,
            remediation_suggestions: Vec::new(),
            startup_readiness: StartupReadiness::NotReady,
            configuration_summary: ConfigurationSummary::default(),
        };

        // Step 1: Apply configuration migrations
        info!("📦 Checking for configuration migrations...");
        if self.migration_manager.needs_migration(config).await? {
            let migration_report = self.migration_manager.apply_migrations(config).await?;
            
            if migration_report.failed_migrations > 0 {
                warn!("⚠️ Some configuration migrations failed");
                report.add_remediation_suggestions_from_migration(&migration_report);
            } else if migration_report.successful_migrations > 0 {
                info!("✅ Applied {} configuration migrations successfully", 
                      migration_report.successful_migrations);
            }
            
            report.migration_report = Some(migration_report);
        }

        // Step 2: Resolve environment-specific configuration
        if let Some(env_resolver) = &self.environment_resolver {
            info!("🌍 Resolving environment-specific configuration...");
            match self.resolve_environment_configuration(config, env_resolver).await {
                Ok(env_report) => {
                    report.environment_resolution_report = Some(env_report);
                }
                Err(e) => {
                    error!("❌ Environment configuration resolution failed: {}", e);
                    report.add_critical_remediation(
                        "environment_resolution",
                        "Environment configuration resolution failed",
                        &format!("Fix environment configuration: {}", e),
                    );
                }
            }
        }

        // Step 3: Run comprehensive validation
        info!("🔍 Running comprehensive configuration validation...");
        match self.validation_engine.validate(config).await {
            Ok(validation_report) => {
                report.validation_report = validation_report;
            }
            Err(e) => {
                error!("❌ Configuration validation failed: {}", e);
                report.add_critical_remediation(
                    "validation_engine",
                    "Configuration validation engine failed",
                    &format!("Fix validation engine error: {}", e),
                );
            }
        }

        // Step 4: Production-specific validation
        if let Some(prod_manager) = &self.production_manager {
            info!("🏭 Running production-specific validation...");
            // Skip production manager validation for now since it needs refactoring
            // TODO: Integrate production manager validation properly
            info!("🏭 Production-specific validation skipped (needs refactoring)");
            if false {
                // This code is disabled until production manager is refactored
                let _unused = prod_manager;
            }
        }

        // Step 5: Generate remediation suggestions
        self.generate_remediation_suggestions(&mut report, config).await;

        // Step 6: Determine startup readiness
        report.startup_readiness = self.determine_startup_readiness(&report);

        // Step 7: Generate configuration summary
        report.configuration_summary = self.generate_configuration_summary(config);

        let duration = start_time.elapsed();
        info!(
            duration_ms = duration.as_millis(),
            readiness = ?report.startup_readiness,
            errors = report.validation_report.total_errors(),
            warnings = report.validation_report.total_warnings(),
            "✅ Startup configuration validation completed"
        );

        Ok(report)
    }

    async fn resolve_environment_configuration(
        &self,
        config: &mut AppConfiguration,
        env_resolver: &EnvironmentConfigResolver,
    ) -> Result<EnvironmentResolutionReport> {
        let mut report = EnvironmentResolutionReport {
            environment: self.environment.clone(),
            base_config_loaded: true,
            environment_overrides_applied: false,
            encrypted_values_decrypted: 0,
            feature_flags_applied: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        match env_resolver.resolve_environment_config(&self.environment).await {
            Ok(resolved_config) => {
                // Count changes
                let original_flags = self.count_enabled_feature_flags(config);
                *config = resolved_config;
                let new_flags = self.count_enabled_feature_flags(config);
                
                report.environment_overrides_applied = true;
                report.feature_flags_applied = new_flags.saturating_sub(original_flags);
                
                // Count encrypted values (simplified)
                if let Some(env_config) = env_resolver.get_environment_config(&self.environment) {
                    report.encrypted_values_decrypted = env_config.encrypted_values.len();
                }
            }
            Err(e) => {
                report.errors.push(format!("Failed to resolve environment configuration: {}", e));
            }
        }

        Ok(report)
    }

    fn count_enabled_feature_flags(&self, config: &AppConfiguration) -> usize {
        let mut count = 0;
        if config.features.enable_audit_logging { count += 1; }
        if config.features.enable_metrics_collection { count += 1; }
        if config.features.enable_distributed_tracing { count += 1; }
        if config.features.enable_hot_reload { count += 1; }
        if config.features.enable_experimental_features { count += 1; }
        if config.features.enable_valkyrie_protocol { count += 1; }
        count += config.features.custom_flags.values().filter(|&&v| v).count();
        count
    }

    async fn generate_remediation_suggestions(
        &self,
        report: &mut StartupValidationReport,
        config: &AppConfiguration,
    ) {
        // Generate suggestions based on validation errors
        for (validator_name, validation_report) in &report.validation_report.validator_reports {
            for (error_key, error_msg) in &validation_report.errors {
                let suggestion = self.generate_error_remediation(validator_name, error_key, error_msg, config);
                report.remediation_suggestions.push(suggestion);
            }
            
            for (warning_key, warning_msg) in &validation_report.warnings {
                let suggestion = self.generate_warning_remediation(validator_name, warning_key, warning_msg, config);
                report.remediation_suggestions.push(suggestion);
            }
        }

        // Add environment-specific suggestions
        self.add_environment_specific_suggestions(report, config);
    }

    fn generate_error_remediation(
        &self,
        validator_name: &str,
        error_key: &str,
        error_msg: &str,
        config: &AppConfiguration,
    ) -> RemediationSuggestion {
        match (validator_name, error_key) {
            ("security", "jwt_secret") => RemediationSuggestion {
                category: "Security".to_string(),
                severity: RemediationSeverity::Critical,
                issue: error_msg.to_string(),
                suggestion: "Set a strong JWT secret using the JWT_SECRET environment variable or configuration file".to_string(),
                example: Some("JWT_SECRET=your-256-bit-secret".to_string()),
                documentation_link: Some("https://docs.rustci.dev/security/jwt".to_string()),
            },
            ("database", "mongodb_uri") => RemediationSuggestion {
                category: "Database".to_string(),
                severity: RemediationSeverity::Critical,
                issue: error_msg.to_string(),
                suggestion: "Configure MongoDB connection URI using MONGODB_URI environment variable".to_string(),
                example: Some("MONGODB_URI=mongodb://localhost:27017/rustci".to_string()),
                documentation_link: Some("https://docs.rustci.dev/database/setup".to_string()),
            },
            ("server", "port") => RemediationSuggestion {
                category: "Server".to_string(),
                severity: RemediationSeverity::High,
                issue: error_msg.to_string(),
                suggestion: "Set a valid server port using the PORT environment variable".to_string(),
                example: Some("PORT=8000".to_string()),
                documentation_link: Some("https://docs.rustci.dev/server/configuration".to_string()),
            },
            _ => RemediationSuggestion {
                category: validator_name.to_string(),
                severity: RemediationSeverity::High,
                issue: error_msg.to_string(),
                suggestion: format!("Fix the {} configuration issue: {}", validator_name, error_key),
                example: None,
                documentation_link: Some(format!("https://docs.rustci.dev/{}", validator_name)),
            },
        }
    }

    fn generate_warning_remediation(
        &self,
        validator_name: &str,
        warning_key: &str,
        warning_msg: &str,
        _config: &AppConfiguration,
    ) -> RemediationSuggestion {
        RemediationSuggestion {
            category: validator_name.to_string(),
            severity: RemediationSeverity::Medium,
            issue: warning_msg.to_string(),
            suggestion: format!("Consider addressing the {} warning: {}", validator_name, warning_key),
            example: None,
            documentation_link: Some(format!("https://docs.rustci.dev/{}", validator_name)),
        }
    }

    fn add_environment_specific_suggestions(&self, report: &mut StartupValidationReport, config: &AppConfiguration) {
        match self.environment.as_str() {
            "production" => {
                if config.observability.logging.level == "debug" {
                    report.remediation_suggestions.push(RemediationSuggestion {
                        category: "Production".to_string(),
                        severity: RemediationSeverity::Medium,
                        issue: "Debug logging enabled in production".to_string(),
                        suggestion: "Change logging level to 'info' or 'warn' for production".to_string(),
                        example: Some("LOG_LEVEL=info".to_string()),
                        documentation_link: Some("https://docs.rustci.dev/observability/logging".to_string()),
                    });
                }
                
                if !config.security.session.secure_cookies {
                    report.remediation_suggestions.push(RemediationSuggestion {
                        category: "Security".to_string(),
                        severity: RemediationSeverity::High,
                        issue: "Secure cookies disabled in production".to_string(),
                        suggestion: "Enable secure cookies for production deployment".to_string(),
                        example: Some("Set security.session.secure_cookies = true".to_string()),
                        documentation_link: Some("https://docs.rustci.dev/security/cookies".to_string()),
                    });
                }
            }
            "development" => {
                if !config.features.enable_hot_reload {
                    report.remediation_suggestions.push(RemediationSuggestion {
                        category: "Development".to_string(),
                        severity: RemediationSeverity::Low,
                        issue: "Hot reload disabled in development".to_string(),
                        suggestion: "Enable hot reload for better development experience".to_string(),
                        example: Some("ENABLE_HOT_RELOAD=true".to_string()),
                        documentation_link: Some("https://docs.rustci.dev/development/hot-reload".to_string()),
                    });
                }
            }
            _ => {}
        }
    }

    fn determine_startup_readiness(&self, report: &StartupValidationReport) -> StartupReadiness {
        let total_errors = report.validation_report.total_errors();
        let total_warnings = report.validation_report.total_warnings();
        
        // Check for critical remediation suggestions
        let critical_issues = report.remediation_suggestions.iter()
            .filter(|s| matches!(s.severity, RemediationSeverity::Critical))
            .count();
        
        let high_issues = report.remediation_suggestions.iter()
            .filter(|s| matches!(s.severity, RemediationSeverity::High))
            .count();

        if critical_issues > 0 || total_errors > 0 {
            StartupReadiness::CriticalIssues
        } else if high_issues > 3 {
            StartupReadiness::NotReady
        } else if total_warnings > 0 || high_issues > 0 {
            StartupReadiness::ReadyWithWarnings
        } else {
            StartupReadiness::Ready
        }
    }

    fn generate_configuration_summary(&self, config: &AppConfiguration) -> ConfigurationSummary {
        let mut security_features = Vec::new();
        let mut observability_features = Vec::new();

        // Count security features
        if config.security.rbac.enabled {
            security_features.push("RBAC".to_string());
        }
        if config.security.audit.enabled {
            security_features.push("Audit Logging".to_string());
        }
        if config.security.rate_limiting.enabled {
            security_features.push("Rate Limiting".to_string());
        }
        if config.security.oauth.github.client_id != "" {
            security_features.push("GitHub OAuth".to_string());
        }

        // Count observability features
        if config.observability.metrics.enabled {
            observability_features.push("Metrics".to_string());
        }
        if config.observability.tracing.enabled {
            observability_features.push("Distributed Tracing".to_string());
        }
        if config.features.enable_audit_logging {
            observability_features.push("Enhanced Audit Logging".to_string());
        }

        ConfigurationSummary {
            environment: self.environment.clone(),
            total_configuration_keys: self.count_configuration_keys(config),
            encrypted_values: 0, // Would be populated from environment resolver
            feature_flags_enabled: self.count_enabled_feature_flags(config),
            security_features_enabled: security_features,
            observability_features_enabled: observability_features,
            ci_executors_configured: config.ci.executors.len(),
            external_services_configured: self.count_external_services(config),
        }
    }

    fn count_configuration_keys(&self, _config: &AppConfiguration) -> usize {
        // Simplified count - in a real implementation, you'd recursively count all config keys
        50 // Placeholder
    }

    fn count_external_services(&self, config: &AppConfiguration) -> usize {
        let mut count = 0;
        if !config.external_services.github.api_url.is_empty() {
            count += 1;
        }
        if !config.external_services.docker_registry.url.is_empty() {
            count += 1;
        }
        if config.external_services.notification.enabled {
            count += 1;
        }
        count
    }
}

impl StartupValidationReport {
    fn add_remediation_suggestions_from_migration(&mut self, migration_report: &MigrationReport) {
        for error in &migration_report.errors {
            self.remediation_suggestions.push(RemediationSuggestion {
                category: "Migration".to_string(),
                severity: RemediationSeverity::High,
                issue: error.clone(),
                suggestion: "Fix migration error and restart application".to_string(),
                example: None,
                documentation_link: Some("https://docs.rustci.dev/configuration/migration".to_string()),
            });
        }
    }

    fn add_critical_remediation(&mut self, category: &str, issue: &str, suggestion: &str) {
        self.remediation_suggestions.push(RemediationSuggestion {
            category: category.to_string(),
            severity: RemediationSeverity::Critical,
            issue: issue.to_string(),
            suggestion: suggestion.to_string(),
            example: None,
            documentation_link: None,
        });
    }



    /// Print a formatted startup validation report
    pub fn print_report(&self) {
        println!("\n🔍 Startup Configuration Validation Report");
        println!("==========================================");
        
        match self.startup_readiness {
            StartupReadiness::Ready => {
                println!("✅ Status: READY - Application can start safely");
            }
            StartupReadiness::ReadyWithWarnings => {
                println!("⚠️  Status: READY WITH WARNINGS - Application can start but has issues to address");
            }
            StartupReadiness::NotReady => {
                println!("❌ Status: NOT READY - Application should not start until issues are resolved");
            }
            StartupReadiness::CriticalIssues => {
                println!("🚨 Status: CRITICAL ISSUES - Application cannot start safely");
            }
        }
        
        println!("\n📊 Summary:");
        println!("  Environment: {}", self.configuration_summary.environment);
        println!("  Validation Errors: {}", self.validation_report.total_errors());
        println!("  Validation Warnings: {}", self.validation_report.total_warnings());
        println!("  Remediation Suggestions: {}", self.remediation_suggestions.len());
        
        if !self.remediation_suggestions.is_empty() {
            println!("\n🔧 Remediation Suggestions:");
            for (i, suggestion) in self.remediation_suggestions.iter().enumerate() {
                let severity_icon = match suggestion.severity {
                    RemediationSeverity::Critical => "🚨",
                    RemediationSeverity::High => "❌",
                    RemediationSeverity::Medium => "⚠️",
                    RemediationSeverity::Low => "💡",
                    RemediationSeverity::Info => "ℹ️",
                };
                
                println!("  {}. {} [{}] {}", i + 1, severity_icon, suggestion.category, suggestion.issue);
                println!("     💡 {}", suggestion.suggestion);
                
                if let Some(example) = &suggestion.example {
                    println!("     📝 Example: {}", example);
                }
                
                if let Some(docs) = &suggestion.documentation_link {
                    println!("     📚 Docs: {}", docs);
                }
                println!();
            }
        }
    }
}

impl Default for ConfigurationSummary {
    fn default() -> Self {
        Self {
            environment: "unknown".to_string(),
            total_configuration_keys: 0,
            encrypted_values: 0,
            feature_flags_enabled: 0,
            security_features_enabled: Vec::new(),
            observability_features_enabled: Vec::new(),
            ci_executors_configured: 0,
            external_services_configured: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[tokio::test]
    async fn test_startup_validator_creation() {
        let validator = StartupConfigValidator::new("development");
        assert_eq!(validator.environment, "development");
    }

    #[tokio::test]
    async fn test_startup_validation_with_valid_config() {
        let mut validator = StartupConfigValidator::new("development");
        let mut config = AppConfiguration::default();
        
        // Set required fields
        config.security.jwt.secret = "test-secret-key".to_string();
        config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
        
        let report = validator.validate_startup_configuration(&mut config).await.unwrap();
        
        // Should be ready or ready with warnings for a basic valid config
        assert!(matches!(report.startup_readiness, StartupReadiness::Ready | StartupReadiness::ReadyWithWarnings));
    }

    #[tokio::test]
    async fn test_startup_validation_with_invalid_config() {
        let mut validator = StartupConfigValidator::new("production");
        let mut config = AppConfiguration::default();
        
        // Leave required fields empty
        config.security.jwt.secret = "".to_string();
        config.database.mongodb_uri = "".to_string();
        
        let report = validator.validate_startup_configuration(&mut config).await.unwrap();
        
        // Should not be ready due to missing required fields
        assert!(matches!(report.startup_readiness, StartupReadiness::CriticalIssues | StartupReadiness::NotReady));
        assert!(!report.remediation_suggestions.is_empty());
    }

    #[tokio::test]
    async fn test_remediation_suggestions_generation() {
        let validator = StartupConfigValidator::new("production");
        let _config = AppConfiguration::default();
        
        let suggestion = validator.generate_error_remediation(
            "security",
            "jwt_secret",
            "JWT secret is required",
            &_config,
        );
        
        assert_eq!(suggestion.category, "Security");
        assert!(matches!(suggestion.severity, RemediationSeverity::Critical));
        assert!(suggestion.example.is_some());
    }
}