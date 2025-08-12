use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Json,
    routing::get,
    Router,
};
use dotenv::dotenv;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::signal;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

mod ci;
mod config;
mod core;
mod error;
#[cfg(test)]
mod integration_tests;
mod service_registry;

mod token;
mod upload;
mod valkyrie;

// Clean Architecture Layers
mod application;
mod domain;
mod infrastructure;
mod presentation;

use config::{AppConfiguration, HotReloadConfigManager};
use core::networking::security::AuditLogger;
use infrastructure::database::DatabaseManager;
use presentation::routes::{auth_router, ci_router, complete_control_plane_router, pr_router};
use presentation::swagger::ApiDoc;
use utoipa::OpenApi;
use core::observability::{
    audit::{AuditConfig, EnhancedAuditLogger},
    monitoring::HealthStatus,
    observability::{ObservabilityConfig, ObservabilityService},
};
use presentation::middleware::{comprehensive_security_middleware, create_cors_middleware};

// CI Engine imports
use ci::engine::{
    CIEngineOrchestrator, ExecutionCoordinator, ExecutionMonitoring, PipelineManager,
    PipelineExecutionSagaFactory, MetricsCollector,
};
use core::{
    patterns::{
        events::EventBus,
        sagas::{SagaOrchestrator, SagaPersistence, SagaExecution, SagaStatus, SagaStatistics},
        correlation::CorrelationTracker,
    },
};
use error::Result;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub env: Arc<AppConfiguration>,
    pub db: Arc<DatabaseManager>,
    pub audit_logger: Option<Arc<dyn AuditLogger>>,
    pub config_manager: Arc<tokio::sync::RwLock<HotReloadConfigManager>>,
    pub observability: Arc<ObservabilityService>,
    pub ci_engine: Arc<CIEngineOrchestrator>,
}

// Simple in-memory saga persistence for development
#[derive(Debug)]
struct InMemorySagaPersistence {
    executions: Arc<tokio::sync::RwLock<std::collections::HashMap<Uuid, SagaExecution>>>,
}

impl InMemorySagaPersistence {
    fn new() -> Self {
        Self {
            executions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl SagaPersistence for InMemorySagaPersistence {
    async fn save_execution(&self, execution: &SagaExecution) -> Result<()> {
        let mut executions = self.executions.write().await;
        executions.insert(execution.saga_id, execution.clone());
        Ok(())
    }

    async fn load_execution(&self, saga_id: Uuid) -> Result<Option<SagaExecution>> {
        let executions = self.executions.read().await;
        Ok(executions.get(&saga_id).cloned())
    }

    async fn find_by_status(&self, status: SagaStatus) -> Result<Vec<SagaExecution>> {
        let executions = self.executions.read().await;
        Ok(executions
            .values()
            .filter(|e| e.status == status)
            .cloned()
            .collect())
    }

    async fn find_by_correlation_id(&self, correlation_id: Uuid) -> Result<Vec<SagaExecution>> {
        let executions = self.executions.read().await;
        Ok(executions
            .values()
            .filter(|e| e.correlation_id == correlation_id)
            .cloned()
            .collect())
    }

    async fn delete_execution(&self, saga_id: Uuid) -> Result<()> {
        let mut executions = self.executions.write().await;
        executions.remove(&saga_id);
        Ok(())
    }

    async fn get_statistics(&self) -> Result<SagaStatistics> {
        let executions = self.executions.read().await;
        let total = executions.len();
        let completed = executions.values().filter(|e| e.status == SagaStatus::Completed).count();
        let failed = executions.values().filter(|e| e.status == SagaStatus::Failed).count();
        let compensated = executions.values().filter(|e| e.status == SagaStatus::Compensated).count();
        
        Ok(SagaStatistics {
            total_executions: total as u64,
            completed_executions: completed as u64,
            failed_executions: failed as u64,
            compensated_executions: compensated as u64,
            average_duration_ms: 0.0, // Would calculate from actual data
            success_rate: if total > 0 { completed as f64 / total as f64 } else { 0.0 },
        })
    }
}



#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv().ok();

    // Initialize structured logging
    init_tracing()?;
    info!("🚀 Starting RustCI Server...");

    // Load configuration with enhanced hot-reload config manager
    let mut config_manager = HotReloadConfigManager::new();
    config_manager.load().await?;
    let config = config_manager.get().await;

    // Validate configuration before starting
    let validation_engine = config::validation_engine::ConfigValidationEngine::new(
        &std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
    );
    let validation_report = validation_engine.validate(&config).await?;

    if !validation_report.is_valid() {
        error!("❌ Configuration validation failed:");
        for (validator, report) in &validation_report.validator_reports {
            for (error_key, error_msg) in &report.errors {
                error!("  {}.{}: {}", validator, error_key, error_msg);
            }
        }
        return Err("Configuration validation failed".to_string().into());
    }

    if validation_report.total_warnings() > 0 {
        for (validator, report) in &validation_report.validator_reports {
            for (warning_key, warning_msg) in &report.warnings {
                warn!("{}.{}: {}", validator, warning_key, warning_msg);
            }
        }
    }

    // Start hot-reload if enabled
    if config.features.enable_hot_reload {
        config_manager.start_hot_reload().await?;
    }

    // Initialize database connection
    let db = DatabaseManager::new(&config.database.mongodb_uri, &config.database.database_name)
        .await
        .map_err(|e| {
            error!("❌ Database connection failed: {}", e);
            e
        })?;

    // Initialize enhanced audit logger
    let audit_logger: Option<Arc<dyn AuditLogger>> = if config.features.enable_audit_logging {
        let audit_config = AuditConfig {
            buffer_size: 1000,
            flush_interval_seconds: 30,
            enable_real_time_alerts: true,
            retention_days: config.security.audit.retention_days,
            sensitive_fields: config.security.audit.sensitive_fields.clone(),
            ..Default::default()
        };
        Some(Arc::new(EnhancedAuditLogger::new(audit_config)))
    } else {
        None
    };

    // Initialize observability service
    let observability_config = ObservabilityConfig {
        enable_metrics: config.features.enable_metrics_collection,
        enable_tracing: config.features.enable_distributed_tracing,
        enable_health_checks: true,
        enable_alerting: true,
        enable_caching: true,
        enable_async_jobs: true,
        metrics_endpoint: "0.0.0.0:9090".to_string(),
        health_endpoint: "/health".to_string(),
        max_concurrent_jobs: 10,
        cache_ttl_seconds: 300,
        alert_check_interval_seconds: 60,
    };

    let observability = ObservabilityService::new(
        observability_config,
        Some(Arc::new(db.get_database().clone())),
    )
    .await?;

    // Initialize CI Engine components
    let correlation_tracker = Arc::new(CorrelationTracker::new());
    let event_bus = Arc::new(EventBus::new(correlation_tracker.clone(), None));
    
    // Create a simple in-memory saga persistence for now
    let saga_persistence = Arc::new(InMemorySagaPersistence::new());
    let saga_orchestrator = Arc::new(SagaOrchestrator::new(
        "pipeline-execution".to_string(),
        event_bus.clone(),
        correlation_tracker.clone(),
        saga_persistence,
    ));

    // Initialize CI engine components
    let pipeline_manager = Arc::new(PipelineManager::new(
        db.clone(),
        event_bus.clone(),
    ));
    
    let monitoring = Arc::new(ExecutionMonitoring::new(correlation_tracker.clone()));
    
    // Create workspace and executor for saga factory
    let workspace_manager = Arc::new(ci::workspace::WorkspaceManager::new(
        std::path::PathBuf::from("/tmp/rustci/workspaces"),
    ));
    let connector_manager = ci::connectors::ConnectorManager::new();
    let executor = Arc::new(ci::executor::PipelineExecutor::new(
        connector_manager,
        std::path::PathBuf::from("/tmp/rustci/cache"),
        std::path::PathBuf::from("/tmp/rustci/deploy"),
    ));
    let metrics_collector = Arc::new(MetricsCollector::new());

    // Initialize CI engine components with strategies
    let execution_coordinator = Arc::new(ExecutionCoordinator::with_strategies(
        correlation_tracker.clone(),
        executor.clone(),
        workspace_manager.clone(),
    ));
    
    let saga_factory = Arc::new(PipelineExecutionSagaFactory::new(
        executor,
        workspace_manager,
        monitoring.clone(),
        metrics_collector,
    ));

    // Create CI Engine Orchestrator
    let ci_engine = Arc::new(CIEngineOrchestrator::new(
        pipeline_manager,
        execution_coordinator,
        event_bus,
        saga_orchestrator,
        saga_factory,
        monitoring,
    ));

    // Create application state
    let app_state = AppState {
        env: Arc::new(config.clone()),
        db: Arc::new(db),
        audit_logger,
        config_manager: Arc::new(tokio::sync::RwLock::new(config_manager)),
        observability: Arc::new(observability),
        ci_engine,
    };

    // Build application
    let app = create_app(app_state).await?;

    // Start server with graceful shutdown
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    println!("🚀 RustCI Server running on http://{}", addr);
    println!("🔗 GitHub OAuth: http://{}/api/sessions/oauth/github", addr);
    println!("🔗 Google OAuth: http://{}/api/sessions/oauth/google", addr);
    println!("📊 Health Check: http://{}/health", addr);
    println!("📚 Swagger UI: http://{}/swagger-ui", addr);
    println!("📋 OpenAPI Spec: http://{}/api-docs/openapi.json", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    info!("✅ Server shutdown complete");
    Ok(())
}

async fn create_app(state: AppState) -> std::result::Result<Router, Box<dyn std::error::Error>> {
    // Create CORS middleware from configuration
    let cors = create_cors_middleware(&state.env.security.cors);

    let router = Router::new()
        .route("/api/healthchecker", get(health_check_handler))
        .route("/health", get(health_check_handler))
        .nest("/api/sessions", auth_router(state.clone()))
        .nest("/api/ci", ci_router())
        .nest("/api/pr", pr_router())
        .nest("/api/control-plane", complete_control_plane_router())
        // Add observability endpoints
        .nest(
            "/observability",
            state
                .observability
                .create_router()
                .with_state(state.observability.clone()),
        )
        // Add OpenAPI spec endpoint
        .route("/api-docs/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .with_state(state.clone())
        // Enhanced middleware pipeline with comprehensive security
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            |state: State<AppState>, req: Request, next: Next| async move {
                comprehensive_security_middleware(state, req, next).await
            },
        ))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    Ok(router)
}

fn init_tracing() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Use the new structured logging system
    Ok(crate::core::observability::logging::init_structured_logging()?)
}

async fn health_check_handler(State(state): State<AppState>) -> std::result::Result<Json<Value>, StatusCode> {
    // Use the observability service for comprehensive health check
    let health_response = state.observability.health_monitor.check_health().await;

    let health_status = json!({
        "status": match health_response.status {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        },
        "message": "DevOps CI Server health check",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": health_response.uptime_seconds,
        "checks": health_response.checks,
        "system_info": health_response.system_info,
        "environment": std::env::var("RUST_ENV").unwrap_or_else(|_| "development".into()),
        "endpoints": {
            "oauth_login": "/api/sessions/oauth/google",
            "github_login": "/api/sessions/oauth/github",
            "github_callback": "/api/sessions/oauth/github/callback",
            "user_profile": "/api/sessions/me",
            "logout": "/api/sessions/logout",
            "metrics": "/metrics",
            "health": "/health",
            "status": "/status",
            "swagger_ui": "/swagger-ui",
            "openapi_spec": "/api-docs/openapi.json"
        }
    });

    let status_code = match health_response.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    // Health check completed

    match status_code {
        StatusCode::OK => Ok(Json(health_status)),
        _ => Err(status_code),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("🛑 Received Ctrl+C, shutting down gracefully..."),
        _ = terminate => info!("🛑 Received terminate signal, shutting down gracefully..."),
    }
}
