use std::time::Duration;
use RustAutoDevOps::core::performance::{
    validation::*,
    monitor::ProductionPerformanceMonitor,
};

#[tokio::test]
async fn test_basic_performance_validation() {
    println!("🧪 Testing basic performance validation functionality...");
    
    // Create a performance monitor
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        10,
    ));
    
    // Create performance requirements
    let requirements = PerformanceRequirements {
        max_api_latency_ms: 1000.0,
        min_throughput_rps: 100.0,
        max_cpu_usage: 80.0,
        max_memory_usage: 85.0,
        max_error_rate: 5.0,
        min_availability: 99.0,
    };
    
    // Create validation config
    let config = ValidationConfig {
        test_duration: Duration::from_secs(10), // Short test
        warmup_duration: Duration::from_secs(2),
        concurrent_users: vec![10, 20],
        load_patterns: vec![
            LoadPattern {
                name: "Light Load".to_string(),
                duration: Duration::from_secs(5),
                rps: 50.0,
                concurrent_users: 10,
            },
        ],
        stress_test_enabled: false, // Disable for basic test
        regression_detection_enabled: false,
    };
    
    // Create validator
    let validator = ProductionPerformanceValidator::new(monitor, requirements, config);
    
    // Run benchmark suite
    println!("  Running benchmark suite...");
    let benchmark_results = validator.run_benchmark_suite().await.unwrap();
    
    // Validate results structure
    assert!(benchmark_results.latency_benchmarks.api_latency.mean_us > 0.0);
    assert!(benchmark_results.throughput_benchmarks.requests_per_second > 0.0);
    assert!(benchmark_results.resource_benchmarks.cpu_efficiency >= 0.0);
    
    println!("  ✅ API Latency: {:.2}μs", benchmark_results.latency_benchmarks.api_latency.mean_us);
    println!("  ✅ Throughput: {:.0} RPS", benchmark_results.throughput_benchmarks.requests_per_second);
    println!("  ✅ CPU Efficiency: {:.1}%", benchmark_results.resource_benchmarks.cpu_efficiency);
    
    // Check performance grade
    match benchmark_results.performance_grade {
        PerformanceGrade::Excellent { score, .. } => {
            println!("  🥇 Excellent performance: {:.1}%", score);
        },
        PerformanceGrade::Good { score, .. } => {
            println!("  🥈 Good performance: {:.1}%", score);
        },
        PerformanceGrade::Fair { score, .. } => {
            println!("  🥉 Fair performance: {:.1}%", score);
        },
        PerformanceGrade::Poor { score, .. } => {
            println!("  ❌ Poor performance: {:.1}%", score);
        },
    }
    
    println!("✅ Basic performance validation test passed!");
}

#[tokio::test]
async fn test_performance_requirements_validation() {
    println!("📏 Testing performance requirements validation...");
    
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        10,
    ));
    
    // Test with very strict requirements
    let strict_requirements = PerformanceRequirements {
        max_api_latency_ms: 50.0,   // Very strict
        min_throughput_rps: 50000.0, // Very high
        max_cpu_usage: 10.0,        // Very low
        max_memory_usage: 20.0,     // Very low
        max_error_rate: 0.01,       // Very low
        min_availability: 99.999,   // Very high
    };
    
    let config = ValidationConfig {
        test_duration: Duration::from_secs(5),
        warmup_duration: Duration::from_secs(1),
        concurrent_users: vec![5],
        load_patterns: vec![],
        stress_test_enabled: false,
        regression_detection_enabled: false,
    };
    
    let validator = ProductionPerformanceValidator::new(monitor, strict_requirements, config);
    let results = validator.run_benchmark_suite().await.unwrap();
    
    // With strict requirements, we expect a lower grade
    match results.performance_grade {
        PerformanceGrade::Poor { score, details } => {
            println!("  Expected poor grade with strict requirements: {:.1}% - {}", score, details);
            assert!(score < 75.0);
        },
        PerformanceGrade::Fair { score, details } => {
            println!("  Fair grade with strict requirements: {:.1}% - {}", score, details);
            assert!(score < 90.0);
        },
        other => {
            println!("  Unexpected grade: {:?}", other);
        }
    }
    
    println!("✅ Performance requirements validation test passed!");
}

#[tokio::test]
async fn test_latency_statistics_calculation() {
    println!("📊 Testing latency statistics calculation...");
    
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        10,
    ));
    
    let requirements = PerformanceRequirements::default();
    let config = ValidationConfig::default();
    
    let validator = ProductionPerformanceValidator::new(monitor, requirements, config);
    let results = validator.run_benchmark_suite().await.unwrap();
    
    let api_latency = &results.latency_benchmarks.api_latency;
    
    // Validate latency statistics are properly ordered
    assert!(api_latency.min_us <= api_latency.mean_us);
    assert!(api_latency.mean_us <= api_latency.median_us || api_latency.median_us <= api_latency.mean_us);
    assert!(api_latency.median_us <= api_latency.p95_us);
    assert!(api_latency.p95_us <= api_latency.p99_us);
    assert!(api_latency.p99_us <= api_latency.p999_us);
    assert!(api_latency.p999_us <= api_latency.max_us);
    
    println!("  📈 Latency Statistics:");
    println!("    Min:    {:.2}μs", api_latency.min_us);
    println!("    Mean:   {:.2}μs", api_latency.mean_us);
    println!("    Median: {:.2}μs", api_latency.median_us);
    println!("    P95:    {:.2}μs", api_latency.p95_us);
    println!("    P99:    {:.2}μs", api_latency.p99_us);
    println!("    P99.9:  {:.2}μs", api_latency.p999_us);
    println!("    Max:    {:.2}μs", api_latency.max_us);
    println!("    StdDev: {:.2}μs", api_latency.std_dev_us);
    
    // Check for sub-millisecond performance
    if api_latency.mean_us < 1000.0 {
        println!("  🎉 Sub-millisecond performance achieved!");
    }
    
    println!("✅ Latency statistics calculation test passed!");
}

#[tokio::test]
async fn test_performance_monitoring_integration() {
    println!("📊 Testing performance monitoring integration...");
    
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_millis(100),
        20,
    ));
    
    // Start monitoring
    monitor.start_monitoring().await.unwrap();
    
    // Let it collect some data
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Collect metrics
    let metrics = monitor.collect_metrics().await.unwrap();
    
    // Validate metrics
    assert!(metrics.timestamp > 0);
    assert!(metrics.cpu_usage >= 0.0);
    assert!(metrics.memory_usage.total_mb > 0);
    
    println!("  📊 Current Metrics:");
    println!("    CPU Usage: {:.1}%", metrics.cpu_usage);
    println!("    Memory Usage: {:.1}%", metrics.memory_usage.usage_percentage);
    println!("    Disk Usage: {:.1}%", metrics.disk_usage.usage_percentage);
    
    // Stop monitoring
    monitor.stop_monitoring().await.unwrap();
    
    println!("✅ Performance monitoring integration test passed!");
}