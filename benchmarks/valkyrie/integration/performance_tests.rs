use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::valkyrie::{TestUtils, TestFixtures, TestAssertions, PerformanceUtils, LoadScenario};
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;
use rustci::core::networking::valkyrie::message::MessageType;

/// Performance testing and load scenarios
#[cfg(test)]
mod performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_sub_millisecond_latency() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Extended warm-up to ensure optimal performance
        for _ in 0..100 {
            let message = TestUtils::create_test_message(MessageType::Ping);
            let _ = connection.send_message(message).await;
        }
        
        // Allow system to stabilize
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Measure latency for small messages with high precision
        let mut latencies = Vec::new();
        let test_count = 10000; // Increased sample size for better statistics
        
        for _ in 0..test_count {
            let message = TestUtils::create_test_message(MessageType::Ping);
            
            // Use high-precision timing
            let start = std::time::Instant::now();
            let _ = connection.send_message(message).await;
            let latency = start.elapsed();
            
            latencies.push(latency);
        }
        
        let stats = PerformanceUtils::calculate_percentiles(latencies);
        
        println!("Sub-millisecond Latency Test Results:");
        println!("  Min: {:?} ({:.2}μs)", stats.min, stats.min.as_nanos() as f64 / 1000.0);
        println!("  P50: {:?} ({:.2}μs)", stats.p50, stats.p50.as_nanos() as f64 / 1000.0);
        println!("  P95: {:?} ({:.2}μs)", stats.p95, stats.p95.as_nanos() as f64 / 1000.0);
        println!("  P99: {:?} ({:.2}μs)", stats.p99, stats.p99.as_nanos() as f64 / 1000.0);
        println!("  P99.9: {:?} ({:.2}μs)", stats.p999, stats.p999.as_nanos() as f64 / 1000.0);
        println!("  Max: {:?} ({:.2}μs)", stats.max, stats.max.as_nanos() as f64 / 1000.0);
        println!("  Mean: {:?} ({:.2}μs)", stats.mean, stats.mean.as_nanos() as f64 / 1000.0);
        
        // Strict sub-millisecond assertions with detailed reporting
        TestAssertions::assert_sub_millisecond_latency(stats.p50, "P50");
        TestAssertions::assert_sub_millisecond_latency(stats.p95, "P95");
        TestAssertions::assert_sub_millisecond_latency(stats.p99, "P99");
        TestAssertions::assert_sub_millisecond_latency(stats.p999, "P99.9");
        
        // Additional strict thresholds for performance validation
        TestAssertions::assert_latency_below(stats.p50, Duration::from_micros(500)); // P50 < 500μs
        TestAssertions::assert_latency_below(stats.p95, Duration::from_micros(800)); // P95 < 800μs
        TestAssertions::assert_latency_below(stats.p99, Duration::from_micros(950)); // P99 < 950μs
        
        // Validate sub-millisecond batch performance
        TestAssertions::assert_sub_millisecond_batch(&latencies, 99.0, "standard latency test");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_ultra_low_latency() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Extensive warm-up for ultra-low latency
        for _ in 0..1000 {
            let message = TestUtils::create_test_message(MessageType::Ping);
            let _ = connection.send_message(message).await;
        }
        
        // Allow system to reach optimal state
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Test with minimal payload for absolute minimum latency
        let mut ultra_low_latencies = Vec::new();
        let test_count = 5000;
        
        for _ in 0..test_count {
            let mut message = TestUtils::create_test_message(MessageType::Ping);
            message.payload = vec![]; // Empty payload for minimum overhead
            
            // Measure with nanosecond precision
            let start = std::time::Instant::now();
            let _ = connection.send_message(message).await;
            let latency = start.elapsed();
            
            ultra_low_latencies.push(latency);
        }
        
        let ultra_stats = PerformanceUtils::calculate_percentiles(ultra_low_latencies);
        
        println!("Ultra-Low Latency Test Results (empty payload):");
        println!("  Min: {:?} ({:.0}ns)", ultra_stats.min, ultra_stats.min.as_nanos() as f64);
        println!("  P50: {:?} ({:.0}ns)", ultra_stats.p50, ultra_stats.p50.as_nanos() as f64);
        println!("  P95: {:?} ({:.0}ns)", ultra_stats.p95, ultra_stats.p95.as_nanos() as f64);
        println!("  P99: {:?} ({:.0}ns)", ultra_stats.p99, ultra_stats.p99.as_nanos() as f64);
        println!("  Mean: {:?} ({:.0}ns)", ultra_stats.mean, ultra_stats.mean.as_nanos() as f64);
        
        // Ultra-strict assertions for empty payload
        TestAssertions::assert_latency_below(ultra_stats.p50, Duration::from_micros(100)); // P50 < 100μs
        TestAssertions::assert_latency_below(ultra_stats.p95, Duration::from_micros(300)); // P95 < 300μs
        TestAssertions::assert_latency_below(ultra_stats.p99, Duration::from_micros(500)); // P99 < 500μs
        
        // Count ultra-fast requests (< 100μs)
        let ultra_fast_count = ultra_low_latencies.iter()
            .filter(|&&lat| lat < Duration::from_micros(100))
            .count();
        let ultra_fast_percentage = (ultra_fast_count as f64 / test_count as f64) * 100.0;
        
        println!("  Ultra-fast requests (<100μs): {}/{} ({:.2}%)", 
                ultra_fast_count, test_count, ultra_fast_percentage);
        
        assert!(
            ultra_fast_percentage >= 50.0,
            "At least 50% of empty payload requests should be <100μs, got {:.2}%",
            ultra_fast_percentage
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_micro_benchmark_critical_path() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Micro-benchmark the most critical path: message creation + send
        let mut micro_latencies = Vec::new();
        let micro_test_count = 100000; // Large sample for micro-benchmark
        
        // Pre-allocate message to avoid allocation overhead
        let base_message = TestUtils::create_test_message(MessageType::Ping);
        
        for _ in 0..micro_test_count {
            let message = base_message.clone();
            
            // Measure only the critical send path
            let start = std::time::Instant::now();
            let _ = connection.send_message(message).await;
            let micro_latency = start.elapsed();
            
            micro_latencies.push(micro_latency);
        }
        
        let micro_stats = PerformanceUtils::calculate_percentiles(micro_latencies);
        
        println!("Micro-benchmark Critical Path Results:");
        println!("  Min: {:?} ({:.0}ns)", micro_stats.min, micro_stats.min.as_nanos() as f64);
        println!("  P50: {:?} ({:.0}ns)", micro_stats.p50, micro_stats.p50.as_nanos() as f64);
        println!("  P95: {:?} ({:.0}ns)", micro_stats.p95, micro_stats.p95.as_nanos() as f64);
        println!("  P99: {:?} ({:.0}ns)", micro_stats.p99, micro_stats.p99.as_nanos() as f64);
        println!("  P99.9: {:?} ({:.0}ns)", micro_stats.p999, micro_stats.p999.as_nanos() as f64);
        
        // Extremely strict micro-benchmark assertions
        TestAssertions::assert_latency_below(micro_stats.p50, Duration::from_micros(50)); // P50 < 50μs
        TestAssertions::assert_latency_below(micro_stats.p95, Duration::from_micros(200)); // P95 < 200μs
        TestAssertions::assert_latency_below(micro_stats.p99, Duration::from_micros(400)); // P99 < 400μs
        
        // Count ultra-fast operations (< 10μs)
        let ultra_fast_count = micro_latencies.iter()
            .filter(|&&lat| lat < Duration::from_micros(10))
            .count();
        let ultra_fast_percentage = (ultra_fast_count as f64 / micro_test_count as f64) * 100.0;
        
        println!("  Ultra-fast operations (<10μs): {}/{} ({:.2}%)", 
                ultra_fast_count, micro_test_count, ultra_fast_percentage);
        
        // At least 10% should be ultra-fast for a well-optimized implementation
        assert!(
            ultra_fast_percentage >= 10.0,
            "At least 10% of operations should be <10μs, got {:.2}%",
            ultra_fast_percentage
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_high_throughput() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Test high throughput message sending
        let throughput_stats = PerformanceUtils::measure_throughput(
            || {
                let conn = connection.clone();
                Box::pin(async move {
                    let message = TestUtils::create_test_message(MessageType::JobRequest);
                    conn.send_message(message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                })
            },
            Duration::from_secs(10)
        ).await;
        
        println!("High Throughput Test Results:");
        println!("  Operations: {}", throughput_stats.operations);
        println!("  Ops/sec: {:.2}", throughput_stats.ops_per_second);
        println!("  Error rate: {:.2}%", throughput_stats.error_rate * 100.0);
        println!("  Avg latency: {:.2}μs", 1_000_000.0 / throughput_stats.ops_per_second);
        
        // Assert minimum throughput for sub-millisecond performance
        TestAssertions::assert_throughput_above(throughput_stats.ops_per_second, 50000.0); // 50K ops/sec = ~20μs avg
        TestAssertions::assert_error_rate_below(throughput_stats.error_rate, 0.001); // <0.1% error rate
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_connections_scalability() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection_counts = vec![10, 100, 1000];
        
        for connection_count in connection_counts {
            println!("Testing {} concurrent connections", connection_count);
            
            let semaphore = Arc::new(Semaphore::new(connection_count));
            let mut connection_tasks = Vec::new();
            
            let start_time = Instant::now();
            
            for _ in 0..connection_count {
                let engine_clone = engine.clone();
                let semaphore_clone = semaphore.clone();
                
                let task = tokio::spawn(async move {
                    let _permit = semaphore_clone.acquire().await.unwrap();
                    
                    let connection = engine_clone.connect(bind_addr).await?;
                    
                    // Send a test message
                    let message = TestUtils::create_test_message(MessageType::Hello);
                    connection.send_message(message).await?;
                    
                    Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
                });
                
                connection_tasks.push(task);
            }
            
            let results = futures::future::join_all(connection_tasks).await;
            let connection_time = start_time.elapsed();
            
            let successful_connections = results.into_iter()
                .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
                .count();
            
            println!("  Successful connections: {}/{}", successful_connections, connection_count);
            println!("  Connection time: {:?}", connection_time);
            
            // Assert reasonable success rate and performance
            assert!(
                successful_connections >= connection_count * 9 / 10,
                "At least 90% of connections should succeed"
            );
            
            TestAssertions::assert_latency_below(
                connection_time,
                Duration::from_secs(connection_count as u64 / 100 + 1)
            );
        }
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_memory_usage_scalability() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let initial_memory = get_memory_usage();
        
        // Create many connections and measure memory growth
        let connection_count = 1000;
        let mut connections = Vec::new();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        for _ in 0..connection_count {
            let connection = engine.connect(bind_addr).await.unwrap();
            connections.push(connection);
        }
        
        let peak_memory = get_memory_usage();
        
        // Close all connections
        drop(connections);
        tokio::time::sleep(Duration::from_secs(1)).await; // Allow cleanup
        
        let final_memory = get_memory_usage();
        
        println!("Memory Usage Test Results:");
        println!("  Initial: {} MB", initial_memory / 1024 / 1024);
        println!("  Peak: {} MB", peak_memory / 1024 / 1024);
        println!("  Final: {} MB", final_memory / 1024 / 1024);
        println!("  Memory per connection: {} KB", (peak_memory - initial_memory) / connection_count / 1024);
        
        // Assert reasonable memory usage (adjust based on requirements)
        let memory_per_connection = (peak_memory - initial_memory) / connection_count;
        assert!(
            memory_per_connection < 10 * 1024, // Less than 10KB per connection
            "Memory usage per connection should be reasonable"
        );
        
        // Assert memory is properly cleaned up
        assert!(
            final_memory < initial_memory + (initial_memory / 10), // Within 10% of initial
            "Memory should be properly cleaned up"
        );
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_load_scenarios() {
        let scenarios = TestFixtures::load_scenarios();
        
        for scenario in scenarios {
            println!("Running load scenario: {}", scenario.name);
            
            let mut engine = TestUtils::create_test_engine().await.unwrap();
            engine.start().await.unwrap();
            
            let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
            let bind_addr = listener.local_addr().unwrap();
            
            // Create connections
            let mut connections = Vec::new();
            for _ in 0..scenario.concurrent_connections {
                let connection = engine.connect(bind_addr).await.unwrap();
                connections.push(connection);
            }
            
            // Run load test
            let start_time = Instant::now();
            let mut message_tasks = Vec::new();
            
            let messages_per_connection = scenario.messages_per_second / scenario.concurrent_connections;
            let message_interval = Duration::from_secs(1) / messages_per_connection as u32;
            
            for connection in connections {
                let scenario_clone = scenario.clone();
                let task = tokio::spawn(async move {
                    let mut successful_messages = 0;
                    let mut failed_messages = 0;
                    
                    let end_time = Instant::now() + scenario_clone.duration;
                    
                    while Instant::now() < end_time {
                        let test_data = TestUtils::generate_test_data(scenario_clone.message_size);
                        let mut message = TestUtils::create_test_message(MessageType::JobRequest);
                        message.payload = test_data;
                        
                        match connection.send_message(message).await {
                            Ok(_) => successful_messages += 1,
                            Err(_) => failed_messages += 1,
                        }
                        
                        tokio::time::sleep(message_interval).await;
                    }
                    
                    (successful_messages, failed_messages)
                });
                
                message_tasks.push(task);
            }
            
            let results = futures::future::join_all(message_tasks).await;
            let total_time = start_time.elapsed();
            
            let (total_successful, total_failed): (u64, u64) = results.into_iter()
                .map(|r| r.unwrap_or((0, 0)))
                .fold((0, 0), |(acc_s, acc_f), (s, f)| (acc_s + s, acc_f + f));
            
            let actual_throughput = total_successful as f64 / total_time.as_secs_f64();
            let error_rate = total_failed as f64 / (total_successful + total_failed) as f64;
            
            println!("  Results:");
            println!("    Successful messages: {}", total_successful);
            println!("    Failed messages: {}", total_failed);
            println!("    Actual throughput: {:.2} msg/sec", actual_throughput);
            println!("    Error rate: {:.2}%", error_rate * 100.0);
            
            // Assert performance meets scenario requirements
            let expected_throughput = scenario.messages_per_second as f64 * 0.8; // 80% of target
            TestAssertions::assert_throughput_above(actual_throughput, expected_throughput);
            TestAssertions::assert_error_rate_below(error_rate, 0.05); // 5% error rate
            
            engine.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_cpu_utilization() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Monitor CPU usage during high load
        let cpu_monitor = tokio::spawn(async {
            let mut cpu_samples = Vec::new();
            let start_time = Instant::now();
            
            while start_time.elapsed() < Duration::from_secs(10) {
                let cpu_usage = get_cpu_usage();
                cpu_samples.push(cpu_usage);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            cpu_samples
        });
        
        // Generate load
        let load_task = tokio::spawn(async move {
            for _ in 0..10000 {
                let message = TestUtils::create_test_message(MessageType::JobRequest);
                let _ = connection.send_message(message).await;
            }
        });
        
        let (cpu_samples, _) = tokio::join!(cpu_monitor, load_task);
        let cpu_samples = cpu_samples.unwrap();
        
        let avg_cpu = cpu_samples.iter().sum::<f64>() / cpu_samples.len() as f64;
        let max_cpu = cpu_samples.iter().fold(0.0, |a, &b| a.max(b));
        
        println!("CPU Utilization Test Results:");
        println!("  Average CPU: {:.2}%", avg_cpu);
        println!("  Peak CPU: {:.2}%", max_cpu);
        
        // Assert reasonable CPU usage
        assert!(avg_cpu < 80.0, "Average CPU usage should be reasonable");
        assert!(max_cpu < 95.0, "Peak CPU usage should not saturate the system");
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_network_bandwidth_utilization() {
        let mut engine = TestUtils::create_test_engine().await.unwrap();
        engine.start().await.unwrap();
        
        let listener = engine.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let bind_addr = listener.local_addr().unwrap();
        
        let connection = engine.connect(bind_addr).await.unwrap();
        
        // Test different message sizes for bandwidth utilization
        let message_sizes = vec![1024, 64 * 1024, 1024 * 1024]; // 1KB, 64KB, 1MB
        
        for message_size in message_sizes {
            println!("Testing bandwidth with {}KB messages", message_size / 1024);
            
            let test_data = TestUtils::generate_test_data(message_size);
            let message_count = 100;
            
            let start_time = Instant::now();
            let mut bytes_sent = 0;
            
            for _ in 0..message_count {
                let mut message = TestUtils::create_test_message(MessageType::JobRequest);
                message.payload = test_data.clone();
                
                match connection.send_message(message).await {
                    Ok(_) => bytes_sent += message_size,
                    Err(_) => {}
                }
            }
            
            let elapsed = start_time.elapsed();
            let bandwidth_mbps = (bytes_sent as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0);
            
            println!("  Bandwidth: {:.2} Mbps", bandwidth_mbps);
            
            // Assert reasonable bandwidth utilization
            assert!(bandwidth_mbps > 10.0, "Should achieve reasonable bandwidth utilization");
        }
        
        engine.stop().await.unwrap();
    }

    // Helper functions for system metrics
    fn get_memory_usage() -> usize {
        // This is a simplified implementation
        // In a real test, you would use a proper system monitoring library
        use std::process::Command;
        
        let output = Command::new("ps")
            .args(&["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        
        let rss_kb = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<usize>()
            .unwrap_or(0);
        
        rss_kb * 1024 // Convert to bytes
    }
    
    fn get_cpu_usage() -> f64 {
        // This is a simplified implementation
        // In a real test, you would use a proper system monitoring library
        use std::process::Command;
        
        let output = Command::new("ps")
            .args(&["-o", "pcpu=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0)
    }
}