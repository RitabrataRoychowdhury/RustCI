use crate::{
    application::handlers::ci::{
        cancel_execution, create_pipeline, create_pipeline_multipart, get_execution, get_pipeline,
        get_pipeline_yaml, list_executions, list_pipelines, trigger_pipeline, webhook_handler,
    },
    core::security::input_validation_middleware::input_validation_middleware,
    AppState,
};
use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};

/// Create the CI/CD pipeline management router
///
/// This router handles all CI/CD pipeline endpoints including pipeline
/// execution, status monitoring, and configuration management.
pub fn ci_router() -> Router<AppState> {
    Router::new()
        // Pipeline management
        .route("/pipelines", post(create_pipeline))
        .route("/pipelines/upload", post(create_pipeline_multipart))
        .route("/pipelines", get(list_pipelines))
        .route("/pipelines/:pipeline_id", get(get_pipeline))
        .route("/pipelines/:pipeline_id/yaml", get(get_pipeline_yaml))
        // Pipeline execution
        .route("/pipelines/:pipeline_id/trigger", post(trigger_pipeline))
        .route("/pipelines/:pipeline_id/webhook", post(webhook_handler))
        // Execution management
        .route("/executions", get(list_executions))
        .route("/executions/:execution_id", get(get_execution))
        .route("/executions/:execution_id/cancel", delete(cancel_execution))
        // Add a simple test endpoint
        .route("/test", get(|| async { "CI Engine is working!" }))
        // Apply input validation middleware to all routes
        .layer(middleware::from_fn(input_validation_middleware))
}
