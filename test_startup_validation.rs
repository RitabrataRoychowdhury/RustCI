use RustAutoDevOps::config::{
    AppConfiguration, StartupConfigValidator, StartupReadiness, RemediationSeverity
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing Enhanced Startup Configuration Validation");
    println!("====================================================");

    // Test 1: Valid configuration
    println!("\n📋 Test 1: Valid Configuration");
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_valid_config();
    
    let report = validator.validate_startup_configuration(&mut config).await?;
    println!("✅ Startup Readiness: {:?}", report.startup_readiness);
    println!("📊 Errors: {}, Warnings: {}", 
             report.validation_report.total_errors(),
             report.validation_report.total_warnings());

    // Test 2: Invalid configuration with remediation suggestions
    println!("\n📋 Test 2: Invalid Configuration with Remediation");
    let mut validator = StartupConfigValidator::new("production");
    let mut config = create_invalid_config();
    
    let report = validator.validate_startup_configuration(&mut config).await?;
    println!("❌ Startup Readiness: {:?}", report.startup_readiness);
    println!("📊 Errors: {}, Warnings: {}", 
             report.validation_report.total_errors(),
             report.validation_report.total_warnings());
    
    println!("🔧 Remediation Suggestions: {}", report.remediation_suggestions.len());
    for (i, suggestion) in report.remediation_suggestions.iter().take(3).enumerate() {
        let severity_icon = match suggestion.severity {
            RemediationSeverity::Critical => "🚨",
            RemediationSeverity::High => "❌",
            RemediationSeverity::Medium => "⚠️",
            RemediationSeverity::Low => "💡",
            RemediationSeverity::Info => "ℹ️",
        };
        println!("  {}. {} [{}] {}", i + 1, severity_icon, suggestion.category, suggestion.issue);
        if let Some(example) = &suggestion.example {
            println!("     📝 Example: {}", example);
        }
    }

    // Test 3: Configuration with migrations
    println!("\n📋 Test 3: Configuration Requiring Migrations");
    let mut validator = StartupConfigValidator::new("development");
    let mut config = create_config_needing_migration();
    
    let report = validator.validate_startup_configuration(&mut config).await?;
    println!("🔄 Startup Readiness: {:?}", report.startup_readiness);
    
    if let Some(migration_report) = &report.migration_report {
        println!("📦 Migrations Applied: {}/{}", 
                 migration_report.successful_migrations,
                 migration_report.total_migrations);
        
        for migration in &migration_report.migrations_applied {
            let status_icon = if migration.success { "✅" } else { "❌" };
            println!("   {} v{}: {}", status_icon, migration.version, migration.description);
        }
    }

    println!("\n✅ Enhanced startup validation testing completed!");
    Ok(())
}

fn create_valid_config() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    config.security.jwt.secret = "test-secret-key-with-sufficient-length".to_string();
    config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
    config.server.host = "localhost".to_string();
    config.server.port = 8000;
    config
}

fn create_invalid_config() -> AppConfiguration {
    let mut config = AppConfiguration::default();
    // Leave critical fields empty to trigger validation errors
    config.security.jwt.secret = "".to_string();
    config.database.mongodb_uri = "".to_string();
    config.server.port = 0;
    config.security.session.secure_cookies = false; // Issue for production
    config.observability.logging.level = "debug".to_string(); // Issue for production
    config
}

fn create_config_needing_migration() -> AppConfiguration {
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