use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;
use chrono::Utc;

use RustAutoDevOps::testing::{
    ProductionTestSuite, ProductionTestSuiteInterface, TestSuiteConfig,
    TestResults, TestCategory, CategoryResults, FailedTestDetail,
    PerformanceTestResults, SecurityTestResults, QualityGateStatus,
    CoverageReport, FileCoverageDetail, UncoveredLine, CoverageTrend, TrendDirection
};

#[tokio::test]
async fn test_production_test_suite_creation() {
    let config = TestSuiteConfig {
        minimum_coverage_threshold: 85.0,
        test_timeout: Duration::from_secs(300),
        parallel_test_limit: 4,
        retry_failed_tests: true,
        max_retries: 2,
        generate_html_reports: true,
        fail_fast: false,
        test_data_cleanup: true,
    };

    let test_suite = ProductionTestSuite::new(config.clone());
    assert_eq!(test_suite.config.minimum_coverage_threshold, 85.0);
    assert_eq!(test_suite.config.parallel_test_limit, 4);
    assert!(test_suite.config.retry_failed_tests);
}

#[tokio::test]
async fn test_quality_gate_evaluation_success() {
    let test_suite = ProductionTestSuite::new(TestSuiteConfig::default());
    
    let test_results = create_successful_test_results();
    let quality_status = test_suite.evaluate_quality_gates(&test_results).await.unwrap();
    
    assert!(quality_status.passed);
    assert!(quality_status.coverage_gate_passed);
    assert!(quality_status.performance_gate_passed);
    assert!(quality_status.security_gate_passed);
    assert!(quality_status.test_success_rate_passed);
    assert!(quality_status.failed_gates.is_empty());
}

#[tokio::test]
async fn test_quality_gate_evaluation_failure() {
    let test_suite = ProductionTestSuite::new(TestSuiteConfig::default());
    
    let test_results = create_failing_test_results();
    let quality_status = test_suite.evaluate_quality_gates(&test_results).await.unwrap();
    
    assert!(!quality_status.passed);
    assert!(!quality_status.coverage_gate_passed);
    assert!(!quality_status.security_gate_passed);
    assert!(!quality_status.failed_gates.is_empty());
}

#[tokio::test]
async fn test_success_rate_calculation() {
    let test_suite = ProductionTestSuite::new(TestSuiteConfig::default());
    
    let test_results = TestResults {
        test_run_id: Uuid::new_v4(),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_tests: 100,
        passed_tests: 95,
        failed_tests: 5,
        skipped_tests: 0,
        coverage_percentage: 92.0,
        test_categories: HashMap::new(),
        performance_metrics: PerformanceTestResults {
            benchmark_results: HashMap::new(),
            load_test_results: HashMap::new(),
            regression_detected: false,
            performance_score: 85.0,
        },
        security_scan_results: SecurityTestResults {
            vulnerability_scan_results: Vec::new(),
            penetration_test_results: Vec::new(),
            compliance_check_results: HashMap::new(),
            security_score: 85.0,
        },
        quality_gate_status: QualityGateStatus {
            passed: false,
            coverage_gate_passed: false,
            performance_gate_passed: false,
            security_gate_passed: false,
            test_success_rate_passed: false,
            failed_gates: Vec::new(),
        },
    };

    let success_rate = test_suite.calculate_success_rate(&test_results);
    assert_eq!(success_rate, 95.0);
}

#[tokio::test]
async fn test_success_rate_calculation_zero_tests() {
    let test_suite = ProductionTestSuite::new(TestSuiteConfig::default());
    
    let test_results = TestResults {
        test_run_id: Uuid::new_v4(),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_tests: 0,
        passed_tests: 0,
        failed_tests: 0,
        skipped_tests: 0,
        coverage_percentage: 0.0,
        test_categories: HashMap::new(),
        performance_metrics: PerformanceTestResults {
            benchmark_results: HashMap::new(),
            load_test_results: HashMap::new(),
            regression_detected: false,
            performance_score: 0.0,
        },
        security_scan_results: SecurityTestResults {
            vulnerability_scan_results: Vec::new(),
            penetration_test_results: Vec::new(),
            compliance_check_results: HashMap::new(),
            security_score: 0.0,
        },
        quality_gate_status: QualityGateStatus {
            passed: false,
            coverage_gate_passed: false,
            performance_gate_passed: false,
            security_gate_passed: false,
            test_success_rate_passed: false,
            failed_gates: Vec::new(),
        },
    };

    let success_rate = test_suite.calculate_success_rate(&test_results);
    assert_eq!(success_rate, 100.0);
}

#[test]
fn test_test_suite_config_default() {
    let config = TestSuiteConfig::default();
    
    assert_eq!(config.minimum_coverage_threshold, 90.0);
    assert_eq!(config.parallel_test_limit, num_cpus::get());
    assert!(config.retry_failed_tests);
    assert_eq!(config.max_retries, 3);
    assert!(config.generate_html_reports);
    assert!(!config.fail_fast);
    assert!(config.test_data_cleanup);
}

#[test]
fn test_category_results_creation() {
    let failed_detail = FailedTestDetail {
        test_name: "test_example".to_string(),
        error_message: "Assertion failed".to_string(),
        stack_trace: Some("Stack trace here".to_string()),
        retry_count: 2,
        category: TestCategory::Unit,
    };

    let category_results = CategoryResults {
        total: 10,
        passed: 8,
        failed: 2,
        skipped: 0,
        duration: Duration::from_secs(30),
        failed_test_details: vec![failed_detail],
    };

    assert_eq!(category_results.total, 10);
    assert_eq!(category_results.passed, 8);
    assert_eq!(category_results.failed, 2);
    assert_eq!(category_results.failed_test_details.len(), 1);
}

#[test]
fn test_coverage_report_creation() {
    let mut file_details = HashMap::new();
    file_details.insert("src/main.rs".to_string(), FileCoverageDetail {
        file_path: "src/main.rs".to_string(),
        line_coverage_percentage: 85.0,
        branch_coverage_percentage: 80.0,
        total_lines: 100,
        covered_lines: 85,
        total_branches: 20,
        covered_branches: 16,
    });

    let uncovered_lines = vec![
        UncoveredLine {
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            line_content: "println!(\"Debug line\");".to_string(),
        }
    ];

    let coverage_trend = CoverageTrend {
        previous_coverage: 80.0,
        current_coverage: 85.0,
        trend_direction: TrendDirection::Improving,
        change_percentage: 6.25,
    };

    let coverage_report = CoverageReport {
        overall_coverage_percentage: 85.0,
        line_coverage: 85.0,
        branch_coverage: 80.0,
        function_coverage: 90.0,
        file_coverage_details: file_details,
        uncovered_lines,
        coverage_trend: Some(coverage_trend),
    };

    assert_eq!(coverage_report.overall_coverage_percentage, 85.0);
    assert_eq!(coverage_report.file_coverage_details.len(), 1);
    assert_eq!(coverage_report.uncovered_lines.len(), 1);
    assert!(coverage_report.coverage_trend.is_some());
}

// Helper functions for creating test data

fn create_successful_test_results() -> TestResults {
    let mut test_categories = HashMap::new();
    
    test_categories.insert(TestCategory::Unit, CategoryResults {
        total: 50,
        passed: 50,
        failed: 0,
        skipped: 0,
        duration: Duration::from_secs(30),
        failed_test_details: Vec::new(),
    });

    test_categories.insert(TestCategory::Integration, CategoryResults {
        total: 20,
        passed: 20,
        failed: 0,
        skipped: 0,
        duration: Duration::from_secs(60),
        failed_test_details: Vec::new(),
    });

    TestResults {
        test_run_id: Uuid::new_v4(),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_tests: 70,
        passed_tests: 70,
        failed_tests: 0,
        skipped_tests: 0,
        coverage_percentage: 95.0,
        test_categories,
        performance_metrics: PerformanceTestResults {
            benchmark_results: HashMap::new(),
            load_test_results: HashMap::new(),
            regression_detected: false,
            performance_score: 90.0,
        },
        security_scan_results: SecurityTestResults {
            vulnerability_scan_results: Vec::new(),
            penetration_test_results: Vec::new(),
            compliance_check_results: HashMap::new(),
            security_score: 95.0,
        },
        quality_gate_status: QualityGateStatus {
            passed: false,
            coverage_gate_passed: false,
            performance_gate_passed: false,
            security_gate_passed: false,
            test_success_rate_passed: false,
            failed_gates: Vec::new(),
        },
    }
}

fn create_failing_test_results() -> TestResults {
    let mut test_categories = HashMap::new();
    
    let failed_detail = FailedTestDetail {
        test_name: "test_security_validation".to_string(),
        error_message: "Security validation failed".to_string(),
        stack_trace: Some("Security error stack trace".to_string()),
        retry_count: 1,
        category: TestCategory::Security,
    };

    test_categories.insert(TestCategory::Unit, CategoryResults {
        total: 50,
        passed: 45,
        failed: 5,
        skipped: 0,
        duration: Duration::from_secs(30),
        failed_test_details: vec![failed_detail.clone()],
    });

    test_categories.insert(TestCategory::Security, CategoryResults {
        total: 10,
        passed: 8,
        failed: 2,
        skipped: 0,
        duration: Duration::from_secs(45),
        failed_test_details: vec![failed_detail],
    });

    TestResults {
        test_run_id: Uuid::new_v4(),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_tests: 60,
        passed_tests: 53,
        failed_tests: 7,
        skipped_tests: 0,
        coverage_percentage: 75.0, // Below threshold
        test_categories,
        performance_metrics: PerformanceTestResults {
            benchmark_results: HashMap::new(),
            load_test_results: HashMap::new(),
            regression_detected: true, // Performance regression
            performance_score: 60.0,
        },
        security_scan_results: SecurityTestResults {
            vulnerability_scan_results: Vec::new(),
            penetration_test_results: Vec::new(),
            compliance_check_results: HashMap::new(),
            security_score: 70.0, // Below threshold
        },
        quality_gate_status: QualityGateStatus {
            passed: false,
            coverage_gate_passed: false,
            performance_gate_passed: false,
            security_gate_passed: false,
            test_success_rate_passed: false,
            failed_gates: Vec::new(),
        },
    }
}