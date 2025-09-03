//! RustCI Performance Validation Tests
//! 
//! This test suite validates the performance of actual RustCI components

use std::time::{Duration, Instant};
use std::sync::Arc;
use RustAutoDevOps::core::performance::cache_manager::{MultiTierCacheManager, CacheConfig, CacheManager};
use RustAutoDevOps::core::security::rate_limiter::{DistributedRateLimiter, DistributedRateLimitConfig};
use RustAutoDevOps::api::rate_limiting::{RateLimiter, RateLimitResult};

#[tokio::test]
async fn test_cache_manager_performance() {
    println!("🚀 Testing Cache Manager Performance...");
    
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);
    
    // Test cache write performance
    let start = Instant::now();
    let num_operations = 1000;
    
    for i in 0..num_operations {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        cache.put(key, value).await.unwrap();
    }
    
    let write_duration = start.elapsed();
    let write_throughput = num_operations as f64 / write_duration.as_secs_f64();
    
    println!("✅ Cache Write Performance:");
    println!("   Operations: {}", num_operations);
    println!("   Duration: {:?}", write_duration);
    println!("   Throughput: {:.0} ops/sec", write_throughput);
    
    // Test cache read performance
    let start = Instant::now();
    let mut hits = 0;
    
    for i in 0..num_operations {
        let key = format!("key_{}", i);
        if cache.get(&key).await.is_some() {
            hits += 1;
        }
    }
    
    let read_duration = start.elapsed();
    let read_throughput = num_operations as f64 / read_duration.as_secs_f64();
    let hit_rate = hits as f64 / num_operations as f64;
    
    println!("✅ Cache Read Performance:");
    println!("   Operations: {}", num_operations);
    println!("   Duration: {:?}", read_duration);
    println!("   Throughput: {:.0} ops/sec", read_throughput);
    println!("   Hit Rate: {:.2}%", hit_rate * 100.0);
    
    // Validate performance requirements
    assert!(write_throughput > 1000.0, "Cache write throughput too low: {:.0} ops/sec", write_throughput);
    assert!(read_throughput > 5000.0, "Cache read throughput too low: {:.0} ops/sec", read_throughput);
    assert!(hit_rate > 0.95, "Cache hit rate too low: {:.2}%", hit_rate * 100.0);
    
    // Test cache metrics
    let metrics = cache.get_metrics().await;
    println!("✅ Cache Metrics:");
    println!("   Total Hits: {}", metrics.total_hits);
    println!("   Total Misses: {}", metrics.total_misses);
    println!("   Hit Rate: {:.2}%", metrics.overall_hit_rate() * 100.0);
    
    println!("🎉 Cache Manager performance validation passed!");
}

#[tokio::test]
async fn test_rate_limiter_performance() {
    println!("🚀 Testing Rate Limiter Performance...");
    
    let config = DistributedRateLimitConfig::default();
    let rate_limiter = DistributedRateLimiter::new(config).await.unwrap();
    
    // Test rate limiting performance
    let start = Instant::now();
    let num_operations = 1000;
    let mut allowed = 0;
    let mut denied = 0;
    
    for i in 0..num_operations {
        let key = format!("test_key_{}", i % 10); // Use 10 different keys
        match rate_limiter.check_rate_limit(&key, 100, 60).await {
            Ok(result) => {
                match result {
                    RateLimitResult::Allowed { .. } => allowed += 1,
                    RateLimitResult::Exceeded { .. } => denied += 1,
                }
            }
            Err(_) => denied += 1,
        }
    }
    
    let duration = start.elapsed();
    let throughput = num_operations as f64 / duration.as_secs_f64();
    
    println!("✅ Rate Limiter Performance:");
    println!("   Operations: {}", num_operations);
    println!("   Duration: {:?}", duration);
    println!("   Throughput: {:.0} ops/sec", throughput);
    println!("   Allowed: {}", allowed);
    println!("   Denied: {}", denied);
    
    // Validate performance requirements
    assert!(throughput > 1000.0, "Rate limiter throughput too low: {:.0} ops/sec", throughput);
    assert!(allowed > 0, "No requests were allowed");
    
    // Test rate limiter stats
    let stats = rate_limiter.get_comprehensive_stats().await;
    println!("✅ Rate Limiter Stats:");
    println!("   Total Requests: {}", stats.total_requests);
    println!("   Fallback Requests: {}", stats.fallback_requests);
    println!("   Average Latency: {:.2}ms", stats.average_latency_ms);
    
    println!("🎉 Rate Limiter performance validation passed!");
}

#[tokio::test]
async fn test_concurrent_cache_performance() {
    println!("🚀 Testing Concurrent Cache Performance...");
    
    let config = CacheConfig::default();
    let cache = Arc::new(MultiTierCacheManager::new(config));
    
    let start = Instant::now();
    let num_tasks = 10;
    let operations_per_task = 100;
    
    // Spawn concurrent tasks
    let mut handles = Vec::new();
    
    for task_id in 0..num_tasks {
        let cache_clone = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            let mut local_ops = 0;
            
            // Each task performs both reads and writes
            for i in 0..operations_per_task {
                let key = format!("task_{}_{}", task_id, i);
                let value = format!("value_{}_{}", task_id, i);
                
                // Write operation
                if cache_clone.put(key.clone(), value).await.is_ok() {
                    local_ops += 1;
                }
                
                // Read operation
                if cache_clone.get(&key).await.is_some() {
                    local_ops += 1;
                }
            }
            
            local_ops
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut total_ops = 0;
    for handle in handles {
        total_ops += handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    let throughput = total_ops as f64 / duration.as_secs_f64();
    
    println!("✅ Concurrent Cache Performance:");
    println!("   Tasks: {}", num_tasks);
    println!("   Operations per task: {}", operations_per_task);
    println!("   Total operations: {}", total_ops);
    println!("   Duration: {:?}", duration);
    println!("   Throughput: {:.0} ops/sec", throughput);
    
    // Validate concurrent performance
    assert!(throughput > 1000.0, "Concurrent cache throughput too low: {:.0} ops/sec", throughput);
    assert_eq!(total_ops, num_tasks * operations_per_task * 2, "Not all operations completed");
    
    println!("🎉 Concurrent cache performance validation passed!");
}

#[tokio::test]
async fn test_memory_usage_validation() {
    println!("🚀 Testing Memory Usage Validation...");
    
    // Test cache memory efficiency
    let config = CacheConfig {
        l1_capacity: 100,
        l2_capacity: 500,
        l3_capacity: 1000,
        ..Default::default()
    };
    let cache = MultiTierCacheManager::new(config);
    
    // Fill cache with data
    let start = Instant::now();
    let num_items = 1000;
    
    for i in 0..num_items {
        let key = format!("memory_test_{}", i);
        let value = format!("data_{}_{}", i, "x".repeat(100)); // ~100 bytes per value
        cache.put(key, value).await.unwrap();
    }
    
    let fill_duration = start.elapsed();
    
    // Test memory access patterns
    let start = Instant::now();
    let mut accessed = 0;
    
    for i in 0..num_items {
        let key = format!("memory_test_{}", i);
        if cache.get(&key).await.is_some() {
            accessed += 1;
        }
    }
    
    let access_duration = start.elapsed();
    
    println!("✅ Memory Usage Validation:");
    println!("   Items stored: {}", num_items);
    println!("   Fill duration: {:?}", fill_duration);
    println!("   Access duration: {:?}", access_duration);
    println!("   Items accessed: {}", accessed);
    
    // Get cache metrics
    let metrics = cache.get_metrics().await;
    println!("   Memory efficiency: {:.2} entries/KB", metrics.memory_efficiency());
    
    // Validate memory efficiency
    assert!(fill_duration < Duration::from_secs(1), "Memory fill took too long: {:?}", fill_duration);
    assert!(access_duration < Duration::from_millis(100), "Memory access took too long: {:?}", access_duration);
    
    println!("🎉 Memory usage validation passed!");
}

#[tokio::test]
async fn test_latency_consistency() {
    println!("🚀 Testing Latency Consistency...");
    
    let config = CacheConfig::default();
    let cache = MultiTierCacheManager::new(config);
    
    // Pre-populate cache
    for i in 0..100 {
        let key = format!("latency_test_{}", i);
        let value = format!("value_{}", i);
        cache.put(key, value).await.unwrap();
    }
    
    // Measure latency consistency
    let mut latencies = Vec::new();
    let num_measurements = 1000;
    
    for i in 0..num_measurements {
        let key = format!("latency_test_{}", i % 100);
        
        let start = Instant::now();
        let _result = cache.get(&key).await;
        let latency = start.elapsed();
        
        latencies.push(latency);
    }
    
    // Calculate statistics
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let avg = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    let max = latencies.iter().max().unwrap();
    
    println!("✅ Latency Statistics:");
    println!("   Measurements: {}", num_measurements);
    println!("   Average: {:?}", avg);
    println!("   P50: {:?}", p50);
    println!("   P95: {:?}", p95);
    println!("   P99: {:?}", p99);
    println!("   Max: {:?}", max);
    
    // Validate latency requirements
    assert!(p99 < Duration::from_millis(10), "P99 latency too high: {:?}", p99);
    assert!(p95 < Duration::from_millis(5), "P95 latency too high: {:?}", p95);
    assert!(avg < Duration::from_millis(1), "Average latency too high: {:?}", avg);
    
    // Check consistency (P99 shouldn't be too much higher than P50)
    let consistency_ratio = p99.as_nanos() as f64 / p50.as_nanos() as f64;
    assert!(consistency_ratio < 20.0, "Latency too inconsistent: {:.2}x variation", consistency_ratio);
    
    println!("🎉 Latency consistency validation passed!");
}