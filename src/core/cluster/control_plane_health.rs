use crate::error::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{CpuExt, DiskExt, NetworkExt, NetworksExt, System, SystemExt};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, instrument};


/// Control plane health monitoring service
pub struct ControlPlaneHealth {
    checks: Arc<RwLock<Vec<Box<dyn HealthCheck>>>>,
    start_time: Instant,
    system: Arc<RwLock<System>>,
    node_health: Arc<RwLock<HashMap<String, NodeHealthStatus>>>,
    subsystem_health: Arc<RwLock<HashMap<String, SubsystemHealthStatus>>>,
    config: HealthConfig,
}

#[derive(Debug, Clone)]
pub struct HealthConfig {
    pub check_interval_seconds: u64,
    pub timeout_seconds: u64,
    pub memory_warning_threshold_percent: f64,
    pub memory_critical_threshold_percent: f64,
    pub disk_warning_threshold_percent: f64,
    pub disk_critical_threshold_percent: f64,
    pub cpu_warning_threshold_percent: f64,
    pub cpu_critical_threshold_percent: f64,
    pub enable_deep_checks: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 30,
            timeout_seconds: 10,
            memory_warning_threshold_percent: 80.0,
            memory_critical_threshold_percent: 95.0,
            disk_warning_threshold_percent: 85.0,
            disk_critical_threshold_percent: 95.0,
            cpu_warning_threshold_percent: 80.0,
            cpu_critical_threshold_percent: 95.0,
            enable_deep_checks: true,
        }
    }
}

/// Health check status levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Individual health check result
#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub duration_ms: u64,
    pub timestamp: u64,
    pub details: HashMap<String, serde_json::Value>,
}

/// Overall health response
#[derive(Debug, Clone, Serialize)]
pub struct ControlPlaneHealthResponse {
    pub status: HealthStatus,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub version: String,
    pub node_id: String,
    pub checks: Vec<HealthCheckResult>,
    pub system_info: SystemInfo,
    pub cluster_info: ClusterInfo,
    pub subsystems: HashMap<String, SubsystemHealthStatus>,
    pub nodes: HashMap<String, NodeHealthStatus>,
}

/// System information
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub platform: String,
    pub architecture: String,
    pub cpu_count: usize,
    pub cpu_usage_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_usage_percent: f64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_available_bytes: u64,
    pub disk_usage_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub load_average: Vec<f64>,
}

/// Cluster information
#[derive(Debug, Clone, Serialize)]
pub struct ClusterInfo {
    pub cluster_id: String,
    pub node_count: usize,
    pub healthy_nodes: usize,
    pub unhealthy_nodes: usize,
    pub leader_node: Option<String>,
    pub cluster_status: HealthStatus,
}

/// Node health status
#[derive(Debug, Clone, Serialize)]
pub struct NodeHealthStatus {
    pub node_id: String,
    pub status: HealthStatus,
    pub last_heartbeat: u64,
    pub uptime_seconds: u64,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub active_jobs: usize,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub last_error: Option<String>,
}

/// Subsystem health status
#[derive(Debug, Clone, Serialize)]
pub struct SubsystemHealthStatus {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: u64,
    pub uptime_seconds: u64,
    pub error_count: u64,
    pub last_error: Option<String>,
    pub metrics: HashMap<String, f64>,
}

/// Health check trait
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthCheckResult;
    fn name(&self) -> &str;
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn is_critical(&self) -> bool {
        false
    }
}

impl ControlPlaneHealth {
    /// Create new control plane health monitor
    pub fn new(config: HealthConfig) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            checks: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
            system: Arc::new(RwLock::new(system)),
            node_health: Arc::new(RwLock::new(HashMap::new())),
            subsystem_health: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Add a health check
    #[instrument(skip(self, check))]
    pub async fn add_check(&self, check: Box<dyn HealthCheck>) {
        let mut checks = self.checks.write().await;
        info!("Adding health check: {}", check.name());
        checks.push(check);
    }

    /// Add multiple health checks
    pub async fn add_checks(&self, checks: Vec<Box<dyn HealthCheck>>) {
        for check in checks {
            self.add_check(check).await;
        }
    }

    /// Update node health status
    #[instrument(skip(self, status))]
    pub async fn update_node_health(&self, node_id: String, status: NodeHealthStatus) {
        let mut node_health = self.node_health.write().await;
        node_health.insert(node_id.clone(), status);
        debug!("Updated health status for node: {}", node_id);
    }

    /// Update subsystem health status
    #[instrument(skip(self, status))]
    pub async fn update_subsystem_health(&self, name: String, status: SubsystemHealthStatus) {
        let mut subsystem_health = self.subsystem_health.write().await;
        subsystem_health.insert(name.clone(), status);
        debug!("Updated health status for subsystem: {}", name);
    }

    /// Perform comprehensive health check
    #[instrument(skip(self))]
    pub async fn check_health(&self) -> ControlPlaneHealthResponse {
        let start_time = Instant::now();
        
        // Refresh system information
        {
            let mut system = self.system.write().await;
            system.refresh_all();
        }

        // Run all health checks
        let checks = self.checks.read().await;
        let mut results = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        for check in checks.iter() {
            let check_start = Instant::now();
            
            let result = match tokio::time::timeout(check.timeout(), check.check()).await {
                Ok(result) => result,
                Err(_) => HealthCheckResult {
                    name: check.name().to_string(),
                    status: HealthStatus::Unhealthy,
                    message: "Health check timed out".to_string(),
                    duration_ms: check_start.elapsed().as_millis() as u64,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    details: HashMap::new(),
                },
            };

            // Update overall status
            match (&result.status, check.is_critical()) {
                (HealthStatus::Unhealthy, true) => overall_status = HealthStatus::Unhealthy,
                (HealthStatus::Unhealthy, false) if overall_status == HealthStatus::Healthy => {
                    overall_status = HealthStatus::Degraded
                }
                (HealthStatus::Degraded, _) if overall_status == HealthStatus::Healthy => {
                    overall_status = HealthStatus::Degraded
                }
                _ => {}
            }

            results.push(result);
        }

        // Get system info
        let system_info = self.get_system_info().await;
        
        // Get cluster info
        let cluster_info = self.get_cluster_info().await;

        // Get subsystem and node health
        let subsystems = self.subsystem_health.read().await.clone();
        let nodes = self.node_health.read().await.clone();

        let response = ControlPlaneHealthResponse {
            status: overall_status,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            node_id: self.get_node_id(),
            checks: results,
            system_info,
            cluster_info,
            subsystems,
            nodes,
        };

        let duration = start_time.elapsed();
        debug!("Health check completed in {:?}", duration);

        response
    }

    /// Get system information
    async fn get_system_info(&self) -> SystemInfo {
        let system = self.system.read().await;
        
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
        let memory_total = system.total_memory();
        let memory_used = system.used_memory();
        let memory_available = system.available_memory();
        let memory_usage_percent = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        };

        let (disk_total, disk_used) = system.disks().iter().fold((0u64, 0u64), |(total, used), disk| {
            (total + disk.total_space(), used + (disk.total_space() - disk.available_space()))
        });
        
        let disk_available = disk_total - disk_used;
        let disk_usage_percent = if disk_total > 0 {
            (disk_used as f64 / disk_total as f64) * 100.0
        } else {
            0.0
        };

        let (network_rx, network_tx) = system.networks().iter().fold((0u64, 0u64), |(rx, tx), (_, network)| {
            (rx + network.received(), tx + network.transmitted())
        });

        let load_average = system.load_average();

        SystemInfo {
            hostname: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            platform: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            cpu_count: system.cpus().len(),
            cpu_usage_percent: cpu_usage,
            memory_total_bytes: memory_total,
            memory_used_bytes: memory_used,
            memory_available_bytes: memory_available,
            memory_usage_percent,
            disk_total_bytes: disk_total,
            disk_used_bytes: disk_used,
            disk_available_bytes: disk_available,
            disk_usage_percent,
            network_rx_bytes: network_rx,
            network_tx_bytes: network_tx,
            load_average: vec![load_average.one, load_average.five, load_average.fifteen],
        }
    }

    /// Get cluster information
    async fn get_cluster_info(&self) -> ClusterInfo {
        let nodes = self.node_health.read().await;
        let healthy_nodes = nodes.values().filter(|n| n.status == HealthStatus::Healthy).count();
        let unhealthy_nodes = nodes.len() - healthy_nodes;

        let cluster_status = if unhealthy_nodes == 0 {
            HealthStatus::Healthy
        } else if healthy_nodes > unhealthy_nodes {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        ClusterInfo {
            cluster_id: "rustci-cluster".to_string(), // Should come from config
            node_count: nodes.len(),
            healthy_nodes,
            unhealthy_nodes,
            leader_node: None, // TODO: Get from leader election
            cluster_status,
        }
    }

    /// Get node ID
    fn get_node_id(&self) -> String {
        // In a real implementation, this would be a persistent node ID
        format!("node-{}", hostname::get()
            .unwrap_or_default()
            .to_string_lossy())
    }

    /// Start background health monitoring
    #[instrument(skip(self))]
    pub async fn start_monitoring(&self) -> Result<()> {
        let health_monitor = Arc::new(self.clone());
        let interval = Duration::from_secs(self.config.check_interval_seconds);

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                let health_response = health_monitor.check_health().await;
                
                // Log health status changes
                match health_response.status {
                    HealthStatus::Healthy => {
                        debug!("Control plane health check: Healthy");
                    }
                    HealthStatus::Degraded => {
                        warn!("Control plane health check: Degraded");
                    }
                    HealthStatus::Unhealthy => {
                        error!("Control plane health check: Unhealthy");
                    }
                    HealthStatus::Unknown => {
                        warn!("Control plane health check: Unknown");
                    }
                }

                // Check for critical system resource usage
                let system_info = &health_response.system_info;
                
                if system_info.memory_usage_percent > health_monitor.config.memory_critical_threshold_percent {
                    error!("Critical memory usage: {:.1}%", system_info.memory_usage_percent);
                } else if system_info.memory_usage_percent > health_monitor.config.memory_warning_threshold_percent {
                    warn!("High memory usage: {:.1}%", system_info.memory_usage_percent);
                }

                if system_info.disk_usage_percent > health_monitor.config.disk_critical_threshold_percent {
                    error!("Critical disk usage: {:.1}%", system_info.disk_usage_percent);
                } else if system_info.disk_usage_percent > health_monitor.config.disk_warning_threshold_percent {
                    warn!("High disk usage: {:.1}%", system_info.disk_usage_percent);
                }

                if system_info.cpu_usage_percent > health_monitor.config.cpu_critical_threshold_percent {
                    error!("Critical CPU usage: {:.1}%", system_info.cpu_usage_percent);
                } else if system_info.cpu_usage_percent > health_monitor.config.cpu_warning_threshold_percent {
                    warn!("High CPU usage: {:.1}%", system_info.cpu_usage_percent);
                }
            }
        });

        info!("Control plane health monitoring started");
        Ok(())
    }

    /// Create HTTP router for health endpoints
    pub fn create_router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .route("/health/deep", get(deep_health_handler))
            .route("/health/nodes", get(nodes_health_handler))
            .route("/health/subsystems", get(subsystems_health_handler))
            .with_state(self)
    }
}

// Clone implementation for background monitoring
impl Clone for ControlPlaneHealth {
    fn clone(&self) -> Self {
        Self {
            checks: Arc::clone(&self.checks),
            start_time: self.start_time,
            system: Arc::clone(&self.system),
            node_health: Arc::clone(&self.node_health),
            subsystem_health: Arc::clone(&self.subsystem_health),
            config: self.config.clone(),
        }
    }
}
/// Database connectivity health check
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

        match self.database.run_command(mongodb::bson::doc! { "ping": 1 }, None).await {
            Ok(_) => HealthCheckResult {
                name: "database".to_string(),
                status: HealthStatus::Healthy,
                message: "Database connection is healthy".to_string(),
                duration_ms: start_time.elapsed().as_millis() as u64,
                timestamp,
                details: HashMap::new(),
            },
            Err(e) => HealthCheckResult {
                name: "database".to_string(),
                status: HealthStatus::Unhealthy,
                message: format!("Database connection failed: {}", e),
                duration_ms: start_time.elapsed().as_millis() as u64,
                timestamp,
                details: {
                    let mut details = HashMap::new();
                    details.insert("error".to_string(), serde_json::Value::String(e.to_string()));
                    details
                },
            },
        }
    }

    fn name(&self) -> &str {
        "database"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

/// Job scheduler health check
pub struct JobSchedulerHealthCheck {
    active_jobs: Arc<RwLock<usize>>,
    queue_length: Arc<RwLock<usize>>,
}

impl JobSchedulerHealthCheck {
    pub fn new(active_jobs: Arc<RwLock<usize>>, queue_length: Arc<RwLock<usize>>) -> Self {
        Self {
            active_jobs,
            queue_length,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for JobSchedulerHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let active_jobs = *self.active_jobs.read().await;
        let queue_length = *self.queue_length.read().await;

        let mut details = HashMap::new();
        details.insert("active_jobs".to_string(), serde_json::Value::Number(active_jobs.into()));
        details.insert("queue_length".to_string(), serde_json::Value::Number(queue_length.into()));

        let (status, message) = if queue_length > 1000 {
            (HealthStatus::Degraded, format!("High queue length: {}", queue_length))
        } else if active_jobs > 100 {
            (HealthStatus::Degraded, format!("High number of active jobs: {}", active_jobs))
        } else {
            (HealthStatus::Healthy, "Job scheduler is healthy".to_string())
        };

        HealthCheckResult {
            name: "job_scheduler".to_string(),
            status,
            message,
            duration_ms: start_time.elapsed().as_millis() as u64,
            timestamp,
            details,
        }
    }

    fn name(&self) -> &str {
        "job_scheduler"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

/// Node registry health check
pub struct NodeRegistryHealthCheck {
    healthy_nodes: Arc<RwLock<usize>>,
    total_nodes: Arc<RwLock<usize>>,
}

impl NodeRegistryHealthCheck {
    pub fn new(healthy_nodes: Arc<RwLock<usize>>, total_nodes: Arc<RwLock<usize>>) -> Self {
        Self {
            healthy_nodes,
            total_nodes,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for NodeRegistryHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let healthy_nodes = *self.healthy_nodes.read().await;
        let total_nodes = *self.total_nodes.read().await;

        let mut details = HashMap::new();
        details.insert("healthy_nodes".to_string(), serde_json::Value::Number(healthy_nodes.into()));
        details.insert("total_nodes".to_string(), serde_json::Value::Number(total_nodes.into()));

        let health_ratio = if total_nodes > 0 {
            healthy_nodes as f64 / total_nodes as f64
        } else {
            1.0
        };

        let (status, message) = if total_nodes == 0 {
            (HealthStatus::Degraded, "No nodes registered".to_string())
        } else if health_ratio < 0.5 {
            (HealthStatus::Unhealthy, format!("Less than 50% of nodes are healthy ({}/{})", healthy_nodes, total_nodes))
        } else if health_ratio < 0.8 {
            (HealthStatus::Degraded, format!("Less than 80% of nodes are healthy ({}/{})", healthy_nodes, total_nodes))
        } else {
            (HealthStatus::Healthy, format!("Node registry is healthy ({}/{} nodes)", healthy_nodes, total_nodes))
        };

        HealthCheckResult {
            name: "node_registry".to_string(),
            status,
            message,
            duration_ms: start_time.elapsed().as_millis() as u64,
            timestamp,
            details,
        }
    }

    fn name(&self) -> &str {
        "node_registry"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

// HTTP handlers
async fn health_handler(
    State(health): State<Arc<ControlPlaneHealth>>,
) -> impl IntoResponse {
    let health_response = health.check_health().await;
    
    let status_code = match health_response.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        HealthStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(serde_json::to_value(health_response).unwrap_or_default()))
}

async fn liveness_handler() -> impl IntoResponse {
    // Liveness probe - just check if the application is running
    (StatusCode::OK, Json(serde_json::json!({
        "status": "alive",
        "timestamp": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    })))
}

async fn readiness_handler(
    State(health): State<Arc<ControlPlaneHealth>>,
) -> impl IntoResponse {
    let health_response = health.check_health().await;
    
    let status_code = match health_response.status {
        HealthStatus::Healthy | HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy | HealthStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(serde_json::json!({
        "status": "ready",
        "health_status": health_response.status,
        "timestamp": health_response.timestamp
    })))
}

async fn deep_health_handler(
    State(health): State<Arc<ControlPlaneHealth>>,
) -> impl IntoResponse {
    if !health.config.enable_deep_checks {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Deep health checks are disabled"
        })));
    }

    let health_response = health.check_health().await;
    let status_code = match health_response.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        HealthStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status_code, Json(serde_json::to_value(health_response).unwrap_or_default()))
}

async fn nodes_health_handler(
    State(health): State<Arc<ControlPlaneHealth>>,
) -> impl IntoResponse {
    let nodes = health.node_health.read().await.clone();
    (StatusCode::OK, Json(serde_json::to_value(nodes).unwrap_or_default()))
}

async fn subsystems_health_handler(
    State(health): State<Arc<ControlPlaneHealth>>,
) -> impl IntoResponse {
    let subsystems = health.subsystem_health.read().await.clone();
    (StatusCode::OK, Json(serde_json::to_value(subsystems).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHealthCheck {
        name: String,
        status: HealthStatus,
        is_critical: bool,
    }

    impl MockHealthCheck {
        fn new(name: String, status: HealthStatus, is_critical: bool) -> Self {
            Self { name, status, is_critical }
        }
    }

    #[async_trait::async_trait]
    impl HealthCheck for MockHealthCheck {
        async fn check(&self) -> HealthCheckResult {
            HealthCheckResult {
                name: self.name.clone(),
                status: self.status.clone(),
                message: "Mock health check".to_string(),
                duration_ms: 10,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                details: HashMap::new(),
            }
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn is_critical(&self) -> bool {
            self.is_critical
        }
    }

    #[tokio::test]
    async fn test_control_plane_health_creation() {
        let config = HealthConfig::default();
        let health = ControlPlaneHealth::new(config);
        
        let health_response = health.check_health().await;
        assert_eq!(health_response.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_checks() {
        let config = HealthConfig::default();
        let health = ControlPlaneHealth::new(config);

        health.add_check(Box::new(MockHealthCheck::new(
            "test1".to_string(),
            HealthStatus::Healthy,
            false,
        ))).await;

        health.add_check(Box::new(MockHealthCheck::new(
            "test2".to_string(),
            HealthStatus::Degraded,
            false,
        ))).await;

        let health_response = health.check_health().await;
        assert_eq!(health_response.status, HealthStatus::Degraded);
        assert_eq!(health_response.checks.len(), 2);
    }

    #[tokio::test]
    async fn test_critical_health_check_failure() {
        let config = HealthConfig::default();
        let health = ControlPlaneHealth::new(config);

        health.add_check(Box::new(MockHealthCheck::new(
            "critical".to_string(),
            HealthStatus::Unhealthy,
            true,
        ))).await;

        let health_response = health.check_health().await;
        assert_eq!(health_response.status, HealthStatus::Unhealthy);
    }
}