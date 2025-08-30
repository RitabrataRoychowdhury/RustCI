use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub timestamp: u64,
    pub correlation_id: Uuid,
    pub cpu_usage: f64,
    pub memory_usage: MemoryMetrics,
    pub disk_usage: DiskMetrics,
    pub network_io: NetworkMetrics,
    pub database_metrics: DatabaseMetrics,
    pub request_metrics: RequestMetrics,
    pub custom_metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub used_mb: u64,
    pub available_mb: u64,
    pub total_mb: u64,
    pub usage_percentage: f64,
    pub heap_size_mb: u64,
    pub gc_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub used_gb: u64,
    pub available_gb: u64,
    pub total_gb: u64,
    pub usage_percentage: f64,
    pub read_iops: u64,
    pub write_iops: u64,
    pub read_throughput_mbps: f64,
    pub write_throughput_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connections_active: u32,
    pub connections_idle: u32,
    pub bandwidth_utilization: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetrics {
    pub connections_active: u32,
    pub connections_idle: u32,
    pub connections_max: u32,
    pub query_count: u64,
    pub slow_query_count: u64,
    pub average_query_time_ms: f64,
    pub transaction_count: u64,
    pub deadlock_count: u64,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub requests_per_second: f64,
    pub concurrent_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    pub id: Uuid,
    pub issue_type: PerformanceIssueType,
    pub severity: IssueSeverity,
    pub description: String,
    pub detected_at: u64,
    pub affected_component: String,
    pub metrics_snapshot: PerformanceMetrics,
    pub suggested_actions: Vec<String>,
    pub auto_resolution_attempted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PerformanceIssueType {
    HighCpuUsage,
    HighMemoryUsage,
    HighDiskUsage,
    SlowDatabaseQueries,
    HighResponseTimes,
    LowThroughput,
    ConnectionPoolExhaustion,
    CacheInefficiency,
    NetworkBottleneck,
    ResourceLeak,
}

impl std::fmt::Display for PerformanceIssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PerformanceIssueType::HighCpuUsage => write!(f, "High CPU Usage"),
            PerformanceIssueType::HighMemoryUsage => write!(f, "High Memory Usage"),
            PerformanceIssueType::HighDiskUsage => write!(f, "High Disk Usage"),
            PerformanceIssueType::SlowDatabaseQueries => write!(f, "Slow Database Queries"),
            PerformanceIssueType::HighResponseTimes => write!(f, "High Response Times"),
            PerformanceIssueType::LowThroughput => write!(f, "Low Throughput"),
            PerformanceIssueType::ConnectionPoolExhaustion => write!(f, "Connection Pool Exhaustion"),
            PerformanceIssueType::CacheInefficiency => write!(f, "Cache Inefficiency"),
            PerformanceIssueType::NetworkBottleneck => write!(f, "Network Bottleneck"),
            PerformanceIssueType::ResourceLeak => write!(f, "Resource Leak"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    pub id: Uuid,
    pub alert_type: AlertType,
    pub threshold: f64,
    pub current_value: f64,
    pub component: String,
    pub message: String,
    pub created_at: u64,
    pub escalation_level: u32,
    pub auto_resolve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    CpuThreshold,
    MemoryThreshold,
    DiskThreshold,
    ResponseTimeThreshold,
    ThroughputThreshold,
    ErrorRateThreshold,
    DatabaseConnectionThreshold,
    CustomMetricThreshold(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingAction {
    pub action_id: Uuid,
    pub action_type: ScalingActionType,
    pub target_component: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub triggered_by: PerformanceIssue,
    pub estimated_impact: String,
    pub execution_time_estimate: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalingActionType {
    ScaleUp,
    ScaleDown,
    ScaleOut,
    ScaleIn,
    OptimizeCache,
    RestartComponent,
    AdjustConnectionPool,
    EnableCircuitBreaker,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            correlation_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: MemoryMetrics::default(),
            disk_usage: DiskMetrics::default(),
            network_io: NetworkMetrics::default(),
            database_metrics: DatabaseMetrics::default(),
            request_metrics: RequestMetrics::default(),
            custom_metrics: HashMap::new(),
        }
    }

    pub fn add_custom_metric(&mut self, name: String, value: f64) {
        self.custom_metrics.insert(name, value);
    }

    pub fn is_healthy(&self) -> bool {
        self.cpu_usage < 80.0
            && self.memory_usage.usage_percentage < 85.0
            && self.disk_usage.usage_percentage < 90.0
            && self.request_metrics.average_response_time_ms < 1000.0
    }
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            used_mb: 0,
            available_mb: 0,
            total_mb: 0,
            usage_percentage: 0.0,
            heap_size_mb: 0,
            gc_count: 0,
        }
    }
}

impl Default for DiskMetrics {
    fn default() -> Self {
        Self {
            used_gb: 0,
            available_gb: 0,
            total_gb: 0,
            usage_percentage: 0.0,
            read_iops: 0,
            write_iops: 0,
            read_throughput_mbps: 0.0,
            write_throughput_mbps: 0.0,
        }
    }
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            connections_active: 0,
            connections_idle: 0,
            bandwidth_utilization: 0.0,
        }
    }
}

impl Default for DatabaseMetrics {
    fn default() -> Self {
        Self {
            connections_active: 0,
            connections_idle: 0,
            connections_max: 0,
            query_count: 0,
            slow_query_count: 0,
            average_query_time_ms: 0.0,
            transaction_count: 0,
            deadlock_count: 0,
            cache_hit_ratio: 0.0,
        }
    }
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            requests_per_second: 0.0,
            concurrent_requests: 0,
        }
    }
}