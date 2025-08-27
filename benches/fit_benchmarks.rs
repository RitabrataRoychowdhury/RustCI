//! Criterion-based benchmarks for Fingerprinted ID Table (FIT)
//!
//! This benchmark suite validates that FIT operations meet the performance requirements:
//! - Sub-82µs p99 latency for lookups
//! - 1M+ operations per second throughput
//! - Linear scalability with concurrent operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

// Import the FIT implementation
use RustAutoDevOps::valkyrie::lockfree::{
    fingerprinted_id_table::{FingerprintedIdTable, FitError, RouteEntry, RouteId, NodeId, ShardId},
    ServiceId
};

/// Generate test route entries for benchmarking
fn generate_route_entries(count: usize) -> Vec<RouteEntry> {
    (0..count)
        .map(|i| {
            RouteEntry::new(
                RouteId::new(i as u64),
                NodeId::new((i * 100) as u64),
                ShardId::new((i % 16) as u32),
                50 + (i % 100) as u32, // Latency hint: 50-149µs
                60000 + (i % 5535) as u16, // Reliability: 60000-65535
            )
        })
        .collect()
}

/// Benchmark FIT insertion operations
fn bench_fit_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("fit_insertion");
    
    for size in [100, 1000, 10000, 50000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("insert", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let fit = FingerprintedIdTable::with_defaults();
                        let entries = generate_route_entries(size);
                        (fit, entries)
                    },
                    |(fit, entries)| {
                        for entry in entries {
                            black_box(fit.insert(entry).unwrap());
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark FIT lookup operations (the critical hot path)
fn bench_fit_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("fit_lookup");
    
    for size in [100, 1000, 10000, 50000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("lookup", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let fit = FingerprintedIdTable::with_defaults();
                        let entries = generate_route_entries(size);
                        let route_ids: Vec<RouteId> = entries.iter().map(|e| e.id).collect();
                        
                        // Pre-populate the FIT
                        for entry in entries {
                            fit.insert(entry).unwrap();
                        }
                        
                        (fit, route_ids)
                    },
                    |(fit, route_ids)| {
                        for &route_id in &route_ids {
                            black_box(fit.lookup(route_id));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark single lookup latency (critical for p99 requirement)
fn bench_single_lookup_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_lookup_latency");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10000);
    
    // Test with different FIT sizes to ensure O(1) behavior
    for fit_size in [1000, 10000, 50000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::new("single_lookup", fit_size),
            fit_size,
            |b, &fit_size| {
                let fit = FingerprintedIdTable::with_defaults();
                let entries = generate_route_entries(fit_size);
                let lookup_id = entries[fit_size / 2].id; // Middle entry
                
                // Pre-populate the FIT
                for entry in entries {
                    fit.insert(entry).unwrap();
                }
                
                b.iter(|| {
                    black_box(fit.lookup(lookup_id));
                });
            },
        );
    }
    group.finish();
}

/// Benchmark FIT removal operations
fn bench_fit_removal(c: &mut Criterion) {
    let mut group = c.benchmark_group("fit_removal");
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("remove", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let fit = FingerprintedIdTable::with_defaults();
                        let entries = generate_route_entries(size);
                        let route_ids: Vec<RouteId> = entries.iter().map(|e| e.id).collect();
                        
                        // Pre-populate the FIT
                        for entry in entries {
                            fit.insert(entry).unwrap();
                        }
                        
                        (fit, route_ids)
                    },
                    |(fit, route_ids)| {
                        for route_id in route_ids {
                            black_box(fit.remove(route_id).unwrap());
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark mixed operations (realistic workload)
fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");
    
    group.bench_function("realistic_workload", |b| {
        b.iter_batched(
            || {
                let fit = FingerprintedIdTable::with_defaults();
                let initial_entries = generate_route_entries(10000);
                
                // Pre-populate with initial data
                for entry in &initial_entries {
                    fit.insert(entry.clone()).unwrap();
                }
                
                (fit, initial_entries)
            },
            |(fit, entries)| {
                // Simulate realistic workload: 80% lookups, 15% inserts, 5% removes
                for i in 0..1000 {
                    match i % 20 {
                        0..=15 => {
                            // 80% lookups
                            let lookup_id = entries[i % entries.len()].id;
                            black_box(fit.lookup(lookup_id));
                        }
                        16..=18 => {
                            // 15% inserts
                            let new_entry = RouteEntry::new(
                                RouteId::new(100000 + i as u64),
                                NodeId::new((100000 + i * 100) as u64),
                                ShardId::new((i % 16) as u32),
                                50,
                                65000,
                            );
                            let _ = fit.insert(new_entry);
                        }
                        19 => {
                            // 5% removes
                            if !entries.is_empty() {
                                let remove_id = entries[i % entries.len()].id;
                                let _ = fit.remove(remove_id);
                            }
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

/// Benchmark concurrent access patterns
fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    
    group.bench_function("concurrent_lookups", |b| {
        b.iter_batched(
            || {
                let fit = Arc::new(FingerprintedIdTable::with_defaults());
                let entries = generate_route_entries(10000);
                let route_ids: Vec<RouteId> = entries.iter().map(|e| e.id).collect();
                
                // Pre-populate the FIT
                for entry in entries {
                    fit.insert(entry).unwrap();
                }
                
                (fit, route_ids)
            },
            |(fit, route_ids)| {
                use std::thread;
                
                let mut handles = vec![];
                let num_threads = 4;
                let lookups_per_thread = route_ids.len() / num_threads;
                
                for i in 0..num_threads {
                    let fit_clone = Arc::clone(&fit);
                    let start_idx = i * lookups_per_thread;
                    let end_idx = if i == num_threads - 1 {
                        route_ids.len()
                    } else {
                        (i + 1) * lookups_per_thread
                    };
                    let thread_route_ids = route_ids[start_idx..end_idx].to_vec();
                    
                    let handle = thread::spawn(move || {
                        for &route_id in &thread_route_ids {
                            black_box(fit_clone.lookup(route_id));
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

/// Benchmark memory usage and cache efficiency
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    
    // Test with different load factors
    for load_factor in [0.5, 0.75, 0.9].iter() {
        group.bench_with_input(
            BenchmarkId::new("load_factor", (load_factor * 100.0) as u32),
            load_factor,
            |b, &load_factor| {
                b.iter_batched(
                    || {
                        let fit = FingerprintedIdTable::new(16384, load_factor).unwrap();
                        let max_entries = (16384.0 * load_factor) as usize;
                        let entries = generate_route_entries(max_entries);
                        (fit, entries)
                    },
                    |(fit, entries)| {
                        // Fill to capacity
                        for entry in entries {
                            let _ = fit.insert(entry);
                        }
                        
                        // Perform lookups to test cache efficiency
                        for i in 0..1000 {
                            let lookup_id = RouteId::new(i % fit.size() as u64);
                            black_box(fit.lookup(lookup_id));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    
    group.finish();
}

/// Benchmark performance under stress conditions
fn bench_stress_conditions(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_conditions");
    group.measurement_time(Duration::from_secs(15));
    
    group.bench_function("high_collision_rate", |b| {
        b.iter_batched(
            || {
                let fit = FingerprintedIdTable::new(1024, 0.9).unwrap(); // High load factor
                let entries: Vec<RouteEntry> = (0..900) // Near capacity
                    .map(|i| {
                        RouteEntry::new(
                            RouteId::new(i as u64),
                            NodeId::new(i as u64),
                            ShardId::new(0), // Same shard to increase collisions
                            50,
                            65000,
                        )
                    })
                    .collect();
                (fit, entries)
            },
            |(fit, entries)| {
                // Insert entries (may cause collisions)
                for entry in entries {
                    let _ = fit.insert(entry);
                }
                
                // Perform lookups
                for i in 0..900 {
                    black_box(fit.lookup(RouteId::new(i)));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    
    group.finish();
}

criterion_group!(
    fit_benches,
    bench_fit_insertion,
    bench_fit_lookup,
    bench_single_lookup_latency,
    bench_fit_removal,
    bench_mixed_operations,
    bench_concurrent_access,
    bench_memory_efficiency,
    bench_stress_conditions
);

criterion_main!(fit_benches);