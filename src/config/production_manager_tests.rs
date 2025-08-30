#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfiguration, DatabaseConfig, SecurityConfig, JwtConfig, EncryptionConfig, SessionConfig, CorsConfig};
    use std::collections::HashMap;
    use tokio;

    fn create_test_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "test_secret_that_is_long_enough_for_validation".to_string();
        config.security.encryption.key = "dGVzdF9lbmNyeXB0aW9uX2tleV90aGF0X2lzXzMyX2J5dGVz".to_string(); // base64 encoded 32 bytes
        config
    }

    fn create_insecure_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "short".to_string(); // Too short
        config.security.encryption.key = "".to_string(); // Empty
        config.security.session.secure_cookies = false;
        config.security.cors.allowed_origins = vec!["*".to_string()];
        config
    }

    #[tokio::test]
    async fn test_production_config_manager_creation() {
        let manager = ProductionConfigManager::new("production");
        assert_eq!(manager.environment, "production");
        assert_eq!(manager.validation_rules.len(), 8); // Should have 8 validation rules
    }

    #[tokio::test]
    async fn test_config_hash_calculation() {
        let manager = ProductionConfigManager::new("test");
        let config1 = create_test_config();
        let config2 = create_test_config();
        let config3 = create_insecure_config();

        let hash1 = manager.calculate_config_hash(&config1);
        let hash2 = manager.calculate_config_hash(&config2);
        let hash3 = manager.calculate_config_hash(&config3);

        assert_eq!(hash1, hash2); // Same config should produce same hash
        assert_ne!(hash1, hash3); // Different config should produce different hash
    }

    #[tokio::test]
    async fn test_validation_cache() {
        let manager = ProductionConfigManager::new("test");
        let config = create_test_config();
        let config_hash = manager.calculate_config_hash(&config);

        // Initially no cache
        let cached = manager.check_validation_cache(config_hash).await;
        assert!(cached.is_none());

        // Create a mock report and cache it
        let report = ProductionValidationReport::new("test");
        manager.cache_validation_result(config_hash, &report).await;

        // Should now find cached result
        let cached = manager.check_validation_cache(config_hash).await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_environment_overrides() {
        let mut manager = ProductionConfigManager::new("production");
        let mut config = create_test_config();
        
        // Apply production overrides
        manager.apply_environment_overrides(&mut config).unwrap();

        assert!(config.security.session.secure_cookies);
        assert_eq!(config.observability.logging.level, "warn");
        assert!(!config.features.enable_experimental_features);
        assert!(!config.security.cors.allowed_origins.contains(&"*".to_string()));
    }

    #[tokio::test]
    async fn test_development_environment_overrides() {
        let mut manager = ProductionConfigManager::new("development");
        let mut config = create_test_config();
        
        manager.apply_environment_overrides(&mut config).unwrap();

        assert!(!config.security.session.secure_cookies);
        assert_eq!(config.observability.logging.level, "debug");
    }

    #[tokio::test]
    async fn test_staging_environment_overrides() {
        let mut manager = ProductionConfigManager::new("staging");
        let mut config = create_test_config();
        
        manager.apply_environment_overrides(&mut config).unwrap();

        assert_eq!(config.observability.logging.level, "info");
        assert!(!config.features.enable_experimental_features);
    }

    #[tokio::test]
    async fn test_security_validation_rule() {
        let rule = SecurityValidationRule::new();
        let secure_config = create_test_config();
        let insecure_config = create_insecure_config();

        // Test secure configuration
        let result = rule.validate(&secure_config, "production").unwrap();
        assert_eq!(result.name, "security");
        assert!(matches!(result.status, ValidationStatus::Pass));
        assert!(result.issues.is_empty());
        assert_eq!(result.score, 100);

        // Test insecure configuration
        let result = rule.validate(&insecure_config, "production").unwrap();
        assert_eq!(result.name, "security");
        assert!(matches!(result.status, ValidationStatus::Error));
        assert!(!result.issues.is_empty());
        assert!(result.score < 100);

        // Check that JWT secret issue is detected
        let jwt_issue = result.issues.iter().find(|issue| issue.field_path == "security.jwt.secret");
        assert!(jwt_issue.is_some());
        assert!(matches!(jwt_issue.unwrap().severity, IssueSeverity::High));
    }

    #[tokio::test]
    async fn test_production_validation_report_creation() {
        let report = ProductionValidationReport::new("production");
        
        assert!(report.is_valid);
        assert_eq!(report.validation_metadata.environment, "production");
        assert!(report.validation_sections.is_empty());
        assert!(report.critical_issues.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_validation_report_issue_handling() {
        let mut report = ProductionValidationReport::new("test");
        
        // Add a critical issue
        let critical_issue = ValidationIssue {
            severity: IssueSeverity::Critical,
            category: "security".to_string(),
            field_path: "test.field".to_string(),
            message: "Critical security issue".to_string(),
            description: "This is a critical security issue".to_string(),
            remediation: "Fix immediately".to_string(),
            impact: "System compromise".to_string(),
            auto_fixable: false,
            compliance_impact: Some("GDPR violation".to_string()),
        };

        report.add_critical_issue(critical_issue.clone());
        
        assert!(!report.is_valid); // Should be invalid due to critical issue
        assert_eq!(report.critical_issues.len(), 1);
        assert_eq!(report.critical_issues[0].message, "Critical security issue");

        // Add a warning
        let warning_issue = ValidationIssue {
            severity: IssueSeverity::Medium,
            category: "performance".to_string(),
            field_path: "test.performance".to_string(),
            message: "Performance warning".to_string(),
            description: "This is a performance warning".to_string(),
            remediation: "Optimize configuration".to_string(),
            impact: "Reduced performance".to_string(),
            auto_fixable: true,
            compliance_impact: None,
        };

        report.add_warning(warning_issue.clone());
        
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].message, "Performance warning");
    }

    #[tokio::test]
    async fn test_validation_section_integration() {
        let mut report = ProductionValidationReport::new("test");
        
        let mut section = ValidationSection {
            name: "test_section".to_string(),
            status: ValidationStatus::Warning,
            issues: vec![
                ValidationIssue {
                    severity: IssueSeverity::Medium,
                    category: "test".to_string(),
                    field_path: "test.field".to_string(),
                    message: "Test warning".to_string(),
                    description: "Test description".to_string(),
                    remediation: "Test remediation".to_string(),
                    impact: "Test impact".to_string(),
                    auto_fixable: false,
                    compliance_impact: None,
                }
            ],
            score: 80,
            details: HashMap::new(),
        };

        report.add_validation_section(section);
        
        assert_eq!(report.validation_sections.len(), 1);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.is_valid); // Should still be valid (only warnings)

        // Add a section with critical issues
        let critical_section = ValidationSection {
            name: "critical_section".to_string(),
            status: ValidationStatus::Critical,
            issues: vec![
                ValidationIssue {
                    severity: IssueSeverity::Critical,
                    category: "critical".to_string(),
                    field_path: "critical.field".to_string(),
                    message: "Critical issue".to_string(),
                    description: "Critical description".to_string(),
                    remediation: "Critical remediation".to_string(),
                    impact: "Critical impact".to_string(),
                    auto_fixable: false,
                    compliance_impact: Some("Compliance violation".to_string()),
                }
            ],
            score: 0,
            details: HashMap::new(),
        };

        report.add_validation_section(critical_section);
        
        assert!(!report.is_valid); // Should now be invalid
        assert_eq!(report.critical_issues.len(), 1);
    }

    #[tokio::test]
    async fn test_validation_rule_priority() {
        let security_rule = SecurityValidationRule::new();
        let database_rule = DatabaseValidationRule::new();
        let performance_rule = PerformanceValidationRule::new();

        assert_eq!(security_rule.priority(), 1);
        assert_eq!(database_rule.priority(), 2);
        assert_eq!(performance_rule.priority(), 3);

        // Security should have higher priority (lower number)
        assert!(security_rule.priority() < database_rule.priority());
        assert!(database_rule.priority() < performance_rule.priority());
    }

    #[tokio::test]
    async fn test_validation_rule_categories() {
        let security_rule = SecurityValidationRule::new();
        let database_rule = DatabaseValidationRule::new();
        let performance_rule = PerformanceValidationRule::new();

        assert_eq!(security_rule.category(), "security");
        assert_eq!(database_rule.category(), "infrastructure");
        assert_eq!(performance_rule.category(), "performance");
    }

    #[tokio::test]
    async fn test_validation_rule_names() {
        let security_rule = SecurityValidationRule::new();
        let database_rule = DatabaseValidationRule::new();
        let performance_rule = PerformanceValidationRule::new();

        assert_eq!(security_rule.name(), "security");
        assert_eq!(database_rule.name(), "database");
        assert_eq!(performance_rule.name(), "performance");
    }

    #[tokio::test]
    async fn test_environment_specific_rule_application() {
        let rule = SecurityValidationRule::new();
        
        // All rules should apply to all environments by default
        assert!(rule.applies_to_environment("production"));
        assert!(rule.applies_to_environment("staging"));
        assert!(rule.applies_to_environment("development"));
        assert!(rule.applies_to_environment("test"));
    }

    #[tokio::test]
    async fn test_report_finalization() {
        let mut report = ProductionValidationReport::new("test");
        let duration = std::time::Duration::from_millis(500);
        
        // Add some validation sections first
        let section = ValidationSection {
            name: "test_section".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        };
        report.add_validation_section(section);

        report.finalize_validation(duration);
        
        assert_eq!(report.validation_metadata.validation_duration, duration);
        assert_eq!(report.validation_metadata.validation_rules_applied.len(), 1);
        assert!(report.validation_metadata.validation_rules_applied.contains(&"test_section".to_string()));
    }

    #[tokio::test]
    async fn test_load_from_environment_variables() {
        // Set test environment variables
        std::env::set_var("SERVER_HOST", "test.example.com");
        std::env::set_var("PORT", "9000");
        std::env::set_var("MONGODB_URI", "mongodb://test:27017/testdb");
        std::env::set_var("JWT_SECRET", "test_jwt_secret_for_environment_loading");

        let manager = ProductionConfigManager::new("test");
        let mut config = AppConfiguration::default();
        
        manager.load_from_environment(&mut config).unwrap();
        
        assert_eq!(config.server.host, "test.example.com");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.database.mongodb_uri, "mongodb://test:27017/testdb");
        assert_eq!(config.security.jwt.secret, "test_jwt_secret_for_environment_loading");

        // Clean up environment variables
        std::env::remove_var("SERVER_HOST");
        std::env::remove_var("PORT");
        std::env::remove_var("MONGODB_URI");
        std::env::remove_var("JWT_SECRET");
    }

    #[tokio::test]
    async fn test_invalid_port_environment_variable() {
        std::env::set_var("PORT", "invalid_port");
        
        let manager = ProductionConfigManager::new("test");
        let mut config = AppConfiguration::default();
        
        let result = manager.load_from_environment(&mut config);
        assert!(result.is_err());
        
        if let Err(AppError::ConfigError(msg)) = result {
            assert!(msg.contains("Invalid PORT"));
        } else {
            panic!("Expected ConfigError for invalid port");
        }

        std::env::remove_var("PORT");
    }

    #[tokio::test]
    async fn test_security_assessment_generation() {
        let manager = ProductionConfigManager::new("production");
        let mut report = ProductionValidationReport::new("production");
        
        // Test with secure config
        let secure_config = create_test_config();
        manager.generate_security_assessment(&mut report, &secure_config).await;
        
        assert!(report.security_assessment.overall_score > 80);
        assert_eq!(report.security_assessment.encryption_status, "Configured");
        assert!(report.security_assessment.vulnerabilities.is_empty());

        // Test with insecure config
        let mut insecure_report = ProductionValidationReport::new("production");
        let insecure_config = create_insecure_config();
        manager.generate_security_assessment(&mut insecure_report, &insecure_config).await;
        
        assert!(insecure_report.security_assessment.overall_score < 50);
        assert!(!insecure_report.security_assessment.vulnerabilities.is_empty());
        
        // Should have vulnerabilities for missing encryption, weak JWT, insecure cookies, and wildcard CORS
        assert!(insecure_report.security_assessment.vulnerabilities.len() >= 3);
    }

    #[tokio::test]
    async fn test_performance_assessment_generation() {
        let manager = ProductionConfigManager::new("production");
        let mut report = ProductionValidationReport::new("production");
        
        let mut config = create_test_config();
        config.database.connection_pool_size = 5; // Small pool
        config.server.max_connections = 500; // Low connections
        
        manager.generate_performance_assessment(&mut report, &config).await;
        
        assert!(report.performance_assessment.overall_score < 100);
        assert!(!report.performance_assessment.bottlenecks.is_empty());
        assert!(!report.performance_assessment.optimization_opportunities.is_empty());
        
        // Should identify small connection pool as bottleneck
        let db_bottleneck = report.performance_assessment.bottlenecks.iter()
            .find(|b| b.component == "Database");
        assert!(db_bottleneck.is_some());
    }

    #[tokio::test]
    async fn test_compliance_assessment_generation() {
        let manager = ProductionConfigManager::new("production");
        let mut report = ProductionValidationReport::new("production");
        
        let config = create_test_config();
        manager.generate_compliance_assessment(&mut report, &config).await;
        
        assert!(report.compliance_status.standards.contains_key("GDPR"));
        
        let gdpr_standard = &report.compliance_status.standards["GDPR"];
        assert_eq!(gdpr_standard.name, "GDPR");
        assert_eq!(gdpr_standard.version, "2018");
        assert!(gdpr_standard.compliance_percentage <= 100);
        
        // Should have some implemented controls (encryption and audit logging)
        assert!(!gdpr_standard.implemented_controls.is_empty());
    }

    #[tokio::test]
    async fn test_cross_section_analysis() {
        let manager = ProductionConfigManager::new("production");
        let mut report = ProductionValidationReport::new("production");
        
        let mut config = create_test_config();
        config.ci.max_concurrent_pipelines = 100;
        config.database.connection_pool_size = 10; // Much smaller than pipelines
        config.security.rate_limiting.requests_per_minute = 2000;
        config.server.max_connections = 100; // Much smaller than rate limit
        
        manager.perform_cross_section_analysis(&mut report, &config).await;
        
        // Should detect configuration conflicts
        assert!(!report.warnings.is_empty());
        
        // Should warn about database vs CI mismatch
        let db_ci_warning = report.warnings.iter()
            .find(|w| w.field_path == "ci.max_concurrent_pipelines");
        assert!(db_ci_warning.is_some());
        
        // Should warn about rate limiting vs server capacity mismatch
        let rate_limit_warning = report.warnings.iter()
            .find(|w| w.field_path == "security.rate_limiting");
        assert!(rate_limit_warning.is_some());
    }
}