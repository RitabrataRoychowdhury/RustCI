use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Primary storage mode
    pub mode: StoreMode,
    /// Fallback chain for when primary fails
    pub fallback_chain: Vec<StoreMode>,
    /// Redis configuration
    pub redis: Option<RedisConfig>,
    /// ValKey configuration
    pub valkey: Option<ValKeyConfig>,
    /// Yggdrasil configuration (future)
    pub yggdrasil: Option<YggdrasilConfig>,
    /// Global timeout settings
    pub timeouts: TimeoutConfig,
    /// Retry configuration
    pub retry: RetryConfig,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            mode: StoreMode::Redis,
            fallback_chain: vec![StoreMode::Redis],
            redis: Some(RedisConfig::default()),
            valkey: None,
            yggdrasil: None,
            timeouts: TimeoutConfig::default(),
            retry: RetryConfig::default(),
        }
    }
}

/// Storage backend modes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StoreMode {
    #[serde(rename = "redis")]
    Redis,
    #[serde(rename = "valkey")]
    ValKey,
    #[serde(rename = "yggdrasil")]
    Yggdrasil,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Connection pool size
    pub pool_size: u32,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Connection timeout
    pub timeout_ms: u64,
    /// Enable cluster mode
    pub cluster_mode: bool,
    /// Username for authentication
    pub username: Option<String>,
    /// Password for authentication
    pub password: Option<String>,
    /// Database number
    pub database: Option<u32>,
    /// Enable TLS
    pub tls: bool,
    /// Custom Redis commands timeout
    pub command_timeout_ms: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            max_retries: 3,
            timeout_ms: 1000,
            cluster_mode: false,
            username: None,
            password: None,
            database: None,
            tls: false,
            command_timeout_ms: 5000,
        }
    }
}

/// ValKey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValKeyConfig {
    /// ValKey connection URL
    pub url: String,
    /// Username for authentication
    pub username: String,
    /// Encrypted password
    pub password_encrypted: String,
    /// Connection pool size
    pub pool_size: u32,
    /// Enable cluster mode
    pub cluster_mode: bool,
    /// Connection timeout
    pub timeout_ms: u64,
    /// Enable TLS
    pub tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS key path
    pub tls_key_path: Option<String>,
    /// TLS CA path
    pub tls_ca_path: Option<String>,
    /// Custom ValKey features
    pub features: ValKeyFeatures,
}

impl Default for ValKeyConfig {
    fn default() -> Self {
        Self {
            url: "valkey://localhost:6380".to_string(),
            username: "valkey_user".to_string(),
            password_encrypted: "".to_string(),
            pool_size: 20,
            cluster_mode: true,
            timeout_ms: 1000,
            tls: true,
            tls_cert_path: None,
            tls_key_path: None,
            tls_ca_path: None,
            features: ValKeyFeatures::default(),
        }
    }
}

/// ValKey-specific features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValKeyFeatures {
    /// Enable advanced compression
    pub compression: bool,
    /// Enable data encryption
    pub encryption: bool,
    /// Enable advanced clustering
    pub advanced_clustering: bool,
    /// Enable performance optimizations
    pub performance_mode: bool,
}

impl Default for ValKeyFeatures {
    fn default() -> Self {
        Self {
            compression: true,
            encryption: true,
            advanced_clustering: true,
            performance_mode: true,
        }
    }
}

/// Yggdrasil configuration (future implementation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YggdrasilConfig {
    /// Enable Yggdrasil (currently always false)
    pub enabled: bool,
    /// Bind address for the server
    pub bind_address: String,
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Number of shards per core
    pub shards_per_core: u32,
    /// Enable NUMA awareness
    pub numa_aware: bool,
    /// Enable zero-copy optimizations
    pub zero_copy: bool,
    /// Consistent hashing replicas
    pub hash_replicas: u32,
    /// Snapshot interval
    pub snapshot_interval_seconds: u64,
}

impl Default for YggdrasilConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: "0.0.0.0:7777".to_string(),
            max_memory_mb: 1024,
            shards_per_core: 16,
            numa_aware: true,
            zero_copy: true,
            hash_replicas: 150,
            snapshot_interval_seconds: 300,
        }
    }
}

/// Timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Default operation timeout
    pub default_timeout_ms: u64,
    /// Connection timeout
    pub connection_timeout_ms: u64,
    /// Health check timeout
    pub health_check_timeout_ms: u64,
    /// Batch operation timeout
    pub batch_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 1000,
            connection_timeout_ms: 5000,
            health_check_timeout_ms: 2000,
            batch_timeout_ms: 10000,
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial retry delay
    pub initial_delay_ms: u64,
    /// Maximum retry delay
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

/// Telemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// OpenTelemetry configuration
    pub opentelemetry: OpenTelemetryConfig,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            opentelemetry: OpenTelemetryConfig::default(),
        }
    }
}

/// OpenTelemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// Enable OpenTelemetry
    pub enabled: bool,
    /// OTLP endpoint
    pub endpoint: String,
    /// Service name
    pub service_name: String,
    /// Additional headers
    pub headers: HashMap<String, String>,
    /// Export timeout
    pub timeout_ms: u64,
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".to_string(),
            service_name: "valkyrie-storage".to_string(),
            headers: HashMap::new(),
            timeout_ms: 10000,
        }
    }
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// List of notification adapters
    pub adapters: Vec<NotificationAdapterConfig>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            adapters: vec![NotificationAdapterConfig {
                adapter_type: "console".to_string(),
                enabled: true,
                config: HashMap::new(),
            }],
        }
    }
}

/// Individual notification adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAdapterConfig {
    #[serde(rename = "type")]
    pub adapter_type: String,
    pub enabled: bool,
    pub config: HashMap<String, serde_json::Value>,
}

/// Rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterConfig {
    /// Default number of tokens
    pub default_tokens: u32,
    /// Token refill rate per second
    pub refill_rate: u32,
    /// Time window in seconds
    pub window_seconds: u32,
    /// Enable distributed rate limiting
    pub distributed: bool,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            default_tokens: 100,
            refill_rate: 10,
            window_seconds: 60,
            distributed: true,
        }
    }
}