use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use rustci::config::valkyrie::ValkyrieConfig;
use rustci::core::networking::valkyrie::message::{ValkyrieMessage, MessageType};
use rustci::core::networking::valkyrie::transport::TransportType;

/// Test fixtures for Valkyrie Protocol testing
pub struct TestFixtures;

impl TestFixtures {
    /// Create a minimal test configuration
    pub fn minimal_config() -> ValkyrieConfig {
        ValkyrieConfig {
            node_id: "test-node".to_string(),
            bind_address: "127.0.0.1".to_string(),
            bind_port: 0, // Let OS choose port
            transport_type: TransportType::Tcp,
            security_enabled: false,
            max_connections: 100,
            message_timeout: std::time::Duration::from_secs(30),
            ..Default::default()
        }
    }

    /// Create a secure test configuration
    pub fn secure_config() -> ValkyrieConfig {
        ValkyrieConfig {
            node_id: "secure-test-node".to_string(),
            bind_address: "127.0.0.1".to_string(),
            bind_port: 0,
            transport_type: TransportType::Tcp,
            security_enabled: true,
            tls_cert_path: Some("test-cert.pem".to_string()),
            tls_key_path: Some("test-key.pem".to_string()),
            max_connections: 100,
            message_timeout: std::time::Duration::from_secs(30),
            ..Default::default()
        }
    }

    /// Create a high-performance test configuration
    pub fn high_performance_config() -> ValkyrieConfig {
        ValkyrieConfig {
            node_id: "perf-test-node".to_string(),
            bind_address: "127.0.0.1".to_string(),
            bind_port: 0,
            transport_type: TransportType::Quic,
            security_enabled: true,
            max_connections: 10000,
            message_timeout: std::time::Duration::from_millis(100),
            enable_compression: true,
            enable_multiplexing: true,
            buffer_size: 65536,
            ..Default::default()
        }
    }

    /// Create test messages for different scenarios
    pub fn create_test_messages() -> HashMap<&'static str, ValkyrieMessage> {
        let mut messages = HashMap::new();

        // Small message
        messages.insert("small", ValkyrieMessage {
            header: Default::default(),
            payload: b"Hello, Valkyrie!".to_vec(),
            signature: None,
            trace_context: None,
        });

        // Large message (1MB)
        messages.insert("large", ValkyrieMessage {
            header: Default::default(),
            payload: vec![0u8; 1024 * 1024],
            signature: None,
            trace_context: None,
        });

        // Job request message
        messages.insert("job_request", ValkyrieMessage {
            header: Default::default(),
            payload: serde_json::to_vec(&serde_json::json!({
                "job_id": "test-job-123",
                "pipeline": "test-pipeline",
                "steps": ["build", "test", "deploy"]
            })).unwrap(),
            signature: None,
            trace_context: None,
        });

        messages
    }

    /// Create test transport configurations
    pub fn transport_configs() -> Vec<ValkyrieConfig> {
        vec![
            // TCP transport
            ValkyrieConfig {
                transport_type: TransportType::Tcp,
                ..Self::minimal_config()
            },
            // WebSocket transport
            ValkyrieConfig {
                transport_type: TransportType::WebSocket,
                ..Self::minimal_config()
            },
            // QUIC transport
            ValkyrieConfig {
                transport_type: TransportType::Quic,
                security_enabled: true,
                ..Self::minimal_config()
            },
        ]
    }

    /// Create test security configurations
    pub fn security_configs() -> Vec<ValkyrieConfig> {
        vec![
            // No security
            ValkyrieConfig {
                security_enabled: false,
                ..Self::minimal_config()
            },
            // TLS security
            ValkyrieConfig {
                security_enabled: true,
                auth_method: Some("tls".to_string()),
                ..Self::minimal_config()
            },
            // Token-based security
            ValkyrieConfig {
                security_enabled: true,
                auth_method: Some("token".to_string()),
                auth_token: Some("test-token-123".to_string()),
                ..Self::minimal_config()
            },
        ]
    }

    /// Create test load scenarios
    pub fn load_scenarios() -> Vec<LoadScenario> {
        vec![
            LoadScenario {
                name: "light_load".to_string(),
                concurrent_connections: 10,
                messages_per_second: 100,
                message_size: 1024,
                duration: std::time::Duration::from_secs(30),
            },
            LoadScenario {
                name: "medium_load".to_string(),
                concurrent_connections: 100,
                messages_per_second: 1000,
                message_size: 4096,
                duration: std::time::Duration::from_secs(60),
            },
            LoadScenario {
                name: "heavy_load".to_string(),
                concurrent_connections: 1000,
                messages_per_second: 10000,
                message_size: 8192,
                duration: std::time::Duration::from_secs(120),
            },
        ]
    }

    /// Create chaos engineering scenarios
    pub fn chaos_scenarios() -> Vec<ChaosScenario> {
        vec![
            ChaosScenario {
                name: "network_partition".to_string(),
                description: "Simulate network partition between nodes".to_string(),
                duration: std::time::Duration::from_secs(30),
                recovery_time: std::time::Duration::from_secs(10),
            },
            ChaosScenario {
                name: "high_latency".to_string(),
                description: "Inject high network latency".to_string(),
                duration: std::time::Duration::from_secs(60),
                recovery_time: std::time::Duration::from_secs(5),
            },
            ChaosScenario {
                name: "packet_loss".to_string(),
                description: "Simulate packet loss".to_string(),
                duration: std::time::Duration::from_secs(45),
                recovery_time: std::time::Duration::from_secs(5),
            },
        ]
    }
}

/// Load testing scenario
#[derive(Debug, Clone)]
pub struct LoadScenario {
    pub name: String,
    pub concurrent_connections: usize,
    pub messages_per_second: usize,
    pub message_size: usize,
    pub duration: std::time::Duration,
}

/// Chaos engineering scenario
#[derive(Debug, Clone)]
pub struct ChaosScenario {
    pub name: String,
    pub description: String,
    pub duration: std::time::Duration,
    pub recovery_time: std::time::Duration,
}

/// Test environment setup
pub struct TestEnvironment {
    pub config: ValkyrieConfig,
    pub temp_dir: tempfile::TempDir,
}

impl TestEnvironment {
    /// Create a new test environment
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config = TestFixtures::minimal_config();
        
        Ok(TestEnvironment {
            config,
            temp_dir,
        })
    }

    /// Create a secure test environment with certificates
    pub fn new_secure() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let mut config = TestFixtures::secure_config();
        
        // Generate test certificates
        let cert_path = temp_dir.path().join("test-cert.pem");
        let key_path = temp_dir.path().join("test-key.pem");
        
        Self::generate_test_certificates(&cert_path, &key_path)?;
        
        config.tls_cert_path = Some(cert_path.to_string_lossy().to_string());
        config.tls_key_path = Some(key_path.to_string_lossy().to_string());
        
        Ok(TestEnvironment {
            config,
            temp_dir,
        })
    }

    /// Generate test certificates for secure testing
    fn generate_test_certificates(
        cert_path: &std::path::Path,
        key_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // This is a simplified certificate generation for testing
        // In a real implementation, you would use a proper certificate generation library
        std::fs::write(cert_path, "-----BEGIN CERTIFICATE-----\nTEST_CERT\n-----END CERTIFICATE-----")?;
        std::fs::write(key_path, "-----BEGIN PRIVATE KEY-----\nTEST_KEY\n-----END PRIVATE KEY-----")?;
        Ok(())
    }
}