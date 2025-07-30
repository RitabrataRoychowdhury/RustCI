use crate::{
    handlers::{
        auth::{logout_handler},
        ci::*,
        pr::*,
    },
    AppState,
};
use axum::{
    routing::{get, post, delete},
    Router,
};

/// Create the main API router with all endpoints
#[allow(dead_code)]
pub fn create_api_router() -> Router<AppState> {
    Router::new()
        // Health check (public)
        .route("/healthchecker", get(crate::health_check_handler))
        
        // Authentication routes (public)
        .nest("/sessions", auth_routes())
        
        // API routes
        .nest("/ci", ci_routes())
        .nest("/pr", pr_routes())
}

/// Authentication routes (public access)
#[allow(dead_code)]
fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/google", get(crate::handlers::auth::google_oauth_handler))
        .route("/oauth/github", get(crate::handlers::auth::github_oauth_handler))
        .route("/oauth/github/callback", get(crate::handlers::auth::github_oauth_callback))
        .route("/me", get(|| async { "User profile endpoint - requires authentication middleware" }))
        .route("/logout", post(logout_handler))
}

/// CI/CD pipeline routes
#[allow(dead_code)]
fn ci_routes() -> Router<AppState> {
    Router::new()
        // Pipeline management
        .route("/pipelines", post(create_pipeline))
        .route("/pipelines/upload", post(create_pipeline_multipart))
        .route("/pipelines", get(list_pipelines))
        .route("/pipelines/:pipeline_id/yaml", get(get_pipeline_yaml))
        
        // Pipeline execution
        .route("/pipelines/:pipeline_id/trigger", post(trigger_pipeline))
        .route("/pipelines/:pipeline_id/webhook", post(webhook_handler))
        
        // Execution management
        .route("/executions", get(list_executions))
        .route("/executions/:execution_id", get(get_execution))
        .route("/executions/:execution_id/cancel", delete(cancel_execution))
        
        // Test endpoint
        .route("/test", get(|| async { "CI Engine is working!" }))
}

/// Pull Request routes
#[allow(dead_code)]
fn pr_routes() -> Router<AppState> {
    Router::new()
        .route("/prs", post(create_pr))
        .route("/prs", get(list_prs))
        .route("/prs/:pr_id", get(get_pr_status))
        .route("/prs/:pr_id/cancel", delete(cancel_pr))
        .route("/prs/webhook", post(pr_webhook_handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_router_creation() {
        // Simple test to ensure the router can be created without panicking
        let _router = create_api_router();
        // If we get here, the router was created successfully
        assert!(true);
    }
}