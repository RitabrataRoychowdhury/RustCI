use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::core::{
    jobs::async_jobs::{JobQueue, QueueStats},
    infrastructure::{
        caching::{CacheManager, QueryCache, SessionCache, CacheStats},
        concurrency::{DeadlockDetector, DistributedLockManager, RateLimiter, RateLimitStats, LockInfo},
        error_manager::{ErrorContext, ErrorManager, ErrorStats},
        runtime_optimization::{RuntimeOptimizer, PerformanceReport},
    },
    patterns::correlation::CorrelationTracker,
};

use super::{
    metrics::{AlertCondition, AlertManager, AlertRule, MetricsCollector, PerformanceMonitor},
    monitoring::{DatabaseHealthCheck, DiskHealthCheck, HealthMonitor, MemoryHealthCheck},
    profiling::{MemoryProfiler, PerformanceProfiler},
};

/// Comprehensive observability service that integrates all monitoring components
pub struct ObservabilityService {
    pub error_manager: Arc<ErrorManager>,
    pub correlation_tracker: Arc<CorrelationTracker>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub performance_monitor: Arc<PerformanceMonitor>,
    pub alert_manager: Arc<AlertManager>,
    pub cache_manager: Arc<CacheManager>,
    pub query_cache: Arc<QueryCache>,
    pub session_cache: Arc<SessionCache>,
    pub health_monitor: Arc<HealthMonitor>,
    pub job_queue: Arc<JobQueue>,
    pub lock_manager: Arc<DistributedLockManager>,
    pub rate_limiter: Arc<RateLimiter>,
    pub deadlock_detector: Arc<DeadlockDetector>,
    pub performance_profiler: Arc<PerformanceProfiler>,
    pub memory_profiler: Arc<MemoryProfiler>,
    pub runtime_optimizer: Arc<RuntimeOptimizer>,
    config: ObservabilityConfig,
}

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub enable_health_checks: bool,
    pub enable_alerting: bool,
    pub enable_caching: bool,
    pub enable_async_jobs: bool,
    pub metrics_endpoint: String,
    pub health_endpoint: String,
    pub max_concurrent_jobs: usize,
    pub cache_ttl_seconds: u64,
    pub alert_check_interval_seconds: u64,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            enable_tracing: true,
            enable_health_checks: true,
            enable_alerting: true,
            enable_caching: true,
            enable_async_jobs: true,
            metrics_endpoint: "0.0.0.0:9090".to_string(),
            health_endpoint: "/health".to_string(),
            max_concurrent_jobs: 10,
            cache_ttl_seconds: 300,
            alert_check_interval_seconds: 60,
        }
    }
}

impl ObservabilityService {
    pub async fn new(
        config: ObservabilityConfig,
        database: Option<Arc<mongodb::Database>>,
    ) -> Result<Self> {
        // Initialize correlation tracker
        let correlation_tracker = Arc::new(CorrelationTracker::new());

        // Initialize metrics collector
        let metrics_collector = Arc::new(MetricsCollector::new());
        if config.enable_metrics {
            metrics_collector.init_prometheus_exporter(&config.metrics_endpoint)?;
            info!("Metrics collection enabled");
        }

        // Initialize performance monitor
        let performance_monitor = Arc::new(PerformanceMonitor::new(Arc::clone(&metrics_collector)));

        // Initialize alert manager
        let alert_manager = Arc::new(AlertManager::new(Arc::clone(&metrics_collector)));

        // Initialize error manager
        let error_manager = Arc::new(ErrorManager::new(Arc::clone(&correlation_tracker)));

        // Initialize cache manager
        let cache_manager = Arc::new(CacheManager::new(
            1000, // L1 max capacity
            Duration::from_secs(config.cache_ttl_seconds),
            Some(Arc::clone(&metrics_collector)),
        ));

        // Initialize specialized caches
        let query_cache = Arc::new(QueryCache::new(Some(Arc::clone(&metrics_collector))));
        let session_cache = Arc::new(SessionCache::new(Some(Arc::clone(&metrics_collector))));

        // Initialize health monitor
        let health_monitor = Arc::new(HealthMonitor::new(
            "1.0.0".to_string(), // Version should come from config
            Some(Arc::clone(&metrics_collector)),
        ));

        // Initialize job queue
        let job_queue = Arc::new(JobQueue::new(
            config.max_concurrent_jobs,
            Some(Arc::clone(&metrics_collector)),
            Arc::clone(&correlation_tracker),
        ));

        // Initialize concurrency components
        let lock_manager = Arc::new(DistributedLockManager::new(Some(Arc::clone(
            &metrics_collector,
        ))));
        let rate_limiter = Arc::new(RateLimiter::new(Some(Arc::clone(&metrics_collector))));
        let deadlock_detector =
            Arc::new(DeadlockDetector::new(Some(Arc::clone(&metrics_collector))));

        // Initialize profiling components
        let performance_profiler = Arc::new(PerformanceProfiler::new(Some(Arc::clone(
            &metrics_collector,
        ))));
        let memory_profiler = Arc::new(MemoryProfiler::new(Some(Arc::clone(&metrics_collector))));

        // Initialize runtime optimizer
        let runtime_optimizer =
            Arc::new(RuntimeOptimizer::new(Some(Arc::clone(&metrics_collector))));

        let service = Self {
            error_manager,
            correlation_tracker,
            metrics_collector,
            performance_monitor,
            alert_manager,
            cache_manager,
            query_cache,
            session_cache,
            health_monitor,
            job_queue,
            lock_manager,
            rate_limiter,
            deadlock_detector,
            performance_profiler,
            memory_profiler,
            runtime_optimizer,
            config: config.clone(),
        };

        // Setup health checks
        if config.enable_health_checks {
            service.setup_health_checks(database).await?;
        }

        // Setup default alerts
        if config.enable_alerting {
            service.setup_default_alerts().await?;
        }

        // Start background tasks
        service.start_background_tasks().await?;

        info!("Observability service initialized successfully");
        Ok(service)
    }

    /// Setup health checks
    async fn setup_health_checks(&self, database: Option<Arc<mongodb::Database>>) -> Result<()> {
        // Database health check
        if let Some(db) = database {
            self.health_monitor
                .add_check(Box::new(DatabaseHealthCheck::new(db)))
                .await;
        }

        // Memory health check (warning at 80%, critical at 95%)
        self.health_monitor
            .add_check(Box::new(MemoryHealthCheck::new(80.0, 95.0)))
            .await;

        // Disk health check (warning at 85%, critical at 95%)
        self.health_monitor
            .add_check(Box::new(DiskHealthCheck::new(85.0, 95.0)))
            .await;

        info!("Health checks configured");
        Ok(())
    }

    /// Setup default alert rules
    async fn setup_default_alerts(&self) -> Result<()> {
        // High memory usage alert
        let memory_alert = AlertRule {
            name: "high_memory_usage".to_string(),
            metric_name: "memory_usage_percent".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 85.0,
            duration_seconds: 300, // 5 minutes
            labels: HashMap::new(),
            annotations: {
                let mut annotations = HashMap::new();
                annotations.insert(
                    "summary".to_string(),
                    "High memory usage detected".to_string(),
                );
                annotations.insert(
                    "description".to_string(),
                    "Memory usage is above 85% for more than 5 minutes".to_string(),
                );
                annotations
            },
            enabled: true,
        };
        self.alert_manager.add_alert_rule(memory_alert).await;

        // High error rate alert
        let error_rate_alert = AlertRule {
            name: "high_error_rate".to_string(),
            metric_name: "error_rate_per_minute".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 10.0,
            duration_seconds: 180, // 3 minutes
            labels: HashMap::new(),
            annotations: {
                let mut annotations = HashMap::new();
                annotations.insert(
                    "summary".to_string(),
                    "High error rate detected".to_string(),
                );
                annotations.insert(
                    "description".to_string(),
                    "Error rate is above 10 errors per minute".to_string(),
                );
                annotations
            },
            enabled: true,
        };
        self.alert_manager.add_alert_rule(error_rate_alert).await;

        // Database connection alert
        let db_connection_alert = AlertRule {
            name: "database_connection_failure".to_string(),
            metric_name: "database_connection_failures_total".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 5.0,
            duration_seconds: 60, // 1 minute
            labels: HashMap::new(),
            annotations: {
                let mut annotations = HashMap::new();
                annotations.insert(
                    "summary".to_string(),
                    "Database connection failures".to_string(),
                );
                annotations.insert(
                    "description".to_string(),
                    "Multiple database connection failures detected".to_string(),
                );
                annotations
            },
            enabled: true,
        };
        self.alert_manager.add_alert_rule(db_connection_alert).await;

        info!("Default alert rules configured");
        Ok(())
    }

    /// Start background tasks
    async fn start_background_tasks(&self) -> Result<()> {
        // Start cache cleanup task
        if self.config.enable_caching {
            let cache_manager = Arc::clone(&self.cache_manager);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
                loop {
                    interval.tick().await;
                    cache_manager.cleanup_expired().await;
                }
            });
        }

        // Start alert checking task
        if self.config.enable_alerting {
            let alert_manager = Arc::clone(&self.alert_manager);
            let check_interval = Duration::from_secs(self.config.alert_check_interval_seconds);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(check_interval);
                loop {
                    interval.tick().await;
                    alert_manager.check_alerts().await;
                }
            });
        }

        // Start job queue processing
        if self.config.enable_async_jobs {
            self.job_queue.start_processing().await?;
        }

        // Start concurrency monitoring
        self.lock_manager
            .start_cleanup_task(Duration::from_secs(300));
        self.deadlock_detector
            .start_detection_task(Duration::from_secs(30));

        // Start profiling monitoring
        self.memory_profiler
            .start_monitoring(Duration::from_secs(60));

        // Initialize runtime optimizer with default profiles
        self.runtime_optimizer.create_default_profiles().await;
        self.runtime_optimizer
            .start_monitoring(Duration::from_secs(60));

        // Start metrics collection task
        if self.config.enable_metrics {
            let metrics_collector = Arc::clone(&self.metrics_collector);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60)); // 1 minute
                loop {
                    interval.tick().await;

                    // Collect and record system metrics
                    let system_metrics = metrics_collector.get_system_metrics();

                    let mut labels = HashMap::new();
                    labels.insert("component".to_string(), "system".to_string());

                    metrics_collector.set_gauge(
                        "cpu_usage_percent",
                        system_metrics.cpu_usage_percent,
                        labels.clone(),
                    );
                    metrics_collector.set_gauge(
                        "memory_usage_bytes",
                        system_metrics.memory_usage_bytes as f64,
                        labels.clone(),
                    );
                    metrics_collector.set_gauge(
                        "memory_total_bytes",
                        system_metrics.memory_total_bytes as f64,
                        labels.clone(),
                    );
                    metrics_collector.set_gauge(
                        "disk_usage_bytes",
                        system_metrics.disk_usage_bytes as f64,
                        labels.clone(),
                    );
                    metrics_collector.set_gauge(
                        "disk_total_bytes",
                        system_metrics.disk_total_bytes as f64,
                        labels,
                    );
                }
            });
        }

        info!("Background tasks started");
        Ok(())
    }

    /// Get comprehensive system status
    pub async fn get_system_status(&self) -> SystemStatus {
        let health_response = self.health_monitor.check_health().await;
        let metrics_snapshot = self.metrics_collector.get_metrics_snapshot().await;
        let uptime_seconds = metrics_snapshot.uptime_seconds;
        let queue_stats = self.job_queue.get_queue_stats().await;
        let cache_stats = self.cache_manager.get_all_stats().await;
        let active_alerts = self.alert_manager.get_active_alerts();
        let error_stats = self.error_manager.get_error_stats().await;
        let lock_stats = self.lock_manager.get_active_locks();
        let rate_limit_stats = self.rate_limiter.get_stats();
        let deadlocks = self.deadlock_detector.detect_deadlocks().await;
        let performance_report = self.performance_profiler.generate_report().await;
        let memory_trend = self
            .memory_profiler
            .get_memory_trend(Duration::from_secs(3600))
            .await; // Last hour
        let optimization_report = self.runtime_optimizer.get_performance_report().await;

        SystemStatus {
            health: health_response,
            metrics: metrics_snapshot,
            queue_stats,
            cache_stats,
            active_alerts,
            error_stats,
            uptime_seconds,
            concurrency_stats: ConcurrencyStats {
                active_locks: lock_stats.len(),
                rate_limit_stats,
                detected_deadlocks: deadlocks.len(),
            },
            performance_stats: PerformanceStats {
                active_profiles: performance_report.active_sessions,
                completed_profiles: performance_report.completed_sessions,
                memory_snapshots: memory_trend.len(),
                optimization_report,
            },
        }
    }

    /// Record an application event with full observability
    pub async fn record_event(&self, event: ObservabilityEvent) {
        // Start correlation tracking
        let correlation_id = uuid::Uuid::new_v4();
        self.correlation_tracker
            .start_correlation(correlation_id)
            .await;

        // Record metrics
        let mut labels = HashMap::new();
        labels.insert("event_type".to_string(), event.event_type.clone());
        labels.insert("component".to_string(), event.component.clone());

        self.metrics_collector
            .increment_counter("events_total", labels.clone());

        if let Some(duration) = event.duration {
            self.metrics_collector
                .record_timing("event_duration_seconds", duration, labels);
        }

        // Log the event
        match event.level.as_str() {
            "error" => error!(
                correlation_id = %correlation_id,
                event_type = event.event_type,
                component = event.component,
                message = event.message,
                metadata = ?event.metadata,
                "Application event"
            ),
            "warn" => warn!(
                correlation_id = %correlation_id,
                event_type = event.event_type,
                component = event.component,
                message = event.message,
                metadata = ?event.metadata,
                "Application event"
            ),
            _ => info!(
                correlation_id = %correlation_id,
                event_type = event.event_type,
                component = event.component,
                message = event.message,
                metadata = ?event.metadata,
                "Application event"
            ),
        }

        // Handle errors
        if event.level == "error" {
            if let Some(error_message) = event.error_message {
                let error = AppError::InternalServerError(error_message);
                let context = ErrorContext::new(&event.event_type, &event.component)
                    .with_correlation_id(correlation_id);

                self.error_manager.handle_error(&error, &context).await;
            }
        }

        // End correlation tracking
        self.correlation_tracker.end_correlation().await;
    }

    /// Create HTTP router for observability endpoints
    pub fn create_router(&self) -> axum::Router<Arc<Self>> {
        use axum::{
            routing::{get, post},
            Router,
        };

        Router::new()
            .route("/health", get(health_handler))
            .route("/metrics", get(metrics_handler))
            .route("/status", get(status_handler))
            .route("/alerts", get(alerts_handler))
            .route("/cache/stats", get(cache_stats_handler))
            .route("/jobs/stats", get(job_stats_handler))
            .route("/concurrency/locks", get(locks_handler))
            .route("/concurrency/rate-limits", get(rate_limits_handler))
            .route("/concurrency/deadlocks", get(deadlocks_handler))
            .route("/performance/profile", post(start_profile_handler))
            .route("/performance/report", get(performance_report_handler))
            .route("/memory/snapshot", post(memory_snapshot_handler))
            .route("/memory/trend", get(memory_trend_handler))
            .route(
                "/optimization/apply/:profile",
                post(apply_optimization_handler),
            )
            .route("/optimization/report", get(optimization_report_handler))
    }
}

/// System status response
#[derive(Debug, serde::Serialize)]
pub struct SystemStatus {
    pub health: super::monitoring::HealthResponse,
    pub metrics: crate::core::observability::metrics::MetricsSnapshot,
    pub queue_stats: QueueStats,
    pub cache_stats: HashMap<String, CacheStats>,
    pub active_alerts: Vec<crate::core::observability::metrics::ActiveAlert>,
    pub error_stats: ErrorStats,
    pub uptime_seconds: u64,
    pub concurrency_stats: ConcurrencyStats,
    pub performance_stats: PerformanceStats,
}

#[derive(Debug, serde::Serialize)]
pub struct ConcurrencyStats {
    pub active_locks: usize,
    pub rate_limit_stats: HashMap<String, RateLimitStats>,
    pub detected_deadlocks: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct PerformanceStats {
    pub active_profiles: usize,
    pub completed_profiles: usize,
    pub memory_snapshots: usize,
    pub optimization_report: PerformanceReport,
}

/// Observability event for recording application events
#[derive(Debug, Clone)]
pub struct ObservabilityEvent {
    pub event_type: String,
    pub component: String,
    pub message: String,
    pub level: String, // "info", "warn", "error"
    pub duration: Option<Duration>,
    pub metadata: HashMap<String, String>,
    pub error_message: Option<String>,
}

impl ObservabilityEvent {
    pub fn info(event_type: String, component: String, message: String) -> Self {
        Self {
            event_type,
            component,
            message,
            level: "info".to_string(),
            duration: None,
            metadata: HashMap::new(),
            error_message: None,
        }
    }

    pub fn warn(event_type: String, component: String, message: String) -> Self {
        Self {
            event_type,
            component,
            message,
            level: "warn".to_string(),
            duration: None,
            metadata: HashMap::new(),
            error_message: None,
        }
    }

    pub fn error(
        event_type: String,
        component: String,
        message: String,
        error_message: String,
    ) -> Self {
        Self {
            event_type,
            component,
            message,
            level: "error".to_string(),
            duration: None,
            metadata: HashMap::new(),
            error_message: Some(error_message),
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }
}

// HTTP handlers
async fn health_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<super::monitoring::HealthResponse> {
    let health_response = service.health_monitor.check_health().await;
    axum::response::Json(health_response)
}

async fn metrics_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> String {
    let snapshot = service.metrics_collector.get_metrics_snapshot().await;

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

    output
}

async fn status_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<SystemStatus> {
    let status = service.get_system_status().await;
    axum::response::Json(status)
}

async fn alerts_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<Vec<crate::core::observability::metrics::ActiveAlert>> {
    let alerts = service.alert_manager.get_active_alerts();
    axum::response::Json(alerts)
}

async fn cache_stats_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<HashMap<String, CacheStats>> {
    let stats = service.cache_manager.get_all_stats().await;
    axum::response::Json(stats)
}

async fn job_stats_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<QueueStats> {
    let stats = service.job_queue.get_queue_stats().await;
    axum::response::Json(stats)
}

async fn locks_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<Vec<(String, LockInfo)>> {
    let locks = service.lock_manager.get_active_locks();
    axum::response::Json(locks)
}

async fn rate_limits_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<HashMap<String, RateLimitStats>> {
    let stats = service.rate_limiter.get_stats();
    axum::response::Json(stats)
}

async fn deadlocks_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<Vec<Vec<uuid::Uuid>>> {
    let deadlocks = service.deadlock_detector.detect_deadlocks().await;
    axum::response::Json(deadlocks)
}

async fn start_profile_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
    axum::extract::Json(request): axum::extract::Json<StartProfileRequest>,
) -> axum::response::Json<StartProfileResponse> {
    let profile_id = service
        .performance_profiler
        .start_profile(request.name, request.metadata)
        .await;
    axum::response::Json(StartProfileResponse { profile_id })
}

async fn performance_report_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<super::profiling::PerformanceReport> {
    let report = service.performance_profiler.generate_report().await;
    axum::response::Json(report)
}

async fn memory_snapshot_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<super::profiling::MemorySnapshot> {
    let snapshot = service.memory_profiler.take_snapshot().await;
    axum::response::Json(snapshot)
}

async fn memory_trend_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<Vec<super::profiling::MemorySnapshot>> {
    let trend = service
        .memory_profiler
        .get_memory_trend(Duration::from_secs(3600))
        .await;
    axum::response::Json(trend)
}

async fn apply_optimization_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
    axum::extract::Path(profile_name): axum::extract::Path<String>,
) -> std::result::Result<axum::response::Json<ApplyOptimizationResponse>, axum::http::StatusCode> {
    match service.runtime_optimizer.apply_profile(&profile_name).await {
        Ok(()) => Ok(axum::response::Json(ApplyOptimizationResponse {
            success: true,
            message: format!("Applied optimization profile: {}", profile_name),
        })),
        Err(_) => Err(axum::http::StatusCode::BAD_REQUEST),
    }
}

async fn optimization_report_handler(
    axum::extract::State(service): axum::extract::State<Arc<ObservabilityService>>,
) -> axum::response::Json<PerformanceReport> {
    let report = service.runtime_optimizer.get_performance_report().await;
    axum::response::Json(report)
}

#[derive(serde::Deserialize)]
struct StartProfileRequest {
    name: String,
    metadata: HashMap<String, String>,
}

#[derive(serde::Serialize)]
struct StartProfileResponse {
    profile_id: uuid::Uuid,
}

#[derive(serde::Serialize)]
struct ApplyOptimizationResponse {
    success: bool,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability_service_creation() {
        let config = ObservabilityConfig::default();
        let service = ObservabilityService::new(config, None).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_observability_event_recording() {
        let config = ObservabilityConfig::default();
        let service = ObservabilityService::new(config, None).await.unwrap();

        let event = ObservabilityEvent::info(
            "test_event".to_string(),
            "test_component".to_string(),
            "Test message".to_string(),
        );

        service.record_event(event).await;

        // Verify event was recorded (in a real test, you'd check metrics/logs)
        let status = service.get_system_status().await;
        assert!(status.uptime_seconds > 0);
    }
}
