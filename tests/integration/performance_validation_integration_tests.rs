use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

use RustAutoDevOps::core::performance::{
    validation::*,
    monitor::{PerformanceMonitor, ProductionPerformanceMonitor},
    metrics::*,
};
use RustAutoDevOps::error::Result;

/// Integration tests for performance validation system
/// 
/// These tests validate the complete performance validation workflow
/// including monitoring, benchmarking, regression detection, and reporting.

#[tokio::test]
async fn test_end_to_end_performance_validation_workflow() {
    println!("🔄 Testing end-to-end performance validation workflow...");
    
    // Create performance validator with production configuration
    let validator = create_production_validator().await;
    
    // Step 1: Run initial baseline validation
    println!("📊 Step 1: Running baseline validation...");
    let baseline_results = validator.run_validation_suite().await.unwrap();
    
    assert_eq!(baseline_results.overall_status, ValidationStatus::Passed);
    assert!(!baseline_results.component_results.is_empty());
    
    // Set baseline for future comparisons
    validator.set_baseline(
        "1.0.0".to_string(), 
        baseline_results.performance_metrics.clone()
    ).await.unwrap();
    
    // Step 2: Generate performance report
    println!("📄 Step 2: Generating performance report...");
    let performance_report = validator.generate_performance_report(&baseline_results).await.unwrap();
    
    assert!(!performance_report.executive_summary.key_metrics.is_empty());
    assert!(!performance_report.recommendations.is_empty());
    
    // Step 3: Run benchmark suite
    println!("🏋️ Step 3: Running comprehensive benchmark suite...");
    let benchmark_results = validator.run_benchmark_suite().await.unwrap();
    
    // Validate benchmark results
    validate_benchmark_results(&benchmark_results);
    
    // Step 4: Simulate performance changes and detect regressions
    println!("🔍 Step 4: Testing regression detection...");
    let historical_data = create_performance_trend_data();
    let regression_report = validator.detect_regressions(&historical_data).await.unwrap();
    
    // Should detect regressions in the simulated data
    assert!(!regression_report.detected_regressions.is_empty());
    
    println!("✅ End-to-end performance validation workflow completed successfully");
}

#[tokio::test]
async fn test_performance_requirements_compliance() {
    println!("📏 Testing performance requirements compliance...");
    
    // Define strict performance requirements
    let strict_requirements = PerformanceRequirements {
        max_api_latency_ms: 500.0,
        min_throughput_rps: 2000.0,
        max_cpu_usage: 70.0,
        max_memory_usage: 80.0,
        max_error_rate: 0.5,
        min_availability: 99.9,
    };
    
    let validator = create_validator_with_requirements(strict_requirements).await;
    
    // Run validation against strict requirements
    let validation_results = validator.run_validation_suite().await.unwrap();
    
    // Check that requirements are properly validated
    for (component_name, result) in &validation_results.component_results {
        println!("  Component: {} - Requirements Met: {}", 
                 component_name, result.requirements_met);
        
        // Validate that performance scores are calculated
        assert!(result.performance_score >= 0.0 && result.performance_score <= 100.0);
        
        // Check that recommendations are provided for failed components
        if result.status == ValidationStatus::Failed {
            assert!(!result.recommendations.is_empty());
        }
    }
    
    println!("✅ Performance requirements compliance test passed");
}

#[tokio::test]
async fn test_concurrent_validation_execution() {
    println!("🔄 Testing concurrent validation execution...");
    
    let validator = std::sync::Arc::new(create_production_validator().await);
    let concurrent_validations = 3;
    
    let mut handles = Vec::new();
    
    for i in 0..concurrent_validations {
        let validator_clone = validator.clone();
        let handle = tokio::spawn(async move {
            println!("  Starting validation {}", i + 1);
            let start_time = Instant::now();
            
            let result = validator_clone.run_benchmark_suite().await;
            let duration = start_time.elapsed();
            
            (i, result, duration)
        });
        handles.push(handle);
    }
    
    // Wait for all validations to complete
    let results = futures::future::join_all(handles).await;
    
    let mut successful_validations = 0;
    let mut total_duration = Duration::ZERO;
    
    for result in results {
        match result {
            Ok((validation_id, benchmark_result, duration)) => {
                match benchmark_result {
                    Ok(benchmarks) => {
                        successful_validations += 1;
                        total_duration += duration;
                        
                        println!("  ✅ Validation {} completed in {:.2}s", 
                                validation_id + 1, duration.as_secs_f64());
                        
                        // Validate results quality
                        validate_benchmark_results(&benchmarks);
                    },
                    Err(e) => {
                        println!("  ❌ Validation {} failed: {}", validation_id + 1, e);
                    }
                }
            },
            Err(e) => {
                println!("  ❌ Validation task failed: {}", e);
            }
        }
    }
    
    let average_duration = total_duration / successful_validations as u32;
    
    println!("📊 Concurrent Validation Results:");
    println!("  Successful validations: {}/{}", successful_validations, concurrent_validations);
    println!("  Average duration: {:.2}s", average_duration.as_secs_f64());
    
    assert_eq!(successful_validations, concurrent_validations);
    assert!(average_duration < Duration::from_secs(60));
    
    println!("✅ Concurrent validation execution test passed");
}

#[tokio::test]
async fn test_performance_monitoring_integration() {
    println!("📊 Testing performance monitoring integration...");
    
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_millis(100), // Fast collection for testing
        50,
    ));
    
    // Start monitoring
    monitor.start_monitoring().await.unwrap();
    
    // Let it collect some data
    sleep(Duration::from_secs(2)).await;
    
    // Collect metrics
    let metrics = monitor.collect_metrics().await.unwrap();
    
    // Validate metrics structure
    assert!(metrics.timestamp > 0);
    assert!(metrics.cpu_usage >= 0.0);
    assert!(metrics.memory_usage.total_mb > 0);
    assert!(metrics.request_metrics.total_requests >= 0);
    
    // Get historical data
    let historical_metrics = monitor.get_historical_metrics(Duration::from_secs(5)).await.unwrap();
    assert!(!historical_metrics.is_empty());
    
    // Detect performance issues
    let issues = monitor.detect_performance_issues().await.unwrap();
    println!("  Detected {} performance issues", issues.len());
    
    // Stop monitoring
    monitor.stop_monitoring().await.unwrap();
    
    println!("✅ Performance monitoring integration test passed");
}

#[tokio::test]
async fn test_stress_and_load_testing_integration() {
    println!("💪 Testing stress and load testing integration...");
    
    let config = ValidationConfig {
        test_duration: Duration::from_secs(30), // Shorter for testing
        warmup_duration: Duration::from_secs(5),
        concurrent_users: vec![10, 50, 100],
        load_patterns: vec![
            LoadPattern {
                name: "Light Load".to_string(),
                duration: Duration::from_secs(10),
                rps: 100.0,
                concurrent_users: 10,
            },
            LoadPattern {
                name: "Heavy Load".to_string(),
                duration: Duration::from_secs(20),
                rps: 1000.0,
                concurrent_users: 100,
            },
        ],
        stress_test_enabled: true,
        regression_detection_enabled: true,
    };
    
    let validator = create_validator_with_config(config).await;
    
    // Run benchmark suite with stress testing
    let benchmark_results = validator.run_benchmark_suite().await.unwrap();
    
    // Validate stress test results
    let stress_results = &benchmark_results.stress_test_results;
    assert!(stress_results.max_concurrent_users > 0);
    assert!(stress_results.breaking_point_rps > 0.0);
    assert!(stress_results.stability_score >= 0.0 && stress_results.stability_score <= 100.0);
    
    // Validate load test results
    let load_results = &benchmark_results.load_test_results;
    assert!(load_results.sustained_load_duration > Duration::ZERO);
    assert!(load_results.throughput_consistency >= 0.0 && load_results.throughput_consistency <= 100.0);
    assert!(load_results.resource_stability >= 0.0 && load_results.resource_stability <= 100.0);
    
    println!("  Stress Test Results:");
    println!("    Max Concurrent Users: {}", stress_results.max_concurrent_users);
    println!("    Breaking Point RPS: {:.0}", stress_results.breaking_point_rps);
    println!("    Stability Score: {:.1}%", stress_results.stability_score);
    
    println!("  Load Test Results:");
    println!("    Sustained Duration: {:.1}s", load_results.sustained_load_duration.as_secs_f64());
    println!("    Throughput Consistency: {:.1}%", load_results.throughput_consistency);
    println!("    Resource Stability: {:.1}%", load_results.resource_stability);
    
    println!("✅ Stress and load testing integration test passed");
}

#[tokio::test]
async fn test_performance_baseline_management() {
    println!("📊 Testing performance baseline management...");
    
    let validator = create_production_validator().await;
    
    // Create initial baseline
    let initial_metrics = create_sample_metrics("baseline");
    validator.set_baseline("1.0.0".to_string(), initial_metrics.clone()).await.unwrap();
    
    // Create current metrics with some changes
    let current_metrics = create_sample_metrics("current");
    
    // Create baseline for comparison
    let baseline = PerformanceBaseline {
        baseline_id: Uuid::new_v4(),
        created_at: initial_metrics.timestamp - 3600, // 1 hour ago
        version: "1.0.0".to_string(),
        metrics: initial_metrics,
        requirements: PerformanceRequirements::default(),
    };
    
    // Compare performance
    let comparison_report = validator.compare_performance(&baseline, &current_metrics).await.unwrap();
    
    // Validate comparison report
    assert!(comparison_report.baseline_timestamp < comparison_report.current_timestamp);
    assert!(!comparison_report.significant_changes.is_empty());
    
    println!("  Performance Comparison Results:");
    println!("    Regression Detected: {}", comparison_report.regression_detected);
    println!("    Improvement Detected: {}", comparison_report.improvement_detected);
    println!("    Significant Changes: {}", comparison_report.significant_changes.len());
    
    // Check performance delta
    let delta = &comparison_report.performance_delta;
    println!("    Latency Change: {:.1}%", delta.latency_change_percent);
    println!("    Throughput Change: {:.1}%", delta.throughput_change_percent);
    println!("    CPU Usage Change: {:.1}%", delta.cpu_usage_change_percent);
    
    // Validate significant changes
    for change in &comparison_report.significant_changes {
        println!("    Change: {} - {:.1}% ({:?})", 
                 change.metric_name, change.change_percent, change.change_type);
        
        assert!(!change.metric_name.is_empty());
        assert!(change.baseline_value >= 0.0);
        assert!(change.current_value >= 0.0);
    }
    
    println!("✅ Performance baseline management test passed");
}

#[tokio::test]
async fn test_performance_report_generation_and_export() {
    println!("📄 Testing performance report generation and export...");
    
    let validator = create_production_validator().await;
    
    // Run validation to get results
    let validation_results = validator.run_validation_suite().await.unwrap();
    
    // Generate comprehensive performance report
    let performance_report = validator.generate_performance_report(&validation_results).await.unwrap();
    
    // Validate report structure
    assert!(!performance_report.executive_summary.key_metrics.is_empty());
    assert!(!performance_report.recommendations.is_empty());
    assert!(!performance_report.next_steps.is_empty());
    
    // Validate executive summary
    let summary = &performance_report.executive_summary;
    match &summary.overall_grade {
        PerformanceGrade::Excellent { score, .. } |
        PerformanceGrade::Good { score, .. } |
        PerformanceGrade::Fair { score, .. } |
        PerformanceGrade::Poor { score, .. } => {
            assert!(*score >= 0.0 && *score <= 100.0);
        }
    }
    
    // Validate key metrics
    for (metric_name, value) in &summary.key_metrics {
        println!("  Key Metric: {} = {:.2}", metric_name, value);
        assert!(!metric_name.is_empty());
        assert!(*value >= 0.0);
    }
    
    // Validate recommendations
    for (i, recommendation) in performance_report.recommendations.iter().enumerate() {
        println!("  Recommendation {}: {} ({:?})", 
                 i + 1, recommendation.title, recommendation.priority);
        
        assert!(!recommendation.title.is_empty());
        assert!(!recommendation.description.is_empty());
        assert!(!recommendation.expected_impact.is_empty());
    }
    
    // Validate next steps
    for (i, step) in performance_report.next_steps.iter().enumerate() {
        println!("  Next Step {}: {}", i + 1, step);
        assert!(!step.is_empty());
    }
    
    println!("✅ Performance report generation and export test passed");
}

// Helper functions

async fn create_production_validator() -> ProductionPerformanceValidator {
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        100,
    ));
    
    let requirements = PerformanceRequirements::default();
    let config = ValidationConfig::default();
    
    ProductionPerformanceValidator::new(monitor, requirements, config)
}

async fn create_validator_with_requirements(requirements: PerformanceRequirements) -> ProductionPerformanceValidator {
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        100,
    ));
    
    let config = ValidationConfig::default();
    
    ProductionPerformanceValidator::new(monitor, requirements, config)
}

async fn create_validator_with_config(config: ValidationConfig) -> ProductionPerformanceValidator {
    let monitor = Box::new(ProductionPerformanceMonitor::new(
        Duration::from_secs(1),
        100,
    ));
    
    let requirements = PerformanceRequirements::default();
    
    ProductionPerformanceValidator::new(monitor, requirements, config)
}

fn validate_benchmark_results(results: &BenchmarkResults) {
    // Validate latency benchmarks
    let latency = &results.latency_benchmarks;
    assert!(latency.api_latency.mean_us > 0.0);
    assert!(latency.api_latency.p95_us >= latency.api_latency.mean_us);
    assert!(latency.api_latency.p99_us >= latency.api_latency.p95_us);
    assert!(latency.api_latency.max_us >= latency.api_latency.p99_us);
    
    // Validate throughput benchmarks
    let throughput = &results.throughput_benchmarks;
    assert!(throughput.requests_per_second > 0.0);
    assert!(throughput.transactions_per_second > 0.0);
    assert!(throughput.concurrent_users > 0);
    
    // Validate resource benchmarks
    let resources = &results.resource_benchmarks;
    assert!(resources.cpu_efficiency >= 0.0 && resources.cpu_efficiency <= 100.0);
    assert!(resources.memory_efficiency >= 0.0 && resources.memory_efficiency <= 100.0);
    
    // Validate stress test results
    let stress = &results.stress_test_results;
    assert!(stress.max_concurrent_users > 0);
    assert!(stress.breaking_point_rps > 0.0);
    assert!(stress.stability_score >= 0.0 && stress.stability_score <= 100.0);
    
    // Validate load test results
    let load = &results.load_test_results;
    assert!(load.sustained_load_duration > Duration::ZERO);
    assert!(load.throughput_consistency >= 0.0 && load.throughput_consistency <= 100.0);
    assert!(load.resource_stability >= 0.0 && load.resource_stability <= 100.0);
}

fn create_performance_trend_data() -> Vec<PerformanceMetrics> {
    let mut data = Vec::new();
    let base_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Create trend data showing performance degradation over time
    for i in 0..10 {
        let mut metrics = PerformanceMetrics::new();
        metrics.timestamp = base_timestamp - (10 - i) * 3600; // 1 hour intervals
        
        // Simulate degrading performance
        metrics.request_metrics.average_response_time_ms = 200.0 + (i as f64 * 30.0);
        metrics.request_metrics.requests_per_second = 5000.0 - (i as f64 * 100.0);
        metrics.cpu_usage = 40.0 + (i as f64 * 2.0);
        metrics.memory_usage.usage_percentage = 50.0 + (i as f64 * 1.5);
        
        data.push(metrics);
    }
    
    data
}

fn create_sample_metrics(variant: &str) -> PerformanceMetrics {
    let mut metrics = PerformanceMetrics::new();
    
    match variant {
        "baseline" => {
            metrics.request_metrics.average_response_time_ms = 250.0;
            metrics.request_metrics.requests_per_second = 4000.0;
            metrics.cpu_usage = 45.0;
            metrics.memory_usage.usage_percentage = 55.0;
        },
        "current" => {
            // Simulate some performance changes
            metrics.request_metrics.average_response_time_ms = 275.0; // 10% increase
            metrics.request_metrics.requests_per_second = 4200.0;     // 5% increase
            metrics.cpu_usage = 50.0;                                 // 11% increase
            metrics.memory_usage.usage_percentage = 58.0;             // 5.5% increase
        },
        _ => {
            // Default values
            metrics.request_metrics.average_response_time_ms = 300.0;
            metrics.request_metrics.requests_per_second = 3500.0;
            metrics.cpu_usage = 60.0;
            metrics.memory_usage.usage_percentage = 65.0;
        }
    }
    
    metrics
}