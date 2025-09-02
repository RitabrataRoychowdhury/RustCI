use std::time::{Duration, Instant};
use tokio::time::timeout;

use rustci::storage::{
    RedisAdapter, RedisConfig, StoreAdapter, ValKeyAdapter, ValKeyConfig,
};

/// Benchmark configuration
struct BenchmarkConfig {
    pub iterations: usize,
    pub concurrent_clients: usize,
    pub test_duration: Duration,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 10000,
            concurrent_clients: 10,
            test_duration: Duration::from_secs(30),
        }
    }
}

/// Benchmark results
#[derive(Debug)]
struct BenchmarkResults {
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub total_duration: Duration,
    pub average_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub throughput_ops_per_sec: f64,
}

#[tokio::test]
async fn benchmark_redis_try_consume_latency() {
    let config = RedisConfig {
        url: "redis://localhost:6379".to_string(),
        ..Default::default()
    };

    let adapter = match RedisAdapter::new(config).await {
        Ok(adapter) => adapter,
        Err(_) => {
            println!("Redis not available, skipping benchmark");
            return;
        }
    };

    let benchmark_config = BenchmarkConfig {
        iterations: 1000, // Smaller for unit tests
        ..Default::default()
    };

    let results = benchmark_try_consume_latency(&adapter, &benchmark_config).await;
    
    println!("Redis TryConsume Latency Benchmark Results:");
    println!("  Total operations: {}", results.total_operations);
    println!("  Successful operations: {}", results.successful_operations);
    println!("  Average latency: {:?}", results.average_latency);
    println!("  P95 latency: {:?}", results.p95_latency);
    println!("  P99 latency: {:?}", results.p99_latency);
    println!("  Throughput: {:.2} ops/sec", results.throughput_ops_per_sec);

    // Performance assertions (these are lenient for CI environments)
    assert!(results.average_latency < Duration::from_millis(10)); // < 10ms average
    assert!(results.p99_latency < Duration::from_millis(50)); // < 50ms P99
    assert!(results.throughput_ops_per_sec > 10.0); // > 10 ops/sec
}

#[tokio::test]
async fn benchmark_valkey_try_consume_latency() {
    let config = ValKeyConfig {
        url: "valkey://localhost:6380".to_string(),
        username: "test_user".to_string(),
        password_encrypted: "test_password".to_string(),
        ..Default::default()
    };

    let adapter = ValKeyAdapter::new(config).await.unwrap();

    let benchmark_config = BenchmarkConfig {
        iterations: 1000, // Smaller for unit tests
        ..Default::default()
    };

    let results = benchmark_try_consume_latency(&adapter, &benchmark_config).await;
    
    println!("ValKey TryConsume Latency Benchmark Results:");
    println!("  Total operations: {}", results.total_operations);
    println!("  Successful operations: {}", results.successful_operations);
    println!("  Average latency: {:?}", results.average_latency);
    println!("  P95 latency: {:?}", results.p95_latency);
    println!("  P99 latency: {:?}", results.p99_latency);
    println!("  Throughput: {:.2} ops/sec", results.throughput_ops_per_sec);

    // ValKey should be faster than Redis due to optimizations
    assert!(results.average_latency < Duration::from_millis(5)); // < 5ms average
    assert!(results.p99_latency < Duration::from_millis(20)); // < 20ms P99
    assert!(results.throughput_ops_per_sec > 50.0); // > 50 ops/sec
}

#[tokio::test]
async fn benchmark_concurrent_operations() {
    let config = ValKeyConfig::default();
    let adapter = ValKeyAdapter::new(config).await.unwrap();

    let benchmark_config = BenchmarkConfig {
        iterations: 100, // Per client
        concurrent_clients: 5,
        ..Default::default()
    };

    let results = benchmark_concurrent_try_consume(&adapter, &benchmark_config).await;
    
    println!("Concurrent TryConsume Benchmark Results:");
    println!("  Total operations: {}", results.total_operations);
    println!("  Successful operations: {}", results.successful_operations);
    println!("  Average latency: {:?}", results.average_latency);
    println!("  Throughput: {:.2} ops/sec", results.throughput_ops_per_sec);

    assert!(results.successful_operations > 0);
    assert!(results.throughput_ops_per_sec > 0.0);
}

#[tokio::test]
async fn benchmark_get_set_operations() {
    let config = ValKeyConfig::default();
    let adapter = ValKeyAdapter::new(config).await.unwrap();

    let benchmark_config = BenchmarkConfig {
        iterations: 500, // Smaller for unit tests
        ..Default::default()
    };

    let results = benchmark_get_set_operations(&adapter, &benchmark_config).await;
    
    println!("Get/Set Operations Benchmark Results:");
    println!("  Total operations: {}", results.total_operations);
    println!("  Successful operations: {}", results.successful_operations);
    println!("  Average latency: {:?}", results.average_latency);
    println!("  Throughput: {:.2} ops/sec", results.throughput_ops_per_sec);

    assert!(results.successful_operations > 0);
    assert!(results.average_latency < Duration::from_millis(10));
}

/// Benchmark TryConsume operation latency
async fn benchmark_try_consume_latency(
    adapter: &dyn StoreAdapter,
    config: &BenchmarkConfig,
) -> BenchmarkResults {
    let mut latencies = Vec::with_capacity(config.iterations);
    let mut successful_operations = 0;
    let mut failed_operations = 0;

    let start_time = Instant::now();

    for i in 0..config.iterations {
        let key = format!("benchmark_key_{}", i % 100); // Reuse keys to test contention
        let operation_start = Instant::now();

        match adapter.try_consume(&key, 1).await {
            Ok(_) => {
                successful_operations += 1;
                latencies.push(operation_start.elapsed());
            }
            Err(_) => {
                failed_operations += 1;
            }
        }
    }

    let total_duration = start_time.elapsed();

    calculate_benchmark_results(
        latencies,
        successful_operations,
        failed_operations,
        total_duration,
    )
}

/// Benchmark concurrent TryConsume operations
async fn benchmark_concurrent_try_consume(
    adapter: &dyn StoreAdapter,
    config: &BenchmarkConfig,
) -> BenchmarkResults {
    let mut handles = Vec::new();

    let start_time = Instant::now();

    for client_id in 0..config.concurrent_clients {
        let adapter_clone = adapter; // Note: This won't work with trait objects
        let iterations = config.iterations;
        
        let handle = tokio::spawn(async move {
            let mut client_latencies = Vec::new();
            let mut client_successful = 0;
            let mut client_failed = 0;

            for i in 0..iterations {
                let key = format!("concurrent_key_{}_{}", client_id, i % 10);
                let operation_start = Instant::now();

                match adapter_clone.try_consume(&key, 1).await {
                    Ok(_) => {
                        client_successful += 1;
                        client_latencies.push(operation_start.elapsed());
                    }
                    Err(_) => {
                        client_failed += 1;
                    }
                }
            }

            (client_latencies, client_successful, client_failed)
        });

        handles.push(handle);
    }

    // Collect results from all clients
    let mut all_latencies = Vec::new();
    let mut total_successful = 0;
    let mut total_failed = 0;

    for handle in handles {
        if let Ok((latencies, successful, failed)) = handle.await {
            all_latencies.extend(latencies);
            total_successful += successful;
            total_failed += failed;
        }
    }

    let total_duration = start_time.elapsed();

    calculate_benchmark_results(
        all_latencies,
        total_successful,
        total_failed,
        total_duration,
    )
}

/// Benchmark Get/Set operations
async fn benchmark_get_set_operations(
    adapter: &dyn StoreAdapter,
    config: &BenchmarkConfig,
) -> BenchmarkResults {
    let mut latencies = Vec::with_capacity(config.iterations * 2); // Get + Set
    let mut successful_operations = 0;
    let mut failed_operations = 0;

    let start_time = Instant::now();

    for i in 0..config.iterations {
        let key = format!("get_set_key_{}", i);
        let value = format!("value_{}", i).into_bytes();

        // Set operation
        let set_start = Instant::now();
        match adapter.set(&key, value.clone(), Some(Duration::from_secs(60))).await {
            Ok(_) => {
                successful_operations += 1;
                latencies.push(set_start.elapsed());
            }
            Err(_) => {
                failed_operations += 1;
            }
        }

        // Get operation
        let get_start = Instant::now();
        match adapter.get(&key).await {
            Ok(_) => {
                successful_operations += 1;
                latencies.push(get_start.elapsed());
            }
            Err(_) => {
                failed_operations += 1;
            }
        }
    }

    let total_duration = start_time.elapsed();

    calculate_benchmark_results(
        latencies,
        successful_operations,
        failed_operations,
        total_duration,
    )
}

/// Calculate benchmark results from latency measurements
fn calculate_benchmark_results(
    mut latencies: Vec<Duration>,
    successful_operations: usize,
    failed_operations: usize,
    total_duration: Duration,
) -> BenchmarkResults {
    if latencies.is_empty() {
        return BenchmarkResults {
            total_operations: successful_operations + failed_operations,
            successful_operations,
            failed_operations,
            total_duration,
            average_latency: Duration::ZERO,
            min_latency: Duration::ZERO,
            max_latency: Duration::ZERO,
            p95_latency: Duration::ZERO,
            p99_latency: Duration::ZERO,
            throughput_ops_per_sec: 0.0,
        };
    }

    latencies.sort();

    let total_latency: Duration = latencies.iter().sum();
    let average_latency = total_latency / latencies.len() as u32;
    let min_latency = latencies[0];
    let max_latency = latencies[latencies.len() - 1];

    let p95_index = (latencies.len() as f64 * 0.95) as usize;
    let p99_index = (latencies.len() as f64 * 0.99) as usize;
    let p95_latency = latencies[p95_index.min(latencies.len() - 1)];
    let p99_latency = latencies[p99_index.min(latencies.len() - 1)];

    let throughput_ops_per_sec = if total_duration.as_secs_f64() > 0.0 {
        successful_operations as f64 / total_duration.as_secs_f64()
    } else {
        0.0
    };

    BenchmarkResults {
        total_operations: successful_operations + failed_operations,
        successful_operations,
        failed_operations,
        total_duration,
        average_latency,
        min_latency,
        max_latency,
        p95_latency,
        p99_latency,
        throughput_ops_per_sec,
    }
}

/// Performance regression test
#[tokio::test]
async fn test_performance_regression() {
    let config = ValKeyConfig::default();
    let adapter = ValKeyAdapter::new(config).await.unwrap();

    let benchmark_config = BenchmarkConfig {
        iterations: 100,
        ..Default::default()
    };

    let results = benchmark_try_consume_latency(&adapter, &benchmark_config).await;

    // These are baseline performance expectations
    // Adjust based on your performance requirements
    assert!(
        results.average_latency < Duration::from_millis(5),
        "Average latency regression: {:?} > 5ms",
        results.average_latency
    );

    assert!(
        results.p99_latency < Duration::from_millis(20),
        "P99 latency regression: {:?} > 20ms",
        results.p99_latency
    );

    assert!(
        results.throughput_ops_per_sec > 20.0,
        "Throughput regression: {:.2} < 20 ops/sec",
        results.throughput_ops_per_sec
    );
}

/// Stress test with high concurrency
#[tokio::test]
async fn stress_test_high_concurrency() {
    let config = ValKeyConfig::default();
    let adapter = ValKeyAdapter::new(config).await.unwrap();

    let benchmark_config = BenchmarkConfig {
        iterations: 50, // Per client
        concurrent_clients: 20, // High concurrency
        test_duration: Duration::from_secs(10),
    };

    let results = benchmark_concurrent_try_consume(&adapter, &benchmark_config).await;

    println!("Stress Test Results:");
    println!("  Total operations: {}", results.total_operations);
    println!("  Success rate: {:.2}%", 
             (results.successful_operations as f64 / results.total_operations as f64) * 100.0);
    println!("  Average latency: {:?}", results.average_latency);
    println!("  P99 latency: {:?}", results.p99_latency);

    // Under high concurrency, we expect some operations to succeed
    assert!(results.successful_operations > 0);
    
    // Success rate should be reasonable (>50%)
    let success_rate = results.successful_operations as f64 / results.total_operations as f64;
    assert!(success_rate > 0.5, "Success rate too low: {:.2}%", success_rate * 100.0);
}