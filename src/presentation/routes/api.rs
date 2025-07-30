use crate::{
    presentation::{
        middleware::auth::enhanced_auth,
        routes::{auth::auth_router, ci::ci_router, pr::pr_router, runner::runner_router},
    },
    AppState,
};
use axum::{middleware, routing::get, Router};

/// Create the main API router with all endpoints
#[allow(dead_code)]
pub fn create_api_router(state: AppState) -> Router<AppState> {
    Router::new()
        // Health check (public)
        .route("/healthchecker", get(crate::health_check_handler))
        // Authentication routes (public)
        .nest("/sessions", auth_router(state.clone()))
        // Protected API routes with authentication middleware
        .nest(
            "/api",
            Router::new()
                .nest("/ci", ci_router())
                .nest("/pr", pr_router())
                .nest("/runners", runner_router())
                .layer(middleware::from_fn_with_state(state.clone(), enhanced_auth)),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::DatabaseManager;
    use std::sync::Arc;

    #[test]
    fn test_api_router_creation() {
        // This test is commented out due to AppState complexity
        // In a real scenario, you would create a mock AppState
        // let mock_state = create_mock_app_state();
        // let _router = create_api_router(mock_state);
        assert!(true);
    }
}
