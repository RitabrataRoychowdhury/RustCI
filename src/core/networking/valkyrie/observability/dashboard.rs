// Internal metrics dashboard

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::RwLock;
use tokio::time::interval;
use serde::{Serialize, Deserialize};

use super::{ObservabilityError, MetricsCollector, HealthMonitor, HealthStatus};
use super::metrics::{MetricQuery, AggregationFunction, MetricsSummary};
use super::health::HealthSummary;

/// Internal metrics dashboard
pub struct MetricsDashboard {
    /// Metrics collector reference
    metrics_collector: Arc<MetricsCollector>,
    /// Health monitor reference
    health_monitor: Arc<HealthMonitor>,
    /// Dashboard configuration
    config: DashboardConfig,
    /// Dashboard data cache
    dashboard_data: Arc<RwLock<DashboardData>>,
    /// Background refresh task handle
    refresh_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// Dashboard title
    pub title: String,
    /// Refresh interval in seconds
    pub refresh_interval_seconds: u64,
    /// Enable real-time updates
    pub real_time_updates: bool,
    /// Maximum data points to display
    pub max_data_points: usize,
    /// Dashboard widgets configuration
    pub widgets: Vec<DashboardWidget>,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            title: "Valkyrie Protocol Dashboard".to_string(),
            refresh_interval_seconds: 5,
            real_time_updates: true,
            max_data_points: 100,
            widgets: vec![
                DashboardWidget {
                    id: "system_overview".to_string(),
                    title: "System Overview".to_string(),
                    widget_type: WidgetType::SystemOverview,
                    position: WidgetPosition { row: 0, col: 0, width: 12, height: 4 },
                    config: WidgetConfig::default(),
                },
                DashboardWidget {
                    id: "health_status".to_string(),
                    title: "Health Status".to_string(),
                    widget_type: WidgetType::HealthStatus,
                    position: WidgetPosition { row: 1, col: 0, width: 6, height: 4 },
                    config: WidgetConfig::default(),
                },
                DashboardWidget {
                    id: "metrics_summary".to_string(),
                    title: "Metrics Summary".to_string(),
                    widget_type: WidgetType::MetricsSummary,
                    position: WidgetPosition { row: 1, col: 6, width: 6, height: 4 },
                    config: WidgetConfig::default(),
                },
                DashboardWidget {
                    id: "performance_chart".to_string(),
                    title: "Performance Metrics".to_string(),
                    widget_type: WidgetType::LineChart {
                        metrics: vec!["latency_ms".to_string(), "throughput_rps".to_string()],
                    },
                    position: WidgetPosition { row: 2, col: 0, width: 12, height: 6 },
                    config: WidgetConfig::default(),
                },
            ],
        }
    }
}

/// Dashboard widget definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardWidget {
    /// Widget identifier
    pub id: String,
    /// Widget title
    pub title: String,
    /// Widget type
    pub widget_type: WidgetType,
    /// Widget position and size
    pub position: WidgetPosition,
    /// Widget-specific configuration
    pub config: WidgetConfig,
}

/// Widget types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WidgetType {
    /// System overview widget
    SystemOverview,
    /// Health status widget
    HealthStatus,
    /// Metrics summary widget
    MetricsSummary,
    /// Line chart widget
    LineChart { metrics: Vec<String> },
    /// Bar chart widget
    BarChart { metric: String },
    /// Gauge widget
    Gauge { metric: String, min: f64, max: f64 },
    /// Table widget
    Table { columns: Vec<String> },
    /// Log viewer widget
    LogViewer { max_entries: usize },
    /// Custom widget
    Custom { widget_name: String },
}

/// Widget position and size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetPosition {
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub height: u32,
}

/// Widget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    /// Widget-specific parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Refresh interval override
    pub refresh_interval_seconds: Option<u64>,
    /// Enable/disable widget
    pub enabled: bool,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            parameters: HashMap::new(),
            refresh_interval_seconds: None,
            enabled: true,
        }
    }
}

/// Dashboard data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    /// Dashboard metadata
    pub metadata: DashboardMetadata,
    /// System overview data
    pub system_overview: SystemOverviewData,
    /// Health status data
    pub health_status: HealthSummary,
    /// Metrics summary data
    pub metrics_summary: MetricsSummary,
    /// Widget data
    pub widgets: HashMap<String, WidgetData>,
    /// Last update timestamp
    pub last_updated: u64,
}

/// Dashboard metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetadata {
    pub title: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub refresh_interval: u64,
}

/// System overview data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOverviewData {
    pub protocol_version: String,
    pub active_connections: u64,
    pub total_messages: u64,
    pub messages_per_second: f64,
    pub average_latency_ms: f64,
    pub error_rate_percent: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

/// Widget data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WidgetData {
    /// Chart data with time series
    Chart {
        series: Vec<ChartSeries>,
        x_axis: Vec<String>,
    },
    /// Table data with rows and columns
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Gauge data with current value
    Gauge {
        current: f64,
        min: f64,
        max: f64,
        unit: String,
    },
    /// Text data
    Text {
        content: String,
    },
    /// JSON data
    Json {
        data: serde_json::Value,
    },
}

/// Chart series data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    pub name: String,
    pub data: Vec<f64>,
    pub color: Option<String>,
}

impl MetricsDashboard {
    /// Create a new metrics dashboard
    pub fn new(
        metrics_collector: Arc<MetricsCollector>,
        health_monitor: Arc<HealthMonitor>,
        refresh_interval_seconds: u64,
    ) -> Self {
        let config = DashboardConfig {
            refresh_interval_seconds,
            ..Default::default()
        };

        Self {
            metrics_collector,
            health_monitor,
            config,
            dashboard_data: Arc::new(RwLock::new(DashboardData::default())),
            refresh_handle: Arc::new(tokio::sync::Mutex::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create dashboard with custom configuration
    pub fn with_config(
        metrics_collector: Arc<MetricsCollector>,
        health_monitor: Arc<HealthMonitor>,
        config: DashboardConfig,
    ) -> Self {
        Self {
            metrics_collector,
            health_monitor,
            config,
            dashboard_data: Arc::new(RwLock::new(DashboardData::default())),
            refresh_handle: Arc::new(tokio::sync::Mutex::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the dashboard
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        *running = true;
        drop(running);

        // Initial data refresh
        self.refresh_data().await?;

        // Start background refresh task
        let metrics_collector = self.metrics_collector.clone();
        let health_monitor = self.health_monitor.clone();
        let dashboard_data = self.dashboard_data.clone();
        let config = self.config.clone();
        let running_flag = self.running.clone();

        *self.refresh_handle.lock().await = Some(tokio::spawn(async move {
            let mut refresh_interval = interval(Duration::from_secs(config.refresh_interval_seconds));

            while *running_flag.read().await {
                refresh_interval.tick().await;
                
                if let Err(e) = Self::update_dashboard_data(
                    &metrics_collector,
                    &health_monitor,
                    &dashboard_data,
                    &config,
                ).await {
                    eprintln!("Dashboard refresh error: {}", e);
                }
            }
        }));

        Ok(())
    }

    /// Stop the dashboard
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);

        if let Some(handle) = self.refresh_handle.lock().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Get current dashboard data
    pub async fn get_data(&self) -> DashboardData {
        let data = self.dashboard_data.read().await;
        data.clone()
    }

    /// Refresh dashboard data manually
    pub async fn refresh_data(&self) -> Result<(), ObservabilityError> {
        Self::update_dashboard_data(
            &self.metrics_collector,
            &self.health_monitor,
            &self.dashboard_data,
            &self.config,
        ).await
    }

    /// Update dashboard data
    async fn update_dashboard_data(
        metrics_collector: &MetricsCollector,
        health_monitor: &HealthMonitor,
        dashboard_data: &Arc<RwLock<DashboardData>>,
        config: &DashboardConfig,
    ) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Collect data from various sources
        let health_summary = health_monitor.summary().await;
        let metrics_summary = metrics_collector.summary().await;
        let system_overview = Self::collect_system_overview(metrics_collector).await?;

        // Update widget data
        let mut widgets = HashMap::new();
        for widget in &config.widgets {
            if !widget.config.enabled {
                continue;
            }

            let widget_data = Self::collect_widget_data(
                &widget.widget_type,
                metrics_collector,
                health_monitor,
            ).await?;
            
            widgets.insert(widget.id.clone(), widget_data);
        }

        // Update dashboard data
        let mut data = dashboard_data.write().await;
        *data = DashboardData {
            metadata: DashboardMetadata {
                title: config.title.clone(),
                version: "1.0.0".to_string(),
                uptime_seconds: timestamp, // Simplified
                refresh_interval: config.refresh_interval_seconds,
            },
            system_overview,
            health_status: health_summary,
            metrics_summary,
            widgets,
            last_updated: timestamp,
        };

        Ok(())
    }

    /// Collect system overview data
    async fn collect_system_overview(
        metrics_collector: &MetricsCollector,
    ) -> Result<SystemOverviewData, ObservabilityError> {
        // Collect various system metrics
        let active_connections = metrics_collector.aggregate(
            MetricQuery {
                name: Some("active_connections".to_string()),
                labels: HashMap::new(),
                start_time: None,
                end_time: None,
                limit: Some(1),
            },
            AggregationFunction::Sum,
        ).await.unwrap_or(0.0) as u64;

        let total_messages = metrics_collector.aggregate(
            MetricQuery {
                name: Some("total_messages".to_string()),
                labels: HashMap::new(),
                start_time: None,
                end_time: None,
                limit: None,
            },
            AggregationFunction::Sum,
        ).await.unwrap_or(0.0) as u64;

        let average_latency = metrics_collector.aggregate(
            MetricQuery {
                name: Some("latency_ms".to_string()),
                labels: HashMap::new(),
                start_time: None,
                end_time: None,
                limit: Some(100),
            },
            AggregationFunction::Average,
        ).await.unwrap_or(0.0);

        Ok(SystemOverviewData {
            protocol_version: "1.0.0".to_string(),
            active_connections,
            total_messages,
            messages_per_second: 0.0, // Calculated from rate
            average_latency_ms: average_latency,
            error_rate_percent: 0.0, // Calculated from error metrics
            memory_usage_mb: 0.0, // From system metrics
            cpu_usage_percent: 0.0, // From system metrics
        })
    }

    /// Collect widget-specific data
    async fn collect_widget_data(
        widget_type: &WidgetType,
        metrics_collector: &MetricsCollector,
        health_monitor: &HealthMonitor,
    ) -> Result<WidgetData, ObservabilityError> {
        match widget_type {
            WidgetType::SystemOverview => {
                Ok(WidgetData::Json {
                    data: serde_json::json!({
                        "status": "operational",
                        "components": ["metrics", "health", "dashboard"]
                    }),
                })
            }
            WidgetType::HealthStatus => {
                let health_results = health_monitor.get_all_results().await;
                let mut rows = Vec::new();
                
                for (check_id, result) in health_results {
                    rows.push(vec![
                        check_id,
                        result.status.to_string(),
                        result.message,
                        result.duration_ms.to_string(),
                    ]);
                }

                Ok(WidgetData::Table {
                    headers: vec![
                        "Check ID".to_string(),
                        "Status".to_string(),
                        "Message".to_string(),
                        "Duration (ms)".to_string(),
                    ],
                    rows,
                })
            }
            WidgetType::MetricsSummary => {
                let summary = metrics_collector.summary().await;
                Ok(WidgetData::Json {
                    data: serde_json::to_value(summary)
                        .map_err(|e| ObservabilityError::Dashboard(format!("Serialization error: {}", e)))?,
                })
            }
            WidgetType::LineChart { metrics } => {
                let mut series = Vec::new();
                
                for metric_name in metrics {
                    let query = MetricQuery {
                        name: Some(metric_name.clone()),
                        labels: HashMap::new(),
                        start_time: None,
                        end_time: None,
                        limit: Some(50),
                    };
                    
                    let metric_series = metrics_collector.query(query).await?;
                    if let Some(first_series) = metric_series.first() {
                        let data: Vec<f64> = first_series.data_points.iter()
                            .map(|point| match &point.value {
                                super::metrics::MetricValue::Counter(v) => *v as f64,
                                super::metrics::MetricValue::Gauge(v) => *v,
                                super::metrics::MetricValue::Histogram(values) => {
                                    values.iter().sum::<f64>() / values.len() as f64
                                }
                                super::metrics::MetricValue::Summary { sum, count } => {
                                    if *count > 0 { *sum / *count as f64 } else { 0.0 }
                                }
                            })
                            .collect();
                        
                        series.push(ChartSeries {
                            name: metric_name.clone(),
                            data,
                            color: None,
                        });
                    }
                }

                Ok(WidgetData::Chart {
                    series,
                    x_axis: (0..50).map(|i| format!("T-{}", 50 - i)).collect(),
                })
            }
            WidgetType::BarChart { metric } => {
                Ok(WidgetData::Chart {
                    series: vec![ChartSeries {
                        name: metric.clone(),
                        data: vec![10.0, 20.0, 15.0, 30.0, 25.0], // Placeholder data
                        color: Some("#3498db".to_string()),
                    }],
                    x_axis: vec!["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string(), "E".to_string()],
                })
            }
            WidgetType::Gauge { metric, min, max } => {
                let current_value = metrics_collector.aggregate(
                    MetricQuery {
                        name: Some(metric.clone()),
                        labels: HashMap::new(),
                        start_time: None,
                        end_time: None,
                        limit: Some(1),
                    },
                    AggregationFunction::Average,
                ).await.unwrap_or(0.0);

                Ok(WidgetData::Gauge {
                    current: current_value,
                    min: *min,
                    max: *max,
                    unit: "".to_string(),
                })
            }
            WidgetType::Table { columns } => {
                Ok(WidgetData::Table {
                    headers: columns.clone(),
                    rows: vec![
                        vec!["Sample".to_string(), "Data".to_string()],
                        vec!["Row".to_string(), "1".to_string()],
                    ],
                })
            }
            WidgetType::LogViewer { max_entries: _ } => {
                Ok(WidgetData::Text {
                    content: "Log viewer not implemented".to_string(),
                })
            }
            WidgetType::Custom { widget_name } => {
                Ok(WidgetData::Text {
                    content: format!("Custom widget '{}' not implemented", widget_name),
                })
            }
        }
    }

    /// Generate HTML dashboard
    pub async fn generate_html(&self) -> String {
        let data = self.get_data().await;
        
        format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{ font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }}
        .dashboard {{ max-width: 1200px; margin: 0 auto; }}
        .header {{ background: #2c3e50; color: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; }}
        .widget {{ background: white; border-radius: 8px; padding: 20px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .status-healthy {{ color: #27ae60; }}
        .status-degraded {{ color: #f39c12; }}
        .status-unhealthy {{ color: #e74c3c; }}
        .status-unknown {{ color: #95a5a6; }}
        .metric-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; }}
        .metric-card {{ background: #ecf0f1; padding: 15px; border-radius: 4px; text-align: center; }}
        .metric-value {{ font-size: 24px; font-weight: bold; color: #2c3e50; }}
        .metric-label {{ font-size: 12px; color: #7f8c8d; text-transform: uppercase; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 8px; text-align: left; border-bottom: 1px solid #ddd; }}
        th {{ background-color: #f8f9fa; }}
    </style>
</head>
<body>
    <div class="dashboard">
        <div class="header">
            <h1>{}</h1>
            <p>Last updated: {} | Uptime: {} seconds</p>
        </div>
        
        <div class="widget">
            <h2>System Overview</h2>
            <div class="metric-grid">
                <div class="metric-card">
                    <div class="metric-value">{}</div>
                    <div class="metric-label">Active Connections</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">{}</div>
                    <div class="metric-label">Total Messages</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">{:.2}</div>
                    <div class="metric-label">Avg Latency (ms)</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value status-{}">{}</div>
                    <div class="metric-label">Health Status</div>
                </div>
            </div>
        </div>
        
        <div class="widget">
            <h2>Health Checks</h2>
            <p>Total: {} | Healthy: {} | Degraded: {} | Unhealthy: {}</p>
        </div>
        
        <div class="widget">
            <h2>Metrics Summary</h2>
            <p>Total Metrics: {} | Total Data Points: {}</p>
        </div>
    </div>
    
    <script>
        // Auto-refresh every 30 seconds
        setTimeout(function() {{
            window.location.reload();
        }}, 30000);
    </script>
</body>
</html>
        "#,
            data.metadata.title,
            data.metadata.title,
            chrono::DateTime::from_timestamp(data.last_updated as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S"),
            data.metadata.uptime_seconds,
            data.system_overview.active_connections,
            data.system_overview.total_messages,
            data.system_overview.average_latency_ms,
            data.health_status.overall_status.to_string().to_lowercase(),
            data.health_status.overall_status,
            data.health_status.total_checks,
            data.health_status.healthy_checks,
            data.health_status.degraded_checks,
            data.health_status.unhealthy_checks,
            data.metrics_summary.total_metrics,
            data.metrics_summary.total_data_points,
        )
    }
}

impl Default for DashboardData {
    fn default() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            metadata: DashboardMetadata {
                title: "Valkyrie Protocol Dashboard".to_string(),
                version: "1.0.0".to_string(),
                uptime_seconds: 0,
                refresh_interval: 5,
            },
            system_overview: SystemOverviewData {
                protocol_version: "1.0.0".to_string(),
                active_connections: 0,
                total_messages: 0,
                messages_per_second: 0.0,
                average_latency_ms: 0.0,
                error_rate_percent: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
            },
            health_status: HealthSummary {
                overall_status: HealthStatus::Unknown,
                total_checks: 0,
                healthy_checks: 0,
                degraded_checks: 0,
                unhealthy_checks: 0,
                unknown_checks: 0,
                last_updated: timestamp,
            },
            metrics_summary: super::metrics::MetricsSummary {
                total_metrics: 0,
                total_data_points: 0,
                type_counts: HashMap::new(),
                retention_seconds: 3600,
            },
            widgets: HashMap::new(),
            last_updated: timestamp,
        }
    }
}