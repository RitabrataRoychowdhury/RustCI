use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use anyhow::Result;

use crate::testing::{TestResults, TestCategory, CategoryResults, FailedTestDetail};

/// Test result aggregator for collecting and analyzing test results across categories
#[derive(Debug, Clone)]
pub struct TestResultAggregator {
    aggregation_rules: AggregationRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationRules {
    pub weight_by_category: HashMap<TestCategory, f64>,
    pub failure_impact_multiplier: f64,
    pub performance_weight: f64,
    pub security_weight: f64,
    pub calculate_trends: bool,
}

impl Default for AggregationRules {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert(TestCategory::Unit, 1.0);
        weights.insert(TestCategory::Integration, 2.0);
        weights.insert(TestCategory::Performance, 1.5);
        weights.insert(TestCategory::Security, 3.0);
        weights.insert(TestCategory::EndToEnd, 2.5);
        weights.insert(TestCategory::Smoke, 1.0);
        weights.insert(TestCategory::Regression, 2.0);

        Self {
            weight_by_category: weights,
            failure_impact_multiplier: 2.0,
            performance_weight: 0.3,
            security_weight: 0.4,
            calculate_trends: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub overall_score: f64,
    pub weighted_success_rate: f64,
    pub category_scores: HashMap<TestCategory, CategoryScore>,
    pub failure_analysis: FailureAnalysis,
    pub performance_impact: f64,
    pub security_impact: f64,
    pub trend_analysis: Option<TrendAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub category: TestCategory,
    pub success_rate: f64,
    pub weighted_score: f64,
    pub total_tests: u32,
    pub failed_tests: u32,
    pub average_duration: Duration,
    pub impact_level: ImpactLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactLevel {
    Critical,
    High,
    Medium,
    Low,
    Minimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    pub total_failures: u32,
    pub failure_patterns: Vec<FailurePattern>,
    pub most_common_errors: Vec<ErrorFrequency>,
    pub failure_distribution: HashMap<TestCategory, u32>,
    pub retry_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub pattern_id: String,
    pub description: String,
    pub frequency: u32,
    pub affected_categories: Vec<TestCategory>,
    pub suggested_remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorFrequency {
    pub error_message: String,
    pub count: u32,
    pub percentage: f64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub success_rate_trend: TrendDirection,
    pub performance_trend: TrendDirection,
    pub failure_trend: TrendDirection,
    pub category_trends: HashMap<TestCategory, TrendDirection>,
    pub improvement_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Declining,
    Stable,
    Volatile,
}

pub trait TestResultAggregatorInterface {
    fn aggregate_results(&self, test_results: &mut TestResults);
    fn calculate_overall_score(&self, test_results: &TestResults) -> f64;
    fn analyze_failures(&self, test_results: &TestResults) -> FailureAnalysis;
    fn calculate_category_scores(&self, test_results: &TestResults) -> HashMap<TestCategory, CategoryScore>;
    fn generate_improvement_suggestions(&self, test_results: &TestResults) -> Vec<String>;
}

impl TestResultAggregator {
    pub fn new() -> Self {
        Self {
            aggregation_rules: AggregationRules::default(),
        }
    }

    pub fn with_rules(rules: AggregationRules) -> Self {
        Self {
            aggregation_rules: rules,
        }
    }

    fn calculate_success_rate(&self, passed: u32, total: u32) -> f64 {
        if total == 0 {
            return 100.0;
        }
        (passed as f64 / total as f64) * 100.0
    }

    fn determine_impact_level(&self, category: &TestCategory, success_rate: f64) -> ImpactLevel {
        let base_threshold = match category {
            TestCategory::Security => 95.0,
            TestCategory::Integration => 90.0,
            TestCategory::EndToEnd => 85.0,
            TestCategory::Performance => 80.0,
            TestCategory::Unit => 95.0,
            TestCategory::Smoke => 100.0,
            TestCategory::Regression => 98.0,
        };

        if success_rate >= base_threshold {
            ImpactLevel::Minimal
        } else if success_rate >= base_threshold - 10.0 {
            ImpactLevel::Low
        } else if success_rate >= base_threshold - 20.0 {
            ImpactLevel::Medium
        } else if success_rate >= base_threshold - 30.0 {
            ImpactLevel::High
        } else {
            ImpactLevel::Critical
        }
    }

    fn detect_failure_patterns(&self, test_results: &TestResults) -> Vec<FailurePattern> {
        let mut patterns = Vec::new();
        let mut error_groups: HashMap<String, Vec<&FailedTestDetail>> = HashMap::new();

        // Group failures by similar error messages
        for category_results in test_results.test_categories.values() {
            for failure in &category_results.failed_test_details {
                let error_key = self.extract_error_pattern(&failure.error_message);
                error_groups.entry(error_key).or_insert_with(Vec::new).push(failure);
            }
        }

        // Create patterns from groups with multiple occurrences
        for (error_pattern, failures) in error_groups {
            if failures.len() > 1 {
                let affected_categories: Vec<TestCategory> = failures
                    .iter()
                    .map(|f| f.category.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                let pattern = FailurePattern {
                    pattern_id: format!("pattern_{}", uuid::Uuid::new_v4().to_string()[0..8].to_string()),
                    description: format!("Multiple tests failing with similar error: {}", error_pattern),
                    frequency: failures.len() as u32,
                    affected_categories,
                    suggested_remediation: self.suggest_remediation(&error_pattern),
                };

                patterns.push(pattern);
            }
        }

        patterns
    }

    fn extract_error_pattern(&self, error_message: &str) -> String {
        // Simplified pattern extraction - in practice, you'd use more sophisticated NLP
        let words: Vec<&str> = error_message.split_whitespace().collect();
        if words.len() >= 3 {
            format!("{} {} {}", words[0], words[1], words[2])
        } else {
            error_message.to_string()
        }
    }

    fn suggest_remediation(&self, error_pattern: &str) -> String {
        // Simple pattern matching for common error types
        if error_pattern.contains("connection") || error_pattern.contains("timeout") {
            "Check network connectivity and increase timeout values".to_string()
        } else if error_pattern.contains("assertion") || error_pattern.contains("expected") {
            "Review test assertions and expected values".to_string()
        } else if error_pattern.contains("permission") || error_pattern.contains("access") {
            "Check file permissions and access rights".to_string()
        } else if error_pattern.contains("memory") || error_pattern.contains("allocation") {
            "Investigate memory usage and potential leaks".to_string()
        } else {
            "Review error logs and stack traces for specific remediation steps".to_string()
        }
    }

    fn calculate_error_frequencies(&self, test_results: &TestResults) -> Vec<ErrorFrequency> {
        let mut error_counts: HashMap<String, (u32, DateTime<Utc>, DateTime<Utc>)> = HashMap::new();
        let mut total_errors = 0u32;

        for category_results in test_results.test_categories.values() {
            for failure in &category_results.failed_test_details {
                total_errors += 1;
                let error_key = failure.error_message.clone();
                let now = Utc::now();
                
                error_counts
                    .entry(error_key)
                    .and_modify(|(count, first_seen, last_seen)| {
                        *count += 1;
                        *last_seen = now;
                    })
                    .or_insert((1, now, now));
            }
        }

        let mut frequencies: Vec<ErrorFrequency> = error_counts
            .into_iter()
            .map(|(error_message, (count, first_seen, last_seen))| {
                let percentage = if total_errors > 0 {
                    (count as f64 / total_errors as f64) * 100.0
                } else {
                    0.0
                };

                ErrorFrequency {
                    error_message,
                    count,
                    percentage,
                    first_seen,
                    last_seen,
                }
            })
            .collect();

        // Sort by frequency (descending)
        frequencies.sort_by(|a, b| b.count.cmp(&a.count));
        frequencies
    }

    fn calculate_retry_success_rate(&self, test_results: &TestResults) -> f64 {
        let mut total_retries = 0u32;
        let mut successful_retries = 0u32;

        for category_results in test_results.test_categories.values() {
            for failure in &category_results.failed_test_details {
                if failure.retry_count > 0 {
                    total_retries += failure.retry_count;
                    // If the test ultimately passed after retries, count as successful
                    // This is a simplified assumption - in practice you'd track this more precisely
                }
            }
        }

        if total_retries > 0 {
            (successful_retries as f64 / total_retries as f64) * 100.0
        } else {
            0.0
        }
    }
}

impl TestResultAggregatorInterface for TestResultAggregator {
    fn aggregate_results(&self, test_results: &mut TestResults) {
        // Calculate totals across all categories
        let mut total_tests = 0u32;
        let mut passed_tests = 0u32;
        let mut failed_tests = 0u32;
        let mut skipped_tests = 0u32;

        for category_results in test_results.test_categories.values() {
            total_tests += category_results.total;
            passed_tests += category_results.passed;
            failed_tests += category_results.failed;
            skipped_tests += category_results.skipped;
        }

        test_results.total_tests = total_tests;
        test_results.passed_tests = passed_tests;
        test_results.failed_tests = failed_tests;
        test_results.skipped_tests = skipped_tests;

        tracing::info!(
            "Aggregated test results: {} total, {} passed, {} failed, {} skipped",
            total_tests, passed_tests, failed_tests, skipped_tests
        );
    }

    fn calculate_overall_score(&self, test_results: &TestResults) -> f64 {
        let mut weighted_score = 0.0;
        let mut total_weight = 0.0;

        for (category, results) in &test_results.test_categories {
            let success_rate = self.calculate_success_rate(results.passed, results.total);
            let weight = self.aggregation_rules.weight_by_category
                .get(category)
                .copied()
                .unwrap_or(1.0);

            weighted_score += success_rate * weight;
            total_weight += weight;
        }

        // Apply performance and security adjustments
        let base_score = if total_weight > 0.0 {
            weighted_score / total_weight
        } else {
            0.0
        };

        let performance_adjustment = test_results.performance_metrics.performance_score 
            * self.aggregation_rules.performance_weight;
        
        let security_adjustment = test_results.security_scan_results.security_score 
            * self.aggregation_rules.security_weight;

        let final_score = base_score + performance_adjustment + security_adjustment;
        
        // Ensure score is between 0 and 100
        final_score.min(100.0).max(0.0)
    }

    fn analyze_failures(&self, test_results: &TestResults) -> FailureAnalysis {
        let total_failures = test_results.failed_tests;
        let failure_patterns = self.detect_failure_patterns(test_results);
        let most_common_errors = self.calculate_error_frequencies(test_results);
        
        let mut failure_distribution = HashMap::new();
        for (category, results) in &test_results.test_categories {
            failure_distribution.insert(category.clone(), results.failed);
        }

        let retry_success_rate = self.calculate_retry_success_rate(test_results);

        FailureAnalysis {
            total_failures,
            failure_patterns,
            most_common_errors,
            failure_distribution,
            retry_success_rate,
        }
    }

    fn calculate_category_scores(&self, test_results: &TestResults) -> HashMap<TestCategory, CategoryScore> {
        let mut category_scores = HashMap::new();

        for (category, results) in &test_results.test_categories {
            let success_rate = self.calculate_success_rate(results.passed, results.total);
            let weight = self.aggregation_rules.weight_by_category
                .get(category)
                .copied()
                .unwrap_or(1.0);
            
            let weighted_score = success_rate * weight;
            let impact_level = self.determine_impact_level(category, success_rate);
            
            let average_duration = if results.total > 0 {
                results.duration / results.total
            } else {
                Duration::ZERO
            };

            let score = CategoryScore {
                category: category.clone(),
                success_rate,
                weighted_score,
                total_tests: results.total,
                failed_tests: results.failed,
                average_duration,
                impact_level,
            };

            category_scores.insert(category.clone(), score);
        }

        category_scores
    }

    fn generate_improvement_suggestions(&self, test_results: &TestResults) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Coverage-based suggestions
        if test_results.coverage_percentage < 90.0 {
            suggestions.push(format!(
                "Increase test coverage from {:.1}% to at least 90%",
                test_results.coverage_percentage
            ));
        }

        // Category-specific suggestions
        for (category, results) in &test_results.test_categories {
            let success_rate = self.calculate_success_rate(results.passed, results.total);
            
            match category {
                TestCategory::Security if success_rate < 95.0 => {
                    suggestions.push("Critical: Address security test failures immediately".to_string());
                }
                TestCategory::Integration if success_rate < 90.0 => {
                    suggestions.push("Review integration test failures - may indicate system integration issues".to_string());
                }
                TestCategory::Performance if success_rate < 80.0 => {
                    suggestions.push("Investigate performance regressions and optimize critical paths".to_string());
                }
                TestCategory::Unit if success_rate < 95.0 => {
                    suggestions.push("Fix unit test failures to ensure code quality".to_string());
                }
                _ => {}
            }
        }

        // Performance-based suggestions
        if test_results.performance_metrics.regression_detected {
            suggestions.push("Performance regression detected - review recent changes".to_string());
        }

        // Security-based suggestions
        if test_results.security_scan_results.security_score < 80.0 {
            suggestions.push("Address security vulnerabilities before deployment".to_string());
        }

        // Failure pattern suggestions
        let failure_analysis = self.analyze_failures(test_results);
        for pattern in &failure_analysis.failure_patterns {
            if pattern.frequency > 2 {
                suggestions.push(format!(
                    "Address recurring failure pattern: {} ({})",
                    pattern.description, pattern.suggested_remediation
                ));
            }
        }

        suggestions
    }
}

impl Default for TestResultAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_aggregator_creation() {
        let aggregator = TestResultAggregator::new();
        assert!(aggregator.aggregation_rules.weight_by_category.contains_key(&TestCategory::Security));
    }

    #[test]
    fn test_success_rate_calculation() {
        let aggregator = TestResultAggregator::new();
        assert_eq!(aggregator.calculate_success_rate(90, 100), 90.0);
        assert_eq!(aggregator.calculate_success_rate(0, 0), 100.0);
    }

    #[test]
    fn test_impact_level_determination() {
        let aggregator = TestResultAggregator::new();
        
        let impact = aggregator.determine_impact_level(&TestCategory::Security, 96.0);
        assert!(matches!(impact, ImpactLevel::Minimal));
        
        let impact = aggregator.determine_impact_level(&TestCategory::Security, 80.0);
        assert!(matches!(impact, ImpactLevel::High));
    }

    #[test]
    fn test_error_pattern_extraction() {
        let aggregator = TestResultAggregator::new();
        let pattern = aggregator.extract_error_pattern("connection timeout occurred during test execution");
        assert_eq!(pattern, "connection timeout occurred");
    }

    #[test]
    fn test_remediation_suggestions() {
        let aggregator = TestResultAggregator::new();
        
        let suggestion = aggregator.suggest_remediation("connection timeout");
        assert!(suggestion.contains("network connectivity"));
        
        let suggestion = aggregator.suggest_remediation("assertion failed");
        assert!(suggestion.contains("test assertions"));
    }
}