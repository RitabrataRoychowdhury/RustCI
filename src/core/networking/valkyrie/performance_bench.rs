//! Performance benchmarks for Valkyrie Protocol optimizations
//!
//! This module provides benchmarking utilities to measure the performance
//! improvements from SIMD and zero-copy optimizations.

use crate::core::networking::valkyrie::simd_processor::{SimdMessageProcessor, SimdPatternMatcher};
use crate::core::networking::valkyrie::types::{
    DestinationType, MessagePayload, MessageType, ValkyrieMessage,
};
use crate::core::networking::valkyrie::zero_copy::{SimdDataProcessor, ZeroCopyBufferPool};
use crate::error::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Performance benchmark suite for Valkyrie optimizations
pub struct PerformanceBenchmark {
    /// Buffer pool for zero-copy operations
    buffer_pool: Arc<ZeroCopyBufferPool>,
    /// SIMD data processor
    data_processor: Arc<SimdDataProcessor>,
    /// SIMD message processor
    message_processor: Arc<SimdMessageProcessor>,
    /// Pattern matcher
    pattern_matcher: Arc<SimdPatternMatcher>,
}

/// Benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Test name
    pub test_name: String,
    /// Number of operations performed
    pub operations: u64,
    /// Total time taken
    pub total_time: Duration,
    /// Operations per second
    pub ops_per_second: f64,
    /// Throughput in MB/s
    pub throughput_mbps: f64,
    /// Average latency per operation
    pub avg_latency: Duration,
    /// Memory usage (bytes)
    pub memory_usage: usize,
    /// SIMD operations performed
    pub simd_operations: u64,
}

impl PerformanceBenchmark {
    /// Create a new performance benchmark suite
    pub fn new() -> Self {
        let buffer_pool = Arc::new(ZeroCopyBufferPool::new());
        let data_processor = Arc::new(SimdDataProcessor::new(Arc::clone(&buffer_pool)));
        let message_processor = Arc::new(SimdMessageProcessor::new(Arc::clone(&buffer_pool)));
        let pattern_matcher = Arc::new(SimdPatternMatcher::new());

        Self {
            buffer_pool,
            data_processor,
            message_processor,
            pattern_matcher,
        }
    }

    /// Benchmark SIMD XOR operations
    pub fn benchmark_simd_xor(
        &self,
        data_size: usize,
        iterations: u64,
    ) -> Result<BenchmarkResults> {
        let data_a = vec![0xAA; data_size];
        let data_b = vec![0x55; data_size];
        let mut output = vec![0; data_size];

        let start_time = Instant::now();

        for _ in 0..iterations {
            self.data_processor
                .simd_xor(&data_a, &data_b, &mut output)?;
        }

        let total_time = start_time.elapsed();
        let total_bytes = data_size as u64 * iterations;

        Ok(BenchmarkResults {
            test_name: "SIMD XOR".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / iterations as u32,
            memory_usage: data_size * 3, // Three buffers
            simd_operations: iterations,
        })
    }

    /// Benchmark SIMD copy operations
    pub fn benchmark_simd_copy(
        &self,
        data_size: usize,
        iterations: u64,
    ) -> Result<BenchmarkResults> {
        let source_data = vec![0x42; data_size];
        let mut dest_data = vec![0; data_size];

        let start_time = Instant::now();

        for _ in 0..iterations {
            self.data_processor
                .simd_copy(&source_data, &mut dest_data)?;
        }

        let total_time = start_time.elapsed();
        let total_bytes = data_size as u64 * iterations;

        Ok(BenchmarkResults {
            test_name: "SIMD Copy".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / iterations as u32,
            memory_usage: data_size * 2, // Two buffers
            simd_operations: iterations,
        })
    }

    /// Benchmark SIMD checksum calculation
    pub fn benchmark_simd_checksum(
        &self,
        data_size: usize,
        iterations: u64,
    ) -> Result<BenchmarkResults> {
        let data = vec![0x33; data_size];

        let start_time = Instant::now();
        let mut total_checksum = 0u64;

        for _ in 0..iterations {
            total_checksum = total_checksum.wrapping_add(self.data_processor.simd_checksum(&data));
        }

        let total_time = start_time.elapsed();
        let total_bytes = data_size as u64 * iterations;

        // Prevent compiler optimization
        std::hint::black_box(total_checksum);

        Ok(BenchmarkResults {
            test_name: "SIMD Checksum".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / iterations as u32,
            memory_usage: data_size,
            simd_operations: iterations,
        })
    }

    /// Benchmark message processing with SIMD optimizations
    pub fn benchmark_message_processing(
        &self,
        message_size: usize,
        iterations: u64,
    ) -> Result<BenchmarkResults> {
        let payload_data = vec![0x77; message_size];
        let payload = MessagePayload::Binary(payload_data);

        let start_time = Instant::now();
        let mut processed_buffers = Vec::new();

        for _ in 0..iterations {
            let processed = self.message_processor.process_payload(&payload)?;
            processed_buffers.push(processed);
        }

        let total_time = start_time.elapsed();
        let total_bytes = message_size as u64 * iterations;

        // Calculate memory usage
        let memory_usage = processed_buffers
            .iter()
            .map(|b| b.capacity())
            .sum::<usize>();

        Ok(BenchmarkResults {
            test_name: "Message Processing".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / iterations as u32,
            memory_usage,
            simd_operations: iterations,
        })
    }

    /// Benchmark batch message processing
    pub fn benchmark_batch_processing(
        &self,
        batch_size: usize,
        message_size: usize,
    ) -> Result<BenchmarkResults> {
        let payload_data = vec![0x88; message_size];
        let messages: Vec<ValkyrieMessage> = (0..batch_size)
            .map(|_| {
                ValkyrieMessage::new(
                    MessageType::StreamData,
                    MessagePayload::Binary(payload_data.clone()),
                )
            })
            .collect();

        let start_time = Instant::now();
        let processed_buffers = self.message_processor.batch_process(&messages)?;
        let total_time = start_time.elapsed();

        let total_bytes = (message_size * batch_size) as u64;
        let memory_usage = processed_buffers
            .iter()
            .map(|b| b.capacity())
            .sum::<usize>();

        Ok(BenchmarkResults {
            test_name: "Batch Processing".to_string(),
            operations: batch_size as u64,
            total_time,
            ops_per_second: batch_size as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / batch_size as u32,
            memory_usage,
            simd_operations: batch_size as u64,
        })
    }

    /// Benchmark pattern matching with SIMD
    pub fn benchmark_pattern_matching(
        &self,
        data_size: usize,
        num_patterns: usize,
        iterations: u64,
    ) -> Result<BenchmarkResults> {
        // Add test patterns
        let mut matcher = SimdPatternMatcher::new();
        for i in 0..num_patterns {
            let pattern = format!("pattern{:04}", i);
            matcher.add_pattern(pattern.as_bytes(), None, i as u32);
        }

        // Create test data with some patterns embedded
        let mut test_data = vec![0x20; data_size]; // Space characters
        for i in 0..num_patterns.min(10) {
            let pattern = format!("pattern{:04}", i);
            let offset = (i * data_size / 10).min(data_size - pattern.len());
            test_data[offset..offset + pattern.len()].copy_from_slice(pattern.as_bytes());
        }

        let start_time = Instant::now();
        let mut total_matches = 0;

        for _ in 0..iterations {
            let matches = matcher.match_patterns(&test_data);
            total_matches += matches.len();
        }

        let total_time = start_time.elapsed();
        let total_bytes = data_size as u64 * iterations;

        // Prevent compiler optimization
        std::hint::black_box(total_matches);

        Ok(BenchmarkResults {
            test_name: "Pattern Matching".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / iterations as u32,
            memory_usage: data_size + (num_patterns * 20), // Approximate pattern storage
            simd_operations: iterations,
        })
    }

    /// Benchmark buffer pool performance
    pub fn benchmark_buffer_pool(
        &self,
        buffer_size: usize,
        iterations: u64,
    ) -> Result<BenchmarkResults> {
        let start_time = Instant::now();

        for _ in 0..iterations {
            let buffer = self.buffer_pool.get_buffer(buffer_size)?;
            self.buffer_pool.return_buffer(buffer);
        }

        let total_time = start_time.elapsed();

        Ok(BenchmarkResults {
            test_name: "Buffer Pool".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: 0.0, // Not applicable for buffer allocation
            avg_latency: total_time / iterations as u32,
            memory_usage: buffer_size,
            simd_operations: 0,
        })
    }

    /// Run comprehensive benchmark suite
    pub fn run_comprehensive_benchmark(&self) -> Result<Vec<BenchmarkResults>> {
        let mut results = Vec::new();

        // Test different data sizes
        let data_sizes = vec![1024, 4096, 16384, 65536, 262144]; // 1KB to 256KB
        let iterations = 10000;

        for &size in &data_sizes {
            // SIMD operations
            results.push(self.benchmark_simd_xor(size, iterations)?);
            results.push(self.benchmark_simd_copy(size, iterations)?);
            results.push(self.benchmark_simd_checksum(size, iterations)?);

            // Message processing
            results.push(self.benchmark_message_processing(size, iterations / 10)?);

            // Buffer pool
            results.push(self.benchmark_buffer_pool(size, iterations)?);
        }

        // Batch processing tests
        let batch_sizes = vec![10, 50, 100, 500];
        for &batch_size in &batch_sizes {
            results.push(self.benchmark_batch_processing(batch_size, 4096)?);
        }

        // Pattern matching tests
        let pattern_counts = vec![10, 50, 100];
        for &pattern_count in &pattern_counts {
            results.push(self.benchmark_pattern_matching(65536, pattern_count, 1000)?);
        }

        Ok(results)
    }

    /// Compare SIMD vs non-SIMD performance
    pub fn compare_simd_performance(
        &self,
        data_size: usize,
        iterations: u64,
    ) -> Result<(BenchmarkResults, BenchmarkResults)> {
        // SIMD version
        let simd_results = self.benchmark_simd_xor(data_size, iterations)?;

        // Non-SIMD version (naive implementation)
        let data_a = vec![0xAA; data_size];
        let data_b = vec![0x55; data_size];
        let mut output = vec![0; data_size];

        let start_time = Instant::now();

        for _ in 0..iterations {
            for i in 0..data_size {
                output[i] = data_a[i] ^ data_b[i];
            }
        }

        let total_time = start_time.elapsed();
        let total_bytes = data_size as u64 * iterations;

        let naive_results = BenchmarkResults {
            test_name: "Naive XOR".to_string(),
            operations: iterations,
            total_time,
            ops_per_second: iterations as f64 / total_time.as_secs_f64(),
            throughput_mbps: (total_bytes as f64 / total_time.as_secs_f64()) / (1024.0 * 1024.0),
            avg_latency: total_time / iterations as u32,
            memory_usage: data_size * 3,
            simd_operations: 0,
        };

        Ok((simd_results, naive_results))
    }

    /// Print benchmark results in a formatted table
    pub fn print_results(&self, results: &[BenchmarkResults]) {
        println!("\n=== Valkyrie Protocol Performance Benchmark Results ===\n");
        println!(
            "{:<20} {:>10} {:>12} {:>12} {:>12} {:>10} {:>12}",
            "Test", "Ops", "Ops/sec", "MB/s", "Avg Lat (μs)", "Mem (KB)", "SIMD Ops"
        );
        println!("{}", "-".repeat(90));

        for result in results {
            println!(
                "{:<20} {:>10} {:>12.2} {:>12.2} {:>12.2} {:>10} {:>12}",
                result.test_name,
                result.operations,
                result.ops_per_second,
                result.throughput_mbps,
                result.avg_latency.as_micros(),
                result.memory_usage / 1024,
                result.simd_operations
            );
        }

        println!("\n=== Summary ===");
        let total_ops: u64 = results.iter().map(|r| r.operations).sum();
        let avg_throughput: f64 = results
            .iter()
            .filter(|r| r.throughput_mbps > 0.0)
            .map(|r| r.throughput_mbps)
            .sum::<f64>()
            / results.iter().filter(|r| r.throughput_mbps > 0.0).count() as f64;
        let total_simd_ops: u64 = results.iter().map(|r| r.simd_operations).sum();

        println!("Total operations: {}", total_ops);
        println!("Average throughput: {:.2} MB/s", avg_throughput);
        println!("Total SIMD operations: {}", total_simd_ops);
        println!(
            "SIMD utilization: {:.1}%",
            (total_simd_ops as f64 / total_ops as f64) * 100.0
        );
    }
}

impl Default for PerformanceBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_xor_benchmark() {
        let benchmark = PerformanceBenchmark::new();
        let result = benchmark.benchmark_simd_xor(1024, 100).unwrap();

        assert_eq!(result.operations, 100);
        assert!(result.ops_per_second > 0.0);
        assert!(result.throughput_mbps > 0.0);
        assert_eq!(result.simd_operations, 100);
    }

    #[test]
    fn test_message_processing_benchmark() {
        let benchmark = PerformanceBenchmark::new();
        let result = benchmark.benchmark_message_processing(4096, 10).unwrap();

        assert_eq!(result.operations, 10);
        assert!(result.ops_per_second > 0.0);
        assert!(result.memory_usage > 0);
    }

    #[test]
    fn test_simd_vs_naive_comparison() {
        let benchmark = PerformanceBenchmark::new();
        let (simd_result, naive_result) = benchmark.compare_simd_performance(4096, 1000).unwrap();

        // SIMD should be faster (higher throughput)
        assert!(simd_result.throughput_mbps >= naive_result.throughput_mbps);
        assert_eq!(simd_result.simd_operations, 1000);
        assert_eq!(naive_result.simd_operations, 0);
    }

    #[test]
    fn test_buffer_pool_benchmark() {
        let benchmark = PerformanceBenchmark::new();
        let result = benchmark.benchmark_buffer_pool(1024, 1000).unwrap();

        assert_eq!(result.operations, 1000);
        assert!(result.ops_per_second > 0.0);
        assert_eq!(result.memory_usage, 1024);
    }
}
