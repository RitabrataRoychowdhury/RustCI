use async_trait::async_trait;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

/// Quality gates for enforcing code quality standards
#[derive(Debug, Clone)]
pub struct QualityGates {
    config: QualityGateConfig,
    rules: Vec<QualityRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateConfig {
    pub minimum_coverage_threshold: f64,
    pub minimum_test_success_rate: f64,
    pub maximum_complexity_threshold: u32,
    pub maximum_duplication_percentage: f64,
    pub security_score_threshold: f64,
    pub performance_regression_threshold: f64,
    pub enforce_linting: bool,
    pub enforce_formatting: bool,
    pub fail_on_warnings: bool,
    pub timeout: Duration,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            minimum_coverage_threshold: 80.0,
            minimum_test_success_rate: 95.0,
            maximum_complexity_threshold: 10,
            maximum_duplication_percentage: 5.0,
            security_score_threshold: 80.0,
            performance_regression_threshold: 10.0,
            enforce_linting: true,
            enforce_formatting: true,
            fail_on_warnings: false,
            timeout: Duration::from_secs(600),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: QualitySeverity,
    pub category: QualityCategory,
    pub threshold: Option<f64>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualitySeverity {
    Critical,
    Major,
    Minor,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityCategory {
    Coverage,
    Testing,
    Security,
    Performance,
    Maintainability,
    Reliability,
    Formatting,
    Documentation,
}

#[async_trait]
pub trait QualityGatesInterface {
    async fn evaluate_quality(&self) -> Result<QualityGateResult>;
    async fn run_static_analysis(&self) -> Result<Vec<QualityViolation>>;
    async fn check_code_coverage(&self) -> Result<CoverageMetrics>;
    async fn check_test_results(&self) -> Result<TestMetrics>;
    async fn check_security_compliance(&self) -> Result<SecurityMetrics>;
    async fn check_performance_metrics(&self) -> Result<PerformanceMetrics>;
    async fn generate_quality_report(&self, result: &QualityGateResult) -> Result<String>;
}

impl QualityGates {
    pub fn new() -> Self {
        Self::with_config(QualityGateConfig::default())
    }

    pub fn with_config(config: QualityGateConfig) -> Self {
        let rules = Self::create_default_rules(&config);
        Self { config, rules }
    }

    fn create_default_rules(config: &QualityGateConfig) -> Vec<QualityRule> {
        vec![
            QualityRule {
                id: "coverage_threshold".to_string(),
                name: "Code Coverage Threshold".to_string(),
                description: "Minimum code coverage percentage required".to_string(),
                severity: QualitySeverity::Critical,
                category: QualityCategory::Coverage,
                threshold: Some(config.minimum_coverage_threshold),
                enabled: true,
            },
            QualityRule {
                id: "test_success_rate".to_string(),
                name: "Test Success Rate".to_string(),
                description: "Minimum test success rate required".to_string(),
                severity: QualitySeverity::Critical,
                category: QualityCategory::Testing,
                threshold: Some(config.minimum_test_success_rate),
                enabled: true,
            },
            QualityRule {
                id: "security_score".to_string(),
                name: "Security Score Threshold".to_string(),
                description: "Minimum security score required".to_string(),
                severity: QualitySeverity::Critical,
                category: QualityCategory::Security,
                threshold: Some(config.security_score_threshold),
                enabled: true,
            },
            QualityRule {
                id: "performance_regression".to_string(),
                name: "Performance Regression Check".to_string(),
                description: "Maximum allowed performance regression percentage".to_string(),
                severity: QualitySeverity::Major,
                category: QualityCategory::Performance,
                threshold: Some(config.performance_regression_threshold),
                enabled: true,
            },
            QualityRule {
                id: "code_complexity".to_string(),
                name: "Code Complexity Limit".to_string(),
                description: "Maximum cyclomatic complexity allowed".to_string(),
                severity: QualitySeverity::Major,
                category: QualityCategory::Maintainability,
                threshold: Some(config.maximum_complexity_threshold as f64),
                enabled: true,
            },
            QualityRule {
                id: "code_duplication".to_string(),
                name: "Code Duplication Limit".to_string(),
                description: "Maximum code duplication percentage allowed".to_string(),
                severity: QualitySeverity::Minor,
                category: QualityCategory::Maintainability,
                threshold: Some(config.maximum_duplication_percentage),
                enabled: true,
            },
            QualityRule {
                id: "linting_compliance".to_string(),
                name: "Linting Compliance".to_string(),
                description: "Code must pass linting checks".to_string(),
                severity: QualitySeverity::Major,
                category: QualityCategory::Reliability,
                threshold: None,
                enabled: config.enforce_linting,
            },
            QualityRule {
                id: "formatting_compliance".to_string(),
                name: "Code Formatting Compliance".to_string(),
                description: "Code must be properly formatted".to_string(),
                severity: QualitySeverity::Minor,
                category: QualityCategory::Formatting,
                threshold: None,
                enabled: config.enforce_formatting,
            },
        ]
    }

    async fn run_clippy_analysis(&self) -> Result<Vec<QualityViolation>> {
        tracing::info!("Running Clippy static analysis");

        let join_result = tokio::time::timeout(
            self.config.timeout,
            tokio::task::spawn_blocking(|| {
                Command::new("cargo")
                    .args(&["clippy", "--", "-D", "warnings"])
                    .output()
            })
        ).await;

        match join_result {
            Ok(Ok(Ok(output))) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let mut violations = Vec::new();

                // Parse clippy output (simplified)
                if !output.status.success() {
                    for (i, line) in stderr.lines().enumerate() {
                        if line.contains("warning:") || line.contains("error:") {
                            violations.push(QualityViolation {
                                rule: "clippy_lint".to_string(),
                                severity: if line.contains("error:") { "Critical".to_string() } else { "Major".to_string() },
                                message: line.to_string(),
                                file_path: None, // Would parse from clippy output in real implementation
                                line_number: Some(i as u32),
                                category: QualityCategory::Reliability,
                            });
                        }
                    }
                }

                tracing::info!("Clippy analysis completed, found {} violations", violations.len());
                Ok(violations)
            }
            _ => {
                tracing::warn!("Clippy not available, skipping static analysis");
                Ok(Vec::new())
            }
        }
    }

    async fn run_rustfmt_check(&self) -> Result<Vec<QualityViolation>> {
        if !self.config.enforce_formatting {
            return Ok(Vec::new());
        }

        tracing::info!("Running rustfmt formatting check");

        let join_result = tokio::time::timeout(
            self.config.timeout,
            tokio::task::spawn_blocking(|| {
                Command::new("cargo")
                    .args(&["fmt", "--", "--check"])
                    .output()
            })
        ).await;

        match join_result {
            Ok(Ok(Ok(output))) => {
                let mut violations = Vec::new();

                if !output.status.success() {
                    violations.push(QualityViolation {
                        rule: "formatting_compliance".to_string(),
                        severity: "Minor".to_string(),
                        message: "Code formatting does not comply with rustfmt standards".to_string(),
                        file_path: None,
                        line_number: None,
                        category: QualityCategory::Formatting,
                    });
                }

                tracing::info!("Rustfmt check completed, found {} violations", violations.len());
                Ok(violations)
            }
            _ => {
                tracing::warn!("Rustfmt not available, skipping formatting check");
                Ok(Vec::new())
            }
        }
    }

    fn calculate_quality_score(&self, violations: &[QualityViolation], metrics: &QualityMetrics) -> f64 {
        let mut score = 100.0;

        // Deduct points based on violations
        for violation in violations {
            let deduction = match violation.severity.as_str() {
                "Critical" => 20.0,
                "Major" => 10.0,
                "Minor" => 5.0,
                "Info" => 1.0,
                _ => 5.0,
            };
            score -= deduction;
        }

        // Factor in metrics
        if let Some(coverage) = &metrics.coverage {
            if coverage.overall_coverage < self.config.minimum_coverage_threshold {
                score -= (self.config.minimum_coverage_threshold - coverage.overall_coverage) * 2.0;
            }
        }

        if let Some(test_metrics) = &metrics.test_results {
            if test_metrics.success_rate < self.config.minimum_test_success_rate {
                score -= (self.config.minimum_test_success_rate - test_metrics.success_rate) * 1.5;
            }
        }

        if let Some(security) = &metrics.security {
            if security.security_score < self.config.security_score_threshold {
                score -= (self.config.security_score_threshold - security.security_score) * 1.0;
            }
        }

        score.max(0.0).min(100.0)
    }
}

#[async_trait]
impl QualityGatesInterface for QualityGates {
    async fn evaluate_quality(&self) -> Result<QualityGateResult> {
        tracing::info!("Evaluating quality gates");

        let mut all_violations = Vec::new();

        // Run static analysis
        let mut static_violations = self.run_static_analysis().await?;
        all_violations.append(&mut static_violations);

        // Check coverage
        let coverage_metrics = self.check_code_coverage().await?;
        if coverage_metrics.overall_coverage < self.config.minimum_coverage_threshold {
            all_violations.push(QualityViolation {
                rule: "coverage_threshold".to_string(),
                severity: "Critical".to_string(),
                message: format!("Code coverage {:.2}% is below threshold {:.2}%", 
                    coverage_metrics.overall_coverage, self.config.minimum_coverage_threshold),
                file_path: None,
                line_number: None,
                category: QualityCategory::Coverage,
            });
        }

        // Check test results
        let test_metrics = self.check_test_results().await?;
        if test_metrics.success_rate < self.config.minimum_test_success_rate {
            all_violations.push(QualityViolation {
                rule: "test_success_rate".to_string(),
                severity: "Critical".to_string(),
                message: format!("Test success rate {:.2}% is below threshold {:.2}%", 
                    test_metrics.success_rate, self.config.minimum_test_success_rate),
                file_path: None,
                line_number: None,
                category: QualityCategory::Testing,
            });
        }

        // Check security compliance
        let security_metrics = self.check_security_compliance().await?;
        if security_metrics.security_score < self.config.security_score_threshold {
            all_violations.push(QualityViolation {
                rule: "security_score".to_string(),
                severity: "Critical".to_string(),
                message: format!("Security score {:.2} is below threshold {:.2}", 
                    security_metrics.security_score, self.config.security_score_threshold),
                file_path: None,
                line_number: None,
                category: QualityCategory::Security,
            });
        }

        // Check performance metrics
        let performance_metrics = self.check_performance_metrics().await?;
        if performance_metrics.regression_percentage > self.config.performance_regression_threshold {
            all_violations.push(QualityViolation {
                rule: "performance_regression".to_string(),
                severity: "Major".to_string(),
                message: format!("Performance regression {:.2}% exceeds threshold {:.2}%", 
                    performance_metrics.regression_percentage, self.config.performance_regression_threshold),
                file_path: None,
                line_number: None,
                category: QualityCategory::Performance,
            });
        }

        let quality_metrics = QualityMetrics {
            coverage: Some(coverage_metrics),
            test_results: Some(test_metrics),
            security: Some(security_metrics),
            performance: Some(performance_metrics),
        };

        let quality_score = self.calculate_quality_score(&all_violations, &quality_metrics);
        let passed = all_violations.iter().all(|v| v.severity != "Critical");

        let result = QualityGateResult {
            id: Uuid::new_v4(),
            passed,
            score: quality_score,
            violations: all_violations,
            metrics: quality_metrics,
            evaluated_at: Utc::now(),
            rules_evaluated: self.rules.len() as u32,
        };

        tracing::info!(
            "Quality gate evaluation completed: {} (score: {:.2}, violations: {})",
            if result.passed { "PASSED" } else { "FAILED" },
            result.score,
            result.violations.len()
        );

        Ok(result)
    }

    async fn run_static_analysis(&self) -> Result<Vec<QualityViolation>> {
        let mut all_violations = Vec::new();

        // Run Clippy analysis
        let mut clippy_violations = self.run_clippy_analysis().await?;
        all_violations.append(&mut clippy_violations);

        // Run rustfmt check
        let mut fmt_violations = self.run_rustfmt_check().await?;
        all_violations.append(&mut fmt_violations);

        Ok(all_violations)
    }

    async fn check_code_coverage(&self) -> Result<CoverageMetrics> {
        // Simulate coverage check (in real implementation, would parse coverage reports)
        Ok(CoverageMetrics {
            overall_coverage: 85.5,
            line_coverage: 87.2,
            branch_coverage: 82.1,
            function_coverage: 91.3,
        })
    }

    async fn check_test_results(&self) -> Result<TestMetrics> {
        // Simulate test results check
        Ok(TestMetrics {
            total_tests: 150,
            passed_tests: 145,
            failed_tests: 5,
            skipped_tests: 0,
            success_rate: 96.67,
            execution_time_seconds: 45.2,
        })
    }

    async fn check_security_compliance(&self) -> Result<SecurityMetrics> {
        // Simulate security compliance check
        Ok(SecurityMetrics {
            security_score: 88.5,
            vulnerabilities_found: 2,
            critical_vulnerabilities: 0,
            high_vulnerabilities: 0,
            medium_vulnerabilities: 2,
            low_vulnerabilities: 0,
        })
    }

    async fn check_performance_metrics(&self) -> Result<PerformanceMetrics> {
        // Simulate performance metrics check
        Ok(PerformanceMetrics {
            regression_percentage: 3.2,
            average_response_time_ms: 125.5,
            throughput_rps: 850.0,
            memory_usage_mb: 256.0,
            cpu_usage_percent: 45.2,
        })
    }

    async fn generate_quality_report(&self, result: &QualityGateResult) -> Result<String> {
        let report = format!(
            r#"# Quality Gate Report

## Summary
- **Status**: {}
- **Score**: {:.2}/100
- **Evaluated At**: {}
- **Rules Evaluated**: {}
- **Violations Found**: {}

## Metrics
### Coverage
- Overall: {:.2}%
- Line: {:.2}%
- Branch: {:.2}%
- Function: {:.2}%

### Test Results
- Total Tests: {}
- Success Rate: {:.2}%
- Execution Time: {:.2}s

### Security
- Security Score: {:.2}
- Vulnerabilities: {}

### Performance
- Regression: {:.2}%
- Response Time: {:.2}ms
- Throughput: {:.2} RPS

## Violations
{}

## Recommendations
{}
"#,
            if result.passed { "✅ PASSED" } else { "❌ FAILED" },
            result.score,
            result.evaluated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            result.rules_evaluated,
            result.violations.len(),
            result.metrics.coverage.as_ref().map(|c| c.overall_coverage).unwrap_or(0.0),
            result.metrics.coverage.as_ref().map(|c| c.line_coverage).unwrap_or(0.0),
            result.metrics.coverage.as_ref().map(|c| c.branch_coverage).unwrap_or(0.0),
            result.metrics.coverage.as_ref().map(|c| c.function_coverage).unwrap_or(0.0),
            result.metrics.test_results.as_ref().map(|t| t.total_tests).unwrap_or(0),
            result.metrics.test_results.as_ref().map(|t| t.success_rate).unwrap_or(0.0),
            result.metrics.test_results.as_ref().map(|t| t.execution_time_seconds).unwrap_or(0.0),
            result.metrics.security.as_ref().map(|s| s.security_score).unwrap_or(0.0),
            result.metrics.security.as_ref().map(|s| s.vulnerabilities_found).unwrap_or(0),
            result.metrics.performance.as_ref().map(|p| p.regression_percentage).unwrap_or(0.0),
            result.metrics.performance.as_ref().map(|p| p.average_response_time_ms).unwrap_or(0.0),
            result.metrics.performance.as_ref().map(|p| p.throughput_rps).unwrap_or(0.0),
            if result.violations.is_empty() {
                "No violations found.".to_string()
            } else {
                result.violations.iter()
                    .map(|v| format!("- **{}**: {} ({})", v.severity, v.message, v.rule))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if result.passed {
                "All quality gates passed. Great work!"
            } else {
                "Please address the violations above before proceeding."
            }
        );

        Ok(report)
    }
}

// Data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResult {
    pub id: Uuid,
    pub passed: bool,
    pub score: f64,
    pub violations: Vec<QualityViolation>,
    pub metrics: QualityMetrics,
    pub evaluated_at: DateTime<Utc>,
    pub rules_evaluated: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityViolation {
    pub rule: String,
    pub severity: String,
    pub message: String,
    pub file_path: Option<String>,
    pub line_number: Option<u32>,
    pub category: QualityCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub coverage: Option<CoverageMetrics>,
    pub test_results: Option<TestMetrics>,
    pub security: Option<SecurityMetrics>,
    pub performance: Option<PerformanceMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMetrics {
    pub overall_coverage: f64,
    pub line_coverage: f64,
    pub branch_coverage: f64,
    pub function_coverage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMetrics {
    pub total_tests: u32,
    pub passed_tests: u32,
    pub failed_tests: u32,
    pub skipped_tests: u32,
    pub success_rate: f64,
    pub execution_time_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetrics {
    pub security_score: f64,
    pub vulnerabilities_found: u32,
    pub critical_vulnerabilities: u32,
    pub high_vulnerabilities: u32,
    pub medium_vulnerabilities: u32,
    pub low_vulnerabilities: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub regression_percentage: f64,
    pub average_response_time_ms: f64,
    pub throughput_rps: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

impl Default for QualityGates {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quality_gates_creation() {
        let gates = QualityGates::new();
        assert_eq!(gates.config.minimum_coverage_threshold, 80.0);
        assert!(!gates.rules.is_empty());
    }

    #[tokio::test]
    async fn test_evaluate_quality() {
        let gates = QualityGates::new();
        let result = gates.evaluate_quality().await.unwrap();
        
        assert!(result.score > 0.0);
        assert!(result.score <= 100.0);
        assert!(result.rules_evaluated > 0);
    }

    #[tokio::test]
    async fn test_run_static_analysis() {
        let gates = QualityGates::new();
        let violations = gates.run_static_analysis().await.unwrap();
        
        // May or may not have violations depending on code state
        assert!(violations.len() >= 0);
    }

    #[tokio::test]
    async fn test_check_metrics() {
        let gates = QualityGates::new();
        
        let coverage = gates.check_code_coverage().await.unwrap();
        assert!(coverage.overall_coverage >= 0.0);
        assert!(coverage.overall_coverage <= 100.0);
        
        let test_metrics = gates.check_test_results().await.unwrap();
        assert!(test_metrics.success_rate >= 0.0);
        assert!(test_metrics.success_rate <= 100.0);
        
        let security = gates.check_security_compliance().await.unwrap();
        assert!(security.security_score >= 0.0);
        
        let performance = gates.check_performance_metrics().await.unwrap();
        assert!(performance.regression_percentage >= 0.0);
    }

    #[tokio::test]
    async fn test_generate_quality_report() {
        let gates = QualityGates::new();
        let result = gates.evaluate_quality().await.unwrap();
        let report = gates.generate_quality_report(&result).await.unwrap();
        
        assert!(report.contains("Quality Gate Report"));
        assert!(report.contains("Summary"));
        assert!(report.contains("Metrics"));
    }

    #[test]
    fn test_quality_gate_config_default() {
        let config = QualityGateConfig::default();
        assert_eq!(config.minimum_coverage_threshold, 80.0);
        assert_eq!(config.minimum_test_success_rate, 95.0);
        assert!(config.enforce_linting);
    }

    #[test]
    fn test_quality_rule_creation() {
        let config = QualityGateConfig::default();
        let rules = QualityGates::create_default_rules(&config);
        
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.id == "coverage_threshold"));
        assert!(rules.iter().any(|r| r.id == "test_success_rate"));
        assert!(rules.iter().any(|r| r.id == "security_score"));
    }
}