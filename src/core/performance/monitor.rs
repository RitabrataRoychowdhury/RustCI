use crate::core::performance::metrics::*;
use crate::error::{AppError, Result};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

#[async_trait]
pub trait PerformanceMonitor: Send + Sync {
    async fn collect_metrics(&self) -> Result<PerformanceMetrics>;
    async fn detect_performance_issues(&self) -> Result<Vec<PerformanceIssue>>;
    async fn trigger_auto_scaling(&self, metrics: &PerformanceMetrics) -> Result<Option<ScalingAction>>;
    async fn register_performance_alert(&self, alert: PerformanceAlert) -> Result<()>;
    async fn get_historical_metrics(&self, duration: Duration) -> Result<Vec<PerformanceMetrics>>;
    async fn start_monitoring(&self) -> Result<()>;
    async fn stop_monitoring(&self) -> Result<()>;
}

pub struct ProductionPerformanceMonitor {
    metrics_history: Arc<RwLock<VecDeque<PerformanceMetrics>>>,
    active_alerts: Arc<RwLock<HashMap<Uuid, PerformanceAlert>>>,
    issue_detectors: Vec<Box<dyn IssueDetector>>,
    alert_thresholds: Arc<RwLock<HashMap<String, f64>>>,
    monitoring_active: Arc<Mutex<bool>>,
    collection_interval: Duration,
    max_history_size: usize,
}

#[async_trait]
pub trait IssueDetector: Send + Sync {
    async fn detect_issues(&self, metrics: &PerformanceMetrics, history: &[PerformanceMetrics]) -> Vec<PerformanceIssue>;
    fn get_detector_name(&self) -> &str;
}

pub struct CpuUsageDetector {
    threshold: f64,
    sustained_duration: Duration,
}

pub struct MemoryUsageDetector {
    threshold: f64,
    leak_detection_enabled: bool,
}

pub struct ResponseTimeDetector {
    threshold_ms: f64,
    percentile_threshold: f64,
}

pub struct DatabasePerformanceDetector {
    slow_query_threshold_ms: f64,
    connection_pool_threshold: f64,
}

impl ProductionPerformanceMonitor {
    pub fn new(collection_interval: Duration, max_history_size: usize) -> Self {
        let mut detectors: Vec<Box<dyn IssueDetector>> = Vec::new();
        detectors.push(Box::new(CpuUsageDetector::new(80.0, Duration::from_secs(300))));
        detectors.push(Box::new(MemoryUsageDetector::new(85.0, true)));
        detectors.push(Box::new(ResponseTimeDetector::new(1000.0, 95.0)));
        detectors.push(Box::new(DatabasePerformanceDetector::new(500.0, 90.0)));

        Self {
            metrics_history: Arc::new(RwLock::new(VecDeque::with_capacity(max_history_size))),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            issue_detectors: detectors,
            alert_thresholds: Arc::new(RwLock::new(HashMap::new())),
            monitoring_active: Arc::new(Mutex::new(false)),
            collection_interval,
            max_history_size,
        }
    }

    pub async fn initialize_default_thresholds(&self) -> Result<()> {
        let mut thresholds = self.alert_thresholds.write().await;
        thresholds.insert("cpu_usage".to_string(), 80.0);
        thresholds.insert("memory_usage".to_string(), 85.0);
        thresholds.insert("disk_usage".to_string(), 90.0);
        thresholds.insert("response_time_ms".to_string(), 1000.0);
        thresholds.insert("error_rate".to_string(), 5.0);
        thresholds.insert("database_connections".to_string(), 90.0);
        Ok(())
    }

    async fn collect_system_metrics(&self) -> Result<PerformanceMetrics> {
        let mut metrics = PerformanceMetrics::new();

        // Collect CPU metrics
        metrics.cpu_usage = self.get_cpu_usage().await?;

        // Collect memory metrics
        metrics.memory_usage = self.get_memory_metrics().await?;

        // Collect disk metrics
        metrics.disk_usage = self.get_disk_metrics().await?;

        // Collect network metrics
        metrics.network_io = self.get_network_metrics().await?;

        // Collect database metrics
        metrics.database_metrics = self.get_database_metrics().await?;

        // Collect request metrics
        metrics.request_metrics = self.get_request_metrics().await?;

        Ok(metrics)
    }

    async fn get_cpu_usage(&self) -> Result<f64> {
        // In a real implementation, this would use system APIs
        // For now, return a simulated value
        Ok(45.0)
    }

    async fn get_memory_metrics(&self) -> Result<MemoryMetrics> {
        // In a real implementation, this would use system APIs
        Ok(MemoryMetrics {
            used_mb: 2048,
            available_mb: 6144,
            total_mb: 8192,
            usage_percentage: 25.0,
            heap_size_mb: 1024,
            gc_count: 150,
        })
    }

    async fn get_disk_metrics(&self) -> Result<DiskMetrics> {
        // In a real implementation, this would use system APIs
        Ok(DiskMetrics {
            used_gb: 45,
            available_gb: 155,
            total_gb: 200,
            usage_percentage: 22.5,
            read_iops: 1200,
            write_iops: 800,
            read_throughput_mbps: 125.0,
            write_throughput_mbps: 85.0,
        })
    }

    async fn get_network_metrics(&self) -> Result<NetworkMetrics> {
        // In a real implementation, this would use system APIs
        Ok(NetworkMetrics {
            bytes_sent: 1024000,
            bytes_received: 2048000,
            packets_sent: 5000,
            packets_received: 7500,
            connections_active: 150,
            connections_idle: 25,
            bandwidth_utilization: 35.0,
        })
    }

    async fn get_database_metrics(&self) -> Result<DatabaseMetrics> {
        // In a real implementation, this would query the database
        Ok(DatabaseMetrics {
            connections_active: 15,
            connections_idle: 5,
            connections_max: 50,
            query_count: 10000,
            slow_query_count: 25,
            average_query_time_ms: 125.0,
            transaction_count: 5000,
            deadlock_count: 2,
            cache_hit_ratio: 92.5,
        })
    }

    async fn get_request_metrics(&self) -> Result<RequestMetrics> {
        // In a real implementation, this would collect from request handlers
        Ok(RequestMetrics {
            total_requests: 50000,
            successful_requests: 48500,
            failed_requests: 1500,
            average_response_time_ms: 250.0,
            p95_response_time_ms: 450.0,
            p99_response_time_ms: 750.0,
            requests_per_second: 125.0,
            concurrent_requests: 45,
        })
    }

    async fn store_metrics(&self, metrics: PerformanceMetrics) -> Result<()> {
        let mut history = self.metrics_history.write().await;
        
        if history.len() >= self.max_history_size {
            history.pop_front();
        }
        
        history.push_back(metrics);
        Ok(())
    }

    async fn check_alert_thresholds(&self, metrics: &PerformanceMetrics) -> Result<Vec<PerformanceAlert>> {
        let mut alerts = Vec::new();
        let thresholds = self.alert_thresholds.read().await;

        // Check CPU usage
        if let Some(&threshold) = thresholds.get("cpu_usage") {
            if metrics.cpu_usage > threshold {
                alerts.push(PerformanceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::CpuThreshold,
                    threshold,
                    current_value: metrics.cpu_usage,
                    component: "system".to_string(),
                    message: format!("CPU usage ({:.1}%) exceeds threshold ({:.1}%)", 
                                   metrics.cpu_usage, threshold),
                    created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                    escalation_level: 1,
                    auto_resolve: true,
                });
            }
        }

        // Check memory usage
        if let Some(&threshold) = thresholds.get("memory_usage") {
            if metrics.memory_usage.usage_percentage > threshold {
                alerts.push(PerformanceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::MemoryThreshold,
                    threshold,
                    current_value: metrics.memory_usage.usage_percentage,
                    component: "system".to_string(),
                    message: format!("Memory usage ({:.1}%) exceeds threshold ({:.1}%)", 
                                   metrics.memory_usage.usage_percentage, threshold),
                    created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                    escalation_level: 1,
                    auto_resolve: true,
                });
            }
        }

        // Check response time
        if let Some(&threshold) = thresholds.get("response_time_ms") {
            if metrics.request_metrics.average_response_time_ms > threshold {
                alerts.push(PerformanceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::ResponseTimeThreshold,
                    threshold,
                    current_value: metrics.request_metrics.average_response_time_ms,
                    component: "api".to_string(),
                    message: format!("Average response time ({:.1}ms) exceeds threshold ({:.1}ms)", 
                                   metrics.request_metrics.average_response_time_ms, threshold),
                    created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                    escalation_level: 1,
                    auto_resolve: true,
                });
            }
        }

        Ok(alerts)
    }
}

#[async_trait]
impl PerformanceMonitor for ProductionPerformanceMonitor {
    async fn collect_metrics(&self) -> Result<PerformanceMetrics> {
        let metrics = self.collect_system_metrics().await?;
        self.store_metrics(metrics.clone()).await?;
        Ok(metrics)
    }

    async fn detect_performance_issues(&self) -> Result<Vec<PerformanceIssue>> {
        let current_metrics = self.collect_system_metrics().await?;
        let history = self.metrics_history.read().await;
        let history_slice: Vec<PerformanceMetrics> = history.iter().cloned().collect();

        let mut all_issues = Vec::new();
        
        for detector in &self.issue_detectors {
            let issues = detector.detect_issues(&current_metrics, &history_slice).await;
            all_issues.extend(issues);
        }

        Ok(all_issues)
    }

    async fn trigger_auto_scaling(&self, metrics: &PerformanceMetrics) -> Result<Option<ScalingAction>> {
        // Check if auto-scaling is needed based on metrics
        if metrics.cpu_usage > 85.0 || metrics.memory_usage.usage_percentage > 90.0 {
            let scaling_action = ScalingAction {
                action_id: Uuid::new_v4(),
                action_type: ScalingActionType::ScaleOut,
                target_component: "application".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("scale_factor".to_string(), serde_json::Value::Number(serde_json::Number::from(2)));
                    params
                },
                triggered_by: PerformanceIssue {
                    id: Uuid::new_v4(),
                    issue_type: if metrics.cpu_usage > 85.0 { 
                        PerformanceIssueType::HighCpuUsage 
                    } else { 
                        PerformanceIssueType::HighMemoryUsage 
                    },
                    severity: IssueSeverity::High,
                    description: "High resource utilization detected".to_string(),
                    detected_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                    affected_component: "system".to_string(),
                    metrics_snapshot: metrics.clone(),
                    suggested_actions: vec!["Scale out application instances".to_string()],
                    auto_resolution_attempted: false,
                },
                estimated_impact: "Reduce resource utilization by 50%".to_string(),
                execution_time_estimate: Duration::from_secs(300),
            };
            
            return Ok(Some(scaling_action));
        }

        Ok(None)
    }

    async fn register_performance_alert(&self, alert: PerformanceAlert) -> Result<()> {
        let mut alerts = self.active_alerts.write().await;
        alerts.insert(alert.id, alert);
        Ok(())
    }

    async fn get_historical_metrics(&self, duration: Duration) -> Result<Vec<PerformanceMetrics>> {
        let history = self.metrics_history.read().await;
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .saturating_sub(duration.as_secs());

        let filtered_metrics: Vec<PerformanceMetrics> = history
            .iter()
            .filter(|m| m.timestamp >= cutoff_time)
            .cloned()
            .collect();

        Ok(filtered_metrics)
    }

    async fn start_monitoring(&self) -> Result<()> {
        let mut active = self.monitoring_active.lock().await;
        if *active {
            return Ok(());
        }
        *active = true;

        let monitor_clone = Arc::new(ProductionPerformanceMonitor {
            metrics_history: self.metrics_history.clone(),
            active_alerts: self.active_alerts.clone(),
            issue_detectors: Vec::new(), // We'll need to clone detectors properly in a real implementation
            alert_thresholds: self.alert_thresholds.clone(),
            monitoring_active: self.monitoring_active.clone(),
            collection_interval: self.collection_interval,
            max_history_size: self.max_history_size,
        });
        let interval_duration = self.collection_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                let is_active = {
                    let active = monitor_clone.monitoring_active.lock().await;
                    *active
                };
                
                if !is_active {
                    break;
                }

                match monitor_clone.collect_metrics().await {
                    Ok(metrics) => {
                        // Check for alerts
                        if let Ok(alerts) = monitor_clone.check_alert_thresholds(&metrics).await {
                            for alert in alerts {
                                if let Err(e) = monitor_clone.register_performance_alert(alert).await {
                                    error!("Failed to register performance alert: {}", e);
                                }
                            }
                        }

                        // Check for performance issues
                        if let Ok(issues) = monitor_clone.detect_performance_issues().await {
                            for issue in issues {
                                warn!("Performance issue detected: {} - {}", 
                                     issue.issue_type, issue.description);
                            }
                        }

                        // Check for auto-scaling opportunities
                        if let Ok(Some(scaling_action)) = monitor_clone.trigger_auto_scaling(&metrics).await {
                            info!("Auto-scaling action triggered: {:?} for component: {}", 
                                 scaling_action.action_type, scaling_action.target_component);
                        }
                    }
                    Err(e) => {
                        error!("Failed to collect performance metrics: {}", e);
                    }
                }
            }
        });

        info!("Performance monitoring started with interval: {:?}", interval_duration);
        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        let mut active = self.monitoring_active.lock().await;
        *active = false;
        info!("Performance monitoring stopped");
        Ok(())
    }
}

impl CpuUsageDetector {
    pub fn new(threshold: f64, sustained_duration: Duration) -> Self {
        Self {
            threshold,
            sustained_duration,
        }
    }
}

#[async_trait]
impl IssueDetector for CpuUsageDetector {
    async fn detect_issues(&self, metrics: &PerformanceMetrics, history: &[PerformanceMetrics]) -> Vec<PerformanceIssue> {
        let mut issues = Vec::new();

        if metrics.cpu_usage > self.threshold {
            // Check if this is sustained over the required duration
            let sustained_count = history
                .iter()
                .rev()
                .take(10) // Check last 10 measurements
                .filter(|m| m.cpu_usage > self.threshold)
                .count();

            if sustained_count >= 5 { // If more than half are above threshold
                issues.push(PerformanceIssue {
                    id: Uuid::new_v4(),
                    issue_type: PerformanceIssueType::HighCpuUsage,
                    severity: if metrics.cpu_usage > 95.0 { IssueSeverity::Critical } else { IssueSeverity::High },
                    description: format!("Sustained high CPU usage: {:.1}%", metrics.cpu_usage),
                    detected_at: metrics.timestamp,
                    affected_component: "system".to_string(),
                    metrics_snapshot: metrics.clone(),
                    suggested_actions: vec![
                        "Consider scaling out application instances".to_string(),
                        "Review CPU-intensive operations".to_string(),
                        "Check for infinite loops or blocking operations".to_string(),
                    ],
                    auto_resolution_attempted: false,
                });
            }
        }

        issues
    }

    fn get_detector_name(&self) -> &str {
        "CpuUsageDetector"
    }
}

impl MemoryUsageDetector {
    pub fn new(threshold: f64, leak_detection_enabled: bool) -> Self {
        Self {
            threshold,
            leak_detection_enabled,
        }
    }
}

#[async_trait]
impl IssueDetector for MemoryUsageDetector {
    async fn detect_issues(&self, metrics: &PerformanceMetrics, history: &[PerformanceMetrics]) -> Vec<PerformanceIssue> {
        let mut issues = Vec::new();

        if metrics.memory_usage.usage_percentage > self.threshold {
            issues.push(PerformanceIssue {
                id: Uuid::new_v4(),
                issue_type: PerformanceIssueType::HighMemoryUsage,
                severity: if metrics.memory_usage.usage_percentage > 95.0 { 
                    IssueSeverity::Critical 
                } else { 
                    IssueSeverity::High 
                },
                description: format!("High memory usage: {:.1}%", metrics.memory_usage.usage_percentage),
                detected_at: metrics.timestamp,
                affected_component: "system".to_string(),
                metrics_snapshot: metrics.clone(),
                suggested_actions: vec![
                    "Review memory-intensive operations".to_string(),
                    "Check for memory leaks".to_string(),
                    "Consider increasing available memory".to_string(),
                ],
                auto_resolution_attempted: false,
            });
        }

        // Memory leak detection
        if self.leak_detection_enabled && history.len() >= 10 {
            let recent_usage: Vec<f64> = history
                .iter()
                .rev()
                .take(10)
                .map(|m| m.memory_usage.usage_percentage)
                .collect();

            // Check for consistent upward trend
            let mut increasing_count = 0;
            for window in recent_usage.windows(2) {
                if window[0] < window[1] {
                    increasing_count += 1;
                }
            }

            if increasing_count >= 7 { // 70% of measurements show increase
                issues.push(PerformanceIssue {
                    id: Uuid::new_v4(),
                    issue_type: PerformanceIssueType::ResourceLeak,
                    severity: IssueSeverity::Medium,
                    description: "Potential memory leak detected - consistent memory usage increase".to_string(),
                    detected_at: metrics.timestamp,
                    affected_component: "application".to_string(),
                    metrics_snapshot: metrics.clone(),
                    suggested_actions: vec![
                        "Investigate memory allocation patterns".to_string(),
                        "Review object lifecycle management".to_string(),
                        "Run memory profiling tools".to_string(),
                    ],
                    auto_resolution_attempted: false,
                });
            }
        }

        issues
    }

    fn get_detector_name(&self) -> &str {
        "MemoryUsageDetector"
    }
}

impl ResponseTimeDetector {
    pub fn new(threshold_ms: f64, percentile_threshold: f64) -> Self {
        Self {
            threshold_ms,
            percentile_threshold,
        }
    }
}

#[async_trait]
impl IssueDetector for ResponseTimeDetector {
    async fn detect_issues(&self, metrics: &PerformanceMetrics, _history: &[PerformanceMetrics]) -> Vec<PerformanceIssue> {
        let mut issues = Vec::new();

        let response_time_to_check = if self.percentile_threshold >= 99.0 {
            metrics.request_metrics.p99_response_time_ms
        } else if self.percentile_threshold >= 95.0 {
            metrics.request_metrics.p95_response_time_ms
        } else {
            metrics.request_metrics.average_response_time_ms
        };

        if response_time_to_check > self.threshold_ms {
            issues.push(PerformanceIssue {
                id: Uuid::new_v4(),
                issue_type: PerformanceIssueType::HighResponseTimes,
                severity: if response_time_to_check > self.threshold_ms * 2.0 { 
                    IssueSeverity::High 
                } else { 
                    IssueSeverity::Medium 
                },
                description: format!("High response times detected: {:.1}ms (P{:.0})", 
                                   response_time_to_check, self.percentile_threshold),
                detected_at: metrics.timestamp,
                affected_component: "api".to_string(),
                metrics_snapshot: metrics.clone(),
                suggested_actions: vec![
                    "Review slow API endpoints".to_string(),
                    "Check database query performance".to_string(),
                    "Consider implementing caching".to_string(),
                    "Review external service dependencies".to_string(),
                ],
                auto_resolution_attempted: false,
            });
        }

        issues
    }

    fn get_detector_name(&self) -> &str {
        "ResponseTimeDetector"
    }
}

impl DatabasePerformanceDetector {
    pub fn new(slow_query_threshold_ms: f64, connection_pool_threshold: f64) -> Self {
        Self {
            slow_query_threshold_ms,
            connection_pool_threshold,
        }
    }
}

#[async_trait]
impl IssueDetector for DatabasePerformanceDetector {
    async fn detect_issues(&self, metrics: &PerformanceMetrics, _history: &[PerformanceMetrics]) -> Vec<PerformanceIssue> {
        let mut issues = Vec::new();

        // Check for slow queries
        if metrics.database_metrics.average_query_time_ms > self.slow_query_threshold_ms {
            issues.push(PerformanceIssue {
                id: Uuid::new_v4(),
                issue_type: PerformanceIssueType::SlowDatabaseQueries,
                severity: IssueSeverity::Medium,
                description: format!("Slow database queries detected: {:.1}ms average", 
                                   metrics.database_metrics.average_query_time_ms),
                detected_at: metrics.timestamp,
                affected_component: "database".to_string(),
                metrics_snapshot: metrics.clone(),
                suggested_actions: vec![
                    "Review and optimize slow queries".to_string(),
                    "Check database indexes".to_string(),
                    "Consider query result caching".to_string(),
                ],
                auto_resolution_attempted: false,
            });
        }

        // Check connection pool utilization
        let pool_utilization = (metrics.database_metrics.connections_active as f64 / 
                               metrics.database_metrics.connections_max as f64) * 100.0;
        
        if pool_utilization > self.connection_pool_threshold {
            issues.push(PerformanceIssue {
                id: Uuid::new_v4(),
                issue_type: PerformanceIssueType::ConnectionPoolExhaustion,
                severity: if pool_utilization > 95.0 { IssueSeverity::High } else { IssueSeverity::Medium },
                description: format!("High database connection pool utilization: {:.1}%", pool_utilization),
                detected_at: metrics.timestamp,
                affected_component: "database".to_string(),
                metrics_snapshot: metrics.clone(),
                suggested_actions: vec![
                    "Increase database connection pool size".to_string(),
                    "Review connection lifecycle management".to_string(),
                    "Check for connection leaks".to_string(),
                ],
                auto_resolution_attempted: false,
            });
        }

        issues
    }

    fn get_detector_name(&self) -> &str {
        "DatabasePerformanceDetector"
    }
}