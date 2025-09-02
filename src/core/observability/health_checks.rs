use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Comprehensive health check system for multi-tier monitoring
pub struct HealthCheckSystem {
    health_checkers: Arc<RwLock<HashMap<String, Box<dyn HealthChecker + Send + Sync>>>>,
    dependency_monitor: Arc<DependencyHealthMonitor>,
    cascade_detector: Arc<CascadeFailureDetector>,
    health_aggregator: Arc<HealthAggregator>,
    config: HealthCheckConfig,
}

/// Configuration for health check system
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub check_interval: Duration,
    pub timeout_duration: Duration,
    pub max_retries: u32,
    pub enable_cascade_detection: bool,
    pub enable_dependency_monitoring: bool,
    pub health_history_retention: Duration,
    pub critical_threshold: f64,
    pub warning_threshold: f64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            timeout_duration: Duration::from_secs(10),
            max_retries: 3,
            enable_cascade_detection: true,
            enable_dependency_monitoring: true,
            health_history_retention: Duration::from_secs(3600), // 1 hour
            critical_threshold: 0.5, // 50% failure rate
            warning_threshold: 0.2,  // 20% failure rate
        }
    }
}

/// Health checker trait for different components
#[async_trait::async_trait]
pub trait HealthChecker {
    async fn check_health(&self) -> HealthCheckResult;
    fn get_name(&self) -> &str;
    fn get_dependencies(&self) -> Vec<String>;
    fn get_check_type(&self) -> HealthCheckType;
    fn get_timeout(&self) -> Duration;
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub component_name: String,
    pub status: HealthStatus,
    pub timestamp: SystemTime,
    pub duration: Duration,
    pub message: String,
    pub details: HashMap<String, serde_json::Value>,
    pub metrics: Option<ComponentMetrics>,
    pub dependencies: Vec<DependencyHealth>,
    pub check_id: Uuid,
}

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
    Degraded,
}

/// Health check type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthCheckType {
    Database,
    ExternalService,
    FileSystem,
    Memory,
    CPU,
    Network,
    Cache,
    Queue,
    Custom(String),
}

/// Component metrics for health assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMetrics {
    pub response_time_ms: f64,
    pub success_rate: f64,
    pub error_rate: f64,
    pub throughput: f64,
    pub resource_usage: ResourceUsage,
    pub custom_metrics: HashMap<String, f64>,
}

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub disk_percent: f64,
    pub network_io: f64,
    pub connection_count: u32,
}

/// Dependency health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub name: String,
    pub status: HealthStatus,
    pub response_time: Duration,
    pub last_check: SystemTime,
    pub error_message: Option<String>,
}

/// Dependency health monitor
pub struct DependencyHealthMonitor {
    dependencies: Arc<RwLock<HashMap<String, DependencyInfo>>>,
    health_history: Arc<RwLock<Vec<DependencyHealthRecord>>>,
    config: HealthCheckConfig,
}

/// Dependency information
#[derive(Debug, Clone)]
pub struct DependencyInfo {
    pub name: String,
    pub endpoint: String,
    pub dependency_type: DependencyType,
    pub critical: bool,
    pub timeout: Duration,
    pub retry_count: u32,
}

/// Dependency type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Database,
    HttpService,
    MessageQueue,
    Cache,
    FileSystem,
    ExternalAPI,
}

/// Dependency health record for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealthRecord {
    pub dependency_name: String,
    pub status: HealthStatus,
    pub timestamp: SystemTime,
    pub response_time: Duration,
    pub error_message: Option<String>,
}

/// Cascade failure detector
pub struct CascadeFailureDetector {
    failure_patterns: Arc<RwLock<Vec<FailurePattern>>>,
    active_incidents: Arc<RwLock<HashMap<String, CascadeIncident>>>,
    config: HealthCheckConfig,
}

/// Failure pattern for cascade detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub pattern_id: String,
    pub trigger_component: String,
    pub affected_components: Vec<String>,
    pub failure_sequence: Vec<FailureStep>,
    pub detection_confidence: f64,
}

/// Failure step in cascade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureStep {
    pub component: String,
    pub expected_delay: Duration,
    pub failure_probability: f64,
}

/// Cascade incident
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeIncident {
    pub incident_id: Uuid,
    pub trigger_component: String,
    pub start_time: SystemTime,
    pub affected_components: Vec<String>,
    pub current_stage: u32,
    pub predicted_impact: ImpactAssessment,
    pub mitigation_actions: Vec<MitigationAction>,
}

/// Impact assessment for cascade failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub severity: ImpactSeverity,
    pub estimated_duration: Duration,
    pub affected_users: u32,
    pub business_impact: BusinessImpact,
}

/// Impact severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Business impact assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessImpact {
    pub revenue_impact: f64,
    pub user_experience_score: f64,
    pub sla_breach_risk: f64,
}

/// Mitigation action for incidents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationAction {
    pub action_id: String,
    pub description: String,
    pub priority: ActionPriority,
    pub estimated_time: Duration,
    pub automated: bool,
    pub prerequisites: Vec<String>,
}

/// Action priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionPriority {
    Immediate,
    High,
    Medium,
    Low,
}

/// Health aggregator for system-wide status
pub struct HealthAggregator {
    aggregated_status: Arc<RwLock<SystemHealthStatus>>,
    health_history: Arc<RwLock<Vec<SystemHealthSnapshot>>>,
    config: HealthCheckConfig,
}

/// System-wide health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthStatus {
    pub overall_status: HealthStatus,
    pub component_statuses: HashMap<String, HealthStatus>,
    pub dependency_statuses: HashMap<String, HealthStatus>,
    pub health_score: f64, // 0-100
    pub last_updated: SystemTime,
    pub active_incidents: u32,
    pub degraded_services: Vec<String>,
    pub critical_alerts: Vec<HealthAlert>,
}

/// Health alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub alert_id: Uuid,
    pub component: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: SystemTime,
    pub acknowledged: bool,
    pub auto_resolve: bool,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// System health snapshot for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthSnapshot {
    pub timestamp: SystemTime,
    pub overall_status: HealthStatus,
    pub health_score: f64,
    pub component_count: u32,
    pub healthy_components: u32,
    pub degraded_components: u32,
    pub critical_components: u32,
}

impl HealthCheckSystem {
    /// Create new health check system
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            health_checkers: Arc::new(RwLock::new(HashMap::new())),
            dependency_monitor: Arc::new(DependencyHealthMonitor::new(config.clone())),
            cascade_detector: Arc::new(CascadeFailureDetector::new(config.clone())),
            health_aggregator: Arc::new(HealthAggregator::new(config.clone())),
            config,
        }
    }

    /// Register a health checker
    pub async fn register_health_checker(
        &self,
        checker: Box<dyn HealthChecker + Send + Sync>,
    ) -> Result<()> {
        let name = checker.get_name().to_string();
        let mut checkers = self.health_checkers.write().await;
        checkers.insert(name.clone(), checker);
        
        info!("Registered health checker: {}", name);
        Ok(())
    }

    /// Run all health checks
    pub async fn run_health_checks(&self) -> Result<SystemHealthStatus> {
        let checkers = self.health_checkers.read().await;
        let mut results = Vec::new();

        // Run all health checks concurrently
        let mut check_futures = Vec::new();
        for (name, checker) in checkers.iter() {
            let checker_clone = checker.as_ref();
            let timeout = checker_clone.get_timeout();
            
            let future = tokio::time::timeout(timeout, checker_clone.check_health());
            check_futures.push((name.clone(), future));
        }

        // Collect results
        for (name, future) in check_futures {
            match future.await {
                Ok(result) => results.push(result),
                Err(_) => {
                    // Timeout occurred
                    results.push(HealthCheckResult {
                        component_name: name.clone(),
                        status: HealthStatus::Critical,
                        timestamp: SystemTime::now(),
                        duration: self.config.timeout_duration,
                        message: "Health check timed out".to_string(),
                        details: HashMap::new(),
                        metrics: None,
                        dependencies: Vec::new(),
                        check_id: Uuid::new_v4(),
                    });
                }
            }
        }

        // Check dependencies if enabled
        if self.config.enable_dependency_monitoring {
            self.dependency_monitor.check_all_dependencies().await?;
        }

        // Detect cascade failures if enabled
        if self.config.enable_cascade_detection {
            self.cascade_detector.analyze_failures(&results).await?;
        }

        // Aggregate health status
        let system_status = self.health_aggregator.aggregate_health(&results).await?;

        Ok(system_status)
    }

    /// Get system health status
    pub async fn get_system_health(&self) -> SystemHealthStatus {
        self.health_aggregator.get_current_status().await
    }

    /// Get health check history
    pub async fn get_health_history(&self, duration: Duration) -> Vec<SystemHealthSnapshot> {
        self.health_aggregator.get_health_history(duration).await
    }

    /// Get dependency health
    pub async fn get_dependency_health(&self) -> HashMap<String, DependencyHealth> {
        self.dependency_monitor.get_all_dependency_health().await
    }

    /// Get active cascade incidents
    pub async fn get_cascade_incidents(&self) -> Vec<CascadeIncident> {
        self.cascade_detector.get_active_incidents().await
    }

    /// Add dependency
    pub async fn add_dependency(&self, dependency: DependencyInfo) -> Result<()> {
        self.dependency_monitor.add_dependency(dependency).await
    }

    /// Remove dependency
    pub async fn remove_dependency(&self, name: &str) -> Result<()> {
        self.dependency_monitor.remove_dependency(name).await
    }
}

impl DependencyHealthMonitor {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            dependencies: Arc::new(RwLock::new(HashMap::new())),
            health_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    pub async fn add_dependency(&self, dependency: DependencyInfo) -> Result<()> {
        let mut deps = self.dependencies.write().await;
        deps.insert(dependency.name.clone(), dependency);
        Ok(())
    }

    pub async fn remove_dependency(&self, name: &str) -> Result<()> {
        let mut deps = self.dependencies.write().await;
        deps.remove(name);
        Ok(())
    }

    pub async fn check_all_dependencies(&self) -> Result<()> {
        let deps = self.dependencies.read().await;
        
        for (name, dep_info) in deps.iter() {
            let health = self.check_dependency_health(dep_info).await;
            
            let record = DependencyHealthRecord {
                dependency_name: name.clone(),
                status: health.status.clone(),
                timestamp: SystemTime::now(),
                response_time: health.response_time,
                error_message: health.error_message.clone(),
            };

            let mut history = self.health_history.write().await;
            history.push(record);

            // Trim history
            let cutoff = SystemTime::now() - self.config.health_history_retention;
            history.retain(|record| record.timestamp > cutoff);
        }

        Ok(())
    }

    async fn check_dependency_health(&self, dep_info: &DependencyInfo) -> DependencyHealth {
        let start_time = SystemTime::now();
        
        // Simulate dependency health check based on type
        let (status, error_message) = match dep_info.dependency_type {
            DependencyType::Database => {
                // Would perform actual database connectivity check
                (HealthStatus::Healthy, None)
            }
            DependencyType::HttpService => {
                // Would perform HTTP health check
                (HealthStatus::Healthy, None)
            }
            DependencyType::MessageQueue => {
                // Would check message queue connectivity
                (HealthStatus::Healthy, None)
            }
            DependencyType::Cache => {
                // Would check cache connectivity
                (HealthStatus::Healthy, None)
            }
            DependencyType::FileSystem => {
                // Would check file system access
                (HealthStatus::Healthy, None)
            }
            DependencyType::ExternalAPI => {
                // Would check external API availability
                (HealthStatus::Healthy, None)
            }
        };

        let response_time = start_time.elapsed().unwrap_or_default();

        DependencyHealth {
            name: dep_info.name.clone(),
            status,
            response_time,
            last_check: SystemTime::now(),
            error_message,
        }
    }

    pub async fn get_all_dependency_health(&self) -> HashMap<String, DependencyHealth> {
        let deps = self.dependencies.read().await;
        let mut health_map = HashMap::new();

        for (name, dep_info) in deps.iter() {
            let health = self.check_dependency_health(dep_info).await;
            health_map.insert(name.clone(), health);
        }

        health_map
    }
}

impl CascadeFailureDetector {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            failure_patterns: Arc::new(RwLock::new(Vec::new())),
            active_incidents: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn analyze_failures(&self, results: &[HealthCheckResult]) -> Result<()> {
        let failed_components: Vec<_> = results
            .iter()
            .filter(|r| matches!(r.status, HealthStatus::Critical | HealthStatus::Degraded))
            .map(|r| r.component_name.clone())
            .collect();

        if failed_components.is_empty() {
            return Ok(());
        }

        // Check for cascade patterns
        let patterns = self.failure_patterns.read().await;
        for pattern in patterns.iter() {
            if failed_components.contains(&pattern.trigger_component) {
                self.create_cascade_incident(pattern, &failed_components).await?;
            }
        }

        Ok(())
    }

    async fn create_cascade_incident(
        &self,
        pattern: &FailurePattern,
        failed_components: &[String],
    ) -> Result<()> {
        let incident_id = Uuid::new_v4();
        
        let incident = CascadeIncident {
            incident_id,
            trigger_component: pattern.trigger_component.clone(),
            start_time: SystemTime::now(),
            affected_components: failed_components.to_vec(),
            current_stage: 0,
            predicted_impact: ImpactAssessment {
                severity: ImpactSeverity::Medium,
                estimated_duration: Duration::from_secs(300),
                affected_users: 100,
                business_impact: BusinessImpact {
                    revenue_impact: 1000.0,
                    user_experience_score: 0.7,
                    sla_breach_risk: 0.3,
                },
            },
            mitigation_actions: vec![
                MitigationAction {
                    action_id: "restart_services".to_string(),
                    description: "Restart affected services".to_string(),
                    priority: ActionPriority::High,
                    estimated_time: Duration::from_secs(60),
                    automated: true,
                    prerequisites: Vec::new(),
                },
            ],
        };

        let mut incidents = self.active_incidents.write().await;
        incidents.insert(incident_id.to_string(), incident);

        warn!(
            "Cascade failure incident created: {} triggered by {}",
            incident_id, pattern.trigger_component
        );

        Ok(())
    }

    pub async fn get_active_incidents(&self) -> Vec<CascadeIncident> {
        let incidents = self.active_incidents.read().await;
        incidents.values().cloned().collect()
    }
}

impl HealthAggregator {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            aggregated_status: Arc::new(RwLock::new(SystemHealthStatus {
                overall_status: HealthStatus::Unknown,
                component_statuses: HashMap::new(),
                dependency_statuses: HashMap::new(),
                health_score: 0.0,
                last_updated: SystemTime::now(),
                active_incidents: 0,
                degraded_services: Vec::new(),
                critical_alerts: Vec::new(),
            })),
            health_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    pub async fn aggregate_health(&self, results: &[HealthCheckResult]) -> Result<SystemHealthStatus> {
        let mut component_statuses = HashMap::new();
        let mut degraded_services = Vec::new();
        let mut critical_alerts = Vec::new();

        let mut healthy_count = 0;
        let mut warning_count = 0;
        let mut critical_count = 0;

        for result in results {
            component_statuses.insert(result.component_name.clone(), result.status.clone());

            match result.status {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Warning => {
                    warning_count += 1;
                    degraded_services.push(result.component_name.clone());
                }
                HealthStatus::Critical | HealthStatus::Degraded => {
                    critical_count += 1;
                    degraded_services.push(result.component_name.clone());
                    
                    critical_alerts.push(HealthAlert {
                        alert_id: Uuid::new_v4(),
                        component: result.component_name.clone(),
                        severity: AlertSeverity::Critical,
                        message: result.message.clone(),
                        timestamp: result.timestamp,
                        acknowledged: false,
                        auto_resolve: false,
                    });
                }
                _ => {}
            }
        }

        let total_components = results.len() as f64;
        let health_score = if total_components > 0.0 {
            (healthy_count as f64 / total_components) * 100.0
        } else {
            100.0
        };

        let overall_status = if critical_count > 0 {
            HealthStatus::Critical
        } else if warning_count > 0 {
            HealthStatus::Warning
        } else if healthy_count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        };

        let system_status = SystemHealthStatus {
            overall_status: overall_status.clone(),
            component_statuses,
            dependency_statuses: HashMap::new(), // Would be populated from dependency monitor
            health_score,
            last_updated: SystemTime::now(),
            active_incidents: 0, // Would be populated from cascade detector
            degraded_services,
            critical_alerts,
        };

        // Update aggregated status
        {
            let mut status = self.aggregated_status.write().await;
            *status = system_status.clone();
        }

        // Add to history
        {
            let mut history = self.health_history.write().await;
            history.push(SystemHealthSnapshot {
                timestamp: SystemTime::now(),
                overall_status,
                health_score,
                component_count: total_components as u32,
                healthy_components: healthy_count,
                degraded_components: warning_count,
                critical_components: critical_count,
            });

            // Trim history
            let cutoff = SystemTime::now() - self.config.health_history_retention;
            history.retain(|snapshot| snapshot.timestamp > cutoff);
        }

        Ok(system_status)
    }

    pub async fn get_current_status(&self) -> SystemHealthStatus {
        self.aggregated_status.read().await.clone()
    }

    pub async fn get_health_history(&self, duration: Duration) -> Vec<SystemHealthSnapshot> {
        let history = self.health_history.read().await;
        let cutoff = SystemTime::now() - duration;
        
        history
            .iter()
            .filter(|snapshot| snapshot.timestamp > cutoff)
            .cloned()
            .collect()
    }
}

// Example health checker implementations

/// Database health checker
pub struct DatabaseHealthChecker {
    name: String,
    connection_string: String,
    timeout: Duration,
}

impl DatabaseHealthChecker {
    pub fn new(name: String, connection_string: String, timeout: Duration) -> Self {
        Self {
            name,
            connection_string,
            timeout,
        }
    }
}

#[async_trait::async_trait]
impl HealthChecker for DatabaseHealthChecker {
    async fn check_health(&self) -> HealthCheckResult {
        let start_time = SystemTime::now();
        let check_id = Uuid::new_v4();

        // Simulate database health check
        let (status, message, details) = {
            // In a real implementation, this would:
            // 1. Attempt to connect to the database
            // 2. Execute a simple query (e.g., SELECT 1)
            // 3. Measure response time
            // 4. Check connection pool status
            
            tokio::time::sleep(Duration::from_millis(50)).await; // Simulate check time
            
            (
                HealthStatus::Healthy,
                "Database connection successful".to_string(),
                {
                    let mut details = HashMap::new();
                    details.insert("connection_pool_size".to_string(), serde_json::json!(10));
                    details.insert("active_connections".to_string(), serde_json::json!(3));
                    details.insert("query_response_time_ms".to_string(), serde_json::json!(25));
                    details
                }
            )
        };

        let duration = start_time.elapsed().unwrap_or_default();

        HealthCheckResult {
            component_name: self.name.clone(),
            status,
            timestamp: SystemTime::now(),
            duration,
            message,
            details,
            metrics: Some(ComponentMetrics {
                response_time_ms: duration.as_millis() as f64,
                success_rate: 0.99,
                error_rate: 0.01,
                throughput: 100.0,
                resource_usage: ResourceUsage {
                    cpu_percent: 15.0,
                    memory_percent: 25.0,
                    disk_percent: 60.0,
                    network_io: 1024.0,
                    connection_count: 10,
                },
                custom_metrics: HashMap::new(),
            }),
            dependencies: Vec::new(),
            check_id,
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_dependencies(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_check_type(&self) -> HealthCheckType {
        HealthCheckType::Database
    }

    fn get_timeout(&self) -> Duration {
        self.timeout
    }
}

/// HTTP service health checker
pub struct HttpServiceHealthChecker {
    name: String,
    endpoint: String,
    timeout: Duration,
}

impl HttpServiceHealthChecker {
    pub fn new(name: String, endpoint: String, timeout: Duration) -> Self {
        Self {
            name,
            endpoint,
            timeout,
        }
    }
}

#[async_trait::async_trait]
impl HealthChecker for HttpServiceHealthChecker {
    async fn check_health(&self) -> HealthCheckResult {
        let start_time = SystemTime::now();
        let check_id = Uuid::new_v4();

        // Simulate HTTP health check
        let (status, message, details) = {
            // In a real implementation, this would:
            // 1. Make HTTP request to health endpoint
            // 2. Check response status and content
            // 3. Measure response time
            // 4. Validate service-specific health indicators
            
            tokio::time::sleep(Duration::from_millis(100)).await; // Simulate network call
            
            (
                HealthStatus::Healthy,
                "HTTP service responding normally".to_string(),
                {
                    let mut details = HashMap::new();
                    details.insert("http_status".to_string(), serde_json::json!(200));
                    details.insert("response_size_bytes".to_string(), serde_json::json!(1024));
                    details.insert("ssl_cert_days_remaining".to_string(), serde_json::json!(45));
                    details
                }
            )
        };

        let duration = start_time.elapsed().unwrap_or_default();

        HealthCheckResult {
            component_name: self.name.clone(),
            status,
            timestamp: SystemTime::now(),
            duration,
            message,
            details,
            metrics: Some(ComponentMetrics {
                response_time_ms: duration.as_millis() as f64,
                success_rate: 0.995,
                error_rate: 0.005,
                throughput: 50.0,
                resource_usage: ResourceUsage {
                    cpu_percent: 10.0,
                    memory_percent: 20.0,
                    disk_percent: 30.0,
                    network_io: 2048.0,
                    connection_count: 25,
                },
                custom_metrics: HashMap::new(),
            }),
            dependencies: vec![DependencyHealth {
                name: "database".to_string(),
                status: HealthStatus::Healthy,
                last_check: SystemTime::now(),
                response_time: Duration::from_millis(50),
                error_message: None,
            }],
            check_id,
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_dependencies(&self) -> Vec<String> {
        vec!["database".to_string()]
    }

    fn get_check_type(&self) -> HealthCheckType {
        HealthCheckType::ExternalService
    }

    fn get_timeout(&self) -> Duration {
        self.timeout
    }
}