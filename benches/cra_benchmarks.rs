//! Criterion-based benchmarks for Compressed Radix Arena (CRA)
//!
//! This benchmark suite validates CRA performance for prefix-based routing:
//! - Fast prefix insertion and lookup operations
//! - Memory-efficient compressed storage
//! - SIMD-optimized search operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::time::Duration;

// Import the CRA implementation
use RustAutoDevOps::valkyrie::lockfree::{
    compressed_radix_arena::{CompressedRadixArena, CraError},
    ServiceId
};

/// Generate test prefixes for benchmarking
fn generate_prefixes(count: usize) -> Vec<(String, u64)> {
    let mut prefixes = Vec::new();
    
    // Generate realistic service name prefixes
    let base_names = [
        "user-service", "auth-service", "payment-service", "notification-service",
        "analytics-service", "order-service", "inventory-service", "shipping-service",
        "customer-service", "billing-service", "reporting-service", "monitoring-service"
    ];
    
    for i in 0..count {
        let base = &base_names[i % base_names.len()];
        let prefix = if i < base_names.len() {
            base.to_string()
        } else {
            format!("{}-{}", base, i / base_names.len())
        };
        prefixes.push((prefix, i as u64));
    }
    
    prefixes
}

/// Generate hierarchical prefixes (realistic for service discovery)
fn generate_hierarchical_prefixes(count: usize) -> Vec<(String, u64)> {
    let mut prefixes = Vec::new();
    
    let environments = ["prod", "staging", "dev"];
    let regions = ["us-east", "us-west", "eu-central", "ap-southeast"];
    let services = ["api", "worker", "db", "cache", "queue"];
    
    for i in 0..count {
        let env = environments[i % environments.len()];
        let region = regions[(i / environments.len()) % regions.len()];
        let service = services[(i / (environments.len() * regions.len())) % services.len()];
        let instance = i / (environments.len() * regions.len() * services.len());
        
        let prefix = format!("{}.{}.{}.{}", env, region, service, instance);
        prefixes.push((prefix, i as u64));
    }
    
    prefixes
}

/// Benchmark CRA insertion operations
fn bench_cra_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("cra_insertion");
    
    for size in [100, 1000, 10000, 50000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("insert", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut cra = CompressedRadixArena::new();
                        let prefixes = generate_prefixes(size);
                        (cra, prefixes)
                    },
                    |(mut cra, prefixes)| {
                        for (prefix, value) in prefixes {
                            black_box(cra.insert(&prefix, ServiceId(value)).unwrap());
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark CRA lookup operations
fn bench_cra_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cra_lookup");
    
    for size in [100, 1000, 10000, 50000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("lookup", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut cra = CompressedRadixArena::new();
                        let prefixes = generate_prefixes(size);
                        let lookup_keys: Vec<String> = prefixes.iter().map(|(k, _)| k.clone()).collect();
                        
                        // Pre-populate the CRA
                        for (prefix, value) in prefixes {
                            cra.insert(&prefix, ServiceId(value)).unwrap();
                        }
                        
                        (cra, lookup_keys)
                    },
                    |(cra, lookup_keys)| {
                        for key in &lookup_keys {
                            black_box(cra.find_readonly(key));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark prefix search operations (critical for service discovery)
fn bench_prefix_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefix_search");
    
    for size in [1000, 10000, 50000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("prefix_search", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut cra = CompressedRadixArena::new();
                        let prefixes = generate_hierarchical_prefixes(size);
                        
                        // Pre-populate the CRA
                        for (prefix, value) in prefixes {
                            cra.insert(&prefix, ServiceId(value)).unwrap();
                        }
                        
                        // Generate search prefixes
                        let search_prefixes = vec![
                            "prod.us-east".to_string(),
                            "staging".to_string(),
                            "prod.eu-central.api".to_string(),
                            "dev.us-west.worker".to_string(),
                        ];
                        
                        (cra, search_prefixes)
                    },
                    |(cra, search_prefixes)| {
                        for prefix in &search_prefixes {
                            black_box(cra.find_prefix_readonly(prefix));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark single lookup latency
fn bench_single_lookup_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_lookup_latency");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10000);
    
    // Test with different CRA sizes to ensure logarithmic behavior
    for cra_size in [1000, 10000, 50000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::new("single_lookup", cra_size),
            cra_size,
            |b, &cra_size| {
                let mut cra = CompressedRadixArena::new();
                let prefixes = generate_prefixes(cra_size);
                let lookup_key = prefixes[cra_size / 2].0.clone(); // Middle entry
                
                // Pre-populate the CRA
                for (prefix, value) in prefixes {
                    cra.insert(&prefix, ServiceId(value)).unwrap();
                }
                
                b.iter(|| {
                    black_box(cra.find_readonly(&lookup_key));
                });
            },
        );
    }
    group.finish();
}

/// Benchmark memory compression efficiency
fn bench_compression_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_efficiency");
    
    group.bench_function("common_prefix_compression", |b| {
        b.iter_batched(
            || {
                let mut cra = CompressedRadixArena::new();
                
                // Generate prefixes with common prefixes (should compress well)
                let mut prefixes = Vec::new();
                for i in 0..10000 {
                    let prefix = format!("com.example.service.module.component.{}", i);
                    prefixes.push((prefix, i as u64));
                }
                
                (cra, prefixes)
            },
            |(mut cra, prefixes)| {
                for (prefix, value) in prefixes {
                    black_box(cra.insert(&prefix, ServiceId(value)).unwrap());
                }
                
                // Perform some lookups to test compressed access
                for i in 0..1000 {
                    let lookup_key = format!("com.example.service.module.component.{}", i);
                    black_box(cra.find_readonly(&lookup_key));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    
    group.finish();
}

/// Benchmark concurrent access patterns
fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    
    group.bench_function("concurrent_lookups", |b| {
        b.iter_batched(
            || {
                let mut cra = CompressedRadixArena::new();
                let prefixes = generate_prefixes(10000);
                let lookup_keys: Vec<String> = prefixes.iter().map(|(k, _)| k.clone()).collect();
                
                // Pre-populate the CRA
                for (prefix, value) in prefixes {
                    cra.insert(&prefix, ServiceId(value)).unwrap();
                }
                
                (Arc::new(cra), lookup_keys)
            },
            |(cra, lookup_keys)| {
                use std::thread;
                
                let mut handles = vec![];
                let num_threads = 4;
                let lookups_per_thread = lookup_keys.len() / num_threads;
                
                for i in 0..num_threads {
                    let cra_clone = Arc::clone(&cra);
                    let start_idx = i * lookups_per_thread;
                    let end_idx = if i == num_threads - 1 {
                        lookup_keys.len()
                    } else {
                        (i + 1) * lookups_per_thread
                    };
                    let thread_keys = lookup_keys[start_idx..end_idx].to_vec();
                    
                    let handle = thread::spawn(move || {
                        for key in &thread_keys {
                            black_box(cra_clone.find_readonly(key));
                        }
                    });
                    
                    handles.push(handle);
                }
                
                for handle in handles {
                    handle.join().unwrap();
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    
    group.finish();
}

/// Benchmark SIMD-optimized operations
fn bench_simd_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_operations");
    
    group.bench_function("simd_prefix_matching", |b| {
        b.iter_batched(
            || {
                let mut cra = CompressedRadixArena::new();
                
                // Generate prefixes optimized for SIMD processing
                let mut prefixes = Vec::new();
                for i in 0..10000 {
                    // Create prefixes with patterns that benefit from SIMD
                    let prefix = format!("service-{:08x}", i);
                    prefixes.push((prefix, i as u64));
                }
                
                // Pre-populate the CRA
                for (prefix, value) in &prefixes {
                    cra.insert(prefix, ServiceId(*value)).unwrap();
                }
                
                // Generate search patterns
                let search_patterns = vec![
                    "service-0000".to_string(),
                    "service-1111".to_string(),
                    "service-aaaa".to_string(),
                    "service-ffff".to_string(),
                ];
                
                (cra, search_patterns)
            },
            |(cra, search_patterns)| {
                for pattern in &search_patterns {
                    black_box(cra.find_prefix_readonly(pattern));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    
    group.finish();
}

/// Benchmark mixed operations (realistic workload)
fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");
    
    group.bench_function("realistic_workload", |b| {
        b.iter_batched(
            || {
                let mut cra = CompressedRadixArena::new();
                let initial_prefixes = generate_hierarchical_prefixes(5000);
                
                // Pre-populate with initial data
                for (prefix, value) in &initial_prefixes {
                    cra.insert(prefix, ServiceId(*value)).unwrap();
                }
                
                (cra, initial_prefixes)
            },
            |(mut cra, prefixes)| {
                // Simulate realistic workload: 70% lookups, 20% prefix searches, 10% inserts
                for i in 0..1000 {
                    match i % 10 {
                        0..=6 => {
                            // 70% exact lookups
                            let lookup_key = &prefixes[i % prefixes.len()].0;
                            black_box(cra.find_readonly(lookup_key));
                        }
                        7..=8 => {
                            // 20% prefix searches
                            let search_prefix = if i % 4 == 0 {
                                "prod"
                            } else if i % 4 == 1 {
                                "staging.us-east"
                            } else if i % 4 == 2 {
                                "dev.eu-central.api"
                            } else {
                                "prod.us-west.worker"
                            };
                            black_box(cra.find_prefix_readonly(search_prefix));
                        }
                        9 => {
                            // 10% inserts
                            let new_prefix = format!("dynamic.service.{}", i);
                            let _ = cra.insert(&new_prefix, ServiceId(i as u64));
                        }
                        _ => unreachable!(),
                    }
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    
    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    
    // Test different prefix patterns for memory efficiency
    let test_cases = vec![
        ("short_prefixes", generate_prefixes(10000)),
        ("long_prefixes", (0..10000).map(|i| {
            (format!("very.long.hierarchical.service.name.with.many.components.{}", i), i as u64)
        }).collect()),
        ("hierarchical", generate_hierarchical_prefixes(10000)),
    ];
    
    for (name, prefixes) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("memory_pattern", name),
            &prefixes,
            |b, prefixes| {
                b.iter_batched(
                    || CompressedRadixArena::new(),
                    |mut cra| {
                        for (prefix, value) in prefixes {
                            black_box(cra.insert(prefix, ServiceId(*value)).unwrap());
                        }
                        
                        // Perform lookups to test access patterns
                        for i in 0..1000 {
                            let lookup_key = &prefixes[i % prefixes.len()].0;
                            black_box(cra.find_readonly(lookup_key));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    cra_benches,
    bench_cra_insertion,
    bench_cra_lookup,
    bench_prefix_search,
    bench_single_lookup_latency,
    bench_compression_efficiency,
    bench_concurrent_access,
    bench_simd_operations,
    bench_mixed_operations,
    bench_memory_patterns
);

criterion_main!(cra_benches);