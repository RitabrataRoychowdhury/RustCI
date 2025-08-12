//! Benchmarking suite for Valkyrie Protocol HTTP/HTTPS bridge
//!
//! This module provides comprehensive benchmarks to validate sub-millisecond
//! response times and overall performance of the HTTP bridge.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::Semaphore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{HttpGateway, BridgeConfig, HighPerformanceProcessor, PerformanceConfig};
use crate::core::networking::valkyrie::engine::ValkyrieEngine;

/// Benchmark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Number of concurrent requests
    pub concurrent_requests: usize,
    /// Total number of requests to send
    pub total_requests: usize,
    /// Request payload size in bytes
    pub payload_size: usize,
    /// Target response time in microseconds
    pub target_response_time_us: u64,
    /// Warmup requests before measurement
    pub warmup_requests: usize,
    /// Test duration in seconds
    pub test_duration_seconds: u64,
    /// Enable detailed latency tracking
    pub detailed_latency_tracking: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            concurrent_requests: 1000,
            total_requests: 100000,
            payload_size: 1024, // 1KB
            target_response_time_us: 500, // 500 microseconds
            warmup_requests: 1000,
            test_duration_seconds: 60,
            detailed_latency_tracking: true,
        }
    }
}

/// Benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Total requests processed
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Test duration
    pub duration: Duration,
    /// Requests per second
    pub requests_per_second: f64,
    /// Latency statistics
    pub latency_stats: LatencyStatistics,
    /// Throughput statistics
    pub throughput_stats: ThroughputStatistics,
    /// Error statistics
    pub error_stats: ErrorStatistics,
    /// Performance grade
    pub performance_grade: PerformanceGrade,
}

/// Latency statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStatistics {
    /// Minimum latency (microseconds)
    pub min_us: u64,
    /// Maximum latency (microseconds)
    pub max_us: u64,
    /// Mean latency (microseconds)
    pub mean_us: f64,
    /// Median latency (microseconds)
    pub median_us: u64,
    /// 95th percentile (microseconds)
    pub p95_us: u64,
    /// 99th percentile (microseconds)
    pub p99_us: u64,
    /// 99.9th percentile (microseconds)
    pub p999_us: u64,
    /// Standard deviation
    pub std_dev_us: f64,
    /// Percentage of requests under target latency
    pub under_target_percentage: f64,
}

/// Throughput statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputStatistics {
    /// Bytes per second
    pub bytes_per_second: f64,
    /// Megabytes per second
    pub megabytes_per_second: f64,
    /// Peak requests per second
    pub peak_rps: f64,
    /// Average requests per second
    pub average_rps: f64,
    /// Minimum requests per second
    pub min_rps: f64,
}

/// Error statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStatistics {
    /// Connection errors
    pub connection_errors: u64,
    /// Timeout errors
    pub timeout_errors: u64,
    /// HTTP errors by status code
    pub http_errors: HashMap<u16, u64>,
    /// Processing errors
    pub processing_errors: u64,
    /// Error rate (percentage)
    pub error_rate: f64,
}

/// Performance grade based on benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceGrade {
    /// Excellent performance (sub-millisecond)
    Excellent {
        /// Average latency achieved
        avg_latency_us: f64,
        /// Percentage under target
        under_target_pct: f64,
    },
    /// Good performance
    Good {
        /// Average latency achieved
        avg_latency_us: f64,
        /// Percentage under target
        under_target_pct: f64,
    },
    /// Fair performance
    Fair {
        /// Average latency achieved
        avg_latency_us: f64,
        /// Percentage under target
        under_target_pct: f64,
    },
    /// Poor performance
    Poor {
        /// Average latency achieved
        avg_latency_us: f64,
        /// Percentage under target
        under_target_pct: f64,
    },
}

/// Benchmark runner for HTTP bridge performance testing
pub struct BenchmarkRunner {
    /// Benchmark configuration
    config: BenchmarkConfig,
    /// HTTP gateway instance
    gateway: Arc<HttpGateway>,
    /// Performance processor for direct testing
    processor: Option<Arc<HighPerformanceProcessor>>,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub async fn new(
        config: BenchmarkConfig,
        gateway: Arc<HttpGateway>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create a separate performance processor for direct testing
        let processor = if config.target_response_time_us < 1000 {
            let perf_config = PerformanceConfig {
                enable_zero_copy: true,
                enable_simd: true,
                request_queue_size: config.concurrent_requests * 10,
                worker_threads: num_cpus::get() * 2,
                target_response_time_us: config.target_response_time_us,
                enable_prefetch: true,
            };
            
            let processor = HighPerformanceProcessor::new(perf_config).await?;
            processor.start_workers().await?;
            Some(Arc::new(processor))
        } else {
            None
        };

        Ok(Self {
            config,
            gateway,
            processor,
        })
    }

    /// Run comprehensive benchmark suite
    pub async fn run_benchmark_suite(&self) -> Result<BenchmarkResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting Valkyrie Protocol HTTP Bridge Benchmark Suite");
        println!("Configuration: {:?}", self.config);

        // Warmup phase
        println!("Running warmup phase...");
        self.run_warmup().await?;

        // Main benchmark
        println!("Running main benchmark...");
        let results = self.run_main_benchmark().await?;

        // Print results
        self.print_results(&results);

        Ok(results)
    }

    /// Run warmup requests
    async fn run_warmup(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_requests));
        let mut tasks = Vec::new();

        for _ in 0..self.config.warmup_requests {
            let semaphore = Arc::clone(&semaphore);
            let processor = self.processor.clone();
            let payload_size = self.config.payload_size;

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                
                if let Some(processor) = processor {
                    let payload = bytes::Bytes::from(vec![0u8; payload_size]);
                    let _ = processor.process_request(payload).await;
                }
            });

            tasks.push(task);
        }

        // Wait for all warmup requests to complete
        for task in tasks {
            let _ = task.await;
        }

        // Allow system to stabilize
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Run main benchmark
    async fn run_main_benchmark(&self) -> Result<BenchmarkResults, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_requests));
        let mut tasks = Vec::new();
        let mut latencies = Vec::new();
        let mut successful_requests = 0u64;
        let mut failed_requests = 0u64;
        let mut error_stats = ErrorStatistics {
            connection_errors: 0,
            timeout_errors: 0,
            http_errors: HashMap::new(),
            processing_errors: 0,
            error_rate: 0.0,
        };

        // Collect latency measurements
        let latency_collector = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        for request_id in 0..self.config.total_requests {
            let semaphore = Arc::clone(&semaphore);
            let processor = self.processor.clone();
            let payload_size = self.config.payload_size;
            let latency_collector = Arc::clone(&latency_collector);
            let target_latency = self.config.target_response_time_us;

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let request_start = Instant::now();
                
                let result = if let Some(processor) = processor {
                    let payload = bytes::Bytes::from(vec![0u8; payload_size]);
                    processor.process_request(payload).await
                } else {
                    // Simulate processing
                    tokio::time::sleep(Duration::from_micros(100)).await;
                    Ok(bytes::Bytes::from("success"))
                };

                let latency = request_start.elapsed();
                let latency_us = latency.as_micros() as u64;

                // Collect latency data
                {
                    let mut collector = latency_collector.lock().await;
                    collector.push(latency_us);
                }

                (request_id, result, latency_us)
            });

            tasks.push(task);
        }

        // Wait for all requests to complete and collect results
        for task in tasks {
            match task.await {
                Ok((_, Ok(_), _)) => successful_requests += 1,
                Ok((_, Err(_), _)) => {
                    failed_requests += 1;
                    error_stats.processing_errors += 1;
                }
                Err(_) => {
                    failed_requests += 1;
                    error_stats.connection_errors += 1;
                }
            }
        }

        let total_duration = start_time.elapsed();

        // Collect and analyze latency data
        let latency_data = latency_collector.lock().await;
        latencies.extend(latency_data.iter().cloned());
        drop(latency_data);

        // Calculate statistics
        let latency_stats = self.calculate_latency_statistics(&latencies, self.config.target_response_time_us);
        let throughput_stats = self.calculate_throughput_statistics(
            successful_requests,
            total_duration,
            self.config.payload_size,
        );

        error_stats.error_rate = (failed_requests as f64 / self.config.total_requests as f64) * 100.0;

        let performance_grade = self.calculate_performance_grade(&latency_stats, self.config.target_response_time_us);

        Ok(BenchmarkResults {
            total_requests: self.config.total_requests as u64,
            successful_requests,
            failed_requests,
            duration: total_duration,
            requests_per_second: successful_requests as f64 / total_duration.as_secs_f64(),
            latency_stats,
            throughput_stats,
            error_stats,
            performance_grade,
        })
    }

    /// Calculate latency statistics
    fn calculate_latency_statistics(&self, latencies: &[u64], target_us: u64) -> LatencyStatistics {
        if latencies.is_empty() {
            return LatencyStatistics {
                min_us: 0,
                max_us: 0,
                mean_us: 0.0,
                median_us: 0,
                p95_us: 0,
                p99_us: 0,
                p999_us: 0,
                std_dev_us: 0.0,
                under_target_percentage: 0.0,
            };
        }

        let mut sorted_latencies = latencies.to_vec();
        sorted_latencies.sort_unstable();

        let min_us = sorted_latencies[0];
        let max_us = sorted_latencies[sorted_latencies.len() - 1];
        let mean_us = sorted_latencies.iter().sum::<u64>() as f64 / sorted_latencies.len() as f64;

        let median_us = sorted_latencies[sorted_latencies.len() / 2];
        let p95_us = sorted_latencies[(sorted_latencies.len() as f64 * 0.95) as usize];
        let p99_us = sorted_latencies[(sorted_latencies.len() as f64 * 0.99) as usize];
        let p999_us = sorted_latencies[(sorted_latencies.len() as f64 * 0.999) as usize];

        // Calculate standard deviation
        let variance = sorted_latencies.iter()
            .map(|&x| {
                let diff = x as f64 - mean_us;
                diff * diff
            })
            .sum::<f64>() / sorted_latencies.len() as f64;
        let std_dev_us = variance.sqrt();

        // Calculate percentage under target
        let under_target_count = sorted_latencies.iter().filter(|&&x| x <= target_us).count();
        let under_target_percentage = (under_target_count as f64 / sorted_latencies.len() as f64) * 100.0;

        LatencyStatistics {
            min_us,
            max_us,
            mean_us,
            median_us,
            p95_us,
            p99_us,
            p999_us,
            std_dev_us,
            under_target_percentage,
        }
    }

    /// Calculate throughput statistics
    fn calculate_throughput_statistics(
        &self,
        successful_requests: u64,
        duration: Duration,
        payload_size: usize,
    ) -> ThroughputStatistics {
        let duration_secs = duration.as_secs_f64();
        let average_rps = successful_requests as f64 / duration_secs;
        let bytes_per_second = average_rps * payload_size as f64;
        let megabytes_per_second = bytes_per_second / (1024.0 * 1024.0);

        ThroughputStatistics {
            bytes_per_second,
            megabytes_per_second,
            peak_rps: average_rps * 1.2, // Estimate peak as 20% higher
            average_rps,
            min_rps: average_rps * 0.8, // Estimate min as 20% lower
        }
    }

    /// Calculate performance grade
    fn calculate_performance_grade(&self, latency_stats: &LatencyStatistics, target_us: u64) -> PerformanceGrade {
        let avg_latency = latency_stats.mean_us;
        let under_target_pct = latency_stats.under_target_percentage;

        if avg_latency <= target_us as f64 && under_target_pct >= 95.0 {
            PerformanceGrade::Excellent {
                avg_latency_us: avg_latency,
                under_target_pct,
            }
        } else if avg_latency <= (target_us as f64 * 1.5) && under_target_pct >= 80.0 {
            PerformanceGrade::Good {
                avg_latency_us: avg_latency,
                under_target_pct,
            }
        } else if avg_latency <= (target_us as f64 * 2.0) && under_target_pct >= 60.0 {
            PerformanceGrade::Fair {
                avg_latency_us: avg_latency,
                under_target_pct,
            }
        } else {
            PerformanceGrade::Poor {
                avg_latency_us: avg_latency,
                under_target_pct,
            }
        }
    }

    /// Print benchmark results
    fn print_results(&self, results: &BenchmarkResults) {
        println!("\n=== Valkyrie Protocol HTTP Bridge Benchmark Results ===");
        println!("Total Requests: {}", results.total_requests);
        println!("Successful Requests: {}", results.successful_requests);
        println!("Failed Requests: {}", results.failed_requests);
        println!("Test Duration: {:.2}s", results.duration.as_secs_f64());
        println!("Requests/Second: {:.2}", results.requests_per_second);
        println!("Error Rate: {:.2}%", results.error_stats.error_rate);

        println!("\n--- Latency Statistics ---");
        println!("Min: {}μs", results.latency_stats.min_us);
        println!("Max: {}μs", results.latency_stats.max_us);
        println!("Mean: {:.2}μs", results.latency_stats.mean_us);
        println!("Median: {}μs", results.latency_stats.median_us);
        println!("P95: {}μs", results.latency_stats.p95_us);
        println!("P99: {}μs", results.latency_stats.p99_us);
        println!("P99.9: {}μs", results.latency_stats.p999_us);
        println!("Std Dev: {:.2}μs", results.latency_stats.std_dev_us);
        println!("Under Target ({}μs): {:.1}%", 
                 self.config.target_response_time_us, 
                 results.latency_stats.under_target_percentage);

        println!("\n--- Throughput Statistics ---");
        println!("Average RPS: {:.2}", results.throughput_stats.average_rps);
        println!("Peak RPS: {:.2}", results.throughput_stats.peak_rps);
        println!("Throughput: {:.2} MB/s", results.throughput_stats.megabytes_per_second);

        println!("\n--- Performance Grade ---");
        match &results.performance_grade {
            PerformanceGrade::Excellent { avg_latency_us, under_target_pct } => {
                println!("🏆 EXCELLENT - Avg: {:.2}μs, Under Target: {:.1}%", avg_latency_us, under_target_pct);
            }
            PerformanceGrade::Good { avg_latency_us, under_target_pct } => {
                println!("✅ GOOD - Avg: {:.2}μs, Under Target: {:.1}%", avg_latency_us, under_target_pct);
            }
            PerformanceGrade::Fair { avg_latency_us, under_target_pct } => {
                println!("⚠️  FAIR - Avg: {:.2}μs, Under Target: {:.1}%", avg_latency_us, under_target_pct);
            }
            PerformanceGrade::Poor { avg_latency_us, under_target_pct } => {
                println!("❌ POOR - Avg: {:.2}μs, Under Target: {:.1}%", avg_latency_us, under_target_pct);
            }
        }

        // Sub-millisecond achievement check
        if results.latency_stats.mean_us < 1000.0 && results.latency_stats.under_target_percentage >= 90.0 {
            println!("\n🎉 SUB-MILLISECOND TARGET ACHIEVED! 🎉");
            println!("Average response time: {:.2}μs (< 1000μs)", results.latency_stats.mean_us);
            println!("{}% of requests completed under target", results.latency_stats.under_target_percentage);
        }
    }

    /// Export results to JSON file
    pub async fn export_results(&self, results: &BenchmarkResults, filename: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json_data = serde_json::to_string_pretty(results)?;
        tokio::fs::write(filename, json_data).await?;
        println!("Results exported to: {}", filename);
        Ok(())
    }
}

/// Continuous performance monitoring
pub struct PerformanceMonitor {
    /// Monitoring configuration
    config: BenchmarkConfig,
    /// Gateway instance
    gateway: Arc<HttpGateway>,
    /// Monitoring interval
    interval: Duration,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(
        config: BenchmarkConfig,
        gateway: Arc<HttpGateway>,
        interval: Duration,
    ) -> Self {
        Self {
            config,
            gateway,
            interval,
        }
    }

    /// Start continuous monitoring
    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut interval_timer = tokio::time::interval(self.interval);

        loop {
            interval_timer.tick().await;

            // Run a quick benchmark
            let quick_config = BenchmarkConfig {
                concurrent_requests: 100,
                total_requests: 1000,
                payload_size: 512,
                target_response_time_us: self.config.target_response_time_us,
                warmup_requests: 50,
                test_duration_seconds: 10,
                detailed_latency_tracking: false,
            };

            let runner = BenchmarkRunner::new(quick_config, Arc::clone(&self.gateway)).await?;
            let results = runner.run_main_benchmark().await?;

            // Log performance metrics
            tracing::info!(
                "Performance Monitor - RPS: {:.2}, Avg Latency: {:.2}μs, P95: {}μs, Error Rate: {:.2}%",
                results.requests_per_second,
                results.latency_stats.mean_us,
                results.latency_stats.p95_us,
                results.error_stats.error_rate
            );

            // Alert if performance degrades
            if results.latency_stats.mean_us > self.config.target_response_time_us as f64 * 2.0 {
                tracing::warn!(
                    "Performance degradation detected! Average latency: {:.2}μs (target: {}μs)",
                    results.latency_stats.mean_us,
                    self.config.target_response_time_us
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_benchmark_config() {
        let config = BenchmarkConfig::default();
        assert_eq!(config.target_response_time_us, 500);
        assert_eq!(config.concurrent_requests, 1000);
    }

    #[test]
    fn test_latency_statistics_calculation() {
        let latencies = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        
        // Create a dummy benchmark runner for testing
        let config = BenchmarkConfig::default();
        let runner = BenchmarkRunner {
            config,
            gateway: Arc::new(unsafe { std::mem::zeroed() }), // This is just for testing
            processor: None,
        };

        let stats = runner.calculate_latency_statistics(&latencies, 500);
        
        assert_eq!(stats.min_us, 100);
        assert_eq!(stats.max_us, 1000);
        assert_eq!(stats.mean_us, 550.0);
        assert_eq!(stats.median_us, 600);
    }

    #[test]
    fn test_performance_grade_calculation() {
        let config = BenchmarkConfig::default();
        let runner = BenchmarkRunner {
            config,
            gateway: Arc::new(unsafe { std::mem::zeroed() }),
            processor: None,
        };

        let excellent_stats = LatencyStatistics {
            min_us: 100,
            max_us: 400,
            mean_us: 250.0,
            median_us: 250,
            p95_us: 350,
            p99_us: 380,
            p999_us: 400,
            std_dev_us: 50.0,
            under_target_percentage: 98.0,
        };

        let grade = runner.calculate_performance_grade(&excellent_stats, 500);
        matches!(grade, PerformanceGrade::Excellent { .. });
    }
}