//! Configuration utilities for Valkyrie server

use valkyrie_sdk::{Config, ServerConfig, ClientConfig, config::GlobalConfig, retry::RetryPolicy};

/// Create a sample configuration with sensible defaults
pub fn create_sample_config() -> Config {
    Config {
        server: Some(ServerConfig {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 10000,
            connection_timeout_ms: 30000,
            enable_tls: false,
            tls_cert_path: Some("certs/server.crt".to_string()),
            tls_key_path: Some("certs/server.key".to_string()),
            enable_metrics: true,
        }),
        client: Some(ClientConfig {
            endpoint: "tcp://localhost:8080".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_connections: 10,
            enable_pooling: true,
            retry_policy: RetryPolicy::exponential_backoff(3, 100, 5000),
            enable_metrics: true,
        }),
        global: GlobalConfig {
            log_level: "info".to_string(),
            enable_metrics: true,
            metrics_endpoint: Some("http://localhost:9090/metrics".to_string()),
        },
    }
}