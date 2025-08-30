use crate::config::AppConfiguration;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Configuration migration manager for handling schema changes
pub struct ConfigMigrationManager {
    migrations: Vec<Box<dyn ConfigMigration>>,
    _current_version: u32,
}

/// Configuration migration trait
#[async_trait::async_trait]
pub trait ConfigMigration: Send + Sync {
    /// Get the version this migration targets
    fn target_version(&self) -> u32;
    
    /// Get migration description
    fn description(&self) -> &str;
    
    /// Check if migration is needed
    async fn needs_migration(&self, config: &AppConfiguration) -> Result<bool>;
    
    /// Apply the migration
    async fn migrate(&self, config: &mut AppConfiguration) -> Result<MigrationResult>;
    
    /// Rollback the migration if possible
    async fn rollback(&self, config: &mut AppConfiguration) -> Result<MigrationResult>;
}

/// Migration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub success: bool,
    pub changes_made: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Migration report for all applied migrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub migrations_applied: Vec<MigrationSummary>,
    pub total_migrations: usize,
    pub successful_migrations: usize,
    pub failed_migrations: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub version: u32,
    pub description: String,
    pub success: bool,
    pub changes: Vec<String>,
}

impl ConfigMigrationManager {
    pub fn new() -> Self {
        let mut manager = Self {
            migrations: Vec::new(),
            _current_version: 1,
        };
        
        // Register default migrations
        manager.register_default_migrations();
        manager
    }
    
    fn register_default_migrations(&mut self) {
        // Migration from v1 to v2: Add new security fields
        self.add_migration(Box::new(SecurityEnhancementMigration));
        
        // Migration from v2 to v3: Add observability enhancements
        self.add_migration(Box::new(ObservabilityEnhancementMigration));
        
        // Migration from v3 to v4: Add Valkyrie integration
        self.add_migration(Box::new(ValkyrieIntegrationMigration));
        
        // Migration from v4 to v5: Add environment-specific configurations
        self.add_migration(Box::new(EnvironmentConfigMigration));
    }
    
    pub fn add_migration(&mut self, migration: Box<dyn ConfigMigration>) {
        self.migrations.push(migration);
        // Sort migrations by version
        self.migrations.sort_by_key(|m| m.target_version());
    }
    
    /// Check if any migrations are needed
    pub async fn needs_migration(&self, config: &AppConfiguration) -> Result<bool> {
        for migration in &self.migrations {
            if migration.needs_migration(config).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    
    /// Apply all needed migrations
    pub async fn apply_migrations(&self, config: &mut AppConfiguration) -> Result<MigrationReport> {
        info!("🔄 Checking for configuration migrations...");
        
        let mut report = MigrationReport {
            migrations_applied: Vec::new(),
            total_migrations: 0,
            successful_migrations: 0,
            failed_migrations: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        for migration in &self.migrations {
            if migration.needs_migration(config).await? {
                info!("Applying migration to version {}: {}", 
                      migration.target_version(), migration.description());
                
                report.total_migrations += 1;
                
                match migration.migrate(config).await {
                    Ok(result) => {
                        if result.success {
                            report.successful_migrations += 1;
                            info!("✅ Migration to version {} completed successfully", 
                                  migration.target_version());
                        } else {
                            report.failed_migrations += 1;
                            warn!("⚠️ Migration to version {} completed with issues", 
                                  migration.target_version());
                        }
                        
                        report.migrations_applied.push(MigrationSummary {
                            version: migration.target_version(),
                            description: migration.description().to_string(),
                            success: result.success,
                            changes: result.changes_made,
                        });
                        
                        report.warnings.extend(result.warnings);
                        report.errors.extend(result.errors);
                    }
                    Err(e) => {
                        report.failed_migrations += 1;
                        let error_msg = format!("Migration to version {} failed: {}", 
                                               migration.target_version(), e);
                        report.errors.push(error_msg.clone());
                        warn!("❌ {}", error_msg);
                    }
                }
            }
        }
        
        if report.total_migrations == 0 {
            info!("✅ No configuration migrations needed");
        } else {
            info!("🔄 Applied {}/{} migrations successfully", 
                  report.successful_migrations, report.total_migrations);
        }
        
        Ok(report)
    }
}

impl Default for ConfigMigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Migration to add enhanced security fields (v1 -> v2)
struct SecurityEnhancementMigration;

#[async_trait::async_trait]
impl ConfigMigration for SecurityEnhancementMigration {
    fn target_version(&self) -> u32 { 2 }
    
    fn description(&self) -> &str {
        "Add enhanced security configuration fields"
    }
    
    async fn needs_migration(&self, config: &AppConfiguration) -> Result<bool> {
        // Check if new security fields are missing or have default values
        Ok(config.security.rbac.role_hierarchy.is_empty() ||
           config.security.audit.retention_days == 0 ||
           config.security.session.timeout_minutes == 0)
    }
    
    async fn migrate(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        // Add default RBAC role hierarchy if missing
        if config.security.rbac.role_hierarchy.is_empty() {
            config.security.rbac.role_hierarchy.insert(
                "Admin".to_string(),
                vec!["Developer".to_string(), "Viewer".to_string()],
            );
            config.security.rbac.role_hierarchy.insert(
                "Developer".to_string(),
                vec!["Viewer".to_string()],
            );
            result.changes_made.push("Added default RBAC role hierarchy".to_string());
        }
        
        // Set default audit retention if not set
        if config.security.audit.retention_days == 0 {
            config.security.audit.retention_days = 90;
            result.changes_made.push("Set default audit retention to 90 days".to_string());
        }
        
        // Set default session timeout if not set
        if config.security.session.timeout_minutes == 0 {
            config.security.session.timeout_minutes = 60;
            result.changes_made.push("Set default session timeout to 60 minutes".to_string());
        }
        
        Ok(result)
    }
    
    async fn rollback(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        // Clear added fields
        config.security.rbac.role_hierarchy.clear();
        config.security.audit.retention_days = 0;
        config.security.session.timeout_minutes = 0;
        
        result.changes_made.push("Removed enhanced security fields".to_string());
        Ok(result)
    }
}

/// Migration to add observability enhancements (v2 -> v3)
struct ObservabilityEnhancementMigration;

#[async_trait::async_trait]
impl ConfigMigration for ObservabilityEnhancementMigration {
    fn target_version(&self) -> u32 { 3 }
    
    fn description(&self) -> &str {
        "Add enhanced observability configuration"
    }
    
    async fn needs_migration(&self, config: &AppConfiguration) -> Result<bool> {
        Ok(config.observability.metrics.custom_metrics.is_empty() ||
           config.observability.tracing.service_name.is_empty())
    }
    
    async fn migrate(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        // Add default custom metrics if missing
        if config.observability.metrics.custom_metrics.is_empty() {
            config.observability.metrics.custom_metrics.insert(
                "pipeline_duration".to_string(),
                "histogram".to_string(),
            );
            config.observability.metrics.custom_metrics.insert(
                "active_connections".to_string(),
                "gauge".to_string(),
            );
            result.changes_made.push("Added default custom metrics".to_string());
        }
        
        // Set default service name if empty
        if config.observability.tracing.service_name.is_empty() {
            config.observability.tracing.service_name = "rustci".to_string();
            result.changes_made.push("Set default tracing service name".to_string());
        }
        
        Ok(result)
    }
    
    async fn rollback(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        config.observability.metrics.custom_metrics.clear();
        config.observability.tracing.service_name = String::new();
        
        result.changes_made.push("Removed observability enhancements".to_string());
        Ok(result)
    }
}

/// Migration to add Valkyrie integration (v3 -> v4)
struct ValkyrieIntegrationMigration;

#[async_trait::async_trait]
impl ConfigMigration for ValkyrieIntegrationMigration {
    fn target_version(&self) -> u32 { 4 }
    
    fn description(&self) -> &str {
        "Add Valkyrie protocol integration configuration"
    }
    
    async fn needs_migration(&self, config: &AppConfiguration) -> Result<bool> {
        Ok(config.valkyrie.is_none())
    }
    
    async fn migrate(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        if config.valkyrie.is_none() {
            config.valkyrie = Some(crate::config::app_config::ValkyrieIntegrationConfig {
                enabled: false,
                config_source: "environment".to_string(),
                fallback_to_legacy: true,
                enabled_for_node_types: vec!["worker".to_string(), "hybrid".to_string()],
                features: crate::config::app_config::ValkyrieFeatureFlags {
                    advanced_routing: true,
                    stream_multiplexing: true,
                    post_quantum_crypto: false,
                    ml_optimizations: false,
                    zero_copy: true,
                    simd: true,
                },
            });
            result.changes_made.push("Added Valkyrie integration configuration".to_string());
        }
        
        Ok(result)
    }
    
    async fn rollback(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        config.valkyrie = None;
        result.changes_made.push("Removed Valkyrie integration configuration".to_string());
        Ok(result)
    }
}

/// Migration to add environment-specific configurations (v4 -> v5)
struct EnvironmentConfigMigration;

#[async_trait::async_trait]
impl ConfigMigration for EnvironmentConfigMigration {
    fn target_version(&self) -> u32 { 5 }
    
    fn description(&self) -> &str {
        "Add environment-specific configuration support"
    }
    
    async fn needs_migration(&self, config: &AppConfiguration) -> Result<bool> {
        // Check if environment-specific features are missing
        let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        
        match environment.as_str() {
            "production" => {
                Ok(config.security.session.secure_cookies == false ||
                   config.observability.logging.level == "debug" ||
                   config.features.enable_experimental_features)
            }
            "development" => {
                Ok(!config.features.enable_hot_reload)
            }
            _ => Ok(false)
        }
    }
    
    async fn migrate(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        
        match environment.as_str() {
            "production" => {
                if !config.security.session.secure_cookies {
                    config.security.session.secure_cookies = true;
                    result.changes_made.push("Enabled secure cookies for production".to_string());
                }
                
                if config.observability.logging.level == "debug" || config.observability.logging.level == "trace" {
                    config.observability.logging.level = "info".to_string();
                    result.changes_made.push("Changed logging level to info for production".to_string());
                }
                
                if config.features.enable_experimental_features {
                    config.features.enable_experimental_features = false;
                    result.changes_made.push("Disabled experimental features for production".to_string());
                }
            }
            "development" => {
                if !config.features.enable_hot_reload {
                    config.features.enable_hot_reload = true;
                    result.changes_made.push("Enabled hot reload for development".to_string());
                }
            }
            _ => {
                result.warnings.push(format!("Unknown environment: {}", environment));
            }
        }
        
        Ok(result)
    }
    
    async fn rollback(&self, config: &mut AppConfiguration) -> Result<MigrationResult> {
        let mut result = MigrationResult {
            success: true,
            changes_made: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        // Reset to default values
        config.security.session.secure_cookies = false;
        config.observability.logging.level = "info".to_string();
        config.features.enable_experimental_features = false;
        config.features.enable_hot_reload = true;
        
        result.changes_made.push("Reset environment-specific configurations".to_string());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_migration_manager_creation() {
        let manager = ConfigMigrationManager::new();
        assert!(!manager.migrations.is_empty());
    }
    
    #[tokio::test]
    async fn test_security_enhancement_migration() {
        let migration = SecurityEnhancementMigration;
        let mut config = AppConfiguration::default();
        
        // Clear fields to simulate old config
        config.security.rbac.role_hierarchy.clear();
        config.security.audit.retention_days = 0;
        config.security.session.timeout_minutes = 0;
        
        assert!(migration.needs_migration(&config).await.unwrap());
        
        let result = migration.migrate(&mut config).await.unwrap();
        assert!(result.success);
        assert!(!result.changes_made.is_empty());
        assert!(!config.security.rbac.role_hierarchy.is_empty());
        assert!(config.security.audit.retention_days > 0);
        assert!(config.security.session.timeout_minutes > 0);
    }
    
    #[tokio::test]
    async fn test_valkyrie_integration_migration() {
        let migration = ValkyrieIntegrationMigration;
        let mut config = AppConfiguration::default();
        config.valkyrie = None;
        
        assert!(migration.needs_migration(&config).await.unwrap());
        
        let result = migration.migrate(&mut config).await.unwrap();
        assert!(result.success);
        assert!(config.valkyrie.is_some());
    }
    
    #[tokio::test]
    async fn test_full_migration_process() {
        let manager = ConfigMigrationManager::new();
        let mut config = AppConfiguration::default();
        
        // Simulate old config
        config.security.rbac.role_hierarchy.clear();
        config.valkyrie = None;
        
        let report = manager.apply_migrations(&mut config).await.unwrap();
        assert!(report.successful_migrations > 0);
        assert!(!config.security.rbac.role_hierarchy.is_empty());
        assert!(config.valkyrie.is_some());
    }
}