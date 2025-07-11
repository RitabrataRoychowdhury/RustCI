use crate::{
    handlers::ci::{
        create_pipeline, trigger_pipeline, get_execution, cancel_execution,
        list_pipelines, list_executions, get_pipeline_yaml, webhook_handler,
    },
    AppState,
};
use axum::{
    routing::{get, post, delete},
    Router,
};

pub fn ci_router() -> Router<AppState> {
    Router::new()
        // Pipeline management
        .route("/pipelines", post(create_pipeline))
        .route("/pipelines", get(list_pipelines))
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
}