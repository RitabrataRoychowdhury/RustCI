//! CI/CD API routes for version 2 (current)
//!
//! This module provides the current V2 API implementation for CI/CD endpoints.

use crate::{
    application::handlers::ci::{
        cancel_execution, create_pipeline, create_pipeline_multipart, get_execution, get_pipeline,
        get_pipeline_yaml, list_executions, list_pipelines, trigger_pipeline, webhook_handler,
    },
    AppState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};

/// V2 CI router (current implementation)
pub fn ci_router_v2() -> Router<AppState> {
    Router::new()
        // V2 pipeline management endpoints
        .route("/pipelines", post(create_pipeline))
        .route("/pipelines/upload", post(create_pipeline_multipart))
        .route("/pipelines", get(list_pipelines))
        .route("/pipelines/:pipeline_id", get(get_pipeline))
        .route("/pipelines/:pipeline_id/yaml", get(get_pipeline_yaml))
        // V2 pipeline execution endpoints
        .route("/pipelines/:pipeline_id/trigger", post(trigger_pipeline))
        .route("/pipelines/:pipeline_id/webhook", post(webhook_handler))
        // V2 execution management endpoints
        .route("/executions", get(list_executions))
        .route("/executions/:execution_id", get(get_execution))
        .route("/executions/:execution_id/cancel", delete(cancel_execution))
        // V2 test endpoint
        .route("/test", get(|| async { "CI Engine V2 is working!" }))
}