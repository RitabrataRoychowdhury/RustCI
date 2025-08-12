use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::valkyrie::{TestUtils, TestFixtures, TestAssertions, ChaosScenario};
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;
use rustci::core::networking::valkyrie::message::MessageType;

/// Chaos engineering tests for resilience validation
#[cfg(test)]
mod chaos_tests {
    use super::*;

    #[tokio::test]
    async fn test_network_partition_recovery() {
        let node_count = 3;
        let mut engines = TestUtils::create_test_cluster(node_count).await.unwrap();
        
        // Start all nodes
        for engine in &mut engines {
            engine.start().await.unwrap();
        }
        
        // Establish connections between nodes
        let mut connections = Vec::new();
        for i in 0..node_count {
            for j in (i + 1)..node_count {
                let addr = format!("127.0.0.1:{}", 8000 + j).parse().unwrap();
                if let Ok(conn) = engines[i].connect(addr).await {
                    connections.push((i, j, conn));
                }
            }
        }
        
        println!("Established {} connections", connections.len());
        
        // Simulate network partition (isolate node 0)
        println!("Simulating network partition...");
        engines[0].simulate_network_partition(vec![1, 2]).await.unwrap();
        
        // Wait for partition to take effect
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Test that remaining nodes can still communicate
        let addr = format!("127.0.0.1:{}", 8002).parse().unwrap();
        let partition_connection = engines[1].connect(addr).await;
        assert!(partition_connection.is_ok(), "Non-partitioned nodes should still communicate");
        
        // Heal the partition
        println!("Healing network partition...");
        engines[0].heal_network_partition().await.unwrap();
        
        // Wait for recovery
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Test that all nodes can communicate again
        let addr = format!("127.0.0.1:{}", 8001).parse().unwrap();
        let recovery_connection = timeout(
            Duration::from_secs(10),
            engines[0].connect(addr)
        ).await;
        
        assert!(recovery_connection.is_ok(), "Nodes should reconnect after partition healing");
        
        // Cleanup
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_high_latency_injection() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Measure baseline latency
        let mut baseline_latencies = Vec::new();
        for _ in 0..10 {
            let message = TestUtils::create_test_message(MessageType::Ping);
            let (_, latency) = TestUtils::measure_time(async {
                connection.send_message(message).await
            }).await;
            baseline_latencies.push(latency);
        }
        
        let baseline_avg = baseline_latencies.iter().sum::<Duration>() / baseline_latencies.len() as u32;
        println!("Baseline latency: {:?}", baseline_avg);
        
        // Inject high latency (500ms)
        engine.inject_network_latency(Duration::from_millis(500)).await.unwrap();
        
        // Measure latency with injection
        let mut high_latency_measurements = Vec::new();
        for _ in 0..10 {
            let message = TestUtils::create_test_message(MessageType::Ping);
            let (_, latency) = TestUtils::measure_time(async {
                connection.send_message(message).await
            }).await;
            high_latency_measurements.push(latency);
        }
        
        let high_latency_avg = high_latency_measurements.iter().sum::<Duration>() / high_latency_measurements.len() as u32;
        println!("High latency: {:?}", high_latency_avg);
        
        // Verify latency injection worked
        assert!(
            high_latency_avg > baseline_avg + Duration::from_millis(400),
            "Latency injection should significantly increase latency"
        );
        
        // Remove latency injection
        engine.remove_network_latency().await.unwrap();
        
        // Wait for recovery
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Measure recovery latency
        let mut recovery_latencies = Vec::new();
        for _ in 0..10 {
            let message = TestUtils::create_test_message(MessageType::Ping);
            let (_, latency) = TestUtils::measure_time(async {
                connection.send_message(message).await
            }).await;
            recovery_latencies.push(latency);
        }
        
        let recovery_avg = recovery_latencies.iter().sum::<Duration>() / recovery_latencies.len() as u32;
        println!("Recovery latency: {:?}", recovery_avg);
        
        // Verify recovery
        assert!(
            recovery_avg < baseline_avg + Duration::from_millis(50),
            "Latency should return to baseline after injection removal"
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_packet_loss_simulation() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Measure baseline success rate
        let baseline_attempts = 100;
        let mut baseline_successes = 0;
        
        for _ in 0..baseline_attempts {
            let message = TestUtils::create_test_message(MessageType::Ping);
            if connection.send_message(message).await.is_ok() {
                baseline_successes += 1;
            }
        }
        
        let baseline_success_rate = baseline_successes as f64 / baseline_attempts as f64;
        println!("Baseline success rate: {:.2}%", baseline_success_rate * 100.0);
        
        // Inject 20% packet loss
        engine.inject_packet_loss(0.2).await.unwrap();
        
        // Measure success rate with packet loss
        let packet_loss_attempts = 100;
        let mut packet_loss_successes = 0;
        
        for _ in 0..packet_loss_attempts {
            let message = TestUtils::create_test_message(MessageType::Ping);
            if connection.send_message(message).await.is_ok() {
                packet_loss_successes += 1;
            }
        }
        
        let packet_loss_success_rate = packet_loss_successes as f64 / packet_loss_attempts as f64;
        println!("Packet loss success rate: {:.2}%", packet_loss_success_rate * 100.0);
        
        // Verify packet loss injection worked
        assert!(
            packet_loss_success_rate < baseline_success_rate - 0.1,
            "Packet loss should reduce success rate"
        );
        
        // Remove packet loss
        engine.remove_packet_loss().await.unwrap();
        
        // Wait for recovery
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Measure recovery success rate
        let recovery_attempts = 100;
        let mut recovery_successes = 0;
        
        for _ in 0..recovery_attempts {
            let message = TestUtils::create_test_message(MessageType::Ping);
            if connection.send_message(message).await.is_ok() {
                recovery_successes += 1;
            }
        }
        
        let recovery_success_rate = recovery_successes as f64 / recovery_attempts as f64;
        println!("Recovery success rate: {:.2}%", recovery_success_rate * 100.0);
        
        // Verify recovery
        assert!(
            recovery_success_rate > baseline_success_rate - 0.05,
            "Success rate should recover after packet loss removal"
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_node_failure_and_recovery() {
        let node_count = 3;
        let mut engines = TestUtils::create_test_cluster(node_count).await.unwrap();
        
        // Start all nodes
        for engine in &mut engines {
            engine.start().await.unwrap();
        }
        
        // Establish connections
        let addr1 = "127.0.0.1:8001".parse().unwrap();
        let addr2 = "127.0.0.1:8002".parse().unwrap();
        
        let conn_0_to_1 = engines[0].connect(addr1).await.unwrap();
        let conn_0_to_2 = engines[0].connect(addr2).await.unwrap();
        
        // Test initial connectivity
        let message = TestUtils::create_test_message(MessageType::Hello);
        assert!(conn_0_to_1.send_message(message.clone()).await.is_ok());
        assert!(conn_0_to_2.send_message(message).await.is_ok());
        
        // Simulate node 1 failure
        println!("Simulating node 1 failure...");
        engines[1].simulate_crash().await.unwrap();
        
        // Wait for failure detection
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Test that connection to failed node is detected
        let message = TestUtils::create_test_message(MessageType::Ping);
        let failed_send = conn_0_to_1.send_message(message).await;
        assert!(failed_send.is_err(), "Connection to failed node should fail");
        
        // Test that other connections still work
        let message = TestUtils::create_test_message(MessageType::Ping);
        let working_send = conn_0_to_2.send_message(message).await;
        assert!(working_send.is_ok(), "Connection to healthy node should work");
        
        // Simulate node 1 recovery
        println!("Simulating node 1 recovery...");
        engines[1].recover_from_crash().await.unwrap();
        engines[1].start().await.unwrap();
        
        // Wait for recovery
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Test reconnection to recovered node
        let recovered_connection = timeout(
            Duration::from_secs(10),
            engines[0].connect(addr1)
        ).await;
        
        assert!(recovered_connection.is_ok(), "Should reconnect to recovered node");
        
        // Cleanup
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_memory_pressure_simulation() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Measure baseline performance
        let baseline_message = TestUtils::create_test_message(MessageType::JobRequest);
        let (_, baseline_latency) = TestUtils::measure_time(async {
            connection.send_message(baseline_message).await
        }).await;
        
        println!("Baseline latency: {:?}", baseline_latency);
        
        // Simulate memory pressure
        engine.simulate_memory_pressure(0.9).await.unwrap(); // 90% memory usage
        
        // Test performance under memory pressure
        let pressure_message = TestUtils::create_test_message(MessageType::JobRequest);
        let (_, pressure_latency) = TestUtils::measure_time(async {
            connection.send_message(pressure_message).await
        }).await;
        
        println!("Memory pressure latency: {:?}", pressure_latency);
        
        // Test that system remains functional under pressure
        assert!(
            pressure_latency < Duration::from_secs(5),
            "System should remain functional under memory pressure"
        );
        
        // Test memory cleanup
        engine.trigger_memory_cleanup().await.unwrap();
        
        // Wait for cleanup
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Test recovery performance
        let recovery_message = TestUtils::create_test_message(MessageType::JobRequest);
        let (_, recovery_latency) = TestUtils::measure_time(async {
            connection.send_message(recovery_message).await
        }).await;
        
        println!("Recovery latency: {:?}", recovery_latency);
        
        // Verify performance recovery
        assert!(
            recovery_latency < pressure_latency,
            "Performance should improve after memory cleanup"
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_cpu_starvation_simulation() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Measure baseline throughput
        let baseline_duration = Duration::from_secs(5);
        let baseline_throughput = crate::valkyrie::PerformanceUtils::measure_throughput(
            || {
                let conn = connection.clone();
                Box::pin(async move {
                    let message = TestUtils::create_test_message(MessageType::Ping);
                    conn.send_message(message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                })
            },
            baseline_duration
        ).await;
        
        println!("Baseline throughput: {:.2} ops/sec", baseline_throughput.ops_per_second);
        
        // Simulate CPU starvation
        engine.simulate_cpu_starvation(0.95).await.unwrap(); // 95% CPU usage
        
        // Measure throughput under CPU starvation
        let starvation_throughput = crate::valkyrie::PerformanceUtils::measure_throughput(
            || {
                let conn = connection.clone();
                Box::pin(async move {
                    let message = TestUtils::create_test_message(MessageType::Ping);
                    conn.send_message(message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                })
            },
            baseline_duration
        ).await;
        
        println!("CPU starvation throughput: {:.2} ops/sec", starvation_throughput.ops_per_second);
        
        // Test that system degrades gracefully
        assert!(
            starvation_throughput.ops_per_second > 0.0,
            "System should remain functional under CPU starvation"
        );
        
        // Remove CPU starvation
        engine.remove_cpu_starvation().await.unwrap();
        
        // Wait for recovery
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Measure recovery throughput
        let recovery_throughput = crate::valkyrie::PerformanceUtils::measure_throughput(
            || {
                let conn = connection.clone();
                Box::pin(async move {
                    let message = TestUtils::create_test_message(MessageType::Ping);
                    conn.send_message(message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                })
            },
            baseline_duration
        ).await;
        
        println!("Recovery throughput: {:.2} ops/sec", recovery_throughput.ops_per_second);
        
        // Verify throughput recovery
        assert!(
            recovery_throughput.ops_per_second > starvation_throughput.ops_per_second,
            "Throughput should improve after CPU starvation removal"
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_cascading_failure_scenario() {
        let node_count = 5;
        let mut engines = TestUtils::create_test_cluster(node_count).await.unwrap();
        
        // Start all nodes
        for engine in &mut engines {
            engine.start().await.unwrap();
        }
        
        // Create a mesh of connections
        let mut connections = Vec::new();
        for i in 0..node_count {
            for j in (i + 1)..node_count {
                let addr = format!("127.0.0.1:{}", 8000 + j).parse().unwrap();
                if let Ok(conn) = engines[i].connect(addr).await {
                    connections.push((i, j, conn));
                }
            }
        }
        
        println!("Established {} connections in mesh", connections.len());
        
        // Simulate cascading failure starting with node 0
        println!("Starting cascading failure simulation...");
        
        // Fail node 0
        engines[0].simulate_crash().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // This should trigger increased load on remaining nodes
        // Simulate overload on node 1 due to redistributed load
        engines[1].simulate_overload().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Fail node 1 due to overload
        engines[1].simulate_crash().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Test that remaining nodes (2, 3, 4) can still communicate
        let addr3 = "127.0.0.1:8003".parse().unwrap();
        let addr4 = "127.0.0.1:8004".parse().unwrap();
        
        let surviving_connection = timeout(
            Duration::from_secs(5),
            engines[2].connect(addr3)
        ).await;
        
        assert!(surviving_connection.is_ok(), "Surviving nodes should still communicate");
        
        // Test message passing between surviving nodes
        let conn = surviving_connection.unwrap().unwrap();
        let message = TestUtils::create_test_message(MessageType::Hello);
        let send_result = conn.send_message(message).await;
        
        assert!(send_result.is_ok(), "Surviving nodes should handle messages");
        
        // Simulate recovery of failed nodes
        println!("Simulating recovery...");
        
        engines[0].recover_from_crash().await.unwrap();
        engines[0].start().await.unwrap();
        
        engines[1].recover_from_crash().await.unwrap();
        engines[1].start().await.unwrap();
        
        // Wait for full recovery
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Test that all nodes can communicate again
        let addr0 = "127.0.0.1:8000".parse().unwrap();
        let recovery_connection = timeout(
            Duration::from_secs(10),
            engines[2].connect(addr0)
        ).await;
        
        assert!(recovery_connection.is_ok(), "All nodes should communicate after recovery");
        
        // Cleanup
        for mut engine in engines {
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_chaos_scenarios_from_fixtures() {
        let scenarios = TestFixtures::chaos_scenarios();
        
        for scenario in scenarios {
            println!("Running chaos scenario: {}", scenario.name);
            
            let mut engine = TestUtils::create_test_engine().await.unwrap();
            engine.start().await.unwrap();
            
            let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
            let bind_addr = listener.local_addr().unwrap();
            
            let connection = engine.connect(bind_addr).await.unwrap();
            
            // Measure baseline performance
            let baseline_message = TestUtils::create_test_message(MessageType::Ping);
            let (_, baseline_latency) = TestUtils::measure_time(async {
                connection.send_message(baseline_message).await
            }).await;
            
            // Apply chaos scenario
            match scenario.name.as_str() {
                "network_partition" => {
                    engine.simulate_network_partition(vec![]).await.unwrap();
                }
                "high_latency" => {
                    engine.inject_network_latency(Duration::from_millis(100)).await.unwrap();
                }
                "packet_loss" => {
                    engine.inject_packet_loss(0.1).await.unwrap();
                }
                _ => {
                    println!("Unknown chaos scenario: {}", scenario.name);
                    continue;
                }
            }
            
            // Wait for chaos to take effect
            tokio::time::sleep(scenario.duration).await;
            
            // Test system behavior under chaos
            let chaos_message = TestUtils::create_test_message(MessageType::Ping);
            let chaos_result = timeout(
                Duration::from_secs(10),
                connection.send_message(chaos_message)
            ).await;
            
            // System should either work (possibly with degraded performance) or fail gracefully
            match chaos_result {
                Ok(Ok(_)) => println!("  System remained functional under chaos"),
                Ok(Err(_)) => println!("  System failed gracefully under chaos"),
                Err(_) => println!("  System timed out under chaos"),
            }
            
            // Remove chaos
            match scenario.name.as_str() {
                "network_partition" => {
                    engine.heal_network_partition().await.unwrap();
                }
                "high_latency" => {
                    engine.remove_network_latency().await.unwrap();
                }
                "packet_loss" => {
                    engine.remove_packet_loss().await.unwrap();
                }
                _ => {}
            }
            
            // Wait for recovery
            tokio::time::sleep(scenario.recovery_time).await;
            
            // Test recovery
            let recovery_message = TestUtils::create_test_message(MessageType::Ping);
            let recovery_result = timeout(
                Duration::from_secs(5),
                connection.send_message(recovery_message)
            ).await;
            
            assert!(recovery_result.is_ok(), "System should recover from chaos scenario: {}", scenario.name);
            
            engine.stop().await.unwrap();
        }
    }
}