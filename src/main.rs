use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::signal;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod domain;
mod dto;
mod handlers;
mod services;
mod infrastructure;
mod utils;

use config::Config;
use handlers::oauth;

// Custom error type for better error handling
#[derive(Debug)]
pub enum AppError {
    ConfigError(String),
    ServerError(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            AppError::ServerError(msg) => write!(f, "Server error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// Optimized AppState with Arc for shared ownership
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        debug!("Creating HTTP client with optimizations");
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            http_client,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    init_tracing()?;
    
    info!("Starting DevOps CI Server...");

    // Load configuration with better error handling
    let config = Config::from_env()
        .map_err(|e| AppError::ConfigError(format!("Failed to load configuration: {}", e)))?;
    
    let server_port = config.server_port;
    let app_state = AppState::new(config);
    
    // Build application with middleware stack
    let app = create_app(app_state).await?;

    // Start server with graceful shutdown
    let addr = format!("0.0.0.0:{}", server_port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| AppError::ServerError(format!("Failed to bind to {}: {}", addr, e)))?;
    
    info!("🚀 Server running on http://localhost:{}", server_port);
    
    // Use axum's serve method with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| AppError::ServerError(format!("Server error: {}", e)))?;
    
    info!("Server shutdown complete");
    Ok(())
}

async fn create_app(state: AppState) -> Result<Router, AppError> {
    let router = Router::new()
        .route("/", get(health_check))
        .route("/health", get(detailed_health_check))
        .route("/auth/github", get(oauth::github_login))
        .route("/auth/github/callback", get(oauth::github_callback))
        .route("/auth/user", get(oauth::get_user))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(create_cors_layer())
        .with_state(state);

    Ok(router)
}

fn create_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(tower_http::cors::Any) // Configure this based on your needs
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any)
        .max_age(std::time::Duration::from_secs(3600))
}

fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    // More sophisticated logging setup
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
    
    Ok(())
}

async fn health_check() -> &'static str {
    "DevOps CI Server is running! 🚀"
}

async fn detailed_health_check(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    debug!("Health check requested");
    
    // You can add more health checks here (database, external services, etc.)
    let health_status = json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "server_port": state.config.server_port,
        "environment": std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()),
        "uptime": "TODO: implement uptime tracking"
    });
    
    Ok(Json(health_status))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down gracefully...");
        },
    }
}