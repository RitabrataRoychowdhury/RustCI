use clap::{Arg, Command};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tracing::{error, info, warn};
use uuid::Uuid;
use RustAutoDevOps::error::AppError;

use RustAutoDevOps::core::performance::{
    validation::*,
    monitor::{PerformanceMonitor, ProductionPerformanceMonitor},
    metrics::PerformanceMetrics,
};
use RustAutoDevOps::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let matches = Command::new("Performance Validator")
        .version("1.0.0")
        .author("RustCI Team")
        .about("Comprehensive performance validation and benchmarking tool")
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("Validation mode: quick, full, benchmark, regression")
                .default_value("full")
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("DIR")
                .help("Output directory for reports")
                .default_value("./performance_reports")
        )
        .arg(
            Arg::new("baseline")
                .short('b')
                .long("baseline")
                .value_name("FILE")
                .help("Baseline performance data file for comparison")
        )
        .arg(
            Arg::new("duration")
                .short('d')
                .long("duration")
                .value_name("SECONDS")
                .help("Test duration in seconds")
                .default_value("300")
        )
        .arg(
            Arg::new("concurrent-users")
                .short('c')
                .long("concurrent-users")
                .value_name("COUNT")
                .help("Number of concurrent users for load testing")
                .default_value("100")
        )
        .arg(
            Arg::new("requirements")
                .short('r')
                .long("requirements")
                .value_name("FILE")
                .help("Performance requirements configuration file")
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("export-format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Export format: json, csv, markdown, all")
                .default_value("all")
        )
        .get_matches();

    let mode = matches.get_one::<String>("mode").unwrap();
    let output_dir = PathBuf::from(matches.get_one::<String>("output").unwrap());
    let baseline_file = matches.get_one::<String>("baseline");
    let duration_secs: u64 = matches.get_one::<String>("duration").unwrap().parse()
        .expect("Invalid duration value");
    let concurrent_users: u32 = matches.get_one::<String>("concurrent-users").unwrap().parse()
        .expect("Invalid concurrent users value");
    let requirements_file = matches.get_one::<String>("requirements");
    let verbose = matches.get_flag("verbose");
    let export_format = matches.get_one::<String>("export-format").unwrap();

    // Create output directory
    fs::create_dir_all(&output_dir).await?;

    info!("🚀 Starting Performance Validation");
    info!("Mode: {}", mode);
    info!("Output Directory: {:?}", output_dir);
    info!("Test Duration: {}s", duration_secs);
    info!("Concurrent Users: {}", concurrent_users);

    // Load performance requirements
    let requirements = if let Some(req_file) = requirements_file {
        load_requirements_from_file(req_file).await?
    } else {
        PerformanceRequirements::default()
    };

    // Create validation configuration
    let config = ValidationConfig {
        test_duration: Duration::from_secs(duration_secs),
        warmup_duration: Duration::from_secs(30),
        concurrent_users: vec![concurrent_users / 4, concurrent_users / 2, concurrent_users],
        load_patterns: create_load_patterns(duration_secs, concurrent_users),
        stress_test_enabled: mode == "full" || mode == "benchmark",
        regression_detection_enabled: mode == "full" || mode == "regression",
    };

    // Create performance monitor
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(5),
        1000,
    ));

    // Create validator
    let validator = ProductionPerformanceValidator::new(monitor, requirements, config);

    // Load baseline if provided
    if let Some(baseline_file) = baseline_file {
        info!("📊 Loading baseline from: {}", baseline_file);
        let baseline = load_baseline_from_file(baseline_file).await?;
        validator.set_baseline(baseline.version.clone(), baseline.metrics).await?;
    }

    // Execute based on mode
    match mode.as_str() {
        "quick" => {
            info!("⚡ Running quick validation...");
            run_quick_validation(&validator, &output_dir, export_format, verbose).await?;
        }
        "full" => {
            info!("🎯 Running full validation suite...");
            run_full_validation(&validator, &output_dir, export_format, verbose).await?;
        }
        "benchmark" => {
            info!("🏋️ Running benchmark suite...");
            run_benchmark_suite(&validator, &output_dir, export_format, verbose).await?;
        }
        "regression" => {
            info!("🔍 Running regression analysis...");
            run_regression_analysis(&validator, &output_dir, export_format, verbose).await?;
        }
        _ => {
            error!("Unknown mode: {}", mode);
            std::process::exit(1);
        }
    }

    info!("✅ Performance validation completed successfully!");
    info!("📄 Reports saved to: {:?}", output_dir);

    Ok(())
}

async fn run_quick_validation(
    validator: &ProductionPerformanceValidator,
    output_dir: &PathBuf,
    export_format: &str,
    verbose: bool,
) -> Result<()> {
    info!("Running quick performance validation...");

    // Run basic benchmark suite
    let benchmark_results = validator.run_benchmark_suite().await?;

    if verbose {
        print_benchmark_summary(&benchmark_results);
    }

    // Export results
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let base_filename = output_dir.join(format!("quick_validation_{}", timestamp));

    export_benchmark_results(&benchmark_results, &base_filename, export_format).await?;

    // Print summary
    print_performance_grade(&benchmark_results.performance_grade);

    Ok(())
}

async fn run_full_validation(
    validator: &ProductionPerformanceValidator,
    output_dir: &PathBuf,
    export_format: &str,
    verbose: bool,
) -> Result<()> {
    info!("Running full performance validation suite...");

    // Run complete validation suite
    let validation_results = validator.run_validation_suite().await?;

    if verbose {
        print_validation_summary(&validation_results);
    }

    // Generate performance report
    let performance_report = validator.generate_performance_report(&validation_results).await?;

    // Export results
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let base_filename = output_dir.join(format!("full_validation_{}", timestamp));

    export_validation_results(&validation_results, &base_filename, export_format).await?;
    export_performance_report(&performance_report, &base_filename, export_format).await?;

    // Print summary
    print_executive_summary(&performance_report.executive_summary);
    print_recommendations(&performance_report.recommendations);

    Ok(())
}

async fn run_benchmark_suite(
    validator: &ProductionPerformanceValidator,
    output_dir: &PathBuf,
    export_format: &str,
    verbose: bool,
) -> Result<()> {
    info!("Running comprehensive benchmark suite...");

    // Run benchmark suite
    let benchmark_results = validator.run_benchmark_suite().await?;

    if verbose {
        print_detailed_benchmark_results(&benchmark_results);
    }

    // Export results
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let base_filename = output_dir.join(format!("benchmark_suite_{}", timestamp));

    export_benchmark_results(&benchmark_results, &base_filename, export_format).await?;

    // Print detailed analysis
    print_latency_analysis(&benchmark_results.latency_benchmarks);
    print_throughput_analysis(&benchmark_results.throughput_benchmarks);
    print_resource_analysis(&benchmark_results.resource_benchmarks);
    print_stress_test_analysis(&benchmark_results.stress_test_results);

    Ok(())
}

async fn run_regression_analysis(
    validator: &ProductionPerformanceValidator,
    output_dir: &PathBuf,
    export_format: &str,
    _verbose: bool,
) -> Result<()> {
    info!("Running regression analysis...");

    // For regression analysis, we need historical data
    // In a real implementation, this would load from a database or file system
    let historical_data = load_historical_performance_data().await?;

    if historical_data.len() < 2 {
        warn!("Insufficient historical data for regression analysis");
        return Ok(());
    }

    // Run regression detection
    let regression_report = validator.detect_regressions(&historical_data).await?;

    // Export results
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let base_filename = output_dir.join(format!("regression_analysis_{}", timestamp));

    export_regression_report(&regression_report, &base_filename, export_format).await?;

    // Print regression analysis
    print_regression_analysis(&regression_report);

    Ok(())
}

// Helper functions for printing results

fn print_benchmark_summary(results: &BenchmarkResults) {
    println!("\n📊 Benchmark Summary");
    println!("==================");
    println!("Test Suite: {}", results.test_suite);
    println!("Environment: {} ({} cores)", results.environment.os, results.environment.cpu_cores);
    println!("Performance Grade: {:?}", results.performance_grade);
    
    println!("\n🚀 Key Metrics:");
    println!("  API Latency: {:.2}μs (P95: {:.2}μs)", 
             results.latency_benchmarks.api_latency.mean_us,
             results.latency_benchmarks.api_latency.p95_us);
    println!("  Throughput: {:.0} RPS", results.throughput_benchmarks.requests_per_second);
    println!("  CPU Efficiency: {:.1}%", results.resource_benchmarks.cpu_efficiency);
    println!("  Memory Efficiency: {:.1}%", results.resource_benchmarks.memory_efficiency);
}

fn print_validation_summary(results: &ValidationResults) {
    println!("\n🎯 Validation Summary");
    println!("====================");
    println!("Overall Status: {:?}", results.overall_status);
    println!("Duration: {:.2}s", results.duration.as_secs_f64());
    println!("Components Tested: {}", results.component_results.len());
    
    println!("\n📋 Component Results:");
    for (name, result) in &results.component_results {
        let status_emoji = match result.status {
            ValidationStatus::Passed => "✅",
            ValidationStatus::Failed => "❌",
            ValidationStatus::Warning => "⚠️",
            ValidationStatus::Skipped => "⏭️",
        };
        println!("  {} {}: {:.1}% ({})", 
                 status_emoji, name, result.performance_score, 
                 if result.requirements_met { "Requirements Met" } else { "Requirements Not Met" });
    }
}

fn print_detailed_benchmark_results(results: &BenchmarkResults) {
    println!("\n🏋️ Detailed Benchmark Results");
    println!("=============================");
    
    print_latency_analysis(&results.latency_benchmarks);
    print_throughput_analysis(&results.throughput_benchmarks);
    print_resource_analysis(&results.resource_benchmarks);
    print_stress_test_analysis(&results.stress_test_results);
    print_load_test_analysis(&results.load_test_results);
}

fn print_latency_analysis(latency: &LatencyBenchmarks) {
    println!("\n🚀 Latency Analysis:");
    println!("  API Latency:");
    println!("    Mean: {:.2}μs", latency.api_latency.mean_us);
    println!("    P95:  {:.2}μs", latency.api_latency.p95_us);
    println!("    P99:  {:.2}μs", latency.api_latency.p99_us);
    println!("    Max:  {:.2}μs", latency.api_latency.max_us);
    
    println!("  Database Latency:");
    println!("    Mean: {:.2}μs", latency.database_latency.mean_us);
    println!("    P95:  {:.2}μs", latency.database_latency.p95_us);
    
    println!("  Cache Latency:");
    println!("    Mean: {:.2}μs", latency.cache_latency.mean_us);
    println!("    P95:  {:.2}μs", latency.cache_latency.p95_us);
    
    // Sub-millisecond achievement check
    if latency.api_latency.mean_us < 1000.0 && latency.cache_latency.mean_us < 100.0 {
        println!("  🎉 SUB-MILLISECOND PERFORMANCE ACHIEVED!");
    }
}

fn print_throughput_analysis(throughput: &ThroughputBenchmarks) {
    println!("\n📈 Throughput Analysis:");
    println!("  Requests per Second: {:.0}", throughput.requests_per_second);
    println!("  Transactions per Second: {:.0}", throughput.transactions_per_second);
    println!("  Messages per Second: {:.0}", throughput.messages_per_second);
    println!("  Data Throughput: {:.1} Mbps", throughput.data_throughput_mbps);
    println!("  Concurrent Users: {}", throughput.concurrent_users);
}

fn print_resource_analysis(resources: &ResourceBenchmarks) {
    println!("\n💾 Resource Analysis:");
    println!("  CPU Efficiency: {:.1}%", resources.cpu_efficiency);
    println!("  Memory Efficiency: {:.1}%", resources.memory_efficiency);
    println!("  Disk Efficiency: {:.1}%", resources.disk_efficiency);
    println!("  Network Efficiency: {:.1}%", resources.network_efficiency);
    
    println!("  Resource Utilization:");
    println!("    CPU Peak: {:.1}%", resources.resource_utilization.cpu_peak);
    println!("    Memory Peak: {:.1}%", resources.resource_utilization.memory_peak);
    println!("    Disk I/O Peak: {:.1} IOPS", resources.resource_utilization.disk_io_peak);
}

fn print_stress_test_analysis(stress: &StressTestResults) {
    println!("\n💪 Stress Test Analysis:");
    println!("  Max Concurrent Users: {}", stress.max_concurrent_users);
    println!("  Breaking Point RPS: {:.0}", stress.breaking_point_rps);
    println!("  Recovery Time: {:.1}s", stress.recovery_time.as_secs_f64());
    println!("  Stability Score: {:.1}%", stress.stability_score);
    println!("  Error Rate Under Stress: {:.2}%", stress.error_rate_under_stress);
}

fn print_load_test_analysis(load: &LoadTestResults) {
    println!("\n🔄 Load Test Analysis:");
    println!("  Sustained Load Duration: {:.1}s", load.sustained_load_duration.as_secs_f64());
    println!("  Average Response Time: {:.1}ms", load.average_response_time.as_millis());
    println!("  Throughput Consistency: {:.1}%", load.throughput_consistency);
    println!("  Resource Stability: {:.1}%", load.resource_stability);
    println!("  Memory Leak Detected: {}", if load.memory_leak_detected { "Yes" } else { "No" });
}

fn print_performance_grade(grade: &PerformanceGrade) {
    match grade {
        PerformanceGrade::Excellent { score, details } => {
            println!("\n🥇 Performance Grade: EXCELLENT ({:.1}%)", score);
            println!("   {}", details);
        }
        PerformanceGrade::Good { score, details } => {
            println!("\n🥈 Performance Grade: GOOD ({:.1}%)", score);
            println!("   {}", details);
        }
        PerformanceGrade::Fair { score, details } => {
            println!("\n🥉 Performance Grade: FAIR ({:.1}%)", score);
            println!("   {}", details);
        }
        PerformanceGrade::Poor { score, details } => {
            println!("\n❌ Performance Grade: POOR ({:.1}%)", score);
            println!("   {}", details);
        }
    }
}

fn print_executive_summary(summary: &ExecutiveSummary) {
    println!("\n🎯 Executive Summary");
    println!("==================");
    
    print_performance_grade(&summary.overall_grade);
    
    println!("\n📊 Key Metrics:");
    for (metric, value) in &summary.key_metrics {
        println!("  {}: {:.2}", metric, value);
    }
    
    if !summary.critical_issues.is_empty() {
        println!("\n🚨 Critical Issues:");
        for issue in &summary.critical_issues {
            println!("  • {}", issue);
        }
    }
    
    if !summary.achievements.is_empty() {
        println!("\n🏆 Achievements:");
        for achievement in &summary.achievements {
            println!("  • {}", achievement);
        }
    }
}

fn print_recommendations(recommendations: &[PerformanceRecommendation]) {
    if recommendations.is_empty() {
        return;
    }
    
    println!("\n💡 Recommendations");
    println!("==================");
    
    for (i, rec) in recommendations.iter().enumerate() {
        let priority_emoji = match rec.priority {
            RecommendationPriority::Critical => "🔴",
            RecommendationPriority::High => "🟠",
            RecommendationPriority::Medium => "🟡",
            RecommendationPriority::Low => "🟢",
        };
        
        println!("{}. {} {} {}", i + 1, priority_emoji, rec.title, 
                 format!("({:?} Priority)", rec.priority));
        println!("   {}", rec.description);
        println!("   Expected Impact: {}", rec.expected_impact);
        println!("   Implementation Effort: {:?}", rec.implementation_effort);
        println!();
    }
}

fn print_regression_analysis(report: &RegressionReport) {
    println!("\n🔍 Regression Analysis");
    println!("=====================");
    
    if report.detected_regressions.is_empty() {
        println!("✅ No performance regressions detected");
        return;
    }
    
    println!("⚠️  {} regressions detected (Severity: {:?})", 
             report.detected_regressions.len(), report.severity_summary);
    
    for (i, regression) in report.detected_regressions.iter().enumerate() {
        let severity_emoji = match regression.severity {
            RegressionSeverity::Critical => "🔴",
            RegressionSeverity::High => "🟠",
            RegressionSeverity::Medium => "🟡",
            RegressionSeverity::Low => "🟢",
        };
        
        println!("\n{}. {} {} in {}", i + 1, severity_emoji, regression.metric_name, regression.component);
        println!("   Baseline: {:.2}", regression.baseline_value);
        println!("   Current: {:.2}", regression.current_value);
        println!("   Degradation: {:.1}%", regression.degradation_percent);
        
        if let Some(analysis) = &regression.root_cause_analysis {
            println!("   Analysis: {}", analysis);
        }
    }
    
    println!("\n📋 Recommended Actions:");
    for (i, action) in report.recommended_actions.iter().enumerate() {
        println!("  {}. {}", i + 1, action);
    }
}

// Helper functions for loading and exporting data

async fn load_requirements_from_file(file_path: &str) -> Result<PerformanceRequirements> {
    let content = fs::read_to_string(file_path).await?;
    let requirements: PerformanceRequirements = serde_json::from_str(&content)?;
    Ok(requirements)
}

async fn load_baseline_from_file(file_path: &str) -> Result<PerformanceBaseline> {
    let content = fs::read_to_string(file_path).await?;
    let baseline: PerformanceBaseline = serde_json::from_str(&content)?;
    Ok(baseline)
}

async fn load_historical_performance_data() -> Result<Vec<PerformanceMetrics>> {
    // In a real implementation, this would load from a database or file system
    // For now, return empty data
    Ok(Vec::new())
}

fn create_load_patterns(duration_secs: u64, concurrent_users: u32) -> Vec<LoadPattern> {
    vec![
        LoadPattern {
            name: "Warmup".to_string(),
            duration: Duration::from_secs(30),
            rps: 100.0,
            concurrent_users: concurrent_users / 4,
        },
        LoadPattern {
            name: "Steady Load".to_string(),
            duration: Duration::from_secs(duration_secs / 2),
            rps: 1000.0,
            concurrent_users: concurrent_users / 2,
        },
        LoadPattern {
            name: "Peak Load".to_string(),
            duration: Duration::from_secs(duration_secs / 4),
            rps: 5000.0,
            concurrent_users,
        },
        LoadPattern {
            name: "Cool Down".to_string(),
            duration: Duration::from_secs(duration_secs / 4),
            rps: 500.0,
            concurrent_users: concurrent_users / 4,
        },
    ]
}

// Export functions

async fn export_benchmark_results(
    results: &BenchmarkResults,
    base_filename: &PathBuf,
    format: &str,
) -> Result<()> {
    match format {
        "json" => export_json(results, &base_filename.with_extension("json")).await?,
        "csv" => export_benchmark_csv(results, &base_filename.with_extension("csv")).await?,
        "markdown" => export_benchmark_markdown(results, &base_filename.with_extension("md")).await?,
        "all" => {
            export_json(results, &base_filename.with_extension("json")).await?;
            export_benchmark_csv(results, &base_filename.with_extension("csv")).await?;
            export_benchmark_markdown(results, &base_filename.with_extension("md")).await?;
        }
        _ => return Err(AppError::ValidationError("Invalid export format".to_string())),
    }
    Ok(())
}

async fn export_validation_results(
    results: &ValidationResults,
    base_filename: &PathBuf,
    format: &str,
) -> Result<()> {
    match format {
        "json" => export_json(results, &base_filename.with_extension("json")).await?,
        "all" => {
            export_json(results, &base_filename.with_extension("json")).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn export_performance_report(
    report: &PerformanceReport,
    base_filename: &PathBuf,
    format: &str,
) -> Result<()> {
    match format {
        "json" => export_json(report, &base_filename.with_suffix("_report").with_extension("json")).await?,
        "markdown" => export_report_markdown(report, &base_filename.with_suffix("_report").with_extension("md")).await?,
        "all" => {
            export_json(report, &base_filename.with_suffix("_report").with_extension("json")).await?;
            export_report_markdown(report, &base_filename.with_suffix("_report").with_extension("md")).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn export_regression_report(
    report: &RegressionReport,
    base_filename: &PathBuf,
    format: &str,
) -> Result<()> {
    match format {
        "json" => export_json(report, &base_filename.with_extension("json")).await?,
        "all" => {
            export_json(report, &base_filename.with_extension("json")).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn export_json<T: serde::Serialize>(data: &T, file_path: &PathBuf) -> Result<()> {
    let json_content = serde_json::to_string_pretty(data)?;
    fs::write(file_path, json_content).await?;
    info!("📄 Exported JSON: {:?}", file_path);
    Ok(())
}

async fn export_benchmark_csv(results: &BenchmarkResults, file_path: &PathBuf) -> Result<()> {
    let mut csv_content = String::new();
    csv_content.push_str("Metric,Component,Value,Unit\n");
    
    // Latency metrics
    csv_content.push_str(&format!("Mean Latency,API,{:.2},microseconds\n", results.latency_benchmarks.api_latency.mean_us));
    csv_content.push_str(&format!("P95 Latency,API,{:.2},microseconds\n", results.latency_benchmarks.api_latency.p95_us));
    csv_content.push_str(&format!("Mean Latency,Database,{:.2},microseconds\n", results.latency_benchmarks.database_latency.mean_us));
    csv_content.push_str(&format!("Mean Latency,Cache,{:.2},microseconds\n", results.latency_benchmarks.cache_latency.mean_us));
    
    // Throughput metrics
    csv_content.push_str(&format!("Requests per Second,System,{:.0},rps\n", results.throughput_benchmarks.requests_per_second));
    csv_content.push_str(&format!("Transactions per Second,Database,{:.0},tps\n", results.throughput_benchmarks.transactions_per_second));
    
    // Resource metrics
    csv_content.push_str(&format!("CPU Efficiency,System,{:.1},percent\n", results.resource_benchmarks.cpu_efficiency));
    csv_content.push_str(&format!("Memory Efficiency,System,{:.1},percent\n", results.resource_benchmarks.memory_efficiency));
    
    fs::write(file_path, csv_content).await?;
    info!("📄 Exported CSV: {:?}", file_path);
    Ok(())
}

async fn export_benchmark_markdown(results: &BenchmarkResults, file_path: &PathBuf) -> Result<()> {
    let grade_emoji = match &results.performance_grade {
        PerformanceGrade::Excellent { .. } => "🥇",
        PerformanceGrade::Good { .. } => "🥈",
        PerformanceGrade::Fair { .. } => "🥉",
        PerformanceGrade::Poor { .. } => "❌",
    };
    
    let content = format!(r#"# Performance Benchmark Report

## Summary

**Performance Grade:** {} {:?}
**Test Suite:** {}
**Environment:** {} ({} cores, {:.1} GB RAM)
**Timestamp:** {}

## Latency Benchmarks

| Component | Mean (μs) | P95 (μs) | P99 (μs) | Max (μs) |
|-----------|-----------|----------|----------|----------|
| API | {:.2} | {:.2} | {:.2} | {:.2} |
| Database | {:.2} | {:.2} | {:.2} | {:.2} |
| Cache | {:.2} | {:.2} | {:.2} | {:.2} |
| Network | {:.2} | {:.2} | {:.2} | {:.2} |
| End-to-End | {:.2} | {:.2} | {:.2} | {:.2} |

## Throughput Benchmarks

| Metric | Value | Unit |
|--------|-------|------|
| Requests per Second | {:.0} | RPS |
| Transactions per Second | {:.0} | TPS |
| Messages per Second | {:.0} | MPS |
| Data Throughput | {:.1} | Mbps |
| Concurrent Users | {} | Users |

## Resource Efficiency

| Resource | Efficiency | Peak Usage |
|----------|------------|------------|
| CPU | {:.1}% | {:.1}% |
| Memory | {:.1}% | {:.1}% |
| Disk | {:.1}% | {:.1} IOPS |
| Network | {:.1}% | {:.1}% |

## Stress Test Results

- **Max Concurrent Users:** {}
- **Breaking Point RPS:** {:.0}
- **Recovery Time:** {:.1}s
- **Stability Score:** {:.1}%
- **Error Rate Under Stress:** {:.2}%

## Load Test Results

- **Sustained Load Duration:** {:.1}s
- **Average Response Time:** {:.1}ms
- **Throughput Consistency:** {:.1}%
- **Resource Stability:** {:.1}%
- **Memory Leak Detected:** {}

---
*Generated by RustCI Performance Validator*
"#,
        grade_emoji, results.performance_grade,
        results.test_suite,
        results.environment.os, results.environment.cpu_cores, results.environment.memory_gb,
        chrono::DateTime::from_timestamp(results.timestamp as i64, 0).unwrap().format("%Y-%m-%d %H:%M:%S UTC"),
        
        // Latency table
        results.latency_benchmarks.api_latency.mean_us, results.latency_benchmarks.api_latency.p95_us, 
        results.latency_benchmarks.api_latency.p99_us, results.latency_benchmarks.api_latency.max_us,
        results.latency_benchmarks.database_latency.mean_us, results.latency_benchmarks.database_latency.p95_us,
        results.latency_benchmarks.database_latency.p99_us, results.latency_benchmarks.database_latency.max_us,
        results.latency_benchmarks.cache_latency.mean_us, results.latency_benchmarks.cache_latency.p95_us,
        results.latency_benchmarks.cache_latency.p99_us, results.latency_benchmarks.cache_latency.max_us,
        results.latency_benchmarks.network_latency.mean_us, results.latency_benchmarks.network_latency.p95_us,
        results.latency_benchmarks.network_latency.p99_us, results.latency_benchmarks.network_latency.max_us,
        results.latency_benchmarks.end_to_end_latency.mean_us, results.latency_benchmarks.end_to_end_latency.p95_us,
        results.latency_benchmarks.end_to_end_latency.p99_us, results.latency_benchmarks.end_to_end_latency.max_us,
        
        // Throughput table
        results.throughput_benchmarks.requests_per_second,
        results.throughput_benchmarks.transactions_per_second,
        results.throughput_benchmarks.messages_per_second,
        results.throughput_benchmarks.data_throughput_mbps,
        results.throughput_benchmarks.concurrent_users,
        
        // Resource table
        results.resource_benchmarks.cpu_efficiency, results.resource_benchmarks.resource_utilization.cpu_peak,
        results.resource_benchmarks.memory_efficiency, results.resource_benchmarks.resource_utilization.memory_peak,
        results.resource_benchmarks.disk_efficiency, results.resource_benchmarks.resource_utilization.disk_io_peak,
        results.resource_benchmarks.network_efficiency, results.resource_benchmarks.resource_utilization.network_io_peak,
        
        // Stress test
        results.stress_test_results.max_concurrent_users,
        results.stress_test_results.breaking_point_rps,
        results.stress_test_results.recovery_time.as_secs_f64(),
        results.stress_test_results.stability_score,
        results.stress_test_results.error_rate_under_stress,
        
        // Load test
        results.load_test_results.sustained_load_duration.as_secs_f64(),
        results.load_test_results.average_response_time.as_millis(),
        results.load_test_results.throughput_consistency,
        results.load_test_results.resource_stability,
        if results.load_test_results.memory_leak_detected { "Yes" } else { "No" },
    );
    
    fs::write(file_path, content).await?;
    info!("📄 Exported Markdown: {:?}", file_path);
    Ok(())
}

async fn export_report_markdown(report: &PerformanceReport, file_path: &PathBuf) -> Result<()> {
    let content = format!(r#"# Performance Validation Report

## Executive Summary

{:?}

### Key Metrics
{}

### Critical Issues
{}

### Achievements
{}

## Recommendations

{}

## Next Steps

{}

---
*Generated by RustCI Performance Validator*
"#,
        report.executive_summary.overall_grade,
        report.executive_summary.key_metrics.iter()
            .map(|(k, v)| format!("- **{}:** {:.2}", k, v))
            .collect::<Vec<_>>()
            .join("\n"),
        if report.executive_summary.critical_issues.is_empty() {
            "None".to_string()
        } else {
            report.executive_summary.critical_issues.iter()
                .map(|issue| format!("- {}", issue))
                .collect::<Vec<_>>()
                .join("\n")
        },
        if report.executive_summary.achievements.is_empty() {
            "None".to_string()
        } else {
            report.executive_summary.achievements.iter()
                .map(|achievement| format!("- {}", achievement))
                .collect::<Vec<_>>()
                .join("\n")
        },
        report.recommendations.iter()
            .map(|rec| format!("### {} ({:?} Priority)\n{}\n\n**Expected Impact:** {}\n**Implementation Effort:** {:?}", 
                              rec.title, rec.priority, rec.description, rec.expected_impact, rec.implementation_effort))
            .collect::<Vec<_>>()
            .join("\n\n"),
        report.next_steps.iter()
            .enumerate()
            .map(|(i, step)| format!("{}. {}", i + 1, step))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    
    fs::write(file_path, content).await?;
    info!("📄 Exported Report Markdown: {:?}", file_path);
    Ok(())
}

// Extension trait for PathBuf
trait PathBufExt {
    fn with_suffix(&self, suffix: &str) -> PathBuf;
}

impl PathBufExt for PathBuf {
    fn with_suffix(&self, suffix: &str) -> PathBuf {
        let stem = self.file_stem().unwrap().to_str().unwrap();
        let new_name = format!("{}{}", stem, suffix);
        self.with_file_name(new_name)
    }
}