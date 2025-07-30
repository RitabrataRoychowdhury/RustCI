use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    timestamp: String,
    version: String,
    uptime_seconds: u64,
    service: String,
}

#[derive(Serialize, Deserialize)]
struct InfoResponse {
    app_name: String,
    version: String,
    description: String,
    endpoints: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct EchoRequest {
    message: String,
}

#[derive(Serialize, Deserialize)]
struct EchoResponse {
    id: String,
    original_message: String,
    echo: String,
    timestamp: String,
}

#[derive(Deserialize)]
struct QueryParams {
    name: Option<String>,
    count: Option<u32>,
}

static START_TIME: std::sync::LazyLock<std::time::Instant> = 
    std::sync::LazyLock::new(std::time::Instant::now);

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("test_app=info,tower_http=debug")
        .init();

    info!("Starting RustCI Test Application");

    // Build our application with routes
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/info", get(info))
        .route("/echo", post(echo))
        .route("/greet", get(greet))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    // Run it with hyper on localhost:8080
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Test application listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> Json<InfoResponse> {
    Json(InfoResponse {
        app_name: "RustCI Test Application".to_string(),
        version: "1.0.0".to_string(),
        description: "Simple test application for RustCI end-to-end testing".to_string(),
        endpoints: vec![
            "/".to_string(),
            "/health".to_string(),
            "/info".to_string(),
            "/echo".to_string(),
            "/greet".to_string(),
        ],
    })
}

async fn health_check() -> Json<HealthResponse> {
    let uptime = START_TIME.elapsed().as_secs();
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: "1.0.0".to_string(),
        uptime_seconds: uptime,
        service: "rustci-test-app".to_string(),
    })
}

async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        app_name: "RustCI Test Application".to_string(),
        version: "1.0.0".to_string(),
        description: "This is a simple test application designed for RustCI end-to-end testing. It provides basic HTTP endpoints for health checks and functionality verification.".to_string(),
        endpoints: vec![
            "GET / - Application information".to_string(),
            "GET /health - Health check endpoint".to_string(),
            "GET /info - Detailed application information".to_string(),
            "POST /echo - Echo service for testing".to_string(),
            "GET /greet?name=<name>&count=<count> - Greeting service".to_string(),
        ],
    })
}

async fn echo(Json(payload): Json<EchoRequest>) -> Result<Json<EchoResponse>, StatusCode> {
    if payload.message.is_empty() {
        warn!("Empty message received in echo request");
        return Err(StatusCode::BAD_REQUEST);
    }

    let response = EchoResponse {
        id: Uuid::new_v4().to_string(),
        original_message: payload.message.clone(),
        echo: format!("Echo: {}", payload.message),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    info!("Echo request processed: {}", payload.message);
    Ok(Json(response))
}

async fn greet(Query(params): Query<QueryParams>) -> Json<HashMap<String, String>> {
    let name = params.name.unwrap_or_else(|| "World".to_string());
    let count = params.count.unwrap_or(1);
    
    let mut response = HashMap::new();
    response.insert("service".to_string(), "rustci-test-app".to_string());
    response.insert("name".to_string(), name.clone());
    response.insert("count".to_string(), count.to_string());
    
    let mut greetings = Vec::new();
    for i in 1..=count {
        greetings.push(format!("Hello, {} ({})", name, i));
    }
    
    response.insert("greetings".to_string(), greetings.join(", "));
    response.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
    
    info!("Greeting request: name={}, count={}", name, count);
    Json(response)
}