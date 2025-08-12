use std::time::Duration;
use tokio::time::timeout;

use crate::valkyrie::{TestUtils, TestFixtures, TestAssertions};
use rustci::core::networking::valkyrie::transport::{TransportType, Transport};
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;

/// Multi-transport compatibility tests
#[cfg(test)]
mod transport_tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_transport() {
        let config = TestFixtures::minimal_config();
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        
        engine.start().await.unwrap();
        
        // Test TCP connection
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = timeout(
            Duration::from_secs(5),
            engine.connect(bind_addr)
        ).await.unwrap().unwrap();
        
        assert!(connection.is_connected(), "TCP connection should be established");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_websocket_transport() {
        let mut config = TestFixtures::minimal_config();
        config.transport_type = TransportType::WebSocket;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test WebSocket connection
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = timeout(
            Duration::from_secs(5),
            engine.connect(bind_addr)
        ).await.unwrap().unwrap();
        
        assert!(connection.is_connected(), "WebSocket connection should be established");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_quic_transport() {
        let mut config = TestFixtures::secure_config();
        config.transport_type = TransportType::Quic;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test QUIC connection
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = timeout(
            Duration::from_secs(5),
            engine.connect(bind_addr)
        ).await.unwrap().unwrap();
        
        assert!(connection.is_connected(), "QUIC connection should be established");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_unix_socket_transport() {
        #[cfg(unix)]
        {
            let mut config = TestFixtures::minimal_config();
            config.transport_type = TransportType::UnixSocket;
            config.unix_socket_path = Some("/tmp/valkyrie-test.sock".to_string());
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            
            // Test Unix socket connection
            let connection = timeout(
                Duration::from_secs(5),
                engine.connect_unix("/tmp/valkyrie-test.sock")
            ).await.unwrap().unwrap();
            
            assert!(connection.is_connected(), "Unix socket connection should be established");
            
            engine.stop().await.unwrap();
            
            // Cleanup
            let _ = std::fs::remove_file("/tmp/valkyrie-test.sock");
        }
    }

    #[tokio::test]
    async fn test_transport_failover() {
        let mut config = TestFixtures::minimal_config();
        config.fallback_transports = vec![
            TransportType::Tcp,
            TransportType::WebSocket,
        ];
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Test automatic failover when primary transport fails
        let endpoint = "127.0.0.1:99999".parse().unwrap(); // Invalid port
        
        let connection_result = timeout(
            Duration::from_secs(10),
            engine.connect_with_failover(endpoint)
        ).await;
        
        // Should eventually succeed with fallback transport
        assert!(connection_result.is_ok(), "Failover should succeed");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_transport_performance_comparison() {
        let transport_types = vec![
            TransportType::Tcp,
            TransportType::WebSocket,
            TransportType::Quic,
        ];
        
        let mut results = Vec::new();
        
        for transport_type in transport_types {
            let mut config = TestFixtures::minimal_config();
            config.transport_type = transport_type;
            
            if transport_type == TransportType::Quic {
                config.security_enabled = true;
            }
            
            let mut engine = ValkyrieEngine::new(config).await.unwrap();
            engine.start().await.unwrap();
            
            // Measure connection establishment time
            let (_, connection_time) = TestUtils::measure_time(async {
                let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
                let bind_addr = listener.local_addr().unwrap();
                engine.connect(bind_addr).await
            }).await;
            
            results.push((transport_type, connection_time));
            
            engine.stop().await.unwrap();
        }
        
        // Log performance comparison
        for (transport, time) in results {
            println!("Transport {:?}: connection time {:?}", transport, time);
        }
    }

    #[tokio::test]
    async fn test_transport_message_sizes() {
        let message_sizes = vec![1024, 64 * 1024, 1024 * 1024]; // 1KB, 64KB, 1MB
        
        for size in message_sizes {
            let mut engine = TestUtils::create_test_engine().await.unwrap();
            engine.start().await.unwrap();
            
            let test_data = TestUtils::generate_test_data(size);
            let mut message = TestUtils::create_test_message(rustci::core::networking::valkyrie::message::MessageType::JobRequest);
            message.payload = test_data;
            
            let (_, send_time) = TestUtils::measure_time(async {
                engine.send_message(message).await
            }).await;
            
            println!("Message size {}: send time {:?}", size, send_time);
            
            // Assert reasonable performance for different message sizes
            match size {
                1024 => TestAssertions::assert_latency_below(send_time, Duration::from_millis(10)),
                65536 => TestAssertions::assert_latency_below(send_time, Duration::from_millis(100)),
                1048576 => TestAssertions::assert_latency_below(send_time, Duration::from_secs(1)),
                _ => {}
            }
            
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_concurrent_connections() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        // Test multiple concurrent connections
        let connection_count = 100;
        let mut connection_tasks = Vec::new();
        
        for _ in 0..connection_count {
            let engine_clone = engine.clone();
            let task = tokio::spawn(async move {
                engine_clone.connect(bind_addr).await
            });
            connection_tasks.push(task);
        }
        
        let results = futures::future::join_all(connection_tasks).await;
        let successful_connections = results.into_iter()
            .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
            .count();
        
        assert!(
            successful_connections >= connection_count / 2,
            "At least half of concurrent connections should succeed"
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_transport_compression() {
        let mut config = TestFixtures::minimal_config();
        config.enable_compression = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        // Create a large, compressible message
        let compressible_data = "A".repeat(10000).into_bytes();
        let mut message = TestUtils::create_test_message(rustci::core::networking::valkyrie::message::MessageType::JobRequest);
        message.payload = compressible_data;
        
        let (_, send_time) = TestUtils::measure_time(async {
            engine.send_message(message).await
        }).await;
        
        // Compressed message should be sent relatively quickly
        TestAssertions::assert_latency_below(send_time, Duration::from_millis(100));
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_transport_multiplexing() {
        let mut config = TestFixtures::minimal_config();
        config.enable_multiplexing = true;
        
        let mut engine = ValkyrieEngine::new(config).await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Send multiple messages concurrently over the same connection
        let message_count = 10;
        let mut send_tasks = Vec::new();
        
        for i in 0..message_count {
            let mut message = TestUtils::create_test_message(rustci::core::networking::valkyrie::message::MessageType::JobRequest);
            message.header.stream_id = uuid::Uuid::new_v4();
            
            let connection_clone = connection.clone();
            let task = tokio::spawn(async move {
                connection_clone.send_message(message).await
            });
            send_tasks.push(task);
        }
        
        let results = futures::future::join_all(send_tasks).await;
        let successful_sends = results.into_iter()
            .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
            .count();
        
        assert_eq!(successful_sends, message_count, "All multiplexed messages should be sent");
        
        engine.stop().await.unwrap();
    }
}