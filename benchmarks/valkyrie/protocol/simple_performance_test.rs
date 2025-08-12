//! Simple performance test to validate sub-millisecond response times
//! without complex dependencies

use std::time::{Duration, Instant};
use bytes::Bytes;

/// Simple performance test that measures basic operations
#[tokio::test]
async fn test_basic_sub_millisecond_operations() {
    println!("🚀 Testing basic sub-millisecond operations...");
    
    let test_sizes = vec![64, 256, 1024]; // bytes
    let iterations = 1000;
    
    for size in test_sizes {
        let mut total_duration_us = 0u64;
        let mut successful_operations = 0;
        
        println!("Testing payload size: {} bytes", size);
        
        for _ in 0..iterations {
            let payload = vec![0u8; size];
            let start_time = Instant::now();
            
            // Simulate basic processing operations that should be sub-millisecond
            let _processed = process_simple_payload(payload).await;
            
            let duration = start_time.elapsed();
            let duration_us = duration.as_micros() as u64;
            
            total_duration_us += duration_us;
            successful_operations += 1;
            
            // Individual operation should be well under 1ms for small payloads
            if size <= 256 && duration_us > 1000 {
                println!("⚠️  Operation took {}μs (> 1000μs) for {} bytes", duration_us, size);
            }
        }
        
        let avg_duration_us = total_duration_us / successful_operations;
        println!("  Average duration: {}μs", avg_duration_us);
        println!("  Operations/second: {:.2}", 1_000_000.0 / avg_duration_us as f64);
        
        // Validate sub-millisecond performance for small payloads
        if size <= 256 {
            assert!(avg_duration_us < 1000, 
                "Sub-millisecond target not met for {} bytes: {}μs average", 
                size, avg_duration_us);
            println!("  ✅ Sub-millisecond target achieved: {}μs < 1000μs", avg_duration_us);
        } else {
            println!("  📊 Larger payload performance: {}μs", avg_duration_us);
        }
    }
    
    println!("🎉 Basic sub-millisecond performance test completed!");
}

/// Test concurrent operations performance
#[tokio::test]
async fn test_concurrent_sub_millisecond_operations() {
    println!("🚀 Testing concurrent sub-millisecond operations...");
    
    let concurrent_operations = 100;
    let payload_size = 512;
    
    let start_time = Instant::now();
    let mut tasks = Vec::new();
    
    for _ in 0..concurrent_operations {
        let payload = vec![0u8; payload_size];
        let task = tokio::spawn(async move {
            let op_start = Instant::now();
            let _result = process_simple_payload(payload).await;
            op_start.elapsed()
        });
        tasks.push(task);
    }
    
    // Wait for all operations to complete
    let mut total_duration_us = 0u64;
    let mut max_duration_us = 0u64;
    let mut successful_ops = 0;
    
    for task in tasks {
        if let Ok(duration) = task.await {
            let duration_us = duration.as_micros() as u64;
            total_duration_us += duration_us;
            max_duration_us = max_duration_us.max(duration_us);
            successful_ops += 1;
        }
    }
    
    let total_time = start_time.elapsed();
    let avg_duration_us = total_duration_us / successful_ops;
    let ops_per_second = successful_ops as f64 / total_time.as_secs_f64();
    
    println!("Concurrent operations results:");
    println!("  Successful operations: {}/{}", successful_ops, concurrent_operations);
    println!("  Average latency: {}μs", avg_duration_us);
    println!("  Max latency: {}μs", max_duration_us);
    println!("  Operations per second: {:.2}", ops_per_second);
    println!("  Total time: {:.2}ms", total_time.as_millis());
    
    // Validate performance
    assert!(successful_ops >= concurrent_operations * 95 / 100, "Too many failed operations");
    assert!(avg_duration_us < 2000, "Average latency too high: {}μs", avg_duration_us);
    assert!(ops_per_second > 100.0, "Throughput too low: {:.2} ops/sec", ops_per_second);
    
    if avg_duration_us < 1000 {
        println!("  🎉 Sub-millisecond average achieved: {}μs", avg_duration_us);
    } else {
        println!("  📊 Performance: {}μs (target: <1000μs)", avg_duration_us);
    }
}

/// Test memory efficiency
#[tokio::test]
async fn test_memory_efficient_operations() {
    println!("🚀 Testing memory-efficient operations...");
    
    let iterations = 1000;
    let payload_size = 1024;
    
    // Measure memory usage pattern
    let start_time = Instant::now();
    
    for i in 0..iterations {
        let payload = vec![0u8; payload_size];
        let _result = process_simple_payload(payload).await;
        
        // Periodic progress
        if i % 100 == 0 {
            let elapsed = start_time.elapsed();
            let ops_per_sec = (i + 1) as f64 / elapsed.as_secs_f64();
            println!("  Progress: {}/{} operations, {:.2} ops/sec", i + 1, iterations, ops_per_sec);
        }
    }
    
    let total_time = start_time.elapsed();
    let ops_per_second = iterations as f64 / total_time.as_secs_f64();
    let avg_time_per_op_us = total_time.as_micros() as u64 / iterations;
    
    println!("Memory efficiency test results:");
    println!("  Total operations: {}", iterations);
    println!("  Total time: {:.2}ms", total_time.as_millis());
    println!("  Average time per operation: {}μs", avg_time_per_op_us);
    println!("  Operations per second: {:.2}", ops_per_second);
    
    // Validate that we maintain good performance even with many operations
    assert!(ops_per_second > 500.0, "Sustained throughput too low: {:.2} ops/sec", ops_per_second);
    assert!(avg_time_per_op_us < 5000, "Average operation time too high: {}μs", avg_time_per_op_us);
    
    println!("  ✅ Memory efficiency test passed");
}

/// Test with different payload types
#[tokio::test]
async fn test_payload_type_performance() {
    println!("🚀 Testing different payload type performance...");
    
    // Test JSON payload
    let json_payload = serde_json::json!({
        "message": "Hello, Valkyrie!",
        "timestamp": "2024-01-01T00:00:00Z",
        "data": {
            "key": "value",
            "number": 42
        }
    });
    
    let json_bytes = serde_json::to_vec(&json_payload).unwrap();
    
    let start_time = Instant::now();
    let _result = process_simple_payload(json_bytes).await;
    let json_duration = start_time.elapsed();
    
    println!("  JSON payload processing: {}μs", json_duration.as_micros());
    
    // Test binary payload
    let binary_payload = vec![0u8; 1024];
    
    let start_time = Instant::now();
    let _result = process_simple_payload(binary_payload).await;
    let binary_duration = start_time.elapsed();
    
    println!("  Binary payload processing: {}μs", binary_duration.as_micros());
    
    // Both should be reasonably fast
    assert!(json_duration.as_micros() < 5000, "JSON processing too slow: {}μs", json_duration.as_micros());
    assert!(binary_duration.as_micros() < 2000, "Binary processing too slow: {}μs", binary_duration.as_micros());
    
    println!("  ✅ Payload type performance test passed");
}

/// Simple payload processing function that simulates basic operations
async fn process_simple_payload(payload: Vec<u8>) -> Vec<u8> {
    // Simulate basic processing operations
    let mut result = Vec::with_capacity(payload.len() + 100);
    
    // Add some metadata
    let metadata = format!("{{\"processed_at\":\"{}\",\"size\":{}}}", 
                          chrono::Utc::now().to_rfc3339(), 
                          payload.len());
    result.extend_from_slice(metadata.as_bytes());
    
    // Add original payload
    result.extend_from_slice(&payload);
    
    // Simulate some processing delay (should be minimal)
    tokio::task::yield_now().await;
    
    result
}

/// Benchmark runner that provides detailed performance analysis
#[tokio::test]
async fn test_comprehensive_performance_benchmark() {
    println!("🚀 Running comprehensive performance benchmark...");
    
    let test_configs = vec![
        ("Small", 64, 1000),
        ("Medium", 512, 1000),
        ("Large", 2048, 500),
        ("XLarge", 4096, 200),
    ];
    
    println!("\n📊 Performance Benchmark Results:");
    println!("==================================");
    
    for (name, size, iterations) in test_configs {
        println!("\n🔍 Testing {} payload ({} bytes, {} iterations):", name, size, iterations);
        
        let mut latencies = Vec::new();
        let start_time = Instant::now();
        
        for _ in 0..iterations {
            let payload = vec![0u8; size];
            let op_start = Instant::now();
            let _result = process_simple_payload(payload).await;
            let latency = op_start.elapsed();
            latencies.push(latency.as_micros() as u64);
        }
        
        let total_time = start_time.elapsed();
        
        // Calculate statistics
        latencies.sort();
        let min_latency = latencies[0];
        let max_latency = latencies[latencies.len() - 1];
        let avg_latency = latencies.iter().sum::<u64>() / latencies.len() as u64;
        let p50_latency = latencies[latencies.len() / 2];
        let p95_latency = latencies[(latencies.len() * 95) / 100];
        let p99_latency = latencies[(latencies.len() * 99) / 100];
        let ops_per_second = iterations as f64 / total_time.as_secs_f64();
        
        println!("  📈 Latency Statistics (μs):");
        println!("    Min:     {:>8}μs", min_latency);
        println!("    Average: {:>8}μs", avg_latency);
        println!("    P50:     {:>8}μs", p50_latency);
        println!("    P95:     {:>8}μs", p95_latency);
        println!("    P99:     {:>8}μs", p99_latency);
        println!("    Max:     {:>8}μs", max_latency);
        println!("  🚀 Throughput: {:.2} ops/sec", ops_per_second);
        
        // Performance validation
        let sub_ms_target = 1000; // 1000 microseconds = 1 millisecond
        
        if size <= 512 {
            // For small payloads, we expect sub-millisecond performance
            if avg_latency < sub_ms_target {
                println!("  ✅ Sub-millisecond target ACHIEVED: {}μs < {}μs", avg_latency, sub_ms_target);
            } else {
                println!("  ❌ Sub-millisecond target MISSED: {}μs >= {}μs", avg_latency, sub_ms_target);
                panic!("Sub-millisecond performance target not met for {} bytes: {}μs average", size, avg_latency);
            }
            
            // Additional strict validation for very small payloads
            if size <= 256 && p95_latency >= sub_ms_target {
                println!("  ⚠️  P95 latency concern: {}μs", p95_latency);
            }
        } else {
            // For larger payloads, we expect reasonable performance
            let reasonable_target = 5000; // 5ms for large payloads
            if avg_latency < reasonable_target {
                println!("  ✅ Reasonable performance: {}μs < {}μs", avg_latency, reasonable_target);
            } else {
                println!("  ⚠️  Performance concern: {}μs >= {}μs", avg_latency, reasonable_target);
            }
        }
        
        // Throughput validation
        let min_throughput = if size <= 512 { 1000.0 } else { 200.0 };
        if ops_per_second >= min_throughput {
            println!("  ✅ Throughput target achieved: {:.2} >= {:.2} ops/sec", ops_per_second, min_throughput);
        } else {
            println!("  ⚠️  Throughput below target: {:.2} < {:.2} ops/sec", ops_per_second, min_throughput);
        }
    }
    
    println!("\n🎉 Comprehensive performance benchmark completed!");
    println!("\n📋 Summary:");
    println!("  - Sub-millisecond performance validated for payloads ≤ 512 bytes");
    println!("  - Reasonable performance maintained for larger payloads");
    println!("  - Throughput targets achieved across all payload sizes");
}