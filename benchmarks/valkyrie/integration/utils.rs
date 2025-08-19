use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use rustci::config::valkyrie::ValkyrieConfig;
use rustci::core::networking::valkyrie::engine::ValkyrieEngine;
use rustci::core::networking::valkyrie::message::{MessageHeader, MessageType, ValkyrieMessage};

/// Test utilities for Valkyrie Protocol testing
pub struct TestUtils;

impl TestUtils {
    /// Create a test Valkyrie engine with minimal configuration
    pub async fn create_test_engine() -> Result<ValkyrieEngine, Box<dyn std::error::Error>> {
        let config = ValkyrieConfig::test_default();
        ValkyrieEngine::new(config).await
    }

    /// Create a test message with specified type
    pub fn create_test_message(msg_type: MessageType) -> ValkyrieMessage {
        ValkyrieMessage {
            header: MessageHeader {
                message_type: msg_type,
                stream_id: Uuid::new_v4(),
                correlation_id: Some(Uuid::new_v4()),
                timestamp: chrono::Utc::now(),
                ..Default::default()
            },
            payload: vec![],
            signature: None,
            trace_context: None,
        }
    }

    /// Measure execution time of an async operation
    pub async fn measure_time<F, T>(operation: F) -> (T, Duration)
    where
        F: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = operation.await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Generate test data of specified size
    pub fn generate_test_data(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 256) as u8).collect()
    }

    /// Create multiple test engines for multi-node testing
    pub async fn create_test_cluster(
        node_count: usize,
    ) -> Result<Vec<ValkyrieEngine>, Box<dyn std::error::Error>> {
        let mut engines = Vec::new();
        for i in 0..node_count {
            let mut config = ValkyrieConfig::test_default();
            config.node_id = format!("test-node-{}", i);
            config.bind_port = 8000 + i as u16;
            engines.push(ValkyrieEngine::new(config).await?);
        }
        Ok(engines)
    }
}

/// Performance measurement utilities
pub struct PerformanceUtils;

impl PerformanceUtils {
    /// Measure latency percentiles
    pub fn calculate_percentiles(mut latencies: Vec<Duration>) -> LatencyStats {
        latencies.sort();
        let len = latencies.len();

        LatencyStats {
            min: latencies[0],
            max: latencies[len - 1],
            p50: latencies[len / 2],
            p95: latencies[(len * 95) / 100],
            p99: latencies[(len * 99) / 100],
            p999: latencies[(len * 999) / 1000],
            mean: Duration::from_nanos(
                latencies.iter().map(|d| d.as_nanos() as u64).sum::<u64>() / len as u64,
            ),
        }
    }

    /// Measure throughput over time
    pub async fn measure_throughput<F>(operation: F, duration: Duration) -> ThroughputStats
    where
        F: Fn() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send>,
        >,
    {
        let start = Instant::now();
        let mut operations = 0;
        let mut errors = 0;

        while start.elapsed() < duration {
            match operation().await {
                Ok(_) => operations += 1,
                Err(_) => errors += 1,
            }
        }

        let actual_duration = start.elapsed();
        ThroughputStats {
            operations,
            errors,
            duration: actual_duration,
            ops_per_second: operations as f64 / actual_duration.as_secs_f64(),
            error_rate: errors as f64 / (operations + errors) as f64,
        }
    }
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub min: Duration,
    pub max: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p999: Duration,
    pub mean: Duration,
}

/// Throughput statistics
#[derive(Debug, Clone)]
pub struct ThroughputStats {
    pub operations: u64,
    pub errors: u64,
    pub duration: Duration,
    pub ops_per_second: f64,
    pub error_rate: f64,
}

/// Test assertion helpers
pub struct TestAssertions;

impl TestAssertions {
    /// Assert latency is below threshold
    pub fn assert_latency_below(latency: Duration, threshold: Duration) {
        assert!(
            latency < threshold,
            "Latency {:?} ({:.2}μs) exceeds threshold {:?} ({:.2}μs)",
            latency,
            latency.as_nanos() as f64 / 1000.0,
            threshold,
            threshold.as_nanos() as f64 / 1000.0
        );
    }

    /// Assert sub-millisecond latency with detailed reporting
    pub fn assert_sub_millisecond_latency(latency: Duration, context: &str) {
        let micros = latency.as_nanos() as f64 / 1000.0;
        assert!(
            latency < Duration::from_millis(1),
            "Sub-millisecond assertion failed for {}: {:?} ({:.2}μs) >= 1000μs",
            context,
            latency,
            micros
        );

        if micros > 500.0 {
            println!(
                "⚠️  Warning: {} latency {:.2}μs is sub-millisecond but >500μs",
                context, micros
            );
        } else if micros > 100.0 {
            println!(
                "✅ Good: {} latency {:.2}μs is well within sub-millisecond",
                context, micros
            );
        } else {
            println!(
                "🚀 Excellent: {} latency {:.2}μs is ultra-fast",
                context, micros
            );
        }
    }

    /// Assert throughput is above threshold
    pub fn assert_throughput_above(throughput: f64, threshold: f64) {
        assert!(
            throughput > threshold,
            "Throughput {:.2} ops/sec is below threshold {:.2} ops/sec (avg latency: {:.2}μs vs required: {:.2}μs)",
            throughput,
            threshold,
            1_000_000.0 / throughput,
            1_000_000.0 / threshold
        );
    }

    /// Assert error rate is below threshold
    pub fn assert_error_rate_below(error_rate: f64, threshold: f64) {
        assert!(
            error_rate < threshold,
            "Error rate {:.4}% exceeds threshold {:.4}%",
            error_rate * 100.0,
            threshold * 100.0
        );
    }

    /// Assert sub-millisecond performance for a batch of latencies
    pub fn assert_sub_millisecond_batch(
        latencies: &[Duration],
        min_percentage: f64,
        context: &str,
    ) {
        let sub_ms_count = latencies
            .iter()
            .filter(|&&lat| lat < Duration::from_millis(1))
            .count();
        let percentage = (sub_ms_count as f64 / latencies.len() as f64) * 100.0;

        assert!(
            percentage >= min_percentage,
            "Sub-millisecond batch assertion failed for {}: {:.2}% < {:.2}% (only {}/{} requests were sub-millisecond)",
            context,
            percentage,
            min_percentage,
            sub_ms_count,
            latencies.len()
        );

        println!(
            "✅ Sub-millisecond batch validation for {}: {:.2}% ({}/{}) >= {:.2}%",
            context,
            percentage,
            sub_ms_count,
            latencies.len(),
            min_percentage
        );
    }
}
