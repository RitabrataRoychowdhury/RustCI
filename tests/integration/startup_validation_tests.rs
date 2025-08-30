use RustAutoDevOps::config::{
    AppConfiguration, StartupConfigValidator, StartupReadiness, RemediationSeverity
};
use std::io::Write;
use tempfile::TempDir;

#[tokio::test]
async fn test_startup_validation_with_valid_config() {
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_valid_test_config();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    assert!(matches!(report.startup_readiness, StartupReadiness::Ready | StartupReadiness::ReadyWithWarnings));
    assert!(report.validation_report.total_errors() == 0);
}

#[tokio::test]
async fn test_startup_validation_with_invalid_config() {
    let mut validator = StartupConfigValidator::new("production");
    let mut config = create_invalid_test_config();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    assert!(matches!(report.startup_readiness, StartupReadiness::CriticalIssues | StartupReadiness::NotReady));
    assert!(report.validation_report.total_errors() > 0);
    assert!(!report.remediation_suggestions.is_empty());
}

#[tokio::test]
async fn test_startup_validation_with_environment_config() {
    let temp_dir = TempDir::new().unwrap();
    let base_config = create_valid_test_config();
    
    // Create environment-specific config
    create_environment_config(&temp_dir, "development", r#"
name: "development"
config_overrides:
  observability:
    logging:
      level: "debug"
feature_flags:
  enable_hot_reload: true
  enable_experimental_features: true
"#).await;

    let mut validator = StartupConfigValidator::with_environment_support(
        "development",
        temp_dir.path(),
        base_config,
    ).await.unwrap();
    
    let mut config = create_valid_test_config();
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    assert!(report.environment_resolution_report.is_some());
    let env_report = report.environment_resolution_report.unwrap();
    assert!(env_report.environment_overrides_applied);
    assert_eq!(config.observability.logging.level, "debug");
}

#[tokio::test]
async fn test_migration_during_startup_validation() {
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_config_needing_migration();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    assert!(report.migration_report.is_some());
    let migration_report = report.migration_report.unwrap();
    assert!(migration_report.successful_migrations > 0);
    
    // Verify migrations were applied
    assert!(!config.security.rbac.role_hierarchy.is_empty());
    assert!(config.valkyrie.is_some());
}

#[tokio::test]
async fn test_production_specific_validation() {
    let mut validator = StartupConfigValidator::new("production");
    let mut config = create_development_config_for_production();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    // Should have warnings/errors about development settings in production
    let production_issues: Vec<_> = report.remediation_suggestions.iter()
        .filter(|s| s.category == "Production" || s.category == "Security")
        .collect();
    
    assert!(!production_issues.is_empty());
    
    // Should suggest enabling secure cookies
    let secure_cookie_suggestion = production_issues.iter()
        .find(|s| s.issue.contains("secure cookies"));
    assert!(secure_cookie_suggestion.is_some());
}

#[tokio::test]
async fn test_remediation_suggestions_generation() {
    let mut validator = StartupConfigValidator::new("production");
    let mut config = AppConfiguration::default();
    
    // Create config with known issues
    config.security.jwt.secret = "".to_string();
    config.database.mongodb_uri = "".to_string();
    config.security.session.secure_cookies = false;
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    // Should have critical suggestions for missing JWT secret and MongoDB URI
    let critical_suggestions: Vec<_> = report.remediation_suggestions.iter()
        .filter(|s| matches!(s.severity, RemediationSeverity::Critical))
        .collect();
    
    assert!(critical_suggestions.len() >= 2);
    
    // Should have examples and documentation links
    let jwt_suggestion = critical_suggestions.iter()
        .find(|s| s.issue.contains("JWT") || s.issue.contains("secret"));
    assert!(jwt_suggestion.is_some());
    assert!(jwt_suggestion.unwrap().example.is_some());
    assert!(jwt_suggestion.unwrap().documentation_link.is_some());
}

#[tokio::test]
async fn test_configuration_summary_generation() {
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_comprehensive_test_config();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    let summary = &report.configuration_summary;
    assert_eq!(summary.environment, "development");
    assert!(summary.feature_flags_enabled > 0);
    assert!(!summary.security_features_enabled.is_empty());
    assert!(!summary.observability_features_enabled.is_empty());
    assert!(summary.ci_executors_configured > 0);
}

#[tokio::test]
async fn test_startup_readiness_determination() {
    // Test Ready state
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_valid_test_config();
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    assert!(matches!(report.startup_readiness, StartupReadiness::Ready | StartupReadiness::ReadyWithWarnings));
    
    // Test CriticalIssues state
    let mut validator = StartupConfigValidator::new("production");
    let mut config = AppConfiguration::default();
    config.security.jwt.secret = "".to_string();
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    assert!(matches!(report.startup_readiness, StartupReadiness::CriticalIssues));
}

#[tokio::test]
async fn test_enhanced_error_reporting_and_remediation() {
    let mut validator = StartupConfigValidator::new("production");
    let mut config = create_config_with_multiple_issues();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    // Should have multiple remediation suggestions
    assert!(report.remediation_suggestions.len() >= 3);
    
    // Should have critical, high, and medium severity suggestions
    let critical_count = report.remediation_suggestions.iter()
        .filter(|s| matches!(s.severity, RemediationSeverity::Critical))
        .count();
    let high_count = report.remediation_suggestions.iter()
        .filter(|s| matches!(s.severity, RemediationSeverity::High))
        .count();
    
    assert!(critical_count > 0);
    assert!(high_count > 0);
    
    // Should have detailed suggestions with examples
    let suggestions_with_examples = report.remediation_suggestions.iter()
        .filter(|s| s.example.is_some())
        .count();
    assert!(suggestions_with_examples > 0);
    
    // Should have documentation links
    let suggestions_with_docs = report.remediation_suggestions.iter()
        .filter(|s| s.documentation_link.is_some())
        .count();
    assert!(suggestions_with_docs > 0);
}

#[tokio::test]
async fn test_configuration_migration_integration() {
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_config_requiring_multiple_migrations();
    
    let report = validator.validate_startup_configuration(&mut config).await.unwrap();
    
    // Should have migration report
    assert!(report.migration_report.is_some());
    let migration_report = report.migration_report.unwrap();
    
    // Should have applied multiple migrations
    assert!(migration_report.total_migrations > 0);
    assert!(migration_report.successful_migrations > 0);
    
    // Should have detailed migration information
    assert!(!migration_report.migrations_applied.is_empty());
    
    // Verify specific migrations were applied
    assert!(config.security.rbac.role_hierarchy.len() > 0);
    assert!(config.valkyrie.is_some());
    
    // Should be ready after migrations
    assert!(matches!(report.startup_readiness, StartupReadiness::Ready | StartupReadiness::ReadyWithWarnings));
}

// Helper functions

fn create_valid_test_config() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    config.security.jwt.secret = "test-secret-key-with-sufficient-length".to_string();
    config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
    config.server.host = "localhost".to_string();
    config.server.port = 8000;
    config
}

fn create_invalid_test_config() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    config.security.jwt.secret = "".to_string();
    config.database.mongodb_uri = "".to_string();
    config.server.port = 0;
    config
}

fn create_config_needing_migration() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    config.security.jwt.secret = "test-secret".to_string();
    config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
    
    // Clear fields that should be migrated
    config.security.rbac.role_hierarchy.clear();
    config.security.audit.retention_days = 0;
    config.valkyrie = None;
    
    config
}

fn create_development_config_for_production() -> AppConfiguration {
    let mut config = create_valid_test_config();
    
    // Set development-like settings that should trigger warnings in production
    config.observability.logging.level = "debug".to_string();
    config.security.session.secure_cookies = false;
    config.features.enable_experimental_features = true;
    config.security.cors.allowed_origins = vec!["*".to_string()];
    
    config
}

fn create_comprehensive_test_config() -> AppConfiguration {
    let mut config = create_valid_test_config();
    
    // Enable various features
    config.features.enable_audit_logging = true;
    config.features.enable_metrics_collection = true;
    config.features.enable_distributed_tracing = true;
    config.features.enable_hot_reload = true;
    
    // Configure security features
    config.security.rbac.enabled = true;
    config.security.audit.enabled = true;
    config.security.rate_limiting.enabled = true;
    config.security.oauth.github.client_id = "test-client-id".to_string();
    
    // Configure observability
    config.observability.metrics.enabled = true;
    config.observability.tracing.enabled = true;
    
    // Add CI executors
    config.ci.executors.insert(
        "docker".to_string(),
        RustAutoDevOps::config::ExecutorConfig {
            enabled: true,
            max_concurrent_jobs: 5,
            resource_limits: RustAutoDevOps::config::ResourceLimits {
                cpu_cores: Some(2.0),
                memory_mb: Some(2048),
                disk_mb: Some(10240),
                network_bandwidth_mbps: None,
            },
            timeout_seconds: 1800,
            retry_attempts: 2,
            config: std::collections::HashMap::new(),
        },
    );
    
    config
}

fn create_config_with_multiple_issues() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    
    // Create multiple configuration issues
    config.security.jwt.secret = "".to_string(); // Critical: missing JWT secret
    config.database.mongodb_uri = "".to_string(); // Critical: missing database URI
    config.server.port = 0; // High: invalid port
    config.security.session.secure_cookies = false; // High: insecure cookies in production
    config.observability.logging.level = "debug".to_string(); // Medium: debug logging in production
    config.security.cors.allowed_origins = vec!["*".to_string()]; // Medium: overly permissive CORS
    
    config
}

fn create_config_requiring_multiple_migrations() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    
    // Set basic required fields
    config.security.jwt.secret = "test-secret-key".to_string();
    config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
    
    // Clear fields that should trigger migrations
    config.security.rbac.role_hierarchy.clear();
    config.security.audit.retention_days = 0;
    config.security.session.timeout_minutes = 0;
    config.observability.metrics.custom_metrics.clear();
    config.observability.tracing.service_name = String::new();
    config.valkyrie = None;
    
    config
}

async fn create_environment_config(temp_dir: &TempDir, env_name: &str, config_content: &str) {
    let config_path = temp_dir.path().join(format!("env-{}.yaml", env_name));
    let mut file = std::fs::File::create(&config_path).unwrap();
    file.write_all(config_content.as_bytes()).unwrap();
}