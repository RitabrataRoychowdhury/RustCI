mod config;
mod database;
mod error;
mod handlers;
mod middleware;
mod models;
mod routes;
mod token;

use axum::{
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    response::Json,
    routing::get,
    Router,
};
use config::Config;
use database::DatabaseManager;
use dotenv::dotenv;
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Config>,
    pub db: Arc<DatabaseManager>,
}

async fn health_checker_handler() -> Json<serde_json::Value> {
    Json(json!({
        "status": "success",
        "message": "DevOps CI Server is running! 🚀",
        "database": "MongoDB connected",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::init();

    // Initialize MongoDB connection
    let db = DatabaseManager::new(&config.mongodb_uri, &config.mongodb_database)
        .await
        .expect("Failed to connect to MongoDB");

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let app_state = AppState {
        env: Arc::new(config.clone()),
        db: Arc::new(db),
    };

    let app = Router::new()
        .route("/api/healthchecker", get(health_checker_handler))
        .nest("/api/sessions", routes::auth_router())
        .layer(cors)
        .with_state(app_state);

    println!("🚀 Server started successfully on port {}", config.port);
    println!("🔗 GitHub OAuth URL: http://localhost:{}/api/sessions/oauth/github", config.port);
    println!("📊 Health check: http://localhost:{}/api/healthchecker", config.port);
    println!("🗄️  MongoDB database: {}", config.mongodb_database);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    axum::serve(listener, app).await?;

    Ok(())
}