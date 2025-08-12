use crate::{
    application::handlers::pr::{
        cancel_pr, create_pr, get_pr_status, list_prs, pr_webhook_handler,
    },
    AppState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};

/// Create the pull request management router
/// 
/// This router handles all PR-related endpoints including creation,
/// status updates, and management operations.
pub fn pr_router() -> Router<AppState> {
    Router::new()
        // PR management
        .route("/prs", post(create_pr))
        .route("/prs", get(list_prs))
        .route("/prs/:pr_id", get(get_pr_status))
        .route("/prs/:pr_id/cancel", delete(cancel_pr))
        // Webhook handling
        .route("/prs/webhook", post(pr_webhook_handler))
}
