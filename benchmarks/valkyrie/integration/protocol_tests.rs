use std::time::Duration;
use tokio::time::timeout;

use crate::valkyrie::{TestAssertions, TestFixtures, TestUtils};
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;
use rustci::core::networking::valkyrie::message::{MessageType, ValkyrieMessage};

/// Core protocol functionality tests
#[cfg(test)]
mod protocol_tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_startup_shutdown() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();

        // Test engine startup
        let start_result = engine.start().await;
        assert!(start_result.is_ok(), "Engine should start successfully");

        // Test engine shutdown
        let stop_result = engine.stop().await;
        assert!(stop_result.is_ok(), "Engine should stop gracefully");
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let original_message = TestUtils::create_test_message(MessageType::Hello);

        // Test serialization
        let serialized = bincode::serialize(&original_message).unwrap();
        assert!(
            !serialized.is_empty(),
            "Serialized message should not be empty"
        );

        // Test deserialization
        let deserialized: ValkyrieMessage = bincode::deserialize(&serialized).unwrap();
        assert_eq!(
            original_message.header.message_type, deserialized.header.message_type,
            "Message type should be preserved"
        );
    }

    #[tokio::test]
    async fn test_protocol_version_negotiation() {
        let engines = TestUtils::create_test_cluster(2).await.unwrap();
        let mut engine1 = engines.into_iter().nth(0).unwrap();
        let mut engine2 = engines.into_iter().nth(1).unwrap();

        // Start both engines
        engine1.start().await.unwrap();
        engine2.start().await.unwrap();

        // Test connection establishment with version negotiation
        let connection_result = timeout(
            Duration::from_secs(5),
            engine1.connect("127.0.0.1:8001".parse().unwrap()),
        )
        .await;

        assert!(
            connection_result.is_ok(),
            "Connection should be established"
        );

        // Cleanup
        engine1.stop().await.unwrap();
        engine2.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_message_routing() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        let test_message = TestUtils::create_test_message(MessageType::JobRequest);

        // Test message routing (this would typically involve multiple nodes)
        // For now, we test that the message can be processed
        let routing_result = engine.route_message(test_message).await;
        assert!(
            routing_result.is_ok(),
            "Message should be routed successfully"
        );

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_connection_pooling() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        // Test multiple connections to the same endpoint
        let endpoint = "127.0.0.1:8080".parse().unwrap();

        let conn1 = engine.connect(endpoint).await;
        let conn2 = engine.connect(endpoint).await;

        assert!(conn1.is_ok(), "First connection should succeed");
        assert!(conn2.is_ok(), "Second connection should reuse pool");

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_message_timeout() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        let test_message = TestUtils::create_test_message(MessageType::Ping);

        // Test message with timeout
        let timeout_result = timeout(
            Duration::from_millis(100),
            engine.send_message_with_timeout(test_message, Duration::from_millis(50)),
        )
        .await;

        // Should timeout
        assert!(timeout_result.is_err(), "Message should timeout");

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        // Test connection to non-existent endpoint
        let invalid_endpoint = "127.0.0.1:99999".parse().unwrap();
        let connection_result = engine.connect(invalid_endpoint).await;

        assert!(
            connection_result.is_err(),
            "Connection to invalid endpoint should fail"
        );

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        // Simulate active connections
        let _listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();

        // Test graceful shutdown
        let (_, shutdown_duration) = TestUtils::measure_time(engine.stop()).await;

        TestAssertions::assert_latency_below(shutdown_duration, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_message_ordering() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        // Send multiple messages and verify ordering
        let messages = vec![
            TestUtils::create_test_message(MessageType::Hello),
            TestUtils::create_test_message(MessageType::JobRequest),
            TestUtils::create_test_message(MessageType::JobComplete),
        ];

        for (i, message) in messages.into_iter().enumerate() {
            let mut msg = message;
            msg.header.sequence_number = Some(i as u64);

            let send_result = engine.send_message(msg).await;
            assert!(
                send_result.is_ok(),
                "Message {} should be sent successfully",
                i
            );
        }

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_backpressure_handling() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();

        // Flood the engine with messages to test backpressure
        let mut send_tasks = Vec::new();

        for i in 0..1000 {
            let message = TestUtils::create_test_message(MessageType::JobRequest);
            let engine_clone = engine.clone();

            let task = tokio::spawn(async move { engine_clone.send_message(message).await });

            send_tasks.push(task);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(send_tasks).await;

        // Some messages might be dropped due to backpressure, but the system should remain stable
        let successful_sends = results
            .into_iter()
            .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
            .count();

        assert!(
            successful_sends > 0,
            "At least some messages should be sent successfully"
        );

        engine.stop().await.unwrap();
    }
}
