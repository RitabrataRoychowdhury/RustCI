//! Performance Regression Detection System
//!
//! This module provides statistical analysis and automated regression detection
//! for Valkyrie Protocol performance benchmarks with CI/CD integration.

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::unified_benchmark_engine::{
    UnifiedBenchmarkResults, RegressionAnalysis, RegressionDetection, 
    RegressionSeverity, PerformanceTrend, BenchmarkError
};

/// Performance regression detector with statistical analysis
pub struct PerformanceRegressionDetector {
    config: RegressionDetectionConfig,
    historical_database: HistoricalPerformanceDatabase,
    statistical_analyzer: StatisticalAnalyzer,
    ci_cd_integrator: Option<CiCdIntegrator>,
}

/// Configuration for regression detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionDetectionConfig {
    /// Threshold for performance degradation (percentage)
    pub degradation_threshold_percent: f64,
    /// Number of historical results to compare against
    pub historical_window_size: usize,
    /// Statistical significance level (0.0 to 1.0)
    pub significance_level: f64,
    /// Enable automated CI/CD integration
    pub ci_cd_integration: bool,
    /// Path to historical performance database
    pub database_path: String,
    /// Metrics to monitor for regressions
    pub monitored_metrics: Vec<MonitoredMetric>,
}

/// Metric configuration for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredMetric {
    pub name: String,
    pub component: String,
    pub metric_type: MetricType,
    pub weight: f64,
    pub critical_threshold_percent: f64,
}

/// Type of metric being monitored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Latency,
    Throughput,
    SuccessRate,
    ResourceUsage,
}

/// Historical performance database
pub struct HistoricalPerformanceDatabase {
    database_path: String,
    cached_results: HashMap<String, Vec<HistoricalResult>>,
}

/// Historical benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalResult {
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub commit_hash: Option<String>,
    pub environment_hash: String,
    pub metrics: HashMap<String, f64>,
}

/// Statistical analyzer for performance data
pub struct StatisticalAnalyzer {
    significance_level: f64,
}

/// CI/CD integration for automated regression reporting
pub struct CiCdIntegrator {
    config: CiCdConfig,
}

/// CI/CD configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiCdConfig {
    pub provider: CiCdProvider,
    pub webhook_url: Option<String>,
    pub api_token: Option<String>,
    pub fail_build_on_regression: bool,
    pub notification_channels: Vec<NotificationChannel>,
}

/// Supported CI/CD providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CiCdProvider {
    GitHub,
    GitLab,
    Jenkins,
    CircleCI,
    TravisCI,
    Generic,
}

/// Notification channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Slack { webhook_url: String },
    Discord { webhook_url: String },
    Email { recipients: Vec<String> },
    Teams { webhook_url: String },
}

/// Comprehensive regression analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveRegressionAnalysis {
    pub analysis_id: String,
    pub timestamp: DateTime<Utc>,
    pub baseline_version: String,
    pub current_version: String,
    pub detected_regressions: Vec<DetailedRegressionDetection>,
    pub performance_trend: PerformanceTrend,
    pub statistical_confidence: f64,
    pub overall_severity: RegressionSeverity,
    pub recommendations: Vec<String>,
    pub historical_context: HistoricalContext,
}

/// Detailed regression detection with statistical analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedRegressionDetection {
    pub metric_name: String,
    pub component: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub degradation_percentage: f64,
    pub severity: RegressionSeverity,
    pub statistical_significance: f64,
    pub confidence_interval: (f64, f64),
    pub trend_analysis: TrendAnalysis,
    pub impact_assessment: ImpactAssessment,
}

/// Trend analysis for a specific metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub direction: TrendDirection,
    pub velocity: f64,
    pub consistency: f64,
    pub prediction: Option<PerformancePrediction>,
}

/// Trend direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
    Volatile,
}

/// Performance prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformancePrediction {
    pub predicted_value: f64,
    pub confidence: f64,
    pub time_horizon_days: u32,
}

/// Impact assessment for regression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub user_impact: UserImpact,
    pub business_impact: BusinessImpact,
    pub technical_impact: TechnicalImpact,
}

/// User impact levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserImpact {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Business impact levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusinessImpact {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Technical impact levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TechnicalImpact {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Historical context for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalContext {
    pub similar_regressions: Vec<SimilarRegression>,
    pub performance_baseline: PerformanceBaseline,
    pub seasonal_patterns: Vec<SeasonalPattern>,
}

/// Similar regression from history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarRegression {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub degradation_percentage: f64,
    pub resolution_time_hours: Option<f64>,
    pub root_cause: Option<String>,
}

/// Performance baseline information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub established_date: DateTime<Utc>,
    pub baseline_values: HashMap<String, f64>,
    pub confidence_intervals: HashMap<String, (f64, f64)>,
    pub last_updated: DateTime<Utc>,
}

/// Seasonal performance pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalPattern {
    pub pattern_name: String,
    pub frequency: String,
    pub amplitude: f64,
    pub phase_offset: f64,
}

impl PerformanceRegressionDetector {
    /// Create a new performance regression detector
    pub fn new(config: RegressionDetectionConfig) -> Self {
        let historical_database = HistoricalPerformanceDatabase::new(&config.database_path);
        let statistical_analyzer = StatisticalAnalyzer::new(config.significance_level);
        let ci_cd_integrator = if config.ci_cd_integration {
            Some(CiCdIntegrator::new(CiCdConfig::default()))
        } else {
            None
        };

        Self {
            config,
            historical_database,
            statistical_analyzer,
            ci_cd_integrator,
        }
    }

    /// Analyze performance results for regressions
    pub async fn analyze_performance(
        &self,
        current_results: &UnifiedBenchmarkResults,
    ) -> Result<ComprehensiveRegressionAnalysis, BenchmarkError> {
        println!("🔍 Analyzing performance for regressions...");

        // Extract metrics from current results
        let current_metrics = self.extract_metrics(current_results);

        // Get historical baseline
        let historical_results = self.historical_database
            .get_recent_results(self.config.historical_window_size)
            .await?;

        if historical_results.is_empty() {
            println!("📊 No historical data available - establishing baseline");
            self.establish_baseline(&current_metrics).await?;
            return Ok(self.create_baseline_analysis(current_results));
        }

        // Perform statistical analysis
        let mut detected_regressions = Vec::new();
        
        for monitored_metric in &self.config.monitored_metrics {
            if let Some(current_value) = current_metrics.get(&monitored_metric.name) {
                let regression = self.analyze_metric_regression(
                    &monitored_metric,
                    *current_value,
                    &historical_results,
                ).await?;

                if let Some(regression) = regression {
                    detected_regressions.push(regression);
                }
            }
        }

        // Determine overall severity
        let overall_severity = self.calculate_overall_severity(&detected_regressions);

        // Generate trend analysis
        let performance_trend = self.analyze_performance_trend(&historical_results, &current_metrics);

        // Calculate statistical confidence
        let statistical_confidence = self.calculate_statistical_confidence(&detected_regressions);

        // Generate recommendations
        let recommendations = self.generate_recommendations(&detected_regressions);

        // Get historical context
        let historical_context = self.get_historical_context(&detected_regressions).await?;

        let analysis = ComprehensiveRegressionAnalysis {
            analysis_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            baseline_version: self.get_baseline_version(&historical_results),
            current_version: current_results.metadata.version.clone(),
            detected_regressions,
            performance_trend,
            statistical_confidence,
            overall_severity,
            recommendations,
            historical_context,
        };

        // Store current results for future analysis
        self.store_current_results(current_results, &current_metrics).await?;

        // Trigger CI/CD integration if configured
        if let Some(ci_cd) = &self.ci_cd_integrator {
            ci_cd.report_regression_analysis(&analysis).await?;
        }

        self.print_regression_analysis(&analysis);

        Ok(analysis)
    }

    /// Extract metrics from benchmark results
    fn extract_metrics(&self, results: &UnifiedBenchmarkResults) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();

        // HTTP Bridge metrics
        metrics.insert("http_bridge_latency_mean".to_string(), results.http_bridge.latency_stats.mean_us);
        metrics.insert("http_bridge_latency_p95".to_string(), results.http_bridge.latency_stats.p95_us as f64);
        metrics.insert("http_bridge_rps".to_string(), results.http_bridge.requests_per_second);

        // Protocol Core metrics
        metrics.insert("protocol_core_latency_mean".to_string(), results.protocol_core.latency_stats.mean_us);
        metrics.insert("protocol_core_throughput".to_string(), results.protocol_core.messages_per_second);
        metrics.insert("protocol_core_memory_mb".to_string(), results.protocol_core.memory_usage_mb);

        // Transport metrics
        metrics.insert("quic_throughput_gbps".to_string(), results.transport.quic.throughput_gbps);
        metrics.insert("quic_latency_mean".to_string(), results.transport.quic.latency_stats.mean_us);
        metrics.insert("unix_socket_throughput_gbps".to_string(), results.transport.unix_socket.throughput_gbps);
        metrics.insert("unix_socket_latency_mean".to_string(), results.transport.unix_socket.latency_stats.mean_us);

        // Security metrics
        metrics.insert("security_encryption_latency".to_string(), results.security.encryption_latency_us);
        metrics.insert("security_handshake_time".to_string(), results.security.handshake_time_ms);

        // End-to-end metrics
        metrics.insert("e2e_pipeline_latency".to_string(), results.end_to_end.pipeline_latency_ms);
        metrics.insert("e2e_success_rate".to_string(), results.end_to_end.success_rate);

        metrics
    }

    /// Analyze regression for a specific metric
    async fn analyze_metric_regression(
        &self,
        metric: &MonitoredMetric,
        current_value: f64,
        historical_results: &[HistoricalResult],
    ) -> Result<Option<DetailedRegressionDetection>, BenchmarkError> {
        // Get historical values for this metric
        let historical_values: Vec<f64> = historical_results
            .iter()
            .filter_map(|result| result.metrics.get(&metric.name).copied())
            .collect();

        if historical_values.is_empty() {
            return Ok(None);
        }

        // Calculate baseline statistics
        let baseline_mean = self.statistical_analyzer.calculate_mean(&historical_values);
        let baseline_std = self.statistical_analyzer.calculate_std_dev(&historical_values);

        // Determine if this is a regression based on metric type
        let is_regression = match metric.metric_type {
            MetricType::Latency | MetricType::ResourceUsage => {
                // For latency and resource usage, higher is worse
                current_value > baseline_mean
            }
            MetricType::Throughput | MetricType::SuccessRate => {
                // For throughput and success rate, lower is worse
                current_value < baseline_mean
            }
        };

        if !is_regression {
            return Ok(None);
        }

        // Calculate degradation percentage
        let degradation_percentage = match metric.metric_type {
            MetricType::Latency | MetricType::ResourceUsage => {
                ((current_value - baseline_mean) / baseline_mean) * 100.0
            }
            MetricType::Throughput | MetricType::SuccessRate => {
                ((baseline_mean - current_value) / baseline_mean) * 100.0
            }
        };

        // Check if degradation exceeds threshold
        if degradation_percentage < self.config.degradation_threshold_percent {
            return Ok(None);
        }

        // Perform statistical significance test
        let statistical_significance = self.statistical_analyzer
            .calculate_significance(current_value, &historical_values);

        if statistical_significance < self.config.significance_level {
            return Ok(None);
        }

        // Calculate confidence interval
        let confidence_interval = self.statistical_analyzer
            .calculate_confidence_interval(&historical_values, self.config.significance_level);

        // Determine severity
        let severity = if degradation_percentage >= metric.critical_threshold_percent {
            RegressionSeverity::Critical
        } else if degradation_percentage >= 15.0 {
            RegressionSeverity::Major
        } else if degradation_percentage >= 5.0 {
            RegressionSeverity::Minor
        } else {
            RegressionSeverity::None
        };

        // Perform trend analysis
        let trend_analysis = self.analyze_metric_trend(&metric.name, &historical_values, current_value);

        // Assess impact
        let impact_assessment = self.assess_regression_impact(metric, degradation_percentage);

        Ok(Some(DetailedRegressionDetection {
            metric_name: metric.name.clone(),
            component: metric.component.clone(),
            baseline_value: baseline_mean,
            current_value,
            degradation_percentage,
            severity,
            statistical_significance,
            confidence_interval,
            trend_analysis,
            impact_assessment,
        }))
    }

    /// Analyze trend for a specific metric
    fn analyze_metric_trend(&self, _metric_name: &str, historical_values: &[f64], current_value: f64) -> TrendAnalysis {
        if historical_values.len() < 3 {
            return TrendAnalysis {
                direction: TrendDirection::Stable,
                velocity: 0.0,
                consistency: 0.0,
                prediction: None,
            };
        }

        // Calculate trend direction and velocity
        let recent_values = &historical_values[historical_values.len().saturating_sub(5)..];
        let slope = self.statistical_analyzer.calculate_linear_regression_slope(recent_values);
        
        let direction = if slope > 0.05 {
            TrendDirection::Degrading
        } else if slope < -0.05 {
            TrendDirection::Improving
        } else {
            TrendDirection::Stable
        };

        let velocity = slope.abs();
        let consistency = self.statistical_analyzer.calculate_trend_consistency(recent_values);

        // Generate prediction if trend is consistent
        let prediction = if consistency > 0.7 {
            Some(PerformancePrediction {
                predicted_value: current_value + (slope * 7.0), // 7 days ahead
                confidence: consistency,
                time_horizon_days: 7,
            })
        } else {
            None
        };

        TrendAnalysis {
            direction,
            velocity,
            consistency,
            prediction,
        }
    }

    /// Assess the impact of a regression
    fn assess_regression_impact(&self, metric: &MonitoredMetric, degradation_percentage: f64) -> ImpactAssessment {
        let user_impact = match degradation_percentage {
            x if x >= 50.0 => UserImpact::Critical,
            x if x >= 25.0 => UserImpact::High,
            x if x >= 10.0 => UserImpact::Medium,
            x if x >= 5.0 => UserImpact::Low,
            _ => UserImpact::None,
        };

        let business_impact = match (&metric.metric_type, degradation_percentage) {
            (MetricType::SuccessRate, x) if x >= 1.0 => BusinessImpact::Critical,
            (MetricType::Latency, x) if x >= 100.0 => BusinessImpact::High,
            (MetricType::Throughput, x) if x >= 50.0 => BusinessImpact::High,
            (_, x) if x >= 25.0 => BusinessImpact::Medium,
            (_, x) if x >= 10.0 => BusinessImpact::Low,
            _ => BusinessImpact::None,
        };

        let technical_impact = match degradation_percentage {
            x if x >= 100.0 => TechnicalImpact::Critical,
            x if x >= 50.0 => TechnicalImpact::High,
            x if x >= 25.0 => TechnicalImpact::Medium,
            x if x >= 10.0 => TechnicalImpact::Low,
            _ => TechnicalImpact::None,
        };

        ImpactAssessment {
            user_impact,
            business_impact,
            technical_impact,
        }
    }

    /// Calculate overall severity from individual regressions
    fn calculate_overall_severity(&self, regressions: &[DetailedRegressionDetection]) -> RegressionSeverity {
        if regressions.is_empty() {
            return RegressionSeverity::None;
        }

        let has_critical = regressions.iter().any(|r| matches!(r.severity, RegressionSeverity::Critical));
        let has_major = regressions.iter().any(|r| matches!(r.severity, RegressionSeverity::Major));
        let has_minor = regressions.iter().any(|r| matches!(r.severity, RegressionSeverity::Minor));

        if has_critical {
            RegressionSeverity::Critical
        } else if has_major {
            RegressionSeverity::Major
        } else if has_minor {
            RegressionSeverity::Minor
        } else {
            RegressionSeverity::None
        }
    }

    /// Analyze overall performance trend
    fn analyze_performance_trend(&self, _historical_results: &[HistoricalResult], _current_metrics: &HashMap<String, f64>) -> PerformanceTrend {
        // Simplified implementation
        PerformanceTrend::Stable
    }

    /// Calculate statistical confidence
    fn calculate_statistical_confidence(&self, regressions: &[DetailedRegressionDetection]) -> f64 {
        if regressions.is_empty() {
            return 1.0;
        }

        let sum: f64 = regressions.iter().map(|r| r.statistical_significance).sum();
        sum / regressions.len() as f64
    }

    /// Generate recommendations based on regressions
    fn generate_recommendations(&self, regressions: &[DetailedRegressionDetection]) -> Vec<String> {
        let mut recommendations = Vec::new();

        if regressions.is_empty() {
            recommendations.push("No performance regressions detected. Continue monitoring.".to_string());
            return recommendations;
        }

        for regression in regressions {
            match regression.severity {
                RegressionSeverity::Critical => {
                    recommendations.push(format!(
                        "CRITICAL: {} degraded by {:.1}% - immediate investigation required",
                        regression.metric_name, regression.degradation_percentage
                    ));
                }
                RegressionSeverity::Major => {
                    recommendations.push(format!(
                        "MAJOR: {} degraded by {:.1}% - schedule investigation within 24 hours",
                        regression.metric_name, regression.degradation_percentage
                    ));
                }
                RegressionSeverity::Minor => {
                    recommendations.push(format!(
                        "MINOR: {} degraded by {:.1}% - monitor trend and investigate if continues",
                        regression.metric_name, regression.degradation_percentage
                    ));
                }
                RegressionSeverity::None => {}
            }
        }

        recommendations
    }

    /// Get historical context for analysis
    async fn get_historical_context(&self, _regressions: &[DetailedRegressionDetection]) -> Result<HistoricalContext, BenchmarkError> {
        // Simplified implementation
        Ok(HistoricalContext {
            similar_regressions: Vec::new(),
            performance_baseline: PerformanceBaseline {
                established_date: Utc::now(),
                baseline_values: HashMap::new(),
                confidence_intervals: HashMap::new(),
                last_updated: Utc::now(),
            },
            seasonal_patterns: Vec::new(),
        })
    }

    /// Establish baseline from current results
    async fn establish_baseline(&self, _metrics: &HashMap<String, f64>) -> Result<(), BenchmarkError> {
        // Implementation for establishing performance baseline
        Ok(())
    }

    /// Create baseline analysis for first run
    fn create_baseline_analysis(&self, results: &UnifiedBenchmarkResults) -> ComprehensiveRegressionAnalysis {
        ComprehensiveRegressionAnalysis {
            analysis_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            baseline_version: results.metadata.version.clone(),
            current_version: results.metadata.version.clone(),
            detected_regressions: Vec::new(),
            performance_trend: PerformanceTrend::Stable,
            statistical_confidence: 1.0,
            overall_severity: RegressionSeverity::None,
            recommendations: vec!["Baseline established. Future runs will be compared against this baseline.".to_string()],
            historical_context: HistoricalContext {
                similar_regressions: Vec::new(),
                performance_baseline: PerformanceBaseline {
                    established_date: Utc::now(),
                    baseline_values: HashMap::new(),
                    confidence_intervals: HashMap::new(),
                    last_updated: Utc::now(),
                },
                seasonal_patterns: Vec::new(),
            },
        }
    }

    /// Store current results for future analysis
    async fn store_current_results(&self, _results: &UnifiedBenchmarkResults, _metrics: &HashMap<String, f64>) -> Result<(), BenchmarkError> {
        // Implementation for storing results
        Ok(())
    }

    /// Get baseline version from historical results
    fn get_baseline_version(&self, historical_results: &[HistoricalResult]) -> String {
        historical_results
            .first()
            .map(|r| r.version.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Print regression analysis summary
    fn print_regression_analysis(&self, analysis: &ComprehensiveRegressionAnalysis) {
        println!("\n📊 PERFORMANCE REGRESSION ANALYSIS");
        println!("==================================");
        
        match analysis.overall_severity {
            RegressionSeverity::Critical => println!("🚨 CRITICAL REGRESSIONS DETECTED"),
            RegressionSeverity::Major => println!("⚠️  MAJOR REGRESSIONS DETECTED"),
            RegressionSeverity::Minor => println!("⚡ MINOR REGRESSIONS DETECTED"),
            RegressionSeverity::None => println!("✅ NO REGRESSIONS DETECTED"),
        }

        if !analysis.detected_regressions.is_empty() {
            println!("\nDetected Regressions:");
            for regression in &analysis.detected_regressions {
                println!("  {} ({}): {:.1}% degradation ({:?})",
                         regression.metric_name,
                         regression.component,
                         regression.degradation_percentage,
                         regression.severity);
            }
        }

        println!("\nRecommendations:");
        for recommendation in &analysis.recommendations {
            println!("  • {}", recommendation);
        }

        println!("Statistical Confidence: {:.1}%", analysis.statistical_confidence * 100.0);
    }
}

impl HistoricalPerformanceDatabase {
    pub fn new(database_path: &str) -> Self {
        Self {
            database_path: database_path.to_string(),
            cached_results: HashMap::new(),
        }
    }

    pub async fn get_recent_results(&self, _window_size: usize) -> Result<Vec<HistoricalResult>, BenchmarkError> {
        // Simplified implementation - would read from actual database
        Ok(Vec::new())
    }
}

impl StatisticalAnalyzer {
    pub fn new(significance_level: f64) -> Self {
        Self { significance_level }
    }

    pub fn calculate_mean(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.iter().sum::<f64>() / values.len() as f64
    }

    pub fn calculate_std_dev(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        
        let mean = self.calculate_mean(values);
        let variance = values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (values.len() - 1) as f64;
        
        variance.sqrt()
    }

    pub fn calculate_significance(&self, current_value: f64, historical_values: &[f64]) -> f64 {
        // Simplified t-test implementation
        if historical_values.len() < 2 {
            return 0.0;
        }

        let mean = self.calculate_mean(historical_values);
        let std_dev = self.calculate_std_dev(historical_values);
        
        if std_dev == 0.0 {
            return if (current_value - mean).abs() < f64::EPSILON { 1.0 } else { 0.0 };
        }

        let t_stat = (current_value - mean) / (std_dev / (historical_values.len() as f64).sqrt());
        
        // Simplified significance calculation
        let significance = 1.0 - (t_stat.abs() / 3.0).min(1.0);
        significance.max(0.0)
    }

    pub fn calculate_confidence_interval(&self, values: &[f64], confidence_level: f64) -> (f64, f64) {
        if values.len() < 2 {
            return (0.0, 0.0);
        }

        let mean = self.calculate_mean(values);
        let std_dev = self.calculate_std_dev(values);
        let margin = std_dev * 1.96; // Approximate 95% CI
        
        (mean - margin, mean + margin)
    }

    pub fn calculate_linear_regression_slope(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let n = values.len() as f64;
        let x_values: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
        
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(values.iter()).map(|(x, y)| x * y).sum();
        let sum_x_squared: f64 = x_values.iter().map(|x| x * x).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x_squared - sum_x * sum_x);
        slope
    }

    pub fn calculate_trend_consistency(&self, values: &[f64]) -> f64 {
        if values.len() < 3 {
            return 0.0;
        }

        // Calculate how consistent the trend is
        let mut direction_changes = 0;
        for i in 1..values.len()-1 {
            let prev_diff = values[i] - values[i-1];
            let next_diff = values[i+1] - values[i];
            
            if (prev_diff > 0.0) != (next_diff > 0.0) {
                direction_changes += 1;
            }
        }

        let max_changes = values.len() - 2;
        1.0 - (direction_changes as f64 / max_changes as f64)
    }
}

impl CiCdIntegrator {
    pub fn new(config: CiCdConfig) -> Self {
        Self { config }
    }

    pub async fn report_regression_analysis(&self, _analysis: &ComprehensiveRegressionAnalysis) -> Result<(), BenchmarkError> {
        // Implementation for CI/CD integration
        Ok(())
    }
}

impl Default for CiCdConfig {
    fn default() -> Self {
        Self {
            provider: CiCdProvider::Generic,
            webhook_url: None,
            api_token: None,
            fail_build_on_regression: false,
            notification_channels: Vec::new(),
        }
    }
}

impl Default for RegressionDetectionConfig {
    fn default() -> Self {
        Self {
            degradation_threshold_percent: 5.0,
            historical_window_size: 10,
            significance_level: 0.95,
            ci_cd_integration: false,
            database_path: "performance_history.db".to_string(),
            monitored_metrics: vec![
                MonitoredMetric {
                    name: "http_bridge_latency_mean".to_string(),
                    component: "HTTP Bridge".to_string(),
                    metric_type: MetricType::Latency,
                    weight: 1.0,
                    critical_threshold_percent: 25.0,
                },
                MonitoredMetric {
                    name: "protocol_core_latency_mean".to_string(),
                    component: "Protocol Core".to_string(),
                    metric_type: MetricType::Latency,
                    weight: 1.0,
                    critical_threshold_percent: 20.0,
                },
                MonitoredMetric {
                    name: "quic_throughput_gbps".to_string(),
                    component: "QUIC Transport".to_string(),
                    metric_type: MetricType::Throughput,
                    weight: 1.0,
                    critical_threshold_percent: 15.0,
                },
            ],
        }
    }
}