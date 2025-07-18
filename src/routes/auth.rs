// routes/auth.rs

use crate::{
    handlers::{
        get_me_handler, github_oauth_callback, github_oauth_handler, google_oauth_handler,
        logout_handler,
    },
    middleware::auth,
    AppState,
};
use axum::{
    extract::{Extension, State},
    middleware,
    routing::get,
    Router,
};
use uuid::Uuid;

/// Accepts AppState from main.rs instead of constructing it here
pub fn auth_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/oauth/google", get(google_oauth_handler))
        .route("/oauth/github", get(github_oauth_handler))
        .route("/oauth/github/callback", get(github_oauth_callback))
        .route("/logout", get(logout_handler))
        .route(
            "/me",
            get(|Extension(user_id): Extension<Uuid>, state: State<AppState>| {
                get_me_handler(user_id, state)
            })
            .route_layer(middleware::from_fn_with_state(state.clone(), auth)),
        )
        .with_state(state) // ✅ Add this to make it Router<AppState>
}

