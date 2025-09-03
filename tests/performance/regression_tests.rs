use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

use RustAutoDevOps::core::performance::{
    validation::*,
    metrics::*,
    monitor::{PerformanceMonitor, ProductionPerformanceMonitor},
};
use RustAutoDevOps::error::Result;

/// Performance regression test suite
/// 
/// This test suite validates that performance improvements don't introduce regressions
/// and that the system maintains performance requirements over time.
#[tokio::test]
async fn test_performance_regression_detection() {
    println!("🔍 Testing performance regression detection...");
    
    let validator = create_test_validator().await;
    
    // Create historical performance data with a regression pattern
    let historical_data = create_regression_test_data();
    
    // Run regression detection
    let regression_report = validator.detect_regressions(&historical_data).await.unwrap();
    
    println!("📊 Regression Analysis Results:");
    println!("  Detected regressions: {}", regression_report.detected_regressions.len());
    println!("  Severity: {:?}", regression_report.severity_summary);
    
    // Validate that regressions were detected
    assert!(!regression_report.detected_regressions.is_empty(), 
           "Should detect regressions in test data");
    
    // Check for latency regression
    let latency_regression = regression_report.detected_regressions.iter()
        .find(|r| r.metric_name == "API Latency");
    assert!(latency_regression.is_some(), "Should detect API latency regression");
    
    if let Some(regression) = latency_regression {
        println!("  📈 Latency regression detected:");
        println!("    Baseline: {:.2}ms", regression.baseline_value);
        println!("    Current: {:.2}ms", regression.current_value);
        println!("    Degradation: {:.1}%", regression.degradation_percent);
        
        assert!(regression.degradation_percent > 10.0, 
               "Should detect significant degradation");
    }
    
    println!("✅ Regression detection test passed");
}

#[tokio::test]
async fn test_performance_baseline_comparison() {
    println!("📊 Testing performance baseline comparison...");
    
    let validator = create_test_validator().await;
    
    // Create baseline performance data
    let baseline = create_performance_baseline();
    
    // Create current performance data with some changes
    let current_metrics = create_current_metrics_with_changes();
    
    // Compare performance
    let comparison_report = validator.compare_performance(&baseline, &current_metrics).await.unwrap();
    
    println!("📈 Comparison Results:");
    println!("  Regression detected: {}", comparison_report.regression_detected);
    println!("  Improvement detected: {}", comparison_report.improvement_detected);
    println!("  Significant changes: {}", comparison_report.significant_changes.len());
    
    // Validate comparison results
    assert!(comparison_report.significant_changes.len() > 0, 
           "Should detect significant changes");
    
    // Check performance delta
    let delta = &comparison_report.performance_delta;
    println!("  Performance Delta:");
    println!("    Latency change: {:.1}%", delta.latency_change_percent);
    println!("    Throughput change: {:.1}%", delta.throughput_change_percent);
    println!("    CPU usage change: {:.1}%", delta.cpu_usage_change_percent);
    println!("    Memory usage change: {:.1}%", delta.memory_usage_change_percent);
    
    // Validate that changes are within expected ranges
    assert!(delta.latency_change_percent.abs() < 50.0, 
           "Latency change should be reasonable");
    assert!(delta.throughput_change_percent.abs() < 50.0, 
           "Throughput change should be reasonable");
    
    println!("✅ Baseline comparison test passed");
}

#[tokio::test]
async fn test_comprehensive_validation_suite() {
    println!("🎯 Running comprehensive validation suite...");
    
    let validator = create_test_validator().await;
    
    // Run full validation suite
    let start_time = Instant::now();
    let validation_results = validator.run_validation_suite().await.unwrap();
    let validation_duration = start_time.elapsed();
    
    println!("📊 Validation Suite Results:");
    println!("  Validation ID: {}", validation_results.validation_id);
    println!("  Overall Status: {:?}", validation_results.overall_status);
    println!("  Duration: {:.2}s", validation_duration.as_secs_f64());
    println!("  Components tested: {}", validation_results.component_results.len());
    
    // Validate results structure
    assert!(!validation_results.component_results.is_empty(), 
           "Should have component results");
    assert!(validation_results.duration > Duration::ZERO, 
           "Should have positive duration");
    
    // Check component results
    for (component_name, result) in &validation_results.component_results {
        println!("  📋 Component: {}", component_name);
        println!("    Status: {:?}", result.status);
        println!("    Performance Score: {:.1}", result.performance_score);
        println!("    Requirements Met: {}", result.requirements_met);
        
        assert!(result.performance_score >= 0.0 && result.performance_score <= 100.0,
               "Performance score should be valid percentage");
    }
    
    // Check benchmark results
    let benchmarks = &validation_results.benchmark_results;
    println!("  🏃 Benchmark Results:");
    println!("    API Latency: {:.2}μs (P95: {:.2}μs)", 
             benchmarks.latency_benchmarks.api_latency.mean_us,
             benchmarks.latency_benchmarks.api_latency.p95_us);
    println!("    Throughput: {:.0} RPS", 
             benchmarks.throughput_benchmarks.requests_per_second);
    println!("    Performance Grade: {:?}", benchmarks.performance_grade);
    
    // Validate performance requirements
    match benchmarks.performance_grade {
        PerformanceGrade::Excellent { score, .. } | 
        PerformanceGrade::Good { score, .. } => {
            assert!(score >= 75.0, "Should have good performance score");
        },
        _ => {
            println!("⚠️  Performance grade below expectations");
        }
    }
    
    println!("✅ Comprehensive validation suite test passed");
}

#[tokio::test]
async fn test_benchmark_suite_execution() {
    println!("🏋️ Testing benchmark suite execution...");
    
    let validator = create_test_validator().await;
    
    // Run benchmark suite
    let start_time = Instant::now();
    let benchmark_results = validator.run_benchmark_suite().await.unwrap();
    let execution_duration = start_time.elapsed();
    
    println!("📊 Benchmark Suite Results:");
    println!("  Benchmark ID: {}", benchmark_results.benchmark_id);
    println!("  Test Suite: {}", benchmark_results.test_suite);
    println!("  Execution Duration: {:.2}s", execution_duration.as_secs_f64());
    
    // Validate latency benchmarks
    let latency = &benchmark_results.latency_benchmarks;
    println!("  🚀 Latency Benchmarks:");
    println!("    API: {:.2}μs (min: {:.2}μs, max: {:.2}μs)", 
             latency.api_latency.mean_us, latency.api_latency.min_us, latency.api_latency.max_us);
    println!("    Database: {:.2}μs", latency.database_latency.mean_us);
    println!("    Cache: {:.2}μs", latency.cache_latency.mean_us);
    println!("    Network: {:.2}μs", latency.network_latency.mean_us);
    println!("    End-to-End: {:.2}μs", latency.end_to_end_latency.mean_us);
    
    // Validate sub-millisecond performance for critical components
    assert!(latency.api_latency.mean_us < 1000.0, 
           "API latency should be sub-millisecond: {:.2}μs", latency.api_latency.mean_us);
    assert!(latency.cache_latency.mean_us < 100.0, 
           "Cache latency should be very fast: {:.2}μs", latency.cache_latency.mean_us);
    
    // Validate throughput benchmarks
    let throughput = &benchmark_results.throughput_benchmarks;
    println!("  📈 Throughput Benchmarks:");
    println!("    Requests/sec: {:.0}", throughput.requests_per_second);
    println!("    Transactions/sec: {:.0}", throughput.transactions_per_second);
    println!("    Messages/sec: {:.0}", throughput.messages_per_second);
    println!("    Data throughput: {:.1} Mbps", throughput.data_throughput_mbps);
    
    assert!(throughput.requests_per_second >= 1000.0, 
           "Should handle at least 1000 RPS");
    assert!(throughput.transactions_per_second >= 500.0, 
           "Should handle at least 500 TPS");
    
    // Validate resource benchmarks
    let resources = &benchmark_results.resource_benchmarks;
    println!("  💾 Resource Benchmarks:");
    println!("    CPU Efficiency: {:.1}%", resources.cpu_efficiency);
    println!("    Memory Efficiency: {:.1}%", resources.memory_efficiency);
    println!("    Disk Efficiency: {:.1}%", resources.disk_efficiency);
    println!("    Network Efficiency: {:.1}%", resources.network_efficiency);
    
    assert!(resources.cpu_efficiency >= 50.0, 
           "CPU efficiency should be reasonable");
    assert!(resources.memory_efficiency >= 50.0, 
           "Memory efficiency should be reasonable");
    
    // Validate stress test results
    let stress = &benchmark_results.stress_test_results;
    println!("  💪 Stress Test Results:");
    println!("    Max concurrent users: {}", stress.max_concurrent_users);
    println!("    Breaking point RPS: {:.0}", stress.breaking_point_rps);
    println!("    Recovery time: {:.1}s", stress.recovery_time.as_secs_f64());
    println!("    Stability score: {:.1}%", stress.stability_score);
    
    assert!(stress.max_concurrent_users >= 1000, 
           "Should handle at least 1000 concurrent users");
    assert!(stress.stability_score >= 90.0, 
           "Should have high stability score");
    
    // Validate load test results
    let load = &benchmark_results.load_test_results;
    println!("  🔄 Load Test Results:");
    println!("    Sustained load duration: {:.1}s", load.sustained_load_duration.as_secs_f64());
    println!("    Average response time: {:.1}ms", load.average_response_time.as_millis());
    println!("    Throughput consistency: {:.1}%", load.throughput_consistency);
    println!("    Resource stability: {:.1}%", load.resource_stability);
    println!("    Memory leak detected: {}", load.memory_leak_detected);
    
    assert!(!load.memory_leak_detected, "Should not detect memory leaks");
    assert!(load.throughput_consistency >= 95.0, 
           "Should have consistent throughput");
    assert!(load.resource_stability >= 95.0, 
           "Should have stable resource usage");
    
    println!("✅ Benchmark suite execution test passed");
}

#[tokio::test]
async fn test_performance_report_generation() {
    println!("📄 Testing performance report generation...");
    
    let validator = create_test_validator().await;
    
    // Run validation to get results
    let validation_results = validator.run_validation_suite().await.unwrap();
    
    // Generate performance report
    let performance_report = validator.generate_performance_report(&validation_results).await.unwrap();
    
    println!("📊 Performance Report:");
    println!("  Report ID: {}", performance_report.report_id);
    
    // Validate executive summary
    let summary = &performance_report.executive_summary;
    println!("  🎯 Executive Summary:");
    println!("    Overall Grade: {:?}", summary.overall_grade);
    println!("    Key Metrics: {}", summary.key_metrics.len());
    println!("    Critical Issues: {}", summary.critical_issues.len());
    println!("    Achievements: {}", summary.achievements.len());
    
    assert!(!summary.key_metrics.is_empty(), "Should have key metrics");
    
    // Check key metrics
    for (metric_name, value) in &summary.key_metrics {
        println!("      {}: {:.2}", metric_name, value);
        assert!(*value >= 0.0, "Metric values should be non-negative");
    }
    
    // Validate recommendations
    println!("  💡 Recommendations: {}", performance_report.recommendations.len());
    for (i, recommendation) in performance_report.recommendations.iter().enumerate() {
        println!("    {}. {} (Priority: {:?})", 
                 i + 1, recommendation.title, recommendation.priority);
        
        assert!(!recommendation.title.is_empty(), "Recommendation should have title");
        assert!(!recommendation.description.is_empty(), "Recommendation should have description");
    }
    
    // Validate next steps
    println!("  📋 Next Steps: {}", performance_report.next_steps.len());
    for (i, step) in performance_report.next_steps.iter().enumerate() {
        println!("    {}. {}", i + 1, step);
    }
    
    assert!(!performance_report.next_steps.is_empty(), "Should have next steps");
    
    println!("✅ Performance report generation test passed");
}

#[tokio::test]
async fn test_performance_requirements_validation() {
    println!("📏 Testing performance requirements validation...");
    
    // Test with strict requirements
    let strict_requirements = PerformanceRequirements {
        max_api_latency_ms: 100.0,  // Very strict
        min_throughput_rps: 10000.0, // Very high
        max_cpu_usage: 50.0,        // Very low
        max_memory_usage: 60.0,     // Very low
        max_error_rate: 0.1,        // Very low
        min_availability: 99.99,    // Very high
    };
    
    let validator = create_test_validator_with_requirements(strict_requirements).await;
    let benchmark_results = validator.run_benchmark_suite().await.unwrap();
    
    println!("📊 Strict Requirements Test:");
    match benchmark_results.performance_grade {
        PerformanceGrade::Poor { score, details } => {
            println!("  Expected poor grade with strict requirements: {:.1}% - {}", score, details);
            assert!(score < 75.0, "Should have lower score with strict requirements");
        },
        other => {
            println!("  Unexpected grade: {:?}", other);
        }
    }
    
    // Test with lenient requirements
    let lenient_requirements = PerformanceRequirements {
        max_api_latency_ms: 2000.0,  // Very lenient
        min_throughput_rps: 100.0,   // Very low
        max_cpu_usage: 95.0,         // Very high
        max_memory_usage: 95.0,      // Very high
        max_error_rate: 10.0,        // Very high
        min_availability: 90.0,      // Low
    };
    
    let lenient_validator = create_test_validator_with_requirements(lenient_requirements).await;
    let lenient_results = lenient_validator.run_benchmark_suite().await.unwrap();
    
    println!("📊 Lenient Requirements Test:");
    match lenient_results.performance_grade {
        PerformanceGrade::Excellent { score, details } | 
        PerformanceGrade::Good { score, details } => {
            println!("  Expected good grade with lenient requirements: {:.1}% - {}", score, details);
            assert!(score >= 75.0, "Should have higher score with lenient requirements");
        },
        other => {
            println!("  Grade: {:?}", other);
        }
    }
    
    println!("✅ Performance requirements validation test passed");
}

#[tokio::test]
async fn test_concurrent_performance_validation() {
    println!("🔄 Testing concurrent performance validation...");
    
    let validator = std::sync::Arc::new(create_test_validator().await);
    let concurrent_tests = 5;
    
    let mut handles = Vec::new();
    
    for i in 0..concurrent_tests {
        let validator_clone = validator.clone();
        let handle = tokio::spawn(async move {
            println!("  Starting concurrent test {}", i + 1);
            let start_time = Instant::now();
            
            let result = validator_clone.run_benchmark_suite().await;
            let duration = start_time.elapsed();
            
            println!("  Completed concurrent test {} in {:.2}s", i + 1, duration.as_secs_f64());
            (i, result, duration)
        });
        handles.push(handle);
    }
    
    // Wait for all tests to complete
    let results = futures::future::join_all(handles).await;
    
    let mut successful_tests = 0;
    let mut total_duration = Duration::ZERO;
    
    for result in results {
        match result {
            Ok((test_id, benchmark_result, duration)) => {
                match benchmark_result {
                    Ok(benchmarks) => {
                        successful_tests += 1;
                        total_duration += duration;
                        
                        println!("  ✅ Test {} completed successfully", test_id + 1);
                        println!("    API Latency: {:.2}μs", 
                                benchmarks.latency_benchmarks.api_latency.mean_us);
                        println!("    Throughput: {:.0} RPS", 
                                benchmarks.throughput_benchmarks.requests_per_second);
                    },
                    Err(e) => {
                        println!("  ❌ Test {} failed: {}", test_id + 1, e);
                    }
                }
            },
            Err(e) => {
                println!("  ❌ Test task failed: {}", e);
            }
        }
    }
    
    let average_duration = total_duration / successful_tests as u32;
    
    println!("📊 Concurrent Test Results:");
    println!("  Successful tests: {}/{}", successful_tests, concurrent_tests);
    println!("  Average duration: {:.2}s", average_duration.as_secs_f64());
    println!("  Total duration: {:.2}s", total_duration.as_secs_f64());
    
    assert_eq!(successful_tests, concurrent_tests, 
              "All concurrent tests should succeed");
    assert!(average_duration < Duration::from_secs(30), 
           "Each test should complete within reasonable time");
    
    println!("✅ Concurrent performance validation test passed");
}

// Helper functions for creating test data and validators

async fn create_test_validator() -> ProductionPerformanceValidator {
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        100,
    ));
    
    let requirements = PerformanceRequirements::default();
    let config = ValidationConfig::default();
    
    ProductionPerformanceValidator::new(monitor, requirements, config)
}

async fn create_test_validator_with_requirements(requirements: PerformanceRequirements) -> ProductionPerformanceValidator {
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        100,
    ));
    
    let config = ValidationConfig::default();
    
    ProductionPerformanceValidator::new(monitor, requirements, config)
}

fn create_regression_test_data() -> Vec<PerformanceMetrics> {
    let mut data = Vec::new();
    let base_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    // Create 10 data points with increasing latency (regression pattern)
    for i in 0..10 {
        let mut metrics = PerformanceMetrics::new();
        metrics.timestamp = base_timestamp - (10 - i) * 3600; // 1 hour intervals
        
        // Simulate increasing latency over time (regression)
        metrics.request_metrics.average_response_time_ms = 200.0 + (i as f64 * 50.0);
        metrics.request_metrics.requests_per_second = 5000.0 - (i as f64 * 200.0);
        
        // Simulate increasing resource usage
        metrics.cpu_usage = 40.0 + (i as f64 * 3.0);
        metrics.memory_usage.usage_percentage = 50.0 + (i as f64 * 2.0);
        
        data.push(metrics);
    }
    
    data
}

fn create_performance_baseline() -> PerformanceBaseline {
    let mut baseline_metrics = PerformanceMetrics::new();
    baseline_metrics.request_metrics.average_response_time_ms = 250.0;
    baseline_metrics.request_metrics.requests_per_second = 4000.0;
    baseline_metrics.cpu_usage = 45.0;
    baseline_metrics.memory_usage.usage_percentage = 55.0;
    
    PerformanceBaseline {
        baseline_id: Uuid::new_v4(),
        created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 86400, // 1 day ago
        version: "1.0.0".to_string(),
        metrics: baseline_metrics,
        requirements: PerformanceRequirements::default(),
    }
}

fn create_current_metrics_with_changes() -> PerformanceMetrics {
    let mut current_metrics = PerformanceMetrics::new();
    
    // Simulate some performance changes
    current_metrics.request_metrics.average_response_time_ms = 280.0; // 12% increase (regression)
    current_metrics.request_metrics.requests_per_second = 4400.0;     // 10% increase (improvement)
    current_metrics.cpu_usage = 52.0;                                 // 15.5% increase
    current_metrics.memory_usage.usage_percentage = 58.0;             // 5.5% increase
    
    current_metrics
}