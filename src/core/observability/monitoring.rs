use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{DiskExt, System, SystemExt};
use tokio::sync::RwLock;
use tracing::info;

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Individual health check result
#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub duration_ms: u64,
    pub timestamp: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Overall health check response
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub version: String,
    pub checks: Vec<HealthCheckResult>,
    pub system_info: SystemInfo,
}

/// System information
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub cpu_count: usize,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_available_bytes: u64,
}

/// Health check trait
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthCheckResult;
    fn name(&self) -> &str;
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
}

/// Health monitor that manages all health checks
pub struct HealthMonitor {
    checks: Arc<RwLock<Vec<Box<dyn HealthCheck>>>>,
    start_time: Instant,
    version: String,
    metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
}

impl HealthMonitor {
    pub fn new(
        version: String,
        metrics_collector: Option<Arc<crate::core::observability::metrics::MetricsCollector>>,
    ) -> Self {
        Self {
            checks: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
            version,
            metrics_collector,
        }
    }

    /// Add a health check
    pub async fn add_check(&self, check: Box<dyn HealthCheck>) {
        let mut checks = self.checks.write().await;
        checks.push(check);
        info!("Added health check: {}", checks.last().unwrap().name());
    }

    /// Run all health checks
    pub async fn check_health(&self) -> HealthResponse {
        let checks = self.checks.read().await;
        let mut results = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        for check in checks.iter() {
            let start_time = Instant::now();

            let result = tokio::time::timeout(check.timeout(), check.check()).await;

            let check_result = match result {
                Ok(result) => {
                    // Record metrics
                    if let Some(metrics) = &self.metrics_collector {
                        let mut labels = HashMap::new();
                        labels.insert("check_name".to_string(), check.name().to_string());
                        labels.insert("status".to_string(), format!("{:?}", result.status));

                        metrics.increment_counter("health_check_total", labels.clone());
                        metrics.record_timing(
                            "health_check_duration_seconds",
                            start_time.elapsed(),
                            labels,
                        );
                    }

                    result
                }
                Err(_) => {
                    // Timeout occurred
                    HealthCheckResult {
                        name: check.name().to_string(),
                        status: HealthStatus::Unhealthy,
                        message: Some("Health check timed out".to_string()),
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        metadata: HashMap::new(),
                    }
                }
            };

            // Update overall status based on individual check results
            match check_result.status {
                HealthStatus::Unhealthy => overall_status = HealthStatus::Unhealthy,
                HealthStatus::Degraded if overall_status == HealthStatus::Healthy => {
                    overall_status = HealthStatus::Degraded
                }
                _ => {}
            }

            results.push(check_result);
        }

        HealthResponse {
            status: overall_status,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: self.version.clone(),
            checks: results,
            system_info: self.get_system_info(),
        }
    }

    /// Get system information
    fn get_system_info(&self) -> SystemInfo {
        let mut system = System::new_all();
        system.refresh_all();

        SystemInfo {
            hostname: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            platform: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            cpu_count: system.cpus().len(),
            memory_total_bytes: system.total_memory(),
            memory_available_bytes: system.available_memory(),
            disk_total_bytes: system.disks().iter().map(|d| d.total_space()).sum(),
            disk_available_bytes: system.disks().iter().map(|d| d.available_space()).sum(),
        }
    }

    /// Create health check router
    pub fn create_router(&self) -> Router<Arc<Self>> {
        Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .route("/metrics", get(metrics_handler))
    }
}

/// Database health check
pub struct DatabaseHealthCheck {
    database: Arc<mongodb::Database>,
}

impl DatabaseHealthCheck {
    pub fn new(database: Arc<mongodb::Database>) -> Self {
        Self { database }
    }
}

#[async_trait::async_trait]
impl HealthCheck for DatabaseHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match self
            .database
            .run_command(mongodb::bson::doc! { "ping": 1 }, None)
            .await
        {
            Ok(_) => HealthCheckResult {
                name: "database".to_string(),
                status: HealthStatus::Healthy,
                message: Some("Database connection is healthy".to_string()),
                duration_ms: start_time.elapsed().as_millis() as u64,
                timestamp,
                metadata: HashMap::new(),
            },
            Err(e) => HealthCheckResult {
                name: "database".to_string(),
                status: HealthStatus::Unhealthy,
                message: Some(format!("Database connection failed: {}", e)),
                duration_ms: start_time.elapsed().as_millis() as u64,
                timestamp,
                metadata: HashMap::new(),
            },
        }
    }

    fn name(&self) -> &str {
        "database"
    }
}

/// Memory health check
pub struct MemoryHealthCheck {
    warning_threshold_percent: f64,
    critical_threshold_percent: f64,
}

impl MemoryHealthCheck {
    pub fn new(warning_threshold_percent: f64, critical_threshold_percent: f64) -> Self {
        Self {
            warning_threshold_percent,
            critical_threshold_percent,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for MemoryHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut system = System::new_all();
        system.refresh_memory();

        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

        let mut metadata = HashMap::new();
        metadata.insert(
            "total_memory_bytes".to_string(),
            serde_json::Value::Number(total_memory.into()),
        );
        metadata.insert(
            "used_memory_bytes".to_string(),
            serde_json::Value::Number(used_memory.into()),
        );
        metadata.insert(
            "usage_percent".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(usage_percent)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );

        let (status, message) = if usage_percent >= self.critical_threshold_percent {
            (
                HealthStatus::Unhealthy,
                format!("Memory usage is critical: {:.1}%", usage_percent),
            )
        } else if usage_percent >= self.warning_threshold_percent {
            (
                HealthStatus::Degraded,
                format!("Memory usage is high: {:.1}%", usage_percent),
            )
        } else {
            (
                HealthStatus::Healthy,
                format!("Memory usage is normal: {:.1}%", usage_percent),
            )
        };

        HealthCheckResult {
            name: "memory".to_string(),
            status,
            message: Some(message),
            duration_ms: start_time.elapsed().as_millis() as u64,
            timestamp,
            metadata,
        }
    }

    fn name(&self) -> &str {
        "memory"
    }
}

/// Disk space health check
pub struct DiskHealthCheck {
    warning_threshold_percent: f64,
    critical_threshold_percent: f64,
}

impl DiskHealthCheck {
    pub fn new(warning_threshold_percent: f64, critical_threshold_percent: f64) -> Self {
        Self {
            warning_threshold_percent,
            critical_threshold_percent,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for DiskHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut system = System::new_all();
        system.refresh_disks();

        let mut total_space = 0u64;
        let mut available_space = 0u64;

        for disk in system.disks() {
            total_space += disk.total_space();
            available_space += disk.available_space();
        }

        let used_space = total_space - available_space;
        let usage_percent = if total_space > 0 {
            (used_space as f64 / total_space as f64) * 100.0
        } else {
            0.0
        };

        let mut metadata = HashMap::new();
        metadata.insert(
            "total_space_bytes".to_string(),
            serde_json::Value::Number(total_space.into()),
        );
        metadata.insert(
            "used_space_bytes".to_string(),
            serde_json::Value::Number(used_space.into()),
        );
        metadata.insert(
            "available_space_bytes".to_string(),
            serde_json::Value::Number(available_space.into()),
        );
        metadata.insert(
            "usage_percent".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(usage_percent)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );

        let (status, message) = if usage_percent >= self.critical_threshold_percent {
            (
                HealthStatus::Unhealthy,
                format!("Disk usage is critical: {:.1}%", usage_percent),
            )
        } else if usage_percent >= self.warning_threshold_percent {
            (
                HealthStatus::Degraded,
                format!("Disk usage is high: {:.1}%", usage_percent),
            )
        } else {
            (
                HealthStatus::Healthy,
                format!("Disk usage is normal: {:.1}%", usage_percent),
            )
        };

        HealthCheckResult {
            name: "disk".to_string(),
            status,
            message: Some(message),
            duration_ms: start_time.elapsed().as_millis() as u64,
            timestamp,
            metadata,
        }
    }

    fn name(&self) -> &str {
        "disk"
    }
}

/// External service health check
pub struct ExternalServiceHealthCheck {
    name: String,
    url: String,
    client: reqwest::Client,
    expected_status: u16,
}

impl ExternalServiceHealthCheck {
    pub fn new(name: String, url: String, expected_status: u16) -> Self {
        Self {
            name,
            url,
            client: reqwest::Client::new(),
            expected_status,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for ExternalServiceHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match self.client.get(&self.url).send().await {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let mut metadata = HashMap::new();
                metadata.insert(
                    "url".to_string(),
                    serde_json::Value::String(self.url.clone()),
                );
                metadata.insert(
                    "status_code".to_string(),
                    serde_json::Value::Number(status_code.into()),
                );

                let (status, message) = if status_code == self.expected_status {
                    (
                        HealthStatus::Healthy,
                        format!("External service {} is healthy", self.name),
                    )
                } else {
                    (
                        HealthStatus::Degraded,
                        format!(
                            "External service {} returned unexpected status: {}",
                            self.name, status_code
                        ),
                    )
                };

                HealthCheckResult {
                    name: self.name.clone(),
                    status,
                    message: Some(message),
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    timestamp,
                    metadata,
                }
            }
            Err(e) => HealthCheckResult {
                name: self.name.clone(),
                status: HealthStatus::Unhealthy,
                message: Some(format!(
                    "External service {} is unreachable: {}",
                    self.name, e
                )),
                duration_ms: start_time.elapsed().as_millis() as u64,
                timestamp,
                metadata: {
                    let mut metadata = HashMap::new();
                    metadata.insert(
                        "url".to_string(),
                        serde_json::Value::String(self.url.clone()),
                    );
                    metadata.insert(
                        "error".to_string(),
                        serde_json::Value::String(e.to_string()),
                    );
                    metadata
                },
            },
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
}

// HTTP handlers
async fn health_handler(
    State(monitor): State<Arc<HealthMonitor>>,
) -> std::result::Result<Json<HealthResponse>, StatusCode> {
    let health_response = monitor.check_health().await;

    let status_code = match health_response.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // Still return 200 for degraded
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    match status_code {
        StatusCode::OK => Ok(Json(health_response)),
        _ => Err(status_code),
    }
}

async fn liveness_handler() -> StatusCode {
    // Liveness probe - just check if the application is running
    StatusCode::OK
}

async fn readiness_handler(State(monitor): State<Arc<HealthMonitor>>) -> StatusCode {
    // Readiness probe - check if the application is ready to serve traffic
    let health_response = monitor.check_health().await;

    match health_response.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // Still ready to serve traffic
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn metrics_handler(
    State(monitor): State<Arc<HealthMonitor>>,
) -> std::result::Result<String, StatusCode> {
    // Return Prometheus-formatted metrics
    if let Some(metrics_collector) = &monitor.metrics_collector {
        let snapshot = metrics_collector.get_metrics_snapshot().await;

        // Convert to Prometheus format (simplified)
        let mut output = String::new();
        output.push_str("# HELP rustci_uptime_seconds Application uptime in seconds\n");
        output.push_str("# TYPE rustci_uptime_seconds counter\n");
        output.push_str(&format!(
            "rustci_uptime_seconds {}\n",
            snapshot.uptime_seconds
        ));

        output.push_str("# HELP rustci_memory_usage_bytes Memory usage in bytes\n");
        output.push_str("# TYPE rustci_memory_usage_bytes gauge\n");
        output.push_str(&format!(
            "rustci_memory_usage_bytes {}\n",
            snapshot.system_metrics.memory_usage_bytes
        ));

        output.push_str("# HELP rustci_cpu_usage_percent CPU usage percentage\n");
        output.push_str("# TYPE rustci_cpu_usage_percent gauge\n");
        output.push_str(&format!(
            "rustci_cpu_usage_percent {}\n",
            snapshot.system_metrics.cpu_usage_percent
        ));

        Ok(output)
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHealthCheck {
        name: String,
        status: HealthStatus,
    }

    impl MockHealthCheck {
        fn new(name: String, status: HealthStatus) -> Self {
            Self { name, status }
        }
    }

    #[async_trait::async_trait]
    impl HealthCheck for MockHealthCheck {
        async fn check(&self) -> HealthCheckResult {
            HealthCheckResult {
                name: self.name.clone(),
                status: self.status.clone(),
                message: Some("Mock health check".to_string()),
                duration_ms: 10,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                metadata: HashMap::new(),
            }
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_health_monitor() {
        let monitor = HealthMonitor::new("1.0.0".to_string(), None);

        monitor
            .add_check(Box::new(MockHealthCheck::new(
                "test1".to_string(),
                HealthStatus::Healthy,
            )))
            .await;

        monitor
            .add_check(Box::new(MockHealthCheck::new(
                "test2".to_string(),
                HealthStatus::Degraded,
            )))
            .await;

        let health_response = monitor.check_health().await;

        assert_eq!(health_response.status, HealthStatus::Degraded);
        assert_eq!(health_response.checks.len(), 2);
        assert_eq!(health_response.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_memory_health_check() {
        let check = MemoryHealthCheck::new(80.0, 95.0);
        let result = check.check().await;

        assert_eq!(result.name, "memory");
        assert!(result.duration_ms > 0);
        assert!(result.metadata.contains_key("usage_percent"));
    }
}

/// Simple health check handler for basic endpoints
pub async fn health_check_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let health_status = serde_json::json!({
        "status": "healthy",
        "message": "RustCI Server is running",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "environment": std::env::var("RUST_ENV").unwrap_or_else(|_| "development".into()),
    });

    Ok(Json(health_status))
}
