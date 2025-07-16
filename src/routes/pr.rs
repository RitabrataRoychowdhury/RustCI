use crate::{
    handlers::pr::{
        create_pr, get_pr_status, cancel_pr, list_prs, pr_webhook_handler,
    },
    AppState,
};
use axum::{
    routing::{get, post, delete},
    Router,
};

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