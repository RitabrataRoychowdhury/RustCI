use crate::config::{AppConfiguration, ProductionConfigManager, ProductionValidationReport};
use crate::error::{AppError, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, RwLock, Mutex};
use tracing::{debug, error, info, warn};

/// Production-grade hot-reload configuration manager with validation and rollback
pub struct ProductionHotReloadManager {
    config: Arc<RwLock<AppConfiguration>>,
    config_manager: Arc<Mutex<ProductionConfigManager>>,
    config_path: Option<PathBuf>,
    backup_configs: Arc<RwLock<Vec<ConfigBackup>>>,
    change_sender: broadcast::Sender<ConfigChangeEvent>,
    _change_receiver: broadcast::Receiver<ConfigChangeEvent>,
    watcher: Option<notify::RecommendedWatcher>,
    hot_reload_config: HotReloadConfig,
    validation_cache: Arc<RwLock<ValidationCache>>,
    rollback_manager: Arc<RollbackManager>,
    change_detector: Arc<ChangeDetector>,
    notification_system: Arc<NotificationSystem>,
}

/// Hot-reload configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotReloadConfig {
    pub enabled: bool,
    pub validation_timeout_seconds: u64,
    pub max_backup_configs: usize,
    pub debounce_delay_ms: u64,
    pub auto_rollback_on_failure: bool,
    pub notification_channels: Vec<NotificationChannel>,
    pub validation_rules: ValidationRules,
    pub file_watch_patterns: Vec<String>,
    pub environment_overrides: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub require_validation_before_apply: bool,
    pub fail_on_warnings: bool,
    pub custom_validation_scripts: Vec<String>,
    pub schema_validation_enabled: bool,
    pub environment_specific_validation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub name: String,
    pub channel_type: NotificationChannelType,
    pub config: HashMap<String, String>,
    pub events: Vec<HotReloadEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannelType {
    Webhook,
    Email,
    Slack,
    Discord,
    Log,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HotReloadEvent {
    ConfigChanged,
    ValidationFailed,
    RollbackTriggered,
    ValidationPassed,
    FileWatchError,
}

/// Configuration change event with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub event_id: uuid::Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: ConfigChangeEventType,
    pub config: AppConfiguration,
    pub changes: Vec<ConfigChange>,
    pub validation_report: Option<ProductionValidationReport>,
    pub source: ConfigChangeSource,
    pub metadata: ConfigChangeMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChangeEventType {
    FileChanged,
    ProgrammaticUpdate,
    EnvironmentChanged,
    Rollback,
    ValidationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChangeSource {
    FileSystem { path: PathBuf },
    Environment,
    Api { user_id: Option<String> },
    Automatic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeMetadata {
    pub hostname: String,
    pub process_id: u32,
    pub environment: String,
    pub git_commit: Option<String>,
    pub deployment_id: Option<String>,
    pub correlation_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    pub field_path: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
    pub change_type: ChangeType,
    pub impact_assessment: ImpactAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
    TypeChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub severity: ImpactSeverity,
    pub affected_components: Vec<String>,
    pub requires_restart: bool,
    pub backward_compatible: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Configuration backup with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBackup {
    pub id: uuid::Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub config: AppConfiguration,
    pub validation_report: ProductionValidationReport,
    pub source: ConfigChangeSource,
    pub description: String,
    pub tags: HashMap<String, String>,
}

/// Validation cache for performance optimization
#[derive(Debug, Clone)]
struct ValidationCache {
    entries: HashMap<String, CachedValidationResult>,
}

#[derive(Debug, Clone)]
struct CachedValidationResult {
    config_hash: u64,
    validation_report: ProductionValidationReport,
    timestamp: chrono::DateTime<chrono::Utc>,
    ttl: Duration,
}

/// Rollback manager for configuration recovery
pub struct RollbackManager {
    backups: Arc<RwLock<Vec<ConfigBackup>>>,
    max_backups: usize,
    auto_rollback_enabled: bool,
}

/// Change detection system with impact analysis
pub struct ChangeDetector {
    impact_rules: HashMap<String, ImpactRule>,
}

#[derive(Debug, Clone)]
struct ImpactRule {
    field_pattern: String,
    severity: ImpactSeverity,
    affected_components: Vec<String>,
    requires_restart: bool,
    backward_compatible: bool,
    description: String,
}

/// Notification system for configuration changes
pub struct NotificationSystem {
    channels: Vec<NotificationChannel>,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            validation_timeout_seconds: 30,
            max_backup_configs: 10,
            debounce_delay_ms: 500,
            auto_rollback_on_failure: true,
            notification_channels: vec![
                NotificationChannel {
                    name: "default_log".to_string(),
                    channel_type: NotificationChannelType::Log,
                    config: HashMap::new(),
                    events: vec![
                        HotReloadEvent::ConfigChanged,
                        HotReloadEvent::ValidationFailed,
                        HotReloadEvent::RollbackTriggered,
                    ],
                }
            ],
            validation_rules: ValidationRules {
                require_validation_before_apply: true,
                fail_on_warnings: false,
                custom_validation_scripts: Vec::new(),
                schema_validation_enabled: true,
                environment_specific_validation: true,
            },
            file_watch_patterns: vec!["*.yaml".to_string(), "*.yml".to_string(), "*.json".to_string()],
            environment_overrides: HashMap::new(),
        }
    }
}

impl ProductionHotReloadManager {
    /// Create a new production hot-reload manager
    pub fn new(environment: &str, hot_reload_config: Option<HotReloadConfig>) -> Self {
        let (change_sender, change_receiver) = broadcast::channel(1000);
        let config_manager = Arc::new(Mutex::new(ProductionConfigManager::new(environment)));
        let hot_reload_config = hot_reload_config.unwrap_or_default();

        Self {
            config: Arc::new(RwLock::new(AppConfiguration::default())),
            config_manager,
            config_path: None,
            backup_configs: Arc::new(RwLock::new(Vec::new())),
            change_sender,
            _change_receiver: change_receiver,
            watcher: None,
            hot_reload_config,
            validation_cache: Arc::new(RwLock::new(ValidationCache {
                entries: HashMap::new(),
            })),
            rollback_manager: Arc::new(RollbackManager::new(10, true)),
            change_detector: Arc::new(ChangeDetector::new()),
            notification_system: Arc::new(NotificationSystem::new(Vec::new())),
        }
    }

    /// Load and validate configuration with comprehensive validation
    pub async fn load_and_validate(&mut self) -> Result<ProductionValidationReport> {
        info!("🔍 Loading configuration with production validation");

        // Load configuration using the production config manager
        let mut config_manager = self.config_manager.lock().await;
        let validation_report = config_manager.load_and_validate().await?;
        let config = config_manager.get_config().await;

        // Update current configuration
        {
            let mut current_config = self.config.write().await;
            *current_config = config.clone();
        }

        // Create initial backup
        self.create_backup(
            config,
            validation_report.clone(),
            ConfigChangeSource::Automatic,
            "Initial configuration load".to_string(),
        ).await?;

        info!("✅ Configuration loaded and validated successfully");
        Ok(validation_report)
    }

    /// Start hot-reload with enhanced file watching and validation
    pub async fn start_hot_reload(&mut self, config_path: PathBuf) -> Result<()> {
        if !self.hot_reload_config.enabled {
            info!("🚫 Hot-reload is disabled");
            return Ok(());
        }

        info!("🔥 Starting production hot-reload system");

        self.config_path = Some(config_path.clone());

        // Create file watcher with debouncing
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let debounce_delay = Duration::from_millis(self.hot_reload_config.debounce_delay_ms);

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    let _ = tx.try_send(event);
                }
            }
        })
        .map_err(|e| AppError::ConfigError(format!("Failed to create file watcher: {}", e)))?;

        // Watch the config file and directory
        watcher
            .watch(&config_path, RecursiveMode::NonRecursive)
            .map_err(|e| AppError::ConfigError(format!("Failed to watch config file: {}", e)))?;

        // Watch parent directory for file moves/renames
        if let Some(parent) = config_path.parent() {
            watcher
                .watch(parent, RecursiveMode::NonRecursive)
                .map_err(|e| AppError::ConfigError(format!("Failed to watch config directory: {}", e)))?;
        }

        self.watcher = Some(watcher);

        // Clone necessary data for the async task
        let config_arc = Arc::clone(&self.config);
        let config_manager_arc = Arc::clone(&self.config_manager);
        let change_sender = self.change_sender.clone();
        let rollback_manager = Arc::clone(&self.rollback_manager);
        let change_detector = Arc::clone(&self.change_detector);
        let notification_system = Arc::clone(&self.notification_system);
        let hot_reload_config = self.hot_reload_config.clone();
        let validation_cache = Arc::clone(&self.validation_cache);
        let backup_configs = Arc::clone(&self.backup_configs);

        // Spawn debounced file change handler
        tokio::spawn(async move {
            let mut last_event_time = SystemTime::UNIX_EPOCH;
            
            while let Some(event) = rx.recv().await {
                let now = SystemTime::now();
                
                // Check if this event is for our config file
                let is_config_file = event.paths.iter().any(|path| {
                    path == &config_path || 
                    (path.file_name() == config_path.file_name() && 
                     path.parent() == config_path.parent())
                });

                if !is_config_file {
                    continue;
                }

                // Debounce rapid file changes
                if now.duration_since(last_event_time).unwrap_or(Duration::MAX) < debounce_delay {
                    continue;
                }
                last_event_time = now;

                debug!("📁 Config file change detected: {:?}", event);

                // Add additional delay to ensure file write is complete
                tokio::time::sleep(Duration::from_millis(200)).await;

                match Self::handle_config_file_change(
                    &config_path,
                    &config_arc,
                    &config_manager_arc,
                    &change_sender,
                    &rollback_manager,
                    &change_detector,
                    &notification_system,
                    &hot_reload_config,
                    &validation_cache,
                    &backup_configs,
                ).await {
                    Ok(()) => {
                        info!("🔄 Configuration hot-reloaded successfully");
                    }
                    Err(e) => {
                        error!("❌ Failed to hot-reload configuration: {}", e);
                        
                        // Send notification about failure
                        let _ = notification_system.notify(
                            HotReloadEvent::ValidationFailed,
                            &format!("Hot-reload failed: {}", e),
                        ).await;

                        // Attempt auto-rollback if enabled
                        if hot_reload_config.auto_rollback_on_failure {
                            if let Err(rollback_err) = rollback_manager.auto_rollback(
                                &config_arc,
                                &change_sender,
                            ).await {
                                error!("❌ Auto-rollback failed: {}", rollback_err);
                            } else {
                                info!("🔄 Auto-rollback completed successfully");
                            }
                        }
                    }
                }
            }
        });

        info!("👀 Production hot-reload system started");
        Ok(())
    }

    /// Handle configuration file change with comprehensive validation and rollback
    async fn handle_config_file_change(
        config_path: &PathBuf,
        config_arc: &Arc<RwLock<AppConfiguration>>,
        config_manager_arc: &Arc<Mutex<ProductionConfigManager>>,
        change_sender: &broadcast::Sender<ConfigChangeEvent>,
        _rollback_manager: &Arc<RollbackManager>,
        change_detector: &Arc<ChangeDetector>,
        notification_system: &Arc<NotificationSystem>,
        hot_reload_config: &HotReloadConfig,
        validation_cache: &Arc<RwLock<ValidationCache>>,
        backup_configs: &Arc<RwLock<Vec<ConfigBackup>>>,
    ) -> Result<()> {
        // Read and parse new configuration
        let content = tokio::fs::read_to_string(config_path)
            .await
            .map_err(|e| AppError::ConfigError(format!("Failed to read config file: {}", e)))?;

        let new_config: AppConfiguration = Self::parse_config_content(&content, config_path)?;

        // Get current configuration for comparison
        let old_config = {
            let config_guard = config_arc.read().await;
            config_guard.clone()
        };

        // Detect changes with impact analysis
        let changes = change_detector.detect_changes_with_impact(&old_config, &new_config)?;

        if changes.is_empty() {
            debug!("📋 No configuration changes detected");
            return Ok(());
        }

        // Check validation cache first
        let config_hash = Self::calculate_config_hash(&new_config);
        if let Some(cached_result) = Self::check_validation_cache(validation_cache, config_hash).await {
            if cached_result.validation_report.is_valid {
                info!("📋 Using cached validation result");
                Self::apply_config_change(
                    config_arc,
                    new_config,
                    changes,
                    cached_result.validation_report,
                    change_sender,
                    ConfigChangeSource::FileSystem { path: config_path.clone() },
                ).await?;
                return Ok(());
            }
        }

        // Validate new configuration
        let validation_report = {
            let _config_manager = config_manager_arc.lock().await;
            // Create a temporary config manager with the new config for validation
            let _temp_manager = ProductionConfigManager::new("production"); // TODO: get actual environment
            // For now, we'll use basic validation - in a full implementation,
            // we'd have a way to validate without loading
            Self::validate_config_basic(&new_config)?;
            
            // Create a mock validation report for now
            ProductionValidationReport::new("production")
        };

        // Cache validation result
        Self::cache_validation_result(validation_cache, config_hash, &validation_report).await;

        // Check if validation passed
        if !validation_report.is_valid {
            let error_msg = format!("Configuration validation failed: {} errors", 
                validation_report.critical_issues.len());
            
            // Send notification
            let _ = notification_system.notify(
                HotReloadEvent::ValidationFailed,
                &error_msg,
            ).await;

            return Err(AppError::ConfigError(error_msg));
        }

        // Check validation rules
        if hot_reload_config.validation_rules.fail_on_warnings && !validation_report.warnings.is_empty() {
            let error_msg = format!("Configuration has warnings and fail_on_warnings is enabled: {} warnings",
                validation_report.warnings.len());
            return Err(AppError::ConfigError(error_msg));
        }

        // Create backup before applying changes
        Self::create_backup_static(
            backup_configs,
            old_config,
            validation_report.clone(),
            ConfigChangeSource::FileSystem { path: config_path.clone() },
            "Pre-hot-reload backup".to_string(),
        ).await?;

        // Apply configuration change
        Self::apply_config_change(
            config_arc,
            new_config,
            changes,
            validation_report,
            change_sender,
            ConfigChangeSource::FileSystem { path: config_path.clone() },
        ).await?;

        // Send success notification
        let _ = notification_system.notify(
            HotReloadEvent::ConfigChanged,
            "Configuration hot-reloaded successfully",
        ).await;

        Ok(())
    }

    /// Apply configuration change with event notification
    async fn apply_config_change(
        config_arc: &Arc<RwLock<AppConfiguration>>,
        new_config: AppConfiguration,
        changes: Vec<ConfigChange>,
        validation_report: ProductionValidationReport,
        change_sender: &broadcast::Sender<ConfigChangeEvent>,
        source: ConfigChangeSource,
    ) -> Result<()> {
        // Update configuration
        {
            let mut config_guard = config_arc.write().await;
            *config_guard = new_config.clone();
        }

        // Create change event
        let change_event = ConfigChangeEvent {
            event_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            event_type: ConfigChangeEventType::FileChanged,
            config: new_config,
            changes,
            validation_report: Some(validation_report),
            source,
            metadata: ConfigChangeMetadata {
                hostname: hostname::get()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                process_id: std::process::id(),
                environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "unknown".to_string()),
                git_commit: std::env::var("GIT_COMMIT").ok(),
                deployment_id: std::env::var("DEPLOYMENT_ID").ok(),
                correlation_id: uuid::Uuid::new_v4(),
            },
        };

        // Notify subscribers
        if let Err(e) = change_sender.send(change_event) {
            warn!("Failed to send config change event: {}", e);
        }

        Ok(())
    }

    /// Parse configuration content based on file extension
    fn parse_config_content(content: &str, config_path: &PathBuf) -> Result<AppConfiguration> {
        let extension = config_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            "yaml" | "yml" => {
                serde_yaml::from_str(content)
                    .map_err(|e| AppError::ConfigError(format!("Invalid YAML config: {}", e)))
            }
            "json" => {
                serde_json::from_str(content)
                    .map_err(|e| AppError::ConfigError(format!("Invalid JSON config: {}", e)))
            }
            _ => Err(AppError::ConfigError(
                "Config file must be .yaml, .yml, or .json".to_string(),
            )),
        }
    }

    /// Basic configuration validation (placeholder for full validation)
    fn validate_config_basic(config: &AppConfiguration) -> Result<()> {
        if config.security.jwt.secret.is_empty() {
            return Err(AppError::ConfigError("JWT secret is required".to_string()));
        }
        if config.database.mongodb_uri.is_empty() {
            return Err(AppError::ConfigError("MongoDB URI is required".to_string()));
        }
        if config.server.port == 0 {
            return Err(AppError::ConfigError("Invalid server port".to_string()));
        }
        Ok(())
    }

    /// Calculate configuration hash for caching
    fn calculate_config_hash(config: &AppConfiguration) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        format!("{:?}", config).hash(&mut hasher);
        hasher.finish()
    }

    /// Check validation cache
    async fn check_validation_cache(
        validation_cache: &Arc<RwLock<ValidationCache>>,
        config_hash: u64,
    ) -> Option<CachedValidationResult> {
        let cache = validation_cache.read().await;
        if let Some(cached) = cache.entries.get("main") {
            if cached.config_hash == config_hash {
                let age = chrono::Utc::now().signed_duration_since(cached.timestamp);
                if age < chrono::Duration::from_std(cached.ttl).unwrap_or_default() {
                    return Some(cached.clone());
                }
            }
        }
        None
    }

    /// Cache validation result
    async fn cache_validation_result(
        validation_cache: &Arc<RwLock<ValidationCache>>,
        config_hash: u64,
        validation_report: &ProductionValidationReport,
    ) {
        let mut cache = validation_cache.write().await;
        cache.entries.insert("main".to_string(), CachedValidationResult {
            config_hash,
            validation_report: validation_report.clone(),
            timestamp: chrono::Utc::now(),
            ttl: Duration::from_secs(300), // 5 minutes
        });
    }

    /// Create configuration backup
    async fn create_backup(
        &self,
        config: AppConfiguration,
        validation_report: ProductionValidationReport,
        source: ConfigChangeSource,
        description: String,
    ) -> Result<()> {
        Self::create_backup_static(
            &self.backup_configs,
            config,
            validation_report,
            source,
            description,
        ).await
    }

    /// Create configuration backup (static version)
    async fn create_backup_static(
        backup_configs: &Arc<RwLock<Vec<ConfigBackup>>>,
        config: AppConfiguration,
        validation_report: ProductionValidationReport,
        source: ConfigChangeSource,
        description: String,
    ) -> Result<()> {
        let backup = ConfigBackup {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            config,
            validation_report,
            source,
            description,
            tags: HashMap::new(),
        };

        let mut backups = backup_configs.write().await;
        backups.push(backup);

        // Keep only the most recent backups
        if backups.len() > 10 {
            backups.remove(0);
        }

        Ok(())
    }

    /// Get current configuration
    pub async fn get_config(&self) -> AppConfiguration {
        self.config.read().await.clone()
    }

    /// Subscribe to configuration changes
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_sender.subscribe()
    }

    /// Manually trigger configuration reload
    pub async fn reload_config(&self) -> Result<()> {
        if let Some(config_path) = &self.config_path {
            Self::handle_config_file_change(
                config_path,
                &self.config,
                &self.config_manager,
                &self.change_sender,
                &self.rollback_manager,
                &self.change_detector,
                &self.notification_system,
                &self.hot_reload_config,
                &self.validation_cache,
                &self.backup_configs,
            ).await
        } else {
            Err(AppError::ConfigError("No config file path set".to_string()))
        }
    }

    /// Get configuration backups
    pub async fn get_backups(&self) -> Vec<ConfigBackup> {
        self.backup_configs.read().await.clone()
    }

    /// Rollback to a specific backup
    pub async fn rollback_to_backup(&self, backup_id: uuid::Uuid) -> Result<()> {
        self.rollback_manager.rollback_to_backup(
            backup_id,
            &self.backup_configs,
            &self.config,
            &self.change_sender,
        ).await
    }
}

impl RollbackManager {
    fn new(max_backups: usize, auto_rollback_enabled: bool) -> Self {
        Self {
            backups: Arc::new(RwLock::new(Vec::new())),
            max_backups,
            auto_rollback_enabled,
        }
    }

    async fn auto_rollback(
        &self,
        config_arc: &Arc<RwLock<AppConfiguration>>,
        change_sender: &broadcast::Sender<ConfigChangeEvent>,
    ) -> Result<()> {
        if !self.auto_rollback_enabled {
            return Err(AppError::ConfigError("Auto-rollback is disabled".to_string()));
        }

        // Get the most recent backup
        let backup = {
            let backups = self.backups.read().await;
            backups.last().cloned()
        };

        if let Some(backup) = backup {
            info!("🔄 Performing auto-rollback to backup: {}", backup.id);
            
            // Apply the backup configuration
            {
                let mut config_guard = config_arc.write().await;
                *config_guard = backup.config.clone();
            }

            // Send rollback event
            let rollback_event = ConfigChangeEvent {
                event_id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                event_type: ConfigChangeEventType::Rollback,
                config: backup.config,
                changes: Vec::new(),
                validation_report: Some(backup.validation_report),
                source: ConfigChangeSource::Automatic,
                metadata: ConfigChangeMetadata {
                    hostname: hostname::get()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    process_id: std::process::id(),
                    environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "unknown".to_string()),
                    git_commit: std::env::var("GIT_COMMIT").ok(),
                    deployment_id: std::env::var("DEPLOYMENT_ID").ok(),
                    correlation_id: uuid::Uuid::new_v4(),
                },
            };

            let _ = change_sender.send(rollback_event);
            Ok(())
        } else {
            Err(AppError::ConfigError("No backup available for rollback".to_string()))
        }
    }

    async fn rollback_to_backup(
        &self,
        backup_id: uuid::Uuid,
        backup_configs: &Arc<RwLock<Vec<ConfigBackup>>>,
        config_arc: &Arc<RwLock<AppConfiguration>>,
        change_sender: &broadcast::Sender<ConfigChangeEvent>,
    ) -> Result<()> {
        let backup = {
            let backups = backup_configs.read().await;
            backups.iter().find(|b| b.id == backup_id).cloned()
        };

        if let Some(backup) = backup {
            info!("🔄 Rolling back to backup: {}", backup.id);
            
            // Apply the backup configuration
            {
                let mut config_guard = config_arc.write().await;
                *config_guard = backup.config.clone();
            }

            // Send rollback event
            let rollback_event = ConfigChangeEvent {
                event_id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                event_type: ConfigChangeEventType::Rollback,
                config: backup.config,
                changes: Vec::new(),
                validation_report: Some(backup.validation_report),
                source: ConfigChangeSource::Api { user_id: None },
                metadata: ConfigChangeMetadata {
                    hostname: hostname::get()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    process_id: std::process::id(),
                    environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "unknown".to_string()),
                    git_commit: std::env::var("GIT_COMMIT").ok(),
                    deployment_id: std::env::var("DEPLOYMENT_ID").ok(),
                    correlation_id: uuid::Uuid::new_v4(),
                },
            };

            let _ = change_sender.send(rollback_event);
            Ok(())
        } else {
            Err(AppError::ConfigError(format!("Backup with ID {} not found", backup_id)))
        }
    }
}

impl ChangeDetector {
    fn new() -> Self {
        let mut impact_rules = HashMap::new();
        
        // Define impact rules for different configuration sections
        impact_rules.insert("security.*".to_string(), ImpactRule {
            field_pattern: "security.*".to_string(),
            severity: ImpactSeverity::High,
            affected_components: vec!["authentication".to_string(), "authorization".to_string()],
            requires_restart: false,
            backward_compatible: true,
            description: "Security configuration changes".to_string(),
        });

        impact_rules.insert("database.*".to_string(), ImpactRule {
            field_pattern: "database.*".to_string(),
            severity: ImpactSeverity::Critical,
            affected_components: vec!["database".to_string(), "repositories".to_string()],
            requires_restart: true,
            backward_compatible: false,
            description: "Database configuration changes require restart".to_string(),
        });

        impact_rules.insert("server.port".to_string(), ImpactRule {
            field_pattern: "server.port".to_string(),
            severity: ImpactSeverity::Critical,
            affected_components: vec!["server".to_string(), "networking".to_string()],
            requires_restart: true,
            backward_compatible: false,
            description: "Server port changes require restart".to_string(),
        });

        Self { impact_rules }
    }

    fn detect_changes_with_impact(
        &self,
        old_config: &AppConfiguration,
        new_config: &AppConfiguration,
    ) -> Result<Vec<ConfigChange>> {
        let old_value = serde_json::to_value(old_config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize old config: {}", e)))?;
        let new_value = serde_json::to_value(new_config)
            .map_err(|e| AppError::ConfigError(format!("Failed to serialize new config: {}", e)))?;

        let mut changes = Vec::new();
        self.compare_json_values_with_impact("", &old_value, &new_value, &mut changes);

        Ok(changes)
    }

    fn compare_json_values_with_impact(
        &self,
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
                            self.compare_json_values_with_impact(&field_path, old_val, new_val, changes);
                        }
                    } else {
                        // New field
                        let impact = self.assess_impact(&field_path, None, new_val);
                        changes.push(ConfigChange {
                            field_path,
                            old_value: None,
                            new_value: new_val.clone(),
                            change_type: ChangeType::Added,
                            impact_assessment: impact,
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
                        let impact = self.assess_impact(&field_path, Some(old_val), &serde_json::Value::Null);
                        changes.push(ConfigChange {
                            field_path,
                            old_value: Some(old_val.clone()),
                            new_value: serde_json::Value::Null,
                            change_type: ChangeType::Removed,
                            impact_assessment: impact,
                        });
                    }
                }
            }
            _ => {
                // Primitive value change
                if old != new {
                    let change_type = if old.type_name() != new.type_name() {
                        ChangeType::TypeChanged
                    } else {
                        ChangeType::Modified
                    };
                    
                    let impact = self.assess_impact(path, Some(old), new);
                    changes.push(ConfigChange {
                        field_path: path.to_string(),
                        old_value: Some(old.clone()),
                        new_value: new.clone(),
                        change_type,
                        impact_assessment: impact,
                    });
                }
            }
        }
    }

    fn assess_impact(
        &self,
        field_path: &str,
        _old_value: Option<&serde_json::Value>,
        _new_value: &serde_json::Value,
    ) -> ImpactAssessment {
        // Find matching impact rule
        for (pattern, rule) in &self.impact_rules {
            if self.matches_pattern(field_path, pattern) {
                return ImpactAssessment {
                    severity: rule.severity.clone(),
                    affected_components: rule.affected_components.clone(),
                    requires_restart: rule.requires_restart,
                    backward_compatible: rule.backward_compatible,
                    description: rule.description.clone(),
                };
            }
        }

        // Default impact assessment
        ImpactAssessment {
            severity: ImpactSeverity::Low,
            affected_components: vec!["configuration".to_string()],
            requires_restart: false,
            backward_compatible: true,
            description: "General configuration change".to_string(),
        }
    }

    fn matches_pattern(&self, field_path: &str, pattern: &str) -> bool {
        // Simple pattern matching - in production, you'd use a proper regex or glob library
        if pattern.ends_with("*") {
            let prefix = &pattern[..pattern.len() - 1];
            field_path.starts_with(prefix)
        } else {
            field_path == pattern
        }
    }
}

impl NotificationSystem {
    fn new(channels: Vec<NotificationChannel>) -> Self {
        Self { channels }
    }

    async fn notify(&self, event: HotReloadEvent, message: &str) -> Result<()> {
        for channel in &self.channels {
            if channel.events.contains(&event) {
                match channel.channel_type {
                    NotificationChannelType::Log => {
                        info!("📢 [{}] {}", channel.name, message);
                    }
                    NotificationChannelType::Webhook => {
                        // Implement webhook notification
                        debug!("📡 Webhook notification: {}", message);
                    }
                    NotificationChannelType::Email => {
                        // Implement email notification
                        debug!("📧 Email notification: {}", message);
                    }
                    NotificationChannelType::Slack => {
                        // Implement Slack notification
                        debug!("💬 Slack notification: {}", message);
                    }
                    NotificationChannelType::Discord => {
                        // Implement Discord notification
                        debug!("🎮 Discord notification: {}", message);
                    }
                }
            }
        }
        Ok(())
    }
}

trait ValueType {
    fn type_name(&self) -> &'static str;
}

impl ValueType for serde_json::Value {
    fn type_name(&self) -> &'static str {
        match self {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfiguration;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};
    use tokio::time::{sleep, Duration};

    fn create_test_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "test_secret_that_is_long_enough_for_validation".to_string();
        config.security.encryption.key = "dGVzdF9lbmNyeXB0aW9uX2tleV90aGF0X2lzXzMyX2J5dGVz".to_string();
        config.database.mongodb_uri = "mongodb://localhost:27017/test".to_string();
        config
    }

    fn create_hot_reload_config() -> HotReloadConfig {
        HotReloadConfig {
            enabled: true,
            validation_timeout_seconds: 10,
            max_backup_configs: 5,
            debounce_delay_ms: 100,
            auto_rollback_on_failure: true,
            notification_channels: vec![
                NotificationChannel {
                    name: "test_log".to_string(),
                    channel_type: NotificationChannelType::Log,
                    config: HashMap::new(),
                    events: vec![HotReloadEvent::ConfigChanged, HotReloadEvent::ValidationFailed],
                }
            ],
            validation_rules: ValidationRules {
                require_validation_before_apply: true,
                fail_on_warnings: false,
                custom_validation_scripts: Vec::new(),
                schema_validation_enabled: true,
                environment_specific_validation: true,
            },
            file_watch_patterns: vec!["*.yaml".to_string()],
            environment_overrides: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_production_hot_reload_manager_creation() {
        let hot_reload_config = create_hot_reload_config();
        let manager = ProductionHotReloadManager::new("test", Some(hot_reload_config));
        
        let config = manager.get_config().await;
        assert_eq!(config.server.port, 8000); // Default port
    }

    #[tokio::test]
    async fn test_config_file_parsing() {
        // Test YAML parsing
        let yaml_content = r#"
server:
  host: "127.0.0.1"
  port: 9000
database:
  mongodb_uri: "mongodb://test:27017/testdb"
security:
  jwt:
    secret: "test_secret_that_is_long_enough_for_validation"
"#;
        
        let temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        let config_path = temp_file.path().to_path_buf();
        
        let config = ProductionHotReloadManager::parse_config_content(yaml_content, &config_path).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.database.mongodb_uri, "mongodb://test:27017/testdb");
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = create_test_config();
        
        // Valid configuration should pass
        assert!(ProductionHotReloadManager::validate_config_basic(&config).is_ok());
        
        // Invalid configurations should fail
        config.security.jwt.secret = "".to_string();
        assert!(ProductionHotReloadManager::validate_config_basic(&config).is_err());
    }

    #[tokio::test]
    async fn test_change_detection_with_impact() {
        let change_detector = ChangeDetector::new();
        
        let old_config = create_test_config();
        let mut new_config = old_config.clone();
        
        // Make some changes
        new_config.server.port = 9000; // Should be critical impact
        new_config.security.jwt.secret = "new_secret_that_is_long_enough_for_validation".to_string(); // Should be high impact

        let changes = change_detector.detect_changes_with_impact(&old_config, &new_config).unwrap();
        
        assert!(!changes.is_empty());
        
        // Find the port change
        let port_change = changes.iter().find(|c| c.field_path == "server.port");
        assert!(port_change.is_some());
        let port_change = port_change.unwrap();
        assert!(matches!(port_change.impact_assessment.severity, ImpactSeverity::Critical));
        assert!(port_change.impact_assessment.requires_restart);
    }

    #[tokio::test]
    async fn test_impact_rule_matching() {
        let change_detector = ChangeDetector::new();
        
        // Test pattern matching
        assert!(change_detector.matches_pattern("security.jwt.secret", "security.*"));
        assert!(change_detector.matches_pattern("database.mongodb_uri", "database.*"));
        assert!(change_detector.matches_pattern("server.port", "server.port"));
        assert!(!change_detector.matches_pattern("server.host", "server.port"));
        assert!(!change_detector.matches_pattern("logging.level", "security.*"));
    }
}