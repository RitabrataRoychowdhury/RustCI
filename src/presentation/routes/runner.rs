use crate::{
    application::handlers::runner::{
        deregister_runner, fetch_job_artifacts, fetch_job_logs, get_runner_status, list_runners,
        register_runner, trigger_job_on_runner,
    },
    AppState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};

/// Create the runner management router with authentication
pub fn runner_router() -> Router<AppState> {
    Router::new()
        // Runner management endpoints
        .route("/", post(register_runner))
        .route("/", get(list_runners))
        .route("/:runner_id", delete(deregister_runner))
        .route("/:runner_id/status", get(get_runner_status))
        // Job management endpoints
        .route("/:runner_id/jobs", post(trigger_job_on_runner))
        .route("/:runner_id/jobs/:job_id/logs", get(fetch_job_logs))
        .route(
            "/:runner_id/jobs/:job_id/artifacts",
            get(fetch_job_artifacts),
        )
    // Authentication middleware will be applied at the parent router level
}
