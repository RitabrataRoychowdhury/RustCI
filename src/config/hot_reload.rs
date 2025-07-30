use crate::config::AppConfiguration;
use crate::error::{AppError, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub config: AppConfiguration,
    pub changes: Vec<ConfigChange>,
}

#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub field_path: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
}

/// Hot-reloadable configuration manager
pub struct HotReloadConfigManager {
    config: Arc<RwLock<AppConfiguration>>,
    config_path: Option<String>,
    change_sender: broadcast::Sender<ConfigChangeEvent>,
    _change_receiver: broadcast::Receiver<ConfigChangeEvent>,
    watcher: Option<notify::RecommendedWatcher>,
}

impl Default for HotReloadConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HotReloadConfigManager {
    pub fn new() -> Self {
        let (change_sender, change_receiver) = broadcast::channel(100);

        Self {
            config: Arc::new(RwLock::new(AppConfiguration::default())),
            config_path: None,
            change_sender,
            _change_receiver: change_receiver,
            watcher: None,
        }
    }

    /// Load configuration from environment and optional file
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

        info!("📋 Hot-reload configuration loaded successfully");
        Ok(())
    }

    /// Start watching for configuration file changes
    pub async fn start_hot_reload(&mut self) -> Result<()> {
        if let Some(config_path) = &self.config_path {
            let config_path = config_path.clone();
            let config_arc = Arc::clone(&self.config);
            let change_sender = self.change_sender.clone();

            // Create file watcher
            let (tx, mut rx) = tokio::sync::mpsc::channel(100);

            let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        let _ = tx.try_send(event);
                    }
                }
            })
            .map_err(|e| AppError::ConfigError(format!("Failed to create file watcher: {}", e)))?;

            // Watch the config file
            watcher
                .watch(Path::new(&config_path), RecursiveMode::NonRecursive)
                .map_err(|e| {
                    AppError::ConfigError(format!("Failed to watch config file: {}", e))
                })?;

            self.watcher = Some(watcher);

            // Spawn task to handle file change events
            tokio::spawn(async move {
                while let Some(_event) = rx.recv().await {
                    debug!("📁 Config file change detected, reloading...");

                    // Add a small delay to ensure file write is complete
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    match Self::reload_config_file(&config_path, &config_arc, &change_sender).await
                    {
                        Ok(()) => {
                            info!("🔄 Configuration hot-reloaded successfully");
                        }
                        Err(e) => {
                            error!("❌ Failed to hot-reload configuration: {}", e);
                        }
                    }
                }
            });

            info!("👀 Started configuration hot-reload watcher");
        }

        Ok(())
    }

    /// Reload configuration from file
    async fn reload_config_file(
        config_path: &str,
        config_arc: &Arc<RwLock<AppConfiguration>>,
        change_sender: &broadcast::Sender<ConfigChangeEvent>,
    ) -> Result<()> {
        // Read and parse new configuration
        let content = tokio::fs::read_to_string(config_path)
            .await
            .map_err(|e| AppError::ConfigError(format!("Failed to read config file: {}", e)))?;

        let new_config: AppConfiguration =
            if config_path.ends_with(".yaml") || config_path.ends_with(".yml") {
                serde_yaml::from_str(&content)
                    .map_err(|e| AppError::ConfigError(format!("Invalid YAML config: {}", e)))?
            } else if config_path.ends_with(".json") {
                serde_json::from_str(&content)
                    .map_err(|e| AppError::ConfigError(format!("Invalid JSON config: {}", e)))?
            } else {
                return Err(AppError::ConfigError(
                    "Config file must be .yaml, .yml, or .json".to_string(),
                ));
            };

        // Validate new configuration
        Self::validate_config_static(&new_config)?;

        // Get current configuration for comparison
        let old_config = {
            let config_guard = config_arc.read().await;
            config_guard.clone()
        };

        // Detect changes
        let changes = Self::detect_changes(&old_config, &new_config)?;

        // Update configuration
        {
            let mut config_guard = config_arc.write().await;
            *config_guard = new_config.clone();
        }

        // Notify subscribers of changes
        if !changes.is_empty() {
            let change_event = ConfigChangeEvent {
                timestamp: chrono::Utc::now(),
                config: new_config,
                changes,
            };

            if let Err(e) = change_sender.send(change_event) {
                warn!("Failed to send config change event: {}", e);
            }
        }

        Ok(())
    }

    /// Detect changes between old and new configuration
    fn detect_changes(
        old_config: &AppConfiguration,
        new_config: &AppConfiguration,
    ) -> Result<Vec<ConfigChange>> {
        let old_value = serde_json::to_value(old_config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize old config: {}", e)))?;
        let new_value = serde_json::to_value(new_config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize new config: {}", e)))?;

        let mut changes = Vec::new();
        Self::compare_json_values("", &old_value, &new_value, &mut changes);

        Ok(changes)
    }

    /// Recursively compare JSON values to detect changes
    fn compare_json_values(
        path: &str,
        old: &serde_json::Value,
        new: &serde_json::Value,
        changes: &mut Vec<ConfigChange>,
    ) {
        match (old, new) {
            (serde_json::Value::Object(old_obj), serde_json::Value::Object(new_obj)) => {
                // Check for changed or new fields
                for (key, new_val) in new_obj {
                    let field_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };

                    if let Some(old_val) = old_obj.get(key) {
                        if old_val != new_val {
                            Self::compare_json_values(&field_path, old_val, new_val, changes);
                        }
                    } else {
                        // New field
                        changes.push(ConfigChange {
                            field_path,
                            old_value: None,
                            new_value: new_val.clone(),
                        });
                    }
                }

                // Check for removed fields
                for (key, old_val) in old_obj {
                    if !new_obj.contains_key(key) {
                        let field_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };
                        changes.push(ConfigChange {
                            field_path,
                            old_value: Some(old_val.clone()),
                            new_value: serde_json::Value::Null,
                        });
                    }
                }
            }
            _ => {
                // Primitive value change
                if old != new {
                    changes.push(ConfigChange {
                        field_path: path.to_string(),
                        old_value: Some(old.clone()),
                        new_value: new.clone(),
                    });
                }
            }
        }
    }

    /// Get current configuration
    pub async fn get(&self) -> AppConfiguration {
        self.config.read().await.clone()
    }

    /// Subscribe to configuration changes
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_sender.subscribe()
    }

    /// Update configuration programmatically
    pub async fn update(&self, new_config: AppConfiguration) -> Result<()> {
        Self::validate_config_static(&new_config)?;

        let old_config = {
            let config_guard = self.config.read().await;
            config_guard.clone()
        };

        let changes = Self::detect_changes(&old_config, &new_config)?;

        {
            let mut config_guard = self.config.write().await;
            *config_guard = new_config.clone();
        }

        // Notify subscribers
        if !changes.is_empty() {
            let change_event = ConfigChangeEvent {
                timestamp: chrono::Utc::now(),
                config: new_config,
                changes,
            };

            if let Err(e) = self.change_sender.send(change_event) {
                warn!("Failed to send config change event: {}", e);
            }
        }

        info!("🔄 Configuration updated programmatically");
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
        *config = file_config;

        debug!(path = path, "📁 Configuration loaded from file");
        Ok(())
    }

    /// Validate configuration
    fn validate_config(&self, config: &AppConfiguration) -> Result<()> {
        Self::validate_config_static(config)
    }

    /// Static validation method with comprehensive checks
    fn validate_config_static(config: &AppConfiguration) -> Result<()> {
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

        // Validate server host
        if config.server.host.is_empty() {
            return Err(AppError::ConfigError(
                "Server host cannot be empty".to_string(),
            ));
        }

        // Validate JWT configuration
        if config.security.jwt.expires_in_seconds <= 0 {
            return Err(AppError::ConfigError(
                "JWT expiration must be positive".to_string(),
            ));
        }

        if config.security.jwt.secret.len() < 32 {
            return Err(AppError::ConfigError(
                "JWT secret must be at least 32 characters".to_string(),
            ));
        }

        // Validate encryption key if provided
        if !config.security.encryption.key.is_empty() {
            use base64ct::Encoding;
            let key_bytes = base64ct::Base64::decode_vec(&config.security.encryption.key)
                .map_err(|e| AppError::ConfigError(format!("Invalid encryption key: {}", e)))?;
            if key_bytes.len() != 32 {
                return Err(AppError::ConfigError(
                    "Encryption key must be 32 bytes (base64 encoded)".to_string(),
                ));
            }
        }

        // Validate rate limiting configuration
        if config.security.rate_limiting.requests_per_minute == 0 {
            return Err(AppError::ConfigError(
                "Rate limit requests per minute must be positive".to_string(),
            ));
        }

        if config.security.rate_limiting.burst_size == 0 {
            return Err(AppError::ConfigError(
                "Rate limit burst size must be positive".to_string(),
            ));
        }

        // Validate CORS configuration
        if config.security.cors.allowed_origins.is_empty() {
            return Err(AppError::ConfigError(
                "CORS allowed origins cannot be empty".to_string(),
            ));
        }

        // Validate database configuration
        if config.database.connection_pool_size == 0 {
            return Err(AppError::ConfigError(
                "Database connection pool size must be positive".to_string(),
            ));
        }

        if config.database.connection_timeout_seconds == 0 {
            return Err(AppError::ConfigError(
                "Database connection timeout must be positive".to_string(),
            ));
        }

        // Validate log level
        match config.observability.logging.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(AppError::ConfigError("Invalid log level".to_string())),
        }

        // Validate CI configuration
        if config.ci.max_concurrent_pipelines == 0 {
            return Err(AppError::ConfigError(
                "Max concurrent pipelines must be positive".to_string(),
            ));
        }

        if config.ci.pipeline_timeout_seconds == 0 {
            return Err(AppError::ConfigError(
                "Pipeline timeout must be positive".to_string(),
            ));
        }

        // Validate OAuth configuration if GitHub OAuth is enabled
        if !config.security.oauth.github.client_id.is_empty() {
            if config.security.oauth.github.client_secret.is_empty() {
                return Err(AppError::ConfigError(
                    "GitHub OAuth client secret is required when client ID is provided".to_string(),
                ));
            }

            if config.security.oauth.github.redirect_url.is_empty() {
                return Err(AppError::ConfigError(
                    "GitHub OAuth redirect URL is required".to_string(),
                ));
            }

            // Validate redirect URL format
            if url::Url::parse(&config.security.oauth.github.redirect_url).is_err() {
                return Err(AppError::ConfigError(
                    "Invalid GitHub OAuth redirect URL format".to_string(),
                ));
            }
        }

        // Validate session configuration
        if config.security.session.timeout_minutes == 0 {
            return Err(AppError::ConfigError(
                "Session timeout must be positive".to_string(),
            ));
        }

        if config.security.session.max_concurrent_sessions == 0 {
            return Err(AppError::ConfigError(
                "Max concurrent sessions must be positive".to_string(),
            ));
        }

        // Validate audit configuration
        if config.security.audit.retention_days == 0 {
            return Err(AppError::ConfigError(
                "Audit retention days must be positive".to_string(),
            ));
        }

        // Validate metrics configuration
        if config.observability.metrics.enabled
            && config.observability.metrics.collection_interval_seconds == 0
        {
            return Err(AppError::ConfigError(
                "Metrics collection interval must be positive when metrics are enabled".to_string(),
            ));
        }

        // Validate tracing configuration
        if config.observability.tracing.enabled
            && (config.observability.tracing.sample_rate < 0.0
                || config.observability.tracing.sample_rate > 1.0)
        {
            return Err(AppError::ConfigError(
                "Tracing sample rate must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Validate external services configuration
        if url::Url::parse(&config.external_services.github.api_url).is_err() {
            return Err(AppError::ConfigError(
                "Invalid GitHub API URL format".to_string(),
            ));
        }

        if config.external_services.github.timeout_seconds == 0 {
            return Err(AppError::ConfigError(
                "GitHub API timeout must be positive".to_string(),
            ));
        }

        if config.external_services.github.rate_limit_per_hour == 0 {
            return Err(AppError::ConfigError(
                "GitHub API rate limit must be positive".to_string(),
            ));
        }

        debug!("✅ Comprehensive configuration validation passed");
        Ok(())
    }
}

impl Drop for HotReloadConfigManager {
    fn drop(&mut self) {
        if let Some(mut watcher) = self.watcher.take() {
            let _ = watcher.unwatch(Path::new(""));
        }
        debug!("🛑 Hot-reload configuration manager dropped");
    }
}

/// Configuration change subscriber trait
#[async_trait::async_trait]
pub trait ConfigChangeSubscriber: Send + Sync {
    async fn on_config_change(&self, event: &ConfigChangeEvent) -> Result<()>;
}

/// Service that manages configuration change subscribers
pub struct ConfigChangeManager {
    subscribers: Vec<Box<dyn ConfigChangeSubscriber>>,
}

impl Default for ConfigChangeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigChangeManager {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    pub fn add_subscriber(&mut self, subscriber: Box<dyn ConfigChangeSubscriber>) {
        self.subscribers.push(subscriber);
    }

    pub async fn notify_subscribers(&self, event: &ConfigChangeEvent) {
        for subscriber in &self.subscribers {
            if let Err(e) = subscriber.on_config_change(event).await {
                error!("Config change subscriber error: {}", e);
            }
        }
    }

    /// Start listening for configuration changes
    pub async fn start_listening(
        &self,
        mut change_receiver: broadcast::Receiver<ConfigChangeEvent>,
    ) {
        tokio::spawn(async move {
            while let Ok(event) = change_receiver.recv().await {
                info!(
                    timestamp = %event.timestamp,
                    changes_count = event.changes.len(),
                    "📢 Configuration change event received"
                );

                for change in &event.changes {
                    info!(
                        field = change.field_path,
                        old_value = ?change.old_value,
                        new_value = ?change.new_value,
                        "🔄 Configuration field changed"
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hot_reload_config_manager() {
        let mut manager = HotReloadConfigManager::new();
        assert!(manager.load().await.is_ok());

        let config = manager.get().await;
        assert_eq!(config.server.port, 8000);
    }

    #[tokio::test]
    async fn test_config_change_detection() {
        let old_config = AppConfiguration::default();
        let mut new_config = old_config.clone();
        new_config.server.port = 9000;
        new_config.security.jwt.secret = "new_secret".to_string();

        let changes = HotReloadConfigManager::detect_changes(&old_config, &new_config).unwrap();
        assert!(!changes.is_empty());

        let port_change = changes.iter().find(|c| c.field_path == "server.port");
        assert!(port_change.is_some());

        let secret_change = changes
            .iter()
            .find(|c| c.field_path == "security.jwt.secret");
        assert!(secret_change.is_some());
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "test_secret".to_string();

        assert!(HotReloadConfigManager::validate_config_static(&config).is_ok());

        config.security.jwt.secret = "".to_string();
        assert!(HotReloadConfigManager::validate_config_static(&config).is_err());
    }
}
