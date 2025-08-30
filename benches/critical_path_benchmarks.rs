use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

use rustci::core::performance::{
    CriticalPathOptimizer, OptimizerConfig, BatchOperation, OperationType
};
use rustci::infrastructure::runners::valkyrie_performance_optimizer::{
    ValkyriePerformanceOptimizer, ValkyrieOptimizerConfig
};
use rustci::infrastructure::runners::valkyrie_adapter::{
    ValkyrieJob, JobType, JobPriority, JobPayload
};
use rustci::presentation::middleware::response_optimizer::{
    ResponseOptimizer, ResponseOptimizerConfig
};

/// Benchmark critical path optimizer performance
fn bench_critical_path_optimizer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("critical_path_optimizer");
    
    // Test different batch sizes
    for batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_processing", batch_size),
            batch_size,
            |b, &batch_size| {
                let config = OptimizerConfig {
                    batch_size,
                    batch_timeout: Duration::from_millis(1),
                    ..Default::default()
                };
                let optimizer = CriticalPathOptimizer::new(config);
                
                b.to_async(&rt).iter(|| async {
                    let operations: Vec<_> = (0..batch_size).map(|i| {
                        BatchOperation {
                            id: Uuid::new_v4(),
                            operation_type: OperationType::DatabaseWrite,
                            data: format!("test_data_{}", i).into_bytes(),
                            created_at: std::time::Instant::now(),
                            priority: 1,
                        }
                    }).collect();
                    
                    for operation in operations {
                        optimizer.optimize_batch_operation(black_box(operation)).await.unwrap();
                    }
                    
                    optimizer.flush_all().await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark cache performance
fn bench_cache_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_performance");
    
    // Test different data sizes
    for data_size in [1024, 10240, 102400].iter() {
        group.bench_with_input(
            BenchmarkId::new("cache_operations", data_size),
            data_size,
            |b, &data_size| {
                let config = OptimizerConfig {
                    cache_size: 1000,
                    cache_ttl: Duration::from_secs(300),
                    ..Default::default()
                };
                let optimizer = CriticalPathOptimizer::new(config);
                
                let test_data = vec![0u8; data_size];
                
                b.to_async(&rt).iter(|| async {
                    let key = format!("test_key_{}", fastrand::u64(..));
                    
                    // Cache data
                    optimizer.cache_data(black_box(key.clone()), black_box(test_data.clone())).await.unwrap();
                    
                    // Retrieve data
                    let _retrieved = optimizer.get_cached_data(black_box(&key)).await;
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark compression performance
fn bench_compression_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("compression_performance");
    
    // Test different data sizes and types
    let test_cases = vec![
        ("json_1kb", generate_json_data(1024)),
        ("json_10kb", generate_json_data(10240)),
        ("text_1kb", generate_text_data(1024)),
        ("text_10kb", generate_text_data(10240)),
        ("binary_1kb", generate_binary_data(1024)),
        ("binary_10kb", generate_binary_data(10240)),
    ];
    
    for (name, data) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("compression", name),
            &data,
            |b, data| {
                let config = OptimizerConfig {
                    compression_threshold: 512,
                    compression_level: 6,
                    ..Default::default()
                };
                let optimizer = CriticalPathOptimizer::new(config);
                
                b.to_async(&rt).iter(|| async {
                    let key = format!("test_key_{}", fastrand::u64(..));
                    optimizer.cache_data(black_box(key), black_box(data.clone())).await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark Valkyrie adapter performance
fn bench_valkyrie_adapter_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("valkyrie_adapter");
    
    // Test different job types and sizes
    let job_configs = vec![
        ("small_build", JobType::Build, JobPayload::Small(vec![0u8; 1024])),
        ("large_build", JobType::Build, JobPayload::Small(vec![0u8; 10240])),
        ("small_test", JobType::Test, JobPayload::Small(vec![0u8; 1024])),
        ("analysis", JobType::Analysis, JobPayload::Small(vec![0u8; 5120])),
    ];
    
    for (name, job_type, payload) in job_configs {
        group.bench_with_input(
            BenchmarkId::new("job_dispatch", name),
            &(job_type, payload),
            |b, (job_type, payload)| {
                let config = ValkyrieOptimizerConfig {
                    dispatch_batch_size: 10,
                    enable_adaptive_batching: true,
                    enable_zero_copy: true,
                    enable_simd_processing: true,
                    ..Default::default()
                };
                let optimizer = ValkyriePerformanceOptimizer::new(config);
                
                b.to_async(&rt).iter(|| async {
                    let job = ValkyrieJob {
                        id: Uuid::new_v4(),
                        job_type: job_type.clone(),
                        priority: JobPriority::Normal,
                        payload: payload.clone(),
                        ..Default::default()
                    };
                    
                    optimizer.dispatch_optimized_job(black_box(job)).await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark response optimization
fn bench_response_optimization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("response_optimization");
    
    // Test different response sizes
    for response_size in [1024, 10240, 102400].iter() {
        group.bench_with_input(
            BenchmarkId::new("response_processing", response_size),
            response_size,
            |b, &response_size| {
                let config = ResponseOptimizerConfig {
                    enable_caching: true,
                    enable_compression: true,
                    compression_threshold: 1024,
                    ..Default::default()
                };
                let optimizer = ResponseOptimizer::new(config);
                
                let test_response = generate_json_data(response_size);
                
                b.to_async(&rt).iter(|| async {
                    // Simulate response optimization
                    let key = format!("response_key_{}", fastrand::u64(..));
                    
                    // This would normally be done through middleware
                    // For benchmarking, we simulate the key operations
                    let _cached = optimizer.get_cached_data(&key).await;
                    optimizer.cache_data(key, black_box(test_response.clone())).await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    
    // Test different concurrency levels
    for concurrency in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_batch_ops", concurrency),
            concurrency,
            |b, &concurrency| {
                let config = OptimizerConfig {
                    batch_size: 20,
                    parallel_workers: concurrency,
                    ..Default::default()
                };
                let optimizer = CriticalPathOptimizer::new(config);
                
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for i in 0..concurrency {
                        let optimizer_clone = optimizer.clone();
                        let handle = tokio::spawn(async move {
                            let operation = BatchOperation {
                                id: Uuid::new_v4(),
                                operation_type: OperationType::ApiRequest,
                                data: format!("concurrent_data_{}", i).into_bytes(),
                                created_at: std::time::Instant::now(),
                                priority: 1,
                            };
                            
                            optimizer_clone.optimize_batch_operation(black_box(operation)).await.unwrap();
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.await.unwrap();
                    }
                    
                    optimizer.flush_all().await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

/// Generate test JSON data
fn generate_json_data(size: usize) -> Vec<u8> {
    let mut data = String::from(r#"{"users":["#);
    
    let entry = r#"{"id":1,"name":"John Doe","email":"john@example.com","active":true},"#;
    let entries_needed = (size.saturating_sub(20)) / entry.len();
    
    for i in 0..entries_needed {
        if i > 0 {
            data.push(',');
        }
        data.push_str(&entry.replace("1", &i.to_string()));
    }
    
    data.push_str("]}");
    data.into_bytes()
}

/// Generate test text data
fn generate_text_data(size: usize) -> Vec<u8> {
    let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ";
    let repeats = (size / text.len()) + 1;
    text.repeat(repeats)[..size].to_string().into_bytes()
}

/// Generate test binary data
fn generate_binary_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

criterion_group!(
    benches,
    bench_critical_path_optimizer,
    bench_cache_performance,
    bench_compression_performance,
    bench_valkyrie_adapter_performance,
    bench_response_optimization,
    bench_concurrent_operations
);

criterion_main!(benches);