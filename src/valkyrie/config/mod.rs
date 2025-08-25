// Configuration module for Valkyrie High-Performance Routing
// Provides configuration management, validation, and hot-reload capabilities

pub mod validation;
pub mod hot_reload;

pub use validation::{ConfigValidator, ValidationReport};
pub use hot_reload::{HotReloadManager, HotReloadConfig};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigValidationError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieConfig {
    pub server: ServerConfig,
    pub performance: PerformanceConfig,
    pub routing: RoutingConfig,
    pub security: SecurityConfig,
    pub observability: ObservabilityConfig,
    pub caching: CachingConfig,
    pub transport: TransportConfig,
    pub streaming: StreamingConfig,
    pub protocol: ProtocolConfig,
}

impl Default for ValkyrieConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            performance: PerformanceConfig::default(),
            routing: RoutingConfig::default(),
            security: SecurityConfig::default(),
            observability: ObservabilityConfig::default(),
            caching: CachingConfig::default(),
            transport: TransportConfig::default(),
            streaming: StreamingConfig::default(),
            protocol: ProtocolConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub https_port: Option<u16>,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub keepalive_timeout_ms: u64,
    pub worker_threads: usize,
    pub blocking_threads: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            https_port: Some(8443),
            max_connections: 10000,
            connection_timeout_ms: 30000,
            request_timeout_ms: 10000,
            keepalive_timeout_ms: 60000,
            worker_threads: num_cpus::get(),
            blocking_threads: num_cpus::get() * 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub mode: String,
    pub enable_simd: Option<bool>,
    pub enable_zero_copy: Option<bool>,
    pub enable_hot_path_optimization: Option<bool>,
    pub performance_budget: Option<String>,
    pub throughput_target: Option<u64>,
    pub memory: Option<MemoryConfig>,
    pub cpu: Option<CpuConfig>,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            mode: "high".to_string(),
            enable_simd: Some(true),
            enable_zero_copy: Some(true),
            enable_hot_path_optimization: Some(true),
            performance_budget: Some("82us".to_string()),
            throughput_target: Some(1_000_000),
            memory: Some(MemoryConfig::default()),
            cpu: Some(CpuConfig::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub max_heap_size: Option<String>,
    pub enable_jemalloc: bool,
    pub gc_threshold: Option<String>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_heap_size: Some("6GB".to_string()),
            enable_jemalloc: true,
            gc_threshold: Some("4GB".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuConfig {
    pub enable_cpu_affinity: bool,
    pub cpu_cores: Option<Vec<usize>>,
    pub enable_numa_awareness: bool,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            enable_cpu_affinity: true,
            cpu_cores: Some((0..num_cpus::get()).collect()),
            enable_numa_awareness: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub strategy: String,
    pub max_routes: u64,
    pub route_cache_size: u64,
    pub connection_pool_size: u32,
    pub max_concurrent_requests: u64,
    pub lockfree: Option<LockFreeConfig>,
    pub fuzzy_matching: Option<FuzzyMatchingConfig>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            strategy: "high_performance".to_string(),
            max_routes: 1_000_000,
            route_cache_size: 100_000,
            connection_pool_size: 1000,
            max_concurrent_requests: 50_000,
            lockfree: Some(LockFreeConfig::default()),
            fuzzy_matching: Some(FuzzyMatchingConfig::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFreeConfig {
    pub fit_capacity: u32,
    pub fit_load_factor: f64,
    pub cra_initial_capacity: u32,
    pub cra_enable_compression: bool,
    pub rcu_grace_period: String,
}

impl Default for LockFreeConfig {
    fn default() -> Self {
        Self {
            fit_capacity: 65536,
            fit_load_factor: 0.75,
            cra_initial_capacity: 32768,
            cra_enable_compression: true,
            rcu_grace_period: "10ms".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzyMatchingConfig {
    pub enable: bool,
    pub max_results: u32,
    pub timeout_ms: u64,
    pub min_score: u32,
}

impl Default for FuzzyMatchingConfig {
    fn default() -> Self {
        Self {
            enable: true,
            max_results: 100,
            timeout_ms: 50,
            min_score: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_tls: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub tls_ca_path: Option<String>,
    pub rate_limiting: Option<RateLimitingConfig>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: true,
            tls_cert_path: Some("/app/config/ssl/cert.pem".to_string()),
            tls_key_path: Some("/app/config/ssl/key.pem".to_string()),
            tls_ca_path: Some("/app/config/ssl/ca.pem".to_string()),
            rate_limiting: Some(RateLimitingConfig::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub enable: bool,
    pub default_rate: String,
    pub api_rate: String,
    pub burst_size: u32,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enable: true,
            default_rate: "1000/min".to_string(),
            api_rate: "10000/min".to_string(),
            burst_size: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub metrics: Option<MetricsConfig>,
    pub tracing: Option<TracingConfig>,
    pub logging: Option<LoggingConfig>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics: Some(MetricsConfig::default()),
            tracing: Some(TracingConfig::default()),
            logging: Some(LoggingConfig::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enable: bool,
    pub port: Option<u16>,
    pub path: String,
    pub enable_detailed_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enable: true,
            port: Some(9090),
            path: "/metrics".to_string(),
            enable_detailed_metrics: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub enable: bool,
    pub sampling_rate: f64,
    pub jaeger_endpoint: Option<String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enable: true,
            sampling_rate: 0.1,
            jaeger_endpoint: Some("http://jaeger:14268/api/traces".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub enable_file_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            enable_file_logging: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachingConfig {
    pub redis: Option<RedisConfig>,
    pub policies: Option<CachePoliciesConfig>,
}

impl Default for CachingConfig {
    fn default() -> Self {
        Self {
            redis: Some(RedisConfig::default()),
            policies: Some(CachePoliciesConfig::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub enable: bool,
    pub url: String,
    pub pool_size: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enable: true,
            url: "redis://redis:6379".to_string(),
            pool_size: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePoliciesConfig {
    pub max_cache_memory: Option<String>,
    pub eviction_policy: String,
}

impl Default for CachePoliciesConfig {
    fn default() -> Self {
        Self {
            max_cache_memory: Some("1GB".to_string()),
            eviction_policy: "lru".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub primary_transport: String,
    pub fallback_transports: Vec<String>,
    pub connection_pool_size: usize,
    pub timeout_ms: u64,
    pub retry_attempts: u32,
    pub enable_compression: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            primary_transport: "tcp".to_string(),
            fallback_transports: vec!["websocket".to_string()],
            connection_pool_size: 100,
            timeout_ms: 30000,
            retry_attempts: 3,
            enable_compression: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub enable_multiplexing: bool,
    pub max_concurrent_streams: usize,
    pub stream_timeout_ms: u64,
    pub buffer_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enable_multiplexing: true,
            max_concurrent_streams: 1000,
            stream_timeout_ms: 60000,
            buffer_size: 8192,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub magic: u32,
    pub version: String,
    pub connection_timeout: u64,
    pub message_timeout: u64,
    pub heartbeat_interval: u64,
    pub max_message_size: usize,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            magic: 0x56414C4B, // "VALK"
            version: "1.0.0".to_string(),
            connection_timeout: 30000,
            message_timeout: 10000,
            heartbeat_interval: 30000,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

// Builder pattern for ValkyrieConfig
#[derive(Debug, Clone)]
pub struct ValkyrieConfigBuilder {
    config: ValkyrieConfig,
}

impl ValkyrieConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ValkyrieConfig::default(),
        }
    }

    pub fn with_server_config(mut self, server: ServerConfig) -> Self {
        self.config.server = server;
        self
    }

    pub fn with_performance_config(mut self, performance: PerformanceConfig) -> Self {
        self.config.performance = performance;
        self
    }

    pub fn with_routing_config(mut self, routing: RoutingConfig) -> Self {
        self.config.routing = routing;
        self
    }

    pub fn with_security_config(mut self, security: SecurityConfig) -> Self {
        self.config.security = security;
        self
    }

    pub fn with_observability_config(mut self, observability: ObservabilityConfig) -> Self {
        self.config.observability = observability;
        self
    }

    pub fn with_caching_config(mut self, caching: CachingConfig) -> Self {
        self.config.caching = caching;
        self
    }

    pub fn with_transport_config(mut self, transport: TransportConfig) -> Self {
        self.config.transport = transport;
        self
    }

    pub fn with_streaming_config(mut self, streaming: StreamingConfig) -> Self {
        self.config.streaming = streaming;
        self
    }

    pub fn with_protocol_config(mut self, protocol: ProtocolConfig) -> Self {
        self.config.protocol = protocol;
        self
    }

    // Convenience methods for common configurations
    pub fn with_connection_timeout(mut self, timeout_ms: u64) -> Self {
        self.config.server.connection_timeout_ms = timeout_ms;
        self
    }

    pub fn with_message_timeout(mut self, timeout_ms: u64) -> Self {
        self.config.server.request_timeout_ms = timeout_ms;
        self
    }

    pub fn with_default_transport(self) -> Self {
        // Default transport configuration is already set
        self
    }

    pub fn with_default_streaming(self) -> Self {
        // Default streaming configuration is already set
        self
    }

    pub fn with_default_security(self) -> Self {
        // Default security configuration is already set
        self
    }

    pub fn enable_container_transport(mut self) -> Self {
        self.config.transport.fallback_transports.push("container".to_string());
        self
    }

    pub fn with_minimal_security(mut self) -> Self {
        self.config.security.enable_tls = false;
        if let Some(rate_limiting) = &mut self.config.security.rate_limiting {
            rate_limiting.enable = false;
        }
        self
    }

    pub fn with_high_throughput_settings(mut self) -> Self {
        self.config.transport.connection_pool_size = 1000;
        self.config.streaming.max_concurrent_streams = 5000;
        self.config.performance.throughput_target = Some(10_000_000);
        self
    }

    pub fn with_feature(mut self, feature: &str, enabled: bool) -> Self {
        // This would typically update feature flags
        // For now, just log the feature setting
        tracing::debug!("Setting feature {} to {}", feature, enabled);
        self
    }

    pub fn enable_simd_optimizations(mut self) -> Self {
        self.config.performance.enable_simd = Some(true);
        self
    }

    pub fn enable_post_quantum_crypto(mut self) -> Self {
        // This would enable post-quantum cryptography
        // For now, just set a flag in security config
        self
    }

    pub fn with_default_observability(self) -> Self {
        // Default observability configuration is already set
        self
    }

    pub fn enable_kubernetes_transport(mut self) -> Self {
        self.config.transport.fallback_transports.push("kubernetes".to_string());
        self
    }

    pub fn with_rustci_defaults(mut self) -> Self {
        // Optimize for RustCI workloads
        self.config.performance.mode = "rustci".to_string();
        self.config.server.worker_threads = num_cpus::get() * 2;
        self.config.routing.connection_pool_size = 500;
        self
    }

    pub fn enable_ci_optimizations(mut self) -> Self {
        self.config.performance.enable_hot_path_optimization = Some(true);
        self.config.routing.route_cache_size = 50_000;
        self
    }

    pub fn enable_zero_copy(mut self) -> Self {
        self.config.performance.enable_zero_copy = Some(true);
        self
    }

    pub fn with_maximum_security(mut self) -> Self {
        self.config.security.enable_tls = true;
        if let Some(rate_limiting) = &mut self.config.security.rate_limiting {
            rate_limiting.enable = true;
            rate_limiting.default_rate = "100/min".to_string();
        }
        self
    }

    pub fn enable_mutual_tls(mut self) -> Self {
        self.config.security.enable_tls = true;
        self
    }

    pub fn enable_audit_logging(self) -> Self {
        // Audit logging would be implemented here
        // For now, just return self as this is a placeholder
        self
    }

    pub fn build(self) -> Result<ValkyrieConfig, ConfigValidationError> {
        // Validate the configuration before returning
        let validator = crate::valkyrie::config::validation::ConfigValidator::new(true);
        let report = validator.validate_config(&self.config);
        
        if !report.is_valid {
            return Err(ConfigValidationError::Invalid(format!(
                "Configuration validation failed: {:?}", 
                report.errors
            )));
        }
        
        Ok(self.config)
    }
}

impl Default for ValkyrieConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}