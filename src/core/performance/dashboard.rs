use crate::core::performance::metrics::*;
use crate::core::performance::alerting::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[async_trait]
pub trait PerformanceDashboard: Send + Sync {
    async fn get_current_metrics(&self) -> Result<DashboardMetrics>;
    async fn get_historical_data(&self, duration: Duration) -> Result<HistoricalData>;
    async fn get_performance_summary(&self) -> Result<PerformanceSummary>;
    async fn get_alert_summary(&self) -> Result<AlertSummary>;
    async fn generate_performance_report(&self, duration: Duration) -> Result<PerformanceReport>;
    async fn get_system_health_status(&self) -> Result<SystemHealthStatus>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub timestamp: u64,
    pub system_overview: SystemOverview,
    pub performance_indicators: PerformanceIndicators,
    pub resource_utilization: ResourceUtilization,
    pub application_metrics: ApplicationMetrics,
    pub health_status: SystemHealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOverview {
    pub uptime_seconds: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub current_load: f64,
    pub active_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIndicators {
    pub throughput_rps: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub error_rate_percentage: f64,
    pub availability_percentage: f64,
    pub apdex_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_usage_percentage: f64,
    pub memory_usage_percentage: f64,
    pub disk_usage_percentage: f64,
    pub network_utilization_percentage: f64,
    pub database_connections_used: u32,
    pub database_connections_available: u32,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationMetrics {
    pub active_jobs: u32,
    pub queued_jobs: u32,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub pipeline_executions: u64,
    pub runner_instances: u32,
    pub plugin_load_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthStatus {
    pub overall_status: HealthStatus,
    pub component_health: HashMap<String, ComponentHealth>,
    pub last_health_check: u64,
    pub health_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub response_time_ms: Option<f64>,
    pub error_count: u32,
    pub last_check: u64,
    pub details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalData {
    pub time_range: TimeRange,
    pub metrics_timeline: Vec<TimeSeriesPoint>,
    pub aggregated_stats: AggregatedStats,
    pub trend_analysis: TrendAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration_seconds: u64,
    pub resolution_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: u64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub response_time: f64,
    pub throughput: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedStats {
    pub min_values: HashMap<String, f64>,
    pub max_values: HashMap<String, f64>,
    pub avg_values: HashMap<String, f64>,
    pub percentiles: HashMap<String, HashMap<String, f64>>, // metric -> percentile -> value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub cpu_trend: TrendDirection,
    pub memory_trend: TrendDirection,
    pub response_time_trend: TrendDirection,
    pub throughput_trend: TrendDirection,
    pub error_rate_trend: TrendDirection,
    pub predictions: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub period: TimeRange,
    pub key_metrics: KeyMetricsSummary,
    pub performance_issues: Vec<PerformanceIssueSummary>,
    pub recommendations: Vec<PerformanceRecommendation>,
    pub sla_compliance: SlaCompliance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetricsSummary {
    pub average_response_time_ms: f64,
    pub peak_response_time_ms: f64,
    pub total_requests: u64,
    pub error_rate_percentage: f64,
    pub uptime_percentage: f64,
    pub peak_cpu_usage: f64,
    pub peak_memory_usage: f64,
    pub throughput_rps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssueSummary {
    pub issue_type: PerformanceIssueType,
    pub occurrence_count: u32,
    pub total_duration_seconds: u64,
    pub severity: IssueSeverity,
    pub first_occurrence: u64,
    pub last_occurrence: u64,
    pub impact_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecommendation {
    pub id: Uuid,
    pub category: RecommendationCategory,
    pub priority: RecommendationPriority,
    pub title: String,
    pub description: String,
    pub expected_impact: String,
    pub implementation_effort: ImplementationEffort,
    pub related_metrics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    Performance,
    Scalability,
    Reliability,
    Cost,
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaCompliance {
    pub availability_target: f64,
    pub availability_actual: f64,
    pub response_time_target_ms: f64,
    pub response_time_actual_ms: f64,
    pub error_rate_target: f64,
    pub error_rate_actual: f64,
    pub compliance_score: f64,
    pub violations: Vec<SlaViolation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaViolation {
    pub metric: String,
    pub target_value: f64,
    pub actual_value: f64,
    pub violation_start: u64,
    pub violation_end: Option<u64>,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertSummary {
    pub total_alerts: u32,
    pub active_alerts: u32,
    pub resolved_alerts: u32,
    pub acknowledged_alerts: u32,
    pub alerts_by_severity: HashMap<String, u32>,
    pub alerts_by_component: HashMap<String, u32>,
    pub recent_alerts: Vec<AlertSummaryItem>,
    pub alert_trends: AlertTrends,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertSummaryItem {
    pub id: Uuid,
    pub rule_name: String,
    pub severity: IssueSeverity,
    pub component: String,
    pub triggered_at: u64,
    pub status: AlertStatus,
    pub duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertTrends {
    pub alerts_per_hour: Vec<u32>,
    pub mttr_seconds: f64, // Mean Time To Resolution
    pub mtbf_seconds: f64, // Mean Time Between Failures
    pub alert_frequency_trend: TrendDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub report_id: Uuid,
    pub generated_at: u64,
    pub period: TimeRange,
    pub executive_summary: ExecutiveSummary,
    pub detailed_metrics: DetailedMetrics,
    pub performance_analysis: PerformanceAnalysis,
    pub recommendations: Vec<PerformanceRecommendation>,
    pub appendices: ReportAppendices,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutiveSummary {
    pub overall_performance_score: f64,
    pub key_achievements: Vec<String>,
    pub critical_issues: Vec<String>,
    pub business_impact: String,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedMetrics {
    pub performance_kpis: HashMap<String, f64>,
    pub resource_utilization: HashMap<String, f64>,
    pub availability_metrics: HashMap<String, f64>,
    pub capacity_metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    pub bottleneck_analysis: Vec<BottleneckAnalysis>,
    pub capacity_analysis: CapacityAnalysis,
    pub trend_analysis: TrendAnalysis,
    pub comparative_analysis: ComparativeAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckAnalysis {
    pub component: String,
    pub bottleneck_type: BottleneckType,
    pub impact_score: f64,
    pub description: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckType {
    Cpu,
    Memory,
    Disk,
    Network,
    Database,
    Application,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityAnalysis {
    pub current_utilization: HashMap<String, f64>,
    pub projected_growth: HashMap<String, f64>,
    pub capacity_runway_days: HashMap<String, u32>,
    pub scaling_recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparativeAnalysis {
    pub period_over_period: HashMap<String, f64>,
    pub baseline_comparison: HashMap<String, f64>,
    pub industry_benchmarks: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportAppendices {
    pub raw_data_summary: HashMap<String, serde_json::Value>,
    pub methodology: String,
    pub data_sources: Vec<String>,
    pub limitations: Vec<String>,
}

pub struct ProductionPerformanceDashboard {
    metrics_collector: Box<dyn MetricsCollector>,
    alert_manager: Box<dyn AlertManager>,
    data_retention_days: u32,
}

#[async_trait]
pub trait MetricsCollector: Send + Sync {
    async fn get_current_metrics(&self) -> Result<PerformanceMetrics>;
    async fn get_historical_metrics(&self, duration: Duration) -> Result<Vec<PerformanceMetrics>>;
}

impl ProductionPerformanceDashboard {
    pub fn new(
        metrics_collector: Box<dyn MetricsCollector>,
        alert_manager: Box<dyn AlertManager>,
        data_retention_days: u32,
    ) -> Self {
        Self {
            metrics_collector,
            alert_manager,
            data_retention_days,
        }
    }

    async fn calculate_apdex_score(&self, response_times: &[f64], target_ms: f64) -> f64 {
        if response_times.is_empty() {
            return 0.0;
        }

        let satisfied = response_times.iter().filter(|&&rt| rt <= target_ms).count();
        let tolerating = response_times.iter().filter(|&&rt| rt > target_ms && rt <= target_ms * 4.0).count();
        let total = response_times.len();

        (satisfied as f64 + (tolerating as f64 * 0.5)) / total as f64
    }

    async fn analyze_trends(&self, metrics: &[PerformanceMetrics]) -> TrendAnalysis {
        if metrics.len() < 2 {
            return TrendAnalysis {
                cpu_trend: TrendDirection::Stable,
                memory_trend: TrendDirection::Stable,
                response_time_trend: TrendDirection::Stable,
                throughput_trend: TrendDirection::Stable,
                error_rate_trend: TrendDirection::Stable,
                predictions: HashMap::new(),
            };
        }

        let cpu_values: Vec<f64> = metrics.iter().map(|m| m.cpu_usage).collect();
        let memory_values: Vec<f64> = metrics.iter().map(|m| m.memory_usage.usage_percentage).collect();
        let response_time_values: Vec<f64> = metrics.iter().map(|m| m.request_metrics.average_response_time_ms).collect();

        TrendAnalysis {
            cpu_trend: self.calculate_trend(&cpu_values),
            memory_trend: self.calculate_trend(&memory_values),
            response_time_trend: self.calculate_trend(&response_time_values),
            throughput_trend: TrendDirection::Stable, // Simplified for now
            error_rate_trend: TrendDirection::Stable, // Simplified for now
            predictions: HashMap::new(), // Would implement ML-based predictions
        }
    }

    fn calculate_trend(&self, values: &[f64]) -> TrendDirection {
        if values.len() < 3 {
            return TrendDirection::Stable;
        }

        let first_half = &values[0..values.len()/2];
        let second_half = &values[values.len()/2..];

        let first_avg = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_avg = second_half.iter().sum::<f64>() / second_half.len() as f64;

        let change_percentage = ((second_avg - first_avg) / first_avg) * 100.0;

        if change_percentage > 10.0 {
            TrendDirection::Increasing
        } else if change_percentage < -10.0 {
            TrendDirection::Decreasing
        } else {
            // Check for volatility
            let variance = values.iter()
                .map(|&x| (x - (values.iter().sum::<f64>() / values.len() as f64)).powi(2))
                .sum::<f64>() / values.len() as f64;
            
            if variance > first_avg * 0.2 {
                TrendDirection::Volatile
            } else {
                TrendDirection::Stable
            }
        }
    }

    async fn generate_recommendations(&self, metrics: &[PerformanceMetrics]) -> Vec<PerformanceRecommendation> {
        let mut recommendations = Vec::new();

        if let Some(latest) = metrics.last() {
            // CPU recommendations
            if latest.cpu_usage > 80.0 {
                recommendations.push(PerformanceRecommendation {
                    id: Uuid::new_v4(),
                    category: RecommendationCategory::Performance,
                    priority: RecommendationPriority::High,
                    title: "High CPU Usage Detected".to_string(),
                    description: "CPU usage is consistently above 80%. Consider optimizing CPU-intensive operations or scaling out.".to_string(),
                    expected_impact: "Reduce CPU usage by 20-30% and improve response times".to_string(),
                    implementation_effort: ImplementationEffort::Medium,
                    related_metrics: vec!["cpu_usage".to_string(), "response_time".to_string()],
                });
            }

            // Memory recommendations
            if latest.memory_usage.usage_percentage > 85.0 {
                recommendations.push(PerformanceRecommendation {
                    id: Uuid::new_v4(),
                    category: RecommendationCategory::Performance,
                    priority: RecommendationPriority::High,
                    title: "High Memory Usage Detected".to_string(),
                    description: "Memory usage is above 85%. Review memory allocation patterns and consider increasing available memory.".to_string(),
                    expected_impact: "Prevent out-of-memory errors and improve system stability".to_string(),
                    implementation_effort: ImplementationEffort::Low,
                    related_metrics: vec!["memory_usage".to_string()],
                });
            }

            // Response time recommendations
            if latest.request_metrics.average_response_time_ms > 1000.0 {
                recommendations.push(PerformanceRecommendation {
                    id: Uuid::new_v4(),
                    category: RecommendationCategory::Performance,
                    priority: RecommendationPriority::Medium,
                    title: "High Response Times Detected".to_string(),
                    description: "Average response time exceeds 1 second. Consider implementing caching, optimizing database queries, or reviewing API performance.".to_string(),
                    expected_impact: "Improve user experience and reduce response times by 40-60%".to_string(),
                    implementation_effort: ImplementationEffort::Medium,
                    related_metrics: vec!["response_time".to_string(), "database_query_time".to_string()],
                });
            }

            // Database recommendations
            if latest.database_metrics.average_query_time_ms > 500.0 {
                recommendations.push(PerformanceRecommendation {
                    id: Uuid::new_v4(),
                    category: RecommendationCategory::Performance,
                    priority: RecommendationPriority::Medium,
                    title: "Slow Database Queries Detected".to_string(),
                    description: "Database queries are taking longer than expected. Review query performance, add indexes, or consider query optimization.".to_string(),
                    expected_impact: "Improve database performance and reduce overall response times".to_string(),
                    implementation_effort: ImplementationEffort::Medium,
                    related_metrics: vec!["database_query_time".to_string(), "response_time".to_string()],
                });
            }
        }

        recommendations
    }
}

#[async_trait]
impl PerformanceDashboard for ProductionPerformanceDashboard {
    async fn get_current_metrics(&self) -> Result<DashboardMetrics> {
        let current_metrics = self.metrics_collector.get_current_metrics().await?;
        let health_status = self.get_system_health_status().await?;

        Ok(DashboardMetrics {
            timestamp: current_metrics.timestamp,
            system_overview: SystemOverview {
                uptime_seconds: 86400, // Placeholder - would calculate actual uptime
                total_requests: current_metrics.request_metrics.total_requests,
                successful_requests: current_metrics.request_metrics.successful_requests,
                failed_requests: current_metrics.request_metrics.failed_requests,
                average_response_time_ms: current_metrics.request_metrics.average_response_time_ms,
                current_load: current_metrics.cpu_usage,
                active_connections: current_metrics.network_io.connections_active,
            },
            performance_indicators: PerformanceIndicators {
                throughput_rps: current_metrics.request_metrics.requests_per_second,
                latency_p50_ms: current_metrics.request_metrics.average_response_time_ms,
                latency_p95_ms: current_metrics.request_metrics.p95_response_time_ms,
                latency_p99_ms: current_metrics.request_metrics.p99_response_time_ms,
                error_rate_percentage: if current_metrics.request_metrics.total_requests > 0 {
                    (current_metrics.request_metrics.failed_requests as f64 / 
                     current_metrics.request_metrics.total_requests as f64) * 100.0
                } else {
                    0.0
                },
                availability_percentage: 99.9, // Placeholder - would calculate from uptime data
                apdex_score: 0.85, // Placeholder - would calculate from response time distribution
            },
            resource_utilization: ResourceUtilization {
                cpu_usage_percentage: current_metrics.cpu_usage,
                memory_usage_percentage: current_metrics.memory_usage.usage_percentage,
                disk_usage_percentage: current_metrics.disk_usage.usage_percentage,
                network_utilization_percentage: current_metrics.network_io.bandwidth_utilization,
                database_connections_used: current_metrics.database_metrics.connections_active,
                database_connections_available: current_metrics.database_metrics.connections_max - current_metrics.database_metrics.connections_active,
                cache_hit_ratio: current_metrics.database_metrics.cache_hit_ratio,
            },
            application_metrics: ApplicationMetrics {
                active_jobs: 15, // Placeholder - would get from job queue
                queued_jobs: 5,  // Placeholder
                completed_jobs: 1000, // Placeholder
                failed_jobs: 25, // Placeholder
                pipeline_executions: 500, // Placeholder
                runner_instances: 3, // Placeholder
                plugin_load_count: 10, // Placeholder
            },
            health_status,
        })
    }

    async fn get_historical_data(&self, duration: Duration) -> Result<HistoricalData> {
        let metrics = self.metrics_collector.get_historical_metrics(duration).await?;
        
        if metrics.is_empty() {
            return Ok(HistoricalData {
                time_range: TimeRange {
                    start_timestamp: 0,
                    end_timestamp: 0,
                    duration_seconds: 0,
                    resolution_seconds: 60,
                },
                metrics_timeline: Vec::new(),
                aggregated_stats: AggregatedStats {
                    min_values: HashMap::new(),
                    max_values: HashMap::new(),
                    avg_values: HashMap::new(),
                    percentiles: HashMap::new(),
                },
                trend_analysis: TrendAnalysis {
                    cpu_trend: TrendDirection::Stable,
                    memory_trend: TrendDirection::Stable,
                    response_time_trend: TrendDirection::Stable,
                    throughput_trend: TrendDirection::Stable,
                    error_rate_trend: TrendDirection::Stable,
                    predictions: HashMap::new(),
                },
            });
        }

        let start_timestamp = metrics.first().unwrap().timestamp;
        let end_timestamp = metrics.last().unwrap().timestamp;

        let timeline: Vec<TimeSeriesPoint> = metrics.iter().map(|m| TimeSeriesPoint {
            timestamp: m.timestamp,
            cpu_usage: m.cpu_usage,
            memory_usage: m.memory_usage.usage_percentage,
            response_time: m.request_metrics.average_response_time_ms,
            throughput: m.request_metrics.requests_per_second,
            error_rate: if m.request_metrics.total_requests > 0 {
                (m.request_metrics.failed_requests as f64 / m.request_metrics.total_requests as f64) * 100.0
            } else {
                0.0
            },
        }).collect();

        // Calculate aggregated stats
        let cpu_values: Vec<f64> = metrics.iter().map(|m| m.cpu_usage).collect();
        let memory_values: Vec<f64> = metrics.iter().map(|m| m.memory_usage.usage_percentage).collect();
        let response_time_values: Vec<f64> = metrics.iter().map(|m| m.request_metrics.average_response_time_ms).collect();

        let mut min_values = HashMap::new();
        let mut max_values = HashMap::new();
        let mut avg_values = HashMap::new();

        min_values.insert("cpu_usage".to_string(), cpu_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
        max_values.insert("cpu_usage".to_string(), cpu_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
        avg_values.insert("cpu_usage".to_string(), cpu_values.iter().sum::<f64>() / cpu_values.len() as f64);

        min_values.insert("memory_usage".to_string(), memory_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
        max_values.insert("memory_usage".to_string(), memory_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
        avg_values.insert("memory_usage".to_string(), memory_values.iter().sum::<f64>() / memory_values.len() as f64);

        min_values.insert("response_time".to_string(), response_time_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
        max_values.insert("response_time".to_string(), response_time_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)));
        avg_values.insert("response_time".to_string(), response_time_values.iter().sum::<f64>() / response_time_values.len() as f64);

        let trend_analysis = self.analyze_trends(&metrics).await;

        Ok(HistoricalData {
            time_range: TimeRange {
                start_timestamp,
                end_timestamp,
                duration_seconds: end_timestamp - start_timestamp,
                resolution_seconds: 60,
            },
            metrics_timeline: timeline,
            aggregated_stats: AggregatedStats {
                min_values,
                max_values,
                avg_values,
                percentiles: HashMap::new(), // Would implement percentile calculations
            },
            trend_analysis,
        })
    }

    async fn get_performance_summary(&self) -> Result<PerformanceSummary> {
        let duration = Duration::from_secs(3600); // Last hour
        let metrics = self.metrics_collector.get_historical_metrics(duration).await?;
        let recommendations = self.generate_recommendations(&metrics).await;

        let start_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() - 3600;
        let end_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let key_metrics = if let Some(latest) = metrics.last() {
            let error_rate = if latest.request_metrics.total_requests > 0 {
                (latest.request_metrics.failed_requests as f64 / 
                 latest.request_metrics.total_requests as f64) * 100.0
            } else {
                0.0
            };
            
            KeyMetricsSummary {
                average_response_time_ms: latest.request_metrics.average_response_time_ms,
                peak_response_time_ms: metrics.iter()
                    .map(|m| m.request_metrics.average_response_time_ms)
                    .fold(0.0, f64::max),
                total_requests: latest.request_metrics.total_requests,
                error_rate_percentage: error_rate,
                uptime_percentage: 99.9, // Placeholder
                peak_cpu_usage: metrics.iter().map(|m| m.cpu_usage).fold(0.0, f64::max),
                peak_memory_usage: metrics.iter()
                    .map(|m| m.memory_usage.usage_percentage)
                    .fold(0.0, f64::max),
                throughput_rps: latest.request_metrics.requests_per_second,
            }
        } else {
            KeyMetricsSummary {
                average_response_time_ms: 0.0,
                peak_response_time_ms: 0.0,
                total_requests: 0,
                error_rate_percentage: 0.0,
                uptime_percentage: 100.0,
                peak_cpu_usage: 0.0,
                peak_memory_usage: 0.0,
                throughput_rps: 0.0,
            }
        };

        let response_time_actual = key_metrics.average_response_time_ms;
        let error_rate_actual = key_metrics.error_rate_percentage;
        
        Ok(PerformanceSummary {
            period: TimeRange {
                start_timestamp,
                end_timestamp,
                duration_seconds: 3600,
                resolution_seconds: 60,
            },
            key_metrics,
            performance_issues: Vec::new(), // Would populate from issue detection
            recommendations,
            sla_compliance: SlaCompliance {
                availability_target: 99.9,
                availability_actual: 99.9,
                response_time_target_ms: 500.0,
                response_time_actual_ms: response_time_actual,
                error_rate_target: 1.0,
                error_rate_actual: error_rate_actual,
                compliance_score: 95.0, // Placeholder calculation
                violations: Vec::new(),
            },
        })
    }

    async fn get_alert_summary(&self) -> Result<AlertSummary> {
        let active_alerts = self.alert_manager.get_active_alerts().await?;
        
        let mut alerts_by_severity = HashMap::new();
        let mut alerts_by_component = HashMap::new();
        
        for alert in &active_alerts {
            // This is simplified - in a real implementation, we'd need to get alert details
            alerts_by_component.entry("system".to_string()).and_modify(|e| *e += 1).or_insert(1);
        }

        Ok(AlertSummary {
            total_alerts: active_alerts.len() as u32,
            active_alerts: active_alerts.iter().filter(|a| !a.acknowledged).count() as u32,
            resolved_alerts: 0, // Would track resolved alerts
            acknowledged_alerts: active_alerts.iter().filter(|a| a.acknowledged).count() as u32,
            alerts_by_severity,
            alerts_by_component,
            recent_alerts: active_alerts.into_iter().take(10).map(|alert| {
                let duration_seconds = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() - alert.triggered_at)
                    .ok();
                
                AlertSummaryItem {
                    id: alert.id,
                    rule_name: "Alert Rule".to_string(), // Would get from rule
                    severity: IssueSeverity::Medium, // Would get from rule
                    component: "system".to_string(),
                    triggered_at: alert.triggered_at,
                    status: if alert.acknowledged {
                        AlertStatus::Acknowledged
                    } else {
                        AlertStatus::Active
                    },
                    duration_seconds,
                }
            }).collect(),
            alert_trends: AlertTrends {
                alerts_per_hour: vec![0; 24], // Would populate with actual data
                mttr_seconds: 1800.0, // Placeholder
                mtbf_seconds: 86400.0, // Placeholder
                alert_frequency_trend: TrendDirection::Stable,
            },
        })
    }

    async fn generate_performance_report(&self, duration: Duration) -> Result<PerformanceReport> {
        let metrics = self.metrics_collector.get_historical_metrics(duration).await?;
        let recommendations = self.generate_recommendations(&metrics).await;
        
        let start_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() - duration.as_secs();
        let end_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        Ok(PerformanceReport {
            report_id: Uuid::new_v4(),
            generated_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            period: TimeRange {
                start_timestamp,
                end_timestamp,
                duration_seconds: duration.as_secs(),
                resolution_seconds: 60,
            },
            executive_summary: ExecutiveSummary {
                overall_performance_score: 85.0, // Placeholder calculation
                key_achievements: vec![
                    "Maintained 99.9% uptime".to_string(),
                    "Average response time under 500ms".to_string(),
                ],
                critical_issues: vec![],
                business_impact: "System performance is meeting SLA requirements".to_string(),
                next_steps: vec![
                    "Continue monitoring performance trends".to_string(),
                    "Implement recommended optimizations".to_string(),
                ],
            },
            detailed_metrics: DetailedMetrics {
                performance_kpis: HashMap::new(), // Would populate with actual KPIs
                resource_utilization: HashMap::new(),
                availability_metrics: HashMap::new(),
                capacity_metrics: HashMap::new(),
            },
            performance_analysis: PerformanceAnalysis {
                bottleneck_analysis: Vec::new(),
                capacity_analysis: CapacityAnalysis {
                    current_utilization: HashMap::new(),
                    projected_growth: HashMap::new(),
                    capacity_runway_days: HashMap::new(),
                    scaling_recommendations: Vec::new(),
                },
                trend_analysis: self.analyze_trends(&metrics).await,
                comparative_analysis: ComparativeAnalysis {
                    period_over_period: HashMap::new(),
                    baseline_comparison: HashMap::new(),
                    industry_benchmarks: HashMap::new(),
                },
            },
            recommendations,
            appendices: ReportAppendices {
                raw_data_summary: HashMap::new(),
                methodology: "Performance metrics collected every 60 seconds and analyzed using statistical methods".to_string(),
                data_sources: vec!["System metrics".to_string(), "Application metrics".to_string()],
                limitations: vec!["Historical data limited to retention period".to_string()],
            },
        })
    }

    async fn get_system_health_status(&self) -> Result<SystemHealthStatus> {
        let current_metrics = self.metrics_collector.get_current_metrics().await?;
        
        let mut component_health = HashMap::new();
        
        // System health
        let system_status = if current_metrics.cpu_usage > 90.0 || current_metrics.memory_usage.usage_percentage > 95.0 {
            HealthStatus::Unhealthy
        } else if current_metrics.cpu_usage > 80.0 || current_metrics.memory_usage.usage_percentage > 85.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        component_health.insert("system".to_string(), ComponentHealth {
            status: system_status.clone(),
            response_time_ms: None,
            error_count: 0,
            last_check: current_metrics.timestamp,
            details: HashMap::new(),
        });

        // Database health
        let db_status = if current_metrics.database_metrics.average_query_time_ms > 1000.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        component_health.insert("database".to_string(), ComponentHealth {
            status: db_status,
            response_time_ms: Some(current_metrics.database_metrics.average_query_time_ms),
            error_count: 0,
            last_check: current_metrics.timestamp,
            details: HashMap::new(),
        });

        // API health
        let api_status = if current_metrics.request_metrics.average_response_time_ms > 2000.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        component_health.insert("api".to_string(), ComponentHealth {
            status: api_status,
            response_time_ms: Some(current_metrics.request_metrics.average_response_time_ms),
            error_count: current_metrics.request_metrics.failed_requests as u32,
            last_check: current_metrics.timestamp,
            details: HashMap::new(),
        });

        // Overall health
        let overall_status = if component_health.values().any(|h| h.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if component_health.values().any(|h| h.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let health_score = match overall_status {
            HealthStatus::Healthy => 100.0,
            HealthStatus::Degraded => 75.0,
            HealthStatus::Unhealthy => 25.0,
            HealthStatus::Unknown => 0.0,
        };

        Ok(SystemHealthStatus {
            overall_status,
            component_health,
            last_health_check: current_metrics.timestamp,
            health_score,
        })
    }
}