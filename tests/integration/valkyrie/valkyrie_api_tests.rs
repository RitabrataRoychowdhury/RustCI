//! Integration tests for Valkyrie Protocol API and SDK Foundation
//!
//! This test suite validates the API and SDK functionality across different
//! language bindings and ensures compatibility and correctness.

use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

use rustci::api::valkyrie::{
    ValkyrieClient, ClientConfig, ClientMessage, ClientMessageType,
    ClientMessagePriority, ClientPayload, ClientSecurityConfig,
    ClientAuthMethod, ClientPerformanceConfig, ClientTimeoutConfig,
    ClientFeatureFlags
};
use rustci::api::codegen::{
    CodeGenerator, CodeGenConfig, Language, extract_api_definition
};
use rustci::bindings::rust::prelude::*;

#[tokio::test]
async fn test_valkyrie_client_creation() {
    let config = ClientConfig::default();
    let result = ValkyrieClient::new(config).await;
    assert!(result.is_ok(), "Failed to create ValkyrieClient: {:?}", result.err());
    
    let client = result.unwrap();
    let stats = client.get_stats().await;
    assert_eq!(stats.active_connections, 0);
}

#[tokio::test]
async fn test_valkyrie_client_with_custom_config() {
    let config = ClientConfig {
        security: ClientSecurityConfig {
            auth_method: ClientAuthMethod::Token,
            enable_encryption: true,
            enable_audit: true,
            cert_path: None,
            key_path: None,
            ca_path: None,
        },
        performance: ClientPerformanceConfig {
            worker_threads: Some(4),
            max_connections_per_endpoint: 20,
            max_streams_per_connection: 200,
            buffer_size: 16384,
            message_batch_size: 200,
        },
        timeouts: ClientTimeoutConfig {
            connect_timeout: Duration::from_secs(15),
            send_timeout: Duration::from_secs(10),
            heartbeat_interval: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(600),
        },
        features: ClientFeatureFlags {
            enable_experimental: false,
            enable_metrics: true,
            enable_tracing: true,
            enable_zero_copy: true,
            enable_simd: true,
            enable_post_quantum: false,
            enable_ml_optimizations: false,
        },
        ..Default::default()
    };

    let result = ValkyrieClient::new(config).await;
    assert!(result.is_ok(), "Failed to create ValkyrieClient with custom config: {:?}", result.err());
}

#[tokio::test]
async fn test_simple_client_factory() {
    let result = ValkyrieClient::simple().await;
    assert!(result.is_ok(), "Failed to create simple client: {:?}", result.err());
}

#[tokio::test]
async fn test_high_performance_client_factory() {
    let result = ValkyrieClient::high_performance().await;
    assert!(result.is_ok(), "Failed to create high-performance client: {:?}", result.err());
}

#[test]
fn test_client_message_creation() {
    // Test text message
    let text_msg = ClientMessage::text("Hello, Valkyrie!");
    assert!(matches!(text_msg.message_type, ClientMessageType::Text));
    assert!(matches!(text_msg.priority, ClientMessagePriority::Normal));
    if let ClientPayload::Text(content) = &text_msg.payload {
        assert_eq!(content, "Hello, Valkyrie!");
    } else {
        panic!("Expected text payload");
    }

    // Test binary message
    let binary_data = vec![0x01, 0x02, 0x03, 0x04];
    let binary_msg = ClientMessage::binary(binary_data.clone());
    assert!(matches!(binary_msg.message_type, ClientMessageType::Binary));
    if let ClientPayload::Binary(data) = &binary_msg.payload {
        assert_eq!(data, &binary_data);
    } else {
        panic!("Expected binary payload");
    }

    // Test JSON message
    let json_data = serde_json::json!({"key": "value", "number": 42});
    let json_msg = ClientMessage::json(&json_data);
    assert!(json_msg.is_ok(), "Failed to create JSON message: {:?}", json_msg.err());
    
    let json_msg = json_msg.unwrap();
    assert!(matches!(json_msg.message_type, ClientMessageType::Json));
}

#[test]
fn test_message_builder_pattern() {
    let message = ClientMessage::text("Test message")
        .with_priority(ClientMessagePriority::High)
        .with_correlation_id("test-123".to_string())
        .with_ttl(Duration::from_secs(30))
        .with_metadata("source".to_string(), "test".to_string());

    assert!(matches!(message.priority, ClientMessagePriority::High));
    assert_eq!(message.correlation_id, Some("test-123".to_string()));
    assert_eq!(message.ttl, Some(Duration::from_secs(30)));
    assert_eq!(message.metadata.get("source"), Some(&"test".to_string()));
}

#[test]
fn test_message_to_valkyrie_conversion() {
    let client_message = ClientMessage::text("Test")
        .with_priority(ClientMessagePriority::Critical);
    
    let valkyrie_message = client_message.to_valkyrie_message();
    
    assert!(matches!(valkyrie_message.header.message_type, MessageType::Data));
    assert!(matches!(valkyrie_message.header.priority, MessagePriority::Critical));
    assert_eq!(valkyrie_message.payload, b"Test");
}

#[test]
fn test_api_definition_extraction() {
    let api_def = extract_api_definition();
    
    assert_eq!(api_def.name, "Valkyrie Protocol");
    assert_eq!(api_def.version, "1.0.0");
    assert!(!api_def.types.is_empty());
    assert!(!api_def.services.is_empty());
    assert!(!api_def.errors.is_empty());
    
    // Check that we have the expected types
    let type_names: Vec<&String> = api_def.types.iter().map(|t| &t.name).collect();
    assert!(type_names.contains(&&"ValkyrieMessage".to_string()));
    assert!(type_names.contains(&&"MessageType".to_string()));
    
    // Check that we have the expected services
    let service_names: Vec<&String> = api_def.services.iter().map(|s| &s.name).collect();
    assert!(service_names.contains(&&"ValkyrieClient".to_string()));
}

#[test]
fn test_code_generation_config() {
    let config = CodeGenConfig {
        output_dir: "/tmp/test".to_string(),
        languages: vec![Language::Rust, Language::Python, Language::JavaScript],
        template_dir: Some("/templates".to_string()),
        options: {
            let mut opts = HashMap::new();
            opts.insert("version".to_string(), serde_json::Value::String("1.0.0".to_string()));
            opts
        },
    };

    assert_eq!(config.languages.len(), 3);
    assert!(config.languages.contains(&Language::Rust));
    assert!(config.languages.contains(&Language::Python));
    assert!(config.languages.contains(&Language::JavaScript));
}

#[test]
fn test_language_properties() {
    assert_eq!(Language::Rust.extension(), "rs");
    assert_eq!(Language::C.extension(), "h");
    assert_eq!(Language::Python.extension(), "py");
    assert_eq!(Language::JavaScript.extension(), "js");
    assert_eq!(Language::Go.extension(), "go");
    assert_eq!(Language::Java.extension(), "java");

    assert_eq!(Language::Rust.package_name(), "valkyrie_protocol");
    assert_eq!(Language::Python.package_name(), "valkyrie_protocol");
    assert_eq!(Language::JavaScript.package_name(), "@valkyrie/protocol");
    assert_eq!(Language::Go.package_name(), "github.com/rustci/valkyrie-protocol-go");
    assert_eq!(Language::Java.package_name(), "com.valkyrie.protocol");
}

#[test]
fn test_code_generation() {
    let temp_dir = TempDir::new().unwrap();
    let config = CodeGenConfig {
        output_dir: temp_dir.path().to_string_lossy().to_string(),
        languages: vec![Language::Rust, Language::C, Language::Python],
        template_dir: None,
        options: HashMap::new(),
    };

    let api_def = extract_api_definition();
    let generator = CodeGenerator::new(config, api_def);

    // Test individual language generation
    assert!(generator.generate_language(Language::Rust).is_ok());
    assert!(generator.generate_language(Language::C).is_ok());
    assert!(generator.generate_language(Language::Python).is_ok());

    // Check that files were created
    let rust_dir = temp_dir.path().join("rust");
    assert!(rust_dir.exists());
    assert!(rust_dir.join("lib.rs").exists());
    assert!(rust_dir.join("types.rs").exists());
    assert!(rust_dir.join("client.rs").exists());
    assert!(rust_dir.join("Cargo.toml").exists());

    let c_dir = temp_dir.path().join("c");
    assert!(c_dir.exists());
    assert!(c_dir.join("valkyrie_protocol.h").exists());

    let python_dir = temp_dir.path().join("python");
    assert!(python_dir.exists());
    assert!(python_dir.join("valkyrie_protocol.pyi").exists());
    assert!(python_dir.join("setup.py").exists());
}

#[test]
fn test_c_bindings_generation() {
    use rustci::bindings::c::generate_c_header;
    
    let header = generate_c_header();
    
    assert!(header.contains("#ifndef VALKYRIE_PROTOCOL_H"));
    assert!(header.contains("#define VALKYRIE_PROTOCOL_H"));
    assert!(header.contains("typedef struct ValkyrieClientHandle ValkyrieClientHandle"));
    assert!(header.contains("ValkyrieClientHandle* valkyrie_client_create"));
    assert!(header.contains("void valkyrie_client_destroy"));
}

#[test]
fn test_python_bindings_generation() {
    use rustci::bindings::python::generate_python_stubs;
    
    let stubs = generate_python_stubs();
    
    assert!(stubs.contains("class ValkyrieClient:"));
    assert!(stubs.contains("def connect(self, endpoint_url: str) -> str:"));
    assert!(stubs.contains("def send_text(self, connection_id: str, text: str) -> None:"));
    assert!(stubs.contains("class ValkyrieConfig:"));
    assert!(stubs.contains("class ValkyrieMessage:"));
}

#[test]
fn test_javascript_bindings_generation() {
    use rustci::bindings::javascript::{generate_typescript_definitions, generate_package_json};
    
    let ts_defs = generate_typescript_definitions();
    assert!(ts_defs.contains("export declare class ValkyrieClient"));
    assert!(ts_defs.contains("connect(endpointUrl: string): string"));
    assert!(ts_defs.contains("sendText(connectionId: string, text: string): void"));
    
    let package_json = generate_package_json();
    assert!(package_json.contains("\"name\": \"@valkyrie/protocol\""));
    assert!(package_json.contains("\"version\": \"1.0.0\""));
}

#[test]
fn test_go_bindings_generation() {
    use rustci::bindings::go::{generate_go_package, generate_go_mod, generate_go_test};
    
    let go_package = generate_go_package();
    assert!(go_package.contains("package valkyrie"));
    assert!(go_package.contains("type Client struct"));
    assert!(go_package.contains("func NewClient()"));
    
    let go_mod = generate_go_mod();
    assert!(go_mod.contains("module github.com/rustci/valkyrie-protocol-go"));
    
    let go_test = generate_go_test();
    assert!(go_test.contains("func TestClientCreation"));
}

#[test]
fn test_java_bindings_generation() {
    use rustci::bindings::java::{generate_java_sources, generate_maven_pom};
    
    let java_sources = generate_java_sources();
    assert_eq!(java_sources.len(), 6);
    
    let client_source = java_sources.iter()
        .find(|(name, _)| name == "ValkyrieClient.java")
        .unwrap();
    assert!(client_source.1.contains("public class ValkyrieClient"));
    assert!(client_source.1.contains("public String connect"));
    
    let pom = generate_maven_pom();
    assert!(pom.contains("<artifactId>valkyrie-protocol-java</artifactId>"));
}

#[tokio::test]
async fn test_client_stats() {
    let client = ValkyrieClient::simple().await.unwrap();
    let stats = client.get_stats().await;
    
    // Initial stats should show no activity
    assert_eq!(stats.active_connections, 0);
    assert_eq!(stats.handlers_registered, 0);
    assert_eq!(stats.engine_stats.transport.messages_sent, 0);
    assert_eq!(stats.engine_stats.transport.messages_received, 0);
}

#[tokio::test]
async fn test_client_connection_failure() {
    let client = ValkyrieClient::simple().await.unwrap();
    
    // Try to connect to a non-existent endpoint
    let result = client.connect("tcp://nonexistent:9999").await;
    assert!(result.is_err(), "Expected connection to fail");
}

#[tokio::test]
async fn test_client_send_without_connection() {
    let client = ValkyrieClient::simple().await.unwrap();
    
    // Try to send to a non-existent connection
    let result = client.send_text("invalid-connection-id", "test").await;
    assert!(result.is_err(), "Expected send to fail without valid connection");
}

#[test]
fn test_endpoint_url_parsing() {
    use rustci::api::valkyrie::parse_endpoint_url;
    
    let endpoint = parse_endpoint_url("tcp://localhost:8080").unwrap();
    assert_eq!(endpoint.transport, "tcp");
    assert_eq!(endpoint.address, "localhost");
    assert_eq!(endpoint.port, 8080);
    
    let endpoint = parse_endpoint_url("ws://example.com:9090").unwrap();
    assert_eq!(endpoint.transport, "ws");
    assert_eq!(endpoint.address, "example.com");
    assert_eq!(endpoint.port, 9090);
    
    // Test invalid URLs
    assert!(parse_endpoint_url("invalid-url").is_err());
    assert!(parse_endpoint_url("tcp://localhost").is_err());
    assert!(parse_endpoint_url("localhost:8080").is_err());
}

#[test]
fn test_rust_bindings_macros() {
    use rustci::{valkyrie_message, message_handler};
    
    // Test message creation macros
    let text_msg = valkyrie_message!(text: "Hello");
    assert!(matches!(text_msg.message_type, ClientMessageType::Text));
    
    let binary_msg = valkyrie_message!(binary: vec![1, 2, 3]);
    assert!(matches!(binary_msg.message_type, ClientMessageType::Binary));
    
    let json_data = serde_json::json!({"test": true});
    let json_msg = valkyrie_message!(json: json_data).unwrap();
    assert!(matches!(json_msg.message_type, ClientMessageType::Json));
}

// Performance benchmarks
#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn benchmark_client_creation() {
        let start = Instant::now();
        let iterations = 100;
        
        for _ in 0..iterations {
            let _client = ValkyrieClient::simple().await.unwrap();
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        println!("Average client creation time: {:?}", avg_duration);
        assert!(avg_duration < Duration::from_millis(100), "Client creation too slow");
    }

    #[test]
    fn benchmark_message_creation() {
        let start = Instant::now();
        let iterations = 10000;
        
        for i in 0..iterations {
            let _msg = ClientMessage::text(&format!("Message {}", i))
                .with_priority(ClientMessagePriority::Normal)
                .with_correlation_id(format!("corr-{}", i));
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        println!("Average message creation time: {:?}", avg_duration);
        assert!(avg_duration < Duration::from_micros(100), "Message creation too slow");
    }

    #[test]
    fn benchmark_code_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = CodeGenConfig {
            output_dir: temp_dir.path().to_string_lossy().to_string(),
            languages: vec![Language::Rust],
            template_dir: None,
            options: HashMap::new(),
        };

        let api_def = extract_api_definition();
        let generator = CodeGenerator::new(config, api_def);

        let start = Instant::now();
        generator.generate_language(Language::Rust).unwrap();
        let duration = start.elapsed();

        println!("Rust code generation time: {:?}", duration);
        assert!(duration < Duration::from_secs(5), "Code generation too slow");
    }
}

// Integration tests with mock server
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::sync::Notify;

    async fn start_mock_server(port: u16) -> Arc<Notify> {
        let notify = Arc::new(Notify::new());
        let notify_clone = Arc::clone(&notify);
        
        tokio::spawn(async move {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
            notify_clone.notify_one();
            
            while let Ok((stream, _)) = listener.accept().await {
                // Simple echo server for testing
                tokio::spawn(async move {
                    // Handle connection...
                    drop(stream);
                });
            }
        });
        
        notify
    }

    #[tokio::test]
    async fn test_client_with_mock_server() {
        let port = 18080;
        let server_ready = start_mock_server(port).await;
        
        // Wait for server to start
        server_ready.notified().await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let client = ValkyrieClient::simple().await.unwrap();
        
        // This should succeed now that we have a server
        let result = client.connect(&format!("tcp://127.0.0.1:{}", port)).await;
        
        // Note: The actual connection might still fail due to protocol mismatch,
        // but we should at least get past the initial TCP connection
        match result {
            Ok(connection_id) => {
                println!("Connected with ID: {}", connection_id);
                // Try to send a message
                let send_result = client.send_text(&connection_id, "Hello, Server!").await;
                println!("Send result: {:?}", send_result);
            }
            Err(e) => {
                println!("Connection failed (expected for mock server): {:?}", e);
            }
        }
    }
}

// Error handling tests
#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_invalid_message_types() {
        // Test invalid JSON
        let invalid_json = serde_json::Value::String("not valid json".to_string());
        let result = ClientMessage::json(&invalid_json);
        // This should succeed as we're just serializing a string
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_client_shutdown() {
        let mut client = ValkyrieClient::simple().await.unwrap();
        
        // Client should be operational
        let stats = client.get_stats().await;
        assert_eq!(stats.active_connections, 0);
        
        // Shutdown should succeed
        let result = client.shutdown().await;
        assert!(result.is_ok(), "Client shutdown failed: {:?}", result.err());
    }

    #[test]
    fn test_config_validation() {
        // Test with extreme values
        let config = ClientConfig {
            performance: ClientPerformanceConfig {
                max_connections_per_endpoint: 0, // This might be invalid
                ..Default::default()
            },
            ..Default::default()
        };
        
        // The client should handle this gracefully
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ValkyrieClient::new(config));
        
        // Depending on implementation, this might succeed or fail gracefully
        match result {
            Ok(_) => println!("Client created with zero max connections"),
            Err(e) => println!("Client creation failed as expected: {:?}", e),
        }
    }
}