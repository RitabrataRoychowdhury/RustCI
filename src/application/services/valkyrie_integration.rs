// Valkyrie Integration Service
// Task 3.2: Unified Control Plane Integration

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use std::time::Instant;

use tokio::sync::{RwLock, Mutex};

use crate::config::valkyrie_integration::ValkyrieIntegrationConfig;
use crate::core::networking::valkyrie::engine::{
    ValkyrieEngine, ValkyrieConfig,
    ProtocolConfig, TransportLayerConfig, SecurityConfig, StreamingConfig, RoutingConfig,
    PerformanceConfig, ObservabilityConfig, FeatureFlags, TransportSelectionStrategy, ConnectionPoolConfig, ConnectionReuseStrategy,
    ProtocolTimeouts, AuthenticationConfig, EncryptionConfig, AuthorizationConfig, AuditConfig,
    FlowControlConfig, LoadBalancingStrategy, CipherSuite, AuthMethod,
    MetricsConfig, TracingConfig, LoggingConfig, LogFormat,
};
use crate::core::networking::transport::{
    TransportConfig,
    TransportType,
    TimeoutConfig,
    BufferConfig,
};

use crate::error::{AppError, Result};
use crate::infrastructure::runners::{
    ValkyrieRunnerAdapter, ValkyrieAdapterConfig, ValkyrieRunnerTrait,
};


/// Service for managing Valkyrie Protocol integration with RustCI
pub struct ValkyrieIntegrationService {
    // Core Valkyrie components
    valkyrie_engine: Option<Arc<ValkyrieEngine>>,
    valkyrie_adapter: Option<Arc<ValkyrieRunnerAdapter>>,
    
    // Configuration management
    config: Arc<RwLock<ValkyrieIntegrationConfig>>,
    config_validator: Arc<ConfigValidator>,
    
    // Integration state
    integration_state: Arc<RwLock<IntegrationState>>,
    
    // Service management
    service_manager: Arc<ServiceManager>,
    health_monitor: Arc<HealthMonitor>,
    
    // Hot reload support
    config_watcher: Arc<Mutex<Option<ConfigWatcher>>>,
}

/// Integration state tracking
#[derive(Debug, Clone)]
pub struct IntegrationState {
    pub initialized: bool,
    pub valkyrie_engine_status: ComponentStatus,
    pub valkyrie_adapter_status: ComponentStatus,
    pub last_health_check: Option<Instant>,
    pub error_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Disabled,
    Initializing,
}

/// Configuration validator for Valkyrie integration
pub struct ConfigValidator {
    validation_rules: Vec<ValidationRule>,
}

/// Validation rule for configuration
pub struct ValidationRule {
    pub name: String,
    pub validator: Box<dyn Fn(&ValkyrieIntegrationConfig) -> Result<()> + Send + Sync>,
}

/// Service manager for Valkyrie components
pub struct ServiceManager {
    startup_timeout: Duration,
    shutdown_timeout: Duration,
    restart_backoff: Duration,
}

/// Health monitor for Valkyrie integration
pub struct HealthMonitor {
    check_interval: Duration,
    failure_threshold: u32,
    recovery_threshold: u32,
}

/// Configuration watcher for hot reload
pub struct ConfigWatcher {
    config_path: String,
    last_modified: Option<std::time::SystemTime>,
    reload_callback: Box<dyn Fn(ValkyrieIntegrationConfig) -> Result<()> + Send + Sync>,
}

impl ValkyrieIntegrationService {
    /// Create a new ValkyrieIntegrationService
    pub async fn new(config: ValkyrieIntegrationConfig) -> Result<Self> {
        let config_validator = Arc::new(ConfigValidator::new());
        
        // Validate initial configuration
        config_validator.validate(&config)?;
        
        let integration_state = IntegrationState {
            initialized: false,
            valkyrie_engine_status: ComponentStatus::Disabled,
            valkyrie_adapter_status: ComponentStatus::Disabled,
            last_health_check: None,
            error_count: 0,
            last_error: None,
        };
        
        let service_manager = Arc::new(ServiceManager {
            startup_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(10),
            restart_backoff: Duration::from_secs(5),
        });
        
        let health_monitor = Arc::new(HealthMonitor {
            check_interval: Duration::from_secs(30),
            failure_threshold: 3,
            recovery_threshold: 2,
        });
        
        let service = Self {
            valkyrie_engine: None,
            valkyrie_adapter: None,
            config: Arc::new(RwLock::new(config)),
            config_validator,
            integration_state: Arc::new(RwLock::new(integration_state)),
            service_manager,
            health_monitor,
            config_watcher: Arc::new(Mutex::new(None)),
        };
        
        Ok(service)
    }
    
    /// Initialize Valkyrie integration
    pub async fn initialize(&mut self) -> Result<()> {
        let config = self.config.read().await;
        
        if !config.valkyrie_enabled {
            tracing::info!("Valkyrie integration disabled in configuration");
            return Ok(());
        }
        
        tracing::info!("Initializing Valkyrie integration...");
        
        // Update state to initializing
        {
            let mut state = self.integration_state.write().await;
            state.valkyrie_engine_status = ComponentStatus::Initializing;
            state.valkyrie_adapter_status = ComponentStatus::Initializing;
        }
        
        // Initialize Valkyrie engine
        let valkyrie_engine = self.initialize_valkyrie_engine(&config).await?;
        self.valkyrie_engine = Some(valkyrie_engine.clone());
        
        // Initialize Valkyrie adapter
        let valkyrie_adapter = self.initialize_valkyrie_adapter(&config, valkyrie_engine).await?;
        self.valkyrie_adapter = Some(valkyrie_adapter);
        
        // Update state to initialized
        {
            let mut state = self.integration_state.write().await;
            state.initialized = true;
            state.valkyrie_engine_status = ComponentStatus::Healthy;
            state.valkyrie_adapter_status = ComponentStatus::Healthy;
            state.last_health_check = Some(Instant::now());
        }
        
        // Start health monitoring
        self.start_health_monitoring().await?;
        
        // Setup configuration hot reload if enabled
        if config.hot_reload_enabled {
            self.setup_config_watcher().await?;
        }
        
        tracing::info!("Valkyrie integration initialized successfully");
        Ok(())
    }
    
    /// Shutdown Valkyrie integration
    pub async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down Valkyrie integration...");
        
        // Stop configuration watcher
        {
            let mut watcher = self.config_watcher.lock().await;
            *watcher = None;
        }
        
        // Shutdown components
        if let Some(_adapter) = &self.valkyrie_adapter {
            // Graceful shutdown of adapter
            tracing::debug!("Shutting down Valkyrie adapter");
        }
        
        if let Some(_engine) = &self.valkyrie_engine {
            // Graceful shutdown of engine
            tracing::debug!("Shutting down Valkyrie engine");
        }
        
        // Clear references
        self.valkyrie_adapter = None;
        self.valkyrie_engine = None;
        
        // Update state
        {
            let mut state = self.integration_state.write().await;
            state.initialized = false;
            state.valkyrie_engine_status = ComponentStatus::Disabled;
            state.valkyrie_adapter_status = ComponentStatus::Disabled;
        }
        
        tracing::info!("Valkyrie integration shutdown complete");
        Ok(())
    }
    
    /// Get Valkyrie adapter instance
    pub async fn get_valkyrie_adapter(&self) -> Result<Arc<ValkyrieRunnerAdapter>> {
        self.valkyrie_adapter.clone()
            .ok_or_else(|| AppError::InternalError {
                component: "valkyrie_integration".to_string(),
                message: "Valkyrie adapter not initialized".to_string(),
            })
    }
    
    /// Get Valkyrie engine instance
    pub async fn get_valkyrie_engine(&self) -> Result<Arc<ValkyrieEngine>> {
        self.valkyrie_engine.clone()
            .ok_or_else(|| AppError::InternalError {
                component: "valkyrie_integration".to_string(),
                message: "Valkyrie engine not initialized".to_string(),
            })
    }
    
    /// Validate configuration
    pub async fn validate_config(&self, config: &ValkyrieIntegrationConfig) -> Result<()> {
        self.config_validator.validate(config)
    }
    
    /// Reload configuration
    pub async fn reload_config(&self) -> Result<()> {
        tracing::info!("Reloading Valkyrie configuration...");
        
        // Load new configuration (placeholder - would load from file/source)
        let new_config = self.load_config_from_source().await?;
        
        // Validate new configuration
        self.config_validator.validate(&new_config)?;
        
        // Apply configuration changes
        self.apply_config_changes(new_config).await?;
        
        tracing::info!("Valkyrie configuration reloaded successfully");
        Ok(())
    }
    
    /// Get integration status
    pub async fn get_integration_status(&self) -> IntegrationState {
        self.integration_state.read().await.clone()
    }
    
    /// Check if Valkyrie is enabled and healthy
    pub async fn is_valkyrie_available(&self) -> bool {
        let state = self.integration_state.read().await;
        state.initialized && 
        state.valkyrie_engine_status == ComponentStatus::Healthy &&
        state.valkyrie_adapter_status == ComponentStatus::Healthy
    }
    
    /// Enable graceful degradation mode
    pub async fn enable_degradation_mode(&self, reason: String) -> Result<()> {
        tracing::warn!("Enabling degradation mode: {}", reason);
        
        let mut state = self.integration_state.write().await;
        state.valkyrie_engine_status = ComponentStatus::Degraded;
        state.valkyrie_adapter_status = ComponentStatus::Degraded;
        state.last_error = Some(reason);
        
        Ok(())
    }
    
    /// Attempt to recover from degradation
    pub async fn attempt_recovery(&self) -> Result<()> {
        tracing::info!("Attempting recovery from degradation...");
        
        // Perform health checks
        let engine_healthy = self.check_engine_health().await;
        let adapter_healthy = self.check_adapter_health().await;
        
        let mut state = self.integration_state.write().await;
        
        if engine_healthy && adapter_healthy {
            state.valkyrie_engine_status = ComponentStatus::Healthy;
            state.valkyrie_adapter_status = ComponentStatus::Healthy;
            state.error_count = 0;
            state.last_error = None;
            tracing::info!("Recovery successful");
        } else {
            state.error_count += 1;
            tracing::warn!("Recovery failed, error count: {}", state.error_count);
        }
        
        Ok(())
    }
    
    // Private helper methods
    
    async fn initialize_valkyrie_engine(
        &self,
        config: &ValkyrieIntegrationConfig,
    ) -> Result<Arc<ValkyrieEngine>> {
        tracing::debug!("Initializing Valkyrie engine");
    
        let engine_config = ValkyrieConfig {
            protocol: ProtocolConfig {
                version: "1.0".to_string(),
                supported_messages: vec![],
                extensions: vec![],
                compatibility_mode: false,
                timeouts: ProtocolTimeouts {
                    connect_timeout: config.connection_timeout,
                    send_timeout: Duration::from_secs(5),
                    heartbeat_interval: Duration::from_secs(30),
                    idle_timeout: Duration::from_secs(60),
                },
            },
            transport: TransportLayerConfig {
                primary: TransportConfig {
                    transport_type: TransportType::Tcp, // adjust if Udp/Quic is needed
                    bind_address: config.valkyrie_listen_address.clone(),
                    port: None, // optional; can parse from listen_address if you want
                    tls_config: None, // or Some(TlsConfig { .. })
                    authentication: crate::core::networking::transport::AuthenticationConfig {
                        method: crate::core::networking::transport::AuthMethod::JwtToken, // pick appropriate variant
                        jwt_secret: None, // or Some("your-secret".to_string()) if JWT is needed
                        token_expiry: Duration::from_secs(3600),
                        require_mutual_auth: false,
                    },                    
                    timeouts: TimeoutConfig {
                        connect_timeout: Duration::from_secs(10),
                        read_timeout: Duration::from_secs(30),
                        write_timeout: Duration::from_secs(30),
                        keepalive_interval: Duration::from_secs(15),
                    },
                    buffer_sizes: BufferConfig {
                        read_buffer_size: 8192,
                        write_buffer_size: 8192,
                        max_message_size: 65536,
                    },
                },
                fallbacks: vec![],
                selection_strategy: TransportSelectionStrategy::RoundRobin, // enum from engine
                connection_pooling: ConnectionPoolConfig {
                    max_connections_per_endpoint: 10,
                    idle_timeout: Duration::from_secs(30),
                    reuse_strategy: ConnectionReuseStrategy::Global, // from engine
                },
            },
            
            security: SecurityConfig {
                authentication: AuthenticationConfig {
                    methods: vec![AuthMethod::None], // import required
                    token_expiry: Duration::from_secs(3600),
                    require_mutual_auth: false,
                },
                encryption: EncryptionConfig {
                    cipher_suites: vec![CipherSuite::Aes256Gcm], // import required
                    key_rotation_interval: Duration::from_secs(86_400),
                    forward_secrecy: true,
                },
                authorization: AuthorizationConfig {
                    enable_rbac: false,
                    default_permissions: vec![],
                    cache_ttl: Duration::from_secs(300),
                },
                audit: AuditConfig {
                    enabled: false,
                    retention_period: Duration::from_secs(86_400 * 7),
                    events: vec![],
                },
            },
            streaming: StreamingConfig {
                max_streams_per_connection: 128,
                buffer_size: 64 * 1024,
                flow_control: FlowControlConfig {
                    initial_window_size: 64 * 1024,
                    max_window_size: 256 * 1024,
                    update_threshold: 0.5,
                },
                priority_scheduling: true,
            },
            routing: RoutingConfig {
                load_balancing: LoadBalancingStrategy::RoundRobin,
                routing_table_size: 1024,
                route_cache_ttl: Duration::from_secs(300),
            },
            performance: PerformanceConfig {
                worker_threads: Some(4),
                message_batch_size: 128,
                zero_copy: true,
                simd: true,
            },
            observability: ObservabilityConfig {
                metrics: MetricsConfig {
                    enabled: config.metrics_enabled,
                    export_interval: Duration::from_secs(15),
                    retention_period: Duration::from_secs(3600),
                },
                tracing: TracingConfig {
                    enabled: config.tracing_enabled,
                    sampling_rate: 1.0,
                    export_endpoint: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Json, // import required
                    structured: true,
                },
            },
            features: FeatureFlags {
                experimental: false,
                post_quantum_crypto: false,
                ml_optimizations: false,
                custom: HashMap::new(),
            },
        };
        
    
        let engine = ValkyrieEngine::new(engine_config).map_err(|e| AppError::InternalError {
            component: "valkyrie_integration".to_string(),
            message: format!("Failed to initialize Valkyrie engine: {}", e),
        })?;        
    
        Ok(Arc::new(engine))
    }
    
    async fn initialize_valkyrie_adapter(
        &self,
        config: &ValkyrieIntegrationConfig,
        valkyrie_engine: Arc<ValkyrieEngine>,
    ) -> Result<Arc<ValkyrieRunnerAdapter>> {
        tracing::debug!("Initializing Valkyrie adapter");
        
        // Create adapter configuration
        let adapter_config = ValkyrieAdapterConfig {
            max_concurrent_jobs: config.max_concurrent_jobs,
            dispatch_timeout: config.dispatch_timeout,
            queue_capacity: config.queue_capacity,
            enable_http_fallback: config.fallback_mode_enabled,
            fallback_timeout: config.fallback_timeout,
            enable_intelligent_routing: config.intelligent_routing_enabled,
            health_check_interval: config.health_check_interval,
            metrics_enabled: config.metrics_enabled,
            metrics_interval: config.metrics_interval,
        };
        
        // Initialize adapter
        let adapter = ValkyrieRunnerAdapter::new(valkyrie_engine, adapter_config).await
            .map_err(|e| AppError::InternalError {
                component: "valkyrie_integration".to_string(),
                message: format!("Failed to initialize Valkyrie adapter: {}", e),
            })?;
        
        Ok(Arc::new(adapter))
    }
    
    async fn start_health_monitoring(&self) -> Result<()> {
        let integration_state = self.integration_state.clone();
        let health_monitor = self.health_monitor.clone();
        let valkyrie_engine = self.valkyrie_engine.clone();
        let valkyrie_adapter = self.valkyrie_adapter.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(health_monitor.check_interval);
        
            loop {
                interval.tick().await;
        
                // Perform health checks
                let engine_healthy = if let Some(engine) = &valkyrie_engine {
                    Self::check_engine_health_static(engine).await
                } else {
                    false
                };
        
                let adapter_healthy = if let Some(adapter) = &valkyrie_adapter {
                    Self::check_adapter_health_static(adapter).await
                } else {
                    false
                };
        
                // Update state
                let mut state = integration_state.write().await;
                state.last_health_check = Some(Instant::now());
        
                if !engine_healthy || !adapter_healthy {
                    state.error_count += 1;
        
                    if state.error_count >= health_monitor.failure_threshold {
                        state.valkyrie_engine_status = ComponentStatus::Unhealthy;
                        state.valkyrie_adapter_status = ComponentStatus::Unhealthy;
                    } else {
                        state.valkyrie_engine_status = ComponentStatus::Degraded;
                        state.valkyrie_adapter_status = ComponentStatus::Degraded;
                    }
                } else {
                    if state.error_count > 0 {
                        state.error_count = state.error_count.saturating_sub(1);
                    }
        
                    if state.error_count == 0 {
                        state.valkyrie_engine_status = ComponentStatus::Healthy;
                        state.valkyrie_adapter_status = ComponentStatus::Healthy;
                    }
                }
            }
        });        
        
        Ok(())
    }
    
    async fn setup_config_watcher(&self) -> Result<()> {
        // Placeholder for configuration file watching
        // In a real implementation, this would watch the configuration file
        // and trigger reloads when changes are detected
        tracing::debug!("Configuration hot reload watcher setup (placeholder)");
        Ok(())
    }
    
    async fn load_config_from_source(&self) -> Result<ValkyrieIntegrationConfig> {
        // Placeholder for loading configuration from external source
        // In a real implementation, this would load from file, environment, etc.
        let config = self.config.read().await;
        Ok(config.clone())
    }
    
    async fn apply_config_changes(&self, new_config: ValkyrieIntegrationConfig) -> Result<()> {
        let mut config = self.config.write().await;
        let old_config = config.clone();
        
        // Check if restart is required
        let restart_required = self.is_restart_required(&old_config, &new_config);
        
        if restart_required {
            tracing::info!("Configuration changes require restart");
            // Would trigger component restart here
        } else {
            tracing::info!("Applying configuration changes without restart");
            // Apply hot-reloadable changes
        }
        
        *config = new_config;
        Ok(())
    }
    
    fn is_restart_required(&self, old_config: &ValkyrieIntegrationConfig, new_config: &ValkyrieIntegrationConfig) -> bool {
        // Check if critical configuration changes require restart
        old_config.valkyrie_enabled != new_config.valkyrie_enabled ||
        old_config.valkyrie_listen_address != new_config.valkyrie_listen_address ||
        old_config.max_connections != new_config.max_connections
    }
    
    async fn check_engine_health(&self) -> bool {
        if let Some(engine) = &self.valkyrie_engine {
            Self::check_engine_health_static(engine).await
        } else {
            false
        }
    }
    
    async fn check_adapter_health(&self) -> bool {
        if let Some(adapter) = &self.valkyrie_adapter {
            Self::check_adapter_health_static(adapter).await
        } else {
            false
        }
    }
    
    async fn check_engine_health_static(_engine: &Arc<ValkyrieEngine>) -> bool {
        // Placeholder for engine health check
        // In a real implementation, this would check engine status
        true
    }
    
    async fn check_adapter_health_static(adapter: &Arc<ValkyrieRunnerAdapter>) -> bool {
        // Check adapter health by getting performance metrics
        adapter.get_performance_metrics().await.is_ok()
    }
}

impl ConfigValidator {
    pub fn new() -> Self {
        let mut validation_rules = Vec::new();
        
        // Add validation rules
        validation_rules.push(ValidationRule {
            name: "valkyrie_enabled_check".to_string(),
            validator: Box::new(|config| {
                if config.valkyrie_enabled && config.valkyrie_listen_address.is_empty() {
                    return Err(AppError::ValidationError(
                        "Valkyrie listen address must be specified when Valkyrie is enabled".to_string()
                    ));
                }
                Ok(())
            }),
        });
        
        validation_rules.push(ValidationRule {
            name: "connection_limits_check".to_string(),
            validator: Box::new(|config| {
                if config.max_connections == 0 {
                    return Err(AppError::ValidationError(
                        "Max connections must be greater than 0".to_string()
                    ));
                }
                if config.max_concurrent_jobs == 0 {
                    return Err(AppError::ValidationError(
                        "Max concurrent jobs must be greater than 0".to_string()
                    ));
                }
                Ok(())
            }),
        });
        
        validation_rules.push(ValidationRule {
            name: "timeout_check".to_string(),
            validator: Box::new(|config| {
                if config.dispatch_timeout.as_millis() == 0 {
                    return Err(AppError::ValidationError(
                        "Dispatch timeout must be greater than 0".to_string()
                    ));
                }
                if config.connection_timeout.as_millis() == 0 {
                    return Err(AppError::ValidationError(
                        "Connection timeout must be greater than 0".to_string()
                    ));
                }
                Ok(())
            }),
        });
        
        Self { validation_rules }
    }
    
    pub fn validate(&self, config: &ValkyrieIntegrationConfig) -> Result<()> {
        for rule in &self.validation_rules {
            (rule.validator)(config).map_err(|e| {
                AppError::ValidationError(format!("Validation rule '{}' failed: {}", rule.name, e))
            })?;
        }
        Ok(())
    }
}

impl Default for IntegrationState {
    fn default() -> Self {
        Self {
            initialized: false,
            valkyrie_engine_status: ComponentStatus::Disabled,
            valkyrie_adapter_status: ComponentStatus::Disabled,
            last_health_check: None,
            error_count: 0,
            last_error: None,
        }
    }
}