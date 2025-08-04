//! Control Plane API Routes
//!
//! This module defines the REST API routes for control plane operations,
//! including job scheduling, node management, and metrics collection.

use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    application::handlers::control_plane::{
        get_job_status, get_metrics, get_node_status, register_node, schedule_job,
    },
    AppState,
};

/// Create the control plane router with all endpoints
pub fn control_plane_router() -> Router<AppState> {
    Router::new()
        // Job scheduling endpoints
        .route("/job/schedule", post(schedule_job))
        .route("/job/status/:job_id", get(get_job_status))
        
        // Node management endpoints  
        .route("/node/register", post(register_node))
        .route("/node/status", get(get_node_status))
        
        // Metrics endpoint
        .route("/metrics", get(get_metrics))
}

/// Create a public control plane router (for node registration without auth)
pub fn public_control_plane_router() -> Router<AppState> {
    Router::new()
        // Node registration endpoint (public for initial node registration)
        .route("/node/register", post(register_node))
        
        // Public metrics endpoint (basic metrics without sensitive data)
        .route("/metrics/public", get(get_metrics))
}

/// Create the complete control plane router with both authenticated and public routes
pub fn complete_control_plane_router() -> Router<AppState> {
    Router::new()
        // Public routes (no authentication required)
        .nest("/public", public_control_plane_router())
        
        // Authenticated routes (authentication applied at main router level)
        .merge(control_plane_router())
}