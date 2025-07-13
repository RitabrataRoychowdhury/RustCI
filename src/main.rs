use axum::{
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use dotenv::dotenv;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::signal;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, debug, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod database;
mod error;
mod token;
mod models;
mod middleware;
mod handlers;
mod routes;
mod ci;

use config::Config;
use database::DatabaseManager;
use routes::{auth_router, ci_router};
use ci::{
    engine::CIEngine, 
    executor::PipelineExecutor, 
    workspace::WorkspaceManager, 
    connectors::ConnectorManager
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Config>,
    pub db: Arc<DatabaseManager>,
    pub ci_engine: Arc<CIEngine>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv().ok();

    // Initialize structured logging
    init_tracing()?;
    info!("🚀 Starting DevOps CI Server...");

    // Load configuration
    let config = Config::init();
    info!("✅ Configuration loaded successfully");
    info!("🌐 Server will run on port: {}", config.port);

    // Initialize database connection
    let db = DatabaseManager::new(&config.mongodb_uri, &config.mongodb_database)
        .await
        .map_err(|e| {
            error!("❌ Database connection failed: {}", e);
            e
        })?;
    info!("✅ Database connection established");

    // Initialize CI engine components
    let connector_manager = Arc::new(ConnectorManager::new());
    let workspace_manager = Arc::new(WorkspaceManager::new("/tmp/ci-workspaces".into()));
    let executor = Arc::new(PipelineExecutor::new(
        connector_manager,
        "/tmp/ci-cache".into(),
        "/tmp/ci-deployments".into(),
    ));
    let ci_engine = Arc::new(CIEngine::new(Arc::new(db.clone()), workspace_manager, executor));
    info!("✅ CI engine initialized");

    // Create application state
    let app_state = AppState {
        env: Arc::new(config.clone()),
        db: Arc::new(db),
        ci_engine,
    };

    // Build application
    let app = create_app(app_state).await?;

    // Start server with graceful shutdown
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    info!("🚀 Server running on http://{}", addr);
    info!("🔗 OAuth Login: http://{}/api/sessions/oauth/google", addr);
    info!("🔗 GitHub OAuth: http://{}/api/sessions/oauth/github", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    info!("✅ Server shutdown complete");
    Ok(())
}

async fn create_app(state: AppState) -> Result<Router, Box<dyn std::error::Error>> {
    let router = Router::new()
        .route("/api/healthchecker", get(health_check_handler))
        .nest("/api/sessions", auth_router(state.clone()))
        .nest("/api/ci", ci_router())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(create_cors_layer(&state.env.client_origin))
        .with_state(state);

    Ok(router)
}

fn create_cors_layer(client_origin: &str) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(client_origin.parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_credentials(true)
        .allow_headers([
            axum::http::header::ACCEPT,
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
        .max_age(std::time::Duration::from_secs(3600))
}

fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug,mongodb=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
    Ok(())
}

async fn health_check_handler(axum::extract::State(state): axum::extract::State<AppState>) -> Result<Json<Value>, StatusCode> {
    debug!("🔍 Health check requested");

    // Ping MongoDB
    let db_status = match state.db.database.run_command(mongodb::bson::doc! {"ping": 1}, None).await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    let health_status = json!({
        "status": "success",
        "message": "DevOps CI Server is running! 🚀",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "database": {
            "status": db_status,
            "type": "MongoDB"
        },
        "environment": std::env::var("RUST_ENV").unwrap_or_else(|_| "development".into()),
        "endpoints": {
            "oauth_login": "/api/sessions/oauth/google",
            "github_oauth": "/api/sessions/oauth/github",
            "github_callback": "/api/sessions/oauth/github/callback",
            "user_profile": "/api/sessions/me",
            "logout": "/api/sessions/logout"
        }
    });

    info!("✅ Health check completed - Database: {}", db_status);
    Ok(Json(health_status))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("📡 Received Ctrl+C, shutting down gracefully..."),
        _ = terminate => info!("📡 Received terminate signal, shutting down gracefully..."),
    }
}
