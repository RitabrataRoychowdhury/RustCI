use crate::error::{AppError, Result};
use base64ct::Encoding;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfiguration {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub observability: ObservabilityConfig,
    pub ci: CIConfig,
    pub external_services: ExternalServicesConfig,
    pub features: FeatureFlags,
    pub valkyrie: Option<ValkyrieIntegrationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
    pub max_connections: u32,
    pub request_timeout_seconds: u64,
    pub graceful_shutdown_timeout_seconds: u64,
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub require_client_cert: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub mongodb_uri: String,
    pub database_name: String,
    pub connection_pool_size: u32,
    pub connection_timeout_seconds: u64,
    pub query_timeout_seconds: u64,
    pub retry_attempts: u32,
    pub enable_ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jwt: JwtConfig,
    pub oauth: OAuthConfig,
    pub encryption: EncryptionConfig,
    pub rate_limiting: RateLimitConfig,
    pub cors: CorsConfig,
    pub rbac: RbacConfig,
    pub audit: AuditConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacConfig {
    pub enabled: bool,
    pub default_role: String,
    pub role_hierarchy: HashMap<String, Vec<String>>,
    pub permission_cache_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    pub log_all_requests: bool,
    pub log_failed_auth: bool,
    pub retention_days: u32,
    pub sensitive_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub timeout_minutes: u32,
    pub max_concurrent_sessions: u32,
    pub secure_cookies: bool,
    pub same_site: String, // "strict", "lax", "none"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expires_in_seconds: i64,
    pub refresh_expires_in_seconds: i64,
    pub issuer: String,
    pub audience: String,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub github: GitHubOAuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub key: String, // Base64 encoded 32-byte key
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub enable_per_user_limits: bool,
    pub whitelist_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
    pub tracing: TracingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String, // "json" or "pretty"
    pub output: String, // "stdout", "stderr", or file path
    pub enable_file_rotation: bool,
    pub max_file_size_mb: u64,
    pub max_files: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub collection_interval_seconds: u64,
    pub export_timeout_seconds: u64,
    pub custom_metrics: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub service_name: String,
    pub service_version: String,
    pub sample_rate: f64,
    pub export_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CIConfig {
    pub max_concurrent_pipelines: u32,
    pub pipeline_timeout_seconds: u64,
    pub workspace_cleanup_interval_seconds: u64,
    pub default_executor: String,
    pub executors: HashMap<String, ExecutorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub enabled: bool,
    pub max_concurrent_jobs: u32,
    pub resource_limits: ResourceLimits,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub cpu_cores: Option<f64>,
    pub memory_mb: Option<u64>,
    pub disk_mb: Option<u64>,
    pub network_bandwidth_mbps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServicesConfig {
    pub github: GitHubServiceConfig,
    pub docker_registry: DockerRegistryConfig,
    pub notification: NotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubServiceConfig {
    pub api_url: String,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub rate_limit_per_hour: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRegistryConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_seconds: u64,
    pub insecure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub channels: Vec<NotificationChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub name: String,
    pub channel_type: String, // "email", "slack", "webhook"
    pub config: HashMap<String, String>,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_audit_logging: bool,
    pub enable_metrics_collection: bool,
    pub enable_distributed_tracing: bool,
    pub enable_hot_reload: bool,
    pub enable_experimental_features: bool,
    pub enable_valkyrie_protocol: bool,
    pub custom_flags: HashMap<String, bool>,
}

/// Valkyrie Protocol integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieIntegrationConfig {
    /// Enable Valkyrie Protocol
    pub enabled: bool,
    /// Configuration source (file path or "environment")
    pub config_source: String,
    /// Fallback to legacy communication if Valkyrie fails
    pub fallback_to_legacy: bool,
    /// Enable Valkyrie for specific node types
    pub enabled_for_node_types: Vec<String>,
    /// Valkyrie-specific feature flags
    pub features: ValkyrieFeatureFlags,
}

/// Valkyrie-specific feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieFeatureFlags {
    /// Enable advanced routing
    pub advanced_routing: bool,
    /// Enable stream multiplexing
    pub stream_multiplexing: bool,
    /// Enable post-quantum cryptography
    pub post_quantum_crypto: bool,
    /// Enable machine learning optimizations
    pub ml_optimizations: bool,
    /// Enable zero-copy optimizations
    pub zero_copy: bool,
    /// Enable SIMD optimizations
    pub simd: bool,
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8000,
                workers: None,
                max_connections: 1000,
                request_timeout_seconds: 30,
                graceful_shutdown_timeout_seconds: 30,
                tls: None,
            },
            database: DatabaseConfig {
                mongodb_uri: "mongodb://localhost:27017".to_string(),
                database_name: "rustci".to_string(),
                connection_pool_size: 10,
                connection_timeout_seconds: 10,
                query_timeout_seconds: 30,
                retry_attempts: 3,
                enable_ssl: false,
            },
            security: SecurityConfig {
                jwt: JwtConfig {
                    secret: "your-secret-key".to_string(),
                    expires_in_seconds: 3600,
                    refresh_expires_in_seconds: 86400,
                    issuer: "rustci".to_string(),
                    audience: "rustci-api".to_string(),
                    algorithm: "HS256".to_string(),
                },
                oauth: OAuthConfig {
                    github: GitHubOAuthConfig {
                        client_id: "".to_string(),
                        client_secret: "".to_string(),
                        redirect_url: "http://localhost:8000/auth/github/callback".to_string(),
                        scopes: vec!["user:email".to_string(), "repo".to_string()],
                    },
                },
                encryption: EncryptionConfig {
                    key: "".to_string(),
                    algorithm: "AES-256-GCM".to_string(),
                },
                rate_limiting: RateLimitConfig {
                    enabled: true,
                    requests_per_minute: 60,
                    burst_size: 10,
                    enable_per_user_limits: true,
                    whitelist_ips: vec!["127.0.0.1".to_string()],
                },
                cors: CorsConfig {
                    allowed_origins: vec!["*".to_string()],
                    allowed_methods: vec![
                        "GET".to_string(),
                        "POST".to_string(),
                        "PUT".to_string(),
                        "DELETE".to_string(),
                        "OPTIONS".to_string(),
                    ],
                    allowed_headers: vec![
                        "Content-Type".to_string(),
                        "Authorization".to_string(),
                        "X-Requested-With".to_string(),
                    ],
                    allow_credentials: true,
                    max_age_seconds: 3600,
                },
                rbac: RbacConfig {
                    enabled: true,
                    default_role: "Viewer".to_string(),
                    role_hierarchy: {
                        let mut hierarchy = HashMap::new();
                        hierarchy.insert(
                            "Admin".to_string(),
                            vec!["Developer".to_string(), "Viewer".to_string()],
                        );
                        hierarchy.insert("Developer".to_string(), vec!["Viewer".to_string()]);
                        hierarchy
                    },
                    permission_cache_ttl_seconds: 300,
                },
                audit: AuditConfig {
                    enabled: true,
                    log_all_requests: false,
                    log_failed_auth: true,
                    retention_days: 90,
                    sensitive_fields: vec![
                        "password".to_string(),
                        "token".to_string(),
                        "secret".to_string(),
                        "key".to_string(),
                    ],
                },
                session: SessionConfig {
                    timeout_minutes: 60,
                    max_concurrent_sessions: 5,
                    secure_cookies: false, // Set to true in production
                    same_site: "lax".to_string(),
                },
            },
            observability: ObservabilityConfig {
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: "pretty".to_string(),
                    output: "stdout".to_string(),
                    enable_file_rotation: false,
                    max_file_size_mb: 100,
                    max_files: 10,
                },
                metrics: MetricsConfig {
                    enabled: true,
                    endpoint: "/metrics".to_string(),
                    collection_interval_seconds: 60,
                    export_timeout_seconds: 10,
                    custom_metrics: HashMap::new(),
                },
                tracing: TracingConfig {
                    enabled: false,
                    endpoint: None,
                    service_name: "rustci".to_string(),
                    service_version: "0.1.0".to_string(),
                    sample_rate: 0.1,
                    export_timeout_seconds: 10,
                },
            },
            ci: CIConfig {
                max_concurrent_pipelines: 10,
                pipeline_timeout_seconds: 3600,
                workspace_cleanup_interval_seconds: 300,
                default_executor: "docker".to_string(),
                executors: {
                    let mut executors = HashMap::new();
                    executors.insert(
                        "docker".to_string(),
                        ExecutorConfig {
                            enabled: true,
                            max_concurrent_jobs: 5,
                            resource_limits: ResourceLimits {
                                cpu_cores: Some(2.0),
                                memory_mb: Some(2048),
                                disk_mb: Some(10240),
                                network_bandwidth_mbps: None,
                            },
                            timeout_seconds: 1800,
                            retry_attempts: 2,
                            config: HashMap::new(),
                        },
                    );
                    executors
                },
            },
            external_services: ExternalServicesConfig {
                github: GitHubServiceConfig {
                    api_url: "https://api.github.com".to_string(),
                    timeout_seconds: 30,
                    retry_attempts: 3,
                    rate_limit_per_hour: 5000,
                },
                docker_registry: DockerRegistryConfig {
                    url: "https://registry-1.docker.io".to_string(),
                    username: None,
                    password: None,
                    timeout_seconds: 60,
                    insecure: false,
                },
                notification: NotificationConfig {
                    enabled: false,
                    channels: vec![],
                },
            },
            features: FeatureFlags {
                enable_audit_logging: true,
                enable_metrics_collection: true,
                enable_distributed_tracing: false,
                enable_hot_reload: true,
                enable_experimental_features: false,
                enable_valkyrie_protocol: false,
                custom_flags: HashMap::new(),
            },
            valkyrie: Some(ValkyrieIntegrationConfig {
                enabled: false,
                config_source: "environment".to_string(),
                fallback_to_legacy: true,
                enabled_for_node_types: vec!["worker".to_string(), "hybrid".to_string()],
                features: ValkyrieFeatureFlags {
                    advanced_routing: true,
                    stream_multiplexing: true,
                    post_quantum_crypto: false,
                    ml_optimizations: false,
                    zero_copy: true,
                    simd: true,
                },
            }),
        }
    }
}

/// Configuration manager with hot-reloading support
pub struct ConfigManager {
    config: Arc<RwLock<AppConfiguration>>,
    config_path: Option<String>,
    watchers: Vec<tokio::task::JoinHandle<()>>,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfiguration::default())),
            config_path: None,
            watchers: Vec::new(),
        }
    }

    /// Load configuration from environment variables and optional config file
    pub async fn load(&mut self) -> Result<()> {
        let mut config = AppConfiguration::default();

        // Load from environment variables first
        self.load_from_env(&mut config)?;

        // Load from config file if specified
        if let Ok(config_path) = std::env::var("CONFIG_FILE") {
            self.load_from_file(&mut config, &config_path).await?;
            self.config_path = Some(config_path);
        }

        // Validate configuration
        self.validate_config(&config)?;

        // Update the configuration
        let mut current_config = self.config.write().await;
        *current_config = config;

        info!("📋 Configuration loaded successfully");
        Ok(())
    }

    /// Load configuration from environment variables
    fn load_from_env(&self, config: &mut AppConfiguration) -> Result<()> {
        // Server configuration
        if let Ok(host) = std::env::var("SERVER_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("PORT") {
            config.server.port = port
                .parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid PORT: {}", e)))?;
        }

        // Database configuration
        if let Ok(uri) = std::env::var("MONGODB_URI") {
            config.database.mongodb_uri = uri;
        }
        if let Ok(db_name) = std::env::var("MONGODB_DATABASE") {
            config.database.database_name = db_name;
        }

        // Security configuration
        if let Ok(secret) = std::env::var("JWT_SECRET") {
            config.security.jwt.secret = secret;
        }
        if let Ok(client_id) = std::env::var("GITHUB_OAUTH_CLIENT_ID") {
            config.security.oauth.github.client_id = client_id;
        }
        if let Ok(client_secret) = std::env::var("GITHUB_OAUTH_CLIENT_SECRET") {
            config.security.oauth.github.client_secret = client_secret;
        }
        if let Ok(redirect_url) = std::env::var("GITHUB_OAUTH_REDIRECT_URL") {
            config.security.oauth.github.redirect_url = redirect_url;
        }

        // Encryption key
        if let Ok(key) = std::env::var("ENCRYPTION_KEY") {
            config.security.encryption.key = key;
        }

        // Logging level
        if let Ok(level) = std::env::var("LOG_LEVEL") {
            config.observability.logging.level = level;
        }

        // Feature flags
        if let Ok(enable_metrics) = std::env::var("ENABLE_METRICS") {
            config.features.enable_metrics_collection = enable_metrics.parse().unwrap_or(true);
        }
        if let Ok(enable_audit) = std::env::var("ENABLE_AUDIT_LOGGING") {
            config.features.enable_audit_logging = enable_audit.parse().unwrap_or(true);
        }
        if let Ok(enable_tracing) = std::env::var("ENABLE_TRACING") {
            config.features.enable_distributed_tracing = enable_tracing.parse().unwrap_or(false);
        }
        if let Ok(enable_hot_reload) = std::env::var("ENABLE_HOT_RELOAD") {
            config.features.enable_hot_reload = enable_hot_reload.parse().unwrap_or(true);
        }
        if let Ok(enable_valkyrie) = std::env::var("ENABLE_VALKYRIE_PROTOCOL") {
            config.features.enable_valkyrie_protocol = enable_valkyrie.parse().unwrap_or(false);
        }

        // Valkyrie Protocol configuration
        if let Some(valkyrie_config) = &mut config.valkyrie {
            if let Ok(valkyrie_enabled) = std::env::var("VALKYRIE_ENABLED") {
                valkyrie_config.enabled = valkyrie_enabled.parse().unwrap_or(false);
            }
            if let Ok(config_source) = std::env::var("VALKYRIE_CONFIG_SOURCE") {
                valkyrie_config.config_source = config_source;
            }
            if let Ok(fallback) = std::env::var("VALKYRIE_FALLBACK_TO_LEGACY") {
                valkyrie_config.fallback_to_legacy = fallback.parse().unwrap_or(true);
            }
            if let Ok(node_types) = std::env::var("VALKYRIE_ENABLED_NODE_TYPES") {
                valkyrie_config.enabled_for_node_types = node_types
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
            }
            
            // Valkyrie feature flags
            if let Ok(advanced_routing) = std::env::var("VALKYRIE_ADVANCED_ROUTING") {
                valkyrie_config.features.advanced_routing = advanced_routing.parse().unwrap_or(true);
            }
            if let Ok(stream_multiplexing) = std::env::var("VALKYRIE_STREAM_MULTIPLEXING") {
                valkyrie_config.features.stream_multiplexing = stream_multiplexing.parse().unwrap_or(true);
            }
            if let Ok(post_quantum) = std::env::var("VALKYRIE_POST_QUANTUM_CRYPTO") {
                valkyrie_config.features.post_quantum_crypto = post_quantum.parse().unwrap_or(false);
            }
            if let Ok(ml_optimizations) = std::env::var("VALKYRIE_ML_OPTIMIZATIONS") {
                valkyrie_config.features.ml_optimizations = ml_optimizations.parse().unwrap_or(false);
            }
            if let Ok(zero_copy) = std::env::var("VALKYRIE_ZERO_COPY") {
                valkyrie_config.features.zero_copy = zero_copy.parse().unwrap_or(true);
            }
            if let Ok(simd) = std::env::var("VALKYRIE_SIMD") {
                valkyrie_config.features.simd = simd.parse().unwrap_or(true);
            }
        }

        debug!("🔧 Configuration loaded from environment variables");
        Ok(())
    }

    /// Load configuration from file
    async fn load_from_file(&self, config: &mut AppConfiguration, path: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            AppError::ConfigError(format!("Failed to read config file {}: {}", path, e))
        })?;

        let file_config: AppConfiguration = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| AppError::ConfigError(format!("Invalid YAML config: {}", e)))?
        } else if path.ends_with(".json") {
            serde_json::from_str(&content)
                .map_err(|e| AppError::ConfigError(format!("Invalid JSON config: {}", e)))?
        } else {
            return Err(AppError::ConfigError(
                "Config file must be .yaml, .yml, or .json".to_string(),
            ));
        };

        // Merge file config with current config (file takes precedence)
        self.merge_configs(config, file_config);

        debug!(path = path, "📁 Configuration loaded from file");
        Ok(())
    }

    /// Merge two configurations (second takes precedence)
    fn merge_configs(&self, base: &mut AppConfiguration, override_config: AppConfiguration) {
        // For simplicity, we'll replace the entire config
        // In a real implementation, you might want more granular merging
        *base = override_config;
    }

    /// Validate configuration
    fn validate_config(&self, config: &AppConfiguration) -> Result<()> {
        // Validate required fields
        if config.security.jwt.secret.is_empty() {
            return Err(AppError::ConfigError("JWT secret is required".to_string()));
        }

        if config.database.mongodb_uri.is_empty() {
            return Err(AppError::ConfigError("MongoDB URI is required".to_string()));
        }

        // Validate port range
        if config.server.port == 0 {
            return Err(AppError::ConfigError("Invalid server port".to_string()));
        }

        // Validate encryption key if provided
        if !config.security.encryption.key.is_empty() {
            let key_bytes = base64ct::Base64::decode_vec(&config.security.encryption.key)
                .map_err(|e| AppError::ConfigError(format!("Invalid encryption key: {}", e)))?;
            if key_bytes.len() != 32 {
                return Err(AppError::ConfigError(
                    "Encryption key must be 32 bytes (base64 encoded)".to_string(),
                ));
            }
        }

        // Validate log level
        match config.observability.logging.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(AppError::ConfigError("Invalid log level".to_string())),
        }

        debug!("✅ Configuration validation passed");
        Ok(())
    }

    /// Get current configuration
    pub async fn get(&self) -> AppConfiguration {
        self.config.read().await.clone()
    }

    /// Update configuration (for hot-reloading)
    pub async fn update(&self, new_config: AppConfiguration) -> Result<()> {
        self.validate_config(&new_config)?;
        let mut config = self.config.write().await;
        *config = new_config;
        info!("🔄 Configuration updated");
        Ok(())
    }

    /// Start watching for configuration file changes
    pub async fn start_hot_reload(&mut self) -> Result<()> {
        if let Some(config_path) = &self.config_path {
            let config_path = config_path.clone();
            let config_arc = Arc::clone(&self.config);

            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                let mut last_modified = std::time::SystemTime::UNIX_EPOCH;

                loop {
                    interval.tick().await;

                    if let Ok(metadata) = tokio::fs::metadata(&config_path).await {
                        if let Ok(modified) = metadata.modified() {
                            if modified > last_modified {
                                last_modified = modified;

                                match tokio::fs::read_to_string(&config_path).await {
                                    Ok(content) => {
                                        let new_config: Result<AppConfiguration> = if config_path
                                            .ends_with(".yaml")
                                            || config_path.ends_with(".yml")
                                        {
                                            serde_yaml::from_str(&content).map_err(|e| {
                                                AppError::ConfigError(format!(
                                                    "Invalid YAML: {}",
                                                    e
                                                ))
                                            })
                                        } else {
                                            serde_json::from_str(&content).map_err(|e| {
                                                AppError::ConfigError(format!(
                                                    "Invalid JSON: {}",
                                                    e
                                                ))
                                            })
                                        };

                                        match new_config {
                                            Ok(config) => {
                                                let mut current = config_arc.write().await;
                                                *current = config;
                                                info!(
                                                    "🔄 Configuration hot-reloaded from {}",
                                                    config_path
                                                );
                                            }
                                            Err(e) => {
                                                warn!("⚠️ Failed to reload config: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("⚠️ Failed to read config file: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            });

            self.watchers.push(handle);
            info!("👀 Started configuration hot-reload watcher");
        }

        Ok(())
    }

    /// Stop all watchers
    pub fn stop_watchers(&mut self) {
        for handle in self.watchers.drain(..) {
            handle.abort();
        }
        debug!("🛑 Stopped configuration watchers");
    }
}

impl Drop for ConfigManager {
    fn drop(&mut self) {
        self.stop_watchers();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_config_manager_load() {
        let mut manager = ConfigManager::new();
        assert!(manager.load().await.is_ok());

        let config = manager.get().await;
        assert_eq!(config.server.port, 8000);
    }

    #[tokio::test]
    async fn test_config_from_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
server:
  host: "127.0.0.1"
  port: 9000
database:
  mongodb_uri: "mongodb://test:27017"
  database_name: "test_db"
security:
  jwt:
    secret: "test_secret"
    expires_in_seconds: 7200
"#
        )
        .unwrap();

        std::env::set_var("CONFIG_FILE", temp_file.path().to_str().unwrap());

        let mut manager = ConfigManager::new();
        assert!(manager.load().await.is_ok());

        let config = manager.get().await;
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.security.jwt.secret, "test_secret");

        std::env::remove_var("CONFIG_FILE");
    }

    #[test]
    fn test_config_validation() {
        let manager = ConfigManager::new();
        let mut config = AppConfiguration::default();

        // Valid config should pass
        config.security.jwt.secret = "valid_secret".to_string();
        assert!(manager.validate_config(&config).is_ok());

        // Empty JWT secret should fail
        config.security.jwt.secret = "".to_string();
        assert!(manager.validate_config(&config).is_err());
    }
}
