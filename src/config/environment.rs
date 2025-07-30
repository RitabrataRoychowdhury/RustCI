use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Environment types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum Environment {
    #[default]
    Development,
    Testing,
    Staging,
    Production,
}

impl Environment {
    pub fn from_string(env: &str) -> Self {
        match env.to_lowercase().as_str() {
            "dev" | "development" => Environment::Development,
            "test" | "testing" => Environment::Testing,
            "stage" | "staging" => Environment::Staging,
            "prod" | "production" => Environment::Production,
            _ => {
                warn!("Unknown environment '{}', defaulting to Development", env);
                Environment::Development
            }
        }
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }

    pub fn is_development(&self) -> bool {
        matches!(self, Environment::Development)
    }

    pub fn is_testing(&self) -> bool {
        matches!(self, Environment::Testing)
    }
}



impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Development => write!(f, "development"),
            Environment::Testing => write!(f, "testing"),
            Environment::Staging => write!(f, "staging"),
            Environment::Production => write!(f, "production"),
        }
    }
}

/// Environment-specific configuration loader
pub struct EnvironmentLoader {
    environment: Environment,
    required_vars: Vec<String>,
    optional_vars: HashMap<String, String>, // var_name -> default_value
}

impl Default for EnvironmentLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvironmentLoader {
    pub fn new() -> Self {
        let env_str = std::env::var("ENVIRONMENT")
            .or_else(|_| std::env::var("ENV"))
            .unwrap_or_else(|_| "development".to_string());

        Self {
            environment: Environment::from_string(&env_str),
            required_vars: Vec::new(),
            optional_vars: HashMap::new(),
        }
    }

    pub fn with_environment(mut self, env: Environment) -> Self {
        self.environment = env;
        self
    }

    pub fn require_var(mut self, var_name: &str) -> Self {
        self.required_vars.push(var_name.to_string());
        self
    }

    pub fn optional_var(mut self, var_name: &str, default_value: &str) -> Self {
        self.optional_vars
            .insert(var_name.to_string(), default_value.to_string());
        self
    }

    /// Load and validate all environment variables
    pub fn load(&self) -> Result<EnvironmentConfig> {
        let mut config = EnvironmentConfig {
            environment: self.environment.clone(),
            variables: HashMap::new(),
        };

        // Load required variables
        for var_name in &self.required_vars {
            match std::env::var(var_name) {
                Ok(value) => {
                    config.variables.insert(var_name.clone(), value);
                }
                Err(_) => {
                    return Err(AppError::ConfigError(format!(
                        "Required environment variable '{}' is not set",
                        var_name
                    )));
                }
            }
        }

        // Load optional variables with defaults
        for (var_name, default_value) in &self.optional_vars {
            let value = std::env::var(var_name).unwrap_or_else(|_| default_value.clone());
            config.variables.insert(var_name.clone(), value);
        }

        // Load environment-specific variables
        self.load_environment_specific(&mut config)?;

        debug!(
            environment = %self.environment,
            required_vars = self.required_vars.len(),
            optional_vars = self.optional_vars.len(),
            "🌍 Environment configuration loaded"
        );

        Ok(config)
    }

    /// Load environment-specific variables based on naming conventions
    fn load_environment_specific(&self, config: &mut EnvironmentConfig) -> Result<()> {
        let env_prefix = match self.environment {
            Environment::Development => "DEV_",
            Environment::Testing => "TEST_",
            Environment::Staging => "STAGE_",
            Environment::Production => "PROD_",
        };

        // Load all environment variables with the specific prefix
        for (key, value) in std::env::vars() {
            if key.starts_with(env_prefix) {
                let clean_key = key.strip_prefix(env_prefix).unwrap().to_string();
                config.variables.insert(clean_key, value);
            }
        }

        Ok(())
    }

    /// Validate environment for production readiness
    pub fn validate_production_readiness(&self, config: &EnvironmentConfig) -> Result<()> {
        if !self.environment.is_production() {
            return Ok(());
        }

        let production_requirements = vec![
            "JWT_SECRET",
            "MONGODB_URI",
            "ENCRYPTION_KEY",
            "GITHUB_OAUTH_CLIENT_SECRET",
        ];

        for req_var in production_requirements {
            if let Some(value) = config.variables.get(req_var) {
                if value.is_empty() || value == "your-secret-key" || value.contains("example") {
                    return Err(AppError::ConfigError(format!(
                        "Production environment requires secure value for '{}'",
                        req_var
                    )));
                }
            } else {
                return Err(AppError::ConfigError(format!(
                    "Production environment requires '{}'",
                    req_var
                )));
            }
        }

        debug!("✅ Production readiness validation passed");
        Ok(())
    }
}

/// Environment configuration container
#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    pub environment: Environment,
    pub variables: HashMap<String, String>,
}

impl EnvironmentConfig {
    pub fn get(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    pub fn get_or_default(&self, key: &str, default: &str) -> String {
        self.variables
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    pub fn get_bool(&self, key: &str) -> Result<bool> {
        match self.variables.get(key) {
            Some(value) => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" => Ok(false),
                _ => Err(AppError::ConfigError(format!(
                    "Invalid boolean value for '{}': {}",
                    key, value
                ))),
            },
            None => Err(AppError::ConfigError(format!(
                "Environment variable '{}' not found",
                key
            ))),
        }
    }

    pub fn get_int<T>(&self, key: &str) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        match self.variables.get(key) {
            Some(value) => value.parse::<T>().map_err(|e| {
                AppError::ConfigError(format!("Invalid integer value for '{}': {}", key, e))
            }),
            None => Err(AppError::ConfigError(format!(
                "Environment variable '{}' not found",
                key
            ))),
        }
    }

    pub fn get_float<T>(&self, key: &str) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        match self.variables.get(key) {
            Some(value) => value.parse::<T>().map_err(|e| {
                AppError::ConfigError(format!("Invalid float value for '{}': {}", key, e))
            }),
            None => Err(AppError::ConfigError(format!(
                "Environment variable '{}' not found",
                key
            ))),
        }
    }

    pub fn get_list(&self, key: &str, separator: &str) -> Vec<String> {
        match self.variables.get(key) {
            Some(value) => value
                .split(separator)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            None => Vec::new(),
        }
    }

    /// Get environment-specific database URL
    pub fn get_database_url(&self) -> Result<String> {
        // Try environment-specific first, then fall back to generic
        let db_url = self
            .get("DATABASE_URL")
            .or_else(|| self.get("MONGODB_URI"))
            .ok_or_else(|| AppError::ConfigError("Database URL not configured".to_string()))?;

        // Validate URL format
        if !db_url.starts_with("mongodb://") && !db_url.starts_with("mongodb+srv://") {
            return Err(AppError::ConfigError(
                "Database URL must be a valid MongoDB connection string".to_string(),
            ));
        }

        Ok(db_url.clone())
    }

    /// Get Redis URL if configured
    pub fn get_redis_url(&self) -> Option<String> {
        self.get("REDIS_URL").cloned()
    }

    /// Check if feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        let key = format!("FEATURE_{}", feature.to_uppercase());
        self.get_bool(&key).unwrap_or(false)
    }

    /// Get service discovery configuration
    pub fn get_service_config(&self, service: &str) -> HashMap<String, String> {
        let prefix = format!("{}_", service.to_uppercase());
        let mut config = HashMap::new();

        for (key, value) in &self.variables {
            if key.starts_with(&prefix) {
                let clean_key = key.strip_prefix(&prefix).unwrap().to_lowercase();
                config.insert(clean_key, value.clone());
            }
        }

        config
    }
}

/// Environment-aware configuration builder
pub struct ConfigBuilder {
    env_config: EnvironmentConfig,
}

impl ConfigBuilder {
    pub fn from_environment() -> Result<Self> {
        let loader = EnvironmentLoader::new()
            .require_var("JWT_SECRET")
            .require_var("MONGODB_URI")
            .optional_var("PORT", "8000")
            .optional_var("HOST", "0.0.0.0")
            .optional_var("LOG_LEVEL", "info")
            .optional_var("WORKERS", "0") // 0 means auto-detect
            .optional_var("MAX_CONNECTIONS", "1000")
            .optional_var("REQUEST_TIMEOUT", "30")
            .optional_var("MONGODB_DATABASE", "rustci");

        let env_config = loader.load()?;
        loader.validate_production_readiness(&env_config)?;

        Ok(Self { env_config })
    }

    pub fn build_server_config(&self) -> Result<crate::config::ServerConfig> {
        Ok(crate::config::ServerConfig {
            host: self.env_config.get_or_default("HOST", "0.0.0.0"),
            port: self.env_config.get_int("PORT")?,
            workers: {
                let workers: u32 = self.env_config.get_int("WORKERS")?;
                if workers == 0 {
                    None
                } else {
                    Some(workers as usize)
                }
            },
            max_connections: self.env_config.get_int("MAX_CONNECTIONS")?,
            request_timeout_seconds: self.env_config.get_int("REQUEST_TIMEOUT")?,
            graceful_shutdown_timeout_seconds: self
                .env_config
                .get_int("GRACEFUL_SHUTDOWN_TIMEOUT")
                .unwrap_or(30),
            tls: self.build_tls_config()?,
        })
    }

    pub fn build_database_config(&self) -> Result<crate::config::DatabaseConfig> {
        Ok(crate::config::DatabaseConfig {
            mongodb_uri: self.env_config.get_database_url()?,
            database_name: self.env_config.get_or_default("MONGODB_DATABASE", "rustci"),
            connection_pool_size: self.env_config.get_int("DB_POOL_SIZE").unwrap_or(10),
            connection_timeout_seconds: self
                .env_config
                .get_int("DB_CONNECTION_TIMEOUT")
                .unwrap_or(10),
            query_timeout_seconds: self.env_config.get_int("DB_QUERY_TIMEOUT").unwrap_or(30),
            retry_attempts: self.env_config.get_int("DB_RETRY_ATTEMPTS").unwrap_or(3),
            enable_ssl: self.env_config.get_bool("DB_ENABLE_SSL").unwrap_or(false),
        })
    }

    fn build_tls_config(&self) -> Result<Option<crate::config::TlsConfig>> {
        if let (Some(cert_path), Some(key_path)) = (
            self.env_config.get("TLS_CERT_PATH"),
            self.env_config.get("TLS_KEY_PATH"),
        ) {
            Ok(Some(crate::config::TlsConfig {
                cert_path: cert_path.clone(),
                key_path: key_path.clone(),
                require_client_cert: self
                    .env_config
                    .get_bool("TLS_REQUIRE_CLIENT_CERT")
                    .unwrap_or(false),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_environment(&self) -> &Environment {
        &self.env_config.environment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_from_string() {
        assert_eq!(Environment::from_string("dev"), Environment::Development);
        assert_eq!(
            Environment::from_string("PRODUCTION"),
            Environment::Production
        );
        assert_eq!(
            Environment::from_string("unknown"),
            Environment::Development
        );
    }

    #[test]
    fn test_environment_config() {
        let mut config = EnvironmentConfig {
            environment: Environment::Development,
            variables: HashMap::new(),
        };

        config
            .variables
            .insert("TEST_BOOL".to_string(), "true".to_string());
        config
            .variables
            .insert("TEST_INT".to_string(), "42".to_string());
        config
            .variables
            .insert("TEST_LIST".to_string(), "a,b,c".to_string());

        assert_eq!(config.get_bool("TEST_BOOL").unwrap(), true);
        assert_eq!(config.get_int::<i32>("TEST_INT").unwrap(), 42);
        assert_eq!(config.get_list("TEST_LIST", ","), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_environment_loader() {
        std::env::set_var("TEST_REQUIRED", "value");
        std::env::set_var("DEV_SPECIFIC", "dev_value");

        let loader = EnvironmentLoader::new()
            .with_environment(Environment::Development)
            .require_var("TEST_REQUIRED")
            .optional_var("TEST_OPTIONAL", "default");

        let config = loader.load().unwrap();
        assert_eq!(config.get("TEST_REQUIRED"), Some(&"value".to_string()));
        assert_eq!(config.get("TEST_OPTIONAL"), Some(&"default".to_string()));
        assert_eq!(config.get("SPECIFIC"), Some(&"dev_value".to_string()));

        std::env::remove_var("TEST_REQUIRED");
        std::env::remove_var("DEV_SPECIFIC");
    }
}
