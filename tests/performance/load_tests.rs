use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use RustAutoDevOps::core::node_communication::{
    ControlPlaneMessage, ExecutionContext, JobDefinition, JobPriority, MessagePayload,
    NodeCapabilities, NodeInfo, NodeMessage, NodeMetrics, NodeResources, NodeStatus, NodeType,
    ProtocolMessage, RunnerType,
};
use RustAutoDevOps::core::node_communication_manager::{
    CommunicationConfig, NodeCommunicationManager,
};
use RustAutoDevOps::core::secure_transport::{AuthenticationManager, EncryptionManager};
use RustAutoDevOps::core::transport::{TcpTransport, Transport, TransportConfig, TransportType};

#[tokio::test]
async fn test_high_throughput_message_processing() {
    let message_count = 10000;
    let batch_size = 100;

    let start_time = Instant::now();
    let mut processed_messages = 0;

    for batch in 0..(message_count / batch_size) {
        let mut batch_messages = Vec::new();

        for i in 0..batch_size {
            let message = create_test_heartbeat_message();
            batch_messages.push(message);
        }

        // Process batch
        for message in batch_messages {
            let serialized = serde_json::to_string(&message).unwrap();
            let _deserialized: ProtocolMessage = serde_json::from_str(&serialized).unwrap();
            processed_messages += 1;
        }

        // Small yield to prevent blocking
        if batch % 10 == 0 {
            tokio::task::yield_now().await;
        }
    }

    let duration = start_time.elapsed();
    let messages_per_second = processed_messages as f64 / duration.as_secs_f64();

    println!(
        "Processed {} messages in {:?}",
        processed_messages, duration
    );
    println!("Throughput: {:.2} messages/second", messages_per_second);

    // Assert minimum performance threshold
    assert!(
        messages_per_second > 5000.0,
        "Throughput too low: {:.2} msg/s",
        messages_per_second
    );
    assert!(
        duration < Duration::from_secs(5),
        "Processing took too long: {:?}",
        duration
    );
}

#[tokio::test]
async fn test_concurrent_authentication_load() {
    let auth_manager = Arc::new(AuthenticationManager::new(
        "load-test-secret-key-that-is-sufficiently-long",
        Duration::from_secs(3600),
    ));

    let concurrent_operations = 1000;
    let semaphore = Arc::new(Semaphore::new(100)); // Limit concurrent operations

    let start_time = Instant::now();
    let mut handles = Vec::new();

    for i in 0..concurrent_operations {
        let auth_manager = auth_manager.clone();
        let semaphore = semaphore.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let node_id = Uuid::new_v4();

            // Generate token
            let token_start = Instant::now();
            let token = auth_manager
                .generate_token(
                    node_id,
                    format!("worker-{}", i),
                    vec!["native".to_string(), "docker".to_string()],
                )
                .await
                .unwrap();
            let token_duration = token_start.elapsed();

            // Verify token
            let verify_start = Instant::now();
            let claims = auth_manager.verify_token(&token).await.unwrap();
            let verify_duration = verify_start.elapsed();

            assert_eq!(claims.node_id, node_id);

            (token_duration, verify_duration)
        });

        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    let total_duration = start_time.elapsed();

    // Analyze results
    let mut token_times = Vec::new();
    let mut verify_times = Vec::new();
    let mut successful_operations = 0;

    for result in results {
        match result {
            Ok((token_time, verify_time)) => {
                token_times.push(token_time);
                verify_times.push(verify_time);
                successful_operations += 1;
            }
            Err(e) => {
                eprintln!("Operation failed: {:?}", e);
            }
        }
    }

    // Calculate statistics
    let avg_token_time = token_times.iter().sum::<Duration>() / token_times.len() as u32;
    let avg_verify_time = verify_times.iter().sum::<Duration>() / verify_times.len() as u32;
    let operations_per_second = successful_operations as f64 / total_duration.as_secs_f64();

    println!("Concurrent authentication load test results:");
    println!("  Total operations: {}", concurrent_operations);
    println!("  Successful operations: {}", successful_operations);
    println!("  Total duration: {:?}", total_duration);
    println!("  Operations per second: {:.2}", operations_per_second);
    println!("  Average token generation time: {:?}", avg_token_time);
    println!("  Average token verification time: {:?}", avg_verify_time);

    // Performance assertions
    assert_eq!(
        successful_operations, concurrent_operations,
        "All operations should succeed"
    );
    assert!(
        operations_per_second > 100.0,
        "Should handle at least 100 ops/sec"
    );
    assert!(
        avg_token_time < Duration::from_millis(50),
        "Token generation should be fast"
    );
    assert!(
        avg_verify_time < Duration::from_millis(20),
        "Token verification should be fast"
    );
}

#[tokio::test]
async fn test_memory_usage_under_load() {
    let auth_manager = Arc::new(AuthenticationManager::new(
        "memory-test-secret",
        Duration::from_secs(3600),
    ));

    // Generate a large number of tokens and keep them in memory
    let token_count = 5000;
    let mut tokens = Vec::with_capacity(token_count);

    let start_time = Instant::now();

    for i in 0..token_count {
        let token = auth_manager
            .generate_token(
                Uuid::new_v4(),
                format!("worker-{}", i),
                vec!["native".to_string()],
            )
            .await
            .unwrap();

        tokens.push(token);

        // Yield occasionally to prevent blocking
        if i % 100 == 0 {
            tokio::task::yield_now().await;
        }
    }

    let generation_duration = start_time.elapsed();

    // Now verify all tokens
    let verify_start = Instant::now();
    let mut successful_verifications = 0;

    for (i, token) in tokens.iter().enumerate() {
        match auth_manager.verify_token(token).await {
            Ok(_) => successful_verifications += 1,
            Err(e) => eprintln!("Token {} verification failed: {:?}", i, e),
        }

        if i % 100 == 0 {
            tokio::task::yield_now().await;
        }
    }

    let verification_duration = verify_start.elapsed();
    let total_duration = start_time.elapsed();

    println!("Memory usage test results:");
    println!(
        "  Generated {} tokens in {:?}",
        token_count, generation_duration
    );
    println!(
        "  Verified {} tokens in {:?}",
        successful_verifications, verification_duration
    );
    println!("  Total duration: {:?}", total_duration);
    println!("  Memory held {} tokens simultaneously", tokens.len());

    // Performance assertions
    assert_eq!(
        successful_verifications, token_count,
        "All tokens should verify successfully"
    );
    assert!(
        generation_duration < Duration::from_secs(30),
        "Token generation should complete quickly"
    );
    assert!(
        verification_duration < Duration::from_secs(20),
        "Token verification should complete quickly"
    );

    // Clean up
    tokens.clear();
}

#[tokio::test]
async fn test_message_size_performance() {
    let sizes = vec![
        ("small", 100),        // 100 bytes
        ("medium", 10_000),    // 10KB
        ("large", 100_000),    // 100KB
        ("xlarge", 1_000_000), // 1MB
    ];

    for (size_name, size_bytes) in sizes {
        let large_data = "x".repeat(size_bytes);

        let message = ProtocolMessage::new(
            Uuid::new_v4(),
            MessagePayload::NodeMessage(NodeMessage::RegisterNode {
                node_info: NodeInfo {
                    hostname: large_data,
                    ip_address: "192.168.1.100".to_string(),
                    port: 8080,
                    node_type: NodeType::Worker,
                    version: "1.0.0".to_string(),
                    platform: "linux".to_string(),
                    architecture: "x86_64".to_string(),
                    tags: HashMap::new(),
                },
                capabilities: NodeCapabilities {
                    runner_types: vec![RunnerType::Native],
                    max_resources: Default::default(),
                    supported_job_types: vec!["build".to_string()],
                    features: vec![],
                    protocols: vec!["tcp".to_string()],
                },
                auth_token: "test-token".to_string(),
            }),
        );

        // Test serialization performance
        let serialize_start = Instant::now();
        let serialized = serde_json::to_string(&message).unwrap();
        let serialize_duration = serialize_start.elapsed();

        // Test deserialization performance
        let deserialize_start = Instant::now();
        let _deserialized: ProtocolMessage = serde_json::from_str(&serialized).unwrap();
        let deserialize_duration = deserialize_start.elapsed();

        let actual_size = serialized.len();
        let throughput_serialize =
            actual_size as f64 / serialize_duration.as_secs_f64() / 1_000_000.0; // MB/s
        let throughput_deserialize =
            actual_size as f64 / deserialize_duration.as_secs_f64() / 1_000_000.0; // MB/s

        println!("Message size performance ({}):", size_name);
        println!("  Target size: {} bytes", size_bytes);
        println!("  Actual serialized size: {} bytes", actual_size);
        println!(
            "  Serialize time: {:?} ({:.2} MB/s)",
            serialize_duration, throughput_serialize
        );
        println!(
            "  Deserialize time: {:?} ({:.2} MB/s)",
            deserialize_duration, throughput_deserialize
        );

        // Performance assertions (adjust thresholds as needed)
        match size_name {
            "small" => {
                assert!(
                    serialize_duration < Duration::from_micros(100),
                    "Small message serialization too slow"
                );
                assert!(
                    deserialize_duration < Duration::from_micros(200),
                    "Small message deserialization too slow"
                );
            }
            "medium" => {
                assert!(
                    serialize_duration < Duration::from_millis(5),
                    "Medium message serialization too slow"
                );
                assert!(
                    deserialize_duration < Duration::from_millis(10),
                    "Medium message deserialization too slow"
                );
            }
            "large" => {
                assert!(
                    serialize_duration < Duration::from_millis(50),
                    "Large message serialization too slow"
                );
                assert!(
                    deserialize_duration < Duration::from_millis(100),
                    "Large message deserialization too slow"
                );
            }
            "xlarge" => {
                assert!(
                    serialize_duration < Duration::from_millis(500),
                    "XLarge message serialization too slow"
                );
                assert!(
                    deserialize_duration < Duration::from_millis(1000),
                    "XLarge message deserialization too slow"
                );
            }
            _ => {}
        }

        // Throughput should be reasonable
        assert!(
            throughput_serialize > 1.0,
            "Serialization throughput too low: {:.2} MB/s",
            throughput_serialize
        );
        assert!(
            throughput_deserialize > 0.5,
            "Deserialization throughput too low: {:.2} MB/s",
            throughput_deserialize
        );
    }
}

#[tokio::test]
async fn test_sustained_load_performance() {
    let duration = Duration::from_secs(10); // Run for 10 seconds
    let target_rate = 1000; // Target 1000 operations per second
    let interval = Duration::from_millis(1000 / target_rate);

    let auth_manager = Arc::new(AuthenticationManager::new(
        "sustained-load-test-secret",
        Duration::from_secs(3600),
    ));

    let (tx, mut rx) = mpsc::unbounded_channel();
    let start_time = Instant::now();

    // Spawn producer task
    let producer_auth_manager = auth_manager.clone();
    let producer_tx = tx.clone();
    let producer_handle = tokio::spawn(async move {
        let mut operation_count = 0;
        let mut last_report = Instant::now();

        while start_time.elapsed() < duration {
            let operation_start = Instant::now();

            // Perform authentication operation
            let node_id = Uuid::new_v4();
            let token = producer_auth_manager
                .generate_token(
                    node_id,
                    format!("worker-{}", operation_count),
                    vec!["native".to_string()],
                )
                .await
                .unwrap();

            let _claims = producer_auth_manager.verify_token(&token).await.unwrap();

            let operation_duration = operation_start.elapsed();
            producer_tx
                .send((operation_count, operation_duration))
                .unwrap();

            operation_count += 1;

            // Report progress every second
            if last_report.elapsed() >= Duration::from_secs(1) {
                println!("Completed {} operations", operation_count);
                last_report = Instant::now();
            }

            // Rate limiting
            if operation_duration < interval {
                sleep(interval - operation_duration).await;
            }
        }

        operation_count
    });

    // Collect results
    let mut operation_times = Vec::new();
    let mut total_operations = 0;

    while let Some((op_id, op_duration)) = rx.recv().await {
        operation_times.push(op_duration);
        total_operations = op_id + 1;

        if start_time.elapsed() >= duration {
            break;
        }
    }

    let final_count = producer_handle.await.unwrap();
    let total_duration = start_time.elapsed();

    // Calculate statistics
    let actual_rate = final_count as f64 / total_duration.as_secs_f64();
    let avg_operation_time =
        operation_times.iter().sum::<Duration>() / operation_times.len() as u32;

    // Calculate percentiles
    operation_times.sort();
    let p50 = operation_times[operation_times.len() / 2];
    let p95 = operation_times[operation_times.len() * 95 / 100];
    let p99 = operation_times[operation_times.len() * 99 / 100];

    println!("Sustained load test results:");
    println!("  Target rate: {} ops/sec", target_rate);
    println!("  Actual rate: {:.2} ops/sec", actual_rate);
    println!("  Total operations: {}", final_count);
    println!("  Test duration: {:?}", total_duration);
    println!("  Average operation time: {:?}", avg_operation_time);
    println!("  P50 operation time: {:?}", p50);
    println!("  P95 operation time: {:?}", p95);
    println!("  P99 operation time: {:?}", p99);

    // Performance assertions
    assert!(
        actual_rate > target_rate as f64 * 0.8,
        "Should achieve at least 80% of target rate"
    );
    assert!(
        avg_operation_time < Duration::from_millis(10),
        "Average operation time should be reasonable"
    );
    assert!(
        p95 < Duration::from_millis(50),
        "95th percentile should be reasonable"
    );
    assert!(
        p99 < Duration::from_millis(100),
        "99th percentile should be reasonable"
    );
}

#[tokio::test]
async fn test_connection_scaling() {
    let connection_counts = vec![10, 50, 100, 500];

    for connection_count in connection_counts {
        println!("Testing with {} concurrent connections", connection_count);

        let auth_manager = Arc::new(AuthenticationManager::new(
            "connection-scaling-test-secret",
            Duration::from_secs(3600),
        ));

        let start_time = Instant::now();
        let mut handles = Vec::new();

        // Simulate multiple concurrent connections
        for i in 0..connection_count {
            let auth_manager = auth_manager.clone();

            let handle = tokio::spawn(async move {
                let node_id = Uuid::new_v4();
                let mut successful_operations = 0;
                let operation_start = Instant::now();

                // Each "connection" performs multiple operations
                for j in 0..10 {
                    let token = auth_manager
                        .generate_token(
                            node_id,
                            format!("worker-{}-{}", i, j),
                            vec!["native".to_string()],
                        )
                        .await
                        .unwrap();

                    let _claims = auth_manager.verify_token(&token).await.unwrap();
                    successful_operations += 1;

                    // Small delay to simulate real usage
                    sleep(Duration::from_millis(1)).await;
                }

                (successful_operations, operation_start.elapsed())
            });

            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        let total_duration = start_time.elapsed();

        // Analyze results
        let mut total_operations = 0;
        let mut total_operation_time = Duration::ZERO;

        for result in results {
            match result {
                Ok((ops, duration)) => {
                    total_operations += ops;
                    total_operation_time += duration;
                }
                Err(e) => {
                    eprintln!("Connection failed: {:?}", e);
                }
            }
        }

        let operations_per_second = total_operations as f64 / total_duration.as_secs_f64();
        let avg_connection_time = total_operation_time / connection_count as u32;

        println!("  Results for {} connections:", connection_count);
        println!("    Total operations: {}", total_operations);
        println!("    Total duration: {:?}", total_duration);
        println!("    Operations per second: {:.2}", operations_per_second);
        println!("    Average connection time: {:?}", avg_connection_time);

        // Performance assertions
        assert_eq!(
            total_operations,
            connection_count * 10,
            "All operations should succeed"
        );
        assert!(
            operations_per_second > 100.0,
            "Should maintain reasonable throughput"
        );
        assert!(
            total_duration < Duration::from_secs(30),
            "Should complete within reasonable time"
        );
    }
}

fn create_test_heartbeat_message() -> ProtocolMessage {
    let node_id = Uuid::new_v4();

    let heartbeat = NodeMessage::Heartbeat {
        node_id,
        status: NodeStatus::Ready,
        resources: NodeResources {
            cpu_cores: 8,
            memory_mb: 16384,
            disk_gb: 500,
            network_mbps: 1000,
            available_cpu: 6.5,
            available_memory_mb: 12000,
            available_disk_gb: 450,
        },
        metrics: NodeMetrics {
            cpu_usage_percent: 18.75,
            memory_usage_percent: 26.56,
            disk_usage_percent: 10.0,
            network_rx_mbps: 50.0,
            network_tx_mbps: 25.0,
            load_average: 1.2,
            active_jobs: 2,
            completed_jobs: 150,
            failed_jobs: 5,
            uptime_seconds: 86400,
        },
        timestamp: chrono::Utc::now(),
    };

    ProtocolMessage::new(node_id, MessagePayload::NodeMessage(heartbeat))
}
