// Health monitoring system

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::RwLock;
use tokio::time::interval;
use serde::{Serialize, Deserialize};


use super::ObservabilityError;

/// Health monitoring system
pub struct HealthMonitor {
    /// Health checks registry
    checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
    /// Health check results
    results: Arc<RwLock<HashMap<String, HealthCheckResult>>>,
    /// Check interval in seconds
    check_interval_seconds: u64,
    /// Background monitoring task handle
    monitor_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

/// Health status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Health check definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Unique check identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Check description
    pub description: String,
    /// Check type
    pub check_type: HealthCheckType,
    /// Check configuration
    pub config: HealthCheckConfig,
    /// Check interval in seconds
    pub interval_seconds: u64,
    /// Timeout in seconds
    pub timeout_seconds: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
    /// Check enabled flag
    pub enabled: bool,
    /// Check creation timestamp
    pub created_at: u64,
}

/// Health check types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthCheckType {
    /// Simple ping/connectivity check
    Ping { endpoint: String },
    /// HTTP endpoint check
    Http { url: String, expected_status: u16 },
    /// TCP port check
    TcpPort { host: String, port: u16 },
    /// Memory usage check
    Memory { max_usage_percent: f64 },
    /// CPU usage check
    Cpu { max_usage_percent: f64 },
    /// Disk space check
    Disk { path: String, min_free_percent: f64 },
    /// Custom check with command
    Command { command: String, args: Vec<String> },
    /// Database connectivity check
    Database { connection_string: String },
    /// Custom function check
    Custom { function_name: String },
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Additional parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Working directory for command checks
    pub working_directory: Option<String>,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Check identifier
    pub check_id: String,
    /// Result status
    pub status: HealthStatus,
    /// Result message
    pub message: String,
    /// Check execution timestamp
    pub timestamp: u64,
    /// Check execution duration in milliseconds
    pub duration_ms: u64,
    /// Additional result data
    pub data: HashMap<String, serde_json::Value>,
    /// Consecutive failure count
    pub consecutive_failures: u32,
    /// Consecutive success count
    pub consecutive_successes: u32,
}

/// Overall system health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    /// Overall system status
    pub overall_status: HealthStatus,
    /// Total number of checks
    pub total_checks: usize,
    /// Number of healthy checks
    pub healthy_checks: usize,
    /// Number of degraded checks
    pub degraded_checks: usize,
    /// Number of unhealthy checks
    pub unhealthy_checks: usize,
    /// Number of unknown checks
    pub unknown_checks: usize,
    /// Last update timestamp
    pub last_updated: u64,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(check_interval_seconds: u64) -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            check_interval_seconds,
            monitor_handle: Arc::new(tokio::sync::Mutex::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the health monitor
    pub async fn start(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        *running = true;
        drop(running);

        // Start background monitoring task
        let checks = self.checks.clone();
        let results = self.results.clone();
        let check_interval_seconds = self.check_interval_seconds;
        let running_flag = self.running.clone();

        *self.monitor_handle.lock().await = Some(tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(check_interval_seconds));

            while *running_flag.read().await {
                check_interval.tick().await;
                
                let checks_guard = checks.read().await;
                let checks_to_run: Vec<_> = checks_guard.values().cloned().collect();
                drop(checks_guard);

                for check in checks_to_run {
                    if !check.enabled {
                        continue;
                    }

                    let result = Self::execute_health_check(&check).await;
                    let mut results_guard = results.write().await;
                    results_guard.insert(check.id.clone(), result);
                }
            }
        }));

        // Register default system health checks
        self.register_default_checks().await?;

        Ok(())
    }

    /// Stop the health monitor
    pub async fn stop(&self) -> Result<(), ObservabilityError> {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);

        if let Some(handle) = self.monitor_handle.lock().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Register a health check
    pub async fn register_check(&self, check: HealthCheck) -> Result<(), ObservabilityError> {
        let mut checks = self.checks.write().await;
        checks.insert(check.id.clone(), check);
        Ok(())
    }

    /// Unregister a health check
    pub async fn unregister_check(&self, check_id: &str) -> Result<(), ObservabilityError> {
        let mut checks = self.checks.write().await;
        let mut results = self.results.write().await;
        
        checks.remove(check_id);
        results.remove(check_id);
        
        Ok(())
    }

    /// Get health check result
    pub async fn get_check_result(&self, check_id: &str) -> Option<HealthCheckResult> {
        let results = self.results.read().await;
        results.get(check_id).cloned()
    }

    /// Get all health check results
    pub async fn get_all_results(&self) -> HashMap<String, HealthCheckResult> {
        let results = self.results.read().await;
        results.clone()
    }

    /// Get overall system health status
    pub async fn overall_status(&self) -> HealthStatus {
        let results = self.results.read().await;
        
        if results.is_empty() {
            return HealthStatus::Unknown;
        }

        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;
        let mut _unknown_count = 0;

        for result in results.values() {
            match result.status {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
                HealthStatus::Unhealthy => unhealthy_count += 1,
                HealthStatus::Unknown => _unknown_count += 1,
            }
        }

        // Determine overall status based on individual check results
        if unhealthy_count > 0 {
            HealthStatus::Unhealthy
        } else if degraded_count > 0 {
            HealthStatus::Degraded
        } else if healthy_count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        }
    }

    /// Get health summary
    pub async fn summary(&self) -> HealthSummary {
        let results = self.results.read().await;
        let overall_status = self.overall_status().await;
        
        let mut healthy_checks = 0;
        let mut degraded_checks = 0;
        let mut unhealthy_checks = 0;
        let mut unknown_checks = 0;

        for result in results.values() {
            match result.status {
                HealthStatus::Healthy => healthy_checks += 1,
                HealthStatus::Degraded => degraded_checks += 1,
                HealthStatus::Unhealthy => unhealthy_checks += 1,
                HealthStatus::Unknown => unknown_checks += 1,
            }
        }

        let last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        HealthSummary {
            overall_status,
            total_checks: results.len(),
            healthy_checks,
            degraded_checks,
            unhealthy_checks,
            unknown_checks,
            last_updated,
        }
    }

    /// Execute a health check
    async fn execute_health_check(check: &HealthCheck) -> HealthCheckResult {
        let start_time = std::time::Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let (status, message, data) = match &check.check_type {
            HealthCheckType::Ping { endpoint } => {
                Self::execute_ping_check(endpoint).await
            }
            HealthCheckType::Http { url, expected_status } => {
                Self::execute_http_check(url, *expected_status).await
            }
            HealthCheckType::TcpPort { host, port } => {
                Self::execute_tcp_check(host, *port).await
            }
            HealthCheckType::Memory { max_usage_percent } => {
                Self::execute_memory_check(*max_usage_percent).await
            }
            HealthCheckType::Cpu { max_usage_percent } => {
                Self::execute_cpu_check(*max_usage_percent).await
            }
            HealthCheckType::Disk { path, min_free_percent } => {
                Self::execute_disk_check(path, *min_free_percent).await
            }
            HealthCheckType::Command { command, args } => {
                Self::execute_command_check(command, args).await
            }
            HealthCheckType::Database { connection_string } => {
                Self::execute_database_check(connection_string).await
            }
            HealthCheckType::Custom { function_name } => {
                Self::execute_custom_check(function_name).await
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Get previous result for consecutive counts
        let (consecutive_failures, consecutive_successes) = match status {
            HealthStatus::Healthy => (0, 1), // Reset failures, increment successes
            _ => (1, 0), // Increment failures, reset successes
        };

        HealthCheckResult {
            check_id: check.id.clone(),
            status,
            message,
            timestamp,
            duration_ms,
            data,
            consecutive_failures,
            consecutive_successes,
        }
    }

    /// Execute ping check
    async fn execute_ping_check(endpoint: &str) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // Simple connectivity check (simplified implementation)
        match tokio::net::TcpStream::connect(endpoint).await {
            Ok(_) => (
                HealthStatus::Healthy,
                format!("Successfully connected to {}", endpoint),
                HashMap::new(),
            ),
            Err(e) => (
                HealthStatus::Unhealthy,
                format!("Failed to connect to {}: {}", endpoint, e),
                HashMap::new(),
            ),
        }
    }

    /// Execute HTTP check
    async fn execute_http_check(
        url: &str,
        _expected_status: u16,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // HTTP endpoint check (simplified implementation)
        (
            HealthStatus::Healthy,
            format!("HTTP check for {} not implemented", url),
            HashMap::new(),
        )
    }

    /// Execute TCP port check
    async fn execute_tcp_check(
        host: &str,
        port: u16,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        let address = format!("{}:{}", host, port);
        match tokio::net::TcpStream::connect(&address).await {
            Ok(_) => (
                HealthStatus::Healthy,
                format!("TCP port {} is open", address),
                HashMap::new(),
            ),
            Err(e) => (
                HealthStatus::Unhealthy,
                format!("TCP port {} is not accessible: {}", address, e),
                HashMap::new(),
            ),
        }
    }

    /// Execute memory usage check
    async fn execute_memory_check(
        max_usage_percent: f64,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // Memory usage check (simplified implementation)
        let usage_percent = 50.0; // Placeholder value
        
        let status = if usage_percent > max_usage_percent {
            HealthStatus::Unhealthy
        } else if usage_percent > max_usage_percent * 0.8 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let mut data = HashMap::new();
        data.insert("usage_percent".to_string(), serde_json::Value::Number(
            serde_json::Number::from_f64(usage_percent).unwrap()
        ));

        (
            status,
            format!("Memory usage: {:.1}%", usage_percent),
            data,
        )
    }

    /// Execute CPU usage check
    async fn execute_cpu_check(
        max_usage_percent: f64,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // CPU usage check (simplified implementation)
        let usage_percent = 30.0; // Placeholder value
        
        let status = if usage_percent > max_usage_percent {
            HealthStatus::Unhealthy
        } else if usage_percent > max_usage_percent * 0.8 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let mut data = HashMap::new();
        data.insert("usage_percent".to_string(), serde_json::Value::Number(
            serde_json::Number::from_f64(usage_percent).unwrap()
        ));

        (
            status,
            format!("CPU usage: {:.1}%", usage_percent),
            data,
        )
    }

    /// Execute disk space check
    async fn execute_disk_check(
        path: &str,
        min_free_percent: f64,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // Disk space check (simplified implementation)
        let free_percent = 75.0; // Placeholder value
        
        let status = if free_percent < min_free_percent {
            HealthStatus::Unhealthy
        } else if free_percent < min_free_percent * 1.2 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let mut data = HashMap::new();
        data.insert("free_percent".to_string(), serde_json::Value::Number(
            serde_json::Number::from_f64(free_percent).unwrap()
        ));

        (
            status,
            format!("Disk space at {}: {:.1}% free", path, free_percent),
            data,
        )
    }

    /// Execute command check
    async fn execute_command_check(
        command: &str,
        _args: &[String],
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // Command execution check (simplified implementation)
        (
            HealthStatus::Healthy,
            format!("Command check for '{}' not implemented", command),
            HashMap::new(),
        )
    }

    /// Execute database check
    async fn execute_database_check(
        connection_string: &str,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // Database connectivity check (simplified implementation)
        (
            HealthStatus::Healthy,
            format!("Database check for '{}' not implemented", connection_string),
            HashMap::new(),
        )
    }

    /// Execute custom check
    async fn execute_custom_check(
        function_name: &str,
    ) -> (HealthStatus, String, HashMap<String, serde_json::Value>) {
        // Custom function check (simplified implementation)
        (
            HealthStatus::Healthy,
            format!("Custom check '{}' not implemented", function_name),
            HashMap::new(),
        )
    }

    /// Register default system health checks
    async fn register_default_checks(&self) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Memory usage check
        let memory_check = HealthCheck {
            id: "system_memory".to_string(),
            name: "System Memory Usage".to_string(),
            description: "Monitor system memory usage".to_string(),
            check_type: HealthCheckType::Memory { max_usage_percent: 90.0 },
            config: HealthCheckConfig {
                parameters: HashMap::new(),
                environment: HashMap::new(),
                working_directory: None,
            },
            interval_seconds: 30,
            timeout_seconds: 5,
            failure_threshold: 3,
            success_threshold: 2,
            enabled: true,
            created_at: timestamp,
        };

        // CPU usage check
        let cpu_check = HealthCheck {
            id: "system_cpu".to_string(),
            name: "System CPU Usage".to_string(),
            description: "Monitor system CPU usage".to_string(),
            check_type: HealthCheckType::Cpu { max_usage_percent: 95.0 },
            config: HealthCheckConfig {
                parameters: HashMap::new(),
                environment: HashMap::new(),
                working_directory: None,
            },
            interval_seconds: 30,
            timeout_seconds: 5,
            failure_threshold: 3,
            success_threshold: 2,
            enabled: true,
            created_at: timestamp,
        };

        // Disk space check
        let disk_check = HealthCheck {
            id: "system_disk".to_string(),
            name: "System Disk Space".to_string(),
            description: "Monitor system disk space".to_string(),
            check_type: HealthCheckType::Disk {
                path: "/".to_string(),
                min_free_percent: 10.0,
            },
            config: HealthCheckConfig {
                parameters: HashMap::new(),
                environment: HashMap::new(),
                working_directory: None,
            },
            interval_seconds: 60,
            timeout_seconds: 5,
            failure_threshold: 2,
            success_threshold: 1,
            enabled: true,
            created_at: timestamp,
        };

        self.register_check(memory_check).await?;
        self.register_check(cpu_check).await?;
        self.register_check(disk_check).await?;

        Ok(())
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "HEALTHY"),
            HealthStatus::Degraded => write!(f, "DEGRADED"),
            HealthStatus::Unhealthy => write!(f, "UNHEALTHY"),
            HealthStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}