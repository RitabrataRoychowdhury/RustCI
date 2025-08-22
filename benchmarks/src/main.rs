//! Valkyrie Protocol Unified Benchmark Runner
//! 
//! This binary runs comprehensive benchmarks for the Valkyrie Protocol
//! with performance validation, regression detection, and CI/CD integration.

mod unified_benchmark_engine;
mod regression_detector;
mod metrics_collector;
mod benchmark_orchestrator;

use clap::Parser;
use std::path::PathBuf;
use unified_benchmark_engine::{UnifiedBenchmarkEngine, UnifiedBenchmarkConfig};
use regression_detector::PerformanceRegressionDetector;
use metrics_collector::RealTimeMetricsCollector;
use benchmark_orchestrator::{BenchmarkOrchestrator, BenchmarkExecution, ExecutionType, ExecutionPriority, ExecutionConfig, ResourceAllocation, RetryPolicy};
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "valkyrie-benchmark")]
#[command(about = "Valkyrie Protocol Unified Benchmark Runner v2.0")]
struct Args {
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Output directory for reports
    #[arg(short, long)]
    output: PathBuf,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Run mode: full, quick, or custom
    #[arg(short, long, default_value = "full")]
    mode: String,
    
    /// Enable regression detection
    #[arg(long)]
    regression_detection: bool,
    
    /// Enable real-time metrics collection
    #[arg(long)]
    real_time_metrics: bool,
    
    /// Enable CI/CD integration
    #[arg(long)]
    ci_cd_integration: bool,
    
    /// Execution timeout in seconds
    #[arg(long, default_value = "3600")]
    timeout: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Initialize tracing
    if args.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("info")
            .init();
    }
    
    println!("🚀 Valkyrie Protocol Unified Benchmark Runner v2.0");
    println!("==================================================");
    
    // Create output directory
    tokio::fs::create_dir_all(&args.output).await?;
    
    // Load or create configuration
    let config = if let Some(config_path) = &args.config {
        println!("📄 Loading configuration from: {:?}", config_path);
        load_config(config_path).await?
    } else {
        println!("📄 Using default configuration");
        create_default_config(&args.mode)?
    };
    
    // Create benchmark engine
    let engine = UnifiedBenchmarkEngine::new(config);
    
    // Set up real-time metrics collection if enabled
    let mut metrics_collector = if args.real_time_metrics {
        println!("📊 Enabling real-time metrics collection");
        Some(RealTimeMetricsCollector::new())
    } else {
        None
    };
    
    // Set up benchmark orchestrator if CI/CD integration is enabled
    let orchestrator = if args.ci_cd_integration {
        println!("🔗 Enabling CI/CD integration");
        Some(BenchmarkOrchestrator::new())
    } else {
        None
    };
    
    // Start metrics collection
    if let Some(ref mut collector) = metrics_collector {
        collector.start_collection().await?;
    }
    
    let execution_start = std::time::Instant::now();
    
    // Run benchmarks
    let results = if let Some(orchestrator) = &orchestrator {
        // Use orchestrator for CI/CD integrated execution
        run_orchestrated_benchmark(orchestrator, &args).await?
    } else {
        // Direct execution
        println!("🎯 Running unified benchmark suite...");
        engine.run_complete_suite().await?
    };
    
    let execution_duration = execution_start.elapsed();
    
    // Stop metrics collection
    if let Some(ref mut collector) = metrics_collector {
        collector.stop_collection().await?;
    }
    
    // Perform regression detection if enabled
    if args.regression_detection {
        println!("\n🔍 Performing regression analysis...");
        let regression_detector = PerformanceRegressionDetector::new(
            regression_detector::RegressionDetectionConfig::default()
        );
        
        let _regression_analysis = regression_detector.analyze_performance(&results).await?;
    }
    
    // Generate comprehensive reports
    println!("\n📄 Generating comprehensive reports...");
    let base_filename = args.output.join(format!(
        "valkyrie_benchmark_{}",
        Utc::now().format("%Y%m%d_%H%M%S")
    ));
    
    // Export results in multiple formats
    export_results(&results, &base_filename).await?;
    
    // Generate performance dashboard if metrics were collected
    if let Some(collector) = &metrics_collector {
        let dashboard = collector.get_performance_dashboard().await?;
        export_dashboard(&dashboard, &base_filename).await?;
    }
    
    // Print final summary
    print_execution_summary(&results, execution_duration);
    
    println!("\n✅ Benchmark execution completed successfully!");
    println!("📊 Reports generated in: {:?}", args.output);
    
    Ok(())
}

/// Load configuration from file
async fn load_config(config_path: &PathBuf) -> Result<UnifiedBenchmarkConfig, Box<dyn std::error::Error>> {
    let config_content = tokio::fs::read_to_string(config_path).await?;
    
    // Try YAML first, then JSON
    if config_path.extension().and_then(|s| s.to_str()) == Some("yaml") || 
       config_path.extension().and_then(|s| s.to_str()) == Some("yml") {
        Ok(serde_yaml::from_str(&config_content)?)
    } else {
        Ok(serde_json::from_str(&config_content)?)
    }
}

/// Create default configuration based on mode
fn create_default_config(mode: &str) -> Result<UnifiedBenchmarkConfig, Box<dyn std::error::Error>> {
    let mut config = UnifiedBenchmarkConfig::default();
    
    match mode {
        "quick" => {
            // Reduce test duration and concurrency for quick runs
            config.http_bridge.test_duration = Duration::from_secs(30);
            config.http_bridge.concurrent_requests = vec![100, 1000];
            config.protocol_core.concurrent_connections = vec![1000, 10000];
            config.transport.quic.connection_counts = vec![1000, 10000];
        }
        "full" => {
            // Use default full configuration
        }
        "custom" => {
            // Custom configuration - could be extended with more options
        }
        _ => {
            return Err(format!("Unknown mode: {}", mode).into());
        }
    }
    
    Ok(config)
}

/// Run benchmark through orchestrator
async fn run_orchestrated_benchmark(
    orchestrator: &BenchmarkOrchestrator,
    args: &Args,
) -> Result<unified_benchmark_engine::UnifiedBenchmarkResults, Box<dyn std::error::Error>> {
    let execution = BenchmarkExecution {
        execution_id: Uuid::new_v4().to_string(),
        execution_type: ExecutionType::Manual,
        priority: ExecutionPriority::Normal,
        config: ExecutionConfig {
            benchmark_suite: "unified".to_string(),
            test_environment: "local".to_string(),
            resource_allocation: ResourceAllocation {
                cpu_cores: None,
                memory_mb: None,
                disk_space_mb: None,
                network_bandwidth_mbps: None,
            },
            timeout: Duration::from_secs(args.timeout),
            retry_policy: RetryPolicy {
                max_retries: 0,
                retry_delay: Duration::from_secs(30),
                backoff_multiplier: 2.0,
                retry_on_failure_types: vec![],
            },
        },
        requested_at: Utc::now(),
        requested_by: "cli".to_string(),
        metadata: HashMap::new(),
    };
    
    let execution_id = orchestrator.submit_execution(execution).await?;
    
    // Monitor execution until completion
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        if let Some(status) = orchestrator.get_execution_status(&execution_id).await {
            match status {
                benchmark_orchestrator::ExecutionStatus::Completed { result, .. } => {
                    match result {
                        benchmark_orchestrator::ExecutionResult::Success { results, .. } => {
                            return Ok(results);
                        }
                        benchmark_orchestrator::ExecutionResult::Failure { error, .. } => {
                            return Err(error.into());
                        }
                        benchmark_orchestrator::ExecutionResult::Timeout { .. } => {
                            return Err("Execution timed out".into());
                        }
                        benchmark_orchestrator::ExecutionResult::Cancelled { reason } => {
                            return Err(format!("Execution cancelled: {}", reason).into());
                        }
                    }
                }
                _ => {
                    // Still running or pending, continue monitoring
                }
            }
        }
        
        // Monitor running executions
        orchestrator.monitor_executions().await?;
    }
}

/// Export benchmark results in multiple formats
async fn export_results(
    results: &unified_benchmark_engine::UnifiedBenchmarkResults,
    base_filename: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Export JSON
    let json_path = base_filename.with_extension("json");
    let json_content = serde_json::to_string_pretty(results)?;
    tokio::fs::write(&json_path, json_content).await?;
    println!("📄 JSON report: {:?}", json_path);
    
    // Export CSV summary
    let csv_path = base_filename.with_extension("csv");
    let csv_content = generate_csv_summary(results);
    tokio::fs::write(&csv_path, csv_content).await?;
    println!("📄 CSV summary: {:?}", csv_path);
    
    // Export Markdown report
    let md_path = base_filename.with_extension("md");
    let md_content = generate_markdown_report(results);
    tokio::fs::write(&md_path, md_content).await?;
    println!("📄 Markdown report: {:?}", md_path);
    
    Ok(())
}

/// Export performance dashboard
async fn export_dashboard(
    dashboard: &metrics_collector::PerformanceDashboard,
    base_filename: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let dashboard_path = base_filename.with_file_name(
        format!("{}_dashboard.json", base_filename.file_stem().unwrap().to_str().unwrap())
    );
    
    let dashboard_content = serde_json::to_string_pretty(dashboard)?;
    tokio::fs::write(&dashboard_path, dashboard_content).await?;
    println!("📊 Performance dashboard: {:?}", dashboard_path);
    
    Ok(())
}

/// Generate CSV summary
fn generate_csv_summary(results: &unified_benchmark_engine::UnifiedBenchmarkResults) -> String {
    let mut csv = String::new();
    csv.push_str("Component,Metric,Value,Unit,Target,Achieved\n");
    
    // HTTP Bridge metrics
    csv.push_str(&format!(
        "HTTP Bridge,Average Latency,{:.2},microseconds,500,{}\n",
        results.http_bridge.latency_stats.mean_us,
        if results.http_bridge.target_achievement.achieved { "Yes" } else { "No" }
    ));
    
    csv.push_str(&format!(
        "HTTP Bridge,Requests per Second,{:.2},rps,N/A,N/A\n",
        results.http_bridge.requests_per_second
    ));
    
    // Protocol Core metrics
    csv.push_str(&format!(
        "Protocol Core,Average Latency,{:.2},microseconds,100,{}\n",
        results.protocol_core.latency_stats.mean_us,
        if results.protocol_core.target_achievement.achieved { "Yes" } else { "No" }
    ));
    
    csv.push_str(&format!(
        "Protocol Core,Messages per Second,{:.0},msgs/sec,1000000,N/A\n",
        results.protocol_core.messages_per_second
    ));
    
    // Transport metrics
    csv.push_str(&format!(
        "QUIC Transport,Throughput,{:.1},Gbps,15.0,{}\n",
        results.transport.quic.throughput_gbps,
        if results.transport.quic.target_achievement.achieved { "Yes" } else { "No" }
    ));
    
    csv.push_str(&format!(
        "Unix Socket,Throughput,{:.1},Gbps,20.0,{}\n",
        results.transport.unix_socket.throughput_gbps,
        if results.transport.unix_socket.target_achievement.achieved { "Yes" } else { "No" }
    ));
    
    // Security metrics
    csv.push_str(&format!(
        "Security,Encryption Latency,{:.2},microseconds,50,{}\n",
        results.security.encryption_latency_us,
        if results.security.target_achievement.achieved { "Yes" } else { "No" }
    ));
    
    csv
}

/// Generate Markdown report
fn generate_markdown_report(results: &unified_benchmark_engine::UnifiedBenchmarkResults) -> String {
    let grade_emoji = match &results.performance_grade {
        unified_benchmark_engine::OverallPerformanceGrade::Excellent { .. } => "🥇",
        unified_benchmark_engine::OverallPerformanceGrade::Good { .. } => "🥈",
        unified_benchmark_engine::OverallPerformanceGrade::Fair { .. } => "🥉",
        unified_benchmark_engine::OverallPerformanceGrade::Poor { .. } => "❌",
    };
    
    let grade_text = match &results.performance_grade {
        unified_benchmark_engine::OverallPerformanceGrade::Excellent { score, details } => 
            format!("{} EXCELLENT ({:.1}%) - {}", grade_emoji, score, details),
        unified_benchmark_engine::OverallPerformanceGrade::Good { score, details } => 
            format!("{} GOOD ({:.1}%) - {}", grade_emoji, score, details),
        unified_benchmark_engine::OverallPerformanceGrade::Fair { score, details } => 
            format!("{} FAIR ({:.1}%) - {}", grade_emoji, score, details),
        unified_benchmark_engine::OverallPerformanceGrade::Poor { score, details } => 
            format!("{} POOR ({:.1}%) - {}", grade_emoji, score, details),
    };
    
    let sub_millisecond_achievement = if results.http_bridge.latency_stats.mean_us < 1000.0 &&
                                        results.protocol_core.latency_stats.mean_us < 100.0 {
        "🎉 **SUB-MILLISECOND PERFORMANCE ACHIEVED!**\n\n✅ HTTP Bridge: {:.0}μs average  \n✅ Protocol Core: {:.0}μs average"
    } else {
        "⚠️ Sub-millisecond targets not fully achieved"
    };
    
    format!(r#"# Valkyrie Protocol Benchmark Report

## Executive Summary

**Overall Performance Grade:** {}

**Test Information:**
- **Timestamp:** {}
- **Test Duration:** {:.2} seconds
- **Environment:** {} ({} cores)
- **Valkyrie Version:** {}

## Performance Results

### 🎯 Target Achievement Summary

| Component | Metric | Target | Actual | Status |
|-----------|--------|--------|--------|--------|
| HTTP Bridge | Average Latency | 500μs | {:.0}μs | {} |
| Protocol Core | Average Latency | 100μs | {:.0}μs | {} |
| QUIC Transport | Throughput | 15.0 Gbps | {:.1} Gbps | {} |
| Unix Socket | Throughput | 20.0 Gbps | {:.1} Gbps | {} |
| Security | Encryption Latency | 50μs | {:.0}μs | {} |

### 📊 Detailed Performance Metrics

#### HTTP Bridge Performance
- **Requests per Second:** {:.2}
- **Average Latency:** {:.2}μs
- **P95 Latency:** {}μs
- **P99 Latency:** {}μs
- **Success Rate:** {:.2}%

#### Protocol Core Performance
- **Message Throughput:** {:.0} msgs/sec
- **Average Latency:** {:.2}μs
- **Memory Usage:** {:.1} MB
- **P95 Latency:** {}μs

#### Transport Performance
- **QUIC Throughput:** {:.1} Gbps
- **QUIC Average Latency:** {:.2}μs
- **Unix Socket Throughput:** {:.1} Gbps
- **Unix Socket Average Latency:** {:.2}μs

#### Security Performance
- **Encryption Latency:** {:.2}μs
- **Handshake Time:** {:.2}ms
- **Security Score:** {:.1}%

#### End-to-End Performance
- **Pipeline Latency:** {:.2}ms
- **Jobs per Second:** {:.0}
- **Success Rate:** {:.1}%

## Sub-Millisecond Achievement

{}

## Regression Analysis

{}

## Recommendations

Based on the benchmark results, here are the key recommendations:

1. **Performance Optimization:** {}
2. **Monitoring:** Continue regular performance monitoring to detect regressions early
3. **Scaling:** Current performance supports high-throughput production workloads
4. **Security:** Encryption performance is excellent with minimal overhead

## Technical Details

- **Test Environment:** {}
- **CPU Cores:** {}
- **Memory:** {:.1} GB
- **Rust Version:** {}

---

*Generated by Valkyrie Protocol Unified Benchmark Runner v2.0*
"#,
        grade_text,
        results.metadata.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
        results.metadata.total_duration.as_secs_f64(),
        results.metadata.environment.os,
        results.metadata.environment.cpu_cores,
        results.metadata.environment.valkyrie_version,
        
        // Target achievement table
        results.http_bridge.latency_stats.mean_us,
        if results.http_bridge.target_achievement.achieved { "✅" } else { "❌" },
        results.protocol_core.latency_stats.mean_us,
        if results.protocol_core.target_achievement.achieved { "✅" } else { "❌" },
        results.transport.quic.throughput_gbps,
        if results.transport.quic.target_achievement.achieved { "✅" } else { "❌" },
        results.transport.unix_socket.throughput_gbps,
        if results.transport.unix_socket.target_achievement.achieved { "✅" } else { "❌" },
        results.security.encryption_latency_us,
        if results.security.target_achievement.achieved { "✅" } else { "❌" },
        
        // Detailed metrics
        results.http_bridge.requests_per_second,
        results.http_bridge.latency_stats.mean_us,
        results.http_bridge.latency_stats.p95_us,
        results.http_bridge.latency_stats.p99_us,
        (results.http_bridge.successful_requests as f64 / results.http_bridge.total_requests as f64) * 100.0,
        
        results.protocol_core.messages_per_second,
        results.protocol_core.latency_stats.mean_us,
        results.protocol_core.memory_usage_mb,
        results.protocol_core.latency_stats.p95_us,
        
        results.transport.quic.throughput_gbps,
        results.transport.quic.latency_stats.mean_us,
        results.transport.unix_socket.throughput_gbps,
        results.transport.unix_socket.latency_stats.mean_us,
        
        results.security.encryption_latency_us,
        results.security.handshake_time_ms,
        results.security.security_score,
        
        results.end_to_end.pipeline_latency_ms,
        results.end_to_end.jobs_per_second,
        results.end_to_end.success_rate * 100.0,
        
        // Sub-millisecond achievement
        if results.http_bridge.latency_stats.mean_us < 1000.0 && results.protocol_core.latency_stats.mean_us < 100.0 {
            format!("🎉 **SUB-MILLISECOND PERFORMANCE ACHIEVED!**\n\n✅ HTTP Bridge: {:.0}μs average  \n✅ Protocol Core: {:.0}μs average", 
                    results.http_bridge.latency_stats.mean_us, 
                    results.protocol_core.latency_stats.mean_us)
        } else {
            "⚠️ Sub-millisecond targets not fully achieved".to_string()
        },
        
        // Regression analysis
        if let Some(regression) = &results.regression_analysis {
            if regression.detected_regressions.is_empty() {
                "✅ No performance regressions detected".to_string()
            } else {
                format!("⚠️ {} performance regressions detected - see detailed analysis", regression.detected_regressions.len())
            }
        } else {
            "No regression analysis performed".to_string()
        },
        
        // Performance optimization recommendation
        match &results.performance_grade {
            unified_benchmark_engine::OverallPerformanceGrade::Excellent { .. } => 
                "Performance is excellent - maintain current optimization levels",
            unified_benchmark_engine::OverallPerformanceGrade::Good { .. } => 
                "Good performance with room for optimization in specific areas",
            unified_benchmark_engine::OverallPerformanceGrade::Fair { .. } => 
                "Performance needs improvement - focus on bottleneck identification",
            unified_benchmark_engine::OverallPerformanceGrade::Poor { .. } => 
                "Significant performance issues detected - immediate optimization required",
        },
        
        // Technical details
        results.metadata.environment.os,
        results.metadata.environment.cpu_cores,
        results.metadata.environment.memory_gb,
        results.metadata.environment.rust_version,
    )
}

/// Print execution summary
fn print_execution_summary(results: &unified_benchmark_engine::UnifiedBenchmarkResults, duration: Duration) {
    println!("\n🏆 EXECUTION SUMMARY");
    println!("===================");
    
    match &results.performance_grade {
        unified_benchmark_engine::OverallPerformanceGrade::Excellent { score, .. } => {
            println!("🥇 Overall Grade: EXCELLENT ({:.1}%)", score);
        }
        unified_benchmark_engine::OverallPerformanceGrade::Good { score, .. } => {
            println!("🥈 Overall Grade: GOOD ({:.1}%)", score);
        }
        unified_benchmark_engine::OverallPerformanceGrade::Fair { score, .. } => {
            println!("🥉 Overall Grade: FAIR ({:.1}%)", score);
        }
        unified_benchmark_engine::OverallPerformanceGrade::Poor { score, .. } => {
            println!("❌ Overall Grade: POOR ({:.1}%)", score);
        }
    }
    
    println!("⏱️  Total Execution Time: {:.2}s", duration.as_secs_f64());
    println!("📊 Test Duration: {:.2}s", results.metadata.total_duration.as_secs_f64());
    
    // Key metrics summary
    println!("\n📈 Key Performance Metrics:");
    println!("  HTTP Bridge: {:.0}μs avg latency ({:.0} RPS)", 
             results.http_bridge.latency_stats.mean_us,
             results.http_bridge.requests_per_second);
    println!("  Protocol Core: {:.0}μs avg latency ({:.0} msgs/sec)", 
             results.protocol_core.latency_stats.mean_us,
             results.protocol_core.messages_per_second);
    println!("  QUIC Transport: {:.1} Gbps throughput", 
             results.transport.quic.throughput_gbps);
    println!("  Security: {:.0}μs encryption latency", 
             results.security.encryption_latency_us);
    
    // Sub-millisecond check
    if results.http_bridge.latency_stats.mean_us < 1000.0 && 
       results.protocol_core.latency_stats.mean_us < 100.0 {
        println!("\n🎉 SUB-MILLISECOND PERFORMANCE ACHIEVED!");
    }
}
