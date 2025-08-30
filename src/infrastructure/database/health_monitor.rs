use crate::error::{AppError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    bson::{doc, Document},
    options::FindOptions,
    Client, Database,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{RwLock, Notify},
    time::{interval, sleep},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Database health monitoring configuration
#[derive(Debug, Clone)]
pub struct DatabaseHealthConfig {
    pub health_check_interval_ms: u64,
    pub slow_query_threshold_ms: u64,
    pub connection_timeout_ms: u64,
    pub max_health_history: usize,
    pub alert_threshold_error_rate: f64,
    pub alert_threshold_response_time_ms: f64,
    pub enable_slow_query_logging: bool,
    pub enable_performance_metrics: bool,
}

impl Default for DatabaseHealthConfig {
    fn default() -> Self {
        Self {
            health_check_interval_ms: 30000,  // 30 seconds
            slow_query_threshold_ms: 1000,    // 1 second
            connection_timeout_ms: 5000,      // 5 seconds
            max_health_history: 100,          // Keep last 100 health checks
            alert_threshold_error_rate: 0.05, // 5% error rate
            alert_threshold_response_time_ms: 2000.0, // 2 seconds
            enable_slow_query_logging: true,
            enable_performance_metrics: true,
        }
    }
}

/// Database health status with detailed metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealthStatus {
    pub is_healthy: bool,
    pub timestamp: DateTime<Utc>,
    pub connection_status: ConnectionStatus,
    pub performance_metrics: PerformanceMetrics,
    pub query_metrics: QueryMetrics,
    pub error_metrics: ErrorMetrics,
    pub resource_usage: ResourceUsage,
    pub alerts: Vec<HealthAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub is_connected: bool,
    pub active_connections: u32,
    pub max_connections: u32,
    pub connection_pool_utilization: f64,
    pub connection_errors: u64,
    pub last_connection_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub average_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub operations_per_second: f64,
    pub slow_queries_count: u64,
    pub total_operations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetrics {
    pub read_operations: u64,
    pub write_operations: u64,
    pub transaction_operations: u64,
    pub failed_operations: u64,
    pub average_query_time_ms: f64,
    pub slow_queries: Vec<SlowQueryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub total_errors: u64,
    pub error_rate: f64,
    pub connection_errors: u64,
    pub timeout_errors: u64,
    pub query_errors: u64,
    pub recent_errors: Vec<ErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    pub disk_usage_mb: Option<f64>,
    pub network_io_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub id: Uuid,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    HighErrorRate,
    SlowQueries,
    ConnectionIssues,
    HighResourceUsage,
    DatabaseUnresponsive,
    PerformanceDegradation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryInfo {
    pub query_id: Uuid,
    pub query_type: String,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
    pub collection: Option<String>,
    pub query_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub error_id: Uuid,
    pub error_type: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub operation: Option<String>,
    pub collection: Option<String>,
}

/// Database health monitoring trait
#[async_trait]
pub trait DatabaseHealthMonitor: Send + Sync {
    async fn start_monitoring(&self) -> Result<()>;
    async fn stop_monitoring(&self) -> Result<()>;
    async fn get_health_status(&self) -> Result<DatabaseHealthStatus>;
    async fn get_health_history(&self) -> Result<Vec<DatabaseHealthStatus>>;
    async fn perform_health_check(&self) -> Result<DatabaseHealthStatus>;
    async fn get_slow_queries(&self, limit: usize) -> Result<Vec<SlowQueryInfo>>;
    async fn get_recent_errors(&self, limit: usize) -> Result<Vec<ErrorInfo>>;
    async fn clear_alerts(&self) -> Result<()>;
}

/// Production database health monitor implementation
pub struct ProductionDatabaseHealthMonitor {
    client: Client,
    database: Database,
    config: DatabaseHealthConfig,
    is_monitoring: AtomicBool,
    health_history: Arc<RwLock<VecDeque<DatabaseHealthStatus>>>,
    slow_queries: Arc<RwLock<VecDeque<SlowQueryInfo>>>,
    recent_errors: Arc<RwLock<VecDeque<ErrorInfo>>>,
    active_alerts: Arc<RwLock<HashMap<Uuid, HealthAlert>>>,
    operation_counter: AtomicU64,
    error_counter: AtomicU64,
    response_times: Arc<RwLock<VecDeque<Duration>>>,
    shutdown_notify: Arc<Notify>,
}

impl ProductionDatabaseHealthMonitor {
    /// Create a new database health monitor
    pub async fn new(
        client: Client,
        database: Database,
        config: DatabaseHealthConfig,
    ) -> Result<Self> {
        info!("🔄 Initializing Database Health Monitor...");

        let monitor = Self {
            client,
            database,
            config,
            is_monitoring: AtomicBool::new(false),
            health_history: Arc::new(RwLock::new(VecDeque::new())),
            slow_queries: Arc::new(RwLock::new(VecDeque::new())),
            recent_errors: Arc::new(RwLock::new(VecDeque::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            operation_counter: AtomicU64::new(0),
            error_counter: AtomicU64::new(0),
            response_times: Arc::new(RwLock::new(VecDeque::new())),
            shutdown_notify: Arc::new(Notify::new()),
        };

        // Perform initial health check
        let initial_status = monitor.perform_health_check().await?;
        info!("✅ Database Health Monitor initialized - Status: {}", 
              if initial_status.is_healthy { "Healthy" } else { "Unhealthy" });

        Ok(monitor)
    }

    /// Record a database operation for monitoring
    pub async fn record_operation(&self, duration: Duration, success: bool, operation_type: &str) {
        self.operation_counter.fetch_add(1, Ordering::SeqCst);
        
        if !success {
            self.error_counter.fetch_add(1, Ordering::SeqCst);
        }

        // Record response time
        {
            let mut response_times = self.response_times.write().await;
            response_times.push_back(duration);
            
            // Keep only recent response times (last 1000 operations)
            if response_times.len() > 1000 {
                response_times.pop_front();
            }
        }

        // Check for slow queries
        if self.config.enable_slow_query_logging && 
           duration.as_millis() as u64 > self.config.slow_query_threshold_ms {
            self.record_slow_query(duration, operation_type).await;
        }
    }

    /// Record a slow query
    async fn record_slow_query(&self, duration: Duration, operation_type: &str) {
        let slow_query = SlowQueryInfo {
            query_id: Uuid::new_v4(),
            query_type: operation_type.to_string(),
            duration_ms: duration.as_millis() as u64,
            timestamp: Utc::now(),
            collection: None, // Could be enhanced to capture collection name
            query_pattern: None, // Could be enhanced to capture query pattern
        };

        let mut slow_queries = self.slow_queries.write().await;
        slow_queries.push_back(slow_query);

        // Keep only recent slow queries
        if slow_queries.len() > 100 {
            slow_queries.pop_front();
        }

        warn!("Slow query detected: {} took {:?}", operation_type, duration);
    }

    /// Record a database error
    pub async fn record_error(&self, error: &AppError, operation: Option<&str>) {
        let error_info = ErrorInfo {
            error_id: Uuid::new_v4(),
            error_type: "DatabaseError".to_string(),
            message: error.to_string(),
            timestamp: Utc::now(),
            operation: operation.map(|s| s.to_string()),
            collection: None,
        };

        let mut recent_errors = self.recent_errors.write().await;
        recent_errors.push_back(error_info);

        // Keep only recent errors
        if recent_errors.len() > 100 {
            recent_errors.pop_front();
        }
    }

    /// Calculate performance metrics
    async fn calculate_performance_metrics(&self) -> PerformanceMetrics {
        let response_times = self.response_times.read().await;
        let total_operations = self.operation_counter.load(Ordering::SeqCst);
        let slow_queries_count = self.slow_queries.read().await.len() as u64;

        if response_times.is_empty() {
            return PerformanceMetrics {
                average_response_time_ms: 0.0,
                p95_response_time_ms: 0.0,
                p99_response_time_ms: 0.0,
                operations_per_second: 0.0,
                slow_queries_count,
                total_operations,
            };
        }

        let mut times: Vec<f64> = response_times
            .iter()
            .map(|d| d.as_millis() as f64)
            .collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let average = times.iter().sum::<f64>() / times.len() as f64;
        let p95_index = (times.len() as f64 * 0.95) as usize;
        let p99_index = (times.len() as f64 * 0.99) as usize;
        
        let p95 = times.get(p95_index.min(times.len() - 1)).copied().unwrap_or(0.0);
        let p99 = times.get(p99_index.min(times.len() - 1)).copied().unwrap_or(0.0);

        // Calculate operations per second (rough estimate)
        let ops_per_second = if !times.is_empty() {
            times.len() as f64 / 60.0 // Assuming we keep 1 minute of data
        } else {
            0.0
        };

        PerformanceMetrics {
            average_response_time_ms: average,
            p95_response_time_ms: p95,
            p99_response_time_ms: p99,
            operations_per_second: ops_per_second,
            slow_queries_count,
            total_operations,
        }
    }

    /// Calculate error metrics
    async fn calculate_error_metrics(&self) -> ErrorMetrics {
        let total_operations = self.operation_counter.load(Ordering::SeqCst);
        let total_errors = self.error_counter.load(Ordering::SeqCst);
        let recent_errors = self.recent_errors.read().await;

        let error_rate = if total_operations > 0 {
            total_errors as f64 / total_operations as f64
        } else {
            0.0
        };

        ErrorMetrics {
            total_errors,
            error_rate,
            connection_errors: 0, // Could be enhanced to track specific error types
            timeout_errors: 0,
            query_errors: total_errors,
            recent_errors: recent_errors.iter().cloned().collect(),
        }
    }

    /// Check for alerts based on current metrics
    async fn check_for_alerts(&self, status: &DatabaseHealthStatus) -> Vec<HealthAlert> {
        let mut alerts = Vec::new();

        // Check error rate
        if status.error_metrics.error_rate > self.config.alert_threshold_error_rate {
            alerts.push(HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::HighErrorRate,
                severity: AlertSeverity::High,
                message: format!(
                    "High error rate detected: {:.2}% (threshold: {:.2}%)",
                    status.error_metrics.error_rate * 100.0,
                    self.config.alert_threshold_error_rate * 100.0
                ),
                timestamp: Utc::now(),
                resolved: false,
                metadata: HashMap::new(),
            });
        }

        // Check response time
        if status.performance_metrics.average_response_time_ms > self.config.alert_threshold_response_time_ms {
            alerts.push(HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::PerformanceDegradation,
                severity: AlertSeverity::Medium,
                message: format!(
                    "High response time detected: {:.2}ms (threshold: {:.2}ms)",
                    status.performance_metrics.average_response_time_ms,
                    self.config.alert_threshold_response_time_ms
                ),
                timestamp: Utc::now(),
                resolved: false,
                metadata: HashMap::new(),
            });
        }

        // Check slow queries
        if status.performance_metrics.slow_queries_count > 10 {
            alerts.push(HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::SlowQueries,
                severity: AlertSeverity::Medium,
                message: format!(
                    "High number of slow queries: {}",
                    status.performance_metrics.slow_queries_count
                ),
                timestamp: Utc::now(),
                resolved: false,
                metadata: HashMap::new(),
            });
        }

        // Check connection status
        if !status.connection_status.is_connected {
            alerts.push(HealthAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::DatabaseUnresponsive,
                severity: AlertSeverity::Critical,
                message: "Database connection lost".to_string(),
                timestamp: Utc::now(),
                resolved: false,
                metadata: HashMap::new(),
            });
        }

        alerts
    }

    /// Background monitoring task
    async fn monitoring_task(&self) {
        let mut interval = interval(Duration::from_millis(self.config.health_check_interval_ms));
        
        while self.is_monitoring.load(Ordering::SeqCst) {
            tokio::select! {
                _ = interval.tick() => {
                    match self.perform_health_check().await {
                        Ok(status) => {
                            // Store health status
                            {
                                let mut history = self.health_history.write().await;
                                history.push_back(status.clone());
                                
                                if history.len() > self.config.max_health_history {
                                    history.pop_front();
                                }
                            }

                            // Update active alerts
                            {
                                let mut active_alerts = self.active_alerts.write().await;
                                for alert in &status.alerts {
                                    active_alerts.insert(alert.id, alert.clone());
                                }
                            }

                            if !status.is_healthy {
                                warn!("Database health check failed: {:?}", status.alerts);
                            } else {
                                debug!("Database health check passed");
                            }
                        }
                        Err(e) => {
                            error!("Health check failed: {}", e);
                            self.record_error(&e, Some("health_check")).await;
                        }
                    }
                }
                _ = self.shutdown_notify.notified() => {
                    info!("Database health monitoring stopped");
                    break;
                }
            }
        }
    }
}

#[async_trait]
impl DatabaseHealthMonitor for ProductionDatabaseHealthMonitor {
    async fn start_monitoring(&self) -> Result<()> {
        if self.is_monitoring.swap(true, Ordering::SeqCst) {
            warn!("Database health monitoring is already running");
            return Ok(());
        }

        info!("🚀 Starting database health monitoring...");
        
        // Clone necessary data for the monitoring task
        let monitor = ProductionDatabaseHealthMonitor {
            client: self.client.clone(),
            database: self.database.clone(),
            config: self.config.clone(),
            is_monitoring: AtomicBool::new(self.is_monitoring.load(Ordering::SeqCst)),
            health_history: Arc::clone(&self.health_history),
            slow_queries: Arc::clone(&self.slow_queries),
            recent_errors: Arc::clone(&self.recent_errors),
            active_alerts: Arc::clone(&self.active_alerts),
            operation_counter: AtomicU64::new(self.operation_counter.load(Ordering::SeqCst)),
            error_counter: AtomicU64::new(self.error_counter.load(Ordering::SeqCst)),
            response_times: Arc::clone(&self.response_times),
            shutdown_notify: Arc::clone(&self.shutdown_notify),
        };

        tokio::spawn(async move {
            monitor.monitoring_task().await;
        });

        info!("✅ Database health monitoring started");
        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        if !self.is_monitoring.swap(false, Ordering::SeqCst) {
            warn!("Database health monitoring is not running");
            return Ok(());
        }

        info!("🛑 Stopping database health monitoring...");
        self.shutdown_notify.notify_one();
        
        // Give the monitoring task time to shut down gracefully
        sleep(Duration::from_millis(100)).await;
        
        info!("✅ Database health monitoring stopped");
        Ok(())
    }

    async fn get_health_status(&self) -> Result<DatabaseHealthStatus> {
        let history = self.health_history.read().await;
        match history.back() {
            Some(status) => Ok(status.clone()),
            None => self.perform_health_check().await,
        }
    }

    async fn get_health_history(&self) -> Result<Vec<DatabaseHealthStatus>> {
        let history = self.health_history.read().await;
        Ok(history.iter().cloned().collect())
    }

    async fn perform_health_check(&self) -> Result<DatabaseHealthStatus> {
        let start_time = Instant::now();
        
        // Test database connectivity
        let connection_status = match self.database.run_command(doc! { "ping": 1 }, None).await {
            Ok(_) => ConnectionStatus {
                is_connected: true,
                active_connections: 0, // MongoDB doesn't expose this easily
                max_connections: 100,  // From config
                connection_pool_utilization: 0.0,
                connection_errors: 0,
                last_connection_error: None,
            },
            Err(e) => {
                self.record_error(&AppError::DatabaseError(e.to_string()), Some("health_check")).await;
                ConnectionStatus {
                    is_connected: false,
                    active_connections: 0,
                    max_connections: 100,
                    connection_pool_utilization: 0.0,
                    connection_errors: 1,
                    last_connection_error: Some(e.to_string()),
                }
            }
        };

        let performance_metrics = self.calculate_performance_metrics().await;
        let error_metrics = self.calculate_error_metrics().await;
        
        let query_metrics = QueryMetrics {
            read_operations: 0, // Could be enhanced to track specific operation types
            write_operations: 0,
            transaction_operations: 0,
            failed_operations: error_metrics.total_errors,
            average_query_time_ms: performance_metrics.average_response_time_ms,
            slow_queries: self.slow_queries.read().await.iter().cloned().collect(),
        };

        let resource_usage = ResourceUsage {
            memory_usage_mb: None, // MongoDB doesn't expose this easily
            cpu_usage_percent: None,
            disk_usage_mb: None,
            network_io_mb: None,
        };

        let is_healthy = connection_status.is_connected && 
                        error_metrics.error_rate < self.config.alert_threshold_error_rate &&
                        performance_metrics.average_response_time_ms < self.config.alert_threshold_response_time_ms;

        let status = DatabaseHealthStatus {
            is_healthy,
            timestamp: Utc::now(),
            connection_status,
            performance_metrics,
            query_metrics,
            error_metrics,
            resource_usage,
            alerts: Vec::new(), // Will be populated below
        };

        let alerts = self.check_for_alerts(&status).await;
        let final_status = DatabaseHealthStatus {
            alerts,
            ..status
        };

        let duration = start_time.elapsed();
        self.record_operation(duration, final_status.is_healthy, "health_check").await;

        Ok(final_status)
    }

    async fn get_slow_queries(&self, limit: usize) -> Result<Vec<SlowQueryInfo>> {
        let slow_queries = self.slow_queries.read().await;
        Ok(slow_queries
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect())
    }

    async fn get_recent_errors(&self, limit: usize) -> Result<Vec<ErrorInfo>> {
        let recent_errors = self.recent_errors.read().await;
        Ok(recent_errors
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect())
    }

    async fn clear_alerts(&self) -> Result<()> {
        let mut active_alerts = self.active_alerts.write().await;
        active_alerts.clear();
        info!("All database health alerts cleared");
        Ok(())
    }
}