//! Benchmarking tools for performance testing

use anyhow::{Context, Result};
use colored::*;
use hdrhistogram::Histogram;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{info, warn};
use valkyrie_sdk::ClientBuilder;

/// Run performance benchmark
pub async fn run_benchmark(
    endpoint: String,
    connections: usize,
    requests: usize,
    message_size: usize,
    duration_secs: u64,
) -> Result<()> {
    println!("{}", "🚀 Valkyrie Performance Benchmark".cyan().bold());
    println!();

    // Configuration summary
    println!("{} Configuration:", "ℹ".blue().bold());
    println!("  Endpoint: {}", endpoint.cyan());
    println!("  Connections: {}", connections.to_string().yellow());
    println!(
        "  Requests per connection: {}",
        requests.to_string().yellow()
    );
    println!(
        "  Message size: {} bytes",
        message_size.to_string().yellow()
    );
    if duration_secs > 0 {
        println!("  Duration: {} seconds", duration_secs.to_string().yellow());
    }
    println!();

    // Create test message
    let test_message = "x".repeat(message_size);

    // Setup progress tracking
    let total_requests = if duration_secs > 0 {
        connections * 1000 // Estimate for progress bar
    } else {
        connections * requests
    };

    let progress = ProgressBar::new(total_requests as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}) {msg}")
            .unwrap()
            .progress_chars("#>-")
    );

    // Statistics tracking
    let histogram = Arc::new(std::sync::Mutex::new(
        Histogram::<u64>::new_with_bounds(1, 60_000_000, 3).unwrap(), // 1μs to 60s
    ));
    let error_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    println!("{} Starting benchmark...", "→".green().bold());
    let benchmark_start = Instant::now();

    // Create semaphore to limit concurrent connections
    let semaphore = Arc::new(Semaphore::new(connections));
    let mut tasks = Vec::new();

    for conn_id in 0..connections {
        let endpoint = endpoint.clone();
        let test_message = test_message.clone();
        let histogram = Arc::clone(&histogram);
        let error_count = Arc::clone(&error_count);
        let success_count = Arc::clone(&success_count);
        let progress = progress.clone();
        let semaphore = Arc::clone(&semaphore);

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Create client for this connection
            let client = match ClientBuilder::new()
                .endpoint(&endpoint)
                .request_timeout_ms(10000)
                .enable_pooling(false)
                .build()
                .await
            {
                Ok(client) => client,
                Err(e) => {
                    warn!("Failed to create client for connection {}: {}", conn_id, e);
                    error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            };

            let end_time = if duration_secs > 0 {
                Some(Instant::now() + Duration::from_secs(duration_secs))
            } else {
                None
            };

            let mut req_count = 0;
            loop {
                // Check if we should stop
                if let Some(end_time) = end_time {
                    if Instant::now() >= end_time {
                        break;
                    }
                } else if req_count >= requests {
                    break;
                }

                // Send request
                let start = Instant::now();
                match client.request_string(&test_message).await {
                    Ok(_) => {
                        let duration = start.elapsed();
                        let duration_micros = duration.as_micros() as u64;

                        // Record latency
                        if let Ok(mut hist) = histogram.lock() {
                            hist.record(duration_micros).ok();
                        }

                        success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        progress.inc(1);
                    }
                    Err(e) => {
                        warn!("Request failed on connection {}: {}", conn_id, e);
                        error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }

                req_count += 1;
            }

            client.disconnect().await.ok();
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        task.await.ok();
    }

    progress.finish_with_message("Benchmark completed");

    let total_duration = benchmark_start.elapsed();
    let success_count = success_count.load(std::sync::atomic::Ordering::Relaxed);
    let error_count = error_count.load(std::sync::atomic::Ordering::Relaxed);
    let total_count = success_count + error_count;

    println!();
    println!("{} Benchmark Results:", "📊".green().bold());
    println!();

    // Overall statistics
    println!("{} Overall Statistics:", "📈".blue());
    println!("  Total requests: {}", total_count.to_string().cyan());
    println!(
        "  Successful: {} ({:.1}%)",
        success_count.to_string().green(),
        (success_count as f64 / total_count as f64 * 100.0)
    );
    println!(
        "  Failed: {} ({:.1}%)",
        error_count.to_string().red(),
        (error_count as f64 / total_count as f64 * 100.0)
    );
    println!("  Duration: {:.2}s", total_duration.as_secs_f64());
    println!(
        "  Throughput: {:.1} req/s",
        success_count as f64 / total_duration.as_secs_f64()
    );
    println!();

    // Latency statistics
    if success_count > 0 {
        let histogram = histogram.lock().unwrap();
        println!("{} Latency Statistics (μs):", "⏱".blue());
        println!("  Min: {}", histogram.min().to_string().cyan());
        println!("  Max: {}", histogram.max().to_string().cyan());
        println!("  Mean: {:.1}", histogram.mean());
        println!(
            "  P50: {}",
            histogram.value_at_quantile(0.5).to_string().yellow()
        );
        println!(
            "  P95: {}",
            histogram.value_at_quantile(0.95).to_string().yellow()
        );
        println!(
            "  P99: {}",
            histogram.value_at_quantile(0.99).to_string().yellow()
        );
        println!(
            "  P99.9: {}",
            histogram.value_at_quantile(0.999).to_string().yellow()
        );
    }

    println!();
    if error_count == 0 {
        println!("{} Benchmark completed successfully!", "✓".green().bold());
    } else {
        println!(
            "{} Benchmark completed with {} errors",
            "⚠".yellow().bold(),
            error_count
        );
    }

    Ok(())
}
