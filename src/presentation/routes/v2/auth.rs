//! Authentication API routes for version 2 (current)
//!
//! This module provides the current V2 API implementation for authentication endpoints.

use crate::{
    application::handlers::auth::{
        get_me_handler, github_oauth_callback, github_oauth_handler, google_oauth_handler,
        logout_handler,
    },
    presentation::middleware::auth,
    AppState,
};
use axum::{
    extract::{Extension, State},
    middleware,
    routing::get,
    Router,
};
use uuid::Uuid;

/// V2 authentication router (current implementation)
pub fn auth_router_v2(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/oauth/google", get(google_oauth_handler))
        .route("/oauth/github", get(github_oauth_handler))
        .route("/oauth/github/callback", get(github_oauth_callback))
        .route("/logout", get(logout_handler))
        .route(
            "/me",
            get(
                |Extension(user_id): Extension<Uuid>, state: State<AppState>| {
                    get_me_handler(user_id, state)
                },
            )
            .route_layer(middleware::from_fn_with_state(state.clone(), auth)),
        )
        .with_state(state)
}