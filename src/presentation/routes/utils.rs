use crate::AppState;
use axum::Router;

/// Common utilities for router creation and management
pub struct RouterUtils;

impl RouterUtils {
    /// Create a router with common middleware and error handling
    pub fn create_base_router() -> Router<AppState> {
        Router::new()
        // Add common middleware here if needed
    }

    /// Apply common route patterns and middleware
    pub fn with_common_middleware(router: Router<AppState>) -> Router<AppState> {
        router
        // Add common middleware layers here
        // .layer(middleware::from_fn(logging_middleware))
        // .layer(middleware::from_fn(error_handling_middleware))
    }
}
