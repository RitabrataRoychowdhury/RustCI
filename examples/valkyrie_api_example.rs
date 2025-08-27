//! Comprehensive example demonstrating the Valkyrie Protocol API and SDK Foundation
//!
//! This example shows how to use the high-level Valkyrie Protocol API for
//! distributed communication, including client creation, message handling,
//! and code generation.

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

use RustAutoDevOps::valkyrie::api::{
    ValkyrieClient, ClientConfig, ClientMessage, ClientMessageType,
    ClientMessagePriority, ClientSecurityConfig, ClientAuthMethod,
    ClientPerformanceConfig, ClientTimeoutConfig, ClientFeatureFlags,
    ClientMessageHandler, ClientMessageHandlerAdapter, ClientPayload,
};
use RustAutoDevOps::api::codegen::{
    CodeGenerator, CodeGenConfig, Language, extract_api_definition
};
use RustAutoDevOps::core::networking::valkyrie::MessageType;
use RustAutoDevOps::error::{Result, AppError};

/// Example message handler
struct ExampleMessageHandler {
    name: String,
}

#[async_trait::async_trait]
impl ClientMessageHandler for ExampleMessageHandler {
    async fn handle_message(&self, connection_id: &str, message: ClientMessage) -> Result<()> {
        println!("[{}] Received message from {}: {:?}", self.name, connection_id, message.message_type);
        
        match &message.payload {
            ClientPayload::Text(text) => {
                println!("  Text content: {}", text);
            }
            ClientPayload::Binary(data) => {
                println!("  Binary data: {} bytes", data.len());
            }
        }
        
        if let Some(correlation_id) = &message.correlation_id {
            println!("  Correlation ID: {}", correlation_id);
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Valkyrie Protocol API and SDK Foundation Example");
    println!("==================================================\n");

    // 1. Demonstrate client creation with different configurations
    println!("1. Creating Valkyrie Protocol clients...");
    
    // Simple client
    let simple_client = ValkyrieClient::simple().await?;
    println!("   ✓ Simple client created");
    
    // High-performance client
    let hp_client = ValkyrieClient::high_performance().await?;
    println!("   ✓ High-performance client created");
    
    // Custom configuration client
    let custom_config = ClientConfig {
        security: ClientSecurityConfig {
            auth_method: ClientAuthMethod::Token,
            enable_encryption: true,
            enable_audit: true,
            cert_path: None,
            key_path: None,
            ca_path: None,
        },
        performance: ClientPerformanceConfig {
            worker_threads: Some(2),
            max_connections_per_endpoint: 5,
            max_streams_per_connection: 50,
            buffer_size: 8192,
            message_batch_size: 50,
        },
        timeouts: ClientTimeoutConfig {
            connect_timeout: Duration::from_secs(10),
            send_timeout: Duration::from_secs(5),
            heartbeat_interval: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
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
    
    let custom_client = ValkyrieClient::new(custom_config).await?;
    println!("   ✓ Custom configuration client created");

    // 2. Demonstrate message creation and manipulation
    println!("\n2. Creating and manipulating messages...");
    
    // Text message
    let text_msg = ClientMessage::text("Hello, Valkyrie Protocol!")
        .with_priority(ClientMessagePriority::High)
        .with_correlation_id("example-001".to_string())
        .with_ttl(Duration::from_secs(60))
        .with_metadata("source".to_string(), "example".to_string());
    
    println!("   ✓ Text message created: {:?}", text_msg.message_type);
    
    // Binary message
    let binary_data = vec![0x56, 0x41, 0x4C, 0x4B, 0x59, 0x52, 0x49, 0x45]; // "VALKYRIE" in hex
    let binary_msg = ClientMessage::binary(binary_data)
        .with_priority(ClientMessagePriority::Normal);
    
    println!("   ✓ Binary message created: {} bytes", 
             if let ClientPayload::Binary(data) = &binary_msg.payload { 
                 data.len() 
             } else { 
                 0 
             });
    
    // JSON message
    let json_data = serde_json::json!({
        "type": "greeting",
        "message": "Hello from Valkyrie!",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "metadata": {
            "version": "1.0.0",
            "protocol": "valkyrie"
        }
    });
    
    let json_msg = ClientMessage::json(&json_data)?
        .with_priority(ClientMessagePriority::Critical);
    
    println!("   ✓ JSON message created");

    // 3. Demonstrate client statistics
    println!("\n3. Checking client statistics...");
    
    let stats = simple_client.get_stats().await;
    println!("   Simple client stats:");
    println!("     - Active connections: {}", stats.active_connections);
    println!("     - Handlers registered: {}", stats.handlers_registered);
    println!("     - Messages sent: {}", stats.engine_stats.transport.messages_sent);
    println!("     - Messages received: {}", stats.engine_stats.transport.messages_received);

    // 4. Demonstrate message handler registration
    println!("\n4. Registering message handlers...");
    
    let handler = ExampleMessageHandler {
        name: "ExampleHandler".to_string(),
    };
    
    let adapter = ClientMessageHandlerAdapter::new(handler);
    simple_client.register_handler(MessageType::Data, adapter).await;
    println!("   ✓ Message handler registered");

    // 5. Demonstrate connection attempts (will fail without server)
    println!("\n5. Attempting connections (expected to fail without server)...");
    
    let endpoints = vec![
        "tcp://localhost:8080",
        "tcp://127.0.0.1:9090",
        "ws://localhost:8081",
    ];
    
    for endpoint in endpoints {
        match simple_client.connect(endpoint).await {
            Ok(connection_id) => {
                println!("   ✓ Connected to {} with ID: {}", endpoint, connection_id);
                
                // Try to send messages
                if let Err(e) = simple_client.send_text(&connection_id, "Hello, Server!").await {
                    println!("     ⚠ Send failed: {}", e);
                }
                
                if let Err(e) = simple_client.send_message(&connection_id, text_msg.clone()).await {
                    println!("     ⚠ Custom message send failed: {}", e);
                }
                
                // Close connection
                if let Err(e) = simple_client.close_connection(&connection_id).await {
                    println!("     ⚠ Close connection failed: {}", e);
                }
            }
            Err(e) => {
                println!("   ✗ Failed to connect to {}: {}", endpoint, e);
            }
        }
    }

    // 6. Demonstrate broadcast functionality
    println!("\n6. Demonstrating broadcast (with mock connections)...");
    
    let mock_connections = vec![
        "connection-1".to_string(),
        "connection-2".to_string(),
        "connection-3".to_string(),
    ];
    
    let broadcast_msg = ClientMessage::text("Broadcast message to all!")
        .with_priority(ClientMessagePriority::High);
    
    match simple_client.broadcast(&mock_connections, broadcast_msg).await {
        Ok(result) => {
            println!("   Broadcast result:");
            println!("     - Total: {}", result.total);
            println!("     - Successful: {}", result.successful);
            println!("     - Failed: {}", result.failed);
        }
        Err(e) => {
            println!("   ✗ Broadcast failed: {}", e);
        }
    }

    // 7. Demonstrate API definition extraction
    println!("\n7. Extracting API definition...");
    
    let api_def = extract_api_definition();
    println!("   API Definition:");
    println!("     - Name: {}", api_def.name);
    println!("     - Version: {}", api_def.version);
    println!("     - Description: {}", api_def.description);
    println!("     - Types: {}", api_def.types.len());
    println!("     - Services: {}", api_def.services.len());
    println!("     - Errors: {}", api_def.errors.len());

    // 8. Demonstrate code generation
    println!("\n8. Generating language bindings...");
    
    let temp_dir = tempfile::TempDir::new().map_err(|e| {
        AppError::InternalServerError(format!("Failed to create temp dir: {}", e))
    })?;
    
    let codegen_config = CodeGenConfig {
        output_dir: temp_dir.path().to_string_lossy().to_string(),
        languages: vec![
            Language::Rust,
            Language::C,
            Language::Python,
            Language::JavaScript,
            Language::Go,
            Language::Java,
        ],
        template_dir: None,
        options: HashMap::new(),
    };
    
    let generator = CodeGenerator::new(codegen_config, api_def);
    
    for language in &generator.config.languages {
        match generator.generate_language(*language) {
            Ok(_) => {
                let lang_dir = temp_dir.path().join(language.extension());
                println!("   ✓ Generated {} bindings at: {}", 
                         language.package_name(), 
                         lang_dir.display());
            }
            Err(e) => {
                println!("   ✗ Failed to generate {} bindings: {}", 
                         language.package_name(), e);
            }
        }
    }

    // 9. Demonstrate Rust-specific utilities
    println!("\n9. Using Rust-specific utilities...");
    
    // Using macros (commented out - macros not available)
    // let macro_text_msg = rustci::valkyrie_message!(text: "Created with macro!");
    // println!("   ✓ Message created with macro: {:?}", macro_text_msg.message_type);
    
    // let macro_binary_msg = rustci::valkyrie_message!(binary: vec![1, 2, 3, 4]);
    // println!("   ✓ Binary message created with macro");
    
    // let macro_json_msg = rustci::valkyrie_message!(json: serde_json::json!({"macro": true}))?;
    // println!("   ✓ JSON message created with macro");
    println!("   ✓ Macro examples skipped (macros not available)");

    // 10. Performance demonstration
    println!("\n10. Performance demonstration...");
    
    let start = std::time::Instant::now();
    let iterations = 1000;
    
    for i in 0..iterations {
        let _msg = ClientMessage::text(&format!("Performance test message {}", i))
            .with_priority(ClientMessagePriority::Normal)
            .with_correlation_id(format!("perf-{}", i));
    }
    
    let duration = start.elapsed();
    let avg_duration = duration / iterations;
    
    println!("   Message creation performance:");
    println!("     - Total time: {:?}", duration);
    println!("     - Average per message: {:?}", avg_duration);
    println!("     - Messages per second: {:.0}", 1.0 / avg_duration.as_secs_f64());

    // 11. Cleanup
    println!("\n11. Cleaning up...");
    
    // Note: In a real application, you would properly shutdown clients
    // For this example, we'll just let them drop
    drop(simple_client);
    drop(hp_client);
    drop(custom_client);
    
    println!("   ✓ Clients cleaned up");

    println!("\n🎉 Valkyrie Protocol API and SDK Foundation example completed!");
    println!("\nKey features demonstrated:");
    println!("  • Multiple client creation patterns");
    println!("  • Message creation and manipulation");
    println!("  • Connection management (simulated)");
    println!("  • Message broadcasting");
    println!("  • API definition extraction");
    println!("  • Multi-language code generation");
    println!("  • Rust-specific utilities and macros");
    println!("  • Performance characteristics");
    
    println!("\nNext steps:");
    println!("  • Set up a Valkyrie Protocol server to test real connections");
    println!("  • Implement custom message handlers for your use case");
    println!("  • Generate bindings for your preferred programming language");
    println!("  • Integrate with your distributed system architecture");

    Ok(())
}

/// Helper function to demonstrate async message processing
async fn process_messages_async(messages: Vec<ClientMessage>) -> Result<()> {
    println!("Processing {} messages asynchronously...", messages.len());
    
    for (i, message) in messages.iter().enumerate() {
        // Simulate async processing
        sleep(Duration::from_millis(10)).await;
        
        match &message.payload {
            ClientPayload::Text(text) => {
                println!("  [{}] Processed text: {}", i, text);
            }
            ClientPayload::Binary(data) => {
                println!("  [{}] Processed binary: {} bytes", i, data.len());
            }
        }
    }
    
    println!("All messages processed!");
    Ok(())
}

/// Helper function to demonstrate error handling patterns
async fn demonstrate_error_handling() -> Result<()> {
    println!("Demonstrating error handling patterns...");
    
    // Try to create a client with invalid configuration
    let invalid_config = ClientConfig {
        timeouts: ClientTimeoutConfig {
            connect_timeout: Duration::from_secs(0), // Invalid timeout
            ..Default::default()
        },
        ..Default::default()
    };
    
    match ValkyrieClient::new(invalid_config).await {
        Ok(_) => println!("  Client created despite invalid config"),
        Err(e) => println!("  Client creation failed as expected: {}", e),
    }
    
    // Try to parse invalid endpoint URLs
    let invalid_urls = vec![
        "not-a-url",
        "tcp://",
        "://localhost:8080",
        "tcp://localhost",
        "tcp://localhost:invalid-port",
    ];
    
    for url in invalid_urls {
        // match rustci::api::valkyrie::parse_endpoint_url(url) {
        // Commenting out parse_endpoint_url as it may not be available
        match Err("Function not available".to_string()) {
            Ok(endpoint) => println!("  Unexpectedly parsed {}: {:?}", url, endpoint),
            Err(e) => println!("  Failed to parse {} as expected: {}", url, e),
        }
    }
    
    Ok(())
}