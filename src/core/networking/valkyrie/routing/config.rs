// Configuration Management for Routing System
// Task 3.1.6: Configuration and Policy Management

use super::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// Configuration manager for routing system
pub struct ConfigurationManager {
    policy_engine: Arc<PolicyEngine>,
    rule_validator: Arc<RuleValidator>,
    hot_reload_manager: Arc<HotReloadManager>,
    version_manager: Arc<VersionManager>,
    config_store: Arc<ConfigStore>,
}

/// Policy engine for rule-based routing
pub struct PolicyEngine {
    policies: Arc<RwLock<HashMap<PolicyId, RoutingPolicy>>>,
    rule_evaluator: Arc<RuleEvaluator>,
    conflict_resolver: Arc<ConflictResolver>,
    policy_cache: Arc<RwLock<HashMap<String, CachedPolicyResult>>>,
}

/// Routing policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPolicy {
    pub id: PolicyId,
    pub name: String,
    pub description: String,
    pub rules: Vec<RoutingRule>,
    pub priority: u8,
    pub enabled: bool,
    pub conditions: Vec<PolicyCondition>,
    pub actions: Vec<PolicyAction>,
    pub metadata: PolicyMetadata,
}

/// Policy identifier
pub type PolicyId = String;

/// Individual routing rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: RuleId,
    pub name: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub weight: f64,
    pub enabled: bool,
    pub metadata: RuleMetadata,
}

/// Rule identifier
pub type RuleId = String;

/// Condition for rule evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleCondition {
    // Source/Destination conditions
    SourceNode(NodeId),
    DestinationNode(NodeId),
    SourceRegion(RegionId),
    DestinationRegion(RegionId),
    
    // QoS conditions
    PriorityEquals(MessagePriority),
    PriorityAbove(MessagePriority),
    LatencyRequirement(Duration),
    BandwidthRequirement(u64),
    ReliabilityRequirement(f64),
    
    // Network conditions
    NodeLoad(NodeId, f64),
    LinkUtilization(LinkId, f64),
    NetworkCongestion(CongestionState),
    
    // Time conditions
    TimeOfDay(u8, u8), // hour, minute
    DayOfWeek(u8),     // 0 = Sunday
    DateRange(String, String), // ISO date strings
    
    // Security conditions
    SecurityLevel(SecurityLevel),
    UserRole(String),
    TenantId(String),
    
    // Logical conditions
    And(Vec<RuleCondition>),
    Or(Vec<RuleCondition>),
    Not(Box<RuleCondition>),
    
    // Custom conditions
    Custom(String), // Expression string
}

/// Action to take when rule matches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleAction {
    // Routing actions
    UseAlgorithm(RoutingAlgorithm),
    PreferNodes(Vec<NodeId>),
    AvoidNodes(Vec<NodeId>),
    PreferRegions(Vec<RegionId>),
    AvoidRegions(Vec<RegionId>),
    
    // QoS actions
    SetPriority(MessagePriority),
    ReserveBandwidth(u64),
    SetLatencyTarget(Duration),
    
    // Traffic shaping actions
    ApplyRateLimit(u64),
    ApplyDelay(Duration),
    ApplyBackpressure,
    
    // Security actions
    RequireEncryption,
    RequireAuthentication,
    ApplySecurityPolicy(String),
    
    // Monitoring actions
    EnableTracing,
    LogEvent(String),
    TriggerAlert(String),
    
    // Custom actions
    Custom(String), // Action expression
}

/// Policy condition for activation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyCondition {
    Always,
    TimeWindow(String, String), // start, end times
    LoadThreshold(f64),
    ErrorRateThreshold(f64),
    Custom(String),
}

/// Policy action when activated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    EnableRules(Vec<RuleId>),
    DisableRules(Vec<RuleId>),
    ModifyWeights(HashMap<RuleId, f64>),
    TriggerRebalancing,
    SendNotification(String),
    Custom(String),
}

/// Policy metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyMetadata {
    pub created_by: String,
    pub created_at: String,
    pub last_modified_by: String,
    pub last_modified_at: String,
    pub version: String,
    pub tags: Vec<String>,
    pub documentation: String,
}

/// Rule metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleMetadata {
    pub created_by: String,
    pub created_at: String,
    pub last_used: Option<String>,
    pub usage_count: u64,
    pub success_rate: f64,
    pub tags: Vec<String>,
}

/// Cached policy evaluation result
#[derive(Debug, Clone)]
pub struct CachedPolicyResult {
    pub result: PolicyEvaluationResult,
    pub created_at: Instant,
    pub ttl: Duration,
    pub hit_count: u32,
}

/// Result of policy evaluation
#[derive(Debug, Clone)]
pub struct PolicyEvaluationResult {
    pub applicable_policies: Vec<PolicyId>,
    pub applicable_rules: Vec<RuleId>,
    pub routing_hints: RoutingHints,
    pub constraints: Vec<RoutingConstraint>,
    pub confidence: f64,
}

/// Routing constraint from policies
#[derive(Debug, Clone)]
pub enum RoutingConstraint {
    MustUseNodes(Vec<NodeId>),
    MustAvoidNodes(Vec<NodeId>),
    MustUseRegions(Vec<RegionId>),
    MustAvoidRegions(Vec<RegionId>),
    MaxLatency(Duration),
    MinBandwidth(u64),
    MinReliability(f64),
    RequiredSecurity(SecurityLevel),
    Custom(String, String), // name, value
}

/// Rule evaluation engine
pub struct RuleEvaluator {
    expression_engine: Arc<ExpressionEngine>,
    context_provider: Arc<ContextProvider>,
    evaluation_cache: Arc<RwLock<HashMap<String, CachedEvaluation>>>,
}

/// Cached rule evaluation
#[derive(Debug, Clone)]
pub struct CachedEvaluation {
    pub result: bool,
    pub created_at: Instant,
    pub ttl: Duration,
    pub context_hash: u64,
}

/// Expression engine for custom conditions/actions
pub struct ExpressionEngine {
    functions: HashMap<String, Box<dyn ExpressionFunction>>,
    variables: Arc<RwLock<HashMap<String, ExpressionValue>>>,
}

/// Expression function trait
pub trait ExpressionFunction: Send + Sync {
    fn call(&self, args: &[ExpressionValue]) -> Result<ExpressionValue, ConfigError>;
    fn get_name(&self) -> &str;
    fn get_signature(&self) -> &str;
}

/// Expression value types
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Duration(Duration),
    NodeId(NodeId),
    Array(Vec<ExpressionValue>),
    Object(HashMap<String, ExpressionValue>),
}

/// Context provider for rule evaluation
pub struct ContextProvider {
    routing_context: Arc<RwLock<Option<RoutingContext>>>,
    topology_context: Arc<RwLock<Option<NetworkTopology>>>,
    system_context: Arc<RwLock<SystemContext>>,
}

/// System context for rule evaluation
#[derive(Debug, Clone)]
pub struct SystemContext {
    pub current_time: Instant,
    pub system_load: f64,
    pub error_rate: f64,
    pub active_connections: u64,
    pub memory_usage: f64,
    pub cpu_usage: f64,
    pub custom_metrics: HashMap<String, f64>,
}

impl Default for SystemContext {
    fn default() -> Self {
        Self {
            current_time: Instant::now(),
            system_load: 0.0,
            error_rate: 0.0,
            active_connections: 0,
            memory_usage: 0.0,
            cpu_usage: 0.0,
            custom_metrics: HashMap::new(),
        }
    }
}

/// Conflict resolution for overlapping policies
pub struct ConflictResolver {
    resolution_strategies: HashMap<ConflictType, Box<dyn ConflictResolutionStrategy>>,
    conflict_detector: Arc<ConflictDetector>,
}

/// Types of policy conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictType {
    PriorityConflict,
    ActionConflict,
    ConstraintConflict,
    ResourceConflict,
}

/// Conflict resolution strategy
pub trait ConflictResolutionStrategy: Send + Sync {
    fn resolve_conflict(
        &self,
        conflicting_policies: &[&RoutingPolicy],
        context: &RoutingContext,
    ) -> Result<ResolvedPolicy, ConfigError>;
}

/// Resolved policy after conflict resolution
#[derive(Debug, Clone)]
pub struct ResolvedPolicy {
    pub merged_rules: Vec<RoutingRule>,
    pub final_actions: Vec<RuleAction>,
    pub resolution_method: String,
    pub confidence: f64,
}

/// Conflict detector
pub struct ConflictDetector {
    conflict_patterns: Vec<ConflictPattern>,
}

/// Pattern for detecting conflicts
pub struct ConflictPattern {
    pub name: String,
    pub description: String,
    pub detector: Box<dyn Fn(&[&RoutingPolicy]) -> Vec<ConflictType> + Send + Sync>,
}

impl std::fmt::Debug for ConflictPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConflictPattern")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("detector", &"<function>")
            .finish()
    }
}

impl Clone for ConflictPattern {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            detector: Box::new(|_| Vec::new()),
        }
    }
}

/// Hot reload manager for configuration updates
pub struct HotReloadManager {
    watchers: HashMap<String, Box<dyn ConfigWatcher>>,
    reload_handlers: Vec<Arc<dyn ReloadHandler>>,
    reload_history: Arc<RwLock<Vec<ReloadEvent>>>,
}

/// Configuration watcher trait
#[async_trait::async_trait]
pub trait ConfigWatcher: Send + Sync {
    async fn watch(&self) -> Result<ConfigChange, ConfigError>;
    fn get_watch_path(&self) -> &str;
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub change_type: ChangeType,
    pub path: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub timestamp: Instant,
}

/// Types of configuration changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// Reload handler trait
#[async_trait::async_trait]
pub trait ReloadHandler: Send + Sync {
    async fn handle_reload(&self, change: &ConfigChange) -> Result<(), ConfigError>;
    fn get_handler_name(&self) -> &str;
}

/// Reload event
#[derive(Debug, Clone)]
pub struct ReloadEvent {
    pub change: ConfigChange,
    pub handler_results: HashMap<String, Result<(), String>>,
    pub total_duration: Duration,
    pub timestamp: Instant,
}

/// Version manager for configuration versioning
pub struct VersionManager {
    versions: Arc<RwLock<HashMap<String, ConfigVersion>>>,
    current_version: Arc<RwLock<String>>,
    rollback_manager: Arc<RollbackManager>,
}

/// Configuration version
#[derive(Debug, Clone)]
pub struct ConfigVersion {
    pub version_id: String,
    pub config_snapshot: ConfigSnapshot,
    pub created_at: Instant,
    pub created_by: String,
    pub description: String,
    pub checksum: String,
}

/// Configuration snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub policies: HashMap<PolicyId, RoutingPolicy>,
    pub global_settings: GlobalSettings,
    pub algorithm_configs: HashMap<RoutingAlgorithm, AlgorithmConfig>,
    pub metadata: SnapshotMetadata,
}

/// Global routing settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub default_algorithm: RoutingAlgorithm,
    pub cache_enabled: bool,
    pub cache_ttl: Duration,
    pub max_route_hops: u32,
    pub timeout_settings: TimeoutSettings,
    pub retry_settings: RetrySettings,
    pub observability_settings: ObservabilitySettings,
}

/// Timeout settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutSettings {
    pub route_calculation_timeout: Duration,
    pub topology_discovery_timeout: Duration,
    pub health_check_timeout: Duration,
    pub policy_evaluation_timeout: Duration,
}

/// Retry settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrySettings {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter_enabled: bool,
}

/// Observability settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilitySettings {
    pub metrics_enabled: bool,
    pub tracing_enabled: bool,
    pub logging_level: String,
    pub export_interval: Duration,
    pub retention_period: Duration,
}

/// Algorithm-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmConfig {
    pub enabled: bool,
    pub priority: u8,
    pub parameters: HashMap<String, ConfigValue>,
    pub resource_limits: ResourceLimits,
}

/// Configuration value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Duration(Duration),
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
}

/// Resource limits for algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f64,
    pub max_execution_time: Duration,
    pub max_concurrent_operations: u32,
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub version: String,
    pub created_at: String,
    pub created_by: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// Rollback manager
pub struct RollbackManager {
    rollback_history: Arc<RwLock<Vec<RollbackOperation>>>,
    max_rollback_versions: usize,
}

/// Rollback operation
#[derive(Debug, Clone)]
pub struct RollbackOperation {
    pub operation_id: String,
    pub from_version: String,
    pub to_version: String,
    pub reason: String,
    pub performed_by: String,
    pub performed_at: Instant,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Configuration store
pub struct ConfigStore {
    storage_backend: Box<dyn StorageBackend>,
    encryption_key: Option<Vec<u8>>,
    compression_enabled: bool,
}

/// Storage backend trait
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn load_config(&self, key: &str) -> Result<String, ConfigError>;
    async fn save_config(&self, key: &str, value: &str) -> Result<(), ConfigError>;
    async fn delete_config(&self, key: &str) -> Result<(), ConfigError>;
    async fn list_configs(&self) -> Result<Vec<String>, ConfigError>;
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Policy not found: {policy_id}")]
    PolicyNotFound { policy_id: PolicyId },
    
    #[error("Rule not found: {rule_id}")]
    RuleNotFound { rule_id: RuleId },
    
    #[error("Policy validation failed: {policy_id} - {error}")]
    PolicyValidationFailed { policy_id: PolicyId, error: String },
    
    #[error("Rule validation failed: {rule_id} - {error}")]
    RuleValidationFailed { rule_id: RuleId, error: String },
    
    #[error("Expression evaluation failed: {expression} - {error}")]
    ExpressionEvaluationFailed { expression: String, error: String },
    
    #[error("Conflict resolution failed: {conflict_type:?} - {error}")]
    ConflictResolutionFailed { conflict_type: ConflictType, error: String },
    
    #[error("Configuration reload failed: {path} - {error}")]
    ReloadFailed { path: String, error: String },
    
    #[error("Version not found: {version}")]
    VersionNotFound { version: String },
    
    #[error("Rollback failed: {from_version} -> {to_version} - {error}")]
    RollbackFailed { from_version: String, to_version: String, error: String },
    
    #[error("Storage error: {error}")]
    StorageError { error: String },
    
    #[error("Serialization error: {error}")]
    SerializationError { error: String },
    
    #[error("Validation error: {field} - {error}")]
    ValidationError { field: String, error: String },
}

impl ConfigurationManager {
    pub fn new(storage_backend: Box<dyn StorageBackend>) -> Self {
        Self {
            policy_engine: Arc::new(PolicyEngine::new()),
            rule_validator: Arc::new(RuleValidator::new()),
            hot_reload_manager: Arc::new(HotReloadManager::new()),
            version_manager: Arc::new(VersionManager::new()),
            config_store: Arc::new(ConfigStore::new(storage_backend)),
        }
    }

    /// Load configuration from storage
    pub async fn load_config(&self) -> Result<ConfigSnapshot, ConfigError> {
        self.config_store.load_snapshot().await
    }

    /// Save configuration to storage
    pub async fn save_config(&self, snapshot: &ConfigSnapshot) -> Result<String, ConfigError> {
        let version_id = self.version_manager.create_version(snapshot).await?;
        self.config_store.save_snapshot(snapshot).await?;
        Ok(version_id)
    }

    /// Add or update a routing policy
    pub async fn add_policy(&self, policy: RoutingPolicy) -> Result<(), ConfigError> {
        // Validate policy
        self.rule_validator.validate_policy(&policy).await?;
        
        // Check for conflicts
        let conflicts = self.policy_engine.detect_conflicts(&policy).await?;
        if !conflicts.is_empty() {
            return Err(ConfigError::ConflictResolutionFailed {
                conflict_type: ConflictType::PriorityConflict,
                error: format!("Policy conflicts detected: {:?}", conflicts),
            });
        }
        
        // Add policy
        self.policy_engine.add_policy(policy).await?;
        
        Ok(())
    }

    /// Remove a routing policy
    pub async fn remove_policy(&self, policy_id: &PolicyId) -> Result<(), ConfigError> {
        self.policy_engine.remove_policy(policy_id).await
    }

    /// Evaluate policies for a routing context
    pub async fn evaluate_policies(
        &self,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<PolicyEvaluationResult, ConfigError> {
        self.policy_engine.evaluate_policies(context, topology).await
    }

    /// Start hot reload monitoring
    pub async fn start_hot_reload(&self) -> Result<(), ConfigError> {
        self.hot_reload_manager.start_watching().await
    }

    /// Rollback to a previous configuration version
    pub async fn rollback_to_version(&self, version_id: &str) -> Result<(), ConfigError> {
        self.version_manager.rollback_to_version(version_id).await
    }

    /// Get configuration history
    pub async fn get_version_history(&self) -> Result<Vec<ConfigVersion>, ConfigError> {
        self.version_manager.get_version_history().await
    }
}

// Implementation stubs for complex components
impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            rule_evaluator: Arc::new(RuleEvaluator::new()),
            conflict_resolver: Arc::new(ConflictResolver::new()),
            policy_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_policy(&self, policy: RoutingPolicy) -> Result<(), ConfigError> {
        let mut policies = self.policies.write().await;
        policies.insert(policy.id.clone(), policy);
        Ok(())
    }

    pub async fn remove_policy(&self, policy_id: &PolicyId) -> Result<(), ConfigError> {
        let mut policies = self.policies.write().await;
        policies.remove(policy_id)
            .ok_or_else(|| ConfigError::PolicyNotFound { policy_id: policy_id.clone() })?;
        Ok(())
    }

    pub async fn evaluate_policies(
        &self,
        context: &RoutingContext,
        topology: &NetworkTopology,
    ) -> Result<PolicyEvaluationResult, ConfigError> {
        // Simplified evaluation
        Ok(PolicyEvaluationResult {
            applicable_policies: Vec::new(),
            applicable_rules: Vec::new(),
            routing_hints: RoutingHints::default(),
            constraints: Vec::new(),
            confidence: 1.0,
        })
    }

    pub async fn detect_conflicts(&self, policy: &RoutingPolicy) -> Result<Vec<ConflictType>, ConfigError> {
        Ok(Vec::new())
    }
}

impl RuleValidator {
    pub fn new() -> Self {
        Self
    }

    pub async fn validate_policy(&self, policy: &RoutingPolicy) -> Result<(), ConfigError> {
        // Validate policy structure and rules
        for rule in &policy.rules {
            self.validate_rule(rule).await?;
        }
        Ok(())
    }

    pub async fn validate_rule(&self, rule: &RoutingRule) -> Result<(), ConfigError> {
        // Validate rule structure
        Ok(())
    }
}

impl RuleEvaluator {
    pub fn new() -> Self {
        Self {
            expression_engine: Arc::new(ExpressionEngine::new()),
            context_provider: Arc::new(ContextProvider::new()),
            evaluation_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ExpressionEngine {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            variables: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ContextProvider {
    pub fn new() -> Self {
        Self {
            routing_context: Arc::new(RwLock::new(None)),
            topology_context: Arc::new(RwLock::new(None)),
            system_context: Arc::new(RwLock::new(SystemContext::default())),
        }
    }
}

impl ConflictResolver {
    pub fn new() -> Self {
        Self {
            resolution_strategies: HashMap::new(),
            conflict_detector: Arc::new(ConflictDetector::new()),
        }
    }
}

impl ConflictDetector {
    pub fn new() -> Self {
        Self {
            conflict_patterns: Vec::new(),
        }
    }
}

impl HotReloadManager {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
            reload_handlers: Vec::new(),
            reload_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start_watching(&self) -> Result<(), ConfigError> {
        // Start configuration watching
        Ok(())
    }
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            versions: Arc::new(RwLock::new(HashMap::new())),
            current_version: Arc::new(RwLock::new("1.0.0".to_string())),
            rollback_manager: Arc::new(RollbackManager::new()),
        }
    }

    pub async fn create_version(&self, snapshot: &ConfigSnapshot) -> Result<String, ConfigError> {
        let version_id = format!("v{}", chrono::Utc::now().timestamp());
        Ok(version_id)
    }

    pub async fn rollback_to_version(&self, version_id: &str) -> Result<(), ConfigError> {
        Ok(())
    }

    pub async fn get_version_history(&self) -> Result<Vec<ConfigVersion>, ConfigError> {
        Ok(Vec::new())
    }
}

impl RollbackManager {
    pub fn new() -> Self {
        Self {
            rollback_history: Arc::new(RwLock::new(Vec::new())),
            max_rollback_versions: 10,
        }
    }
}

impl ConfigStore {
    pub fn new(storage_backend: Box<dyn StorageBackend>) -> Self {
        Self {
            storage_backend,
            encryption_key: None,
            compression_enabled: false,
        }
    }

    pub async fn load_snapshot(&self) -> Result<ConfigSnapshot, ConfigError> {
        let config_data = self.storage_backend.load_config("current").await?;
        serde_json::from_str(&config_data)
            .map_err(|e| ConfigError::SerializationError { error: e.to_string() })
    }

    pub async fn save_snapshot(&self, snapshot: &ConfigSnapshot) -> Result<(), ConfigError> {
        let config_data = serde_json::to_string(snapshot)
            .map_err(|e| ConfigError::SerializationError { error: e.to_string() })?;
        self.storage_backend.save_config("current", &config_data).await
    }
}

// Default implementations
impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            default_algorithm: RoutingAlgorithm::LoadAware,
            cache_enabled: true,
            cache_ttl: Duration::from_secs(300),
            max_route_hops: 10,
            timeout_settings: TimeoutSettings::default(),
            retry_settings: RetrySettings::default(),
            observability_settings: ObservabilitySettings::default(),
        }
    }
}

impl Default for TimeoutSettings {
    fn default() -> Self {
        Self {
            route_calculation_timeout: Duration::from_millis(100),
            topology_discovery_timeout: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            policy_evaluation_timeout: Duration::from_millis(50),
        }
    }
}

impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_enabled: true,
        }
    }
}

impl Default for ObservabilitySettings {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            tracing_enabled: true,
            logging_level: "info".to_string(),
            export_interval: Duration::from_secs(60),
            retention_period: Duration::from_secs(86400 * 7), // 7 days
        }
    }
}

pub struct RuleValidator;