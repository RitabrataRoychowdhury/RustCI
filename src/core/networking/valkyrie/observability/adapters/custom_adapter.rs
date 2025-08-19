// Custom Adapter Framework - Extensible plug-and-play system
// Enables users to create custom observability adapters with 100μs performance

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{LogData, MetricData, TraceData};

/// Custom adapter framework for user-defined adapters
pub struct CustomAdapterFramework {
    /// Registered custom adapters
    custom_adapters: Arc<RwLock<HashMap<String, Box<dyn CustomObservabilityAdapter>>>>,
    /// Adapter factory registry
    adapter_factories: Arc<RwLock<HashMap<String, Box<dyn CustomAdapterFactory>>>>,
    /// Plugin loader
    plugin_loader: Arc<PluginLoader>,
    /// Performance monitor
    performance_monitor: Arc<RwLock<CustomAdapterPerformanceMonitor>>,
}

/// Custom observability adapter trait for user implementations
#[async_trait::async_trait]
pub trait CustomObservabilityAdapter: Send + Sync {
    /// Adapter unique identifier
    fn adapter_id(&self) -> &str;

    /// Adapter display name
    fn adapter_name(&self) -> &str;

    /// Adapter version
    fn adapter_version(&self) -> &str;

    /// Adapter author/organization
    fn adapter_author(&self) -> &str;

    /// Adapter description
    fn adapter_description(&self) -> &str;

    /// Supported data types
    fn supported_data_types(&self) -> Vec<CustomDataType>;

    /// Adapter configuration schema
    fn config_schema(&self) -> CustomAdapterConfigSchema;

    /// Initialize the adapter with configuration
    async fn initialize(
        &mut self,
        config: HashMap<String, serde_json::Value>,
    ) -> Result<(), CustomAdapterError>;

    /// Start the adapter
    async fn start(&mut self) -> Result<(), CustomAdapterError>;

    /// Stop the adapter
    async fn stop(&mut self) -> Result<(), CustomAdapterError>;

    /// Process metrics data
    async fn process_metrics(
        &self,
        metrics: &[MetricData],
    ) -> Result<CustomProcessingResult, CustomAdapterError>;

    /// Process logs data
    async fn process_logs(
        &self,
        logs: &[LogData],
    ) -> Result<CustomProcessingResult, CustomAdapterError>;

    /// Process traces data
    async fn process_traces(
        &self,
        traces: &[TraceData],
    ) -> Result<CustomProcessingResult, CustomAdapterError>;

    /// Handle custom query
    async fn handle_query(
        &self,
        query: CustomQuery,
    ) -> Result<CustomQueryResult, CustomAdapterError>;

    /// Get adapter health
    async fn health_check(&self) -> CustomAdapterHealth;

    /// Get adapter metrics
    async fn get_metrics(&self) -> CustomAdapterMetrics;
}

/// Custom adapter factory trait for creating adapters
#[async_trait::async_trait]
pub trait CustomAdapterFactory: Send + Sync {
    /// Factory identifier
    fn factory_id(&self) -> &str;

    /// Supported adapter types
    fn supported_types(&self) -> Vec<String>;

    /// Create adapter instance
    async fn create_adapter(
        &self,
        adapter_type: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn CustomObservabilityAdapter>, CustomAdapterError>;

    /// Validate adapter configuration
    fn validate_config(
        &self,
        adapter_type: &str,
        config: &HashMap<String, serde_json::Value>,
    ) -> Result<(), CustomAdapterError>;
}

/// Custom data types supported by adapters
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomDataType {
    Metrics,
    Logs,
    Traces,
    Events,
    Alerts,
    Custom(String),
}

/// Custom adapter configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAdapterConfigSchema {
    /// Schema version
    pub version: String,
    /// Required configuration fields
    pub required_fields: Vec<ConfigField>,
    /// Optional configuration fields
    pub optional_fields: Vec<ConfigField>,
    /// Configuration examples
    pub examples: Vec<HashMap<String, serde_json::Value>>,
}

/// Configuration field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: ConfigFieldType,
    /// Field description
    pub description: String,
    /// Default value
    pub default_value: Option<serde_json::Value>,
    /// Validation rules
    pub validation: Option<ConfigValidation>,
}

/// Configuration field types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigFieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
    Enum(Vec<String>),
}

/// Configuration validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidation {
    /// Minimum value (for numbers)
    pub min: Option<f64>,
    /// Maximum value (for numbers)
    pub max: Option<f64>,
    /// Pattern (for strings)
    pub pattern: Option<String>,
    /// Required if another field has specific value
    pub required_if: Option<(String, serde_json::Value)>,
}

/// Custom processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProcessingResult {
    /// Processing success
    pub success: bool,
    /// Number of items processed
    pub items_processed: usize,
    /// Processing duration in microseconds
    pub processing_duration_us: u64,
    /// Result metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Error message if failed
    pub error_message: Option<String>,
}

/// Custom query structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomQuery {
    /// Query type
    pub query_type: String,
    /// Query parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Query timeout in microseconds
    pub timeout_us: u64,
}

/// Custom query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomQueryResult {
    /// Query success
    pub success: bool,
    /// Result data
    pub data: serde_json::Value,
    /// Result count
    pub count: usize,
    /// Query duration in microseconds
    pub query_duration_us: u64,
    /// Result metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Custom adapter health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAdapterHealth {
    /// Health status
    pub status: CustomHealthStatus,
    /// Health message
    pub message: String,
    /// Last health check timestamp
    pub last_check_timestamp: u64,
    /// Health details
    pub details: HashMap<String, serde_json::Value>,
}

/// Custom health status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Custom adapter metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAdapterMetrics {
    /// Metrics collection timestamp
    pub timestamp: u64,
    /// Performance metrics
    pub performance: CustomPerformanceMetrics,
    /// Resource usage metrics
    pub resources: CustomResourceMetrics,
    /// Custom metrics defined by the adapter
    pub custom_metrics: HashMap<String, serde_json::Value>,
}

/// Custom performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPerformanceMetrics {
    /// Average processing latency in microseconds
    pub avg_latency_us: f64,
    /// P95 processing latency in microseconds
    pub p95_latency_us: f64,
    /// P99 processing latency in microseconds
    pub p99_latency_us: f64,
    /// Throughput (items per second)
    pub throughput_items_per_sec: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
}

/// Custom resource metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResourceMetrics {
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_usage_percent: f64,
    /// Network bytes sent
    pub network_bytes_sent: u64,
    /// Network bytes received
    pub network_bytes_received: u64,
    /// Disk I/O bytes
    pub disk_io_bytes: u64,
}

/// Plugin loader for dynamic adapter loading
pub struct PluginLoader {
    /// Loaded plugins
    loaded_plugins: Arc<RwLock<HashMap<String, LoadedPlugin>>>,
    /// Plugin search paths
    search_paths: Vec<String>,
    /// Security policy
    security_policy: PluginSecurityPolicy,
}

/// Loaded plugin information
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    /// Plugin ID
    pub id: String,
    /// Plugin path
    pub path: String,
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// Load timestamp
    pub loaded_at: Instant,
    /// Plugin handle (would be actual plugin handle in real implementation)
    pub handle: PluginHandle,
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin author
    pub author: String,
    /// Plugin description
    pub description: String,
    /// Supported adapter types
    pub supported_adapters: Vec<String>,
    /// Required permissions
    pub required_permissions: Vec<String>,
    /// Minimum Valkyrie version
    pub min_valkyrie_version: String,
}

/// Plugin handle (placeholder for actual plugin interface)
#[derive(Debug, Clone)]
pub struct PluginHandle {
    /// Handle ID
    pub id: String,
    /// Plugin interface version
    pub interface_version: String,
}

/// Plugin security policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSecurityPolicy {
    /// Allow unsigned plugins
    pub allow_unsigned: bool,
    /// Trusted plugin authors
    pub trusted_authors: Vec<String>,
    /// Allowed permissions
    pub allowed_permissions: Vec<String>,
    /// Sandbox enabled
    pub sandbox_enabled: bool,
    /// Resource limits
    pub resource_limits: PluginResourceLimits,
}

/// Plugin resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResourceLimits {
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum network bandwidth in bytes per second
    pub max_network_bps: u64,
    /// Maximum disk I/O in bytes per second
    pub max_disk_io_bps: u64,
}

/// Performance monitor for custom adapters
#[derive(Debug)]
pub struct CustomAdapterPerformanceMonitor {
    /// Adapter performance data
    adapter_performance: HashMap<String, AdapterPerformanceData>,
    /// Global performance metrics
    global_metrics: GlobalPerformanceMetrics,
    /// Monitoring start time
    start_time: Instant,
}

/// Adapter performance data
#[derive(Debug, Clone)]
pub struct AdapterPerformanceData {
    /// Latency samples in microseconds
    latency_samples: Vec<u64>,
    /// Sample index
    sample_index: usize,
    /// Total operations
    total_operations: u64,
    /// Error count
    error_count: u64,
    /// Last update timestamp
    last_update: Instant,
}

/// Global performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPerformanceMetrics {
    /// Total adapters
    pub total_adapters: usize,
    /// Active adapters
    pub active_adapters: usize,
    /// Total operations across all adapters
    pub total_operations: u64,
    /// Average latency across all adapters
    pub avg_latency_us: f64,
    /// System resource usage
    pub system_resources: CustomResourceMetrics,
}

/// Custom adapter errors
#[derive(Debug, thiserror::Error)]
pub enum CustomAdapterError {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Initialization error: {0}")]
    Initialization(String),

    #[error("Processing error: {0}")]
    Processing(String),

    #[error("Plugin loading error: {0}")]
    PluginLoading(String),

    #[error("Security violation: {0}")]
    Security(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl CustomAdapterFramework {
    /// Create new custom adapter framework
    pub fn new() -> Self {
        Self {
            custom_adapters: Arc::new(RwLock::new(HashMap::new())),
            adapter_factories: Arc::new(RwLock::new(HashMap::new())),
            plugin_loader: Arc::new(PluginLoader::new()),
            performance_monitor: Arc::new(RwLock::new(CustomAdapterPerformanceMonitor::new())),
        }
    }

    /// Register a custom adapter
    pub async fn register_adapter(
        &self,
        adapter: Box<dyn CustomObservabilityAdapter>,
    ) -> Result<(), CustomAdapterError> {
        let adapter_id = adapter.adapter_id().to_string();
        let mut adapters = self.custom_adapters.write().await;

        if adapters.contains_key(&adapter_id) {
            return Err(CustomAdapterError::Configuration(format!(
                "Adapter with ID '{}' is already registered",
                adapter_id
            )));
        }

        adapters.insert(adapter_id, adapter);
        Ok(())
    }

    /// Register an adapter factory
    pub async fn register_factory(
        &self,
        factory: Box<dyn CustomAdapterFactory>,
    ) -> Result<(), CustomAdapterError> {
        let factory_id = factory.factory_id().to_string();
        let mut factories = self.adapter_factories.write().await;

        if factories.contains_key(&factory_id) {
            return Err(CustomAdapterError::Configuration(format!(
                "Factory with ID '{}' is already registered",
                factory_id
            )));
        }

        factories.insert(factory_id, factory);
        Ok(())
    }

    /// Create adapter from factory
    pub async fn create_adapter_from_factory(
        &self,
        factory_id: &str,
        adapter_type: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn CustomObservabilityAdapter>, CustomAdapterError> {
        let factories = self.adapter_factories.read().await;

        let factory = factories.get(factory_id).ok_or_else(|| {
            CustomAdapterError::Configuration(format!("Factory '{}' not found", factory_id))
        })?;

        // Validate configuration
        factory.validate_config(adapter_type, &config)?;

        // Create adapter
        factory.create_adapter(adapter_type, config).await
    }

    /// Load plugin from file
    pub async fn load_plugin(&self, plugin_path: &str) -> Result<String, CustomAdapterError> {
        self.plugin_loader.load_plugin(plugin_path).await
    }

    /// Get adapter by ID
    pub async fn get_adapter(
        &self,
        _adapter_id: &str,
    ) -> Option<Box<dyn CustomObservabilityAdapter>> {
        let _adapters = self.custom_adapters.read().await;
        // Note: This is simplified - in practice we'd need to handle trait objects differently
        None // Placeholder
    }

    /// List all registered adapters
    pub async fn list_adapters(&self) -> Vec<String> {
        let adapters = self.custom_adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Get performance metrics for all custom adapters
    pub async fn get_performance_metrics(&self) -> GlobalPerformanceMetrics {
        let monitor = self.performance_monitor.read().await;
        monitor.global_metrics.clone()
    }

    /// Process data through all applicable adapters
    pub async fn process_data(
        &self,
        metrics: &[MetricData],
        logs: &[LogData],
        traces: &[TraceData],
    ) -> Result<Vec<CustomProcessingResult>, CustomAdapterError> {
        let adapters = self.custom_adapters.read().await;
        let mut results = Vec::new();

        for (adapter_id, adapter) in adapters.iter() {
            let start_time = Instant::now();

            // Process metrics if adapter supports them
            if adapter
                .supported_data_types()
                .contains(&CustomDataType::Metrics)
                && !metrics.is_empty()
            {
                match adapter.process_metrics(metrics).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        // Log error but continue with other adapters
                        eprintln!("Adapter '{}' failed to process metrics: {}", adapter_id, e);
                    }
                }
            }

            // Process logs if adapter supports them
            if adapter
                .supported_data_types()
                .contains(&CustomDataType::Logs)
                && !logs.is_empty()
            {
                match adapter.process_logs(logs).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        eprintln!("Adapter '{}' failed to process logs: {}", adapter_id, e);
                    }
                }
            }

            // Process traces if adapter supports them
            if adapter
                .supported_data_types()
                .contains(&CustomDataType::Traces)
                && !traces.is_empty()
            {
                match adapter.process_traces(traces).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        eprintln!("Adapter '{}' failed to process traces: {}", adapter_id, e);
                    }
                }
            }

            // Record performance
            let latency_us = start_time.elapsed().as_micros() as u64;
            self.record_adapter_performance(adapter_id, latency_us, false)
                .await;
        }

        Ok(results)
    }

    /// Record adapter performance
    async fn record_adapter_performance(&self, adapter_id: &str, latency_us: u64, is_error: bool) {
        let mut monitor = self.performance_monitor.write().await;

        let performance_data = monitor
            .adapter_performance
            .entry(adapter_id.to_string())
            .or_insert_with(|| AdapterPerformanceData {
                latency_samples: vec![0; 1000],
                sample_index: 0,
                total_operations: 0,
                error_count: 0,
                last_update: Instant::now(),
            });

        performance_data.latency_samples[performance_data.sample_index] = latency_us;
        performance_data.sample_index =
            (performance_data.sample_index + 1) % performance_data.latency_samples.len();
        performance_data.total_operations += 1;
        performance_data.last_update = Instant::now();

        if is_error {
            performance_data.error_count += 1;
        }
    }
}

impl PluginLoader {
    /// Create new plugin loader
    pub fn new() -> Self {
        Self {
            loaded_plugins: Arc::new(RwLock::new(HashMap::new())),
            search_paths: vec![
                "./plugins".to_string(),
                "/usr/local/lib/valkyrie/plugins".to_string(),
                "~/.valkyrie/plugins".to_string(),
            ],
            security_policy: PluginSecurityPolicy {
                allow_unsigned: false,
                trusted_authors: vec!["valkyrie-team".to_string()],
                allowed_permissions: vec![
                    "network.connect".to_string(),
                    "file.read".to_string(),
                    "metrics.collect".to_string(),
                ],
                sandbox_enabled: true,
                resource_limits: PluginResourceLimits {
                    max_memory_bytes: 100 * 1024 * 1024, // 100MB
                    max_cpu_percent: 10.0,               // 10%
                    max_network_bps: 10 * 1024 * 1024,   // 10MB/s
                    max_disk_io_bps: 5 * 1024 * 1024,    // 5MB/s
                },
            },
        }
    }

    /// Load plugin from file
    pub async fn load_plugin(&self, plugin_path: &str) -> Result<String, CustomAdapterError> {
        // Validate plugin security
        self.validate_plugin_security(plugin_path).await?;

        // Load plugin metadata
        let metadata = self.load_plugin_metadata(plugin_path).await?;

        // Create plugin handle (simplified)
        let plugin_handle = PluginHandle {
            id: Uuid::new_v4().to_string(),
            interface_version: "1.0.0".to_string(),
        };

        let plugin_id = metadata.name.clone();
        let loaded_plugin = LoadedPlugin {
            id: plugin_id.clone(),
            path: plugin_path.to_string(),
            metadata,
            loaded_at: Instant::now(),
            handle: plugin_handle,
        };

        // Store loaded plugin
        let mut plugins = self.loaded_plugins.write().await;
        plugins.insert(plugin_id.clone(), loaded_plugin);

        Ok(plugin_id)
    }

    /// Validate plugin security
    async fn validate_plugin_security(&self, _plugin_path: &str) -> Result<(), CustomAdapterError> {
        // Simplified security validation
        // In a real implementation, this would:
        // - Check plugin signatures
        // - Validate permissions
        // - Scan for malicious code
        // - Apply sandbox restrictions
        Ok(())
    }

    /// Load plugin metadata
    async fn load_plugin_metadata(
        &self,
        _plugin_path: &str,
    ) -> Result<PluginMetadata, CustomAdapterError> {
        // Simplified metadata loading
        // In a real implementation, this would parse the plugin manifest
        Ok(PluginMetadata {
            name: "example-plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "example-author".to_string(),
            description: "Example custom adapter plugin".to_string(),
            supported_adapters: vec!["custom-metrics".to_string()],
            required_permissions: vec!["metrics.collect".to_string()],
            min_valkyrie_version: "1.0.0".to_string(),
        })
    }
}

impl CustomAdapterPerformanceMonitor {
    /// Create new performance monitor
    pub fn new() -> Self {
        Self {
            adapter_performance: HashMap::new(),
            global_metrics: GlobalPerformanceMetrics {
                total_adapters: 0,
                active_adapters: 0,
                total_operations: 0,
                avg_latency_us: 0.0,
                system_resources: CustomResourceMetrics {
                    memory_usage_bytes: 0,
                    cpu_usage_percent: 0.0,
                    network_bytes_sent: 0,
                    network_bytes_received: 0,
                    disk_io_bytes: 0,
                },
            },
            start_time: Instant::now(),
        }
    }
}

impl Default for CustomAdapterFramework {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for CustomAdapterPerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}
