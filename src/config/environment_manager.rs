use crate::config::{AppConfiguration, ProductionValidationReport};
use crate::error::{AppError, Result};
use base64ct::Encoding;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Environment-specific configuration resolver with inheritance and encryption support
pub struct EnvironmentConfigResolver {
    base_config: AppConfiguration,
    environment_overrides: HashMap<String, EnvironmentConfig>,
    encryption_service: EncryptionService,
    config_inheritance: ConfigInheritance,
}

/// Environment-specific configuration with inheritance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub name: String,
    pub inherits_from: Option<String>,
    pub config_overrides: serde_json::Value,
    pub encrypted_values: HashMap<String, EncryptedValue>,
    pub feature_flags: HashMap<String, bool>,
    pub resource_limits: Option<ResourceLimits>,
    pub deployment_config: Option<DeploymentConfig>,
    pub validation_rules: Option<ValidationRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedValue {
    pub encrypted_data: String,
    pub encryption_method: EncryptionMethod,
    pub key_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionMethod {
    Aes256Gcm,
    ChaCha20Poly1305,
    Rsa4096,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_cores: Option<f64>,
    pub max_disk_gb: Option<u64>,
    pub max_network_mbps: Option<u64>,
    pub max_concurrent_connections: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub deployment_strategy: DeploymentStrategy,
    pub health_check_config: HealthCheckConfig,
    pub scaling_config: ScalingConfig,
    pub monitoring_config: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStrategy {
    BlueGreen,
    RollingUpdate,
    Canary { percentage: u8 },
    Recreate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub interval_seconds: u64,
    pub timeout_seconds: u64,
    pub failure_threshold: u32,
    pub success_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingConfig {
    pub auto_scaling_enabled: bool,
    pub min_instances: u32,
    pub max_instances: u32,
    pub target_cpu_utilization: f64,
    pub target_memory_utilization: f64,
    pub scale_up_cooldown_seconds: u64,
    pub scale_down_cooldown_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub metrics_enabled: bool,
    pub logging_level: String,
    pub tracing_enabled: bool,
    pub alert_rules: Vec<AlertRule>,
    pub dashboard_config: Option<DashboardConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub condition: String,
    pub threshold: f64,
    pub duration_seconds: u64,
    pub severity: AlertSeverity,
    pub notification_channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub enabled: bool,
    pub refresh_interval_seconds: u64,
    pub panels: Vec<DashboardPanel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardPanel {
    pub title: String,
    pub panel_type: PanelType,
    pub query: String,
    pub time_range: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PanelType {
    Graph,
    SingleStat,
    Table,
    Heatmap,
    Logs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub required_fields: Vec<String>,
    pub custom_validators: Vec<CustomValidator>,
    pub schema_validation: bool,
    pub strict_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomValidator {
    pub name: String,
    pub field_path: String,
    pub validation_type: ValidationType,
    pub parameters: HashMap<String, serde_json::Value>,
    pub error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    Range { min: f64, max: f64 },
    Regex { pattern: String },
    Enum { values: Vec<String> },
    Custom { script: String },
}

/// Configuration inheritance system
pub struct ConfigInheritance {
    inheritance_graph: HashMap<String, Vec<String>>,
}

/// Encryption service for sensitive configuration values
pub struct EncryptionService {
    encryption_keys: HashMap<String, EncryptionKey>,
    default_method: EncryptionMethod,
}

#[derive(Debug, Clone)]
struct EncryptionKey {
    key_data: Vec<u8>,
    method: EncryptionMethod,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl EnvironmentConfigResolver {
    /// Create a new environment configuration resolver
    pub fn new(base_config: AppConfiguration) -> Self {
        Self {
            base_config,
            environment_overrides: HashMap::new(),
            encryption_service: EncryptionService::new(),
            config_inheritance: ConfigInheritance::new(),
        }
    }

    /// Load environment configurations from directory
    pub async fn load_environment_configs<P: AsRef<Path>>(&mut self, config_dir: P) -> Result<()> {
        let config_dir = config_dir.as_ref();
        info!("Loading environment configurations from: {}", config_dir.display());

        let mut entries = tokio::fs::read_dir(config_dir).await
            .map_err(|e| AppError::ConfigError(format!("Failed to read config directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| AppError::ConfigError(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "yaml" || extension == "yml" || extension == "json" {
                        if let Some(stem) = path.file_stem() {
                            if let Some(env_name) = stem.to_str() {
                                if env_name.starts_with("env-") {
                                    let environment_name = &env_name[4..]; // Remove "env-" prefix
                                    self.load_environment_config(&path, environment_name).await?;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Build inheritance graph
        self.build_inheritance_graph()?;

        info!("Loaded {} environment configurations", self.environment_overrides.len());
        Ok(())
    }

    /// Load a specific environment configuration file
    async fn load_environment_config<P: AsRef<Path>>(&mut self, config_path: P, environment_name: &str) -> Result<()> {
        let config_path = config_path.as_ref();
        debug!("Loading environment config: {} from {}", environment_name, config_path.display());

        let content = tokio::fs::read_to_string(config_path).await
            .map_err(|e| AppError::ConfigError(format!("Failed to read config file: {}", e)))?;

        let env_config: EnvironmentConfig = if config_path.extension().unwrap() == "json" {
            serde_json::from_str(&content)
                .map_err(|e| AppError::ConfigError(format!("Invalid JSON config: {}", e)))?
        } else {
            serde_yaml::from_str(&content)
                .map_err(|e| AppError::ConfigError(format!("Invalid YAML config: {}", e)))?
        };

        self.environment_overrides.insert(environment_name.to_string(), env_config);
        Ok(())
    }

    /// Build inheritance graph for configuration resolution
    fn build_inheritance_graph(&mut self) -> Result<()> {
        let mut graph = HashMap::new();
        
        for (env_name, env_config) in &self.environment_overrides {
            let mut parents = Vec::new();
            if let Some(parent) = &env_config.inherits_from {
                parents.push(parent.clone());
            }
            graph.insert(env_name.clone(), parents);
        }

        // Validate no circular dependencies
        self.validate_inheritance_graph(&graph)?;
        
        self.config_inheritance.inheritance_graph = graph;
        Ok(())
    }

    /// Validate inheritance graph for circular dependencies
    fn validate_inheritance_graph(&self, graph: &HashMap<String, Vec<String>>) -> Result<()> {
        for env_name in graph.keys() {
            let mut visited = std::collections::HashSet::new();
            let mut path = Vec::new();
            
            if self.has_circular_dependency(env_name, graph, &mut visited, &mut path) {
                return Err(AppError::ConfigError(format!(
                    "Circular dependency detected in environment inheritance: {}",
                    path.join(" -> ")
                )));
            }
        }
        Ok(())
    }

    /// Check for circular dependencies in inheritance
    fn has_circular_dependency(
        &self,
        env_name: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut std::collections::HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        if path.contains(&env_name.to_string()) {
            path.push(env_name.to_string());
            return true;
        }

        if visited.contains(env_name) {
            return false;
        }

        visited.insert(env_name.to_string());
        path.push(env_name.to_string());

        if let Some(parents) = graph.get(env_name) {
            for parent in parents {
                if self.has_circular_dependency(parent, graph, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        false
    }

    /// Resolve configuration for a specific environment
    pub async fn resolve_environment_config(&self, environment: &str) -> Result<AppConfiguration> {
        info!("Resolving configuration for environment: {}", environment);

        let mut resolved_config = self.base_config.clone();

        // Get inheritance chain
        let inheritance_chain = self.get_inheritance_chain(environment)?;
        
        // Apply configurations in inheritance order (parent to child)
        for env_name in inheritance_chain.iter().rev() {
            if let Some(env_config) = self.environment_overrides.get(env_name) {
                self.apply_environment_overrides(&mut resolved_config, env_config).await?;
            }
        }

        // Apply environment-specific validation
        self.validate_environment_config(&resolved_config, environment).await?;

        info!("Successfully resolved configuration for environment: {}", environment);
        Ok(resolved_config)
    }

    /// Get inheritance chain for an environment
    fn get_inheritance_chain(&self, environment: &str) -> Result<Vec<String>> {
        let mut chain = Vec::new();
        let mut current = environment.to_string();

        while let Some(env_config) = self.environment_overrides.get(&current) {
            chain.push(current.clone());
            
            if let Some(parent) = &env_config.inherits_from {
                current = parent.clone();
            } else {
                break;
            }
        }

        Ok(chain)
    }

    /// Apply environment-specific overrides to configuration
    async fn apply_environment_overrides(
        &self,
        config: &mut AppConfiguration,
        env_config: &EnvironmentConfig,
    ) -> Result<()> {
        // Apply JSON overrides
        self.apply_json_overrides(config, &env_config.config_overrides)?;

        // Decrypt and apply encrypted values
        for (field_path, encrypted_value) in &env_config.encrypted_values {
            let decrypted_value = self.encryption_service.decrypt(encrypted_value).await?;
            self.set_config_field(config, field_path, &decrypted_value)?;
        }

        // Apply feature flags
        for (flag_name, flag_value) in &env_config.feature_flags {
            self.set_feature_flag(config, flag_name, *flag_value);
        }

        // Apply resource limits
        if let Some(resource_limits) = &env_config.resource_limits {
            self.apply_resource_limits(config, resource_limits);
        }

        Ok(())
    }

    /// Apply JSON overrides to configuration
    fn apply_json_overrides(&self, config: &mut AppConfiguration, overrides: &serde_json::Value) -> Result<()> {
        if let serde_json::Value::Object(override_map) = overrides {
            for (key, value) in override_map {
                self.set_config_field_from_json(config, key, value)?;
            }
        }
        Ok(())
    }

    /// Set configuration field from JSON value
    fn set_config_field_from_json(&self, config: &mut AppConfiguration, field_path: &str, value: &serde_json::Value) -> Result<()> {
        match field_path {
            "server.host" => {
                if let serde_json::Value::String(host) = value {
                    config.server.host = host.clone();
                }
            }
            "server.port" => {
                if let serde_json::Value::Number(port) = value {
                    if let Some(port_u64) = port.as_u64() {
                        config.server.port = port_u64 as u16;
                    }
                }
            }
            "database.mongodb_uri" => {
                if let serde_json::Value::String(uri) = value {
                    config.database.mongodb_uri = uri.clone();
                }
            }
            "security.jwt.secret" => {
                if let serde_json::Value::String(secret) = value {
                    config.security.jwt.secret = secret.clone();
                }
            }
            "observability.logging.level" => {
                if let serde_json::Value::String(level) = value {
                    config.observability.logging.level = level.clone();
                }
            }
            _ => {
                warn!("Unknown configuration field path: {}", field_path);
            }
        }
        Ok(())
    }

    /// Set configuration field from decrypted value
    fn set_config_field(&self, config: &mut AppConfiguration, field_path: &str, value: &str) -> Result<()> {
        match field_path {
            "security.jwt.secret" => config.security.jwt.secret = value.to_string(),
            "security.encryption.key" => config.security.encryption.key = value.to_string(),
            "database.mongodb_uri" => config.database.mongodb_uri = value.to_string(),
            "security.oauth.github.client_secret" => config.security.oauth.github.client_secret = value.to_string(),
            _ => {
                warn!("Unknown encrypted field path: {}", field_path);
            }
        }
        Ok(())
    }

    /// Set feature flag
    fn set_feature_flag(&self, config: &mut AppConfiguration, flag_name: &str, flag_value: bool) {
        match flag_name {
            "enable_metrics_collection" => config.features.enable_metrics_collection = flag_value,
            "enable_audit_logging" => config.features.enable_audit_logging = flag_value,
            "enable_distributed_tracing" => config.features.enable_distributed_tracing = flag_value,
            "enable_hot_reload" => config.features.enable_hot_reload = flag_value,
            "enable_experimental_features" => config.features.enable_experimental_features = flag_value,
            "enable_valkyrie_protocol" => config.features.enable_valkyrie_protocol = flag_value,
            _ => {
                config.features.custom_flags.insert(flag_name.to_string(), flag_value);
            }
        }
    }

    /// Apply resource limits
    fn apply_resource_limits(&self, config: &mut AppConfiguration, limits: &ResourceLimits) {
        if let Some(max_connections) = limits.max_concurrent_connections {
            config.server.max_connections = max_connections;
        }
        
        // Apply limits to CI configuration
        if let Some(max_memory) = limits.max_memory_mb {
            for executor_config in config.ci.executors.values_mut() {
                executor_config.resource_limits.memory_mb = Some(max_memory);
            }
        }
        
        if let Some(max_cpu) = limits.max_cpu_cores {
            for executor_config in config.ci.executors.values_mut() {
                executor_config.resource_limits.cpu_cores = Some(max_cpu);
            }
        }
    }

    /// Validate environment-specific configuration
    async fn validate_environment_config(&self, config: &AppConfiguration, environment: &str) -> Result<()> {
        if let Some(env_config) = self.environment_overrides.get(environment) {
            if let Some(validation_rules) = &env_config.validation_rules {
                // Check required fields
                for required_field in &validation_rules.required_fields {
                    if !self.is_field_set(config, required_field) {
                        return Err(AppError::ConfigError(format!(
                            "Required field '{}' is not set for environment '{}'",
                            required_field, environment
                        )));
                    }
                }

                // Apply custom validators
                for validator in &validation_rules.custom_validators {
                    self.apply_custom_validator(config, validator)?;
                }
            }
        }
        Ok(())
    }

    /// Check if a configuration field is set
    fn is_field_set(&self, config: &AppConfiguration, field_path: &str) -> bool {
        match field_path {
            "security.jwt.secret" => !config.security.jwt.secret.is_empty(),
            "database.mongodb_uri" => !config.database.mongodb_uri.is_empty(),
            "server.host" => !config.server.host.is_empty(),
            _ => false,
        }
    }

    /// Apply custom validator
    fn apply_custom_validator(&self, config: &AppConfiguration, validator: &CustomValidator) -> Result<()> {
        match &validator.validation_type {
            ValidationType::Range { min, max } => {
                let value = self.get_numeric_field_value(config, &validator.field_path)?;
                if value < *min || value > *max {
                    return Err(AppError::ConfigError(validator.error_message.clone()));
                }
            }
            ValidationType::Regex { pattern } => {
                let value = self.get_string_field_value(config, &validator.field_path)?;
                let regex = regex::Regex::new(pattern)
                    .map_err(|e| AppError::ConfigError(format!("Invalid regex pattern: {}", e)))?;
                if !regex.is_match(&value) {
                    return Err(AppError::ConfigError(validator.error_message.clone()));
                }
            }
            ValidationType::Enum { values } => {
                let value = self.get_string_field_value(config, &validator.field_path)?;
                if !values.contains(&value) {
                    return Err(AppError::ConfigError(validator.error_message.clone()));
                }
            }
            ValidationType::Custom { script: _ } => {
                // Custom script validation would be implemented here
                // For now, we'll just log a warning
                warn!("Custom script validation not implemented for validator: {}", validator.name);
            }
        }
        Ok(())
    }

    /// Get numeric field value for validation
    fn get_numeric_field_value(&self, config: &AppConfiguration, field_path: &str) -> Result<f64> {
        match field_path {
            "server.port" => Ok(config.server.port as f64),
            "server.max_connections" => Ok(config.server.max_connections as f64),
            "database.connection_pool_size" => Ok(config.database.connection_pool_size as f64),
            _ => Err(AppError::ConfigError(format!("Unknown numeric field: {}", field_path))),
        }
    }

    /// Get string field value for validation
    fn get_string_field_value(&self, config: &AppConfiguration, field_path: &str) -> Result<String> {
        match field_path {
            "server.host" => Ok(config.server.host.clone()),
            "database.mongodb_uri" => Ok(config.database.mongodb_uri.clone()),
            "observability.logging.level" => Ok(config.observability.logging.level.clone()),
            _ => Err(AppError::ConfigError(format!("Unknown string field: {}", field_path))),
        }
    }

    /// Add encrypted configuration value
    pub async fn add_encrypted_value(&mut self, environment: &str, field_path: &str, value: &str) -> Result<()> {
        let encrypted_value = self.encryption_service.encrypt(value).await?;
        
        if let Some(env_config) = self.environment_overrides.get_mut(environment) {
            env_config.encrypted_values.insert(field_path.to_string(), encrypted_value);
        } else {
            return Err(AppError::ConfigError(format!("Environment '{}' not found", environment)));
        }
        
        Ok(())
    }

    /// List available environments
    pub fn list_environments(&self) -> Vec<String> {
        self.environment_overrides.keys().cloned().collect()
    }

    /// Get environment configuration
    pub fn get_environment_config(&self, environment: &str) -> Option<&EnvironmentConfig> {
        self.environment_overrides.get(environment)
    }
}

impl ConfigInheritance {
    fn new() -> Self {
        Self {
            inheritance_graph: HashMap::new(),
        }
    }
}

impl EncryptionService {
    fn new() -> Self {
        Self {
            encryption_keys: HashMap::new(),
            default_method: EncryptionMethod::Aes256Gcm,
        }
    }

    /// Encrypt a value
    async fn encrypt(&self, value: &str) -> Result<EncryptedValue> {
        // This is a simplified implementation
        // In production, you'd use proper encryption libraries
        let encrypted_data = base64ct::Base64::encode_string(value.as_bytes());
        
        Ok(EncryptedValue {
            encrypted_data,
            encryption_method: self.default_method.clone(),
            key_id: "default".to_string(),
            created_at: chrono::Utc::now(),
        })
    }

    /// Decrypt a value
    async fn decrypt(&self, encrypted_value: &EncryptedValue) -> Result<String> {
        // This is a simplified implementation
        // In production, you'd use proper decryption
        let decrypted_bytes = base64ct::Base64::decode_vec(&encrypted_value.encrypted_data)
            .map_err(|e| AppError::ConfigError(format!("Failed to decrypt value: {}", e)))?;
        
        String::from_utf8(decrypted_bytes)
            .map_err(|e| AppError::ConfigError(format!("Invalid UTF-8 in decrypted value: {}", e)))
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            inherits_from: None,
            config_overrides: serde_json::Value::Object(serde_json::Map::new()),
            encrypted_values: HashMap::new(),
            feature_flags: HashMap::new(),
            resource_limits: None,
            deployment_config: None,
            validation_rules: None,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: Some(2048),
            max_cpu_cores: Some(2.0),
            max_disk_gb: Some(10),
            max_network_mbps: Some(100),
            max_concurrent_connections: Some(1000),
        }
    }
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            deployment_strategy: DeploymentStrategy::RollingUpdate,
            health_check_config: HealthCheckConfig::default(),
            scaling_config: ScalingConfig::default(),
            monitoring_config: MonitoringConfig::default(),
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "/health".to_string(),
            interval_seconds: 30,
            timeout_seconds: 5,
            failure_threshold: 3,
            success_threshold: 1,
        }
    }
}

impl Default for ScalingConfig {
    fn default() -> Self {
        Self {
            auto_scaling_enabled: false,
            min_instances: 1,
            max_instances: 10,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            scale_up_cooldown_seconds: 300,
            scale_down_cooldown_seconds: 600,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            logging_level: "info".to_string(),
            tracing_enabled: false,
            alert_rules: Vec::new(),
            dashboard_config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::io::Write;

    fn create_test_base_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.server.host = "localhost".to_string();
        config.server.port = 8000;
        config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
        config.security.jwt.secret = "base_secret".to_string();
        config
    }

    async fn create_test_environment_config(temp_dir: &TempDir, env_name: &str, config_content: &str) -> PathBuf {
        let config_path = temp_dir.path().join(format!("env-{}.yaml", env_name));
        let mut file = std::fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();
        config_path
    }

    #[tokio::test]
    async fn test_environment_config_resolver_creation() {
        let base_config = create_test_base_config();
        let resolver = EnvironmentConfigResolver::new(base_config.clone());
        
        assert_eq!(resolver.base_config.server.host, "localhost");
        assert_eq!(resolver.base_config.server.port, 8000);
        assert!(resolver.environment_overrides.is_empty());
    }

    #[tokio::test]
    async fn test_load_environment_configs() {
        let temp_dir = TempDir::new().unwrap();
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        // Create test environment config files
        let dev_config = r#"
name: "development"
config_overrides:
  server:
    port: 8001
  observability:
    logging:
      level: "debug"
feature_flags:
  enable_metrics_collection: true
  enable_experimental_features: true
"#;

        let prod_config = r#"
name: "production"
inherits_from: "development"
config_overrides:
  server:
    port: 8080
  observability:
    logging:
      level: "warn"
feature_flags:
  enable_experimental_features: false
resource_limits:
  max_memory_mb: 4096
  max_cpu_cores: 4.0
"#;

        create_test_environment_config(&temp_dir, "development", dev_config).await;
        create_test_environment_config(&temp_dir, "production", prod_config).await;

        // Load environment configurations
        resolver.load_environment_configs(temp_dir.path()).await.unwrap();

        assert_eq!(resolver.environment_overrides.len(), 2);
        assert!(resolver.environment_overrides.contains_key("development"));
        assert!(resolver.environment_overrides.contains_key("production"));

        let prod_env = resolver.environment_overrides.get("production").unwrap();
        assert_eq!(prod_env.inherits_from, Some("development".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_environment_config() {
        let temp_dir = TempDir::new().unwrap();
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        let dev_config = r#"
name: "development"
config_overrides:
  server:
    port: 8001
  observability:
    logging:
      level: "debug"
feature_flags:
  enable_metrics_collection: true
"#;

        create_test_environment_config(&temp_dir, "development", dev_config).await;
        resolver.load_environment_configs(temp_dir.path()).await.unwrap();

        let resolved_config = resolver.resolve_environment_config("development").await.unwrap();
        
        assert_eq!(resolved_config.server.port, 8001);
        assert_eq!(resolved_config.observability.logging.level, "debug");
        assert!(resolved_config.features.enable_metrics_collection);
    }

    #[tokio::test]
    async fn test_config_inheritance() {
        let temp_dir = TempDir::new().unwrap();
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        let base_env_config = r#"
name: "base"
config_overrides:
  server:
    port: 8000
  observability:
    logging:
      level: "info"
feature_flags:
  enable_metrics_collection: true
  enable_audit_logging: true
"#;

        let dev_config = r#"
name: "development"
inherits_from: "base"
config_overrides:
  observability:
    logging:
      level: "debug"
feature_flags:
  enable_experimental_features: true
"#;

        let prod_config = r#"
name: "production"
inherits_from: "base"
config_overrides:
  server:
    port: 8080
  observability:
    logging:
      level: "warn"
feature_flags:
  enable_experimental_features: false
"#;

        create_test_environment_config(&temp_dir, "base", base_env_config).await;
        create_test_environment_config(&temp_dir, "development", dev_config).await;
        create_test_environment_config(&temp_dir, "production", prod_config).await;

        resolver.load_environment_configs(temp_dir.path()).await.unwrap();

        // Test development environment (inherits from base)
        let dev_resolved = resolver.resolve_environment_config("development").await.unwrap();
        assert_eq!(dev_resolved.server.port, 8000); // From base
        assert_eq!(dev_resolved.observability.logging.level, "debug"); // Overridden
        assert!(dev_resolved.features.enable_metrics_collection); // From base
        assert!(dev_resolved.features.enable_experimental_features); // From dev

        // Test production environment (inherits from base)
        let prod_resolved = resolver.resolve_environment_config("production").await.unwrap();
        assert_eq!(prod_resolved.server.port, 8080); // Overridden
        assert_eq!(prod_resolved.observability.logging.level, "warn"); // Overridden
        assert!(prod_resolved.features.enable_metrics_collection); // From base
        assert!(!prod_resolved.features.enable_experimental_features); // Overridden
    }

    #[tokio::test]
    async fn test_circular_dependency_detection() {
        let temp_dir = TempDir::new().unwrap();
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        let env_a_config = r#"
name: "env_a"
inherits_from: "env_b"
config_overrides:
  server:
    port: 8001
"#;

        let env_b_config = r#"
name: "env_b"
inherits_from: "env_a"
config_overrides:
  server:
    port: 8002
"#;

        create_test_environment_config(&temp_dir, "env_a", env_a_config).await;
        create_test_environment_config(&temp_dir, "env_b", env_b_config).await;

        let result = resolver.load_environment_configs(temp_dir.path()).await;
        assert!(result.is_err());
        
        if let Err(AppError::ConfigError(msg)) = result {
            assert!(msg.contains("Circular dependency"));
        } else {
            panic!("Expected circular dependency error");
        }
    }

    #[tokio::test]
    async fn test_encryption_service() {
        let encryption_service = EncryptionService::new();
        
        let original_value = "secret_password_123";
        let encrypted = encryption_service.encrypt(original_value).await.unwrap();
        
        assert_ne!(encrypted.encrypted_data, original_value);
        assert!(matches!(encrypted.encryption_method, EncryptionMethod::Aes256Gcm));
        
        let decrypted = encryption_service.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted, original_value);
    }

    #[tokio::test]
    async fn test_encrypted_values_in_config() {
        let temp_dir = TempDir::new().unwrap();
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        // Add encrypted value
        resolver.add_encrypted_value("test", "security.jwt.secret", "encrypted_secret_123").await.unwrap();
        
        // Create environment config with encrypted values
        let env_config = EnvironmentConfig {
            name: "test".to_string(),
            inherits_from: None,
            config_overrides: serde_json::Value::Object(serde_json::Map::new()),
            encrypted_values: resolver.environment_overrides.get("test").unwrap().encrypted_values.clone(),
            feature_flags: HashMap::new(),
            resource_limits: None,
            deployment_config: None,
            validation_rules: None,
        };

        resolver.environment_overrides.insert("test".to_string(), env_config);

        let resolved_config = resolver.resolve_environment_config("test").await.unwrap();
        assert_eq!(resolved_config.security.jwt.secret, "encrypted_secret_123");
    }

    #[tokio::test]
    async fn test_resource_limits_application() {
        let temp_dir = TempDir::new().unwrap();
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        let config_with_limits = r#"
name: "limited"
config_overrides: {}
resource_limits:
  max_memory_mb: 1024
  max_cpu_cores: 1.0
  max_concurrent_connections: 500
"#;

        create_test_environment_config(&temp_dir, "limited", config_with_limits).await;
        resolver.load_environment_configs(temp_dir.path()).await.unwrap();

        let resolved_config = resolver.resolve_environment_config("limited").await.unwrap();
        
        assert_eq!(resolved_config.server.max_connections, 500);
        
        // Check that resource limits were applied to CI executors
        for executor_config in resolved_config.ci.executors.values() {
            assert_eq!(executor_config.resource_limits.memory_mb, Some(1024));
            assert_eq!(executor_config.resource_limits.cpu_cores, Some(1.0));
        }
    }

    #[tokio::test]
    async fn test_custom_validation() {
        let base_config = create_test_base_config();
        let resolver = EnvironmentConfigResolver::new(base_config);

        let mut test_config = create_test_base_config();
        test_config.server.port = 9999;

        // Test range validation
        let range_validator = CustomValidator {
            name: "port_range".to_string(),
            field_path: "server.port".to_string(),
            validation_type: ValidationType::Range { min: 1000.0, max: 8999.0 },
            parameters: HashMap::new(),
            error_message: "Port must be between 1000 and 8999".to_string(),
        };

        let result = resolver.apply_custom_validator(&test_config, &range_validator);
        assert!(result.is_err());

        // Test regex validation
        let regex_validator = CustomValidator {
            name: "host_format".to_string(),
            field_path: "server.host".to_string(),
            validation_type: ValidationType::Regex { pattern: r"^[a-zA-Z0-9.-]+$".to_string() },
            parameters: HashMap::new(),
            error_message: "Invalid host format".to_string(),
        };

        let result = resolver.apply_custom_validator(&test_config, &regex_validator);
        assert!(result.is_ok());

        // Test enum validation
        let enum_validator = CustomValidator {
            name: "log_level".to_string(),
            field_path: "observability.logging.level".to_string(),
            validation_type: ValidationType::Enum { 
                values: vec!["trace".to_string(), "debug".to_string(), "info".to_string(), "warn".to_string(), "error".to_string()]
            },
            parameters: HashMap::new(),
            error_message: "Invalid log level".to_string(),
        };

        let result = resolver.apply_custom_validator(&test_config, &enum_validator);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_inheritance_chain() {
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        // Create inheritance chain: base -> staging -> production
        let base_env = EnvironmentConfig {
            name: "base".to_string(),
            inherits_from: None,
            ..Default::default()
        };

        let staging_env = EnvironmentConfig {
            name: "staging".to_string(),
            inherits_from: Some("base".to_string()),
            ..Default::default()
        };

        let production_env = EnvironmentConfig {
            name: "production".to_string(),
            inherits_from: Some("staging".to_string()),
            ..Default::default()
        };

        resolver.environment_overrides.insert("base".to_string(), base_env);
        resolver.environment_overrides.insert("staging".to_string(), staging_env);
        resolver.environment_overrides.insert("production".to_string(), production_env);

        resolver.build_inheritance_graph().unwrap();

        let chain = resolver.get_inheritance_chain("production").unwrap();
        assert_eq!(chain, vec!["production", "staging", "base"]);
    }

    #[tokio::test]
    async fn test_list_environments() {
        let base_config = create_test_base_config();
        let mut resolver = EnvironmentConfigResolver::new(base_config);

        resolver.environment_overrides.insert("dev".to_string(), EnvironmentConfig::default());
        resolver.environment_overrides.insert("staging".to_string(), EnvironmentConfig::default());
        resolver.environment_overrides.insert("prod".to_string(), EnvironmentConfig::default());

        let environments = resolver.list_environments();
        assert_eq!(environments.len(), 3);
        assert!(environments.contains(&"dev".to_string()));
        assert!(environments.contains(&"staging".to_string()));
        assert!(environments.contains(&"prod".to_string()));
    }

    #[test]
    fn test_default_implementations() {
        let env_config = EnvironmentConfig::default();
        assert_eq!(env_config.name, "default");
        assert!(env_config.inherits_from.is_none());

        let resource_limits = ResourceLimits::default();
        assert_eq!(resource_limits.max_memory_mb, Some(2048));
        assert_eq!(resource_limits.max_cpu_cores, Some(2.0));

        let deployment_config = DeploymentConfig::default();
        assert!(matches!(deployment_config.deployment_strategy, DeploymentStrategy::RollingUpdate));

        let health_check = HealthCheckConfig::default();
        assert!(health_check.enabled);
        assert_eq!(health_check.endpoint, "/health");

        let scaling_config = ScalingConfig::default();
        assert!(!scaling_config.auto_scaling_enabled);
        assert_eq!(scaling_config.min_instances, 1);

        let monitoring_config = MonitoringConfig::default();
        assert!(monitoring_config.metrics_enabled);
        assert_eq!(monitoring_config.logging_level, "info");
    }
}